"""Which enemy groups have a given enemy: reads tools_re/out/enmgrp2.txt (see egtb.py).

Usage: groups_with.py [--all] <enemy number> [...]
       the first few groups each is a member of, or with --all every one
"""
import os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
lines = open(os.path.join(HERE, 'out', 'enmgrp2.txt'), encoding='utf-8').read().splitlines()

groups = []   # (id, mode, [(enemy, variant)], waves)
for line in lines:
    m = re.match(r'group\s+(\d+): flags \S+ kind (\d+) .* waves (\d+)', line)
    if m:
        groups.append((int(m.group(1)), int(m.group(2)), [], int(m.group(3))))
        continue
    m = re.match(r'\s+member: enemy\s+(\d+) b2 \d+ variant\s+(\d+)', line)
    if m and groups:
        groups[-1][2].append((int(m.group(1)), int(m.group(2))))

shown = None if '--all' in sys.argv else 6
for wanted in (int(a) for a in sys.argv[1:] if a != '--all'):
    found = [g for g in groups if any(enemy == wanted for enemy, _ in g[2])]
    print(f'enemy {wanted}: {len(found)} groups')
    for ident, mode, members, waves in found[:shown]:
        text = ' '.join(f'{enemy}/{variant}' for enemy, variant in members)
        print(f'   group {ident:4d} (rule {mode}, {waves} kept back): {text}')
