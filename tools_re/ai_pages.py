"""Which states each AI table of a decompiled enemy script can reach, with weights.

usage: ai_pages.py <dec_*.txt>

Reads the output of bscr_dec.py.  Page 0 of every AI draws a random number and
steps to another page by thresholds; each page asks for one or more states.
"""
import re
import sys
from collections import defaultdict


def functions(lines):
    name, body = None, []
    for line in lines:
        m = re.match(r'^([A-Za-z_0-9]+)\(\)\s+# type', line)
        if m:
            if name:
                yield name, body
            name, body = m.group(1), []
        elif name:
            body.append(line.rstrip('\n'))
    if name:
        yield name, body


def main():
    lines = open(sys.argv[1], encoding='utf-8').read().splitlines()
    pages = defaultdict(dict)       # ai -> page -> [states]
    draws = {}                      # ai -> (sides, [(threshold, step)], {step: page})
    for name, body in functions(lines):
        m = re.match(r'_ai_(\d+)_page_(\d+)_update_main', name)
        if not m:
            continue
        ai, page = int(m.group(1)), int(m.group(2))
        states = []
        for line in body:
            s = re.search(r'change_state_AI\((\d+)\)', line)
            if s:
                states.append(int(s.group(1)))
        pages[ai][page] = states
        if page != 0:
            continue
        sides, thresholds, step_page = None, [], {}
        step = None
        for i, line in enumerate(body):
            d = re.search(r'g45 = e13_random\((\d+)\)', line)
            if d:
                sides = int(d.group(1))
            t = re.search(r'if not \(\(g45 < (\d+)\)\) goto', line)
            if t:
                nxt = re.search(r'g42 = (\d+)', body[i + 1])
                thresholds.append((int(t.group(1)), int(nxt.group(1))))
            c = re.search(r'if not \(\((\d+) == g42\)\) goto', line)
            if c:
                step = int(c.group(1))
            p = re.search(r'g41 = (\d+)', line)
            if p and step is not None and step not in step_page:
                step_page[step] = int(p.group(1))
        draws[ai] = (sides, thresholds, step_page)

    for ai in sorted(pages):
        sides, thresholds, step_page = draws.get(ai, (None, [], {}))
        print(f'AI {ai}: random({sides})')
        low = 0
        for threshold, step in thresholds:
            weight = max(0, threshold - low)
            low = max(low, threshold)
            page = step_page.get(step)
            if weight == 0:
                continue
            states = pages[ai].get(page, [])
            print(f'   {weight}/{sides}  page {page}  states {states}')
        unreached = sorted(set(pages[ai]) - {0} - {step_page.get(s) for t, s in thresholds if True})
        print(f'   all pages: ' + ', '.join(f'{p}:{pages[ai][p]}' for p in sorted(pages[ai])))


main()
