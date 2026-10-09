"""Check the CLIM octree theory on every file. Exploration only.

Node (0x40): u32 flags, u32 child[8], u32 group_ref, f32 bbox[6].
Group (8):   u16 attr, u16 triangle count, u32 offset of u16 index triples.
Vertices:    3 x f32 from header[8] to the end of the file.
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

stats = collections.Counter()
flag_vals = collections.Counter()
attrs = collections.Counter()
hdr_shapes = collections.Counter()
problems = []
tri_total = 0
for dp, _, fs in os.walk(ROOT):
    for f in fs:
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            if fh.read(4) != b'CLIM':
                continue
            d = b'CLIM' + fh.read()
        rel = os.path.relpath(p, ROOT)
        if len(d) < 0x20:
            stats[f'empty ({len(d)} bytes)'] += 1
            continue
        stats['files'] += 1
        root_at, verts_at, x0c = struct.unpack_from('<III', d, 4)
        node_count, h12 = struct.unpack_from('<HH', d, 0x10)
        hdr_shapes[(root_at, x0c, h12, d[0x14:root_at].hex())] += 1
        nverts = (len(d) - verts_at) / 12
        if nverts != int(nverts):
            problems.append((rel, 'vertex area not a multiple of 12'))
            continue
        nverts = int(nverts)

        seen_nodes = 0
        groups = {}
        stack = [root_at]
        ok = True
        while stack:
            at = stack.pop()
            if at + 0x40 > len(d):
                problems.append((rel, f'node {at:#x} out of range'))
                ok = False
                break
            seen_nodes += 1
            flags = struct.unpack_from('<I', d, at)[0]
            kids = struct.unpack_from('<8I', d, at + 4)
            ref = struct.unpack_from('<I', d, at + 36)[0]
            flag_vals[flags] += 1
            ngroups = flags >> 8
            for g in range(ngroups):
                groups[ref + g * 8] = groups.get(ref + g * 8, 0) + 1
            stack.extend(k for k in kids if k)
        if not ok:
            continue
        if seen_nodes != node_count:
            problems.append((rel, f'visited {seen_nodes} nodes, header says {node_count}'))
        tris = 0
        max_index = -1
        for g in sorted(groups):
            attr, count, idx = struct.unpack_from('<HHI', d, g)
            attrs[attr] += count
            tris += count
            vals = struct.unpack_from(f'<{count * 3}H', d, idx)
            max_index = max(max_index, max(vals) if vals else -1)
        if max_index >= nverts:
            problems.append((rel, f'index {max_index} >= {nverts} vertices'))
        tri_total += tris
        stats['nodes'] += seen_nodes
        stats['groups'] += len(groups)
        stats['shared groups'] += sum(1 for v in groups.values() if v > 1)
        if stats['files'] <= 3:
            print(f'{rel}: {seen_nodes} nodes, {len(groups)} groups, {tris} triangles, {nverts} vertices, max index {max_index}')

print(dict(stats), 'triangles', tri_total)
print('node flag values:', sorted(flag_vals.items()))
print('triangles per attribute:', sorted(attrs.items()))
print('header shapes (root_at, x0c, h12, bytes 0x14..root):')
for k, v in hdr_shapes.most_common(8):
    print('  ', v, k)
print('problems:', len(problems))
for x in problems[:12]:
    print('  ', x)
