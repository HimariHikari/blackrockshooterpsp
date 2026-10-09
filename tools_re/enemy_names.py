"""Lists the executable's table of enemy archives: 8-byte entries {enemy number, name
pointer} from 0x169AB8, as searched by the code at 0x9308C.

Writes tools_re/out/enemy_names.txt, and with `rust` as argument prints the table as Rust
source for crates/brs-game/src/battle/enemy_table.rs.
"""
import os, sys

import mips

HERE = os.path.dirname(os.path.abspath(__file__))
TABLE = 0x169ab8


def entries():
    index = 0
    while True:
        number, pointer = mips.word(TABLE + index * 8), mips.word(TABLE + index * 8 + 4)
        if not mips.is_string(pointer):
            return
        yield number, mips.cstring(pointer)
        index += 1


if len(sys.argv) > 1 and sys.argv[1] == 'rust':
    for number, name in entries():
        print(f'    ({number}, "{name.upper()}"),')
else:
    lines = [f'{number:4d}  {name}' for number, name in entries()]
    out = os.path.join(HERE, 'out', 'enemy_names.txt')
    open(out, 'w').write('\n'.join(lines) + '\n')
    print(f'wrote {out} ({len(lines)} lines)')
