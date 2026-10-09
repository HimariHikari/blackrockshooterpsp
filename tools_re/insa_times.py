"""Print the time fields of INSA animation headers. Exploration only.

Usage: insa_times.py <file relative to game/unpacked> [...]
Header: +4 version byte, +0xC start, +0x10 end, and for version >= 2 a loop range at
+0x20/+0x24. The battle code converts with 1024 units per frame of 1/60 s.
"""
import os, struct, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'unpacked')
for rel in sys.argv[1:]:
    with open(os.path.join(ROOT, rel), 'rb') as f:
        d = f.read(0x40)
    version = d[4]
    start, end = struct.unpack_from('<ii', d, 0xC)
    loop = struct.unpack_from('<ii', d, 0x20) if version >= 2 else (start, end)
    frames = (end - start) / 1024
    print(f'{rel}: magic {d[:4]!r} version {version} start {start:#x} end {end:#x} '
          f'loop {loop[0]:#x}..{loop[1]:#x}  = {frames:g} frames of 1024 '
          f'({frames / 60:.2f} s at 60/s, {(end - start) / 4096 / 30:.2f} s at 4096 per 1/30 s); '
          f'loop frames {(loop[0] - start) / 1024:g}..{(loop[1] - start) / 1024:g}')
