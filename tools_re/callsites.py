"""Shows how each call of a function sets its arguments: the instructions just before each
`jal` and its delay slot. Exploration only.

Usage: callsites.py <function vaddr hex> [instructions before, default 6] [--skip <function hex> ...]
  --skip   leaves out the calls made from inside these functions
"""
import sys

import mips


def main():
    args = sys.argv[1:]
    skip = set()
    while '--skip' in args:
        at = args.index('--skip')
        skip.add(int(args[at + 1], 16))
        del args[at:at + 2]
    want = int(args[0], 16)
    before = int(args[1]) if len(args) > 1 else 6
    refs = dict(mips.load_refs())
    encoded = 0x0c000000 | (want >> 2)
    for a in range(0, mips.TEXT_END, 4):
        if mips.word(a) != encoded:
            continue
        start = mips.function_bounds(a)[0]
        if start in skip:
            continue
        print(f'--- in function {start:#x}')
        for b in range(max(start, a - 4 * before), a + 8, 4):
            print(mips.annotate(b, refs))


main()
