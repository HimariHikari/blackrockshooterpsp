"""Fights every enemy group of the game in turn, unseen, for a little while each, and
says what each run's log has of warnings, errors and things not ported.

usage: python group_sweep.py [seconds each] [workers] [first group] [last group]

Each run is the game with `--battle <map> --group N --level 50`, her shooting at
the nearest enemy from the fifth second, no window and no sound; its log is
`out/groups/group_NNNN.log` and a picture of its last moment `group_NNNN.png`. The
bosses are fought on their own battle maps (`BATTLE_NOTES.md`, "A boss's battle"),
the rest on the first stage's first. The runs are of a copy of the game
(`target/debug/brs-groups.exe`), so it can be built again while they go; several go
at once, at a low priority. The groups are those of `out/enmgrp2.txt` that have
anyone in them. The summary is printed and written to `out/groups/summary.txt`.
"""
import collections
import concurrent.futures
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
BUILT = os.path.join(ROOT, 'target', 'debug', 'brs-game.exe')
GAME = os.path.join(ROOT, 'target', 'debug', 'brs-groups.exe')
OUT = os.path.join(HERE, 'out', 'groups')
NO_WINDOW = 0x08000000
BELOW_NORMAL = 0x00004000
STAMP = re.compile(r'^\S+\s+')
BOSS_MAPS = {3: 'STG01_02', 4: 'STG02_06', 5: 'STG03_02', 6: 'STG04_02', 7: 'STG05_03', 8: 'STG06_02', 9: 'STG06_03', 10: 'STG06_03'}


def groups():
    found, current = [], None
    for line in open(os.path.join(HERE, 'out', 'enmgrp2.txt'), encoding='utf-8'):
        head = re.match(r'group\s+(\d+):', line)
        if head:
            current = int(head.group(1))
        elif current is not None and re.match(r'\s+member: enemy', line):
            found.append(current)
            current = None
    return found


def fight(group, seconds):
    picture = os.path.join(OUT, f'group_{group:04}.png')
    shots = max(1, int((seconds - 5) / 1.0))
    script = 'wait:5,' + ','.join(['seek+square:0.2,seek:0.8'] * shots)
    run = [GAME, '--battle', BOSS_MAPS.get(group, 'STG01_01'), '--group', str(group), '--level', '50', '--saves', os.path.join(HERE, 'out', '_saves'),
           '--script', script, '--screenshot', picture, '--screenshot-at', f'{seconds:.1f}']
    try:
        done = subprocess.run(run, capture_output=True, text=True, encoding='utf-8', errors='replace', creationflags=NO_WINDOW | BELOW_NORMAL, timeout=seconds + 120, cwd=ROOT)
        log, code = done.stdout + done.stderr, done.returncode
    except subprocess.TimeoutExpired as late:
        log, code = (late.stdout or b'').decode('utf-8', 'replace') + (late.stderr or b'').decode('utf-8', 'replace'), 'timed out'
    with open(os.path.join(OUT, f'group_{group:04}.log'), 'w', encoding='utf-8') as out:
        out.write(log)
    told = collections.Counter()
    for line in log.splitlines():
        if ' WARN ' in line or 'ERROR' in line or 'panicked' in line or 'not ported' in line:
            told[re.sub(r'\d+(\.\d+)?', 'N', STAMP.sub('', line).strip())[:170]] += 1
    defeated = log.count(': 0 HP left')
    won = 'the battle is won' in log
    return group, code, told, defeated, won


def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 15.0
    workers = int(sys.argv[2]) if len(sys.argv) > 2 else 3
    first = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    last = int(sys.argv[4]) if len(sys.argv) > 4 else 99999
    os.makedirs(OUT, exist_ok=True)
    shutil.copyfile(BUILT, GAME)
    wanted = [group for group in groups() if first <= group <= last]
    summary, everywhere = [], collections.Counter()
    with concurrent.futures.ThreadPoolExecutor(workers) as pool:
        for group, code, told, defeated, won in pool.map(lambda group: fight(group, seconds), wanted):
            head = f'group {group:4}: exit {code}, {defeated} beaten' + (', won' if won else '')
            summary.append(head)
            summary.extend(f'    {count:3} x {text}' for text, count in sorted(told.items()))
            everywhere.update(told.keys())
            print(head + (f', {len(told)} kinds of warning' if told else ''), flush=True)
    summary.append(f'\n{len(wanted)} groups; warnings by how many groups had them:')
    summary.extend(f'  {count:4} x {text}' for text, count in everywhere.most_common())
    with open(os.path.join(OUT, 'summary.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(summary) + '\n')
    print('\n'.join(summary[-len(everywhere) - 1:]))


if __name__ == '__main__':
    main()
