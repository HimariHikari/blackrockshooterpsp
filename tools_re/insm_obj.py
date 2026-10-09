"""Prototype INSM model reader: exports OBJ (+MTL) and renders a quick preview PNG. Exploration only.

Usage: insm_obj.py <model file relative to game/unpacked> [output basename]
"""
import math, os, struct, sys, zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import ROOT, load

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')


def quat_to_mat(q):
    x, y, z, w = q
    return [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]


def mat_mul(a, b):
    """4x4 (row-major lists) a*b, where transforms are (R, t) with column vectors."""
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def trs(t, q):
    r = quat_to_mat(q)
    return [r[0] + [t[0]], r[1] + [t[1]], r[2] + [t[2]], [0, 0, 0, 1]]


def apply(m, v):
    return tuple(m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] + m[i][3] for i in range(3))


def parse(d):
    mats_n, tex_n, nodes_n, bind_n = d[5], d[6], d[8], d[9]
    sub_n = struct.unpack_from('<I', d, 0x0C)[0]
    a, b, c, dd, e, f, dls, dlsz, bufsz, _ = struct.unpack_from('<10I', d, 0x10)

    materials = [(d[a + i * 0x24 + 1], struct.unpack_from('<I', d, a + i * 0x24 + 4)[0]) for i in range(mats_n)]
    subs = []
    for i in range(sub_n):
        flags, nb, mat, _ = d[b + i * 8:b + i * 8 + 4]
        subs.append({'flags': flags, 'nbones': nb, 'mat': mat, 'dl': struct.unpack_from('<I', d, b + i * 8 + 4)[0]})
    nodes = []
    for i in range(nodes_n):
        o = c + i * 0x20
        nodes.append({'flags': d[o], 'id': d[o + 1], 'parent': d[o + 2], 'next': d[o + 3],
                      't': struct.unpack_from('<3f', d, o + 4), 'q': struct.unpack_from('<4f', d, o + 16)})
    for n in nodes:
        local = trs(n['t'], n['q'])
        n['world'] = local if n['parent'] == 0xFF else mat_mul(nodes[n['parent']]['world'], local)

    slots = []
    binds = []
    for i in range(bind_n):
        o = dd + i * 12
        nskin, nrigid, node = d[o], d[o + 1], struct.unpack_from('<H', d, o + 2)[0]
        lst, ibm = struct.unpack_from('<II', d, o + 4)
        binds.append({'node': node, 'nskin': nskin, 'nrigid': nrigid, 'ibm': ibm})
        for k in range(nskin + nrigid):
            slots.append((struct.unpack_from('<I', d, lst + k * 4)[0], i))
    slots.sort()
    it = iter(slots)
    for s in subs:
        s['binds'] = [next(it)[1] for _ in range(max(s['nbones'], 1))]
    leftover = len(list(it))
    return {'materials': materials, 'subs': subs, 'nodes': nodes, 'binds': binds, 'tex_off': f,
            'leftover_slots': leftover, 'bufsz': bufsz, 'slots': slots}


def read_strips(d, sub):
    """Walk a submesh display list; returns (cull, [strips]) where a strip is a list of vertex dicts."""
    o = origin = sub['dl']
    vtype = vaddr = 0
    cull = 0
    strips = []
    while True:
        w = struct.unpack_from('<I', d, o)[0]
        cmd, arg = w >> 24, w & 0xffffff
        if cmd == 0x14:
            origin = o
        elif cmd == 0x1D:
            cull = arg
        elif cmd == 0x12:
            vtype = arg
        elif cmd == 0x01:
            vaddr = origin + arg
        elif cmd == 0x04:
            count, prim = arg & 0xffff, (arg >> 16) & 7
            verts = [read_vertex(d, vaddr, vtype, i) for i in range(count)]
            strips.append((prim, verts))
        elif cmd in (0x0B, 0x0C):
            break
        o += 4
    return cull, strips


