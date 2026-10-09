"""Count textures that take the 'assumed linear' path in the Rust unswizzle. Exploration only."""
import collections, os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
BPP = {0: 16, 1: 16, 2: 16, 3: 32, 4: 4, 5: 8}
ENTRIES = {4: 16, 5: 256}

total = 0
small = collections.Counter()
examples = {}
for dp, _, fs in os.walk(ROOT):
    if dp.endswith('.tex'):
        continue
    for f in fs:
        if f.endswith('.png'):
            continue
        p = os.path.join(dp, f)
        d = open(p, 'rb').read()
        pos, idx = 0, 0
        bare = d[:4] == b'PTMD'
        while True:
            o = d.find(b'PTMD', pos)
            if o < 0 or o + 16 > len(d):
                break
            dims = d[o + 6] | (d[o + 7] << 8)
            w, h, fmt = 1 << (dims >> 12), dims & 0xfff, d[o + 8]
            size = struct.unpack_from('<I', d, o + 12)[0]
            if d[o + 4] != 2 or fmt not in BPP or size < 16:
                pos = o + 4
                continue
            pal = ENTRIES.get(fmt, 0) * (4 if d[o + 9] >> 4 == 3 else 2)
            total += 1
            row = w * BPP[fmt] // 8
            if row < 16 or h < 8:
                key = (w, h, fmt, 'row<16' if row < 16 else 'h<8')
                small[key] += 1
                rel = os.path.relpath(p, ROOT)
                examples.setdefault(key, rel + ('.png' if bare else f'.tex\\{idx:03}.png'))
            idx += 1
            pos = o + size + pal

print('textures', total, 'on the assumed-linear path', sum(small.values()))
for (w, h, fmt, why), n in small.most_common():
    print(f'  {n:5}  {w}x{h} fmt={fmt} {why}   e.g. {examples[(w, h, fmt, why)]}')
