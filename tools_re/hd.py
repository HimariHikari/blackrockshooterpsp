"""Hex dump helper. Usage: hd.py <file relative to game/unpacked or absolute> [offset] [length] [mode]

mode: x (hex+ascii, default), u (u32 columns), f (float columns), h (u16 columns)
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def load(path):
    p = path if os.path.isabs(path) else os.path.join(ROOT, path)
    return open(p, 'rb').read()


def main():
    d = load(sys.argv[1])
    off = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0
    n = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0x100
    mode = sys.argv[4] if len(sys.argv) > 4 else 'x'
    end = min(len(d), off + n)
    print(f'# {sys.argv[1]} size={len(d):#x} dump {off:#x}..{end:#x}')
    for i in range(off, end, 16):
        row = d[i:min(i + 16, end)]
        if mode == 'x':
            print(f'{i:06X}  {" ".join(f"{b:02X}" for b in row):<47}  {"".join(chr(b) if 32 <= b < 127 else "." for b in row)}')
        elif mode == 'u':
            vals = struct.unpack_from('<' + 'I' * (len(row) // 4), row)
            print(f'{i:06X}  ' + ' '.join(f'{v:10X}' for v in vals))
        elif mode == 'f':
            vals = struct.unpack_from('<' + 'f' * (len(row) // 4), row)
            print(f'{i:06X}  ' + ' '.join(f'{v:14.6g}' for v in vals))
        elif mode == 'h':
            vals = struct.unpack_from('<' + 'h' * (len(row) // 2), row)
            print(f'{i:06X}  ' + ' '.join(f'{v:7d}' for v in vals))


if __name__ == '__main__':
    main()
