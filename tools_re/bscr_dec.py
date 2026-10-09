"""Battle script (`bscr`) decompiler: folds the stack code back into expressions and
statements, with gotos for the jumps. Exploration only.

Usage: bscr_dec.py <file relative to game/unpacked> [function name ...]
       -> tools_re/out/dec_<name>.txt (or the named functions on the console)

Engine functions print as `e<number>` (table 2) or `s<number>` (table 1), with a name
where the function has been read out of the executable.
"""
import os, struct, sys

import mips
from bscr import Script, ROOT, OUT

RETURNS = 0x81

# Table 2 functions whose meaning is known (see docs/BATTLE_NOTES.md).
NAMES_2 = {
    1: 'print', 4: 'sound', 5: 'sound_stop', 6: 'random', 7: 'anim', 8: 'anim_loop', 9: 'wait',
    10: 'wait_anim', 11: 'collision_on', 12: 'move_init', 13: 'random', 14: 'anim_loop_between',
    15: 'place', 17: 'moving', 18: 'move_to', 19: 'action', 22: 'model', 23: 'collision', 24: 'effect_model',
    25: 'move_reset', 26: 'get', 27: 'set', 28: 'remove', 30: 'move', 31: 'wait_move', 33: 'move_flag',
    36: 'anim_frame', 37: 'map_count', 38: 'prop_set', 39: 'prop_get', 40: 'prop_name', 41: 'prop_range',
    42: 'layer', 43: 'colour', 44: 'view', 48: 'turn', 51: 'shadow', 56: 'shake', 62: 'param',
    63: 'effect', 64: 'effect_off', 65: 'root_flag', 67: 'ai', 68: 'ai_get', 69: 'voice', 73: 'drop',
    80: 'log', 103: 'percent', 104: 'ai_check', 105: 'ai_cond', 109: 'bg_fade', 110: 'model_attach',
    114: 'lock_point', 115: 'lock_point_on', 123: 'effects_off', 126: 'anim_playing',
}

BINARY = {0x02: '+', 0x03: '-', 0x04: '*', 0x05: '/', 0x06: '%', 0x08: '&', 0x09: '|', 0x0A: '^',
          0x0B: '<<', 0x0C: '>>', 0x70: '<', 0x71: '<=', 0x72: '>', 0x73: '>=', 0x74: '==', 0x75: '!='}
IMM, PUSH, POP, GI, LI = 0xF0, 0xF2, 0xF3, 0xF4, 0xF6


def signatures(vaddr):
    entries = {}
    at = vaddr + mips.FILE_BIAS
    while True:
        kind, argc, number = struct.unpack_from('<3I', mips.DATA, at)
        if number == 0:
            return entries
        entries[number] = (argc, kind == RETURNS)
        at += 12


TABLES = {1: signatures(0x171370), 2: signatures(0x1720a8)}


def variable(kind, value):
    return {GI: f'g{value}', LI: f'l{value}'}.get(kind, f'?{kind:02x}:{value}')


def decompile(s, only=()):
    code = list(s.instructions())
    starts = sorted(f['code'] for f in s.funcs)
    by_start = {f['code']: f for f in s.funcs}
    targets = {off + vals[0] for off, op, kinds, vals in code if op == 0x84}
    lines = []
    show = False
    stack = []

    def emit(text):
        if show:
            lines.append('    ' + text)

    def pop():
        return stack.pop() if stack else '<empty>'

    skip_test = None
    for index, (off, op, kinds, vals) in enumerate(code):
        if off in by_start:
            f = by_start[off]
            show = not only or f['name'] in only
            stack = []
            if show:
                arguments = ', '.join(f'l{i}' for i in range(f['argc']))
                lines.append('')
                lines.append(f'{f["name"]}({arguments})   # type {f["type"]:#x}, at {off:#x}')
        if off in targets:
            if stack:
                emit(f'# stack at label: {stack}')
            if show:
                lines.append(f'  L{off:x}:')
        if skip_test is not None:
            # The instruction after a test runs only when the test's value is zero.
            condition, skip_test = skip_test, None
            if op == 0x84:
                emit(f'if not ({condition}) goto L{off + vals[0]:x}')
                continue
            emit(f'if not ({condition}):')
        if op in BINARY:
            if kinds[0] == IMM and kinds[1] == IMM:
                a, b = str(vals[0]), str(vals[1])
            elif kinds[0] == IMM:
                a, b = str(vals[0]), pop()
            elif kinds[1] == IMM:
                a, b = pop(), str(vals[1])
            else:
                b = pop()
                a = pop()
            stack.append(f'({a} {BINARY[op]} {b})')
        elif op == 0x0E:
            stack.append(f'!{pop()}')
        elif op in (0x11, 0x12):
            emit(f'{variable(kinds[0], vals[0])} {"+" if op == 0x11 else "-"}= 1')
        elif op == 0x13:
            value = str(vals[0]) if kinds[0] == IMM else pop() if kinds[0] == POP else variable(kinds[0], vals[0])
            if kinds[1] == PUSH:
                stack.append(value)
            else:
                emit(f'{variable(kinds[1], vals[1])} = {value}')
        elif op == 0x18:
            stack.append(str(vals[0]))
        elif op == 0x1A:
            stack.append(repr(s.strings.get(vals[0], vals[0])))
        elif op in (0x1B, 0x1C, 0x1D, 0x85):
            pass
        elif op == 0x80:
            skip_test = str(vals[0]) if kinds[0] == IMM else pop()
        elif op == 0x81:
            f = s.funcs[vals[0]]
            arguments = [pop() for _ in range(f['argc'])][::-1]
            text = f'{f["name"]}({", ".join(arguments)})'
            if f['type'] == 0xE1:
                stack.append(text)
            else:
                emit(text)
        elif op in (0x82, 0x86, 0x87, 0x88):
            table = 1 if op in (0x82, 0x86) else 2
            number = kinds[0] + (0x100 if op in (0x86, 0x88) else 0)
            argc, returns = TABLES[table].get(number, (0, False))
            arguments = [pop() for _ in range(argc)][::-1]
            name = f'e{number}' if table == 2 else f's{number}'
            if table == 2 and number in NAMES_2:
                name += '_' + NAMES_2[number]
            text = f'{name}({", ".join(arguments)})'
            if returns:
                stack.append(text)
            else:
                emit(text)
        elif op == 0x83:
            emit(f'return {stack[-1]}' if stack else 'return')
            stack = []
        elif op == 0x84:
            emit(f'goto L{off + vals[0]:x}')
        else:
            emit(f'# op {op:#x} {bytes(kinds).hex()} {vals}')
    return lines


if __name__ == '__main__':
    import bscr
    rel = sys.argv[1]
    script = Script(bscr.read(rel))
    only = tuple(sys.argv[2:])
    lines = decompile(script, only)
    if only:
        print('\n'.join(lines))
    else:
        path = os.path.join(OUT, f'dec_{script.name or os.path.basename(rel)}.txt')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('\n'.join(lines) + '\n')
        print(f'wrote {os.path.relpath(path)} ({len(lines)} lines)')