def layout(vtype):
    tc, col, nrm, pos, wt = vtype & 3, (vtype >> 2) & 7, (vtype >> 5) & 3, (vtype >> 7) & 3, (vtype >> 9) & 3
    nw = ((vtype >> 14) & 7) + 1
    sz = [0, 1, 2, 4]
    off, align, fields = 0, 1, {}

    def put(name, size, count):
        nonlocal off, align
        off = (off + size - 1) // size * size
        fields[name] = off
        off += size * count
        align = max(align, size)
    if wt:
        put('w', sz[wt], nw)
    if tc:
        put('uv', sz[tc], 2)
    if col:
        put('col', {4: 2, 5: 2, 6: 2, 7: 4}[col], 1)
    if nrm:
        put('n', sz[nrm], 3)
    put('p', sz[pos], 3)
    return fields, (off + align - 1) // align * align, nw


def read_vertex(d, base, vtype, i):
    fields, size, nw = layout(vtype)
    o = base + i * size
    v = {'p': struct.unpack_from('<3f', d, o + fields['p'])}
    if 'uv' in fields:
        if vtype & 3 == 2:
            u, t = struct.unpack_from('<2H', d, o + fields['uv'])
            v['uv'] = (u / 32768.0, t / 32768.0)
        elif vtype & 3 == 3:
            v['uv'] = struct.unpack_from('<2f', d, o + fields['uv'])
    if 'n' in fields:
        n = struct.unpack_from('<3b', d, o + fields['n'])
        v['n'] = tuple(x / 127.0 for x in n)
    if 'w' in fields:
        v['w'] = struct.unpack_from(f'<{nw}f', d, o + fields['w'])
    return v


def png(path, w, h, rgb):
    def chunk(t, dd):
        c = struct.pack('>I', len(dd)) + t + dd
        return c + struct.pack('>I', zlib.crc32(t + dd) & 0xffffffff)
    raw = b''.join(b'\0' + bytes(rgb[y * w * 3:(y + 1) * w * 3]) for y in range(h))
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def render(tris, path, size=480):
    """Orthographic flat-shaded z-buffer render, front (XY) and side (ZY) views next to each other."""
    pts = [p for t in tris for p in t]
    mins = [min(p[i] for p in pts) for i in range(3)]
    maxs = [max(p[i] for p in pts) for i in range(3)]
    span = max(maxs[i] - mins[i] for i in range(3)) or 1.0
    scale = (size - 20) / span
    W = size * 2
    img = bytearray([24, 24, 32] * (W * size))
    zb = [-1e30] * (W * size)
    cen = [(mins[i] + maxs[i]) / 2 for i in range(3)]
    for view, (ax, az, sgn) in enumerate([(0, 2, 1), (2, 0, -1)]):
        ox = size // 2 + view * size
        for t in tris:
            ps = [((p[ax] - cen[ax]) * scale * sgn + ox, size / 2 - (p[1] - cen[1]) * scale, p[az] * sgn) for p in t]
            e1 = [t[1][i] - t[0][i] for i in range(3)]
            e2 = [t[2][i] - t[0][i] for i in range(3)]
            n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
            ln = math.sqrt(sum(x * x for x in n)) or 1.0
            shade = abs(n[az]) / ln
            col = (int(60 + 180 * shade), int(70 + 170 * shade), int(90 + 150 * shade))
            x0, x1 = max(view * size, int(min(p[0] for p in ps))), min(view * size + size - 1, int(max(p[0] for p in ps)) + 1)
            y0, y1 = max(0, int(min(p[1] for p in ps))), min(size - 1, int(max(p[1] for p in ps)) + 1)
            (ax_, ay_, az_), (bx, by, bz), (cx, cy, cz) = ps
            den = (by - cy) * (ax_ - cx) + (cx - bx) * (ay_ - cy)
            if abs(den) < 1e-9:
                continue
            for y in range(y0, y1 + 1):
                for x in range(x0, x1 + 1):
                    l0 = ((by - cy) * (x - cx) + (cx - bx) * (y - cy)) / den
                    l1 = ((cy - ay_) * (x - cx) + (ax_ - cx) * (y - cy)) / den
                    l2 = 1 - l0 - l1
                    if l0 < -0.01 or l1 < -0.01 or l2 < -0.01:
                        continue
                    z = l0 * az_ + l1 * bz + l2 * cz
                    i = y * W + x
                    if z > zb[i]:
                        zb[i] = z
                        img[i * 3:i * 3 + 3] = bytes(col)
    png(path, W, size, img)


