"""Prints the cases of a switch in the executable, each up to where it leaves.

Usage: switch.py <jump table vaddr hex> <count> [--base N] [case ...]
  --base N   the number of the table's first entry (a switch on `value - N`)
  cases      only these (by their own numbers); all of them if left out

A case ends at its first unconditional branch or `jr`, with the delay slot, or after 60
instructions. Cases that share a target are listed together.
"""
import sys

import mips


def main():
    args = sys.argv[1:]
    base = 0
    if '--base' in args:
        at = args.index('--base')
        base = int(args[at + 1])
        del args[at:at + 2]
    table, count = int(args[0], 16), int(args[1])
    wanted = {int(a) for a in args[2:]}
    refs = dict(mips.load_refs())
    by_target = {}
    for index in range(count):
        by_target.setdefault(mips.word(table + index * 4), []).append(index + base)
    for target, numbers in sorted(by_target.items(), key=lambda item: item[1][0]):
        if wanted and not wanted & set(numbers):
            continue
        print(f'case {", ".join(map(str, numbers))} -> {target:#x}')
        a = target
        for _ in range(60):
            print(mips.annotate(a, refs))
            w = mips.word(a)
            leaves = (w >> 26 == 4 and (w >> 16) & 0x3ff == 0) or (w & 0xfc1fffff) == 0x00000008 or w >> 26 == 2
            a += 4
            if leaves:
                print(mips.annotate(a, refs))
                break


main()
