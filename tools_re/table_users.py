"""List how the code uses the pointer a getter function returns. Exploration only.

Usage: table_users.py <getter vaddr hex> [instructions to scan after each call, default 10]

For each call of the getter, prints the loads made through the returned pointer ($v0, or
a register it was copied to) in the instructions that follow: the offsets show which
fields of the table each function reads.
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

getter = int(sys.argv[1], 16)
span = int(sys.argv[2]) if len(sys.argv) > 2 else 10
jal = 0x0C000000 | (getter >> 2)
LOADS = {0x20: 'lb', 0x24: 'lbu', 0x21: 'lh', 0x25: 'lhu', 0x23: 'lw', 0x31: 'lwc1'}
by_offset = {}
for a in range(0, mips.TEXT_END, 4):
    if mips.word(a) != jal:
        continue
    holders = {2}  # $v0
    found = []
    for b in range(a + 8, a + 8 + span * 4, 4):
        w = mips.word(b)
        op, rs, rt, imm = w >> 26, (w >> 21) & 31, (w >> 16) & 31, w & 0xFFFF
        if op == 0 and (w & 0x7FF) == 0x25 and rs in holders and rt == 0:      # move rd, rs
            holders.add((w >> 11) & 31)
        elif op == 0 and (w & 0x7FF) == 0x25 and ((w >> 11) & 31) in holders:   # holder overwritten
            holders.discard((w >> 11) & 31)
        elif op in LOADS and rs in holders:
            found.append(f'{LOADS[op]} {imm:#x}')
            by_offset.setdefault(imm, []).append(a)
        elif op == 3:                                                          # another call: $v0 is gone
            holders.discard(2)
        if w == 0x03E00008:
            break
    print(f'{a:#08x} (fn {mips.function_bounds(a)[0]:#x}): ' + (', '.join(found) or '-'))
print()
for offset in sorted(by_offset):
    print(f'{offset:#06x}: ' + ' '.join(f'{a:#x}' for a in by_offset[offset]))
