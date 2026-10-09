"""The field maps' scripts, all together. Exploration only.

Usage: fld_scripts.py survey            every map's script: does it parse, how big, and how
                                        often each engine function of table 1 is called
                                        -> tools_re/out/fld_scripts_survey.txt
       fld_scripts.py grep <regex>      decompiled lines matching, over every map
       fld_scripts.py dec               decompiles every map -> tools_re/out/fld/<map>.txt

A map's script is slot 10 of its bundle under game/unpacked/FLD/MAP (see bscr.read).
"""
import os, re, sys

import bscr
from bscr_dec import decompile, TABLES

MAPS = os.path.join(bscr.ROOT, 'FLD', 'MAP')


def maps():
    for name in sorted(os.listdir(MAPS)):
        folder = os.path.join(MAPS, name)
        if os.path.isfile(os.path.join(folder, '10.bscr')):
            yield name, bscr.Script(bscr.read(os.path.join('FLD', 'MAP', name)))


def survey():
    lines, used, in_maps = [], {}, {}
    for name, s in maps():
        ok = s.size == len(s.d) and s.code_at + s.code_size == len(s.d) and s.func_end <= s.str_at
        entries = [f['name'] for f in s.funcs if f['type'] in (0xE7, 0xE8, 0xE9)]
        lines.append(f'{"ok " if ok else "BAD"} {name}: {s.name}, {s.nfunc} functions, {s.nstr} strings, '
                     f'{s.code_size // 16} instructions, entries {", ".join(entries)}')
        seen = set()
        for off, op, kinds, vals in s.instructions():
            if op in (0x82, 0x86):
                n = kinds[0] + (0x100 if op == 0x86 else 0)
                used[n] = used.get(n, 0) + 1
                seen.add(n)
            elif op in (0x87, 0x88):
                lines.append(f'    table 2 call at {off:#x}')
        for n in seen:
            in_maps[n] = in_maps.get(n, 0) + 1
    lines.append('')
    lines.append(f'table 1 functions called ({len(used)} of {len(TABLES[1])}): number, arguments, calls, maps')
    for n, count in sorted(used.items(), key=lambda item: -item[1]):
        argc, returns = TABLES[1].get(n, (0, False))
        lines.append(f'  s{n:<3} {argc:2d} {"ret" if returns else "   "} {count:6d} {in_maps[n]:3d}')
    never = sorted(set(TABLES[1]) - set(used))
    lines.append('never called: ' + ', '.join(f's{n}' for n in never))
    path = os.path.join(bscr.OUT, 'fld_scripts_survey.txt')
    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines) + '\n')
    print(f'wrote {os.path.relpath(path)} ({len(lines)} lines)')


def main():
    command = sys.argv[1]
    if command == 'survey':
        survey()
    elif command == 'grep':
        rx = re.compile(sys.argv[2])
        for name, s in maps():
            function = ''
            for line in decompile(s):
                if line and not line.startswith(' '):
                    function = line.split('(')[0]
                elif rx.search(line):
                    print(f'{name} {function}: {line.strip()}')
    elif command == 'dec':
        folder = os.path.join(bscr.OUT, 'fld')
        os.makedirs(folder, exist_ok=True)
        for name, s in maps():
            with open(os.path.join(folder, name + '.txt'), 'w', encoding='utf-8') as f:
                f.write('\n'.join(decompile(s)) + '\n')
        print(f'wrote {os.path.relpath(folder)}')
    elif command == 'usage':
        usage()


# Functions whose argument at this position (counting from 0) picks one of many cases.
SELECTORS = {52: 1, 132: 1, 143: 1, 96: 0, 45: 1, 47: 0, 104: 0, 105: 0, 79: 0, 224: 1}


def usage():
    """How often each case of the many-case functions is asked for, over every map."""
    counts = {number: {} for number in SELECTORS}
    for name, s in maps():
        for line in decompile(s):
            for number, position in SELECTORS.items():
                for call in re.finditer(rf'\bs{number}\(([^()]*)\)', line):
                    arguments = [a.strip() for a in call.group(1).split(',')]
                    if position < len(arguments) and re.fullmatch(r'-?\d+', arguments[position]):
                        case = int(arguments[position])
                        counts[number][case] = counts[number].get(case, 0) + 1
    for number, cases in counts.items():
        ordered = sorted(cases.items(), key=lambda item: -item[1])
        print(f's{number}: ' + ', '.join(f'{case}:{n}' for case, n in ordered))


if __name__ == '__main__':
    main()
