"""Lists the challenges ("records") of `SYSTEM.VOL/rec_script.ss`: the table
`lst_record` (60-byte records, `n_record` of them) and the tables its functions
match a battle's events against (engine function 0x5F, 0x14A6E0).

A record: name (+0), the count it is cleared at (+4) and the span it is announced
by (+6), its category (+8: 0 battles, 1 missions), the stage it is for (+9), flags
(+10: 1 open from the start, 2 opened by its first count, 4 kept by a retry, 8 only
on its stage, 0x10 not shown as it opens, 0x20 always announced), a byte (+11),
three texts (+12, +16, +20) and three rewards of 12 bytes (+24: kind, a number,
another number, a text).

Usage: rec_table.py [--tables]

Exploration only; the port runs the script itself.
"""
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / 'game' / 'unpacked'
data = (ROOT / 'SYSTEM.VOL' / 'rec_script.ss').read_bytes()

RECORDS_AT, RECORD = 0x1850, 60
REWARDS = {0: '-', 1: 'item', 2: 'skill', 5: 'record', 6: 'flag', 7: 'gallery', 8: 'skill up', 9: 'text'}


def u32(at):
    return struct.unpack_from('<I', data, at)[0]


# A pointer is nothing in the file itself: the table of relocations (pairs of where
# a pointer is and what it points at; the header has how many and where) fills it.
relocations, relocations_at = struct.unpack_from('<H', data, 0x0a)[0], u32(0x10)
POINTS_AT = dict(struct.unpack_from('<II', data, relocations_at + 8 * n) for n in range(relocations))


def text(pointer_at):
    """The text the pointer at that place is to; nothing for none."""
    at = POINTS_AT.get(pointer_at)
    if at is None:
        return None
    end = data.index(b'\0', at)
    return data[at:end].decode('shift_jis', 'replace')


count = u32(0x184c)
print(f'{count} records')
for number in range(count):
    at = RECORDS_AT + number * RECORD
    goal, span, category, stage, flags, byte = struct.unpack_from('<hhBBBB', data, at + 4)
    texts = [text(at + 12 + 4 * n) for n in range(3)]
    rewards = []
    for n in range(3):
        kind, _, a, b = struct.unpack_from('<BBhi', data, at + 24 + 12 * n)
        words = text(at + 24 + 12 * n + 8)
        if kind:
            rewards.append(f'{REWARDS.get(kind, kind)}({a}, {b}{", " + repr(words) if words else ""})')
    print(f'{number:3} {text(at)!r:44} goal {goal:4} span {span:3} {"MIS" if category else "BTL"} stage {stage} flags {flags:#04x} b {byte}')
    print(f'      {texts}')
    print(f'      rewards: {"; ".join(rewards) or "none"}')

if '--tables' in sys.argv:
    for name, at, size, count_at in (('kills', 0x5c2c, 12, 0x5eb4), ('kills with a skill', 0x5e48, 12, 0x5eb8), ('battles finished', 0x677c, 16, 0x684c), ('levels', 0x6610, 4, 0x6614)):
        entries = u32(count_at)
        print(f'{name}: {entries} entries of {size} bytes at {at:#x}')
        for n in range(entries):
            words = struct.unpack_from(f'<{size // 4}i', data, at + n * size)
            print('   ', ' '.join(f'{word:6}' for word in words))
