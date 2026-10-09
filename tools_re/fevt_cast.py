"""Who is in an event's scene: the character each of its slots holds (the event's own
`lst_fchr`, a byte for each slot naming one of the field's characters), the model
files of that character, and the animations and camera cuts the event brings.

Usage: fevt_cast.py <event number or name> ...
"""
import contextlib, io, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
with contextlib.redirect_stdout(io.StringIO()):
    saved, sys.argv = sys.argv, ['fld_resident.py', 'raw', 'dat_tbl', '0', '0']
    import fld_resident
    sys.argv = saved

UNPACKED = os.path.join(HERE, '..', 'game', 'unpacked')
EVENTS = os.path.join(UNPACKED, 'FLD', 'FEVT')
CHARACTERS = os.path.join(UNPACKED, 'FLD', 'FCHR')
NOBODY = 0xff


def field_names(table, record):
    r = fld_resident.Resident(open(fld_resident.PATH, 'rb').read())
    address, size = r.table(table)
    return [r.d[address + n * record:address + (n + 1) * record].split(b'\0')[0].decode('latin1')
            for n in range(size // record)]


def symbols(d):
    """The symbols of an SSCR file: name -> where in the file."""
    count, = struct.unpack_from('<H', d, 8)
    table, = struct.unpack_from('<I', d, 0x0c)
    found = {}
    for n in range(count):
        address, name = struct.unpack_from('<II', d, table + n * 8)
        found[d[name:d.index(b'\0', name)].decode('latin1')] = address
    return found


def listing(folder):
    if not os.path.isdir(folder):
        return []
    return sorted(os.listdir(folder))


def main():
    events = field_names('lst_fevt', 17)
    characters = field_names('lst_fchr', 16)
    for wanted in sys.argv[1:]:
        name = events[int(wanted)] if wanted.isdigit() else wanted
        bundle = os.path.join(EVENTS, name.upper() + '.BIN')
        print(f'{name} (event {events.index(name) if name in events else "?"})')
        script = open(os.path.join(bundle, '00.sscr'), 'rb').read()
        table = symbols(script)
        cast = script[table['lst_fchr']:]
        ends = sorted(a for a in table.values() if a > table['lst_fchr'])
        cast = cast[:(ends[0] - table['lst_fchr']) if ends else 16]
        for slot, number in enumerate(cast):
            if number == NOBODY:
                continue
            who = characters[number] if number < len(characters) else '?'
            models = listing(os.path.join(CHARACTERS, f'FCHR_{who.upper()}.BIN', '01.bin'))
            motions = listing(os.path.join(bundle, '01.bin', f'{slot:02}.bin'))
            print(f'  slot {slot}: character {number} {who}; models {" ".join(models) or "none"}')
            if motions:
                print(f'      motions {" ".join(m.split(".")[0] for m in motions)}')
        cuts = listing(os.path.join(bundle, '03.bin'))
        print(f'  camera cuts: {" ".join(c.split(".")[0] for c in cuts) or "none"}')
        for slot, what in ((4, 'effects'), (5, 'recordings'), (7, 'faces')):
            found = listing(os.path.join(bundle, f'{slot:02}.bin'))
            if found:
                print(f'  {what}: {len(found)}')


if __name__ == '__main__':
    main()
