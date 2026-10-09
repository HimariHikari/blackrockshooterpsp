"""The figures of every enemy's `.edx` that the reinforcement code reads (0xC0B8C):
the byte at 0x18 and the words at 0x2A and 0x36, with the kind of its param1.bin.

Writes tools_re/out/edx_survey.txt.
"""
import os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked', 'BTL', 'CHR')

lines = []
for name in sorted(os.listdir(ROOT)):
    folder = os.path.join(ROOT, name)
    edx = [f for f in os.listdir(folder) if f.lower().endswith('.edx')]
    if not edx:
        continue
    d = open(os.path.join(folder, edx[0]), 'rb').read()
    kind = '-'
    p1 = os.path.join(folder, 'param1.bin')
    if os.path.exists(p1):
        p = open(p1, 'rb').read()
        records_at, record_len, count, pool_at = struct.unpack_from('<4I', p, 0x0c)
        row = struct.unpack_from(f'<{record_len // 4}i', p, records_at)
        kind = f'id {row[0]} kind {row[2]:#x} rest {row[3:]}'
    words = struct.unpack_from('<28H', d, 0)
    lines.append(f'{name:22s} {len(d):4d} bytes  byte18 {d[0x18]}  w2a {struct.unpack_from("<H", d, 0x2a)[0]:4d}'
                 f'  w36 {struct.unpack_from("<H", d, 0x36)[0]:4d}  param1: {kind}')
out = os.path.join(HERE, 'out', 'edx_survey.txt')
open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print('\n'.join(lines))
