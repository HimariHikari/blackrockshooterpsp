"""The field's events (`lst_fevt` of the field's resident data: 710 names of 17 bytes
each): what a map's script plays with `s237(number, argument)`. Each is a bundle
under `FLD/FEVT` with an `SSCR` script in it.

Usage: fevt.py                 every event: number, name, whether its bundle is there
       fevt.py <number> ...    what is in those events' bundles
"""
import contextlib, io, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
with contextlib.redirect_stdout(io.StringIO()):
    saved, sys.argv = sys.argv, ['fld_resident.py', 'raw', 'dat_tbl', '0', '0']
    import fld_resident
    sys.argv = saved

RECORD = 17
ROOT = os.path.join(HERE, '..', 'game', 'unpacked', 'FLD', 'FEVT')


def names():
    r = fld_resident.Resident(open(fld_resident.PATH, 'rb').read())
    address, size = r.table('lst_fevt')
    for number in range(size // RECORD):
        record = r.d[address + number * RECORD:address + (number + 1) * RECORD]
        yield number, record.split(b'\0')[0].decode('latin1')


def main():
    have = {name.upper() for name in os.listdir(ROOT)}
    wanted = [int(n, 0) for n in sys.argv[1:]]
    for number, name in names():
        bundle = f'{name.upper()}.BIN'
        if not wanted:
            print(f'{number:4} {name:18} {"bundle" if bundle in have else "------"}')
        elif number in wanted:
            print(f'{number} {name}')
            folder = os.path.join(ROOT, bundle)
            for path, _, files in os.walk(folder):
                for file in sorted(files):
                    full = os.path.join(path, file)
                    print(f'    {os.path.relpath(full, folder):40} {os.path.getsize(full):8}')


if __name__ == '__main__':
    main()
