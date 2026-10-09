"""Which of the engine functions the field's events and its system script call (the
table at 0x1726D0) the port has, and which it still lacks: reads the port's own
source (`field/system.rs`, the constants of its `mod native`) for what is ported, so
it stays in step with it, and every script for what is called.

Usage: fevt_coverage.py     -> a summary; the events that call a function are listed
                               by `fevt_survey.py <native>`
"""
import collections, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import sscr

EVENTS = os.path.join(HERE, '..', 'game', 'unpacked', 'FLD', 'FEVT')
SYSTEM = os.path.join(HERE, '..', 'game', 'unpacked', 'SYSTEM.VOL', 'fld_pack.bin', '08.bin', '00.sscr')
SOURCE = os.path.join(HERE, '..', 'crates', 'brs-game', 'src', 'field', 'system.rs')


def ported():
    """The numbers the port's `mod native` names."""
    text = open(SOURCE, encoding='utf-8').read()
    body = text[text.index('mod native {'):]
    body = body[:body.index('\n}\n')]
    return {int(number, 16): name for name, number in re.findall(r'pub const (\w+): u32 = 0x([0-9a-fA-F]+);', body)}


def called(path):
    """The engine functions a script calls: `li 0xNN01, rv` and then `callr rv`."""
    script = sscr.Script(open(path, 'rb').read())
    d, at, previous, used = script.d, script.code_at, None, []
    while at < script.code_end:
        ins = sscr.decode(d, at)
        if ins is None:
            at += 1
            previous = None
            continue
        name, args, length = ins
        if name == 'callr' and previous is not None:
            used.append(previous)
        previous = None
        if name == 'li' and args[1][1] == 3 and args[0][1] & 0xff == 1 and args[0][1] < 0x10000:
            previous = args[0][1] >> 8
        at += length
    return used


def main():
    have = ported()
    calls, scripts, where = collections.Counter(), collections.Counter(), {}
    paths = [('the system script', SYSTEM)]
    for bundle in sorted(os.listdir(EVENTS)):
        path = os.path.join(EVENTS, bundle, '08.bin', '00.sscr')
        if os.path.exists(path):
            paths.append((bundle, path))
    for name, path in paths:
        used = called(path)
        for number in used:
            calls[number] += 1
        for number in set(used):
            scripts[number] += 1
            where.setdefault(number, name)
    lacking = sorted(number for number in calls if number not in have)
    done = sum(calls[number] for number in calls if number in have)
    print(f'{len(paths)} scripts call {len(calls)} functions; {len(calls) - len(lacking)} of them are ported, '
          f'which is {done} of {sum(calls.values())} calls')
    print('not ported: number, scripts that call it, calls, the first of them')
    for number in lacking:
        print(f'  {number:#04x}  {scripts[number]:4}  {calls[number]:6}  {where[number]}')


if __name__ == '__main__':
    main()
