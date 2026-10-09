"""Reads the interface's layouts (`SDOa`): a header, a record for each object, and
for each object a list of the PSP's drawing commands with the corners it draws.
Exploration only.

Usage: sdoa.py <file relative to game/unpacked>          one file, in full
       sdoa.py --survey [folder relative to game/unpacked]  what all of them use

Header: `SDOa`, u16 objects, u16 textures. Records (16 bytes, from 8): u16 kind,
u16 texture (0xFFFF none), s16 x, y, width, height, u32 offset of its commands.
A command is a word, its number in the top byte; an address is counted from the
start of the object's commands.
"""
import os, struct, sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

NAMES = {0x01: 'VADDR', 0x02: 'IADDR', 0x04: 'PRIM', 0x0B: 'RET', 0x10: 'BASE', 0x12: 'VTYPE', 0x14: 'ORIGIN',
         0x1E: 'TME', 0x20: '0x20', 0xC6: 'TFLT', 0xC7: 'TWRAP', 0xC9: 'TFUNC', 0xDF: 'BLEND', 0xDB: 'ATEST', 0x21: 'ABE', 0x22: 'ATE'}
PRIMS = ['points', 'lines', 'line strip', 'triangles', 'strip', 'fan', 'sprites']


def vertex_size(vtype):
    tex, col, nrm, pos = vtype & 3, (vtype >> 2) & 7, (vtype >> 5) & 3, (vtype >> 7) & 3
    size = 0
    fields = []
    def put(name, width, count):
        nonlocal size
        if width == 0:
            return
        size = (size + width - 1) // width * width
        fields.append((name, size, width, count))
        size += width * count
    put('uv', [0, 1, 2, 4][tex], 2)
    put('colour', {0: 0, 4: 2, 5: 2, 6: 2, 7: 4}.get(col, 0), 1)
    put('normal', [0, 1, 2, 4][nrm], 3)
    put('position', [0, 1, 2, 4][pos], 3)
    widest = max([w for _, _, w, _ in fields], default=1)
    size = (size + widest - 1) // widest * widest
    return size, fields


class Object:
    def __init__(self, data, at):
        self.kind, self.texture, self.x, self.y, self.w, self.h, self.offset = struct.unpack_from('<HH4hI', data, at)
        self.commands = []
        self.draws = []
        if self.offset == 0:
            return
        pc = self.offset
        vtype, vaddr, base = 0, 0, 0
        for _ in range(4096):
            if pc + 4 > len(data):
                self.commands.append(('END?', 0))
                break
            word = struct.unpack_from('<I', data, pc)[0]
            cmd, arg = word >> 24, word & 0xffffff
            self.commands.append((cmd, arg))
            pc += 4
            if cmd == 0x12:
                vtype = arg
            elif cmd == 0x10:
                base = arg
            elif cmd == 0x01:
                vaddr = arg
            elif cmd == 0x04:
                prim, count = (arg >> 16) & 7, arg & 0xffff
                size, fields = vertex_size(vtype)
                at_v = self.offset + vaddr
                vertices = []
                for n in range(count):
                    v = {}
                    for name, off, width, cnt in fields:
                        fmt = {1: 'b', 2: 'h', 4: 'f'}[width] if name != 'colour' else {2: 'H', 4: 'I'}[width]
                        vals = struct.unpack_from('<' + fmt * cnt, data, at_v + n * size + off)
                        v[name] = vals if cnt > 1 else vals[0]
                    vertices.append(v)
                self.draws.append((prim, vtype, vertices))
                vaddr += count * size
            elif cmd == 0x0B:
                break


def parse(data):
    assert data[:4] == b'SDOa', data[:4]
    objects, textures = struct.unpack_from('<HH', data, 4)
    return [Object(data, 8 + 16 * n) for n in range(objects)], textures


def show(path):
    data = open(path, 'rb').read()
    objects, textures = parse(data)
    print(f'{len(objects)} objects, {textures} textures, {len(data)} bytes')
    for n, o in enumerate(objects):
        print(f'{n:3d} kind {o.kind} texture {o.texture if o.texture != 0xffff else "-"} at ({o.x},{o.y}) size ({o.w},{o.h}) commands at {o.offset:#x}')
        print('     ' + ' '.join(f'{NAMES.get(c, hex(c))}={a:#x}' for c, a in o.commands))
        for prim, vtype, vertices in o.draws:
            print(f'     {PRIMS[prim]} vtype {vtype:#x}:')
            for v in vertices:
                parts = []
                if 'uv' in v:
                    parts.append('uv (%g, %g)' % v['uv'])
                if 'colour' in v:
                    parts.append(f'colour {v["colour"]:08x}')
                parts.append('at (%g, %g, %g)' % v['position'])
                print('       ' + ' '.join(parts))


def survey(folder):
    kinds, commands, vtypes, prims, values = Counter(), Counter(), Counter(), Counter(), {}
    files = bad = 0
    for base, _, names in os.walk(os.path.join(ROOT, folder)):
        for name in names:
            if not name.endswith('.sdoa'):
                continue
            files += 1
            path = os.path.join(base, name)
            try:
                objects, _ = parse(open(path, 'rb').read())
            except Exception as error:
                bad += 1
                print('failed', os.path.relpath(path, ROOT), error)
                continue
            for o in objects:
                kinds[o.kind] += 1
                shape = tuple(c for c, _ in o.commands)
                commands[shape] += 1
                for c, a in o.commands:
                    if c not in (0x01, 0x04, 0x12):
                        values.setdefault(c, Counter())[a] += 1
                for prim, vtype, vertices in o.draws:
                    vtypes[vtype] += 1
                    prims[(PRIMS[prim], len(vertices))] += 1
    print(files, 'files,', bad, 'failed')
    print('kinds', dict(kinds))
    print('vertex types', {hex(k): v for k, v in vtypes.items()})
    print('primitives', dict(prims))
    print('command sequences:')
    for shape, n in commands.most_common(20):
        print(f'  {n:7d}  ' + ' '.join(NAMES.get(c, hex(c)) for c in shape))
    for c, counter in sorted(values.items()):
        print(f'  {NAMES.get(c, hex(c))}: ' + ', '.join(f'{a:#x} x{n}' for a, n in counter.most_common(8)))


if __name__ == '__main__':
    if sys.argv[1] == '--survey':
        survey(sys.argv[2] if len(sys.argv) > 2 else '')
    else:
        show(sys.argv[1] if os.path.isabs(sys.argv[1]) else os.path.join(ROOT, sys.argv[1]))
