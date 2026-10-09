# Battle system notes

Working notes on how battles work in the original, read out of the executable
(`game/extracted/PSP_GAME/SYSDIR/EBOOT.OLD`, addresses are virtual addresses) and the
data files. "Verified" means checked against data or running code in the port; anything
else is a reading of the disassembly that has not been exercised yet.

## Where things are in the executable

The battle engine is C++ (objects with function-pointer slots). Class names survive as
strings; each is referenced from its constructor:

| Class | Constructor | Notes |
| --- | --- | --- |
| `BtActorState` | 0x7437C | Base of every player/enemy action state. Player state code runs 0x74000-0x85490, one translation unit per skill |
| `BtBgColor` | 0x85490 | The black sheet that dims the map behind her for a skill's scene (see "Her skills") |
| `BtBullet` | 0x862E4 | |
| `BtAssultBullet` | 0x87684 | The player's normal shot |
| `BtRoot` | 0x8D3A0 | |
| `BtCutin` | 0x8D860 | |
| `BtEncountFade` | 0x9062C | |
| `BtEnActor` | 0x91BBC | Enemy actor |
| `BtEnter` / `BtFinish` / `BtGameOver` | 0x93CBC / 0x940C8 / 0x94988 | |
| `BtItemMenu` / `BtItemMng` | 0x94DD4 / 0x950EC | |
| `BtLoad` | 0x958F4 | |
| `BtLogMng` | 0x96368 | Battle message log (`btl_msg_tbl.bin`) |
| `BtMapActor` | 0x96B40 | |
| `BtPause` | 0x97958 | The battle's pause: START while the fight is on (its flag 4, 0x93FE4 to 0x8D79C) makes the game's pause with kind 4 (0x97A30), and until that has gone everything of the battle that moves stands still (0x97BB8). See "The start window" in `MENU_NOTES.md`; `battle::pause` in the port |
| `BtPcActor` | 0x999E8 | Player actor; code from about 0x97600 |
| `BtPcMdlData` | 0x9B994 | |
| `BtResMng` | 0x9C46C | Resource lookup by file name (0x9C974) |
| `BtResult` | 0x9D7C0 | |
| `BtSimpleEffectManager` | 0x9E81C | |
| `BtSkillPallet` | 0x9F068 | |
| `BtStageActor` | 0xA0484 | |
| `BtWipeManager` | 0xA2184 | |
| `HtBase`, `HtObjectBase`, `HtGameActor`, `HtScene*`, `HtTask`, `HtUtilJob` | 0xA25F4-0xAE7CC | Engine layer under the battle classes |
| battle script VM | 0xC8F1C-0xCE648 | See below |

## `BXCB` tables

All 183 files share one layout (verified by `tools_re/bxcb.py survey`):

| Offset | Meaning |
| --- | --- |
| 0x00 | `BXCB` |
| 0x04 | file size |
| 0x08 | 0 |
| 0x0C | offset of the records (always 0x20) |
| 0x10 | record size |
| 0x14 | record count |
| 0x18 | offset of the string pool (= records offset + size x count) |
| 0x1C | 0 |

Records are arrays of 32-bit values. String fields hold an offset into the pool; the
pool starts with an empty string, so offset 0 is "no string". Strings are UTF-8.

| File | Record | What it is |
| --- | --- | --- |
| `SYSTEM.VOL/btl_system.bin` | 0x20 x 88 | Tunables: id, type (0 integer, 1 hundredths), value, max, min, name, description, index |
| `SYSTEM.VOL/btl_atk_tbl.bin` | 0x58 x 177 | Attacks |
| `SYSTEM.VOL/btl_bullet_tbl.bin` | 0xC4 x 215 | Bullets |
| `SYSTEM.VOL/btl_eff_tbl.bin` | 0x34 x 71 | Status effects (id, type, chance, duration in 1/60 s, value..., message ids) |
| `SYSTEM.VOL/btl_finish_tbl.bin` | 0x58 x 24 | Finishing moves |
| `SYSTEM.VOL/btl_msg_tbl.bin` | 0x0C x 184 | Log messages: id, text, display time in 1/60 s |
| `SYSTEM.VOL/btl_skl_tbl.bin` | 0x50 x 66 | Skills (the string is the skill's archive in `BTL/SKL`) |
| `BTL/CHR/*/param1.bin` | 0x10 | Per enemy |
| `BTL/CHR/*/param2.bin` | 0xC4 | Per enemy: one record per variant: id, name, ..., level, HP, ... |

`btl_system.bin` values that matter for the player (id: value):

- 49 heat gauge maximum 100; 13 heat recovered per second 22.05 (83: 27.56 while
  overheated); 14 wait before recovery starts 30.00 (in 1/60 s); 15 extra wait after a
  step 0
- heat cost: 48 normal shot 10, 51 dodge 15, 52 guard 0, 50 item 0, 53 taking damage 0
- 47 stiffness after a shot 10 (1/60 s)
- 55-58 heat thresholds between burst stages: 39, 59, 69, 89
- 59/60 stage 1: 6 bullets, 6 apart; 61/62 stage 2: 3, 6; 63/64 stage 3: 3, 6; 65/66
  stage 4: 2, 6; 67/68 stage 5: 1, 6
- 5 hit stop 6
- battle camera: 40-42 position (-145, 35, -265), 43/44 rotation (-10, 12) degrees,
  39 X rotation axis height 100, 45/24/25 up-down default/min/max 0/0/10, 46/26/27
  left-right default/min/max 0/-40/20
- 54 guard correction 50; 69 time before skills can be used 600; 70 recast 1800
- 79/80 HP gauge turns yellow at 50%, red at 25%; 81/82 heat gauge yellow above 40%,
  red above 80%
- 87 blade hit height limit 330

## Battle scripts (`bscr`, the `.bms` files)

203 files: one per enemy archive, one per battle map, and the player's (`brs00.bms`,
which is nearly empty: the player is driven by the executable, not a script).

### File layout (loader at 0xCE3B8)

| Offset | Meaning |
| --- | --- |
| 0x00 | `bscr` |
| 0x06 | source file name |
| 0x24 | file size |
| 0x28 | function count |
| 0x2C | offset of the function table |
| 0x30 | string count |
| 0x34 | offset of the strings (zero-terminated, back to back) |
| 0x38 | code size |
| 0x3C | offset of the code |

Function table entry: u16 entry size, u16 type, char name[32], u32 code offset (from
the start of the code), u32 argument count, then 8 bytes per argument.

Function types: 0xE0 plain, 0xE1 plain returning a value, 0xE4 run once when the
script is loaded, and the entry points the engine calls by type rather than by name:
0xE7 (one argument; context slot 0x68), 0xE8 (one argument; slot 0x64), 0xE9 (one
argument; slot 0x60), 0xEC "signal" (three arguments, returns a value; slot 0x70),
0xED "update" (no arguments; slot 0x6C).

### Instructions (interpreter step at 0xCC3A0, run loop at 0xCE0B8)

Each instruction is 16 bytes: opcode, three operand-kind bytes, three 32-bit operands.
Unused kind bytes and operands are filled with 0x5A.

Operand kinds: 0xF0 immediate, 0xF2 "push the result", 0xF3 "pop", 0xF4 global integer,
0xF5 global float, 0xF6 local integer, 0xF7 local float (0xF1, 0xF8, 0xF9 appear only
as fillers). Scripts only ever use F0, F2, F3, F4 and F6. A script has 128 globals
(context + 0x104) and each function 16 locals, which live in the function's table
entry (so they are per function, not per call) and are zeroed on each call. Arguments
are popped into the first locals, the last argument first.

