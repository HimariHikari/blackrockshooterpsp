"""The PSP's vector unit (VFPU): its instructions, which capstone does not know.

`decode(word, address)` gives the instruction as text, or None when the word is not
one of the vector unit's. Registers are named as the PSP's assemblers name them:
`S<matrix><column><row>` a single float, `C<matrix><column><row>` a column,
`R<matrix><row><column>` a row, `M<matrix>..` a matrix and `E<matrix>..` its transpose.

What the less obvious ones compute (s, t the sources, d the result):

  vcrs.t   d = (s1*t2, s2*t0, s0*t1)             half of a cross product
  vcrsp.t  d = s x t                             the whole cross product
  vqmul.q  d = s * t                             quaternions (x, y, z, w)
  vmmul    d = s * t                             matrices, a register's columns being
                                                 the matrix's columns
  vtfmN    d = M * t                             matrix times column vector
  vsin, vcos, vasin, vrot                        angles in quarter turns: sin(x * pi/2)
  vocp     d = 1 - s
  vscl     d = s * t (t one float)
  vpfxs/vpfxt  what the next instruction reads its s/t as: components swapped,
               made absolute, negated or replaced by a constant
  vpfxd    how the next instruction writes: [0:1] or [-1:1] clamps, M leaves alone

Usage: vfpu.py <hex word> ...
"""
import struct, sys

SIZES = 'sptq'
CONDITIONS = ['FL', 'EQ', 'LT', 'LE', 'TR', 'NE', 'GE', 'GT', 'EZ', 'EN', 'EI', 'ES', 'NZ', 'NN', 'NI', 'NS']
CONSTANTS = ['(undef)', 'HUGE', 'SQRT2', 'SQRT1_2', '2/SQRTPI', '2/PI', '1/PI', 'PI/4', 'PI/2', 'PI', 'E',
             'LOG2E', 'LOG10E', 'LN2', 'LN10', '2PI', 'PI/6', 'LOG10_2', 'LOG2_10', 'SQRT3/2']
GPR = ['zero', 'at', 'v0', 'v1', 'a0', 'a1', 'a2', 'a3', 't0', 't1', 't2', 't3', 't4', 't5', 't6', 't7',
       's0', 's1', 's2', 's3', 's4', 's5', 's6', 's7', 't8', 't9', 'k0', 'k1', 'gp', 'sp', 'fp', 'ra']

# Two-operand instructions of 0xD0xxxxxx, by bits 25-16.
UNARY = {
    0x00: 'vmov', 0x01: 'vabs', 0x02: 'vneg', 0x04: 'vsat0', 0x05: 'vsat1',
    0x10: 'vrcp', 0x11: 'vrsq', 0x12: 'vsin', 0x13: 'vcos', 0x14: 'vexp2', 0x15: 'vlog2', 0x16: 'vsqrt',
    0x17: 'vasin', 0x18: 'vnrcp', 0x1a: 'vnsin', 0x1c: 'vrexp2',
    0x32: 'vf2h', 0x33: 'vh2f', 0x36: 'vsbz', 0x37: 'vlgb', 0x38: 'vuc2i', 0x39: 'vc2i', 0x3a: 'vus2i',
    0x3b: 'vs2i', 0x3c: 'vi2uc', 0x3d: 'vi2c', 0x3e: 'vi2us', 0x3f: 'vi2s',
    0x40: 'vsrt1', 0x41: 'vsrt2', 0x42: 'vbfy1', 0x43: 'vbfy2', 0x44: 'vocp', 0x45: 'vsocp', 0x46: 'vfad',
    0x47: 'vavg', 0x48: 'vsrt3', 0x49: 'vsrt4', 0x4a: 'vsgn', 0x59: 'vt4444', 0x5a: 'vt5551', 0x5b: 'vt5650',
}
NO_SOURCE = {0x03: 'vidt', 0x06: 'vzero', 0x07: 'vone'}
# Those whose result is one float whatever the size of what they read.
SINGLE_RESULT = {'vfad', 'vavg'}


def size(word):
    return ((word >> 14) & 2) | ((word >> 7) & 1)


def single(reg):
    return f'S{(reg >> 2) & 7}{reg & 3}{(reg >> 5) & 3}'


def vector(reg, n):
    if n == 0:
        return single(reg)
    matrix, column, across = (reg >> 2) & 7, reg & 3, (reg >> 5) & 1
    row = ((reg >> 6) & 1) if n == 2 else ((reg >> 5) & 2)
    return f'R{matrix}{row}{column}' if across else f'C{matrix}{column}{row}'


