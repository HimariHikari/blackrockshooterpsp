"""The bullet table (SYSTEM.VOL/btl_bullet_tbl.bin) by class. Exploration only.

Usage: bullets.py            a summary by class and enemy, then every record
       bullets.py <id> ...   those bullets in full

Writes tools_re/out/bullets.txt in the first form.
"""
import os, struct, sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join(HERE, '..', 'game', 'unpacked', 'SYSTEM.VOL', 'btl_bullet_tbl.bin')

NAMES = {
    0: 'id', 1: 'enemy', 2: 'variant', 3: 'node', 4: 'x', 5: 'y', 6: 'z', 7: 'yaw', 8: 'pitch',
    9: 'muzzle', 10: 'sound', 11: 'attack', 12: 'count', 13: 'bank', 14: 'class', 15: 'effect',
    16: 'delay', 17: 'speed', 18: 'l_pitch', 19: 'l_yaw', 20: 'homing', 21: 'h_time', 22: 'h_range',
    23: 'h_until', 24: 'trail', 30: 'f78', 31: 'f7c', 32: 'f80', 33: 'f84', 34: 'f88', 35: 'f8c',
    36: 'f90', 37: 'f94', 38: 'f98', 39: 'next', 40: 'pierce', 41: 'hp', 42: 'radius', 43: 'burst',
    44: 'scale', 45: 'fb4', 46: 'from_node', 47: 'burst_fx', 48: 'life',
}


def rows():
    d = open(PATH, 'rb').read()
    assert d[:4] == b'BXCB'
    records_at, record_len, count, pool_at = struct.unpack_from('<4I', d, 0x0c)
    return [struct.unpack_from(f'<{record_len // 4}i', d, records_at + i * record_len) for i in range(count)]


def full(row):
    return ' '.join(f'{NAMES.get(i, f"f{i * 4:x}")}={v}' for i, v in enumerate(row) if v != 0 or i in (0, 1, 2, 14))


def main():
    table = rows()
    if len(sys.argv) > 1:
        wanted = {int(a) for a in sys.argv[1:]}
        for row in table:
            if row[0] in wanted:
                print(full(row))
        return
    lines = [f'{len(table)} records']
    by_class = defaultdict(lambda: defaultdict(list))
    for row in table:
        by_class[row[14]][row[1]].append(row[0])
    for kind in sorted(by_class):
        total = sum(len(v) for v in by_class[kind].values())
        lines.append(f'class {kind}: {total} records')
        for enemy in sorted(by_class[kind]):
            ids = sorted(set(by_class[kind][enemy]))
            lines.append(f'    enemy {enemy:3d}: ' + ' '.join(str(i) for i in ids))
    lines.append('')
    for row in sorted(table, key=lambda r: (r[14], r[1], r[0], r[2])):
        lines.append(full(row))
    out = os.path.join(HERE, 'out', 'bullets.txt')
    open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
    print(f'wrote {out} ({len(lines)} lines)')


main()
