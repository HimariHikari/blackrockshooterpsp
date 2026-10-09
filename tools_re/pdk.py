"""Lists the members of a `PDK` pack (the files of the ride on the bike,
`MINI_GAME/BMG_*.BIN`), and writes them out for looking at.

usage: python pdk.py <file relative to game/unpacked>            list it
       python pdk.py <file relative to game/unpacked> <folder>   write its members there

Header: `PDK\\x01`, u16 members, u16 where they begin, then for each member where it
is, counted from there, and how long (nought for both: there is none). A member is
written as `NNN.<its magic>`. Exploration only: the game reads the packs as they are
(`GameData::read`), and `brs-extract` unpacks them like any other.
"""
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def members(d):
    assert d[:4] == b'PDK\x01', 'not a PDK pack'
    count, begin = struct.unpack_from('<HH', d, 4)
    for i in range(count):
        off, size = struct.unpack_from('<II', d, 8 + 8 * i)
        yield i, d[begin + off: begin + off + size]


def extension(blob):
    magic = blob[:4].rstrip(b'\0')
    if magic == b'PTMD':
        return 'ptm'
    if magic and all(48 <= c < 58 or 65 <= c < 91 or 97 <= c < 123 for c in magic):
        return magic.decode().lower()
    return 'bin'


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    with open(os.path.join(ROOT, sys.argv[1]), 'rb') as f:
        d = f.read()
    out = sys.argv[2] if len(sys.argv) > 2 else None
    if out:
        os.makedirs(out, exist_ok=True)
    kinds = {}
    for i, blob in members(d):
        if not blob:
            continue
        kinds.setdefault(extension(blob), []).append(i)
        if out:
            with open(os.path.join(out, f'{i:03}.{extension(blob)}'), 'wb') as f:
                f.write(blob)
        else:
            print(f'{i:3} {len(blob):8} {extension(blob):5} {blob[:16].hex(" ")}')
    for kind, numbers in sorted(kinds.items()):
        print(f'{kind}: {len(numbers)} ({numbers[0]}..{numbers[-1]})')


if __name__ == '__main__':
    main()
