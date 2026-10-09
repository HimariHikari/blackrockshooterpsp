"""Find loads/stores at a given struct offset in the executable. Exploration only.

Usage: mips_find.py <hex offset> [store|load|any]
Lists sw/sh/sb/swc1 (store) or lw/lh/lb/lbu/lhu/lwc1 (load) instructions using that offset.
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

STORES = {0x2b: 'sw', 0x29: 'sh', 0x28: 'sb', 0x39: 'swc1'}
LOADS = {0x23: 'lw', 0x21: 'lh', 0x20: 'lb', 0x24: 'lbu', 0x25: 'lhu', 0x31: 'lwc1'}

want = int(sys.argv[1], 16)
mode = sys.argv[2] if len(sys.argv) > 2 else 'store'
ops = dict(STORES) if mode == 'store' else dict(LOADS) if mode == 'load' else {**STORES, **LOADS}
last_func = None
for a in range(0, mips.TEXT_END, 4):
    w = mips.word(a)
    op, base, imm = w >> 26, (w >> 21) & 31, w & 0xffff
    if op in ops and imm == want and base not in (29,):  # skip stack-relative
        func = mips.function_bounds(a)[0]
        print(f'{a:#08x} (function {func:#08x})  {mips.disasm_one(a)}')
