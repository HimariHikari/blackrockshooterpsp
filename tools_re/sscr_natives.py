"""Finds tables of the engine functions SSCR scripts call. Exploration only.

An SSCR engine function takes its arguments from the script's context (0x1134C8) and
answers through it (0x1135F4). A script kind has a table of pointers to its functions.

Usage: sscr_natives.py find <fn hex> [<fn hex> ...]   functions that read an argument
                                                      and call one of these, and where
                                                      pointers to them are stored
       sscr_natives.py table <addr hex> <count>       the table's entries, with what each
                                                      one calls
"""
import struct, sys

import mips

GET_ARGUMENT = 0x1134c8


def calls_in(start, end):
    out = []
    for a in range(start, end, 4):
        w = mips.word(a)
        if w >> 26 == 3:
            out.append((w & 0x3ffffff) << 2)
    return out


def callers(fn):
    encoded = 0x0c000000 | (fn >> 2)
    return {mips.function_bounds(a) for a in range(0, mips.TEXT_END, 4) if mips.word(a) == encoded}


def find(functions):
    found = set()
    for fn in functions:
        for start, end in callers(fn):
            if GET_ARGUMENT in calls_in(start, end):
                found.add(start)
    print('functions:', ', '.join(hex(s) for s in sorted(found)))
    for off in range(mips.TEXT_END + mips.FILE_BIAS, len(mips.DATA) - 4, 4):
        value = struct.unpack_from('<I', mips.DATA, off)[0]
        if value in found:
            print(f'pointer to {value:#x} at {off - mips.FILE_BIAS:#x}')


def table(address, count):
    for index in range(count):
        fn = mips.word(address + index * 4)
        if fn == 0 or fn >= mips.TEXT_END:
            print(f'{index:#04x}: {fn:#x}')
            continue
        start, end = mips.function_bounds(fn)
        called = [c for c in calls_in(fn, end) if c not in (GET_ARGUMENT, 0x1135f4, 0x11349c)]
        size = (end - fn) // 4
        print(f'{index:#04x}: {fn:#x} ({size} instructions) calls ' + ' '.join(hex(c) for c in called[:10]))


if __name__ == '__main__':
    if sys.argv[1] == 'find':
        find([int(a, 16) for a in sys.argv[2:]])
    else:
        table(int(sys.argv[2], 16), int(sys.argv[3]))
