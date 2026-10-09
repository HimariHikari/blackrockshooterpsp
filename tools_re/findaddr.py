"""Finds `addiu` instructions with a given immediate: where the address of a field
of an object is taken, which findstore.py does not see.

Usage: findaddr.py <from hex> <to hex> <immediate hex>
"""
import sys

from mips import disasm_one, function_bounds, word


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return
    start, end, imm = (int(arg, 16) for arg in sys.argv[1:4])
    for a in range(start, end, 4):
        w = word(a)
        if w >> 26 == 0x09 and (w & 0xffff) == (imm & 0xffff):
            print(f'{a:#08x} (function {function_bounds(a)[0]:#08x}): {disasm_one(a)}')


if __name__ == '__main__':
    main()
