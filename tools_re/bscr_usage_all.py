"""Which engine functions the battle scripts under a folder call, summed over the files.

Usage: bscr_usage_all.py <folder relative to game/unpacked> [substring of the file name]
"""
import collections, os, sys

import bscr

folder = os.path.join(bscr.ROOT, sys.argv[1])
want = sys.argv[2] if len(sys.argv) > 2 else ''
counts = collections.Counter()
files = collections.defaultdict(set)
total = 0
for path, _, names in os.walk(folder):
    for name in names:
        if not name.endswith('.bms') or want not in name:
            continue
        d = open(os.path.join(path, name), 'rb').read()
        if len(d) < 0x40:
            continue
        total += 1
        s = bscr.Script(d)
        for off, op, kinds, vals in s.instructions():
            if op in (0x82, 0x86):
                key = ('sys', kinds[0] + (0x100 if op == 0x86 else 0))
            elif op in (0x87, 0x88):
                key = ('sysb', kinds[0] + (0x100 if op == 0x88 else 0))
            else:
                continue
            counts[key] += 1
            files[key].add(name)
print(f'{total} scripts')
for key, n in sorted(counts.items()):
    print(f'{key[0]:4} {key[1]:3d}  x{n:5d}  in {len(files[key])} files')
