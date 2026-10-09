"""Prints disassembly of address ranges, compactly, straight from the executable.

Usage: show.py <from hex> <to hex> [<from> <to> ...]
       show.py f <address> [...]     whole functions containing the addresses
"""
import struct, sys

import mips

refs = dict(mips.load_refs())


def show(start, end):
    for a in range(start, end, 4):
        w = mips.word(a)
        text = mips.disasm_one(a)
        note = ''
        if w >> 26 == 3:
            note = f'   ; call {(w & 0x3ffffff) << 2:#x}'
        elif w >> 26 == 0x0f and (w & 0xffff) >= 0x3000:
            f = struct.unpack('<f', struct.pack('<I', (w & 0xffff) << 16))[0]
            if 1e-6 < abs(f) < 1e9:
                note = f'   ; {f:g}f?'
        if a in refs:
            target = refs[a]
            note = f'   ; -> {target:#x}' + (f' "{mips.cstring(target)}"' if mips.is_string(target) else '')
            if not mips.is_string(target) and (w >> 26) in (0x09, 0x0d):
                f = struct.unpack('<f', struct.pack('<I', target))[0]
                if 1e-6 < abs(f) < 1e9 and target > 0x200000:
                    note += f' = {f:g}f'
        if w == 0x03e00008:
            note += '   ; ---- return'
        print(f'{a:06x}  {text}{note}')


args = sys.argv[1:]
if args and args[0] == 'f':
    for address in args[1:]:
        start, end = mips.function_bounds(int(address, 16))
        print(f'== function {start:#x}..{end:#x}')
        show(start, end)
else:
    for i in range(0, len(args), 2):
        show(int(args[i], 16), int(args[i + 1], 16))
        print()
