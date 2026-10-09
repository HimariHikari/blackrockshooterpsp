"""Search the game executable for strings. Exploration only.

Usage: exe_strings.py <regex> [<regex>...]   (case-insensitive, matched against each printable string)
"""
import os, re, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.path.join(HERE, '..', 'game', 'extracted', 'PSP_GAME', 'SYSDIR', 'EBOOT.OLD')

d = open(EXE, 'rb').read()
strings = [(m.start(), m.group().decode('latin1')) for m in re.finditer(rb'[\x20-\x7e]{3,}', d)]
print(f'{len(d):#x} bytes, {len(strings)} strings')
for pattern in sys.argv[1:]:
    rx = re.compile(pattern, re.I)
    hits = [(o, s) for o, s in strings if rx.search(s)]
    print(f'== /{pattern}/ : {len(hits)} hits')
    for o, s in hits[:60]:
        print(f'   {o:#08x}  {s[:100]}')
