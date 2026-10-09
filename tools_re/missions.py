"""Lists the game's missions (`lst_mission` of the field's resident data, 120 records
of 0x74 bytes): each one's title, the map and entrance it starts on, the missions
that must be cleared for it to open and the flag that must be set, its story event,
and the lines its display can show.

Usage: missions.py [-v]      -v: the briefing, objective and hint as well
"""
import contextlib, io, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
with contextlib.redirect_stdout(io.StringIO()):
    saved, sys.argv = sys.argv, ['fld_resident.py', 'raw', 'dat_tbl', '0', '0']
    import fld_resident
    sys.argv = saved

RECORD = 0x74


def main():
    verbose = '-v' in sys.argv[1:]
    r = fld_resident.Resident(open(fld_resident.PATH, 'rb').read())
    address, size = r.table('lst_mission')

    def text(at):
        to = r.relocations.get(at)
        return r.string(to) if to else None

    for number in range(size // RECORD):
        at = address + number * RECORD
        title = text(at + 4)
        if not title:
            continue
        first, = struct.unpack_from('<i', r.d, at)
        event, = struct.unpack_from('<i', r.d, at + 0x18)
        # The list ends at the first -1 (0x2538 of the system script).
        needs = list(struct.unpack_from('<8b', r.d, at + 0x1c))
        needs = needs[:needs.index(-1)] if -1 in needs else needs
        flag, = struct.unpack_from('<i', r.d, at + 0x28)
        start_map, entrance = struct.unpack_from('<hh', r.d, at + 0x2c)
        lines = [text(at + 0x30 + 4 * i) for i in range((RECORD - 0x30) // 4)]
        lines = [line for line in lines if line]
        print(f'{number:3} {title!r}  map {start_map} entrance {entrance}  needs {needs} flag {flag}'
              f'  event {event}  word0 {first:#x}')
        if verbose:
            for name, offset in (('briefing', 8), ('objective', 0xc), ('hint', 0x10)):
                print(f'      {name}: {text(at + offset)!r}')
        for index, line in enumerate(lines):
            print(f'      line {index}: {line!r}')


if __name__ == '__main__':
    main()
