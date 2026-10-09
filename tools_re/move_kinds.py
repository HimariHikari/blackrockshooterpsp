"""Which code runs each kind of enemy move: follows the switch of the movement
update (0xB9D74) for the kinds given, by running its compares and jump tables.

Usage: move_kinds.py <kind> [...]        e.g. move_kinds.py 16 17 202-206
"""
import struct, sys

import mips

#: The first compare of the switch; the instruction before it loads the kind into a1.
SWITCH = 0xB9D78
#: Where a kind with no code of its own ends up (it only sets the "ended" flag).
DEFAULTS = {0xBC05C, 0xBC060}

REG = ['zero', 'at', 'v0', 'v1', 'a0', 'a1', 'a2', 'a3', 't0', 't1', 't2', 't3', 't4', 't5', 't6', 't7',
       's0', 's1', 's2', 's3', 's4', 's5', 's6', 's7', 't8', 't9', 'k0', 'k1', 'gp', 'sp', 'fp', 'ra']


def signed(value, bits=16):
    return value - (1 << bits) if value & (1 << (bits - 1)) else value


def step(pc, regs):
    """Runs the instruction at pc. Returns ('next', pc), ('jump', target, likely) or ('stop',)."""
    w = mips.word(pc)
    op, rs, rt, rd = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31
    imm, simm = w & 0xffff, signed(w & 0xffff)
    target = pc + 4 + (simm << 2)
    get = lambda r: regs.get(r)
    if op == 0x0a:      # slti
        if get(rs) is None:
            return ('stop',)
        regs[rt] = int(signed(get(rs) & 0xffffffff, 32) < simm)
    elif op == 0x0b:    # sltiu
        if get(rs) is None:
            return ('stop',)
        regs[rt] = int((get(rs) & 0xffffffff) < (simm & 0xffffffff))
    elif op == 0x09:    # addiu
        if get(rs) is None:
            return ('stop',)
        regs[rt] = (get(rs) + simm) & 0xffffffff
    elif op == 0x0d:    # ori
        if get(rs) is None:
            return ('stop',)
        regs[rt] = get(rs) | imm
    elif op == 0x0f:    # lui
        regs[rt] = (imm << 16) & 0xffffffff
    elif op == 0x23:    # lw, from the image only
        if get(rs) is None or rs not in (1,):
            return ('stop',)
        regs[rt] = mips.word((get(rs) + simm) & 0xffffffff)
    elif op == 0 and (w & 0x3f) == 0x00 and w != 0:     # sll
        if get(rt) is None:
            return ('stop',)
        regs[rd] = (get(rt) << ((w >> 6) & 31)) & 0xffffffff
    elif op == 0 and (w & 0x3f) == 0x21:                # addu
        if get(rs) is None or get(rt) is None:
            return ('stop',)
        regs[rd] = (get(rs) + get(rt)) & 0xffffffff
    elif op == 0 and (w & 0x3f) == 0x08:                # jr
        if get(rs) is None:
            return ('stop',)
        return ('jump', get(rs), False, True)
    elif w == 0:
        pass
    elif op in (4, 5, 0x14, 0x15):                      # beq bne beql bnel
        if get(rs) is None or get(rt) is None:
            return ('stop',)
        equal = get(rs) == get(rt)
        taken = equal if op in (4, 0x14) else not equal
        return ('jump', target, op in (0x14, 0x15), taken)
    else:
        return ('stop',)
    return ('next', pc + 4)


#: The registers the switch itself works in: zero, at, a0, a1 and a2.
OWN = {0, 1, 4, 5, 6}


def of_the_switch(pc):
    """Whether the instruction at pc is one the switch is made of."""
    w = mips.word(pc)
    op, rs, rt, rd = w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31
    if w == 0:
        return True
    if op in (0x0a, 0x0b, 0x09, 0x0d):
        return rs in OWN and rt in OWN
    if op == 0x0f:
        return rt in OWN
    if op == 0x23:
        return rs == 1 and rt == 1
    if op in (4, 5, 0x14, 0x15):
        return rs in OWN and rt in OWN
    if op == 0:
        function = w & 0x3f
        if function == 0x00:
            return rt in OWN and rd in OWN
        if function == 0x21:
            return rs in OWN and rt in OWN and rd in OWN
        if function == 0x08:
            return rs == 1
    return False


def handler(kind, flags=0):
    regs = {0: 0, 5: kind & 0xffffffff, 4: flags}
    pc = SWITCH
    for _ in range(400):
        if pc in DEFAULTS or not of_the_switch(pc):
            return pc
        result = step(pc, regs)
        if result[0] == 'stop':
            return pc
        if result[0] == 'next':
            pc = result[1]
            continue
        _, target, likely, taken = result
        # The delay slot runs unless the branch is a "likely" one that is not taken;
        # one that is not the switch's own business is some case's first instruction
        # or a pointer set up for all of them, and is left out.
        if (taken or not likely) and of_the_switch(pc + 4):
            step(pc + 4, regs)
        pc = target if taken else pc + 8
    return None


def kinds(arguments):
    for argument in arguments:
        if '-' in argument:
            first, last = argument.split('-')
            yield from range(int(first), int(last) + 1)
        else:
            yield int(argument)


def has_code(kind):
    """Whether the switch has a case for this kind of move."""
    at = handler(kind)
    return at is not None and at not in DEFAULTS


if __name__ == '__main__':
    for kind in kinds(sys.argv[1:]):
        at = handler(kind)
        note = '  (no code of its own: the move just ends)' if at in DEFAULTS else ''
        print(f'{kind:6d} -> {at:#x}{note}' if at is not None else f'{kind:6d} -> ?')
