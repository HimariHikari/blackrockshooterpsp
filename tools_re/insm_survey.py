"""Survey all INSM files: header counts, GE commands used, vertex types. Exploration only."""
import collections, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import GE, ROOT, vsize, vtype_str

cmds = collections.Counter()
vtypes = collections.Counter()
prims = collections.Counter()
hdr4 = collections.Counter()
exts = collections.Counter()
bfirst = collections.Counter()
node_b0 = collections.Counter()
bad = []
seen = set()
nfiles = 0
for dp, _, fs in os.walk(ROOT):
    if dp.endswith('.tex'):
        continue
    for f in fs:
        if f.endswith('.png'):
            continue
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            if fh.read(4) != b'INSM':
                continue
            d = b'INSM' + fh.read()
        ext = os.path.splitext(f)[1].lower()
        exts[ext] += 1
        if d in seen:
            continue
        seen.add(d)
        nfiles += 1
        a, b, c, dd, e, ff, dls, dlsz, x30, x34 = struct.unpack_from('<10I', d, 0x10)
        hdr4[(ext, d[4:8].hex(), d[10])] += 1
        nodes, meshes = d[8], d[9]
        if (c - b) != meshes * 8 or (dd - c) != nodes * 0x20:
            bad.append((os.path.relpath(p, ROOT), 'section sizes', nodes, meshes, hex(b), hex(c), hex(dd)))
        for i in range(meshes):
            bfirst[struct.unpack_from('<I', d, b + i * 8)[0]] += 1
        for i in range(nodes):
            node_b0[d[c + i * 0x20]] += 1
        o, end = dls, dls + dlsz
        if end > len(d):
            bad.append((os.path.relpath(p, ROOT), 'dl out of range'))
            continue
        while o + 4 <= end:
            w = struct.unpack_from('<I', d, o)[0]
            cmd, arg = w >> 24, w & 0xffffff
            cmds[cmd] += 1
            if cmd == 0x12:
                vtypes[arg] += 1
            elif cmd == 0x04:
                prims[(arg >> 16) & 7] += 1
            o += 4

print('INSM files by extension:', dict(exts), ' unique:', nfiles)
print('header (ext, bytes4..8, byte10):')
for k, v in hdr4.most_common():
    print('   ', v, k)
print('GE commands:')
for k, v in sorted(cmds.items()):
    print(f'    {k:02X} {GE.get(k, "?"):<8} {v}')
print('vertex types:')
for k, v in vtypes.most_common():
    print(f'    {k:06X} x{v}  {vtype_str(k)} vsize={vsize(k)}')
print('prim types:', dict(prims))
print('mesh table first u32:', sorted(bfirst.items()))
print('node byte0:', sorted(node_b0.items()))
print('problems:', len(bad))
for x in bad[:20]:
    print('   ', x)
