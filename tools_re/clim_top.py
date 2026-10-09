"""Draws a field map's collision from above: the ground shaded by its height (dark is
low), what is too steep to stand on in red, the walls that fence the map in (slot 6)
in blue, a grid line every 1000 units with its coordinate, and any points asked for.
For working out where a scripted run can walk.

Usage: clim_top.py <map name, e.g. d_stg01_01> <out.png> [<x>,<z>[,<label>] ...]
"""
import math, os, struct, sys

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked', 'FLD', 'MAP')
SIZE = 1800
# The steepest ground counts as ground: the cosine of its slope.
FLOOR = 0.5


def triangles(path):
    with open(path, 'rb') as f:
        d = f.read()
    if d[:4] != b'CLIM' or len(d) < 0x20:
        return []
    root_at, verts_at, _ = struct.unpack_from('<III', d, 4)
    groups, stack = set(), [root_at]
    while stack:
        at = stack.pop()
        flags = struct.unpack_from('<I', d, at)[0]
        ref = struct.unpack_from('<I', d, at + 36)[0]
        groups.update(ref + g * 8 for g in range(flags >> 8))
        stack.extend(k for k in struct.unpack_from('<8I', d, at + 4) if k)
    out = []
    for g in sorted(groups):
        _, count, idx = struct.unpack_from('<HHI', d, g)
        for t in range(count):
            corners = [struct.unpack_from('<3f', d, verts_at + i * 12) for i in struct.unpack_from('<3H', d, idx + t * 6)]
            out.append(corners)
    return out


def normal_y(a, b, c):
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    length = math.sqrt(sum(x * x for x in n))
    return abs(n[1]) / length if length else 0.0


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return
    name, out = sys.argv[1], sys.argv[2]
    bundle = os.path.join(ROOT, f'MAP_{name.upper()}.BIN', '11.bin')
    world = triangles(os.path.join(bundle, '03.clim'))
    fence_path = os.path.join(bundle, '06.clim')
    fence = triangles(fence_path) if os.path.exists(fence_path) else []
    points = [p for t in world + fence for p in t]
    xs, ys, zs = ([p[i] for p in points] for i in range(3))
    span = max(max(xs) - min(xs), max(zs) - min(zs)) * 1.04
    cx, cz = (max(xs) + min(xs)) / 2, (max(zs) + min(zs)) / 2
    low, high = min(ys), max(ys)

    def at(x, z):
        return ((x - cx) / span + 0.5) * SIZE, ((z - cz) / span + 0.5) * SIZE

    image = Image.new('RGB', (SIZE, SIZE), (20, 20, 28))
    draw = ImageDraw.Draw(image)
    floors = [t for t in world if normal_y(*t) >= FLOOR]
    steep = [t for t in world if normal_y(*t) < FLOOR]
    # Low ground first, so that what is above it is drawn over it.
    for t in sorted(floors, key=lambda t: sum(p[1] for p in t)):
        height = sum(p[1] for p in t) / 3
        shade = int(70 + 170 * (height - low) / max(high - low, 1))
        draw.polygon([at(p[0], p[2]) for p in t], fill=(shade, shade, shade))
    for t in steep:
        draw.polygon([at(p[0], p[2]) for p in t], outline=(220, 60, 60))
    for t in fence:
        draw.polygon([at(p[0], p[2]) for p in t], outline=(80, 140, 255))
    step = 1000
    for x in range(int(min(xs) // step) * step, int(max(xs)) + step, step):
        px = at(x, 0)[0]
        draw.line([(px, 0), (px, SIZE)], fill=(60, 90, 60))
        draw.text((px + 3, 3), f'x {x}', fill=(140, 220, 140))
    for z in range(int(min(zs) // step) * step, int(max(zs)) + step, step):
        pz = at(0, z)[1]
        draw.line([(0, pz), (SIZE, pz)], fill=(60, 90, 60))
        draw.text((3, pz + 3), f'z {z}', fill=(140, 220, 140))
    for mark in sys.argv[3:]:
        x, z, *label = mark.split(',')
        px, pz = at(float(x), float(z))
        draw.ellipse([px - 6, pz - 6, px + 6, pz + 6], outline=(255, 230, 0), width=2)
        if label:
            draw.text((px + 9, pz - 6), label[0], fill=(255, 230, 0))
    image.save(out)
    print(f'{len(floors)} ground triangles, {len(steep)} steep, {len(fence)} of the fence; '
          f'x {min(xs):.0f}..{max(xs):.0f}, y {low:.0f}..{high:.0f}, z {min(zs):.0f}..{max(zs):.0f}')


if __name__ == '__main__':
    main()
