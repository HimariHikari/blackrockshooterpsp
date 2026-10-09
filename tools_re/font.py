"""Reads a font of the game (`SYSTEM.VOL/font.lpk/000.bin`, `001.bin`) as the font
library does (0x1D124 on). Exploration only.

Usage: font.py <file relative to game/unpacked> [sheet.png [first [count [lo|hi]]]] [--codes] [--metrics]

Header (0x20 bytes): u32 offset of the code table (header, palette and pixels),
u32 palette bytes, u32 pixel bytes, u32 bytes of the code table, u32 bytes of the
metric table, u32 flags (bit 31: has a palette; low 2 bits: how glyphs are found),
u16 glyphs, u8 width, u8 height, u8 bits a pixel.
"""
import os, struct, sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


class Font:
    def __init__(self, data):
        self.data = data
        (self.codes_at, self.palette_bytes, self.pixel_bytes, self.code_bytes, self.metric_bytes, self.flags,
         self.glyphs, self.width, self.height, self.bits) = struct.unpack_from('<6IHBBB', data, 0)
        self.palette = [tuple(data[0x20 + 4 * i:0x24 + 4 * i]) for i in range(1 << self.bits)] if self.flags >> 31 else None
        self.pixels_at = 0x20 + ((1 << self.bits) * 4 if self.flags >> 31 else 0)
        self.glyph_bytes = (self.width * self.height * self.bits + 7) // 8
        a = self.pixels_at + self.pixel_bytes
        self.codes = list(struct.unpack_from(f'<{self.code_bytes // 2}H', data, a))
        b = a + self.code_bytes
        self.metrics = [struct.unpack_from('<4h', data, b + 8 * i) for i in range(self.metric_bytes // 8)]
        c = b + self.metric_bytes
        self.metric_codes = list(struct.unpack_from(f'<{self.metric_bytes // 8}H', data, c))
        self.end = c + 2 * (self.metric_bytes // 8)

    def glyph(self, index, high_first):
        at = self.pixels_at + index * self.glyph_bytes
        out = []
        for i in range(self.width * self.height):
            byte = self.data[at + i // 2]
            out.append((byte >> 4) if (i % 2 == 0) == high_first else (byte & 15))
        return out


def char(code):
    raw = bytes([code]) if code < 0x100 else bytes([code >> 8, code & 0xff])
    try:
        return raw.decode('cp932')
    except UnicodeDecodeError:
        return '?'


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    path = args[0] if os.path.isabs(args[0]) else os.path.join(ROOT, args[0])
    font = Font(open(path, 'rb').read())
    print(f'{font.glyphs} glyphs of {font.width}x{font.height} at {font.bits} bits, {font.glyph_bytes} bytes each; '
          f'flags {font.flags:#x}; {len(font.codes)} codes, {len(font.metrics)} metrics; ends at {font.end:#x} of {len(font.data):#x}')
    print('palette alpha:', [p[3] for p in font.palette] if font.palette else None)
    if '--codes' in sys.argv:
        for i in range(0, len(font.codes), 16):
            row = font.codes[i:i + 16]
            print(f'{i:5d}  ' + ' '.join(f'{c:04X}' for c in row) + '  ' + ''.join(char(c) for c in row))
    if '--metrics' in sys.argv:
        for code, metric in zip(font.metric_codes, font.metrics):
            print(f'{code:04X} {char(code)!r:6} {metric}')
    if len(args) > 1:
        first = int(args[2]) if len(args) > 2 else 0
        count = int(args[3]) if len(args) > 3 else 256
        high = (args[4] if len(args) > 4 else 'lo') == 'hi'
        count = min(count, font.glyphs - first)
        across = 32
        cell = (font.width + 1, font.height + 1)
        scale = 3
        image = Image.new('RGBA', (across * cell[0], ((count + across - 1) // across) * cell[1]), (40, 40, 90, 255))
        for n in range(count):
            pixels = font.glyph(first + n, high)
            ox, oy = (n % across) * cell[0], (n // across) * cell[1]
            for i, p in enumerate(pixels):
                alpha = font.palette[p][3] if font.palette else p * 17
                image.putpixel((ox + i % font.width, oy + i // font.width), (alpha, alpha, alpha, 255))
        image = image.resize((image.width * scale, image.height * scale), Image.NEAREST)
        image.save(args[1])
        print('wrote', args[1])


main()
