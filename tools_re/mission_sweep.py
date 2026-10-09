"""Starts every mission in turn, unseen, lets each run for a while with its events
played and its messages passed, and says what each run's log has of warnings, errors
and things not ported.

usage: python mission_sweep.py [seconds each] [first mission] [last mission]

Each run is the game with `--mission N --play-events --skip-messages --skip-battles`,
no window and no sound; its log is `out/sweep/mission_NN.log` and a picture of its last
moment `out/sweep/mission_NN.png`. The summary is printed and written to
`out/sweep/summary.txt`. Build the game first (`cargo build -p brs-game`): the runs
are of a copy of it (`target/debug/brs-sweep.exe`), so it can be built again while
they go. The numbers the game has no mission of are passed by.
"""
import collections
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
BUILT = os.path.join(ROOT, 'target', 'debug', 'brs-game.exe')
GAME = os.path.join(ROOT, 'target', 'debug', 'brs-sweep.exe')
OUT = os.path.join(HERE, 'out', 'sweep')
NO_WINDOW = 0x08000000
MISSIONS = 113
STAMP = re.compile(r'^\S+\s+')


def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 45.0
    first = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    last = int(sys.argv[3]) if len(sys.argv) > 3 else MISSIONS - 1
    os.makedirs(OUT, exist_ok=True)
    shutil.copyfile(BUILT, GAME)
    summary = []
    for mission in range(first, last + 1):
        picture = os.path.join(OUT, f'mission_{mission:02}.png')
        run = [GAME, '--mute', '--verbose', '--saves', os.path.join(HERE, 'out', '_saves'), '--mission', str(mission), '--play-events', '--skip-messages',
               '--skip-battles', '--screenshot', picture, '--screenshot-at', f'{seconds:.1f}']
        try:
            done = subprocess.run(run, capture_output=True, text=True, encoding='utf-8', errors='replace', creationflags=NO_WINDOW, timeout=seconds + 120, cwd=ROOT)
            log, code = done.stdout + done.stderr, done.returncode
        except subprocess.TimeoutExpired as late:
            log, code = (late.stdout or b'').decode('utf-8', 'replace') + (late.stderr or b'').decode('utf-8', 'replace'), 'timed out'
        if 'the game has no mission' in log:
            continue
        with open(os.path.join(OUT, f'mission_{mission:02}.log'), 'w', encoding='utf-8') as out:
            out.write(log)
        told = collections.Counter()
        events, films, maps = 0, 0, []
        for line in log.splitlines():
            if ' WARN ' in line or 'ERROR' in line or 'panicked' in line or 'not ported' in line:
                told[STAMP.sub('', line).strip()[:170]] += 1
            events += 'is loaded for its scene' in line or ' is played: ' in line
            films += 'is made ready' in line
            found = re.search(r'map \d+ is (\w+)', line)
            if found:
                maps.append(found.group(1))
        head = f'mission {mission:2}: exit {code}, maps {", ".join(maps) or "none"}, {events} events, {films} films'
        summary.append(head)
        summary.extend(f'    {count:3} x {text}' for text, count in sorted(told.items()))
        print(head, flush=True)
    with open(os.path.join(OUT, 'summary.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(summary) + '\n')
    print('\n'.join(summary))


if __name__ == '__main__':
    main()
