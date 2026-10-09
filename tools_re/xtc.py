"""Surveys the `XTC` record tables (SYSTEM.VOL/db_resident.lpk and others).

Usage: xtc.py                 header of every XTC file under game/unpacked
       xtc.py <file> [fmt]    records of one file (relative to game/unpacked); `fmt` is a
                              struct format for one record, default: 16-bit words
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
HEADER = 0x10


def header(d):
    size, count, record, extra = struct.unpack_from('<IIHH', d, 4)
    return size, count, record, extra


def survey():
    for folder, _, names in os.walk(ROOT):
        for name in sorted(names):
            path = os.path.join(folder, name)
            with open(path, 'rb') as f:
                d = f.read()
            if d[:4] != b'XTC\0':
                continue
            size, count, record, extra = header(d)
            end = HEADER + record * count
            rest = d[end:end + 16].hex(' ')
            print(f'{os.path.relpath(path, ROOT):44s} len {len(d):6d} size {size:6d} count {count:4d} record {record:3d}'
                  f' extra {extra:#x} records end {end:6d} rest {len(d) - end:6d}  {rest}')


def dump(rel, fmt):
    with open(os.path.join(ROOT, rel), 'rb') as f:
        d = f.read()
    size, count, record, extra = header(d)
    print(f'{rel}: {len(d)} bytes, {count} records of {record}, extra {extra:#x}')
    fmt = fmt or '<' + 'h' * (record // 2) + 'b' * (record % 2)
    for i in range(count):
        at = HEADER + i * record
        if at + record > len(d):
            print(f'  [{i:3d}] runs past the end of the file')
            break
        print(f'  [{i:3d}]', struct.unpack_from(fmt, d, at))
    rest = d[HEADER + record * count:]
    if rest:
        print(f'  after the records: {len(rest)} bytes: {rest[:64].hex(" ")}')


if len(sys.argv) > 1:
    dump(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None)
else:
    survey()
