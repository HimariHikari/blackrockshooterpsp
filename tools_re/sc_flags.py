"""Surveys the low four bits of the slot offsets of every SC bundle in the game data.

An SC bundle's slots are 16-byte aligned; the low four bits of a slot's offset are not
part of the offset but the number of padding bytes at the end of that slot (the code at
0xE7064 subtracts them from the slot's span to get its size). This walks every bundle,
nested ones included, and reports which slots carry such a count and what they hold.

Usage: sc_flags.py   -> tools_re/out/sc_flags.txt
"""
import gzip, os, struct, zlib

import rtdp

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', 'sc_flags.txt')


def unwrap(d):
    if d[:3] == b'\x1f\x8b\x08':
        return gzip.decompress(d)
    if d[:4] == b'CZAA' and len(d) > 16:
        return zlib.decompress(d[16:])
    return d


def table(d):
    """The offsets of an SC bundle (count + 1 of them), or None if it is not one."""
    if d[:2] != b'SC' or len(d) < 8:
        return None
    count = struct.unpack_from('<H', d, 2)[0]
    if count == 0 or 4 + (count + 1) * 4 > len(d):
        return None
    offsets = struct.unpack_from(f'<{count + 1}I', d, 4)
    starts = [o & ~0xF for o in offsets]
    if starts[0] < 4 + (count + 1) * 4 or offsets[count] != len(d) or any(a > b for a, b in zip(starts, starts[1:])):
        return None
    return offsets


def magic(d):
    head = d[:4]
    return head.decode('latin1') if head and all(32 <= b < 127 for b in head) else head.hex()


def walk(d, label, kind, stats, odd):
    offsets = table(d)
    if offsets is None:
        if d[:2] == b'SC':
            odd.append(f'{label}: starts SC but its table does not fit ({len(d):#x} bytes, words '
                       + ' '.join(f'{w:#x}' for w in struct.unpack_from("<6I", d.ljust(28, b"\0"), 4)) + ')')
        return
    count = len(offsets) - 1
    for i in range(count):
        start, end, pad = offsets[i] & ~0xF, offsets[i + 1] & ~0xF, offsets[i] & 0xF
        if end == start:
            if pad:
                odd.append(f'{label}/{i}: empty slot with padding {pad}')
            continue
        body = d[start:end - pad]
        key = (kind, count, i, pad, magic(body))
        entry = stats.setdefault(key, [0, label])
        entry[0] += 1
        if pad and any(d[end - pad:end]):
            odd.append(f'{label}/{i}: the {pad} padding bytes are not zero: {d[end - pad:end].hex()}')
        walk(body, f'{label}/{i}', kind + f'.{i}', stats, odd)


def main():
    stats, odd = {}, []
    for folder, _, names in os.walk(rtdp.DATA):
        for name in sorted(names):
            path = os.path.join(folder, name)
            rel = os.path.relpath(path, rtdp.DATA)
            d = open(path, 'rb').read()
            if d[:4] == b'RTDP':
                for member, start, size in rtdp.members(d):
                    blob = unwrap(bytes(b ^ rtdp.KEY for b in d[start:start + size]))
                    walk(blob, f'{rel}:{member}', f'{rel}:{member}', stats, odd)
                continue
            try:
                blob = unwrap(d)
            except Exception as e:  # not really wrapped
                continue
            walk(blob, rel, rel.split(os.sep)[0] + '/' + (rel.split(os.sep)[1] if os.sep in rel else ''), stats, odd)
    lines = ['kind, slots in the bundle, slot, padding, what it holds: how many (an example)']
    for (kind, count, slot, pad, what), (n, example) in sorted(stats.items()):
        if pad:
            lines.append(f'{kind:28s} {count:3d} slots  slot {slot:2d}  pad {pad:2d}  {what:8s} x{n:5d}   {example}')
    lines.append('')
    lines.append('slots without padding, by what they hold:')
    plain = {}
    for (kind, count, slot, pad, what), (n, example) in stats.items():
        if not pad:
            plain[what] = plain.get(what, 0) + n
    lines.append('  ' + ', '.join(f'{what} x{n}' for what, n in sorted(plain.items(), key=lambda item: -item[1])))
    lines.append('')
    lines.append(f'{len(odd)} oddities:')
    lines += ['  ' + line for line in odd[:80]]
    with open(OUT, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines) + '\n')
    print(f'wrote {OUT} ({len(lines)} lines)')


main()
