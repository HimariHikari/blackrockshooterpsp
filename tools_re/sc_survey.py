"""Survey SC bundles: slot layout and what each slot holds. Exploration only."""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def magic(b):
    m = b[:4]
    return ''.join(chr(x) if 32 <= x < 127 else '.' for x in m) if m else '(empty)'


counts = collections.Counter()
slot_magic = collections.defaultdict(collections.Counter)
slot_sizes = collections.defaultdict(list)
dirs = collections.defaultdict(collections.Counter)
bad = []
examples = {}
for dp, _, fs in os.walk(ROOT):
    if dp.endswith('.tex'):
        continue
    for f in fs:
        if f.endswith('.png'):
            continue
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            head = fh.read(4)
            if head[:2] != b'SC' or head[3] != 0 or head[2] not in (6, 8, 16):
                continue
            d = head + fh.read()
        n = d[2]
        offs = struct.unpack_from(f'<{n + 1}I', d, 4)
        rel = os.path.relpath(p, ROOT)
        counts[n] += 1
        if offs[-1] != len(d) or any(offs[i] > offs[i + 1] for i in range(n)):
            bad.append((rel, n, [hex(o) for o in offs], hex(len(d))))
            continue
        top = rel.split(os.sep)[0] + '\\' + (rel.split(os.sep)[1] if rel.count(os.sep) > 1 else '')
        for i in range(n):
            if offs[i + 1] > offs[i]:
                m = magic(d[offs[i]:offs[i + 1]])
                slot_magic[(n, i)][m] += 1
                slot_sizes[(n, i)].append(offs[i + 1] - offs[i])
                dirs[(n, i)][top] += 1
                examples.setdefault((n, i, m), rel)

print('bundles by slot count:', dict(counts), ' problems:', len(bad))
for b in bad[:5]:
    print('   bad:', b)
for key in sorted(slot_magic):
    n, i = key
    sizes = slot_sizes[key]
    print(f'SC{n:<2} slot {i:2}: {len(sizes):4} used, size {min(sizes):>8}..{max(sizes):<8} magics {dict(slot_magic[key].most_common(4))}  dirs {dict(dirs[key].most_common(3))}')
    for m, _ in slot_magic[key].most_common(2):
        print(f'          e.g. [{m}] {examples[(n, i, m)]}')
