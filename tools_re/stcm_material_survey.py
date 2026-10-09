"""Surveys the material state of every STCM map under game/unpacked: which GE commands the
non-texture display lists use, and with which values.

Writes tools_re/out/stcm_material_survey.txt.
"""
import collections, os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', 'game', 'unpacked')

NAMES = {0x1d: 'CULL_EN', 0x1e: 'TEX_EN', 0x1f: 'FOG_EN', 0x21: 'BLEND_EN', 0x22: 'ATEST_EN', 0x48: 'TEX_SCALE_U',
         0x49: 'TEX_SCALE_V', 0x4a: 'TEX_OFFSET_U', 0x4b: 'TEX_OFFSET_V', 0x53: 'MAT_UPDATE', 0x54: 'MAT_EMISSIVE',
         0x55: 'MAT_AMBIENT', 0x56: 'MAT_DIFFUSE', 0x57: 'MAT_SPECULAR', 0x58: 'MAT_ALPHA', 0xc7: 'TEX_WRAP',
         0xc9: 'TEX_FUNC', 0xdb: 'ATEST', 0xdf: 'BLEND', 0xe0: 'BLEND_FIX_A', 0xe1: 'BLEND_FIX_B', 0xe7: 'ZMSK'}


def lists_of(d):
    list_at = struct.unpack_from('<I', d, 0x10)[0]
    origin, base, pc = list_at, 0, list_at
    seen = {}
    while pc + 4 <= len(d):
        w = struct.unpack_from('<I', d, pc)[0]
        cmd, arg = w >> 24, w & 0xffffff
        if cmd == 0x14:
            origin = pc
        elif cmd == 0x10:
            base = (arg << 8) & 0xff000000
        elif cmd == 0x0a:
            target = origin + (base | arg)
            if target not in seen:
                body, at = [], target
                while at + 4 <= len(d):
                    c = struct.unpack_from('<I', d, at)[0]
                    if c >> 24 in (0x0b, 0x0c):
                        break
                    body.append(c)
                    at += 4
                seen[target] = body
        elif cmd in (0x0b, 0x0c):
            break
        pc += 4
    return seen.values()


def main():
    values = collections.defaultdict(collections.Counter)
    shapes = collections.Counter()
    files = 0
    for folder, _, names in os.walk(ROOT):
        for name in names:
            path = os.path.join(folder, name)
            with open(path, 'rb') as f:
                if f.read(4) != b'STCM':
                    continue
                d = b'STCM' + f.read()
            files += 1
            for body in lists_of(d):
                if any(c >> 24 == 0xa0 for c in body):
                    continue
                shapes[' '.join(NAMES.get(c >> 24, hex(c >> 24)) for c in body)] += 1
                for c in body:
                    values[NAMES.get(c >> 24, hex(c >> 24))][c & 0xffffff] += 1
    out = [f'{files} STCM files', '', 'material list shapes:']
    out += [f'  {n:6d}  {shape}' for shape, n in shapes.most_common()]
    out += ['', 'values per command:']
    for name in sorted(values):
        out.append(f'  {name}: ' + ', '.join(f'{v:#x} x{n}' for v, n in values[name].most_common(12)))
    path = os.path.join(HERE, 'out', 'stcm_material_survey.txt')
    open(path, 'w', encoding='utf-8').write('\n'.join(out) + '\n')
    print(f'wrote {path}')


main()
