# The ride on the bike

What has been read of the mini-game the second stage's story has her ride through
(`startBikeMiniGame`, 0x147360; task `BikeMiniGame`, 0x1473AC), read from
2026-10-08 on, and what the port has of it (`crates/brs-game/src/bike`).

**Ported**: the road, the characters and their movers, the rules, the scripts with
all eighty-four of their functions, the camera with its own motions, the three
views' fog and what is drawn how far, the light by where a model is, the display,
its sounds, voices and tune, its effects, its talk, its pause, and the way to it
from the field and back (`--ride <course>` rides one alone; with no field there
its talk is taken as said after four seconds).
**Not yet**: the loudness the road's table has (word 5), and whether a sound
still plays (`n4c` takes one that was begun as playing). No script gives a character a piece (`n19`), so those have
nothing to be for.

**A bone a script turns** (`n24(id, slot, bone, way, n)`, 0x1443A8): the bone,
found by its number, has its matrix in the world multiplied by a turn about its
own X (way 0, 0x162224) or its Z (way 1, 0x16216C), there and then, so that it
lasts the frame; the number is given to the turn as it is, with no making
radians of degrees. Only the script of the bike's parts does it (`_update_tire`):
bones 100 to 102, the three wheels, by the sum of her speed times the frame's
time. In the port the turn is kept with the model and given to its animation
each frame (`Animator::set_own_turn`).

## How it is come to

A map's script sets the course with `s96(41, n)` and where to come back to with
`s96(42, ..)` and `s96(43, ..)`, and leaves the field with `s173(2)` (0xE6E94), which
makes the task. When it is over the field is started again and the script reads how
it came out with `s260()`: 1 ridden through, 3 given up, anything else failed.

The course's number is a stage of the system's table (0x147344, records of 0x54
bytes: a name, what it is called, the field's events it has, a number): 0 `easy`,
1 `normal`, 2 `hard` (the events `C_S2M2_BALL`, `C_S2M3_BALL`, `C_S2M4_BALL`: the
second stage's missions 2 to 4), 3 `ex00`, 4 `ex01`, 5 `ex_easy`, 6 `ex_normal`,
7 `ex_hard`. `MAP_D_STG02_02` asks for 0 when the trike there is looked at in
the story (the scenario at 21: cross by it, a scene, the lesson's pictures, then
the ride and map 20), and for 5, 3 and 4 later on; `STG02_08` for 1 and 6,
`STG02_09` for 2 and 7. In the port (`field::leave_field`) the field is put out
of sight, the ride is ridden (`Mode::Ride`) and the field then starts anew where
the script said, with the ride's outcome.

## Its files

`gamedata/mini_game/bmg_%s.bin` (0x147D1C, 0x147EE0): `MINI_GAME/BMG_SYS.BIN` and a
course, `BMG_EASY`, `BMG_NORMAL`, `BMG_HARD`, `BMG_EX_EASY`, `BMG_EX_NORMAL`,
`BMG_EX_HARD`, `BMG_EX00` and `BMG_EX01`. It also reads events of the field's
(`gamedata/fld/fevt/FEVT_%s.bin`).

Each is a **`PDK` pack** (`brs_formats::container::parse_pdk`, `tools_re/pdk.py`):
`PDK\x01`, u16 members, u16 where the members begin, then for each member where it
is, counted from there, and how long; a member there is none of has nought for both.
A member's number is what the game finds it by (0x149200(file, member, &size): file
0 the system's, 1 the course's, 2 the field's events). `brs-extract` unpacks them
like any other pack; a data folder unpacked before it could has them as single
files, and the game reads a member out of such a file as it is (`GameData::read`,
by a path like `MINI_GAME/BMG_SYS.BIN/002.sscr`).

`BMG_SYS.BIN`, 124 places:

| members | what |
|---------|------|
| 0 | script of tables: `tbl_data`, `mdlres_tbl`, `smres_tbl`, `aires_tbl`, `effres_tbl`, `voice_tbl`, `chr_tbl`, `state`, `int_init` |
| 1 | an `SC` bundle of sixteen slots: slot 0 a script of 64 bytes, slot 6 **the bank of the mini-game's sounds** (a bundle of two: the bank's header, `PPHD`, and its samples), as slot 6 of a bundle of the field's is its bank |
| 2 | the camera's script: `ws_lst` (wobbles), `val_lst`, `ip_lst`, `_set_mode`, `_set_type`, `_set_com`, `_update_mode`, `_update_param`, `int_init`, `int_event`, `_wobble`, `_bank`, ... |
| 3 | an `LPK` pack of twelve: the display's layout and its textures (see "The display") |
| 4 to 13 | ten models (`INSM`) |
| 14 to 50 | thirty-seven animations (`INSA`) |
| 54 to 64 | eleven scripts, of what is met on the road (206 to 2,367 lines each as `sscr_dec.py` writes them) |
| 74 to 99 | twenty-five effects (`EFC`) |
| 104 to 119 | sixteen recordings (`RIFF`, ATRAC3plus) |

A course, 97 places: two scripts (member 0 the road's tables, `map_tbl`,
`chip_lst`, `pattern_lst`, `line_lst`; member 1 the course's own, 285 lines for
`BMG_NORMAL`: `emy_tbl`, `trp_tbl`, `int_init`, `int_event`, `int_start`,
`int_finish`, `int_gameover`, `evt_*`) and seventeen or eighteen pieces of road
(`STCM`, from member 2 on).

The scripts are the same machine's as every other (`SSCR`, the VM at 0x112790), with
engine functions of the mini-game's own: eighty-four of them, in the table at
0x172D98 (below). The names the engine looks up in them (0x16EB10 to 0x16F060):
`map_tbl`, `chip_lst`, `pattern_lst`, `line_lst`, `init`, `int_init`, `int_event`,
`int_start`, `int_finish`, `int_gameover`, `int_justavoid`, `int_touch`,
`int_tarminate`, `int_grounded`, `int_turn_left`, `int_turn_right`, `int_bomb`,
`table`, `emy_tbl`, `trp_tbl`, `state`, `tbl_data`, `stage_tbl`, `mdlres_tbl`,
`smres_tbl`, `aires_tbl`, `effres_tbl`, `voice_tbl`, `chr_tbl`, `boot_bmg`.

`sscr_dec.py` writes these scripts with the field's names for three functions that
are others here: its `set_flag` is `n15`, its `print` is `n26` and its `chr_set` is
`n49`.

## Its code

0x132000 to 0x14A400: 474 functions, some 25,000 instructions. The whole of it is
listed function by function in `tools_re/out/bmg/code/code.txt`, with an index
(`map.txt`: where each begins in the listing, what it calls, who calls it, the
strings and numbers it names), by `python out/_bmg_map.py 132000 14a400 bmg/code`
run in `tools_re`.

### The tasks

- `boot_bmg` (0x1473AC, made by `startBikeMiniGame` with number 0x73). Made, it
  keeps twelve bytes, clears the game's flag 0x1818 and makes the task
  `BikeMiniGame` (0x14765C, number 0x8F). Each frame it asks that task whether it
  is done (0x147704: bit 0 of the task's word) and, when it is, how (0x147724:
  bit 2, kept at its + 8), ends it, and six frames later ends itself; as it ends,
  0xE6C64 has the field begin again.
- `BikeMiniGame` (0x14765C): made by 0x147564 (which makes the game's block,
  0x148A8C, and the task that draws, `bmgStartDrawTask`, 0x132538), ended by
  0x1475D8 (0x148CE0), its frame 0x147C4C and what it draws 0x147620.

The frame, by the task's state (its + 4; the table at 0x16F068). Before any of
them it leaves if the game is held (0x148A38) and runs what is the same every
frame (0x149CBC: sounds, the display, the pools).

| state | what |
|-------|------|
| 0 | fade out begun (0x297E4(2, 0)), to 3 |
| 3 | asks for `bmg_sys.bin` (file 0), to 4 |
| 4 | waits for it, to 5 |
| 5 | the system's tables read (0x14713C with member 0 of it: `state`, `tbl_data`, `stage_tbl`, `mdlres_tbl`, ...); to 6 if bit 2 of the task's word is set (a chooser of the course, for trying it out), else 8 |
| 6, 7 | the chooser: up and down move, cross takes it (bit 4, state 1), circle goes on to 8 |
| 8 | the course's name (0x147344) gives `bmg_<course>.bin` (file 1) and the field's events `FEVT_<..>.bin` (file 2) to ask for; the music asked for (0x149880); to 9 |
| 9 | waits for both, to 10 |
| 10 | the display made ready (0x132EEC), the course's script given its tables (0x146FB8 with member 1 of the course: `table`, `emy_tbl`, `trp_tbl`), resources begun (0x149FCC(0)), the system script's `int_init` run (0x1472FC), **the road built** (0x139368 with member 0 of the course), the course script's `int_init` run (0x1470F4), 0x149FCC(1), the game's word 4 nought and word 6 one, and the task `_pause_task` (0x14829C) made; to 11 |
| 11 | waits for the system's script to have done (0x1472D0), then 0x149FCC(2); to 12 |
| 12 | **the ride.** START pauses (0x147BA8). If the course's script has ended (0x1498E4) to 1. Else the two scripts run (0x1472A4, 0x14709C) and, unless the course's flags (below) have bit 31, the world's frame (0x142574) and the two checks of where she is on the road (0x147738, 0x147924); then the list of what is to be drawn (0x132254) |
| 1 | fade out, the sounds told to stop (0x1384FC); to 2 |
| 2 | waits for them (0x138518), then sets bit 0 of the task's word: done |

### The game's block

One block of 0x73ED0 bytes, kept at 0x172F60 (0x148A80 gives it). What is in it:

| at | what |
|----|------|
| + 0 | flags: bit 1 the camera is held (word 7) |
| + 4, + 8 | words 6 (read) and 6 (written) of 0x14914C and 0x149024 |
| + 0x0C | three numbers, 0, 1, 0.12 |
| + 0x18 | the time a frame takes, in seconds (0x149008(0)) |
| + 0x1C | the world (what 0x142574 is given) |
| + 0x20 | its sounds |
| + 0x218 | three files, 0x110 each: the system's, the course's, the field's events |
| + 0x550 | three views, 0x100 each (0x14960C) |
| + 0x850 | a block of 0x13F4B8's; word 0 at + 0x854 |
| + 0x8A0 | **the road** (0xFC10 bytes) |
| + 0x104B0 | the system's script |
| + 0x10674 | where the course's flags are (what `n48` gives) |
| + 0x1067C | the course's script |
| + 0x10824, + 0x10828, + 0x1082C | whether the course's tables are had, `emy_tbl` (records of 7 bytes, `n38`), `trp_tbl` (records of 28, `n39`) |
| + 0x10830 | the camera (what the scripts call 3) |
| + 0x10CB0 | the display |
| + 0x10CE8 | what is to be drawn, in order (0x132254) |
| + 0x11300 | the resources' pools: models, pieces, effects, scripts |
| + 0x232E0 | the pools of what there is on the road: characters (0x13D888), scenery (0x13D8DC), traps (0x13D930) |
| + 0x73DF0 | the traps' list |
| + 0x73E10 | the characters' list (a node's + 4 the next; the character is its + 0x10, found by number with 0x1492E0) |
| + 0x73E14 | word 4 |
| + 0x73E18 | twenty-three sounds playing, by slot (0x149624 begins one, 0x14979C stops it) |
| + 0x73E74 | word 2: the frames counted by `n0d` |
| + 0x73E80 | word 5: a loudness, 0 to 255 |
| + 0x73EA0 | a resource of the common game's (kind 3) |
| + 0x73EA8, + 0x73EAC | the next number for a character (from 0x100) and for a trap |

