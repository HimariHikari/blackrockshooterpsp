"""Writes the textures of an unpacked file as PNGs, for looking at by hand: every
`PTMD` found in it, as the port's decoder reads them (swizzled unless too small).

Usage: ptm_png.py <file relative to game/unpacked> <out folder or out.png> [crop w,h]
  e.g. ptm_png.py IF/PICT/TUT_PIT002.PTM.ZZZ tools_re/out/tut_pit002.png 480,272
"""
import os, re, struct, sys, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def png(path, w, h, rgba):
    def chunk(t, d):
        c = struct.pack('>I', len(d)) + t + d
        return c + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    raw = b''.join(b'\0' + rgba[y * w * 4:(y + 1) * w * 4] for y in range(h))
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def unswizzle(data, wbytes, h):
    out = bytearray(len(data))
    src = 0
    for by in range(0, h, 8):
        for bx in range(wbytes // 16):
            for y in range(8):
                if by + y < h:
                    o = (by + y) * wbytes + bx * 16
                    out[o:o + 16] = data[src:src + 16]
                src += 16
    return bytes(out)


def decode(b, o):
    hlo, whi, fmt = b[o + 6], b[o + 7], b[o + 8]
    wh = hlo | (whi << 8)
    w, h = 1 << (wh >> 12), wh & 0xfff
    size = struct.unpack_from('<I', b, o + 12)[0] - 16
    px = b[o + 16:o + 16 + size]
    bpp = {0: 16, 1: 16, 2: 16, 3: 32, 4: 4, 5: 8}[fmt]
    wbytes = w * bpp // 8
    if wbytes >= 16 and h >= 8:
        px = unswizzle(px, wbytes, h)
    out = bytearray(w * h * 4)
    if fmt in (4, 5):
        n = 16 if fmt == 4 else 256
        pal = b[o + 16 + size:o + 16 + size + n * 4]
        for i in range(w * h):
            idx = px[i] if fmt == 5 else (px[i >> 1] >> (4 * (i & 1))) & 15
            out[i * 4:i * 4 + 4] = pal[idx * 4:idx * 4 + 4]
    elif fmt == 3:
        out[:] = px[:w * h * 4]
    else:
        for i in range(w * h):
            v = px[i * 2] | (px[i * 2 + 1] << 8)
            if fmt == 0:
                r, g, bl, a = (v & 31) << 3, ((v >> 5) & 63) << 2, (v >> 11) << 3, 255
            elif fmt == 1:
                r, g, bl, a = (v & 31) << 3, ((v >> 5) & 31) << 3, ((v >> 10) & 31) << 3, 255 if v >> 15 else 0
            else:
                r, g, bl, a = (v & 15) * 17, ((v >> 4) & 15) * 17, ((v >> 8) & 15) * 17, (v >> 12) * 17
            out[i * 4:i * 4 + 4] = bytes((r, g, bl, a))
    return w, h, bytes(out), fmt


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return
    rel, out = sys.argv[1], sys.argv[2]
    crop = tuple(int(n) for n in sys.argv[3].split(',')) if len(sys.argv) > 3 else None
    with open(os.path.join(ROOT, rel), 'rb') as f:
        b = f.read()
    offsets = [m.start() for m in re.finditer(b'PTMD', b)]
    for index, at in enumerate(offsets):
        w, h, rgba, fmt = decode(b, at)
        if crop:
            cw, ch = min(crop[0], w), min(crop[1], h)
            rgba = b''.join(rgba[y * w * 4:y * w * 4 + cw * 4] for y in range(ch))
            w, h = cw, ch
        if out.lower().endswith('.png'):
            path = out if len(offsets) == 1 else f'{out[:-4]}_{index}.png'
        else:
            os.makedirs(out, exist_ok=True)
            path = os.path.join(out, f'{os.path.basename(rel).split(".")[0]}_{index}.png')
        png(path, w, h, rgba)
        print(f'{path}: {w} x {h}, format {fmt}')


if __name__ == '__main__':
    main()
