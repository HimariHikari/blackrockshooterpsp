"""Battle script (`bscr`) disassembler. Exploration only.

Usage: bscr.py dis <file relative to game/unpacked> [function name]   -> tools_re/out/bscr_<name>.txt
       bscr.py survey                                                 header + native usage over all files

Layout (from the loader at 0xCE3B8 in the executable):
  0x00 "bscr", 0x06 name, 0x24 file size, 0x28 function count, 0x2C function table offset,
  0x30 string count, 0x34 string area offset, 0x38 code size, 0x3C code offset.
  Function entry: u16 entry size, u16 type, char name[32], u32 code offset, u32 argument
  count, then 8 bytes per argument.
  Code: 16-byte instructions: opcode, three operand-kind bytes, three 32-bit operands.
"""
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
OUT = os.path.join(HERE, 'out')

OPS = {0x02: 'add', 0x03: 'sub', 0x04: 'mul', 0x05: 'div', 0x06: 'mod', 0x08: 'and', 0x09: 'or',
       0x0A: 'xor', 0x0B: 'shl', 0x0C: 'shr', 0x0E: 'not', 0x11: 'inc', 0x12: 'dec', 0x13: 'mov',
       0x14: 'movm', 0x15: 'leas', 0x17: 'push?', 0x18: 'push', 0x19: 'pushf?', 0x1A: 'pushs',
       0x1B: 'result', 0x1C: 'savesp', 0x1D: 'loadsp', 0x70: 'lt', 0x71: 'le', 0x72: 'gt',
       0x73: 'ge', 0x74: 'eq', 0x75: 'ne', 0x80: 'if', 0x81: 'call', 0x82: 'sys', 0x83: 'ret',
       0x84: 'jmp', 0x85: 'label', 0x86: 'sys', 0x87: 'sysb', 0x88: 'sysb'}
KINDS = {0xF0: '#', 0xF1: 'r', 0xF2: 'push', 0xF3: 'pop', 0xF4: 'gi', 0xF5: 'gf', 0xF6: 'li',
         0xF7: 'lf', 0xF8: 'ge', 0xF9: 'le'}
TYPES = {0xE0: 'func', 0xE1: 'func ret', 0xE4: 'init', 0xE7: 'entry68', 0xE8: 'entry64',
         0xE9: 'entry60', 0xEC: 'signal', 0xED: 'update'}


def read(rel):
    """The bytes of a script, by its path under game/unpacked. A field map's script is
    slot 10 of the map's bundle: naming the bundle's folder gives it."""
    path = os.path.join(ROOT, rel)
    if os.path.isdir(path):
        path = os.path.join(path, '10.bscr')
    return open(path, 'rb').read()


class Script:
    def __init__(self, d):
        assert d[:4] == b'bscr', d[:4]
        self.d = d
        self.name = d[6:0x24].split(b'\0')[0].decode('latin1')
        (self.size, self.nfunc, self.func_at, self.nstr, self.str_at, self.code_size,
         self.code_at) = struct.unpack_from('<7I', d, 0x24)
        self.funcs = []
        p = self.func_at
        for _ in range(self.nfunc):
            size, kind = struct.unpack_from('<HH', d, p)
            name = d[p + 4:p + 0x24].split(b'\0')[0].decode('latin1')
            code, argc = struct.unpack_from('<II', d, p + 0x24)
            args = [struct.unpack_from('<II', d, p + 0x2C + 8 * i) for i in range(argc)]
            self.funcs.append({'name': name, 'type': kind, 'code': code, 'argc': argc, 'args': args,
                               'size': size})
            p += 0x2C + 8 * argc
        self.func_end = p
        self.strings = {}
        p = self.str_at
        for _ in range(self.nstr):
            end = d.index(b'\0', p)
            self.strings[p - self.str_at] = d[p:end].decode('utf-8', 'replace')
            p = end + 1

    def instructions(self):
        for off in range(0, self.code_size, 16):
            raw = self.d[self.code_at + off:self.code_at + off + 16]
            yield off, raw[0], raw[1:4], struct.unpack_from('<3i', raw, 4)


def operand(kind, value):
    if kind == 0x5A:
        return None
    if kind == 0xF0:
        return f'#{value}'
    if kind in (0xF2, 0xF3):
        return KINDS[kind]
    if kind in (0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9):
        return f'{KINDS[kind]}{value}'
    if kind == 0xF1:
        return 'r'
    return f'?{kind:02x}:{value:#x}'


