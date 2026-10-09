"""Summarise the animations in a folder: length, looping, root motion. Exploration only.

Usage: insa_summary.py <folder relative to game/unpacked>
"""
import math, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import ROOT

folder = os.path.join(ROOT, sys.argv[1])
FRAME = 0x1000


def keys8(d, off, count):
    return [struct.unpack_from('<If', d, off + k * 8) for k in range(count)]


for name in sorted(os.listdir(folder)):
    p = os.path.join(folder, name)
    d = open(p, 'rb').read()
    if d[:4] != b'INSA':
        continue
    ver, ncount, mcount, x0c, dur, nodes_at, sec2, end, t0, t1 = struct.unpack_from('<IHHIIIIIii', d, 4)
    last = 0
    info = []
    motion = 0.0
    for i in range(ncount):
        at = nodes_at + i * 0x24
        ntracks = d[at + 1]
        tracks = struct.unpack_from('<I', d, at + 4)[0]
        for t in range(ntracks):
            ta = tracks + t * 0x18
            kind, mode = d[ta], d[ta + 1]
            if mode == 0:
                continue
            if kind == 4:
                count, off = struct.unpack_from('<II', d, ta + 4)
                times = [struct.unpack_from('<I', d, off + k * 20)[0] for k in range(count)]
                last = max(last, times[-1] if times else 0)
                if count > 2:
                    motion += count
            elif kind in (0, 2) and mode in (1, 3):
                counts = struct.unpack_from('<3H', d, ta + 4)
                offs = struct.unpack_from('<3I', d, ta + 12)
                for c in range(1 if mode == 3 else 3):
                    ks = keys8(d, offs[c], counts[c])
                    if ks:
                        last = max(last, ks[-1][0])
                    if i == 0 and kind == 0 and ks:
                        vals = [v for _, v in ks]
                        info.append(f'{"xyz"[c]}:{min(vals):.0f}..{max(vals):.0f} (start {vals[0]:.0f} end {vals[-1]:.0f})')
    window = '' if (t0, t1) == (0, dur) else f' window {t0 / FRAME:.1f}..{t1 / FRAME:.1f}'
    print(f'{name}: v{ver} {ncount} nodes, {last / FRAME:6.1f} frames ({last / FRAME / 30:.2f}s), keyed-rotation weight {motion:5.0f}{window}  root {" ".join(info)}')
