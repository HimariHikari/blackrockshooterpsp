"""Which of the engine functions the field maps' scripts call the port has, and which
it still lacks: reads the port's own source (`field/host.rs`) for what is ported, so
it stays in step with it, and every map's script for what is called.

Usage: fld_coverage.py     -> tools_re/out/fld_coverage.txt and a summary
"""
import os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bscr
from fld_scripts import maps

HOST = os.path.join(HERE, '..', 'crates', 'brs-game', 'src', 'field', 'host.rs')
SUMMARY = os.path.join(HERE, 'out', 'natives1_summary.txt')


def ported():
    """The numbers the map script's engine answers: the arms of its match."""
    text = open(HOST, encoding='utf-8').read()
    body = text[text.index('fn engine_with('):text.index('impl Engine<')]
    numbers = set()
    for found in re.finditer(r'^ {12}(\d+(?: \| \d+)*) =>', body, re.M):
        numbers.update(int(n) for n in found.group(1).split(' | '))
    return numbers


def handlers():
    """What `natives2.py 1` found for each function: its arguments and its code."""
    table = {}
    if os.path.exists(SUMMARY):
        for line in open(SUMMARY, encoding='utf-8'):
            found = re.match(r'\s*(\d+)\s+argc\s+(\d+)\s+(ret)?\s*at (0x[0-9a-f]+)\s+calls (\S+)', line)
            if found:
                table[int(found.group(1))] = (int(found.group(2)), bool(found.group(3)), found.group(5))
    return table


def main():
    calls, in_maps = {}, {}
    count = 0
    for _, script in maps():
        count += 1
        seen = set()
        for _, op, kinds, _ in script.instructions():
            if op in (0x82, 0x86):
                number = kinds[0] + (0x100 if op == 0x86 else 0)
                calls[number] = calls.get(number, 0) + 1
                seen.add(number)
        for number in seen:
            in_maps[number] = in_maps.get(number, 0) + 1
    have, table = ported(), handlers()
    lines = []
    for wanted, title in ((False, 'not ported'), (True, 'ported')):
        lines.append(f'{title}: number, arguments, calls, maps, code')
        for number in sorted(calls, key=lambda n: -calls[n]):
            if (number in have) != wanted:
                continue
            arguments, returns, code = table.get(number, ('?', False, '?'))
            lines.append(f'  s{number:<4} {arguments} {"ret" if returns else "   "} {calls[number]:6} {in_maps[number]:3}  {code}')
        lines.append('')
    used = set(calls)
    summary = (f'{count} maps call {len(used)} functions; {len(used & have)} of them are ported, '
               f'which is {sum(calls[n] for n in used & have)} of {sum(calls.values())} calls')
    with open(os.path.join(HERE, 'out', 'fld_coverage.txt'), 'w', encoding='utf-8') as out:
        out.write(summary + '\n\n' + '\n'.join(lines))
    print(summary)
    print('\n'.join(lines[:42]))


if __name__ == '__main__':
    main()
