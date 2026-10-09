"""List where the executable reads each `btl_system.bin` tunable. Exploration only.

Every read goes through 0xA111C(id); the id is loaded in the delay slot or just before.
Prints id -> call sites (with the containing function).
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

TARGET = 0x0C000000 | (0xA111C >> 2)
users = {}
for a in range(0, mips.TEXT_END, 4):
    if mips.word(a) != TARGET:
        continue
    value = None
    for b in (a + 4, a - 4, a - 8):
        w = mips.word(b)
        # ori $a0, $zero, imm  /  addiu $a0, $zero, imm / move $a0, $zero
        if w >> 16 in (0x3404, 0x2404):
            value = w & 0xFFFF
            break
        if w == 0x00002025:
            value = 0
            break
    users.setdefault(value, []).append(a)
for value in sorted(users, key=lambda v: (v is None, v)):
    sites = ', '.join(f'{a:#x}(fn {mips.function_bounds(a)[0]:#x})' for a in users[value])
    print(f'{value}: {sites}')
