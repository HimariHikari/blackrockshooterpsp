"""Which status effects the enemies deal in: the effect each attack of the attack table
inflicts, and the effects the decompiled scripts hand out themselves (engine functions
67/8 and 120, whose arguments an AI page sets in globals 32 and 33 before the state).

Usage: status_survey.py         (reads tools_re/out/dec_*.txt and the game's tables)
"""
import collections, glob, os, re, struct

import bxcb

HERE = os.path.dirname(os.path.abspath(__file__))


def table(name):
    d = open(os.path.join(bxcb.ROOT, 'SYSTEM.VOL', name), 'rb').read()
    records_at, record_size, count, _ = struct.unpack_from('<4I', d, 0x0c)
    return [struct.unpack_from(f'<{record_size // 4}i', d, records_at + i * record_size) for i in range(count)]


effects = {record[0]: record for record in table('btl_eff_tbl.bin')}

print('# attacks that inflict an effect: effect id (type) -> attack ids')
by_effect = collections.defaultdict(list)
for record in table('btl_atk_tbl.bin'):
    effect = record[8]
    if effect > 0:
        by_effect[effect].append((record[0], record[10], record[11]))
for effect in sorted(by_effect):
    kind = effects.get(effect, (0, '?'))[1]
    attacks = ' '.join(f'{attack}' + (f'/e{enemy}' if enemy else '') for attack, enemy, _ in by_effect[effect][:24])
    print(f'  effect {effect:5} (type {kind:>2}): {len(by_effect[effect]):3} attacks: {attacks}')

print('# effects the scripts hand out: script -> state -> (on whom, effect)')
for path in sorted(glob.glob(os.path.join(HERE, 'out', 'dec_*.txt'))):
    text = open(path, encoding='utf-8').read()
    states = collections.defaultdict(set)
    for m in re.finditer(r'g32 = (-?\d+)\n\s+g33 = (-?\d+)\n\s+change_state_AI\((\d+)\)', text):
        whom, effect, state = int(m.group(1)), int(m.group(2)), int(m.group(3))
        if effect in effects:
            states[state].add((whom, effect))
    calls = sorted(set(re.findall(r'e67_ai\(8, [^)]*\)|e120\([^)]*\)', text)))
    name = os.path.basename(path)[4:-8]
    print(f'  {name}: calls {calls}')
    for state in sorted(states):
        print(f'      state {state}: {sorted(states[state])}')
