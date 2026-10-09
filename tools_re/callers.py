"""Lists the places that call or take the address of functions in the executable.

Usage: callers.py <address hex> [...]
Prints each `jal` to the address and each place its address is formed (function pointers
stored in objects), with the start of the function the reference is in.
"""
import sys

import mips

targets = [int(a, 16) for a in sys.argv[1:]]
refs = mips.load_refs()
for target in targets:
    print(f'== {target:#x}')
    jal = 0x0c000000 | (target >> 2)
    for a in range(0, mips.TEXT_END, 4):
        if mips.word(a) == jal:
            print(f'  call at {a:#08x}  (function {mips.function_bounds(a)[0]:#x})')
    for a, value in refs:
        if value == target:
            print(f'  address taken at {a:#08x}  (function {mips.function_bounds(a)[0]:#x})')
