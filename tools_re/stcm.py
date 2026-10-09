"""STCM map structure dumper. Exploration only.

Usage: stcm.py <file relative to game/unpacked>
"""
import collections, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import GE, PRIMS, disasm, load, vsize, vtype_str

d = load(sys.argv[1])
print(f'# {sys.argv[1]} size={len(d):#x}')
print('bytes 4..8:', d[4:8].hex(' '))
hdr = struct.unpack_from('<10I', d, 8)
print('header u32s from 0x08:', ' '.join(f'{x:#x}' for x in hdr))
ntex = d[5]
tex = struct.unpack_from(f'<{ntex}I', d, 0x30)
print(f'{ntex} texture offsets:', ' '.join(f'{x:#x}' for x in tex[:6]), '...', f'{tex[-1]:#x}')
after = 0x30 + ntex * 4
print(f'table ends at {after:#x}; next words:', d[after:after + 16].hex(' '))

dl = hdr[2]
size = hdr[3]
print(f'display list at {dl:#x}, region size {size:#x} -> vertex data from {dl + size:#x}')

# Walk the main list, collecting the sublists it calls.
origin = dl
calls = collections.Counter()
cmds = collections.Counter()
vtypes = collections.Counter()
prims = collections.Counter()
o = dl
n = 0
main_end = None
while o < dl + size:
    w = struct.unpack_from('<I', d, o)[0]
    cmd, arg = w >> 24, w & 0xffffff
    cmds[cmd] += 1
    if cmd == 0x0A:
        calls[origin + arg] += 1
    elif cmd == 0x12:
        vtypes[arg] += 1
    elif cmd == 0x04:
        prims[(arg >> 16) & 7] += 1
    elif cmd in (0x0B, 0x0C) and main_end is None:
        main_end = o + 4
    o += 4
print('main list ends at', hex(main_end) if main_end else None)
print('commands in region:', {GE.get(k, hex(k)): v for k, v in sorted(cmds.items())})
print('vertex types:', {hex(k): (v, vtype_str(k), vsize(k)) for k, v in vtypes.items()})
print('prim types:', {PRIMS[k]: v for k, v in prims.items()})
targets = sorted(calls)
print(f'{len(targets)} distinct call targets; first few:', [hex(t) for t in targets[:8]])
shown = 0
prev = None
for t in targets:
    # Print the first two sublists of each "family" (a jump in spacing starts a new one).
    if prev is None or t - prev not in (0x30, 0x28) or shown < 2:
        print(f'  -- sublist at {t:#x} (called {calls[t]}x)')
        disasm(d, t)
        shown += 1
    if prev is not None and t - prev not in (0x30, 0x28):
        shown = 1
    prev = t