### The road

The road's block (the game's + 0x8A0): up to 500 **chips** of 0x80 bytes from its
start, how many at + 0xFBF0; the first chip of each of five **lanes** at + 0xFA00
and the last at + 0xFA14; the road's own script at + 0xFA28 (run for `init`); its
scenery's list at + 0xFBCC; the light's two turns at + 0xFBD0 (-80 and 210
degrees) and the way it shines at + 0xFBE0 (0x138AE0; see "How it is drawn": the
way comes to nothing); two colours, both black, at + 0xFBD8 and + 0xFBDC; the
four tables at + 0xFBF4 on.

A chip:

| at | what |
|----|------|
| + 0, + 4 | the chip beside it to the left, to the right |
| + 8, + 0xC | the chip before it, after it |
| + 0x10 | flags: 1 in use, 2 its turn's radius worked out, 4 its end worked out, 8 it turns |
| + 0x14 | its number (word 2) |
| + 0x18 | its lane (word 3) |
| + 0x1C | its row (word 4) |
| + 0x20 | how long it is (number 3; 10000 unless set) |
| + 0x24 | how far it turns, in degrees (number 4) |
| + 0x28 | the turn's radius: length x 180 / degrees / pi |
| + 0x30, + 0x40 | which way it faces where it begins, where it ends (three turns in degrees; + 0x34 the heading) |
| + 0x50, + 0x60 | where it begins (numbers 0 to 2), where it ends |
| + 0x70 | its marks (word 5): the line table's word for this lane |

Where a chip is at a part `t` of the way along it, 0 to 1 (0x142144): from its
beginning to its end in a straight line when the end is worked out, else
`(0, 0, length x t)` turned by its heading and added to its beginning; a chip that
turns goes round its radius instead. Which way it faces there (0x141E4C): its
beginning's, with the heading turned on by `degrees x t` if it turns. 0x142400
works out a chip's radius (0) and its end (1), and gives the end to the chip
after it as its beginning. 0x141D90 puts one chip beside or after another.

**Building it** (0x139368). From `map_tbl`: the lanes' width (+ 0, 250), where
lane 0 is across (+ 4, 500: lane `k` begins at `x = 500 - 250 k`, so lane 2 is
the middle), how many rows (+ 0xC, 100), the piece that is the sky (+ 0x14, a
byte), a loudness (+ 0x18), the colour the screen is cleared to (+ 0x1C: 0.1,
0.1, 0.2), five colours of light (+ 0x28, twelve bytes each) and from + 0x64
eight numbers for each of the three views (see "How it is drawn"). For each row, `chip_lst` has three bytes: two
patterns of scenery and the row's kind of line. A row is five chips, one a lane,
each 10000 long and straight, its marks the word `line_lst` has for that kind and
lane (five words a kind), each put beside the one before. Then every lane's chips
have their ends worked out from the first on: **the road is straight, a hundred
rows of 100 m**, units being centimetres. Then the scenery: the sky if there is
one, and for each row each of its two patterns that has anything, one thing at
`(0, 0, 10000 row + 5000)` seen within 6000, with up to five pieces of road
(`STCM`) and four numbers for each from `pattern_lst` (260 bytes a pattern, 20 a
piece: the piece's number, a byte, then the four numbers; after the five pieces,
from + 0x64, five zones of light of 32 bytes: a kind, a byte, and seven whole
numbers, which are a place in the row, three half sizes and a heading in
degrees). The sky is given view 0 and its piece the bit 0x1000000; a row's first
pattern view 2 and the bit 0x2000000, its second view 1 and 0x4000000. Last the
road's script runs `init`.

