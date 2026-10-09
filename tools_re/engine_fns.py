"""Which code each engine function of table 2 runs, from the dump natives2.py made.

Usage: engine_fns.py            the whole list, to tools_re/out/engine_fn_addresses.txt
       engine_fns.py <number>   the dispatcher's code for those functions
"""
import os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.join(HERE, 'out', 'bscr_natives2.txt')


def handlers():
    """Yields (cases, lines) for each handler of the dispatcher."""
    cases, body = None, []
    for line in open(SOURCE, encoding='utf-8-sig').read().splitlines():
        m = re.match(r'=== handler (0x[0-9a-f]+)\s+cases \[([^\]]*)\]', line)
        if m:
            if cases is not None:
                yield cases, body
            cases = [int(c.strip(), 16) for c in m.group(2).split(',') if c.strip()]
            body = [line]
        elif cases is not None:
            body.append(line)
    if cases is not None:
        yield cases, body


def main():
    if len(sys.argv) > 1:
        wanted = {int(a) for a in sys.argv[1:]}
        for cases, body in handlers():
            if wanted & set(cases):
                print(f'--- e{",".join(str(c) for c in cases)}')
                print('\n'.join(body))
        return
    lines = []
    for cases, body in handlers():
        calls = [m.group(1) for line in body for m in [re.search(r'call (0x[0-9a-f]+)', line)] if m]
        arguments = sum(1 for line in body if re.search(r'lw \$\w+, (0x[0-9a-f]+|)\(\$a3\)', line))
        for case in cases:
            lines.append((case, f'e{case:<4d} {arguments} arguments  -> {" ".join(calls) or "(inline)"}'))
    lines.sort()
    out = os.path.join(HERE, 'out', 'engine_fn_addresses.txt')
    open(out, 'w', encoding='utf-8').write('\n'.join(text for _, text in lines) + '\n')
    print(f'wrote {out} ({len(lines)} lines)')


main()
