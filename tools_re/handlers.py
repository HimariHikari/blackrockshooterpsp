"""Dump a jump-table switch from the executable, one block per handler. Exploration only.

Usage: handlers.py <table vaddr> <count> <first case> <end vaddr> <out name>
Writes tools_re/out/<out name>.txt: the case -> handler map, then each handler's code up
to the next handler (or the end address).
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

table, count, first, end_all = (int(a, 16) for a in sys.argv[1:5])
name = sys.argv[5]

out_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')
os.makedirs(out_dir, exist_ok=True)

refs = dict(mips.load_refs())
handlers = [mips.word(table + i * 4) for i in range(count)]
starts = sorted(set(handlers))
lines = ['case -> handler:']
for i, h in enumerate(handlers):
    lines.append(f'  {i + first:#04x} -> {h:#08x}')
lines.append('')
by_handler = {}
for i, h in enumerate(handlers):
    by_handler.setdefault(h, []).append(i + first)
for n, start in enumerate(starts):
    end = starts[n + 1] if n + 1 < len(starts) else end_all
    cases = ', '.join(f'{o:#04x}' for o in by_handler[start])
    lines.append(f'=== handler {start:#08x}  cases [{cases}]  ({(end - start) // 4} instructions)')
    for a in range(start, end, 4):
        lines.append(mips.annotate(a, refs))
    lines.append('')
path = os.path.join(out_dir, name + '.txt')
open(path, 'w').write('\n'.join(lines))
print(f'wrote {path}: {len(lines)} lines, {len(starts)} distinct handlers for {count} cases')
