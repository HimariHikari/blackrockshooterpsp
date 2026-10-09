"""What the animations' track records really hold, measured against how the original
reads them (its evaluator at 0x20614): the layouts nodes use, the rotation orders of
the records of angles, what is written where a record has no keys, and how tracks
carry on before their first key and after their last.

Usage: insa_records.py [folder]     -> tools_re/out/insa_records.txt and a summary
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, '..', 'game', 'unpacked')


def files(root):
    for folder, _, names in os.walk(root):
        for name in names:
            path = os.path.join(folder, name)
            with open(path, 'rb') as handle:
                head = handle.read(4)
                if head == b'INSA':
                    yield path, head + handle.read()
                elif head == b'INSM':
                    # A camera move: a model's header with a whole animation after it.
                    data = head + handle.read()
                    if len(data) > 0x38 and data[7] == 1:
                        at = struct.unpack_from('<I', data, 0x34)[0]
                        if data[at:at + 4] == b'INSA':
                            yield path + '#camera', data[at:]


def main():
    layouts = collections.Counter()
    layout_example = {}
    orders = collections.Counter()
    wraps = collections.Counter()
    quaternion_wraps = collections.Counter()
    quaternion_high = collections.Counter()
    constants = collections.Counter()
    notes = collections.Counter()
    node_extra = collections.Counter()
    second = collections.Counter()
    count = 0
    lines = []
    for path, d in files(ROOT):
        count += 1
        version = d[4]
        nodes, = struct.unpack_from('<H', d, 8)
        nodes_at, = struct.unpack_from('<I', d, 0x14)
        second[(d[0x0a], d[0x0b])] += 1
        for n in range(nodes):
            at = nodes_at + n * 0x24
            flags, tracks, ident, sibling = d[at:at + 4]
            tracks_at, = struct.unpack_from('<I', d, at + 4)
            reference = struct.unpack_from('<3f', d, at + 8)
            extra = struct.unpack_from('<3f', d, at + 0x14)
            if any(extra):
                node_extra['node +0x14 not zero'] += 1
            if struct.unpack_from('<I', d, at + 0x20)[0]:
                node_extra['node +0x20 not zero'] += 1
            layout = []
            for t in range(tracks):
                r = tracks_at + t * 0x18
                kind, mode, order, pad = d[r:r + 4]
                layout.append((kind, mode))
                if pad:
                    notes[f'kind {kind}: byte 3 = {pad}'] += 1
                if kind == 4:
                    keys, wrap, high = struct.unpack_from('<HBB', d, r + 4)
                    quaternion_wraps[wrap] += 1
                    quaternion_high[high] += 1
                    if order:
                        notes[f'kind 4: byte 2 = {order}'] += 1
                    if keys == 0:
                        value = struct.unpack_from('<4f', d, r + 8)
                        constants[f'kind 4 mode {mode} without keys: ' + ('identity' if value == (0, 0, 0, 1)
                                  else 'zero' if not any(value) else 'a rotation')] += 1
                    continue
                if kind == 3:
                    if any(d[r + 4:r + 0x18]):
                        notes['kind 3 with something after its head'] += 1
                    continue
                counts = struct.unpack_from('<3H', d, r + 4)
                wrap, = struct.unpack_from('<H', d, r + 10)
                wraps[(kind, wrap)] += 1
                if kind == 1:
                    orders[(order, mode)] += 1
                elif order:
                    notes[f'kind {kind}: byte 2 = {order}'] += 1
                shared = mode & 2
                for axis in range(1 if shared else 3):
                    if counts[axis]:
                        continue
                    value, = struct.unpack_from('<f', d, r + 12 + axis * 4)
                    if kind == 0:
                        what = ('equal to the node\'s reference' if abs(value - reference[axis]) < 1e-4
                                else 'zero' if value == 0 else 'another value')
                    elif kind == 2:
                        what = 'one' if value == 1 else 'zero' if value == 0 else 'another value'
                    else:
                        what = 'zero' if value == 0 else 'an angle'
                    constants[f'kind {kind} mode {mode} axis without keys: {what}'] += 1
                if shared and kind != 2:
                    notes[f'kind {kind} with one channel for all three axes'] += 1
            layout = tuple(layout)
            layouts[layout] += 1
            layout_example.setdefault(layout, f'{os.path.relpath(path, ROOT)} node {n}')

    lines.append(f'{count} animations')
    lines.append('\nlayouts of a node (kind.mode in order): nodes, an example')
    for layout, n in layouts.most_common():
        text = ' '.join(f'{k}.{m}' for k, m in layout)
        lines.append(f'  {n:7}  {text:40} {layout_example[layout]}')
    lines.append('\nkind 1 (angles): (order byte, mode): records')
    for key, n in sorted(orders.items()):
        lines.append(f'  {key}: {n}')
    lines.append('\nkinds 0-2: (kind, word at +0xA saying how the track carries on outside its keys): records')
    for key, n in sorted(wraps.items()):
        lines.append(f'  {key[0]} {key[1]:#06x}: {n}')
    lines.append('\nkind 4: byte at +6 (the same for quaternions): records')
    for key, n in sorted(quaternion_wraps.items()):
        lines.append(f'  {key:#04x}: {n}')
    lines.append('kind 4: byte at +7: ' + ', '.join(f'{k}: {n}' for k, n in sorted(quaternion_high.items())))
    lines.append('\nwhat stands where a record has no keys')
    for key, n in sorted(constants.items()):
        lines.append(f'  {n:8}  {key}')
    lines.append('\nother things seen')
    for key, n in sorted(notes.items()) + sorted(node_extra.items()):
        lines.append(f'  {n:8}  {key}')
    lines.append('\n(second section entries, lens tracks): animations')
    for key, n in sorted(second.items()):
        lines.append(f'  {key}: {n}')
    text = '\n'.join(lines) + '\n'
    with open(os.path.join(HERE, 'out', 'insa_records.txt'), 'w', encoding='utf-8') as out:
        out.write(text)
    print(text)


if __name__ == '__main__':
    main()
