"""Which engine functions an `SSCR` script calls, from its disassembly (`sscr.py`).

Usage: sscr_calls.py <disassembly .txt> [natives table .txt]

A call of an engine function is `li 0xNN01, rv` followed by `callr rv`: NN is the
function's number in the table the script runs with. Prints each number with how many
times it is called and the functions of the script that call it; with the table's
listing (`sscr_natives.py table`), the address of each as well.
"""
import re, sys


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    lines = open(sys.argv[1], encoding='utf-8').read().splitlines()
    table = {}
    if len(sys.argv) > 2:
        for line in open(sys.argv[2], encoding='utf-8'):
            found = re.match(r'\s*(0x[0-9a-f]+|\d+)\s*[:=]?\s*(0x[0-9a-f]+)(.*)', line.strip(), re.I)
            if found:
                table[int(found.group(1), 0)] = (found.group(2) + found.group(3)).strip()
    calls, function = {}, '?'
    for index, line in enumerate(lines[:-1]):
        label = re.match(r'^(\w+):$', line)
        if label:
            function = label.group(1)
        load = re.search(r'li\s+0x([0-9a-f]+)01, rv', line)
        if load and re.search(r'callr\s+rv', lines[index + 1]):
            number = int(load.group(1), 16)
            calls.setdefault(number, []).append(function)
    for number in sorted(calls):
        users = sorted(set(calls[number]))
        print(f'{number:#04x} x{len(calls[number]):3}  {table.get(number, "")}')
        print(f'        {", ".join(users)}')
    print(f'{len(calls)} functions')


if __name__ == '__main__':
    main()
