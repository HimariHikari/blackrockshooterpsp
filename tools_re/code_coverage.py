"""How much of the executable's code the port's notes and sources speak of.

Finds the executable's functions (each begins by making its stack frame), gathers every
address the notes (docs/*.md) and the port's sources (crates/**/*.rs) name, and says how
many functions, and how much of the code, have at least one address named in them. A
function that is named has been read; how thoroughly, this cannot say. Functions that
make no stack frame (small leaves) count with the function before them.

Usage (from tools_re): python code_coverage.py
"""
import glob, os, re, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips  # noqa: E402

CODE_END = 0x165000


def word(vaddr):
    return struct.unpack_from('<I', mips.DATA, vaddr + mips.FILE_BIAS)[0]


starts = [at for at in range(0, CODE_END, 4) if word(at) >> 16 == 0x27bd and word(at) & 0x8000]
bounds = starts + [CODE_END]

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
named = set()
files = glob.glob(os.path.join(root, 'docs', '*.md')) + glob.glob(os.path.join(root, 'crates', '**', '*.rs'), recursive=True) \
    + glob.glob(os.path.join(root, 'crates', '**', '*.wgsl'), recursive=True) + [os.path.join(root, 'README.md')]
for path in files:
    for found in re.findall(r'0x([0-9A-Fa-f]{4,6})\b', open(path, encoding='utf-8', errors='replace').read()):
        value = int(found, 16)
        if starts[0] <= value < CODE_END:
            named.add(value & ~3)

import bisect
read = set()
for value in named:
    read.add(bisect.bisect_right(starts, value) - 1)
sizes = [bounds[i + 1] - bounds[i] for i in range(len(starts))]
total = sum(sizes)
print('%d files looked through, %d different addresses of code named' % (len(files), len(named)))
print('functions: %d of %d have an address named (%.0f%%)' % (len(read), len(starts), 100.0 * len(read) / len(starts)))
print('code: %d of %d bytes are in those functions (%.0f%%)' % (sum(sizes[i] for i in read), total, 100.0 * sum(sizes[i] for i in read) / total))
# By the parts of the executable, as the notes have them.
parts = [(0x0, 0x3000, 'the game\'s own start and block'), (0x3000, 0x1a000, 'saves, sound, system'), (0x1a000, 0x48000, 'models, animation, layouts, effects'),
         (0x48000, 0x70000, 'menus, title, hub'), (0x70000, 0xc0000, 'battle'), (0xc0000, 0x132000, 'field'), (0x132000, 0x14a000, 'bike'), (0x14a000, CODE_END, 'libraries (maths, C, the PSP\'s)')]
for low, high, name in parts:
    inside = [i for i in range(len(starts)) if low <= starts[i] < high]
    if inside:
        size = sum(sizes[i] for i in inside)
        got = sum(sizes[i] for i in inside if i in read)
        print('  %-38s %6x-%6x  %4d functions, %3.0f%% of its code named' % (name, low, high, len(inside), 100.0 * got / size))
