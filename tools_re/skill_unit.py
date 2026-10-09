"""Lists what a state of hers does, phase by phase. The states of her attack skills
have an update that is a switch on the state's phase; each case's calls are printed
in order, with the constants passed and the time a wait is for.

Every skill's code unit has its own copies of the same small helpers (say the
skill, take the stage, dim, camera, animation, effect ...), at other addresses in
each unit. A helper is named here by what it calls in its turn, so the listing
reads the same for any unit.

Usage: skill_unit.py <hex address of a state's update or of its end> ...
       skill_unit.py --maker <hex address of a state's maker> ...   finds both
"""
import sys

import mips
from skill_states import ALLOCATE, REG, calls, shown, step

# What a helper is, by a function it calls: the first of these that it does.
BY_CALL = [
    (0xa7ba4, 'take the stage'),
    (0xa7b20, 'give the stage back'),
    (0x85814, 'dim (to, frames)'),
    (0xa7da0, 'camera'),
    (0xa7e58, 'camera back'),
    (0xa7cac, 'lens (near, far)'),
    (0xa7ce0, 'lens back'),
    (0x96a14, 'say the skill'),
    (0x8f5e4, 'a boost is spent'),
    (0x8fe28, 'give its effects'),
    (0x9e9f4, 'end effect'),
    (0x9e5d4, 'make effect'),
    (0xacd14, 'effect follows node'),
    (0xace9c, 'effect at place'),
    (0xacdd0, 'effect layer'),
    (0xa7eac, 'voice'),
    (0xd34a0, 'sound'),
    (0x9b608, 'part mode'),
    (0x86e5c, 'fire (how, attack)'),
    (0xaa220, 'stand'),
    (0xaa1d0, 'animation'),
]
# Functions of the engine a state calls itself.
ENGINE = {
    0x744b0: 'wait until', 0x744a4: 'clock to nought', 0xa111c: 'battle value', 0xa4da8: 'as a number',
    0xa4db0: 'as a flag', 0x743fc: 'the state is done with', 0x7437c: 'a state is made', 0x12ab9c: 'skill record',
    0x12b22c: 'its attack', 0x12927c: 'attack record', 0xa0a0c: 'the stage', 0xa0b54: 'her',
    0x98e84: 'her state ends', 0x9ed3c: 'slot starts over', 0xa1cbc: 'the cursor', 0x97fc0: 'what is locked on to',
}
PLAYER_STATES = range(0x74000, 0x85490)
SW, LW, LUI, SLTIU, JR_AT = 0x2b, 0x23, 0x0f, 0x0b, 0x00200008


def callees(function):
    start, end = function, mips.function_bounds(function)[1]
    found = []
    for vaddr in range(start, end, 4):
        w = mips.word(vaddr)
        if w >> 26 == 3:
            found.append((w & 0x3ffffff) << 2)
    return found


def stores(function, offset):
    """The registers stored at an offset of anything, in a function."""
    end = mips.function_bounds(function)[1]
    return [(mips.word(v) >> 16) & 31 for v in range(function, end, 4) if mips.word(v) >> 26 == SW and mips.word(v) & 0xffff == offset]


def name_of(target, cache={}):
    if target in ENGINE:
        return ENGINE[target]
    if target not in PLAYER_STATES:
        return f'{target:#x}'
    if target not in cache:
        called = callees(target)
        name = next((name for callee, name in BY_CALL if callee in called), None)
        # A helper that reads one model's matrix and writes another's puts her model
        # where an actor's is; one that only writes hers as it takes the stage puts it
        # at the origin (0x24020 turns it half round about Y). Her own update places
        # her model again each frame unless the helper also sets her + 0x7FC.
        if 0xacecc in called and 0xace9c in called and 0x24020 in called:
            name = 'she is put where an actor stands, turned half round'
        elif name == 'take the stage' and 0xace9c in called:
            kept = 0x7fc in [mips.word(v) & 0xffff for v in range(target, mips.function_bounds(target)[1], 4) if mips.word(v) >> 26 == SW]
            turned = ', turned half round' if 0x24020 in called else ''
            name += f' (her model at the origin{turned}' + (', and it stays' if kept else ', until her update places it again') + ')'
        if name is None:
            locks, aim = stores(target, 0x750), stores(target, 0x7f4)
            if locks:
                name = 'unlock' if locks == [0] else 'lock everything'
            elif aim and 0x993b8 in called:
                name = 'aim off' if aim == [0] else 'aim on'
            else:
                name = f'{target:#x} (calls ' + ', '.join(ENGINE.get(c, f'{c:#x}') for c in called if c not in (0xa0a0c, 0xa0b54)) + ')'
        cache[target] = name
    return cache[target]


