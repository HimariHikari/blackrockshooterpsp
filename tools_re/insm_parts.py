"""Dump an INSM model's table E (the "parts" the engine shows and hides by id) and the
submesh and node tables next to it. Exploration only.

Usage: insm_parts.py <file relative to game/unpacked> [...]
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load

for rel in sys.argv[1:]:
    d = load(rel)
    tex, mat, _, nodes, binds, parts = d[5], d[6], d[7], d[8], d[9], d[10]
    sub_count, mat_at, sub_at, node_at, bind_at, part_at = struct.unpack_from('<6I', d, 0x0C)
    print(f'{rel}: {tex} textures, {mat} materials, {nodes} nodes, {binds} bindings, {parts} parts, {sub_count} submeshes')
    for i in range(parts):
        e = d[part_at + i * 16:part_at + i * 16 + 16]
        print(f'  part {i}: {e.hex(" ", 2)}   id {struct.unpack_from("<H", e, 2)[0]}')
    print('  submeshes: ' + ' '.join(f'{i}:{d[sub_at + i * 8:sub_at + i * 8 + 4].hex()}' for i in range(sub_count)))
    ids = [(i, d[node_at + i * 0x20 + 1]) for i in range(nodes)]
    print('  node ids: ' + ' '.join(f'{i}:{n}' for i, n in ids if n != 255))
    for i in range(binds):
        e = d[bind_at + i * 12:bind_at + i * 12 + 12]
        skinned, rigid, node = e[0], e[1], struct.unpack_from('<H', e, 2)[0]
        if rigid:
            print(f'  binding {i}: node {node} (id {d[node_at + node * 0x20 + 1]}) skinned {skinned} rigid {rigid}')
