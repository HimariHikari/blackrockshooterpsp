"""Pictures of a run of the game at several times, taken by the game with no window and
no sound, and tiled into one sheet.

usage: python field_sheet.py <name> <columns> <seconds>... -- <options for the game>

e.g.   python field_sheet.py opening 4 6 9 12 15 -- --map STG01_01 --play-events --skip-messages

The pictures (`<name>_N.png`) and the sheet (`<name>_sheet.png`) are put in
`out/fx/field`; the game's log is `<name>.log` there, and the lines of it that tell of
effects, warnings and errors are printed. `--mute`, `--verbose` and `--saves` are given
for the run. Build the game first (`cargo build -p brs-game`).
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
GAME = os.path.join(ROOT, 'target', 'debug', 'brs-game.exe')
NO_WINDOW = 0x08000000
TOLD = ('is made of the effect', 'is not made', 'WARN', 'ERROR', 'panicked', 'no effect')


def main():
    arguments = sys.argv[1:]
    if '--' not in arguments or arguments.index('--') < 3:
        sys.exit(__doc__)
    at = arguments.index('--')
    arguments, more = arguments[:at], arguments[at + 1:]
    name, columns, times = arguments[0], arguments[1], [float(value) for value in arguments[2:]]
    folder = os.path.join(HERE, 'out', 'fx', 'field')
    os.makedirs(folder, exist_ok=True)
    for old in os.listdir(folder):
        if old.startswith(name + '_') and old.endswith('.png'):
            os.remove(os.path.join(folder, old))
    run = [GAME, '--mute', '--verbose', '--saves', os.path.join(HERE, 'out', '_saves'), '--screenshot', os.path.join(folder, name + '.png'),
           '--screenshot-at', ','.join(f'{time:.3f}' for time in times)] + more
    done = subprocess.run(run, capture_output=True, text=True, encoding='utf-8', errors='replace', creationflags=NO_WINDOW, timeout=600, cwd=ROOT)
    with open(os.path.join(folder, name + '.log'), 'w', encoding='utf-8') as log:
        log.write(done.stdout + done.stderr)
    for line in (done.stdout + done.stderr).splitlines():
        if any(word in line for word in TOLD):
            print(line[-190:])
    pictures = [os.path.join(folder, f'{name}_{index + 1}.png') for index in range(len(times))]
    if len(times) == 1:
        pictures = [os.path.join(folder, name + '.png')]
    pictures = [picture for picture in pictures if os.path.exists(picture)]
    out = os.path.join(folder, name + '_sheet.png')
    subprocess.run([sys.executable, os.path.join(HERE, 'montage.py'), out, columns, 'full@0.375'] + pictures, check=True)


main()
