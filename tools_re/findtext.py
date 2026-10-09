"""Finds byte strings in the unpacked game data, binary files included.

Usage: findtext.py <text> [<text> ...] [--under <folder relative to game/unpacked>]

Prints each file that holds one of the texts, with which and how often. Exploration only.
"""
import os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def main():
    arguments = sys.argv[1:]
    under = ''
    if '--under' in arguments:
        at = arguments.index('--under')
        under = arguments[at + 1]
        del arguments[at:at + 2]
    if not arguments:
        print(__doc__)
        return
    wanted = [text.encode('utf-8') for text in arguments]
    for folder, _, names in os.walk(os.path.join(ROOT, under)):
        for name in sorted(names):
            path = os.path.join(folder, name)
            try:
                data = open(path, 'rb').read()
            except OSError:
                continue
            found = [(text.decode(), data.count(text)) for text in wanted if text in data]
            if found:
                where = os.path.relpath(path, ROOT)
                print(where, ' '.join(f'{text}x{count}' for text, count in found))


if __name__ == '__main__':
    main()
