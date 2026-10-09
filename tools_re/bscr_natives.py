"""Dump the battle-script VM's two native-function tables from the executable. Exploration only.

Table 1 at 0x171370 (282 entries) is reached by opcodes 0x82/0x86, table 2 at 0x1720a8
(127 entries) by 0x87/0x88. Entries are 12 bytes; the last word is the id scripts use.
Writes tools_re/out/bscr_natives.txt.
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

out = []
for label, table, count in (('table1 (ops 0x82/0x86)', 0x171370, 0x11A), ('table2 (ops 0x87/0x88)', 0x1720A8, 0x7F)):
    out.append(f'== {label} at {table:#x}, {count} entries')
    for i in range(count):
        a, b, c = (mips.word(table + i * 12 + k) for k in (0, 4, 8))
        note = ''
        for v in (a, b):
            if mips.is_string(v):
                note += f' "{mips.cstring(v)}"'
        out.append(f'  [{i:3}] {a:#010x} {b:#010x} id={c:#x}{note}')
path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', 'bscr_natives.txt')
open(path, 'w').write('\n'.join(out) + '\n')
print(f'wrote {path}')
print('\n'.join(out[:24]))
print('...')
print('\n'.join(out[283:300]))
