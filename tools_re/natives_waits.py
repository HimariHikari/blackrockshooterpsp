"""Which engine functions of table 1 make the script wait, and on what.

A function that waits sets bit 0x80 of the script context and stores a callback at
context + 0x310; the context's update (0xC9838) calls it each frame and lets the script go
on once it answers non-zero. This lists each such function with its callback(s).

Usage: natives_waits.py   -> tools_re/out/natives1_waits.txt
"""
import os

import mips
from natives2 import cases, calls_of

WAIT_SLOT = 0x310


def waits_in(start, end, refs):
    """Callback addresses stored at context + 0x310 between start and end."""
    found = []
    for a in range(start, end, 4):
        w = mips.word(a)
        # sw rt, 0x310(rs)
        if w >> 26 == 0x2b and (w & 0xffff) == WAIT_SLOT:
            rt = (w >> 16) & 31
            if rt == 0:
                continue
            # The callback's address was built into rt a little earlier.
            for b in range(a + 4, max(start, a - 4 * 12), -4):
                wb = mips.word(b)
                if b in refs and (wb >> 16) & 31 == rt and wb >> 26 in (0x09, 0x0d):
                    found.append(refs[b])
                    break
            else:
                found.append(None)
    return found


def main():
    refs = dict(mips.load_refs())
    lines = []
    for number, argc, returns, start, end in cases(1):
        callbacks = waits_in(start, end, refs)
        for call in calls_of(start, end):
            target = int(call, 16)
            f_start, f_end = mips.function_bounds(target)
            if f_start != target:
                f_start = target
            callbacks += waits_in(f_start, f_end, refs)
        if callbacks:
            text = ', '.join('?' if c is None else f'{c:#x}' for c in callbacks)
            lines.append(f's{number:<3} argc {argc:2d}  waits on {text}')
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', 'natives1_waits.txt')
    open(path, 'w').write('\n'.join(lines) + '\n')
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
