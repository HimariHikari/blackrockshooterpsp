"""Find instructions by opcode and immediate in the executable's code. Exploration only.

Usage: findins.py <mnemonic> <imm hex> [...]
  mnemonics: addiu ori andi slti sltiu xori lui lb lbu lh lhu lw sb sh sw lwc1 swc1
Prints every address whose instruction has that opcode and 16-bit immediate, grouped so
that clusters (many hits close together) stand out.
"""
import os, struct, sys

EXE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'game', 'extracted',
                   'PSP_GAME', 'SYSDIR', 'EBOOT.OLD')
d = open(EXE, 'rb').read()
BASE = 0xC0
TEXT_END = 0x164520
OPS = {'addiu': 0x09, 'ori': 0x0D, 'andi': 0x0C, 'slti': 0x0A, 'sltiu': 0x0B, 'xori': 0x0E, 'lui': 0x0F,
       'lb': 0x20, 'lbu': 0x24, 'lh': 0x21, 'lhu': 0x25, 'lw': 0x23, 'sb': 0x28, 'sh': 0x29, 'sw': 0x2B,
       'lwc1': 0x31, 'swc1': 0x39}

args = sys.argv[1:]
for i in range(0, len(args), 2):
    op = OPS[args[i]]
    imm = int(args[i + 1], 16) & 0xFFFF
    hits = [va for va in range(0, TEXT_END, 4)
            if (w := struct.unpack_from('<I', d, va + BASE)[0]) >> 26 == op and (w & 0xFFFF) == imm]
    print(f'{args[i]} {imm:#x}: {len(hits)} hits')
    line = []
    for h in hits:
        if line and h - line[-1] > 0x400:
            print('   ' + ' '.join(f'{x:#x}' for x in line))
            line = []
        line.append(h)
    if line:
        print('   ' + ' '.join(f'{x:#x}' for x in line))
