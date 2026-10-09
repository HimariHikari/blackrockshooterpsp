"""Print the scale tracks of an animation that are not constantly 1. Exploration only.

Usage: insa_scale.py <animation file relative to game/unpacked>
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load

d = load(sys.argv[1])
ver, ncount, mcount, x0c, dur, nodes_at = struct.unpack_from('<IHHIII', d, 4)
print(f'{sys.argv[1]}: version {ver}, {ncount} nodes, duration {dur / 0x1000:.1f} frames')
shown = 0
for i in range(ncount):
    at = nodes_at + i * 0x24
    ntracks = d[at + 1]
    tracks = struct.unpack_from('<I', d, at + 4)[0]
    for t in range(ntracks):
        ta = tracks + t * 0x18
        kind, mode = d[ta], d[ta + 1]
        if kind != 2 or mode == 0:
            continue
        counts = struct.unpack_from('<3H', d, ta + 4)
        offs = struct.unpack_from('<3I', d, ta + 12)
        raw = d[ta:ta + 0x18].hex(' ')
        chans = []
        for c in range(1 if mode == 3 else 3):
            keys = [struct.unpack_from('<If', d, offs[c] + k * 8) for k in range(counts[c])]
            chans.append(keys)
        flat = [v for ch in chans for _, v in ch]
        if any(abs(v - 1.0) > 1e-3 for v in flat) or i < 2:
            shown += 1
            print(f'node {i:2} id={d[at + 2]:02X} scale mode {mode} counts {counts}: {raw}')
            for c, keys in enumerate(chans):
                print('     ' + ' '.join(f'({tm / 0x1000:.1f}f {v:.3f})' for tm, v in keys[:10]))
print('non-trivial scale tracks shown:', shown)
