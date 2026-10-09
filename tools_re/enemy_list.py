"""Lists every enemy archive's variants from its param2.bin (BXCB): id, name, level, HP.

Writes tools_re/out/enemy_list.txt.
"""
import os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked', 'BTL', 'CHR')


def table(d):
    assert d[:4] == b'BXCB'
    records_at, record_len, count, pool_at = struct.unpack_from('<4I', d, 0x0c)
    pool = d[pool_at:]
    rows = [struct.unpack_from(f'<{record_len // 4}i', d, records_at + i * record_len) for i in range(count)]
    return rows, pool


def string(pool, offset):
    end = pool.index(b'\0', offset)
    return pool[offset:end].decode('utf-8', 'replace')


lines = []
for name in sorted(os.listdir(ROOT)):
    folder = os.path.join(ROOT, name)
    files = sorted(os.listdir(folder))
    path = os.path.join(folder, 'param2.bin')
    if not os.path.exists(path):
        lines.append(f'{name}: no param2.bin ({len(files)} files)')
        continue
    rows, pool = table(open(path, 'rb').read())
    anims = sum(1 for f in files if f.endswith('.anm'))
    bms = [f for f in files if f.endswith('.bms')]
    size = os.path.getsize(os.path.join(folder, bms[0])) if bms else 0
    lines.append(f'{name}: {anims} animations, script {bms[0] if bms else "-"} {size} bytes')
    for row in rows:
        lines.append(f'    id {row[0]:4d} "{string(pool, row[1])}" kind {row[2]} level {row[3]} hp {row[4]} | ' +
                     ' '.join(str(v) for v in row[5:16]))
out = os.path.join(HERE, 'out', 'enemy_list.txt')
open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print(f'wrote {out} ({len(lines)} lines)')
