"""Pictures of the scene of one of her skills, taken by the game with no window and no
sound, and tiled into one sheet.

usage: python scene_sheet.py <skill> <columns> <sixtieths into the scene>... [-- more options for the game]

The battle is the game's first (`--battle STG01_01 --group 2`); the skill is on triangle
and is used six seconds in, so that its scene begins 7.033 s in (the game's log says
when: "her skill N takes the stage"). The pictures and the sheet (`skillN_sheet.png`)
are put in `out/fx/scene`. Build the game first (`cargo build -p brs-game`).
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
GAME = os.path.join(ROOT, 'target', 'debug', 'brs-game.exe')
BEGINS = 7.033
NO_WINDOW = 0x08000000


def main():
    arguments = sys.argv[1:]
    if len(arguments) < 3:
        sys.exit(__doc__)
    more = []
    if '--' in arguments:
        at = arguments.index('--')
        arguments, more = arguments[:at], arguments[at + 1:]
    skill, columns, times = arguments[0], arguments[1], [float(value) for value in arguments[2:]]
    folder = os.path.join(HERE, 'out', 'fx', 'scene')
    os.makedirs(folder, exist_ok=True)
    name = f'skill{skill}'
    for old in os.listdir(folder):
        if old.startswith(name + '_') and old.endswith('.png'):
            os.remove(os.path.join(folder, old))
    # A little past each time, so that the frame of the game's logic it names has been drawn.
    at = ','.join(f'{BEGINS + time / 60.0 + 0.017:.3f}' for time in times)
    last = BEGINS + max(times) / 60.0 + 3.0
    run = [GAME, '--battle', 'STG01_01', '--group', '2', '--skills', f'{skill},0,0,0', '--mute', '--saves', os.path.join(HERE, 'out', '_saves'),
           '--script', f'wait:6,r+triangle:0.2,wait:{last:.1f}', '--screenshot', os.path.join(folder, name + '.png'), '--screenshot-at', at] + more
    done = subprocess.run(run, capture_output=True, text=True, creationflags=NO_WINDOW, timeout=300, cwd=ROOT)
    for line in (done.stdout + done.stderr).splitlines():
        if any(word in line for word in ('takes the stage', 'WARN', 'ERROR', 'panicked', 'no effect')):
            print(line[-200:])
    pictures = [os.path.join(folder, f'{name}_{index + 1}.png') for index in range(len(times))]
    if len(times) == 1:
        pictures = [os.path.join(folder, name + '.png')]
    pictures = [picture for picture in pictures if os.path.exists(picture)]
    out = os.path.join(folder, name + '_sheet.png')
    subprocess.run([sys.executable, os.path.join(HERE, 'montage.py'), out, columns, 'full@0.375'] + pictures, check=True)


main()
