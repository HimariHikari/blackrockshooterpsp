"""Reads one member out of an RTDP archive (a .VOL file), as stored. Exploration only.

Usage: rtdp.py <archive under game/extracted/PSP_GAME/USRDIR/GAMEDATA> [member [out file]]
  with no member: lists the archive
  with a member: prints the start of it, or writes it to the out file
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, '..', 'game', 'extracted', 'PSP_GAME', 'USRDIR', 'GAMEDATA')
KEY = 0x55


def members(d):
    data_start, count = struct.unpack_from('<II', d, 4)
    for i in range(count):
        e = d[0x20 + i * 40:0x20 + (i + 1) * 40]
        name = e[:32].split(b'\0')[0].decode('latin1')
        size, offset = struct.unpack_from('<II', e, 32)
        yield name, data_start + offset, size


def read(archive, member):
    d = open(os.path.join(DATA, archive), 'rb').read()
    for name, start, size in members(d):
        if name == member:
            return bytes(b ^ KEY for b in d[start:start + size])
    raise KeyError(member)


if __name__ == '__main__':
    archive = sys.argv[1]
    if len(sys.argv) == 2:
        d = open(os.path.join(DATA, archive), 'rb').read()
        for name, start, size in members(d):
            print(f'{name:32s} {size:9d} at {start:#x}')
    else:
        blob = read(archive, sys.argv[2])
        if len(sys.argv) > 3:
            open(sys.argv[3], 'wb').write(blob)
            print(f'wrote {sys.argv[3]} ({len(blob)} bytes)')
        else:
            for at in range(0, min(len(blob), 0x100), 16):
                row = blob[at:at + 16]
                print(f'{at:06x}  {row.hex(" ")}  {"".join(chr(b) if 32 <= b < 127 else "." for b in row)}')
