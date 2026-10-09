"""A jump table in short: which cases go where.

Usage: jt.py <table address hex> <entries> [<first case>]

Prints each target once with the cases that lead to it. Exploration only.
"""
import sys

from mips import word


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return
    table, count = int(sys.argv[1], 16), int(sys.argv[2], 0)
    first = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0
    targets = {}
    for index in range(count):
        targets.setdefault(word(table + index * 4), []).append(first + index)
    for target in sorted(targets):
        cases = ', '.join(f'{case:#x}' for case in targets[target])
        print(f'{target:#08x}: {cases}')


if __name__ == '__main__':
    main()
