"""Shows what is in an effect file (`EFC`) or a pack of effects' parts (`EFP`): its head
and the parts that follow it, each with its size, its kind, its name and what its
data begins with.

usage: python efc.py <file under game/unpacked> [--hex part [count]]
       python efc.py --all        (the kinds and counts over every file)
"""
import collections
import os
import struct
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'unpacked')


def parts(data):
    """The parts after the head: (offset of its data, size, kind, name)."""
    # A chain (0x126E80): each part's head is 0x20 bytes, its first word how far
    # it is to the next part's head, the byte after that its kind; a part of kind
    # nought ends the chain, and has the file's own name.
    at = 0x10
    out = []
    while at + 0x20 <= len(data):
        size = struct.unpack_from('<I', data, at)[0]
        kind = data[at + 4]
        name = data[at + 8:at + 0x20].split(b'\0')[0].decode('ascii', 'replace')
        if kind == 0:
            return out, at + 0x20
        if size < 0x20 or at + size > len(data):
            break
        out.append((at + 0x20, size - 0x20, kind, name))
        at += size
    return out, at


def hexdump(data, start, count):
    for at in range(start, min(start + count, len(data)), 16):
        row = data[at:at + 16]
        text = ''.join(chr(byte) if 32 <= byte < 127 else '.' for byte in row)
        print(f'  {at:05x}  ' + ' '.join(f'{byte:02x}' for byte in row).ljust(48) + '  ' + text)


def every():
    counts = collections.Counter()
    kinds = collections.Counter()
    heads = collections.Counter()
    for folder, _, names in os.walk(ROOT):
        for name in names:
            if os.path.splitext(name)[1].lower() not in ('.efc', '.efp'):
                continue
            with open(os.path.join(folder, name), 'rb') as f:
                data = f.read()
            found, end = parts(data)
            heads[(data[:4], data[4:8].hex(), data[12:16].hex())] += 1
            for _, _, kind, part in found:
                kinds[(kind, os.path.splitext(part)[1])] += 1
            counts[(len(found), end == len(data) or end)] += 0
            if end != len(data):
                counts['left over'] += 1
    print('heads:')
    for head, count in heads.most_common(40):
        print('  ', head, count)
    print('kinds:', dict(kinds))
    print('files whose parts do not come to their end:', counts['left over'])


def main():
    if sys.argv[1] == '--all':
        every()
        return
    path = sys.argv[1] if os.path.isabs(sys.argv[1]) else os.path.join(ROOT, sys.argv[1])
    with open(path, 'rb') as f:
        data = f.read()
    print(path, len(data))
    print('head:', data[:4], 'version', struct.unpack_from('<I', data, 4)[0], 'word', struct.unpack_from('<I', data, 8)[0], 'bytes', list(data[12:16]))
    found, end = parts(data)
    for number, (at, size, kind, name) in enumerate(found):
        print(f'  part {number}: at {at:#x} size {size:#x} kind {kind} {name!r} begins {data[at:at + 8]!r}')
    print(f'  ends at {end:#x} of {len(data):#x}')
    if '--hex' in sys.argv:
        which = int(sys.argv[sys.argv.index('--hex') + 1])
        count = int(sys.argv[sys.argv.index('--hex') + 2], 0) if len(sys.argv) > sys.argv.index('--hex') + 2 else 0x200
        at, size, _, _ = found[which]
        hexdump(data, at, min(size, count))


main()
