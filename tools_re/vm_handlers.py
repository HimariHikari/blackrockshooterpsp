"""Dump the character-script VM's opcode handlers from the executable. Exploration only.

The interpreter is the function at 0x112790; its jump table (65 entries) is at 0x16de30.
Writes tools_re/out/vm_handlers.txt.
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

TABLE = 0x16de30
COUNT = 0x41
FUNC_START, FUNC_END = 0x112790, 0x1133e4

out_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')
os.makedirs(out_dir, exist_ok=True)

refs = dict(mips.load_refs())
handlers = [mips.word(TABLE + i * 4) for i in range(COUNT)]
starts = sorted(set(handlers))
lines = []
lines.append('opcode -> handler:')
for i, h in enumerate(handlers):
    lines.append(f'  {i:#04x} -> {h:#08x}')
lines.append('')
by_handler = {}
for i, h in enumerate(handlers):
    by_handler.setdefault(h, []).append(i)
for n, start in enumerate(starts):
    end = starts[n + 1] if n + 1 < len(starts) else FUNC_END
    ops = ', '.join(f'{o:#04x}' for o in by_handler[start])
    lines.append(f'=== handler {start:#08x}  opcodes [{ops}]  ({(end - start) // 4} instructions)')
    for a in range(start, end, 4):
        lines.append(mips.annotate(a, refs))
    lines.append('')
path = os.path.join(out_dir, 'vm_handlers.txt')
open(path, 'w').write('\n'.join(lines))
print(f'wrote {path}: {len(lines)} lines, {len(starts)} distinct handlers for {COUNT} opcodes')
print('\n'.join(lines[:COUNT + 1]))
