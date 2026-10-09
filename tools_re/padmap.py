"""Print the battle input mapping table (0x16A3C4: 12 pairs of game bit, PSP button mask)."""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mips

NAMES = {0x1: 'SELECT', 0x8: 'START', 0x10: 'UP', 0x20: 'RIGHT', 0x40: 'DOWN', 0x80: 'LEFT',
         0x100: 'L', 0x200: 'R', 0x1000: 'TRIANGLE', 0x2000: 'CIRCLE', 0x4000: 'CROSS',
         0x8000: 'SQUARE'}
for i in range(12):
    game = mips.word(0x16A3C4 + i * 8)
    psp = mips.word(0x16A3C4 + i * 8 + 4)
    print(f'game bit {game:#06x} <- PSP {psp:#06x} {NAMES.get(psp, "?")}')
