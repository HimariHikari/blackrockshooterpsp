"""Survey of the `.cam` files: an INSM header with one camera node, then an INSA animation.

Usage: cam_survey.py            one line per distinct shape, with counts
       cam_survey.py <file>     everything about one file (path relative to game/unpacked)
"""
import collections, os, struct, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'unpacked')


def u32(d, at):
    return struct.unpack_from('<I', d, at)[0]


def f32(d, at):
    return struct.unpack_from('<f', d, at)[0]


def describe(d):
    """The fields that say how the file is laid out."""
    assert d[:4] == b'INSM', d[:4]
    out = {
        'insm bytes 4-11': d[4:12].hex(),
        'insm offsets': [u32(d, 0x10 + i * 4) for i in range(10)],
    }
    insa = u32(d, 0x34)
    assert d[insa:insa + 4] == b'INSA', (hex(insa), d[insa:insa + 4])
    out['insa bytes 4-11'] = d[insa + 4:insa + 12].hex()
    out['insa words'] = [u32(d, insa + 0x0c + i * 4) for i in range(7)]
    return out, insa


def camera_record(d):
    at = u32(d, 0x24)
    return {
        'at': hex(at),
        'head': d[at:at + 4].hex(),
        'floats': [round(f32(d, at + 4 + i * 4), 4) for i in range(5)],
        'tail': d[at + 0x18:at + 0x20].hex(),
    }


def lens_tracks(d, insa):
    """The table the INSA header's field 0x1C points at: the camera's own tracks."""
    count = d[insa + 0x0b]
    at = insa + u32(d, insa + 0x1c)
    tracks = []
    for i in range(count):
        rec = at + i * 16
        head = d[rec:rec + 4].hex()
        counts = struct.unpack_from('<2H', d, rec + 4)
        offsets = struct.unpack_from('<2I', d, rec + 8)
        keys = []
        for n, off in zip(counts, offsets):
            keys.append([(u32(d, insa + off + k * 8), round(f32(d, insa + off + k * 8 + 4), 4)) for k in range(n)])
        tracks.append((head, keys))
    return tracks


def main():
    if len(sys.argv) > 1:
        d = open(os.path.join(ROOT, sys.argv[1]), 'rb').read()
        info, insa = describe(d)
        for key, value in info.items():
            print(f'{key}: {value}')
        print('camera record:', camera_record(d))
        for head, keys in lens_tracks(d, insa):
            print('lens track', head)
            for channel in keys:
                print('   ', channel)
        return

    shapes = collections.Counter()
    heads = collections.Counter()
    animated = []
    for folder, _, files in os.walk(ROOT):
        for name in files:
            if not name.endswith('.cam'):
                continue
            path = os.path.join(folder, name)
            d = open(path, 'rb').read()
            info, insa = describe(d)
            record = camera_record(d)
            shapes[(info['insm bytes 4-11'], info['insa bytes 4-11'][:2] + '..' + info['insa bytes 4-11'][8:],
                    record['head'], record['tail'], tuple(record['floats'][i] for i in (0, 1, 3, 4)))] += 1
            for head, keys in lens_tracks(d, insa):
                heads[(head, len(keys[0]) > 0, len(keys[1]) > 0)] += 1
                for index, channel in enumerate(keys):
                    values = {value for _, value in channel}
                    if len(values) > 1:
                        animated.append((os.path.relpath(path, ROOT), index, min(values), max(values)))
    for shape, n in shapes.most_common():
        print(n, shape)
    print('lens track heads:', dict(heads))
    print(f'{len(animated)} animated lens channels, e.g.')
    for row in animated[:12]:
        print('   ', row)
    print('channel 1 animated in', sum(1 for row in animated if row[1] == 1), 'files')


if __name__ == '__main__':
    main()
