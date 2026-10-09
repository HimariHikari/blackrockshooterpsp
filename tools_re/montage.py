"""Tile screenshots into one labelled contact sheet, to compare them at a glance.

Usage: montage.py <out.png> <columns> <crop: left,top,right,bottom as fractions | full>[@scale] <png>...

A scale after the crop (`full@0.5`) makes each tile that much smaller.
"""
import os, sys

from PIL import Image, ImageDraw

out, columns, crop = sys.argv[1], int(sys.argv[2]), sys.argv[3]
crop, _, scale = crop.partition('@')
scale = float(scale) if scale else 1.0
paths = sys.argv[4:]
tiles = []
for path in paths:
    image = Image.open(path).convert('RGB')
    if crop != 'full':
        l, t, r, b = (float(v) for v in crop.split(','))
        w, h = image.size
        image = image.crop((int(l * w), int(t * h), int(r * w), int(b * h)))
    if scale != 1.0:
        image = image.resize((max(1, round(image.size[0] * scale)), max(1, round(image.size[1] * scale))), Image.LANCZOS)
    draw = ImageDraw.Draw(image)
    draw.text((6, 4), os.path.splitext(os.path.basename(path))[0], fill=(255, 255, 0))
    tiles.append(image)
w, h = tiles[0].size
rows = (len(tiles) + columns - 1) // columns
sheet = Image.new('RGB', (w * columns, h * rows))
for i, tile in enumerate(tiles):
    sheet.paste(tile, ((i % columns) * w, (i // columns) * h))
sheet.save(out)
print(f'{out}: {sheet.size[0]}x{sheet.size[1]} from {len(tiles)} images')
