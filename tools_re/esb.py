"""Shows what an effect's script (`.esb`, a part of an `EFC` or `EFP` file) says, as far
as it has been read (0x127EE0, 0x128234, 0x11E05C, 0x125708): its root, the entries
of its timeline, and each entry's element with its runs of keys.

usage: python esb.py <file under game/unpacked> [effect number or index]
       python esb.py --all        (what there is over every file, and whether every
                                   run of keys ends where its size of key says)
"""
import collections
import os
import struct
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'unpacked')

# An element's eleven offsets: where the game keeps each in the element, the size of
# the keys of the run it is of, and how the rest of a key is read after its head of
# eight bytes (0x11F708 on for the first four, the element's own; 0x125708 for the
# rest, which are a particle's).
PARTS = [
    ('+94 place', 0x14, '<3f'),
    ('+98 shape', 0x20, '<6f'),
    ('+9c way', 0x1c, '<5f'),
    ('+a0 count', 0x10, '<HHf'),
    ('+a4', 0xc, '<hh'),
    ('+b0 size', 0x14, '<3f'),
    ('+b4 speed', 0xc, '<f'),
    ('+b8 colour', 0x10, '<4H'),
    ('+bc turn', 0x14, '<3f'),
    ('+c0', 0x18, '<4f'),
    ('+c4', 0x14, '<3f'),
]
# The figures a run has before its keys, which the game's small readers take by
# negative offsets (0x128AD8 to 0x128BAC; `out/_before_runs.py` lists who reads
# what): how far before, and how it is read.
BEFORE = {
    '+94 place': [(2, '<H', 'fixed to')],
    '+98 shape': [(4, '<H', 'shape'), (2, '<H', 'turns with')],
    '+9c way': [(2, '<H', 'space')],
    '+a4': [(4, '<H', 'run'), (2, '<H', 'run')],
    '+b0 size': [(0x10, '<f', 'by'), (0xc, '<f', 'spread'), (8, '<f', 'spread y'), (4, '<f', 'spread z')],
    '+b4 speed': [(4, '<f', 'spread')],
    '+b8 colour': [(0x14, '<H', 'blend'), (0x10, '<f', ''), (0xc, '<f', ''), (8, '<f', ''), (4, '<f', '')],
    '+bc turn': [(0x10, '<H', 'faces'), (0xe, '<H', ''), (0xc, '<f', ''), (8, '<f', ''), (4, '<f', '')],
    '+c0': [(4, '<H', ''), (2, '<H', 'kind')],
}
# The part at +a4 has two more runs, whose offsets are the two half-words before it.
BEFORE_A4 = [('+a8', 0x10, '<2f'), ('+ac', 0x10, '<2f')]
END = 0x4000


def chain(data):
    at = 0x10
    while at + 0x20 <= len(data):
        size = struct.unpack_from('<I', data, at)[0]
        kind = data[at + 4]
        if kind == 0 or size < 0x20:
            return
        yield kind, data[at + 8:at + 0x20].split(b'\0')[0].decode('ascii', 'replace'), data[at + 0x20:at + size]
        at += size


def u16(data, at):
    return struct.unpack_from('<H', data, at)[0]


def run(esb, at, size, shape):
    """The keys of a run: (frame, back to, shape and flags, word, values); None if it
    does not end."""
    keys = []
    while at + 2 <= len(esb):
        frame = u16(esb, at)
        if frame == END:
            return keys
        if at + size > len(esb) or len(keys) > 400:
            return None
        back, how = struct.unpack_from('<bB', esb, at + 2)
        word = struct.unpack_from('<I', esb, at + 4)[0]
        keys.append((frame, back, how, word, struct.unpack_from(shape, esb, at + 8)))
        at += size
    return None


class Script:
    def __init__(self, esb, version):
        self.esb = esb
        self.version = version
        self.root = u16(esb, 0)
        self.entries = []
        if not self.root:
            return
        root = self.root
        self.length, first, self.flags = u16(esb, root), u16(esb, root + 2), u16(esb, root + 4)
        self.root_flags, self.root_vector = u16(esb, root - 4), u16(esb, root - 2)
        step = 0xe if version == 100 else 0x10
        at = first
        while at and at + step <= len(esb) and u16(esb, at) < END:
            self.entries.append(at)
            at += step

    def entry(self, at):
        esb = self.esb
        start, end, element = struct.unpack_from('<HHH', esb, at)
        parent, flags7, byte8, byte9 = struct.unpack_from('<bbbb', esb, at + 6)
        four, byte_c, byte_d = u16(esb, at + 0xa), struct.unpack_from('<b', esb, at + 0xc)[0], esb[at + 0xd]
        mode = esb[at + 0xe] if self.version != 100 else None
        return dict(at=at, start=start, end=end, element=element, parent=parent, flags7=flags7, byte8=byte8, byte9=byte9, four=four, byte_c=byte_c, byte_d=byte_d, mode=mode)

    def element(self, at):
        esb = self.esb
        word0, word2 = u16(esb, at), u16(esb, at + 2)
        scale = struct.unpack_from('<f', esb, at + 4)[0]
        kind = u16(esb, at + 8)
        offsets = [u16(esb, at + 0xa + 2 * index) for index in range(11)]
        return dict(at=at, word0=word0, word2=word2, scale=scale, kind=kind, offsets=offsets)

    def runs(self, element):
        """Every run of an element: (name, offset, size, keys or None)."""
        out = []
        for (name, size, shape), offset in zip(PARTS, element['offsets']):
            if not offset:
                continue
            out.append((name, offset, size, run(self.esb, offset, size, shape)))
            if name == '+a4':
                for (other, other_size, other_shape), before in zip(BEFORE_A4, (4, 2)):
                    sub = u16(self.esb, offset - before)
                    if sub:
                        out.append((other, sub, other_size, run(self.esb, sub, other_size, other_shape)))
        return out


