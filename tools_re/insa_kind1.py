"""Look at kind-1 tracks in field animations. Exploration only.

Usage: insa_kind1.py <animation file relative to game/unpacked>
"""
import math, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load

d = load(sys.argv[1])
ver, ncount, mcount, x0c, dur, nodes_at, sec2, end, t0, t1 = struct.unpack_from('<IHHIIIIIii', d, 4)
print(f'{sys.argv[1]}: version {ver}, {ncount} nodes, duration {dur:#x}, size {len(d):#x}')
for i in range(min(ncount, 4)):
    at = nodes_at + i * 0x24
    flags, ntracks, nid, nxt = d[at:at + 4]
    tracks = struct.unpack_from('<I', d, at + 4)[0]
    print(f'node {i}: flags={flags:02X} tracks={ntracks} id={nid:02X} vecA={struct.unpack_from("<3f", d, at + 8)} vecB={struct.unpack_from("<3f", d, at + 20)}')
    for t in range(ntracks):
        ta = tracks + t * 0x18
        raw = d[ta:ta + 0x18]
        kind, mode = raw[0], raw[1]
        print(f'   track {t} kind={kind} mode={mode}: {raw.hex(" ")}')
        if kind == 1 and mode == 1:
            counts = struct.unpack_from('<3H', d, ta + 4)
            offs = struct.unpack_from('<3I', d, ta + 12)
            for c in range(3):
                keys = [struct.unpack_from('<If', d, offs[c] + k * 8) for k in range(min(counts[c], 5))]
                print(f'      axis {c}: {counts[c]} keys at {offs[c]:#x}: ' + ', '.join(f'({t:#x}, {v:.4f} = {math.degrees(v):.1f} deg)' for t, v in keys))
