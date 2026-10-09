"""Which animations scale a node, and how: by the same factor on every axis (which
no child's "undo parent scale" record can turn into a shear) or differently on each.

Usage: insa_scales.py     -> a summary; tools_re/out/insa_scales.txt lists the files
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from insa_records import files, ROOT


def channel(d, r, axis):
    count, = struct.unpack_from('<H', d, r + 4 + axis * 2)
    if count == 0:
        return [struct.unpack_from('<f', d, r + 12 + axis * 4)[0]]
    at, = struct.unpack_from('<I', d, r + 12 + axis * 4)
    return [struct.unpack_from('<f', d, at + i * 8 + 4)[0] for i in range(count)]


def main():
    kinds = collections.Counter()
    lines = []
    for path, d in files(ROOT):
        nodes, = struct.unpack_from('<H', d, 8)
        nodes_at, = struct.unpack_from('<I', d, 0x14)
        uneven, scaled = [], []
        for n in range(nodes):
            at = nodes_at + n * 0x24
            tracks = d[at + 1]
            tracks_at, = struct.unpack_from('<I', d, at + 4)
            for t in range(tracks):
                r = tracks_at + t * 0x18
                kind, mode = d[r], d[r + 1]
                if kind != 2:
                    continue
                if mode & 2:
                    values = channel(d, r, 0)
                    if any(abs(v - 1) > 1e-3 for v in values):
                        scaled.append((n, min(values), max(values)))
                    continue
                axes = [channel(d, r, axis) for axis in range(3)]
                low = [min(a) for a in axes]
                high = [max(a) for a in axes]
                if all(abs(v - 1) <= 1e-3 for v in low + high):
                    continue
                same = (max(low) - min(low) < 1e-3 and max(high) - min(high) < 1e-3)
                (scaled if same else uneven).append((n, min(low), max(high)))
        name = os.path.relpath(path, ROOT)
        if uneven:
            kinds['animations with a node scaled differently on its axes'] += 1
            lines.append(f'uneven  {name}: nodes ' + ' '.join(f'{n}({lo:.2f}..{hi:.2f})' for n, lo, hi in uneven[:8]))
        elif scaled:
            kinds['animations with a node scaled evenly'] += 1
            lines.append(f'even    {name}: nodes ' + ' '.join(f'{n}({lo:.2f}..{hi:.2f})' for n, lo, hi in scaled[:8]))
        else:
            kinds['animations that scale nothing'] += 1
    with open(os.path.join(HERE, 'out', 'insa_scales.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(sorted(lines)) + '\n')
    for key, n in sorted(kinds.items()):
        print(f'{n:6}  {key}')
    print('\n'.join(sorted(l for l in lines if l.startswith('uneven'))[:40]))


if __name__ == '__main__':
    main()
