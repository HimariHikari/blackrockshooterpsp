"""Lists what each of her skills of kind 2 does in its state's update: the calls it
makes with the constants it passes, in order, with the time each step waits for.

The states are made by 0x81CD4 (a jump table by the skill's number from 23). Each
has a maker that calls its constructor, which sets its update (state + 0x88) and
looks its skill's record up; the update is a list of steps, each waiting for the
state's clock to reach a time (0x744B0).

Usage: skill_states.py [skill number]
       skill_states.py --update <hex address of an update>   any state's update
"""
import struct, sys

import mips

TABLE, CASES, FIRST, NONE = 0x168fc8, 49, 23, 0x81dfc
ALLOCATE, RECORD, BASE = 0xa7fdc, 0x12ab9c, 0x7437c

NAMES = {
    0x80670: 'say the skill', 0x744a4: 'clock to nought', 0x80784: 'take the stage', 0x8073c: 'dim (to, frames)',
    0x80874: 'camera', 0x808e0: 'lens', 0x80960: 'animation', 0x80dd8: 'voice', 0x809bc: 'make effect',
    0x80aa8: 'effect follows node', 0x80a28: 'effect at node', 0x80b10: 'effect layer', 0x80d58: 'sound',
    0x80d8c: 'sound of the skill', 0x80e14: 'give its effects', 0x80b3c: 'end effect', 0x808b4: 'camera back',
    0x80934: 'lens back', 0x80be4: 'give the stage back', 0x80b58: 'stand', 0x80ccc: 'unlock',
    0x744b0: 'wait until', 0xa111c: 'battle value', 0xa4da8: 'as a number', 0xa4db0: 'as a flag',
    0x80d2c: 'a boost is spent', 0x743fc: 'the state is done with',
}
REG = {name: number for number, name in enumerate(
    'zero at v0 v1 a0 a1 a2 a3 t0 t1 t2 t3 t4 t5 t6 t7 s0 s1 s2 s3 s4 s5 s6 s7 t8 t9 k0 k1 gp sp fp ra'.split())}
A0, A1, A2 = REG['a0'], REG['a1'], REG['a2']


def step(vaddr, known):
    """Follows one instruction's effect on the registers whose values are constants."""
    w = mips.word(vaddr)
    op, rs, rt, rd, imm, funct = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, w & 0xffff, w & 0x3f
    simm = imm - 0x10000 if imm & 0x8000 else imm
    if op == 0x0f:
        known[rt] = imm << 16
    elif op == 0x0d:
        known[rt] = (known[rs] | imm) if rs in known else None
    elif op == 0x09:
        known[rt] = ((known[rs] + simm) & 0xffffffff) if rs in known else None
    elif op == 0 and funct in (0x21, 0x25) and (rt == 0 or rs == 0):  # move
        source = rs if rt == 0 else rt
        known[rd] = known.get(source)
    elif op == 0 and rd:
        known[rd] = None
    elif op in (0x23, 0x21, 0x25, 0x20, 0x24):  # loads
        known[rt] = None
    for register in [r for r, value in known.items() if value is None]:
        del known[register]


def shown(value):
    if value is None:
        return '?'
    if mips.is_string(value):
        return f'"{mips.cstring(value)}"'
    return str(value - (1 << 32)) if value >= 0x80000000 else str(value)


def calls(start, end):
    """The calls between two addresses, each with what a1 and a2 are known to be and
    the last float loaded before it."""
    known, last_float, found = {0: 0}, None, []
    vaddr = start
    while vaddr < end:
        w = mips.word(vaddr)
        if w >> 26 == 0x0f and (w & 0xffff) >= 0x3000:
            last_float = struct.unpack('<f', struct.pack('<I', (w & 0xffff) << 16))[0]
        if w >> 26 == 3:
            step(vaddr + 4, known)
            found.append((vaddr, (w & 0x3ffffff) << 2, known.get(A0), known.get(A1), known.get(A2), last_float))
            # What a call returns, and what it may have done to the argument registers.
            for register in (REG['v0'], A0, A1, A2, REG['a3']):
                known.pop(register, None)
            vaddr += 8
            continue
        step(vaddr, known)
        vaddr += 4
    return found


def update_of(ctor):
    """The update a constructor sets, what it has run as the state ends, and the
    skill whose record it looks up."""
    update = end = skill = None
    known = {0: 0}
    for vaddr in range(ctor, mips.function_bounds(ctor)[1], 4):
        w = mips.word(vaddr)
        if w >> 26 == 3 and (w & 0x3ffffff) << 2 == RECORD:
            step(vaddr + 4, known)
            skill = known.get(A0)
        if w >> 26 == 0x2b and ((w >> 16) & 31) in known:  # sw r, n(state)
            if (w & 0xffff) == 0x88:
                update = known[(w >> 16) & 31]
            elif (w & 0xffff) == 0x20:
                end = known[(w >> 16) & 31]
        step(vaddr, known)
    return update, end, skill


def describe(update):
    end = mips.function_bounds(update)[1]
    for vaddr, target, a0, a1, a2, last_float in calls(update, end):
        name = NAMES.get(target, f'{target:#x}')
        if target == 0x744b0:
            print(f'   {vaddr:#x}  -- wait until {last_float:g}')
        elif target in (0xa111c,):
            print(f'   {vaddr:#x}  {name} {shown(a0)}')
        else:
            print(f'   {vaddr:#x}  {name}({shown(a1)}, {shown(a2)})')


def main():
    if len(sys.argv) > 2 and sys.argv[1] == '--update':
        describe(int(sys.argv[2], 16))
        return
    only = int(sys.argv[1]) if len(sys.argv) > 1 else None
    seen = {}
    for index in range(CASES):
        case, skill = mips.word(TABLE + 4 * index), FIRST + index
        if case == NONE or (only and skill != only):
            continue
        maker = (mips.word(case) & 0x3ffffff) << 2
        made = [target for _, target, *_ in calls(maker, mips.function_bounds(maker)[1]) if target != ALLOCATE]
        ctor = made[0]
        update, end, record = update_of(ctor)
        ending = [NAMES.get(target, f'{target:#x}') for _, target, *_ in calls(end, mips.function_bounds(end)[1])]
        print(f'skill {skill}: maker {maker:#x}, constructor {ctor:#x}, update {update:#x}, record of skill {record}')
        print(f'   as it ends ({end:#x}): ' + ', '.join(ending))
        if update in seen:
            print(f'   the same update as skill {seen[update]}')
            continue
        seen[update] = skill
        describe(update)


if __name__ == '__main__':
    main()
