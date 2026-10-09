# Format notes (work in progress)

Running notes on the game's formats. Anything marked **unverified** is a hypothesis
from reading hex, not something tested.
Exploration scripts are in `tools_re/` (`hd.py`, `insm.py`, `insm_survey.py`,
`insm_counts.py`, `insm_obj.py`).

## INSM — models (`.mdl`, raw `.ZZZ` in `BTL/WEP`, CZAA `.ZZZ` in `BTL/DBG`)

Status: implemented in `crates/brs-formats/src/insm.rs`. All 120 model files in the
game parse (196,406 triangles). Rendering checked by eye in `brs-viewer` on one skinned
enemy (`chr_m_apl00.mdl`), one player model with its rigid weapon attachments
(`CHR_P_BRS00.ZZZ`) and one weapon (`chr_w_smg01.mdl`). Skinning is implemented.
Lighting and blending state are not decoded. Culling is: a map's batch that has
it on (`BCE` in its display list) is drawn from the front of its faces only,
the front being the side its corners go round anticlockwise on (the game sets
that wherever it sets a scene up, 0x15AB34 with 1). The walls at the edge of a
battle's stage are such faces: a camera move that ends beyond them, as Steel
Cutter's does, sees the stage through them.

`.cam` files also start with `INSM` but have header bytes `01 00 00 01` and are camera
data, not models: see "Camera moves" below.

All offsets are from the start of the file. Little-endian.

### Header (0x38 bytes)

| off  | type | meaning |
|------|------|---------|
| 0x00 | 4    | magic `INSM` |
| 0x04 | u8   | 1 (version?) |
| 0x05 | u8   | texture count |
| 0x06 | u8   | material count (equals texture count or one more) |
| 0x07 | u8   | 0 |
| 0x08 | u8   | node count |
| 0x09 | u8   | bone-binding count (entries in table D) |
| 0x0A | u8   | unknown (0, 1, 5, 7, 9 seen) |
| 0x0B | u8   | 0 |
| 0x0C | u32  | submesh count (entries in table B) |
| 0x10 | u32  | offset of material table (A) |
| 0x14 | u32  | offset of submesh table (B) |
| 0x18 | u32  | offset of node table (C) |
| 0x1C | u32  | offset of bone-binding table (D) |
| 0x20 | u32  | offset of a 16-byte block E (unknown) |
| 0x24 | u32  | offset of textures (PTMD, packed back to back) |
| 0x28 | u32  | offset of display lists |
| 0x2C | u32  | size of display lists; vertex data follows immediately |
| 0x30 | u32  | size of a runtime command buffer the engine builds (see table D) |
| 0x34 | u32  | same value as 0x24 in every file looked at |

### A — materials, 0x24 bytes each

| off  | type | meaning |
|------|------|---------|
| 0x00 | u8   | 1 = textured, 0 = untextured |
| 0x01 | u8   | texture index (0 when untextured) |
| 0x02 | u16  | 0 |
| 0x04 | u32  | offset of the material's display list (render state) |
| 0x08 | 7×u32 | colours? `80000000 80000000 00000000 FFFFFFFF 00FFFFFF 00000000 FFFFFFFF` in the files looked at. **Unverified.** |

### B — submeshes, 8 bytes each

| off  | type | meaning |
|------|------|---------|
| 0x00 | u8   | flags: which state the engine sets up before drawing (see below) |
| 0x01 | u8   | number of bone matrices this submesh uses; 0 = rigid, attached to one node |
| 0x02 | u8   | material index |
| 0x03 | u8   | 0 |
| 0x04 | u32  | offset of the submesh's display list |

### C — nodes (skeleton), 0x20 bytes each

| off  | type | meaning |
|------|------|---------|
| 0x00 | u8   | flags: bit 1 = has a child (the next node), bit 2 = has a next sibling, bit 3 = set on skeleton nodes of skinned models |
| 0x01 | u8   | unknown id, usually 0xFF (0x00, 0x32, 0x34 seen) |
| 0x02 | u8   | parent node index, 0xFF = root |
| 0x03 | u8   | index of the first node after this node's subtree, 0xFF = none |
| 0x04 | 3×f32 | translation relative to parent |
| 0x10 | 4×f32 | rotation quaternion (x, y, z, w) |

### D — bone bindings, 12 bytes each, followed by their u32 lists

| off  | type | meaning |
|------|------|---------|
| 0x00 | u8   | number of skinned matrix slots driven by this node |
| 0x01 | u8   | number of rigid submeshes attached to this node |
| 0x02 | u16  | node index |
| 0x04 | u32  | offset of a list of (skinned + rigid) u32 values |
| 0x08 | u32  | offset of a 4×4 f32 matrix (inverse bind matrix); for rigid-only entries it just points at block E |

Each list value is an offset into the runtime command buffer (size at header 0x30) where
this node's matrix gets written each frame. There is one value per matrix slot in the
whole model. Sorting all values ascending gives the slots in submesh order, so
**submesh k, slot j is driven by the node whose list contains the corresponding value.**

Slots per submesh:

- skinned (bone count > 0): one slot per bone;
- rigid with flag 0x02: one slot (the node the submesh hangs off);
- rigid without flag 0x02: no slot; it is drawn with the previous submesh's matrix.

### Runtime command buffer layout (deduced, matches every size checked)

The engine builds one display list per model, of the size given at header 0x30:

- 4 bytes at the start;
- then per submesh, in order:
  - if flags & 0x0C: 0x18 bytes of state (material and culling, presumably; how the two
    bits split is **unverified**);
  - if flags & 0x01: 0x34 bytes (a 12-float matrix nothing points at; purpose unknown);
  - if flags & 0x02: 0x34 bytes = 1 command word + 12-float matrix (binding value = start + 4);
  - per bone: 0x38 bytes = 2 command words + 12-float matrix (binding value = start + 8);
  - 0x0C bytes to draw the submesh.

Checked against: `chr_w_bla00.mdl` 0x5C, `ITM_EQU_ASSUALT_LV1` 0x9C, `ITM_EQU_ASSUALT_LV2`
0x68, `chr_w_smg01.mdl` 0xD4, and the slot spacing in `chr_m_apl00.mdl`.

### Display lists

Standard PSP GE command words: `u32 = (command << 24) | argument`.

Submesh list: `ORIGIN`, `BCE` (cull on/off), then per strip `VTYPE`, `BASE 0`, `VADDR`,
`PRIM`, and `RET` at the end. `VADDR` is relative to the address of the list's `ORIGIN`
word. Every primitive in the game is a triangle strip, except two indexed triangle
lists (vertex type 0x1183, uses `IADDR`).

Material list: `TWRAP`, `TME`, `TFUNC`, `ALPHA`, `SFIX`, `DFIX`, `ATST`, `ZMSK`, `CMAT`,
`SPOW`, `RET`.

No `BONE` commands appear in the files; the engine uploads bone matrices itself.

### Vertex data

Standard PSP vertex layouts, selected by the `VTYPE` argument. Component order in
memory: weights, texture coords, colour, normal, position; each component aligned to
its own element size, vertex padded to the largest alignment.

Vertex types used (count of strips in the whole game):

| VTYPE    | layout | bytes |
|----------|--------|-------|
| 0x0007A2 | 1 weight f32, uv u16, normal i8, pos f32 | 24 |
| 0x0007BA | as above + colour 4444 | 28 |
| 0x0047A2 | 2 weights | 28 |
| 0x0001A2 | no weights (rigid) | 20 |
| 0x0047BA | 2 weights + colour | 32 |
| 0x0087A2 | 3 weights | 32 |
| 0x0087BA | 3 weights + colour | 36 |
| 0x000782 | 1 weight, no normal | 20 |
| 0x00C7A2 / 0x00C7BA | 4 weights (+ colour) | 36 / 40 |
| 0x0107A2, 0x0147A2 | 5 and 6 weights | 40 / 44 |
| 0x001183 | uv f32, pos f32, 16-bit indices | 20 |

UV: `u16 / 32768`. Normal: `i8 / 127`.

### Skinning

Skinned vertices are stored in model space in the bind pose. The 4×4 matrix each bone
binding points at is that node's inverse bind matrix, stored column by column (checked
numerically: it equals the inverse of the node table's world transform to about 1e-5
on `chr_m_apl00`). Skin with `node world transform × inverse bind matrix`.

The node table holds a rest pose that is not always the bind pose: on the player
models the two arm chains differ, so always use the stored matrices rather than
computing them from the node table.

## INSA — animations (`.anm`)

Status: implemented in `crates/brs-formats/src/insa.rs`; all 3,459 animations parse,
and the 263 camera moves that end in one. Read from the original's evaluator at
0x20614 (one node at a time) and 0x21454 (the walk over the nodes), so what follows is
what the game does, not a guess. Checked by eye in `brs-viewer` on an enemy
(`chr_m_apl00`), the player in a scene (`EV_LL_EX2_00`), a soldier in the opening
(`FEVT_1_1_0200B_1`, motion 21) and the treasure box that bursts open
(`FCHR_G_BOX01`, animation 60).

### Header (0x28 bytes)

| off  | type | meaning |
|------|------|---------|
| 0x00 | 4    | magic `INSA` |
| 0x04 | u32  | version: 2 or 3. Version 2 nodes have 3 track records, version 3 nodes have 4 |
| 0x08 | u16  | node count; entries match the model's node table by index (verified) |
| 0x0A | u8   | count of entries in the second section (material tracks?) |
| 0x0B | u8   | count of lens tracks: 1 in a camera move, 0 elsewhere |
| 0x0C | u32  | start time (0 in almost every file) |
| 0x10 | u32  | end time. The length is end minus start |
| 0x14 | u32  | offset of node entries |
| 0x18 | u32  | offset of the second section |
| 0x1C | u32  | offset of the lens tracks; with none, of the 4 zero bytes that end the file |
| 0x20 | i32  | loop start (version 2 and later; before that the loop is the whole clip) |
| 0x24 | i32  | loop end |

