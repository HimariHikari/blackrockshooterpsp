"""Checks the port's way of decoding a bank's samples against ffmpeg's: wraps the
samples of a bank in the header ffmpeg knows such data by and compares what it makes
of them with what the same arithmetic as crates/brs-formats/src/pphd.rs gives.

Usage: pphd_check.py <bank header, relative to game/unpacked> [sound numbers...]
"""
import os, struct, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')
FILTERS = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)]


def decode(frames):
    out, last, before = [], 0, 0
    for a in range(0, len(frames) - 15, 16):
        f = frames[a:a + 16]
        filt, shift, flags = f[0] >> 4, f[0] & 15, f[1]
        if flags == 7:
            break
        if shift > 12:
            shift = 9
        k0, k1 = FILTERS[min(filt, 4)]
        for byte in f[2:]:
            for nibble in (byte & 15, byte >> 4):
                delta = ((nibble << 12) - (0x10000 if nibble & 8 else 0)) >> shift
                s = delta + ((last * k0 + before * k1) >> 6)
                s = max(-32768, min(32767, s))
                before, last = last, s
                out.append(s)
        if flags & 1:
            break
    return out


def main():
    head = os.path.join(ROOT, sys.argv[1])
    d = open(head, 'rb').read()
    if head.lower().endswith('.phd'):
        body = head[:-4] + '.pbd'
    else:
        slot = int(os.path.basename(head).split('.')[0])
        body = os.path.join(os.path.dirname(head), f'{slot + 1:02d}.bin')
    b = open(body, 'rb').read()
    tn, va = struct.unpack_from('<II', d, 0x14)
    first, last = struct.unpack_from('<II', d, tn + 0x10)
    wanted = [int(a) for a in sys.argv[2:]] or list(range(first, last + 1))
    worst = 0
    for number in wanted:
        sample, = struct.unpack_from('<I', d, tn + 0x20 + (number - first) * 0x60)
        if sample == 0xffffffff:
            continue
        offset, rate, length, _ = struct.unpack_from('<4I', d, va + 0x20 + sample * 0x10)
        frames = b[offset:offset + length]
        mine = decode(frames)
        vag = b'VAGp' + struct.pack('>IIII', 0x20, 0, len(frames), rate) + bytes(12) + b'check'.ljust(16, b'\0') + frames
        with tempfile.NamedTemporaryFile(suffix='.vag', delete=False) as f:
            f.write(vag)
        try:
            raw = subprocess.run(['ffmpeg', '-v', 'error', '-f', 'vag', '-i', f.name, '-f', 's16le', '-'], capture_output=True).stdout
        finally:
            os.unlink(f.name)
        theirs = list(struct.unpack(f'<{len(raw) // 2}h', raw))
        n = min(len(mine), len(theirs))
        off = max((abs(a - b) for a, b in zip(mine[:n], theirs[:n])), default=0)
        worst = max(worst, off)
        print(f'sound {number}: {len(mine)} samples here, {len(theirs)} from ffmpeg, furthest apart {off}, peak {max(map(abs, mine), default=0)}')
    print('furthest apart over all:', worst)


if __name__ == '__main__':
    main()