| Opcode | Meaning |
| --- | --- |
| 0x02-0x06 | add, sub, mul, div, mod: operands by kind (pop or immediate), result pushed |
| 0x08 | and (same operand handling as add) |
| 0x09, 0x0A | or, xor: both operands popped |
| 0x0B, 0x0C | shift left, arithmetic shift right (unused by scripts) |
| 0x0E | logical not of the popped value |
| 0x11, 0x12 | increment, decrement a variable |
| 0x13 | move: pop to variable, variable to stack, immediate to variable, variable to variable |
| 0x18 | push immediate |
| 0x1A | push string (operand is the string's byte offset within the string area) |
| 0x1B | no effect (pops and pushes the same value; the compiler emits it after pushes) |
| 0x1C, 0x1D | save / restore the stack index for the current call level |
| 0x70-0x75 | compare <, <=, >, >=, ==, != (signed), result 0/1 pushed |
| 0x80 | if the popped (or immediate) value is non-zero, skip the next instruction |
| 0x81 | call function (operand 1 = index) |
| 0x82, 0x86 | call engine function from table 1; number = kind byte 1 (+0x100 for 0x86) |
| 0x87, 0x88 | call engine function from table 2; number = kind byte 1 (+0x100 for 0x88) |
| 0x83 | return |
| 0x84 | jump; operand 1 is the byte offset from this instruction |
| others | no effect |

Operand order, exactly as the interpreter does it (a = first operand, b = second):
for sub and the comparisons with both operands popped, b is popped first, so the
value pushed earlier is a. For div and mod the first pop is a. Scripts only use
`pop / immediate` for div and mod, so the difference never shows.

Engine functions: table 1 at 0x171370 (282 entries, dispatcher 0xC9B58) and table 2 at
0x1720A8 (127 entries, dispatcher 0xCB5B8). Each entry is {result kind (1 none, 0x81
value), argument count, number}. A result is pushed when the kind is not 1. Scripts use
154 functions of table 1 and 108 of table 2.

A script can be suspended inside an engine function (context flag 0x80, optionally
with a "still waiting?" callback at context + 0x310) and resumes at the next
instruction on a later tick, at whatever call depth it was. The "signal" entry point
runs on a copy of the context so that it can be called while the script is suspended;
only its changes to the globals are kept.

### How an actor runs its script

Each actor has an owner record: the context, the time step of the current run (+4),
and flags (+0xC): 1 running (a signal sent meanwhile is dropped), 2 "cancel", 4 has
entry points, 0x10 started.

- Every frame (0xADFBC, with the actor's time step): if the script is suspended it
  is resumed (0xC9838): the callback is asked, and if it says the wait is over (or
  there is none) the script runs on from where it stopped. Then, if the script is
  not suspended (it never was, or it has just run to its end), the update entry is
  called from the top (0xC95E8). So the frame a wait ends on, the rest of the update
  runs and the update starts again at once.
- A signal (0xAE0D8 -> 0xC9664) runs the signal entry with (number, 0, 0). If the
  signal set the cancel flag (engine functions 25 and 28 do), the suspended update
  is then thrown away (0xC98E4: stack and call depth reset), so the next frame starts
  the update from the top: how a hit or the AI interrupts whatever an enemy was doing.
- The waits: function 9 waits N logic frames (N x 2 sixtieths; the callback at
  0xBC768 takes the step off each frame and ends the wait at zero or below), 10
  waits for a model's animation to reach the end of its range (0xBC7F4), 31 waits
  for the current move to end (0xBCF28).

`tools_re/bscr_dec.py` turns a script back into readable statements.

## Logic frames

Field and battle logic run 30 times a second (menus at 60). Times are counted in
sixtieths of a second: one logic frame adds `dt` = 2 (0xA80B4, scaled by tunable 8
"frame ratio" at 0x9BB78) to timers, and advances animations by two frames.

Everything of a battle is a task (`HtTask`, 0xAE20C) with a function to run before
its children (at + 0x38) and one after them (+ 0x40), and two to draw with, before
and after its children (+ 0x48, + 0x50). A task joins the end of its parent's
children (0xAE53C), and the run goes on to whoever is next only once a child has had
its turn (0xAE30C), so one made during a frame has its first turn in that frame.

Per frame the battle root's own update comes first (0x8CF6C): the battle's clock,
the time of the effects that are the battle's own (0x8D094; see "Effects in a
battle"), the shake's swing (0x9DED8) and the camera (0x8CE94). Then its children
have theirs, in the order they joined: she, the enemies, and whatever has been
fired. The player's own update (0x99E50) does, in this order:

1. hit stop runs down (0x742D8);
2. the heat gauge cools, unless a state has locked that (0x974CC);
3. status effects tick;
4. her models' animations advance by `dt` (0xA7520);
5. the aim cursor is moved by the pad (0xA1CC8), if she can act;
6. the current state's update runs (0x9936C; with no state, "idle" is created);
7. she moves if a dodge is carrying her (0x99130);
8. her model is placed from her position and direction (0x992DC);
9. the pad is read and the next state chosen (0x9A474). Its constructor runs at
   once, its first update on the next frame.

The port does the same in `crates/brs-game/src/battle/actor.rs`, with step 4 done by
the engine just before.

## Buttons

The pad code (table at 0x16A3C4) turns the hardware's bits into the game's:

| Bit | Button | In battle (0x9A474) |
| --- | --- | --- |
| 0x001 / 0x002 | Start / Select | |
| 0x004, 0x008, 0x010, 0x020 | D-pad up, right, down, left | move the aim cursor (the stick does too) |
| 0x040 | Triangle | skill slot (with R) |
| 0x080 | Circle | dodge, when pressed |
| 0x100 | Cross | guard, while held |
| 0x200 | Square | shoot, when pressed |
| 0x400 | L | open the item menu, when pressed |
| 0x800 | R | held: the other buttons pick skills and do nothing else |

## Player actor (`BtPcActor`, 0x870 bytes, built at 0x999E8)

| Offset | Meaning |
| --- | --- |
| 0x1E0 / 0x1E4 | maximum HP / HP (floats, from the save data) |
| 0x1E8 | status-effect block (0x8EFA0(block, type) tests for an effect type) |
| 0x708 | state kind: 1 normal, 0xFF not controllable |
| 0x70C / 0x710 | hit stop: flag and time left |
| 0x734 | the item menu is open (the actor's logic waits) |
| 0x740 / 0x744 | current state object / action number |
| 0x748 | alive; cleared when a state ends with HP at 0 |
| 0x750 | a state is running: do not fall back to standing |
| 0x754 | in the middle of a dodge (read once, in the damage code at 0x9B594) |
| 0x758, 0x75C, 0x760 | dodge, guard and shoot are locked |
| 0x764 | overheating may not interrupt the state yet |
| 0x768, 0x74C | written by every state, never read |
| 0x76C, 0x770 | skills, items are locked |
| 0x778 | the heat gauge does not cool |
| 0x77C / 0x780 | heat maximum (tunable 49) / heat |
| 0x788 | time left before the heat starts to fall |
| 0x78C | bit 0: overheated |
| 0x794 | which standing place she is at or heading for (0 or 1) |
| 0x7A0 | dodge destination; 0x7B0 moving; 0x7C0 velocity; 0x7E4 braking factor; 0x7E8 distance left; 0x7EC dodge speed (45) |
| 0x7F0 | aim cursor mode: 0 the player's, 1 automatic (skills) |
| 0x7F4 | aiming: her upper body bends towards the cursor |
| 0x7F8 | set by a guarded hit |
| 0x7FC | something else places her model (skills) |
| 0x80C | position, and at +0x10 the direction she faces |
| 0x830-0x854 | written by the idle states, never read |

Action numbers (0x744): 1 defeated, 3 idle, 4 shoot, 5 aim-idle, 6 and 7 skills, 9
guard, 10 dodge, 14 overheat. A fresh actor has everything locked and action 0; its
first update creates "idle" without counting it as an action.

Actor flag 0x7C bit 1 (0xA7C34 sets, 0xA7C44 clears, 0xA7C58 reads) is "guarding":
the damage code asks it first.

### Heat

- Adding heat (0x97658): nothing happens under status effect type 22. Otherwise heat
  rises by the amount, clamped to the maximum; at the maximum the overheated bit is
  set. Every call sets the wait before cooling to tunable 14 (30).
- Cooling (0x974CC), once the wait has run out (the frame it runs out already cools):
  heat falls by `dt` x rate / 60, the rate being tunable 13 (22.05) or, while
  overheated, tunable 83 (27.56). Reaching 0 clears the overheated bit.
- While overheated she can start nothing; an emptied gauge lets her act again.
- Gauge level (0x97770): 3 overheated, else 0 up to 33%, 1 up to 66%, 2 above.
- Burst stage (0x99644): heat below tunables 55/56/57/58 (39, 59, 69, 89) gives stage
  1 to 4, else 5. Bullets per burst are tunables 59/61/63/65/67 (6, 3, 3, 2, 1) and
  the time between them 60/62/64/66/68 (6 each).
- Equipment changes two of these when a battle starts: tunable 59 is overwritten
  with field 0x14 of the equipped item in slot 9 (0x8D47C), and a dodge's heat is
  field 0x10 of the item in slot 12 (0x7E7B0), not tunable 51. The port does both
  as a battle starts (`battle/mod.rs`).

### Choosing the next state (0x9A474)

In this order, the first that applies wins:

1. not alive: the defeat state (0x78E88), action 1;
2. L pressed and items allowed: the item menu;
3. R held and skills allowed: a skill slot's button starts that skill; nothing else
   below can start while R is held;
4. guard: lock 0x75C clear, no blocking status, not overheated, cross held;
5. dodge: lock 0x758 clear, not overheated, story flag 7500 set (or tunable 38),
   circle pressed. Adds tunable 15 to the cooling wait if that is shorter. The
   flag is what the first of the challenges is worth: four battles into the
   first mission (the port reads it each frame, `Rules::dodge_unlocked`; a
   battle run alone has it set);
6. shoot: lock 0x760 clear, not overheated, square pressed;
7. overheated and lock 0x764 clear: the overheat state, unless already in it;
8. lock 0x750 clear: aim-idle if aiming, else idle, unless already in it.

### Animation player

An actor's model has animation slots of 0x40 bytes (`model + 0x50`), slot 0 being the
body. Times are in frames of 1/60 s. The wrappers used by states:

| Function | Meaning |
| --- | --- |
| 0xAA1D0(model, anim) | switch animation at once, time 0, play to the end |
| 0xAA220(model, anim, frames) | switch with a blend from the current pose over that many frames |
| 0xAA2A0(model, t) | set the time (clamped to the animation) |
| 0xAA2E8 | current time |
| 0xAA364 | animation length |
| 0xAA388(model, frames) | hold the animation still for that long first (the frame the delay runs out on does not play either) |
| 0xAA3D0 | loop over the animation's own loop range |
| 0xAA408(model, a, b) | loop between frames a and b (a = b holds on that frame once reached) |
| 0xAA4A8 | stop looping: play through to the end |
| 0xAA460 / 0xAA484 | start / end of the current loop range (both the length when not looping) |
| 0xA8CC0 / 0xA8CC8 / 0xA8CD0 (anim) | an animation's length, loop start and loop end |
| 0xAA100(model, id) | index of the node with that id, or -1 |
| 0xAA868(model, node, matrix) | override "mode 4": turn the node by the matrix in the model's space, about the node (applied at 0xA9CCC); a null matrix removes it |
| 0xAA730 | override "mode 2": the matrix is multiplied onto the node's own |
| 0xAA9A0 | remove every override |

### Player states

Each state is an object with an update function (+0x88), an exit function (+0x20), a
phase counter (+0x94) and the actor (+0x90). All of them show part modes 4 and 5.

| State | Ctor / update | What it does |
| --- | --- | --- |
| Idle | 0x7D324 / 0x7D410 | Clears every lock but 0x750. `btl0000` looping, switched to at once unless already playing |
| Aim-idle | 0x7DC94 / 0x7DD60 | After a burst: keeps the last pose with the cannon raised. Ends (clearing "aiming") when the gauge overheats |
| Overheat | 0x7D608 / 0x7E080 | One frame with everything locked, `btl0000`, effect 700001 twice; then unlocks. She stays in it, unable to act, until the gauge is empty |
| Hit | 0x7D750 / 0x7E2F4 | `btl5000_all` (argument 0) or `btl5001_all`: runs to frame 6, waits there for the hit stop, plays out, unlocks |
| Guard | 0x7D8AC / 0x7E4CC | Heat tunable 52. `btl6000_all` held at frame 0 while cross is held. A guarded hit (0x7F8) replays it from 0 to its loop end. On release, once that has played, it jumps to frame 22, unlocks the actions, and plays out |
| Dodge | 0x7DA7C / 0x7E704 | Toggles the standing place. Heat (see above), sound 0x78, `btl2020` (to place 1) or `btl2021` (back) blended in over 8 frames after an 8-frame hold, effects 608006 or 608008, 608007, 608010. Moves at 45 per 1/60 s straight to the other place. With 200 or less to go: effect 608005, brakes (factor 0.15), blends to `btl0000` over 16 frames, unlocks |
| Shoot | 0x7DEA0 / 0x7EB68 | `btl3000` at frame 38 at once (the first 38 frames are never played); sets "aiming"; takes bullets and interval from the heat stage; adds heat tunable 48. Per bullet: plays 38 to 46 and, unless overheated, sound 0xA0, effect 999004 on node 0x32, and a bullet (0x98D5C). After the interval the next bullet, so one every 4 logic frames; after the last, the interval and then tunable 47 pass before she can shoot again, and everything else unlocks |

Movement (0x99130), while a dodge carries her: the velocity loses `factor x dt` of
itself per frame once braking (components under 0.1 become 0); the step is the
velocity x `dt`, cut to the distance left; she always faces the stage's middle.

Effect numbers are decimal in the code and data (e.g. 999004, 608005, 700001).

### Model

- Battles use the field model of the costume worn (`FLD/FCHR/FCHR_P_BRS0N.BIN`) with
  the battle animations of `SYSTEM.VOL/pack.vol`. Animation slot 1 loops
  `chr_p_brs02_fld9700_hair.anm` unless tunable 36 is set.
- Parts (0x9B608), by mode: 0 shows 60 and hides 61, 72, 50; 1 shows 61; 2 shows 72;
  3 shows 70 and hides 71, 51; 4 shows 71 and hides 70, 51; 5 shows 60, hides 61, 72,
  50 and attaches the weapon model; 6 shows 70 and 51 and hides 71. Parts 60/61/72
  are right-hand shapes, 70/71 left-hand shapes, 50 the cannon built into the model
  and 51 the blade.
- The weapon is a model of its own (`BTL/WEP/ITM_EQU_ASSUALT_LV1-4.ZZZ`) fixed to node
  0x50. Verified: it then coincides with the built-in cannon.
- Node 0x32 is the muzzle; nodes 1 and 2 bend her towards her aim; nodes 0, 0x14 and
  0x18 get something set at 0x99D48 (radii 75, 20, 20: probably shadows).

### Aiming her body (0x993B8)

Run each frame of the shoot state while "aiming" is set: every override is removed;
then, if the cursor points at something, the rotation from "facing ahead" to "facing
the aim point" (both measured from 100 above her feet) is halved and given to node 1
and to node 2 as a mode-4 override, so that from node 2 up she is turned all the way.
Outside the shoot state the overrides stay as they were, and they are removed as soon
as "aiming" is cleared (0x9A184).

## Shots

Firing (0x98D5C) takes the aim point, the muzzle's place, and makes a
`BtAssultBullet` (0x87684) with velocity 150 per 1/60 s towards the aim point, a
collision sphere of radius 70, and a life of 8000 / speed (so a range of 8000). It
is hidden until it is 2/60 s old (0x87A2C), drawn as effect 999036 scaled twofold
(unless the battle system's value 30, "no efc scale", is set) and turned the way it
flies (0x87954), and moves in a straight line (0x8789C).

Its update (0x8B310) has its first turn on the frame she fires, since it joins the
end of the tasks being run; but it keeps the number of the frame it was made on
(+ 0x24C) and does not fly on that frame (0x8B374). So on the frame she fires it is
at the muzzle and not to be seen, and on the next it is a frame's flight on (300
units) and seen. Flown or not, it then tries what it has swept through, from where
its sphere was to where it is (see "Collision spheres").

The aim point (0x989F8) comes from the ray through the cursor (camera position, and
the cursor's pixel unprojected at 0x98488), tried against, in order:

1. each enemy's collision data whose flags have bits 1 and 3 set (0x98560): the point
   of the ray nearest to the data's node, if within its radius. The first enemy in
   the list that qualifies wins, not the nearest;
2. the ground plane y = 0, in front of the camera (0x98720);
3. the plane z = 5000, where the ray meets it above the ground (0x988A4).

## Stage layout (0xA0A80)

The enemies are around the origin. The player's two standing places are the point
(0, 0, -R) turned about the vertical by -(A/2)/R and +(A/2)/R radians, with R = 1500
(root + 0xEC) and A = 800 (root + 0xF0): about (395, 0, -1447) and (-395, 0, -1447).
She starts at the first.

## Camera

Six cameras hang off a manager (0x8BB24, global at 0x171250); the battle starts on
number 1 (0x8D5D0).

- 0: follows a point (0, 145, -640) behind the player, easing by 10% a frame, with a
  vertical field of view of 0.506 rad.
- 1 (0x8C334), the normal one: fixed to the player. With her position P and facing
  frame F (Z along her direction, X = up x Z), the camera's matrix is
  `F . T(0, h, 0) . RotY(turn) . RotX(tilt) . T(x, y - h, z) . RotY(yaw) . RotX(pitch)`
  with tunables h = 39 (100), (x, y, z) = 40-42 (-145, 35, -265), pitch = 43 (-10
  degrees), yaw = 44 (12 degrees). The eye is its origin and it looks along its +Z.
  The lens is 35 mm on a 1.417 inch film back: 0.567 rad vertically on the 480 x 272
  screen. All matrix helpers post-multiply (0x23EE0), and the rotations are the usual
  right-handed ones (verified from the VFPU code at 0x16216C).
- 2 (0x8C844): camera 1 zoomed towards a target (snipe skill).
- 3: an animated camera from a `.cam` file (skills, 0xA7678).
- 4: a fixed overview at (45.85, 1459.03, -3240.26).

`turn` and `tilt` start at tunables 46 and 45 (both 0) and are changed by the aim
cursor (0x8C640): tilt is kept within tunables 24..25 (0 to 10 degrees, looking
down) and turn within 26..27 (-40 to 20 degrees; negative is to the right).

The camera's matrices are worked out when the frame is drawn (0x8D0C4), so logic
sees the camera of the frame before.

## Aim cursor (0xA1CC8)

On the 480 x 272 screen the cursor starts at (240, 136) and stays within x 180..300
and y 76..196. Each frame:

- the D-pad's direction, or the stick if the D-pad is not used, moves it by 7.5
  pixels x `dt` x deflection, and unlocks it;
- with no input, a lock point within 64 pixels (or the one already locked onto) draws
  it at 30 pixels x `dt`, and it counts as locked (sound 0x6A the first time);
- whatever would carry it out of the box turns the camera instead, by
  `2 atan(tan(fov / 2) x pixels / 272)`: sideways overflow changes `turn`, vertical
  overflow `tilt`.

Lock points (list at 0x171300, 0x60 bytes each, built at 0xA1144) belong to enemies:
an actor, a model and node, the world position (+0x30) and the screen position
(+0x40). The current target is at 0x171314 and the "locked" flag at 0x171310. The
target is kept only while the cursor is settled on it (0xA1F64 stores nought
there otherwise).

A flag at 0x171308 (set by 0xA15E4, cleared by 0xA15F8) has the search for the
nearest lock point (0xA146C) find none. The point the cursor is on already stays
its target; no other draws it. Her skills that fire a volley set it for as long
as the volley lasts, so that the cursor can be swept over the enemies.

## The display (0x3556C; the entries' code at 0x3C988 to 0x3EA58, drawn by 0x406B0)

A battle's player makes its interface as she is set up (0x8D4B0): a record of two
objects that each play a layout and keep *entries* on it (0x50 bytes each: kind
+0xC, set +0x10, flags +0x14, a place +0x20, a parameter +0x30, a number +0x34, a
share +0x3C). The first (0x3CCA4) plays the battle's own layout, the first files of
`if_btlsys.lpk` (objects, motions, groups), with seven textures: the pack's files
3, 4 and 5, none, and of `if_rsdsys.lpk` its files 20 and 9 and the last texture
of its `018.lpk`. The second (0x3CE0C) plays the layout of that `018.lpk` (the
signs and marks, which the field uses too). Her HP is an entry of a third object,
which the game makes as it starts (see "Her HP gauge" in `docs/FIELD_NOTES.md`);
in a battle it stands where its layout has it (0x36154).

The battle layout's sets: 0 and 1 the line of information, coming and going; 2 and
3 a part of it; 4 and 5 the column of the four buttons, opening and closing, and 6
its four rows' lights; 7 the heat gauge; 8 the count of enemies; 9 another of the
heat gauge; 10 the marks over it. Its files 6 and 11 are packs of two more
layouts: the battle's results, and the window of items. File 7 is a cross on a
dark sheet.

Entries by kind, each drawn its own way (0x166AD8):

| kind | set | what | drawn by |
|------|-----|------|----------|
| 8 | 0 (of its own layout) | her HP | 0x40774 |
| 9 | 7 | her heat (0x3DBE8; share 0x3DC60, overheated 0x3DC98 = flag 2) | 0x40C5C |
| 10 | 4 (signs) | an enemy's mark: its level and HP (0x3DD0C) | 0x3FE94 |
| 11 | 1 (signs) | the aim cursor, and the mark on what is locked on to (0x3E388) | 0x40338 |
| 12 | 8 | the count of enemies (0x3E6FC; the number 0x3E75C) | 0x41108 |
| 13 | 10, or 5 of the signs | the mark of a status effect, hers or an enemy's (0x3DEE4) | by kinds 9 and 10 (0x3D2D0) |
| 14 | 2, 21, 22 or 23 | (of the signs' layout) | 0x40620 |
| 15 | 4, 5 | the column of her skills, with four texts (0x3E794) | 0x41318 |
| 16 | 6 | the flashes of that column's rows (0x3EA58) | by kind 15 |
| 17 | | (not met) | 0x41B9C |
| 18 | 0 to 3 | the line of information: the battle's log (0x3EB54) | 0x41BB0 |

An entry's flags: bit 31 keeps it when its motion is over (one without it is taken
away then), bit 30 holds its motion still (0x3CFD4: its motion goes on, and is
asked whether it is over, only without it), bit 29 sets bit 30 once the motion is
over, bit 28 keeps it from being drawn (0x3D114). The low bits are each kind's
own.

**The heat gauge** (0x40C5C) goes through the set's tracks by their groups: 15 is
the bar, cut to her heat's share of its object's width and drawn in a colour by
that share: (0x8C, 0x8C, 0xFF) up to the battle system's value 81 in hundredths
(0.4), (0xFF, 0xE6, 0x82) up to its value 82 (0.8), (0xFF, 0x78, 0x50) beyond
(0xD2C64). 16 is the bar of a gun that has overheated, cut the same and not
coloured. 17 is her heat in hundredths: two figures at the least, three from a
hundred, six apart from the right, each from its cell of twenty of the HP figures'
sheet. 18 is drawn only while it has not overheated, 19 and 22 only while it has;
26 is where the marks of the status effects on her go (0x3D2D0; see "The marks of
status effects" below).

**The count** (0x41108): group 23 is the number, each figure from its cell of
eleven of the signs' sheet (the second of the sheets at 0x171078: ten to a row,
cells of eleven), twelve apart from the right and moved eleven to the right for
each figure beyond the first; 24 stands in for it when there is no count (a
number below nought); everything else, the word, is moved to the right by as
much.

The number counted is the stage's (0xA0B7C): the enemies standing (in the list,
not on their way out) and those of the group still to come; with a tower in the
group only the ones standing, and nothing (a number below nought) while a tower
stands.

**An enemy's mark** (0x3FE94). Every enemy has a record of its own (0x3550C) with
only the signs' object, and on it an entry of kind 10 playing set 4, put each
frame at the node its script names for it (flags bit 0: a place in the world,
which the draw puts on the screen and cuts to whole pixels). Every track is drawn
about that place rather than where the layout has the group (0x314DC: the place
of the group's first object is taken off). Group 35 is the bar, cut to the share
of its HP of its object's 43 across; 37 is there only for one with a second life;
39 is its level in figures six apart from the left (the ninth of the sheets at
0x171078); 40 (a dark square that is never drawn itself) is the turn in which the
marks of the status effects on it are drawn.

**The marks of status effects** (kind 13). An effect that stays and whose record
has an icon (field 0x24, 0 to 17) has an entry put up for it as it is given
(0x8E334 to 0x35A80 to 0x3DEE4): on her status object (the one whose word at +0x7C
is 0) playing set 10 of the battle's layout, or on the enemy's own signs object
(+0x7C = 1, the object at enemy + 0xA28) playing set 5 of the signs' layout. Both
sets are one track standing still, whose group has an object for each icon: group
26 (objects 39 to 56, at (460, 217)) and group 48 (objects 64 to 81). All eighteen
are fifteen square, from file 20 of the pack that is always there: the first
sixteen four to a row from (1, 161), sixteen apart; the seventeenth (the unseen)
from (17, 33); the eighteenth from (1, 225). The entry is taken away with the
effect (0x8E258 to 0x35AD8 to 0x3DFB0).

Every frame the bearer's effects that have a mark (thirty-two at most) are put in
order (0x8F128, compared by 0x8F068) and each mark is told its number (0x3E1A0):
those that run out before those that stay, the one with least time left first,
and of two alike the one given first (a running number of the bearer's record). A
mark has number 0 from when it is put up until that, so for the frame an effect is
given in the marks can stand otherwise.

The draw (0x3D2D0, from the heat gauge's group 26 and from an enemy mark's group
40) shows one page of them: eight to a page for her, three for an enemy. With the
page at record + 0x1D it takes the marks numbered from page x room, as many as
there are up to a page, and draws for each the object of its entry's group that is
its icon: hers with the layout's inner matrix moved sixteen to the left for each
place (0x31130), an enemy's about the point its mark hangs at, moved (count - 1 -
place) x 16 to the right, by the place of the group's first object as the rest of
the mark is (0x31768). So the first mark is the rightmost of both, hers right over
the right end of her HP gauge, an enemy's last right over the left end of its bar.

With more marks than a page holds, the record's entry (kind 9 for her, kind 10 for
an enemy) is given a wait of 120 sixtieths (0x3E030, at +0x38, with what to call
at +0x40 and flag 27): each frame of the display takes the frame's sixtieths off
it (0x3CFD4), and the frame after it has run out the page goes up by one, or back
to the first when there is nothing on the next (0x3E0E4); so a page is up for 61
frames. Paging begins when a mark is put up and there are more than a page holds,
and ends, with the first page up, when one is taken down and there are no more
than that. A page that marks taken down have left empty stays up for its time.

**The cursor** (0x40338) is an entry of kind 11 playing set 1 of the signs'
layout, on the signs' object of her record; a second record of hers (player + 0xF8,
0x8D4E0) has the same entry with its flag 4 set, which is the mark on what she has
locked on to. All of it is moved to the entry's place (the layout's outer matrix)
from the place of each group's first object. The groups of a state come in fours:
the first is of both, the second of the cursor alone, the third and fourth of the
mark alone; the states are 0 (groups 2 to 5), 1 (flag 8: 6 to 9), 2 (flag 0x10: 10
to 13) and 3 (flag 0x20: only 14 and 15, of both). The state is told to the
cursor's entry alone, when it changes (0xA1A10): 3 with the gun overheated, else
0 up to a third of her heat, 1 up to two thirds, 2 beyond (0x97770); the mark stays
in state 0. Changing state, being shown and having flag 4 changed each start the
entry's motion anew (0x30B88).

Each frame the battle's draw (0xA194C) puts the cursor's entry where the cursor is
and, while there is something locked on to and its point is before the camera, the
mark's entry on that point. Locking on (0xA1608) plays sound 0x6A, hides the
cursor's entry and shows the mark's; letting go (0xA1684) the other way about; both
do nothing while there is no cursor. Her states that aim make the cursor's entry if
it is not there (0x74CB8: the root's + 0x194), those that take the stage take it
away (0x74650); the state it was last told (root + 0x190) is not forgotten, so a
cursor made anew looks as with a cool gun until her heat changes state. A third
thing the cursor's record can have (0x41E18, made with the entry) is a sight of the
battle's layout for two of her skills (her states 0x7FC90 and 0x80180): not read.

**The column of her skills** is a task of its own (`BtSkillPallet`, 0x9F068) with
a record of its own and on it the entries of kinds 15 and 16. Kind 15 starts held
still at the start of set 4, where its motion begins: 140 to the right of where it
ends, so that only its four buttons are on the screen. While R is held in a battle
that is on (root flag 8) it plays set 4 (0x35F58, with sound 0x64), and set 5 when
it is let go or the battle is no longer on (0x35F80): 0 to 144 and back to 140.
Its four rows are 21 apart; each has a text (the skill's name, 0x35FA8) and a share
(0x35FDC: how far the skill is to being ready). Its draw (0x41318), by group: 5
the bar, for each row cut to the share of its object's width (at most to the
screen's edge) in (0x70, 0xA7, 0xF0, 0x80), or (0, 0x71, 0xFF, 0x80) when full; 6
the rows' backs (the set's four tracks); 7 to 10 the buttons, each at half
strength until its row is ready; 11 the names, written at the group's object with
a shadow, in (0x2E, 0xCF, 0xFF) when ready and at half strength when not; 12 for
each row that is not ready and 13 for each that is; 20 the row's flash, a track of
the kind-16 entry's set 6 (started for a row by 0x36010), moved as the column's
first track is. The task (0x9F254) fills a row by `dt` over the skill's time while
the battle is on and nobody has the stage (root flags 8, and neither 9 nor 10),
flashes it and plays sound 0x65 when it is full; a skill starts empty unless its
first time is nought (0x12AD74; after use it is 0x12AD7C). The slots are the
save's (0x14CC00, by the table at 0x16A190). It is drawn while the display is on
(root flag 11) and it has not been taken away (0x9EEAC, 0x9EEB8), and stands still
while the battle loads, is paused, or she has her items open (0x9EBA4).

**The log** (`BtLogMng`, 0x96368) is the line of information: a task with a record
of its own and an entry of kind 18 on it, which starts held still. A line is asked
for by its number in `btl_msg_tbl.bin` with three names, an actor's, a user's and
a target's (0x96978 to 0x969E0), which take the places of `<actor>`, `<user>` and
`<target>` in the line's words (0x96698; 127 bytes at most). An enemy's script
writes one with its own name for all three (engine function 80, 0xBE418); a part
of an enemy that breaks writes line 6 and one that mends line 7, with the part's
name (0x85CBC: the variant's name for its first gauge, record fields 0xBC and 0xC0
for the others); a status effect writes its record's first line as it is given,
once its chance has let it take and whatever comes of it, with the giver's name
for the actor and the user and the bearer's for the target (0x8DE7C, from
0x8FEB4), and its second line with the bearer's name for all three as its time
runs out (0x8E3D0), as a hit ends a stun (0x8F420) and as her use of a skill ends
a boost (0x8F5E4, from her nine skill states), but not when it is taken away by
its number or by a cure (0x8DEF8 has only those three callers). The names are
0x8DC1C's: hers is the three letters BRS in their wide forms (0x99638), an
enemy's its variant's (0x910C0), anybody else's empty. An effect whose record has
no line names line 0, which the table does not have. Her own states and her
skills write theirs (0x744D4 and its nine twins; 0x9F654 would for her skills
that act of themselves, and is given no line to write). The lines wait in a list (0x96A9C): with the list not empty
the band comes (set 0) and with it the first line's words (set 2, sound 0x11);
once that motion is over they stay for the line's time in sixtieths, or for the
battle system's value 73 when another line waits; then the words go (set 3) and
the next come, or after the last the band goes (set 1). The words are written at
the place of group 2's object, moved as its track is, in its colour, with a
shadow (0x41BB0), eighty glyphs with nothing between them (0x3EBB8). The log is
drawn whether the display is on or not, and what follows a battle won waits for it
to have nothing left to say (0x9DB90) unless the battle system's value 86 is set.

**When it is drawn** (0x8D0C4). Her record is drawn while the root's flag 11 is
set, which it is from the battle's start (0x93DF4) until her last state of a
battle won is over and the log has said its last (0x9DBC4), or a script says
otherwise (0xBEA90, 0xBEAAC). The mark on what she has locked on to is drawn only
while nobody has the stage (root flags 9, 10 and 12 clear). The root's flags are
a set of bits at root + 0x84 (0xA42DC sets one, 0xA4348 tests one): 8 the battle
is on (0x93FFC; cleared when it is decided), 9 and 12 she has the stage, 10 set
and cleared by 0xBE194 and 0xBE26C (not read), 11 the display.

In the port: `battle/hud.rs` (HP, heat, count, the enemies' marks, the cursor and
the mark, the column of her skills with its empty rows, the marks of status
effects) and `battle/log.rs` (the log, written to by the enemies' scripts and
their parts and by the status effects' records); `battle/status.rs` keeps the
order of a bearer's marks, the page that is up and the lines for the log.
Compared with the walkthrough in `docs/compare/battle_display.png`,
`battle_start.png`, `battle_log.png` and `status_marks.png` (the mark of a
defence raised and its line, 41:21 to 41:23 of the video: her first mark is
within half a pixel of the original's; `status_marks_port.png` has two marks on
everybody). The marks are in their order from the frame an effect is given.
The sight of the skills that snipe and the log's lines came later (see "Her
skills" and `battle/log.rs`). Looked into on 2026-10-09: every line of the
table that anything writes is written in the port: 1 to 5 and the moves' names
by the enemies' scripts (engine function 80), 6 and 7 as a part breaks and
mends, 8 as a snipe finds nothing, her skills' own by their records, and the
status effects' by theirs. Line 3301 ("No longer Overheated.") is in no
record and no code loads its number: the game never writes it. The results of
a battle won are under "How a battle begins and ends".

## Data tables behind a battle

`SYSTEM.VOL` is read by position, so a resource number in the code is the index of a
file in the archive: 3 `db_resident.lpk`, 0xB `btl_system.bin`, 0xD `enmgrp2.dat`, 0xF
`btl_atk_tbl.bin`, 0x10 `btl_bullet_tbl.bin`.

### `XTC` tables (`db_resident.lpk`)

Header: `XTC\0`, file size, record count, u16 record size at 0x0C; records from 0x10.

- `004.xtc`, the level table (0x12B5C0, level capped at 50): 100 12-byte records {u32
  experience needed, u16 gains in HP, attack, special attack, defence}. Record `n` is
  about going from level `n` to the next: the experience at which that happens and
  what it adds to the four figures (the result screen's level-up at 0x37E24; see
  "What a battle is worth"), HP capped at 9999 and the others at 999 (0x14D2EC).
  Record 0 is all zeroes, record 49 has 999999, and the records from 50 on can never
  be reached (0xFFFFFFFF). The port's `--level N` adds records 1 to N - 1 to a new
  game's figures, the Rock Cannon's 15 attack included, and gives her the experience
  that level starts at: level 20 has 1557 HP, 124 attack, 177 special attack and 85
  defence; level 50 has 2465, 219, 337 and 157.
- `005.xtc`, starting stats (0x12B5EC; applied by 0x14C9D4): eight 20-byte records
  {i32 experience, i16 HP, attack, special attack, defence, weapon item, suit item,
  four bytes: the skills of her slots, for her figures 0xB, 0xE, 0xD and 0xC
  (0x12B550)}; anything below nought is not given. Record 0 is who the opening
  shows (999999, 9999, 999, 999, 999, weapon 131, suit 135, skills 2, 15, 12, 23):
  the game's own start gives it (0x908). Record 1 is a new game: HP 372, attack 29,
  special attack 41, defence 24, weapon 129, suit 133, no skills; the opening's
  script gives it when its film is over (`s262(1)`). Records 2 and 3 have only a
  suit (134, 135); 4 to 7 are her at 35290, 104121, 205786 and 227232 experience
  with things on and skills set, which nothing gives. See "Where a new game
  starts" in `FIELD_NOTES.md`.

The player's stats live in the save data (0xDAC), read by number with 0x14CC00:
0 maximum HP, 1 attack, 2 defence, 3 special attack, 4-7 bonuses to those, 8
experience, 0xB-0xE equipped items, 0xF HP, 0x10 level. Numbers 21-24 are the totals
with bonuses and equipment (0x14CDE0): 22 attack (capped at 999), 24 special attack.

### Attacks (`btl_atk_tbl.bin`, 0x58-byte records)

Found by (attack id, enemy id, variant id) with 0x129068, falling back to any variant
and then to enemy id 0; the player's are found by id alone (0x129014).

| Offset | Meaning |
| --- | --- |
| 0x00 | id |
| 0x04 / 0x08 | percent of the attacker's attack / special attack that makes the power |
| 0x0C | percent of the target's defence taken off |
| 0x10 | percent of the result dealt |
| 0x20 | status effect it carries, by id; see "Status effects" |
| 0x24 | spread: the damage varies by this many percent either way |
| 0x28 / 0x2C | enemy id and variant the record is for (0 and -1: any) |
| 0x30 | the look of a hit: which effect is seen where it lands (see "Effects in a battle"); 0x50 / 0x34 sound bank kind and sound |
| 0x38 | if set, a hit sets the word at 0x171234 (not followed yet) |
| 0x3C | which shake of the camera a hit gives (0x9E278; see "The camera's shake") |
| 0x40 | which hit reaction the player plays (1: the second) |
| 0x44 | reaction strength: she reels if it is at least her poise (actor + 0x708) |
| 0x48 | hit stop, in logic frames |
| 0x4C | cannot be guarded |
| 0x54 | break power against an enemy's lock-point gauge |

The player's normal shot is attack 5 (0x98D5C): 100% of attack, 50% of defence, spread 5.

### Damage

To an enemy (0x92510) and to the player (0x9AD90) the sum is the same:

    power   = attack% x attack / 100 + special% x special attack / 100
    damage  = (power - defence% x defence / 100) x dealt% / 100 x modifiers x spread
    spread  = a random factor from (100 - n)% to (100 + n)%          (0x972D4)

cut to a whole number and not below zero. The modifiers are status effects on either
side, the guard (the player's guard multiplies by tunable 54, 50%; an enemy's by its
own guard percent), and for a broken enemy the attack's weak-point bonus. The
player's attack figures are stats 22 and 24, her defence is stats 2 + 6; an enemy's
are in its parameters.

A hit on the player: every enemy gets signal 101; unless guarded she reels when dead,
when the attack cannot be guarded, or when its strength is at least her poise, with
the attack's hit stop; a guarded hit adds heat tunable 53 and plays effect 505001 and
sound 0x6E. Then the attack's own sound plays (0x9B4FC): field 0x34 of its record,
if not below zero, of the bank field 0x50 names (0 the one always loaded, 1 the
attacker's own). An enemy that is hit sounds the same way (0x92A38, 0x92BA0), but
only for attacks whose sound is of bank 0.

Other sounds of the battle's code, all of the bank that is always loaded: 0x78 her
dodge, 0xA0 each of her shots, 0x6A the cursor locking on, 0x8E an attack's warning
(0x91644), 0x6B an enemy called in, 124 a bomb's first landing; a bullet's own sound
as it leaves and, for a bomb, as it bursts (0x86568: fields 0x28 and 0x90 of its
record, of the bank field 0x34 names); a status effect's sound as it takes (0x8FC80,
see "The sound an effect is given with"); 0x68 as a part of an enemy breaks
(0x85BEC, from 0x860E8 as its gauge is worn to nought) and as an attack whose
effect is of kind 26 comes on a guard that is up, hers or an enemy's, and goes
through it (0x8FA88, asked only where the hit would have been guarded, 0x9AE24
and 0x925D4); 0x63 as a finish's gauge is full (0x946F4), 0x11 with the log's
words (0x96B40), 0x82 as an item is used and as a skill acts of itself
(0x981BC), and the result screen's (0x3821C: the music's volume to 1, the
result jingle on stream 0 at volume 0x60, 0xB for each tick of the count and
0x66 for a level gained). All of these are in the port (`SOUND_BREAK` in
`battle/enemy/actor.rs`, `SOUND_FINISH_FULL`, `log::SOUND_WORDS`, `auto::SOUND`,
`result::SOUND_COUNT` and `SOUND_LEVEL`).

### Enemy groups (`enmgrp2.dat`, magic `EGTB`)

Read by the accessors at 0x93108-0x93CA8. Header: 0x0C group count, 0x10 groups
offset, 0x18 waves offset, 0x1C wave count, 0x20 members offset, 0x24 member count.

- Group, 0x14 bytes: u16 id, u16 flags, u8 mode (the rule its later enemies come in
  by, below), two bytes, u8 wave count (+7), u8 member count (+8), u8 place count
  (+9), u16 first wave (+10), u16 first member (+12), u16 first place (+14).
- Member, 16 bytes: u16 enemy id (0x7F: none), u8 flags, u8 variant, i16 x, y, z, i16,
  u16 size (+12), u16. The members are the enemies that stand there as the battle
  begins. The places are records of the same shape with no enemy: where a later
  enemy can come in. Of a place's flags, 2 says it is in the air; its size is how
  much room it has.
- Wave, one u16: an enemy kept back for later. The enemy id is in the low seven bits,
  then six bits of variant and three of how it enters (as member field 0x238 of the
  actor: 1 drops in, 2 rises).

Group 2 is the first battle of the game: two Eaters (enemy 8, variant 10) at (600, 0,
0) and (300, 0, 0) and two of enemy 19 at (900, 300, 0) and (0, 300, 0).
`tools_re/egtb.py` writes the whole table out, `tools_re/groups_with.py` finds the
groups an enemy is in, and `tools_re/enemy_variants.py` lists every enemy's variants
with their names, AI numbers and levels.

### Enemy archives

The table at 0x169AB8 ({enemy id, name}, 88 entries, searched by 0x9308C) names each
enemy's archive `BTL/CHR/BCHR_<NAME>.VOL`: 1-8 egg00-07, 9-13 cat00-04, 14-19
bee00-05, 20-23 apl00-03, and so on (`tools_re/enemy_names.py`). An archive holds the
script `<name>.bms`, `<name>.edx`, the models and animations the script asks for by
file name, an effect pack, `param1.bin` and `param2.bin`.

- `param1.bin`, one record: enemy id, name, a kind, -1.
- `param2.bin`, one 0xC4-byte record per variant:

| Offset | Meaning |
| --- | --- |
| 0x00 | variant id |
| 0x04 | name |
| 0x08 | which of the script's AIs it uses |
| 0x0C | level shown |
| 0x10 | HP; 0x14 HP of a second life, if any |
| 0x18 / 0x1C | attack / special attack |
| 0x20 | defence |
| 0x24 | the experience beating it is worth |
| 0x28 / 0x2C | chance out of 255 that it leaves an item / the item |
| 0x30 / 0x34 | chance out of 255 that the item left is this other one instead / that item |
| 0x38 | hit-stop setting (actor + 0x7A4) |
| 0x3C, 0x40, 0x44 | lock-point gauge 0: strength, recovery, time it stays broken |
| 0xA0 | percent of damage taken while guarding |
| 0xA4-0xAC, 0xB0-0xB8 | gauges 1 and 2 |

- `.edx` (magic `EDXD`), read with engine function 62 by ids -1000 to -990: u16 at
  0x04, 0x10, 0x12, 0x14, 0x16, nothing, u16 at 0x2A, i16 at 0x2E, 0x30, 0x32, u16 at
  0x36. Ids -993 to -991 are where the HP gauge floats over the model.

## Enemies (`BtEnActor`, 0xA40 bytes, built at 0x91BBC)

The stage (0xA0820) makes one actor per member of the battle's group, at the member's
position. The constructor loads the enemy's tables, copies its figures from the
variant's `param2` record, attaches the script and sends signal 1 (signal 0 for an
enemy that enters as a wave); the script's signal function then loads the model,
defines the collision spheres, the shadow and a lock point.

| Offset | Meaning |
| --- | --- |
| 0x7C | flags: 1 remove me, 2 guarding, bit 30 "down" |
| 0x88 | time step |
| 0x98 | up to 16 slots: models, collision spheres, effects; 0xD8 their count |
| 0x150 | the point its moves aim at, usually the player's position (0x91108) |
| 0x1B0 | the last hit: slot hit, damage, then two words that are always written 0 |
| 0x1C0-0x1D8 | slot, node and offset the HP gauge hangs on |
| 0x228 / 0x230 | enemy id / variant |
| 0x238 | how it enters (member field 0x0C): 1 drops in, 2 rises, else just turns |
| 0x23A | i16: it is worth nothing when beaten. 0 for a group's own members, 1 for every enemy called in later (0xC0C20) |
| 0x23C / 0x240 / 0x244 / 0x248 | attack, special attack, defence, guard percent |
| 0x24C | the AI runs |
| 0x250 | the HP gauge is shown |
| 0x260 | home position |
| 0x270 / 0x274 | dead / dying |
| 0x278 / 0x27C | maximum HP / HP |
| 0x284 | status effects |
| 0x7B0 / 0x7BC | three gauges and their lock points; 0x7C8 which gauge a slot belongs to |
| 0x830 | the point its shots aim at: one of the player's standing places, 75 up |
| 0x840 | the movement block, below |
| 0xA20 | a token the scripts pass between enemies so that they take turns |
| 0xA30 / 0xA34 | attack warning flash: length 30 and time run |

Its update (0x91F4C), unless the battle is paused: hit stop; the gauges; status
effects; background colour; the animations advance; lock points; effects; the
movement block (0xB9B50); signal 2000 if the AI runs and it is alive; the script
(0xA75C0); if it has been marked for removal it goes; else each of its enabled attack
spheres is tried against each enabled body sphere of the player (touching when the
centres are no further apart than the radii together): a hit sends the player the
attack's record and switches the sphere off.

### Collision spheres

A sphere is a slot of its own, parented to a node of a model slot, with flags (2
enabled, 4 attack, 8 body, 0x10 passed along with a hit), a radius and an attack id.
The player has a body sphere of radius 45 in slot 2; a bullet an attack sphere of 70.

The player's bullet (0x8B310), once a frame after it has moved: the segment from the
sphere's last place to its new one is tried against enemy bullets and then every
enabled body sphere of every enemy, as a sphere of both radii together; the nearest
hit wins. A bullet that has reached the ground (y at or below 0) hits nothing. The
enemy gets a hit record with the slot hit, the point, and the attack's record (id 5
for the normal shot), and the bullet goes.

A hit on an enemy (0x92510) takes the damage off its HP, shows the number, wears the
gauge of the part hit down by the attack's break power, records the hit at 0x1B0 and
sends signal 100. Normal shots have no break power.

### The movement block (actor + 0x840)

