"""Finds where the address of something in the executable's data is built: a `lui`
and, within a few instructions, an `addiu` or `ori` of the same register that come
to it. For finding who uses a table. Exploration only.

Usage: builtat.py <address hex> [<address hex> ...]
"""
import sys

import mips

CODE_END = 0x160000
REACH = 0x30


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    for wanted in (int(argument, 16) for argument in sys.argv[1:]):
        low = wanted & 0xffff
        # An `addiu` takes its half as signed, an `ori` as it is.
        highs = (((wanted + 0x8000) >> 16) & 0xffff, wanted >> 16)
        found = False
        for at in range(0, CODE_END, 4):
            first = mips.word(at)
            if first >> 26 != 0x0f or first & 0xffff not in highs:
                continue
            register = (first >> 16) & 31
            for then in range(at + 4, at + REACH, 4):
                second = mips.word(then)
                operation, source, immediate = second >> 26, (second >> 21) & 31, second & 0xffff
                if operation in (0x09, 0x0d) and source == register and immediate == low:
                    print(f'{wanted:#x}: built at {at:#x} and {then:#x}, in function {mips.function_bounds(at)[0]:#x}')
                    found = True
                    break
        if not found:
            print(f'{wanted:#x}: not built anywhere')


if __name__ == '__main__':
    main()
