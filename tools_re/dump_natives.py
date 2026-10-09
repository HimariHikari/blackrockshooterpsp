"""Dumps the code of a set of a script table's engine functions into one file.

Usage: dump_natives.py <table listing .txt> <out name> <number hex> [<number hex> ...]

The listing is what `sscr_natives.py table` wrote (`0xNN: 0xADDRESS ...` per line). Each
function is disassembled whole, one after the other, into tools_re/out/<out name>.txt.
Exploration only.
"""
import os, re, sys

import mips

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return
    table = {}
    for line in open(sys.argv[1], encoding='utf-8'):
        found = re.match(r'\s*(0x[0-9a-f]+):\s*(0x[0-9a-f]+)', line, re.I)
        if found:
            table[int(found.group(1), 16)] = int(found.group(2), 16)
    # A function with a switch has more than one return: it runs up to the next
    # function of the table, or to the first return after 0x600 bytes if that is far.
    addresses = sorted(set(table.values()))
    out = []
    for text in sys.argv[3:]:
        number = int(text, 16)
        address = table[number]
        following = [other for other in addresses if other > address]
        end = following[0] if following and following[0] - address <= 0x600 else mips.function_bounds(address + 0x600)[1]
        out.append(f'==== {number:#04x} at {address:#x} ({(end - address) // 4} instructions)')
        for a in range(address, end, 4):
            line = mips.disasm_one(a)
            word = mips.word(a)
            # A jal's target, and an address built by lui + addiu, are worth a note.
            if word >> 26 == 3:
                line += f'   ; call {(word & 0x3ffffff) << 2:#x}'
            out.append(f'{a:06x}  {line}')
    path = os.path.join(HERE, 'out', sys.argv[2] + '.txt')
    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(out) + '\n')
    print(f'wrote {os.path.relpath(path)} ({len(out)} lines)')


if __name__ == '__main__':
    main()
