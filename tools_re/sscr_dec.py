"""Decompiles an `SSCR` script (the VM at 0x112790) into C-like text. Exploration only.

Usage: sscr_dec.py <file relative to game/unpacked> [function name] [--table field|symbol|bike]

The scripts are compiled C: every expression is worked out in `rv` with `r4` as the
second operand, arguments are pushed last first, locals live below `bp` and arguments
above it. This follows the registers and the stack symbolically and prints one line
per statement, with labels and gotos for the jumps. Calls of engine functions are
written `nXX(...)`, or by name where the table's function is known.
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load
from sscr import Script, decode

PC, BP, SP, RV = 0, 1, 2, 3

# Names for the engine functions that have been read, by table.
NAMES = {
    'field': {
        0x07: 'set_sys_value', 0x13: 'get_var', 0x14: 'get_flag', 0x15: 'set_flag', 0x16: 'sys_value',
        0x17: 'get_player_figure', 0x18: 'set_var', 0x26: 'print', 0x29: 'chr_by_id', 0x2a: 'chr_action',
        0x32: 'mission_record', 0x44: 'prm', 0x49: 'chr_set', 0x4c: 'set_player_figure',
    },
    'symbol': {
        0x00: 'set_byte', 0x01: 'get_byte', 0x02: 'set_bit', 0x03: 'get_bit', 0x04: 'wait_frames',
        0x05: 'chase', 0x06: 'run_event', 0x07: 'wait_until', 0x08: 'noticed', 0x09: 'restart_state',
        0x0b: 'set_home', 0x0c: 'walk_home', 0x0d: 'get_chr_flag', 0x0f: 'state', 0x10: 'print',
        0x11: 'action', 0x12: 'setting', 0x13: 'set_chr_flag', 0x14: 'set_int', 0x15: 'wait_action',
        0x18: 'sys_value', 0x19: 'wait_model', 0x1a: 'set_flag', 0x1b: 'get_flag', 0x1c: 'get_int',
        0x1d: 'test_switch',
    },
    # The ride on the bike's (the table at 0x172D98; docs/BIKE_NOTES.md).
    'bike': {
        0x00: 'trace', 0x01: 'set_game_flag', 0x04: 'lane_chip', 0x05: 'trap_make', 0x06: 'trap_end',
        0x07: 'trap_set', 0x08: 'chr_end', 0x09: 'chr_set', 0x0a: 'chr_setf', 0x0b: 'wait_model',
        0x0c: 'chr_do', 0x0d: 'wait_game_frames', 0x0e: 'fade', 0x0f: 'wait_fade', 0x10: 'chr_getf',
        0x11: 'chr_get', 0x12: 'chip_get', 0x13: 'wait_lane', 0x14: 'new_id', 0x15: 'cam_apply',
        0x17: 'math', 0x18: 'set_model', 0x19: 'set_piece', 0x1a: 'set_effect', 0x1b: 'model_do',
        0x1c: 'piece_do', 0x1d: 'effect_do', 0x1e: 'drop_model', 0x1f: 'drop_piece', 0x20: 'drop_effect',
        0x21: 'model_part', 0x22: 'drop_script', 0x23: 'own', 0x24: 'turn_bone', 0x25: 'wait',
        0x26: 'wait_time', 0x28: 'say', 0x29: 'wait_say', 0x2a: 'frame_time', 0x2b: 'box_set',
        0x2c: 'chr_exists', 0x2d: 'send', 0x2e: 'sound', 0x2f: 'cam_set', 0x30: 'cam_setf',
        0x31: 'shift_all', 0x32: 'shift_chr', 0x37: 'chr_make', 0x38: 'emy_rec', 0x39: 'trp_rec',
        0x3a: 'chr_setv', 0x3b: 'chr_getv', 0x3c: 'chip_at', 0x3d: 'bone_at', 0x3e: 'v_scale',
        0x3f: 'v_unit', 0x40: 'v_add', 0x41: 'v_sub', 0x42: 'v_turn', 0x43: 'has_effect',
        0x44: 'effect_setv', 0x45: 'effect_getv', 0x46: 'set_script', 0x47: 'chr_send', 0x48: 'course',
        0x4a: 'cam_setv', 0x4b: 'sound_stop', 0x4c: 'sound_plays', 0x4d: 'cam_motion',
        0x4e: 'wait_cam_motion', 0x4f: 'pressed', 0x50: 'cam_motion_plays', 0x52: 'music_fade',
        0x53: 'music',
    },
}

BINARY = {
    'add': '+', 'addu': '+', 'addf': '+.', 'sub': '-', 'subu': '-', 'subf': '-.', 'mul': '*', 'mulu': '*',
    'mulf': '*.', 'div': '/', 'divu': '/u', 'divf': '/.', 'mod': '%', 'modu': '%u', 'shl': '<<', 'shlu': '<<',
    'sar': '>>', 'shr': '>>u', 'and': '&', 'or': '|', 'xor': '^',
}
SETCC = {'sete': '==', 'setne': '!=', 'setl': '<', 'setg': '>', 'setle': '<=', 'setge': '>='}
SIZES = {'lw': '', 'lh': '(i16)', 'lb': '(i8)', 'sw': '', 'sh': '(i16)', 'sb': '(i8)'}


def signed(value):
    return value - (1 << 32) if value & 0x80000000 else value


class Function:
    def __init__(self, script, start, end, names):
        self.s, self.start, self.end, self.names = script, start, end, names
        self.ins = []
        at = start
        while at < end:
            decoded = decode(script.d, at)
            if decoded is None:
                break
            self.ins.append((at,) + decoded)
            at += decoded[2]
        self.labels = set()
        for at, name, args, _ in self.ins:
            if name in ('jmp', 'je', 'jne'):
                self.labels.add(self.target(args[0]))

    def target(self, arg):
        """The image offset a relocated immediate points at."""
        return self.s.relocs.get(arg[2], arg[1])

    def index_of(self, at):
        """The place in the listing of the instruction at an image offset."""
        for index, ins in enumerate(self.ins):
            if ins[0] == at:
                return index
        return len(self.ins)

    @staticmethod
    def uses(name, regs):
        """The registers an instruction reads and those it writes."""
        if name == 'push':
            return regs, []
        if name == 'pop' or name in ('li', 'li16', 'li8') or name in SETCC:
            return [], regs
        if name in ('mov', 'f2i', 'i2f'):
            return regs[:1], regs[1:]
        if name in ('lw', 'lh', 'lb'):
            return regs[:1], regs[1:]
        if name in ('sw', 'sh', 'sb', 'cmp', 'cmpu', 'cmpf', 'callr'):
            return regs, []
        if name in BINARY:
            return regs, regs[1:]
        return regs, regs

    def live(self, index, seen=None):
        """Whether rv is read before it is written, going on from an instruction. A
        return reads it: it is what the function answers."""
        seen = set() if seen is None else seen
        while index < len(self.ins) and index not in seen:
            seen.add(index)
            _, name, args, _ = self.ins[index]
            regs = [a[1] for a in args if a[0] == 'r']
            if name == 'pop' and regs[0] == PC:
                return True
            if name == 'jmp':
                index = self.index_of(self.target(args[0]))
                continue
            if name in ('je', 'jne'):
                return self.live(self.index_of(self.target(args[0])), seen) or self.live(index + 1, seen)
            if name in ('call', 'native'):
                return False
            reads, writes = self.uses(name, regs)
            if RV in reads:
                return True
            if RV in writes:
                return False
            index += 1
        return False

    # -- expressions -------------------------------------------------------------
    def const(self, arg):
        value, where = arg[1], arg[2]
        if where in self.s.relocs:
            return ('addr', self.s.relocs[where])
        return ('const', value)

    def text(self, e, floaty=False):
        kind = e[0]
        if kind == 'const':
            value = e[1]
            if floaty or (value > 0xffff and 1e-4 < abs(struct.unpack('<f', struct.pack('<I', value))[0]) < 1e7
                          and value & 0xfff == 0 and not 0xff000000 <= value):
                return f'{struct.unpack("<f", struct.pack("<I", value))[0]:g}f'
            number = signed(value)
            return str(number) if -4096 < number < 4096 else f'{value:#x}'
        if kind == 'addr':
            name = self.s.by_addr.get(e[1])
            if name:
                return name
            string = self.string(e[1])
            return string if string is not None else f'@{e[1]:x}'
        if kind == 'reg':
            return e[1]
        if kind == 'frame':
            return '&' + self.variable(e[1])
        if kind == 'bin':
            floaty = e[1].endswith('.')
            return f'({self.text(e[2], floaty)} {e[1].rstrip(".")} {self.text(e[3], floaty)})'
        if kind == 'un':
            return f'{e[1]}{self.text(e[2], e[1] == "-.")}'.replace('-.', '-')
        if kind == 'deref':
            return self.lvalue(e[1], e[2])
        if kind == 'call':
            return f'{e[1]}({", ".join(self.text(a) for a in e[2])})'
        return str(e)

    def string(self, at):
        """The text at an image offset, if it is one."""
        d = self.s.d
        if not self.s.code_at <= at < len(d):
            return None
        raw = d[at:at + 200].split(b'\0')[0]
        if len(raw) < 2:
            return None
        for encoding in ('utf-8', 'cp932'):
            try:
                text = raw.decode(encoding)
            except UnicodeDecodeError:
                continue
            if all(c.isprintable() or c in '\n\t' for c in text):
                return '"' + text.replace('\n', '\\n') + '"'
        return None

    def variable(self, offset):
        offset = signed(offset & 0xffffffff)
        if offset >= 8:
            return f'a{(offset - 8) // 4}'
        return f'v{-offset:x}' if offset < 0 else f'bp[{offset}]'

    def lvalue(self, address, size=''):
        if address[0] == 'frame':
            return size + self.variable(address[1])
        return f'{size}*({self.text(address)})' if address[0] not in ('bin',) else f'{size}*{self.text(address)}'

    def add(self, a, b, op='+'):
        """a op b, folding what is known about the frame and constants."""
        if op == '+':
            for x, y in ((a, b), (b, a)):
                if x[0] == 'frame' and y[0] == 'const':
                    return ('frame', x[1] + signed(y[1]))
                if x[0] == 'const' and x[1] == 0:
                    return y
            if a[0] == 'const' and b[0] == 'const':
                return ('const', (a[1] + b[1]) & 0xffffffff)
        return ('bin', op, a, b)

    # -- the walk ----------------------------------------------------------------
    def run(self):
        out = []
        reg = {}
        stack = []
        flags = None
        pending = [None]  # a call whose result sits in rv and has not been used

        fresh = [False]  # rv has been written and not read since

        def get(r):
            if r == RV:
                pending[0] = None
                fresh[0] = False
            if r == BP:
                return ('frame', 0)
            return reg.get(r, ('reg', {PC: 'pc', SP: 'sp', RV: 'rv'}.get(r, f'r{r}')))

        def flush():
            if pending[0] is not None:
                out.append(f'    {self.text(pending[0])};')
                pending[0] = None

        def put(r, e):
            if r == RV:
                flush()
                fresh[0] = True
            reg[r] = e

        def boundary():
            flush()
            if stack:
                out.append(f'    // {len(stack)} values left on the stack: {", ".join(self.text(e) for e in stack)}')
                stack.clear()

        def keep(target_index):
            """Before control leaves this run of code for `target_index`: if what rv
            holds is read there, it has to be said."""
            value = reg.get(RV)
            if value is not None and value != ('reg', 'rv') and self.live(target_index):
                pending[0] = None
                out.append(f'    rv = {self.text(value)};')
                reg[RV] = ('reg', 'rv')

        index = 0
        # The prologue: push bp; mov sp, bp; then room for the locals.
        while index < len(self.ins):
            at, name, args, _ = self.ins[index]
            if at in self.labels:
                keep(index)
                boundary()
                reg.clear()
                fresh[0] = False
                out.append(f'  L{at:x}:')
            index += 1
            if name == 'push':
                r = args[0][1]
                if (r == BP and index == 1) or r == PC:
                    continue
                stack.append(get(r))
            elif name == 'pop':
                r = args[0][1]
                if r == PC:
                    if RV not in reg:
                        out.append('    return rv;')
                    elif fresh[0]:
                        pending[0] = None
                        out.append(f'    return {self.text(reg[RV])};')
                    else:
                        flush()
                        out.append('    return;')
                    reg.pop(RV, None)
                    fresh[0] = False
                    continue
                if r == BP:
                    continue
                put(r, stack.pop() if stack else ('reg', '<empty stack>'))
            elif name == 'mov':
                src, dst = args[0][1], args[1][1]
                if (src, dst) in ((SP, BP), (BP, SP)):
                    continue
                put(dst, get(src))
            elif name in ('li', 'li16', 'li8'):
                put(args[1][1], self.const(args[0]) if name == 'li' else ('const', args[0][1] & 0xffffffff))
            elif name in ('lw', 'lh', 'lb'):
                offset, base, dst = self.const(args[0]), args[1][1], args[2][1]
                put(dst, ('deref', self.add(get(base), offset), SIZES[name]))
            elif name in ('sw', 'sh', 'sb'):
                src, offset, base = args[0][1], self.const(args[1]), args[2][1]
                value = get(src)
                out.append(f'    {self.lvalue(self.add(get(base), offset), SIZES[name])} = {self.text(value)};')
            elif name in BINARY:
                src, dst = args[0][1], args[1][1]
                if dst == SP:
                    # Room for locals (down) or arguments dropped after a call (up).
                    amount = get(src)
                    count = signed(amount[1]) // 4 if amount[0] == 'const' else 0
                    for _ in range(max(count, 0)):
                        if stack:
                            stack.pop()
                    continue
                put(dst, self.add(get(dst), get(src), BINARY[name]))
            elif name in ('not', 'lnot', 'neg', 'negf'):
                r = args[0][1]
                put(r, ('un', {'not': '~', 'lnot': '!', 'neg': '-', 'negf': '-.'}[name], get(r)))
            elif name in ('f2i', 'i2f'):
                src, dst = args[0][1], args[1][1]
                put(dst, ('call', 'int' if name == 'f2i' else 'float', [get(src)]))
            elif name in ('cmp', 'cmpu', 'cmpf'):
                flags = (get(args[1][1]), get(args[0][1]), name == 'cmpf')
            elif name in SETCC:
                a, b, floaty = flags or (('reg', '?'), ('reg', '?'), False)
                put(args[0][1], ('bin', SETCC[name] + ('.' if floaty else ''), a, b))
            elif name in ('jmp', 'je', 'jne'):
                target = self.target(args[0])
                keep(self.index_of(target))
                if name == 'jmp':
                    boundary()
                    out.append(f'    goto L{target:x};')
                else:
                    a, b, floaty = flags or (('reg', '?'), ('reg', '?'), False)
                    flush()
                    zero = b[0] == 'const' and b[1] == 0
                    if zero:
                        condition = f'!{self.text(a)}' if name == 'je' else self.text(a)
                    else:
                        condition = f'{self.text(a, floaty)} {"==" if name == "je" else "!="} {self.text(b, floaty)}'
                    out.append(f'    if ({condition}) goto L{target:x};')
            elif name in ('call', 'callr', 'native'):
                if name == 'call':
                    target = self.target(args[0])
                    callee = self.s.by_addr.get(target, f'sub_{target:x}')
                elif name == 'native':
                    callee = self.native(args[0][1])
                else:
                    e = get(args[0][1])
                    if e[0] == 'const' and e[1] & 1:
                        callee = self.native(e[1] >> 8)
                    elif e[0] == 'addr':
                        callee = self.s.by_addr.get(e[1], f'sub_{e[1]:x}')
                    else:
                        callee = f'(*{self.text(e)})'
                # The caller drops the arguments afterwards: that says how many there are.
                count = 0
                if index + 1 < len(self.ins):
                    (_, n1, a1, _), (_, n2, a2, _) = self.ins[index], self.ins[index + 1]
                    if n1 in ('li', 'li16', 'li8') and n2 == 'add' and a2[1][1] == SP and a2[0][1] == a1[1][1]:
                        count = a1[0][1] // 4
                        index += 2
                arguments = [stack.pop() if stack else ('reg', '<missing>') for _ in range(count)]
                call = ('call', callee, arguments)
                put(RV, call)
                pending[0] = call
            else:
                out.append(f'    // {name} {args}')
        boundary()
        return out

    def native(self, number):
        return self.names.get(number, f'n{number:02x}')


def main():
    arguments = [a for a in sys.argv[1:] if not a.startswith('--')]
    table = 'field'
    if '--table' in sys.argv:
        table = sys.argv[sys.argv.index('--table') + 1]
        arguments = [a for a in arguments if a != table]
    if not arguments:
        print(__doc__)
        return
    s = Script(load(arguments[0]))
    want = arguments[1] if len(arguments) > 1 else None
    # Functions start at a symbol or at a prologue (push bp; mov sp, bp): not every
    # function has a name.
    starts, at = set(), s.code_at
    while at < s.code_end:
        decoded = decode(s.d, at)
        if decoded is None:
            at += 1
            continue
        name, args, length = decoded
        following = decode(s.d, at + length)
        if (name == 'push' and args[0][1] == BP and following and following[0] == 'mov'
                and [a[1] for a in following[1]] == [SP, BP]):
            starts.add(at)
        at += length
        # A function ends with its return; the next starts on a multiple of four.
        if name == 'pop' and args[0][1] == PC:
            at = (at + 3) & ~3
    starts = sorted(starts | {v for v, _ in s.symbols if s.code_at <= v < s.code_end})
    for value in starts:
        s.by_addr.setdefault(value, f'sub_{value:x}')
    for i, start in enumerate(starts):
        name = s.by_addr[start]
        if want and name != want:
            continue
        end = starts[i + 1] if i + 1 < len(starts) else s.code_end
        function = Function(s, start, end, NAMES.get(table, {}))
        # Whatever the decoder cannot read is data, not code: stop there.
        if not function.ins:
            continue
        print(f'{name}()   // at {start:#x}')
        print('\n'.join(function.run()))
        print()


if __name__ == '__main__':
    main()
