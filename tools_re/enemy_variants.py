"""The names, AI numbers, levels and rewards of every enemy's variants, from each
archive's param2.bin (see docs/BATTLE_NOTES.md for the record).

Usage: enemy_variants.py            -> tools_re/out/enemy_variants.txt and a summary
"""
import os, struct

import bxcb
import items


def item_names():
    """Item number -> name, through the three tables one after the other."""
    names, number = {}, 1
    for name in items.TABLES:
        data, count, length = items.table(name)
        for index in range(count):
            names[number] = items.text(data, struct.unpack_from('<I', data, 0x10 + index * length)[0])
            number += 1
    return names


def main():
    item = item_names()
    folder = os.path.join(bxcb.ROOT, 'BTL', 'CHR')
    lines, summary = [], []
    for archive in sorted(os.listdir(folder)):
        path = os.path.join(folder, archive, 'param2.bin')
        if not os.path.exists(path):
            continue
        d = open(path, 'rb').read()
        _, _, records_at, record_size, count, pool_at, _ = bxcb.parse(d)
        pool = bxcb.strings(d, pool_at)
        names = []
        lines.append(f'{archive}: {count} variants')
        for i in range(count):
            fields = struct.unpack_from('<14i', d, records_at + i * record_size)
            variant, name, ai, level, hp = fields[:5]
            experience, chance, common, rare_chance, rare = fields[9:14]
            name = pool.get(name, '?')
            # Chances are out of 255; the rare item takes the place of the usual one.
            leaves = f'{chance:3}/255 {item.get(common, common)!r}' if chance else 'nothing'
            if chance and rare_chance:
                leaves += f', {rare_chance}/255 of those {item.get(rare, rare)!r}'
            lines.append(f'   variant {variant:3}  AI {ai:2}  level {level:3}  HP {hp:6}  exp {experience:6}  {name}: leaves {leaves}')
            if name not in names:
                names.append(name)
        summary.append(f'{archive:22} {", ".join(names)}')

    out = os.path.join(bxcb.OUT, 'enemy_variants.txt')
    open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
    print('\n'.join(summary))


if __name__ == '__main__':
    main()
