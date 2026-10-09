"""Classify CLIM triangles as floor-like or wall-like per slot and attribute. Exploration only."""
import collections, math, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def read(d):
    root_at, verts_at = struct.unpack_from('<II', d, 4)
    nattr = struct.unpack_from('<H', d, 0x12)[0]
    attrs = [d[0x14 + i * 4:0x18 + i * 4].hex() for i in range(nattr)]
    verts = [struct.unpack_from('<3f', d, o) for o in range(verts_at, len(d), 12)]
    groups = set()
    stack = [root_at]
    while stack:
        at = stack.pop()
        flags = struct.unpack_from('<I', d, at)[0]
        ref = struct.unpack_from('<I', d, at + 36)[0]
        for g in range(flags >> 8):
            groups.add(ref + g * 8)
        stack.extend(k for k in struct.unpack_from('<8I', d, at + 4) if k)
    tris = []
    for g in sorted(groups):
        attr, count, idx = struct.unpack_from('<HHI', d, g)
        for t in range(count):
            i = struct.unpack_from('<3H', d, idx + t * 6)
            tris.append((attr >> 8, [verts[k] for k in i]))
    return attrs, verts, tris


table = collections.defaultdict(collections.Counter)
attr_bytes = collections.defaultdict(collections.Counter)
ybounds = {}
for dp, _, fs in os.walk(ROOT):
    for f in fs:
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            if fh.read(4) != b'CLIM':
                continue
            d = b'CLIM' + fh.read()
        if len(d) < 0x20:
            continue
        slot = f[:2]
        attrs, verts, tris = read(d)
        for i, a in enumerate(attrs):
            attr_bytes[slot][(i, a)] += 1
        for attr, (a, b, c) in tris:
            e1 = [b[i] - a[i] for i in range(3)]
            e2 = [c[i] - a[i] for i in range(3)]
            n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
            ln = math.sqrt(sum(x * x for x in n)) or 1.0
            ny = n[1] / ln
            kind = 'floor(up)' if ny > 0.7 else 'ceiling(down)' if ny < -0.7 else 'wall' if abs(ny) < 0.3 else 'slope'
            table[(slot, attr)][kind] += 1
        if 'STG01_01' in p:
            ys = [v[1] for v in verts]
            ybounds[slot] = (min(ys), max(ys), len(tris))

for key in sorted(table):
    print(f'slot {key[0]} attr {key[1]}: {dict(table[key])}')
for slot in sorted(attr_bytes):
    print(f'slot {slot} attribute table entries:', sorted(attr_bytes[slot].items()))
print('MAP_D_STG01_01 y range and triangle count per slot:', ybounds)
