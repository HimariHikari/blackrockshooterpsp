"""Check what the per-binding 4x4 matrices in an INSM model are. Exploration only.

Compares each stored matrix against the inverse of the node's bind-pose world matrix,
in both row-major and column-major readings.
"""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load
from insm_obj import parse, mat_mul


def inverse_rigid(m):
    """Inverse of a rotation+translation 4x4 (row-major, column vectors)."""
    r = [[m[j][i] for j in range(3)] for i in range(3)]
    t = [-sum(r[i][k] * m[k][3] for k in range(3)) for i in range(3)]
    return [r[0] + [t[0]], r[1] + [t[1]], r[2] + [t[2]], [0, 0, 0, 1]]


def main():
    rel = sys.argv[1] if len(sys.argv) > 1 else 'BTL\\CHR\\BCHR_APL00.VOL\\chr_m_apl00.mdl'
    d = load(rel)
    m = parse(d)
    worst = {'row': 0.0, 'col': 0.0}
    for i, b in enumerate(m['binds']):
        if b['nskin'] == 0:
            continue
        raw = struct.unpack_from('<16f', d, b['ibm'])
        row = [list(raw[k * 4:k * 4 + 4]) for k in range(4)]
        col = [[raw[c * 4 + r] for c in range(4)] for r in range(4)]
        want = inverse_rigid(m['nodes'][b['node']]['world'])
        errs = {}
        for name, got in (('row', row), ('col', col)):
            errs[name] = max(abs(got[r][c] - want[r][c]) for r in range(4) for c in range(4))
            worst[name] = max(worst[name], errs[name])
        if i < 3:
            print(f'binding {i} node {b["node"]}: err row-major {errs["row"]:.4g}, column-major {errs["col"]:.4g}')
            print('   raw:', ' '.join(f'{x:9.4f}' for x in raw))
    print(f'worst error over all skinned bindings: row-major {worst["row"]:.4g}, column-major {worst["col"]:.4g}')


if __name__ == '__main__':
    main()
