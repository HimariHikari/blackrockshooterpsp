"""One animation written out: its header, and for each node its flags and its track
records as the original's evaluator (0x20614) reads them.

Usage: insa_dump.py <file under game/unpacked, or a path> [first node] [last node]
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
KINDS = {0: 'translation', 1: 'angles', 2: 'scale', 3: 'undo parent scale', 4: 'quaternion'}


def load(path):
    if not os.path.exists(path):
        path = os.path.join(ROOT, path)
    d = open(path, 'rb').read()
    if d[:4] == b'INSM':
        d = d[struct.unpack_from('<I', d, 0x34)[0]:]
    return d


def keys(d, at, count, limit=4):
    found = [struct.unpack_from('<If', d, at + i * 8) for i in range(count)]
    text = ' '.join(f'{t / 1024:g}:{v:.4f}' for t, v in found[:limit])
    return text + (f' ... {found[-1][0] / 1024:g}:{found[-1][1]:.4f}' if count > limit else '')


def main():
    d = load(sys.argv[1])
    first = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    version = d[4]
    nodes, = struct.unpack_from('<H', d, 8)
    last = int(sys.argv[3]) if len(sys.argv) > 3 else nodes - 1
    start, end, nodes_at, second_at, lens_at, loop_start, loop_end = struct.unpack_from('<7I', d, 0x0c)
    print(f'version {version}, {nodes} nodes, {d[0x0a]} material tracks, {d[0x0b]} lens tracks, '
          f'time {start / 1024:g}..{end / 1024:g} frames, loop {loop_start / 1024:g}..{loop_end / 1024:g}')
    for n in range(first, min(last, nodes - 1) + 1):
        at = nodes_at + n * 0x24
        flags, tracks, ident, sibling = d[at:at + 4]
        tracks_at, = struct.unpack_from('<I', d, at + 4)
        reference = struct.unpack_from('<3f', d, at + 8)
        print(f'node {n}: flags {flags:#04x}{" (not driven)" if flags & 1 else ""} id {ident} next {sibling} '
              f'reference ({reference[0]:.3f}, {reference[1]:.3f}, {reference[2]:.3f})')
        for t in range(tracks):
            r = tracks_at + t * 0x18
            kind, mode, order, _ = d[r:r + 4]
            head = f'  {KINDS.get(kind, kind)} mode {mode}'
            if kind == 3:
                print(head)
            elif kind == 4:
                count, = struct.unpack_from('<H', d, r + 4)
                at_keys, = struct.unpack_from('<I', d, r + 8)
                if count == 0:
                    print(head, 'constant', struct.unpack_from('<4f', d, r + 8))
                    continue
                found = [struct.unpack_from('<I4f', d, at_keys + i * 20) for i in range(count)]
                text = ' '.join(f'{k[0] / 1024:g}:({k[1]:.3f} {k[2]:.3f} {k[3]:.3f} {k[4]:.3f})' for k in found[:2])
                print(head, f'{count} keys {text}')
            else:
                counts = struct.unpack_from('<3H', d, r + 4)
                wrap, = struct.unpack_from('<H', d, r + 10)
                parts = []
                for axis in range(1 if mode & 2 else 3):
                    if counts[axis]:
                        offset, = struct.unpack_from('<I', d, r + 12 + axis * 4)
                        parts.append(f'[{counts[axis]} keys {keys(d, offset, counts[axis])}]')
                    else:
                        parts.append(f'{struct.unpack_from("<f", d, r + 12 + axis * 4)[0]:.4f}')
                extra = (f' order {order}' if kind == 1 else '') + (f' wrap {wrap:#x}' if wrap else '')
                print(head + extra, ' '.join(parts))


if __name__ == '__main__':
    main()