**A place on the road** (20 bytes; a character has two, at its + 0x774 the one it
is leaving and at + 0x788 the one it is on): the chip (+ 0), how far along it
(+ 4), which part of the way (+ 8), its **speed in km/h** (+ 0xC) and how far it
has come in all (+ 0x10). 0x138230 sets one of these (0 speed, 1 the part, 2 how
far, 3 the whole way come), going on to the chips after or before when the part
is past an end; 0x138008 reads one. Each frame (0x137EE4) it goes on by
`speed x 100000 / 3600 x the frame's time` (0x138138, which goes from chip to
chip and stops at the road's ends). 0x13A390 takes a character a lane to the left
(0) or the right (1): the place it is leaving becomes the one it was on and the
one it is on goes to the chip beside, the same part of the way along.

### A character

0x820 bytes (made by 0x139C6C):

| at | what |
|----|------|
| + 0 | its node in the list |
| + 4, + 8 | **what it can still take** (0x13A58C takes a blow's worth from + 4) and what it could at first |
| + 0xC | how many blows it has taken |
| + 0x10 | its kind |
| + 0x14 | a word |
| + 0x18 | flags (0x83 at first): 1 just made, 8 takes nothing from a blow, 0x80 can be hurt, 0x100 on the ground, 0x800 may change lanes, 0x2000 can be struck by her special |
| + 0x1C | its number |
| + 0x20 | its model: two models, five pieces, twenty effects, three scripts |
| + 0x5C0, + 0x5D0 | where it was, where it is |
| + 0x5E0, + 0x5F0 | the two things its place is the sum of: where the road has it, and what is added |
| + 0x600, + 0x610, + 0x620 | its turns in degrees: the road's, what is added, and their sum |
| + 0x630, + 0x640 | two more threes of numbers (`n3a`, `n3b`) |
| + 0x658 | seven boxes of 0x38 bytes: what it hits with and is hit in (`n2b`) |
| + 0x770 | how it moves: the two places on the road, + 0x7E0 a number |
| + 0x7F0 | 0x146A8C's |

A character's words, as `n09` and `n11` number them: 0 its kind, 3 the word at
+ 0x14, 4 the chip it is on, 5 takes nothing from a blow (flag 8), 6 **what it
can still take** (+ 4), 7 the word at + 8 (what it could at first), 8 **may change
lanes** (flag 0x800), 13 can be struck by her special (flag 0x2000). Its numbers
(`n0a`, `n10`): 0 speed, 1 the part of the way along its chip, 3 how long a
change of lanes takes, 4 the part of that time it waits first, 5 **how long
nothing can touch it** (+ 0x7E0, counted down).

**Its frame** (0x13A958):

1. its mover's frame (0x13B644, below); the time nothing can touch it counted
   down;
2. if its kind is odd and it may change lanes: on a chip whose marks have bit 0
   and not bit 4, it is told to turn towards whichever side has the nearer chip
   with bit 5 beyond it (event 9, left, or 10, right);
3. where the road has it and which way it faces there are taken from the mover;
   what is added moves by the three numbers at + 0x630 a second; its place is the
   sum of the two, its turns likewise, and + 0x640 is how fast it went;
4. when its height comes under 0.1 it is told it has landed (event 8), once,
   until it is over 0.1 again;
5. its shadow (+ 0x7F0) is put under it at height 0.

**Its mover** (0x70 bytes at + 0x770): the two places on the road, how long a
change of lanes takes (+ 0x30, half a second), how long this one has gone on
(+ 0x34), the part of the time waited first (+ 0x2C), three numbers of the
change's curve (+ 0x60 3, + 0x64 8, + 0x68 0), which curve (+ 0x6C) and bit 0 of
+ 0 while a change goes on. Each frame both places go on along the road. While a
change goes on its part `p` is `(time - wait) / (length - wait)` held to 0..1, and
how far across she is, for curve 0, `(f(c) - f((b - c) p + c)) / (f(c) - f(b))`
with `f(x) = a^3 / (x^2 + a^2)` (`a`, `b`, `c` the three numbers: slow to begin,
quickest a third of the way through and slow to settle), and for curve 1 `p`
itself. Where she is and which way she
faces are those of the place left and the place come to, mixed by that much. Then
the **banks**: a chip whose marks have bit 1 (the left edge) or bit 2 (the right)
is a wall ridden on. Coming on to one, from half way across she rises to 256 and
leans 45 degrees (-45 on the right) by the part crossed; leaving it the same
backwards; wholly on one she is 256 up and it is her heading (not her lean) that
45 is added to, as the code has it (0x13BA1C: + 0x54 where the changes use
+ 0x58).

### The rules

The block at the game's + 0x850 (0x48 bytes): flags (+ 0), her number (+ 4), its
state (+ 8: 0 stopped, 1 riding, 2 her special), the turn the pad asked for this
frame (+ 0xC: 1 left, 2 right), her speed (+ 0x10), how long she has been in her
gear (+ 0x14), the gear, 0 to 3 (+ 0x18), how many are near her this frame and up
to three of their numbers (+ 0x20, + 0x24), **how many she has just avoided in a
row** (+ 0x30), how long that row still holds, or her special still lasts
(+ 0x34), the lane mark the camera was last told (+ 0x3C), how far she has come
and has to go (+ 0x40, + 0x44).

**The course's flags and numbers** (what `n48` gives; the course's script sets
them):

| at | what |
|----|------|
| + 0 | flags: 1 the pad is heeded, 2 the goal is looked for, 4 she can die, 8 the time can run out, 0x10 the time runs, 0x20, 0x40, 0x80, 0x100 four parts of the display shown, 0x200 the pad changes lanes, 0x1000, 0x2000, 0x4000 gears 1, 2, 3 can be come to, 0x8000 her special's blow is out (set by her script), 0x10000 the special can be had, bit 31 the world stands still |
| + 4 | how she goes: 0 not driven (her speed falls by the factor at + 0x1C a frame), 1 hit, 2 riding, 3 her special |
| + 8, + 0xC | the chips she starts on and the goal is at |
| + 0x10 | how long her special lasts |
| + 0x14 | how long a row of avoidings holds |
| + 0x18 | how many avoidings fill the gauge |
| + 0x1C | the factor her speed falls by |
| + 0x20 to + 0x30 | five speeds: where gear 0 begins, and where gears 0 to 3 end |
| + 0x34 to + 0x40 | how long each gear takes |
| + 0x44 | a time she is held for (nothing of the gears happens while it runs: 99999 before the start, 1 when hit) |
| + 0x48 | a time after a blow in which nothing is "just avoided" (0.3) |
| + 0x4C | **the time left**, in seconds (240 on `BMG_NORMAL`) |

Each frame (0x140424), in this order:

1. **The pad** (0x13FA58), if the pad is heeded. If it changes lanes, she may,
   and her chip's marks have not bit 4: left or right **held** asks for the chip
   that side if there is one and its marks have not bit 5 (event 9 or 10 to her
   scripts), and the turn is kept for step 3. If the special can be had and the
   gauge is full (flag 0x100; a voice and sound 14 the first time): **square**
   begins it: state 2, its time the course's, the display told (flag 0x10).
2. **Her speed** (0x13FC70). Unless she is held: state 1 makes her go 2 (sound 3
   the first time), state 2 makes it 3. While she goes at all the gear's time
   runs; past the gear's length, her scripts are told once (event 1: the puff of
   a change of gear) and, if the next gear can be come to, it is, from nought;
   else the time stays at the top. Her speed is eased (0x14897C) from the gear's
   first speed to its last by the part of its time gone. Not going, it falls by
   the factor. **The gauge**: riding, with fewer avoided than fill it, the row's
   time runs down and at nought the row is lost (sound 13 when under 3 seconds);
   with enough, the gauge is full. In her special, its time runs down; as it
   begins her scripts get event 11 with 0 and as it ends 11 with 1, and the state
   is 1 again. **The camera** is told the lane she is in whenever byte 1 of its
   marks changes (event 0 with it). With nothing left that she can take she
   stops (state 0). **The time** runs down while flag 0x10 (sound 16 at 30
   seconds, 17 at nought).
