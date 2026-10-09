"""Finds the code that takes one of the resident resources: a call of 0x97DC (the
resource of a list, by number) whose list is the word at a given offset of the
game's manager (0x26C() + 0x20000 + offset) and whose number is the one asked for.
Exploration only.

Usage: res_users.py <offset hex, e.g. 0x14> <number>
"""
import sys

import mips


def main():
    offset = int(sys.argv[1], 16)
    number = int(sys.argv[2], 0)
    call = 0x0c000000 | (0x97DC >> 2)
    seen = {}
    for a in range(0, mips.TEXT_END, 4):
        if mips.word(a) != call:
            continue
        text = [mips.disasm_one(b) for b in range(a - 24, a + 8, 4)]
        list_ok = any(line.replace(' ', '') == f'lw$a0,{offset:#x}($a0)' for line in text)
        number_ok = any(line.replace(' ', '') in (f'ori$a1,$zero,{number:#x}' if number > 9 else f'ori$a1,$zero,{number}',) for line in text[-3:])
        if number == 0:
            number_ok = any(line.replace(' ', '') == 'move$a1,$zero' for line in text[-3:])
        if list_ok and number_ok:
            start = mips.function_bounds(a)[0]
            seen.setdefault(start, []).append(a)
    for start, calls in sorted(seen.items()):
        print(f'{start:#x}: ' + ' '.join(f'{c:#x}' for c in calls))


main()
