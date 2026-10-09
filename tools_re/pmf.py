"""Looks into a film (`MOVIE/*.PMF`): the PSMF header, the packs of the program stream
after it and the streams their packets carry. Exploration only.

Usage: pmf.py <file> [packs to list]
       pmf.py <file> --audio <out.at3>    writes the sound as a RIFF the way the game's
                                          other ATRAC3plus recordings are, for ffmpeg
       pmf.py <file> --video <out.h264>   writes the picture's H.264 stream as it is

A PSMF file is a header of 0x800 bytes ("PSMF", a version, where the stream begins and
how long it is, the first and last time stamps, and a table of its streams) and then an
MPEG program stream in packs of 0x800 bytes. The picture is stream 0xE0, H.264; the
sound is private stream 1 (0xBD), sub-stream 0, ATRAC3plus in frames that each begin
with 0F D0 and two bytes that say how long the frame is.
"""
import collections
import struct
import sys

PACK = 0x800


def header(data):
    magic, version = data[:4], data[4:8]
    start, size = struct.unpack_from('>II', data, 8)
    first = int.from_bytes(data[0x54:0x5a], 'big')
    last = int.from_bytes(data[0x5a:0x60], 'big')
    return magic, version, start, size, first, last


def stamp(b):
    """A time stamp of five bytes, in 90000ths of a second."""
    return ((b[0] >> 1) & 7) << 30 | b[1] << 22 | (b[2] >> 1) << 15 | b[3] << 7 | b[4] >> 1


def packets(data, start):
    """Each packet of the program stream: (offset, stream id, time stamp or None, payload)."""
    at = start
    while at + 4 <= len(data):
        if data[at:at + 3] != b'\x00\x00\x01':
            at += 1
            continue
        code = data[at + 3]
        if code == 0xba:      # a pack's header: 14 bytes and its stuffing
            at += 14 + (data[at + 13] & 7)
            continue
        if code == 0xb9:
            break
        length = struct.unpack_from('>H', data, at + 4)[0]
        body = data[at + 6:at + 6 + length]
        if code in (0xbb, 0xbe, 0xbf):      # system header, padding, private stream 2
            yield at, code, None, body
        else:
            flags, extra = body[1], body[2]
            pts = stamp(body[3:8]) if flags & 0x80 else None
            yield at, code, pts, body[3 + extra:]
        at += 6 + length


def frames_of_sound(data, start):
    """The ATRAC3plus frames of sub-stream 0, each without its eight bytes of header."""
    stream = bytearray()
    for _, code, _, body in packets(data, start):
        if code == 0xbd and body and body[0] == 0:
            stream += body[4:]
    at, frames = 0, []
    while at + 8 <= len(stream):
        if stream[at] != 0x0f or stream[at + 1] != 0xd0:
            at += 1
            continue
        size = ((stream[at + 2] & 3) << 8 | stream[at + 3]) * 8 + 8
        frames.append(bytes(stream[at + 8:at + 8 + size]))
        at += 8 + size
    return frames, stream[:16]


def riff(frames, channels=2, rate=44100):
    """The frames as a RIFF WAVE of ATRAC3plus (the extensible format, with its GUID)."""
    block = len(frames[0])
    guid = bytes.fromhex('bfaa23e958cb7144a119fffa01e4ce62')
    fmt = struct.pack('<HHIIHHHHI', 0xfffe, channels, rate, rate * block // 2048, block, 0, 34, 2048, 3 if channels == 2 else 4)
    fmt += guid + struct.pack('<HH', 1, 0x0028 if block == 0x2e8 else 0x5c28) + b'\x00' * 8
    fact = struct.pack('<II', len(frames) * 2048, 2048)
    body = b''.join(frames)
    chunks = b'WAVE' + b'fmt ' + struct.pack('<I', len(fmt)) + fmt + b'fact' + struct.pack('<I', len(fact)) + fact + b'data' + struct.pack('<I', len(body)) + body
    return b'RIFF' + struct.pack('<I', len(chunks)) + chunks


def main():
    path = sys.argv[1]
    data = open(path, 'rb').read()
    magic, version, start, size, first, last = header(data)
    if '--audio' in sys.argv:
        frames, head = frames_of_sound(data, start)
        sizes = collections.Counter(len(frame) for frame in frames)
        print(f'{len(frames)} frames of sound, sizes {dict(sizes)}; the stream begins {head.hex()}')
        open(sys.argv[sys.argv.index('--audio') + 1], 'wb').write(riff(frames))
        return
    if '--video' in sys.argv:
        out = open(sys.argv[sys.argv.index('--video') + 1], 'wb')
        for _, code, _, body in packets(data, start):
            if code == 0xe0:
                out.write(body)
        return
    print(f'{magic} {version}: the stream is from {start:#x}, {size:#x} long (file {len(data):#x}); time {first} to {last} ({(last - first) / 90000:.3f} s)')
    print('the table of streams:', data[0x50:0xa0].hex())
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 12
    seen = collections.Counter()
    sizes = collections.Counter()
    for index, (at, code, pts, body) in enumerate(packets(data, start)):
        seen[code] += 1
        sizes[code] += len(body)
        if index < count:
            when = '' if pts is None else f' at {pts / 90000:.4f} s'
            print(f'  {at:#08x} stream {code:#04x} {len(body):4} bytes{when}: {body[:12].hex()}')
    for code in sorted(seen):
        print(f'stream {code:#04x}: {seen[code]} packets, {sizes[code]} bytes')


if __name__ == '__main__':
    main()
