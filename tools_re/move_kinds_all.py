"""Every kind of enemy move the engine's switch has a case for, as runs of numbers
that share the case's code, by following the switch for each number in turn. With
each run goes the first function its case calls, which is the move's own code where
it has any (a case that does its work inline calls none, or only a helper).

Usage: move_kinds_all.py        -> tools_re/out/move_kinds.txt
"""
import os

import mips
from move_kinds import DEFAULTS, handler

HERE = os.path.dirname(os.path.abspath(__file__))


def first_call(at):
    """The target of the first call in the case at `at`, before it leaves for the tail."""
    for a in range(at, at + 4 * 12, 4):
        w = mips.word(a)
        if w >> 26 == 3:
            return (w & 0x3ffffff) << 2
        if w >> 16 == 0x1000 or w == 0x03e00008:      # an unconditional branch, or a return
            return None
    return None


runs = []       # [first, last, code]
for kind in range(0, 12000):
    at = handler(kind)
    if at is None or at in DEFAULTS:
        continue
    if runs and runs[-1][1] == kind - 1 and runs[-1][2] == at:
        runs[-1][1] = kind
    else:
        runs.append([kind, kind, at])

lines = []
for first, last, at in runs:
    call = first_call(at)
    calls = f'  calls {call:#x}' if call is not None else '  inline'
    lines.append((f'{first}-{last}' if first != last else f'{first}') + f'  {at:#x}{calls}')
lines.append(f'# {sum(last - first + 1 for first, last, _ in runs)} kinds in {len(runs)} runs')
out = os.path.join(HERE, 'out', 'move_kinds.txt')
open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print('\n'.join(lines))
