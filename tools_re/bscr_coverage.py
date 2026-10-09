"""What each enemy's battle script needs that the port does not have yet: engine
functions, kinds of move, classes of bullet. Reads the port's own source for what is
ported, so it stays in step with it.

Usage: bscr_coverage.py            -> tools_re/out/bscr_coverage.txt and a summary
"""
import collections, os, re, struct, sys

import bscr
from bscr_dec import decompile

HERE = os.path.dirname(os.path.abspath(__file__))
GAME = os.path.join(HERE, '..', 'crates', 'brs-game', 'src', 'battle')


def ported_functions():
    """Table-2 numbers that the enemy actor's script host answers."""
    text = open(os.path.join(GAME, 'enemy', 'actor.rs'), encoding='utf-8').read()
    host = text[text.index('impl Host for ScriptHost'):]
    numbers = set()
    # The arms of its match: `(7, [slot, ...])`, `(1 | 2, [text]) | (3, [text, _])`, `(5 | 51, _)`.
    for match in re.finditer(r'\((\d+(?: \| \d+)*), (?:\[|_\))', host):
        numbers.update(int(n) for n in match.group(1).split(' | '))
    return numbers


def ported_moves():
    text = open(os.path.join(GAME, 'enemy', 'mover.rs'), encoding='utf-8').read()
    block = text[text.index('pub mod kind'):text.index('}', text.index('pub mod kind'))]
    values = {name: int(value) for name, value in re.findall(r'pub const (\w+): i32 = (\d+);', block)}
    kinds = set(values.values())
    # A run of kinds is given by its ends: X_FIRST and X_LAST, or X and X_LAST.
    for name, first in values.items():
        if name.endswith('_FIRST'):
            kinds.update(range(first, values[name[:-len('FIRST')] + 'LAST'] + 1))
        elif name + '_LAST' in values:
            kinds.update(range(first, values[name + '_LAST'] + 1))
    return kinds


def bullet_classes():
    """Bullet id -> class, from the bullet table (the first record of each id)."""
    d = open(os.path.join(bscr.ROOT, 'SYSTEM.VOL', 'btl_bullet_tbl.bin'), 'rb').read()
    records_at, size, count = struct.unpack_from('<3I', d, 0x0c)
    classes = {}
    for i in range(count):
        fields = struct.unpack_from('<15i', d, records_at + i * size)
        classes.setdefault(fields[0], fields[14])
    return classes


def used_moves(text):
    """The kinds of move a decompiled script can start. A kind is often worked out
    (`l1 = (l0 + 501)`, `(813 + g13)`, a global set elsewhere), so every kind near
    such a base that the engine's switch has a case for is counted."""
    from move_kinds import has_code

    def family(base):
        return {kind for kind in range(base, base + 10) if has_code(kind)}

    kinds = set()
    bodies = re.split(r'\n(?=\S+\(.*\)\s+# type)', text)
    for body in bodies:
        for match in re.finditer(r'e30_move\([^,]+, ([^,]+),', body):
            wanted = match.group(1).strip()
            if wanted.isdigit():
                kinds.add(int(wanted))
                continue
            summed = re.fullmatch(r'\((\d+) \+ \w+\)', wanted)
            if summed:
                kinds |= family(int(summed.group(1)))
                continue
            scope = text if wanted.startswith('g') else body
            for assigned in re.finditer(rf'\b{re.escape(wanted)} = (\d+)\b', scope):
                kinds.add(int(assigned.group(1)))
            for assigned in re.finditer(rf'\b{re.escape(wanted)} = \(\w+ \+ (\d+)\)', scope):
                kinds |= family(int(assigned.group(1)))
    # A kind the switch has no case for ends at once in the original as well.
    return {kind for kind in kinds if has_code(kind)}


def runs_of(numbers):
    """A set of numbers as text, runs of neighbours folded: `10-15 100 102-104`."""
    runs = []
    for number in sorted(numbers):
        if runs and runs[-1][1] == number - 1:
            runs[-1][1] = number
        else:
            runs.append([number, number])
    return ' '.join(str(first) if first == last else f'{first}-{last}' for first, last in runs)


