"""Work out palette size per header from standalone PTMD files, whose exact length is known. Exploration only."""
import collections, os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
BPP = {0: 16, 1: 16, 2: 16, 3: 32, 4: 4, 5: 8}

table = collections.defaultdict(collections.Counter)
examples = {}
files = 0
for dp, _, fs in os.walk(ROOT):
    if dp.endswith('.tex'):
        continue
    for f in fs:
        if f.endswith('.png'):
            continue
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            head = fh.read(16)
        if head[:4] != b'PTMD' or len(head) < 16:
            continue
        files += 1
        total = os.path.getsize(p)
        flags, fmt, b9 = head[5], head[8], head[9]
        dims = head[6] | (head[7] << 8)
        w, h = 1 << (dims >> 12), dims & 0xfff
        size = struct.unpack_from('<I', head, 12)[0]
        exp = w * h * BPP.get(fmt, 0) // 8
        key = (flags, fmt, b9)
        left = total - size
        table[key][(left, 'data==exp' if size - 16 == exp else f'data-exp={size - 16 - exp:+#x}')] += 1
        examples.setdefault((key, left), os.path.relpath(p, ROOT))

print('standalone PTMD files:', files)
print('flags fmt byte9 -> {(bytes after pixel data, data check): count}')
for key in sorted(table):
    flags, fmt, b9 = key
    print(f'  flags={flags:#04x} fmt={fmt} b9={b9:#04x}:')
    for (left, chk), n in table[key].most_common():
        print(f'      {n:6}  trailing={left:#06x}  {chk}   e.g. {examples[(key, left)]}')
