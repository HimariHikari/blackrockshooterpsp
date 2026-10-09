"""Find STCM files whose texture lists do not follow the 'straight after the main list, 0x30 apart' rule. Exploration only."""
import collections, os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import GE, ROOT


def sublist(d, t):
    out = []
    while t + 4 <= len(d) and len(out) < 64:
        w = struct.unpack_from('<I', d, t)[0]
        out.append((w >> 24, w & 0xffffff))
        t += 4
        if out[-1][0] in (0x0B, 0x0C):
            break
    return out


shown = 0
total = fails = 0
patterns = collections.Counter()
for dp, _, fs in os.walk(ROOT):
    for f in fs:
        p = os.path.join(dp, f)
        with open(p, 'rb') as fh:
            if fh.read(4) != b'STCM':
                continue
            d = b'STCM' + fh.read()
        total += 1
        ntex = d[5]
        dl = struct.unpack_from('<I', d, 0x10)[0]
        o = origin = dl
        tex_targets = []
        main_end = None
        cmds = collections.Counter()
        while o + 4 <= len(d):
            w = struct.unpack_from('<I', d, o)[0]
            cmd, arg = w >> 24, w & 0xffffff
            cmds[cmd] += 1
            if cmd == 0x0A:
                t = origin + arg
                if any(c == 0xA0 for c, _ in sublist(d, t)):
                    tex_targets.append(t)
            elif cmd in (0x0B, 0x0C):
                main_end = o + 4
                break
            o += 4
        uniq = sorted(set(tex_targets))
        ok = all((t - main_end) % 0x30 == 0 and 0 <= (t - main_end) // 0x30 < ntex for t in uniq)
        if ok:
            continue
        fails += 1
        lens = collections.Counter(len(sublist(d, t)) for t in uniq)
        gaps = collections.Counter(b - a for a, b in zip(uniq, uniq[1:]))
        first = uniq[0] - main_end if uniq else None
        patterns[(tuple(sorted(lens.items())), first)] += 1
        if shown < 3:
            shown += 1
            print(os.path.relpath(p, ROOT))
            print(f'   textures {ntex}, main list {dl:#x}..{main_end:#x}, {len(uniq)} texture lists, first at +{first:#x}')
            print(f'   list lengths (commands): {dict(lens)}  gaps: {dict(gaps)}')
            print('   main list commands:', {GE.get(k, hex(k)): v for k, v in sorted(cmds.items())})
            for t in uniq[:2]:
                print(f'   list at {t:#x}:', ' '.join(f'{GE.get(c, hex(c))}={a:x}' for c, a in sublist(d, t)))
            between = sublist(d, main_end)
            print(f'   at main_end {main_end:#x}:', ' '.join(f'{GE.get(c, hex(c))}={a:x}' for c, a in between))
print(f'{fails} of {total} files fail')
print('patterns (list lengths, offset of first list from main end):')
for k, v in patterns.most_common():
    print('  ', v, k)
