"""What a sound bank's header says: its programs, tones and samples.

Usage: pphd.py <file relative to game/unpacked> [--tones] [--raw]
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def chunk(d, at, name):
    assert d[at:at + 4] == name, (d[at:at + 4], name)
    size, record, _, zero, last = struct.unpack_from('<IIIII', d, at + 4)
    return size, record, last


def main():
    arguments = [a for a in sys.argv[1:] if not a.startswith('--')]
    path = arguments[0] if os.path.exists(arguments[0]) else os.path.join(ROOT, arguments[0])
    d = open(path, 'rb').read()
    assert d[:5] == b'PPHD8', d[:8]
    pg, tn, va = struct.unpack_from('<III', d, 0x10)
    print(f'{os.path.basename(path)}: {len(d)} bytes; header words {struct.unpack_from("<II", d, 8)}; chunks at {pg:#x} {tn:#x} {va:#x}')

    size, = struct.unpack_from('<I', d, pg + 4)
    print(f'PPPG: size {size:#x}; words {[hex(w) for w in struct.unpack_from("<6I", d, pg + 8)]}')
    programs = []
    for n in range(128):
        off, = struct.unpack_from('<I', d, pg + 0x20 + n * 4)
        if off != 0xffffffff:
            programs.append((n, struct.unpack_from('<5I', d, pg + off)))
    for n, words in programs:
        print(f'  program {n:3d}: ' + ' '.join('-' if w == 0xffffffff else f'{w:x}' for w in words))

    size, record, last = chunk(d, tn, b'PPTN')
    print(f'PPTN: size {size:#x}, records of {record:#x}, last {last}')
    if '--tones' in sys.argv:
        for n in range(last + 1):
            words = struct.unpack_from(f'<{record // 4}I', d, tn + 0x20 + n * record)
            print(f'  tone {n:3d}: ' + ' '.join('-' if w == 0xffffffff else f'{w:x}' for w in words))

    size, record, last = chunk(d, va, b'PPVA')
    print(f'PPVA: size {size:#x}, records of {record:#x}, last {last}')
    for n in range(last + 1):
        offset, rate, length, _ = struct.unpack_from('<4I', d, va + 0x20 + n * record)
        print(f'  sample {n:3d}: at {offset:#8x}, {length:#7x} bytes, {rate} Hz, {length / 16 * 28 / rate:.2f} s')


if __name__ == '__main__':
    main()
