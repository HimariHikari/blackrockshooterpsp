import gzip, os, re, sys

os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'iso', 'PSP_GAME'))


def hx(b, n=16):
    return ' '.join(f'{x:02X}' for x in b[:n])


def asc(b):
    return ''.join(chr(x) if 32 <= x < 127 else '.' for x in b)


def dump(d, end, start=0):
    for i in range(start, end, 16):
        print(f'  {i:04X} {hx(d[i:i+16]):<47} {asc(d[i:i+16])}')


MAGICS = rb'(MIG\.00\.1PSP|OMG\.00\.1PSP|RIFF|\x89PNG|TIM2|PTMD|STCM|RTDP|INSM|PSMF|CZAA|\x1f\x8b\x08)'


def embedded(u):
    mg = {}
    for m in re.finditer(MAGICS, u):
        mg[m.group()] = mg.get(m.group(), 0) + 1
    return mg


print('== SYSDIR')
for f in ['SYSDIR/BOOT.BIN', 'SYSDIR/EBOOT.BIN', 'SYSDIR/EBOOT.OLD']:
    d = open(f, 'rb').read()
    print(f, len(d), hx(d[:16]), asc(d[:16]))

for f in ['SYSDIR/BOOT.BIN', 'SYSDIR/EBOOT.OLD']:
    d = open(f, 'rb').read()
    strs = set(m.group().decode() for m in re.finditer(rb'[\x20-\x7e]{5,}', d))
    print(f' {f}: {len(strs)} printable strings')
    for k in ['.VOL', '.BIN', '.GZ', '.ZZZ', '.PTM', '.SM', 'GAMEDATA', 'sce', '.gmo', '.GIM', '.cpp', '.c:', 'assert', 'printf', '%s']:
        hits = sorted(s for s in strs if k.lower() in s.lower())
        print(f'   {k!r}: {len(hits)}  e.g. {hits[:8]}')

print('== RTDP VOL (battle character)')
d = open('USRDIR/GAMEDATA/BTL/CHR/BCHR_APL00.VOL', 'rb').read()
print(len(d))
dump(d, 0x340)
print('   embedded:', embedded(d))

print('== gz inner')
for f in ['USRDIR/GAMEDATA/FLD/FCHR/FCHR_B_CYZ00.BIN',
          'USRDIR/GAMEDATA/FLD/MAP/MAP_D_STG01_01.BIN',
          'USRDIR/GAMEDATA/FLD/FEVT/EV_LL_EX2_00.BIN',
          'USRDIR/GAMEDATA/GALLERY/GLR_ART_001.BIN',
          'USRDIR/GAMEDATA/IF/MENU/IF_MISSION_FAILED.GZ',
          'USRDIR/GAMEDATA/IF/EVT/MINI_MAP_STAGE01.GZ',
          'USRDIR/GAMEDATA/MINI_GAME/BMG_EASY.BIN']:
    try:
        u = gzip.open(f, 'rb').read()
        print(f.split('/')[-1], os.path.getsize(f), '->', len(u))
        dump(u, 0x60)
        print('   embedded:', embedded(u))
    except Exception as e:
        print(f, 'ERR', e)

print('== CZAA / INSM / STCM / PTMD')
for f in ['USRDIR/GAMEDATA/BTL/DBG/CHR_P_BRS00.ZZZ',
          'USRDIR/GAMEDATA/IF/PICT/BTL_PIT260.PTM.ZZZ',
          'USRDIR/GAMEDATA/BTL/WEP/ITM_EQU_ASSUALT_LV1.ZZZ',
          'USRDIR/GAMEDATA/BTL/TEST00/BTL_MAP.SM',
          'USRDIR/GAMEDATA/BTL/TEST00/SAMPLE.PTM']:
    d = open(f, 'rb').read()
    print(f.split('/')[-1], len(d))
    dump(d, 0x40)
    print('   embedded:', embedded(d[4:]))