The loop range is what a looping playback repeats. It is the whole clip in most files.
Where it is shorter the game uses it as a hold or repeat section: the player's shooting
animation `btl3000` has the range 38..46 frames, and the code that fires a shot plays
exactly "from frame 38 to 46" (the executable reads these fields at 0x1DEF0 and
0x1DF10, and builds its animation object at 0xA8B28).

### Node entry, 0x24 bytes each

| off  | type  | meaning |
|------|-------|---------|
| 0x00 | u8    | flags: bit 0 = this animation does **not** drive the node (see "Layers"), bit 1 = has a child (as in the model's node table) |
| 0x01 | u8    | number of track records: 3 or 4 on characters in battle, 5 in event scenes, 9 on objects with pivots, 0 on a node that just sits on its parent |
| 0x02 | u8    | node id, same value as byte 1 of the model's node entry |
| 0x03 | u8    | the first node after this one's subtree, 0xFF = none |
| 0x04 | u32   | offset of this node's track records |
| 0x08 | 3×f32 | where the node sits under its parent in the skeleton the animation was made on (see "Fitting") |
| 0x14 | 3×f32 | zero in every file |
| 0x20 | u32   | 0 |

### Track records, 0x18 bytes each: the parts of a node's transform

A node's matrix starts as the identity, and each record in turn is worked out as a
matrix and multiplied **on the left** of what is there (`node = part × node`,
column vectors, translation in the fourth column; the library's multiply is at
0x16205C). So the first record is the innermost. The node is then placed under its
parent as a whole matrix: `world = parent × node`.

All kinds start with:

| off  | type  | meaning |
|------|-------|---------|
| 0x00 | u8    | kind: 0 = translation, 1 = rotation as three angles, 2 = scale, 3 = undo the parent's scale, 4 = rotation as quaternions |
| 0x01 | u8    | bit 1 = one channel does for all three axes (scale only). Bit 0 is set on keyed records; the evaluator does not look at it |
| 0x02 | u8    | kind 1: the order the angles turn in (below); otherwise 0 |

Kinds 0, 1 and 2 continue with three channels:

| off  | type  | meaning |
|------|-------|---------|
| 0x04 | 3×u16 | key count per axis |
| 0x0A | u16   | how each channel carries on outside its keys: two bits for before the first key and two for after the last, for each channel, the first channel's highest (bits 11-10 and 9-8). 1 = repeat the keys, anything else = hold the nearest. Not zero in 10 records of the whole game |
| 0x0C | 3×u32 | offset of each axis's keys `(u32 time, f32 value)`. **A channel with no keys holds its value here instead, as an f32**: that is how a record with mode 0 gives a fixed translation or a fixed turn |

Kind 4 continues differently:

| off  | type  | meaning |
|------|-------|---------|
| 0x04 | u16   | key count |
| 0x06 | u8    | bits 3-2: 1 = repeat before the first key. (The other end is read from a bit that can never say "repeat", 0x2086C.) Zero in every file |
| 0x08 | u32   | offset of the keys `(u32 time, 4×f32 quaternion x y z w)`, 20 bytes each; with no keys the quaternion itself is here. Every file has keys |

Kind 3 has nothing after its head: the matrix is a scale by one over the parent's
scale (the scale the parent's own kind-2 record gave it this frame, each axis kept
above 1e-5). A node whose parent is scaled thus keeps its own size, and only its
place under the parent scales. Skipped on a node without a parent.

The angles of kind 1 are radians. Order byte 0 (every file) and 2 turn about X, then
Y, then Z (the library's quaternion-from-angles at 0x162D58: `q = qz × qy × qx`);
4 turns about Z, then X, then Y (0x162DA0: `q = qy × qx × qz`); any other number
turns nothing.

Between two keys a channel goes straight from one value to the next; a quaternion
track goes along the shorter arc at an even rate (0x162CAC), straight across when
the two are within a cosine of 0.998.

The layouts in the files (kind.mode, in order):

| nodes | records | where |
|-------|---------|-------|
| 77,000 | `2.x 4.1 3.0 0.1`, or without the 3.0 | characters and cameras: scale, quaternion, (undo parent scale), translation |
| 31,000 | `2.x 1.1 1.0 3.0 0.1` | event scenes: scale, keyed angles, a fixed turn, undo parent scale, translation. The fixed turn is the joint's own orientation in the tool the scenes were made in, and the whole is that tool's joint matrix |
| 2,000 | `0.0 2.x 0.0 0.0 0.0 1.x 0.0 0.0 0.x` | objects that turn about a pivot: minus the scale's pivot, scale, the pivot back, a shift, minus the turn's pivot, the turn, the pivot back, a shift, the translation |
| 100 | `1.0 3.0 0.0` and the like | a node that does not move |
| 889 | none | a node with no records sits exactly on its parent |

Time unit: **1024 units per animation frame, 60 frames per second**. The executable
converts a frame count to file time with `start + frame * 1024` (0xA8CD8) and advances
its players by the time step in sixtieths of a second. The player's field walk confirms
it: the cycle is 70 frames (35 game frames at 30 a second), and at the 40/7 units per
game frame her script gives, that is exactly 200 units. (An earlier reading of 4096
units per thirtieth of a second played everything at double speed.) Key times
themselves are not always whole frames.

### Which model node an animation node drives

With as many nodes as the model, they match by index. Otherwise (0x215E8) the two
trees are walked together and matched by id: an animation node with id 0xFF takes a
model node with id 0xFF, any other looks along the model's siblings for its id, and a
model node the animation has nothing for stays as the model has it (0x1EC9C). 86
animations need this, the soldiers' own stand and walk among them (25 nodes on models
of 29): `tools_re/insa_counts.py` lists them. Ported as `animation::match_nodes`.

### Fitting an animation to another skeleton

A node of a skinned model's skeleton (flag 8 in the model's node table) whose parent
is one too has its translation fitted to the model (0x20E1C): the turn that takes
the animation's own place for the node (node entry +8) to the translation the
animation asks for is applied to the model's place for it, and the length is scaled
by the same factor. On the skeleton the animation was made on this changes nothing;
on a model with other proportions the bones keep the model's lengths.

### Blending from one animation to the next

When an animation starts with a blend (0x2314C), each node it drives has its last
rotation and translation copied aside (0x23030), and until the blend is over a
quaternion record's value is turned from the copy by the blend's weight and a
translation record's comes straight from it. Scale and angles do not blend. What is
copied is what the animation before gave **before its own blend**, so cutting a blend
short jumps.

### Layers

A character plays several animations at once, each on its own player, and a node
flagged "not driven" (bit 0) is left to another layer. The player's body animations
do not drive her twin tails (nodes 7 to 13); a hair animation that drives only those
plays alongside (`50.insa` in the field bundle, `chr_p_brs02_fld9700_hair.anm` in
battle), and in the field `51.insa` drives only the head and face nodes. In the
field these layers are set up by the character script's `define_gimmick` (gimmick 0
plays animation 50, gimmick 1 animation 51; gimmicks 2 and 3 are chains simulated on
the nodes with ids 100 and 101, the roots of the twin tails, which the port does not do
yet). Implemented in `crates/brs-engine/src/animation.rs`.

### Second section: material tracks — not read yet

Most field animations have some (up to 9). The code that plays them follows the
nodes' in the same function (0x219C0 on): an entry can slide a material's texture
(two channels of 16-bit keys in 2048ths) and scale its colour. Not decoded further.
One entry in a sample: `u32 1, u32 offset, u32 offset, 0x80000000, 0, 4×f32 1.0`, then
a small table with two keys at times 0 and 0x2000 holding the value 0x11.

## Camera moves (`.cam`)

Status: implemented in `crates/brs-formats/src/cam.rs` and played by
`crates/brs-engine/src/camera.rs`. All 261 `.cam` files parse, and the two cameras of
the stage-select menu that are stored as `.insm`. Checked by eye in `brs-viewer
--camera` on the player's victory pose and in battle on her defeat.

A file is an `INSM` header and node table with no geometry, a camera record, and then
a whole `INSA` file:

| off  | type | meaning |
|------|------|---------|
| 0x04 | u8   | 1 |
| 0x07 | u8   | 1: the file holds a camera (0 in a model) |
| 0x08 | u8   | node count: 1 in 206 files, 5 in 55 (a rig, with the camera the last node) |
| 0x18 | u32  | offset of the node table (as in a model, 0x20 bytes a node) |
| 0x24 | u32  | offset of the camera record |
| 0x34 | u32  | offset of the `INSA` part; a camera that never moves ends there |

Camera record: byte 2 is the index of the camera's node; f32 at +4 and +8 are near
and far distances the game does not use (it keeps its own), +0x0C the focal length in
millimetres, +0x10 always 1, +0x14 the film's width in inches (1.4173 in every file).

The `INSA` part animates the nodes like any other animation (the camera node also
carries a constant scale of 50, which the game ignores), and has one lens track at
the offset in its header's field 0x1C: 16 bytes, `u8 3`, three zero bytes, two u16 key
counts and two u32 offsets of `(u32 time, f32 value)` keys. The first channel is the
focal length, animated in 200 of the files (between about 6 and 200 mm); the second
scales the picture's width and is 1 throughout every file.

The camera node looks along its own -Z with +Y up. How the game turns that into its
camera, and the focal length into an angle, is in `docs/BATTLE_NOTES.md`.

