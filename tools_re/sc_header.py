"""Shows an SC bundle's offset table next to where its slots really begin. Exploration only.

Usage: sc_header.py <file under GAMEDATA, or VOL:member> [slot path, e.g. 11/1]

Prints the table's words, then for each offset the 16 bytes at it and at 8 and 12 bytes
before it, to see what the offsets count from.
"""
import gzip, os, struct, sys, zlib

import rtdp


def load(spec):
    if ':' in spec and not os.path.isabs(spec):
        archive, member = spec.split(':', 1)
        d = rtdp.read(archive, member)
    else:
        d = open(os.path.join(rtdp.DATA, spec), 'rb').read()
    if d[:3] == b'\x1f\x8b\x08':
        d = gzip.decompress(d)
    elif d[:4] == b'CZAA':
        d = zlib.decompress(d[16:])
    return d


def show(d, label):
    count = struct.unpack_from('<H', d, 2)[0]
    words = struct.unpack_from(f'<{count + 2}I', d, 4)
    print(f'{label}: {len(d):#x} bytes, magic {d[:2]!r}, count {count}')
    print('  table: ' + ' '.join(f'{w:#x}' for w in words))
    for i, w in enumerate(words[:count + 1]):
        for back in (0, 8, 12):
            at = w - back
            if 0 <= at < len(d):
                row = d[at:at + 16]
                text = ''.join(chr(b) if 32 <= b < 127 else '.' for b in row)
                print(f'    [{i:2d}] {w:#8x} - {back:2d}: {row.hex(" ")}  {text}')


def main():
    d = load(sys.argv[1])
    label = sys.argv[1]
    if len(sys.argv) > 2:
        for part in sys.argv[2].split('/'):
            count = struct.unpack_from('<H', d, 2)[0]
            offsets = struct.unpack_from(f'<{count + 1}I', d, 4)
            i = int(part)
            d = d[offsets[i]:offsets[i + 1] if offsets[i + 1] else len(d)]
            label += '/' + part
    show(d, label)


if __name__ == '__main__':
    main()
