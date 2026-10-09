"""Walks a file of chunks as the game does (0xD2F18): a tag of three letters and a
zero, a byte of flags (0x80 always, 0x08 for contents aligned to 16 instead of 4),
the contents' length in three bytes (most significant first), then the contents;
`EOFC` ends it. Exploration only.

Usage: chunks.py <file relative to game/unpacked> [--sbs] [--save <folder>]
  --sbs    reads `SBS` chunks as sprite tables and `DAT` chunks as the font's icon list
  --save   writes each chunk's contents to <folder>/<n>_<tag>.bin
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

# What a sprite table's fields are stored as, by the three bits each has in the
# table's first word (0x2E010, 0x2E09C).
FIELD = {0: 'B', 1: 'b', 2: 'H', 3: 'h', 4: 'I', 5: 'i'}


def chunks(data, at=0):
    while at + 8 <= len(data):
        tag = data[at:at + 4]
        flags = data[at + 4]
        size = int.from_bytes(data[at + 5:at + 8], 'big')
        if not flags & 0x80:
            break
        align = 0xf if flags & 8 else 3
        head = (align + 8) & ~align
        yield tag.rstrip(b'\0').decode('latin1'), flags, at + head, size
        if tag == b'EOFC':
            break
        at += head + ((size + align) & ~align)


def sprites(data, at):
    word, count = struct.unpack_from('<HB', data, at)
    kinds = [(word >> shift) & 7 for shift in (0, 3, 6, 9)]
    layout = '<' + ''.join(FIELD.get(kind, 'B') for kind in kinds)
    size = struct.calcsize(layout)
    return [struct.unpack_from(layout, data, at + 3 + size * n) for n in range(count)], kinds


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    path = args[0] if os.path.isabs(args[0]) else os.path.join(ROOT, args[0])
    data = open(path, 'rb').read()
    save = args[1] if '--save' in sys.argv else None
    for n, (tag, flags, at, size) in enumerate(chunks(data)):
        print(f'{n:3d} {tag:4} flags {flags:#04x} contents {at:#x}..{at + size:#x} ({size} bytes)  {data[at:at + 16].hex(" ")}')
        if save:
            os.makedirs(save, exist_ok=True)
            open(os.path.join(save, f'{n:03d}_{tag}.bin'), 'wb').write(data[at:at + size])
        if '--sbs' in sys.argv and tag == 'SBS':
            rects, kinds = sprites(data, at)
            print(f'    {len(rects)} sprites, fields stored as {kinds}')
            for i, rect in enumerate(rects):
                print(f'    {i:3d} {rect}')
        if '--sbs' in sys.argv and tag == 'DAT':
            count = struct.unpack_from('<I', data, at)[0]
            print(f'    {count} icons: number, sprite, x, y, scaled, size')
            for i in range(count):
                print('    ', struct.unpack_from('<6h', data, at + 4 + 12 * i))


main()
