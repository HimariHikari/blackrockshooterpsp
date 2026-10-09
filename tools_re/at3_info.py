"""What an ATRAC3plus recording says of itself: its format, its length in samples,
where it loops, and with `--decode` how many samples ffmpeg makes of it.

A recording is a RIFF file: `fmt ` (the extensible kind, with the codec's GUID),
`fact` (the samples it has, and for ATRAC3plus the samples the decoder gives before
the first of them), `smpl` (a loop: its first and last sample) and `data`.

Usage: at3_info.py [--decode] <file under game/unpacked, or a path> ...
"""
import os, struct, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')


def chunks(d):
    at = 12
    while at + 8 <= len(d):
        name, size = d[at:at + 4], struct.unpack_from('<I', d, at + 4)[0]
        yield name, d[at + 8:at + 8 + size]
        at += 8 + size + (size & 1)


def info(d):
    found = {}
    for name, body in chunks(d):
        if name == b'fmt ':
            tag, channels, rate, byte_rate, block = struct.unpack_from('<HHIIH', body, 0)
            found['format'] = f'tag {tag:#x}, {channels} ch, {rate} Hz, {byte_rate * 8 // 1000} kbit/s, block {block}'
            found['rate'] = rate
        elif name == b'fact':
            words = struct.unpack_from(f'<{len(body) // 4}I', body, 0)
            found['fact'] = words
        elif name == b'smpl':
            loops, = struct.unpack_from('<I', body, 28)
            found['loops'] = [struct.unpack_from('<II', body, 36 + i * 24 + 8) for i in range(loops)]
        elif name == b'data':
            found['data'] = len(body)
    return found


def main():
    arguments = sys.argv[1:]
    decode = '--decode' in arguments
    for name in [a for a in arguments if a != '--decode']:
        path = name if os.path.exists(name) else os.path.join(ROOT, name)
        d = open(path, 'rb').read()
        if d[:4] != b'RIFF':
            print(f'{name}: not a RIFF file')
            continue
        found = info(d)
        line = f'{os.path.basename(name)}: {found.get("format")}; fact {found.get("fact")}; loops {found.get("loops", [])}; data {found.get("data")}'
        if decode:
            pcm = subprocess.run(['ffmpeg', '-v', 'error', '-i', path, '-f', 's16le', '-ac', '2', '-'],
                                 capture_output=True).stdout
            line += f'; ffmpeg gives {len(pcm) // 4} samples'
        print(line)


if __name__ == '__main__':
    main()
