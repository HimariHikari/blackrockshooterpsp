"""Finds where a constant is built in a stretch of the executable.

Usage: findimm.py <from hex> <to hex> <value hex>

Lists every `lui` of the value's upper half, with the instruction after it (which
gives the lower half if there is one) and the function it is in. For flag bits and
colours. Exploration only.
"""
import sys

from mips import disasm_one, function_bounds, word


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return
    start, end, value = (int(arg, 16) for arg in sys.argv[1:4])
    upper = (value >> 16) & 0xffff
    for a in range(start, end, 4):
        w = word(a)
        if w >> 26 == 0x0f and w & 0xffff == upper:
            lo, _ = function_bounds(a)
            print(f'0x{a:06x} (function 0x{lo:06x}): {disasm_one(a)} / {disasm_one(a + 4)} / {disasm_one(a + 8)}')


if __name__ == '__main__':
    main()
