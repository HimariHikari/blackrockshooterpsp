"""Where a string of the executable is, and what points at it: code that builds its
address (lui + addiu/ori) and words of data that hold it.

Usage: strref.py <text> [<text> ...]
"""
import struct, sys

import mips


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    refs = mips.load_refs()
    for text in sys.argv[1:]:
        raw = text.encode('latin1')
        start = 0
        while True:
            at = mips.DATA.find(raw, start)
            if at < 0:
                break
            start = at + 1
            vaddr = at - mips.FILE_BIAS
            # Only whole strings: the byte before is a terminator.
            if mips.DATA[at - 1] != 0:
                continue
            print(f'{vaddr:#08x} "{mips.cstring(vaddr)}"')
            for where, target in refs:
                if target == vaddr:
                    print(f'    code {where:#08x} (function {mips.function_bounds(where)[0]:#08x})')
            word = struct.pack('<I', vaddr)
            found = mips.TEXT_END + mips.FILE_BIAS
            while True:
                found = mips.DATA.find(word, found)
                if found < 0:
                    break
                if (found - mips.FILE_BIAS) % 4 == 0:
                    print(f'    data {found - mips.FILE_BIAS:#08x}')
                found += 1


if __name__ == '__main__':
    main()
