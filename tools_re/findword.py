"""Finds where a word is stored in the executable's data: a function's address in a
table of them, say. Exploration only.

Usage: findword.py <word hex> [<word hex> ...]
"""
import sys

import mips

DATA_START, DATA_END = 0x150000, 0x1d9000


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    for wanted in (int(argument, 16) for argument in sys.argv[1:]):
        hits = [address for address in range(DATA_START, DATA_END, 4) if mips.word(address) == wanted]
        print(f'{wanted:#x}:', ' '.join(f'{address:#x}' for address in hits) or 'nowhere')


if __name__ == '__main__':
    main()
