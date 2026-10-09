"""Lists the effects of the scenes of her skills: for each state's update, phase by
phase, what is made, where it is put and when it is ended, with every argument.

Each skill's code unit has its own copies of the same small helpers; one is named
here by what it calls (see `KINDS`). The value a "make" returns is followed through
the registers and through the words of the state it is kept in, so that every later
call says which effect it is for.

Usage: skill_effects.py                 every skill
       skill_effects.py <skill number>  one of them
       skill_effects.py --helpers       the helpers of each unit, as they are named
"""
import struct
import sys

import mips
from skill_states import CASES, FIRST, NONE, REG, TABLE, update_of
from skill_unit import ENGINE, PLAYER_STATES, callees, switch_of

# The makers of the states of her attack skills, by the skill's number.
MAKERS = {
    1: 0x7680c, 2: 0x76934, 3: 0x7512c, 4: 0x77b78, 5: 0x77cd4, 6: 0x7521c, 7: 0x7fb14, 8: 0x7fc20,
    10: 0x7c270, 11: 0x7c380, 12: 0x7c490, 14: 0x7ad34, 15: 0x7ae48, 18: 0x798f4, 19: 0x79a34,
    20: 0x849d4, 22: 0x84ad8,
}
ALLOCATE = 0xa7fdc
WAIT, RESTART = 0x744b0, 0x744a4
ARGS = [REG[name] for name in ('a1', 'a2', 'a3', 't0', 't1')]
SAVED = [REG[name] for name in 's0 s1 s2 s3 s4 s5 s6 s7 sp fp gp ra zero'.split()]


def kind_of(target, cache={}):
    """What a helper of a unit does with an effect, by what it calls."""
    if target not in PLAYER_STATES:
        return None
    if target not in cache:
        called = set(callees(target))
        if 0x9e5d4 in called:
            kind = 'make'
        elif 0x9e9f4 in called:
            kind = 'end all of'
        elif 0x9e62c in called or 0xa6020 in called:
            kind = 'end'
        elif 0xacdd0 in called:
            kind = 'layer'
        elif 0xacd14 in called:
            kind = 'goes with node' if 0xa74a8 in called else 'goes with'
        elif 0xace9c in called and 0xacecc in called and 0xa74a8 in called and 0x24020 not in called:
            kind = 'left where node is'
        elif 0xace9c in called and 0x23f6c in called:
            kind = 'moved by'
        elif 0xace9c in called and 0x240c0 in called:
            kind = 'turned by (degrees)'
        elif 0xace9c in called and 0xa7ba4 not in called and 0x24020 not in called:
            kind = 'placed (' + ', '.join(f'{c:#x}' for c in sorted(called)) + ')'
        elif 0xacda0 in called and 0xa7ba4 not in called and 0xa7b20 not in called:
            kind = 'shown or not'
        else:
            kind = None
        cache[target] = kind
    return cache[target]


def follow(vaddr, known, memory):
    """One instruction's effect on what the registers are known to hold: constants,
    and effects by the number they were made with."""
    w = mips.word(vaddr)
    op, rs, rt, rd, imm, funct = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, w & 0xffff, w & 0x3f
    simm = imm - 0x10000 if imm & 0x8000 else imm
    number = lambda r: known[r] if isinstance(known.get(r), int) else None
    if op == 0x0f:
        known[rt] = imm << 16
    elif op == 0x0d:
        known[rt] = (number(rs) | imm) if number(rs) is not None else None
    elif op == 0x09:
        known[rt] = ((number(rs) + simm) & 0xffffffff) if number(rs) is not None else None
    elif op == 0 and funct in (0x21, 0x25) and (rt == 0 or rs == 0):  # move
        known[rd] = known.get(rs if rt == 0 else rt)
    elif op == 0 and rd:
        known[rd] = None
    elif op == 0x2b:  # sw: an effect kept in the state
        if isinstance(known.get(rt), tuple):
            memory[imm] = known[rt]
        else:
            memory.pop(imm, None)
    elif op == 0x23:  # lw
        known[rt] = memory.get(imm) if rs != REG['sp'] else None
    elif op in (0x21, 0x25, 0x20, 0x24):
        known[rt] = None
    for register in [r for r, value in known.items() if value is None]:
        del known[register]


def shown(value):
    if value is None:
        return '?'
    if isinstance(value, tuple):
        return f'effect {value[1]}'
    if mips.is_string(value):
        return f'"{mips.cstring(value)}"'
    return str(value - (1 << 32)) if value >= 0x80000000 else str(value)


