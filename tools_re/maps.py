"""Lists the game's field maps (`lst_map` of the field's resident data): each one's
number, name, battle map, whether its bundle is unpacked and has a script, and the
name of the place.

Usage: maps.py
"""
import contextlib, io, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
with contextlib.redirect_stdout(io.StringIO()):
    saved, sys.argv = sys.argv, ['fld_resident.py', 'raw', 'dat_tbl', '0', '0']
    import fld_resident
    sys.argv = saved

RECORD = 24


def main():
    r = fld_resident.Resident(open(fld_resident.PATH, 'rb').read())
    address, size = r.table('lst_map')
    root = os.path.join(HERE, '..', 'game', 'unpacked', 'FLD', 'MAP')
    have = {name.upper() for name in os.listdir(root)}
    for number in range(size // RECORD):
        at = address + number * RECORD
        record = r.d[at:at + RECORD]
        name = record[:12].split(b'\0')[0].decode('latin1')
        first, battle, second, third = struct.unpack_from('<HhHH', record, 12)
        place_at = r.relocations.get(at + 0x14)
        place = r.string(place_at) if place_at else ''
        bundle = f'MAP_{name.upper()}.BIN'
        script = os.path.exists(os.path.join(root, bundle, '10.bscr'))
        marks = ('bundle' if bundle in have else '------') + (' script' if script else '       ')
        print(f'{number:3} {name:12} battle {battle:4} words {first:5} {second:5} {third:5}  {marks}  {place}')


if __name__ == '__main__':
    main()
