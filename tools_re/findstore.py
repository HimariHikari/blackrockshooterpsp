"""Finds loads and stores with a given offset in a stretch of the executable.

Usage: findstore.py <from hex> <to hex> <offset hex> [r|w]

Lists every load (r) or store (w) whose displacement is the offset, whatever the base
register, with the function it is in. For fields that `rootfield.py` cannot find
because the object's pointer was not loaded just before. Exploration only.
"""
import sys

from mips import disasm_one, function_bounds, word

LOADS = (0x20, 0x21, 0x23, 0x24, 0x25, 0x31)
STORES = (0x28, 0x29, 0x2b, 0x39)


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return
    start, end, offset = (int(arg, 16) for arg in sys.argv[1:4])
    want = sys.argv[4] if len(sys.argv) > 4 else 'rw'
    for a in range(start, end, 4):
        w = word(a)
        op, imm = w >> 26, w & 0xffff
        if imm != offset:
            continue
        kind = 'r' if op in LOADS else 'w' if op in STORES else None
        if kind and kind in want:
            print(f'{a:#08x} (function {function_bounds(a)[0]:#08x}) {kind}: {disasm_one(a)}')


if __name__ == '__main__':
    main()
