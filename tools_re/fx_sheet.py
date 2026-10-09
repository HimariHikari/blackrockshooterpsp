"""Pictures of effects, taken by the viewer with no window and tiled into one sheet.

usage: python fx_sheet.py <out.png> <columns> <scale> <effect file> <number>@<frame>[/<frame>...] ...
       python fx_sheet.py <out.png> <columns> <scale> <effect file> all@<frame>

The file is given from the project's folder (or in full); `all` takes every effect of
it. More options for the viewer go after `--` (as `-- --yaw 0 --distance 2`). The
pictures themselves are kept beside the sheet, in a folder of its name.
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
VIEWER = os.path.join(ROOT, 'target', 'debug', 'brs-viewer.exe')
NO_WINDOW = 0x08000000


def viewer(arguments):
    done = subprocess.run([VIEWER] + arguments, capture_output=True, text=True, creationflags=NO_WINDOW, timeout=180)
    return done.stdout + done.stderr


def main():
    arguments = sys.argv[1:]
    more = []
    if '--' in arguments:
        at = arguments.index('--')
        arguments, more = arguments[:at], arguments[at + 1:]
    out, columns, scale, file = arguments[0], arguments[1], arguments[2], arguments[3]
    file = file if os.path.isabs(file) else os.path.join(ROOT, file)
    folder = os.path.splitext(out)[0]
    os.makedirs(folder, exist_ok=True)
    wanted = []
    for what in arguments[4:]:
        number, _, frames = what.partition('@')
        frames = frames.split('/') if frames else ['1']
        if number == 'all':
            log = viewer([file, '--frame', '1', '--screenshot', os.path.join(folder, '_list.png')])
            found = re.search(r'effects: \[([0-9, ]*)\]', log)
            numbers = found.group(1).replace(' ', '').split(',') if found else []
        else:
            numbers = [number]
        wanted += [(number, frame) for number in numbers for frame in frames]
    pictures = []
    for number, frame in wanted:
        picture = os.path.join(folder, f'{number}_{frame}.png')
        log = viewer([file, '--effect', number, '--frame', frame, '--screenshot', picture] + more)
        held = re.search(r'held .*', log)
        print(number, frame, held.group(0) if held else log[-300:])
        if os.path.exists(picture):
            pictures.append(picture)
    if pictures:
        subprocess.run([sys.executable, os.path.join(HERE, 'montage.py'), out, columns, f'full@{scale}'] + pictures, check=True)


main()
