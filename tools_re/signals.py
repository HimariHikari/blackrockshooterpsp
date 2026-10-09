"""Which signals each enemy's script answers, and what it does on one of them.

Usage: signals.py            -> for every signal number, the scripts that test for it
       signals.py <number>   -> what each script does when that signal comes

A signal is what the engine sends a script with 0xAE0D8: the script's `signalEnemyAi`
is run with (number, 0, 0) and compares its first argument with the numbers it knows.
"""
import glob, os, re, sys

import bscr
from bscr import Script, ROOT
from bscr_dec import decompile

TEST = re.compile(r'if not \(\((\d+) == l0\)\) goto (L[0-9a-f]+)')


def scripts():
    for path in sorted(glob.glob(os.path.join(ROOT, 'BTL', 'CHR', 'BCHR_*.VOL', '*.bms'))):
        yield os.path.basename(path)[:-4], os.path.relpath(path, ROOT)


def branches(lines):
    """The statements under each `number == l0` test of a signal entry."""
    found = {}
    for index, line in enumerate(lines):
        match = TEST.search(line)
        if not match:
            continue
        number, label = int(match.group(1)), match.group(2)
        body = []
        for later in lines[index + 1:]:
            if later.strip() == label + ':':
                break
            body.append(later.strip())
        found.setdefault(number, []).extend(body)
    return found


if __name__ == '__main__':
    wanted = int(sys.argv[1]) if len(sys.argv) > 1 else None
    by_signal = {}
    for name, rel in scripts():
        script = Script(bscr.read(rel))
        found = branches(decompile(script, ('signalEnemyAi',)))
        for number, body in found.items():
            by_signal.setdefault(number, []).append((name, body))
    if wanted is None:
        for number in sorted(by_signal):
            names = [name for name, _ in by_signal[number]]
            print(f'{number:5}: {len(names):3} scripts  {" ".join(names[:12])}{" ..." if len(names) > 12 else ""}')
    else:
        same = {}
        for name, body in by_signal.get(wanted, []):
            same.setdefault('\n'.join(body), []).append(name)
        for body, names in sorted(same.items(), key=lambda item: -len(item[1])):
            print(f'--- {len(names)} scripts: {" ".join(names)}')
            print(body if body else '(nothing)')
