"""Find animations with kind-1 tracks and show where they live and what the keys look like. Exploration only."""
import collections, math, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import ROOT

where = collections.Counter()
shapes = collections.Counter()
shown = 0
for dp, _, fs in os.walk(ROOT):
    for f in fs:
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            if fh.read(4) != b'INSA':
                continue
            d = b'INSA' + fh.read()
        ver, ncount, mcount, x0c, dur, nodes_at = struct.unpack_from('<IHHIII', d, 4)
        found = None
        for i in range(ncount):
            at = nodes_at + i * 0x24
            ntracks = d[at + 1]
            tracks = struct.unpack_from('<I', d, at + 4)[0]
            kinds = tuple((d[tracks + t * 0x18], d[tracks + t * 0x18 + 1]) for t in range(ntracks))
            if any(k == 1 and m == 1 for k, m in kinds):
                found = (i, tracks, kinds)
                break
        if not found:
            continue
        rel = os.path.relpath(p, ROOT)
        parts = rel.split(os.sep)
        where[(parts[0], parts[1] if len(parts) > 2 else '', os.path.basename(os.path.dirname(p)))] += 1
        shapes[(ver, ncount, found[2])] += 1
        if shown < 2:
            shown += 1
            i, tracks, kinds = found
            print(rel, f'version {ver}, {ncount} nodes, duration {dur:#x}')
            for t in range(len(kinds)):
                ta = tracks + t * 0x18
                raw = d[ta:ta + 0x18]
                print(f'   node {i} track {t}: {raw.hex(" ")}')
                if raw[0] == 1 and raw[1] == 1:
                    counts = struct.unpack_from('<3H', d, ta + 4)
                    offs = struct.unpack_from('<3I', d, ta + 12)
                    for c in range(3):
                        keys = [struct.unpack_from('<If', d, offs[c] + k * 8) for k in range(min(counts[c], 4))]
                        print(f'      axis {c}: {counts[c]} keys at {offs[c]:#x}: ' + ', '.join(f'({tm:#x}, {v:.4f} rad = {math.degrees(v):.1f} deg)' for tm, v in keys))

print('where (top dir, sub dir, containing folder):')
for k, v in where.most_common(12):
    print('  ', v, k)
print('shapes (version, node count, (kind, mode) per track):')
for k, v in shapes.most_common(8):
    print('  ', v, k)
