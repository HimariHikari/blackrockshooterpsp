"""Parse the RVLS index (SYSTEM.VOL/instdat.vls) of INSDIR/INSTDAT.VLD. Exploration only.

Checks the XOR-key theory against a file that also exists on the disc, and lists the
entries that have no same-named file under GAMEDATA.
"""
import collections, os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
GAME = os.path.join(HERE, '..', 'game')
INDEX = os.path.join(GAME, 'unpacked', 'SYSTEM.VOL', 'instdat.vls')
VLD = os.path.join(GAME, 'extracted', 'PSP_GAME', 'INSDIR', 'INSTDAT.VLD')
GAMEDATA = os.path.join(GAME, 'extracted', 'PSP_GAME', 'USRDIR', 'GAMEDATA')

idx = open(INDEX, 'rb').read()
magic, idx_size, count, total, x10 = struct.unpack_from('<4sIIII', idx, 0)
print(f'{magic} index size {idx_size:#x}, {count} entries, total {total:#x}, x10 {x10:#x}; VLD size {os.path.getsize(VLD):#x}')

entries = []
for i in range(count):
    o = 0x20 + i * 0x3C
    name = idx[o:o + 32].split(b'\0')[0].decode('latin1')
    key = idx[o + 32:o + 52]
    size, offset = struct.unpack_from('<II', idx, o + 52)
    entries.append((name, key, size, offset))

on_disc = {}
for dp, _, fs in os.walk(GAMEDATA):
    for f in fs:
        on_disc.setdefault(f.upper(), []).append(os.path.join(dp, f))


def decrypt(vld, key, size, offset):
    vld.seek(offset)
    raw = vld.read(size)
    return bytes(b ^ key[i % 20] for i, b in enumerate(raw))


with open(VLD, 'rb') as vld:
    # 1. Verify against files that exist on the disc with the same size.
    same = differ = 0
    checked = 0
    for name, key, size, offset in entries:
        paths = on_disc.get(name.upper())
        if not paths or checked >= 40:
            continue
        plain = open(paths[0], 'rb').read()
        if len(plain) != size:
            continue
        checked += 1
        if decrypt(vld, key, size, offset) == plain:
            same += 1
        else:
            differ += 1
    print(f'decryption check on {checked} same-named, same-sized disc files: {same} identical, {differ} different')

    # 2. What is in the install data that is not on the disc?
    missing = [(n, s, o) for n, k, s, o in entries if n.upper() not in on_disc]
    print(f'{len(missing)} entries have no same-named file under GAMEDATA; {count - len(missing)} do')
    ext = collections.Counter(os.path.splitext(n)[1].upper() for n, _, _ in missing)
    print('  by extension:', dict(ext))
    print('  total size of those: %.1f MB' % (sum(s for _, s, _ in missing) / 1e6))
    for n, s, o in missing[:60]:
        with_key = next(k for nn, k, ss, oo in entries if nn == n and oo == o)
        head = decrypt(vld, with_key, min(s, 16), o)
        print(f'    {s:>9}  {n:<28} {head[:8].hex(" ")}  {"".join(chr(b) if 32 <= b < 127 else "." for b in head[:8])}')
    sizes_differ = [(n, s) for n, k, s, o in entries if n.upper() in on_disc and os.path.getsize(on_disc[n.upper()][0]) != s]
    print(f'{len(sizes_differ)} same-named entries differ in size from the disc copy, e.g. {sizes_differ[:5]}')
    in_disc_not_vld = sorted(set(on_disc) - {n.upper() for n, _, _, _ in entries})
    print(f'{len(in_disc_not_vld)} disc files are not in the install data, e.g. {in_disc_not_vld[:12]}')