## Recordings — ATRAC3plus in `RIFF` (`SOUND/*.AT3`, `.riff` and `.at3` elsewhere)

The music (32 files), what is spoken in the field's events (slot 5 of an event's
bundle, 3,377 files), the battle's voices (`BTL/CHR/*.VOL`, 736), the jingles
(`SYSTEM.VOL/jin_pack.bin`) and a few more: 4,154 files, all `RIFF` `WAVE` files of
ATRAC3plus at 44.1 kHz, the music in stereo at 64 kbit/s, the rest mono.

| chunk | what |
|-------|------|
| `fmt ` | the extensible format (tag 0xFFFE): channels at + 2, rate at + 4, block size at + 12 (376 bytes, or 192), the codec in a GUID |
| `fact` | the samples the recording has; then, for ATRAC3plus, two more words, the last of them **the samples a decoder gives before the recording's first** (2232 for speech: a frame of 2048 and the codec's 184; about 3,000 for music) |
| `smpl` | for a tune that loops: the count of loops at + 28, and the first loop's first and last sample at + 44 and + 48, **counted in the decoder's samples**, so the lead-in comes off both |
| `data` | the frames |

The port has no decoder for the codec. `brs-extract --sound` has ffmpeg make an Ogg
Vorbis file beside each recording, cut to the recording's own samples
(`atrim=start_sample=<lead-in>:end_sample=<lead-in + samples>,asetpts=N/SR/TB`; with
the decoder's own time stamps the file comes out claiming half its length). The
result has the recording's sample count to the sample, give or take one, which the
port evens out when it loads it; so a loop given in samples lands where it should.
The original files stay: the port reads the length and the loop from them
(`crates/brs-formats/src/at3.rs`).

## Sound banks — `PPHD8` (`.phd` + `.pbd`, `.pphd` + the next slot)

The short sounds: 248 banks, 779 sounds, an hour of them in all. A bank is a header
and the data it points into: `btl_*.phd` / `.pbd` in an enemy's archive (54), `fin_*`
in the archives of the finishing moves (22), one in `BTL/SKL`, and in the field's
bundles `06.bin/00.pphd` with its data in `06.bin/01.bin`: the maps' (40), the
events' (130), and `SYSTEM.VOL/fld_pack.bin`, the bank that is always loaded (0x308C;
189 sounds: the interface's, the field's and the battle's).

**Header.** `PPHD8`, then at + 0x10, + 0x14 and + 0x18 the offsets of three chunks,
each a name, its size, the length of a record (+ 8), the numbers of its first and
last record (+ 0x10, + 0x14) and the records from + 0x20:

| chunk | record | what |
|-------|--------|------|
| `PPPG` | | programs: 128 offsets from + 0x20. The game does not use them |
| `PPTN` | 0x60 | tones: **what a sound's number names** (0x15F054). + 0 the number of its sample; + 0x20 and + 0x28 volumes, 0xC80 of 0x1000; + 0x24 and + 0x2C places between left and right, 0x40; + 0x30 the note it is recorded at, 0x40; + 0x50 and + 0x54 an envelope, 0x800F and 0xDFE0, which is the sample as it is. **Every tone of the game has exactly these**, so a sound is its sample |
| `PPVA` | 0x10 | samples: where in the data, the rate (22050 or 44100, give or take), the length |

**Samples** are the PlayStation's ADPCM: frames of 16 bytes, a byte of filter (high
half) and shift, a byte of flags, and 28 samples of four bits, low half first. A
sample is its four bits, signed, at the top of 16 and down by the shift, plus
`(last * a + before * b) >> 6` with (a, b) one of (0, 0), (60, 0), (115, -52), (98,
-55), (122, -60). Flags: 1 the sample ends with this frame; with 2 as well it goes
back to the frame flagged 4 and loops (130 of the sounds loop); a frame of flags 7
after a sample that does not loop marks its end and holds nothing.
`tools_re/pphd_check.py` compares the port's decoding with ffmpeg's: the same to
within rounding.

