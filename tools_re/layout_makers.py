"""Lists the code that makes layouts of the interface: every function that calls the
layout library's constructor (0x305B8), with the packs it takes (the resident
resources' getters), the files of a pack it reads (the words it loads at an offset
of 0xC + 4n from a pack's start) and the names of tasks near it. Exploration only.

Usage: layout_makers.py
"""
import re

import mips

MAKE = 0x305B8
GETTERS = {
    0x2CC94: 'if_rsdsys',
    0x2CCC4: 'if_fldsys',
    0x2CCF4: 'if_btlsys',
    0x2CD24: 'if_rsdsys/009.ptm',
    0x2CD58: 'if_rsdsys/010.ptm',
}


def calls(start, end):
    for a in range(start, end, 4):
        w = mips.word(a)
        if w >> 26 == 3:
            yield a, (w & 0x3ffffff) << 2


def main():
    refs = dict(mips.load_refs())
    call = 0x0c000000 | (MAKE >> 2)
    makers = {}
    for a in range(0, mips.TEXT_END, 4):
        if mips.word(a) == call:
            makers.setdefault(mips.function_bounds(a)[0], []).append(a)
    for start, sites in sorted(makers.items()):
        end = mips.function_bounds(sites[-1])[1]
        packs = [GETTERS[target] for _, target in calls(start, end) if target in GETTERS]
        resources = []
        for a, target in calls(start, end):
            if target == 0x97DC:
                # The number of the resource is set just before, or in the delay slot.
                for b in (a + 4, a - 4, a - 8):
                    text = mips.disasm_one(b).replace(' ', '')
                    m = re.fullmatch(r'ori\$a1,\$zero,(0x[0-9a-f]+|\d+)', text)
                    if m:
                        resources.append(int(m.group(1), 0))
                        break
        files = sorted({(int(m.group(1), 16) - 0xC) // 4
                        for a in range(start, end, 4)
                        for m in [re.fullmatch(r'lw \$\w+, (0x[0-9a-f]+)\(\$\w+\)', mips.disasm_one(a))]
                        if m and 0xC <= int(m.group(1), 16) <= 0x100 and int(m.group(1), 16) % 4 == 0})
        names = sorted({mips.cstring(target) for a, target in refs.items()
                        if start - 0x600 <= a < end + 0x200 and mips.is_string(target) and 'Task' in mips.cstring(target)})
        print(f'{start:#x} ({len(sites)} call{"s" if len(sites) > 1 else ""}, to {end:#x}): packs {packs or "-"}'
              + (f', resources {resources}' if resources else '') + f', files at {files}' + (f', near {names}' if names else ''))


main()