def walk(start, end, memory):
    """The calls between two addresses that have to do with an effect, and the waits."""
    known, last_float, found = {0: 0}, None, []
    vaddr = start
    while vaddr < end:
        w = mips.word(vaddr)
        if w >> 26 == 0x0f and (w & 0xffff) >= 0x3000:
            last_float = struct.unpack('<f', struct.pack('<I', (w & 0xffff) << 16))[0]
        if w >> 26 != 3:
            follow(vaddr, known, memory)
            vaddr += 4
            continue
        follow(vaddr + 4, known, memory)
        target = (w & 0x3ffffff) << 2
        values = [known.get(r) for r in ARGS]
        made = None
        kind = kind_of(target)
        if target == WAIT:
            found.append((vaddr, 'wait', last_float))
        elif target == RESTART:
            found.append((vaddr, 'restart', None))
        elif kind == 'make':
            made = ('fx', values[0], vaddr)
            found.append((vaddr, 'make', shown(values[0])))
        elif kind:
            count = {'layer': 2, 'goes with node': 2, 'left where node is': 2, 'moved by': 4, 'turned by (degrees)': 4, 'end all of': 1, 'end': 1}.get(kind, 4)
            found.append((vaddr, kind, ', '.join(shown(v) for v in values[:count])))
        elif any(isinstance(v, tuple) for v in values + [known.get(REG['a0'])]):
            name = ENGINE.get(target, f'{target:#x}')
            found.append((vaddr, f'{name}', ', '.join(shown(v) for v in [known.get(REG['a0'])] + values[:3])))
        for register in list(known):
            if register not in SAVED:
                del known[register]
        if made:
            known[REG['v0']] = made
        vaddr += 8
    return found


def describe(update, memory):
    end = mips.function_bounds(update)[1]
    cases = switch_of(update)
    if not cases:
        # One that is a row of steps, each behind its own wait.
        time = None
        for vaddr, what, detail in walk(update, end, memory):
            if what == 'wait':
                time = detail
            elif what == 'restart':
                time = None
                print(f'      {vaddr:#x}  the clock starts over')
            else:
                print(f'      {vaddr:#x}  ' + ('' if time is None else f'at {time:g}, ') + f'{what}: {detail}')
        return
    places = sorted(set(cases))
    for place in places:
        if not update <= place < end:
            continue
        after = [other for other in places if other > place]
        phases = ', '.join(str(i) for i, case in enumerate(cases or []) if case == place)
        lines, time = [], None
        for vaddr, what, detail in walk(place, after[0] if after else end, memory):
            if what == 'wait':
                time = detail
            elif what == 'restart':
                lines.append(f'      {vaddr:#x}  the clock starts over')
            else:
                lines.append(f'      {vaddr:#x}  {what}: {detail}')
        if any('clock' not in line for line in lines):
            print(f'   phase {phases} ({place:#x})' + ('' if time is None else f', at {time:g}'))
            print('\n'.join(lines))


def of_maker(maker):
    made = [(mips.word(v) & 0x3ffffff) << 2 for v in range(maker, mips.function_bounds(maker)[1], 4) if mips.word(v) >> 26 == 3]
    constructor = next(target for target in made if target != ALLOCATE)
    update, end, _ = update_of(constructor)
    return constructor, update, end


def helpers(unit_of):
    """The helpers the updates call, by unit."""
    seen = {}
    for skill, update in sorted(unit_of.items()):
        for vaddr in range(update, mips.function_bounds(update)[1], 4):
            w = mips.word(vaddr)
            target = (w & 0x3ffffff) << 2
            if w >> 26 == 3 and kind_of(target):
                seen.setdefault(target, (kind_of(target), set()))[1].add(skill)
    for target, (kind, skills) in sorted(seen.items()):
        print(f'{target:#x}  {kind}   (skills {", ".join(str(s) for s in sorted(skills))})')


def main():
    arguments = sys.argv[1:]
    makers = dict(MAKERS)
    for index in range(CASES):
        case = mips.word(TABLE + 4 * index)
        if case != NONE:
            makers[FIRST + index] = (mips.word(case) & 0x3ffffff) << 2
    states = {skill: of_maker(maker) for skill, maker in makers.items()}
    if arguments and arguments[0] == '--helpers':
        helpers({skill: update for skill, (_, update, _) in states.items() if update})
        return
    only = int(arguments[0]) if arguments else None
    seen = {}
    for skill, (constructor, update, end) in sorted(states.items()):
        if only and skill != only:
            continue
        print(f'skill {skill}: constructor {constructor:#x}, update {update:#x}, end {end:#x}')
        if update in seen:
            print(f'   the same update as skill {seen[update]}')
            continue
        seen[update] = skill
        memory = {}
        describe(update, memory)
        if end:
            lines = [f'      {vaddr:#x}  {what}: {detail}' for vaddr, what, detail in walk(end, mips.function_bounds(end)[1], {}) if what not in ('wait', 'restart')]
            if lines:
                print('   as it ends')
                print('\n'.join(lines))


if __name__ == '__main__':
    main()
