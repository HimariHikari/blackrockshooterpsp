"""Dumps the tables of the field's resident data (SYSTEM.VOL/fld_resident.bin, slot 0):
an SSCR-format file whose symbols are data tables (`lst_map`, `lst_fchr`, ...).

Usage: fld_resident.py                  the symbols, with the size of each
       fld_resident.py <symbol> <record length> [struct format]
                                        its records; a field a relocation points at is
                                        shown as the string it points to
       fld_resident.py raw <symbol> [from] [length]
                                        its words one by one, with what each pointer
                                        among them points to

The record length of a list is found by looking at where its relocations fall.
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join(HERE, '..', 'game', 'unpacked', 'SYSTEM.VOL', 'fld_resident.bin', '00.sscr')


class Resident:
    def __init__(self, d):
        assert d[:4] == b'SSCR'
        self.d = d
        nsym, nrel = struct.unpack_from('<HH', d, 8)
        sym_at, rel_at, self.data_at = struct.unpack_from('<III', d, 0x0c)
        self.symbols = []
        for i in range(nsym):
            address, name_at = struct.unpack_from('<II', d, sym_at + i * 8)
            self.symbols.append((d[name_at:d.index(b'\0', name_at)].decode('latin1'), address))
        self.relocations = dict(struct.unpack_from('<II', d, rel_at + i * 8) for i in range(nrel))
        starts = sorted(address for _, address in self.symbols) + [min(self.relocations.values(), default=len(d))]
        self.sizes = {name: starts[starts.index(address) + 1] - address for name, address in self.symbols}

    def string(self, at):
        return self.d[at:self.d.index(b'\0', at)].decode('utf-8', 'replace')

    def table(self, name):
        address = dict(self.symbols)[name]
        return address, self.sizes[name]


def main():
    r = Resident(open(PATH, 'rb').read())
    if len(sys.argv) == 1:
        for name, address in r.symbols:
            size = r.sizes[name]
            inside = sorted(location - address for location in r.relocations if address <= location < address + size)
            gaps = sorted({b - a for a, b in zip(inside, inside[1:])})
            print(f'{name:12s} at {address:#7x}  {size:6d} bytes  {len(inside):4d} pointers'
                  + (f', first at +{inside[0]:#x}, spaced {gaps[:4]}' if inside else ''))
        return
    if sys.argv[1] == 'raw':
        address, size = r.table(sys.argv[2])
        start = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0
        length = int(sys.argv[4], 0) if len(sys.argv) > 4 else 0x100
        for i in range(start, min(size, start + length), 4):
            at = address + i
            word, = struct.unpack_from('<I', r.d, at)
            signed, = struct.unpack_from('<i', r.d, at)
            low, high = struct.unpack_from('<hh', r.d, at)
            note = f'  -> {r.string(r.relocations[at])!r}'[:90] if at in r.relocations else ''
            print(f'+{i:#06x}  {word:#010x} {signed:11d}  ({low:6d},{high:6d}){note}')
        return
    name, length = sys.argv[1], int(sys.argv[2], 0)
    fmt = sys.argv[3] if len(sys.argv) > 3 else None
    address, size = r.table(name)
    for index in range(size // length):
        at = address + index * length
        record = r.d[at:at + length]
        if fmt:
            values = list(struct.unpack_from(fmt, record))
        else:
            values = list(struct.unpack_from(f'<{length // 2}h', record))
        strings = {}
        for field in range(0, length, 4):
            if at + field in r.relocations:
                strings[field] = r.string(r.relocations[at + field])
        text = ' '.join(f'+{field:#x}={text!r}' for field, text in strings.items())
        print(f'[{index:3d}] {values} {text}')


main()