def text(s, off, op, kinds, vals, starts):
    name = OPS.get(op, f'op{op:02x}')
    if op in (0x82, 0x86):
        return f'sys {kinds[0] + (0x100 if op == 0x86 else 0)}'
    if op in (0x87, 0x88):
        return f'sysb {kinds[0] + (0x100 if op == 0x88 else 0)}'
    if op == 0x81:
        f = s.funcs[vals[0]] if 0 <= vals[0] < len(s.funcs) else None
        return f'call {f["name"] if f else vals[0]}'
    if op == 0x84:
        return f'jmp {off + vals[0]:#06x}'
    if op == 0x1A:
        return f'pushs {s.strings.get(vals[0], vals[0])!r}'
    if op == 0x18:
        return f'push #{vals[0]}'
    parts = [operand(k, v) for k, v in zip(kinds, vals)]
    # The third kind byte has no operand of its own beyond the three values; show what is set.
    parts = [p for p in parts if p is not None]
    return (name + ' ' + ', '.join(parts)).strip()


def disassemble(s, only=None):
    starts = {f['code']: f for f in s.funcs}
    lines = [f'# {s.name}: {s.nfunc} functions, {s.nstr} strings, code {s.code_size:#x} bytes at {s.code_at:#x}']
    for i, f in enumerate(s.funcs):
        lines.append(f'#   [{i:3}] {TYPES.get(f["type"], hex(f["type"])):9} {f["name"]}({f["argc"]}) at {f["code"]:#06x}'
                     + ''.join(f' arg{a}' for a in f['args'] if a[0] != 0x81))
    for key, value in s.strings.items():
        lines.append(f'#   string {key:#x}: {value!r}')
    show = only is None
    for off, op, kinds, vals in s.instructions():
        if off in starts:
            f = starts[off]
            show = only is None or f['name'] == only
            if show:
                lines.append('')
                lines.append(f'{f["name"]}({f["argc"]}):   ; {TYPES.get(f["type"], hex(f["type"]))}')
        if show:
            lines.append(f'  {off:06x}  {text(s, off, op, kinds, vals, starts)}')
    return lines


def all_scripts():
    for folder, _, names in os.walk(ROOT):
        for name in names:
            path = os.path.join(folder, name)
            with open(path, 'rb') as f:
                if f.read(4) != b'bscr':
                    continue
                f.seek(0)
                yield os.path.relpath(path, ROOT), f.read()


def survey():
    ops, sys1, sys2, types, kinds = {}, {}, {}, {}, {}
    count = 0
    lines = []
    for rel, d in all_scripts():
        count += 1
        if len(d) < 0x40:
            lines.append(f'stub {rel}: {len(d)} bytes {d.hex()}')
            continue
        s = Script(d)
        ok = s.size == len(d) and s.code_at + s.code_size == len(d) and s.func_end <= s.str_at
        lines.append(f'{"ok " if ok else "BAD"} {rel}: {s.nfunc} functions, {s.nstr} strings, {s.code_size // 16} instructions')
        for f in s.funcs:
            types[f['type']] = types.get(f['type'], 0) + 1
        for off, op, k, vals in s.instructions():
            ops[op] = ops.get(op, 0) + 1
            if op in (0x82, 0x86):
                n = k[0] + (0x100 if op == 0x86 else 0)
                sys1[n] = sys1.get(n, 0) + 1
            elif op in (0x87, 0x88):
                n = k[0] + (0x100 if op == 0x88 else 0)
                sys2[n] = sys2.get(n, 0) + 1
            else:
                key = (op, bytes(k))
                kinds[key] = kinds.get(key, 0) + 1
    lines.append('')
    lines.append(f'{count} scripts')
    lines.append('function types: ' + ', '.join(f'{t:#x}:{n}' for t, n in sorted(types.items())))
    lines.append('opcodes: ' + ', '.join(f'{OPS.get(o, hex(o))}({o:#x}):{n}' for o, n in sorted(ops.items())))
    lines.append('operand kind combinations:')
    for (op, k), n in sorted(kinds.items()):
        lines.append(f'  {OPS.get(op, hex(op)):7} {k.hex()} x{n}')
    lines.append(f'table 1 natives used ({len(sys1)}): ' + ', '.join(f'{i}:{n}' for i, n in sorted(sys1.items())))
    lines.append(f'table 2 natives used ({len(sys2)}): ' + ', '.join(f'{i}:{n}' for i, n in sorted(sys2.items())))
    path = os.path.join(OUT, 'bscr_survey.txt')
    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines) + '\n')
    print('\n'.join(lines[count:]))


if __name__ == '__main__':
    if sys.argv[1] == 'survey':
        survey()
    else:
        rel = sys.argv[2]
        with open(os.path.join(ROOT, rel), 'rb') as f:
            s = Script(f.read())
        lines = disassemble(s, sys.argv[3] if len(sys.argv) > 3 else None)
        name = os.path.basename(rel)
        path = os.path.join(OUT, f'bscr_{name}.txt')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('\n'.join(lines) + '\n')
        print(f'wrote {os.path.relpath(path)} ({len(lines)} lines)')
