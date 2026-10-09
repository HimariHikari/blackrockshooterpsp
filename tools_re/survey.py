"""Survey container and texture headers across GAMEDATA. Exploration only."""
import gzip, os, re, struct, sys, zlib, collections

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'extracted', 'PSP_GAME', 'USRDIR', 'GAMEDATA')


def hx(b):
    return ' '.join(f'{x:02X}' for x in b)


def unwrap(d):
    """Peel gzip / CZAA layers."""
    if d[:3] == b'\x1f\x8b\x08':
        return gzip.decompress(d)
    if d[:4] == b'CZAA':
        return zlib.decompress(d[16:])
    return d


def rtdp(d):
    hdr, count, total, key = struct.unpack_from('<IIII', d, 4)
    out = []
    for i in range(count):
        o = 0x20 + i * 40
        name = d[o:o + 32].split(b'\0')[0].decode('latin1')
        size, off = struct.unpack_from('<II', d, o + 32)
        out.append((name, bytes(b ^ 0x55 for b in d[hdr + off: hdr + off + size])))
    return out


blobs = []  # (label, bytes)
exts = collections.Counter()
for dp, _, fs in os.walk(ROOT):
    for f in fs:
        p = os.path.join(dp, f)
        if f.upper().endswith('.AT3'):
            continue
        d = unwrap(open(p, 'rb').read())
        rel = os.path.relpath(p, ROOT)
        if d[:4] == b'RTDP':
            for name, sub in rtdp(d):
                exts[os.path.splitext(name)[1].lower() + ' magic=' + repr(sub[:4])] += 1
                blobs.append((rel + ':' + name, unwrap(sub) if sub[:4] == b'CZAA' else sub))
        else:
            blobs.append((rel, d))

print('== RTDP member types')
for k, v in sorted(exts.items(), key=lambda kv: -kv[1]):
    print(f'  {v:5} {k}')

print('== top-level magics')
mg = collections.Counter(repr(b[:4]) for _, b in blobs)
for k, v in mg.most_common(30):
    print(f'  {v:5} {k}')

print('== PTMD headers')
pt = collections.Counter()
examples = {}
total = 0
for label, b in blobs:
    for m in re.finditer(b'PTMD', b):
        o = m.start()
        h = b[o:o + 16]
        if len(h) < 16:
            continue
        total += 1
        key = hx(h[4:8])
        pt[key] += 1
        examples.setdefault(key, []).append((label, o, hx(h[8:16])))
print('total PTMD', total)
for k, v in pt.most_common(60):
    ex = examples[k][:3]
    print(f'  {v:5}  [{k}]  ' + ' | '.join(f'{l}@{o:#x} {r}' for l, o, r in ex))
