"""Surveys the scripts of the field's events (`FLD/FEVT/*/08.bin/00.sscr`): which entry
each has, and which engine functions of the field's table (0x1726D0) they call, with
how many events call each and how often.

Usage: fevt_survey.py            -> tools_re/out/fevt_survey.txt and a summary
       fevt_survey.py <native>   the events that call that function (number in hex)
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import sscr

ROOT = os.path.join(HERE, '..', 'game', 'unpacked', 'FLD', 'FEVT')
ENTRIES = ('script_entry', 'async_event', 'comm_event')


def natives(script):
    """The engine functions a script calls: `li 0xNN01, rv` and then `callr rv`."""
    d, found = script.d, []
    for start in sorted(value for value, _ in script.symbols if script.code_at <= value < script.code_end):
        at = start
        previous = None
        while at < script.code_end:
            ins = sscr.decode(d, at)
            if ins is None:
                at += 1
                previous = None
                continue
            name, args, length = ins
            if name == 'callr' and previous is not None:
                found.append(previous)
            previous = None
            if name == 'li' and args[1][1] == 3 and args[0][1] & 0xff == 1 and args[0][1] < 0x10000:
                previous = args[0][1] >> 8
            if name == 'ret':
                break
            at += length
    return found


def main():
    wanted = int(sys.argv[1], 16) if len(sys.argv) > 1 else None
    entries = collections.Counter()
    calls, events = collections.Counter(), collections.Counter()
    lines = []
    for bundle in sorted(os.listdir(ROOT)):
        path = os.path.join(ROOT, bundle, '08.bin', '00.sscr')
        if not os.path.exists(path):
            entries['no script'] += 1
            continue
        script = sscr.Script(open(path, 'rb').read())
        names = [name for _, name in script.symbols]
        entry = next((name for name in ENTRIES if name in names), 'none')
        entries[entry] += 1
        used = []
        # Every function of the script counts, not only the one that is started.
        d = script.d
        at = script.code_at
        previous = None
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
        for number in used:
            calls[number] += 1
        for number in set(used):
            events[number] += 1
        if wanted is not None and wanted in used:
            print(bundle, entry)
        lines.append(f'{bundle:24} {entry:12} {len(script.symbols):3} functions  '
                     + ' '.join(f'{number:x}' for number in sorted(set(used))))
    if wanted is not None:
        return
    summary = [f'entries: {dict(entries)}', 'engine functions: number, events that call it, calls']
    summary += [f'  {number:#04x}  {events[number]:4}  {calls[number]:6}' for number in sorted(calls)]
    with open(os.path.join(HERE, 'out', 'fevt_survey.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(summary + [''] + lines) + '\n')
    print('\n'.join(summary))


if __name__ == '__main__':
    main()