| Offset | Meaning |
| --- | --- |
| 0x00 | phase of the current move (0: not started) |
| 0x04 | updates since the move began |
| 0x08 | time left of a turn |
| 0x0C | the script's "action" timer |
| 0x10 | flags: 1 moving, 2 hold, 4 the move has ended, 8, 0x10, 0x20 script flags |
| 0x12 / 0x14 | slot being moved / a second slot given the same placement |
| 0x16 | kind of move |
| 0x1A | which of the offsets near home was picked last |
| 0x24 / 0x28 | time run / length, in sixtieths |
| 0x2C | how long a walk has been going, for the rhythm of its strides |
| 0x3C / 0x40 | the bearing a head of its own was last turned to, and is turning to (kind 17) |
| 0x44 | set to 1 by the moves that put the enemy somewhere all at once; nothing below the bosses' code changes it |
| 0x50 / 0x60 | start / target |
| 0x70, 0x80 | two further stops that an approach notes for the moves after it |
| 0xA0 | control point, or velocity |
| 0xB4 | turn per frame; 0xC4 the angle to end on; 0xD4 the angle now |
| 0xE0 | the placement matrix (its last row, 0x110, is the position) |
| 0x160 | a clock that runs by itself, which scripts set and wait on (functions 52 / 53) |
| 0x170 | the position as of the last update |

Engine function 30 (slot, kind, frames) starts a move: it clears the ended flag and
the counters, and unless the same slot is already making the same kind of move it
notes the position as the start. Each frame (0xB9B50) the move's code works out a new
position; then the turn timer runs down, the angle steps towards its target (or snaps
to it once the time is up), and the slot is placed with that angle about the vertical.

"Turn to face" (0xAE910; from, to, frames): the target angle is atan2 of the x and z
differences, taken the short way round, reached in that many logic frames.

The switch at 0xB9D74 picks the move's code by kind. It has cases for 232 kinds in
126 runs (`tools_re/move_kinds_all.py` follows it for every number and lists them
with the function each case calls); any other kind ends at once. Each family of
enemies has kinds of its own, the hundreds telling them apart, and the bosses' start
at 10000. Every kind a script starts is ported, the bosses' with the rest
(`tools_re/bscr_coverage.py` checks that against the scripts).

Nearly all of them carry the enemy along a curve through a control point (start,
control, target: the position `t` of the way along is start x (1-t)² + control x
2t(1-t) + target x t²) over the move's length, which is in sixtieths. With the
control point half way the pace is even; with it two thirds of the way along the
enemy sets off a third faster than even and arrives a third slower; lifted, it is a
hop. "So far short of the player" is on the line along the ground from where the
enemy stands to her: her place is the middle of her body (node 0 of her model, 0x91108),
and for a few moves her right ankle (node 0x17). Unless said otherwise a move leaves
the angle the enemy is turning to alone.

| Kind | Code | What it does |
| --- | --- | --- |
| 10 | 0xBA318 | turn to face the player over `frames` logic frames; ends at once |
| 11 | 0xAEACC | drop in: from 1000-2000 above home, falling at 0.98 x step per frame per frame, to home |
| 12 | 0xBA394 | like 10, measured from where it stands now |
| 13 | 0xAEC28 | rise from 400-1400 below home, gaining 0.196 x step per frame |
| 14 | 0xBA410 | keep turning to the player for `frames` sixtieths |
| 15 | 0xBA474 | wait for `frames` sixtieths |
| 16 | 0xBA4A8 | face a bearing given in degrees in place of a length, at once: 0 is the player's side of the stage, more is further round towards +x |
| 17 | 0xBA568 | turn node 0x33 of its model (a head of its own) to the player's bearing over the move's length, the short way; the body stays |
| 100 / 101 | 0xAF788 | go to home plus one of five offsets (the first, or a random one other than the last): (0,0,0), (0,0,200), (0,0,400), (50,0,0), (-50,0,0); then face the player |
| 102 | 0xAF990 | go to the point 400 short of the player on the line to her, and note the points 200 and 50 short; turn the way it goes in 2 frames |
| 103 / 104 | 0xAF990 / 0xAFCE8 | go on to 200 short / 50 short |
| 105 | 0xAFE80 | hop back to home plus a random offset, the path's control point 1000 high |
| 106 / 107 | 0xB0064 | back away from the aim point by 200 / 500 |
| 108-110 | 0xB0260 | go to 1000 / 750 / 500 short of the player |
| 111 | 0xB055C | rush to 100 short of the player, weaving sideways by 20-40 four times |
| 201 | 0xB2264 | hop to home plus one of nine offsets (0x16A928: x in -50, 0, 50 and z in -100, 0, 100), a random one other than the last, the control point 300 high; it ends on the ground, and turns to the player in 12 frames |
| 202 / 203 | 0xB2468 | approach like 102 / 103, to 600 short, noting 300 and 50; then on to 300 |
| 204 | 0xB27C0 | vault to another of the nine offsets of 201, the control point 800 high |
| 205 | 0xBA920 | be put on the ground where it is (y = 0); ends at once |
| 206 | 0xB29A4 | hop back from the aim point by 250, the control point 100 high, ending on the ground |
| 207 / 208 | 0xB2BAC | leap to 800 / 350 short of the player, the control point 150 high; turn the way it goes |
| 209 | 0xB2BAC | leap on to whatever target the move before left in the block |
| 301 | 0xAF4F4 | drift to home plus one of ten offsets (table at 0x16A628: the nine combinations of x and y in -50, 0, 50, and (0, 0, -250)), a random one other than the last; on its first frame turn to the player in 12 frames |
| 302 | 0xAF4F4 | dart likewise between the offsets at 0x16A6A0 (x in -250, 0, 250 and y in -125, 0, 125, and (0, 0, -100)), the control point two thirds of the way along |
| 303 | 0xAED90 | fall straight down, gaining 0.98 x step per frame, until y is 0 |
| 304 | 0xAEEB8 | rise straight up, gaining the same, to the height of home |
| 400-401, 410-412 | 0xB2E64 | walk to home plus an offset from two rows of three (0x16A994: (150,0,-300), (-150,0,-280), (0,0,0); (300,0,-710), (0,0,-700), (-300,0,-720)), the kind's tens the row and its units the place; it ends on the ground and turns to the player all the way |
| 413 / 414 | 0xB2E64 | walk to 450 short of the player's ankle / home, facing where it goes and her once there |
| 415 | 0xB31C4 | leap home, the control point 500 high, facing back the way it came |
| 416 | 0xAEFE0 | hop on the spot: y = 200 x sin(1.5 degrees x the time run), for 120 sixtieths whatever length was given. Its turn is from where it is to where it is, which comes out as angle 0: the far side of the stage |
| 501-503 | 0xB3368 | go to 1000 / 650 / 350 short of the player, ending on the ground |
| 504 | 0xB366C | step to home plus one of five offsets (0x16A9DC: (0,0,0), (50,0,80), (-50,0,80), (50,0,-80), (-50,0,-80)), along the ground |
| 601 | 0xBAC3C | turn to the player in 12 frames; ends at once |
| 602 | 0xB3860 | go to 200 short of the player |
| 603 | 0xB3AB8 | the same along a path whose control point is pulled 1500 along x, facing the way it goes |
| 604 | 0xB3D28 | go on to the first stop an approach noted |
| 605 / 606 | 0xB3EC0 | home round a wide arc: to home plus (400, 0, 100) / (-400, 0, 100), the control point pulled (3000, 0, 500) / (-3000, 0, 500) from half way. For the first 0.8 of its frames it faces the way it goes; then it turns to the aim point in 12 frames |
| 607 / 608 | 0xB3EC0 | straight home, turning to the aim point |
| 609 / 610 | 0xB41CC | off the stage: to x = 2500 / -2500 at its own height and depth, the control point a quarter of the way |
| 611-614 | 0xB41CC | be put off the stage at x = 2500 (611, 613) or -2500, level with the point 500 (611, 612) or 100 from the player on the line from her to its home, and stay there for the move's length |
| 615 / 616 | 0xB41CC | go to the point 500 from the player on that line, moved 300 / -300 along x |
| 617 | 0xB41CC | go to the point 300 from her on it, the control point two thirds of the way along |
| 618 / 619 | 0xB41CC | as 615 / 616, facing the player's side of the stage throughout |
| 620 / 621 | 0xB41CC | come in from x = 2500 / -2500, at home's height and depth, to home, the control point two thirds of the way along |
| 800 | 0xB15EC | float like 1400 between offsets closer in (0x16A790: the last two are (0,0,-150) and (0,0,150)) |
| 801-804 | 0xB1944 | fly in to home from home plus (2000,1200,500), (1800,500,500), (-1800,600,500) or (-2000,1000,500), the control point the half-way point with its x, y and z multiplied by 0.5, 3.5 and 4; faces home |
| 805-820 | 0xB1B3C | swerve to home plus an offset from four formations of four (0x16A844); the control point is half way, pulled 400 to one side (the other for the last two of a formation), up 300 or down 100 at random, and to 300 on the player's side of home; faces the way it goes |
| 821 | 0xB1DE8 | chase: each frame the path is laid afresh from where it is to 150 short of the player, 70 up, and it takes the share of it that the frame is of the time left; faces her |
| 822-824 | 0xB2088 | dart to home plus (100,100,-50), (-80,-100,20) or (40,1000,400) (0x16A904), the control point two thirds of the way along; turn to where it goes in 12 frames |
| 900-902, 910-912 | 0xB49F0 | stay put for the move's length and then be, all at once, at home plus an offset from two rows of three (0x16AA18: (0,0,-200), (600,0,-180), (-600,0,-190); (0,0,-500), (400,0,-510), (-400,0,-520)), facing the player. The script hides the model meanwhile |
| 913 / 914 | 0xB49F0 | the same to 750 short of the player's ankle / to home |
| 1001 / 1002 | 0xB4C1C | stride towards the player, to 575 / 800 short: laid afresh each frame like 821, but moving, and counting its time, only while a foot is off the ground. A stride is 126 sixtieths and both feet are down from 8 to 50 and from 70 to 112 of it (ten strides are provided for); faces where it goes, and her once done |
| 1003 | 0xB4FE0 | back out to 900 from where the player was as it set off, with the same strides, facing the aim point |
| 1101-1103 | 0xB5310 | glide to 1000 / 650 / 350 short of the player along a line that slopes with its own height, facing the player's side of the stage |
| 1104 / 1105 | 0xB560C / 0xB57C8 | straight up / down by 300, the control point half as far again, so it goes past and comes back; facing the player's side |
| 1200-1204 | 0xB5984 | go to home plus one of five places across the stage (0x16AA60: (600,0,-90), (300,0,-50), (0,0,0), (-300,0,-50), (-600,0,-90)), the control point two thirds of the way along, ending on the ground; turns to the player all the way |
| 1205-1209 | 0xB5984 | the same with a hop: the control point 80 high |
| 1300-1301, 1310-1312 | 0xB5B9C | go to home plus an offset from two rows of three (0x16AA9C: (300,0,-300), (-300,0,-280), (0,0,0); (600,0,-710), (0,0,-700), (-600,0,-720)), the control point two thirds of the way along, ending on the ground and then facing the player |
| 1320-1321, 1330-1332 | 0xB5B9C | the same with a hop of 80 |
| 1333 / 1334 | 0xB5B9C | hop to 600 short of the player's ankle / hop home facing the way it came |
| 1400 | 0xB5F2C | float to home plus one of eleven offsets (0x16AAE4: x in -150, 0, 150 with y in -50, 0, 50, then (0,0,-300) and (0,0,300)); on arriving it draws the numbers for another leg, which the next move throws away; turn to the player in 12 frames |
| 1500 | 0xB07C8 | as 100, by a table of its own that says the same (0x16A754) |
| 1501-1503 | 0xB09D0 / 0xB0D28 | approach like 102-104: to 400 short, noting 300 and 250; on to 300; on to 250 |
| 1504 | 0xB0EC0 | bound to 350 short of the player, the control point 800 high; turn the way it goes |
| 1505 | 0xB110C | as 105 |
| 1506-1508 | 0xB12F0 | as 108-110 |
| 10000, 10001-10003 | 0xB6284 | Mefe: a leap to home plus the first of three places (0x16AB68: (0,0,-200), (300,0,-180), (-300,0,-190)), or the one the kind names; the control point 0.95 of the way across the ground and 250 high, and what of the height is over 200 counts for a fifth (`y / 5 + 200`). On its first frame turn to the player in 12 frames (10000 from where it last stood) |
| 10004, 10005-10007 | 0xB64F4 | the same to the next three places of that table ((0,0,0), (300,0,0), (-300,0,0)), the control point half way |
| 10008 | 0xB6750 | up to the player: to 600 short of her on the line to her, moved 150 along the X of the frame that faces from her to the enemy; it notes the points 140 and 125 short (block + 0x70, 0x80) and the one 120 short moved -100 along that X (+ 0x90); turn to her in 2 frames |
| 10010 | 0xB6C14 | on to the second of those, the control point pulled 100 along -z |
| 10011 | 0xB6DD0 | on to the third |
| 10012 | 0xB6F80 | (no script starts it) |
| 10013 | 0xBB78C | be put on the ground where it is; ends at once |
| 10100-10103 | 0xB7378 | Mazuma: stay put for the move's length and then be, all at once, at home plus one of four places (0x16ABB0: (100,0,-240), (400,0,-415), (0,0,-415), (-500,0,-225)). One of her that was called into the battle (her + 0x23A) is 90 to one side of the place or the other, by chance. On the first frame turn to the player from that place in 1 frame, as all the kinds from here on do unless said otherwise |
| 10104 / 10105 | 0xB7378 | the same to 250 short of the player's ankle on the line to it / to home |
| 10106 | 0xB75FC | a leap home, the control point 200 high |
| 10107-10110 | 0xB77D4 | fly in from (-1500, 400, 0), (1500, 200, 0), (-1500, 50, 0) or (1500, 300, 0) to up to 200 either way across from where it was and up to 100 up or down from that height, at z = 0; the control point two thirds of the way along; facing the player every frame |
| 10202-10210 | 0xB7AB0 | the third boss: to home plus one of nine places (0x16ABE0: (560,0,715), (20,0,715), (-560,0,715), (495,0,200), (40,0,205), (-360,0,210), (375,0,-300), (45,0,-310), (-370,0,-305)); turn to the player in 12 frames |
| 10211 / 10212 | 0xB7C84 | to 250 / 100 short of the player's ankle; the first turns to her in 4 frames, the second not at all |
| 10213-10221 | 0xB7AB0 | to the nine places again, turned back the way it came (from the place to where it set off) in 4 frames |
| 10222 / 10223 | 0xB7F14 | a pass across the stage before her: from x = 1500 to -1500 (or back), at z = -1300 and from the ground up to the height it had; the angle is held at nought (or half way round) |
| 10300-10304 | 0xB81DC | the fourth boss: at once, as 10100, at home plus one of five places (0x16AC4C: (595,0,-240), (360,0,-315), (10,0,-300), (60,0,-315), (-180,0,-225)) |
| 10305 / 10306 / 10307 | 0xB81DC | at once at 200 short of the player (her body) / at home / 1500 behind home |
| 10308-10310 | 0xB8430 | a glide to home plus (500,0,10), (0,0,0) or (-500,0,10) (0x16AC88), the control point two thirds of the way along and on the ground; it ends on the ground |
| 10400 / 10403 | 0xB863C / 0xB89D8 | the fifth boss: to its place 100 over home (the first of the table at 0x16ACAC; the first would pick among five if it were ever asked to); turn to the player in 12 frames |
| 10402 | 0xB882C | straight down to the ground where it is |
| 10500-10502 | 0xB8B80 | the sixth boss: to home, or 600 to either side of it (0x16ACE8), by a path whose control point is past the end (half the way plus two thirds of it), so it goes past and comes back; turn to the player in 12 frames |
| 10503 | 0xB8D9C | after the player: each frame the way is laid afresh from where it is to 700 short of her, and it is put as far along it as the move's time has run; turn to her in 12 frames |
| 10504 | 0xB8D9C | straight home, turned back the way it came in 12 frames |
| 10505 / 10506 | 0xB9110 | at once at 700 short of the player's ankle / at home; it turns to her once it is there |
| 10600-10604 | 0xB92CC | the seventh boss: at once at home plus one of five places (0x16AD0C: (395,0,-240), (60,0,-415), (5,0,-300), (60,0,-415), (-380,0,-225)) |
| 10605 / 10606 | 0xB92CC | at once at 100 short of the player's ankle, 10 under home's height / at home |
| 10607-10609 | 0xAF2B8 | at once at home plus (500,0,10), (0,0,0) or (-500,0,10) (0x16AD48) |
| 10700-10702 | 0xB94F0 | the last boss: at once at home plus (160,0,-415), (0,0,-300) or (160,0,-415) (0x16AD6C), at a height of 0, 10, 20, 30 or 40 |
| 10703 / 10704 | 0xB94F0 | at once at 400 short of the player's ankle / at home |
| 10705-10707 | 0xB9738 | a sweep to home plus (400,0,10), (0,0,0) or (-400,0,10) (0x16AD90) by a path that goes past its end, as 10500; facing the player every frame |
| 10708-10710 | 0xAF3B4 | at once at those three places |
| 10711 | 0xB9960 | after the player like 10503, to the point 400 along +z from her ankle; facing her every frame |
| 10712 | 0xBC03C | away: its height is 5000; ends at once |

The offset tables of 301 and 302 are followed by the one of kinds 100/101 at 0x16A718,
and every table shares the "picked last" byte.

Which family's scripts start which, besides the turns and waits of 10-15 that all of
them use (`tools_re/bscr_coverage.py` works it out from the scripts and lists it at
the end of its report):

| Scripts | Kinds |
| --- | --- |
| `egg` (the Eaters) | 101-105, 108-111 |
| `bee` (the Hornets) | 301-304 |
| `bat` (the Generator) | 16, 17 |
| `cat` | 202-209 |
| `apl` (the Spider) | 400-401, 410-416 |
| `had` | 501-504 |
| `tir` | 602, 606-621 |
| `nafc` | 302, 801-824 |
| `cof` | 900-902, 910-914 |
| `yub` | 1001-1003 |
| `shi` | 16, 1101-1105 |
| `hul` | 1200-1209 |
| `hua` | 1300-1301, 1310-1312, 1320-1321, 1330-1334 |
| `cub` | 1400 |
| `egb` | 1501-1508 |
| `twi` | none of its own |
| the bosses: `mee`, `maz`, `cyzkal`, `rir`, `naf`, `zah`, `wrs` | 10000-10013, 10100-10110, 10211-10223, 10300-10310, 10400-10403, 10500-10506, 10600-10712 |

Kinds 100, 106, 107, 201, 601, 603-605, 800 and 1500 have code that no script starts,
as do some of the bosses' (10012, 10014, 10200-10210, 10213-10221).

### Engine functions an enemy script uses (table 2)

| No. | Arguments | Meaning |
| --- | --- | --- |
| 1, 3 | string | debug print |
| 4 | bank, id, volume, note | play a sound (0xBC3CC); returns the number to stop it by, or -1. Bank 0 is the one that is always loaded (`fld_pack.bin`), 1 the enemy's own (`btl_*.phd` in its archive), 2 its second (`fin_*.phd`, for finishing moves). The note is 64 for the sound as recorded: 221 of 237 calls; the rest play it lower or higher |
| 5 | handle | stop a sound |
| 6, 13 | n | random number below n |
| 7 | slot, name, loop, speed %, blend frames, force | play an animation unless it is already playing (or `force`); the speed applies to the slot |
| 8 | slot, on | loop over the animation's own range, or stop looping and play out |
| 9 | frames | wait |
| 10 | slot | wait for the animation's end |
| 11 | sphere or -1, on | enable or disable one sphere or all |
| 12 | mode | set up the movement block: facing the player, at home |
| 14 | slot, a, b | loop between two frames (given in logic frames) |
| 15 | 6-12 | the player's standing place number; her two places' x, y, z |
| 17 | | still moving |
| 18 | mode, slot, node, layer, effect | play an effect at (mode 0) or fixed to (mode 1) a node; modes 10+ on the player |
| 19 | what, a, b, c | 0 set / 1 read / 2 run down the action timer; 3 set hit-stop setting; 4 choose the aim point (0x91308) and face it in 12 frames; 5 choose only; 6 number of bullets of bullet id a; 7 fire them; 8 / 9 aim the moves at a standing place plus an offset |
| 22 | slot, name | load a model |
| 23 | sphere, parent slot, node, x, y, z, radius, kind | define a sphere (kind 1 attack, 2 body); its attack id starts at 1 |
| 24 | slot, parent slot, node, effect | an effect kept in a slot of its own, fixed to a node of another (0xBCC74) |
| 25 | stop | cancel the running update (and with `stop`, end the move) |
| 26 | what | read: 0 HP, 1 maximum HP, 2 slot last hit, 3 last damage, 4 and 12 always 0, 5 down, 7 gauge 0 whole, 10 variant, 11 AI number, 13 in hit stop, 15 entry kind, 16 time broken, 54/100/101 movement flags 8/0x10/0x20 |
| 27 | what, a, b, c | write: 5 down flag, 14 may be interrupted, 50-52 HP gauge anchor and offset, 53 a sphere's attack id, 54/100/101 movement flags, 55 dead, 56 dying |
| 28 | | remove the actor |
| 29 | slot | hide a slot |
| 30 | slot, kind, frames | start a move |
| 31 | | wait for the move's end |
| 33 | hold | hold the move where it is (movement flag 2), or let it go on |
| 36 | slot, frame | jump to a frame and play out |
| 42 | slot, list | what is in a slot is put in one of the scene's draw lists (0xBCCEC, 0xACDD0: its + 0x6C and the list at + 0x70; lists 0 to 2 are drawn as they come, flag 8, the others by how far each thing is, flag 0x10). The enemies' scripts only ever name the last, 4, for the effect just kept in slot 2 with function 24 (the cats, the hulks and the first boss: 27 calls); a map's script puts its models in lists 0 and 2. In the port the effect is then of the last list (`actor.rs`, `Body::kept`) |
| 50 | on | guarding (actor flag 0x7C bit 1) |
| 51 | slot, node, radius, strength | a node of the slot's model throws a round shadow (0xBDA28; "Light and shadow in a battle"): the node is put in draw list 3 and given the radius and the strength, of 255 |
| 52 / 53 | frames / | set the clock of the movement block (+0x160), given in logic frames and kept in sixtieths; read it (0xBDAB8, 0xBDAF4) |
| 54, 57 | | nothing |
| 56 | kind, a, b | camera shake |
| 62 | id | a figure from the `.edx` |
| 63 / 64 | effect | stop an effect |
| 65 | which, bits, value | what the enemies and the stage keep between them (0xBDE60): which 0 sets or clears bits of a word of flags, which 1 sets the number of towers standing |
| 66 | which, bits | read the same (0xBDEEC): whether any of the bits is set; the number of towers |
| 67 | what, a, b, c | 6: set this enemy's token to a, or with b = 1 every other enemy's; 8: give status effect b to the player (a = 0) or itself (1), or with c = 0 take it away; 31: choose the aim point |
| 68 | what, a, b, c | read: 1 own HP %, 2 the player's HP %, 4 distance to her, 5 enemies alive, 6 own token, 7 dead, 20-24 the player is shooting / using a skill / dodging / guarding / overheated, 25 her heat %, 26 / 27 a status effect is on the player (a = 0) or itself (1), by its id / by its kind (b), 28 / 29 gauge broken, 30 lowest HP % of the others |
| 69 | stream, name | a recording of the enemy's archive, by its file's name, on one of the two streams; what the stream was playing stops at once (0xA7EAC, 0x9CCAC). Her own lines are played the same way, from `pack.vol`, on stream 0 |
| 70 | frames | wait until that clock has reached the time (0xBDB9C) |
| 73 | first place, last place, keep | call one of the group's reserves into the battle (0xC06E8; see "Enemies that come in later") |
| 74 | slot, on | show or hide what is in a slot (0xBE0CC: flag 0x40 of its model, 0xACF80) |
| 80 | n | battle log line with the enemy's name |
| 81 | | attack warning: sound 0x8E and a flash |
| 82 | mode, slot, node, x, y, z, layer, effect | like 18, with the effect moved off its node (0xBD320) |
| 120 | effect, give, itself | give every other enemy a status effect, or end it (0xBF020) |
| 101 | on | show the HP gauge |
| 103 | n | n scaled by the status effects that change the pace of its waits: n x (100 + kind 17 - kind 13) / 100 (0xBEAEC) |
| 104 | cond, a, b, c | an AI condition: 0 always; 1 / 2 own HP % at most / above a; 3 / 4 the player's; 6 / 7 the effect with id b is / is not on the player (a = 0) or itself (1), 8 / 9 likewise an effect of kind b; 10 another enemy's HP % at most a; 22-24 other enemies alive equal to / at most / more than a; 31 / 32 distance at most / above a; 81 token equals a; 100-104 what the player is doing; 105 / 106 her heat above / at most a; 107-113 gauges |
| 105 | cond, a, ..., eight variables | compare a with one of the script's own variables: 33-48 equal, 49-64 variable at most a, 65-80 variable at least a |
| 109 | a, b | fade the background colour |
| 114 | gauge, slot, node, x, y, z | make a lock point with its gauge |
| 115 | gauge, on | lock point on or off |
| 116 | slot, gauge | which gauge a sphere wears down |
| 123 | | every effect of its own is no more, at once (0xBF1EC: 0xA77A4 unloads the effects kept in its slots, 0x9E7E0 takes every one of its own pool away, each by 0x9E62C). All 88 scripts call it, 197 times: as the enemy is beaten, before it falls. In the port: `Event::StopEffects`, `Horde::effects_dropped` |

The bosses' scripts use forty-two more (`tools_re/bscr_coverage.py` lists which
script needs which; their code is in `tools_re/out/boss_natives.txt` as dumped by
`dump_natives.py`). "The scene's frame" is the origin of the world, facing π.

