import os, re, struct

os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'iso', 'PSP_GAME'))
d = open('SYSDIR/EBOOT.OLD', 'rb').read()

(e_type, e_machine, _, e_entry, e_phoff, e_shoff, e_flags, _, _, e_phnum, _, e_shnum, e_shstrndx) = struct.unpack_from('<HHIIIIIHHHHHH', d, 16)
print(f'type={e_type:#x} machine={e_machine} entry={e_entry:#x} phnum={e_phnum} shnum={e_shnum} shoff={e_shoff:#x}')

secs = []
for i in range(e_shnum):
    secs.append(struct.unpack_from('<IIIIIIIIII', d, e_shoff + i * 40))
if secs and e_shstrndx < len(secs):
    stroff = secs[e_shstrndx][4]
    for s in secs:
        name = d[stroff + s[0]: d.index(b'\0', stroff + s[0])].decode('latin1')
        print(f'  {name:<28} type={s[1]:#x} addr={s[3]:#010x} off={s[4]:#x} size={s[5]:#x}')

strs = sorted(set(m.group().decode() for m in re.finditer(rb'[\x20-\x7e]{4,}', d)))
print('total strings', len(strs))
for label, pat in [('Ht* class-like', r'^Ht[A-Z]\w+$'), ('*Task', r'Task$'), ('sce imports', r'^sce[A-Z]\w+$'), ('file exts', r'\.(mdl|anm|efp|phd|pbd|bms|edx|ptm|sm|at3|pmf)\b')]:
    hits = [s for s in strs if re.search(pat, s)]
    print(f'{label}: {len(hits)}')
    print('   ', hits[:40])

for f in ['USRDIR/GAMEDATA/SOUND/BGM_BATTLE1.AT3']:
    a = open(f, 'rb').read(64)
    fmt, ch, rate = struct.unpack_from('<HHI', a, 20)
    print(f, f'fmt={fmt:#x} ch={ch} rate={rate}')
big = [(os.path.getsize(os.path.join('USRDIR/GAMEDATA', f)), f) for f in os.listdir('USRDIR/GAMEDATA') if os.path.isfile(os.path.join('USRDIR/GAMEDATA', f))]
print('GAMEDATA root files:', sorted(big, reverse=True))
