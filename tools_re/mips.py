"""Disassembly helpers for the game executable (EBOOT.OLD, a MIPS/Allegrex PRX). Exploration only.

Usage:
  mips.py xref <string regex>          code that loads the address of matching strings
  mips.py xrefaddr <hex vaddr>         code that loads this address
  mips.py dis <hex vaddr> [count]      disassemble
  mips.py func <hex vaddr>             disassemble the whole function containing the address
  mips.py calls <hex vaddr>            call sites (jal) of this function
"""
import os, re, struct, sys

import capstone

import vfpu

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.path.join(HERE, '..', 'game', 'extracted', 'PSP_GAME', 'SYSDIR', 'EBOOT.OLD')

DATA = open(EXE, 'rb').read()
# The image is linked at address 0; the first loadable segment starts 0xC0 into the file.
FILE_BIAS = 0xC0
TEXT_END = 0x164520

MD = capstone.Cs(capstone.CS_ARCH_MIPS, capstone.CS_MODE_MIPS32 + capstone.CS_MODE_LITTLE_ENDIAN)


def word(vaddr):
    return struct.unpack_from('<I', DATA, vaddr + FILE_BIAS)[0]


def cstring(vaddr, limit=120):
    raw = DATA[vaddr + FILE_BIAS:vaddr + FILE_BIAS + limit].split(b'\0')[0]
    return raw.decode('latin1')


def is_string(vaddr):
    if not (TEXT_END <= vaddr < len(DATA) - FILE_BIAS):
        return False
    raw = DATA[vaddr + FILE_BIAS:vaddr + FILE_BIAS + 64].split(b'\0')[0]
    return len(raw) >= 3 and all(32 <= b < 127 for b in raw)


def disasm_one(vaddr):
    # The vector unit's instructions first: capstone takes some of them for others.
    vector = vfpu.decode(word(vaddr), vaddr)
    if vector:
        return vector
    code = DATA[vaddr + FILE_BIAS:vaddr + FILE_BIAS + 4]
    for ins in MD.disasm(code, vaddr):
        return f'{ins.mnemonic} {ins.op_str}'.strip()
    return f'.word {word(vaddr):#010x}'


def load_refs():
    """All (address of the second instruction, target) pairs for lui+addiu/ori address loads."""
    refs = []
    hi = {}  # register -> (lui value, address)
    for a in range(0, TEXT_END, 4):
        w = word(a)
        op, rs, rt, imm = w >> 26, (w >> 21) & 31, (w >> 16) & 31, w & 0xffff
        simm = imm - 0x10000 if imm & 0x8000 else imm
        if op == 0x0f:  # lui
            hi[rt] = (imm << 16, a)
        elif op == 0x09 and rs in hi and a - hi[rs][1] <= 64:  # addiu
            refs.append((a, (hi[rs][0] + simm) & 0xffffffff))
        elif op == 0x0d and rs in hi and a - hi[rs][1] <= 64:  # ori
            refs.append((a, hi[rs][0] | imm))
        # Function boundaries reset tracking loosely: jr ra.
        if w == 0x03e00008:
            hi = {}
    return refs


def function_bounds(vaddr):
    """Guess the function around vaddr: back to the previous `jr ra` (+ delay slot), forward to the next."""
    start = vaddr & ~3
    while start > 0:
        if word(start - 8) == 0x03e00008:
            break
        start -= 4
    end = vaddr & ~3
    while end < TEXT_END and word(end) != 0x03e00008:
        end += 4
    return start, end + 8


def annotate(a, refs_by_addr):
    text = disasm_one(a)
    note = ''
    w = word(a)
    if w >> 26 == 3:  # jal
        note = f'   ; call {(w & 0x3ffffff) << 2:#x}'
    if a in refs_by_addr:
        target = refs_by_addr[a]
        note = f'   ; -> {target:#x}' + (f' "{cstring(target)}"' if is_string(target) else '')
    return f'  {a:#08x}  {word(a):08x}  {text}{note}'


def main():
    cmd = sys.argv[1]
    refs = load_refs()
    refs_by_addr = dict(refs)
    if cmd == 'xref':
        rx = re.compile(sys.argv[2])
        targets = {}
        for m in re.finditer(rb'[\x20-\x7e]{3,}', DATA):
            s = m.group().decode('latin1')
            if rx.search(s):
                targets[m.start() - FILE_BIAS] = s
        for a, t in refs:
            if t in targets:
                start, _ = function_bounds(a)
                print(f'{a:#08x} (function {start:#08x}) loads {t:#x} "{targets[t]}"')
    elif cmd == 'xrefaddr':
        want = int(sys.argv[2], 16)
        for a, t in refs:
            if t == want:
                print(f'{a:#08x} (function {function_bounds(a)[0]:#08x})')
    elif cmd == 'dis':
        start = int(sys.argv[2], 16)
        for a in range(start, start + 4 * int(sys.argv[3] if len(sys.argv) > 3 else 40), 4):
            print(annotate(a, refs_by_addr))
    elif cmd == 'dump':
        # dump <start> <end> <out name>: compact listing (no raw words) to tools_re/out/<name>.txt
        start, end = int(sys.argv[2], 16), int(sys.argv[3], 16)
        lines = []
        for a in range(start, end, 4):
            w = word(a)
            text = disasm_one(a)
            note = ''
            if w >> 26 == 3:
                note = f'   ; call {(w & 0x3ffffff) << 2:#x}'
            elif w >> 26 == 0x0f and (w & 0xffff) >= 0x3000:  # lui: maybe the top half of a float
                f = struct.unpack('<f', struct.pack('<I', (w & 0xffff) << 16))[0]
                if 1e-6 < abs(f) < 1e9:
                    note = f'   ; {f:g}f?'
            if a in refs_by_addr:
                target = refs_by_addr[a]
                note = f'   ; -> {target:#x}' + (f' "{cstring(target)}"' if is_string(target) else '')
                if not is_string(target) and (word(a) >> 26) in (0x09, 0x0d):
                    f = struct.unpack('<f', struct.pack('<I', target))[0]
                    if 1e-6 < abs(f) < 1e9 and target > 0x200000:
                        note += f' = {f:g}f'
            if w == 0x03e00008:
                note += '   ; ---- return'
            lines.append(f'{a:06x}  {text}{note}')
        path = os.path.join(HERE, 'out', sys.argv[4] + '.txt')
        open(path, 'w').write('\n'.join(lines) + '\n')
        print(f'wrote {path} ({len(lines)} lines)')
    elif cmd == 'func':
        start, end = function_bounds(int(sys.argv[2], 16))
        print(f'function {start:#x}..{end:#x} ({(end - start) // 4} instructions)')
        for a in range(start, end, 4):
            print(annotate(a, refs_by_addr))
    elif cmd == 'calls':
        want = int(sys.argv[2], 16)
        enc = 0x0c000000 | (want >> 2)
        for a in range(0, TEXT_END, 4):
            if word(a) == enc:
                print(f'{a:#08x} (function {function_bounds(a)[0]:#08x})')


if __name__ == '__main__':
    main()
