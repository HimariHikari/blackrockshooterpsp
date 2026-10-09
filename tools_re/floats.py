"""Prints a table of floats from the executable's data. Exploration only.

Usage: floats.py <vaddr hex> <rows> [floats per row, 3 if left out]
"""
import struct, sys

import mips

at, rows = int(sys.argv[1], 16), int(sys.argv[2])
per_row = int(sys.argv[3]) if len(sys.argv) > 3 else 3
for row in range(rows):
    values = [struct.unpack('<f', struct.pack('<I', mips.word(at + (row * per_row + i) * 4)))[0] for i in range(per_row)]
    print(f'{at + row * per_row * 4:#x}  [{row:2d}]  ' + '  '.join(f'{v:g}' for v in values))