def matrix(reg, n):
    number, column, across = (reg >> 2) & 7, reg & 3, (reg >> 5) & 1
    row = ((reg >> 6) & 1) if n == 2 else ((reg >> 5) & 2)
    return f'{"E" if across else "M"}{number}{column}{row}'


def half(bits):
    sign, exponent, mantissa = bits >> 15, (bits >> 10) & 31, bits & 0x3ff
    if exponent == 0:
        value = mantissa / 1024 * 2.0 ** -14
    elif exponent == 31:
        value = float('inf') if mantissa == 0 else float('nan')
    else:
        value = (1 + mantissa / 1024) * 2.0 ** (exponent - 15)
    return -value if sign else value


def prefix(data):
    constants = ['0', '1', '2', '1/2', '3', '1/3', '1/4', '1/6']
    parts = []
    for i in range(4):
        which, absolute = (data >> (i * 2)) & 3, (data >> (8 + i)) & 1
        if (data >> (12 + i)) & 1:
            text = constants[which + (absolute << 2)]
        else:
            text = 'xyzw'[which]
            if absolute:
                text = f'|{text}|'
        parts.append(('-' if (data >> (16 + i)) & 1 else '') + text)
    return '[' + ', '.join(parts) + ']'


def prefix_result(data):
    parts = []
    for i in range(4):
        clamp = (data >> (i * 2)) & 3
        text = {0: '', 1: '0:1', 3: '-1:1'}.get(clamp, '?')
        if (data >> (8 + i)) & 1:
            text = 'M' if not text else text + ' M'
        parts.append(text or '.')
    return '[' + ', '.join(parts) + ']'


