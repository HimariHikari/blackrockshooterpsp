"""The handler of functions of the field's script table 1 (dispatcher 0xC9B58, whose jump
table is at 0x16C0D0): the code the dispatcher jumps to for each, which as a rule sets
the arguments up and calls the function proper.

Usage: fn1.py <number> [<number> ...]     prints each function's handler address
       fn1.py dump <number> ...           and disassembles each (to the next function)
"""
import sys

import mips

TABLE, COUNT = 0x16c0d0, 281


def handler(number):
    # The table starts at function 1.
    return mips.word(TABLE + (number - 1) * 4)


def main():
    arguments = sys.argv[1:]
    dump = arguments and arguments[0] == 'dump'
    if dump:
        arguments = arguments[1:]
    every = sorted(set(handler(n) for n in range(1, COUNT + 1)))
    refs = dict(mips.load_refs())
    for number in (int(a) for a in arguments):
        at = handler(number)
        later = [h for h in every if h > at]
        end = later[0] if later else at + 0x200
        print(f'=== s{number}: {at:#x} .. {end:#x}')
        if dump:
            for a in range(at, min(end, at + 0x600), 4):
                print(mips.annotate(a, refs))


if __name__ == '__main__':
    main()


