"""Lists the items of the game: the three tables of `db_resident.lpk` that item numbers
run through (consumables, equipment, the third table), as the lookup at 0x12C9C8 has them.

Usage: items.py [<first> <last>]      item numbers; all of them if left out
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
FOLDER = os.path.join(HERE, '..', 'game', 'unpacked', 'SYSTEM.VOL', 'db_resident.lpk')
TABLES = ['000.xtc', '001.xtc', '002.xtc']


def table(name):
    data = open(os.path.join(FOLDER, name), 'rb').read()
    size, count, length = struct.unpack_from('<IIH', data, 4)
    return data, count, length


def text(data, offset):
    if not 0 < offset < len(data):
        return ''
    return data[offset:].split(b'\0')[0].decode('utf-8', 'replace')


def main():
    first, last = (int(sys.argv[1]), int(sys.argv[2])) if len(sys.argv) > 2 else (1, 10 ** 6)
    number = 1
    for kind, name in enumerate(TABLES):
        data, count, length = table(name)
        print(f'-- class {kind}: {name}, {count} records of {length} bytes, items {number} to {number + count - 1}')
        for index in range(count):
            record = data[0x10 + index * length:0x10 + (index + 1) * length]
            words = struct.unpack_from(f'<{length // 4}I', record)
            if first <= number <= last:
                rest = ' '.join(f'{word:x}' for word in words[2:])
                print(f'{number:4} {text(data, words[0])!r:40} {rest}')
            number += 1


if __name__ == '__main__':
    main()
