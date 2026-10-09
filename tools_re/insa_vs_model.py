"""Compare an animation's first keys with the model's rest pose, node by node. Exploration only.

Usage: insa_vs_model.py <model> <animation>
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load
from insm_obj import parse

model = parse(load(sys.argv[1]))
d = load(sys.argv[2])
ver, ncount, mcount, x0c, dur, nodes_at, sec2, end, t0, t1 = struct.unpack_from('<IHHIIIIIii', d, 4)
print(f'anim: version {ver}, {ncount} nodes, second section {mcount}, x0c={x0c:#x}, duration {dur:#x}, window {t0:#x}..{t1:#x}')
print(f'model: {len(model["nodes"])} nodes')


def fmt(v):
    return '(' + ','.join(f'{x:7.3f}' for x in v) + ')'


for i in range(ncount):
    at = nodes_at + i * 0x24
    flags, ntracks, nid, nxt = d[at:at + 4]
    tracks = struct.unpack_from('<I', d, at + 4)[0]
    vec_a = struct.unpack_from('<3f', d, at + 8)
    vec_b = struct.unpack_from('<3f', d, at + 20)
    tail = struct.unpack_from('<I', d, at + 32)[0]
    first = {}
    kinds = []
    for t in range(ntracks):
        ta = tracks + t * 0x18
        kind, mode = d[ta], d[ta + 1]
        counts = struct.unpack_from('<3H', d, ta + 4)
        offs = struct.unpack_from('<3I', d, ta + 12)
        kinds.append(f'{kind}/{mode}:{counts[0]},{counts[1]},{counts[2]}')
        if mode == 0:
            continue
        if kind == 4:
            first['rot'] = struct.unpack_from('<4f', d, offs[0] + 4)
        elif kind in (0, 2):
            vals = []
            for c in range(3):
                o = offs[0] if mode == 3 else offs[c]
                vals.append(struct.unpack_from('<f', d, o + 4)[0] if (counts[0] if mode == 3 else counts[c]) else float('nan'))
            first['pos' if kind == 0 else 'scl'] = vals
    line = f'{i:2} flags={flags:02X} id={nid:02X} next={nxt:3} vecA={fmt(vec_a)} vecB={fmt(vec_b)} tail={tail:#x} tracks[{" ".join(kinds)}]'
    print(line)
    if i < len(model['nodes']):
        n = model['nodes'][i]
        print(f'     model: flags={n["flags"]:02X} id={n["id"]:02X} parent={n["parent"]:3} next={n["next"]:3} t={fmt(n["t"])} q={fmt(n["q"])}')
    print(f'     anim : pos={fmt(first.get("pos", []))} rot={fmt(first.get("rot", []))} scl={fmt(first.get("scl", []))}')
