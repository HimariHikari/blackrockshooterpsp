"""Surveys the effect files of the unpacked game: how many there are of each kind, their
sizes, and what they start with.

usage: python efc_survey.py [path of one file to show the head of]
"""
import collections
import os
import struct
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'unpacked')


def head(path, count=0x80):
    with open(path, 'rb') as f:
        data = f.read(count)
    for at in range(0, len(data), 16):
        row = data[at:at + 16]
        text = ''.join(chr(byte) if 32 <= byte < 127 else '.' for byte in row)
        print(f'  {at:04x}  ' + ' '.join(f'{byte:02x}' for byte in row).ljust(48) + '  ' + text)


if len(sys.argv) > 1:
    path = sys.argv[1] if os.path.isabs(sys.argv[1]) else os.path.join(ROOT, sys.argv[1])
    print(path, os.path.getsize(path))
    head(path, int(sys.argv[2], 0) if len(sys.argv) > 2 else 0x100)
    sys.exit()

kinds = collections.defaultdict(list)
for folder, _, names in os.walk(ROOT):
    for name in names:
        extension = os.path.splitext(name)[1].lower()
        if extension in ('.efc', '.efp'):
            kinds[extension].append(os.path.join(folder, name))
for extension, paths in sorted(kinds.items()):
    sizes = sorted(os.path.getsize(path) for path in paths)
    magics = collections.Counter()
    for path in paths:
        with open(path, 'rb') as f:
            magics[f.read(4)] += 1
    print(extension, len(paths), 'files; sizes', sizes[0], 'to', sizes[-1], 'median', sizes[len(sizes) // 2], 'total', sum(sizes))
    print('   magics', dict(magics))
    places = collections.Counter(os.path.relpath(os.path.dirname(path), ROOT).split(os.sep)[0] for path in paths)
    print('   in', dict(places))
    for path in paths[:3]:
        print('  ', os.path.relpath(path, ROOT), os.path.getsize(path))