def decode(word, address=0):
    op = word >> 26
    vt, vs, vd = (word >> 16) & 0x7f, (word >> 8) & 0x7f, word & 0x7f
    n = size(word)
    suffix = '.' + SIZES[n]
    offset = word & 0xfffc
    if offset & 0x8000:
        offset -= 0x10000
    base = GPR[(word >> 21) & 31]

    if op == 0x12:  # the coprocessor's moves and branches
        kind, rt = (word >> 21) & 31, GPR[(word >> 16) & 31]
        if kind == 3:
            return f'mfvc {rt}, {word & 0x7f}' if word & 0x80 else f'mfv {rt}, {single(vd)}'
        if kind == 7:
            return f'mtvc {rt}, {word & 0x7f}' if word & 0x80 else f'mtv {rt}, {single(vd)}'
        if kind == 8:
            name = ['bvf', 'bvt', 'bvfl', 'bvtl'][(word >> 16) & 3]
            target = address + 4 + ((word & 0xffff) - (0x10000 if word & 0x8000 else 0)) * 4
            return f'{name} cc{(word >> 18) & 7}, {target:#x}'
        return None
    if op in (0x32, 0x3a):
        reg = ((word >> 16) & 0x1f) | ((word & 3) << 5)
        return f'{"lv.s" if op == 0x32 else "sv.s"} {single(reg)}, {offset:#x}({base})'
    if op in (0x36, 0x3e):
        reg = ((word >> 16) & 0x1f) | ((word & 1) << 5)
        return f'{"lv.q" if op == 0x36 else "sv.q"} {vector(reg, 3)}, {offset:#x}({base})'
    if op in (0x35, 0x3d):
        reg = ((word >> 16) & 0x1f) | ((word & 1) << 5)
        name = ('lv' if op == 0x35 else 'sv') + ('r' if word & 2 else 'l') + '.q'
        return f'{name} {vector(reg, 3)}, {offset:#x}({base})'

    if op in (0x18, 0x19, 0x1b):
        group = (word >> 23) & 7
        name = {0x18: {0: 'vadd', 1: 'vsub', 2: 'vsbn', 7: 'vdiv'},
                0x19: {0: 'vmul', 1: 'vdot', 2: 'vscl', 4: 'vhdp', 5: 'vcrs', 6: 'vdet'},
                0x1b: {0: 'vcmp', 2: 'vmin', 3: 'vmax', 5: 'vscmp', 6: 'vsge', 7: 'vslt'}}[op].get(group)
        if name is None:
            return None
        if name == 'vcmp':
            return f'vcmp{suffix} {CONDITIONS[word & 15]}, {vector(vs, n)}, {vector(vt, n)}'
        result = single(vd) if name in ('vdot', 'vhdp', 'vdet') else vector(vd, n)
        second = single(vt) if name == 'vscl' else vector(vt, n)
        return f'{name}{suffix} {result}, {vector(vs, n)}, {second}'

    if op == 0x34:
        high = (word >> 21) & 31
        code = (word >> 16) & 0x3ff
        if high < 3:
            if code in NO_SOURCE:
                return f'{NO_SOURCE[code]}{suffix} {vector(vd, n)}'
            if code in UNARY:
                name = UNARY[code]
                result = single(vd) if name in SINGLE_RESULT else vector(vd, n)
                return f'{name}{suffix} {result}, {vector(vs, n)}'
            if code == 0x50:
                return f'vmfvc {single(vd)}, {(word >> 8) & 0x7f}'
            if code == 0x51:
                return f'vmtvc {word & 0x7f}, {single(vs)}'
            return None
        if high == 3:
            number = (word >> 16) & 31
            name = CONSTANTS[number] if number < len(CONSTANTS) else str(number)
            return f'vcst{suffix} {vector(vd, n)}, {name}'
        if 0x10 <= high <= 0x14:
            name = ['vf2in', 'vf2iz', 'vf2iu', 'vf2id', 'vi2f'][high - 0x10]
            return f'{name}{suffix} {vector(vd, n)}, {vector(vs, n)}, {(word >> 16) & 31}'
        if high == 0x15:
            name = 'vcmovf' if word & 0x80000 else 'vcmovt'
            return f'{name}{suffix} {vector(vd, n)}, {vector(vs, n)}, cc{(word >> 16) & 7}'
        if high >= 0x18:
            return f'vwbn{suffix} {vector(vd, n)}, {vector(vs, n)}, {(word >> 16) & 0xff}'
        return None

    if op == 0x37:
        kind, data = (word >> 24) & 3, word & 0xfffff
        if kind == 0:
            return f'vpfxs {prefix(data)}'
        if kind == 1:
            return f'vpfxt {prefix(data)}'
        if kind == 2:
            return f'vpfxd {prefix_result(word & 0xfff)}'
        immediate = word & 0xffff
        if word & 0x800000:
            return f'vfim.s {single(vt)}, {half(immediate):g}'
        return f'viim.s {single(vt)}, {immediate - (0x10000 if immediate & 0x8000 else 0)}'

    if op == 0x3c:
        group = (word >> 23) & 7
        if group == 0:
            # The first source is stored transposed: undo that to name it.
            return f'vmmul{suffix} {matrix(vd, n)}, {matrix(vs ^ 0x20, n)}, {matrix(vt, n)}'
        if group in (1, 2, 3):
            elements = n + 1
            name = f'vtfm{elements}' if elements == group + 1 else f'vhtfm{elements + 1}'
            return f'{name}{suffix} {vector(vd, n)}, {matrix(vs, group)}, {vector(vt, n)}'
        if group == 4:
            return f'vmscl{suffix} {matrix(vd, n)}, {matrix(vs, n)}, {single(vt)}'
        if group == 5:
            name = 'vqmul' if n == 3 else 'vcrsp'
            return f'{name}{suffix} {vector(vd, n)}, {vector(vs, n)}, {vector(vt, n)}'
        if group == 7:
            if (word >> 21) & 3 == 1:
                imm = (word >> 16) & 31
                lanes = ['0'] * (n + 1)
                sine, cosine = (imm >> 2) & 3, imm & 3
                text = '-s' if imm & 0x10 else 's'
                if sine == cosine:
                    lanes = [text] * (n + 1)
                if sine <= n:
                    lanes[sine] = text
                if cosine <= n:
                    lanes[cosine] = 'c'
                return f'vrot{suffix} {vector(vd, n)}, {single(vs)}, [{", ".join(lanes)}]'
            code = (word >> 16) & 31
            if code == 0:
                return f'vmmov{suffix} {matrix(vd, n)}, {matrix(vs, n)}'
            name = {3: 'vmidt', 6: 'vmzero', 7: 'vmone'}.get(code)
            return f'{name}{suffix} {matrix(vd, n)}' if name else None
        return None
    return None


if __name__ == '__main__':
    for text in sys.argv[1:]:
        value = int(text, 16)
        print(f'{value:08x}  {decode(value)}')
