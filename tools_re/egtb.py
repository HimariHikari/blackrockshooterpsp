"""Dumps the enemy group table SYSTEM.VOL/enmgrp2.dat, following the accessors in the
executable (0x93108-0x93CA8, the branch taken when the table is not the older format).

Writes tools_re/out/enmgrp2.txt.
"""
import os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join(HERE, '..', 'game', 'unpacked', 'SYSTEM.VOL', 'enmgrp2.dat')

d = open(PATH, 'rb').read()
lines = [f'{len(d)} bytes, magic {d[:4]!r}', 'header words: ' + ' '.join(f'{v:#x}' for v in struct.unpack_from('<12I', d, 0))]
count, groups_at = struct.unpack_from('<II', d, 0x0c)
waves_at = struct.unpack_from('<I', d, 0x18)[0]
members_at = struct.unpack_from('<I', d, 0x20)[0]
lines.append(f'{count} groups at {groups_at:#x}, waves at {waves_at:#x}, members at {members_at:#x}')
for g in range(count):
    at = groups_at + g * 0x14
    ident, flags, kind, b5, b6, waves, members, extra, wave0, member0, extra0 = struct.unpack_from('<HhBBBBBBHHH', d, at)
    tail = d[at + 0x10:at + 0x14].hex()
    lines.append(f'group {ident:5d}: flags {flags:#x} kind {kind} b5 {b5} b6 {b6} waves {waves} members {members} extra {extra}'
                 f' wave0 {wave0} member0 {member0} extra0 {extra0} tail {tail}')
    for i in range(members):
        m = members_at + (member0 + i) * 16
        enemy, b2, variant, x, y, z, w, last = struct.unpack_from('<HBBhhhhH', d, m)
        lines.append(f'      member: enemy {enemy:4d} b2 {b2} variant {variant:3d} at ({x}, {y}, {z}) w {w} last {last} | {d[m + 14:m + 16].hex()}')
    for i in range(extra):
        m = members_at + (extra0 + i) * 16
        enemy, b2, variant, x, y, z, w, last = struct.unpack_from('<HBBhhhhH', d, m)
        lines.append(f'      extra : enemy {enemy:4d} b2 {b2} variant {variant:3d} at ({x}, {y}, {z}) w {w} last {last}')
    if waves:
        raw = struct.unpack_from(f'<{waves}H', d, waves_at + wave0 * 2)
        lines.append('      waves: ' + ' '.join(f'{v & 0x7f}/{(v >> 7) & 0x3f}/{v >> 13}' for v in raw))
out = os.path.join(HERE, 'out', 'enmgrp2.txt')
open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print(f'wrote {out} ({len(lines)} lines)')
