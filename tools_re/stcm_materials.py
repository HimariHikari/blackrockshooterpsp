"""Lists the display lists an STCM map calls (its texture and material state) and how much geometry uses each.

Usage: stcm_materials.py <unpacked .stcm or battle map .bin>
"""
import struct, sys, zlib

GE = {0x00: 'NOP', 0x01: 'VADDR', 0x04: 'PRIM', 0x0a: 'CALL', 0x0b: 'RET', 0x0c: 'END', 0x10: 'BASE', 0x12: 'VTYPE',
      0x14: 'ORIGIN', 0x17: 'LIGHTING', 0x1d: 'CULL_EN', 0x1e: 'TEX_EN', 0x1f: 'FOG_EN', 0x20: 'DITHER_EN',
      0x21: 'BLEND_EN', 0x22: 'ATEST_EN', 0x23: 'ZTEST_EN', 0x50: 'SHADE', 0x53: 'MAT_UPDATE', 0x54: 'MAT_EMISSIVE',
      0x55: 'MAT_AMBIENT', 0x56: 'MAT_DIFFUSE', 0x57: 'MAT_SPECULAR', 0x58: 'MAT_ALPHA', 0x5b: 'SPEC_POWER',
      0x9b: 'FRONT_FACE', 0xa0: 'TEX_ADDR', 0xa8: 'TEX_WIDTH', 0xb0: 'CLUT_ADDR', 0xb1: 'CLUT_ADDR_HI',
      0xb8: 'TEX_SIZE', 0xc2: 'TEX_MODE', 0xc3: 'TEX_FORMAT', 0xc4: 'CLUT_LOAD', 0xc5: 'CLUT_FORMAT',
      0xc6: 'TEX_FILTER', 0xc7: 'TEX_WRAP', 0xc8: 'TEX_BIAS', 0xc9: 'TEX_FUNC', 0xca: 'TEX_ENV', 0xcb: 'TEX_FLUSH',
      0xcc: 'TEX_SYNC', 0xdb: 'ATEST', 0xde: 'ZTEST', 0xdf: 'BLEND', 0xe0: 'BLEND_FIX_A', 0xe1: 'BLEND_FIX_B',
      0xe7: 'ZMSK', 0xe8: 'PMSK_C', 0xe9: 'PMSK_A'}


def main():
    d = open(sys.argv[1], 'rb').read()
    if d[:4] != b'STCM':
        raise SystemExit('not an STCM file (unwrap it first)')
    list_at = struct.unpack_from('<I', d, 0x10)[0]
    origin, base = list_at, 0
    lists, order = {}, []
    current = []
    usage = {}
    pc = list_at
    while True:
        w = struct.unpack_from('<I', d, pc)[0]
        cmd, arg = w >> 24, w & 0xffffff
        if cmd == 0x14:
            origin = pc
        elif cmd == 0x10:
            base = (arg << 8) & 0xff000000
        elif cmd == 0x0a:
            target = origin + (base | arg)
            if target not in lists:
                body, at = [], target
                while True:
                    c = struct.unpack_from('<I', d, at)[0]
                    if c >> 24 in (0x0b, 0x0c):
                        break
                    body.append(c)
                    at += 4
                lists[target] = body
                order.append(target)
            is_texture = any(c >> 24 == 0xa0 for c in lists[target])
            current = [t for t in current if (any(c >> 24 == 0xa0 for c in lists[t])) != is_texture] + [target]
        elif cmd == 0x04:
            key = tuple(current)
            count = arg & 0xffff
            usage[key] = usage.get(key, 0) + count
        elif cmd in (0x0b, 0x0c):
            break
        pc += 4

    for target in order:
        body = lists[target]
        text = ', '.join(f'{GE.get(c >> 24, hex(c >> 24))}={c & 0xffffff:#x}' for c in body)
        print(f'list {target:#x}: {text}')
    print()
    for key, count in sorted(usage.items(), key=lambda kv: -kv[1]):
        print(f'{count:6d} vertices with lists ' + ' + '.join(f'{t:#x}' for t in key))


main()
