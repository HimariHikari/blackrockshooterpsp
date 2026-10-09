"""Lists a `BXCB` table: its records' 32-bit fields, with the text a field refers to
when it is an offset into the table's pool of strings.

Usage: bxcb.py <file relative to game/unpacked> [string field numbers, e.g. 1 or 0,2]

Exploration only; the port reads these with `brs_formats::bxcb`.
"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / 'game' / 'unpacked'


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    data = (ROOT / sys.argv[1]).read_bytes()
    assert data[:4] == b'BXCB', data[:4]
    strings = {int(n) for n in sys.argv[2].split(',')} if len(sys.argv) > 2 else set()
    records_at, record_len, count, pool_at = struct.unpack_from('<4I', data, 0x0c)
    fields = record_len // 4
    print(f'{count} records of {fields} fields; the pool is {len(data) - pool_at} bytes')
    for index in range(count):
        values = struct.unpack_from(f'<{fields}I', data, records_at + index * record_len)
        shown = []
        for number, value in enumerate(values):
            if number in strings:
                end = data.index(b'\0', pool_at + value)
                shown.append(repr(data[pool_at + value:end].decode('utf-8', 'replace')))
            else:
                shown.append(str(value if value < 0x80000000 else value - (1 << 32)))
        print(f'{index:4}: ' + ', '.join(shown))


if __name__ == '__main__':
    main()