| No. | Code | Arguments | Meaning |
| --- | --- | --- | --- |
| 34 | 0xBCFEC | camera file, slot, node | a scene begins: the movement block's placement and its two angles are put by (block + 0x190, 0x1D0, 0x1D4), the enemy's slot and her model are put in the scene's frame, her animation's blends are dropped (0xAA9A0), its HP gauge goes (+ 0x250) and the camera move plays in the frame of the slot's node (0xA7DA0) |
| 35 | 0xBD284 | | the scene ends: the placement and the angles come back, she is put at her standing place again (0x99950, which also clears her + 0x7FC), the HP gauge is back and the camera move ends (0xA7E58) |
| 119 | 0xBD138 | camera file, slot, -, node | as 34, the camera in the frame of another slot's node |
| 45 | 0xBD7F8 | slot of hers, animation, loop, speed % | an animation of her model, by its file's name in what is loaded, with no blend; looped or played out |
| 55 | 0xBEC68 | slot or -1, x, y, z | the slot is moved by so much in its own frame (its matrix times a move, 0x23F6C); -1 for her model |
| 111 | 0xBEB78 | slot or -1, degrees | the slot, or her model, at the origin and facing that way; the block's two angles are set to it |
| 71 / 72 | 0xBE024 / 0xBE0A8 | which, near, far | how near and how far the cameras see (0xA7CAC: 0 the first draw list's, 1 the others'); 72 stops their being set (0xA7CE0) |
| 75 | 0xBE194 | - | it takes the stage: root flag 10, the display taken away if it was there (0x35BF4, root + 0x194), every other actor of the list held and out of sight (+ 0x7C flags 0x10 and 0x20), and what is behind dimmed to 196 over 20 sixtieths |
| 76 | 0xBE26C | | it gives the stage back: the cut-in dropped (as 113), she is seen (as 100 with 1), the others are let go, the display is back, root flag 10 cleared |
| 96 | 0xBE9DC | file | the cut-in's layout, by name (`ifBtlCutinTask`, 0x331B8) |
| 77 / 78 | 0xBE31C / 0xBE354 | set | a set of the cut-in's layout plays (0x33204; with set 1 sound 0x70 of the battle's bank) / is no longer played or drawn |
| 79 | 0xBE3F8 | set | wait until that set has played through (0x3328C) |
| 113 | 0xBEA14 | | the cut-in is dropped |
| 83 | 0xC0D78 | - | the finish begins (`BtFinish`, below), with the record that goes with the enemy's number; the column of her skills is taken away (0x9EEAC) and the camera move's animation starts over (0xA7E30) |
| 84 | 0xBE5AC | | wait for the finish to be over |
| 89 | 0xBE5D0 | | answers 1 |
| 85, 91, 92 | | | nothing |
| 86 | 0xBE5D8 | kind, archive | have an archive loaded (0x95F78), unless it is |
| 87 | 0xBE684 | archive | wait until it is |
| 88 | 0xBE6DC | -, archive | let it go |
| 90 | 0xBFFB8 | mode, slot, node, x, y, z, rx, ry, rz, layer, effect | as 82, the effect turned as well: about Y, then X, then Z, in degrees |
| 93 | 0xBE71C | what, value | 0: lets the encounter go (0x90800, what `BtEnter` does for other groups); 1: a sheet of colour `value` over the whole screen (the object at 0x171318, drawn at 0xA249C in draw list 6); 2: the sheet goes over `value` logic frames, `alpha = trunc((1 - t / T) x 255)` (0xA2268) |
| 94 | 0xBE824 | | wait for the encounter to be over, telling it to go each frame |
| 97 | 0xBEA44 | | the fight begins (0x93FA0): her skills that act of themselves, the pause, root flag 8 |
| 98 / 99 | 0xBEA90 / 0xBEAAC | - | the display comes (root flag 11) / goes |
| 100 | 0xBE118 | shown | her model is drawn, or not (her + 0x738) |
| 102 | 0xBEAC8 | | its second life: once, if its variant has HP for one, its HP and maximum become that and its gauge is whole (0x911A8) |
| 106 | 0xBDFF0 | stream | wait until the stream (0 or 1) is silent |
| 107 | 0xBEB38 | | every other enemy goes (0x917CC) |
| 108 | 0xBEB5C | think | every enemy that is dying is made again from the record it was made from (0xA0E50: its own first signal, then signal 2), its AI on, with signal 4 if `think`, and the old one goes (0x91830). The fifth boss has its four helpers back this way |
| 110 | 0xBCA8C | slot, name, parent slot, node | a model in a slot (0xA7830) that is fixed to a node of another slot's, or to that model whole with -1 (0x97CB8). The last boss has a thing of its own for one of its moves so |
| 112 | 0xBED60 | | both streams stop, every effect of the battle ends (0x9EA50) and every bullet goes (0x87AE8) |
| 117 | 0xBED9C | a, b | what her model shows, twice (0x9B608: the part modes) |
| 121 | 0xBEDF8 | a, b | what its own model shows, twice (0x92CB0, for the two bosses of her build; by the table at 0x1694C0): modes 0 to 2 a shape of the right hand (parts 0x3C, 0x3D, 0x48) with the cannon that is of the model itself (0x32) hidden, 5 the first of them with that cannon shown, 3 and 4 the left hand (0x46, 0x47) without the blade (0x33) and 6 with it, 7 and 8 hide and show part 0x37; a mode below nought is passed over |
| 118 | 0xBEE54 | who, pace, span, on | a mouth that moves (0xBC154, each frame while `on`): node 6 of her model (who 0) or of its own first slot is turned about Z by `sin(-n°) / 6`, where `n` grows each frame by 4 and a random number below `pace`, twice or three times it (one chance in four each; two in four for once), and wraps at 20 x `span`; with no span the mouth is shut |
| 122 | 0xBF148 | | back to where it belongs: angles nought, the placement plain at home, both slots placed; every bullet goes |
| 125 | 0xBF2D0 | time | the music stops, over that time if it is more than nought (0xA10C) |

What 68 answers for 1000, and so condition 1000 of 104 (`sense(1000) == a`), is
the word at actor + 0xA24, which nothing ever writes: 0. The bosses' AI asks it
before each of its own choices ("has somebody said which attack"), and nobody has.

### The Eater's script (`egg07.bms`)

Global 0 is its state, global 1 turns 9999 when the state has run its course, global
41 the AI number. Signal 1 (or 0) builds it: model `chr_m_egg01.mdl`, body sphere
(slot 1, node 4, radius 60), attack sphere (slot 3, node 50, 100 ahead, radius 180),
shadow, lock point, movement block, then state 1 (turn to the player) or 2 (enter).
Signal 100 is "I was hit": dead -> state 20, the death (animation `btl5010`, shakes,
effects, removal). Signal 2000 runs the AI: generated state machines ("pages") that
test conditions with functions 104/105 and pick the next state. The update runs the
state: 4 wait, 1001 advance, 1002/1003 bite once/twice (approach in three moves, the
attack sphere switched on with attack id 100/101, hop back), 1004-1007 rushes, 1008
self-destruct, 1009 poison shots, 1010 a status skill, 1011 return home.

The Eaters of the first battle are variant 10, whose AI (number 1) only waits while
the player's HP is whole and bites when another enemy hands it the token; variant 0
(AI 2, as in group 100) advances and bites of its own accord.

### The Hornet's script (`bee05.bms`)

Built like the Eater's. Its states: 4 wait (animation `btl0000`, move 15); 1001 /
1002 move (kind 301 for 40 sixtieths / 302 for 16, with the lock point off while it
flies); 1003 / 1004 a volley (turn to the player with move 14, animation `btl3000`,
after 5 frames choose the aim, then bullet 300 / 301 as many times as its record's
count, six frames apart); 1005 / 1006 a single stronger shot (bullet 310 / 311,
announced by effect 999992 and sound 140 and a wait of 24 frames); 1007 a beam (bullet
320), 1008 a formation shot (330) and 1009 funnels (340), all of bullet class 3; 1010
a status skill. Shot down (signal 100 with no HP left) it goes to state 20: spheres
off, animation `btl5010`, move 303 to the ground, a wait, the log line, two
explosions (effect 509001, sounds 150 and 152, shakes), removal and three drops.
Knocked down but alive (state 20, then 21) it lies for the time its gauge stays
broken, then rises with move 304.

## Enemy bullets (`BtBullet`, built at 0x862E4)

Engine function 19 fires them. With `what` 6 it answers how many bullets a volley of
bullet `a` is (0x91458: the record's count); with 7 it fires one (0x914D4) at the
actor's shot aim (actor + 0x830), and then, while the record names a next bullet,
that one too, from the same aim. The script does the counting and the waiting
between the bullets of a volley.

### The bullet table (`btl_bullet_tbl.bin`, 0xC4-byte records)

Found like an attack (0x1292D0): by (bullet id, enemy id, variant id), then any
variant, then enemy 0.

| Offset | Meaning |
| --- | --- |
| 0x00 / 0x04 / 0x08 | id / enemy id / variant |
| 0x0C | node of the shooter's first model it leaves from, by id (-1: the model itself) |
| 0x10-0x18 | offset from that node |
| 0x1C / 0x20 | yaw and pitch from that node's frame, in degrees |
| 0x24 | effect at the muzzle |
| 0x28 / 0x34 | sound, and its bank: 0 the battle's, 1 the shooter's own (0x86568) |
| 0x2C | attack id of a hit |
| 0x30 | bullets in a volley |
| 0x38 | class: 0 ray, 1 straight, 2 guided, 3 beam, 4 bomb, 5 another (below) |
| 0x3C | the effect that is its look |
| 0x40 | delay before it leaves, in sixtieths |
| 0x44 | speed, per sixtieth |
| 0x48 / 0x4C | pitch and yaw a guided bullet leaves at, on top of its frame |
| 0x50 | homing: percent of the turn taken per sixtieth, kept within 0-100 |
| 0x54 | time the homing follows the player (negative: 10000) |
| 0x58 | distance from its start within which it does (negative: 10000) |
| 0x5C | distance to its target under which it stops following |
| 0x60 / 0x64-0x70 | trail length and colour (guided bullets only) |
| 0x78 / 0x7C | classes 3 and 4: how long the beam, or the bomb's burst, strikes for, and the time between strikes, in sixtieths |
| 0x80 | class 4: how high above the ground the bomb's first hop takes it |
| 0x84 | class 4: how long it lies before it bursts, in sixtieths |
| 0x88 | class 4: how far the burst reaches |
| 0x8C / 0x90 | class 4: the burst's effect and sound |
| 0x94 / 0x98 | class 5: the sixtieths before the strike lands, and how far it reaches |
| 0x9C | another bullet fired along with it |
| 0xA0 | pierces: it goes on after a hit, hitting each enemy once |
| 0xA4 | hit points: above zero it can be shot down and gets a lock point |
| 0xA8 | radius of its sphere |
| 0xAC | bullet it bursts into when its life ends (only with hit points) |
| 0xB0 | scale of its effect, in percent |
| 0xB8 | 1: it starts at the shooter's node; otherwise the offset is from the stage's origin |
| 0xBC | effect of the burst (default 999002) |
| 0xC0 | life in sixtieths; 0 for as long as 8000 units take at its speed |

The Hornet's plain volley (300/301): node 50, class 1, five bullets, speed 100,
radius 50, attack 300, effects 998004 and 998012, sound 161. Attack 300 deals 20% of
(attack - half the defence): 3 HP from a level 1 Hornet, with no hit reaction.

### Classes 1 and 2 (built at 0x883B4, 0x370 bytes, update 0x886F8)

A bullet is an actor of its own. It keeps its record (+0x240), its owner's id
(+0x1E4, looked up again each frame with 0x86224) and the owner's kind (1 the
player's, 2 an enemy's: 0x8654C).

- Start frame (+0x250): with record field 0xB8 set, the world matrix of the owner's
  node, its axes normalised; otherwise identity. Then moved by the offset and turned
  by the yaw and the pitch, in that order, each in the frame so far.
- The frame it is made on it only appears: actors join the end of the list being run
  (0xAE53C), so its first update comes that same frame, and that update plays the
  muzzle effect and the sound, sets position, speed and life, and the velocity: class
  1 straight at the target point (+0x290, the shooter's shot aim), class 2 along the Z
  of its start frame turned by the launch yaw and pitch, with homing on (+0x348).
- Each later frame: its age (+0x2F4) grows by the step. While the age is within the
  delay it stays where it is, and a class 1 bullet re-aims at the player's present
  standing place, 75 up. After that the life (+0x360) runs down and the bullet goes
  when it is spent. While homing, an enemy's bullet moves its target to the player's
  standing place (75 up) until the homing time is over, it is further from its start
  than field 0x58, or nearer its target than field 0x5C. A class 2 bullet that has not
  "passed" steers: velocity = lerp(velocity, direction to target x speed, homing x
  step, at most 1). Then position += velocity x step, and outside x -2000..2000, y
  -500..1000, z -1600..1000 it is gone. Once its z is within its speed of the
  target's it has passed (+0x344) and the homing stops.
- An enemy's bullet can only hit while its owner still exists and once its own z is
  below -1400: each enabled body sphere of the player is tried against the stretch
  flown this frame, with the two radii together (0x897A4). Within reach of either end
  counts, or of the nearest point of the line when that lies between the ends. A hit
  sends 0x9AD90 the attack record for (field 0x2C, owner's enemy id, variant) and
  the owner, and the bullet goes.
- The player's own bullets of these classes (her skills', 0x86E5C) differ in
  these ways:
  - While homing (a class 2 bullet only), its target is the place of the lock
    point the cursor was on as it was made (bullet + 0x2A8, looked up each frame
    with 0xA159C; its place 0xA12C8), and the homing ends when that is gone or its
    enemy has no HP left. The limits of time and distance are not looked at.
  - A class 2 bullet that has not passed steers with a share of its own (+ 0x35C)
    and not the record's: with `d` the distance between the directions it goes
    and it should go in (each of length 1), the share grows by 0.1 a frame, up to
    1, while `d` is 0.5 or less, and while `d` is 0.1 or less its speed grows by
    three times the step. Once the speed is over 200 the code stores 200.0, not
    in the speed but in the share (0x88F8C), so from then on it turns all the way
    at once. `velocity = lerp(velocity, direction x speed, min(share x step, 1))`.
  - A class 1 bullet is not kept to the stage's box; a class 2 one is.
  - It has passed, and homes no more, when its lock point's enemy has no HP left.
  - It can hit from its first frame of flight, anywhere: every enemy it has not
    hit yet (0x86820), each of the enemy's models' collision spheres that is on
    (flags 8 and 2), by the swept test of her normal shot (0x8949C: the same as
    0x8B4A0), and of all it meets the one whose point of contact is nearest to
    where it was. That enemy is hit (0x92510) with her attack of the record's
    field 0x2C (0x129014), and the hit carries the bullet's + 0x1EC. If the
    record's field 0xA0 is set the enemy goes on the bullet's list (0x867CC) and
    the bullet goes on; otherwise the bullet is gone.

  Both are ported (`projectile::SkillShot`, hit in `enemy::settle`, with what has
  become of the lock point worked out there each frame as `projectile::Lock`).

### Class 0: a ray (built at 0x86854, 0x260 bytes, update 0x87F8C)

It lives for one frame, the one it is made on. Its start frame (+0x1F0, by 0x86614)
is the world matrix of the owner's node copied as it stands (no normalising, and
whatever record field 0xB8 says), moved by the offset and turned; without the node,
identity. The ray (0x86960) runs from that frame's position (+0x220) towards the
target point (+0x250), 3000 long.

- An enemy's ray is tried against the player's body spheres (0x869DC): a sphere the
  ray starts inside of is struck at the start; otherwise the sphere's middle has to be
  ahead of the start, no more than 3000 plus its radius along the ray, and within its
  radius of the line, and the point where the ray enters it no further than 3000. A
  hit goes to 0x9AD90 with the owner's attack record, at that point moved by up to 50
  either way in x and y.
- The player's own ray (0x88008; her skills make them with 0x86C2C) is tried with
  0x87B5C against every enemy's body spheres that are in use (flags 8 and 2), by
  the same test, and then against every bullet that can be shot down (the list at
  0x171248: those whose record has hit points, with the bullet's own place and
  radius). Of all it meets, the one whose point of meeting is nearest the ray's
  start is taken. An enemy is hit (0x92510) with her attack of the record's field
  0x2C and with the skill's record (the ray's + 0x1EC), at that point moved by up
  to 100 either way in x and y; a bullet is taken away (0xA28A4). A miss shows
  effect 998021 at the ray's far end, moved by up to 100 either way in x, y and z.
  The only bullets with hit points are 2051 to 2056 and 2550, which no enemy that
  is ported fires, so the port has her rays meet enemies only.

A ray does nothing while it is held (0x863D0): one made while somebody has the
stage would wait. As it acts it plays its muzzle effect and its sound (0x86728),
on that one frame of its life.

### Class 3: a beam (built at 0x8709C, 0x310 bytes, update 0x89C00)

A ray that stays. On its first frame it takes the owner's node matrix as it stands
(+0x260, moved and turned like a ray's), plays the muzzle effect and the sound, and
makes its own effect (record field 0x3C) laid along a look-at frame (+0x2B0) from its
start (+0x290) to the target point (+0x250). Neither end moves afterwards.

Its clocks: age (+0x2F4), time since the last strike (+0x2FC) and the wait until the
next (+0x2F8, 4 to begin with). Each later frame both clocks grow by the step. Once
the age reaches record field 0x78 the effect is told to die away and the beam is
spent (+0x300); a beam with no effect is never spent. While it is not spent and the
wait is over, it strikes: the wait becomes record field 0x7C, the clock is cleared,
and the ray test above is tried against each body sphere of the player, the first
one met being hit at the point the ray enters it (no scatter). The beam is removed
once its age passes field 0x78 plus 30.

The Hornet's beam (320): node 50, 30 sixtieths, a strike every 8, so four strikes
at ages 4, 12, 20 and 28; attack 302, effect 203005, sound 164.

### Class 4: a bomb (built at 0x872E8, 0x360 bytes, update 0x8A284)

Lobbed in two hops, it lies where it comes down until its fuse runs out, and then
bursts; the burst stays and strikes like a beam. The Spider's "Humpty" is one (bullets
410 / 411: speed 20, first hop 550 high, fuse 60, reach 200, the burst lasting 270 and
striking every 45, effects 998016 and 998018, attack 401). The table's only other
bombs are 40 and 41, which no enemy's script fires; the code has a branch for a bomb
of the player's, so they are presumably hers.

- First frame (state 0 at +0x2E0): its frame (+0x2A0) is the owner's node matrix as
  it stands, moved by the offset and turned, like a ray's; the muzzle effect and the
  sound are played; its position (+0x2F0) is that frame's; its look (record field
  0x3C) is set at a matrix (+0x250) that looks from there at the target point
  (+0x290, the shooter's shot aim). That frame it does not move.
- The next frame it lays its way (+0x340 from 0 to 1). The distance along the ground
  to the target point counts as no less than 750 and no more than 1500 for an enemy's
  bomb, and as 750 for anyone else's. Two points are noted on the line towards the
  target: where the first hop is to end, 200 short of that distance (+0x320), and
  where the second is, 100 short (+0x330).
- A hop (velocity at +0x300, gravity at +0x310): the time it takes is the distance
  along the ground over the speed, and no less than 10 sixtieths; the speed along the
  ground is the distance over that time. The upward speed is the one that takes it to
  the height of record field 0x80 above the ground and on down to the ground by the
  end of that time (0x87264: the larger root of v² - 4Hv/T + 4Hb/T² = 0, where H is
  that height less its own and b is its own height taken from 0; with no two roots,
  0), and the gravity is what then brings it down in that time (0x872C4: 2(b - vT)/T²).
  Each frame the gravity times the step is added to the upward speed, and then the
  velocity times the step to the position; so, stepped two sixtieths at a time, it
  tops out lower than the figure by about its launch speed, and comes down a little
  early.
- Once it is no higher than 20 (0x12950C) it is put at 20: the first hop is over, the
  camera shakes (0x9E0B8) and sound 124 plays. The second hop is laid at once and
  takes its first step that same frame: to the second point, at 3 a sixtieth along
  the ground, a tenth as high. Down from that it stops, and the time it bursts at
  (+0x344) is its clock (+0x318) plus record field 0x84.
- The burst (state 2): its look is taken away, the effect of record field 0x8C is put
  in its place, the sound of field 0x90 plays, the camera shakes (0x9E2E8), and its
  clock starts again from 0. It strikes that same frame, and then whenever the time
  since the last strike (+0x34C) has reached the wait (+0x348, record field 0x7C).
  Once the clock reaches field 0x78 its effect is told to die away and it strikes no
  more (+0x350); a burst with no effect is never told. It is removed once the clock
  passes field 0x78 plus 30.
- A strike of an enemy's bomb tries each enabled body sphere of the player: one whose
  middle is no further from the bomb than the reach (field 0x88) and its own radius
  together is hit, the first such, with the owner's attack record (0x9AD90); with
  the owner gone it hits nothing. The player's own is tried the same way against
  every enemy's body spheres, the first of each enemy (0x92510).

### Class 5: a strike at a spot (built at 0x874EC, update 0x8AF40)

Four of the bosses' bullets (1910, 2020, 2220 and 2340). Nothing flies. The
constructor keeps the point aimed at with its height nought (+ 0x250). The first
update plays the record's sound (not the muzzle's effect), puts the bullet a step
along +z from that spot (+ 0x270) and plays its look there from the battle's own
effects, turned to face the spot (0xA8340). Its time (+ 0x2C4) runs from the frame
after; when it has come to record field 0x94 (55, 5, 5 and 30 sixtieths) each
enabled body sphere of the player is tried, and the first whose middle is no
further from the spot than field 0x98 (200 for all four) and its own radius
together is hit, with the owner's attack record (0x9AD90); then the bullet is gone,
and what was seen of it plays out by itself.

### The frame a bullet is made on

An enemy's script fires during that enemy's update, and the new bullet joins the end
of the list of actors being run (0xAE53C), so the bullet's own first update comes
later in the same frame. For every class that first update only sets it up: it moves,
strikes and counts its time from the frame after. (A ray is the exception by its
nature: its one frame is that one.) The port makes the bullet as the script fires it,
notes the logic frame (`Fired`), and leaves it alone for the rest of that frame.

## Effects in a battle

How an effect runs, how it is drawn and how it has its time are in
`FORMAT_NOTES.md` ("Effects"); the port's side is `battle/effects.rs`. A battle has
the effect files of its enemies' archives, of her skills' and of `SYSTEM.VOL`, and an
effect is asked for by number (0x9CA6C looks in each).

**Hers** (0x7D0E8 makes one of the battle's own; 0x7D1D4 has it go with a node of
her model, or with her whole for node -1; 0x7D154 leaves it where a node is; 0x7D23C
moves it by a matrix; 0x7D28C names its draw list):

| when | effect | where |
| --- | --- | --- |
| a shot leaves (0x7ED24) | 999004 | on node 0x32, the muzzle, going with it |
| her gun overheats (0x7E21C) | 700001, twice | on her whole; the second of the last draw list |
| a dodge begins (0x7E940 on) | 608006 to her first standing place or 608008 to her second, and 608007 | on the ground where she sets off, turned to where she goes |
| | 608010 | on her whole, turned so from her, going with her |
| she is within 200 of where she goes (0x7EA44) | 608005 | left where node 0x3C is |
| a skill of hers acts of itself (0x9F6D8) | 998045 | on her whole, of the last draw list |
| such a skill gives her HP as the battle is won (0x9FA40) | 998042 | where she stands |

**A bullet's** (0x887B0, 0x89D80, 0x8A41C, 0x86784): the effect at its muzzle is the
battle's own and stays where the bullet leaves from: the frame a flying bullet starts
in, or for a ray, a beam and a bomb the node's frame as it stands, moved and turned
as the record says. Its look is the bullet's model (0xA78B8): a flying bullet's is
turned from where it starts to what it was fired at (0x8899C) and stays turned so,
sized by the record (a size of nought leaves it as it is, 0x889B0); a beam's is at
its start, turned along it; a bomb's is turned to what it was thrown at and not
sized, and at the burst gives way to the burst's effect in the same frame (0x8A96C).
A beam's look and a burst's are told to end when their time is up (0x89F24,
0x8AA8C) and have 30 sixtieths more to die away in.

**An enemy's script's** (engine functions 18 and 82, 0xBD320): the enemy's own; by
a node of one of its models, left there (mode 0) or going with it (mode 1), in the
draw list the script names.

**A hit on an enemy** (0x92A38, all the battle's own, at the place the hit record
has): a guarded hit has effect 505002 and then the look of a guarded hit of the
attack; any other has 998048 first if the attack does more to an enemy that is
broken and this one is, and then the attack's look. All but 505002 are of the last
draw list. The look is the attack record's word at 0x30:

| look | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| on an enemy (0x9089C) | none | 999002 | 998022 | 998021 | 998019 | 999033 | 998046 | 998046 | 998047 | 998047 | 998048 |
| on her (0x97E90) | none | 999003 | 999024 | 999008 | 998024 | 999029 | 998023 | 105051 | none | none | none |
| guarded, the look of (0x9081C, 0x97E10) | 0 | 0 | 1 | 3 | 0 | 4 | 5 | 7 | 8 | 9 | 4 |

Where the hit record's place is: for a shot of hers and a flying bullet of a skill,
the point of the body sphere that is towards where the bullet first touches it
(0x8B5F8); for a ray, where it meets the sphere, up to a hundred off across and up
or down (0x88088); for a bomb's burst, the point of the sphere towards the bomb
(0x8AC10); for a skill that strikes outright, the middle of the sphere (0x74FA4),
no higher than the battle system's value 87 (330, "blade hit pos y limit") for the
one that is for one enemy (0x779C0); for a snipe, her lock point (0x7F7D8). A ray of
hers that meets nothing has effect 998021 at its far end, 3000 on, up to a hundred
off each way (0x88154).

**A hit on her** (0x9B300 to 0x9B4F8): a guarded hit has effect 505001 go with her,
75 up and 50 before her; a skill of hers that acts on the guard has its 998045 too.
Where the hit lands, of the last draw list: if the attack carries an ailment and it
took, 998025 for poison, 998026 for a stun, 998027 for any other of kinds 7 to 14;
or else the attack's look, by the table above. The place is the point of her body
sphere towards where a flying bullet came from (0x89898), towards a bomb (0x8ADE4)
or towards the attack sphere that touched her (0x923E0); where a beam meets her; or
where a ray does, up to fifty off across and up or down (0x88288).

**A status effect given** (0x8FC80): one of three effects where its bearer stands,
with its sound; see "The sound an effect is given with".

Not ported: an enemy's effects standing still while the enemy is in hit stop.

### The effects of her skills' scenes

