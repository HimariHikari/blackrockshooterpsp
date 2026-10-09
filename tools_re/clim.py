"""CLIM collision file dumper. Exploration only.

Usage: clim.py <file relative to game/unpacked>
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load

d = load(sys.argv[1])
bias, data_at, x0c = struct.unpack_from('<III', d, 4)
n16 = struct.unpack_from('<4H', d, 0x10)
print(f'# {sys.argv[1]} size={len(d):#x} bias={bias:#x} data_at={data_at:#x} (file {data_at - bias:#x}) x0c={x0c:#x} u16s@0x10={n16}')
print('header 0x10..0x24:', d[0x10:0x24].hex(' '))


def real(o):
    return o - bias


table_at = bias  # the table seems to start where the header ends
table = struct.unpack_from('<10I', d, table_at)
print(f'table at {table_at:#x}:', ' '.join(f'{x:#x}' for x in table))

nodes_start = min(real(x) for x in table[:9] if x)
nodes_end = real(table[9])
print(f'nodes {nodes_start:#x}..{nodes_end:#x} = {(nodes_end - nodes_start) / 0x40} nodes')

leaf_refs = set()
for at in range(nodes_start, nodes_end, 0x40):
    ref = struct.unpack_from('<I', d, at)[0]
    bb = struct.unpack_from('<6f', d, at + 4)
    extra = struct.unpack_from('<I', d, at + 28)[0]
    kids = struct.unpack_from('<8I', d, at + 32)
    leaf_refs.add(ref)
    if at < nodes_start + 0x40 * 12:
        print(f'  node {at:#06x}: ref={ref:#x}->{real(ref):#x} x1c={extra:#x} bb=({bb[0]:.0f},{bb[1]:.0f},{bb[2]:.0f})..({bb[3]:.0f},{bb[4]:.0f},{bb[5]:.0f}) kids=[{" ".join(f"{real(k):x}" if k else "-" for k in kids)}]')
print('distinct leaf refs:', len(leaf_refs), sorted(hex(real(r)) for r in leaf_refs)[:12], '...')

after = nodes_end
print(f'-- after nodes {after:#x}..{real(data_at):#x} ({real(data_at) - after:#x} bytes) as u32:')
for i in range(after, min(real(data_at), after + 0xC0), 16):
    print(f'  {i:06X}  ' + ' '.join(f'{v:8X}' for v in struct.unpack_from('<4I', d, i)))
da = real(data_at)
print(f'-- data at {da:#x}..{len(d):#x} ({len(d) - da:#x} bytes)')
for i in range(da, min(len(d), da + 0x80), 16):
    row = d[i:i + 16]
    print(f'  {i:06X}  {row.hex(" ")}')
print('   as floats:')
for i in range(da, min(len(d), da + 0xA0), 16):
    print(f'  {i:06X}  ' + ' '.join(f'{v:12.3f}' for v in struct.unpack_from('<4f', d, i)))
print(f'   x0c={x0c} -> records of that size: {(len(d) - da) / x0c}')
