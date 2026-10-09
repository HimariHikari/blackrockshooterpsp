"""Puts two pictures side by side with a label over each, to compare a frame of the
walkthrough with the port's picture of the same moment. Exploration only.

Usage: side_by_side.py <out.png> <left picture> <left label> <right picture> <right label>
"""
import sys

from PIL import Image, ImageDraw

WIDTH, HEIGHT, GAP, TOP = 640, 360, 12, 28

out, left, left_label, right, right_label = sys.argv[1:6]
sheet = Image.new('RGB', (WIDTH * 2 + GAP, HEIGHT + TOP), (24, 24, 28))
draw = ImageDraw.Draw(sheet)
for index, (path, label) in enumerate(((left, left_label), (right, right_label))):
    picture = Image.open(path).convert('RGB').resize((WIDTH, HEIGHT), Image.LANCZOS)
    x = index * (WIDTH + GAP)
    sheet.paste(picture, (x, TOP))
    draw.text((x + 8, 8), label, fill=(230, 230, 230))
sheet.save(out)
print(out, sheet.size)
