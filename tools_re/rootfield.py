"""Finds code that reads or writes a field of the battle's root object (the global at 0x2EAF0).

Usage: rootfield.py <offset hex> [r|w]

Looks for a load of the global (`lw x, -0x1510(y)` after `lui y, 3`) followed within a
few instructions by a load or store at the given offset from that register.
"""
import sys

import mips

offset = int(sys.argv[1], 16)
want = sys.argv[2] if len(sys.argv) > 2 else 'rw'

for a in range(0, mips.TEXT_END, 4):
    w = mips.word(a)
    # lw rt, -0x1510(rs)
    if w >> 26 != 0x23 or (w & 0xffff) != 0xeaf0:
        continue
    reg = (w >> 16) & 31
    for b in range(a + 4, a + 4 * 14, 4):
        v = mips.word(b)
        op, rs, imm = v >> 26, (v >> 21) & 31, v & 0xffff
        if rs == reg and imm == offset and op in (0x23, 0x2b, 0x31, 0x39, 0x20, 0x24, 0x28, 0x21, 0x25, 0x29):
            kind = 'w' if op in (0x2b, 0x39, 0x28, 0x29) else 'r'
            if kind in want:
                start, _ = mips.function_bounds(b)
                print(f'{b:#08x} (function {start:#08x}) {kind}: {mips.disasm_one(b)}')
        # The register is overwritten: stop following it.
        if op in (0x23, 0x09, 0x0f, 0x0d) and ((v >> 16) & 31) == reg and b != a:
            break
