"""Dump a data region of the executable as 32-bit words with annotations. Exploration only.

Usage: datadump.py <vaddr hex> <byte length hex> [out name]
Each word is shown as hex, and where plausible as a pointer to a string, a pointer into
code, a float, or inline ASCII. With an out name the result goes to tools_re/out/<name>.txt.
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

start = int(sys.argv[1], 16)
length = int(sys.argv[2], 16)
lines = []
a = start
while a < start + length:
    w = mips.word(a)
    raw = mips.DATA[a + mips.FILE_BIAS:a + mips.FILE_BIAS + 4]
    note = ''
    if mips.is_string(w):
        note = f'-> "{mips.cstring(w)}"'
    elif 0x1000 <= w < mips.TEXT_END and w % 4 == 0:
        note = f'-> code {w:#x}'
    elif mips.TEXT_END <= w < len(mips.DATA) - mips.FILE_BIAS and w > 0x164520:
        note = f'-> data {w:#x}'
    else:
        f = struct.unpack('<f', raw)[0]
        if w and 1e-4 < abs(f) < 1e7 and (w >> 16) != 0:
            note = f'{f:g}f'
        if all(32 <= b < 127 or b == 0 for b in raw) and any(b for b in raw) and raw[0] != 0:
            note += f' "{raw.split(bytes([0]))[0].decode()}"'
    lines.append(f'{a:#08x}  {w:08x}  {w if w < 0x80000000 else w - (1 << 32):>11}  {note}')
    a += 4
text = '\n'.join(lines)
if len(sys.argv) > 3:
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', sys.argv[3] + '.txt')
    open(path, 'w').write(text + '\n')
    print(f'wrote {path} ({len(lines)} lines)')
else:
    print(text)
