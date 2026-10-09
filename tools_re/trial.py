"""Runs a battle of the port unattended and sums up what its log says: the states each
enemy's script went through, the moves it made, the bullets fired, the hits either
way, and every warning or error. For checking an enemy family after porting it.

Usage: trial.py <group> [--level N] [--script STEPS] [--seconds N] [--shot PNG] [--stage NAME]

Without a script the player stands idle for `--seconds` (30 if left out). The log is
kept in tools_re/out/trial_<group>.txt.
"""
import collections, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..')


def option(name, default):
    if name in sys.argv:
        return sys.argv[sys.argv.index(name) + 1]
    return default


def main():
    group = sys.argv[1]
    level, seconds = option('--level', '1'), option('--seconds', '30')
    script = option('--script', f'wait:{seconds}')
    shot = option('--shot', os.path.join('game', 'screenshots', f'_trial_{group}.png'))
    stage = option('--stage', 'STG01_01')
    command = ['cargo', 'run', '-q', '-p', 'brs-game', '--', '--battle', stage, '--group', group, '--level', level,
               '--verbose', '--script', script, '--screenshot', shot]
    done = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding='utf-8', errors='replace')
    log = done.stderr
    path = os.path.join(HERE, 'out', f'trial_{group}.txt')
    open(path, 'w', encoding='utf-8').write(log)

    states = collections.defaultdict(list)
    moves = collections.defaultdict(collections.Counter)
    for number, step, state, move in re.findall(r'enemy (\d+) step (\d+): state (-?\d+) \(-?\d+\) wait \S+ move (-?\d+)', log):
        if not states[number] or states[number][-1] != state:
            states[number].append(state)
        moves[number][move] += 1
    print(f'group {group}: exit {done.returncode}, {len(log.splitlines())} log lines -> {os.path.relpath(path, ROOT)}')
    for line in re.findall(r"the player's figures: (.*)", log):
        print('  player:', line)
    for number in sorted(states, key=int):
        seen = collections.Counter(states[number])
        order = ' '.join(states[number][:40]) + (' ...' if len(states[number]) > 40 else '')
        print(f'  enemy {number}: states {dict(seen)}')
        print(f'      in order: {order}')
        print(f'      moves: {dict(moves[number])}')
    shots = collections.Counter(re.findall(r'fires bullet (\d+ of class \d+)', log))
    if shots:
        print('  bullets fired:', dict(shots))
    hits = collections.Counter(re.findall(r'(?:enemy \d+|bullet \d+ of enemy \d+) hits the player with attack (\d+)', log))
    if hits:
        damage = [int(d) for d in re.findall(r'hits the player.*?damage: (\d+)', log)]
        print(f'  hits on the player by attack: {dict(hits)}; damage {min(damage)}-{max(damage)}, {sum(damage)} in all')
    struck = collections.Counter(re.findall(r'shot hits enemy (\d+)', log))
    if struck:
        print('  shots that hit, by enemy:', dict(struck))
    # Status effects: those an attack carried, and those the scripts handed out.
    carried = collections.Counter(re.findall(r'hits the player.*?effect: Some\(\((\d+), (\w+(?:\(\d+\))?)', log))
    if carried:
        print('  effects carried by hits on the player:', {f'{effect} {outcome}': n for (effect, outcome), n in carried.items()})
    drained = [int(d) for d in re.findall(r'hits the player.*?drained: (\d+)', log) if int(d) > 0]
    if drained:
        print(f'  HP drained from the player: {len(drained)} times, {sum(drained)} in all')
    handed = collections.Counter(re.findall(r'INFO \S+ (enemy \d+ (?:takes|gives the player|gives enemy \d+) status effect \d+: \w+(?:\(\d+\))?)', log))
    for line, count in handed.items():
        print(f'  {count} x {line}')
    for kind in ('calls in', 'is gone'):
        for line in re.findall(rf'INFO \S+ (.*{kind}.*)', log):
            print('  ' + line[:150])
    trouble = collections.Counter(line.split(': ', 1)[-1][:170] for line in log.splitlines() if re.search(r' (WARN|ERROR) |panicked', line))
    for line, count in trouble.items():
        print(f'  !! {count} x {line}')
    if not os.path.exists(os.path.join(ROOT, shot)) or os.path.getsize(os.path.join(ROOT, shot)) < 20000:
        print('  (the screenshot did not come out)')


main()
