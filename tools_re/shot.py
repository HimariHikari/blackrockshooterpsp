"""Runs one of the workspace's programs for a screenshot, and again if the picture came
out empty: a window that opens minimised or behind something else is captured as a
single pixel.

Usage: shot.py <out.png> <cargo run arguments...>
  e.g. shot.py game/screenshots/a.png -p brs-game -- --battle STG01_01 --script wait:5
"""
import os, subprocess, sys

out, rest = sys.argv[1], sys.argv[2:]
for attempt in range(1, 5):
    if os.path.exists(out):
        os.remove(out)
    subprocess.run(['cargo', 'run', '-q', *rest, '--screenshot', out],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if os.path.exists(out) and os.path.getsize(out) > 20000:
        print(f'ok {out} (try {attempt})')
        break
else:
    print(f'FAILED {out}')
    sys.exit(1)
