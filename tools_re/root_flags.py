"""Lists every place the battle's root flags are set (0xA42DC) and tested (0xA4348),
by the flag's number and, where it is set, the value, when those are constants.
The flags are a set of bits at root + 0x84: 8 the battle is on, 9 and 12 she has
the stage, 11 the display is on (`docs/BATTLE_NOTES.md`). Exploration only.

Usage: root_flags.py
"""
from mips import function_bounds, word

CODE_END = 0x160000
SETTER, TESTER = 0xa42dc, 0xa4348
A1, A2 = 5, 6
REACH = 0x40


def constant(register, call, start):
    """The constant last put in a register before a call, or in its delay slot."""
    for at in [call + 4] + list(range(call - 4, max(start, call - REACH) - 4, -4)):
        w = word(at)
        operation, source, target, destination, function = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, w & 63
        if operation in (0x0d, 0x09) and target == register and source == 0:
            return w & 0xffff
        if operation == 0 and function == 0x25 and destination == register:
            return 0 if source == 0 and target == 0 else None
        if target == register and operation in (0x23, 0x21, 0x25, 0x20, 0x24, 0x0f):
            return None
    return None


def main():
    for target, name in ((SETTER, 'set'), (TESTER, 'tested')):
        call = 0x0c000000 | (target >> 2)
        found = {}
        for at in range(0, CODE_END, 4):
            if word(at) == call:
                start, _ = function_bounds(at)
                number = constant(A1, at, start)
                value = constant(A2, at, start) if target == SETTER else None
                found.setdefault((number, value), set()).add(start)
        print(f'== {name}')
        for (number, value), functions in sorted(found.items(), key=lambda item: (item[0][0] is None, item[0][0] or 0, item[0][1] or 0)):
            functions = sorted(functions)
            shown = ' '.join(f'{function:#x}' for function in functions[:14]) + (' ...' if len(functions) > 14 else '')
            what = f'flag {number}' + (f' to {value}' if target == SETTER else '')
            print(f'  {what}: {len(functions)} functions: {shown}')


if __name__ == '__main__':
    main()
