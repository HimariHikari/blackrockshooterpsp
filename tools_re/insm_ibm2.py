"""Per-binding comparison of stored matrix vs inverse bind-pose world matrix. Exploration only."""
import os, struct, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from insm import load
from insm_obj import parse
from insm_ibm import inverse_rigid

rel = sys.argv[1]
d = load(rel)
m = parse(d)
print(f'{len(m["nodes"])} nodes, {len(m["binds"])} bindings')
bad = 0
for i, b in enumerate(m['binds']):
    if b['nskin'] == 0:
        continue
    raw = struct.unpack_from('<16f', d, b['ibm'])
    col = [[raw[c * 4 + r] for c in range(4)] for r in range(4)]
    want = inverse_rigid(m['nodes'][b['node']]['world'])
    rot_err = max(abs(col[r][c] - want[r][c]) for r in range(3) for c in range(3))
    pos_err = max(abs(col[r][3] - want[r][3]) for r in range(3))
    n = m['nodes'][b['node']]
    flag = '' if max(rot_err, pos_err) < 1e-2 else '  <-- differs'
    bad += bool(flag)
    if flag or '--all' in sys.argv:
        print(f'binding {i:3} node {b["node"]:3} id={n["id"]:02X} parent={n["parent"]:3} nskin={b["nskin"]} nrigid={b["nrigid"]} rot_err={rot_err:8.4f} pos_err={pos_err:9.4f}{flag}')
print(f'{bad} skinned bindings differ from the node table bind pose')
