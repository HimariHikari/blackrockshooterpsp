"""Prints a data symbol of an `SSCR` file as bytes, with what a few common reads of it
give. Exploration only.

Usage: sscr_data.py <file.sscr> <symbol> [how many bytes]
"""
import struct
import sys


def symbols(data):
    """(name, offset) of every symbol: pairs of (value, name offset) up to the first name."""
    at, names_at = struct.unpack_from('<II', data, 0x0c)
    found = []
    while at < names_at:
        value, name_at = struct.unpack_from('<II', data, at)
        names_at = min(names_at, name_at)
        end = data.index(b'\0', name_at)
        found.append((data[name_at:end].decode('ascii', 'replace'), value))
        at += 8
    return found


def main():
    path, wanted = sys.argv[1:3]
    data = open(path, 'rb').read()
    found = symbols(data)
    if wanted not in dict(found):
        print('symbols:', ', '.join(name for name, _ in found))
        return
    start = dict(found)[wanted]
    later = [offset for _, offset in found if offset > start]
    end = min(later) if later else len(data)
    count = int(sys.argv[3], 0) if len(sys.argv) > 3 else end - start
    table = data[start:start + count]
    print(f'{wanted}: {start:#x}..{end:#x} ({end - start} bytes)')
    for at in range(0, len(table), 16):
        row = table[at:at + 16]
        print(f'{at:04x}  ' + ' '.join(f'{byte:02x}' for byte in row))
    print('as u16:', [struct.unpack_from('<H', table, at)[0] for at in range(0, len(table) - 1, 2)])
    print('as f32:', [round(struct.unpack_from('<f', table, at)[0], 4) for at in range(0, len(table) - 3, 4)])


main()
