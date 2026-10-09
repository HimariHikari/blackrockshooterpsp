"""Puts a full-screen picture (`IF/PICT/*.PTM`) together as the game draws it (0x2DE94):
the texture is 512 x 256; its left 480 columns are the top 256 rows of the screen, and
the bottom 16 rows are kept on their side in the 32 columns left over, as two strips
of 16 x 240 that are drawn turned a quarter turn at the screen's foot.

Usage: pict_screen.py <name, e.g. TUT_PIT002> <out.png> [a|b]
       (a and b are the two ways the strips could be turned; a is the default)
"""
import os, sys

from PIL import Image

import ptm_png

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return
    name, out = sys.argv[1], sys.argv[2]
    way = sys.argv[3] if len(sys.argv) > 3 else 'a'
    with open(os.path.join(ptm_png.ROOT, 'IF', 'PICT', f'{name.upper()}.PTM.ZZZ'), 'rb') as f:
        b = f.read()
    w, h, rgba, _ = ptm_png.decode(b, b.index(b'PTMD'))
    texture = Image.frombytes('RGBA', (w, h), rgba)
    screen = Image.new('RGBA', (480, 272))
    screen.paste(texture.crop((0, 0, 480, 256)), (0, 0))
    for index, left in enumerate((480, 496)):
        strip = texture.crop((left, 0, left + 16, 240))
        turned = strip.rotate(90 if way == 'a' else -90, expand=True)
        screen.paste(turned, (index * 240, 256))
    screen.save(out)
    print(f'{out}: {w} x {h} texture')


if __name__ == '__main__':
    main()
