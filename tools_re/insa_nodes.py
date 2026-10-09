"""List the nodes of an INSA animation with their flags and track kinds. Exploration only.

Usage: insa_nodes.py <file relative to game/unpacked> [...]
Per node: index, id, flags byte, then each track as kind:mode(key counts).
"""
import os, struct, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'unpacked')
for rel in sys.argv[1:]:
    with open(os.path.join(ROOT, rel), 'rb') as f:
        d = f.read()
    count = struct.unpack_from('<H', d, 8)[0]
    nodes_at = struct.unpack_from('<I', d, 0x14)[0]
    print(f'{rel}: header {d[4:0x30].hex(" ", 4)} nodes {count}')
    for i in range(count):
        at = nodes_at + i * 0x24
        flags, tracks, node_id = d[at], d[at + 1], d[at + 2]
        tracks_at = struct.unpack_from('<I', d, at + 4)[0]
        parts = []
        for t in range(tracks):
            ta = tracks_at + t * 0x18
            kind, mode = d[ta], d[ta + 1]
            counts = struct.unpack_from('<3H', d, ta + 4)
            parts.append(f'{kind}:{mode}{counts if mode in (1, 3) else ""}')
        rest = d[at + 3:at + 4].hex() + ' ' + d[at + 8:at + 0x24].hex(' ', 4)
        print(f'   [{i:2}] id {node_id:3} flags {flags:#04x} {" ".join(parts)}  | {rest}')
