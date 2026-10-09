"""Hex dump of a part of a file, sixteen bytes a row, with the words as u32 and f32.
Exploration only.

Usage: hexdump.py <file> [from] [length]
"""
import struct
import sys

path = sys.argv[1]
start = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0
data = open(path, 'rb').read()
length = int(sys.argv[3], 0) if len(sys.argv) > 3 else len(data) - start
print(f'{path}: {len(data)} bytes')
for at in range(start, min(start + length, len(data)), 16):
    row = data[at:at + 16]
    text = ''.join(chr(byte) if 32 <= byte < 127 else '.' for byte in row)
    words = ''
    if len(row) == 16:
        floats = struct.unpack('<4f', row)
        words = '  ' + ' '.join(f'{value:10.4g}' if 1e-4 < abs(value) < 1e7 else '         .' for value in floats)
    print(f'{at:06x}  ' + ' '.join(f'{byte:02x}' for byte in row).ljust(47) + f'  {text}{words}')
