"""Whether animations have as many nodes as the models they are played on. When they
do the original matches nodes by index; when not, it walks both trees and matches by
id (0x215E8), which the port does not do yet.

Looks at the field characters' own animations, the battle characters', and the
motions event scenes bring for the characters in their slots.

Usage: insa_counts.py     -> a summary; the mismatches are listed
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import fevt_cast

UNPACKED = fevt_cast.UNPACKED


def model_nodes(path):
    d = open(path, 'rb').read()
    return d[8] if d[:4] == b'INSM' else None


def animation_nodes(path):
    d = open(path, 'rb').read()
    return struct.unpack_from('<H', d, 8)[0] if d[:4] == b'INSA' else None


def main():
    same, differ = collections.Counter(), []

    def compare(where, model, animation, name):
        if model is None or animation is None:
            return
        if model == animation:
            same[where] += 1
        else:
            differ.append(f'{where}: {name}: model {model} nodes, animation {animation}')

    characters = os.path.join(UNPACKED, 'FLD', 'FCHR')
    for bundle in sorted(os.listdir(characters)):
        model = os.path.join(characters, bundle, '01.bin', '00.insm')
        folder = os.path.join(characters, bundle, '02.bin')
        if not os.path.exists(model) or not os.path.isdir(folder):
            continue
        for name in sorted(os.listdir(folder)):
            compare('field characters', model_nodes(model), animation_nodes(os.path.join(folder, name)), f'{bundle}/{name}')

    battle = os.path.join(UNPACKED, 'BTL', 'CHR')
    for bundle in sorted(os.listdir(battle)):
        folder = os.path.join(battle, bundle)
        if not os.path.isdir(folder):
            continue
        models = [n for n in sorted(os.listdir(folder)) if n.endswith('.mdl')]
        for name in sorted(os.listdir(folder)):
            if not name.endswith('.anm'):
                continue
            # An animation belongs to the model whose name it starts with.
            owner = [m for m in models if name.startswith(m[:-4])]
            if owner:
                compare('battle characters', model_nodes(os.path.join(folder, owner[0])),
                        animation_nodes(os.path.join(folder, name)), f'{bundle}/{name}')

    names = fevt_cast.field_names('lst_fchr', 16)
    events = os.path.join(UNPACKED, 'FLD', 'FEVT')
    for bundle in sorted(os.listdir(events)):
        script = os.path.join(events, bundle, '00.sscr')
        if not os.path.exists(script):
            continue
        d = open(script, 'rb').read()
        table = fevt_cast.symbols(d)
        if 'lst_fchr' not in table:
            continue
        for slot, number in enumerate(d[table['lst_fchr']:table['lst_fchr'] + 16]):
            folder = os.path.join(events, bundle, '01.bin', f'{slot:02}.bin')
            if number == 0xff or number >= len(names) or not os.path.isdir(folder):
                continue
            model = os.path.join(characters, f'FCHR_{names[number].upper()}.BIN', '01.bin', '00.insm')
            if not os.path.exists(model):
                differ.append(f'event scenes: {bundle} slot {slot}: no model for {names[number]}')
                continue
            for name in sorted(os.listdir(folder)):
                compare('event scenes', model_nodes(model), animation_nodes(os.path.join(folder, name)),
                        f'{bundle}/{slot:02}/{name} on {names[number]}')

    for where, count in sorted(same.items()):
        print(f'{where}: {count} animations have as many nodes as their model')
    print(f'{len(differ)} do not:')
    by_kind = collections.Counter(line.split(':')[0] for line in differ)
    for where, count in sorted(by_kind.items()):
        print(f'  {where}: {count}')
    print('\n'.join(differ[:40]))
    with open(os.path.join(HERE, 'out', 'insa_counts.txt'), 'w', encoding='utf-8') as out:
        out.write('\n'.join(differ) + '\n')


if __name__ == '__main__':
    main()
