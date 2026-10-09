"""Summarises the engine functions the scripts call, for either of the script machine's
two tables: for each number, its argument count, whether it returns a value, where its
case of the dispatcher is and the functions that case calls.

Table 1 (dispatcher 0xC9B58) is what a field map's script calls; table 2 (0xCB5B8) what
an enemy's battle script calls.

Usage: natives2.py [1|2]     -> tools_re/out/natives<table>_summary.txt (table 2 if left out)
       natives2.py 1 <number> [...]    the code of those cases of the dispatcher
"""
import os, struct, sys

import mips

# table: ({result kind, argument count, number} per function, the dispatcher's jump table
# by number - 1, how many numbers it has, where the dispatcher ends)
TABLES = {
    1: (0x171370, 0x16c0d0, 0x119, 0xcb5b8),
    2: (0x1720a8, 0x16c538, 0x7e, 0xcc360),
}


def cases(which):
    """(number, argument count, returns, start, end) for each function of a table."""
    table, jumps, count, end_of_all = TABLES[which]
    handlers = [mips.word(jumps + i * 4) for i in range(count)]
    starts = sorted(set(handlers)) + [end_of_all]
    for number in range(1, count + 1):
        kind, argc, _ = struct.unpack_from('<3I', mips.DATA, table + (number - 1) * 12 + mips.FILE_BIAS)
        start = handlers[number - 1]
        yield number, argc, kind == 0x81, start, starts[starts.index(start) + 1]


def calls_of(start, end):
    calls = []
    for a in range(start, end, 4):
        w = mips.word(a)
        if w >> 26 == 3:
            calls.append(f'{(w & 0x3ffffff) << 2:#x}')
        # A branch back to the common exit ends the case.
        if w >> 26 == 4 and (w >> 16) & 0x3ff == 0 and a > start:   # beq zero, zero
            break
    return calls


def main():
    which = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    wanted = {int(a) for a in sys.argv[2:]}
    if wanted:
        refs = dict(mips.load_refs())
        for number, argc, returns, start, end in cases(which):
            if number in wanted:
                print(f'--- function {number} of table {which}: {argc} arguments{", returns" if returns else ""}')
                for a in range(start, min(end, start + 4 * 120), 4):
                    print(mips.annotate(a, refs))
        return
    lines = []
    for number, argc, returns, start, end in cases(which):
        result = 'ret' if returns else '   '
        lines.append(f'{number:3d}  argc {argc:2d}  {result}  at {start:#x}  calls ' + (', '.join(calls_of(start, end)) or '-'))
    name = 'natives2_summary.txt' if which == 2 else f'natives{which}_summary.txt'
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', name)
    open(path, 'w').write('\n'.join(lines) + '\n')
    print(f'wrote {path}')


if __name__ == '__main__':
    main()
