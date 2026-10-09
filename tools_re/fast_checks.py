"""Which of the map scripts' engine functions look at the script context's flags, and
at which: 0x200 is set while an event is being skipped (a function that tests it does
something else then, usually nothing), 0x400 is set by functions that change what is
shown, 0x100 marks the part of an event that can be skipped.

Reads the handlers `natives2.py 1` found (tools_re/out/natives1_summary.txt): the
function each case of the dispatcher calls.

Usage: fast_checks.py      -> tools_re/out/fast_checks.txt and a summary
"""
import os, re, struct, sys

import mips

HERE = os.path.dirname(os.path.abspath(__file__))
SUMMARY = os.path.join(HERE, 'out', 'natives1_summary.txt')
FLAGS = (0x100, 0x200, 0x400)


def flags_tested(start):
    """The context flags a handler tests (andi) and sets (ori), in its own code."""
    begin, end = mips.function_bounds(start)
    tested, set_ = set(), set()
    for address in range(begin, end, 4):
        word = mips.word(address)
        op, immediate = word >> 26, word & 0xffff
        if op == 0x0c and immediate in FLAGS:       # andi
            tested.add(immediate)
        if op == 0x0d and immediate in FLAGS:       # ori
            set_.add(immediate)
    return tested, set_


def main():
    lines, fast = [], []
    for line in open(SUMMARY, encoding='utf-8'):
        found = re.match(r'\s*(\d+)\s+argc\s+(\d+)\s+(ret)?\s*at (0x[0-9a-f]+)\s+calls (\S+)', line)
        if not found or found.group(5) == '-':
            continue
        number, handler = int(found.group(1)), int(found.group(5).rstrip(','), 16)
        tested, set_ = flags_tested(handler)
        if tested or set_:
            lines.append(f's{number:<4} {handler:#x}  tests {sorted(hex(f) for f in tested)}  sets {sorted(hex(f) for f in set_)}')
        if 0x200 in tested:
            fast.append(number)
    with open(os.path.join(HERE, 'out', 'fast_checks.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(lines) + '\n')
    print(f'{len(fast)} functions do something else while an event is skipped:')
    print(' '.join(f's{number}' for number in fast))


if __name__ == '__main__':
    main()
