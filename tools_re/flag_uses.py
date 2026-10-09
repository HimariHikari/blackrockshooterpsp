"""Finds the calls of a function that pass a given small constant in $a1: who reads or
sets one flag or parameter. Exploration only.

Usage: flag_uses.py <function vaddr hex> <value hex> [<value hex> ...]
  prints, for each value, the functions the calls are in and the address of each call
"""
import sys

import mips


def main():
    want = int(sys.argv[1], 16)
    values = [int(v, 16) for v in sys.argv[2:]]
    encoded = 0x0c000000 | (want >> 2)
    found = {value: [] for value in values}
    for a in range(0, mips.TEXT_END, 4):
        if mips.word(a) != encoded:
            continue
        # The constant is loaded with `ori $a1, $zero, n` just before the call or in its
        # delay slot.
        for b in (a + 4, a - 4, a - 8):
            w = mips.word(b)
            if w >> 16 == 0x3405:
                value = w & 0xffff
                if value in found:
                    found[value].append((mips.function_bounds(a)[0], a))
                break
    for value in values:
        print(f'{value:#x}:')
        for start, at in found[value]:
            print(f'  in {start:#x} at {at:#x}')


if __name__ == '__main__':
    main()
