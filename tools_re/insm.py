"""INSM model structure dumper + GE display list disassembler. Exploration only.

Usage: insm.py <file> [dl]     (dl = also disassemble display lists)
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

GE = {
    0x00: 'NOP', 0x01: 'VADDR', 0x02: 'IADDR', 0x04: 'PRIM', 0x05: 'BEZIER', 0x06: 'SPLINE', 0x07: 'BBOX',
    0x08: 'JUMP', 0x09: 'BJUMP', 0x0A: 'CALL', 0x0B: 'RET', 0x0C: 'END', 0x0E: 'SIGNAL', 0x0F: 'FINISH',
    0x10: 'BASE', 0x12: 'VTYPE', 0x13: 'OFFSET', 0x14: 'ORIGIN', 0x15: 'REGION1', 0x16: 'REGION2',
    0x17: 'LTE', 0x18: 'LTE0', 0x19: 'LTE1', 0x1A: 'LTE2', 0x1B: 'LTE3', 0x1C: 'CPE', 0x1D: 'BCE', 0x1E: 'TME',
    0x1F: 'FGE', 0x20: 'DTE', 0x21: 'ABE', 0x22: 'ATE', 0x23: 'ZTE', 0x24: 'STE', 0x25: 'AAE', 0x26: 'PCE',
    0x27: 'CTE', 0x28: 'LOE', 0x2A: 'BOFS', 0x2B: 'BONE', 0x2C: 'MW0', 0x2D: 'MW1', 0x36: 'PSUB',
    0x3A: 'WMS', 0x3B: 'WORLD', 0x3C: 'VMS', 0x3D: 'VIEW', 0x3E: 'PMS', 0x3F: 'PROJ', 0x40: 'TMS', 0x41: 'TMATRIX',
    0x48: 'USCALE', 0x49: 'VSCALE', 0x4A: 'UOFFSET', 0x4B: 'VOFFSET', 0x50: 'SHADE', 0x51: 'RNORM',
    0x53: 'CMAT', 0x54: 'EMC', 0x55: 'AMC', 0x56: 'DMC', 0x57: 'SMC', 0x58: 'AMA', 0x5B: 'SPOW',
    0x5C: 'ALC', 0x5D: 'ALA', 0x5E: 'LMODE', 0x9B: 'FFACE', 0xA0: 'TBP0', 0xA8: 'TBW0', 0xB0: 'CBP', 0xB1: 'CBPH',
    0xB8: 'TSIZE0', 0xC0: 'TMAP', 0xC1: 'TSHADE', 0xC2: 'TMODE', 0xC3: 'TPSM', 0xC4: 'CLOAD', 0xC5: 'CMODE',
    0xC6: 'TFLT', 0xC7: 'TWRAP', 0xC8: 'TBIAS', 0xC9: 'TFUNC', 0xCA: 'TEC', 0xCB: 'TFLUSH', 0xCC: 'TSYNC',
    0xDB: 'ATST', 0xDC: 'STST', 0xDD: 'SOP', 0xDE: 'ZTST', 0xDF: 'ALPHA', 0xE0: 'SFIX', 0xE1: 'DFIX',
    0xE6: 'LOP', 0xE7: 'ZMSK', 0xE8: 'PMSKC', 0xE9: 'PMSKA',
}
PRIMS = ['points', 'lines', 'linestrip', 'tris', 'tristrip', 'trifan', 'sprites', '?']


def load(path):
    p = path if os.path.isabs(path) else os.path.join(ROOT, path)
    return open(p, 'rb').read()


def vtype_str(v):
    tc, col, nrm, pos, wt, idx = v & 3, (v >> 2) & 7, (v >> 5) & 3, (v >> 7) & 3, (v >> 9) & 3, (v >> 11) & 3
    nw, nm, thru = ((v >> 14) & 7) + 1, ((v >> 18) & 7) + 1, (v >> 23) & 1
    return f'tc={tc} col={col} nrm={nrm} pos={pos} wt={wt} idx={idx} nweights={nw} morphs={nm} through={thru}'


def vsize(v):
    """Size in bytes of one vertex for a GE vertex type, with PSP alignment rules."""
    tc, col, nrm, pos, wt = v & 3, (v >> 2) & 7, (v >> 5) & 3, (v >> 7) & 3, (v >> 9) & 3
    nw = ((v >> 14) & 7) + 1
    sz = [0, 1, 2, 4]
    off, align = 0, 1

    def put(size, count):
        nonlocal off, align
        if size == 0:
            return
        off = (off + size - 1) // size * size
        off += size * count
        align = max(align, size)
    if wt:
        put(sz[wt], nw)
    if tc:
        put(sz[tc], 2)
    if col:
        put({4: 2, 5: 2, 6: 2, 7: 4}.get(col, 1), 1)
    if nrm:
        put(sz[nrm], 3)
    if pos:
        put(sz[pos], 3)
    return (off + align - 1) // align * align


def disasm(d, start, limit=4000, quiet=False):
    """Disassemble from start until RET/END. Returns (end offset, list of (off, cmd, arg))."""
    out = []
    o = start
    while o + 4 <= len(d) and len(out) < limit:
        w = struct.unpack_from('<I', d, o)[0]
        cmd, arg = w >> 24, w & 0xffffff
        out.append((o, cmd, arg))
        if not quiet:
            name = GE.get(cmd, f'?{cmd:02X}')
            extra = ''
            if cmd == 0x12:
                extra = f'  ({vtype_str(arg)}; vsize={vsize(arg)})'
            elif cmd == 0x04:
                extra = f'  ({PRIMS[(arg >> 16) & 7]} x{arg & 0xffff})'
            elif cmd in (0x5B, 0x48, 0x49, 0x4A, 0x4B, 0x2B, 0x3B):
                extra = f'  (float {struct.unpack("<f", struct.pack("<I", arg << 8))[0]:g})'
            print(f'    {o:06X}  {name:<7} {arg:06X}{extra}')
        o += 4
        if cmd in (0x0B, 0x0C):
            break
    return o, out


def main():
    d = load(sys.argv[1])
    print(f'# {sys.argv[1]} size={len(d):#x}')
    print('header bytes 4..16:', ' '.join(f'{b:02X}' for b in d[4:16]))
    hdr = struct.unpack_from('<10I', d, 0x10)
    names = ['A(mat?)', 'B(mesh?)', 'C(node?)', 'D', 'E', 'F(tex?)', 'dl_start', 'dl_size', 'x30', 'x34']
    for n, v in zip(names, hdr):
        print(f'  {n:<10} {v:#x}')
    a, b, c, dd, e, f, dls, dlsz, x30, x34 = hdr

    def dump(label, start, end, width=16):
        print(f'-- {label} {start:#x}..{end:#x} ({end - start:#x} bytes)')
        for i in range(start, min(end, len(d)), width):
            row = d[i:min(i + width, end)]
            print(f'  {i:06X}  {" ".join(f"{x:02X}" for x in row)}')

    dump('A', a, b, 36)
    dump('B', b, c, 8)
    dump('C', c, dd, 32)
    dump('D', dd, dls, 16)
    dump('E', e, f, 16)
    print(f'-- F at {f:#x}: {d[f:f + 16].hex(" ")}')
    if len(sys.argv) > 2:
        o = dls
        end = dls + dlsz
        while o < end:
            print(f'  -- display list at {o:#x}')
            o, _ = disasm(d, o)


if __name__ == '__main__':
    main()
