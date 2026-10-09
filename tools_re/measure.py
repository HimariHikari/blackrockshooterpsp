"""Measures how far two pictures of the same moment are apart in colour: the mean
colour of the same areas of each, side by side. For a frame of the port against a
frame of the walkthrough; a difference that grows with distance is fog, one that
is everywhere is how the colours are mixed. Exploration only.

Usage: measure.py <port frame> <walkthrough frame> [x0,y0,x1,y1 ...]

An area is given in parts of the picture's width and height (0.3,0.75,0.7,0.95);
without any, a grid of four by three is measured. The pictures need not be of one
size.
"""
import sys

from PIL import Image, ImageStat


def mean(image, box):
    width, height = image.size
    area = image.crop((int(width * box[0]), int(height * box[1]), int(width * box[2]), int(height * box[3])))
    return [round(value) for value in ImageStat.Stat(area).mean]


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return
    port, walkthrough = (Image.open(path).convert('RGB') for path in sys.argv[1:3])
    boxes = [tuple(float(part) for part in area.split(',')) for area in sys.argv[3:]]
    if not boxes:
        boxes = [(x / 4, y / 3, (x + 1) / 4, (y + 1) / 3) for y in range(3) for x in range(4)]
    worst = 0
    for box in boxes:
        ours, theirs = mean(port, box), mean(walkthrough, box)
        apart = max(abs(a - b) for a, b in zip(ours, theirs))
        worst = max(worst, apart)
        print(f'{",".join(f"{part:.2f}" for part in box)}  port {ours}  walkthrough {theirs}  apart {apart}')
    print('the furthest apart:', worst)


if __name__ == '__main__':
    main()