`skills::scene` has them among each scene's steps (`Step::Effect`, `EndEffects`,
`EffectList`), and `tools_re/skill_effects.py` lists them from the code. Every
skill's code has its own copy of the same small helpers (the addresses here are
those of Photon Shower's): 0x7488C makes an effect of the battle's own and gives
back its node; 0x74978 has it go with a node of her model, or with her model whole
for node -1 (0xA74A8); 0x748F8 leaves it where her model is (its matrix is the
node's in the world, 0xACECC); 0x74A0C moves it by so much in its own frame and
0x74A90 turns it by so many degrees, about Y, then X, then Z (0x240C0; every one of
these multiplies after what is there, 0x23EE0); 0x749E0 names its draw list; and
0x74B5C has every effect of a number that plays be no more at once (0x9E9F4, which
unmakes them: one that is ended by 0x9E99C dies away instead). An effect that is
only made is where the world begins, which is the middle of the stage: every
emitter of all of those but one is fixed to the screen, and the one, BB Genocide's
10153, is in the world there, where the enemies stand.

In the table "on 50" is going with that node of hers (50 her cannon's muzzle, 60
her eye, 80 where a gun of a skill is held, 11 a blade, 16 a sword or a hammer),
"on her" going with her model, "left" left where she stands, "nowhere" only made;
the number after is the draw list, the fourth with whoever fights unless it says
otherwise. One "on the weapon" goes with the weapon's node turned (90, 0, 90) and
is of the last list. Times are sixtieths on the state's clock.

| skill | effects |
| --- | --- |
| Charge Shot | 68: 10001 on 50, last list. 150, as the shot leaves: 10003 on her, and 10001 is of the fourth list from here. As the state ends both are no more |
| Burst Shot | 0: 10011 nowhere, last list. 88: 10012 on 50, last list, moved (0, 2, 15). 168: 10013 on 50. As the state ends all three are no more |
| Photon Shower | 0: 10160 on 50, last list; 10161 nowhere, **first list**. 298: both no more; 10163 nowhere, last list. 340: 10162 left, moved (-600, 1500, -570) and turned (50, 0, 0). 346: 10164 left, moved (-600, 1, 700). 520: those three no more (and all five as the state ends) |
| Blade Kill | 142: 10041 on her, 10043 on 11. 162: 10132 nowhere. 166: 10142 on her. As the state ends all are no more. (Its archive has 10041 to 10044: 10132 is Steel Cutter's, so it is there to be seen only with that skill in a slot too, and nothing has 10142) |
| Iksa Blade | 142: 10051 on her, 10053 on 11. 160: 10054 nowhere. 166: 10052 on her. As the state ends all are no more |
| BB Genocide | 0: 10150 left; 10151 on 11, last list; 10152 on her; 10153 nowhere. 170: 10151 is of the fourth list. 400: all four no more |
| Stun Snipe, G-1 Snipe | 0: 10020 (10030) on the weapon; 10021 (10031) nowhere |
| Vulcannon | 0: 10060 on the weapon. 2: 10061 nowhere. 128: both no more. As the volley is over: 10062 on the weapon |
| Volcane, Vortex | 0: 10080 (10200) on the weapon, 10081 (10201) nowhere. As the volley is over: 10082 (10202) on the weapon |
| Tracer Gun, Chaser Gun | 0: 10090 (10100) on the weapon, 10091 (10101) nowhere. 200, before the bullets: 998003 on 50. 250: 10092 (10102) on the weapon |
| Star Comet and its G | 0: 10110 on the weapon, 10111 nowhere. 192, before the bomb: 998002 on 50. 240: 10112 on the weapon. As the state ends all four are no more |
| Steel Cutter (War Hammer) | 0: 10134 (10194) on the weapon, 10131 (10191) on her, last list. 192: 10132 (10192) nowhere, last list. 196: 10133 (10193) on her, moved (0, 0, 400) |
| Aggressor, Defender, Absolute 0 and the better of each | 72: 10120 on 60, last list. 76: 10121 left, last list. 160: both no more |
| Skill Boost, Regenerate and the better of each | 0: 10170 on 60, last list. 72: 10171 left. 160: both no more |
| Heat-Free, Overlimit, Ignition | 0: 10182 nowhere, last list. 222: 10180 left, last list. 240: 10181 left. 372: all three no more; Ignition then has 998042 on her |

**The first draw list, and the sheet when it is all black.** The sheet that dims
what is behind her (`BtBgColor`, 0x85490) is the first thing of the fourth list,
over the map, while it is less than all there (0x855BC). At 255 it is the first
thing of the *first* list instead (0x855AC), and the map's own draw leaves at once
(0x97084): nothing of the map is drawn. So an effect of the first list is under the
map and the sheet while any of the map is to be seen, and is all there is behind
her once it is black. Photon Shower's 10161 is the one such effect of her skills: a
model fixed to the screen, the sky of its scene, which has the sheet black from 20
to 298. In the port (`dimmer.rs`) a camera before the map's clears the picture (to
black at 255) and draws what such an effect has on the screen; the map's camera
draws what it has in the world, with the map or, at 255, without.

### The camera's shake

`battle/shake.rs`. A shake moves the whole picture by whole pixels: the battle's
camera keeps one (at its + 0x16C), and as the scene is drawn every draw list's
camera is given the same offset on the screen (0x8BE90 from the root's 0x8D110, to
0xA30C8), together with the scope's under a snipe (0x8BEF0).

A shake has a strength, a line across the screen that leans so far from upright, a
time it keeps its strength and a time it loses it over, and a swing of four
sixtieths that begins at its far end (0x9DF74 has what is not given). The picture
is `strength x sin(swing)` along the line (0x9DD44). The root has its frame before
anybody's turn (0x9DED8 from 0x8D0A4): its time and its swing go on, and when its
times are up it is over (0x9DE28). One that is asked for while another goes on takes
its place unless it is weaker than that one is now (0x9DFA0). At thirty frames a
second a frame is half a swing, so the picture is to one side on one frame and to
the other on the next.

| what | strength | keeps, loses over | line |
| --- | --- | --- | --- |
| kind 0 (0x9E0B8): a shot of hers (0x7ED08) | 3 | 2, 0: the one frame | upright |
| kinds 1, 2, 3: a bomb's first landing is 1 (0x8A7A8) | 4, 6, 8 | 0, 8 | up to 30 degrees either way, by chance |
| a hit, on an enemy or on her (0x9E278, from 0x92A30 and 0x9B3E0) | by the attack's word at 0x3C: 1 to 3 those kinds, 4 kind 0, anything else none | | |
| a bomb's burst (0x9E2E8) | 8 | 0, 32 | by chance |
| an enemy's script (engine function 56, 0xBDBC8 to 0x9E368) | 2, 5 or 8 for its first argument 10, 20 or 30 | 0, twice its third argument | its second argument, in degrees |

### The numbers a battle puts up

`battle/numbers.rs`. Where something lands a number is put up: an entry of the signs'
layout (0x3D81C) that plays a set once and is gone (0x3CFD4), at the place the camera
has for it on the screen as it is put up (0x3D8CC), cut to whole pixels (0x3EFA8).

| what | made by | set | figures |
| --- | --- | --- | --- |
| what a hit takes off (entry kind 0) | 0x357E8: a hit on an enemy, where it lands (0x92A10), unless the battle is decided; a hit on her, where it lands, and what poison takes off her, by her own place (0x98C24) | 3 | group 17, cells of 11 from the signs' pack's file 8 |
| HP given back (kind 2) | 0x35868: to her (0x98B8C) or to an enemy (0x91750), by whoever's own place; every gift of HP goes through these (0x8DE18): an effect that gives it, one that gives it every so often, and the share of the damage an attack gives back, when that is more than nought (0x8FBE8) | 18 | the same |
| what a hit she guarded takes off her, if she stands (kind 3) | 0x358A8, from 0x98CEC | 24 | group 18, cells of 7 from the pack's file 14 |

(Kind 1 plays set 19 and nothing makes one.) A set is its figures and nothing more:
they come in over four frames of the layout, stand for six, and rise 23 pixels as
they go over the next 22. The group's one object is drawn once for each figure, from
that figure's cell of the sheet, the figures a pixel further apart than their cells
and about the place by half their cells (0x3F178). What a hit takes off is drawn by
a colour: (230, 230, 230) off an enemy, (230, 100, 30) off her (0x3F058, by the
display's word at 0x7C, which 0x3577C sets for whose it is); what is given back has
its set's own (110, 240, 255).

Somebody's own place is the world's place of a node of theirs (0xA7F58): node 0 of
her model (0x99E14), and for an enemy the node its script has its mark hang from
(0xBFB38), without the mark's own distance from it.

## Status effects (0x8E164-0x9062C)

The player and every enemy have a record of the effects in force on them (the block
at actor + 0x1E8 for her, + 0x284 for an enemy; 0x8F780 picks it). It is a chain of
nodes, the newest first, with room in the block for one node of each kind that stays
(+0x10 kind 6, +0x58 kind 7, then one every 0x40 up to kind 25 at +0x4E0; kinds 6 and
20 are 8 bytes longer).

A node (0x8E164): flags (bit 0: it has no time limit), the block, the next node, a
running number, its record, its kind, who gave it, the giver's special attack at the
time (+0x1C), its duration and time run (+0x20, +0x24), what to call when its time is
up (+0x28), and the icon the gauges show for it (+0x30-0x3C).

### The table (`btl_eff_tbl.bin`, 0x34-byte records, 71 of them)

| Offset | Meaning |
| --- | --- |
| 0x00 / 0x04 | id / kind |
| 0x08 | chance of taking, in percent |
| 0x0C | duration in sixtieths; 0 for no limit |
| 0x10-0x1C | four figures, read by kind through accessors that clamp them (0x129B1C on) |
| 0x24 | icon; negative for none |
| 0x28 / 0x2C | battle-log lines: as it takes, and as it runs out |
| 0x30 | it shows an effect and plays a sound as it takes (by kind, table at 0x169278) |

An attack names the effect it carries by id (attack record 0x20, read by 0x129130).

| Kind | What it is | Figures |
| --- | --- | --- |
| 1 | HP back at once | a set amount (0-9999), percent of the giver's special attack (0-999), percent of the taker's most HP (0-100), spread |
| 2 | cure | bits: 1 kind 6, 2 kind 7, 4 kind 9, 8 kind 10, 0x10 kind 11, 0x20 kind 12, 0x40 kind 13, 0x80 kind 14 |
| 3 | has the skills of her slots ready at once (0x900E0, 0x9EEC4): each that was not is filled and heard (sound 0x65) | bits: 1 square's slot, 2 triangle's, 4 circle's, 8 cross's |
| 4 | her heat is gone, and her overheating with it (0x978A8) | |
| 5 | shields a guard: used from the player's skills, not given as an effect | percent off a guarded hit; 0x1C names a heal she gets on one |
| 6 | poison | interval in sixtieths (0-600), percent of the giver's special attack, percent of the taker's most HP |
| 7 | she cannot shoot | |
| 8 | she cannot dodge | |
| 9 | stun: she can do nothing; ends with the next hit | |
| 10 | she cannot use skills | |
| 11 / 12 | attack / defence down | percent (0-100) |
| 13 | shortens an enemy's waits | percent |
| 14 | not read: nothing asks for it but the cure | |
| 15 / 16 | attack / defence up | percent (0-999) |
| 17 | lengthens an enemy's waits | percent |
| 18, 23 | nothing asks for them | |
| 19 | a boost to what the bearer gives | percent more damage, more healing, and longer durations |
| 20 | regeneration | interval (0-3600), percent of the giver's special attack, percent of the taker's most HP |
| 21 | ward: every hit counts as guarded | percent off damage (0-100); percent off the taker's side of the chance sum |
| 22 | nothing adds heat (0x97658) | |
| 24, 25 | the cursor passes over the enemy: no lock (0xA14D0); with 24 its gauges are not drawn either (0x90DC4) | |
| 26 | of an attack: it goes through a guard, with sound 0x68 (0x8FA88) | |
| 27 | of an attack: more damage to an enemy whose first gauge is broken (0x8FAF8) | percent |
| 28 | of an attack: its giver gets a share of the damage back as HP (0x8FB68) | percent |
| 29 | changes an enemy's own attack, special attack, defence and guard percent for good, each kept within 0-999 | the four changes (-999 to 999) |

### Giving one (0x8FE28)

The request names the effect, the giver and the taker. First the chance (0x8F8F8):

    chance = (record's chance + 2 x giver's level)
             - (taker's resistance + 2 x taker's level) x ward

and it takes if a random number below 100 is less than that. The comparison is
unsigned, so a chance that comes out below nothing counts as certain: a taker far
above the giver's level is always caught. The player's level is her stat 16, an
enemy's is in its variant record; the player resists nothing, and an enemy has a
figure for kinds 6, 8, 9, 11, 12 and 13 (variant record 0x70-0x84, 0x8F7DC). `ward` is
1, or with a ward on the taker (100 - its second figure) / 100.

Then the log line and the effect's show (0x8DE7C, 0x8FC80), and by kind:

- 1: the taker heals by (set amount + giver's special attack x percent / 100 +
  taker's most HP x percent / 100) x (100 + the giver's boost to healing) / 100 x
  spread, cut to a whole number (0x8E078, 0x8DE18).
- 2: the nodes of the kinds its bits name are removed.
- 6-14: nothing if the taker has one of the kind already; otherwise a node is made.
- 15-25: one the taker has is removed first, then a node is made: it starts afresh.
- 29: only on an enemy.
- 5, 26, 27, 28: nothing.

A node's duration is the record's x (100 + the giver's boost to durations) / 100; 0
means no limit.

### While it lasts

- The player's update (0x99F54), unless a staged moment is playing, the battle is
  decided or she has no HP: every node's clock runs on by 2 (a constant, not the
  step; 0x8F01C), and one whose time is up logs its second line and is removed; a
  regeneration (0x8F634) heals her when its interval has passed; then a poison
  (0x8E504, 0x8E538) takes its toll through 0x98C24, and if that leaves her without HP
  she is given 1: poison cannot kill.
- An enemy's update (0x92110), unless its first gauge is broken: the clocks and the
  regeneration. Nothing counts an enemy's poison.
- The toll of a poison or a regeneration: giver's special attack x percent / 100 +
  bearer's most HP x percent / 100.
- Her choice of action (0x9A474): a stun blocks guarding, dodging and shooting; kind
  8 blocks dodging and kind 7 shooting. 0x99050 asks before a skill or an item: not
  overheated, no stun, no kind 10, and by which it is no kind 7 or no kind 8.
- A hit (0x9AD90 on her, 0x92510 on an enemy; the two agree). A stun on the one hit
  ends (0x8F420). A ward makes the hit a guarded one; an attack that cannot be guarded
  is not, nor one whose effect is of kind 26; nor does an enemy with a broken first
  gauge guard. The damage is multiplied, in this order, by: the attack's own percent;
  (100 + attacker's attack up - down) / 100; (100 - target's defence up + down) / 100;
  the ward's share, (100 - its first figure) / 100; (100 + attacker's boost to damage)
  / 100; on a broken enemy (100 + the attack's kind-27 figure) / 100; and through a
  guard the guard's percent. If the hit does not kill: the attacker heals by damage x
  the attack's kind-28 figure / 100, and the attack's effect is given to the one hit,
  with the attacker as its giver. A guard does not stop either.
- Engine function 103 scales a number by (100 + kind 17 - kind 13) / 100 of the
  enemy's own record; the scripts put their waits through it.

### The scripts' side

Engine function 67 with `what` 8 (0xC02D8; whom, effect, give): gives the effect to the
player (whom 0) or to the enemy itself (1), or with `give` 0 takes it away (0x8FC30).
Function 120 (0xBF020; effect, give, itself) does so for every other enemy that has
something in the slot its HP gauge hangs on (0xA7F58), and for itself too with
`itself`; nothing happens unless somebody besides itself stands. Function 68 asks
whether an effect is in force on the player (first argument 0) or itself (1): `what`
26 by its id, 27 by its kind.

What the ordinary enemies do with all this (`tools_re/status_survey.py`): the
Spider's shot poisons (attack 400, effects 600 / 602); attacks of the Hand, of Big
Mouth and of the Giant Bomb stun (501, 1000, 1501: effects 900 / 901); the Masher's
take away the dodge (1303, 1304: 800); the Gunner's lower her attack (1202: 1101); the
attacks of the cat called Wild Turkey drain (2800), as does attack 202 of every cat
(2801). Most enemies have a skill state that heals
themselves by 50 (effect 100); the Generator heals the others with it; Big Mouth
raises its own attack (1500); the Spider hides in a cloud (2500); the Twins
regenerate for as long as their script lets them (2005).

The icons by the gauges and the log's lines are under "The display" ("The marks of
status effects", "The log"). `--afflict 602,1101` gives her and every enemy those
effects as the first fight begins, for a look at them.

Not ported: the effect's show (its sound is). The check before a skill and a
boost ending as she uses one are under "Her skills".

## Her items (`BtItemMenu` 0x94DD4, `BtItemMng` 0x950EC, `ifBtlMenuTask` 0x34234)

**Opening them.** The first thing her choosing looks at (0x9A474) is L: if she
can act, her lock on items (her + 0x770) is clear and she is not stunned
(0x98FA0), a press of it sets her + 0x734 and the menu's flag 1 (0x94F74), and
nothing else is chosen. While that flag is set her own frame does nothing
(0x9A0BC), and everything else of the battle that moves asks 0x99620 (her +
0x734) before it has a frame: the enemies (0x90F94), what flies (0x86470), the
battle's own clocks (0x8CF6C, 0x8D624) and the slots of her skills (0x9EBA4). So
the battle stands still while she looks at her items.

**The list** (0x953AC) is of the consumables she has any of whose record says
they can be used in a battle (the three bits over the limit: 2 in battles only, 3
anywhere; 0x12C520), in the order of the tables' own word for it. That is the
Repairers and the Nano Repairs (which give HP back: so much, and so many
hundredths of all she can have) and the fourteen that are for battles only, each
of which gives her a status effect (the record's + 0x18, 0x12C6F0): the four
Charges 301 to 304 and Max Charge 300 (kind 3), Body Cooler 400 (kind 4), the
five Breaks 200 to 204 and Rest Up 205 (cures), ATK Boost 1501 and DEF Boost
1601.

**The window** is a task of its own with a state (0x3438C): 1 coming, until its
frame's set has played; 2 open; 3 going. Open, it reads the pad the menus read:
L or circle leaves (answer 2, sound 5); up and down move the cursor, held or
pressed, and at the first or last of the list only a fresh press does anything,
which goes round to the other end (0x34814; sound 2); cross chooses the item the
cursor is on (answer 1, sound 3), or is refused if there is none (sound 7). It
shows four rows; the cursor's row and the first row shown (its + 0x60, + 0x64)
are kept from one opening to the next and only held within the list (0x3495C).
The menu takes the answer the frame after (0x95058), clears its flag, and has
the window go (0x3460C): the battle is on again while the window is still
going.

**The layout** is `if_btlsys.lpk/011.lpk` (five textures: its own three, the
icons `if_rsdsys.lpk/020.ptm` and the figures `009.ptm`), played in five places
with a set to come by and one to go by each (0x34584, 0x3460C): the frame (sets
0 and 1), the cursor's row (2 and 3), a row (4 and 5), a row for an item whose
record has bit 11 of its limits' word (6 and 7; no item has), and a row with
nothing in it (8 and 9). Drawing (0x34B9C): of the frame, group 15 is not drawn
but gives the place and the colour of what is said of the cursor's item
(written without a shadow, 0x2D900), and group 13, what goes along the bar at
the side, is drawn only with more than four items, four rows of all the list
long and as far down as the first row shown. The rows are drawn twenty apart;
of a row group 19 is the item's icon by its kind (0x2D198, uncoloured), 20 its
name, 21 how many she has, in two figures at the least, from the left on.

**Using one** (0x981BC, in her next frame): she has one fewer of it; her skills
that act of themselves are told (event 9, and 0xA after); one that gives HP
back gives `flat + most x hundredths / 100`, more by what Six Pocket has it be
(event 0xE: the amount by the skill's fifth figure, 150, in a hundred), with
sound 0x82 and effect 998042 where she stands; any other gives her its status
effect as she would give one herself (0x8FE28); then the heat of tunable 50 is
added, which is none.

In the port: `battle/item_menu.rs` (`ItemMenu`, `Window`, `ItemLayout`; the
battle's systems stand still by `not_paused`, which asks it too),
`PcActor::items_asked` and `PcActor::use_item`, `Items::for_battles`,
`status::kind::{CHARGE, COOL}`, `Slots::fill`, `Heat::cool`, and Six Pocket in
`auto.rs`. A run can be given items with `--carry 1:3,9:1,14:1`; picture
`docs/compare/battle_items_port.png`. Not compared with the walkthrough yet
(where it opens the window is not known).

## Her skills

### The table (`btl_skl_tbl.bin`, 20 fields a record, 66 records; 0x12AB9C)

| Field | What |
| --- | --- |
| 0 | the skill's number |
| 1 | its kind: 1 an attack, 2 one that helps her for a while, 3 one that adds to a figure of hers, 4 one that acts of itself |
| 2, 3 | the sixtieths it takes to be ready the first time in a battle (nought for every one) and after a use (0x12AD74, 0x12AD7C): 1200 for Charge Shot, 10800 for the three that strike everyone |
| 4 to 9 | for kind 4: when it acts, on what condition and with what figure for it, what it does and with what two figures (see "Those that act of themselves") |
| 10, 11, 12 | for kind 3: the figure (1 HP, 2 attack, 3 special attack, 4 defence), how much it adds, and hundredths of the figure besides (nought in every record) |
| 13 | the archive it is played from (`bskl_assult_00.vol`) |
| 14 | the line of `btl_msg_tbl.bin` that names it |
| 15, 18 | the status effects it gives (Aggressor 1500 and 1600, Defender 1600) |
| 16, 17 | its attack: one for those that shoot, one for the others |

Its name and what the menus say of it are record `n - 1` of
`db_resident.lpk/003.xtc` (0x12CE7C: twelve bytes, two texts and a word whose
bits 16 to 18 are what the menus sort it under, 0x12CDBC: 1 for those put in a
slot, 2 for those always in force; `tools_re/xtc.py` lists the records). They
are: 1 Charge Shot, 2 Burst Shot, 3 Photon Shower, 4 Blade Kill, 5 Iksa Blade, 6 BB
Genocide, 7 Stun Snipe, 8 G-1 Snipe, 10 Vulcannon, 11 Volcane, 12 Vortex, 14
Tracer Gun, 15 Chaser Gun, 18 Star Comet, 19 Star Comet G, 20 Steel Cutter, 22
War Hammer; 23 Aggressor, 24 Skill Boost, 25 Defender, 26 Absolute 0, 27
Regenerate, 28 Heat-Free, 29 Ignition, and 66 to 71 the better ones of 23 to 28;
30 to 38 those that act of themselves (Aftercare and its better, Max Up, Rocket
Starter, Dead Guard, Six Pocket, Attack-Free, Danger Guard, Sway Three), 39
Treasure Hunt, 40 Stealth and 41 without a name; 42 to 47 ATK Bonus (8 each), 48 to 53 DEF Bonus (10), 54 to 59 HP
Bonus (70), 60 to 65 SKL Bonus (5 or 6).

### Having one, and what it adds

Flag 7000 + n of the save says she has skill n (0x14D95C); the challenges set
it. Flag 7200 + n (0x14D994) says of a skill of a slot that it is in one, and
of one always in force that it is put aside. Setting or clearing either for a
skill that is always in force works her figures out anew at once (the flag
setter itself, 0x13DC and 0x1418, calls 0x14D008): each of the four totals
(the save's fields 21 to 24: HP, attack, defence, special attack) is its own
figure, what she wears, what the skills in her four slots add and what every
skill always in force that she has and has not put aside adds (0x14CDE0), kept
to 9999 for HP and 999 for the others; and her HP is kept to what she can have.

### Her slots

The save's fields 11 to 14 (her figures 13 to 16 to a script, 0xB60) are the
skills on triangle, circle, cross and square. A new game has none (the start
table's last four bytes, for triangle, square, cross and circle: 0x12B550);
the game's menu puts them there. In a battle `BtSkillPallet` (0x9F068) makes a
slot for each that has one, by a table (0x16A190) of the field, the row of the
column and the button: triangle row 0, circle row 2, cross row 3, square row 1.
A slot has its skill's record, how far it is to being ready and the time that
takes (0x9F130): ready at once if the first time is nought. While the battle
is on and nobody has the stage each one that is not ready gains `dt` over its
time a frame (0x9F2E4); come to be ready it flashes its row and sound 0x65 is
heard. Using one (0x9ED3C) starts it over with the time after a use. Two more
ways to be ready at once: every slot of a field (0x9EEC4), and every skill but
one (0x9EF90, which is what Ignition does). The column's rows are written in
the text's font, twenty glyphs with nothing between them (0x3E828); a row with
no skill keeps the seven dashes it is made with (0x1669FC).

### Using one (0x9A568)

With R held and the battle on, each slot in turn: its skill must be ready (or
the battle's value 17 set), its button pressed, and she must be able to
(0x99050): not overheated, not stunned, not kept from skills (status kinds 9
and 10); skills 1 to 8, 10 to 12, 14, 15, 18 to 20 and 22 not while she is
kept from shooting (kind 7), and 3 to 6 not while she is kept from dodging
(kind 8). Then the slot starts over, her state is ended (0x98E84) and the
skill's own state is made: action 7 for 1, 2, 10 to 12, 14, 15, 18 and 19, and
6 for the others.

Skills 4 and 5 are for one enemy: what the cursor has settled on (0x171314,
which is nothing while the cursor is being moved). It must have a body sphere in
use (0x97FC0: flags 8 and 2 of one of its actor's spheres); if there is no such
enemy the loop goes on to the next slot, and the skill stays ready (0x9A728).
Skills 7 and 8 are given the same enemy without the test, and their slot does
not start over here: their own state has it start over as their shot is fired.

| Skill | State made by |
| --- | --- |
| 1, 2 | 0x7680C, 0x76934 |
| 3, 6 | 0x7512C, 0x7521C |
| 4, 5 | 0x77B78, 0x77CD4 |
| 7, 8 | 0x7FB14, 0x7FC20 |
| 10, 11, 12 | 0x7C270, 0x7C380, 0x7C490 |
| 14, 15 | 0x7AD34, 0x7AE48 |
| 18, 19 | 0x798F4, 0x79A34 |
| 20, 22 | 0x849D4, 0x84AD8 |
| 23 to 29, 66 to 71 | 0x81CD4, which makes one of thirteen (0x81400 on) |

Those of kind 2 are each a small scene of the same shape: a state whose update
waits for its clock to come to one time after another (0x744B0;
`tools_re/skill_states.py` lists what each of the thirteen does):

- the line that names it is written to the log (0x80670), with her name;
- she takes the stage (0x80784: the root's flags 12 and 9, every enemy and
  bullet held and out of sight) and everything of hers is locked (0x806EC);
- what is behind her is dimmed (0x8073C) to the battle's value 0x54, which is
  196 of 255, over twenty sixtieths. The two Skill Boosts are not dimmed;
- a camera move and an animation of hers start (`chr_p_brs02_<clip>_cam.cam`
  and `_all.anm`, of the skill's archive), and the cameras are told how near
  and how far to see (0x808E0);
- its sounds and its effects come at their times;
- at its end the skill does what it does: it gives her its status effects
  (0x80E14, as she would give them to another: by her level and special attack,
  and by a boost if she has one), or, Ignition, has every other skill ready
  (sound 130, and 0x65 for each that was not). The effects end, the dimming
  goes back over twenty sixtieths, the camera and the stage are given back
  (0x808B4, 0x80BE4), she stands (`btl0000` blended in over eight frames,
  0x80B58) and can act again (0x80CCC).

As the state gives way to the next a boost she has is spent (0x80D2C), by all
but the two that give that boost.

| Skills | Clip | Near plane | What is heard, in sixtieths | End |
| --- | --- | --- | --- | --- |
| 23 Aggressor, 25 Defender, 26 Absolute 0, and 66, 68, 69 | `btl4020` | 50, and 45 from 76 on | 64 a line of hers, 74 sound 10 | 160 |
| 24 Skill Boost, 27 Regenerate, and 67, 70 | `btl4007` | 35 | 70 or 72 a line of hers, 80 or 98 sound 55 | 160 |
| 28 Heat-Free, 71, and 29 Ignition | `btl4008` | 20 | 50 sound 57, 218 sound 99, lines of hers at 20 or 90 and at 240 or 250, sounds 151 and 163, and for Ignition the first of its archive's own bank | 372 |

#### The sheet over what is behind her (`BtBgColor`, 0x85490)

Whoever wants the scene dimmed has an object for it (hers is at her + 0x714, an
enemy's at its + 0x80C): its strength now, where that came from, where it is
going and the sixtieths the way takes. 0x85814 sends it somewhere, at once if it
is there already or is given no time. 0x8570C takes it a frame on, and her frame
calls that before her state runs (0x99FF8):
`strength = trunc(from + (to - from) * time / frames)`. An object with any
strength is in the task's list, and the task draws one black sheet over the
whole screen with the strongest of them for its alpha (0x85930).

The sheet is the first thing of the scene's fourth draw list (0x855A4; of the
first when the strength is 255). The lists are drawn in order: the sky is in
the first (a map model's layer 0), the ground in the third (layer 2), and she,
the enemies and what they fire in the fourth. So the map is dimmed and nothing
that fights is.

The walkthrough has it to the unit, on the frames after Defender's scene
(2479.37 s on): a piece of wall that is (143, 166, 143) is (45, 52, 46), then
(55, 65, 56), (67, 78, 65), (78, 91, 79), (89, 103, 88) and on to (131, 153,
132), which are 1 - 176/255, 1 - 156/255 and so on of it: the strengths of the
ten frames back from 196.

#### How near and how far the cameras see

Each draw list has a camera of its own, which takes the view and the lens of
the scene's camera and keeps its own near and far planes (0xA6610). A map's
script sets those for the first list and for the other three (`e44_view`,
0xA6F1C: 50 and 50000 on the first stage's map). A skill's scene sets them for
the other three (0xA7CAC with 1, which 0xA7678 applies each frame through
0xA6724 and 0xA675C): the near plane of the table above, the closer the nearer
the move comes to her, and 20000 for the far one. "Lens back" (0xA7CE0) only
stops their being set each frame. Nothing puts the map's own back, so they stay
as the skill left them for the rest of the battle.

#### The attacks: Charge Shot and Burst Shot (0x7699C, 0x76D84)

A skill that attacks has a state of its own, whose update is a switch on the
state's phase (+ 0x9C). `tools_re/skill_unit.py --maker 0x7680C` lists one phase
by phase, with its unit's own copies of the helpers named by what they call
(`tools_re/out/skill_1_charge_shot.txt`, `skill_2_burst_shot.txt`). These two
are a scene like those above and then a shot:

| Sixtieths | Charge Shot (`btl4000`) | Sixtieths | Burst Shot (`btl4022`) |
| --- | --- | --- | --- |
| 0 | the log's line, the stage taken, dimmed, the camera's move and her animation, her hands as in a battle (part modes 4 and 5, 0x9B608) | 0 | the same, and effect 10011 |
| 30 | sound 144 | 30 | her line `brs_1003`, sound 144 |
| 34 | her line `brs_1003` | 86 | sound 143 |
| 46 | near plane 20 | 88 | effect 10012 on her node 50 |
| 68 | effect 10001 on her node 50 | 160 | sound 176, her line `brs_4900` |
| 76 | sound 143 | 168 | camera and stage given back, she aims, the bullet, effect 10013 |
| 136 | her line `brs_2600` | 180 | the dimming goes back |
| 146 | sound 176 | 238 | she stops aiming |
| 150 | the camera and the stage are given back, she aims (her + 0x7F4 and 0x993B8), the bullet is made, effect 10003 | 252 | she stands and can act |
| 170 | the dimming goes back over twenty | | |
| 236 | she stops aiming | | |
| 244 | she stands and can act | | |

As either state ends its effects end and a boost is spent. Her action while it
runs is 7, which is what an enemy's script asks after with sense 21 (0xC054C).

The bullet (0x766B4): the skill's field 16 is a bullet of `btl_bullet_tbl.bin`,
found by its number alone (0x12B22C, 0x12927C: Charge Shot's is 10). If its
class is 1 it is made as a projectile of classes 1 and 2 (0x86E5C, which builds
it with 0x883B4): its record, what the cursor is locked on to (0xA1CBC), her aim
point (0x989F8), herself as its owner, and the skill's record (the bullet's
+ 0x244). What such a bullet of hers does differently from an enemy's is under
"Classes 1 and 2".

Bullet 10 leaves her node 50, the cannon's muzzle, flies straight at 120 a
sixtieth for as long as 8000 units take, has a sphere of 50 and goes through
what it hits, each of which takes her attack 10; Burst Shot's (11) has a sphere
of 75 and attack 11.

The other attacks, as their listings have them (`tools_re/out/skill_<n>_*.txt`,
one for each maker). All but Photon Shower put another weapon in her hand for
the scene: a model of the skill's archive, which the unit's own helper (0x7AB48
in Tracer Gun's) puts in her second model slot, where the cannon is
(0xA7830(her, 1, file)), and fixes to a node of hers (0xACD14 with
0xA74A8(her, 0, node)). Which node is the unit's own:

| Helper | Skills | Node |
| --- | --- | --- |
| 0x7AB48, 0x7C0E8, 0x79738, 0x7F634 | the guns: 14 and 15, 10 to 12, 18 and 19, 7 and 8 | 0x50, as the cannon |
| 0x777BC, 0x74E80 | the blades: 4 and 5, and 6 | 0x0B, her left hand |
| 0x84750 | the sword and the hammer: 20 and 22 | 0x10, her right hand |

Her hands are changed for it with part modes 3 and 0 (3 and 1 for the blades of
4 and 5), and with 3 and 5 she has the cannon back.

Where her model stands in a scene. Her update places her model from where she
is and the way she faces every frame (0x992DC), unless her + 0x7FC is set
(0x9A08C). A unit's own "take the stage" is one of three sorts:

- 0x745E8, 0x787D4, 0x80784, 0x84008 (those that help her, 3 and 6, 20 and 22,
  and her last states) set + 0x7FC and nothing else: the model stays where it
  was;
- 0x75FC8, 0x79004, 0x7A430, 0x7B94C, 0x7EFA4 (1 and 2, 18 and 19, 14 and 15,
  10 to 12, 7 and 8) give the model the identity matrix and do not set + 0x7FC,
  so her update has it back where she is the same frame: nothing is seen of it;
- 0x7720C (4 and 5) sets + 0x7FC and gives the model a matrix that is a half
  turn about Y at the origin: for that scene she is at the middle of the map,
  turned round, whatever her place.

Every unit's "give the stage back" clears + 0x7FC and places the model again
(0x99950). A camera move plays in the frame of her model (see "Camera moves"),
so it goes where the model is put.

Tracer Gun's firing (0x7ABB8) makes the skill's bullet if its class is 2, with
what the cursor is locked on to and her aim point as Charge Shot's is made, and
then does the same for the bullet that record names as fired along with it
(field 0x9C), and so on down the chain, all in the one frame. Its bullets are
33, 34 and 35: from 65 above her feet (node -1, the model itself), at 25 a
sixtieth, leaving sixty degrees to her right, upwards and to her left, with a
sphere of 25 and her attack 33. Chaser Gun's are 36 to 39, with attack 34.

Those that hit the enemies themselves (0x74EF0 in Photon Shower's unit, 0x847C0
in Steel Cutter's) do it while she still has the stage. A little before, the
enemies are shown again, held as they are (0x746D8, 0x840F8: the flag that
keeps an actor from being drawn is cleared on every actor but her and the
stage's own). Then every enemy that has a body sphere in use (flags 8 and 2)
takes her attack of the skill's field 17 (0x12B24C, 0x129014) once, at the first
such sphere of it (0x92510, with the skill's record in the hit).

| Sixtieths | Photon Shower (`btl4010`) | BB Genocide (`btl4009`) | Steel Cutter, War Hammer (`btl4006`) |
| --- | --- | --- | --- |
| 0 | part modes 3 and 5, the stage, dimmed over twenty, the camera (near 50, far 35000), sound 105; no line in the log | the log's line, the stage, dimmed at once, near 50, the camera, part modes 0 and 3, the blade | the log's line, the stage, dimmed, the camera, part modes 3 and 0, the weapon |
| | 20 all black at once (to 255) | 50 `brs_1000` | 50 sound 132 |
| | 40 `brs_0310`, 44 near 1 | 76 sound 164 | 102 or 110 `brs_0020`, 104 sound 144 (the sword only) |
| | 48 and 80 sound 108 | 170 near 1, sound 56, `brs_1008` | 122 sounds 164 and 144 |
| | 144 `brs_1007`, 190 sound 143 | 180 part mode 2 (her hand is empty) | 164 the enemies are shown |
| | 230 `brs_3600`, sound 104 | 260 `brs_2900` | 190 `brs_4600`, or 180 `brs_3500` |
| | 240 sounds 164 and 163 | 266 near 200, the enemies are shown | 198 sound 185, or 105 |
| | 298 near 200, dimmed as usual at once, the enemies are shown | 270 sound 181 | 200 the blow |
| | 344 sounds 163 and 164, 354 sound 151, 420 sound 163 | 296 sound 151, the blow | 300 the camera and the stage back, part modes 3 and 5, the dimming goes back; the clock starts over |
| | 460 the blow, 466 sound 150, 470 sound 181 | 400 the dimming, the stage and the camera back, part modes 5 and 3, she stands and can act | 20 more: she stands and can act |
| | 520 the dimming, the camera and the stage back, her animation is put back to its start (0x74B78), she can act | | |

A strength of 255 has the sheet drawn first of the scene's first list and not of
its fourth (0x855A4). What keeps the map from being drawn over it then has not
been found; the port takes it for what the figure says, all black behind her.

Blade Kill and Iksa Blade (0x77D44, 0x781B0) are for the one enemy she had
locked on to as the skill began, which the state keeps (+ 0x9C; its constructor
0x77A8C also sends that enemy's script signal 103, which none of the 88 scripts
answers: `tools_re/signals.py` lists what each answers). The scene has two
parts, each with an animation and a camera's move of its own:

| Sixtieths | Blade Kill | Iksa Blade |
| --- | --- | --- |
| 0 | the stage (0x7720C: her model at the origin, turned half round), the log's line, dimmed over twenty, part modes 1 and 3, the blade `chr_w_bla01` at her node 0x0B, the camera and her animation `btl2000` | the same with `chr_w_bla02` |
| 20 | her line `brs_1004` | `brs_0311` |
| 24, 78 | sounds 168 and 105 | the same |
| 142 | the enemy is shown again, and it alone (0x778C0: the flag that keeps it from being drawn is cleared on its actor); the camera and her animation `btl3100`; effects 10041 and 10043; her model is put where the enemy's stands, on the ground (0x7782C) | the same with effects 10051 and 10053 |
| 150 | her line `brs_5000` | `brs_4900`, sound 181 |
| 154 | sound 181 | |
| 162 | effect 10132, the blow (0x778F8) | at 160, with effect 10054 |
| 166 | effect 10142 | effect 10052 |
| 200 | the dimming goes back over twenty; the clock starts over | the same |
| 20 more | the camera and the stage back, part modes 5 and 3, her animation is put back to its start (0x775B0), she can act | the same |

0x7782C takes the matrix of the enemy's first model, and gives her model a half
turn about Y at that place with its height made nought: she is on the ground
under an enemy that flies. The blow (0x778F8) is one hit (0x92510) with her
attack of the skill's field 17, at the enemy's first body sphere in use; where
the hit is said to be is that sphere's centre, no higher than the battle's
value 0x57.

Vulcannon, Volcane and Vortex (0x7C4F8, 0x7C930, 0x7CCF4; a switch of 22 phases,
of which 10, 11, 20 and 21 are the volley and the end). Their constructor keeps
the skill's bullet (field 16: bullets 29, 30 and 31, which are rays) and that
record's count (field 0x30: 35) as the rays left (+ 0x98).

| Sixtieths | Vulcannon (`btl4002`) | Volcane (`btl4003`) | Vortex (`btl4021`) |
| --- | --- | --- | --- |
| 0 | the log's line, the stage, dimmed, the camera (near 20), her animation, which is set to go round the part of it that repeats (0x7BB8C); effects 10060 and, at 2, 10061 | the same; effects 10080 and 10081 | the same; effects 10200 and 10201 |
| | 16 sound 132 | 12 her line `brs_4500` | 20 sound 132 |
| | 28 part modes 3 and 0, the gun `chr_w_smg01` | 20 sound 132 | 40 her line `brs_3400` |
| | 50 her line `brs_2800` | 22 part modes 3 and 0, the gun `chr_w_gtr00` | 56 part modes 3 and 0, the gun `chr_w_smg02` |
| | 60 sounds 164 and 144 | 74 sounds 164 and 144 | 64 sounds 164 and 144 |
| | 64 near 20 again | | 100 near 20 again |
| the stage back | 128 | 120 | 122 |

As the stage is given back (the camera and the stage, the effects ended) the
cursor is let go of the lock points (0xA15E4) and the state goes to phase 10,
with what is behind her still dimmed:

- phase 10: while any rays are left she is turned to where she aims (her + 0x7F4
  and 0x993B8, as for Charge Shot, anew each time), one is made (0x7C158: only if
  the bullet's class is 0; towards her aim point of that moment, 0x989F8, with
  0x86C2C), the clock starts over, and the phase is 11. With none left the phase
  is 20.
- phase 11: once the clock has come to 6, one fewer is left, and phase 10 runs at
  once.
- phase 20: the cursor is as before (0xA15F8), her animation plays on to its end
  from where it is (0x7BBC0), effect 10062 (10082, 10202), part modes 3 and 5,
  sound 108, she stops aiming, the clock starts over, and the dimming goes back
  over twenty.
- phase 21: at 20 she stands and can act.

So thirty-five rays leave, six sixtieths apart, over three and a half seconds,
each with the bullet's sound (174) and each at wherever the cursor points as it
leaves. Each strikes at once (see "Class 0"): the nearest enemy on its line takes
her attack 29 (30, 31).

Star Comet and Star Comet G (0x79A9C, 0x79EDC) throw a bomb: the skill's bullet
(40, 41) is of class 4.

| Sixtieths | Star Comet (`btl4005`) | Star Comet G (`btl4005`) |
| --- | --- | --- |
| 0 | the log's line, the stage, dimmed, the camera (near 20), her animation; effects 10110 and 10111 | the same |
| | 30 her line `brs_1006`, 38 sound 132 | 38 sound 132, 56 her line `brs_0021` |
| 92 | part modes 3 and 0, the launcher `chr_w_grn00` | the same |
| 96 | sounds 164 and 144 | the same |
| 152 | the camera and the stage back | the same |
| 180 | her line `brs_4300` | `brs_3800` |
| 192 | effect 998002 on her node 50, sound 175, she aims, the bomb is made (0x797A8: only if the bullet's class is 4; with her aim point of that moment, by 0x87454) | the same |
| 240 | effect 10112, part modes 3 and 5, sound 108, the clock starts over, the dimming goes back over twenty | the same |
| 20 more | she stops aiming, stands and can act | the same |

The bomb is the enemies' bomb with her for its owner (see "Class 4"): it leaves
her node 50 at 5 a sixtieth, and whatever she aims at it is thrown as near as a
bomb can be: its first hop, 300 high, ends 550 along the ground towards her aim
point and its second 650. It lies for 90 sixtieths, and its burst lasts 45 and
strikes at once and every 16 after, three times in all: each time every enemy
with a body sphere in use within 300 and that sphere's radius of it takes her
attack 37 (38), once, at the first such sphere of it. So it is for enemies that
come at her; those at their own places are some 1500 away.

Stun Snipe and G-1 Snipe (0x7FC90, 0x80180). The dispatcher gives the state
what the cursor has settled on (which may be nothing) and does not have the
slot start over. The state keeps it (+ 0x98) and whether it has fired (+ 0x9C).

| Sixtieths | Stun Snipe (`btl4001`, `chr_w_sni00`) | G-1 Snipe (`btl4001`, `chr_w_sni01`) |
| --- | --- | --- |
| 0 | the log's line, the stage, dimmed, the camera, her animation, part modes 3 and 0, the rifle at her node 0x50; effects 10020 and 10021 | the same; effects 10030 and 10031 |
| 10, 36, 50 | sound 132; her line `brs_0001`; sounds 164 and 144 | her line at 42 |
| 102 | the camera's move is over, but the stage is kept: the enemy the state has is shown again (0x7F6A4), her + 0x73C is set (0x7F9B0: her own models are not drawn, 0x9A314), and the sight comes (0x7F86C); the clock starts over | the same |

0x7F86C: the battle's display makes its cursor's entry and its snipe part if it
has none (0x35B94; the snipe part is 0x41E18 with the battle's pack) and the
root's + 0x194 notes that; her + 0x7F0 is set; the display goes to its snipe
mode (0x35C48 with 1: the cursor's entry is put out of sight with 0x3E680 and
the snipe part started with 0x422EC); the root's camera becomes kind 2
(0x8BE24), which is told where the camera is now (0x8C788) and, if there is an
enemy, that enemy's lock point's place (0x8C7E4).

While her + 0x7F0 is set her frame runs another routine for the cursor
(0x9A018: 0xA1A5C in place of 0xA1CC8). In it what would move the cursor is
nought: the cursor stays where it is, and the camera is told where that is on
the screen (0x8C81C).

From there, on the clock that started over:

- once it has come to the battle's value 77 (10): if the enemy has a body
  sphere in use (0x7F6DC) the sight starts to lock on (0x35D5C, which is 0x42380
  of the snipe part); otherwise the log has line 8 with her name (0x7FA04);
- once it has come to value 76 (10), and with such an enemy to values 77 and 78
  together (20), the clock starts over again and the window to fire is open;
- in the window (phase 7), until she has fired: if square has just been pressed
  (the pad's word of buttons newly down, 0xAB280 of 0xAB298, has bit 0x200),
  then, with such an enemy, the skill's slot starts over (0x9F05C, 0x9EE64 by
  the skill's number, 0x9ED3C) and the enemy is hit (0x7F7B4); either way she
  has fired;
- the window closes when the clock has come to 120, or before that if cross has
  just been pressed (bit 0x100): the sight goes (0x7F964: her + 0x7F0 cleared, camera
  kind 1 again, the display out of its snipe mode), the stage is given back,
  her + 0x73C is cleared, part modes 3 and 5, she stops aiming, the clock
  starts over and the dimming goes back over twenty; at 20 she stands and can
  act.

So a snipe that is not fired costs nothing: its slot never starts over. Nor
does one fired at nothing, which the log's line 8 has said: "Snipe Lock
failed!".

The hit (0x7F7B4) is one call of 0x92510 on the lock point's enemy with her
attack of the skill's field 17 (26 for Stun Snipe, 27 for G-1 Snipe) and the
skill's record: at the sphere the lock point names (its + 0x10) and wearing
the gauge that sphere has (0x91A5C), at the lock point's own place. Every
other hit of hers wears an enemy's first gauge (the hit's third word is the
gauge's number, which 0x928C8 takes); this one wears the part she had locked
on to.

The values (`btl_system.bin`): 3 "sniper lens size" 200, 4 "sniper enter frame"
30, 76 "snipe ui enter frm" 10, 77 "snipe ui lock start frm" 10, 78 "snipe ui
lock wait frm" 10; and 87 "blade hit pos y limit" 330, which is the blades'.

Camera kind 2 (0x8C844) keeps the eye and the point looked at of the camera
over her shoulder and changes the lens. With `d` the distance from the eye to
the enemy's lock point, the lens it makes for is `2 atan(200 / d)` (15 degrees
with no enemy): 400 units high at the enemy. With `a` the share of value 4's
30 sixtieths that have passed and `b` that of the 120 after them, the lens is
the usual one moved towards that one by `1 - (1 - a)^3 + 0.01 b` (`b` is
counted from before the 30 are over, so the lens starts a hair wider than
usual). Two figures it keeps beside (+ 0x15C, + 0x160) are how far the cursor's
place on the screen (+ 0x154, + 0x158) is from the screen's middle, taken to
the new lens (0x8BA6C, 0x8BAC8: a length on the screen is `2 z tan(fov / 2) x
pixels / 272` in the world and back) and eased by `1 - (1 - a)^3`. They are
added to the middle of every camera of the scene (0x8BEEC, 0xA30C8), which
moves the picture the other way: what the cursor is on comes to the middle of
the screen, where the sight is. It also sets three depth figures from the
enemy's distance (0x19B68, 0x19BC8, 0x19C28), which have not been read.

The sight is the display's snipe part (0x41E18): a layout of its own, the
battle pack's files 7 to 9 (`if_btlsys.lpk/007.sdoa`) with file 10 for its one
texture, playing four sets: 0 as it comes (0x422EC, with sound 0x7B of the
common bank), 2 as it locks on (sound 0x2F), 3 as it lets go, which nothing
asks for, and 1 as it goes (0x42340), after which it is done with. Each is
played over what the others left (0x319BC). It is drawn first of all the
display (0x356D8), under her gauges: every track in turn (0x4217C), but group
9 only until it has locked on and groups 10 and 11 (the marks about the target
and the word SHOOT with square's sign) only once it has. Group 0 is the lens,
which is not drawn from the layout but made of corners (0x423AC, 0x41F8C):
about the middle of the screen a round of radius 100 times the track's scale
along x, which the coming takes from 2 to 1, in (5, 150, 245) at 60 of 255 of
the track's alpha; and from that round out to 300, all of the screen, in the
track's colour, which is (0, 0, 10) at an alpha that comes to 180.

A held enemy is not only kept from acting: its update is passed over whole
(0x92048), so its animations stand too. Under the scope that is plain to see.

#### Those that act of themselves (kind 4)

A battle keeps a list of them (0x1712E8), made as it is set up (0x9F79C): for
each number from 1 to the table's count, a skill she has (flag 7000 + n,
0x14D940) and has not put aside (flag 7200 + n, 0x14D978) whose kind is 4. Each
node goes in front of those before it, so they are asked from the highest
number down. A node has the next, the skill's record at + 4, flags at + 8 of
which bit 0 is "is acting", and a frame number at + 0xC.

Whenever something happens that one may act on, 0x9FF58 is handed an event (0x2C
bytes, cleared by 0x9F77C: its number at + 0, how many began to act at + 4, how
many answered at + 8, HP to give her at + 0xC, a flag at + 0x20, another at
+ 0x24 with a figure at + 0x28) and has every node look at it, by the record's
field 7 (0x12AE18); then 0x9F9E0 carries out the event's HP. What else comes of
it is the asker's to carry out. The record's fields 4 to 8 are when it acts
(0x12ADB8), the condition and its figure (0x12ADD8, 0x12ADF8), what it does
(0x12AE18) and with what (0x12AE38).

The condition (0x9F49C): 1 a chance, `rand(100) < figure`; 2 her HP at just that
many hundredths of all of it; 3 at no more than that many; 4 nothing at all.

None of them is ever named in the log. Each that begins to act calls 0x9F654,
which asks the record for its line by 0x12B1AC, and that gives one only for a
skill of a slot: the lines 39 to 47 of `btl_msg_tbl.bin`, which the records
name, are never read. What is heard and seen of one that begins to act is sound
0x82 of the battle's bank and effect 998045 on her (0x9F6D8), which the asker
calls when the event's count is above nought.

| Skill | Fields 4 to 9 | When | What |
| --- | --- | --- | --- |
| 30 Aftercare, 31 Aftercare + | 7, 1, 20 (30), 1, 25 | a battle won, 20 or 30 in 100 | she has a quarter of all her HP back |
| 32 Max Up | 8, 2, 100, 2, 125 | never | (more experience) |
| 33 Rocket Starter | 1, 1, 25, 6, 1507 | the fight begins, 25 in 100 | status effect 1507: her attack up by 20 in 100 for 450 sixtieths |
| 34 Dead Guard | 2, 3, 25, 6, 1608 | any frame of hers with her HP at a quarter or less | status effect 1608: her defence up by 20 in 100 for 1800 sixtieths; once in a battle |
| 35 Six Pocket | 5, 1, 30, 3, 150, 200 | she uses an item, 30 in 100 | what the item gives back in HP is half as much again |
| 36 Attack-Free | 3, 1, 10, 4 | she begins a burst, 10 in 100 | the burst adds no heat |
| 37 Danger Guard | 6, 4, 0, 5, 501 | a hit on the second frame after she began to guard | her guard is that of status effect 501 for that hit |
| 38 Sway Three | 4, 1, 10, 4 | she begins a dodge, 10 in 100 | the dodge adds no heat |

The events, who raises them, and what the routines make of them:

- **0, the fight begins** (0x93FA0, just before the root's flag 8 is set), and
  **1, a frame of hers** (0x99FD4: in her update, after her status effects have
  run, so only while the battle is on, she lives and nobody has the stage).
  Field 7's 6 (0xA0054): a skill whose field 4 is 1 on event 0, or 2 on event 1,
  that is not acting and whose condition holds begins to act, for good, and
  gives her the status effect of its field 8 as she would give it herself
  (0xA00F8: 0x8FC10, 0x8FE28). Rocket Starter's 1507 has no sound of its own
  (its record is not "shown"); both have the effect's own line in the log
  (1507's is "Attack has increased.").
- **2 and 3, her guard begins and ends** (0x7D990 in the guard state's
  constructor, 0x7D9F4 in its destructor), **4, a hit lands on her guard**
  (0x9AEC8) and **0xF, which guard is hers** (0x9AF00, at every hit). Field 7's 5
  (0x9FDF0), for a skill whose field 4 is 6: on 2 it keeps the battle scene's
  frame number (0xA6E28: the object at 0x171340, + 4, which 0xA6B48 counts up
  once a frame and the states' clocks go by, 0x74418); on 3 it stops acting; on
  4, if the frame is not the one kept and two it stops acting, and if it is and
  the skill is not acting and its condition holds it begins to; on 0xF, if it
  is acting, the event's + 0x24 is set and its + 0x28 is the skill's field 8.
- **5 and 6, a burst begins and ends** (0x7DF60, 0x7DFF8), **7 and 8, a dodge**
  (0x7DB74, 0x7DC0C), and **0xC, is it to cost no heat** (0x7DF98, 0x7DBAC).
  Field 7's 4 (0x9FC94): on 5 a skill whose field 4 is 3, and on 7 one whose
  field 4 is 4, begins to act if its condition holds; on 6 and 8 the same skill
  stops; on 0xC, if it is acting, the event's + 0x20 is set. The constructor of
  the state clears its own flag (+ 0xA4 of a burst, + 0x9C of a dodge) and sets
  it if the answer is yes, and the state then adds no heat as it starts. The
  question does not say which of the two it is about, but the state before is
  always done with first (0x9AC4C, 0x9AB84, 0x9AA54: its destructor runs, and
  only then is the new one made), so a skill that acted on a burst has stopped
  by the time a dodge asks.
- **9, 0xA and 0xE**, as she uses an item (0x981BC), for field 7's 3 (0x9FB6C):
  Six Pocket. On 9 it begins to act if its chance has it; on 0xE, asked with how
  much HP the item is about to give, it adds what that amount by its fifth figure
  in a hundred is more than the amount; on 0xA it acts no longer.
- **0xB, a battle won** (0x9DA70, in the second state of `BtResult`: see "The
  results of a battle won"). Field 7's 1 (0x9F8D8): a skill whose field 4 is 7
  and whose condition holds is counted, every time, and the HP it gives
  (0x9F5FC: so many hundredths of all her HP, its field 8) is the event's if it
  is more than the event has already. With both Aftercares each has its chance,
  and she has the one quarter back. 0x9F9E0 then plays sound 0x82, has effect
  998042 where she stands and gives her the HP (0x98B8C). If any was counted
  the battle's end waits sixty sixtieths from there.
- **0xD, the experience of a battle won** (0x9DACC, the same state, with the
  experience earned at the event's + 0x18), for field 7's 2 (0x9FA98): what the
  routine gives back at + 0x1C is added to the experience. It wants a skill
  whose field 4 is 7, and Max Up's is 8: nothing ever has it act.

**Her guard's record.** A hit on her (0x9AD90) looks up a record of the status
effects for her guard, of kind 5: record 500 while she holds her guard up
(0x9AE9C), whatever the attack, and after event 0xF the record a skill names,
if one is acting: Danger Guard's 501. Such a record has the hundredths of the
hit it keeps off her (0x129A98: its first figure, 0 to 100) and a status effect
(0x129B14: its fourth figure); the two figures between are read by nothing.
With a record, the effect it names, if that is one that gives HP back (kind 1),
is given to her, by her (0x9AF50), before the hit takes anything; and the
damage is taken as so many hundredths less, besides what guarding does (the
battle system's value 54, when the attack is one that her guard stops). Record
500 is 0, 50, 0, 0: nothing more kept off, nothing given. Record 501 is 80, 75,
50, 101: four fifths kept off, so that a guarded hit takes a tenth of what it
would, and effect 101, a tenth of all her HP back. An attack that cannot be
guarded, or that breaks guards, is still met with the record: it takes a fifth
and she has the HP, and she reels.

In the port: `battle/auto.rs` (`Autos::ask` with `Happens`, `Answer`),
`Skill::auto` and `Skills::of_themselves` in `skills.rs`, `Effect::guard` in
`status.rs`; in `actor.rs` her frame, `enter_guard`, `spared_heat` for a burst
and a dodge, `begin` for the ends and `take_hit` for her guard; in
`battle/mod.rs` `step` for the fight's beginning and `conclude` for a battle
won. `--auto 33,36,37` has her have some in a run whatever the save says. Run
unseen: Rocket Starter against group 1 (`docs/compare/skill_rocket_starter.png`:
the log's line and the ATK mark), Attack-Free and Sway Three over fourteen
bursts and dodges (three spared), Danger Guard with a guard begun two frames
before a bullet lands (`--script "wait:3.7,cross:2"` against group 2: "her guard
be that of status effect 501", the hit takes 0 for 1), Aftercare after a battle
won at level 50 (616 of 2465 HP). Six Pocket and the two effects (998045,
998042) came with her items (`PcActor::use_item`, `auto::EFFECT`).

Three more records sit with these in the table. 39 Treasure Hunt and 41, which
has no name, are of kind 3 and add to figures that her four are not (figure 6
by 25, figure 5 by 50), and 40 Stealth is of kind 0. What reads those has not
been looked for.

| Skills | Clip | Weapon | What it does | Last wait |
| --- | --- | --- | --- | --- |
| 3 Photon Shower | `btl4010` | | hits the enemies itself (0x74EF0, which calls 0x92510) while she has the stage | 520 |
| 6 BB Genocide | `btl4009` | `chr_w_bla00` | the same | 400 |
| 4 Blade Kill, 5 Iksa Blade | `btl2000`, then `btl3100` | `chr_w_bla01`, `chr_w_bla02` | hits what is locked on to (0x778F8), which they must have | 200, and 20 more |
| 7 Stun Snipe, 8 G-1 Snipe | `btl4001` | `chr_w_sni00`, `chr_w_sni01` | the sight and the camera that closes in (0x7F86C: 0x8BE24, 0x8C788) on what she had locked on to, if anything, and one hit on that part of it if square is pressed in time | 102, then its own clock |
| 10 Vulcannon, 11 Volcane, 12 Vortex | `btl4002`, `btl4003`, `btl4021` | `chr_w_smg01`, `chr_w_gtr00`, `chr_w_smg02` | gives the stage back at 128, 120 or 122, then rays one after the other (0x7C158 makes one with 0x86C2C, six sixtieths apart) | then its own clock |
| 14 Tracer Gun, 15 Chaser Gun | `btl4004` | `chr_w_sho00` | gives the stage back at 162 and at 200 fires as Charge Shot does (0x7ABB8), a bullet that is guided | 280 |
| 18 Star Comet, 19 Star Comet G | `btl4005` | `chr_w_grn00` | gives the stage back, then a bomb (0x797A8 makes it with 0x87454) | 240 |
| 20 Steel Cutter, 22 War Hammer | `btl4006` | `chr_w_swo01`, `chr_w_ham00` | hits the enemies itself (0x847C0) | 300 |

While the state runs a hit takes HP off her and does not make her reel: the lock
at her + 0x754 is set for a skill as it is for a dodge, and the end of a skill
does not clear it (0x9AD90 lets her reel only without it). So a skill is not cut
short, and what it dimmed is always lit again.

Checked against the walkthrough, where Charge Shot's scene begins at 2436.5 s and
its shot leaves at 2439.0 s, a hundred and fifty sixtieths on
(`docs/compare/skill_charge_shot.png`: her pose and the framing half a second in,
the muzzle a second and a half in, and the battle with the shot in it).

In the port: `battle/skills.rs` has the table (`Skills`), what the skills add
to her figures (`Skills::bonuses`, taken by `PlayerStats::take_bonuses`
whenever the save's flags have changed), her slots in a battle (`Slots`: made
by the display's `begin`, a frame of them in `charge`), the scenes of the
thirteen and of the seventeen attacks as lists of steps with their times (`scene`,
`Step`: one step for each call of the original's update) and what each is
played with (`Stagings`, loaded with the battle for the skills in her slots:
the animation and the camera's move, of both parts if the scene has two, and
for an attack its bullet's record, the weapon she takes and the attack it
strikes with). `battle/hud.rs` shows the slots in the
column, with their names (`docs/compare/skill_column.png`, against the
walkthrough at 2436.75 s). `PlayerStats::skills` are her slots, which the
challenges' script reads and sets as it makes a skill better, and
`--skills 25,24,0,0` fills them for want of the menu.

`PcActor::choose` reads the buttons (`can_use` is 0x99050) and `run_skill` runs
the steps (her action is `Skill`, or `SkillShot` for one that shoots, which is
what an enemy's sense 21 asks after); the battle's `step` carries out what they
ask for: the camera move and its end, the near and far planes, the dimming, her
line (the archive's `.at3`), a sound of the archive's bank, the effects given,
and the bullet, which is made at her node as `projectile::SkillShot` and flies
and hits in `enemy::settle` from the frame after. Her hands change by the part
modes (`ShownParts::set_mode` in `player.rs`), and what she holds is the
`Arsenal`'s to put in her hand (`wield`): the cannon she wears, or a skill's
weapon at the node that goes with it (`Step::Weapon(node)`). A blow that lands
on every enemy, or on the one a skill is for, is asked for through
`skills::Strikes` and lands in `enemy::settle`, held as the enemies are; who of
them is seen again is `PcActor::shown` (everybody, or that one). A skill that
is for one enemy (`Scene::aimed`) begins only with a lock the battle's `step`
hands her state (`LockOn`, from `Horde::lock_on`), and keeps the enemy in its
state. Where a scene puts her model is `PcActor::placed` (`Step::Put`), which
`PcActor::transform` gives in place of where she is until the stage is given
back; the enemies' places come from `Horde::places`. A volley is two steps
(`Step::Volley`, `Step::Reload`) that send the state back and forth as phases 10
and 11 do, with the rays left kept in the state and the state's clock started
over by each (`Step::Restart` is the plain "clock to nought"); each ray is asked
for with `Event::SkillRay`, made by the battle's `step` as a `projectile::Ray`
of hers (`SkillRays`) and lands in `enemy::settle` that same frame.
`Aim::free` is the flag that lets the cursor go of the lock points. A bomb of
hers is the `projectile::Bomb` the enemies' are, marked with its skill
(`Bomb::hers`, thrown by `Step::Fire(Firing::Lobbed)`); `enemy::settle` has its
burst strike the enemies. A snipe (`Scene::scoped`) takes her lock as it is
and waits on the battle's own figures and on the pad in three steps of its own
(`Step::SightLock`, `SightWait`, `Trigger`); `battle/scope.rs` is its scope:
the lens of the camera that closes in and how far the picture is moved, which
the battle's `step` gives both cameras as a `SubCameraView`, and what the
sight is playing, which `hud.rs` draws from the sight's layout with the lens
made of corners. Her model is hidden meanwhile (`Step::Hide`), the cursor is
held (`PcActor::scoped`), and the blow is `Blow::Part`: at the sphere of her
lock point, wearing that sphere's gauge. A held enemy's animations are paused
(`enemy::begin`). A hit of a
skill tells the records which skill it was (`Turn::skill`, as 0x92C34 does),
whatever the hit was made by. While she has
the stage nothing of hers runs on or out (0x99EF8), and her skills do not come
nearer to being ready.

`battle/dimmer.rs` is the sheet. A battle is drawn by three cameras, one after
the other, as the original draws its lists: the map's (layer 1), the sheet's (a
2D camera, which draws only while there is any of the sheet), and the battle's
own, which keeps the depth the first left so that what is nearer of the map
still hides what fights. The port has one near and far plane for all of it
(Bevy's far plane only culls whole meshes, and the sky's dome is far beyond any
near plane).

A camera move can cut to another shot: Defender's goes from all of her to her
head a second in (frames 58 to 60, 251 units in one step, then the lens closes
from 60 to 100 mm). The original shows a move frame by frame; the port draws
between its logic frames, and would show the camera on its way from the one
shot to the other for a sixtieth. `CameraClip::cuts` tells a cut by the camera
going, or turning, several times as far as just before or after, and the battle
snaps the camera there.

Checked against the walkthrough (`docs/compare/skill_defender.png`, from 2476.67
s): the framing half a second, a second and a second and a third in, the dimmed
map, the log's two lines and the DEF mark. Tracer Gun, Chaser Gun, the four
that strike outright, the two blades, the three that fire a volley, the two
that throw a bomb and the two snipes have only been run in the port
(`docs/compare/skills_that_strike.png`, `skill_blades.png`, `skill_volleys.png`,
`skill_star_comet.png`, `skill_snipes.png`): where the walkthrough uses them
has not been looked for. Every skill of her slots is ported. Not ported: the
enemies' marks while she has the stage and some of them are shown again (the
port draws none then), the depth figures of the scope's camera, Six Pocket of
her skills that act of themselves, and
the scenes' effects (`.efp`: 10120 and 10121 for
Defender's shape, 10170 and 10171 for Skill Boost's, 10001 and 10003 for Charge
Shot with 10004 for its bullet, 10011 to 10013 for Burst Shot): a plain ball
stands in for a bullet's.

## How a battle begins and ends

The root (`BtRoot`, the global at 0x2EAF0) has a state of its own at +0x7C, run once
a frame by 0x8D624 unless the battle is paused:

| State | What happens |
| --- | --- |
| 0 | wait for the loading to finish, then build the stage (0xA0A18) |
| 100 / 101 | make `BtEnter` and wait for it |
| 200 | the fight: wait for root + 0x88, "the battle is decided" |
| 300 | clear root flag 8, send every enemy signal 3 (0x91B58), make `BtResult` |
| 301 | wait for it; then, if the player has no HP (0x8D358), make `BtGameOver` |
| 302 | wait for that; then the root removes itself and the battle is over |

Other fields of the root: 0x84 a set of flags (8: the fight is on, set by `BtEnter`
and cleared when the battle is decided; 9 and 0xC: a staged moment is playing; 0xB:
the display is on), 0x8C / 0x90 the battle's time in seconds and sixtieths, 0x94
experience won, 0x9C the damage she has taken (0x98D30), 0xA0 the damage she has dealt
(0x92844), 0xA4 the enemies beaten, 0xA8 / 0xB8 four kinds of item dropped and how
many of each, 0xE4 the player, 0xE8 the first enemy (each links to the next at
+0x214), 0xEC / 0xF0 the stage's radius and arc, 0xD4 the cameras.

`BtEnter` (0x93CBC, update 0x93D9C): turns the display on (root flag 0xB), lets
the encounter finish (below), then sets the AI flag of every enemy (0x93F0C),
starts the skill palette and the pause handler and sets root flag 8. A group with
flag 4 skips the encounter and flag 8, which something else then has to set: those
are the boss battles. The members a group starts with are built with signal 1 and
their AI off; enemies that come later, as waves, with signal 0 and their AI on.

### How the screen comes to a battle

When a battle is asked for the field has a picture taken of the screen with all
it has on it, to be shown as it is (0xD876C: field + 0x1E34 and flag 0x17; see
`FIELD_NOTES.md` for the picture). Once it is taken (0xD8890) the map's sounds are
silenced, the encounter is started with the enemy group (0x11ACDC) and the battle
begins to load. The encounter is an object of the field's (at 0x172C30; its frame
0x11B3AC, its draw 0x11B588 on the layer of the screen's fade, so over the
battle's display) that goes by one of two records (0x172C10), the second for the
groups she strikes her pose over (the group's flag 2):

| | most groups | those groups |
|---|---|---|
| its sound, of the bank that is always there | 0x2B | 0x2C |
| frames the field's picture stands first | 0 | 134, while an effect of the field's plays over it (file 7 of the field's own pack) |
| then | the picture stays (the fade's kind 0x804) | the screen is white at once (0x12) |
| when the battle says it is ready | the picture goes over 30 frames of the interface (0x803) | the white comes off over 30 (0x11) |
| the encounter is over | when the picture has gone | at once |

The battle's own object of the name (`BtEncountFade`, 0x906E8) waits for the
standing to be over, then for `BtEnter` to say go (0x90800), tells the field's
object the battle is ready (0x11AEA8) and is done when that is over (0x11AE64).
So an ordinary battle comes up under the field's picture, display and all, and
the fight is on a frame after the picture has gone.

In the port: `start_battle` in `field/mod.rs` has the picture taken first;
`battle/encounter.rs` is the encounter (17 frames of the game for most groups:
nothing here takes time to load), drawn by `draw_fade` in `battle/mod.rs`, and
`Flow::advance` lets the fight begin when it is over. A battle run alone takes the
same frames with nothing over it; one against a group she strikes her pose over
begins as a battle of note does, with its effect over nothing. Seen against the
walkthrough: `docs/compare/encounter.png`.

**The effect of a battle of note.** The field's object makes it as it starts
(0x11AD3C: an effect of the eighth bundle of the field's own pack, by the record's
number, so effect 1 of `fld_resident.bin/03.bin/07.bin/03.bin/01.efc`, and only if
the record's fourth word is set: the second record's is). It is the library's own
effect, with no object of the field's or of the scenes' engine about it: each frame
the picture stands it has one frame (0x11B2A4 to 0x127BA0 with 1), and it is drawn
after the picture (0x11B6A8) by a camera of its own that looks straight on from
where the world begins, 45 degrees from top to bottom, from 1 to 20000 (0x11B164).
Eleven of its fourteen emitters are fixed to the screen and three to that camera:
lines that fly past, a lattice, a chequered sheet, and squares that fly apart. When
the picture has stood its 134 frames the effect is unmade (0x11AC70) and the
screen is white. In the port: `battle/encounter.rs` (`play`), with a camera of the
battle's that is the only one to draw while the effect plays, so that it is drawn
over nothing, and the canvas laying what it drew over the picture
(`Canvas::scene_here`).

Not ported: the way the picture is drawn while the effect plays, for the record's
third word (0x11AF48: not by the fade's own drawing, 0x298DC, but as a strip of
triangles over the whole screen from the picture kept, laid over what is there by
the picture's own alpha).

The stage (`BtStageActor`, 0xA0484, update 0xA05F0) counts the enemies each frame:
those in the list that are not being removed, not dying and not dead (0xA0BFC), plus
those still to come as reinforcements (0xA0D0C). With none of either the battle is
won (0xA022C: the result record at 0x2EAF8 gets the time and three counters, root
flag 8 is cleared and root + 0x88 set). If nobody is standing but some are still to
come for more than 600 sixtieths, it is given up as won too.

The battle is lost by the player's defeat state, below. A player without HP at the
end means game over even if the last enemy fell too.

### A battle lost

There is no screen for it in the battle. `BtResult` runs as after a battle won,
but everywhere it asks whether she has no HP (0x8D358): her last state is not
started, nothing is added up, and the display goes once the log has said its last;
her HP is kept (none), and the results are made all the same. The results' task
then plays no tune, goes from its panels having come (eight frames of their
layout) straight to the challenges (state 6) and on to having told all, and its
picture draws nothing of its panels and rows (0x36714, 0x36B0C). `BtResult` does
not wait for a button (0x9DCAC), asks for the picture of the screen and starts the
way back as after a battle won (0x9D758, below).
Then the root makes `BtGameOver` (0x94988, update 0x94A40), which waits for the
log and for 0x14B29C, has the tune fade out over 60 blocks (0xA10C), waits for the
screen's fade to be over (0x29948), keeps her HP as none (0xF44) and ends the
battle. What is shown of a mission failed is the field's doing (`FIELD_NOTES.md`,
"A mission failed").

In the port (`conclude`): the display goes (`Conclusion::display_gone`), 11 frames
of the game go by for all the above, the battle gives way as after one won, the
tune is asked to fade and the battle is over.

### The way back from a battle

Won or lost, a battle's end asks its main object for a picture of the screen
(main + 0x48: 0x73DA8 takes it, and from then on the main object draws the picture
the fade keeps, 0x73DD0), waits for it and then (0x9D758):

- for a group she strikes her pose over (0x9D594), the screen's fade goes to
  black over 30 frames of the interface (0x2983C with 2), and the battle waits
  for that: the field comes back dark;
- for any other, the picture is put up whole to stay and the fade's kind is made
  0x804 (0xE6B28; the 0x802 that is asked for first starts nothing): the field
  comes back under the battle's last picture, with its own interface over it.

Either way the field's script then brings the screen back by the kind last used
(`s7(0, -1, 30)` in the library's battle functions): from black, or by the
picture going over thirty frames of the game, which is the results of a battle
dissolving into the field.

In the port: `WayBack` in `battle/mod.rs` (the picture is asked for and waited
for, or fifteen frames of dark), `Conclusion::picture`, and `end_battle` in
`field/mod.rs`, which puts the picture up (`Fade::hold_picture`) or leaves the
screen dark. Seen against the walkthrough: `docs/compare/battle_to_field_cmp.png`.

### The results of a battle won (`BtResult` 0x9D7C0, `ResultBtlTask` 0x3814C)

`BtResult`'s own states (0x9D988), a frame each unless it waits:

| State | What happens |
| --- | --- |
| 0, 1 | if she lives and the group is not one without results, her last state is started (0x999BC) |
| 2 | on the same terms her skills that act of themselves are asked what they make of a battle won (0x9FF58 with 0xB: Aftercare) and of its experience (with 0xD: what comes back is added to root + 0x94; nothing ever does) |
| 3 | waits: sixty sixtieths if one of those skills acted, then (on the same terms) for her last state to be over (her + 0x774), for the log to have said its last (unless the battle system's value 86 is set) and for 0x14B29C; then the display goes (root flag 11) and the column of her skills with it |
| 4 | her HP is kept (0xF44); unless the group is one without results, a picture of the screen is taken and the results are made (0x379F0) and started with the experience and the four kinds of item (0x9D8B0) |
| 5 | waits for the results to have told all (0x37BC4: their state 8), if there are any |
| 6 | if there are results and she lives, waits for cross or circle to be pressed (0x9D6A8); then asks the battle's main object for a picture of the screen (main + 0x48) |
| 7 | waits for that picture, then starts the way back (0x9D758: see "The way back from a battle") |
| 8 | waits for it to be over |

A group is one without results (0x9D5FC) when one of the enemies its battle
begins with (0x934D0, 0x937D8) is enemy 110 or enemy 122 (0x9D5C0). Each is
alone in the two groups that have it (122 in 9 and 1006, 110 in 29 and 1020):
the white, winged one, at level 35 and at level 55. A battle won against it has
no last state of hers, none of her skills asked, no results and so nothing
given for it; her HP is kept and the battle gives way at once. In the port:
`Group::no_results` in `battle/enemy/mod.rs`, read by `step` and `conclude` in
`battle/mod.rs`; the fights that have it are the bosses' (see "A boss's battle").

The results' task keeps (0x70 bytes): her level and experience as the battle
ended (+0x44, +0x30), what was earned (+0x48) and how much of it is counted
(+0x50), the experience shown (+0x34), the level the count is at (+0x40) and the
experience that level begins and ends with (+0x38, +0x3C: the level table's
records for the level before and for the level), a frame's worth of counting
(+0x4C: a thirtieth of the level's span, one at the least, 0x3795C), and the four
rows of item and count (+0x10). Its states (0x3821C):

- 1: for a group she strikes her pose over, if she has HP (the task's flag 8, set
  as the results are made, 0x37B68), the tune of a battle won is played (0x38288):
  the music is turned down to 1 of 127 at once (0xA204) and the first jingle of
  `jin_pack.bin` (file 29 of the common sounds; five seconds) is played on the
  first stream at 96 of 127 (0xABA8). Then 2. Whatever its state, each frame the
  task looks whether that jingle has begun and then whether it is over (0x387E0),
  and when it is the music goes back at once to the volume it had as the results
  were made (0x37B94 keeps it); and when the task is done with, as the battle is
  taken down (0x37D30), what the first stream is playing is stopped and the music
  goes back to that volume, whichever results they were.
- 2: waits for the panels to have come (set 0, eight frames); then 3.
- 3, the count: nothing to count goes on to 5. Cross pressed counts it all at
  once (0x383C0: the level for the sum, the band if that is a higher one, a tick,
  then 4). Otherwise, while the band of a level reached is still coming the count
  waits (0x3660C); a level just reached is taken as her level and its span as the
  new one; then a frame's worth is counted, no further than the level's end and no
  more than was earned, with a tick (sound 0xB) each frame; the experience shown
  is what she had and what is counted, a million less one at the most. When it
  reaches the level's end the band is shown or its figure started anew (0x365A4)
  with the two levels (0x36600) and sound 0x66. With all of it counted, or at
  level fifty: 4.
- 4: she is given the experience (0x37E24: the level's gains, record by record).
  5: she is given the items (0x37F44). 6, 7: the challenges the battle has opened
  or cleared are shown one after the other, each in a window (0x37F8C), once the
  panel that tells of a challenge is gone (0x37FD8); the results' own picture is
  not drawn meanwhile (0x37DDC). 8: all is told, once the last window has gone
  and no panel is up (0x3807C). See "The records (challenges)" in
  `FIELD_NOTES.md`.
- 9, 10: all of it going (set 3); nothing in the battle's end uses them.

The picture (0x361D8) plays four sets of `if_btlsys.lpk/006.lpk`, each with a
player of its own, and has five textures: the pack's two, and files 9 (the
figures), 6 and 20 (the icons) of the pack that is always there. Its draw
(0x366AC):

- Set 0, the panels. Group 26 is the bar, cut to how far the experience shown
  is into the level's span of its object's width (all of it past the last
  level). Group 28 is how much is wanting to the next level (nought at fifty) and
  group 31 what was earned: each figure from its cell of the figures' sheet (the
  first of the sheets at 0x171078), the highest first, a figure's width and one
  apart, each on the object of the group before it (27, 30).
- Set 1, a row, drawn four times twenty apart. With an item: group 14 its count,
  two figures at the least, ending where the object is and each on group 13's;
  group 15 its name (0x12C05C), written at the object with a shadow, twenty-four
  glyphs to a line; group 16 its icon (0x2D198): fifteen square from the icons'
  sheet, in cells of sixteen four to a row, by the item's kind (bits 21 to 25 of
  its flags: kinds 1 and 3 icon 4, 2 icon 5, 4 to 6 icon 9, 7 to 9 icon 6, 10
  icon 7, 11 icon 8). Without one: group 17 four times eleven apart, group 18
  twice (each on group 13's), group 19 once.
- Set 2, the band of a level reached, once there has been one. Group 4 is the
  level she has, in figures that start 17 further left for each figure past the
  first; group 3 the level she had, further left by that and by 11 for each of
  its own figures past the first; groups 5 and 6 (the word and the arrow) are
  moved by as much as what follows them.

In the port: `battle/result.rs`, made by `conclude` once her last state is over
and the log has nothing left to say, drawn by `battle/hud.rs`; compared with the
walkthrough in `docs/compare/battle_results.png`. She is given the experience and
the items as the results are made rather than after the count, which nothing shows.
The tune of a battle won is asked for as the results are made (`conclude`), the
music coming back with `Request::MusicVolumeBackAfter` or as the battle is cleared
away. The challenges are told of as the results are made, shown with
`records::Shown` and forgotten as the battle is cleared away (`clear`). Not
ported: states 0 to 3's waits other than those for her last state, the log and
the challenges' panel.

### Enemies that come in later

A group's waves are the enemies it keeps back. The stage has a record of them
(`BtStageActor` + 0x1E8, set up at 0xA08E8): which have come in, how many are left,
how many enemies stood at the start, a word of flags the scripts keep between them,
and the number of "towers" standing. A group with mode 0 never looks at its waves.

Nothing brings one in but the enemies' own scripts, with engine function 73 (first
place, last place, keep; 0xC06E8). Each enemy's script calls it as it dies, some
while they wait, and a tower when its AI says so. Whether the call is answered is the
group's mode (0x93108):

| Mode | A reserve comes |
| --- | --- |
| 0 | never |
| 1 | when a tower calls, while at least one tower stands and fewer than 9 enemies do |
| 2 | when nobody is left standing |
| 3 / 4 | while fewer than two / three stand |
| 5 | whenever fewer stand than did at the start |

A tower is an enemy whose `param1` record has kind 0x10 (the Generator); an enemy of
kind 0x12 may call one in at a place somebody already stands at.

Answering a call (0xC06E8): the group's places are taken in an order drawn afresh each
time (as many swaps as there are places, of two places picked at random), or only
those from `first` to `last` when `first` is not negative. The first reserve not yet
called that fits one of them comes in there. It fits when twice the place's size is at
least both the width and the depth its `.edx` gives (the u16 at 0x2A and at 0x36);
when the place is in the air if and only if the enemy flies (the `.edx` byte at 0x18
is 1); and when nobody with HP left has a home within 100 of the place (0xA0D50),
unless the caller is of kind 0x12. With `keep` the reserve is not marked as called.
A tower's reserves never run out: when it finds none it clears every mark and tries
once more. The new enemy is made at once (0xA0E50) with the wave's variant and way of
entering, is sent signal 2 after the signal 0 that builds it, and sound 0x6B plays; it
takes its first turn that same frame, after everybody who was there already.

What is left to beat (0xA0B7C): the enemies standing plus the reserves not yet called.
A group that began with a tower is different (the stage's flag 1, 0xA03C4): while a
tower stands the count cannot be told and the battle cannot be won, and once none
does the reserves no longer count at all. The towers count themselves out with engine
function 65 as they fall.

Verified in the port: group 405 (mode 5) played all seven of its reserves to a win,
and group 113's Generator called in an Eater.

`BtResult` (0x9D7C0, update 0x9D988): stops the pause handler; unless the player has
fallen, clears her "finished" flag and puts her into her last state (0x999BC, which
also runs her update once more at once); waits for a skill to finish its say (60
sixtieths) and for her last state to finish (her + 0x774); waits for messages and
fades; takes the display away (root flag 0xB); stores her HP; then runs the result
screen (the module at 0x379F0, given the experience and the four item drops) and
waits for it. `BtGameOver` (0x94988, update 0x94A40) is under "A battle lost", and
the results under "The results of a battle won".

### What a battle is worth

An enemy is beaten when its script says so: engine function 27 with 55 sets its
"dead" word (0x912A0), and the first time that happens 0x90B20 counts it:

- root + 0xA4, the enemies beaten, goes up by one;
- if the enemy's + 0x23A is set that is all: an enemy called in by engine function
  73 is worth nothing, however many a tower calls;
- its experience (the variant's record + 0x24, never below 0) is added to root +
  0x94. In six missions it counts for half, `experience x 50 / 100` in whole
  numbers: when variable 10004 is 5, 25, 45, 65, 85 or 102 (0x973B4);
- it may leave an item (0x94C54). A number below 0x7FFF is drawn (0x93A0) and
  divided by 32767; if that is below the variant's chance / 255 it leaves one, and a
  second draw against the second chance says whether it is the other item instead
  of the usual one. Item 0 is nothing. The item is counted in the first of the
  root's four places that already holds its kind, or else in the first empty one; a
  fifth kind in one battle is lost.

Two of her skills are asked about as the battle ends (0x9FF58 with 0xB and 0xD,
0x9DA60 and 0x9DAAC): the second can add to the experience. The port has no skills
yet, so nothing is added.

When the battle is won her HP goes back to the save data cut to a whole number
(0x9DC10), and the result screen gets the experience and the four item places
(0x9D8B0 to 0x37C24). The screen counts the experience up, a level at a time, or all
at once when cross is pressed (0x383A0); either way it comes to this (0x37E24):

- at level 50 nothing is taken;
- the level is the one that goes with the new total (0x14D240): level `n` while the
  total is at least record `n - 1`'s experience and below record `n`'s, and 50 from
  record 49's on. Reaching 50 leaves her with exactly record 49's experience,
  999999, which is also the most the counting ever shows (0x38644);
- for each level gained, the level's record is added to her four figures (record
  `n` when going up from level `n`: 0x12B3F0 HP, 0x12B410 attack, 0x12B430 defence,
  0x12B450 special attack; each through 0x14D2EC, which caps them);
- having gained a level she is given all her HP (0xFAC then 0xF44), except when
  variable 10004 is 66 (0x37F08);
- then 0x14B908 is called, which is not read yet.

The items are added to what she carries afterwards (0x37F44, with 0x14D594 for each
place): see "Items and equipment".

The level table's first records: level 2 at 16 experience (+62 HP, +1 attack, +3
special attack, +1 defence), level 3 at 119, 4 at 374, 5 at 843, 6 at 1584. The
enemies of the first battle (variant 10 of enemies 8 and 19, the first map's Eater
and Hornet, at level 1) are worth 2 each; the map's own Eaters (variant 0, level 2)
8 and its Hornets 6. The early enemies leave a Repairer 200 with chance 112 / 255,
48 / 255 of those a Repairer 500 instead; for some the second item is 0, and the
second roll then takes the item away. `tools_re/enemy_variants.py` lists every
variant's experience and items.

In the port: `Spoils` (`battle/enemy/actor.rs`) is the root's count, `PlayerStats::gain`
and `Levels` (`battle/tables.rs`) the level rule, and `Rewards::give` (`battle/mod.rs`)
what the result screen does to her. A line of text stands in for the screen itself:
it stays for 120 frames or until cross is pressed. Verified: group 2 pays 8
experience, and a new game's first Eater on the field (group 100, 16 experience)
takes her to level 2 with 434 HP, all of it given back.

The battle's own figures: its clock (root + 0x90 gathers the step and + 0x8C counts
the whole seconds, 0x8CF6C, while the battle is not paused), the damage she takes
(every loss of HP goes through 0x98C24, a hit's and a poison's alike, and is added
whole to root + 0x9C even when it is more than she has), the damage she deals
(0x92844, likewise) and the enemies beaten. When the battle is won they go into the
result record at 0x2EAF8 (0xA022C: + 0xC seconds, + 0x10 damage taken, + 0x14 damage
dealt, + 0x18 enemies beaten), from which the field takes them into the mission's
counts (`FIELD_NOTES.md`). Once the battle is decided (root + 0x88) neither she nor
an enemy loses HP to a hit any more, and nothing is counted (0x98C3C, 0x9283C). In
the port: `Flow::clock`, `PcActor::damage_taken` and `spared`, `Spoils::damage_dealt`,
and the `Tally` the battle's `Conclusion` hands over.

`BtFinish` (0x940C8) is not the end of a battle but a boss's finishing move: a gauge
from `if_btl_finish.lpk` that the player fills by hammering a button while it drains
(figures from `btl_finish_tbl.bin`), with sound 0x63 when it is full. See "A boss's
battle" below.

### A boss's battle

A group whose flag 4 is set is a boss's (groups 3 to 30 and 1000 to 1021), and
`BtEnter` (0x93D9C) does none of what it does for another: it does not have the
display come, does not let the encounter go, does not wait for it and does not
begin the fight. All it does is set every enemy's AI flag (0x93F0C), on its first
frame, and go. The boss's script does the rest, with the engine functions listed
under "Engine functions an enemy script uses". As Mefe's (`mee00.bms`) has it:

- signal 1 builds her and has her state be 1 (`START_PROCESS`: her archive of the
  scene asked for, animation `btl0000`, move 10000 for a frame, spheres on);
- the AI's first page finds its variable 24 nought, which means she has not been
  met, and has state 3, `CUTIN_PROCESS`: archives asked for (86), the stage taken
  (75), the dim off (109), she hidden (100) and shown again, a scene begun (34)
  with `chr_b_mee00_fld0000_cam.cam` and the animation `fld0000`, **the encounter
  let go (93 with 0) and waited for (94)**, her line on stream 1 with her mouth
  moving until the stream is silent (118, 106), the scene ended (35), the stage
  given back (76), the card (96, then sets 0 and 1 of it played and waited for:
  77, 79, 78), **the display (98)**, and **the fight begun (97)**.

So the screen comes to a boss's battle over the boss's own scene: the field's
picture, or the white of a battle of note, stands until the script has the camera
on the boss.

**Having the stage.** Function 75 sets the root's flag 10 and gives every other
actor of the list (the enemies and all that has been fired; not her) flags 0x10 and
0x20 at + 0x7C, which stop its frame and its drawing as 4 and 8 do (0x90FE0,
0x91094, 0x864A0, 0x86528). While the flag is set she has no frame but her
animations' (0x99EC4: 0xA7520 and 0xA7598 and nothing else, so no state, no
input, none of her status effects' time), she cannot act (0x98F28), the column of
her skills does not fill (0x9F2BC), the mark on what she is locked on to is not
drawn (0x8D248), and the enemy that has the stage runs without its gauges'
stun and without its status effects' time (0x92070 skips to 0x92124, as it does
while she has the stage).

**A scene** (34, 35) is played at the origin of the world, facing half way round:
the enemy's slot is put there by setting the movement block's own placement
(block + 0xE0) to that, and her model's first slot likewise, so the animations of
the two (hers through 45: `chr_p_brs02_btl9000_all.anm` and the like, of the
boss's archives) meet as they were made to. The camera move's player is the
actor's own (+ 0x144, with the node it plays in the frame of at + 0x148), as hers
is for her skills.

**The card** (`ifBtlCutinTask`, made at 0x331B8 from a layout of the boss's
archive: `CUTIN_STAGE0n.bin`, a folder with the layout's three files and its
textures) keeps a flag for each set of the layout. Its frame (0x33490) steps each
set whose flag is set (0x30C8C) and its draw (0x33520) draws them (0x30CD4), at
priority 0x300 like the battle's display. A set is waited for with 0x31C20.

**The sheet** (the object at 0x171318, made at 0xA2184): a rectangle over the
whole screen, drawn in the scene's draw list 6 with its colour (0xA249C). Asked
for with a colour it is all there; told to go over a time it is `trunc((1 - t /
T) x 255)` of 255 strong each frame and gone at the end (0xA2268). Mefe's finish
and her change of AI begin white and come out of it over thirty frames.

**The finish** (`BtFinish`, frame 0x946F4; its figures a record of
`btl_finish_tbl.bin`, looked up by its first word, 0x12A9CC). Function 83 picks
the record by the enemy's number (0xC0DB4, table at 0x16B348): 116 to 121 have
records 1, 4, 7, 10, 13 and 16; 98 to 109, who are those six met again twice
over, the two after each; 123 has 19, 20 or 21 and 111 has 22, 23 or 24 by how
many finishes there have been (the first again from the fourth on); anybody else
has record 1.

| Field | In all 24 records | What it is |
| --- | --- | --- |
| 0 | 1 to 24 | the record's number |
| 1 | 0 to 6 | whose it is: which name the gauge shows |
| 2, 3 | 10000, 0 | what the gauge holds, and what it starts with |
| 4, 5 | 250, 300 | what a press adds: by chance between the two |
| 6, 7 | 6000, 9000 | between these amounts of the gauge a press adds less |
| 8, 9 | 0, 10 | by so many hundredths at the first and at the second |
| 10, 11 | 10, 20 | what it loses each time it drains, just after a press |
| 12, 13 | 2, 8 | the frames to the next drain then |
| 14 | 5 | so many drains with no press between and it drains its worst |
| 15, 16 | 25, 30 | what it loses then |
| 17, 18 | 2, 3 | and the frames to the next drain |
| 19, 20 | 10, 30 | one time in so many the first drain after a press is this much |
| 21 | 13 to 52 | the frames a full gauge stays (nought for four of them) |

Its frame, while the battle is not paused: on **cross pressed** (0x10B1C, bit
0x4000) the gauge gains `between(4, 5)` less the hundredths `between(8, 9)` by
how far the gauge is from field 6 to field 7 (0x94228), and the count of drains
is nought again. The first drain comes after ten sixtieths; each (0x94308) takes
`between(lo, hi)` off, where `lo` and `hi` go from fields 10 and 11 to fields 15
and 16 as the count of drains goes from 1 to field 14, and draws the wait to the
next between fields 12, 13 and 17, 18 likewise. When the gauge is full: sound
0x63 of the battle's bank, the count of finishes done (+ 0x84) goes up and the
script's wait (84) ends; the gauge stays for field 21 and then all of it goes.

Its interface (`ifBtlFinishTask`, 0x33670) has six players of the layout, of which
four are used, each coming by one set and going by another (0x336BC to 0x337DC):
the words that tell her to press (sets 3 and 4), what a full gauge has (5, 6), the
gauge (9, 10) and what rides its end (11, 12). The draw (0x33C44): the first two
plainly; of the gauge's tracks, groups 10 to 16 are the seven names, of which only
the look's own is drawn (looks 0, 1, 3, 2, 4, 5, 6 in the groups' order), group 20
is the filled part of the bar and group 18 the rest, each cut (0x15B098) to its
share of the bar's own room with its pattern moved across (0x15AE38) by a share
that goes once round in 120 ticks, one way for the filled part and the other for
the rest; what rides the end is drawn moved along by the bar's width times the
share (0x321F8).

In the port: `battle/boss.rs` (`Show`, `direct`, `draw`; `Finish` is the gauge),
the script's side in `enemy/actor.rs`, the bosses' archives merged into
`tables::Archive`, `Flow::leave_to_script` and `Encounter::hold`. Run alone:
`--battle STG01_02 --group 3 --level 30`, and a script that presses cross for
the finish. Seen in `tools_re/out/fx/field/boss1_intro_sheet.png` and
`boss1_fight_sheet.png` (both taken on `STG01_03`, which is not her map).

**Where each is fought.** The field asks for a boss's battle through its script's
`func00_BATTLE_BOSS(group, map, ...)`, whose second argument the function never
uses (the battle's map is the field map's own, `lst_map[map].battle`) but which
says what was meant: group 3 on 102 (`MAP_B_STG01_02`), 4 on 202, 5 on 302, 7 on
503, 8 on 602, 9 and 10 on 603; 6 has a stale 102. Stage 6 also asks for groups
12 to 22, 29 and 30 (the bosses met again) on 601 and 603.

**From the field.** The first boss is met on `MAP_D_STG01_06` in mission 4 (the
scenario, variable 10004), where trap 503 is made only while variable 220 is 108
or 109; walking into it plays three story scenes (events 18, 676 and 703, the
last with the lines the walkthrough has) and then asks for the battle. Unseen:
`--map STG01_06 --variables 10004:4,220:108 --play-events --skip-messages
--skip-battles --fight 3 --level 30 --start 680,150 --script "wait:6,to680/-300:6,
wait:108,<90 x seek+square:0.1,seek:0.45>,<80 x cross:0.05,wait:0.05>,wait:12,
<5 x cross:0.2,wait:2>,wait:40"` (`--fight 3` has every other battle passed
over): the battle is asked for at about 110 s, the fight begins at 118.6 s, the
finish at about 166 s, the results at about 184 s, and back on the field the
story goes on (event 19) to MISSION COMPLETE at about 216 s and the scene after
it. Seen in `docs/compare/boss1_from_field_port.png` and
`boss1_end_from_field_port.png`: the scene before, the card, the fight on the
map the walkthrough has, the finish, the results, the scenes after.

The second boss is met on `MAP_D_STG02_07` (the rooftop): trap 500 at (231, 1,
1008), made while the scenario is at 24, 29 or 31, whose callback at 24 plays
`EV02_03_0200B` to `0203B` (her introduction, the aircraft, the talk of "Big
Snipe") and has `0203B` ask for the battle with group 4 on `STG02_02`. Unseen:
`--map STG02_07 --variables 10004:24 --play-events --skip-messages --skip-battles
--fight 4 --level 30 --start 354,1325 --script "wait:4,to231/1008:8,wait:230"`:
the scenes take until about 207 s, then the battle is asked for, her scene and
card follow and the fight begins at about 217 s
(`docs/compare/boss2_from_field_port.png`). Nothing of the port's own is warned
of on the way. With the fight fought too (`--level 50`, and for the script
`wait:4,to231/1008:8,wait:206,` then 400 times `seek+square:0.1,cross:0.05,seek:0.3,`
then six times `cross:0.2,wait:2,` and `wait:40`; the game's script has no way of
saying "so many times", so the text is written out, about 14,000 characters):
her copy is beaten at about 240 s, the finish begins at about 262 s (record 4)
and is full twenty seconds later with a press every 0.45 s, the results are over
and the field has the battle's outcome at about 302 s, and the story goes on
with the scenes after it (`docs/compare/boss2_end_from_field_port.png`).

The other five (2026-10-08, each run unseen with `--skip-messages --skip-battles
--fight <group>` to the fight, and without `--fight` for what follows a fight
won; nothing of the port's own is warned of on any of them;
`docs/compare/bosses_from_field_port.png`):

| Boss | Group, battle map | Met on | How | After it |
| --- | --- | --- | --- | --- |
| 3 | 5, `STG03_02` | `MAP_D_STG03_04`, scenario 43 with variable 220 at 103 or 104 | trap 500 at (-753, 0, -2830): `EV03_05_0500B` to `0502B`, the last asking for the battle | `EV03_05_0700B`, 220 = 105 and map 39 by its entrance 5 (the snowstorm) |
| 4 | 6, `STG04_02` | `MAP_E_STG00_04`, scenario 64 with 220 at 104 | the map's `INIT_EVENT`: `EV04_05_0200C`, `0400B` to `0403B` | 220 = 107 to 109 with `EV04_05_0500` and `0600B`, then map 41 |
| 5 | 7, `STG05_03` | `MAP_D_STG05_06`, scenario 84 with 220 at 102 | the map's `INIT_EVENT`: `EV05_03_0500B` | `EV05_03_0700B` and `0800C`, the mission cleared and the screen between missions |
| 6 | 8, `STG06_02` | `MAP_D_STG06_04`, scenario 100 with 220 at 103 | the pad 206 at (117, 10, -606) of the first floor, which takes her to the fourth (callback 206: the leap, then `EV06_01_0300B` and `0301B`); she comes into the map on the second floor, so pad 200 at (-991, -289, 2543) first | `EV06_01_0500C`, 220 = 104 and map 81 |
| 7 | 9 and then 10, `STG06_03` | `MAP_D_STG06_06`, scenario 101 with 220 at 101 | the map's `INIT_EVENT`: `EV06_01_0600B` to `0602B` (group 9) and `0750C` (group 10) | `EV06_01_0800B`, 220 = 102 and the last map (87, `e_stg00_05`): the end roll, `s205(1)` |

Against the walkthrough, the third (its fight is at about 13400 to 13500 s of
the video, the finish's "PRESS X BUTTON" at 13540, the snowstorm after it from
13700): `docs/compare/boss3_walkthrough.jpg` has a frame of the fight at 13420
s (top right) and `boss3_fight_port.png` the port's (`--battle STG03_02 --group
5 --level 21`, ten seconds in): the ruins of the battle's map, the pair with
"LV21" and "x2" over them, ENEMIES at the bottom left, her HP and heat gauges
and the column of her skills are as the video has them. The other four
likewise (`docs/compare/boss4_walkthrough.jpg`, `boss5_walkthrough.jpg` and
`boss6_7_walkthrough.jpg` are the video's frames, `bosses_fight_port.png` and
`boss7_first_fight_port.png` the port's, each `--battle <map> --group <n>
--level <the video's>` nine and fifteen seconds in):

| Boss | In the video | What is the same |
| --- | --- | --- |
| 4, Lilio | fight 18540 to 18740 s, finish 18780 | the hangar with its lamps and yellow rail, "LV28" and "x2", "Lilio >> Air Conquer" in the log as the port has "Lilio >> Speed has increased." |
| 5, Nafe | fight 22870 to 23020 s (the items' window at 22960) | the cage with its screens on chains, Nafe at LV32 behind her shield and the Mini Rabbits at LV26 with theirs, "5 ENEMIES" |
| 6 | fight 24380 to 24540 s | the red floor of six-sided tiles, the dark machine with its spikes, "LV34" |
| 7, WRS | first fight 24780 to 24940 s, second 25180 to 25340, finish 25420 | the floor among the stars with the dark round and its chains, WRS at LV35 with her wings and scythe, "WRS >> World Maker" and "Red Wind" |

What was looked at is the place, the enemies, their labels and the display;
not the timing of their moves.

For example `--map STG03_04 --variables 10004:43,220:103 --start "-753,-2300"
--script "wait:5,to-753/-2830:8,wait:200"`, `--map E_STG00_04 --variables
10004:64,220:104`, `--map STG05_06 --variables 10004:84,220:102`, `--map STG06_04
--variables 10004:100,220:103 --script "wait:5,to-991/2543:9,wait:0.5,cross:0.3,
wait:6,to117/-606:7,wait:0.5,cross:0.3,wait:200"` and `--map STG06_06 --variables
10004:101,220:101`. With `--play-events` the scenes before the third take more
than five minutes. A fight lost (the third, at level 50 with a script that only
shoots) ends in MISSION FAILED and, tried again, at the mission's start. The
stale "102" in the fourth's call does no harm: the battle's map is the field
map's own.

**Against the walkthrough** (the first boss, 2026-10-08). The video has the story
scene on the field until about 4211 s, the effect of a battle of note from 4212
(the lattice at 4213.0, the squares over the field's picture at 4214.5), the
white at 4216.0, her scene from about 4216.5 (4217.2 and 4218.2 in
`docs/compare/boss1_intro_walkthrough.jpg`), the card at 4219.2 with the scene
behind it by 4219.8, and the fight with its display at 4220.5: 8.6 s from the
effect's start to the fight, where the port takes 8.5. The port's picture of
her scene on that map (`docs/compare/boss1_intro_port.png`, lower left) has
her, the leaning building to the left and the tower to the right where the
video has them. The finish is near 4378 (the white it begins with) to 4388; it
has not been compared frame by frame.

All of it is ported: `tools_re/bscr_coverage.py` finds nothing that any of the 88
enemy scripts calls, starts or fires that the port lacks. The second boss is seen
in `tools_re/out/fx/field/boss2_fight_sheet.png` (`--battle STG02_06 --group 4`);
groups 5 to 10 are the other five and the last one's two forms, and run through
`tools_re/trial.py` without a warning of the port's own. Not done: the sky's own
near and far planes (71 with 0), which no boss's script asks for; and none of the
bosses but the first has been compared with the walkthrough yet (all seven are
met from the field, above).

Four battle maps (`MAP_B_STG02_06`, `STG03_04`, `STG04_03` and `STG06_04`) have
their script as `btlmap02.bms` and not by the map's own name: the port takes the
archive's one `.bms`, whatever it is called.

### The sound an effect is given with

As an effect takes (0x8FE28, after the chance), 0x8FC80 shows something over its
bearer and plays a sound of the battle's bank at full volume, if the effect's record
has it shown (field 12). Both go by the effect's kind, from a table of 24 records
at 0x169278 (kind, sound, class): the class picks what is shown (1, 2 or 3; other
ones for the player and for an enemy: 998042, 998043, 998041 and 998035, 998036,
998034), and with class 0 nothing is shown and the routine leaves before the sound.
What is shown is an effect of the battle's own (0x8DB5C), put where its bearer's
first model stands as it is given and left there (0x98AD0 for her, 0x91694 for an
enemy). So: kinds 1 to 4 and 20 (heal, cure, regeneration) are class 1 with sound
0x82, kinds 16 to 18 (the boons of defence) class 2 and kinds 15 and 19 (attack up,
boost) class 3 with 0x84, kind 22 (no heat) class 2 with 0x8C; the table's sounds
for the ailments (0xBB for poison, 0xBA for kinds 7 to 10, 0x86 for 11 to 14), a
ward (0x70) and the unseen (0x7B) are never played by it. In the port:
`status::sound` and `status::burst`, kept in `Status::heard` and `Status::seen` as
the effect is given and taken by the battle's systems.

### The player's last states

All three lock every action. The first two are started by 0x9996C when the battle is
won, by the group's flag 2 (0x93154); the third by the choice of state when a state
has ended with no HP left (0x9A4B8).

| State | Ctor / update | What it does |
| --- | --- | --- |
| Standing down | 0x788C4 / 0x7896C | Unless `btl0000` is playing already, blends to it over 8 frames (after an 8-frame hold) and loops it. After 20 sixtieths sets her + 0x774. Does not clear "aiming", so the bend of her upper body stays |
| Victory pose | 0x78A9C / 0x78B44 | Takes the stage (below), plays the camera move `chr_p_brs02_btl8010_cam.cam` and the animation `btl8010_all`; after 60 sixtieths sets her + 0x774 |
| Defeat | 0x78C94 / 0x78D38 | Takes the stage, plays `btl8020_cam.cam` and `btl8020_all` and the voice `brs_0201.at3` (`brs_0200.at3` once save flag 6014 is set); after 180 sixtieths the battle is lost: result 2, root flag 8 cleared, signal 3 to every enemy, root + 0x88 set |

A state's clock (state + 0x84) is set to zero by its first update and grows by the
step before each later one (0x74418); 0x744B0 asks whether it is still short of a
time.

"Taking the stage" (0x787D4) is what every staged moment starts with: her state kind
becomes 1 and "aiming" is cleared, root flags 0xC and 9 are set, her + 0x7FC is set,
every actor in the list gets flags 4 and 8 at +0x7C, and her own collision spheres
are switched off (0xA7BA4). Flag 4 stops an enemy's or a bullet's update (0x90EE4,
0x863D0) and flag 8 its drawing (0x9107C, 0x8651C), so everyone else freezes out of
sight and nothing can hit her.

### Camera moves (`.cam`)

An actor can play a camera move (0xA7DA0: file name, slot and node of its own to
play it in the frame of; -1 for the model itself). Each frame its camera player
(built at 0xA3E70) is advanced by the step along with its animations (0xA7598), and
0xA7678 switches the root to camera 3 and has the player set that camera (0xA400C):

- the move's rig is posed by its animation, which plays once and stays at its end;
- the camera node's matrix is taken with its axes normalised and its X and Z axes
  negated (the node looks along its -Z; the engine's cameras along +Z);
- that is put in the frame of the actor's node (0xA8324);
- the lens is set from the animated focal length, the film width of the file's
  camera record and the animated width scale (0xA31F4): horizontal field of view
  `2 atan(film x 0.5 / (focal x 0.03937))` (0x24AC0), the picture's width at the near
  plane scaled by the third figure, and its height the width / 1.76471.

Verified in the viewer (`brs-viewer <model> --anim ... --camera ...`) and in battle:
the victory pose and the defeat are framed as shots, with her left of centre.

## Items and equipment (`db_resident.lpk`)

Item numbers start at 1 and run through three tables one after the other (0x12C9C8;
the tables are the archive's first three files, 0x12BED8), 128 records each:

| Items | Table | What they are | Record |
| --- | --- | --- | --- |
| 1-128 | `000.xtc` | consumables: Repairers, Charges, Breaks, Boosts (21 real ones) | 48 bytes: name, description, a shorter description, flags at 0x0C, at 0x10 a word whose low 7 bits are the most she can carry (5 for all of them) and whose high half is the HP it restores, at 0x14 the percentage of her HP it restores, at 0x18 a number that looks like a status effect's (not read yet) |
| 129-256 | `001.xtc` | equipment: four weapons, six suits | 24 bytes: name, description, flags at 0x08, then three numbers |
| 257-384 | `002.xtc` | valuables, kept for the story (Gum, the letters, the spare helmets) | 24 bytes: name, description, flags at 0x08, at 0x0C a word whose low 7 bits are the most she can carry (10 Gum, 10 Smoke Grenades, 1 of the rest) |

The tables are padded out with records that are not in the game: an item is real
when bit 16 of its flags is set (0x12C24C). `003.xtc` holds the skills.

What she carries is a byte per item number in the save data (0x246C), the count in
its low 7 bits (0x1BF4 reads it, 0x1BA4 sets it). Giving her an item (0x14D594: item,
how many) does nothing for one that is not real; otherwise the count becomes what
she had plus the number, kept between 0 and the item's limit: the table's for a
consumable (0x12C4C4) or a valuable (0x12C85C), one for a piece of equipment. In the
port: `Items` and `Inventory` in `crates/brs-game/src/items.rs`; `tools_re/items.py`
lists the tables.

For a weapon the three numbers are its
attack bonus (added to stat 5 by 0x14D0CC), a heat figure and the bullets of a cold
burst: Rock Cannon 15 / 10 / 6, Blue Square 25 / 10 / 7, Dynehalcyon 50 / 10 / 8,
Hyperion 60 / 8 / 8. For a suit the second is the heat a dodge costs (read by
0x7E7B0): Normal Suit 15, the others 10. A new game has weapon 129 (Rock Cannon): 29
+ 15 = 44 attack.

## Light and shadow in a battle

**No light.** The PSP's light is given a place and colours in five places of the
whole program (0x15AB84: the frame's own setting up at 0x2DDC, two of the
interface's models at 0x3B7E4 and 0x66318, the field's characters at 0x10F3B4
and the ride on the bike's at 0x13E33C), and none of them is the scenes' engine,
which draws a battle and a story scene. The frame of a scene (0xAB7F8) has the
light that is all about white (0x15AA20 with all ones) and switches the PSP's
lighting off (0xAB97C, 0xAB984: states 10 and 11 of 0x15A458; only the round
shadows are drawn with it on, 0xAC958): a model of a battle is drawn by its
corners' own colours, which is how the port draws them. So there is nothing to
port of a battle's lighting.

What a material's own list says still holds with no light (the lists are
counted in `docs/FIELD_NOTES.md`, "How a material is put on what is behind
it"): its picture is put on by its function, which adds it for one material of
each of 24 of a battle's enemies (`bee02`, `egb00` to `03`, `egg00` to `05`
and `07`, `had00` to `03`, `twi00` to `03`, `yub00` to `03`), and it is mixed
as the list has it. In the port such a piece gets the lit models' material
with no light (`brs_engine::lit`, `dress`).

**The round shadows.** A node of a scene can throw a round shadow on the ground
(0xACF64(node, radius, strength) sets its + 0x54 and + 0x58 and its flags 2 and
0x20). 0xAC878 is a node's drawing, node after node down the scene: one whose
flags 2 and 0x40 are set has what it draws drawn (its + 0x68), and then, with
flag 0x20, its shadow: where the node is in the world with its height made
nought, a fan of ten corners (0x16A4E8: the middle (30, 0, 30) and all there,
eight round it one away with no alpha) made as large as the radius, whatever
the size of the model, and mixed in by its alpha times the strength (the
light all about is white with the strength for its alpha). Flag 0x40 is
whether the node is to be seen: engine function 74 (0xBE0CC, 0xACF80) sets or
clears it on the model of a slot.

A battle gives her hers as it makes her (0x999E8): 75 under her model's first
node, 20 under each of nodes 20 and 24, all of strength 96 of 255. An enemy's
script gives its own with engine function 51 (0xBDA28: the slot of a model, a
node of it by 0xA74A8, the radius, and the strength of 255): every one of the
88 enemy scripts does, as it loads its model, 104 calls between them
(`tools_re/out/_shadow_calls.py`). Most have one under node 0, 4, 50 or 51:
(120, 100) for the bats, bees, hulks, the twins and one boss, (75, 240) for
the eaters and the first boss, (200, 100) and two of (75, 180) under nodes 22
and 52 for the cats, (60, 100) for the cubes, (50, 240) for two bosses, (200,
240) for two more, (480, 200) for the largest thing there is, and three under
nodes 50, 52 and 54 for the ships.

In the port: `battle/shadows.rs` (`give`, and `follow` each frame); an enemy's
are asked for by `Event::Shadow`. A shadow is seen while its model is, where
the original goes by the node's own flag.
