"""Cross-check INSM header counts against table sizes for every model. Exploration only."""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import ROOT

rows = []
seen = set()
for dp, _, fs in os.walk(ROOT):
    if dp.endswith('.tex'):
        continue
    for f in fs:
        if f.endswith('.png'):
            continue
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            d = fh.read()
        if d[:4] != b'INSM' or d[7] != 0 or d in seen:
            continue
        seen.add(d)
        a, b, c, dd, e, ff, dls, dlsz, bufsz, x34 = struct.unpack_from('<10I', d, 0x10)
        nsub = struct.unpack_from('<I', d, 0x0C)[0]
        mats_by_size = (b - a) / 0x24
        subs = [(d[b + i * 8], d[b + i * 8 + 1], d[b + i * 8 + 2], d[b + i * 8 + 3]) for i in range(nsub)]
        max_mat = max(s[2] for s in subs) if subs else -1
        b3 = sorted(set(s[3] for s in subs))
        nmat = int(mats_by_size)
        tex_idx = [d[a + i * 0x24 + 1] for i in range(nmat)]
        mat_b0 = [d[a + i * 0x24] for i in range(nmat)]
        ntex = d.count(b'PTMD', ff)
        slots_needed = sum(max(s[1], 1) for s in subs)
        nbind = d[9]
        slots_have = sum(d[dd + i * 12] + d[dd + i * 12 + 1] for i in range(nbind))
        rows.append((os.path.relpath(p, ROOT), d[5], d[6], mats_by_size, max_mat, tex_idx, mat_b0, ntex, d[10], slots_needed, slots_have, b3, x34 - ff))

print(f'{len(rows)} unique models')
print('file | hdr5 hdr6 | (B-A)/0x24 | max submesh mat | material tex idx | material byte0 | PTMD count | hdr10 | slots need/have | sub byte3 | x34-F')
for r in rows:
    odd = r[1] != r[3] or r[4] >= r[3] or r[9] != r[10] or r[2] != r[7]
    if odd or '--all' in sys.argv:
        print(('* ' if odd else '  ') + ' | '.join(str(x) for x in r))