def ported_bullets():
    """The bullet classes the port has: the constants at the top of its bullet module."""
    text = open(os.path.join(GAME, 'projectile.rs'), encoding='utf-8').read()
    return {int(value) for value in re.findall(r'^pub const [A-Z]+: i32 = (\d+);', text, re.M)}


def main():
    functions, moves, classes = ported_functions(), ported_moves(), bullet_classes()
    PORTED_BULLETS = ported_bullets()
    rows = []
    wanted = collections.Counter()
    wanted_moves = collections.Counter()
    wanted_bullets = collections.Counter()
    family_moves = collections.defaultdict(set)
    folder = os.path.join(bscr.ROOT, 'BTL', 'CHR')
    for archive in sorted(os.listdir(folder)):
        for name in sorted(os.listdir(os.path.join(folder, archive))):
            if not name.endswith('.bms'):
                continue
            d = open(os.path.join(folder, archive, name), 'rb').read()
            if len(d) < 0x40:
                continue
            script = bscr.Script(d)
            used = set()
            first_table = set()
            for off, op, kinds, vals in script.instructions():
                if op in (0x87, 0x88):
                    used.add(kinds[0] + (0x100 if op == 0x88 else 0))
                elif op in (0x82, 0x86):
                    first_table.add(kinds[0] + (0x100 if op == 0x86 else 0))
            text = '\n'.join(decompile(script))
            moves_used = used_moves(text)
            family_moves[re.sub(r'^BCHR_|\d*\.VOL$', '', archive)] |= moves_used
            shots = {int(k) for k in re.findall(r'e19_action\([67], (\d+),', text)}
            # Bullets a script picks by variable are set from numbers just before.
            shots |= {int(k) for k in re.findall(r'\bl\d+ = (\d{2,4})\n', text) if int(k) in classes}
            missing = sorted(used - functions)
            missing_moves = sorted(moves_used - moves)
            missing_bullets = sorted({classes.get(shot, -1) for shot in shots} - PORTED_BULLETS)
            for number in missing:
                wanted[number] += 1
            for kind in missing_moves:
                wanted_moves[kind] += 1
            for kind in missing_bullets:
                wanted_bullets[kind] += 1
            rows.append((len(missing) + len(missing_moves) + len(missing_bullets), archive, name, missing,
                         sorted(first_table), missing_moves, missing_bullets))

    rows.sort()
    lines = [f'# {len(rows)} enemy scripts; ported: {len(functions)} engine functions, {len(moves)} move kinds']
    for total, archive, name, missing, first_table, missing_moves, missing_bullets in rows:
        lines.append(f'{archive:22} needs {total:2}: functions {missing} table-1 {first_table} '
                     f'moves {missing_moves} bullet classes {missing_bullets}')
    lines.append('')
    lines.append('# engine functions by how many scripts need them')
    for number, n in wanted.most_common():
        lines.append(f'e{number:<4} {n}')
    lines.append('# move kinds')
    for kind, n in sorted(wanted_moves.items()):
        lines.append(f'{kind:<5} {n}')
    lines.append('# bullet classes')
    for kind, n in sorted(wanted_bullets.items()):
        lines.append(f'{kind:<5} {n}')
    # Which family of enemies starts which kinds of move, and the kinds the engine
    # has code for that no script starts.
    lines.append('# move kinds each family of scripts can start')
    for family, kinds in sorted(family_moves.items(), key=lambda item: (min(item[1], default=0), item[0])):
        lines.append(f'{family:8} {runs_of(kinds)}')
    from move_kinds import has_code
    started = set().union(*family_moves.values())
    lines.append('# kinds with code that no script starts: ' + runs_of({k for k in range(12000) if has_code(k)} - started))
    path = os.path.join(bscr.OUT, 'bscr_coverage.txt')
    open(path, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
    ready = sum(1 for row in rows if row[0] == 0)
    print(f'{len(rows)} scripts, {ready} need nothing more; wrote {os.path.relpath(path)}')
    print('most wanted functions:', ', '.join(f'e{n} x{c}' for n, c in wanted.most_common(25)))


if __name__ == '__main__':
    main()
