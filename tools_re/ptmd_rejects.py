"""List PTMD magic occurrences in the unpacked tree that the Rust header check rejects. Exploration only."""
import collections, os, re, struct

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
BPP = {0: 16, 1: 16, 2: 16, 3: 32, 4: 4, 5: 8}
PAL = {4: 64, 5: 1024}


def hx(b):
    return ' '.join(f'{x:02X}' for x in b)


def check(d, o):
    """Mirror of brs_formats::ptmd::parse_header. Returns (ok, reason, total_len)."""
    if len(d) - o < 16:
        return False, 'truncated header', 0
    if d[o + 4] != 2:
        return False, f'version {d[o+4]}', 0
    dims = d[o + 6] | (d[o + 7] << 8)
    w, h = 1 << (dims >> 12), dims & 0xfff
    fmt = d[o + 8]
    if fmt not in BPP:
        return False, f'format {fmt}', 0
    size = struct.unpack_from('<I', d, o + 12)[0]
    if size < 16:
        return False, 'size<16', 0
    data = size - 16
    exp = w * h * BPP[fmt] // 8
    if h == 0:
        return False, 'height 0', 0
    if w > 1024:
        return False, f'width {w}', 0
    if data != exp and data != (exp + 15) // 16 * 16:
        return False, f'size mismatch fmt={fmt} {w}x{h} expected={exp:#x} got={data:#x}', 0
    pfmt = d[o + 9] >> 4
    if pfmt > 3:
        return False, f'palette format {pfmt}', 0
    total = 16 + data + PAL.get(fmt, 0) // (1 if pfmt == 3 else 2)
    if o + total > len(d):
        return False, f'runs past end by {o + total - len(d):#x} (fmt={fmt})', 0
    return True, '', total


magic_total = accepted = 0
reasons = collections.Counter()
examples = {}
for dp, _, fs in os.walk(ROOT):
    if dp.endswith('.tex'):
        continue
    for f in fs:
        if f.endswith('.png'):
            continue
        p = os.path.join(dp, f)
        d = open(p, 'rb').read()
        pos = 0
        while True:
            o = d.find(b'PTMD', pos)
            if o < 0:
                break
            magic_total += 1
            ok, why, total = check(d, o)
            if ok:
                accepted += 1
                pos = o + total
            else:
                key = re.sub(r'\d+x\d+ expected=\S+ got=\S+', '', why)
                reasons[why if 'mismatch' not in why else why] += 1
                examples.setdefault(why, []).append((os.path.relpath(p, ROOT), o, hx(d[o + 4:o + 16]), hx(d[o + 16:o + 32])))
                pos = o + 4

print('PTMD magics', magic_total, 'accepted', accepted, 'rejected', magic_total - accepted)
for why, n in reasons.most_common(40):
    print(f'{n:6}  {why}')
    for rel, o, h, nxt in examples[why][:3]:
        print(f'          {rel} @{o:#x}  [{h}]  next: {nxt}')
