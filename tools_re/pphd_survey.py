"""Surveys every sound bank under game/unpacked: the values the tones' fields take,
the samples' rates, and the flags of the samples' frames (which say where one ends
and loops). Exploration only.
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

FIELDS = {0x04: 'w4', 0x08: 'w8', 0x0c: 'wC', 0x10: 'w10', 0x14: 'w14', 0x20: 'vol?20', 0x24: 'pan?24', 0x28: 'w28', 0x2c: 'w2c',
          0x30: 'note30', 0x34: 'w34', 0x38: 'w38', 0x40: 'w40', 0x44: 'w44', 0x48: 'w48', 0x50: 'adsr1', 0x54: 'adsr2'}


def banks():
    for path, _, names in os.walk(ROOT):
        for name in names:
            lower = name.lower()
            if lower.endswith('.phd'):
                yield os.path.join(path, name), os.path.join(path, name[:-4] + '.pbd')
            elif lower.endswith('.pphd'):
                # The data is the slot after the header's.
                number = int(name.split('.')[0])
                body = [n for n in names if n.split('.')[0] == f'{number + 1:02d}']
                yield os.path.join(path, name), os.path.join(path, body[0]) if body else None


def main():
    values = {name: collections.Counter() for name in FIELDS.values()}
    rates, flags, unused_words = collections.Counter(), collections.Counter(), collections.Counter()
    count = tones = samples = empty = missing_body = no_sample = 0
    first_tone, loops = collections.Counter(), 0
    seconds = 0.0
    for head, body in banks():
        d = open(head, 'rb').read()
        if d[:5] != b'PPHD8':
            print('not a bank:', head)
            continue
        count += 1
        pg, tn, va = struct.unpack_from('<III', d, 0x10)
        b = open(body, 'rb').read() if body and os.path.exists(body) else None
        if b is None:
            missing_body += 1
        tsize, trecord, _, tfirst, tlast = struct.unpack_from('<IIIII', d, tn + 4)
        vsize, vrecord, _, vfirst, vlast = struct.unpack_from('<IIIII', d, va + 4)
        assert trecord == 0x60 and vrecord == 0x10, (head, trecord, vrecord)
        first_tone[(tfirst, vfirst)] += 1
        for n in range(tlast - tfirst + 1):
            at = tn + 0x20 + n * 0x60
            words = struct.unpack_from('<24I', d, at)
            if all(w == 0xffffffff for w in words):
                empty += 1
                continue
            tones += 1
            if words[0] == 0xffffffff:
                no_sample += 1
            for off, name in FIELDS.items():
                values[name][words[off // 4]] += 1
            for i, w in enumerate(words):
                if i * 4 not in FIELDS and i != 0 and w != 0xffffffff:
                    unused_words[(i * 4, w)] += 1
        for n in range(vlast - vfirst + 1):
            offset, rate, length, _ = struct.unpack_from('<4I', d, va + 0x20 + n * 0x10)
            if offset == 0xffffffff:
                continue
            samples += 1
            rates[rate] += 1
            seconds += length / 16 * 28 / rate
            if b is not None:
                looped = False
                for a in range(offset, min(offset + length, len(b)), 16):
                    flags[b[a + 1]] += 1
                    looped |= b[a + 1] in (2, 6)
                loops += looped
    print(f'{count} banks ({missing_body} without data), {tones} tones ({empty} empty, {no_sample} with no sample), {samples} samples, {seconds:.0f} s, {loops} loop')
    print('first tone and sample numbers:', dict(first_tone))
    for name, counter in values.items():
        print(f'  {name}: ' + ', '.join(f'{v:#x} x{c}' for v, c in counter.most_common(12)))
    print('  other words set:', dict(unused_words.most_common(12)))
    print('  rates:', dict(rates.most_common()))
    print('  frame flags:', {hex(k): v for k, v in flags.most_common()})


if __name__ == '__main__':
    main()