3. **Just avoided** (0x140728), unless within the time after a blow or in her
   special: every other character one of whose boxes of kind 2 meets one of hers
   of kind 2 (0x13ADC0) is near, up to three. If any is, the display may show
   its sign (flag 2) unless the chips on both sides have marks bit 0; and if the
   pad turned her this frame her scripts get event 5 with (0, the turn) and each
   near one's event 5 with (1, the turn), the row is one longer and its time
   begins again.
4. **Her special's aim** (0x13F534), while the course's flag 0x8000: of the
   others that can be struck, the nearest within 100 across, 400 up and down and
   8000 ahead gets event 11 with 2.
5. **Blows** (0x13F6DC), unless nothing can touch her: every other character
   that can be touched whose boxes of kind 1 meet hers; each of its five boxes
   that struck takes its worth from her (0x13A58C: her scripts get event 6 with
   the box's + 0x30, and event 7 when nothing is left), it gets event 6 itself,
   the gear's time is nought again and she goes as 1 (unless in her special). A
   blow that took anything sets the game's flag 0x1818.
6. **The end** (0x13F8CC): with flag 2, at the goal chip's beginning she has
   arrived (0x149FCC(3)); with flag 4 and nothing left, or flag 8 and no time
   left, she has failed (0x149FCC(4)).
7. The display is given (0x1335D8): 0 the sign of step 3, 1 and 2 two signs from
   bits 16 and 17 of her chip's marks, 3 to 6 the four parts shown, 8 how far
   she has come, 9 the part of the way in hundredths, 10 and 11 what she can
   take and could, 12 her speed, 13 the row, 14 the row's time left as a part in
   hundredths, 15 the time left in hundredths of a second.

The events a character's scripts get (0x13C6DC): 5 `int_justavoid`, 6 `int_touch`,
7 `int_tarminate`, 8 `int_grounded`, 9 `int_turn_left`, 10 `int_turn_right`,
11 `int_bomb`; and `int_event` with a number and what comes with it (0x13C678,
`n47`).

The gears' easing (0x14897C): `a + (b - a) (t + sin(pi t) / pi)`.

### What is on the road besides her

**Boxes** (a character has five that are looked at, 0x38 bytes each at + 0x658;
set by `n2b`, whose `what` is in brackets): its shape (+ 0: 0 none, 1 a ball, 2 a
box) [1]; flags (+ 4: 1 in use [10], 2 met something this frame); the bone it
goes with, or -1 for the whole character (+ 8) [9] and which of the two models
the bone is of (+ 0xC) [13]; where its middle is from there (+ 0x10) [2, 3, 4]; a
ball's radius (+ 0x1C) [5]; a box's three sizes (+ 0x20) [6, 7, 8]; what kinds it
is (+ 0x2C) [0: the script's bits 1 and 2 make kind 1, **a blow**, its bit 4 kind
2, **near**]; what a blow of it says (+ 0x30) [12] and is worth (+ 0x34) [11].
She has a ball of 40 at height 60 for blows and one of 70 at height 40 for
"near". Two characters meet (0x13ADC0(a, b, kinds of a, kinds of b)) when one of
a's balls of the kind, swept from where it was a frame ago to where it is,
touches one of b's balls or boxes of the kind; both are then flagged as met.
Only a's balls are tried.

**Traps** (0x70 bytes; 0x148488): where it is (+ 0x10), its turns (+ 0x20), its
three sizes (+ 0x30, 100 each at first) and where its middle is from its place
(+ 0x40); its number (+ 0x50); bit 0 of + 0x54 once it has gone off; whom it
watches (+ 0x58, word 2: a character's number); whom it tells (+ 0x5C, word 3: 0
the system's script, 1 the course's, 2 the road's, 3 the camera's); which of the
script's entries (+ 0x68, word 1), with what event (+ 0x60, word 4) and what
besides (+ 0x64, word 5). It goes off, once, when the way the watched one came
this frame passes through its box.

**As she comes** (0x147738, 0x147924, each frame of the ride): the course's
`table` says how many records `emy_tbl` and `trp_tbl` have. A record whose byte
0 has not bit 0 yet is made, and given the bit, when she is within a row (10000)
of where it is: its row and hundredths of a row along.

- `emy_tbl`, 7 bytes: flags, the number to give it (-1 a new one), its kind, a
  number for its script, lane, row, hundredths. It is made (0x149968) as a
  character with the script `chr_tbl` has for its kind, put on that chip at that
  part of the way.
- `trp_tbl`, 28 bytes: flags, number, lane, row, hundredths, then words: three
  sizes, the event and what goes with it. It watches her and tells the course's
  script's `int_event`.

A course's script clears a record's bit 0 to have it made again: so a stretch
is ridden round and round while a talk goes on (`n31` moves everything back a
row or two, and the gates are made anew).

**Scripts** (0x13F124 to 0x13F490). One that runs is 0x1A0 bytes: bit 0 of + 0
while it stands still; how it goes on (+ 4); the machine (+ 0xC, its stack 0x100
bytes from + 0x3C); the function it is in when nothing else is asked of it
(+ 0x13C); what `n23` reads (+ 0x140: the character it is of, a flag that it is
not to be broken in on (bit 0 of + 0x144, `n49`), then `n23`'s 0 to 5: its
character's number, the number it was made with, the event and what came with
it, a count for its waits, and 0x40 bytes of its own from + 0x160).

- An entry is begun by name (0x13F1F4) **at once, whatever the script was in
  the middle of**, which is dropped; unless the flag says not to break in. The
  names, by number (0x172D68): 0 `int_init`, 1 `int_event`, 2 `int_start`,
  3 `int_finish`, 4 `int_gameover`, 5 `int_justavoid`, 6 `int_touch`,
  7 `int_tarminate`, 8 `int_grounded`, 9 `int_turn_left`, 10 `int_turn_right`,
  11 `int_bomb`.
- Each frame it runs on from where it waited (0x13F278). A character's script
  (made by 0x141604) is besides a round of states: when a function returns, what
  it returned is the function to be in from now on if it is not nought, and the
  function it is in is begun again at once. The system's, the course's and the
  road's just stop when they return.

**The system's tables** (member 0 of `BMG_SYS`): `tbl_data`, bytes: 8 stages, 13
models, 50 pieces, 11 scripts, 37 effects, 16 voices, 9 kinds of character.
`stage_tbl`, 84 bytes a stage: the course's name (`easy`, `normal`, `hard`,
`ex00`, `ex01`, `ex_easy`, ...), a name to show, the field's events' name
(`C_S2M2_BALL`, `C_S2M3_BALL`, `C_S2M4_BALL`, `PS_EX3_BALL`, `MZ_EX1_BALL`, ...)
and a word. The resources are pairs of bytes, which pack (1 the system's, 2 the
course's, 3 the common game's) and which member: `mdlres_tbl` eighteen pairs a
model (the model, then seventeen animations: model 0 is the bike, model 1 she
herself, out of the common game); `smres_tbl` the pieces (members 2 to 51 of the
course); `aires_tbl` the scripts (members 56, 55, 61, 54, 57, 58, 59, 60, 62, 63,
64); `effres_tbl` the effects; `voice_tbl` the voices (members 104 to 119).
`chr_tbl`: the script of each kind of character (0, 1, 5, 7, 8, 2, 6, 9, 10: kind
0 is she, script 56). `state` is the course's flags and numbers, at first: her
special 5 s, a row holds 10 s, 5 fill the gauge, the factor 0.99, the speeds 80,
140, 200, 290, 360 km/h, the gears 2, 30, 15 and 15 s.

