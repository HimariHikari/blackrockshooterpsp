"""Decode a few PTMD textures to PNG to check swizzle/palette hypotheses. Exploration only."""
import gzip, os, re, struct, sys, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'extracted', 'PSP_GAME', 'USRDIR', 'GAMEDATA')
OUT = os.path.join(HERE, 'out')
os.makedirs(OUT, exist_ok=True)


def png(path, w, h, rgba):
    def chunk(t, d):
        c = struct.pack('>I', len(d)) + t + d
        return c + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    raw = b''.join(b'\0' + rgba[y * w * 4:(y + 1) * w * 4] for y in range(h))
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def unswizzle(data, wbytes, h):
    out = bytearray(len(data))
    rowblocks = wbytes // 16
    src = 0
    for by in range(0, h, 8):
        for bx in range(rowblocks):
            for y in range(8):
                if by + y < h:
                    o = (by + y) * wbytes + bx * 16
                    out[o:o + 16] = data[src:src + 16]
                src += 16
    return bytes(out)


def decode(b, o, swz):
    flags, hlo, whi, fmt, f2 = b[o + 5], b[o + 6], b[o + 7], b[o + 8], b[o + 9]
    wh = hlo | (whi << 8)
    w, h = 1 << (wh >> 12), wh & 0xfff
    size = struct.unpack_from('<I', b, o + 12)[0] - 16
    px = b[o + 16:o + 16 + size]
    bpp = {0: 16, 1: 16, 2: 16, 3: 32, 4: 4, 5: 8}[fmt]
    wbytes = w * bpp // 8
    if swz and wbytes >= 16 and h >= 8:
        px = unswizzle(px, wbytes, h)
    out = bytearray(w * h * 4)
    if fmt in (4, 5):
        n = 16 if fmt == 4 else 256
        pal = b[o + 16 + size:o + 16 + size + n * 4]
        for i in range(w * h):
            if fmt == 5:
                idx = px[i]
            else:
                idx = (px[i >> 1] >> (4 * (i & 1))) & 15
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
    return w, h, bytes(out), (flags, fmt, f2)


def load(rel):
    d = open(os.path.join(ROOT, rel), 'rb').read()
    if d[:2] == b'\x1f\x8b':
        d = gzip.decompress(d)
    if d[:4] == b'CZAA':
        d = zlib.decompress(d[16:])
    return d


for rel, picks in [('IF/MENU/IF_STAGE_SELECT.GZ', [0, 1, 2, 3, 4, 5, 6, 7]), ('GALLERY/GLR_ART_001.BIN', [0]), ('GALLERY/GLR_ART_007.BIN', [0]), ('IF/PICT/BTL_PIT260.PTM.ZZZ', [0])]:
    b = load(rel)
    offs = [m.start() for m in re.finditer(b'PTMD', b)]
    print(rel, len(offs), 'textures')
    for i in picks:
        if i >= len(offs):
            continue
        for swz in (0, 1):
            w, h, rgba, meta = decode(b, offs[i], swz)
            name = f'{os.path.basename(rel).split(".")[0]}_{i}_{"swz" if swz else "lin"}.png'
            png(os.path.join(OUT, name), w, h, rgba)
        print(' ', i, w, h, meta)
