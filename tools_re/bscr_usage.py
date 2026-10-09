"""Which engine functions a battle script calls, how often, and from which of its functions.

Usage: bscr_usage.py <file relative to game/unpacked>   -> tools_re/out/bscr_usage_<name>.txt
"""
import collections, os, sys

import bscr

rel = sys.argv[1]
s = bscr.Script(open(os.path.join(bscr.ROOT, rel), 'rb').read())
starts = sorted((f['code'], f['name']) for f in s.funcs)
counts = collections.Counter()
where = collections.defaultdict(collections.Counter)
current = None
index = 0
for off, op, kinds, vals in s.instructions():
    while index < len(starts) and starts[index][0] <= off:
        current = starts[index][1]
        index += 1
    if op in (0x82, 0x86):
        key = ('sys', kinds[0] + (0x100 if op == 0x86 else 0))
    elif op in (0x87, 0x88):
        key = ('sysb', kinds[0] + (0x100 if op == 0x88 else 0))
    else:
        continue
    counts[key] += 1
    where[key][current] += 1

lines = [f'# {rel}: {len(counts)} engine functions used']
for key, n in sorted(counts.items()):
    users = ', '.join(f'{name} x{c}' for name, c in where[key].most_common(6))
    more = len(where[key]) - 6
    lines.append(f'{key[0]:4} {key[1]:3d}  x{n:4d}  {users}' + (f' (+{more} more)' if more > 0 else ''))
path = os.path.join(bscr.OUT, f'bscr_usage_{os.path.basename(rel)}.txt')
open(path, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print(f'wrote {path} ({len(lines)} lines)')
