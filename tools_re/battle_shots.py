"""Pictures of a battle at several times, taken by the game with no window and no
sound, and tiled into one sheet.

usage: python battle_shots.py <name> <columns> <seconds>... -- <options for the game>

e.g.   python battle_shots.py items 3 5.2 6 7.9 -- --battle STG01_01 --group 100 --carry 1:3,9:1 --script "wait:5,l:0.1,wait:3"

The pictures (`<name>_N.png`) and the sheet (`<name>_sheet.png`) are put in
`out/fx/battle`; the game's log is `<name>.log` there, and the lines of it that tell
of warnings, errors and what she uses are printed. `--mute`, `--verbose` and `--saves`
are given for the run. Build the game first (`cargo build -p brs-game`).
"""
import os
import subprocess
import sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
GAME = os.path.join(ROOT, 'target', 'debug', 'brs-game.exe')
OUT = os.path.join(HERE, 'out', 'fx', 'battle')
NO_WINDOW = 0x08000000
TOLD = ('she uses', 'WARN', 'ERROR', 'panicked')


def main():
    arguments = sys.argv[1:]
    if '--' not in arguments or arguments.index('--') < 3:
        sys.exit(__doc__)
    at = arguments.index('--')
    arguments, more = arguments[:at], arguments[at + 1:]
    name, columns, times = arguments[0], int(arguments[1]), arguments[2:]
    os.makedirs(OUT, exist_ok=True)
    picture = os.path.join(OUT, f'{name}.png')
    run = [GAME, '--mute', '--verbose', '--saves', os.path.join(HERE, 'out', '_saves'), *more, '--screenshot', picture, '--screenshot-at', ','.join(times)]
    done = subprocess.run(run, capture_output=True, text=True, encoding='utf-8', errors='replace', creationflags=NO_WINDOW, cwd=ROOT, timeout=max(float(t) for t in times) + 120)
    log = done.stdout + done.stderr
    with open(os.path.join(OUT, f'{name}.log'), 'w', encoding='utf-8') as out:
        out.write(log)
    for line in log.splitlines():
        if any(word in line for word in TOLD):
            print(line[:240])
    pictures = [picture] if len(times) == 1 else [os.path.join(OUT, f'{name}_{n + 1}.png') for n in range(len(times))]
    tiles = [Image.open(path).convert('RGB').resize((640, 360)) for path in pictures if os.path.exists(path)]
    if not tiles:
        sys.exit(f'no picture was taken (exit {done.returncode})')
    rows = (len(tiles) + columns - 1) // columns
    sheet = Image.new('RGB', (640 * columns, 360 * rows))
    for index, tile in enumerate(tiles):
        sheet.paste(tile, ((index % columns) * 640, (index // columns) * 360))
    sheet.save(os.path.join(OUT, f'{name}_sheet.png'))
    print(f'{len(tiles)} pictures, exit {done.returncode}')


if __name__ == '__main__':
    main()