def switch_of(function):
    """The cases of a function's switch, if it has one: their addresses in order."""
    end = mips.function_bounds(function)[1]
    for vaddr in range(function, end, 4):
        if mips.word(vaddr) != JR_AT:
            continue
        count = high = low = None
        for back in range(vaddr - 4, max(function, vaddr - 128) - 4, -4):
            w = mips.word(back)
            op, rs, rt, imm = w >> 26, (w >> 21) & 31, (w >> 16) & 31, w & 0xffff
            if op == LW and rs == 1 and rt == 1 and low is None:
                low = imm - 0x10000 if imm & 0x8000 else imm
            elif op == LUI and rt == 1 and high is None:
                high = imm << 16
            elif op == SLTIU and count is None:
                count = imm
        if None not in (count, high, low):
            return [mips.word(high + low + 4 * index) for index in range(count)]
    return None


def describe_range(start, end):
    for vaddr, target, a0, a1, a2, last_float in calls(start, end):
        name = name_of(target)
        if target == 0x744b0:
            # (A time that is not a constant is worked out by the state.)
            print(f'      {vaddr:#x}  -- wait until ' + ('a time of its own' if last_float is None else f'{last_float:g}'))
        elif target in (0xa0a0c, 0xa0b54):
            continue
        elif target in (0xa111c,):
            print(f'      {vaddr:#x}  {name} {shown(a0)}')
        else:
            print(f'      {vaddr:#x}  {name}({shown(a1)}, {shown(a2)})')


def describe(update):
    end = mips.function_bounds(update)[1]
    cases = switch_of(update)
    print(f'{update:#x} to {end:#x}' + (f': a switch of {len(cases)}' if cases else ''))
    if not cases:
        describe_range(update, end)
        return
    places = sorted(set(cases))
    for place in places:
        phases = [str(index) for index, case in enumerate(cases) if case == place]
        after = [other for other in places if other > place]
        stop = after[0] if after else end
        print(f'   phase {", ".join(phases)} ({place:#x})' + ('  [outside the function]' if not update <= place < end else ''))
        if update <= place < end:
            describe_range(place, stop)


def of_maker(maker):
    """The update and the end of the state a maker makes: what its constructor
    stores at the state's + 0x88 and + 0x20."""
    made = [target for _, target, *_ in calls(maker, mips.function_bounds(maker)[1]) if target != ALLOCATE]
    constructor, known, found = made[0], {0: 0}, {}
    for vaddr in range(constructor, mips.function_bounds(constructor)[1], 4):
        w = mips.word(vaddr)
        if w >> 26 == SW and ((w >> 16) & 31) in known and (w & 0xffff) in (0x88, 0x20):
            found[w & 0xffff] = known[(w >> 16) & 31]
        step(vaddr, known)
    print(f'maker {maker:#x}: constructor {constructor:#x}')
    describe_range(constructor, mips.function_bounds(constructor)[1])
    return found.get(0x88), found.get(0x20)


def main():
    arguments = sys.argv[1:]
    if not arguments:
        sys.exit(__doc__)
    if arguments[0] == '--maker':
        for maker in arguments[1:]:
            update, end = of_maker(int(maker, 16))
            for what, function in (('its update', update), ('as it ends', end)):
                if function:
                    print(f'{what}:')
                    describe(function)
        return
    for update in arguments:
        describe(int(update, 16))


if __name__ == '__main__':
    main()
