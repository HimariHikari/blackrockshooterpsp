"""Prints the colours at some places of some pictures, a picture a line: for
following a place through a run of frames (the map coming back from its dimming
after a skill, a fade, a panel coming in).

Usage: pixels.py <x,y;x,y;...> <picture>...

The places are in the pictures' own pixels. `measure.py` gives the mean colours of
areas of two frames instead.
"""
import os, sys

from PIL import Image

if len(sys.argv) < 3:
    sys.exit(__doc__)
points = [tuple(int(v) for v in point.split(',')) for point in sys.argv[1].split(';')]
for name in sys.argv[2:]:
    image = Image.open(name).convert('RGB')
    print(os.path.basename(name).ljust(22), ' | '.join(','.join(str(v) for v in image.getpixel(point)) for point in points))
