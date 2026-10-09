"""Find files in game/unpacked (or game/extracted with --raw) by magic or content. Exploration only.

Usage: findmagic.py magic <ascii or hex:..> [--raw]     files that START with the bytes
       findmagic.py contains <ascii or hex:..> [--raw]  files that CONTAIN the bytes (first hit)
       findmagic.py magics [--raw]                      histogram of the first four bytes
"""
import os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
raw = '--raw' in sys.argv
ROOT = os.path.join(HERE, '..', 'game', 'extracted' if raw else 'unpacked')
args = [a for a in sys.argv[1:] if a != '--raw']


def needle(text):
    return bytes.fromhex(text[4:]) if text.startswith('hex:') else text.encode()


def files():
    for folder, _, names in os.walk(ROOT):
        for name in names:
            yield os.path.join(folder, name)


mode = args[0]
if mode == 'magic':
    n = needle(args[1])
    hits = []
    for path in files():
        with open(path, 'rb') as f:
            if f.read(len(n)) == n:
                hits.append((os.path.relpath(path, ROOT), os.path.getsize(path)))
    for rel, size in hits[:200]:
        print(f'{size:10} {rel}')
    print(f'{len(hits)} files')
elif mode == 'contains':
    n = needle(args[1])
    count = 0
    for path in files():
        with open(path, 'rb') as f:
            d = f.read()
        pos = d.find(n)
        if pos >= 0:
            count += 1
            if count <= 200:
                print(f'{pos:#10x} {os.path.relpath(path, ROOT)} ({len(d)} bytes)')
    print(f'{count} files')
elif mode == 'magics':
    seen = {}
    for path in files():
        with open(path, 'rb') as f:
            m = f.read(4)
        entry = seen.setdefault(m, [0, os.path.relpath(path, ROOT)])
        entry[0] += 1
    for m, (count, example) in sorted(seen.items(), key=lambda kv: -kv[1][0])[:120]:
        print(f'{count:6} {m!r:22} {example}')