def main():
    rel = sys.argv[1]
    d = load(rel)
    m = parse(d)
    base = sys.argv[2] if len(sys.argv) > 2 else os.path.splitext(os.path.basename(rel))[0]
    os.makedirs(OUT, exist_ok=True)
    print(f'{rel}: {len(m["nodes"])} nodes, {len(m["binds"])} bindings, {len(m["subs"])} submeshes, '
          f'{len(m["materials"])} materials, leftover slots {m["leftover_slots"]}, bufsz {m["bufsz"]:#x}')
    for i, n in enumerate(m['nodes']):
        print(f'  node {i:2} flags={n["flags"]:02X} id={n["id"]:02X} parent={n["parent"]:3} next={n["next"]:3} '
              f't=({n["t"][0]:8.2f},{n["t"][1]:8.2f},{n["t"][2]:8.2f}) world=({n["world"][0][3]:8.2f},{n["world"][1][3]:8.2f},{n["world"][2][3]:8.2f})')
    gaps = [m['slots'][i + 1][0] - m['slots'][i][0] for i in range(len(m['slots']) - 1)]
    print('  slot gaps:', ' '.join(f'{g:X}' for g in gaps))

    tris = []
    lines = ['mtllib ' + base + '.mtl']
    vi = 1
    for si, s in enumerate(m['subs']):
        cull, strips = read_strips(d, s)
        nodes = [m['binds'][b]['node'] for b in s['binds']]
        nverts = sum(len(v) for _, v in strips)
        print(f'  sub {si:2} flags={s["flags"]:02X} bones={s["nbones"]} mat={s["mat"]} cull={cull} strips={len(strips)} verts={nverts} nodes={nodes}')
        lines.append(f'o sub{si:02}')
        lines.append(f'usemtl mat{s["mat"]}')
        xf = m['nodes'][nodes[0]]['world'] if s['nbones'] == 0 else None
        for prim, verts in strips:
            ps = [apply(xf, v['p']) if xf else v['p'] for v in verts]
            for v, p in zip(verts, ps):
                lines.append(f'v {p[0]:.5f} {p[1]:.5f} {p[2]:.5f}')
                uv = v.get('uv', (0, 0))
                lines.append(f'vt {uv[0]:.5f} {1 - uv[1]:.5f}')
            if prim == 4:
                for k in range(len(verts) - 2):
                    i0, i1, i2 = (k, k + 1, k + 2) if k % 2 == 0 else (k + 1, k, k + 2)
                    lines.append(f'f {vi + i0}/{vi + i0} {vi + i1}/{vi + i1} {vi + i2}/{vi + i2}')
                    tris.append((ps[i0], ps[i1], ps[i2]))
            vi += len(verts)
    open(os.path.join(OUT, base + '.obj'), 'w').write('\n'.join(lines) + '\n')
    with open(os.path.join(OUT, base + '.mtl'), 'w') as fh:
        for i, (tex, _) in enumerate(m['materials']):
            fh.write(f'newmtl mat{i}\nKd 1 1 1\nmap_Kd {base}_tex{tex}.png\n')
    print(f'  wrote {base}.obj with {vi - 1} verts, {len(tris)} tris')
    render(tris, os.path.join(OUT, base + '.png'))


if __name__ == '__main__':
    main()
