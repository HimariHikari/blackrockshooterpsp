"""Lists the tasks the executable makes (0xAE3C): each call's name, its priority (what
is drawn or run before what: lower first) and its function, in order of priority.
The name, the priority and the function are set in the instructions before the call
or in its delay slot. Exploration only.

Usage: task_makers.py [text the name has]
"""
import re
import sys

import mips

MAKE = 0xAE3C
BACK = 24


def main():
    want = sys.argv[1].lower() if len(sys.argv) > 1 else ''
    call = 0x0c000000 | (MAKE >> 2)
    found = []
    for a in range(0, mips.TEXT_END, 4):
        if mips.word(a) != call:
            continue
        # The instructions that set the arguments, nearest first: the delay slot,
        # then back from the call. A register's high half is set before its low.
        priority = name = function = None
        high = {}
        lines = [mips.disasm_one(b) for b in range(a - 4 * BACK, a + 8, 4)]
        for text in lines:
            m = re.match(r'lui \$(\w+), (0x[0-9a-f]+|\d+)', text)
            if m:
                high[m.group(1)] = int(m.group(2), 0) << 16
            m = re.match(r'ori \$a2, \$zero, (0x[0-9a-f]+|\d+)', text)
            if m:
                priority = int(m.group(1), 0)
            m = re.match(r'addiu \$(a1|a3), \$(\w+), (-?0x[0-9a-f]+|-?\d+)', text)
            if m and m.group(2) in high:
                value = (high[m.group(2)] + int(m.group(3), 0)) & 0xffffffff
                if m.group(1) == 'a1':
                    name = mips.cstring(value) if mips.is_string(value) else f'{value:#x}'
                else:
                    function = value
        found.append((priority if priority is not None else -1, a, name, function))
    for priority, a, name, function in sorted(found, key=lambda item: (item[0], item[1])):
        if want and want not in (name or '').lower():
            continue
        shown = f'{priority:#06x}' if priority >= 0 else '     ?'
        print(f'{shown}  {name or "?":32}  function {function:#x}' if function else f'{shown}  {name or "?":32}  function ?', f' (made at {a:#x})')


main()
