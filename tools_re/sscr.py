"""Disassembler for the game's compiled scripts (the VM at 0x112790 in the executable). Exploration only.

Usage: sscr.py <file relative to game/unpacked> [function name | --range <from hex> <to hex>]

Instruction set, from the interpreter's handlers:
  registers are 32-bit slots in the call context: r0 = pc, r1 = frame pointer,
  r2 = stack pointer, r3 = return value, r4.. = scratch.
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load

REG = {0: 'pc', 1: 'bp', 2: 'sp', 3: 'rv'}


def r(n):
    return REG.get(n, f'r{n}')


# opcode -> (mnemonic, operand layout). Layout letters: r = register byte, i = imm32,
# h = imm16, b = imm8.
OPS = {
    0x00: ('push', 'r'), 0x01: ('pop', 'r'), 0x02: ('mov', 'rr'),
    0x03: ('li', 'ir'), 0x04: ('lw', 'irr'), 0x05: ('sw', 'rir'),
    0x07: ('li16', 'hr'), 0x08: ('lh', 'irr'), 0x09: ('sh', 'rir'),
    0x0b: ('li8', 'br'), 0x0c: ('lb', 'irr'), 0x0d: ('sb', 'rir'),
    0x10: ('add', 'rr'), 0x11: ('addu', 'rr'), 0x12: ('addf', 'rr'),
    0x13: ('sub', 'rr'), 0x14: ('subu', 'rr'), 0x15: ('subf', 'rr'),
    0x16: ('mul', 'rr'), 0x17: ('mulu', 'rr'), 0x18: ('mulf', 'rr'),
    0x19: ('div', 'rr'), 0x1a: ('divu', 'rr'), 0x1b: ('divf', 'rr'),
    0x1c: ('mod', 'rr'), 0x1d: ('modu', 'rr'),
    0x1e: ('shl', 'rr'), 0x1f: ('shlu', 'rr'), 0x20: ('sar', 'rr'), 0x21: ('shr', 'rr'),
    0x22: ('and', 'rr'), 0x23: ('or', 'rr'), 0x24: ('xor', 'rr'),
    0x25: ('not', 'r'), 0x26: ('lnot', 'r'), 0x27: ('neg', 'r'), 0x28: ('negf', 'r'),
    0x29: ('f2i', 'rr'), 0x2a: ('i2f', 'rr'),
    0x30: ('cmp', 'rr'), 0x31: ('cmpu', 'rr'), 0x32: ('cmpf', 'rr'),
    0x33: ('sete', 'r'), 0x34: ('setne', 'r'), 0x35: ('setl', 'r'),
    0x36: ('setg', 'r'), 0x37: ('setle', 'r'), 0x38: ('setge', 'r'),
    0x39: ('jmp', 'i'), 0x3a: ('je', 'i'), 0x3b: ('jne', 'i'),
    0x3c: ('call', 'i'), 0x3d: ('ret', ''), 0x3e: ('callr', 'r'), 0x40: ('native', 'i'),
}
SIZE = {'r': 1, 'i': 4, 'h': 2, 'b': 1}


def decode(d, at):
    if at >= len(d):
        return None
    op = d[at]
    if op not in OPS:
        return None
    name, layout = OPS[op]
    # The padding after a script's last instruction can look like the start of one.
    if at + 1 + sum(SIZE[kind] for kind in layout) > len(d):
        return None
    args = []
    o = at + 1
    for kind in layout:
        if kind == 'r':
            args.append(('r', d[o]))
        elif kind == 'i':
            args.append(('i', struct.unpack_from('<I', d, o)[0], o))
        elif kind == 'h':
            args.append(('h', struct.unpack_from('<h', d, o)[0]))
        elif kind == 'b':
            args.append(('b', struct.unpack_from('<b', d, o)[0]))
        o += SIZE[kind]
    return name, args, o - at


class Script:
    def __init__(self, d):
        self.d = d
        # Files without the 12-byte SSCR header store every offset 12 too large.
        self.bias = 0 if d[:4] == b'SSCR' else 12
        self.base = 12 if d[:4] == b'SSCR' else 0
        h = struct.unpack_from('<7I', d, self.base)
        self.symbols_at, self.relocs_at, self.code_at, self.code_end = (x - self.bias for x in h[:4])
        self.header = h
        # The symbol table (value, name offset) runs up to the first name string.
        self.symbols = []
        at, names_at = self.symbols_at, self.relocs_at
        while at < names_at:
            value, name_at = struct.unpack_from('<II', d, at)
            names_at = min(names_at, name_at - self.bias)
            name = d[name_at - self.bias:].split(b'\0')[0].decode('latin1')
            self.symbols.append((value - self.bias, name))
            at += 8
        # Relocations are (location of a 32-bit immediate, target) pairs.
        self.relocs = {}
        for at in range(self.relocs_at, self.code_at - 7, 8):
            where, target = struct.unpack_from('<II', d, at)
            self.relocs[where - self.bias] = target - self.bias
        self.by_addr = {v: n for v, n in self.symbols}

    def listing(self, start, end):
        d = self.d
        at = start
        out = []
        while at < end:
            ins = decode(d, at)
            if ins is None:
                out.append(f'  {at:05x}  .byte {d[at]:02x}')
                at += 1
                continue
            name, args, length = ins
            parts = []
            for a in args:
                if a[0] == 'r':
                    parts.append(r(a[1]))
                elif a[0] == 'i':
                    value, where = a[1], a[2]
                    if where in self.relocs:
                        target = self.relocs[where]
                        label = self.by_addr.get(target)
                        parts.append(f'@{target:x}' + (f' <{label}>' if label else ''))
                    else:
                        f = struct.unpack('<f', struct.pack('<I', value))[0]
                        note = f' ({f:g}f)' if 1e-4 < abs(f) < 1e7 and value > 0xffff else ''
                        parts.append(f'{value:#x}{note}')
                else:
                    parts.append(str(a[1]))
            label = self.by_addr.get(at)
            if label:
                out.append(f'{label}:')
            out.append(f'  {at:05x}  {name:<7}{", ".join(parts)}')
            at += length
        return out


def main():
    d = load(sys.argv[1])
    s = Script(d)
    print(f'# {sys.argv[1]} size={len(d):#x} bias={s.bias} header={[hex(x) for x in s.header]}')
    print(f'symbols at {s.symbols_at:#x}, relocations at {s.relocs_at:#x} ({len(s.relocs)}), code {s.code_at:#x}..{s.code_end:#x}')
    for value, name in s.symbols:
        print(f'  {value:05x}  {name}')
    # A function that has no name (one only ever called through a table) is listed
    # by where it is.
    if len(sys.argv) > 4 and sys.argv[2] == '--range':
        print()
        print('\n'.join(s.listing(int(sys.argv[3], 16), int(sys.argv[4], 16))))
        return
    starts = sorted(v for v, _ in s.symbols if s.code_at <= v <= s.code_end)
    want = sys.argv[2] if len(sys.argv) > 2 else None
    for i, start in enumerate(starts):
        name = s.by_addr[start]
        if want and name != want:
            continue
        end = starts[i + 1] if i + 1 < len(starts) else s.code_end
        print()
        print('\n'.join(s.listing(start, end)))


if __name__ == '__main__':
    main()
