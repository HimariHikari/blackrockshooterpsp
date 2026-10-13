"""Every scene's motions set against the models they are played on: which nodes of
an animation find a node of the model the way the port (and the original, 0x21454)
looks for them, and which do not though they have keys that move.

Usage: motion_match.py            -> a summary and tools_re/out/motion_match.txt
       motion_match.py <event>    -> every node of that event's motions
"""
import contextlib, io, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
with contextlib.redirect_stdout(io.StringIO()):
    saved, sys.argv = sys.argv, ['fld_resident.py', 'raw', 'dat_tbl', '0', '0']
    import fevt_cast
    sys.argv = saved

NO_ID = 0xff
NONE = 0xff


def model_nodes(path):
    """A model's nodes: (id, has child, the first node after it and all under it)."""
    d = open(path, 'rb').read()
    count = d[8]
    # The node table is the third of the header's tables, 0x20 bytes a node:
    # flags, id, parent, the node after it and all under it.
    table, = struct.unpack_from('<I', d, 0x18)
    nodes = [d[table + i * 0x20:table + i * 0x20 + 4] for i in range(count)]
    return [(n[1], bool(n[0] & 2), None if n[3] == NONE else n[3]) for n in nodes]


def animation_nodes(path):
    """An animation's nodes: (id, driven, has child, next, moves)."""
    d = open(path, 'rb').read()
    count, = struct.unpack_from('<H', d, 8)
    nodes_at, = struct.unpack_from('<I', d, 0x14)
    found = []
    for n in range(count):
        at = nodes_at + n * 0x24
        flags, tracks, ident, sibling = d[at:at + 4]
        tracks_at, = struct.unpack_from('<I', d, at + 4)
        moves = False
        for t in range(tracks):
            r = tracks_at + t * 0x18
            kind, mode = d[r], d[r + 1]
            if kind == 3:
                continue
            if kind == 4:
                keys, = struct.unpack_from('<H', d, r + 4)
                at_keys, = struct.unpack_from('<I', d, r + 8)
                values = {struct.unpack_from('<4f', d, at_keys + i * 20 + 4) for i in range(keys)}
                moves |= len(values) > 1
                continue
            counts = struct.unpack_from('<3H', d, r + 4)
            for axis in range(1 if mode & 2 else 3):
                if counts[axis] > 1:
                    offset, = struct.unpack_from('<I', d, r + 12 + axis * 4)
                    values = [struct.unpack_from('<f', d, offset + i * 8 + 4)[0] for i in range(counts[axis])]
                    moves |= max(values) - min(values) > 1e-4
        found.append((ident, not flags & 1, bool(flags & 2), None if sibling == NONE else sibling, moves))
    return found


def match(model, animation):
    """The animation's node for each of the model's (`match_nodes` in the engine)."""
    count = len(model)
    if len(animation) == count:
        return list(range(count)), 'by place'
    matched = [None] * count
    if not animation:
        return matched, 'empty'
    first = animation[0][0]
    at = 0 if first == NO_ID else next((i for i, node in enumerate(model) if node[0] == first), 0)
    node = 0
    while node < len(animation) and at < count:
        ident, _, has_child, sibling, _ = animation[node]
        if ident == NO_ID:
            if model[at][0] != NO_ID:
                if sibling is None:
                    break
                node = sibling
                continue
        else:
            gave_up = False
            while model[at][0] != ident:
                if model[at][2] is None:
                    gave_up = True
                    break
                at = model[at][2]
            if gave_up:
                return matched, f'gave up looking for id {ident} at animation node {node}'
        matched[at] = node
        at = model[at][2] if (model[at][2] is not None and not has_child and model[at][1]) else at + 1
        node += 1
    return matched, 'walked'


def main():
    events = fevt_cast.field_names('lst_fevt', 17)
    characters = fevt_cast.field_names('lst_fchr', 16)
    only = [a.upper() for a in sys.argv[1:]]
    lines, lost_files, files, lost_nodes = [], 0, 0, 0
    by_character = {}
    for name in events:
        bundle = os.path.join(fevt_cast.EVENTS, name.upper() + '.BIN')
        if only and name.upper() not in only and not any(name.upper().endswith(o) for o in only):
            continue
        script_path = os.path.join(bundle, '00.sscr')
        if not os.path.exists(script_path):
            continue
        script = open(script_path, 'rb').read()
        table = fevt_cast.symbols(script)
        if 'lst_fchr' not in table:
            continue
        cast = script[table['lst_fchr']:]
        ends = sorted(a for a in table.values() if a > table['lst_fchr'])
        cast = cast[:(ends[0] - table['lst_fchr']) if ends else 16]
        for slot, number in enumerate(cast):
            if number == NO_ID or number >= len(characters):
                continue
            who = characters[number]
            model_path = os.path.join(fevt_cast.CHARACTERS, f'FCHR_{who.upper()}.BIN', '01.bin', '00.insm')
            folder = os.path.join(bundle, '01.bin', f'{slot:02}.bin')
            if not os.path.exists(model_path) or not os.path.isdir(folder):
                continue
            model = model_nodes(model_path)
            for motion in sorted(os.listdir(folder)):
                if not motion.endswith('.insa'):
                    continue
                animation = animation_nodes(os.path.join(folder, motion))
                matched, how = match(model, animation)
                used = {node for node in matched if node is not None}
                lost = [n for n, node in enumerate(animation) if node[4] and node[1] and n not in used]
                files += 1
                if lost or only:
                    ids = [f'{n}(id {animation[n][0]})' for n in lost]
                    lines.append(f'{name} slot {slot} {who} motion {motion}: model {len(model)} nodes, animation {len(animation)}, {how}; moving nodes with no model node: {" ".join(ids) or "none"}')
                if lost:
                    lost_files += 1
                    lost_nodes += len(lost)
                    by_character.setdefault(who, [0, set()])
                    by_character[who][0] += 1
                    by_character[who][1].update(animation[n][0] for n in lost)
                if only:
                    for at, node in enumerate(matched):
                        if node is not None:
                            a = animation[node]
                            lines.append(f'    model {at:3} id {model[at][0]:3} <- animation {node:3} id {a[0]:3}{" moves" if a[4] else ""}')
    out = os.path.join(HERE, 'out', 'motion_match.txt')
    open(out, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
    print(f'{files} motions; {lost_files} have moving nodes that find no node of the model ({lost_nodes} nodes); wrote {out}')
    for who, (count, ids) in sorted(by_character.items(), key=lambda item: -item[1][0]):
        print(f'  {who}: {count} motions, ids {sorted(ids)}')
    if only:
        print('\n'.join(lines[:120]))


if __name__ == '__main__':
    main()