def show(path, which=None):
    with open(path, 'rb') as f:
        data = f.read()
    version = u16(data, 4)
    effects = [(name, part) for kind, name, part in chain(data) if kind == 1]
    for index, (name, esb) in enumerate(effects):
        if which is not None and which not in (str(index), name[2:8], name):
            continue
        script = Script(esb, version)
        print(f'{name} ({len(esb):#x} bytes, version {version}): root {script.root:#x}')
        if not script.root:
            continue
        print(f'  length {script.length} flags {script.flags:#x}; before the root: {script.root_flags:#x}, vector at {script.root_vector:#x}')
        for at in script.entries:
            entry = script.entry(at)
            print('  entry at {at:#x}: frames {start}..{end} element {element:#x} parent {parent} +7 {flags7} +8 {byte8} +9 {byte9} four at {four:#x} +c {byte_c} +d {byte_d} mode {mode}'.format(**entry))
            if not entry['element']:
                continue
            element = script.element(entry['element'])
            print('    element: kind {kind} +0 {word0:#x} +2 {word2:#x} scale {scale:g}'.format(**element))
            for name, offset, size, keys in script.runs(element):
                figures = []
                for back, shape, what in BEFORE.get(name, []):
                    value = struct.unpack_from(shape, esb, offset - back)[0]
                    figures.append((what + ' ' if what else '') + (f'{value:g}' if isinstance(value, float) else str(value)))
                print(f'      {name} at {offset:#x}' + (': ' + ', '.join(figures) if figures else ''))
                if keys is None:
                    print('         does not end')
                    continue
                for frame, back, how, word, values in keys:
                    shown = ', '.join(f'{value:g}' if isinstance(value, float) else str(value) for value in values)
                    extra = (f' back to {back}' if back >= 0 else '') + (f' shape {how:#x}' if how else '') + (f' word {word:#x}' if word else '')
                    print(f'         {frame:4}: {shown}{extra}')


def every():
    kinds = collections.Counter()
    ends = collections.Counter()
    lengths = collections.Counter()
    shapes = collections.Counter()
    count = 0
    for folder, _, names in os.walk(ROOT):
        for name in names:
            if os.path.splitext(name)[1].lower() not in ('.efc', '.efp'):
                continue
            with open(os.path.join(folder, name), 'rb') as f:
                data = f.read()
            version = u16(data, 4)
            for kind, part, esb in chain(data):
                if kind != 1:
                    continue
                count += 1
                script = Script(esb, version)
                for at in script.entries:
                    entry = script.entry(at)
                    if not entry['element']:
                        continue
                    element = script.element(entry['element'])
                    kinds[element['kind']] += 1
                    for part_name, _, _, keys in script.runs(element):
                        ends[(part_name, keys is not None)] += 1
                        if keys is not None:
                            lengths[(part_name, min(len(keys), 9))] += 1
                            for _, back, how, word, _ in keys:
                                shapes[(part_name, how, back >= 0, word != 0)] += 1
    print(count, 'effects; element kinds:', dict(sorted(kinds.items())))
    print('runs that end where their key size says (True) or do not (False):')
    for (part_name, good), number in sorted(ends.items()):
        print(f'   {part_name:12} {good}: {number}')
    print('how many keys the runs have (9 = nine or more):')
    for part_name in sorted({name for name, _ in lengths}):
        print(f'   {part_name:12}', {n: number for (name, n), number in sorted(lengths.items()) if name == part_name})
    print('key heads by (run, byte +3, goes back, word +4 set):')
    for key, number in sorted(shapes.items()):
        print('  ', key, number)


if __name__ == '__main__':
    if sys.argv[1] == '--all':
        every()
    else:
        path = sys.argv[1] if os.path.isabs(sys.argv[1]) else os.path.join(ROOT, sys.argv[1])
        show(path, sys.argv[2] if len(sys.argv) > 2 else None)
