"""Lists a jump table of the executable: jumptable.py <table vaddr hex> <count> [instructions per case]."""
import sys

import mips

table, count = int(sys.argv[1], 16), int(sys.argv[2])
length = int(sys.argv[3]) if len(sys.argv) > 3 else 8
refs = dict(mips.load_refs())
for case in range(count):
    target = mips.word(table + case * 4)
    print(f'case {case} -> {target:#x}')
    for a in range(target, target + 4 * length, 4):
        print(mips.annotate(a, refs))
