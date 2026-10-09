"""Find 32-bit little-endian values or ASCII strings in the executable. Exploration only.

Usage: findbytes.py u32 <hex> [<hex> ...]     every file offset holding that value
       findbytes.py str <text>                every occurrence of the text
       findbytes.py dump <hexoff> [len]       hex dump of the executable at a file offset
"""
import os, struct, sys

EXE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'extracted',
                   'PSP_GAME', 'SYSDIR', 'EBOOT.OLD')
d = open(EXE, 'rb').read()
VADDR = 0xC0  # file offset = vaddr + 0xC0


def findall(needle):
    pos = d.find(needle)
    while pos >= 0:
        yield pos
        pos = d.find(needle, pos + 1)


mode = sys.argv[1]
if mode == 'u32':
    for text in sys.argv[2:]:
        hits = list(findall(struct.pack('<I', int(text, 16))))
        print(f'{text}: {len(hits)} hits ' + ' '.join(f'{h:#x}(v{h - VADDR:#x})' for h in hits[:40]))
elif mode == 'str':
    for hit in findall(sys.argv[2].encode()):
        start = d.rfind(b'\0', 0, hit) + 1
        end = d.find(b'\0', hit)
        print(f'{hit:#x} (v{hit - VADDR:#x}): {d[start:end][:120]!r}')
elif mode == 'dump':
    off = int(sys.argv[2], 16)
    n = int(sys.argv[3], 16) if len(sys.argv) > 3 else 0x100
    for o in range(off, off + n, 16):
        row = d[o:o + 16]
        print(f'{o:06x} (v{o - VADDR:06x})  ' + ' '.join(f'{b:02x}' for b in row) + '  '
              + ''.join(chr(b) if 32 <= b < 127 else '.' for b in row))