**The game's events** (0x149FCC): 0 loads everything the tables name, and the
sounds (member 1 of the system's pack, an `SC` bundle); 1 makes the camera with
its script (member 2); 2 runs the course's `int_start`; 3 **the goal**: the
course's `int_finish`, and the ride's outcome 1 (0x2338); 4 **failed**: the
course's `int_gameover`, outcome 2, and the traps stop; 5 given up from the
pause: outcome 3, and the ride ends at once. The ride ends when the course's
script has done after 3 or 4.

**The time a frame takes** (0x149CBC): a thirtieth of a second (a sixtieth if
the game runs at sixty), times the number the game's word 6 chooses of 0, 1 and
0.12. Talk (`n34(n)` asks for the field's event `n`; 0x149CBC begins it when the
voice is free, with sound 16 turned down meanwhile) holds the bits of + 0x73EB0
until it is over; `n35` waits for that.

### The scripts' functions

A function that waits is called again each frame until what it waits for is so
(0x113510 tells the first call from the later ones). `id` is a character's number
(she is 0); a function given a number there is no character of does nothing.

| | at | what |
|--|----|------|
| `n00` | 0x143604 | nothing (a trace) |
| `n01(a, v)` | 0x142A08 | `a` 0: sets the game's flag 0x1818 to `v` |
| `n02`, `n16` | | nothing |
| `n03(a)` | 0x142A5C | 0x149FCC(3) for 0, (4) for 1 |
| `n04(lane, from_end, n)` | 0x142AB0 | the number of the chip `n` on from the lane's first (or back from its last), -1 if none |
| `n05(id)` | 0x142D08 | makes a trap |
| `n06(id)` | 0x142D2C | ends a trap |
| `n07(id, what, v)` | 0x142D64 | sets a trap's word: `what` 0, 1, 2, 3, 4 are its words 2, 3, 1, 4, 5 |
| `n08(id)` | 0x142E5C | ends a character (bit 0 of its node's + 0x830) |
| `n09(id, what, v)` | 0x1461CC | sets a character's word (above): 1 and 2 are its model's words 0 and 1, 9 its mover's curve, 10, 11, 12 words 2, 0, 1 of its shadow |
| `n0a(id, what, f)` | 0x142FD0 | sets a number: 0 **speed**, 1 **the part of the way along its chip**, 2 its model's number 6, 3 and 4 its mover's numbers 0 and 4, 5 the number at + 0x7E0 |
| `n0b(id, x)` | 0x143230 | waits while 0x13C558(model, x) |
| `n0c(id, what)` | 0x14360C | 0: 0x13C61C(model); 1 **a lane to the left**; 2 **a lane to the right** |
| `n0d(frames)` | 0x143314 | waits that many frames, counted in the game's word 2 |
| `n0e(kind, time)` | 0x1433EC | fades: 0 in and 1 out (0x297E4(1 / 2)), 2 and 3 the same in white (0x11 / 0x12), 4 the display's (0x1335D8(.., 0x10, time)) |
| `n0f()` | 0x14357C | waits for the fade and the display's to be over |
| `n10(id, what)` | 0x14310C | reads a number: 0 speed, 1 the part of the way along, 3 its mover's number 0, 5 the number at + 0x7E0 |
| `n11(id, what)` | 0x142E94 | reads a character's word: 0, 4 to 8 and 13 |
| `n12(chip, what)` | 0x142B94 | of a chip: 1 **its lane**, 2 its row, 3 and 4 the number of the chip to its left and right (-1 none), 5 its marks |
| `n13(id)` | 0x1436BC | waits for its mover to have come to rest (0x13B55C) |
| `n14(k)` | 0x143774 | a new number: 1 for a character, 2 for a trap |
| `n15(bits)` | 0x1437E8 | the camera's: bits 1, 2, 4, 8, 16, 32, 64 ask 0x1375D8 for 2, 1, 6, 4, 5, 7, 3 |
| `n17(k, f)` | 0x1438E8 | the sine (0), cosine (1) or square root (2) of `f` |
| `n18(id, slot, res)` | 0x14398C | a model into one of its two slots |
| `n19(id, slot, res)` | 0x143A70 | a piece into one of its five |
| `n1a(id, slot, res)` | 0x143B54 | an effect into one of its twenty |
| `n1b(id, a, what, v)` | 0x143C38 | of model `a`: 0 0x13BE84(.., 0, a bit: 0x2000000 for `v` 0, 0x8000000 for 1); 1, 2, 4 0x13BF00(.., 3 / 4 / 5, v / 100); 3 0x13BE84(.., 1, v) |
| `n1c(id, a, what, v)` | 0x143DC0 | of piece `a`: 0x13C070 with the same bit |
| `n1d(id, a, what, v)` | 0x143E90 | of effect `a`: 0 to 4 0x13C248(.., what, v); 5 0x13C3C0(v / 100) |
| `n1e`, `n1f`, `n20`, `n22` `(id, slot)` | 0x144018 on | take the model, piece, effect or script out of a slot |
| `n21(id, slot, a, what, b)` | 0x14418C | 0x13EA7C of a model |
| `n23(k)` | 0x1442F0 | of the script's own: 0 and 1 its + 8 and + 0xC, 2 the frames this frame, 3 **the event** and 4 **what came with it**, 5 **its own memory** |
| `n24(id, slot, bone, what, deg)` | 0x1443A8 | turns a bone of a model (0 and 1 two ways) |
| `n25(frames)` | 0x1445A0 | waits that many frames |
| `n26(n)` | 0x144660 | waits `n` sixtieths of a second of the game's time |
| `n27()` | | 0 |
| `n28(text)`, `n29()` | 0x144768 | has the display say something (0x132E9C); waits for it (0x132ECC) |
| `n2a(k)` | 0x144850 | 0: the time a frame takes |
| `n2b(id, box, what, v)` | 0x1463D8 | sets one of a character's boxes |
| `n2c(id)` | 0x144894 | whether there is such a character |
| `n2d(who, entry, event, with)` | 0x1448E4 | **begins an entry** of a script (1 is `int_event`) with an event and what goes with it: 0 the system's, 1 the course's, 3 the camera's |
| `n2e(k, n)` | 0x144A0C | plays a sound: 0 of the common game's, 1 of the mini-game's into slot `n` |
| `n2f(what, v)`, `n30(what, f)` | 0x144A90, 0x144C68 | the camera's words (0x137328) and numbers (0x137454) |
| `n31(f)` | 0x144EE8 | **moves everything along the road by `f`** (every character and the camera: the road wraps) |
| `n32(id, f)` | 0x144F68 | moves one character so |
| `n33(n)` | 0x144FC8 | 0x93A0(n) |
| `n34(n)`, `n35()`, `n36()` | 0x145004 on | asks for talk `n` of the field's events; waits for all talk to be over; whether any is asked for or going on |
| `n37(id, kind, with)` | 0x1450E0 | **makes a character** (0x149968; -1 for a new number) |
| `n38(i)`, `n39(i)` | 0x145140 | where record `i` of `emy_tbl`, of `trp_tbl` is |
| `n3a(id, what, v)`, `n3b(id, what, out)` | 0x146638, 0x1467DC | set and read a character's threes of numbers (place, turns, which way it faces) |
| `n3c(chip, f, what, out)` | 0x145264 | where a chip is at that part of the way (0), which way it faces (1) |
| `n3d(id, slot, bone, what, out)` | 0x14536C | where a bone of a model is |
| `n3e` to `n42` | 0x14545C on | sums of threes of numbers: scaled, made one long, added, taken away, turned |
| `n43(id, i)` | 0x1456F8 | whether its model has piece `i` |
| `n44`, `n45` `(id, i, what, v)` | 0x1458E8 | set and read two threes of numbers of an effect's |
| `n46(id, slot, with, res)` | 0x145AC0 | **gives a character a script** (one of three), with a number of its own |
| `n47(id, slot, entry, event, with)` | 0x145BF4 | begins an entry of the script in one of a character's three slots (0x13C678) |
| `n48()` | 0x145CB4 | **the course's flags and numbers** |
| `n49(x)` | 0x145CEC | 0x13F490 of the script's own |
| `n4a(what, v)` | 0x145D2C | four threes of the camera's numbers |
| `n4b(k, slot)`, `n4c(k, slot)` | 0x145E84 | stops a sound; whether it plays |
| `n4d(a, b)`, `n4e()`, `n50()` | 0x145F48 | the camera's own model and motion; waits for it; reads it |
| `n4f(pad, bits)` | 0x146058 | buttons just pressed: 1 left, 2 right, 4 triangle, 8 square, 16 cross |
| `n51(x)`, `n52(x)`, `n53(x)` | 0x146160 on | 0x149B80, 0x1498B0 (the music faded out), 0x149880 (the music) |

### The camera

One block of 0x480 bytes (the game's + 0x10830; 0x135C30 to 0x137EB8), with a
script of its own (member 2 of `BMG_SYS`, at its + 0x288) and a model of its own
for motions (at + 0x428).

| at | what |
|----|------|
| + 4, + 8, + 0xC | whom it goes with, which of its models, which bone |
| + 0x40, + 0x50, + 0x60 | the eye, what it looks at, which way is up |
| + 0x70, + 0x80 | the eye and the mark it is going to |
| + 0x100 | where the one it goes with is (the anchor) |
| + 0xB0 on | six threes of numbers: how fast the eye and the mark follow, across, up and along, for each pace of the game (pace 1: 0.3, 1.0, 0.9; pace 2: 0.03, 1.0, 0.3) |
| + 0x140, + 0x170, + 0x190, + 0x120 | the eye's offset now, from, to, and as set; its time at + 0x1B0 of + 0x1B4, how it goes at + 0x1C0 |
| + 0x150, + 0x180, + 0x1A0, + 0x130 | the mark's offset likewise; + 0x1B8 of + 0x1BC, + 0x1C4 |
| + 0x1D0, + 0x1D4, + 0x1D8 | the lens now, from, to (the angle up and down, in degrees); + 0x1E0 of + 0x1DC, + 0x1C8 |
| + 0x1F4, + 0x1F8, + 0x1FC | the roll likewise; + 0x204 of + 0x200, + 0x1CC |
| + 0x1E8, + 0x1EC, + 0x1F0 | the bank: how long (0.5 s), how far in, how big (11; the scripts set 6 and -6). The angle is `size x 2 x (x / (x x + 1)) x (1 - p)` with `x = 3p` |
| + 0x258, + 0x210 | a shake as set (0x24 bytes), and two that go on; their sums at + 0x208 and + 0x20C |

How a change goes from one value to the next: 0 evenly, 1 `t - sin(pi t) / pi`,
2 `t + sin(pi t) / pi`.

Its words (0x137328, what `n2f` sets through the list 0, 1, 2, 3, 8, 9, 10, 11,
12, 13, 4, 5, 6, 7): 0 a shake's length, 1 its angle, 2 its random range, 3 its
slot, 4 to 7 how the eye, the mark, the lens and the roll go, 8 whom it goes
with, 9 which model, 10 which bone, 14 the flag 0x20. Its numbers (0x137454, what
`n30` sets through the list 0, 1, 2, 3, 4, 5, 0x18, 0x19, 0x1D, 0x1E, 0x1F, 8, 9,
10, 12, 14, 16, 18, 20, 22, 23): 0 to 2 the eye's offset, 3 to 5 the mark's, 8 a
shake's fading, 9 its size, 0xA to 0x15 the following, 0x16 and 0x17 how long the
eye and the mark take, 0x18 and 0x19 the lens and how long it takes, 0x1D and
0x1E the roll, 0x1F the bank's size. What it is asked to do (0x1375D8; `n15`'s
bits 1, 2, 4, 8, 16, 32, 64 ask for 2, 1, 6, 4, 5, 7, 3): 0 gives the three views
what it has (the sky's view with the eye in the middle of everything), 1 begins
the eye's and the mark's change, 2 a shake, 3 stops the shakes, 4 begins the
roll's change, 5 a bank, 6 has everything at once where it is going, 7 begins
the lens's change. `n4a` sets a three of numbers at once: the eye's offset, the
mark's, and the following of each.

The camera's script keeps the tables (member 2): `val_lst`, fourteen ways of
looking of eight numbers (the eye's offset, the mark's, the lens, the roll: 0
and 1 behind her to either side, (-55, 114, -250) looking at (-35, 107, 60)
through 30 degrees rolled -3; 4 and 5 the banks; 12 and 13 from in front);
`ip_lst`, eight ways of changing (four times, six followings, four ways of
going); `ws_lst`, eight shakes. The marks of the lane she is in (bits 8 to 15)
choose among them.

**Its own motion** (`n4d(model, animation)`, -1 stops it; `n4e` waits for it):
model 11 of the system's table is a camera's rig (member 13 of `BMG_SYS`: four
nodes, the camera the second, 35 mm) with motions (members 38 to 50) that drive
its first nodes. While one plays the view is the rig's, the rig being where the
one the camera goes with is and turned as it is. The ride begins with motion 5
(the fly-by round her), and talk has others.

### How it is drawn

Eight tasks draw (0x132538, each 0x132788 with its own number), the lower
numbers first:

| order | what |
|-------|------|
| 0x100 | what is far off beside the road: view 1, the things with bit 0x4000000 |
| 0x100 | the picture kept of the screen (0x2A0E8), when the game's + 0x73E90 is set; nothing else is drawn then |
| 0x101 | the road: view 2, bit 0x2000000 |
| 0x102 | the sky: view 0, bit 0x1000000; then the depth is cleared |
| 0x102 | effects and shadows, through view 2 |
| 0x103 | the depth cleared again, then everything on the road: view 2, bit 0x8000000 |
| 0x300 | the display, and the window that speaks |
| 0x700 | the display's other part (its + 0xC) |

Before any of it the screen is cleared to the road's own colour (0x147620).
What is to be drawn is listed anew each frame (0x132254): scenery, models,
effects and characters, sorted by a number each has.

**A view** (0x100 bytes; the game's + 0x550, 0x14960C): its fog's colour (+ 4),
where the fog begins and where it is all there is (+ 8, + 0xC; 0x132B68 sets
them and 0x132B34 gives them to the PSP before the view's things are drawn), and
the view proper (+ 0x10): the eye (+ 0x90), the mark, up, how far a thing is
still drawn (+ 0xC0; 15000 unless set), how near and how far it sees (+ 0xD4, +
0xD8; 50 and 20000) and its lens (+ 0xDC, 45). `map_tbl` has for each: the fog's
colour, its two distances, how far a thing is drawn, near, far. Every course has
the fog (0.55, 0.62, 0.5) from 1000 to 9000 in all three; the sky's view draws to
10000 and sees to 10000, the far one 60000 and 60000, the road's 15000 and 60000.

**Whether a thing of scenery is drawn** (0x13C7B8 with 0x131128): the sky
always. Another by how far along the road it is from the view's eye: nearer than
its own size (6000 for a row's scenery), always; nearer than that and how far
the view draws, when it is in front of the eye; else not.

**The light.** Each frame (the end of 0x142574) every model is given a light:
the way it shines (the road's + 0xFBE0), a colour by where the model is
(0x138F68), and the road's two black colours. The colour: the model's place is
looked for in the zones of its row's first pattern, each a box turned about the
upright; the first it is in gives the road's colour of that zone's kind (1 to 4),
and none gives colour 0. Every course has 0.8 grey in the open, (0.35, 0.35, 0.5)
in zones of kind 1 (shade) and (1.0, 0.95, 0.85) in zones of kind 2 (the shafts
of light). A model is drawn (0x13E33C) with the PSP's light 0 only, **the zone's
colour as the light's share of what is all about and black for the light's own
and its shine**, and nothing all about besides: so a model is of one brightness
all over, and the way the light shines changes nothing.

### The display

At the game's + 0x10CB0 (0x38 bytes): flags now (+ 0) and as they were a frame
ago (+ 4), the window that speaks (+ 8, the common game's, 0x6B5C8), another part
of the common game's (+ 0xC, 0x69330), the gauges (+ 0x10), her life as last
shown (+ 0x14), her speed (+ 0x18), the time left in hundredths of a second
(+ 0x1C), the gear as last shown (+ 0x2C), and a fade to black of its own (how
long, + 0x30, and how far in, + 0x34: flags 0x80 and 0x100; a black sheet over
the whole screen, 0x132D60).

What the rules and the scripts set of it (0x1335D8):

| | what |
|--|------|
| 0 to 6 | the flags' bits 0 to 6 |
| 7 | the flag 0x200 |
| 8 | how far she has come, in centimetres |
| 9 | a share, 0 to 100 (the gauges' number 1) |
| 0xA | her life; when it is less than it was the gauges play the blow (below, 0xE) |
| 0xB | the most life she can have |
| 0xC | her speed, km/h |
| 0xD | the gear, 0 to 5; when it is more than it was the gauges play 0xF, or 0x10 for the fifth |
| 0xE | a share, 0 to 100 (the gauges' number 7) |
| 0xF | the time left |
| 0x10 | how long the display's fade takes |

Each frame (0x132F9C) a flag that has come on or gone off has the gauges play
something, the time left is cut into minutes, seconds and hundredths, and the
dial's needle is worked out from the speed and the course's five speeds: up to
the first speed 0 to 0.3, then to 0.8, 0.87, 0.94 and 1.0 at the fifth and
over.

| flag | comes on | goes off |
|------|----------|----------|
| 1 | 0 | 1 |
| 2 (a sign) | 2 | 4 |
| 4 (the other sign) | 3 | 5 |
| 8 | 6 | 7 |
| 0x10 | 8 | 9 |
| 0x20 | 0xA | 0xB |
| 0x40 | 0xC, or 0x12 in the fifth gear | 0xD, or 0x13 |

When either sign comes on and neither was on, sound 0xC is played.

**The gauges** (0xDC bytes, 0x133DB8) are a layout (member 3 of `BMG_SYS`, a pack
of twelve: nine textures, then the objects, the motions and the groups; the
layout's eleven textures are the pack's 0 to 6, the common game's figures
(0x2CD24), the pack's 7, none, and the pack's 8) with seven places, and eight
texts of capitals: 0 and 1 the kilometres and the metres come (`%01d`, `%03d`),
2 and 3 her life and the most of it (set to the right), 4 her speed (twice the
size), 5 to 7 the minutes, seconds and hundredths left (twice the size, set to
the right, `%d`, `%02d`, `%02d`). What they play (0x133914, a place and a set):

| | place, set | | place, set |
|--|-----------|--|-----------|
| 0 | 0, 8 | 0xA | 5, 0x12 |
| 1 | 0, 9 | 0xB | 5, 0x13 |
| 2 | 1, 0xA | 0xC | 6, 0x14 |
| 3 | 2, 0xC | 0xD | 6, 0x15 |
| 4 | 1, 0xB | 0xE | 5, 0x16 |
| 5 | 2, 0xD | 0xF | 6, 0x17 |
| 6 | 3, 0xE | 0x10 | 6, 0x18 |
| 7 | 3, 0xF | 0x11 | 6, 0x19 (when the display is made ready) |
| 8 | 4, 0x10 | 0x12 | 6, 0x1A |
| 9 | 4, 0x11 | 0x13 | 6, 0x1B |

2 and 3 also set the gauges' bits 0x10 and 0x20, and 4 and 5 clear them: while
both are set (both signs are up) a count goes up to ten frames, and down again
otherwise, and the count moves the two signs apart, by -24 and 25 times its
tenth (0x134140).

What is what, from the layout's sets and what the rules give: flag 1 is the sign
that something is near enough to be avoided (place 0, groups 26 and 27, at the
top in the middle); flags 2 and 4 the signs of a lane that ends (places 1 and 2:
group 28 with 29 or with 30); flag 8 the bar of the way (place 3: groups 0 to 3
and 33), 0x10 the time (place 4: groups 4 to 6), 0x20 her life (place 5: groups
20 to 25), 0x40 the dial (place 6). The "gear" of 0x1335D8's 0xD is the row of
those she has just avoided, and 0xE the row's time left.

**How the gauges are drawn** (0x13446C): every track of every place, in order;
a track of one of these groups in a way of its own, the rest as the layout has
them.

| group | what |
|-------|------|
| 1, 3, 33 | the mark on the bar, the unit and the point of the figures beside it: moved up by the bar's height (object 1, 103) times the share of the way |
| 2 | the figures beside the mark: the kilometres at object 7 and the metres at object 47, moved up likewise |
| 4 | the time: its three tracks are the hundredths, the seconds and the minutes, each written at object 9 as its track is moved |
| 8 | the dial's needle: a part of a round picture (below) |
| 9, 10 | her speed, written in the middle of object 18; and its unit, moved right by half the figures' width less 5 |
| 14 | the ring of the row's time |
| 15, 34, 35, 16, 17, 12 | the lamps of the row: each has two tracks, and the second is drawn when the row is 0, 1, 2, 3, 4, 5 long (the first never: the number it goes by is never set) |
| 36, 37, 38, 18, 19 | drawn when the row is 1, 2, 3, 4, 5 long |
| 21 | her life's gauge |
| 23, 24 | her life and the most of it, set to the right of object 36: the most, then her life left of it by its width and the stroke's (object 37); and the stroke, moved left by the most's width |
| 28, 29, 30 | the signs' parts: in place 1 moved across by the first of the two distances above, in place 2 by the second; in any other place not drawn |

A text is written where its object is, moved as its track is, in the object's
colour by the track's (0x135190): the kilometres, the metres, her life and the
time with a shadow, her speed without.

**The round gauges** (0x32E40 of the common game, and 0x135760 for her life) are
fans of triangles about a middle, a corner at the middle and one for every
eighth of the whole way round that the share has, with the last where the share
ends; a corner is `radius x (sin a, cos a)` from the middle, on the screen and
in the picture alike, so that so much of a round picture is shown. Each is cut
to a part of the screen (0x15B098) and has its track's alpha.

| | picture | its middle in the picture | begins, goes round | radius | middle on the screen | cut to |
|--|---------|---------------------------|--------------------|--------|----------------------|--------|
| the needle | the pack's 1 | (32, 69) | -67.5, -225 degrees | 80 | the middle of object 17, (440, 232) | object 17 |
| the row's time | the pack's 8 | (20, 20) | 0, -360 | 40 | object 24's place, (440, 232) | object 24's size about it |
| her life | the pack's 5 | (47, 34) | -45, -90 | 60 | 46 right of object 32's corner and half its height down, (440, 232) | object 32 |

The needle is (114, 247, 255), the ring (0, 240, 255); her life goes from
(87, 255, 249) where it begins to (47, 116, 255) at its full, the corner at the
middle having the colour between.

Looked at with: `python tools_re\pdk.py MINI_GAME\BMG_SYS.BIN [folder]` (lists a
pack, or writes its members out), then `python tools_re\sscr_dec.py
..\..\tools_re\out\bmg\SYS\002.sscr` on a script written out so, and
`python tools_re\sscr_data.py out\bmg\NORMAL\000.sscr map_tbl` for a table's
bytes.

### Its sounds

**A sound of its own** (0x149624(slot, loudness); `n2e(1, n)` and the rules):
sound `n` of the bank is played into slot `n` of twenty-three (the game's
+ 0x73E18 keeps what plays in each), whatever was in the slot being stopped
first; sound 13 is not begun while there is talk. 0x14979C stops a slot.
`n2e(0, n)` plays sound `n` of the common game's bank (0x149704). **A voice**
(0x149B80; `n51(n)`, and the rules' when the gauge fills): recording `n` of the
system's `voice_tbl`, which are the pack's members 104 to 119, on the second
stream if that is free, else on the first. **The tune** is the number in the
stage's record (+ 0x50: 8 for every stage), asked for as the stage's files are
(0x149880, the common game's 0x9F84); `n53` asks for another and `n52(blocks)`
has it fade out (0xA10C). When the ride is over all its sounds are stopped
(0x1384FC).

In the port (`bike/show.rs`, `request`) these are asked of the sound's board
like any other. The packs lie in a data folder made before the extractor took
them apart as single files: the bank is read out of the pack as it is
(`GameData::read`, by the path `MINI_GAME/BMG_SYS.BIN/001.bin/06.bin/00.pphd`),
and `brs-extract --sound` makes what the voices are played from beside the
pack, in `MINI_GAME/BMG_SYS.BIN.sound`.

### Its effects

A character has twenty slots for effects (`n1a(id, slot, res)` puts the effect
`res` of `effres_tbl` in one: an `EFC` file, the system's members 74 to 99 or
the course's). An entry is 0x30 bytes at the character's + 0x140: flags (+ 0),
a model's slot (+ 4) and a bone's number (+ 8, less than nought for none), its
size (+ 0xC, 0x13C3C0), where it is (+ 0x10) and how it is turned (+ 0x20), both
set with `n44`. `n1d(id, slot, what, v)` (0x13C248): 0 a number of the effect's
own, 1 the bone, 2 **it follows** (flag 1), 3 the model, 4 **it has the clock's
time, not the game's** (flag 8), 5 **it is not drawn** (flag 0x20), 6 **it
stands still** (flag 0x10).

Each frame (0x13C9C0 with 2), for each: one that has played out is taken out
of its slot (so that `n43` then answers no). One that stands still is left.
One that follows, or has not been put yet, is put: its own place and turns
from where the character is, or from the bone's place in the world (0x13C754,
0x13EA00: the bone is found by its number). Then it is given its size and the
frame's time: the game's (a frame is 0x800 of the effects' units at thirty a
second), or the clock's if it has that.

In the port: `bike/show.rs`, `show_effects`, with the engine's effects
(`brs_engine::effects`); what has played out is told to the ride
(`Ride::effect_over`).

### Its talk, and its pause

**Talk.** `n34(n)` sets bit `n` of the game's + 0x73EB0. Each frame (0x149CBC),
while none goes on and the game's flag 4 allows it, the lowest is begun once the
first stream is free: the script the game's + 0x73E88 holds is started at
`async_event` with `n` (0x130778 with 1: a "sysScrTask", the task that runs a
script for a screen that is not the field's; its entries are the table at
0x172D50, `mission_select` and `async_event`). That script is the stage's
events, `FLD/FEVT/FEVT_<events>.BIN` (the third of the task's files): an event of
the field's like any other, whose `async_event(n)` says its lines through the
window people speak to her through (see `FIELD_NOTES.md`, "Events, and the
messages people send her"). While it goes on sound 16 is turned down and sound
13 stopped; when it is over (0x130998) the flag goes and sound 16 comes up again.

In the port the field plays it (`field::talk_of_the_ride`): the event is put in
the field's own queue with the talk's number and run by the field's player, and
its window is drawn over the ride. Seen: the easy course's first talk, Phobos's
"How are you liking the bike, kid?" and Rothcall's two lines, with the camera's
own motion for talk (`docs/compare/bike_talk_port.png`; the walkthrough has it
at 5780 to 5792 s).

**The pause.** START (0x147BA8), while the ride is not ending, brings up the
common game's pause of kind 3 (0x12CFF0), has a picture of the screen taken
(the game's + 0x73E94) and holds the sounds. The task `_pause_task` (0x14829C)
then has nothing drawn but that picture (+ 0x73E90) until the pause is over
(0x12D0D8); given up from it (0x12D304 answers 2) the game's event 5 ends the
ride at once with outcome 3. In the port: `bike/show.rs`, `pause_ride`, with
`pause::Kind::Ride`; the ride, its models and its effects have no frames while
it is up.

### What the display says, shadows, and a ride nobody plays

**`n28(text)`** (0x144768, 0x132E9C) has the display's window, the common game's
that a script puts up (0x6B5C8), say a text; `n29` waits until it has been read
(0x132ECC). Of all the courses only `ex00` uses it, once: "Goal speed: 360 km/h".
In the port the window is the field's (`field::window::Window`), put up by
`bike/show.rs` and drawn with the layout the pause has for its question.

**A character's shadow** (0x30 bytes at its + 0x7F0; 0x146A8C makes it, 0x146B1C
draws it with the effects, before the characters): whether it is off (+ 0, bit
0; `n09(id, 10, v)`), how wide and how long (+ 4, + 8: 60 unless set, `n09` with
11 and 12: she has 80 by 210, what she meets 120 to 260 square), how strong
(+ 0xC), where (+ 0x10). It is the common game's round shadow (the texture of
0x1100BC(0)) on a flat square of that size, 3 above where the character is,
turned to the ground, with an alpha of 150 times its strength. `n09(id, 10, v)`
leaves it out when `v` is not nought (0x146F4C: bit 0 of its flags), which the
scripts of what is met do as they make it and undo later. In the port:
`crate::shadow` and `bike/show.rs`, `show_shadows`; the lie of the ground is not
looked at.

**A ride nobody plays.** A run that passes battles over (`--skip-battles`) has
the ride ridden by a rule of thumb (`Ride::rule_of_thumb`: away from whatever
is ahead in her lane, by the display's signs past a hole, her special whenever
it can be had), with her taking no harm, so that what follows a ride ridden
through can be looked at unattended.