**Playing one** (the library at 0x15DAE8 on, over the PSP's software synthesiser, 32
voices mixed into the music's output, 0x12384): `0x15E3A0(bank, sound, note, 0,
{volume, place, ...})`. The pitch is the sample's rate times two to the power of
(note - 0x40) / 12, and never more than four times 44100 (0x15EEC4, 0x15ED14); the
volume to each side is 0xC80 / 0x1000 times the volume asked, of 127, and to one
side of the middle the other side falls away (0x1616F4). The game's own calls are
0xD34A0 (bank, sound, volume: note 0x40), 0xD3554 (with a note), 0xD3614 (stop),
0xD3674 (volume), 0xD36DC (place); the volume asked is first scaled by the option
for sounds.

In the port: `crates/brs-formats/src/pphd.rs` (header and decoding),
`crates/brs-game/src/sound.rs` (playing). `tools_re/pphd.py` lists a bank,
`pphd_survey.py` all of them.

## EGTB — enemy groups (`SYSTEM.VOL/enmgrp2.dat`)

Status: implemented in `crates/brs-formats/src/egtb.rs`: groups, the members that
stand there as a battle begins, the waves kept back for later, the places those can
come in at, and the mode that says by what rule. Layout, and how the waves come in, in
`docs/BATTLE_NOTES.md`. A group's flags (u16 at +2): 1 is set on all 484 groups; 2
(117 groups) has the player strike her victory pose when the battle is won; 4 (48
groups, the boss battles) starts the battle without the encounter's screen wipe.

## XTC — record tables (`SYSTEM.VOL/db_resident.lpk`)

Status: implemented in `crates/brs-formats/src/xtc.rs`; all 34 files parse. Header:
`XTC\0`, u32 header size, u32 record count, u16 record length at 0x0C; fixed-length
records from 0x10; strings after the records, referred to by offset from the start of
the file. What the tables hold (items, equipment, skills, the level table, starting
figures) is in `docs/BATTLE_NOTES.md`.

## EDXD — enemy figures (`.edx` in an enemy's archive)

Status: implemented in `crates/brs-formats/src/edxd.rs`; all 88 files parse. Eleven
numbers a battle script reads by id (-1000 to -990) with its engine function 62; their
offsets are in `docs/BATTLE_NOTES.md`.

## SC — slot bundles (everything under `FLD/`, plus `fld_resident.bin`)

Status: implemented (`container::parse_sc`); `brs-extract` unpacks them recursively.

Header: magic `SC`, u16 slot count, then one u32 word per slot, usually followed by
one more that is the bundle's size (two bundles of `fld_resident.bin` leave it out:
their last slot runs to the end of the data). Slots are aligned to 16 bytes, so the
low 4 bits of a slot's word are free, and hold **the number of padding bytes at the
end of that slot**: slot `i` starts at `word[i] & !15` and ends at
`(word[i + 1] & !15) - (word[i] & 15)`. The original computes sizes this way at
0xE7064. Taking the whole word as the offset is nearly right and badly wrong: it
cuts the first 4 to 12 bytes off every slot that follows a padded one (map scripts
come out in two pieces, `SSCR` files without their headers, most collision empty).
An empty slot has the same start as the next. Slots frequently hold further SC
bundles with other slot counts (6, 8, 16, 32, 61, 64 seen). The slot number is the
meaning: each position has a fixed role.

Top-level layouts seen:

| Files | Slots | What is where |
|-------|-------|---------------|
| `FLD/MAP/*.BIN` (88) | 16 | 0 = `SSCR`; 4 = SC of 32 with effects (`EFC`); 10 = the map's script, a `bscr` (see `FIELD_NOTES.md`); 11 = SC holding the map: its slot 1 is an SC with the `STCM` geometry (one large, one small), slots 3 and 6 are `CLIM` collision |
| `FLD/FEVT/*.BIN` (690) | 16 | 0 = `SSCR`; 1, 2, 6, 7, 8, 9 = SC; 3, 5 = SC of 64; event scenes with their models and animations; 464 of them hold a header-only `LPK` stub |
| `FLD/FCHR/*.BIN` (177) | 8 | 0 = `SSCR`; 1 = SC with the model; 2 = SC whose slot numbers are animation numbers (unpacked as `02.bin` or `02.sc4`) |
| `SYSTEM.VOL/fld_resident.bin` | | 0 = the field's tables, an `SSCR`-format file of data symbols (`lst_map`, `lst_fchr`, `prm`, ...); 4 = the three behaviour scripts of the enemies on the field; 5 = more tunables (`turn_frame`) |
| `SYSTEM.VOL/fld_pack.bin` | | 8 = the field's system script (`mission_start`, `update_player`, ...) |

After unpacking everything with the fixed reader (`brs-extract`: 15,648 archives, no
errors): 363 models, 3,459 animations, 263 camera moves, 1,597 `SSCR` scripts, 203
`bscr` scripts and 123 collision files parse.

## PDK — the packs of the ride on the bike (`MINI_GAME/BMG_*.BIN`)

`PDK\x01`, u16 members, u16 where the members begin, then for each member a word
for where it is, counted from there, and a word for how long; a member there is
none of has nought for both. A member's number is what the game finds it by.
`container::parse_pdk`, unpacked by `brs-extract` into `NNN.<kind>`; what is in
them is in `docs/BIKE_NOTES.md`.

## bms — compiled map scripts (slot 10 of `FLD/MAP` bundles)

A map's script is a `bscr` file like the battle scripts (see `BATTLE_NOTES.md` for
the format and the interpreter), calling the engine through its table 1. What its
functions do, and how the field runs it, is in `FIELD_NOTES.md`. Player and object
placement on a field map is done by these scripts; there is no static placement
table.

## STCM — field and battle map geometry

Status: implemented in `crates/brs-formats/src/stcm.rs` for geometry and textures.
All 204 files parse (2,542,401 triangles). Checked by eye in `brs-viewer` on one field
map (`FLD/MAP/MAP_D_STG01_01.BIN/11.bin/01.bin/00.stcm`): streets, buildings and
backdrops display with the right textures and baked lighting.

### Header

| off  | type | meaning |
|------|------|---------|
| 0x00 | 4    | magic `STCM` |
| 0x04 | u8   | 1 (version?) |
| 0x05 | u8   | texture count |
| 0x08 | u32  | unknown |
| 0x0C | u32  | unknown (an offset near the end of the vertex data) |
| 0x10 | u32  | offset of the main display list |
| 0x14 | u32  | size of the display-list region; vertex data follows it |
| 0x18..0x2F | | more offsets and two values (e.g. 0xF000, 0x320000); unknown |
| 0x30 | u32 × texture count | offset of each `PTMD` texture |

### Display lists

The main list is `ORIGIN`, then per strip: `BASE`, `CALL` texture list, `BASE`, `CALL`
material list, `BCE` (culling), `VTYPE`, `BASE`, `VADDR`, `PRIM` (triangle strip), and a
final `RET`. Addresses are relative to the `ORIGIN` word, as in INSM.

Texture lists start straight after the main list's `RET`, one per texture in table
order. A paletted texture's list is 12 commands: `TMODE`, `TPSM`, `TBIAS`, `TBW0`,
`TBP0`, `TSIZE0`, `CMODE`, `CBPH`, `CBP`, `CLOAD`, `TFLUSH`, `RET`. A direct-colour
texture's list is 8 commands (no palette commands). The texture addresses in them are
zero; the engine patches them at load time. So **the k-th list after the main list
belongs to texture k**; walk them in order, since their lengths differ. The reader
cross-checks the width in `TSIZE0` against the texture's own header, and that check
passes on all 204 files.

Material lists follow the texture lists. Two kinds seen: textured (`TWRAP`, `UOFFSET`,
`VOFFSET`, `TME 1`, `TFUNC`, `ALPHA`, `ATST`, `ZMSK`, `FGE`) and untextured (`CMAT`,
`EMC`, `AMC`, `AMA`, `DMC`, `SMC`, `TME 0`, ...). Only `TME` is interpreted so far.

`TFUNC` is 0x010100: modulate, with colour doubling on. Final colour is
texture × vertex colour × 2, so a vertex colour of 0x80 leaves the texture unchanged
and a lighter one lightens it, up to twice; the product is cut off at full only
after the doubling.

### How the colours are mixed

The PSP does all of this on the numbers the textures and colours hold: it
multiplies them, mixes the result with the fog's colour by distance and with what
is on the screen by alpha, and shows what comes of it. A graphics card of today
is used to turning such numbers into light first and back at the end, which mixes
otherwise: a pale fog a fifth of the way in lifts a dark wall half as far again,
and a see-through panel is easier to see through. So the port gives the card the
game's numbers as they are: textures are plain numbers (`Rgba8Unorm`), the meshes'
colours and the fog's too (`brs_engine::texture::values`), and the cameras of the
scenes mix them as such, with nothing mapped to a film's look or dithered
(`camera_of_values`: Bevy's `CompositingSpace::Srgb`, `Tonemapping::None`). The
interface's canvas has always worked this way (`canvas.rs`). Until this was done
the port's scenes were duller than the PSP's and hazier in the distance.

### Vertices

Every strip in the sample uses vertex type 0x00019F: UV 2×f32, colour RGBA8888,
position 3×f32, 24 bytes. No normals; lighting is baked into the colours. Positions
are in world space.

### The order of drawing

The PSP draws the main list as it stands: strip after strip, each tested against
the depth of what was drawn before it (nearer or as near passes, 0x10EBD0), each
writing its depth unless its material's `ZMSK` says not, and each mixed with what
is on the screen by its material's `ALPHA` (blend) command. Nothing is put in
order by how far it is. Maps count on it: 564 of the field maps' runs of strips
are both seen through in part (by their corners' alpha or their texture's) and
write depth, at every place in their lists. The clearest case is the hall of the
story's opening (`MAP_D_STG05_0A1`): its polished floor is the first thing in the
list, with corners whose alpha goes from 0 to 255, and the hall hangs a second
time upside down beneath it. Drawn first, the floor is mixed with the colour the
screen was cleared to (near black there: the scene script's function 24,
`FIELD_NOTES.md`) and hides the hall beneath it wherever it is drawn at all;
only where its alpha is nothing, and the alpha test drops the point, is the
hall beneath seen. Drawn last, as a card of today would have what is seen
through, the pale sky of the hall beneath shows through it in great flat
shapes, which is what the port drew until 2026-10-08.

So the reader keeps the list's order (`Map::batches`: a batch is a run of
strips drawn alike, 10,723 of them in the 159 files of the field's maps, 503 at
most in one), and `brs_engine::map` draws every batch of a scene's maps in one
pass in that order with a material of its own (`MapMaterial`, `map.wgsl`):
tested against the depth, writing it or not as the list says, mixed by the
list's blend (laid over, added, taken away, or all there), the picture's alpha
counted or not and the picture added to the corner's colour where the texture
function says so, cut off at full before the fog, and the fog by how deep in
the view a point lies, as the PSP's is. The characters are drawn before it and
everything else that is seen through after it.

A field map's two pieces are drawn in this order (0x10EC28): the second of the
bundle first (`01.stcm`, at the field's + 0x13C: the sky and what stands far
off, every list of it without fog), with a lens of a range of its own
(0xD9F48: the projection at the camera's + 0x110), with the depth neither
tested nor written, and before the scene's fog is set (0x10E6A0); then the
first (`00.stcm`, + 0x134), tested and written, with the lens of the scene
(0xD9F88: + 0xD0). So what the map proper mixes with, where nothing of it was
drawn before, is the far piece and not the cleared screen: the opening hall's
backdrop is a see-through sheet of the first piece over the sky of the second.
The port draws the far piece first too (`map::Piece::FarOff`); it tests it
against the depth all the same, since the port's characters are drawn before any
of a map, and has it write none.

`brs-extract --map <file.stcm>` lists a map's batches in order with how each is
drawn.

### Not done

The fog itself is not the map's but its
scene's: a field map's scene script sets it (`FIELD_NOTES.md`), a battle map's
script its own (`BATTLE_NOTES.md`).

## CLIM — collision meshes (slots 3 and 6 of a field map's inner bundle)

Status: implemented in `crates/brs-formats/src/clim.rs`. All 123 files parse (30,395
triangles). Checked by eye as an overlay on `MAP_D_STG01_01` in
`brs-viewer --collision`.

An octree over a triangle soup. All offsets are plain file offsets.

| off  | type | meaning |
|------|------|---------|
| 0x00 | 4    | magic `CLIM` |
| 0x04 | u32  | offset of the root node (0x18 + 4 × attribute count) |
| 0x08 | u32  | offset of the vertices: 3 × f32 each, running to the end of the file |
| 0x0C | u32  | 0x14 in every file; unknown |
| 0x10 | u16  | node count, including the root |
| 0x12 | u16  | attribute count |
| 0x14 | 4 × count | attribute table, 4 bytes each (below) |

Node, 0x40 bytes: `u32 flags` (bits 8 and up = number of triangle groups it owns),
`u32 child[8]` (0 = none; the eight octants), `u32 offset of its first group`,
`f32 bbox[6]` (min xyz, max xyz).

Group, 8 bytes: `u8 0`, `u8 attribute index`, `u16 triangle count`, `u32 offset of
u16 index triples`.

Roles of the two slots, from the triangle orientations across every file:

- slot 3: the physical world. Floors, walls, slopes and a few ceilings.
- slot 6: almost all walls. Invisible boundaries fencing in the playable area. It is
  the one the camera that follows her is kept in by (field + 0x118, 0xE0650; see
  "The camera" in `docs/FIELD_NOTES.md`).

An attribute, as what a ray or a sweep has met is noted (0x13EC8, which every one
of the collision's queries goes through: 0x151F8, 0x153E8, 0x17690):

| byte | meaning |
|------|---------|
| 0 | bits: 1 the triangle stops nothing, it only marks the place; 2 byte 1 counts; 4 byte 2 counts |
| 1 | the kind of ground, which the sound of a step is chosen by |
| 2 | the number of the scene's light that falls there |
| 3 | 0 in every file; not read |

A query keeps three things apart: the nearest triangle that stops it (bit 1
clear), the nearest that says a kind, and the nearest that says a light, whether
or not those stop it. So `06 00 00 00` is solid ground of the first kind in the
first light (6,878 triangles in 34 maps), `06 05 00 00` the same of kind 5, and
`05 00 01 00` or `07 00 01 00` a sheet that only says "light 1": the maps have
such sheets, level, 12 to 20 above the ground of every stretch that a building
or a bridge shades (615 and 41 triangles), and `03 07 00 00` likewise for a
floor that sounds otherwise. Slot 6's are `00 00 00 00` but for a few.
`tools_re/out/_clim_regions.py` lists them, for one map or all.

## CPMD — the map of a camera's path (slot 8 of a field map's inner bundle)

Status: implemented in `crates/brs-formats/src/cpmd.rs`, with the camera it belongs
to in `brs_engine::camera::CameraPath`. All 91 paths of the 41 maps that have any
read (`data::tests::every_path_of_every_maps_camera_reads`). Checked by eye on stage
1-1, whose one path looks over the fallen building in its south-west.

Slot 8 of the inner bundle (`11.bin/08.bin`) is an `SC` bundle with one slot for each
path, by the number scripts know the path by (`s124`, `s45(trap, 27, path)`). A path
is an `SC` bundle again: slot 1 this grid, slot 2 an `INSM` rig that is a camera
(see "Camera moves"), slot 3 the `INSA` move of it.

| off  | type | meaning |
|------|------|---------|
| 0x00 | 4    | magic `CPMD` |
| 0x08 | u16  | cells across, along X |
| 0x0A | u16  | cells deep, along Z |
| 0x0C | f32 × 4 | where the grid begins and ends along X, then along Z |
| 0x1C | u32  | offset of the points (0x20) |

The points are u16, one more each way than there are cells, row by row with a row
running along X. A point is how far along the move the camera is with her standing
there: 0 the move's first frame, 0xFFFF its last. Between points the four about her
count each by how near it is, and beyond the grid its edge holds (0x18254). The
frame is the move's first time plus that part of its length (the `INSA` header's
+ 0x0C and + 0x10, 0xDABAC).

## SSCR — compiled scripts

Status: implemented in `crates/brs-formats/src/script.rs` (loader and interpreter,
including engine functions that make the script wait). Run for real: the field
characters' scripts (speeds, animation layers) and the behaviour script of the
enemies on the field (`FIELD_NOTES.md`). All 1,597 files parse.

Found as slot 0 of every `FLD/FCHR`, `FLD/FEVT` and `FLD/MAP` bundle, and in
`SYSTEM.VOL`. A file whose symbols are all data is used as a table (`fld_resident`).

### Layout

| off  | type | meaning |
|------|------|---------|
| 0x00 | 4    | magic `SSCR` |
| 0x04 | u32  | 0x00020000 |
| 0x08 | u16, u16 | symbol count, relocation count |
| 0x0C | u32  | offset of the symbol table |
| 0x10 | u32  | offset of the relocation table |
| 0x14 | u32  | offset of code and data |
| 0x18 | u32  | offset of the import table |
| 0x1C | u16, u16 | import count, import fixup count |
| 0x20 | u32  | same as 0x18 in the files looked at |
| 0x24 | u32  | offset of the import fixups |

Nine scripts are of a first version (the word at 4 is 0x00010000): four maps' own
scripts and their scenes' (`MAP_D_STG01_0G`, `STG03_02`, `STG03_02C`, `STG05_0A`)
and one event's. Their header ends at 0x14, after the offsets of the symbol table
and of the relocations (the game reads both versions the same way that far,
0x1122B0 and 0x1122D8); such a script has no imports.

- Symbols: `(u32 address, u32 name offset)` pairs, then the names. A symbol is a
  function or plain data; nothing marks which.
- Relocations: `(u32 location, u32 target)` pairs. The 32-bit immediate at `location`
  is zero in the file and becomes the address of `target` when loaded.
- Imports: `(u32 0, u32 name offset)` pairs; fixups are `(u32 location, u32 import
  index)`. The immediate at `location` becomes a call target for that import.

### Virtual machine

Read from the interpreter at 0x112790 in `EBOOT.OLD` (jump table at 0x16DE30, 65
opcodes). Register-based: registers are 32-bit slots of the call context and operands
are register numbers. r0 = program counter, r1 = frame pointer, r2 = stack pointer
(grows down), r3 = return value, r4 and r5 = scratch.

Two-register forms are `op src dst` and compute `dst = dst op src`.

| opcode | bytes | meaning |
|--------|-------|---------|
| 00 / 01 | 2 | push r / pop r |
| 02 | 3 | mov src, dst |
| 03 | 6 | dst = imm32 (`03 imm32 dst`) |
| 04 / 08 / 0C | 7 | load 32 / 16 / 8 bits: `op imm32 base dst`, dst = mem[base + imm], narrow ones sign-extended |
| 05 / 09 / 0D | 7 | store 32 / 16 / 8 bits: `op src imm32 base` |
| 07 | 4 | dst = sign-extended imm16 |
| 0B | 3 | dst = sign-extended imm8 |
| 10, 11 / 12 | 3 | add (int) / add (float) |
| 13, 14 / 15 | 3 | subtract |
| 16, 17 / 18 | 3 | multiply |
| 19 / 1A / 1B | 3 | divide signed / unsigned / float |
| 1C / 1D | 3 | remainder signed / unsigned |
| 1E, 1F / 20 / 21 | 3 | shift left / arithmetic right / logical right |
| 22 / 23 / 24 | 3 | and / or / xor |
| 25 / 26 / 27 / 28 | 2 | bitwise not / logical not / negate int / negate float |
| 29 / 2A | 3 | float to int / int to float |
| 30 / 31 / 32 | 3 | compare signed / unsigned / float: sets "equal", and "less" when the **second** operand is less than the first |
| 33 / 34 | 2 | r = equal / not equal (1 or 0) |
| 35 / 38 | 2 | r = less / not less (2 or 0: the flag bit itself) |
| 36 / 37 | 2 | r = greater / less-or-equal (1 or 0) |
| 39 | 5 | jump to imm32 |
| 3A / 3B | 5 | jump if equal / if not equal |
| 3C | 5 | call imm32: push return address, jump |
| 3D | 1 | return: pop the program counter |
| 3E | 2 | call through a register: if bit 0 of the value is set it is engine function `value >> 8`, otherwise a script address |
| 40 | 5 | call engine function imm32 |

06, 0A, 0E, 0F, 2B-2F and 3F do nothing in the interpreter and are not used.

Calling convention: arguments are pushed last-first, so the first argument is lowest;
the caller removes them afterwards. Functions start `push r1; mov r2, r1` and end
`mov r1, r2; pop r1; pop r0`. The engine starts a call by pushing a zero return
address, and the run ends when the program counter becomes zero. An engine function
sees its arguments at the stack pointer and returns through r3.

An engine function makes the script wait by clearing the context's "running" bit
(0x1134A4). When the script is next run the interpreter calls that same function
again (its number is kept at context + 0x1C) with the same arguments; the function
can tell a first call from a later one (0x113510 answers 0, then 1), and the script
goes on once the function sets the bit again (0x1134B8). `script::Call::wait` and
`Call::resumed` are this.

### What the player's field script contains

`FLD/FCHR/FCHR_P_BRS00.BIN/00.sscr`. Functions the engine looks up by name: `init`,
`set_status`, `update_time`, `get_move_speed`, `define_gimmick`; data it looks up by
name: `num_gimmick` (4), `gim_facial` (1). Not present here but looked up when it
exists: `get_stat_anim` (status to animation override).

`get_move_speed(status)` gives distance per frame: 40/7 (about 5.71) for status 0,
32 for status 1 and any other, 96 for status 0x40, and for the player alone 1.0 for
status 0x41. The script's engine calls seen so far: function 8 sets an integer
parameter, 9 reads one, 10 sets a float parameter; imports `fchr_z_rsd_set_face`
and `fchr_z_rsd_update_foot_se`.

## Field characters in the executable

Facts read out of `EBOOT.OLD` (addresses are in the loaded image, file offset − 0xC0).
Exploration tools: `tools_re/mips.py`, `mips_find.py`, `vm_handlers.py`, `sscr.py`.
`mips.py` also decodes the PSP's vector unit (`vfpu.py`), which the game's maths
library (0x161B00 to 0x162E24, Sony's) is written in: matrices are stored column by
column with the translation last, vectors are columns, and `0x16205C(out, a, b)`
is `out = a × b`.

- Default animations for every field character: stand = 0, walk = 10, run = 11, set
  in the constructor at 0xFCA00 and re-read at start-up (0xFCBA0) through the script's
  `get_stat_anim` if it has one. Default blend between them: 0x4000 time units, which
  is 16 animation frames (0.27 s).
- Status is a byte at object + 0x160. Changing it (0xFDB24) plays the new status's
  animation (0xFDA60, mapping in 0xFD960) and caches `get_move_speed(status)` at
  object + 0x8E8. Mapping: status 0 walks, 1 runs, 3 stands (animation 5 instead if
  game flag 0x102D is set), 12 is animation 12, 0x41 is animation 22, anything else
  asks the script. Entering status 12 blends over 0x2000 units (8 frames) instead.
- Animation players are stepped by an integer number of time units on each update
  (0x234C8), looping between the file's loop start and end. The size of that step
  and the field's update rate have not been read out of the code; the walk cycle
  arithmetic (see INSA) implies speeds are per thirtieth of a second.
- The two movement statuses are bytes at object + 0x8F6 and + 0x8F7, both 1 (run)
  by default, with their speeds cached at + 0x8E8 and + 0x8EC (32.0 by default).
- The character's script instance is at object + 0x7C. Parameters are set through a
  46-case switch at 0x10A720.
- Bundle layout (`FLD/FCHR/*.BIN`): slot 0 the script, slot 1 the model, slot 2 an
  SC whose slot numbers are the animation numbers.
- A script may also define the floats `pop_height`, `pop_radius`, `destroy_radius`
  (100 each if absent) and `shadow_radius` (60), read at 0x102888.

The per-frame movement, the actions, the map's script and the enemies' behaviour are
in `docs/FIELD_NOTES.md`; battles are in `docs/BATTLE_NOTES.md`. Not read yet: the
field camera's follow code.

## The font's pack (`SYSTEM.VOL/font.lpk`)

Status: implemented in `crates/brs-formats/src/font.rs` (`Font`, `Codes`, `Icons`),
`chunks.rs` and `sbs.rs`; both fonts, the icons and the code table parse. Read from
the font library (0x1D124 on) and the set-up at 0x34E0. Tools: `tools_re/font.py`
(tables, and a sheet of glyphs as a picture), `chunks.py`.

Four files. `003.xtc` is an `XTC` table of sixteen colours, four bytes each (red,
green, blue, alpha): the colours a text's colour code picks from (see
`FIELD_NOTES.md`).

**`000.bin` and `001.bin` are fonts**: the text's (3,354 glyphs: Japanese, and the
full-width forms of the Latin letters, which are what the game draws English with)
and one of big capitals and figures (40 glyphs).

| off  | size | meaning |
|------|------|---------|
| 0x00 | 4    | offset of the code table: the header, palette and glyphs before it |
| 0x04 | 4    | bytes of palette (0x40) |
| 0x08 | 4    | bytes of glyphs |
| 0x0C | 4    | bytes of the code table (two a glyph) |
| 0x10 | 4    | bytes of the metric table (eight a record) |
| 0x14 | 4    | flags: bit 31 there is a palette; the low two bits how a glyph is found (3: by the code table) |
| 0x18 | 2    | glyphs |
| 0x1A | 1, 1 | a glyph's width and height (15, 15) |
| 0x1C | 1    | bits a pixel (4) |

The palette (0x20) is sixteen whites, each with the alpha that makes the glyph. A
glyph is 15 by 15 pixels row by row with nothing between rows, the first pixel of a
byte in its high half, in 113 bytes. After the glyphs: the code table, the glyphs'
characters in rising order as Shift-JIS (a glyph's place is its code's place); the
metric table, eight bytes for each character that is not as wide as the font (three
signed words: how far right of the pen it is drawn, how far above its line, how far
the pen goes on); and those characters' codes, in rising order. A character with no
metric is drawn at the pen and is 15 wide. The library copies the glyphs it needs
into a texture of its own (128 by 128 for the text's font) and draws from that; the
port makes one picture of both fonts.

**`SYSTEM.VOL/ucs2jis.bin`** turns a character into its code: two bytes for each of
65,536 characters, the JIS code or zero. A code of two bytes is made Shift-JIS by
rule (0x15C940).

**`002.sbs` is the icons a text can have in it**: a file of chunks (below) with a
sprite table (`SBS`), a texture (`PTM`, 64 by 240) and a list (`DAT`: u32 count, then
12 bytes an icon: its number, its sprite, how far right and down of the pen it is
drawn, whether it is drawn to the text's size, and the size it is drawn as it is
at). 80 icons: the four buttons (0 circle, 1 cross, 2 triangle, 3 square), L and R
(0x29, 0x2A), select and start (0x2B, 0x2C, 23 wide), the directions (0x2D to 0x30),
and the signs of items, skills and status effects.

## Files of chunks, and sprite tables (`.sbs`)

Status: implemented in `crates/brs-formats/src/chunks.rs` and `sbs.rs`. Walked at
0xD2F18. A chunk: a tag of three letters and a zero, a byte of flags (0x80 on every
chunk, 0x08 when it is aligned to 16 bytes instead of 4), the contents' length in
three bytes, most significant first; then the contents, header and contents each
padded to the alignment. `EOFC` ends the file.

A sprite table (`SBS`, 0x2E15C): a word that says how the four fields of a record
are stored (three bits each: 0 a byte, 1 a signed byte, 2 a word, 3 a signed word,
7 four bytes), a byte of how many sprites, then the records: x, y, width, height of
each sprite in the sheet's texture.

## Layouts of the interface (`SDOa`, `XBN`, and the `.bin` after them)

Status: implemented in `crates/brs-formats/src/layout.rs` (`Objects`, `Motions`,
`Groups`); all 10,310 `SDOa`, 81 `XBN` and 79 group files parse. At work in
`crates/brs-engine/src/layout.rs`. Read from the layout library (0x2E424 to
0x33C44) and 0x25430. Tools: `tools_re/sdoa.py`, `xbn.py`.

A screen of the interface is three files that follow each other in a pack, and the
textures after them:

**Objects (`SDOa`)**: u16 objects, u16 textures, then 16 bytes an object from 8: u16
kind (bit 0 it is drawn, bit 1 it has a texture), u16 texture (0xFFFF none), its
place on the screen (two words), its width and height, and the offset of its
commands. An object that is drawn is one strip of four corners, and its commands are
a list of the PSP's own drawing commands with the corners after them: texture on
(0x1E), dither (0x20), the texture's filter (0xC6: smooth for all but 160 objects),
the blend (0xDF: 0x32, the usual; 0xA2 with a fixed colour of white, 0xE1, which adds
to what is behind: 124 objects), what a corner is made of (0x12: 0x19F texture,
colour and place; 0x19C colour and place; 0x183 texture and place; 0x180 place
alone), where the corners are (0x10, 0x01, counted from the list's start), the strip
(0x04) and the end (0x0B). A corner's place is counted from the object's own, so an
object that turns or grows has its corners round (0, 0). Kind 0 objects only mark a
place.

**Motions (`XBN`)**: u32 sets, u32 clips, an offset for each. A set is u32 tracks and
16 bytes a track: u16 group, u16 clip, u16 frames before the clip starts, u16 flags
(bit 0: not drawn), two floats the group is moved by. A clip is u32 channels; a
channel is u32 what it changes (1 x, 2 y, 4 and 5 the scales, 9 the turn in degrees,
11 to 14 red, green, blue and alpha, 0 to 255), u32 keys, a zero, and 16 bytes a key:
u32 frame, f32 value, u32 how the way to the next key goes (0 evenly, 1 slowly at
first, 2 slowly at the end: `t -+ sin(pi t) / pi`), u32 mark (1 where a clip that
goes round goes back to, 2 where it goes back from). Frames are sixtieths of a
second.

**Groups**: u32 the file's size, u32 objects, u32 groups, a colour for each object,
an offset for each group; a group is u32 how many objects and their numbers. A
track moves a group.

**At work**: a layout (0x305B8) is given players (0x308A8, or one for each set,
0x30A68), each with a number of its own. A player keeps tracks: a track keeps nine
values (x, y, the scales, the turn, the colour: 0, 0, 1, 1, 0, 255 four times), each
channel with its own frame, which are stepped by the frames gone by (0x2FA54) and
can be set to a clip's start, its end or a frame. To draw a player (0x30CD4) each
track that is not hidden and has any alpha draws its group's objects: a corner goes
through scale, turn and the track's move, then to the object's place (0x25430:
`T(object) x T(track) x R x S`), and its colour is multiplied by the track's and by
the layout's own (the PSP's light, whose ambient colour is set to it, 0x15AA20).

**Playing a set** (0x319BC) does not replace what the player has. Each track of the
set takes over the track the player has for its group (the set's first of a group
the player's first of it, and so on, 0x2E8C4) and starts it anew at its clip's
first keys (0x2F6CC); one told to play the clip it already has keeps its delay, its
flags and its offset (0x2F194). A group the player has no track for is given one.
The tracks of groups the set does not have are left playing what they were. So a
set of one group changes one part of what is on the screen: the window people
speak through has such sets for its sign and for the answer chosen. The order
tracks are drawn in is made anew each time (0x2F27C): each in turn is put at the
place its number in the set it was made for says.

**Colours of objects.** The groups' file has a colour for each object: what the
object's corners have. Code that writes a text where an object marks takes it from
there (0x33190) and multiplies it by the colour the group's track has come to
(0x31DE4).

**Mixing with what is there.** The PSP mixes on the values of the screen's pixels,
not on light: blend 0x32 leaves `1 - alpha` of each value and adds `alpha` of the
new one. The port's canvas does the same (`crates/brs-engine/src/canvas.rs`): its
camera keeps values in its picture, and every other camera draws to a picture
that the canvas's camera lays down first, so that the interface is mixed with the
scene as the PSP mixes it. Mixing light instead leaves a dark panel a good deal
easier to see through (a panel of alpha 175 over a pixel of 89 gives 45 on the PSP,
and gave 62).

**The height of a texture.** A `PTMD` texture's header (+ 6, u16) has its width as a
power of two (the top four bits) and its rows as a plain number (the other twelve),
and 38 of the interface's 63 textures have a number of rows that is no power of
two (`if_rsdsys.lpk/009.ptm`, the figures, is 256 by 24). Where a texture is given
to be drawn from (0x2508C) its height is made up to the next power of two
(0x250EC) and that is what the PSP is told (0x15ADB8), so a corner's place in a
texture counts its rows of that height: the figures' cells, twenty rows, end at
0.625. The port's images are of that height too, the rows past a texture's own
empty (`brs_engine::texture::image`). Before 2026-10-04 they were of the texture's
own rows, and everything drawn from such a texture was drawn from the wrong rows.

**Figures.** A number is one object drawn once for each of its figures, with the
texture moved (0x15AE38) to the figure's cell of a sheet. For the sheets that are
files of `if_rsdsys.lpk` a table says how they are laid out (0x171078, 24 bytes a
sheet, 0x2D064; + 8 cells to a row, + 0xC and + 0x10 a cell's width and height,
+ 0x14 the number of the first cell), and 0x2D080 turns a number into how far to
move: `cell width x column / the texture's width` and `cell height x row / the
texture's rows` (its own rows, not the power of two: no sheet has a second row).
The first sheet, `009.ptm`, is ten cells of 20 by 20; the others have cells of 11,
17, 11, 7 and 13 by 12. How many figures a number has is 0x22ACC.

## Pictures of the screen (`IF/PICT/*.PTM`)

A `PTMD` texture of 512 by 256 for a screen of 480 by 272: see "The windows a
script puts up" in `FIELD_NOTES.md` for how the last 16 rows are kept.

## Effects — `EFC` and `EFP` (`.efc`, `.efp`)

Read by `brs-formats/src/efc.rs` (the file) and `esb.rs` (an effect's script),
and played by `brs-engine/src/effect.rs`; `parse_all` reads all 730 files with
every part of them.

730 files (`tools_re/efc_survey.py`, `tools_re/efc.py`): 611 `.efc` (581 under
`FLD`, the 32 slots of effects of each map's bundle among them; 15 under `BTL`,
14 in `SYSTEM.VOL`, one under `IF`) and 119 `.efp` (all but one under `BTL`,
one for each fighter's archive; 13 of them are `EFC` inside). An `.efc` is one
effect with what it is made of; an `.efp` is many effects with what they share.

The head is 16 bytes: `EFC\0` or `EFP\0`, a version (100 or 101, 0x126B08), a
word of nought, and four bytes of counts: effects, models, animations,
textures (`eb102150.efc` 1, 0, 0, 1; `eb205.efp` 5, 8, 5, 0). A chain of parts
follows (0x126E80 walks it): each has a head of 0x20, whose first word is how
far it is to the next part's head, the byte after it the part's kind, and a
name of 24 bytes from + 8; a part of kind 0 ends the chain and has the file's
own name. Every one of the 730 files walks to its end so:

| kind | named | what | in all |
|---|---|---|---|
| 1 | `.esb` | an effect's script: what it lets go, when, and how that moves | 1546 |
| 2 | `.mdl` | a model, `INSM` | 3148 |
| 3 | `.anm` | an animation, `INSA` | 2839 |
| 4 | `.ptm` | a texture, `PTMD` | 981 |

So most of what an effect shows is models moved by animations, both of kinds
the port already reads.

**An effect's script** (`.esb`; `tools_re/esb.py` shows one, and with `--all`
checks every one of the 1546). All its offsets are of 16 bits, from its start,
and nought is none. Its first half-word is the offset of its root, a record
near its end: a half-word, the offset of its timeline, and flags (0x127EE0);
records also have fields before the place they are pointed at, which small
readers take by negative offsets (0x128AD8 to 0x128BAC).

The timeline is entries of 16 bytes (14 in a file of version 100, 0x128218),
ended by one whose first half-word is 0x4000; each makes an element as the
effect is made (0x128234, 0x11E05C):

| at | what |
|---|---|
| + 0, + 2 | the frames it starts and ends at (no end: nought) |
| + 4 | the offset of the element's record |
| + 6 | the entry its element hangs from, -1 for none; only one made before it is found (0x12828C), and three scripts name themselves, a later one or one that is not there |
| + 7 | two bits: how its particles keep to what it is fixed to once they are let go, 0 not at all, 1 to its place, 2 to its turning as well (0x1245CC) |
| + 8 | a signed byte: its place in the order the elements are drawn in, the lower first (0x128768, a sort of the game's own) |
| + 9, + 0xD | bits nothing reads to any purpose |
| + 0xA | the offset of four floats, for fading by distance; never set |
| + 0xC | a signed byte; above nought, the square root of a hundredth of it is kept; always -1 |
| + 0xE | version 101 only: which of the effect's matrices its particles are put by, 0 as they are now, 1 as they were when the particle was let go, 2 as they were when the element was made, 3 none (0x125A4C) |

An element is an emitter, and what is drawn are the particles it lets go
(0x125708 sets one up, 0x125B1C is its frame). Its record: a particle's life
in frames (nought: it never ends); a byte, the hundredths by which a
particle's own pace is spread (its time runs at 1 + random x that); a float
that would be gravity (kept by a hundredth, 0x123E90) and is nought in every
record; its kind at + 8, only ever 0, a flat picture (1762 of them), or 2, a
model (5866); and eleven offsets from + 0xA, each of a run of keys. A key has
a head of eight bytes: its frame, the key the run goes back to when this
key's frame comes (-1 for none: a run that goes round, 0x11F454), a byte whose low four bits shape the mix to the next key and whose
0x10 asks for a curve through four keys (every key in the data has nought
here: a straight mix), and a word that is never set; a frame of 0x4000 ends
the run (0x11F06C to 0x11F704). What follows the head, and so the key's size,
is the run's own, and so are the figures a run has before its keys, which
small readers take by negative offsets (`tools_re/out/_before_runs.py`):

| run | key | values | what | before its keys |
|---|---|---|---|---|
| 1 | 0x14 | a vec3 | the emitter's place | a half-word 2 before: what it is fixed to (0, 6, 11, 16 or 17 in the data) |
| 2 | 0x20 | two vec3 | the shape particles come out of: its sizes, and its turns about x, y and z (0x11C028; set in 182 keys) | 4 before: the shape, 0 a ball of a random radius, 1 a ball's skin, 2 a box, 3 a ring, 4 a round with a height (0x11D254) |
| 3 | 0x1C | a vec3, two floats | the way they go, and an angle and a random more of it by which each is tipped off it (0x11D4A0) | 2 before: the space the way is in (0 or 11) |
| 4 | 0x10 | a half-word, a float | how many are let go at a time, and the frames between one time and the next (0x11D88C) | |
| 5 | 0xC | two shorts | which of the file's textures, or of its models, a particle is of; the second is always -1 | |
| 6 | 0x14 | a vec3 | its size | four floats: one all of it is multiplied by, and three spreads (a particle's size is by 1 - random x spread; one for all three axes when the last two are below nought) (0x1200E8) |
| 7 | 0xC | a float | its speed along its way (0x123C40) | a float, a spread |
| 8 | 0x10 | four half-words | its colour and alpha | 0x14 before: the blend, 0 laid over by its alpha, 1 added, 2 taken away, 4 solid and written to the depth (0x11C680; nine elements have a number that sets nothing, so that the last one set stays); then four floats, the spreads of red, green, blue and alpha, each kept within 0 and 1 (one draw for the three colours when green's and blue's are below nought, 0x120824) |
| 9 | 0x14 | a vec3 | its turning, in radians | 0x10 before: how it faces (below); 0xE before: if set, its turns are by a random part of themselves, either way; three floats: random turns added once (a particle that faces the camera takes the third alone, one upright to it the first and third; 0x1215C0) |
| 10 | 0x18 | a vec3, a float | a drift: it is moved by the vector times the float a frame besides its own way (0x122944); in 1151 elements | 2 before: its kind, always 0 |
| 11 | 0x14 | a vec3 | how far its middle is from its place, turned as it is; in 768 | 2 before: if set, the offset is in its own sizes (0x124ED0) |

With those sizes every run of every element in the game's data ends on its
0x4000 (`tools_re/esb.py --all`), and `tools_re/out/_esb_census.py` counts what
the data uses of each choice.

A flat particle is one quad of one by one in its own flat, its corners at
plus and minus a half (0x16E3A8), from all of its texture, in its colour; a
plain one if its texture's number is below nought (0x123390). Its matrix is
its facing by its turning by its sizes, at its place (0x12448C, 0x124CB0),
and the facing is by the half-word before run 9 (0x123FE0 for the turning,
0x11EAE8 for the matrices the camera gives):

| faces | what |
|---|---|
| 0, 1 | the camera, whole: the view's own turn undone; turned about its middle by the third turn only |
| 2, 3 | the camera's heading only: upright, turned about the upright axis; then the first and third turns |
| 4 | nothing: as the world, or what its emitter is fixed to, has it; all three turns |
| 5 | laid flat (a quarter turn about x), then its turns |
| 6 | a quarter turn about y, then its turns |
| 7 | the way it is moving (from where it was to where it is), then its turns |

An effect is asked for by number, and the number is its name's: the six
figures after the two letters (0xA5A6C: `eb102150.esb` is 102150). What loads
a file (0xA5BD0, 0xA5928: the battle's loader has `.EFP` and `.EFC` both made
by it, 0x9D194) reads the parts with the effect library's own loader
(0x127018, which knows `.IMM`, `.ANM` and `.IMTA`), counts the effects
(0x126E80), takes each one's name (0x126DE4) and keeps them by number in
sixteen lists (0xA5C5C finds one).

### How an effect runs

All of this is `brs-engine/src/effect.rs`, which says where each part was
read. In short:

- **Time.** A frame is a thirtieth of a second (0xA5F5C halves the
  sixtieths it is given). An effect has no time of its own: see "Who gives an
  effect its time" below. Its elements are made at its first frame, all of
  them, and are stepped in the order of their entries; it is over the frame
  after none is left (0x1286B0), which takes the 0x1000 out of the half-word
  at its + 0xA0 (0x127C6C; making it puts that in, 0x128088).
- **Runs** (0x11F3B8, 0x11F0F4). A cursor has a time and a key. Each step it
  goes on by the step, then the run's value is mixed straight between the
  key it is at and the next; exactly on a key the value is left as it was.
  When the frame of a key that goes back comes the run goes back to that
  key, unless the effect has been told to end (0x127908), when it goes on to
  the keys after: that is how an effect that goes round is ended. A run of
  one key is never stepped. The run of which picture a particle is of is
  not mixed, and goes back by the key it is at (0x11F534).
- **An emitter** (0x11C57C) waits for its start, lets lots go until its end
  (its first frame's lot goes whatever the end), then steps its particles
  until none is left. A lot is so many particles (the count is mixed and cut
  to a whole number) when the frames between lots have gone by; in the data
  that is never less than one frame. Once its run of lots has come to a
  last key of none it has an end though it had none (0x11C234). One that
  hangs from another lets its lots go from each particle of that one, and
  its start and end are then of that particle's age (0x11DC34). A frame of
  no time lets nothing go, and moves and draws what there is.
- **Where an emitter is** (0x11CAD8), by what it is fixed to: 0 its place as
  it is, in the effect's space; 6 the place of the particle it hangs from,
  as that was last drawn; 11 the screen, in pixels from its middle (240,
  136), drawn flat with no depth; 16 about the camera on the flat, at the
  height given; 17 before the camera, whole.
- **A particle** (0x125708, 0x125B1C) comes out of the shape about that
  place (0x11D254: a ball is three random numbers made one long, a box is
  its whole sizes, a round is a ball seen from above with a height) and is
  sent the emitter's way, tipped off it to any side (0x11D4A0). Its own
  time goes at its pace. Each frame its life runs down, its runs are
  stepped, it moves by its way times its speed and by its drift, and its
  turning is made anew while its turns still change (or while it moves, if
  it faces its way). It has a frame in the frame it is let go.
- **Its matrix** (0x12448C) is its facing by its turning by its sizes, at
  its place; the effect's matrix is for what is fixed to nothing of the
  screen or the camera, and only the place of a particle that faces the
  camera goes by it. An effect is put by a matrix (0x127960), whose sizes
  are taken out of it for the particles that keep to something. So an
  effect put by a matrix of twice the size has its models and its flats
  that face nothing twice as big, and those that face the camera as big as
  they were, twice as far from its middle.
- **Its colour** (0x120D20) is the run's, by its own spread, by the effect's
  colour; it is drawn only while its alpha is not nought, and its matrix is
  worked out only then (0x1239E4).
- **A model particle** is of the model its run first names, has its size
  given to the model (0x122578), and is drawn lit by nothing but its colour
  (0x122724). The model's animation (the `.anm` after it; an `.imta` is
  not taken, 0x1223B8) plays once from its start at two of its frames a
  frame of the effect and holds at its end (0x1226F0, 0x2359C).
- **Limits.** All the effects share 128 emitters and 128 particles
  (0x128D4C): a particle beyond them is not let go. The random numbers are
  one Mersenne Twister for all of them, seeded 5489 (0x121818).

### How an effect is drawn

`brs-engine/src/effects.rs` and `effects.wgsl`. The original (0x127C9C)
draws the emitters of an effect in the order of the script (0x128768), each
particle over what was drawn before:

- no fog, no lighting, both faces; the depth is read (nearer or as near
  wins) and not written, but for the solid blend; nothing is thrown away
  for its alpha (the test passes everything);
- the blend is the emitter's (0x11C680): laid over by its alpha, added by
  its alpha, taken away by its alpha (what is there less it), or put in
  place of what is there; a number that names none of them leaves the one
  that was set last;
- **a flat particle** (0x1231A0, 0x123390) is the one square, corners at
  (-0.5, 0.5), (-0.5, -0.5), (0.5, 0.5), (0.5, -0.5) with the picture's
  top left at the first, in the particle's colour times the picture's,
  alpha too, the picture held at its edges and smoothed. Its picture is
  the file's by number, in the order the file has them (0x127018);
- **what is fixed to the screen**, flat or a model, is drawn in the screen's
  pixels, up from its bottom left corner: 0x128C14 changes how the library
  sees for it, to straight on from one before the screen (from (0, 0, 1)
  towards the origin, 0x16281C) with no perspective, 480 by 272 wide and
  high and 500 deep either way (0x1628A8), and back again for what is in
  the world. 548 emitters are fixed so, 194 of them models: the flashes and
  the speed lines of the scenes, and the sky of one;
- **a model particle** (0x122724) is drawn with lighting on and no light:
  the ambient light is the particle's colour, and the material's ambient
  is the corners' own colours (0xA60F8 has the colour material be the
  ambient), so a piece whose corners have colours is its colour times the
  corner's, and one whose corners have none (1081 of 7318 pieces) is its
  colour times itself, alpha too. Its materials say only whether there is
  a texture and how that goes on past its edges.

The port draws every particle as a mesh of its own, the colour going with
it, and has an emitter's turn in the script's order count for more than how
far a particle is when what is seen through is put in order. What is fixed
to the screen is drawn the same way by a camera that sees the screen as the
library does (`effects::screen_camera`): one the engine keeps, which draws
after the cameras of a scene while there is anything for it, and for an
effect of a scene's first draw list one that the scene puts before its own
(the battle's is in `battle/dimmer.rs`).

### Who gives an effect its time

The scenes' engine keeps an effect that plays as an object of 0x60 bytes
(0xA5D94: its matrix; flags at + 0x40, 1 put somewhere, 2 yet to be made, 8
told to end; the library's own at + 0x50; the sixtieths it has been given
and not had at + 0x54, and those it has had at + 0x58), on a node of the
scene ("HtSceneEffect", 0xAD5E0). Three things happen to it:

- **it is put** by its node's matrix in the world whenever that is worked
  out (0xAD6C4 to 0xA5E20);
- **it is given time** by whoever steps its node (0xACF1C, to 0xAD698 and
  0xA5E4C): the sixtieths wait at + 0x54. One that is over takes none;
- **it is drawn**, once a frame while its node is to be seen (0xAC8A4), as
  one of the things of its draw list that are seen through (0xA6148 puts it
  in the list by how far its middle is; 0xA60F8 is called when its turn
  comes), and it is then that it has its frames (0xA5E90). If it is yet to
  be made it is made, put, and has one whole frame (0xA5D48: its + 0x58
  goes up by 2). Then it has the time that waits for it, as one step
  (sixtieths x 30 / 60), and is drawn if it is not over; with none waiting,
  one that has been put somewhere since it was last drawn and has had any
  time has a step of no time, and it is drawn.

So how far in an effect is when it is first seen goes by who gives it its
time and when:

| whose | given time | first seen |
|---|---|---|
| the battle's (the list at the root's + 0xD0: what she asks for, 0x7D0E8 and 0x97F54; the effect at a bullet's muzzle, 0x887B0; what is seen of a hit, 0x909AC) | by the root before anybody's turn (0x8D094), whoever is held | a frame in, on the frame it is asked for |
| an enemy's (the list at its + 0x808: what its script asks for, 0xBD400) | by the enemy in its own frame, before its script runs (0x921AC); drawn with the enemy (0x90EB8) | a frame in; it stands still while the enemy is held, and is not seen while the enemy is out of sight |
| a bullet's look (the bullet's model, 0xA78B8) | by the bullet when it has a frame of its own (0xA7520): from the frame after it was made | an enemy's bullet has its look made as it is set up and is to be seen at once: a frame in. A shot of hers is not to be seen until it is two sixtieths old (0x87A2C), which it is on the frame after she fires: two frames in |

The scene's draw lists (0xA6488, 0x73C78) are drawn one after another: the
sky, then with the depth cleared the map and the ground, then what fights,
and last, with the depth cleared again (0xABB20), a fifth list. An effect is
of the fourth unless it is said otherwise (0x9E6C4), and of the fifth
(0xACDD0 with 4) where it is to be seen whatever is before it: what is seen
of a hit, her skills that act of themselves, the second of the two effects
of her gun overheating, and what an enemy's script asks for so (the fourth
argument of engine function 18, the seventh of 82). A node is told its list
by 0xACDD0, which keeps the list's number (node + 0x6C) and the list itself
(+ 0x70, from 0xA6E68), and sets the node's flag 8 for the first three lists
and 0x10 for the others. An effect of the first list is drawn before the
map: the scenes of her skills have one such, and what becomes of it is in
`BATTLE_NOTES.md` ("The effects of her skills' scenes").

The code:

| what | where |
|---|---|
| the effect library (ported: `brs-engine/src/effect.rs`) | about 0x11B600 to 0x129000, some 14,000 instructions: made 0x127EE0, its matrix 0x127960, a frame 0x127BA0 (in thirtieths of a second: the caller gives it sixtieths halved), drawn 0x127C9C, stopped 0x127908, unmade 0x127B10; it is playing while the half-word at + 0xA0 has 0x1000 |
| one effect that is playing, for the scenes' engine (ported: `Playing` in `brs-engine/src/effects.rs`) | 0xA5D94 makes it, 0xA5E20 puts it, 0xA5E4C gives it time, 0xA5E90 is what becomes of it as it is drawn, 0xA6020 tells it to end, 0xA5FF8 says whether it plays |
| the engine's node for one | "HtSceneEffect", 0xAD5E0: 0xAD7EC gives it its effect, 0xAD698 its time, 0xAD6C4 its matrix, 0xAD70C draws it |
| the battle's (ported: `brs-game/src/battle/effects.rs`) | 0x9E5D4 makes one of a list and 0x9E660 gives it its number (found among the battle's files, 0x9CA6C); 0x9E730 gives a list its time, unless the battle is paused (0x9E508), and 0x9E778 draws it; "BtSimpleEffectManager", 0x9E81C, takes away those that are over (0x9E8F8), ends all of a number (0x9E99C: they die away) and unmakes all of a number at once (0x9E9F4, which the scenes of her skills use) |
| the camera the library is given | 0xA5750: the view, the camera's own turn, its place, its angles (the heading is its turn about the upright axis, 0x24968) |

The viewer shows the effects of a file (`brs-viewer <file.efp> --effect
<number> [--frame <n>]`), and `tools_re/fx_sheet.py` makes a sheet of
frames of several with it.

## INSTDAT.VLD — install data

Not needed. `SYSTEM.VOL/instdat.vls` (`RVLS`) is its index: a 0x20-byte header with
the entry count at 0x08, then 60-byte entries of a 32-byte name, a 20-byte XOR key,
a size and an offset. Each file is XORed with its repeating 20-byte key. All 1,186
real entries are byte-for-byte copies of files in `USRDIR/GAMEDATA` (40 compared in
full, all names and sizes matched), bracketed by `START` and `END` markers.
