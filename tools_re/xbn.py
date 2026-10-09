"""Reads what goes with a layout (`SDOa`): its motions (`XBN`) and its groups (the
`.bin` after them), as the layout library does (0x304C4 on, 0x33138). Exploration only.

Usage: xbn.py <xbn file relative to game/unpacked>     one file, in full
       xbn.py --groups <bin file>                       a groups file, in full
       xbn.py --survey                                  what all of them use

XBN: `XBN\\0`, u32 sets, u32 clips, an offset for each set and then for each clip.
A set: u32 tracks, then 16 bytes a track: u16 group, u16 clip, u16 delay, u16 flags,
f32 x, f32 y. A clip: u32 channels, each u32 kind, u32 keys, u32 ?, then 16 bytes a
key: u32 frame, f32 value, u32 ease, u32 mark.
Groups: u32 size, u32 objects, u32 groups, a colour for each object, an offset for
each group; a group is u32 count and so many object numbers.
"""
import os, struct, sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

KINDS = {1: 'x', 2: 'y', 4: 'scale x', 5: 'scale y', 9: 'turn', 10: 'turn?', 11: 'red', 12: 'green', 13: 'blue', 14: 'alpha'}


def parse(d):
    assert d[:4] == b'XBN\0', d[:4]
    sets_n, clips_n = struct.unpack_from('<II', d, 4)
    offsets = struct.unpack_from(f'<{sets_n + clips_n}I', d, 12)
    sets = []
    for at in offsets[:sets_n]:
        count = struct.unpack_from('<I', d, at)[0]
        sets.append([struct.unpack_from('<4H2f', d, at + 4 + 16 * i) for i in range(count)])
    clips = []
    for at in offsets[sets_n:]:
        count = struct.unpack_from('<I', d, at)[0]
        p = at + 4
        channels = []
        for _ in range(count):
            kind, keys, other = struct.unpack_from('<3I', d, p)
            channels.append((kind, other, [struct.unpack_from('<IfII', d, p + 12 + 16 * k) for k in range(keys)]))
            p += 12 + 16 * keys
        clips.append(channels)
    return sets, clips


def groups(d):
    size, objects, count = struct.unpack_from('<3I', d, 0)
    assert size == len(d), (size, len(d))
    colours = [d[12 + 4 * i:16 + 4 * i].hex() for i in range(objects)]
    offsets = struct.unpack_from(f'<{count}I', d, 12 + 4 * objects)
    out = []
    for at in offsets:
        n = struct.unpack_from('<I', d, at)[0]
        out.append(list(struct.unpack_from(f'<{n}I', d, at + 4)))
    return colours, out


def show(path):
    sets, clips = parse(open(path, 'rb').read())
    for n, tracks in enumerate(sets):
        print(f'set {n}:')
        for group, clip, delay, flags, x, y in tracks:
            print(f'   group {group} clip {clip} delay {delay} flags {flags:#x} at ({x:g}, {y:g})')
    for n, channels in enumerate(clips):
        print(f'clip {n}:')
        for kind, other, keys in channels:
            print(f'   {KINDS.get(kind, kind)} ({other}): ' + ', '.join(f'{f}:{v:g}' + ('/' + str(e) if e else '') + ('*' * m) for f, v, e, m in keys))


def survey():
    kinds, eases, marks, others, flags, delays = Counter(), Counter(), Counter(), Counter(), Counter(), Counter()
    files = bad = paired = 0
    group_sizes = Counter()
    for base, _, names in os.walk(ROOT):
        for name in sorted(names):
            path = os.path.join(base, name)
            if name.endswith('.xbn'):
                files += 1
                try:
                    sets, clips = parse(open(path, 'rb').read())
                except Exception as error:
                    bad += 1
                    print('failed', os.path.relpath(path, ROOT), repr(error))
                    continue
                for tracks in sets:
                    for group, clip, delay, flag, x, y in tracks:
                        flags[flag] += 1
                        delays[min(delay, 100)] += 1
                        assert clip < len(clips), (path, clip)
                for channels in clips:
                    for kind, other, keys in channels:
                        kinds[kind] += 1
                        others[other] += 1
                        for f, v, e, m in keys:
                            eases[e] += 1
                            marks[m] += 1
                        frames = [k[0] for k in keys]
                        assert frames == sorted(frames), (path, frames)
            if name.endswith('.sdoa'):
                number = int(name[:3])
                bin_path = os.path.join(base, f'{number + 2:03d}.bin')
                if os.path.exists(bin_path) and os.path.exists(os.path.join(base, f'{number + 1:03d}.xbn')):
                    try:
                        colours, out = groups(open(bin_path, 'rb').read())
                        objects = struct.unpack_from('<H', open(path, 'rb').read(), 4)[0]
                        assert objects == len(colours), (objects, len(colours))
                        for g in out:
                            group_sizes[len(g)] += 1
                            assert all(i < objects for i in g)
                        paired += 1
                    except Exception as error:
                        print('groups failed', os.path.relpath(bin_path, ROOT), repr(error))
    print(files, 'xbn files,', bad, 'failed;', paired, 'layouts with groups read')
    print('channel kinds', dict(sorted(kinds.items())))
    print('third word of a channel', dict(others))
    print('eases', dict(eases), 'marks', dict(marks))
    print('track flags', dict(flags))
    print('delays', dict(sorted(delays.items())))
    print('objects in a group', dict(sorted(group_sizes.items())))


if __name__ == '__main__':
    if sys.argv[1] == '--survey':
        survey()
    elif sys.argv[1] == '--groups':
        colours, out = groups(open(os.path.join(ROOT, sys.argv[2]), 'rb').read())
        print('colours', colours)
        for n, g in enumerate(out):
            print(f'group {n}: {g}')
    else:
        show(sys.argv[1] if os.path.isabs(sys.argv[1]) else os.path.join(ROOT, sys.argv[1]))
