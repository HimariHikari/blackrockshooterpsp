"""Says how far pictures of the same size are from each other, a pair at a time: how
many of their pixels differ at all, by more than 2 and by more than 10 in a channel,
the worst difference, and the box the differences of more than 2 lie in.

Usage: picdiff.py <a.png> <b.png> [<a.png> <b.png> ...]

For checking that a change to how something is drawn left a picture alone. Two
unseen runs of the same build are not the same picture: what moves is caught a
frame earlier or later, so 2 to 3 percent of a battle's pixels differ from run to
run. Take two runs of the old build first, and hold the new one against that.
"""
import sys

from PIL import Image, ImageChops

pairs = sys.argv[1:]
if not pairs or len(pairs) % 2:
    sys.exit(__doc__)
for first, second in zip(pairs[0::2], pairs[1::2]):
    a, b = Image.open(first).convert('RGB'), Image.open(second).convert('RGB')
    if a.size != b.size:
        print(f'{first} {a.size} and {second} {b.size} differ in size')
        continue
    diff = ImageChops.difference(a, b)
    worst = max(band.getextrema()[1] for band in diff.split())
    # The greatest of the three channels' differences at each pixel.
    red, green, blue = diff.split()
    greatest = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    histogram = greatest.histogram()
    total = a.size[0] * a.size[1]
    over = lambda limit: 100 * sum(histogram[limit + 1:]) / total
    box = greatest.point(lambda value: 255 if value > 2 else 0).getbbox()
    print(f'{first} vs {second}: {over(0):.3f}% differ, {over(2):.3f}% by more than 2, '
          f'{over(10):.3f}% by more than 10, worst {worst}, box of those over 2: {box}')
