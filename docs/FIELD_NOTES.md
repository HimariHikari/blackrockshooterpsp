# Field notes

What has been read out of `EBOOT.OLD` about the field: the map's script and the engine
functions it calls, the progress flags and variables, characters and how they move,
traps and walls, the enemy symbols and their behaviour scripts, the field's system
script, the mission's display, going from map to map and from mission to mission, and
how a battle is started and handed back. Addresses are in the loaded
image (file offset − 0xC0). Formats are in `FORMAT_NOTES.md`, battles in
`BATTLE_NOTES.md`.

Where it is in the port: `crates/brs-game/src/field/` (`host.rs` the map script's
engine, `scene.rs` the plain data it works on, `motion.rs` and `mover.rs` a
character's frame, `symbol.rs` the enemies' behaviour, `system.rs` the system script
and the mission's display,
`tables.rs` the game's lists, `assets.rs` character bundles, `mod.rs` the systems) and
`progress.rs`.

Tools: `tools_re/fld_scripts.py` (every map's script: survey, grep, decompile, usage
of the many-case functions), `bscr_dec.py` (one script), `natives2.py 1` (the table of
engine functions), `natives_waits.py` (which of them wait), `switch.py` (the cases of
a switch), `callsites.py` (how each call of a function sets its arguments),
`flag_uses.py` (who reads or sets one flag), `sscr_natives.py` (the tables of engine
functions `SSCR` scripts call), `fld_resident.py` (the field's tables), `sscr.py`
(disassembles an `SSCR` script), `sscr_dec.py` (decompiles one into C-like text),
`sscr_calls.py` (which engine functions one calls), `dump_natives.py` (the code of
some of a table's functions), `jt.py` (a jump table in short), `findstore.py` (loads
and stores at an offset), `mips.py dump` (a stretch of the executable to
`tools_re/out`), `strref.py` and `findtext.py` (where a string is and what points at
it), `maps.py` and `missions.py` (the game's maps and missions, as listed),
`clim_top.py` (a map's collision drawn from above, for planning a scripted run),
`ptm_png.py` and `pict_screen.py` (a texture, and a picture of the screen, as a PNG),
`fevt.py` and `fevt_survey.py` (the field's events, and what their scripts call).

## The map script

Slot 10 of a `FLD/MAP` bundle is a `bscr` script, the same format and interpreter as
the enemies' battle scripts. 42 of the maps have one. It calls the engine through
**table 1** (0x171370, 281 functions, dispatcher 0xC9B58, jump table 0x16C0D0; opcodes
0x82 and 0x86; written `s<number>` here). 155 of them are called by some map.

The field keeps one script context for the map (made at 0xD9014). Its entry points are
found by function type (0xC9314), which also runs every `init` function (type 0xE4):

| type | name in every map | started by | when |
|------|-------------------|-----------|------|
| 0xE7 | `OPEN_EVENT(0)` | 0xD90AC | the map has loaded |
| 0xE8 | `CALLBACK_NPC(id)` | 0xD9118 | a character is touched or looked at |
| 0xE9 | `CALLBACK_TRAP(id)` | 0xD916C | the player enters a trap |

Only one runs at a time: starting one resets the context's stack. While one runs the
field is in an event state (4 for a character's, 5 for a trap's, 7 for the opening;
0x116850) and the player is not the player's to move: she is stopped as it starts
(0x1078A4). A function that has to wait sets bit 0x80 of the context and leaves a
callback at context + 0x310; the context's update (0xC9838, once a frame through
0xD91C0) calls it and lets the script go on when it answers non-zero. 41 functions
wait (`tools_re/out/natives1_waits.txt`).

**Skipping an event.** `s190(0)` marks the start of the part of an event that can be
skipped (bit 0x100 of the context, 0xC33C4) and `s191()` its end (0xC3580); the
library's `EVENT_START2` and `EVENT_END1` call them, round the event's own content.
While the bit is set the menu start brings up offers to skip (0xD93DC); choosing it
(0x10C998) stops the camera, empties the queues of events, resets the window people
speak through, turns the picture black at once and sets field flag 0x3F, on which
the next frame sets bit 0x200 of the context (0xD91F8, 0xC9B10). With that bit set
the script runs on to `s191`, and 28 functions that only show something do
something else (`tools_re/fast_checks.py` finds them by the test of the bit):
`s21` does not wait, `s99` does not wait, `s7` does not fade, `s48` leaves the
character where it is, a walk started with `s132` is over at once where it was going
(0xE44D4), `s63`, `s70` and `s265` give no animation, `s237`, `s242`, `s250` and the
camera's functions (`s100`, `s101`, `s103`, `s146`) do nothing. `s191` puts back
what it has to (the line window, the camera, 0xE313C, 0xE57D0), waits for anything
still loading and clears both bits. The scripts are written for it: what an event
leaves behind is put in place after `s191`, not by the event. In the port
(`MapScript::can_skip`, `skip_event`): start brings the pause up over the scene
as in the original, and cross there skips it (`pause.rs`; "The start window" in
`MENU_NOTES.md`); and unless `--play-events` is given every event is skipped as
soon as it can be, since the port cannot show all of them yet.

Each map's script is the same library (`func00_…`) plus the map's own functions:
`func01_` the field itself (`INIT_FIELD`, `PUT_FIELD`, `PC_ENTRY`, trap callbacks),
`func02_` objects, `func03_` encounters (`PUT_ENCOUNT`, `CALLBACK_ENC_TOUCH`), `func04_`
events and the mission's logic. `CALLBACK_NPC` and `CALLBACK_TRAP` hand an id on by
its range: characters 200-298 objects, 400-498 treasure, 500-598 enemy symbols, 600-698
event characters, 700-798 save points, 3000-3098 symbols that pop up; traps 0-98 map
changes, 100-198 camera, 200-298 field, 300-398 object, 400-498 encounter, 500-598
event. The tutorials' text is in the script itself (in English in this version).

Waits the port keeps: `s21(n)` n frames (the callback counts them, 0xC180C), `s99(id)`
until character `id` is no longer acting (0xC4AF8), `s20()` the fade started with
`s7(way, colour, frames)` (way 1 out, 0 back in), `s81` a battle, `s247` an entry of
the system script, `s173` the field being left (for good), `s252` a window being
read, `s75` a question being answered, `s258` a press of cross, `s238` the events
that are being played. Waits it lets pass because what they wait for is not shown
yet: `s100` and `s145` (the camera), `s60` and `s280` (loading), `s275`/`s276`,
`s240` (a story scene's line).

## Flags and variables (the save's progress)

`s8(n)` reads flag n, `s13(n)` sets it, `s12(n)` clears it (0x1494, 0x1354);
`s23(n)` reads variable n and `s24(n, v)` sets it (0x16BC, 0x15B8). All of it lives in
the save block (0x2444):

| numbers | what | where |
|---------|------|-------|
| flags 0-9999 | one bit each | save + 0x80, bit `n & 31` of word `n >> 5` |
| flags 100000-100256 | not saved | the game's work area |
| variables 10000-10031 | 32 bits | save + (n − 10000) × 4; 10001 is kept below 10000 |
| variables 20000-20500 | 16 bits, unsigned | save + 0x764 |
| variables 100000-100002 | not saved | the work area |
| any other number (0-511 are used) | 8 bits, unsigned | save + 0x564 + n |

Setting a flag in 7000-7127 or 7200-7327 also checks a record (0x12CDBC); not read.

A new game (0x13C, which `launchTitle` calls before the title is up) clears all
of it. In order: the times the game was cleared and the time played; her figures
(0x990); where she is (0x1C68); the player set-up (0x908); every flag (0x12FC);
variables 10000 to 10031, the bytes and the halfwords (0x1548); what she carries
(0x1B78); and the records (0x14BBD0, 0x14B63C). The player set-up gives her
**record 0 of the start table** (0xE10 with 0: every figure at its most, see
"Where a new game starts"), sets her figures 0x13, 0x11 and 0x14 to 0, 5 and 9 and
variable 40 to 1, gives her items 133 and 129 (the Normal Suit and the Rock
Cannon), names map 67 as the one to go to, runs the system script's `init_player`
and backs the save up. As the flags and what she carries are cleared after it, a
game begun at the title has her with that record's figures and nothing carried,
and no skill marked as had. (As the game boots the set-up is run once more by
itself, 0x316C, before the title is launched and clears again.) The title's "new game" (0x11A1B4) then sets variable 100 to 10, variable
10004 to 0 and variable 220 to 100, and starts the field. Variable 10004 is the
scenario (mission) number, 220 the progress within it, 239 a count shown on screen,
10003 the last story event seen, 100 + n the state of mission n (1 newly open, 2
open, 10 begun, 255 cleared).

After a battle the field (0xD893C) takes the battle's figures from the result record
(0x2EAF8; a battle lost leaves them at zero) into the counts the missions go by:

- variable 10005, while it is above zero, goes down by the damage she took, to no
  less than zero;
- variable 10006 goes up by the damage she dealt, to no more than 999999;
- variable 237 goes up by the enemies beaten, to no more than 99;
- the battle's seconds (at most 10800), times 0xF000, are handed to the system
  script's `update_mis_time`: the mission's clock goes on by them;
- after a battle won, HP of zero or less becomes 1 (0xD8A40).

Then the field's flags 0x22 and 0x10 are cleared, which is what `s81` waits for. In
the port: `Progress::count_battle` and `end_battle`, which hands the battle's time to
the system script.

## Characters

A map's characters are numbered by the script. 0 is the player, whom the field makes
before the script runs. A character is made from a resource,
`FLD/FCHR/FCHR_<name>.BIN`, whose name is record `chr` of `lst_fchr`.

| call | what it does |
|------|--------------|
| `s40(chr)` | loads resource `chr` (counted; 0xE1638) |
| `s41(chr)` | lets it go, waiting for a load in progress |
| `s42(id, chr, x, y, z, angle)` | makes character `id` from it, unless there is one already, at that place facing `angle` degrees about the vertical (0xE1844) |
| `s48(id, x, y, z, angle)` | puts it there (0xC3AAC) and stops it |
| `s144(id)` | removes it |
| `s52(id, what, value)` | sets one of 135 things (switch at 0xC7520, table 0x16B8D0) |
| `s143(id, what)` | reads one (0xC8054): 11-13 its place, 14-16 its turns in degrees, 17-19 its target, 20-22 its target's turns, 23 its resource number; places are cut to whole units |
| `s133(id, x, y, z, angle)` | sets its target and the way it is to face there |
| `s132(id, n)` | has it do something (0xC5064): 0 walk to its target, 2 turn to its target facing, 6 be destroyed; from 7 on by 0xE6068: 9 pop up out of the ground, 10 drop in from above, 14 appear through a ring, 12 go over to the face settings 117 and 118 name (0x1095D4; see "Faces"); 3, 4, 7, 8, 11 and 13 do nothing |
| `s62(id, n)`, `s70(id)` | names one of its own animations (0x1071A0) and sets what is named playing (0xF4EEC, not in a skipped event); `s63` and `s265` name an event's instead |
| `s64(id, frame)`, `s65(id, from, to)` | puts its animation at a frame of the game and holds it there (0xE3400: flag 9); sets what is named playing for `to` frames (0xE3364, 0xF4F20: the frame to start from is overwritten as the animation is started) |
| `s179(a, b)` | the way from one character to another, in degrees |
| `s53(id, kind, n)` | makes a character that is an effect and sets the effect playing (0xC3FBC): see "Characters that are effects" |
| `s220(id, chr, node)`, `s222(id)` | fastens such a character to a node of another's model (0xC6098), and lets it go (0xC60D8) |
| `s280(id)` | waits until its resource has been read and it is ready (0xC5ED8) |
| `s255(frame, what, value)` | sets one of the field's sixteen frames (0xC66D4): 0 to 2 its turns in degrees, 3 to 5 its place; see "The frames" |
| `s257(trap, x, y, z, a, b, c, turn, turn, turn)` | gives a trap its box as `s44` does, with all three of the box's turns in degrees, about the axis across, the upright one and the one ahead (0xC3CC8, 0xE1FB4, 0xED4CC): the camera's traps of the sixth stage |
| `s236(id, x, y, z, a, b, c, angle)` | makes it a box to those that would walk through it (see "What stands in a character's way") |
| `s159(id, n)`, `s160(id, n, how)` | its eyes and its mouth (see "Faces" under Story scenes) |
| `s228(a, b, frames)`, `s229(frames)` | the camera shakes, `a` and `b` hundredths of a degree up and down and from side to side, coming over so many frames (0xC613C, 0xE5A5C), and dies away over so many (0xC619C) |
| `s234(trap, way, x, y, z, x, y, z)`, `s235(trap, n)` | gives a trap a place to take her to, and has it take her to its places (see "Taken from place to place") |
| `s180(point, x, y, z, reach)`, `s181(id, n, point)` | gives the field a numbered point, and a symbol's route its n-th point (see "Patrols") |
| `s107(0, item)` | how many of an item she has (0xC4D7C, 0x1BF4: the low seven bits of the item's byte) |
| `s130(n)` | a number by chance from nought up to but not reaching `n` (0xC2620, 0x93A0: a fraction made of the generator's bits, times `n`, cut) |
| `s263()` | makes her model again if what she has on is not what it was made of, and waits for it (0xC67E4, 0xE6528); the port does that whenever what she wears changes |
| `s262(n)` | gives her record `n` of the start table (0xC6788, 0xE10, 0x14C9D4): each figure the record has (one below nought is left as it is), one of what it has her wear if she has none, and the skills of its slots marked as had and as set. Called once, with 1, by the opening's script when its film is over: see "Where a new game starts". `Starts::give` |
| `s205(n)` | the game is cleared (0x340): the system data's bit for it is `n`'s lowest and the save's count of the times is `n`. Called once, with 1, by the last map's script (`MAP_E_STG00_05`) when the story has been played to its end, after variable 10001 has gone up by one and before "Would you like to save your Clear Data?". `s206()` would answer whether it has been (0x394: that bit, or the count, or variable 10001 above nought); no script calls it. `Scene::game_cleared`, `mark_cleared` |
| `s281(0)` | before a battle of note: lets go of what the field has loaded with the sign of the disc up, and waits (0xC6F7C); nothing to do in the port |
| `s231(n)` | eight things by number (0xE6434; the maps' scripts give 1 969 times, 2 440, 3 171, 5 six times and 4, 6 and 7 once each): 2 drops the events asked for; 3, after a battle of note, empties thirty-three of the system's list of what is loaded (0xEAF0C and 0xEAF44: 0x14BCB8 for 6 to 37 and for 38; nothing to do in the port); 4 runs the system script's `sys_cmn_start_mis_mazuma` to its end (0x130CE8: the time of the mission against Mazuma, 0x5A000 in variable 10013, its counts at nought and flag 4701); 5 makes the game's variable 300 one (0x23E0) before a save is offered; 6 keeps how loud the options have the music and makes it 127, and 7 puts it back (the field's + 0x1D43, 0x428, 0x460, 0x5E8), about the opening film. In the port: `Scene::system_call` for 4, which the map's script waits on, and `Request::MusicOptionFull` for 6 and 7 (see "Films"); 0 is not ported, and no script gives it |
| `s260()` | reads a byte of the game's (+ 0x2073D) that scripts take for how the ride on the bike came out: 1 ridden through, 3 given up (the script then leaves for the screen between missions), anything else failed. In the port the ride says how it came out when it is over (`Work::ride_outcome`, `BIKE_NOTES.md`) |
| `s110(n)`, `s111(id, n)` | makes thing `n` of the map's own ready (0xC4E18, 0xE4044: a resource of kind 4 from file 2 of the map's scene, 0x1108DC), and makes a character of it (0xC4E34, 0xE4134): see "Things of a map's own" |
| `s71(id, place, n)`, `s150(id, place, 0, away)` | gives a character thing `n` of the loaded event to hold in one of its five places for such (0xC443C, 0xE3554, 0xFDBB8; not in a skipped event), and puts what is in one away or brings it out (0xC53E4, 0xFE3C4): see "Things held in a scene" |
| `s83(file)`, `s84()`, `s85(x)`, `s86()` | a film: makes the task `Movie` and has the film made ready (0xC3170, 0x11863C), waits for that (0x1188A4), starts it (0x1188E4; the argument is not looked at) and waits for it to be over (0x1189F0), 97 times in 49 maps; see "Films" |
| `s164(r, g, b, a)` to `s171` | the scene's lights, in hundredths: its ambient colour (0xF0174), one of eight lights chosen and given two colours, a third, switched on or off, put at a place or pointed a way (0xE4874 to 0xE49F4), and `s165()` has them put back (0xF0264): see "The lights". The maps call the first three only |

A place given to `s42` or `s48` is dropped onto the ground if there is ground within
40 units above or below it (0x108520), unless the character's flag 0x18 is set.

`s52` goes three ways: most cases set one of the character's flags (0xE5F70), an int
parameter (0xE5FB4; switch at 0x10A720) or a decimal parameter (0xE1AFC). Cases read
so far:

| case | what |
|------|------|
| 0 | hides it (flag 4) and ends what it is doing |
| 2, 3 | the kind of its contact shape 0 or 1, and switches the shape on: 1 is something to look at (home points, boxes) |
| 5 | whether a button does anything with it at all (its contact's flag 0, 0xE374C) |
| 8 | flag 0x18: not solid, and not dropped onto the ground when placed |
| 9 | how near she has to come to be in touch with it (decimal 0xC, 0xE43D8) |
| 10 | the status it moves in (int 0xE): 0 walk, 1 run |
| 11-22 | its place, turns, target and target's turns, as `s143` reads them: 14 to 16 are its turns about the axis across, the upright one and the one ahead, in degrees (decimals 3 to 5) |
| 24 | flag 0x1B: its time stands still (0xE4758, 0x108350). The time a character has of a frame (0x108240) is nought while the field is held (0x10CE24, 0xF3348) or this flag is set, and else its pace by the field's; its model's animations go on by that time (0xFCE90: the body's, 0x10BC18, and the layers', 0x10BC58), and all it does. The maps set it 116 times and clear it 103: the fallen and the sleeping of a scene, put in a pose and left in it. `Character::time`, `flag::STOPPED`; the model's animator goes at `pace_fraction` |
| 25 | its pace in percent (int 0x28) |
| 27 | flag 8: the animation a script starts on it loops |
| 28, 29, 30 | ints 0x21 to 0x23: the animation it stands in, runs in and walks in (0x10A920, 0xFE4AC: its + 0x6D0, + 0x6CC and + 0x6C8, read by its status at 0xFD960; 0, 11 and 10 as it is made, 0xFCA1C, and its own script's `get_stat_anim` would say otherwise as its model is ready, 0xFCD4C, but no script has one). The maps set 28 over four thousand times: an arrow's and an opened box's stand, a scene's people. `Character::motion_of` |
| 33 | int 0x27, given in frames |
| 35 | the behaviour script that runs it, none if negative (int 0x25) |
| 38 | how a symbol moves about when left to itself (int 0 by way of int 0x26): 1 patrols its route, 2 wanders |
| 39 | how many points its route has (0xF9C04) |
| 40 | marks it an enemy symbol (flag 0x27) and clears "chasing" (0x60) |
| 41 | flag 0x28 |
| 47 | flag 0x53: it keeps its pose when what it is doing ends |
| 70 | how far out it is round (0xE53F4: its + 0x170, see "What stands in a character's way"). Every map's start gives her 40, which is what it is: nothing to do |
| 71 | 0 sets flag 0x56 (0xE542C), which nothing found reads |
| 72 | 0 sets flag 0x57: it has its frame and is drawn however far from her and however near the camera it is (see below) |
| 77 | 0 sets flag 0x61: it has no shadow |
| 78 | 0 sets flag 0x62: it does not idle when it has stood a while (0xE5924; read with the field's flag 0x47 at 0xF9030). 160 times in 49 maps, for a scene's cast (`flag::NO_IDLING`) |
| 80, 81, 82 | how far from where it would lie its shadow is put: up (its + 0x440), and along the world's x (+ 0x444) and z (+ 0x448), added as the shadow's point is made (0x107B54, 0x107B74). A scene of the fifth stage lifts her shadow and another's by one; one of the second pushes a shadow 20 and 15 aside (`Character::shadow_offset`) |
| 75 | the pace it goes about at when left to itself, in percent (int 0x29) |
| 88 | its size, in hundredths: all three of its scales (decimal 0x1D) |
| 91 | nothing: its case goes straight to the switch's end (0xC7C64) |
| 92 | gives it a sign, the one a trap can have (0xE5FF8, 0x107710: the figure at its + 0x4B4, its flag 0x75, and the object of its marks, which is made for that flag too, has the second thing it holds, 0x44B10, made a sign of kind 4 at the character's place, 0x1073E4 and 0x44BF8). It is drawn after the character's mark (0x1077EC) wherever the character is before the camera, with no say of how far she is as a trap's has, unless the field's flag 0x5A or the character's 0x74 (setting 93, which no map makes) is set. The library's `SET_KONTENA` gives one to each of the 23 containers of the game (`Character::sign`) |
| 94 | flag 0x76: what she touches by walking into it (0x118330) counts only while she is in action 7, the dash that nothing begins (see the actions): so a container, which has it, is never walked into. Nothing to do |
| 95 | its + 0x4B8 (0xE6030, 0x109044), which nothing found reads |
| 97-111 | figures of a gun emplacement (the library's `HOUDAI_SEARCH_RANGE`, `HOUDAI_ATTACK_RANGE` and `HOUDAI_ATTACK_PARAM`: decimals 0x1F to 0x28 and ints 1 to 5). No map calls the three: there is none in the game |
| 112 | touching it calls the script (flag 0x78) |
| 114 | flag 0x9D: its gimmicks are off. Each frame of its model (0xFD284) hands the flag to its resource (0x10BEBC), and with it what is layered on its body's animation is not gone on with (0x10BC78), not put on the model (0x10BD58: the body's alone is), and its chains are left as they hang (0x10BE04). The library's `SETUP_B_CHAR` sets it for a scene's cast, whose animations move hair and face themselves, and a scene clears it for one it is about to give a face (`setting::NO_LAYERS`, `Animator::layers_off`) |
| 115 | int 8, a byte: which of the field's sixteen frames it is in (see "The frames") |
| 116 | int 0xA: its face, at once (`set_face`; see "Faces") |
| 117, 118 | ints 0xB and 0xC: the face it is to go over to and in how many frames, which `s132(id, 12)` has it do (`shift_face`) |
| 119 | int 0xD, set to 3 with every face a scene has one go over to (1187 times); nothing found reads it |
| 122 | int 0x11 and flag 0xA5: the tile with the chevron stands by its mark (0x10A854, 0x46CD0; see "The marks over characters"). A mission's script sets it for the enemy it is after (`flag::POINTED_OUT`) |
| 123 | int 0x12 and flag 0xA6: its turn among those that are drawn in a set order (0x116B38: of two characters the one with the lower number first if the first has one, else the further from the camera first; asked as each character is put into the list the field draws them from, 0x116BC0 from 0x115408). 70 times in 13 maps, 50 of them in the first map's opening, always for a character that is an effect and has setting 124 too. In the port the effects of those with a number are drawn in their order after those with none (`Character::drawn_order`, `order_of` in `field/effects.rs`, `Playing::order`) |
| 124 | flag 0xA7: the effects it carries (its + 0x510, drawn by 0x102FD4 and 0x103074) are drawn by the lens that sees from one unit before the camera on (the camera's + 0x150), rather than the scene's own (0xE7568). 164 times in 55 maps, for the enemies of a story scene. `Lens::Close` of `brs_engine::effects`, by `lens_of` in `field/effects.rs` |
| 126 | the character a symbol is after (int 0x18) |
| 129 | how far away a symbol notices its target (decimal 0x2A) |
| 131 | how far from its home a symbol wanders (decimal 0x2C) |
| 132 | how many frames one that patrols stops at each point of its route (int 0x2D, as time) |
| 133 | 0 sets flag 0xA9: a symbol chases without its dashes |
| 134 | how far its territory reaches (decimal 0x1A) |

A character's own figures start from the field's `prm` table (0x109728): collision
radius `prm[+8]` = 40, touch radius `prm[+0xA]` = 120, territory `prm[+0x42]` = 2000,
pace `prm[+0x44]` = 95 percent; a symbol's notice range `prm[+0x48]` = 1800 and
wander radius 300 (0xF9A00). Its script may define the floats `pop_height`,
`pop_radius`, `destroy_radius` (100 if not) and `shadow_radius` (60).

**Out of her reach, and too near the camera.** A character's frame (0x10600C)
takes its time (0x108240), and then, while the field runs, asks 0x106558 before
it goes on to what the character does (0x106174, or 0xF9C2C for one that
thinks): one further from her than `prm[+0xC]` (4000; her place and its own,
0x106BB8) has no frame at all. It neither moves nor thinks, and nothing of it
is animated. Its draw (0x1067F8) asks two things first. 0x1066A8: one further
from her than `prm[+0xE]` (3000) is not drawn. 0x1063B8: a model of the first
kind (its + 0x78 names the kind) that is nearer the camera than the field's
+ 0x246C is not drawn either, the distance taken from a hundred over where it
stands to where the camera is (0xDA518); models of other kinds are not asked.
That figure is `prm[+0x3C]` (150) from when the field is made (0x118238) until
a script gives another with `s96(11, n)`, which the maps do 1288 times: it is
what lets a scene's camera come close to a face. Neither question of how far
from her is asked while she is not there, while the field's flag 0x32 is set
(`s96(21, 0)`) or of a character with flag 0x57 (setting 72: events set it on
the characters they use); nor how near the camera with the field's flag 0x31
(`s96(12, 0)`) or the character's 0x57. With nothing drawn, its shadow and the
effect it is are not drawn either, being drawn from there.

In the port: `Scene::reach_characters` (from `reach_characters` in
`field/mod.rs`, before the characters' frames) says of each whether it is
`asleep` and whether it is `unseen`; one asleep has no frame (`motion::step`),
does not think (`Brains::update`), does not idle and has no time
(`Character::time`), so its animation, its tails and its effect stand; one
unseen is not drawn (`Character::drawn`), nor its shadow or its effect. The
kinds 0x1063B8 does not ask are all but one: the character's + 0x78 is the
resource it is made of, whose first word is its kind (0x105B1C), and only kind
0, a character of the field's list, is asked. A thing of the map's own is of
kind 4 and an effect of the kinds effects are loaded as (5 for an event's), so
neither is, which is how the port has it (`Scene::sight_characters`); a
character made of no resource would be asked from where it stands and not from
a hundred over it, and has nothing to draw.

**Where a character is put** (0x10925C, each frame, into the matrix at its + 0x120).
Its place is at + 0xA0, its three turns in radians at + 0x90 and its three scales
at + 0xB0. The turns make one turn by 0x162D58, which is the turn about the axis
across, then the one about the upright axis, then the one about the axis ahead
(`Rz·Ry·Rx`); that is turned again by the turn of the frame it is in (0x109128:
the frame's turn times its own, 0x162C90 being the product of the two in that
order), and the matrix is the place, times the scales, times that turn. The port's
`Character::frame` is this, with the frame's turn kept as the character's `lay`.

### The frames

The field has sixteen frames (0x70 bytes each at its + 0x4C60, made as the world is
by 0x117C10): three turns (+ 0), the one turn they make (+ 0x10, by 0x162D58 as a
character's own), its matrix (+ 0x20) and a place (+ 0x60). A character is in the
one its int 8 names (its byte + 0x4C0, setting 115), frame 0 unless a script says.
`s255(frame, what, value)` chooses a frame (the field's + 0x5364, 0xE6314) and sets
one of its turns, given in degrees, or one of the figures of its place (0xE633C); a
turn has the frame's one turn and matrix made anew (0x117B28). Only the five maps of
the sixth stage do it (210 times between them): their floors hang in the air at
every angle, each level to what is in its frame. The first of them has seven:

| frame | turns (x, y, z) | | frame | turns |
|-------|-----------------|-|-------|-------|
| 1 | none | | 5 | 90, 0, 90 |
| 2 | 0, 0, 90 | | 6 | 0, 0, 180 |
| 3 | 0, 0, 20 | | 7 | 0, 0, -60 |
| 4 | 0, 0, 65 | | | |

**Nothing reads a frame's place.** The only loads from a frame are of its turn
(0x109128, 0x117B6C) and, where a character changes frames, of its matrix
(0x10930C); the three figures at + 0x60 are written by `s255` and by 0x117C10 and
by nothing looked at since. A frame is a turn about where the world begins.

**What goes by a character's frame.** 0x109128 gives the turn of the frame it is
in, 0x109228 that as a matrix, and 0x1090E0 a vector turned by it; everything that
has an upright asks one of them:

- its model's matrix (0x10925C, 0x106260), as above;
- the ray that finds what it stands on and how that place is lit (0xFD3CC: from 300
  along its own upright over it to 200 under it), the one that puts it on the ground
  where a script places it or a leap lands it (0x108520: 40 either way), the one
  under the mover (0x101490) and the mover's way over the ground (0x101B1C);
- the throw of a leap (0xF5538) and the effects of one (0xF8648, 0xF891C: the
  effect is turned as she is, frame and all);
- the player's stick: the way pushed is taken into her frame to be set against
  the way she faces (0x105538), and the way itself is kept as the world has it
  (0x109630, her + 0x4A0);
- where the system script looks for room for an enemy to pop up (0xD5470 and
  0xD5260 both ask her frame's matrix, at 0xD54DC and 0xD52A8; what they do with
  it was not read through, and the port has the place be ahead of her as her
  frame lies, on ground found by its upright);
- a trap's box: its three turns are turned again by the turn of the frame the
  box is in (0x117B6C from 0xED640; the box's byte + 1), the place tried being
  the world's;
- the camera, of every kind: see "The camera in a frame" below.

**Traps.** `s45(trap, 39, n)` puts a trap's box in frame `n` (both its boxes'
byte + 1) and has `n` be the frame the trap takes her into (the trap's byte + 8);
`s45(trap, 40, n)` then says another frame to take her into; `s45(trap, 41, n)` is
the trap's figure at + 0x430, which while set has the trap passed over where the
one she has walked into is looked for (0xD3F10, 0xD3F9C, 0x118484). The sixth
stage's library makes its pads with `SET_STG06_GIMMICK(id, box..., from, to, ...)`:
an event's trap that a button works, with 39 the frame it stands in and 40 the one
it leads to, and one place to leap to (`s234`). `s235` (0xE5EC8), when the two
differ, sets her int 9 to the second (her byte + 0x4C1: the frame her leap is to
change to, none being -1) before it sets her off.

**A leap into another frame.** Where the leap begins (0xF7F04) a frame to change to
marks it (the action's + 0x34): it is timed by how far it goes alone, at her run's
speed (0xF80F0), springs for none of that, and is thrown against a tenth of the
gravity (0xF5590: 0.980665). She gathers herself in the frame she is in. As she
leaves the ground, after the effect and its sound (0xF8694), she changes frames
(0x10930C, for as long as the leap is in the air) and her int 9 is none again:

- with `d` the way she is going and `u` the new frame's upright, the way she is to
  face is `d` laid over the new frame's ground (0x105224 with `d`, `u` made square
  to it, and `u`: `d - (d·u) u`, made a unit), kept at her + 0x4E0;
- her int 8 is the new frame; the turn from the new frame's lie back to the old
  (the model's matrix before, times the inverse of the one after, as a turn,
  0x1629F4) is kept at her + 0x4D0, a count at + 0x4F0 is nought and + 0x4F4 the
  time it is to take, and her flag 0xA3 is set;
- while the flag is set 0x109128 gives, for the frame's turn, that turn eased back
  to none and then the frame's: the old lie at first, the new at last, with `1 - (1
  - t)²` of the way gone at `t` of the time (0x162CAC is the usual going over
  between two turns, by the shorter way);
- each frame of her action (0xF5E50) the count goes up by the time gone, her turn
  about her upright is what faces the way kept, as her frame lies just then
  (0x107E34), and when the count is more than the time the flag is cleared.

So she turns over in the air to stand on the floor she is coming to, facing on
the way she flew; the throw is by her upright as it is just then.

**The camera in a frame.** A camera that stays put (0xDDC00) and one that looks from
a point (0xDDE44) are turned by their angles and then by her frame's matrix
(0x109228; the angles of the second are worked out from the way to the point as her
frame has it, and so kept); one that follows (0xDB29C, 0xDB528, 0xDD99C) has what it
looks at, its offset and its upright by her frame. So the view rolls over with her
as she leaps. A camera on a path (0xDAA48) asks the path's grid where she is *as her
frame has it* (her place by the inverse of her frame's matrix, its x and z), and is
then where the path's own rig has it, as the world has that: the paths of the sixth
stage are made in the world, over grids that are each level in their frame.

**In the port** (`field/frames.rs`, 2026-10-08). `Frames` is the sixteen and, for
each that is turned, a copy of the map's collision turned the other way
(`CollisionWorld::turned`), which is the map as what is in the frame has it
(`Lie`: the ground under a place, what marks it, into the frame and out). A
character keeps how its frame lies (`lay`, the frame's turn, between two while it
goes over: `Turning`), the frame a leap is to change to (`group_next`), and its
frame's number where setting 115 put it (`group()`). Its places and ways stay the
world's, as the original's do; what differs is how it is moved. The original turns
each upright it needs; the port takes one whose frame is turned *into* its frame for
the frame of its action (`motion::act_in_its_frame`: its place, target, way and
speed, the walls and the others that are solid), has it act there on the copy where
its ground is level, and takes it out again, with whatever it had to show. One that
is between two frames acts in the world, by its `up()`. Enemy symbols find the
ground of their route and of their wandering by their frame, and face what they
chase as their frame has the way; the system script's pop-ups and placings go by the
frame too; her touch with others is measured over her own ground; a trap's box has
its three turns and its frame (`Trap::contains`), and `s257`, `s255` and the three
settings are the script's. The camera's kinds are turned as above (`sight`).
Stands in: the stick. The original works out the way pushed by her upright from the
view's own turns (0x101898, by the mode at the field's + 0x2688; not read through);
the port, in a frame that is turned, has "up" be away from where the camera is, over
her ground. `s179`, the bearing from one character to another, is given as the
first one's frame has the way (not read in the original).

Checked on 2026-10-08, mission 100 on `STG06_01`, unseen: the pad on the first
floor takes her to the second (frame 2, a quarter turned) in 2.1 s after eight
frames of gathering, turning over in the air; she and the enemy there stand on it
and she runs about it (her x stays the floor's 2100 while y and z change), its
camera's path follows, the enemy notices her and a battle starts. The pad to frame
7 and from there the one to frame 6 (upside down) work the same. Against the
walkthrough: the mission's first view is the same (23230 s), and the cut to the
point's camera and the leap from the seventh floor's pad at 23286.5-23288 s have
the same things in the same places (`docs/compare/stage6_leap_video.jpg`,
`stage6_leap_port.png`; the walkthrough's player was on the seventh floor there,
which was worked out from which camera the script gives which pad). Tests:
`a_frame_that_is_turned_has_its_own_ground_under_it`,
`one_in_a_frame_that_is_turned_walks_over_that_frames_ground`,
`a_leap_into_another_frame_turns_her_over_on_the_way`,
`a_traps_box_is_turned_by_its_three_turns_and_by_the_frame_it_is_in`. Listings:
`tools_re/out/frames_*.txt`.

### A character's frame

Time for a character runs in units of 0x800 a frame at full pace; its pace is how
many units a frame is worth to it (character + 0x19C, set in percent as
`percent × 0xF000 / 3000`, 0x10821C), and its timers count in the same units.

The frame (0xF483C): a character that is acting (flag 0x10) has its action updated
(0xF7844); one that is not is moved by the mover (0x101F04), the player after her
input has been read (0x105480). When an action has ended within the frame the
character goes back to standing (0xF4908, called from 0x1061B8) unless flag 0x53 is
set.

The mover (0x101F04): the character has a direction it wants to go in and a way it
faces. Its movement for the frame (0x101D64) is, while its status is the one it moves
in, `direction × speed × pace × max(0, facing · direction)` (0x101B1C); otherwise what
it had, less 2.5 units a frame if flag 0xA4 is set and nothing if not (0x101C68). The
speed is what its script's `get_move_speed(status)` gives (32 if the script has no
such function). It is then kept out of the walls, out of the other characters and on
the ground (0x10194C: a sphere of the collision radius resting on the ground, see
"What stands in a character's way"; the port uses its own push-out and ground
snap), and turns towards its direction by 10° to 70° a frame at full pace, the
more the further it has to turn (`(70° − 10°) × angle / 180° + 10°`), facing it
outright once within 10°.

The player (0x105480): any deflection of the stick, or the D-pad, has her run (status
1) the way it is pushed as the camera sees it; there is no walking by half a push.
A character's frame (0xF483C) is its action's if it is at one (0xF7844), and
otherwise, for her, this control, then the mover (0x101F04) and the frame of one
with nothing to do (0xF5F78: a stand's idling for status 3; for one whose status
is the status it moves in, its pace added to the time it has been moving, its +
0x35C).

- **Pushed** with her status not the one she moves in: that becomes 1, or 0x41
  while the game's flag 0x102D is set (0x1054D8), her status becomes it, and the
  time she has moved is nought. Then, if the way wanted is behind the way she is
  going (their product under nought, 0x105580), she has moved more than ten
  frames (0x5000) and moves in status 1, she **pulls up** (0xF573C); otherwise the
  way wanted is her way (0x109630).
- **Let go** while her status is the one she moves in: if that is 1 and she has
  moved more than ten frames she pulls up; otherwise she stands (0xF4908).
- **The pull-up** is action 10: status 12, whose animation is number 12 played
  once and come to over four frames (0xFDA60 on: every other status's goes round
  and is come to over eight, or at once from status 0x17). Its frame (0xF5798)
  counts its time; past thirteen frames (0x6800) flag 0xA1 is set, which lets a
  push of the stick end it there and then (0xF4864, 0xF463C: she sets off in the
  same frame); and when its animation has played (0xFE080) it is over, and she
  stands as anyone does whose action ends (0x1061B8). Until then the mover has
  her as one that is not moving.
- **Heavy going** (flag 0x102D, 4141: the third stage's storm sets it with
  `s12` and clears it with `s13`): she moves in status 0x41, which her script
  has at one unit a frame where a run is 32, with animation 22 (0xFD960); and
  whoever stands does so with animation 5 if it has it (0xFD9C8).

The animation of a status (0xFD960): 0x41 number 22, 12 number 12, 3 the stand
(or 5 as above), 1 and 0 the character's own for a run and a walk (0x1083AC), any
other by 0xFFDE0; one the character has not got is its stand instead (0xFDE7C).

In the port: `control_player` (`field/mod.rs`), `Character::brake` and
`Action::Brake` (`scene.rs`, `motion.rs`), `Character::ran`, `status::BRAKE` and
`TRUDGE`, `Work::heavy`; seen in runs (`--map STG01_02 --start "-1300,740"
--skip-battles --skip-messages --script "wait:7,left:1.0,wait:3"`, pictures at
8.6 to 9.2 s; the storm is mission 44, where she covers 30 units a second).

Actions (the number at character + 0x330; each starts by ending the last, 0xF45BC):

- Walk (5; 0xF51DC, update 0xF8A10): the status becomes the one it moves in, and each
  frame its direction is the way to its target over the ground. It has arrived when
  the target is nearer over the ground than twice its speed and less than 200 above
  or below. A dash (0xF52E0, flag 0x9F) is a walk whose direction is fixed as the way
  it faces and which does not arrive.
- Turn (3; 0xF4A68): to the target facing, by the short way, in `turn_frame` = 34
  frames whatever the angle. One that has animation 15 (0xFDE7C with 0xF, which
  also wants bit 0 of its resource's + 0x10) plays 14 for a turn its angle grows
  by and 15 for the other way (0xF4C4C: the sign of the turn a frame, + 0x34),
  once, come to over 0x4000 of its time, which is eight frames. Status 6. (Until
  2026-10-08 the port had these as 15 and 16, and going round; 16 is what the
  people idle with.)
- Stop (0xF4908): status 3, no action, and a time to its next idle animation between
  `prm[+0x18]` and `prm[+0x1A]` (120 to 240 frames).
- An animation (4; 0xF7CF4, update 0xF7C08). Setting playing what a script has named
  ends what the character was doing, lets go of a hold (flag 9), finds where the
  animation starts and ends (0xFE0E4, 0xFE124, or an event's own figures), puts the
  model's animation at its start and starts it (0xFDED8, looping if flag 8 is set,
  blended in over the frames of setting 33). With flag 8 that is all, and flag 0x1C
  is set. Without it the character is busy (action 4, flag 0x10): its own count of
  time (+ 0x1A4) goes up by its frame's and the action is over when it reaches the
  end (+ 0x1AC), after which it stands again as after any action unless flag 0x53
  has it keep its pose. This is what `s99` waits for after a `START_MOTION` that
  does not loop, 507 times in the maps' scripts. `s65` gives the end itself, in
  frames of the game, and has the animation played once whatever flag 8 says.
  In the port: `Character::play_motion`, `Action::Motion`; the length of one of
  its own animations is asked of its resource as it plays, since that may not have
  been read when the script starts it.
- Appear (20; 0xF7090, update 0xF72B4): its place becomes its target, it sinks by its
  `pop_height`, stays there 10 frames, then rises over 30 frames as
  `1 − (1 − t)²`; animation 10 if it has it. Sets flag 0x18. A ring is on the
  ground where it is to stand from the first (effect 510001, as wide across and
  along as its `pop_radius` of a hundred), and goes over 30 frames once it is up.
- Pop up (18; 0xF6D20, update 0xF6EE4): the same from 80 below, in smoke (effect
  999020 where it is to stand) and thrown: it is `(4.13766 t − 4.9033 t²) × 100` up
  from where it started, `t` in thirty frames, and there after nine.
- Drop in (19; 0xF7458, update 0xF7544): from 1000 above, thrown down:
  `(31.8623 t + 4.9033 t²) × 100` in nine frames; something rises where it lands
  (effect 999016).
- Destroy (15; 0xF93EC, update 0xF9660): animation 60, status 0x11, the sound its
  script's `se_destroy` names (no script has one). What follows goes by its
  script's `type_destroy` (0x1027CC). With none, as every enemy has it, it bursts
  when more than 25 frames have gone by (0xF9778, once, by its flag 0xA8): effect
  509001 half its `destroy_radius` above where it stands and then as far as the
  whole of that towards the camera, a shake of the camera (below) and sound 0x96;
  and it is hidden once more than 30 have. With 3, as the boxes she can break and
  one other thing have it (`g_box01` to `06`, `11`, `12` and `g_gar01`), it
  goes in smoke from the first (effect 999020) and fades over 55 frames: its
  float 0x1E, how much of it there is, is one less the share of them that has
  gone by (0xF96F8), and nothing at their end, where it is hidden. A character
  with no model is hidden at once. In the port: `Character::alpha`, which goes
  to its model's light (`Lit::alpha`); tested, not yet seen on the screen (a
  box that is checked opens and stays).

`ACT` (0xD4D74) and `s132` from seven on (0xE6068) have the three ways of coming
as 14, 9 and 10. Action 10 is her pull-up after a run (above). Actions 7 and 8
are a dash that nothing in the game begins: 7 (0xF58A8; its frame 0xF8D70) has
her move in status 2 the way she faces with animation 0x1D and the aura 999018
of the field's sixth bundle about her, to sound 0x38; 8 (0xF5B28, frame 0xF5C94)
is what follows it; and 0xF77A4, which begins another while square is held and
the stick pushed, at the cost of one of the player's figures (0x13, less
0x14) and no more often than tunable 0x28 allows, is called only from those two
frames. So the first of them never comes: square on the field is the map. Not
ported. Actions 1, 16, 17 and 21 are how a trap takes her from place to place:

**Taken from place to place** (`s234`, `s235`; `scene::Taken`, `Action::Leap`,
`Rush`, `RushStop` and `Flight`, `motion::step`). These are her jumps between the
levels of a map: 257 places given to 174 traps in 25 maps, 242 of them leaps, one
the other leap, 8 rushes and 6 flights. A map's script makes an
event's trap that a button works (`SET_GIMMICK`: `s67`, `s44`, `s45(id, 2, 1)`,
how she must be turned, and with `s45(id, 37, n)` a sign, 0xD3D18) and
gives it places with `s234(trap, way, x, y, z, x, y, z)` (0xC61D4, 0xE5E08: a
record of 0x40 bytes from the trap's + 0x130, counted at + 0x128): two points, and
a way of getting there that the script gives as 0 to 3 and the trap keeps as 1, 2,
0x10 and 0x15. The trap's callback then fixes the camera, calls `s235(trap, n)`
(0xC6384, 0xE5EC8; `n`, the count, is not looked at) and waits with `s99(0)`.

`s235` ends what she was doing (0xF463C), has her count of places (+ 0x500) be
nought and her trap (+ 0x504) the trap, and sets her off (0x109AC8). Setting off:
where she is going is the point of the place's line that is nearest to where she
is (0x108DFC), and the way she is to face is the angle between the straight way
there and +Z, to the side its X is on: an angle of the whole way, height and
all, not its bearing, so a place far above turns her less than would face it.
Her flag 0x73 is that another place follows, flag 0x72 that this is a leap (way
1) and the place before was flown to. Then by the way: 1 and 2 a leap (0xF7F04),
0x10 a rush (0xF650C), 0x15 a flight (0xF6944). When any action of hers ends
(0x1061B8, right after the action's frame, 0x106174) and she has a trap, the count
goes up and, if the trap has that many, she sets off for the next; else she stands
as after any action. Nothing ever takes the trap away again: the count just
goes past it.

- *A leap* (action 1, status 4; start 0xF7F04, frame 0xF8434, place 0xF5538). The
  speed is what her script gives for status 1 (32 a frame; int parameter 0xF
  chooses the status decimal 0x1C is the speed of, as 0xE does for 0x1B). With
  `d` the way and `h` its height: within 100 up or down it takes `|d| / speed`;
  higher, if steeper than 70 degrees `0.2 h / speed`, else the length of `d`
  with 500 more height, over the speed; lower, `sqrt(2 |h| / 100 / 9.80665) +
  0.5` seconds. She first gathers herself (animation 0x23, come over to in four
  frames) until the animation reaches the part of it that goes round (0xFE164
  with bit 1: 8 frames for her), turning evenly from the way she faced to the
  way she is to face. Then the effect 999016 at half size, 3 above where she
  stands, sounds 0x39 and 0x2D, and she is in the air: `from + d t/T + up × 100 ×
  (g T t / 2 − g t² / 2)`, `t` and `T` in seconds, so always the straight way with
  a throw on top that is `12.26 T²` high in the middle. For the first fifth of `T`
  the animation goes on going round (none of it for a leap down); then she comes
  over to animation 0x27 for the rest of the time. At `T` she is at the place. If
  flag 0x73 is set that is the end of the action. Else the effect again at full
  size, sound 0x39, she is put on the ground if there is ground within 40
  (0x108520), and she lands with animation 0x28 for as long as that is (13
  frames). A leap from where she flew to has no gathering: sound 0x2D, animation
  0x23 put at its end, and she is coming down from the first frame. (With a
  frame of the sixth stage's to change to, + 0x4C1, the way is timed by its
  length alone and the gravity is a tenth, and she changes frames as she
  leaves the ground: see "The frames".)
- *A rush* (action 0x10; start 0xF650C, frame 0xF66C4). Statuses 0xE and 0x10
  both 2, animation 0x21 over one frame, the aura 999018 going with her
  (0xE7DD8), and each frame the mover (0x101F04) with the straight way to the
  place as its direction, height and all, until the place is nearer than 120.
  Then the aura goes out over ten frames (0xE7904) and the action ends, or, were
  it the last place, action 0x11 follows (0xF6818, frame 0xF6888): animation 0xC,
  and the mover goes on for as long as that plays. No trap ends on a rush. The
  port's mover keeps to the ground, so `Mover::rush` stands in: the same speed
  and turning, straight through the air, and on the ground wherever there is
  ground within a step.
- *A flight* (action 0x15; start 0xF6944, frame 0xF6AFC). As a rush, with
  animation 0x1D going round, over three frames. Each frame her turns are set
  to a quarter about X, none about Y and a half about Z, and she is moved
  straight to the place by the speed of status 2 (decimal 0x1B), no mover and no
  map in the way, until it is nearer than 10; then her turns are none but a half
  about Y, and the aura goes out where it is (0xF6C98). (A step of 32 that passes
  the place and leaves her 10 or more beyond would go to and fro for ever; the
  port has her there instead.)

An action that another ends is ended by its kind (0xF4674, table 0x16D4F8): a
rush and a flight as above, a walk loses its dash, one destroyed is hidden, and
the rest just stop being busy.

**The effects of the field's own** (`field/effects.rs`). The field keeps thirty-two
objects for them (0xE7808 to 0xE82F8, at its + 0x26D0; 0x117A20 finds a free one).
One is given an effect of the field's own pack (0xE7C78, then made by 0xE78B4),
and is put at a place with a way it is turned and sizes (0xE7980; 0xE7FEC gives
other sizes), by a character (0xE7E4C) or by a node of a character's model
(0xE7D28). Each frame of the field's (0x1155CC, not while the field's flags 0x12,
0x15, 0x1D or 0x46 are set) every one that is in use has a frame (0xE80A8) by one
of the field's two times, as its setting 1 says: its own time goes on; once its
effect has played out it is done with; one told to go (0xE7904) has its effect's
own alpha come down to nothing over so long and is then done with; it is put
again; and its effect has a frame for each of the field's (0x127BA0 with the time
over 0x800). They are drawn after the characters (0x10E898).

The effects are in `SYSTEM.VOL/fld_resident.bin`: its file 3 is a bundle for each
of nine things (0x110104 gives one), each with its effect files in its own file 3:

| bundle | effects |
| --- | --- |
| 0 to 4, 8 | 999019, a twinkle: 8 is what `s53(id, 1, 8)` makes a character of, something on the ground to check |
| 5 | 999020 smoke, 509001 a burst, 510001 the ring of an appearing |
| 6 | 999016 what rises at a landing, 999018 an aura with a ring, for her own actions |
| 7 | 0 and 1: what a battle of note begins with (0x11ACDC) |

A character's script says how it comes and goes by data of these names (0x102738,
0x102888): the floats `pop_height`, `pop_radius` and `destroy_radius` (100 each
without; the enemies have 200 or 400, 150 or 200, 200 or 300), `shadow_radius`
(60), and the whole numbers `turn_frame` (34), `se_destroy` (none), `gim_facial`
and `type_destroy` (0: 3 for `G_BOX01` to `06`, `11`, `12` and `G_GAR01`).

**The camera's shake** (the camera's + 0xC50 on, in its frame at 0xDD6A4; `Shake`
in `field/effects.rs`). A point wanders in a round of one across: each frame it
goes 1.4 towards where it is going, or is there and is given somewhere else to go
(0xDBD88: any way from the middle, or about the way back through the middle,
four to ten tenths out). The camera is turned in its own frame, about the axis
across and the upright one, by where the point is, times how far it is shaken
each way (0.8 degrees until 0xDC018 says otherwise), times how strong the shake
is: coming over so many frames (0xDBF74), there, or dying away over so many
(0xDBFD0). A burst has it be 1.3 degrees up and down and 0.2 from side to side and
die away over 18 frames. A map's script has it come with `s228` (0xE5A5C: how far
each way, in hundredths of a degree, and over how many frames; over none it is
there at once, and either way the point starts from the middle) and die away
with `s229`: 578 and 444 calls, most of them in the scenes (0.8 degrees over
eight frames the commonest, and a faint 0.2 that stays for a scene's length).

### Characters that are effects

`s53(id, kind, n)` (0xC3FBC) makes a character that is an effect and not a model:
the ring of light about a home point, what swirls about her as it heals her, and
nearly everything a story scene has of smoke, fire, flashes and light (4,641 uses
in the 88 maps' scripts, most of them `s53(id, 2, n)`). It is a character like any
other to the scripts, which put it somewhere (`s48`), turn and size it (settings
14 to 16 and 88), hide it (setting 0), give it a contact so that a button does
something with it, and take it away (`s144`).

What it is made of is a resource of the field's list of them (0x116DD4 finds one
by its type and number), which has the characters' resources as type 0 and:

| kind | type | what | where |
| --- | --- | --- | --- |
| 0 | 1 | the map's own effects, by number (0xF1E28) | `FLD/MAP/MAP_<name>.BIN/04.bin/NN.efc` |
| 1 | 3 | the field's nine bundles of them (0xEFD44, see above) | `SYSTEM.VOL/fld_resident.bin/03.bin/NN.bin` |
| 2 | 5 | those of the event that is loaded, thirty-two at most (0xE95D4, 0xE9418) | `FLD/FEVT/FEVT_<name>.BIN/04.bin/NN.efc` |

Any other kind makes nothing. The maps' numbers are 0, 1, 2 and 4 (things of single
maps), 11, 12 (the ring about a home point), 13 (what heals her) and 14 (a yellow
terminal's); of the field's bundles only 8 is used, the twinkle, for something to
check on the ground in twelve maps.

0xE3028 makes the character unless there is one of that number already, or no
such resource: an object of 0x6E0 bytes (0x1029B4, its name `EFC` at 0x16D92C)
with room for eight effects at its + 0x510, 0x18 bytes each (how far a fade has
gone, how long it takes, which way it goes, flags, the effect's file, the effect
made of it), and how many of them it has at + 0x5D0. Set up (0x102E10), a
resource that begins `EFC` is its one effect; any other is a bundle whose file 3
has them (0x10264C, 0x102684). Every effect file has a single effect (the first
byte after its version), and the one bundle that is used has a single file, so
every such character in the game has one effect. Then, whether it has made one
or not, `s53` has the character of that number be something to look at (flag 3;
contact shape 0 of kind 1 and on, as settings 2 and 5 do) and sets its first
effect playing from the start (0xE5510, 0x1031B0, 0x102B2C: what was made of it
is unmade, 0x127B10, and it is made anew, 0x127A10).

Its frame (0x102FD4, from the character's own at 0xF5E50, which is not for
resources of type 2 and gives the model's frame to the rest) works out where it
is (0x102A2C) and gives each effect that plays that (0x127960), a colour of all
ones with the fade's part for its alpha (its + 0x120), and so much of a frame as
the character's time has gone on by, of 0x800 (0x127BA0 with the time times
thirty over 0xF000). That is whether it is hidden or not: one that is hidden
goes on, and is only not drawn (0x103074 from 0x1067F8, for a character that is
ready and has not flag 4, which is in reach of her and of the camera).

Where it is: as any character is put, by its place, its turns, its frame's turn
and its scales; and if its flag 0x5C is set (`s220`, 0x108AB0: the character at
its + 0x414 and the node at + 0x418), that from where the node of that number
of that character's model is (0xFF7D4: the first node whose number it is, 0x1E20C,
so 255 is the first node that has none; the model's matrix for the node as it
last was) rather than from where the world begins. It is not so fastened if there
is no such character, if that is an effect too, or if its model has no such node.
The library's `EFF_ATTACH(id, kind, n, chr, node)` is `s53`, `s220` and setting 72
to 0. Soldiers' guns have their flashes on node 50, and so on.

An effect is faded by a way (1 it is there, 0 it is not) and a count of frames:
going to the other way, its alpha goes from one to the other over so many frames
(0x102CF0, and from wherever a fade that is going on has it); and it can be had
be either at once (0x102DEC). The scripts' functions for that and for the other
seven effects take the character and which effect: `s215` sets one playing,
`s216` stops it (0x102B9C), `s217` waits for it to have ended (bit 0x1000 of the
effect's + 0xA0), `s218(id, n, way, frames)` fades it (the script's 0 is there
and 1 gone), `s219` waits for the fade, `s221(id, n, way)` has it be so at once.
No script of the game calls any of them. The field's own code does (0x10DBEC):
as she comes into reach of a contact of kind 1 whose character is an effect, its
second effect is set playing, or faded in over thirty frames if it was fading,
and as she leaves it is faded out over thirty. No character has a second effect,
so that does nothing either.

In the port: `scene::EffectOf` on a `Character` (`of_effect`, `begin_effect`),
`s53`, `s220`, `s222` and `s280` in `field/host.rs`, the event's files in
`field/staging.rs`, and `of_characters` in `field/effects.rs`, which plays the
file's effect where the character has it, a frame for each of the character's,
drawn while the character is not hidden (`Playing::unseen`). Not ported: the
other seven effects and the fades (nothing uses them). Setting 123's order of
drawing is in the table of settings, and setting 124's lens in "Its lenses".

## Touching and looking

The field's normal state (0x10D588) asks the collision system for the first thing the
player is in contact with (0x118330). A character counts if its flag 0x78 is set and
0x79 and 4 are not, and, if its flag 0x76 is set (containers), only while the player
is in her attack action (7); it starts `CALLBACK_NPC(id)` (0xE84A0). A trap counts
unless it is off; it starts `CALLBACK_TRAP(id)`, or `CALLBACK_NPC` with the character
it belongs to (0xE852C). The character whose event runs has its behaviour paused
(0x107828) until the field is back in its normal state (0xE8980).

What she is in touch with is kept in a list (field + 0x1DF8), which 0x10DBEC goes
over every frame: a character whose shape she stands in (0x10D18C: never one a
behaviour script runs, flag 0x24; 0x109CF0 tests her place against its shape by
its decimal parameter 0xC), and a trap that is not off whose box she is in,
turned as it asks (0x10DF38, 0x117274). A thing is put at the end of the list as
she comes to it (0x11709C) and taken out as she leaves it (0x1170F0). Each has a
*contact* (0xD3750): + 0 flags (0 a button does something with it, 2 it is in
the list, 3 and 4 she is in touch with it now), + 4 whether it is a character's
(1) or a trap's (0), + 8 whose, + 0xC four records of eight bytes, + 0x38 the next
in the list. A record is a word whose bit 0 says there is something to do, and a
byte, the word of the sign for it. What starts of itself is found first, and then
what a button does: see "The sign of what a button would do". The port stands in
distances for the collision shapes: a character is touched within its touch
radius.

## Traps and walls

`s43`, `s67` and `s118` make a trap (a map change, an event trap, a camera trap);
`s44(id, x, y, z, a, b, c, angle)` gives it a box and `s45(id, what, value)` a setting
(4: off when 1; 0 and 1: the word of the sign for what cross and square do with
it; 2: a button does it, and then walking into it does not; 31: how far from the
way its box faces she may be turned, in degrees, see "The sign of what a button
would do"; 6 to 28: what it gives the camera, see "The camera"; the rest is not
ported: 38 the other way of having it fire as it is walked into, 5 a margin
about its box, 3 and 29 on things of the map's that a trap can stand for). The box (0xED4CC) is centred on (x, y, z), reaches a, b and c to
either side along its own axes (100 where one is not positive), and is turned `angle`
degrees about the vertical. A point is inside when, taken into the box's frame, each
coordinate is within its reach (0xED640). On stage 1-1 the three event traps lie
across the street like gates, which fixes the turn's sense: the box's own x axis is
the world's turned by `angle` the usual way.

The margin, read on 2026-10-08 (the port needs nothing for it, as the end of
this says): `s45(id, 5, n)` (0xC8684,
0xE2280) keeps `n` as a decimal at the trap's contact + 0x30 (0xD381C), sets the
contact's flag 5 and makes the trap's second box anew (0xD3BA4): the second box
(0xD3AF8) is the first (0xD3AF0) copied, with the margin added to its reach along
its own x and z (+ 0x10 and + 0x18; not up and down), and only while the contact's
flag 1 is set. Where what she touches is looked for (0x10DF38, at 0x10E3B0), a
contact with flag 5 is also tried against that second box (0xED640): being in it
sets the contact's flag 4, as being in the box proper sets flag 3, and either
counts as touching. The whole of 0x10DF38(trap, character), which is where a trap is tried against
her: flags 3 and 4 of the contact are cleared; she is in the box proper when her
place is (0xED640) and the trap can be used (0x117274), and, for a trap of type 3
(0xD3B00: one that takes her somewhere) only when there is ground and room where
it would take her (the two blocks with 200 and 300, and 100 and 700, by the
trap's kind 0 or 1, 0xD3B9C; they also turn her, 0x106D7C). Then, without flag 5,
being in the box proper sets both flags 3 and 4; with flag 5, flag 3 is the box
proper and flag 4 the second, wider box. It answers whether either is set.

What goes by which (0x10DBEC, from 0x10DE48 for a trap): a trap that answers and
that she is turned for (0x117274) is put on the list of what she is in touch
with (0x11709C, which sets the contact's flag 2) and the camera is told she has
come into it (0xDAF78); one that no longer does is taken off (0x1170F0) and the
camera told she has left (0xDAFF8). So the margin is how far out she still
counts as in touch with it, and as in it for the camera. But what a trap does
goes by flag 3, the box proper: the search for what a button would act on
(0x11743C) and the search for the trap that fires by being walked into
(0x118330: on the list, flag 3, the trap's flag 2 and not its 0xD3F9C with 2)
both pass over a contact without it. Every map change is made with a margin of
400 (`SET_MAP_CHANGE`'s last figure; 123 calls of `s45(id, 5, ..)` in all), so
an exit fires where its box is and not 400 sooner: on 2026-10-08 the port was
for a moment made to fire by the wider box, and that was taken out again. There is no
latch either: the field's frame (0x10D624) brings the list up to date and then
asks, every frame, for the callback of the trap that search finds (0xE852C);
0xD3F9C with 2 is only a figure of the trap's (its + 0x430) that a script can
set. So for an exit the margin changes nothing that fires; what it changes is
who is on the list, and with it the figures a trap gives the camera, which
would hold out to the wider box. Only the ways out are given a margin (always
400, and none of them has figures for the camera), and their callback walks
her 400 on before it changes the map (see "The ways out"): the margin is what
keeps her in the exit while she is walked on past its box. Nothing else reads
the list (0x116FF8 only unlinks a contact that is unmade). The port goes by
the box alone, and since nothing there comes of leaving an exit's box while
its callback runs, there is nothing for a margin to change in it. The contact's flag 1, without which the
second box is not made, is set where a trap is made (0xE1DC4) and by `s45`'s
case 3 (0xE2238). Listings: `tools_re/out/trap_margin*.txt`, `trap_touch.txt`,
`trap_flags*.txt`.

`s185(id)` makes a character with no model (0xE30D4) and `s116(id, x, y, z, a, b, c,
angle)` makes its collision shape a box in the same terms (0xE4214): a wall, which
nothing walks through while it is solid (0x108B18: not hidden, flag 0x18 not set).
The first mission has two: 800 across the street by the crates, 810 by the trailer.

**What stands in a character's way.** Every character has a shape (0x40 bytes at
its + 0x460, set whole by 0x108C18): a kind, flags, three sizes, three turns and a
place. As a character is made (0x109A08) the kind is 0 and flag 2 is set: a round
of its collision radius (its + 0x170, 40 from the field's `prm`, which only
setting 70 changes and only ever to 40) and a hundred high, upright where it
stands (0x108CA0 fills that in). `s116` makes it kind 1, a box, with no flag: the
box lies in the world. `s236(id, x, y, z, a, b, c, angle)` (0xC4F24, 0xE42F8)
makes it a box with flag 2: about a place of the character's own frame and turned
in it, which is what a box she can open has (50 by 100 by 70, thirty behind it),
a container and a barrier (140 by 200 by 140, sixty up) and a generator (60 by
100 by 170); a size that is not positive is a hundred.

A character that moves (0x10194C) has a list made of the others (0x10231C: every
character but itself that is solid, 0x108B18, which an effect is only with its
flag 0x4E, which nothing sets), each with its shape and, for flag 2, the inverse
of its matrix without its sizes. Its sphere, of its own radius, is then swept
from where it is by how it moves, against the map (0x17910) and against each of
those (0xE0FB4: 0x15978 for a round, 0x1642C for a box, in the shape's own frame
where it has one), and slides along what it hits, twelve times at the most
(0xE1268). The camera's own sweep (0xE0650) goes through the same list and takes
only the shapes with flag 1, which none has: the camera is kept in by the map
alone.

In the port: `Character::solid` and `scene::Solid` (a round or a box), which
`step_characters` gathers each frame and `motion::walk_on` keeps a mover out of.
It is a stand-in for the sweep, as the walls' is: a mover that has come into a
round is put straight out from its middle to the two radii, and out of a box by
its nearest side, so it slides round both much as the original does.

## The camera

The camera is a record of figures (field + 0x180 + 0x590, 0xB0 bytes; 0xDC428 makes
it, anew for every map) that scripts set by number, and a frame of its own (0xDC880,
from the field's frame) that places it by its kind. `s49(kind)` (0xDC290) picks the
kind: 0 follows the player, 1 stays where it is put, 3 and 6 look at her from a
point, 4 plays a scene's cut (see "Story scenes"), 5 goes along a path, 7 is 0 by
another name (`SET_AROUND_CAM`). It also stops whatever was moving the figures
(0xDA9EC). `s79(what, value)` (0xC4484) sets a figure:

| what | slot | figure | the record at first | the script's value at first |
|------|------|--------|---------------------|-----------------------------|
| 1 | 0 | how wide it sees, in degrees | 72.3 | 72.3 |
| 2, 3 | | how near and how far it sees | 50, 20000 | |
| 5, 6, 7 | 1–3 | where it is | 0 | 0 |
| 8, 9, 10 | 4–6 | its pitch, yaw and roll, in degrees | −7.6, 0, 0 | 0 |
| 11, 12, 13 | 7–9 | what it looks at, from her feet | 0, 110, 0 | 0, 150, 0 |
| 14, 15, 16 | 10–12 | where it stands from that, before it is turned | 0, 0, 492 | 0, 0, 492 |
| 17, 18, 19 | 13–15 | the point a camera looks at her from | 0 | 0 |
| 20, 21, 22 | 16–18 | how far the picture is moved over, as the camera sees it (0xDE310) | 0 | 0 |

A new map's camera is of kind 1, at the origin, until a script or a trap makes it
another: on stage 1-1 the script does (`SET_FOLLOW_CAM(0, 0, 500, 0, 47, 0, 140, 0)`
is offset, pitch, yaw, look-at) and so does the trap she starts in.

**Its lenses** (0xD9D54, whenever a figure of the lens is set, 0xD99BC). Three
are made of the record's width (+ 0x68), the screen's shape (+ 0x74) and how
near and how far it sees (+ 0x6C and + 0x70: `s79(2, n)` and `s79(3, n)`,
0xE28B4 and 0xE28EC, which have no slot, so nothing moves them and no trap
gives them; 50 and 20000 in a new record, 0xDC6E0; kept and taken up with the
rest of the record, 0xDA33C). At the camera's + 0xD0 the one the map proper
and its people are drawn through (0xD9F88, from 0x10ED1C and 0x106994), from
the near figure to the far one; at + 0x110 the one what stands far off is drawn
through (0xD9F48, from 0x10ECC4: the map's second piece, the sky and the
like), from 50 to 200000 whatever the figures; at + 0x150 one from 1 to the far
figure, for the characters that are effects and ask for it (0xD9FC8, by
0xE7568 with something given: 0x103010, 0x103094, where flag 0xA7 is set) so
that the near figure does not cut them. A scene's cut (kind 4) makes the same
three with its own lens for the width (0xD9E44, 0x24D0C) and the record's
distances. 0xE7568 is what chooses for effects: given nought it takes the
scene's lens (+ 0xD0), given anything the close one. The field's own effects
(its slots at + 0x26D0, drawn by 0x10E898 once the characters that are seen
through have been) are drawn after 0xE7ACC has called it with nought, and so is
the list of characters put in order (0x1153A8): the scene's lens for all of
those, the near figure and the far one both.

Scenes use the near figure all the time: the first map's script sets it 23
times, to 10, 20 or 25 for a close shot, back to 50, and to 150, 200 or 300 to
see past what stands between the camera and what it is to show. The port has
one lens where the original has three, from 1 (`CAMERA_NEAR`, the close lens's)
to 200000 (`CAMERA_FAR`, which the far pieces need: on stage 5's maps they lie
as far as 71000 from the middle; a test goes through all 88), and what is drawn
leaves out for itself what its own lens would not show, point by point, by how
far before the camera each point is: every one of the three shaders has a
`reach` of how far and from how near. The scene proper has the two figures
(`MapMaterial::seen_between`, `map.wgsl`; `end_the_map` in `field/mod.rs`
gives them whenever they change), and what is far off about the scene is seen
from 50 with no far end (`FAR_OFF_FROM`). The characters have the two figures
(`Lit::reach`, `lit.wgsl`, from `light_of`). The effects have them by their
lens (`brs_engine::effects::Lens` on a `Playing`, `EffectReach`, `effects.wgsl`):
the field's own and the characters that are effects the scene's
(`Lens::Scene`), a character with flag 0xA7 the close one (`Lens::Close`: no
near cut but the lens's own 1), and what a battle or a ride plays, which have
one lens, all of it (`Lens::Whole`). What is drawn on the screen (the HUD, an
effect's screen pieces) is before no lens.
The maps' scripts set the figure 236 times in 66 maps: 20000 seventy-seven
times, 23000 and 28000, 70000 and 80000 where a scene looks far, 150000, and
10000 for the fourth stage's woods. A camera made anew has the lens it is made
with, fifty to twenty thousand (0xDC428, at a map's start, 0x115B38, and when
the field is taken up again after a battle, 0xEFB60), until a script says
again; so has the port's (`Camera::new`, `resumed`). A character's round
shadow is drawn through the scene's lens as the character is (0x106994); the
port leaves one out whole when its middle is not between the two figures
(`seen` in `field/shadows.rs`), where the original would cut it across.
Looked at on 2026-10-09 with the figures forced for one build
(`docs/compare/lens_cuts_port.png`: as it is; far at 2500, where the street's
far buildings go and the sky stays; near at 900, where the ground before her,
she and her shadow go).

Beside the record the camera keeps nineteen slots (camera + 0x7A8, 0x24 bytes each),
one for each figure but the two distances: flags (1 moving, 4 a trap that gave it
was left, 8 a trap that has it was come into), **what the script has it at** (+4),
where a move started (+8), the frames gone and to go (+0xC, +0x10), **where it is
going** (+0x14), the trap that gives that (+0x18), the trap last come into or left
(+0x1C) and how it moves (+0x20). The script's value is what a script last set
*while no camera was put by*: every setter writes the record and then, if the depth
is nought (0xDA450), the slot (0xDAE94), and likewise the kind (camera + 0xA58,
0xDAF20) and the path (+0xA5C).

`s139(what, value, frames, ease)` (0xC8D30, 0xDC3B0, 0xDB21C) has a figure go to a
value over so many frames of the game, from what it is: the camera's frame moves it
on (0xDCC60), evenly (ease 0), slowly at first (1: the square of the part of the
time gone), slowly at the end (2), or both, each half of the way in half of the
frames (3); with any other number it stays put until the frames are up. An event's
script has the same three as its functions 0x59, 0x4F and 0x5A (0xD6470, 0xD73CC,
0xD6494; 0x4F's number 26 is the kind, and 0x5A's value is a decimal), and its
library wraps them as the map's does (`COMM_SET_FIX_CAM`, `COMM_CAM_MOVE_FIX`,
`COMM_SET_FOLLOW_CAM`). `s124(path)` (0xE2F34) makes it a camera on a path.

`s50` puts the camera by for an event (0xDA324: the record is copied and the copy
used, two deep at most) and `s51` takes it up again (0xDA40C), when nothing of it is
moving any more (0xDA3A8).

**The field of view is the screen's width.** The figure goes as it is into the
projection (0xD99BC to 0x239BC), whose first scale is the cotangent of half of it
and whose second is that times the screen's shape (480 / 272): so 62 degrees, which
every event leaves the field with (`EVENT_END1`), is 37.6 from top to bottom. The
record's own first value is 45 degrees from top to bottom made into a width
(0x249E8). The port took the figure for the height until 2026-10-06, and showed
the field far too wide.

**The camera that follows** (kinds 0 and 7: 0xDE974, 0xDECBC, 0xDB528, 0xDF28C,
0xDF6D4). What it looks at is her place and the look-at figures (0xDB29C). It
stands at that point and the offset turned by the pitch and then the yaw (0xDB528:
a pitch below nought raises it; the pitch is the record's as a scene has it
lean, below), so the yaw is the angle of the camera's place
about her and behind her is her facing plus 180°. A sphere of radius 10 (camera
+ 0x7A4) is swept from the point to there against the second of the map's two
collision files alone (field + 0x118, slot 6 of the map's bundle; the first, slot 3,
at + 0x114, is the ground), and the camera stops where it touches (0xE0650). Its
view is from there at the point, with the world's up (0xDD99C); closer than 100 it
looks the way its angles have it instead. L and R turn the yaw (0xDECBC: rates in
`prm` at +0x2C, +0x30, +0x34, at most 7° a frame), and the turned yaw is written as
the script's value too (0xDF24C).

It gets to that place with a lag (0xDF28C: camera + 0xBE0 where it is, + 0xBF0 its
speed). Each frame the speed towards the place changes by at most 36 (camera
+ 0xC04): while the place is further than twice what the speed covers in the
frames a start from rest would take (the root of twice the distance over 36), and
further than 10 (+ 0xC00), it gathers; otherwise it slows by what stops it there
(the square of the speed over twice the distance). Speed across the way dies out,
by 36 a frame at most; nearer than 0.1, or than a frame's move, it is there. A
cut (a new kind, a camera taken up again) starts it from the place (0xDBB58).

**Where the view is from** (0xD9B14) is between the two. It starts from the
place worked out where that is no further from the point looked at than the
lagging place, give or take 0.5 (5 in a frame in which the walls stopped the
camera, its flag 0x1C), and from the lagging place otherwise; and from there it
is drawn back towards the lagging place by two counts of the camera's, their
shares multiplied: + 0xC58, which goes up to 32 a frame at a time while the
walls leave the camera alone and down to nought while they stop it (0xDE9EC),
and + 0xC5C, which goes up to 16 while the lagging place is 18 or more from the
place worked out and down while it is nearer (0xDF340). Both are nought when the
camera is made and when one is taken up again (0xDA3C0), and a cut leaves them.
So in the open the view trails her whichever way the camera has to go, once it
has been well behind for half a second; near a wall, and when it is close
behind, it is where the figures have it. The lagging place itself is never put
anywhere by this: it goes on after the place worked out.

After the lag has moved it (0xDF6D4, each frame), the lagging place is put on
the side of her that the place worked out is on. Both are taken from the point
looked at, with what they have along her frame's upright left out (0x1090E0
turns the upright (0, 1, 0) of 0x16CF80 by her frame); if the two ways differ
(their product under 0.99999) the lagging one is turned onto the other, about
what is square to both, or about the world's upright when it lies right behind
(product at most -0.99999), by a turn made of that axis and the angle between
them (0x162DE8, which makes the axis one long). So the camera lags only in how
far off and how high it is. Where the walls stopped the camera this frame
(camera flag 0x1C) and the place worked out is nearer the point than the lagging
one, the lagging one is brought in to that distance. One that is at the point,
or straight over or under it, is simply the place worked out.

**Its pitch leans.** The pitch its place is worked out with is not the record's
alone: 0xDB528 (and 0xDB374) add the camera's + 0xBDC to it, and that is worked
out first in each of its frames (0xDE974, the first thing 0xDC9E0 calls; also as
its kind is set, 0xDA2F4). 0xDB748 works out where the record's pitch alone
would have it, as far as the walls let it, and says whether they stopped it (a
count at the camera's + 0xC58 goes down to nought while they do and up to 32
while they do not, and camera flag 0x1C says which). Then, with the pitch P and
the ground looked for under that place from 100 over it to 1000 under (the
map's first collision file, the field's + 0x114), S is the pitch of the way
from what it looks at to the place, the place taken as much lower as the ground
under it is higher than her feet (where there is no ground, no lower):

    + 0xBDC = (1 - level) x (P + (S - P) x rate),

the rate being the camera's + 0xBD0 where S is under P and + 0xBD4 where it is
over, and the level its + 0xBD8. All three are nought as a map starts
(0x1116F8) and are for the map's scene script to say (its functions 22 and 23,
0x1120D4 and 0x112140, in hundredths): eight maps give rates (`STG01_04` 1 and
0, `STG01_0C` 1 and 0.3, `STG01_0D` 0.9 and 0.2, `STG02_08` 1 and 1,
`STG02_09` 1 and 0.5, `STG03_0C`, `STG03_0D` and `STG05_03` 1 and 0) and none
a level. On level ground S is P, so + 0xBDC is the pitch itself whatever the
rates, and on those eight maps it is steeper where the ground behind her is
higher than where she stands.

**What is not understood.** Read to the letter, the place is worked out with
the record's pitch *and* + 0xBDC, which on level ground would be twice the
pitch (the commonest setting of the later stages, `SET_FOLLOW_CAM(0, 0, 600,
-7, 352, 0, 150, 0)`, 14 degrees down). The game does not show that: at the
start of mission 24 (`STG02_06`, a camera with a pitch of its own; the
walkthrough at 8621 to 8622.5 s, `docs/compare/follow_pitch_walkthrough.jpg`)
the arrow over the street and the foot of the far wall stand where the port
has them with the record's pitch alone, and a twentieth of the screen off
where it has them with twice that (`docs/compare/follow_pitch_before_after.png`,
left and right). So one of the two is not what it reads as, and which has not
been found (the record's + 0x10 is written straight from the figure, 0xDA748).
The port goes by what is seen: + 0xBDC is the pitch, the record's own counting
once.

In the port: `Camera::lean` and `leaning` (`field/camera.rs`), `slope_under` and
`following` (`field/mod.rs`), the rates read by `field/scenery.rs`. On level
ground it is as it was; the lean on the eight maps is from the code and has not
been looked at against the walkthrough.

**The camera that stays put** (kind 1, 0xDDC00) is at its place, turned by its
yaw, then its pitch, then its roll (0x24B8C), a pitch below nought looking down:
what the opening's radio talk is shot with.

**The cameras that look from a point** (kinds 3 and 6, 0xDDE44). The point is
figures 17 to 19 and what they look at is her place and the look-at figures. Kind 3
is at the point; kind 6 is on the line from the point to her, as far off what it
looks at as the offset's third figure. Either is turned to what it looks at: the
yaw is the angle of the way from there to the point, the pitch its slope, and both
are written into the record, with (kind 3) the distance as the offset's third
figure. The roll is the record's.

**The camera on a path** (kind 5, 0xDAA48). Slot 8 of a map's inner bundle is a
bundle of paths, each a bundle of three: a `CPMD` grid over the ground whose points
say how far along the path the camera is with her there (0x18254, see
`docs/FORMAT_NOTES.md`), an `INSM` camera rig and the `INSA` move of it. The frame
of the move is its first plus that part of its length (0x110C4C gives the two), the
camera is where the rig's node is then and turned as it is (the camera's + 0xC10),
and its yaw goes into the record with no pitch and no roll (0xDE2FC). The angle of
its lens does *not* become the camera's: it is only written as where slot 0 is
going (0xDAC68), so it shows if that figure happens to be moving. 110 of the
game's 192 camera traps are of this kind, on 40 maps; stage 1-1 has one over the
fallen building in its south-west.

### What a trap gives the camera

Any trap can carry figures for the camera (`s45(id, what, value)`, 0xC8588, which
numbers them otherwise: 6 the width, 7 to 9 pitch, yaw and roll, 10 to 12 the
place, 14 to 16 the look-at, 17 to 19 the offset, 20 to 22 the point, 23 to 25 the
shift; angles in degrees), a kind (26), a path (27), the frames a change takes (13;
a trap is made with 90) and how the change goes (28: one of the four ways). The
map's library has one function for each kind: `SET_CAMTRAP_FOLLOW(id, offset,
pitch, yaw, look-at, frames)`, `SET_CAMTRAP_FIX`, `SET_CAMTRAP_FIXF` (with a width),
`SET_PATH_CAMTRAP(id, path, frames)`, `SET_CAMTRAP_POINT`, `SET_CAMTRAP_POINT_OFS`.

The field keeps a list of the traps she is in (field + 0x1DF8), in the order she
came into them. While she is the player's to move, each frame every trap is looked
at in the order the traps were made (0x10DBEC): one she has come into goes on the
list and its figures' slots are marked as come into (0xDAF78); one she has left
comes off and the slots it gave are marked as left (0xDAFF8); one that is switched
off just comes off. After each coming or going the camera settles what it is to be
(0xDE460):

- Every slot is to go to the value of the first trap on the list that has it, else
  to the script's value; the kind and path are the first listed trap's that has a
  kind, else the script's.
- If the kind is to change, every figure is that at once, the view goes over from
  what it was to the new camera's in the frames (and the way) of the trap the old
  kind was from, or else of the new one, or else 30 (0xDB11C), and the camera is of
  the new kind. Going over means the place goes evenly, and the turn by the
  shortest way, from the view kept to each frame's new one (0xDCEDC); with no
  frames it is a cut.
- If not, the slots marked as come into (where the trap come into is the one that
  now gives them) and those marked as left go to their value over that trap's
  frames; but a camera that follows her keeps its yaw, which is the player's.

When an event is over the same look is taken at once (0xE8C6C, 0x10CFC4): the
figures are what they are to be with no frames taken, and a camera whose kind
differs only moves the figures the old kind worked out for itself and the new one
goes by (0xDE39C). That is how a map's first camera comes about: she is found in a
trap, or the script has set one.

**After a battle** the original makes the field's camera anew (0xEFB60) and gives
it the kind the script had the old one at and, by that kind, the script's values
of its angles, of its place or of its point (0xE8A84, from what the camera's frame
saves every frame, 0xDCB4C); it then finds the traps she is in again. Everything
else is as a new map's: so the 62 degrees an event left are 72.3 again until the
next event, which is why she looks smaller on the field after a battle (walkthrough
at 800 s; `docs/compare/after_battle.png`).

In the port: `field/camera.rs` is the record, the slots, the moves, what traps give
and the settling (`Camera::resolve`), with the lag (`Lag`); `Scene::walk_traps`
goes through the traps; `field/mod.rs` has the view by kind (`sight`), the frame
(`spin_camera`) and the drawing between frames (`follow_camera`);
`Lag::kept_behind` is 0xDF6D4 and `Lag::step` ends with 0xD9B14; the counts are
put back to nought when a camera is taken up again or is a new one
(`Camera::renewed`, `Lag::renew`).

Left out, there being nothing to it: what field flag 0x50 does to the view
(0xDD0EC, in the frame of a camera that follows her). With her there (0x1179D8:
the character the field's + 0xE64 names, which is nought from when the field is
made, her own number) a count at the camera's + 0xC80 goes up to 30 a frame at
a time while the flag is set and down to nought while it is not. With `s` its
share of 30 and `k = 1 - (1 - s)^2`, the eye goes `k` of the way from where it
is to the point on the line from a hundred over her through the eye that is a
third as far from there as the eye is and 200 more (0xDD20C), and the view is
turned `k` of the way (0xDD3C0) to one that looks at a hundred over her from
there: the camera would come in on her and look straight at her. At nought it
is the view as it was. Nothing sets the flag: every call of 0x1176FC has its number written in
it but the one in 0xE6208, whose callers give it 0x54 (`s96(40)`), the four of
`s104` and 0x5A and 0x5B (0xE6650); none is 0x50, and the system script has no
80 in it. So the count is always nought and the view never changes by this.

**The view the stick goes by** (the camera's + 0x50; 0x101700 takes the stick's
way through its inverse, and 0x1017C0 lays that on the ground she is on). The
stick is read as two figures, left and up, which the D-pad's buttons make one or
minus one (0x1012D0), and by the mode at the field's + 0x2688 (0x101158): 0 has
it go by the view, unless she is in something that names an angle of its own
(0x117170, 0xD3D10); 1 by the angles kept at the field's + 0x2690. Each frame of
the camera (0xDC880) the view the stick goes by is made the view itself (its +
0x10), but not after a cut: when the view goes over to another with no frames
taken (0xDB11C), 0xDBC6C keeps what the stick went by (to + 0x90, and its way
over the ground to the field's + 0x26A0), clears a count (+ 0x26B0) and sets
field flags 0x4C and 0x44; while flag 0x44 is set the count goes up, and until
it has come to the 16-bit figure at + 0x40 of `prm` (45) the stick goes by what
lies between the view kept and the view (0xDFC48: turned from the one to the
other by the square of the count's share of that figure); then by the view
again, and the flag is cleared. So a push held across a cut carries her on the
way she was going, and comes round to the new view in a second and a half.

In the port: `Camera::stick_yaw` (`field/camera.rs`; the view's yaw is what the
port's stick goes by, and it is that which is turned), begun in `blend_over`,
moved on in `Camera::step`, read by `control_player`.

## A battle from the field

`s81(group, n, m)` (0xC47B8) asks for a battle against enemy group `group` and waits
for the field's flag 0x10 to clear. The request (0xD8364) takes the battle map from the
field map: `lst_map[map].battle` (e.g. 101 is `MAP_B_STG01_01`). The field stays loaded
while the "BATTLE TASK" runs (0xD84A0). The battle leaves its outcome at 0x2EAF8:
`s156()` reads the word at +4 (1 won, 2 lost, 3 got away) and `s157()` the one at +8.
The record is cleared as a battle starts and set to 1 and 3 (0x73EFC); a win leaves
those and adds its figures (0xA022C: + 0xC seconds, + 0x10 damage taken, + 0x14
damage dealt, + 0x18 enemies beaten), a loss sets both words to 2 (0x78E34). The
scripts look at `s157()` only after getting away, for 1. The field comes back dark:
the script fades it in.

She comes back with the HP the battle left her (see `BATTLE_NOTES.md`, "What a
battle is worth"), and with whatever experience, levels and items it was worth.

The library's `BATTLE_NORMAL_SYMBOL(group, _, symbol, m, fade, flag)` is what a touched
symbol runs: on a win it sets `flag`, destroys the symbol and counts records; on a loss
it runs `GAMEOVER`; after an escape with `s157() == 1` it calls `s132(symbol, 4)`,
which does nothing: the symbol is told of the escape by the system script instead
(action 8, its `int_battle_escape`).

## Enemy symbols

`SET_SYMBOL(id, chr, x, y, z, angle, flag, kind)` in the library makes one unless
`flag` is already set (it was beaten). Kind 1 is the roaming kind: `s52(id, 35, 0)`
gives it behaviour script 0 and `s52(id, 40, 1)` marks it. Kind 0 stands still and is
only touchable.

The behaviour scripts are three `SSCR` scripts in `SYSTEM.VOL/fld_resident.bin`, slot 4
(0: symbols, 1: nothing but standing, 2: another kind). A script is a set of states:
each `stat_…` function does what the state does, waiting inside engine functions, and
returns the next one.

What the engine keeps for a symbol (the block 0x1086EC returns, set up at 0xF9A00):
bits (1: an interruption named a new state, 3, 4: paused), the script, three byte
variables, a route of up to 16 of the map's points, its home, its way of moving about
(int 0: 0 stands, the default, 1 patrols the route, 2 wanders), the hide and unaware
timers (ints 6 and 7), the character it is after (int 0x18, the player by default),
its notice range, its wander radius (300) and a pause at each route point (int 0x2D).

Its frame (0xF9C2C): paused, the character just has its own frame. Otherwise
interruptions first (0xF9CB4): `int_touch_player` if the field is in its normal state
and the player is within the touch radius (0xFB868), `int_touch_enemy` likewise for
another symbol (0xFB960); a state either returns becomes the state, which starts from
nothing (0xF9DE8) with the character's "acting" cleared. Then the state runs
(0xF9E6C) until an engine function waits; a state that returns is followed at once by
the one it names. The functions that wait give the character its own frame
themselves, once for each frame they wait, so a symbol moves only while its script
is in one of them.

The engine functions (table at 0x172900, 30 of them; `int_init`, `int_touch_player`,
`int_battle_escape` and `int_touch_enemy` are found by name, table 0x1728F0):

| n | what |
|---|------|
| 0, 1 | write and read a byte variable |
| 2, 3 | set and read one of its bits |
| 4 | wait so many frames (of the game's time, not its own) |
| 5 | the chase: see below |
| 6 | start the map's `CALLBACK_NPC` for it and wait for the event to be over |
| 7 | stand, wander or patrol until something: see below |
| 8 | it has noticed: a cry (sound 0x2F), a mark, flag 0x60 |
| 9 | start its state afresh |
| 0xB | where it stands becomes its home |
| 0xC | walk home; answers 1 if it notices its target on the way, 0 when within 120 of home |
| 0xD | read a character flag |
| 0xE | answers 0 |
| 0xF | the state it is in (a chase's own states do not count) |
| 0x10 | a debug print |
| 0x11 | an action (`ACT`, 0xD4D74): 0 walk, 2 turn, 7 stop, 8 `int_battle_escape`, 9 pop up, 0xA drop in, 0xB dash, 0xC another, 0xD the animation named, 0xE appear |
| 0x12 | a setting, by `s52`'s numbers (0x19 the pace) |
| 0x13 | set a character flag |
| 0x14 | set an int parameter |
| 0x15 | wait for what it is doing to end |
| 0x18 | a system value: 0x2E `prm[+0x4A]` = 15 frames, 0x30 `prm[+0x46]` = 100 percent |
| 0x19 | wait for its model to have loaded |
| 0x1A, 0x1B | set and read one of the game's flags |
| 0x1C | read an int parameter (0x18 its target's number, 0x29 its own pace) |
| 0x1D | one of the game's test switches (0x28: enemies ignore the player); none is on |

Function 7 takes bits: 1 answers 1 when it notices its target, 2 answers 3 when its
flag 0xA0 has cleared, 4 answers 0 when within 500 of its target, 8 has it move about
in its own way meanwhile. Each frame it counts the hide and unaware timers down
(0xFBD7C: a timer that runs out hides it if flag 0x7A is set, or clears flag 0xA0);
hidden, it does nothing else.

It notices its target (0xFB750) when the field is in its normal state, it is a symbol
and ready (0xFAAD4), its flag 0xA0 is clear, the target is nearer than its notice
range and the target is inside its territory: within decimal 0x1A (2000) of its home,
or of its route if it patrols (0xFA7EC).

Wandering (0xFAD18, 0xFAF38): a place at a random distance up to its wander radius
and a random angle from home, dropped onto the ground within 500 above or below; it
walks there, and picks another as soon as it arrives or after 300 frames.

The chase (function 5, 0xFBEBC) is given a state function. Its pace becomes its own
(int 0x29), it walks, and each frame its target is where the character it is after
now is and its target facing the way to it; the chase's states run inside it on the
same script. It ends, answering 1, when the symbol can no longer notice anything or
the target has left its territory (unless she is touching it): the symbol stops and
flag 0x60 clears.

Script 0, in short:

- `int_init`: remembers where it stands as home; `stat_pop` if flag 0x9C, else
  `stat_stable`.
- `stat_stable`: function 7 with 9 (moving about its way until it notices), then
  `stat_trace`.
- `stat_trace`: function 8, a wait of 15 frames, then the chase with
  `stat_trace_normal`; when that ends, pace back to 100, a wait of 60 frames, home
  moved to where it is if flag 0x9E, and function 0xC: `stat_trace` again if it
  notices her on the way, else `stat_stable`.
- `stat_trace_normal`: pace its own, walk, 20 frames, then function 7 with 4 (with 0,
  that is for ever, if flag 0xA9); then `stat_trace_dash`.
- `stat_trace_dash`: pace 100, turn to her, 8 frames, pace `prm[+0x46]`, dash for 15
  frames, pace 100, stop, 10 frames, back to `stat_trace_normal`.
- `int_touch_player`: `stat_event`, unless it is a symbol whose target is the player
  and it is not in `stat_trace`: an enemy that has not noticed her cannot be walked
  into.
- `stat_event`: function 6, then `stat_stable`.
- `stat_wait_escape` (from `int_battle_escape`, unless it is in `stat_pop`): stops,
  cannot notice for 60 frames (int 7, flag 0xA0), moves about meanwhile.
- `stat_pop`: the appear action, a wait for it, then `stat_stable`.

`int_battle_escape` is run for every symbol after every battle, by the system
script (below): unless it is in `stat_pop` it goes to `stat_wait_escape`.

**Patrols.** A map's script gives the field numbered points with
`s180(point, x, y, z, reach)` (0xC5AC4, 0x117974: sixteen bytes each from the
field's + 0x2484), a symbol its route with `s181(id, n, point)` (0xC5B00,
0xF9BA0: the route's n-th byte) and `s52(id, 39, count)`, and has it patrol with
`s52(id, 38, 1)` (int 0 by way of int 0x26, 0xFA124; `(id, 38, 2)` has it
wander, `s52(id, 131, radius)` saying how far). The scripts have a symbol
patrol in 70 places and wander in 55; most routes have two points, and the
scripts give the patrolling ones a pace of 200 (`s52(id, 25, 200)`), which
doubles how far they go in a frame.

The point a symbol was last at is its byte variable 2. Setting out (0xFABF4,
0xFAEA0) it walks to the point after that one, round the route, and bit 3 of
its bits is set. Where it walks to (0xFBAA0) is not the point itself but a place
any way from it, up to the point's fifth figure off (the scripts give the
angle the symbol was put at there, which the engine takes for this), put on
the ground if there is ground within 200 above or below. Each frame of
patrolling (0xFC4D4): while its walk lasts it goes on; when the walk is over it
has come to the point, which becomes the one it was last at, and it walks on to
the one after (0xFAC30), or first stands there for as long as int 0x2D says
(`s52(id, 132, frames)`, which no script uses). While the field is not in its
normal state a symbol stands where it is with flag 0x67 set (0xFAE58), and walks
on to the same point when it is again.

Its territory is then about its route (0xFA7EC, 0xFB378): the distance that
counts is to the nearest of its points, or to the nearest place on the stretch
from one point to the next where that is nearer by more than a hundredth. And
walking home (function 0xC, 0xFA894) is walking back to the route: to the
nearest point, or from between two points on to the second of them, that being
where it takes its route up again; it is home when it is within 300 of its
route.

Not ported: `int_touch_enemy`, and a count at the character's + 0x42C that would
have one that patrols give up a walk (nothing was found that ever counts it up).

## The field's system script

`SYSTEM.VOL/fld_pack.bin`, slot 8, is an `SSCR` script the engine runs for the field
itself: the game's own account of what a mission is. `tools_re/sscr_dec.py` turns it
(and any other `SSCR` script) into readable code; `tools_re/out/dec_fld_system.txt`
is its output. The port runs the script itself (`field/system.rs`).

The engine calls its functions by name, on a context of their own each time
(0x130CE8 with no argument, 0x130DA8 with one; the script is "kind 1" of 0x2148):

| function | called | what it does |
|----------|--------|--------------|
| `init_player` | by a new game (0x908), after variable 40 is set and before the title sets the first mission | variable 10004 = −1; variable 10014 = `prm[+0x54]` (300 frames); the pop timer is set to it and the pop distance to 0 |
| `sys_fld_enter_map` | when a map has been entered (0x115A34) | variable 232 = `prm[+0x58]` (3) |
| `update_player(record)` | every frame the field is in its normal state, at the end of it (0x10DBA4) | the four below |
| `update_mis_time(time)` | after a battle, with the battle's time (0xD8A38) | the mission's clock |
| `mission_update_info` | when a script sets system value 49 (0x117D2C) | gives window 5 the mission's line of that number and shows the display's first panel |
| `sys_fld_exit_field(how)` | when the field is left (0xE6E7C) | 3 and 5 restore the backup and make her whole; 5 (a retry) selects the mission again |
| `script_entry(n)` | by a map's script, `s247(1, n, 0)`, which waits for it (0xC656C, 0xD7D88) | 300 `sys_cmn_post_proc_battle`, 400 `mission_start`, 401 `mission_clear`, 403 `mission_fail` |
| `enter_stage_select`, `mission_select(n)` | by the screen where the next mission is chosen (the module at 0x5C000, through its "sysScrTask", table 0x172D30) | below |

The record `update_player` is handed (0x10D2C4): the field's two time steps (clock +
0x18 and + 0x14; 0x800 a frame) and how far the player is from where she was a frame
ago (the field keeps that place at + 0x2150).

What the functions do:

- `mission_select(n)`: backs the save up; variable 100 + n becomes at least 10 (a
  mission's state: 1 newly open, 2 open, 10 begun, 255 cleared); variable 10004 = n,
  220 = 100; the map and entrance to start on from the mission's record; flags 0 to
  5009 cleared; variable 10003 = the record's first story event; 230 and 10011 = 0.
- `mission_start`: 220 = 101; the mission's window (9) is given the mission's title,
  opened and waited for; then `mission_update_info`.
- `mission_clear`: 220 = 200; the display's panels closed; the clear window (12)
  opened and waited for, except for mission 43. `mission_fail`: the same with
  window 13, and 220 left alone.
- `sys_cmn_post_proc_battle`, after every battle: every character numbered 500 to
  599 is given action 8, which runs its behaviour script's `int_battle_escape` (so
  every enemy on the field stands down for 60 frames, whoever won); every one that
  popped up (3000 to 3099) is hidden unless it is being destroyed; the pop timer
  becomes at least 120 frames; in a "mazuma" mission (flag 4701) a counter is redone.
- `pop_enemy`, the enemies that come up out of the ground about her: while system
  value 40 is on and fewer than variable 232 of them are about, the distance she
  goes is added up; when it reaches `prm[+0x50]` (3000), or the pop timer has run
  out while flag 6178 is set, a place is looked for (below). If there is one, a
  character is made under the first free number from 3000, of a kind drawn from the
  map's list (variable 231 picks one of the 18 lists of `lst_pop`: eight kinds, each
  a character resource and its share out of 100), put there, given behaviour script
  0 and flags 0x9C (it comes up with its appear action), 0x27 (a symbol), 0x7A and
  hide timer 150 frames (it goes again if it has not found her), 0x9E, and the
  player's int parameter 8; then the timer and the distance start again.
- `update_pop_enemy`: one that popped up and is more than 3000 from her is hidden;
  the pop timer runs down while value 40 and flag 6178 are on.
- `update_weakness`: while variable 10011 is above zero, every second (value
  55 counts the frame's time up past 0xF000) it loses `prm[+0x4C]` (10) for
  each enemy within 300 of what she is guarding (value 53, 0x10D370), and the
  first number of the display's first panel is given what is left; at zero
  the field is to be left: value 57 is given value 52, the guarded character's
  own number, and value 56 is set, so that character's callback is started.
- `update_mis_time(time)`: while variable 10012 (thousandths of a second) is above
  zero it runs down by the time, and the first number of the second panel is
  given it; at zero the field is to be left with nought for value 57, her own
  number, so the callback is hers. In a "mazuma" mission a count goes up every
  180 frames.

Tried in play on 2026-10-09 (`docs/compare/guard_mission_port.png`,
`timed_mission_port.png`). Guarding: `--map STG02_09 --variables
10004:22,220:108 --skip-battles --skip-messages --script wait:5,seek:5,wait:70`:
with the first enemy beaten the map's script has four more come for the bike
(character 600: `s52(n, 126, 600)`, `s96(52, 600)`, `s24(10011, 500)`), the
display reads "Defeat all the enemies n/4" and "Bike endurance:" with its gauge,
the gauge runs down while they stand by the bike, and after eighteen seconds
the callback of 600 is started and MISSION FAILED comes. A time: `--mission 28
--skip-battles --skip-messages --script wait:80` (the map's script gives it
sixty seconds at its start): the clock counts down on the display, and as it
ends her own callback is started, in which that map's script brings the next
enemy on and gives another sixty. Neither has been found in the walkthrough
yet: mission 28 is none of the story's, and of mission 22 (6740 to 7869 s) what
was seen between 7300 and 7780 s were the grenades ("n/4", 7240 to 7605) and a
wave of three (7692 to 7734), with advertisements on almost every jump.

Tried again on 2026-10-13, each to its end. The guarding won
(`docs/compare/guard_mission_won_port.png`): the same start with a script that
runs at each enemy in turn and goes round the taxi that stands between
(`wait:5,seek:5,seek:3,seek:3,seek:2,to-300/-500:1.5,seek:2.5,to-700/-100:1.5,
seek:2.5,to-400/500:1.5,seek:2.5,...`): the count goes to 4 of 4 with the gauge
not yet out, event 695 plays and the display is back at "Determine where to use
the grenades 2/5". Mission 69, on `STG04_0A`, which its map's script gives 300
seconds (`--mission 69 --skip-battles --skip-messages --level 50 --script
wait:330`; `timed_mission_300_port.png`): the clock runs from 5:00, event 475
plays 300.5 s after the mission's start and MISSION FAILED comes. Mission 68,
on `STG04_0F`, the one against Mazuma, which has 180 seconds
(`mazuma_mission_port.png`): the display's two lines are the clock and "BRS 00
vs nn MZMA", the second number one more every six seconds (the system script's
`sub_16cc`, each 0x5A000 of the field's time) to 29, and at the end event 246
and MISSION FAILED. Her own number is the game's variable 237, which that run
had no fight to raise.
- `enter_stage_select`: makes her whole; if the mission was cleared (220 = 200) its
  state becomes 255, and every mission whose required missions (eight bytes at
  record + 0x1C) are all cleared and whose required flag (record + 0x28) is set
  becomes newly open; each newly open one is announced and becomes open.

A place for an enemy to pop up (function 0x3A, 0xD5470): the middle is `prm[+0x5C]`
(1500) ahead of her; a distance up to `prm[+0x60]` (300) and a starting angle are
drawn, and six places a twelfth of a turn apart are tried. A place will do
(0xD5260) if there is ground within 200 above or below it and a sphere of radius 40,
its middle 50 above that ground, touches nothing.

The script's engine functions are the field's general table at 0x1726D0 (117 of
them; `tools_re/out/sscr_field_natives.txt`; `tools_re/sscr_calls.py` lists the 43
the script calls). Those it uses:

| n | what |
|---|------|
| 0x07, 0x16 | set and read a system value (below) |
| 0x13, 0x18 | read and set a variable; 0x14, 0x15 a flag |
| 0x17, 0x4C | read and set one of the player's figures (0x9C4, 0xB98): 0 experience, 1 level, 2 HP, 3 maximum HP |
| 0x26, 0x39 | debug prints |
| 0x29 | finds a character by number and makes it the one the functions below work on (field + 0x1D4C) |
| 0x2A | an action, by `s132`'s numbers; 8 runs a symbol's `int_battle_escape` |
| 0x31, 0x32, 0x44 | the addresses of `lst_mission`, of one mission's record, of `prm` |
| 0x33 | the address of a function, by name, of a script of some kind (0x2148: 0 is the map's, 1 this script, 2 the script of the event that is loaded or of the ride, which 0x21FC keeps as each is made, 0xE9BD4 and 0x148D74. The system script asks kind 2 for `update_async_time` alone, and no script of the game has a function of that name: the port answers nought) |
| 0x34, 0x35 | set and read a float of the game's work area (index 0: the pop distance) |
| 0x36 | makes a character of a resource under a number (0xD69B0) |
| 0x37, 0x48, 0x38 | count the characters not hidden with numbers in a range; the number of the n-th of them; the first free number from one on |
| 0x3A, 0x3B | find a place to pop up at; put the character there |
| 0x3C, 0x3D, 0x3F, 0x49 | its behaviour script, a flag, an int parameter, a setting by `s52`'s numbers |
| 0x42, 0x4A | read a decimal parameter (0x29: how far it is from the player) and one of four other things (0x82 its status, 0x73 int parameter 8, 0x30 flag 0x54, 0x17 its resource) |
| 0x43 | removes the character |
| 0x45, 0x64 | the text of a window: by kind and number (kind 0: a mission's title, 0xECD04), or as given |
| 0x46, 0x47 | the window command (0xEDB24, below); wait for a window (9 mission start, 12 clear, 13 fail, 14 a fade) |
| 0x6B | draws the kind of enemy to pop up |
| 0x73 | sets something of the mission's display (0xE6650, below) |
| 0x66, 0x67 | back the save up, restore it (0x2398, 0x23C4) |
| 0x6A | three things to do with skills (0x14D73C, 0x14AEB4, 0x14A5DC) |
| 0x72 | makes her whole |
| 0x19, 0x1A | announce a mission newly open |

System values are the field's own figures, read and set by number: the system script
with 0x16 and 0x07, a map's script with `s47` and `s96` (0xC726C; `s96` has cases of
its own, in the second table below).

| n | what |
|---|------|
| 0, 1 | the map she is on and the entrance she came by; setting them names where to go next |
| 37 | time that runs in the field's normal state (field + 0x537C) |
| 38 | the pop timer (the game's work area + 0x20) |
| 40 | field flag 0x54: enemies pop up |
| 42, 43 | the map and entrance the field starts again on when it is left with `s173(2)` (field + 0x1D3C, + 0x1D3E) |
| 45, 46, 48 | `prm[+0x44]`, `prm[+0x4A]`, `prm[+0x46]` (read only) |
| 49 | which of the mission's lines says what is to be done (field + 0x1D40); setting it runs `mission_update_info` |
| 50 | the answer given in the window of a mission failed (field + 0x1D42); 51 reads field flag 0x3D |
| 52 | the number of the character she guards (field + 0xE68; -1 from when the field is made, 0x1181F0; 0x1179F4 gives the character) |
| 53 | the enemies about what she guards (read only, 0x10D370): the characters numbered 500 to 599 and 3000 to 3099 that are not hidden and no further than 300 from it; none where nobody is guarded |
| 54, 59 | set field flags 0x1A and 0x59 |
| 55 | the guard's one-second timer (field + 0x53A4) |
| 56, 57 | field flag 0x58, the field is to be left, and how (field + 0xE6C) |
| 58 | the mission's time left in seconds (variable 10012 / 1000) |
| 60 | a float (field + 0x26C0) |
| 61 | a byte of the map's record (read only) |
| 62 | the time between pop-ups in seconds (variable 10014 / 0xF000) |

`s96(n, value)` goes by a table of its own (0x16B7F0, for 5 to 60) before it comes
to those, and sets nothing for a number outside 5 to 60: the one call with 62
does nothing. What the maps call it with, and how often over all of them:

| n | calls | what (0xC726C's case) |
|---|---|------|
| 8 | 3099 | the event's frame count (0xE3D84) |
| 9 | 2040 | field flag 0x2B, the tune is kept (0xE3E38) |
| 11 | 1288 | how near the camera a character's model may be and still be drawn (0xE4A78: field + 0x246C, read by 0x1063B8; `Scene::too_close`) |
| 12 | 176 | field flag 0x31 is set with 0 and cleared otherwise (0xE4AA8): with it every character is drawn however near the camera (`field_flag::ALL_IN_SIGHT`) |
| 21 | 193 | field flag 0x32 likewise (0xE4AF4): with it every character has its frame and is drawn however far from her (0x106558, 0x1066A8; `field_flag::ALL_NEAR`) |
| 28 | 368 | field flag 0x47 likewise (0xE5978): with it nobody that stands idles (0xF8FE0 stays at plain standing, as for a character with flag 0x62), and as it is set every character that is not acting is stood still (0xF6004); see "Idling" below |
| 30 | 176 | field flag 0x4A (0xE5DD4): with it a character with flag 0x26 that `s70` starts an animation for (0xE32EC, from 0xC42DC) has its matrix worked out there and then (0x106260: where it is, how it is turned and sized, into + 0x120, and its model's by 0xFD24C) and what goes with it placed (0x108BE8), not at its next frame. The port places every character anew each frame from what it is, so there is nothing for it to do |
| 40 | 316 | field flag 0x54, enemies pop up |
| 41 | 9 | a byte of the game's work area (0x22D8: + 0x2073C) |
| 13 to 16 | none | 0xE4B90 with 0 to 3 |
| 42, 43, 49, 52, 54, 58, 59, 60 | | the field's figures above (0xE63FC) |

5, 17, 22 to 26 and 29 have cases too and no map calls them. Since everything takes
part in the port whatever its distance, 11, 12 and 21 have nothing to change there.
An event begins with `s96(28, 0)` and ends with `s96(28, 1)` (`EVENT_END1`), so
nobody idles in a scene.

**Idling** (0xF5F78, each frame of a character whose status is 3, standing: it
calls 0xF8FE0 in place of the model's plain frame). Nothing happens while field
flag 0x47 or the character's flag 0x62 is set, nor for one that is not a plain
model (its flag 0, its resource's type 0) or has neither animation 0x10 nor 0x11.
Otherwise, by its state (+ 0x198):

| State | What |
| --- | --- |
| 0 | counts frames (+ 0x41C) up to its wait (+ 0x420); then, while its animation has not come to its end (0xFE080: bit 0x10 of the animation's flags), tells it that this round is the last (0xFE3F4 with 0, which clears its bit 8); when it has ended: with 0x10 and no 0x11, plays 0x10 once and state 3; with both, 0x10 once and state 1; with 0x11 alone, 0x11 going round, a new wait from `prm[+0x1C]` to `prm[+0x1E]` (60 to 120 frames) and state 2 |
| 1 | when 0x10 has ended: 0x11 going round with that wait, state 2 |
| 2 | counts up to its wait, has the round be the last, and when it has ended: 0x12 once and state 3 if it has it; else its stand (0x1083AC with 0, going round, come to over the frames of 0x1084B0), a new wait from `prm[+0x18]` to `prm[+0x1A]` (120 to 240) and state 0 |
| 3 | when what plays has ended: its stand, at once, with that wait, and state 0 |

Each of the animations it plays comes at once (0xFDED8 with no frames to come
over in). A wait is the least plus the field's random number modulo the
difference (0xF60C4). Setting flag 0x47 (0xF6004, for each character not acting)
lets a stand that was told its round was the last go round again, and has one in
any other state stand, with a new wait. Of the field's own bundles fifteen have
animation 16 (the people: `FCHR_S_ALB00`, `ALX00`, `BOB00`, `CRS00`, `DAR00`,
`FOB00`, `GRS00`, `HAS00`, `LAR00`, `LOS00` and their like) and none has 17 or 18,
so what is seen is state 0 to 3 and back: someone who has stood four to eight
seconds looks about once. In the port: `scene::Idle` (stepped by
`Character::idle_on` after every character's frame, with numbers of its own
for the waits), the animation chosen in `animate`, and `s96(28, ..)` in the
map script's host. It is left to idle only while no script's animation plays on
it, which is narrower than the original's test of its status alone. Seen unseen
on `STG01_04` with the scenario at 7, where character 600 stands by the wall:
`--map STG01_04 --variables 10004:7 --skip-messages --start -2300,-1600` has the
log say "character 600 has stood a while and idles: animation 16" twice in half
a minute.

In the port: the script runs with the field's tables mapped into its address space,
so it reads `prm` and the missions' records itself. Stand-ins and gaps: nothing
the save's
backup and the skills' functions do nothing (what the backup is for is under "A
mission failed"). Who is guarded and the enemies about it are
`value::GUARDED` and `value::ENEMIES_NEAR` in `field/system.rs`. When value 56 is set (the field's flag 0x58: a mission's
time has run out, or what she guards is lost) the field's frame starts the map
script's callback for a character with the number in value 57, and clears the
flag (0x10D6BC to 0x10D6E8: 0x117E14 with 0x39, then 0xE84A0); so "how the
field is left" is a number the map's own script answers to, as it is for value
52 (`s96(52, 600)` on `STG02_09`). In the port: `notice_touches` in
`field/mod.rs`, before whatever she has walked into. A script of "kind 2"
is the loaded event's, of which the system script only asks a function that
none has (see function 0x33 above).

## The mission's display and the field's windows

The display is two panels, each a line of the mission's that says what is to be done
and, by the panel's kind, a number or two. Everything they show is kept in the save,
and a panel is drawn from there (0xEF084):

| | first panel | second panel |
|---|---|---|
| shown | flag 6165 | flag 6166 |
| which of the mission's lines (record + 0x30 on); 255 none | variable 233 | variable 234 |
| how its numbers are shown | variable 235 | variable 236 |
| the two numbers | variables 10007, 10008 | variables 10009, 10010 |

A script sets these with `s105(window, what, value)`, the system script with its
function 0x73 (both 0xE6650): window 10 is the first panel and 11 the second; what 1
is the kind of numbers, 2 the line (a negative one is none), 3 and 4 the two
numbers. The kinds, as scripts give them (0xEEF90 turns them into what 0xEF198 draws
by): 1 both numbers (the first mission's "0 / 6"), 2 a gauge of the first against
the second, 3 the first number alone, 4 both in another way that has not been read.
Windows 15 and 16 with what 0 set field flags 0x5A and 0x5B instead.

The window command (0xEDB24) is `s104(window, how)` for windows from 5 on (the ones
below 5 are parts of the screen that are not ported) and the system script's 0x46:

| window | how | what |
|--------|-----|------|
| 5, 10 | 0 or 2 / 1 or 3 | show / hide the first panel |
| 11 | 0 or 2 / 1 or 3 | show / hide the second panel |
| 9, 12, 13 | 4 | the windows of a mission's start (with its title), clearing and failing |
| 16 | 4 | the name of the place (the map's record) |
| 6, 7, 8, 14, 15 | | nothing |

**How the original draws it** (ported: the panels in `field/hud/display.rs`, the
notice as `scene::Notice` and `field/hud/notice.rs`, "BEGIN MISSION" and the
title, and her HP gauge in `field/hud/gauge.rs`). The display is an object of
its own (made at 0x48624, drawn at 0x48834, nothing of it moves): the layout
`if_fldsys.lpk` files 8 to 11, with a player for each kind of panel (sets 0 text
alone, 2 a clock, 5 a gauge, 7 two numbers, 11 two figures). The second panel is 13
higher than the first. A panel's line is a text object of 48 glyphs whose glyphs
are drawn 10 by 10 (0x26E18), put at its group's object 5 higher than the object
is, and everything after it is moved right by the line's width. The kinds, as
0xEEF90 makes them of what a script gives: script 1 is two numbers, the first in
big figures right to left (as many as the second has, 12 apart), a stroke, and the
second in small ones left to right (7 apart); script 2 a gauge, the bar cut to
its share by the scissor; script 3 a clock of the first number in thousandths of a
second, as minutes, seconds and hundredths of two figures each; script 4 two
numbers of two figures at fixed places, with no line. A figure is the object of
its group drawn with the texture moved (0x15AE38) to the figure's cell: cells one
more than the object's size, five to a row for the big ones and nine for the
small. Nothing of the panels is drawn while the save's flag 6167 is set (0x2D66C).
The object also owns the notice a mission starts with (window 9; made at 0x44618
from files 23 to 25 and texture 18 of the same pack): a band across the middle of
the screen with the mission's title, which comes in over 10 frames (set 0), stays
while jingle 3 of `jin_pack.bin` plays on the lines' stream (three seconds with no
jingle for mission 44), and goes over 6 (set 1); the script's wait for window 9
lasts as long as all that (0xD5CA0).

**Her HP gauge.** The game makes it once, as it starts (0x42630, from 0x2C564): an
object of a module that keeps lists of small things to show (0x3C988 to 0x3DC40:
an entry is 0x50 bytes with its kind at + 0xC, the set it plays at + 0x10, flags at
+ 0x14, a number at + 0x34 and a share at + 0x3C; each entry has a player of the
object's layout, stepped by 0x3CFD4 and drawn by 0x3D114 by its kind), kept at
0x2EAA8, with one entry of kind 8, which is the gauge and is never taken away.
Its layout is files 13 to 15 of `if_rsdsys.lpk` with nine textures, of which it is
given the first (file 20) and the last three (files 16, 17 and 9, the figures).
The field's frame gives it her HP as a number and as a share of the most she can
have (0xED950: 0x44D00, 0x44CE0) and steps it (0x44C1C), and the field's draw
shows it (0xEDAAC to 0x44C70) unless field flag 0x51 is set. (The same frame works
out a second share from her experience and gives it to a second object, 0x2EAEC,
whose functions are all empty: a gauge the game does not have.)

It plays set 0 (0x40774 draws it): group 0 the frame and the dark ground, 2 the
bar, 1 a glow over the bar, 5 a pattern with a mark at the middle, 3 "HP", 4 one
figure. Groups 1, 3 and 4 have two tracks each, one after the other: the first
is drawn while she is well, the second, which goes round every 24 frames with the
glow coming and going, while her share is at or under the battle system's value
80 in hundredths (25: "hp ui lv1 rate"). The bar is cut by the scissor to her
share of its 152 pixels, one at the least while she has any HP, and drawn by a
colour of the layout: red (220, 80, 70) in danger, amber (255, 190, 80) at or
under value 79 (50), blue (60, 100, 230) otherwise. The number is the figure
drawn once for each of its figures, 11 apart from the right, from the cells of
`009.ptm`. Where it stands goes by the object's + 0x70: the field sets 0 each
frame, which is a pixel left of the layout and ten lower (the bar from (319, 243)
to (471, 261)); 2 is ten higher (0x55FC8, 0x605D4) and 1 the layout's own place
(0x36154), which is where a battle has it, over the gauge of her heat (an entry
of kind 9, set 7, 0x3DBE8). When she is hit in a battle the gauge plays set 1,
a flash of 16 frames (0x3DB94, from 0x357E8), unless the object's + 0x7C is set.
Set 1 is played over what the gauge has (0x319BC): it has one track for the
glow, one for "HP" and one for the number, which take over the first of each
of those groups' two tracks, the ones drawn while she is well, so in danger
nothing of it is seen: the glow comes to 150 at the second frame and is gone
at the sixteenth, "HP" and the number go amber (255, 229, 124) and all there
and back. 0x357E8 puts up what a hit takes off, an enemy's as well as hers,
and asks for the flash where the number is over nought and the numbers'
object is not marked (its + 0x7C, 0x3D814): each actor has its own (hers at
her + 0x808, an enemy's at its + 0xA28), and an enemy's is marked as it is
made (0x91E10 with 1) where hers is not (0x99B7C with nought). So only what
is taken off her flashes it. In the port: `gauge::FLASH`, played from
`Numbers::step`'s answer in `battle/hud.rs`. Looked for in the walkthrough on
2026-10-09: in the first battle (660 to 665 s, every other frame) the number
never goes amber while she shoots the Eaters, which is what the mark says; a
hit on her was not found in that stretch, so the flash itself is still to be
seen there.

**When the field draws all this** (0x10F0F8). The field is free (0x10E554) unless
one of its flags 0, 0x12 (a battle), 0x1B, 0x1E, 0x2C, 0x34, 0x39, 0x3D, 0x42 is
set, or it is in state 3 without flag 0x17, or flag 0x56 (the mission failed) is
set without 0x17. While it is free it draws the marks over characters (if flag
0x27 is set and 0x28 is not: 0x107748 for each, and 0xD3DB8), then the mission's
display (0xEF39C), a sign made of sets 6 to 14 of `if_rsdsys.lpk/018.lpk` (0xEDF20
to 0x466F8, made at 0x464FC, shown by bits of field + 0x1D64), the place's name
(0xEEBF4 to 0x46EBC; see "The sign that names a place"), something of field + 0x21FC (0xEE560 to
0x2C8BC) and the gauge (0xEDAE4). While it is not free it draws the same in
state 3 (a map being left), the gauge alone in state 6 unless 0xF1B28 answers,
the window of a mission failed while flag 0x56 is set (0xEE73C), and nothing
otherwise. Both the display and the gauge are left out while the save's flag 6167
is set (0x2D66C), which is how scripts have the screen clear: a map's
`S1M1_ST_COMM` sets it before the opening calls and `INIT_EVENT` clears it only
after the notice (`s13(6167); s247(1, 400, 0); s12(6167)`), and the functions of a
mission's end set it first.

`s104(part, how)` for parts below 5 (0xC8AB0; how 0 and 2 open, 1 and 3 close):
part 0 does nothing, 1 sets field flag 0x25, 2 flag 0x27 (the marks over
characters), 3 clears flag 0x51 (her gauge) and 4 flag 0x52, which nothing has
been found to read. A map's `EVENT_START1` closes 3, 1 and window 5 (the first
panel), and `EVENT_END2` opens them again.

In the port: `system::panels`, `set_display` and `open_window`; the panels, the
notice and the gauge are drawn from their layouts (`field/hud/`), the display and
the gauge while flag 6167 is clear, the gauge unless flag 0x51 is set. The gauge's
motion is stepped by the frames drawn. The windows of a mission cleared and of
a mission failed are the original's (see "A mission cleared" and "A mission
failed"), and so is the place's name (see "The sign that names a place"). The
display, the place's name and the gauge are left out under either window and
while the field is being left. The sign of sets 6 to 14 is the sign of what a
button would do, and the marks over characters are in "The marks over
characters".

**What the field is not free in**, flag by flag, and what the port has for each.
While it is not free the field draws nothing of its world either (0x10E750: only
the picture the screen's fade keeps, and that only under the MENU), so each of
these is a screen that stands over a picture of the field:

| Flag | Set by | What it is | In the port |
| --- | --- | --- | --- |
| 0 | 0xEFA60, with the field's mode put at 2 | the field has been told to end | `Scene::leaving`: the display is left out |
| 0x12 | the encounter | a battle | another mode of the game |
| 0x1B | 0xF19B8 (and cleared in 0xF1BC4's state 3) | the MENU is up | `menu/`: nothing of the field has a frame or is drawn |
| 0x1E | nothing: no routine sets it by number | | |
| 0x2C | 0xE68AC | a film | `film.rs`, over everything |
| 0x34 (with 0xA) | 0xE6964, from the pause (0x10CEE8) and the map of the stage (0xF3400) | the field stands still under one of the two | `Paused`, `MapScreen`: the field has no frame |
| 0x39 | 0x1112E4 | the list of saves is up (0x1113CC draws it) | `save_screen.rs` |
| 0x3D | 0x10CEE8 | the pause is up (0x10CC3C draws it) | `pause.rs` |
| 0x42 | 0x113898 | a window of the field's own at + 0x223C (0x1137DC draws it), of the title's kind or of the scripts' | nothing: its bit 1, which has it started (0x1139DC), is set nowhere |
| (0xF347C) | the field's + 0x2230 | the map of the stage is up (0xF326C draws it) | `field/map_screen.rs` |
| 0x56 without 0x17 | 0xEE760 | the window of a mission failed, once its picture is taken | `field/failed.rs` |
| mode 3 without 0x17 | 0x115E88 | a map is being left, once its picture is taken | `Scene::map_change` |

The picture each of them stands over is taken where the field's flag 0x17 is
found set, and the field's + 0x1E34 says where that is: with nought as the
field's own display is about to be drawn (0x10F358: the world and the marks over
characters, nothing else), with 1 in the last of the field's tasks (0x10EA18:
all that is on the screen). The MENU (0xF18F8), a map being left (0x115E88) and
a script's picture of the scene alone (0xE4790: `s145`, and `s172` and `s223`
with 0) have nought; the pause (0x10CE64), the map of the stage (0xF3388), the
list of saves (0x11148C), the window of a mission failed (0xEE760), a battle
beginning (0xD876C) and a script's picture of all the screen (0xE47C0: `s172`
and `s223` with 1) have 1. So under the pause and the like the gauge and the mission's display stand
as they were, which is what a scene that stands still with them drawn comes to
(the port's way), and while a map is left they go on being drawn over the
picture of the street (the field's mode 3: the picture, then the display, the
sign, the place's name and the gauge, 0x10F134), which the port does too:
`interface_shown` does not ask whether a map is being left. Nothing is left to
port of this.

## The windows a script puts up

**The window with a text** is `s252(text, choice, choice)` (0xC6618): the game's one
system window (the object at `[0x2268()] + 4`) is given the text and up to two
choices (0x6BAFC; a zero is no choice), opened with a sound (0x6BE38), and the script
waits until it has closed (0xC65F0, 0x6BDE0). `s253` reads the choice the cursor was
on; no map calls it. What the window does each frame (0x6BEE8, pad bits as the PSP
has them):

- nothing until the text's track of the set that opens it has been played through
  (0x6B888 asks 0x31C68 of track 6: frame 22 of the layout's frames, which are
  sixtieths of a second, two to a frame of the game);
- while the text has more pages, cross (0x4000) turns to the next;
- with no choices, cross closes it (0x6CE54);
- with choices, which come in with the last page: a sound (0x11) when the cursor's
  bar has come (frame 24), and once the set that brings them is played through
  (frame 42) up (0x10) and down (0x40) move the cursor, round the ends, with a
  sound (2); circle (0x2000) puts the cursor on the choice kept at window + 0x48
  (it does not make it), with the same sound; cross makes the choice;
- it is opened with sound 0 and closed with a sound that goes by the choice made
  (0x6CE88, by the window's + 0x78: 3 for the first or for none to make, 4 for
  the second, none for any other), and closing plays the set that closes it (20
  frames): the script has its answer only when that is over.

**The window that asks a question** is `s95(text, nine choices, start, cancel)`
(0xC1EC4): a "SelectWin" task (0xCF618) round the same kind of window, with the
choices a script does not use given as a single space, the choice the cursor starts
on and the one circle goes to. `s75()` waits for the answer (0xC2004) and `s76()`
reads it (the script context + 0x44C). The terminal asks "Save Game Data?" with it
(start 0, "Yes"; cancel 1, "No").

**Pictures over the whole screen**, five slots of them (the script context + 0x634;
a "Pict Task" each, 0xD0528): `s248(slot, name)` loads `IF/PICT/<name>.PTM` unless
the slot has one, `s249(slot)` waits for it, `s251(slot)` lets it go, and
`s250(slot, way, kind, frames)` (0xC22E4, 0xD0778) brings it in (way 0) or takes it
out (way 1) over so many frames. The frames are the interface's, sixtieths of a
second (the picture's object, 0x6B208, adds 0x850 each frame of the game): a
lesson's `s250(.., 2, 30)` is half a second. Kind 2 fades the picture itself (its
alpha is the frames gone by over the frames given, 0x6B484, and it is drawn white
by that alpha, 0x2DE94); kinds 0 and 1 leave it whole and have the screen's fade
come in from black or from white as it comes, and go out to them as it goes
(0x297E4 with 1, 0x11, 2 and 0x12), over the same time. `s258()` waits for a press
of cross (0xC235C). A lesson is a picture or two faded in, its windows one after
the other, and the pictures faded out again; the pictures are screenshots of the
original with the thing taught marked. The story's opening (`d_stg05_0a1`) is
pictures of kind 0 with captions over them.

**The screen's fade** is one task for the whole game ("Trans", 0x2A30C, made at
0x28890), told what to do with 0x297E4(kind, frames), the frames being the
interface's (it divides them by 0x850 and counts frames of the game; none is one).
Its data: a state (0 idle, 1 going, 2 there), a level of 0 to 255, the kind, the
step a frame, a colour; from + 0x54 a figure for each of the screen's 272 rows;
at + 0x1484 the picture it keeps (below). The kinds (0x2B6EC starts them; each
starts from all or from nothing whatever was there, and 0x2A51C moves the level
by 255 over the frames each frame):

| kind | what |
|---|---|
| 1, 2 | a black sheet comes off, goes over |
| 0x11, 0x12 | the same in white |
| 4, 8 | a black sheet comes off from half (128), goes over as far as half |
| 0x100, 0x200 | the sheet comes off, goes over, in the screen's rows, each at a pace of its own (a random 0 to 31 and twice 480 over the frames): the title leaves by it, and the port has it there (`title/rows.rs`; see `TITLE_NOTES.md`), not as a kind of the field's fade |
| 0x400 | only kept as the kind |
| 0x800, with 0x1000 to 0x5000 | the picture kept is drawn moved about (its middle, scales and angle from + 0x30 on); not read further, and never asked for: of every start of a fade in the executable (0x297E4) the kinds are 1, 2, 4, 0x11, 0x12, 0x803 and 0x804 |
| 0x803 | the picture kept goes: there is less and less of it (0x2C244) |
| 0x804 | the picture kept stays, and darkens unless it was put up to be shown whole (0x2C258) |

Nothing is started for 0x801 and 0x802. Its draw (0x2ADB8) draws nothing for
0x803 and 0x804, nor while it is idle. Whether the screen counts as covered goes by
the kind alone (0x29974: 2, 8, 0x12, 0x200, 0x800, 0x802 and 0x804 do), as does
whether it is over (0x29948: for the picture's kinds, when the picture is not on
its way; otherwise when it is idle or there). 0x2BF5C puts it at once at the end
of the kind it has, covered or clear, with the kind turned into its other half (1
into 2, 0x803 into 0x804 and so on).

**The picture the fade keeps** (0x2C100 to 0x2C548; 0x29A1C gives it). The game
copies its screen to a buffer of 512 by 512 (0x29864, from wherever the frame has
got to) and this object draws it over the whole screen (0x2C2F8), by a colour:
its alpha is what is left of the frames its going takes (+ 8, + 0x10), its red,
green and blue what is left of the frames its darkening takes (+ 0xC, + 0x14), or
all of them while its flag 1 is set (0x2C298 with 0 sets it: the picture is shown
as it was taken). Its states: 0 off, 1 going (off at the end), 2 standing
(0x2C2C0), 3 darkening (standing at the end). The fade's task steps it while the
kind is 0x803 or 0x804 (0x2C13C, a frame of the game at a time), and who draws it
is whoever is on the screen: the field as the first thing its own interface draws
(0x10F324, so under the mission's display, her gauge, the windows), a battle's
main object (0x73DD0), the encounter (below). The field takes the picture when
its flag 0x17 is set, and clears the flag: where its interface is about to be
drawn (0x10F358: the scene alone) or, with field + 0x1E34 set, after all of its
interface (0x10EA18).

**What scripts do with them.** `s7(way, colour, frames)` (0xC6FE0; nothing in a
script that is being run through fast): way 0 brings the screen back and is done
only if it counts as covered, way 1 takes it away and is done only if it does not.
Colour 0 is the black sheet (kinds 1 and 2), 1 the white (0x11, 0x12), 2 the
picture (0x803, 0x804); −1 is the other half of the kind last started (0x804 has
0x803, 0x12 has 0x11, 2 has 1, and the other way about). Its frames are the
game's. Then:

| function | what |
|---|---|
| `s145()` | a picture of the scene is taken, and the script waits a frame (0xC5270, 0xE4790) |
| `s146(frames)` | the picture, shown whole, goes over so many frames of the interface (0xC5314, 0xE47F4: kind 0x803) |
| `s172(n)` | a picture is taken, of the scene (0) or of all the field has on the screen (1) (0xC52A0); no script calls it |
| `s223(n)` | the same, and a frame later the picture is put up whole to stay (0xC5370, 0xE4838: kind 0x804), which leaves the screen covered |

`s145(); ...; s146(20)` with a change of shot between them is how one shot of a
scene gives way to the next: every map's scenes do it, some 2000 times over. A
scene ends with `s223(0)`, its last shot standing while the next scene is set up
or the field comes back, and what follows has it go with `s7(0, -1, n)`.

In the port: `scene::Fade` (the sheet, the kind, `Kept` for the picture's state;
`begin`, `script`, `hold_picture`, `dissolve`), `scene::Capture` and
`take_pictures` in `field/mod.rs` for the taking (the engine's `canvas::Captured`:
the graphics card hands a picture over a frame or two after it is asked for, and
the script waits for that rather than for one frame), `draw_fade` for the drawing,
on a layer of its own under the field's interface. A picture of the scene alone
is one of what the scene's cameras drew; one of everything is one of the window.
Seen in unseen runs against the walkthrough (`docs/compare/battle_to_field_cmp.png`
and `encounter.png`).

**The scene alone can be faded**, with everything of the interface left as it is:
`s197(way, colour, frames)` (0xC21C0) makes a task of its own for it ("Fade
Priority", 0xD01CC) and starts it, `s198()` waits for it (0xC2238) and `s199()`
does away with it (0xC2254). It is a sheet over the screen, black (colour 0) or
white (1), whose alpha goes evenly from none to all (way 1) or from all to none
(way 0) over the frames, which are the game's; what waits for it goes on a frame
after it has had them all. Scripts use it some 940 times: before a window that
tells what was done ("Handed the letter over."), and where a mission ends.

**What is drawn over what.** Every task has a number (0xAE3C; `tools_re/
task_makers.py` lists them all), and each frame the tasks have their turn in
the order of their numbers: first each its frame (0xADE0), then each its drawing
(0xAE08; the field's tasks for the scene draw straight after their frame,
0xB8FC). For the field:

| number | task | what |
|--------|------|------|
| 0x100 | fldTask3dPre, fldTask3dObj, fldTask3dPost | the field's frame (the map's script with it) and the scene |
| 0x200, 0x240, 0x241 | Bloom, Focus, Glare, Blur | what is done to the scene's picture |
| 0x229 | Fade Priority | the fade of the scene alone |
| 0x300 | fldTask2d | 0x10F0F8: the marks over characters, the mission's display, her gauge |
| 0x300 | Information | a script's lines at the top of the screen (0xCF3C0) |
| 0x33F | ifRecInfoTask | a record or a challenge reached |
| 0x340 | Pict Task | a script's pictures |
| 0x341 | SystemWin, SelectWin | a script's windows |
| 0x37D | ifComWindowTask | the window people speak through |
| 0x37E | Movie | |
| 0x3FF | fldTask2dPost | 0x10E93C |
| 0x400 | ifFldMiniMap, ifFldFailedTask | the map, the window of a mission failed |
| 0x402 to 0x404 | the menus, ifLoadingTask | |
| 0x600 | Trans, enc_eff | the screen's fade, and what the screen does as a battle starts |
| 0x601 | Location | the title of a scene's place |
| 0x602 | telop_auto_task | a scene's captions |
| 0x610 | Pause | |

So a lesson's picture covers the mission's display and her gauge, its window is
over the picture, and the screen's fade covers all of them but the title of a
scene's place and the captions. A battle's own tasks: ifBtlCutinTask and ifBtlFinishTask 0x300,
ifBtlMenuTask and ResultBtlTask 0x301.

A picture of the screen is a texture of 512 by 256 (0x2DE94 draws it): its left 480
columns are the screen's first 256 rows, and the screen's last 16 rows lie on their
side in the 32 columns left over, as two strips of 16 by 240, the first column of a
strip being the screen's last row. `tools_re/pict_screen.py` puts one together.

**Texts** are UTF-8 with codes in them, laid out glyph by glyph at 0x2837C (jump
table 0x1661B8). A code's number is two digits in sixteens (0x25CF4), or four for
those marked long (0x27438); in place of either, `L` or `G` and eight digits name a
variable of the script that is showing the text: a local of the function that
called (its frame + 0x10 + 4n) or a global (context + 0x104 + 4n).

| code | what |
|------|------|
| 0x0A | new line |
| 0x0C | the page ends here: cross goes on to the rest |
| 0x13 nn | the colour: 0 the one text starts in |
| 0x14 nnnn (long) | the name of item n, in colour 1; from 500 on a name of another table (0x12CC4C), not ported: no text of the data has the code (they have only `\b`, `\c`, `\z` and `\f` written in them; `tools_re/out/_text_codes.py`), a script puts it in with a number of its own, and no box of any map gives a thing numbered so; the port warns if one is ever asked |
| 0x15 nn | a button's icon |
| 0x16 nnnn (long) | read and dropped |
| 0x17 nn | a dash n pieces long (U+2015 each) |
| 0x19 nn | a text the game no longer has (0x12CF74 answers none) |
| 0x1A nn | one of the work area's texts (0x12FBE8: 97 bytes each); only the records' script gives such words, and the port has those by another way (`records.rs`: a reward's own words) |
| 0x1D nnnn (long) | the number, in tens |

The same in writing: a backslash, a letter and a number in brackets (table
0x166208): `\b(n)` an icon, `\c(n)` a colour, `\f` the page's end, `\z(n)` a dash.
A mission's title uses it ("To Mama\z(2)").

The colours are the sixteen of `SYSTEM.VOL/font.lpk/003.xtc` (0x12CF7C): 0 white, 1
light blue (115, 205, 255), 2 grey (150), 3 light red (255, 100, 100), the rest
white. Icons as the texts use them: 0 circle (dodge), 1 cross (guard, and the button
that looks at things), 2 triangle (the menu), 3 square (shoot, and the map); 0x29
and 0x2A are what uses an item and what goes with a button for a skill (L and R),
0x2C what ends a mission with square (start), 0x2F and 0x30 what changes lanes
(left and right): those five are read off the texts, not off the icons.

Two more that lessons and objects use: `s134` makes her whole whatever it is given
(0xC51C4, 0xD88), and `s109(who, item, count)` gives her so many of an item, or
takes them, when who is 0 (0xC4DE4, 0x14D594).

**How a text is drawn.** A text object (0x280E8) lays its text out glyph by glyph
into lines (0x277A4): no wrapping by width, only the text's own new lines and a new
line when one has as many glyphs as it was made to hold. ASCII letters and figures
are drawn as their full-width forms (the code plus 0xFEE0) and twenty signs as
theirs (a space as U+3000, the tables at 0x170F98 and 0x170FAC); any other character
below 0x81 is of a font that is never loaded and draws nothing. A glyph is the
font's whole cell drawn at the style's size; the pen then moves on by the
character's metric (scaled) or by the cell, plus the gap between glyphs. A character
the font lacks is drawn as an at sign. An icon is drawn as it is in the middle of the
line's height (0x26640); a dash is the left quarter of U+2015, half a glyph's width
of its middle column for each piece after the first, and its second quarter
(0x26FC8). Lines are 15 apart plus the gap between lines. The styles (0x2D788): the
text's font at 15, or the capitals at 5, 10 or 15, every one with widths by the
metrics; each user gives the glyphs to a line, the lines, and the two gaps. A text
can come up glyph by glyph (0x26E44: so many frames each, line after line). It is
drawn with the PSP's light on, the light's colour being the text's own tint (which
is how one fades), and with a shadow if asked (0x2D9B4): the same glyphs one pixel
right and down, in (0x22, 0x18, 0x15).

**How the window is drawn** (0x6C0F8), from the layout `if_rsdsys.lpk/000.lpk`
(`docs/FORMAT_NOTES.md` has what a layout is). Its sets: 0 opens a window that
tells and 1 closes it, 2 and 3 the same for one that asks, 4 brings the choices
and 5 takes them (6 to 9 are variants and the sign for more). The layout has the
window two lines tall; a window's lines are the new lines of all its text and one
more, or its choices if those are more. For each line over two it is 18 taller, and
is drawn in parts, each through a matrix of its own: the middle of the band across
the screen stretched (1 + 18n/36 about 5 down in its own corners), the band's top
with the frame's top and the label moved up 9n, the band's bottom and the frame's
bottom moved down 9n, the frame's sides stretched by 18n over their height in
whole 18s, and the bar down the left stretched by 18n over its height and moved up.
The text is written with its shadow in the object that marks its place (object
14: at (83, 130), 319 wide; object 25 for a window that asks), its lines against
the right of that width, 9n higher, in the colour the text's track has come to:
which is how it comes up after the window and goes before it. The choices are
written from object 32's place (359, 131), 18 apart: the one the cursor is on has
the cursor's bar under it and the colour of one track, the others the mark and the
colour of another; one the cursor has just left dims from 230 to 100 over ten
frames (clip 16).

In the port: `text.rs` cuts a text into pages and runs of one colour, `writing.rs`
lays it out and draws it with the game's fonts, `field/window.rs` is what the
window and a picture do (its timings are the layout's, which a test holds them
to), and `field/hud/window.rs` draws the window from the layout. Everything is
drawn on the canvas in layers that are the tasks' own numbers (`interface::layer`):
the fade of the scene alone (`scene::SceneFade`), the display and the gauge, the
pictures, the windows, the screen's fade (`scene::Fade`, which a picture of kind 0
or 1 restarts) and the captions. Of what was once left here: the variant that
opens with sound 7 (0x6BE90: a window made by 0xCFB9C whose + 0x20 is nought,
and the menu's 0x5463C) and sound 5 for choice 1 of a window with flags 0x80
and 0x100 are of no window a map's script makes (`field/window.rs` says where
they are); the fade's half kinds (4 and 8) are started only by the tasks that
save, load and delete (0x5330, 0x5804, 0x5F28: the game's picture goes to
half behind the PSP's own dialog, where the port has its list's own sheet) and
by 0x6EF70, and its rows (0x100, 0x200) only by the title, which has them. Still
to do: the port's windows have their frame before the script's, where the
original's tasks have theirs after it (a frame's difference at the most). The fade's kind 0x800 with a
move is never started (see the table of kinds).
`--skip-messages`, and holding start, take every window as
read and a question as answered with what its cursor starts on.

## Events, and the messages people send her

`s237(number, argument)` has an event played (0xC63B4; nothing in a script that is
being run through fast): one of the 710 names of `lst_fevt`, each a bundle under
`FLD/FEVT` (690 are there; `tools_re/fevt.py` lists them). The request joins a queue
(0xD7774: a pool of 32 requests, eight queues of which map scripts use the second),
and the script goes on; `s238()` waits until the queue is empty (0xC644C,
0xD7958). The queue's runner (0xD7C78 on) loads the event's bundle, finds
`script_entry`, `async_event` or `comm_event` in its script, in that order, starts
it with the argument and runs it a frame at a time until it returns (0xD7EC0).

An event's bundle: slot 8 its script (an `SSCR` like the system script, calling the
same table of engine functions, 0x1726D0), slot 9 its texts (`TD`, a count of 16
bits, where each text starts, the texts), slot 5 its recordings (`RIFF` files of
ATRAC3plus), slot 7 the faces of those who speak (22 packs for the first stage's
events, each a layout, the name as a picture and the face, 128 by 80 with the face
in its left 72 columns), slot 1 animations, slot 0 a small `SSCR` of data.
`tools_re/fevt_survey.py` tells what the 465 scripts call: 374 say something, and
some 200 do nothing else.

What they say goes through the window people speak to her through (the
"ifComWindowTask", 0x38B04, behind a manager at 0x12E4F0), with the table's first
functions:

| n | what |
|---|------|
| 0x00 | waits so many frames (0xD4030) |
| 0x03, 0x04 | open the window; close it, the faces going with it |
| 0x05 | `(place, _, face)`: one of the window's two places shows a face of the bundle (the second number is not looked at, 0x12ED3C) |
| 0x06 | `(place)`: the place shows none |
| 0x01 | `(place, text, recording, frames)`: a line is queued: which place speaks, which text, which recording (below zero: none) and how long it stays (below zero: as long as its recording) |
| 0x02 | waits until nothing is left to be said (0x12E9D0) |
| 0x5C | `(text, text)`: the next line said asks, with two of the event's texts for answers (0x12EBF8) |
| 0x5D | waits for the answer and returns it (0x130088): 0 the first, 1 the second |

A message is `n03(); n05(0, 1, face); n01(0, 0, 0, -1); n02(); n06(0); n04();`.

**The manager** (0x12E288 is its frame) keeps the lines in a queue and gives the
window one at a time (0x12E600), and none while a place's face is coming or going
or the window is on its way out (0x12F074): that, or a line still in the queue, is
what `n02` waits for. A place that is given another face sends the one it has out
first and brings the new one in when that has gone (0x12ED3C, 0x12EE6C). A window
told to open while it is going out opens when it has gone (0x12EA28, 0x12EB78).
Each frame the line at the head has its turn (0x12E6D0), and when it is over
(0x12E894: the window has nothing left to say) the next is begun and has its turn
at once.

**A line** (the window's frame, 0x399C0): it is over when its frames are up if it has
a time, counted in the layout's frames, two to a frame of the game; otherwise when
its recording, once heard to have started, has stopped (0xA3DC); with neither it is
over at once. Its text is all there from the first: the text object is told to
come up glyph by glyph, and then, as the line is given to the window, to have all
of it up (0x38EDC), so cross (0x39164) never has anything to bring up and always
stops the recording and ends the line, with sound 3. A recording's length is the
samples of its `fact` chunk at the rate of its `fmt ` chunk.

**A line that asks** (0x12E714) is held (0x39284): when its voice is over it stays.
Its answers come up when it has been said, or at once on cross (0x392B0, which
cuts the voice short), and from the next frame L gives the first and R the second
(bits 0x400 and 0x800 of the game's pad, which is the PSP's with the two shoulder
buttons moved there, 0x10BE4): the script has its answer there and then, sound 3 is
heard, the window shows the answer chosen for ten frames (0x39418), and then the
answers go (0x393CC, ten frames) and the line with them. The mission's first call
asks "Kid, can you hear me!?", and goes on differently for "Yes." and "No.".

**The window** is the layout `if_fldsys.lpk/000.lpk`, with eight textures: files 6
to 9 of the pack that is always there, then its own four (0x39790). Its task keeps
eight players of the layout: one for the body (set 8 opens it and 9 closes it, 13
brings the sign that a line can be passed and 14 takes it), one for the answers
(10 brings them, 12 shows the one chosen, 11 takes them), and for each place one
for the face's frame (0 and 1 in and out for the first place, 4 and 5 for the
second), one for what goes over the face (2, 6) and one for the three marks by the
name that move while the place speaks (3, 7). It is along the top of the screen:
a face at each end, 72 by 78 at (10, 10) and (398, 10), and the body between them
from 90 to 390. What the layout does not have is put where objects of it mark
(0x39F2C):

- what is said: the text object is 36 glyphs to a line, three lines, drawn with
  its shadow across the middle of the screen, as high as object 21 is (32 down)
  for a single line and as object 22 (26) for more, in that object's colour by the
  colour of the track of group 0x14;
- the speaker's name: objects 1 and 2 of a file of the face's own (`000.sdoa`: the
  name's shadow and the name, a texture of their own), in the colour of the track
  of group 0x22, for the place that is speaking only;
- a face: its picture in the rectangle of object 40 or 55, in the colour of the
  track of group 0x26 or 0x32; over it the lines and the noise of the place's
  second player, cut to that rectangle (0x15B098); and while the other place
  speaks a dark sheet over it (27 in 255 of that colour, alpha 64 in 255);
- the answers: each written in the middle of an object of its group (9 and 10;
  11 and 12 for the one chosen, which is blue), with the sign of its button (61
  and 63; 62 and 64).

It opens with sound 0x31 on its first frame; closing makes no sound unless the
window never had a frame (0x39D48). The sign comes eight frames after a line is
up, and is taken and brought again as a line ends; it shrinks away when answers
come up and when the window closes. An event that is skipped, and a battle, drop
all of it at once (0x12E368).

In the port: `field/event.rs` loads an event, `field/system.rs` runs its script
(`update_events`) with the functions above, `field/comm.rs` is the manager and the
window, counting the frames each of the window's motions has played, and
`field/hud/comm.rs` draws it from the layout. The manager's frame is taken to come
before the window's (the manager is part of the field's own frame, the window a
task of the interface). With no one listening a line lasts the frames its
recording is long. Functions an event's script calls that are not ported are told
of once and answer zero. `--skip-messages` has every line over at once, a question
answered with its first answer, and nothing of the window take its time.

**What an event does with people.** An event's library has the map's functions
under other names, and they come down to the same code. A character is chosen with
0x29 and then:

- 0x49 sets a setting of it (0xD6ED4), by `s52`'s numbers but only some of them: 0
  hidden, 10 the status it walks in, 25 its pace, 27 whether an animation a script
  names loops, 33 the frames it takes to go over to another animation (0x1084E8:
  eight until a script says; the first call of the first mission has her go over in
  twenty), 47, 48 to 51 and 60 to 69 its neck (see "Necks"), 115 to 119 and 125 int
  parameters, and an animation of the script's choosing: 120 one of its own
  (0x1071A0, the map's `s62`), 121 the one the event keeps for its kind (0xE94EC,
  `s63`), 127 a slot of the event's cast and 128 one of that slot (0xE9578, `s265`).
- 0x2A has it do something (0xD4D74, by the table at 0x16CC18): 0 walk to where
  it was told, 2 turn, 7 stop, 8 stand down, 11 dash, 13 start the animation
  named (0xF4EEC, `s70`); 9, 10 and 14 are a map's ways of coming to where it
  stands (0xF6D20 up out of the ground, 0xF7458 in from above, 0xF7090 up
  through a ring: `Scene::coming`), and 12 is 0x1095D4, which no event asks
  for. The events' scripts ask for 0 and 13 (165 times each, in the libraries'
  `COMM_CHR_MOVE` and `COMM_START_MOTION`) and for 14 three times: the first
  mission's three Warriors, shown one after the other as they come up.
- 0x58 (x, y, z, degrees: decimals) tells it where to go and how to face there
  (0xD6374, `s133`); 0x60 puts it somewhere, on the ground, facing a way, stopped
  (0xD6680, `s48`).
- 0x61 (frames, way) moves its neck to what the settings say (0xD6784); 0x62
  (how much, flags) has its lips move while it speaks (0xFE678, the maps' `s160`):
  the amount, at most 100, with a wait of ten frames (0xFE620: its + 0x6D4 bit 0,
  + 0x6DA, + 0x6D6), and its flags 0x6A and 0x6B from the two bits. In an event,
  once the wait is over, its update (0x100410) looks whether the first stream is
  still playing (0xA3DC): when it is not, flag 0x6A has the lips stop, and flag
  0x6B ends what it was doing (0xF4908) unless its flag 0x53 is set. The first
  mission's call has character 600 speak so (`n62(10, 3)`). Both are ported: the
  neck as `s209` is (0xD6784 makes the frames the character's time, by 0xF000
  over 0x1E, and takes ways 0 to 3, any other being 0), the lips as `s160` is.
- 0x5B waits for what it is doing to end (0xD65DC, by 0x10819C: its action, or
  its flags 0x54 and 0x55 both), the maps' `s99`.

And with the screen: 0x68 (way, colour, frames) is the screen's fade as a map's
`s7` has it (0xED1DC: way 0 brings the screen back if it counts as covered, 1
takes it away if it does not; colour 0 black, 1 white, 2 the picture of the
screen, -1 whichever was last used; the frames are the game's, made the
interface's with 0x850 for 0x297E4), and 0x69 waits for it to be over (0xECBE8,
by 0x29948; `s20`). The fifth mission's opening event uses all of these.

So `COMM_START_MOTION(own or staged, who, animation, loops)` is settings 27 and 120
(or 121, or 127 and 128) and action 13, and `COMM_CHR_MOVE` is setting 10, 0x58 and
action 0. In the first mission's first call she stands at ease (her animation 1,
looped) while the others speak.

## Story scenes

A story scene is played by the map's script with what an event's bundle brings for
it. The opening of the game is three such events one after the other (169, 170 and
171: `FEVT_1_1_0200B_1` to `_3`), 29 cuts and 143 seconds up to its battle.
`tools_re/fevt_cast.py <event>` lists who is in a scene and what it brings.

**The bundle.** `s59(event)` loads it (0xC40A4, 0xE8E5C; a script that is being run
through fast only notes which) and `s60()` waits for it; `s61()` lets it go. Slots:

| slot | what |
|------|------|
| 0 | a small `SSCR` of data: `lst_fchr`, a byte for each of 16 places of a cast: the character resource the place is for |
| 1 | the cast's motions: a bundle for each place, its slots the motions by number |
| 2 | up to four more models for things of the scene (11 events) |
| 3 | the camera's cuts by number: each a bundle laid out like a field character's, the rig (an `INSM` camera) its first model and the move its first animation (0xE73C8, 0xE7458). The words of the slots it leaves empty, and its size, are zero |
| 4 | effects (`.efc`), up to 32 |
| 5 | recordings |
| 6 | a sound bank (`PPHD` and its data) |
| 7, 8, 9 | faces, script and texts, for the events that are messages |

**Motions.** `s62(who, n)` gives a character one of its own animations and
`s63(who, n)` one of the event's: the bundle's place for the character's resource is
looked up in `lst_fchr` (0xE9398) and motion `n` taken from that place (0xE9024);
with no place for it nothing happens, and with the place but not the motion the
character is left without one. `s265(who, place, n)` names the place itself, which
is how she is given hers (place 0). `s70(who)` then starts it, and setting 27 says
whether it loops. The library's `START_MOTION_NON_PC(kind, who, n, loops)` is
`s52(who, 27, loops); s62 or s63; s70`. The characters of a scene are made at the
origin, unturned: where they are is in the motions, which move their root nodes
about the map.

**The camera.** `s50()` puts the camera as it is by (its kind, figures and all; two
deep at most, 0xDA324) and `s51()` takes it up again (0xDA40C): the library's
`EVENT_START1` and `EVENT_END1` do it around a scene, so that she is followed again
afterwards as she was before. `s101(n)` makes the camera of kind 4 and has it play
cut `n` (0xE2EC4): a model instance of the rig and a player of the move, started at
the move's first time. Each frame (0xDCA10) the move goes on by 0x800 units, two of
its frames, the first time in the frame it is started; the camera is at the rig's
camera node, turned as it is, with the angle of view its lens gives (the same code
as a battle's camera moves); and when the player has reached the move's end the
cut is over. `s100()` waits for that (0xC4BEC). The camera stays at the cut's end
until it is given another or made of another kind.

**Time.** A scene is timed by a count of frames the field keeps (+ 0x2178): the
script sets it with `s96(8, n)` (0xC72B4), it goes up by one every frame, after the
map script's frame (0xD9224), and `s103(n)` waits until it is at least `n`
(0xC4CE4). A scene's script goes `s100(); s96(8, 1); s101(cut); ... s103(109); ...`:
the numbers are frames of the game since the cut began, counted from 1.

**Captions.** `s239()` makes the task that shows what is spoken (the
"telop_auto_task", 0xD13A8; eight captions at most), `s242(text, frames, in, out)`
puts one up for `frames` in all, of which it takes `in` to come and `out` to go
(0xD1C18), `s240()` waits until none is left, and `s241()` ends the task. `s243(n)`
with `n` not zero has a caption stay until one of the face or shoulder buttons
passes it (they are only looked at while the script waits in `s240`, 0xC2D80), and
`s261()` draws them 110 lines higher. As the task is made it is the player's
option that says which way its captions go (0xD1634: the options' bit 21,
"Automatically skip through dialogue", which a new game has off, so that they
wait); `s243` is how a scene overrides that, as those cut to their voices do
(`s239(); s243(0)`). A caption's text is laid out like any other
(colours, dashes). Each frame (0xD1D58): while coming its alpha is its count over
`in`; then it is whole for `frames - in - out`; then going. The text is centred in
the screen's width, at a height the interface's layout gives for one line or for
two, which the port does not read yet (it stands 30 lines above the foot).

**Parts of models.** `s224(who, part, on)` shows or hides one of the switchable
parts of a character's model (0xC60FC): `part` is a number into the field's
`lst_mesh` (33 bytes: 50, 51, 60, 61, 70, 71, 90-93, 150, 151, 0, 1, 2, 10, 11, 12,
72, 40, ...), which gives the id the part has in the model, and the character keeps
a flag for each (flags 0x7B on). **A part the list has is hidden until its flag is
set**: when a character's model is ready every flag is applied (0xFE2EC), and a new
character's flags are all clear (0x109758). So the scripts say which hands a
character has (`CHR_HAND`: 60 and 70 on the field, 61 and 71 in scenes, set by
`SETUP_B_CHAR`), whether a soldier has his rifle (part 0, id 50), which of the
truck's doors are open. Setting 76 says whether the weapon she carries, a model of
its own, is shown (flag 0x5F clear); the port has no such model yet and shows the
cannon that is part of hers instead.

**Parts of the screen.** `s104(part, how)` with `part` below 5 opens (0, 2) or
closes (1, 3) a part of the screen by setting a flag of the field's (0xC8AB0): 0x25
and 0x27 are set while parts 1 and 2 are up, 0x51 and 0x52 while parts 3 and 4 are
down. Part 3 is the mission's display (0xEDAAC draws it unless 0x51 is set); the
port takes its HP figure down with it.

In the port: `field/staging.rs` loads the bundle and has the cut's and the
captions' state, `field/host.rs` the functions, `field/mod.rs` puts the camera
where the cut has it and gives the cast its motions, `field/hud.rs` draws the
captions. Without `--play-events` a scene is skipped as soon as it can be, as
before. Its tune and what is spoken are played (see Sound), and the title of
its place is typed at the start (`s36` to `s39`, `s25`: see "The sign that
names a place"). Its effects are characters the script makes of the files of
its slot 4 (`s53`, `s220`: see "Characters that are effects"), and the mouths
eyes and heads of its cast move as below. Ported since: the sounds of its bank
(`s27`), a head that looks at somebody (the neck's fourth way), and the things
of slot 2 (`s71`, `s150`: a model an event brings for a character to have, at
its node 0x50 or where it stands, 0xFDBB8 and 0xFD0C8; eleven events have any;
see "Things held in a scene").

**Faces** (`scene::Face`, `step_faces` and `face` in `field/mod.rs`). A model's
frame (0xFD24C) has two things done to it once it has been posed: its mouth
(0x100164) and its eyes (0xFFE9C). Each takes the matrix of one node of the
model, node 6 (the jaw) or node 5 (the lids), turns it where it is and writes it
back; both nodes hang off node 4, the head, in the 95 models that have them. The
turn is about a line over the ground: the head's third axis (which lies across
the face: (0, 0, 10) through the head's matrix) is taken, its bearing about the
upright axis found (0x162620 of its x and z), and the node is turned back by that
bearing, turned about the axis ahead by the angle, and turned forth again
(0x1621C8, 0x16216C, 0x1621C8, each of which puts its turn before the matrix).

- **The mouth**, while bit 1 of the character's + 0x6D4 is set. Each frame a
  count of degrees (+ 0x6D8) goes on by eight and a number by chance: under the
  amount (+ 0x6DA; 2 if it is none) two times in four, under twice it one time,
  under four times it one time (0x93A0). The jaw's angle is the sine of what is
  left of the count over the half turns in it, over six: nought to a sixth of a
  radian, at a pace that keeps changing.
- **The eyes**, while bit 2 is set, by a count of frames (+ 0x6DE). At 9999 the
  lids are 0.35 down and stay; at 1 they are up and stay. Otherwise the count
  goes down by one a frame: when it is under three it becomes 120 and up to 119
  more and the blink's own count of degrees (+ 0x6DC) is nought; any other frame
  that count goes on by 50, to 180 at the most; and the lids are down by the
  sine of what is left of it over the half turns in it, over three. So a blink
  is three frames, and there is one every four to eight seconds.

**The face itself** is a layer of the character's animation. A character's
script describes its gimmicks (`num_gimmick`, `define_gimmick`); one of kind 0
plays an animation of the character's own on top of its body's, and the script's
`gim_facial` says which gimmick is the face: for her (`FCHR_P_BRS00`) gimmick 0
plays animation 50 and gimmick 1, the face, animation 51 (the other two are the
chains of her hair, under "Her tails" below). Thirty of the field's characters have a face: her and her
other suits, the bosses, and the people of the story (`S_DAR00`, `S_FOB00`,
`S_GRS00`, `S_HAS00`, `S_LOS00`, `S_MAR00`, `S_TEO00` and their like).

Animation 51 is the faces one after another, five frames of the game apart.
The field's resident script (`SYSTEM.VOL/fld_resident.bin/05.sscr`) has the two
functions that show one, and the engine calls them by name:

- `set_face(n)` (0x10C3B0), called as a character's int 0xA is set (0x10A7C4):
  setting 116 of `s52`. The face's gimmick is given animation 51 (its function
  0xB with 1, 0x10B4D8), told to start from how the model is posed (0xC with 0,
  0x10B5B8), held (0xB with 0: bit 0 of the gimmick's + 4), put at `n x 5 x
  0x800` of its time (0xB with 2) and given no time to come over in (0xB with
  3: the player's + 0xC).
- `shift_face(n, time)` (0x10C444) is the same with the time to come over in:
  action 12 of `s132` and of an event's `ACT` (0x1095D4), with the character's
  ints 0xB and 0xC, which settings 117 (the face) and 118 (frames of the game,
  made the character's time by 0xF000 over 0x1E) set beforehand. Setting 119
  is its int 0xD, which nothing read so far reads.

A gimmick that is held does not go on with its animation: each frame (0x10BC58)
one that is held only has its coming over go on (0x234E4 with no time), and one
that is not is stepped like any animation (0x234C8). So until a script gives a
face, the face's layer plays animation 51 round and round, as her hair's plays
50.

The maps' scripts set a face at once 2064 times and have one come over about
1340 times, mostly as two steps ten frames each: a face that goes with closing
the eyes and then the face meant (5 and then 7, say), and back to face 0
afterwards. In the port: `setting::FACE`, `FACE_NEXT` and `FACE_FRAMES`,
`Character::shift_face` and `expression`, `CharacterAsset::facial` (the layer
`gim_facial` names) and the actor's layer held at the face
(`Animator::set_slot_blended`).

**Her tails** are the other kind of gimmick, a chain (kind 1; the module is
0xE9C48 to 0xEAC78, and `tools_re/out/chain.txt` is its listing). Her script
and those of her nine other suits describe two: `define_gimmick(2)` says a
chain from the node whose id is 100 (function 1, 0x10ADEC: the node is found by
its id, 0x1E20C, and the chain is that node and those under it one after
another, 0x1E5CC: nodes 7 to 10 of her model, and 11 to 13 for id 101), whose
bones lie along the node's own x (function 4: `(1, 0, 0)`, and `(-1, 0, 0)`
for the other), in air of 0.02 (function 7 with 0: the chain's + 0x34), and
for each link (function 3) its bone's length (19.9, 27.222, 18.852 and 27.094;
24.148, 14.299 and 17.251: what the model has between the nodes, and a length
for the tip), its mass (0.0125 at the root to 0.005 at the tip), its strength
(20.5 to 0.5) and its area (0.002 to 0.0004), the three by the square of how
far along the link is (`_get_mass`, `_get_pow_bend`, `_get_area`).

A link is a point at the far end of its bone, kept in the world with a speed
(0x90 bytes: its length at + 0, its place at + 0x10, the node as the model's
file has it under its parent at + 0x20, its speed at + 0x60, where its
animation would have it at + 0x70, mass, strength and area from + 0x80). Each
frame of the model that has its gimmicks on (0x10BDD8) every link has its
frame (0xEA870, 0xEA5F0), with the time the character has of the frame:

1. where it would be by its animation: the node's matrix in the world by the
   bone (its way by its length);
2. a chain that is held (its flag 2) has the link there, at rest, and no more;
3. a push round the point it hangs from (the node itself for the first link,
   the link before for the others) towards that place (0xE9F98): across the
   bone, the link's strength by `1 - (1 - angle / pi)^2` by the seconds the
   time is (it over 0xF000), added to the speed over the mass (0xEA200);
   nothing where the two ways are within a hundred-thousandth of one or of
   opposite;
4. it is put back at its length from the point it hangs from (0xEA274), and
   what that moved it by, by the seconds, is added to its speed and to the
   speed of the link before (the same way for both);
5. what its speed has along the bone is taken off (0xEA43C);
6. the air (0xEA4B4): against its speed, half the air's figure by its area by
   the square of its speed, by one less how much of its speed is along the
   bone, by the frames the time is (it over 0x800), over the mass;
7. it moves by its speed and the seconds (0xEA3AC).

Nothing pulls a link down. Then each node of the chain is turned (0xEABB0,
0xEA8D8): the node as the file has it under its parent, in the world (the
parent as it is now, which for the second node on is the node just turned),
turned about itself so that its bone points at a point between where the
animation has the link and where the link is, as far towards the link as the
chain's weight (+ 0x38) says (0xE9DE4 for the axis and the angle, 0x162DE8 and
0x162C04 for the turn); and that is the node's matrix in the world. The
animation's own turn of a chain's node is not looked at.

The weight and the hold are her script's to say. The engine gives every chain
the resource's + 0x14 as its weight and the resource's bit 2 as its hold just
before the nodes are turned (0x10BE54, 0x10BE64). `set_status` (as her status
is set, 0x10C67C) sets the weight to one and lets go for any status but 3,
standing. `update_time` (0xFCFE4: called with her status, the time she has
been in it, her + 0x8FC, and the frame's time) does nothing but standing; then
with `t` the time stood and this frame's over 0x3C000 (four seconds), the
weight is `(1 - t)^2`, and once `t` is over one the chains are held and the
weight is nothing. A chain is stopped by the air alone, which takes less the
slower it goes: left to itself it would never hang still.

In the port: `brs_engine::chain` (`Chain::step` and `turned`, `Chains` on the
model, `swing` after the animation's step; the turns go to the animator as
`set_facing`, in the model's space and blended between steps like everything
else that is drawn), `field/assets.rs` reads the chains from the scripts
(`Traits::chains`), `field/tails.rs` has her script's weight and hold. A
chain's links begin where the animation has them, where the original's begin
at the middle of the world and are flung from there in their first frame.
Against the walkthrough (her run at 650 to 654 s, seen from behind;
`docs/compare/tails_port.png` has the port with the chains above and without
below, `tails_walkthrough.jpg` the video): the tails trail behind her head as
she runs, where the animation alone has them spread to both sides.

`s159(id, n)` (0xC3314, 0xFE5B4) sets the eyes' count: 9999 and 1 as they are, 0
(or what comes to less) lets the eyes be, anything else is two more than it, 6000
at the most, and the blink's own count starts over, so that a blink is seen at
once. The library has `s159(0, 60)` for her as a map begins, `s159(id, 1)` for
the cast as a scene is set up and `s159(id, 60)` after it; 118 calls shut a
character's eyes. `s160(id, n, how)` (0xC3364, 0xFE678, 0xFE620) sets the mouth
moving by `n` of a hundred, or with nought stills it, has it wait ten frames
(+ 0x6D6), and sets the character's flags 0x6A and 0x6B from the two bits of
`how`. The library's `LIPSYNC(id)` is `s160(id, 10, 3)`, a wait for the caption
(`s240`) and `s160(id, 0, 0)`; it is the function the maps' scripts call most
after the settings (18,873 times).

What the two flags do is at the end of the mouth's frame (0x1003DC), while the
mouth moves and the script's flag 0x1000 is set, which the sound's own frame sets
as a recording begins on the first stream (0xA8A8) and `s30` clears (0xC27EC):
the ten frames are counted down, and after them, once the first stream has
nothing more to say (0xA3DC), a character with flag 0x6A has its mouth stilled
and one with flag 0x6B is stood again (0xF4908) unless it keeps its pose (flag
0x53); each flag is then cleared. The scenes that are filmed (a cut of the
camera's with motions for the cast) do not use `s160`: their mouths are in the
motions.

**Necks** (`scene::Neck`, `pose_head` in `field/mod.rs`). After its mouth and its
eyes a model's frame has its neck (0x1004F0), which turns the head, node 4, and
everything that hangs off it. A character that has a model keeps for it (at its
+ 0x6E0): how the head is turned (+ 0x130) and how it is to be turned next
(+ 0x134); three angles for each (+ 0xF0 and + 0x110, with two more sets nothing
uses, + 0x100 and + 0x120); a character it looks at and the next one (+ 0x170,
+ 0x174); how long it has been going over, how long that takes and how it goes
(+ 0x178, + 0x17C, + 0x180); the turn the head is to have, the turn it has this
frame and the turn it had when it was last started (+ 0x140, + 0x150, + 0x160),
which are turns in the world. The character's flag 0x54 is that the neck turns
the head at all, 0x55 that it is going over.

| setting | what |
| --- | --- |
| 48 | how the head is turned, 1 to 4 (anything more is 1), and the neck is on; 0 has it off (0xC7A2C: int 0x1D). `s143` reads back 0 while it is off, else the number (0xC8258) |
| 49 to 51 | its angles about the axis across, the upright one and the one ahead, in degrees (decimals 0xE to 0x10) |
| 60 | the character it looks at (int 0x2A) |
| 61 | how the head is to be turned next (int 0x1E); 0 has the neck off |
| 62 to 64 | the angles of that (decimals 0x17 to 0x19) |
| 68 | the character of that (int 0x2B) |

`s209(id, frames, ease)` (0xC543C) sets how long going over takes (int 0x1F, the
frames in the field's units of time) and how it goes (int 0x20: 0 to 3, anything
else 0), and starts it (0xE6184, 0xF6188, 0xFF6BC): what was next is what is, the
turn the head was to have is the turn it starts from, the time is nought, and
both flags are set. The character's frame counts the time up and clears flag 0x55
when it is up (0xFEA30).

Each frame the turn the head is to have is worked out by how it is turned:

- 1 (0xFEED0): the head's matrix as the model's pose has it, times the inverse of
  the turn its file gives the head in the model's space (0xFECEC, from 0x1E534),
  is taken for a frame; the turn is the angle about the upright axis of that
  frame, then the one about its axis across, then the one about its axis ahead
  (0x162DE8 for each, 0x162C90 to put each before the last). So the head is
  turned from wherever its animation has it.
- 2 (0xFF088, 0xFF1FC): the same three turns about the world's own axes, put
  before the turn of the character's own matrix (0x1070A8), are where that frame
  is to be; the turn is that times the inverse of the frame. So the head faces a
  way from the body's whatever the animation does with it.
- 3: at a point written into the game, (-2136, 1053, 3051), with the code of 4.
- 4 (0x100610, 0x1007F8): at the character of + 0x170, at the node of its head
  as its pose has it, or where it stands if it has no model (0x1089C0); with
  no such character the turn stays what it was. The way to that point from
  where the head is, is held back by the frame of what the head hangs off (its
  matrix times the inverse of the turn its file gives it, + 0xB0, as + 0x60 is
  the head's own): its part across that frame's upright axis may be no more
  than `prm[+0x64]` degrees (80) from the frame's way ahead, and if it is more
  the way is turned back about the axis the two make by what it is over; then
  its part across the frame's axis across may be no more than `prm[+0x65]`
  (80) or `prm[+0x66]` (30) from ahead, by which side the axis of that turn is
  on, which comes to eighty up and thirty down. What is past 170 degrees round
  is turned about the upright axis itself, and the side is kept: flag 0x6C,
  which saying how the neck turns the head sets (0xFF31C), has the side chosen
  and kept in flag 0x6D, and a way that then comes out on the other side is
  turned back the other way round, so that what walks round behind a head
  does not swing it over. The frame the head is to have is then stood on that
  way: its third axis the way, its first across that and the head's own
  upright axis (or its own way ahead, backwards, if that is the way), its
  second across those two (0x100dc8 on). Last it is tipped, about the axis
  across the way and the way to a point 5 above the one looked at, by the
  angle whose sine is minus 5 over how far that is (0xFE8B8 gives the 5, with
  a 12 ahead that is not used): what looks is taken to be that much above the
  head's node. That frame as a turn (0x1629F4) goes the way of 2 (0xFF1FC).

The turn it has this frame is the one it is to have, or while it is going over
one between the turn it started from and that (0x162CAC), by the part of the
time that is up: evenly, by its square, by one less the square of what is left,
or by half the square to half-way and half of one less the square of what is
left after it. Then every matrix of the head and of what hangs off it is put
through that turn about where the head is (0xFEAC0).

The library's `SET_CHR_NECK_ROT(id, x, y, z, frames, ease, way)` (3,991 calls)
either sets the turn at once (settings 48 to 51) or, with more than one frame,
switches the neck on by angles of nought if it was off, names what is next
(settings 61 to 64) and starts it; `SET_CHR_NECK_RESET` (2,818) does the same
back to angles of nought, or switches the neck off at once; `SET_CHR_NECK_OBJ`
(383) is the same with a character to look at. Nearly all of the first are of
way 1. In the port all four are done (`pose_head` and `look` in `field/mod.rs`;
the side kept is the model's, `NeckPose::side`). The last two have been
checked by figures only: the talks they are in belong to missions a run here
does not get into by itself.

## Films

The disc has 36 films (`USRDIR/MOVIE/*.PMF`, beside `GAMEDATA`): the opening
(`BRS_OP_FINAL`), three for the end, and the story's (`EV01_01_0100B` and so
on, named as the events are that play them). A film is a `PSMF` file: a header
of 0x800 bytes and an MPEG program stream whose stream 0xE0 is the picture
(H.264, 480 by 272, 30000/1001 a second) and whose private stream 1 is the
sound (ATRAC3plus, 44100 Hz, two channels, in frames of 0x2E8 bytes that begin
`0F D0`); `brs_formats::psmf` has the particulars and takes one apart, and
`tools_re/pmf.py` looks into one.

**In a script**, `s83(file)` to `s86()`, as in the first mission's
`EV01_01_0500D`: `EVENT_START1; s83('movie/EV01_01_0500D.pmf'); s239();
s243(0); s84(); s96(8, 1); s85(0); EVENT_START2(0, 1, 32); s20(); s190(0);
s86(); s191(); EVENT_END1 ...`. So a film is played inside an event, which the
pause can skip like any other.

- `s83` (0xC3170): if the script's context has a film task already (its + 0x444)
  that is ended (0xB074); the pictures the script has up are put aside
  (0xD0D74 with 0, undone with 1 when the film is over); the field puts by what
  it does of itself (0xE68AC, which sets its flag 0x2C; 0xE68F8 undoes it); a
  task "Movie" is made (0xAE3C: its routine 0x118E70, number 0x37E of what is
  drawn) and the film made ready (0x11863C).
- 0x11863C: the film's path is `disc0:/PSP_GAME/USRDIR/` and the name; the
  PSP's own player is made (`scePsmfPlayer`, the calls at 0x1647C8 to 0x164838;
  its object at 0x22700), told not to go round again and to give its picture
  as four bytes a pixel (0x1647D0 with 0 and 1, then 1 and 3), given the film
  (0x1647E8) and started in its own sense (0x1647F8, with picture codec 0xE,
  sound codec 0xF, each stream 0, speed 1), which is not yet playing: the byte
  at 0x172BF0 (over) is cleared and 0x172BF2 (made) set.
- `s84` waits until the player's state is 2 or more (0x1188A4: it has read
  enough to play).
- `s85` (0xC3240) has it play (0x1188E4: 0x164810 with 0 and 1) and sets
  0x172BF1 (playing). Its argument is not looked at.
- `s86` waits for 0x172BF0 (0xC3264); then the task is ended, the pictures come
  back and the field takes up again what it put by.

**The task** (0x118E70; its messages 0 made, 1 unmade, 3 a frame, 4 drawing).
Made, it keeps the rate the game is drawn at (0x830) and has it drawn every
sixtieth while the film is up (0x808 with 0; put back when it is unmade). Its
frame (0x1189FC) reads the pad only for the PSP's own buttons (bits 0x90000
while playing stop the film where it is, 0x118924; bit 0x40000 while stopped has
it go on): nothing of the game's buttons does anything to a film, and skipping
one is the pause's doing (0x118958 stops it: the player is stopped and let go
of the film, 0x11854C marks it over). When the player says it has played
through (its state 0x200) 0x11854C marks it over.

Its drawing (0x118B58, with 0 and 0 for where): nothing once it is over;
otherwise the player goes on by a frame (0x118B2C, while playing), its picture
is taken (0x1647F0, into a texture 512 wide) and drawn as it is, 480 by 272 at
the screen's corner, in fifteen strips of 32 (the sixteenth of a texture's
width that the PSP draws fastest). As task 0x37E it is over the field's display
(0x300), a script's pictures (0x340) and windows (0x341) and the window people
speak through (0x37D), and under the menus (0x401 on), the screen's fade
(0x600), a scene's captions (0x602) and the pause (0x610): which is why a film
event brings the screen back with its fade after starting the film
(`EVENT_START2` and `s20`).

**How loud a film is.** As the film is given to the player the sound's manager
is told (0x118824: 0xA264, which stops the tune and both streams at once and
hands the film's two buffers of 0x2000 bytes to the music's stream, 0x12070;
0xA2B8 and 0x12118 undo it when the film is stopped). While that is so the
music stream's thread (0x121E4, from 0x122B0) takes the film's sound from the
player instead of decoding a tune (0x164818) and puts it out at the stream's
own share of full (0x12384: its + 0x12088 times 32768), which is what the
options have the music at, of 127 (0x9DD8, 0x1200C): the music's volume as the
game sets it (0x11F60, the + 0x1207C beside it) has no part in it. So a film
is as loud as the options' music, 60 of 127 in a new game; and about the
opening film the game makes that option 127 and puts back what it was, in the
opening's script (`s231(6)` before `s85` and `s231(7)` after `s86`: 0xE64DC
keeps it in the field's + 0x1D43, 0xE6508 puts it back, each with 0x5E8 to
have the manager take it up) and on the title (0x11A0C0 as the film's task is
made, kept in the title's + 0x39C; 0x11A2EC in its state 0x12). None of the
other films has that: they are heard at what the player has the music at.

**In the port.** `brs-extract --sound` takes each film's sound out and has
ffmpeg make `MOVIE/<NAME>.ogg` of it under what was unpacked (ffmpeg reads the
picture of a `.PMF` but not its sound, so the frames are written out as a
recording of the kind the game's others are; the sound begins 2416 samples
before the first picture, which are cut off). The game reads the film itself,
from `extracted/PSP_GAME/USRDIR/MOVIE` beside the data folder
(`GameData::film`), and decodes its picture with OpenH264 on a thread of its
own, a few frames ahead (`brs_engine::film`). The script's side is
`scene::FilmAsked` (`s83` makes it, `s84` waits for `ready`, `s85` sets
`started`, `s86` waits for `over` and does away with it; a skipped event has
none, and the pause's skip does away with one), and `film.rs` does what it
says: ready when the first frame is decoded and the sound can be played at
once, started with its sound (`sound::Request::Film`, `FilmStart`,
`FilmStop`; the sound is as loud as the options have the music,
`Mixer::music_share`, which `Request::MusicOptionFull` makes full about the
opening film), shown by the game's clock
at layer 0x37E with black beside it where the window is wider than the
screen, held while the pause is up, over when its time is. Seen in an unseen
run of the story's opening (`--map STG05_0A1 --play-events --skip-messages`):
the opening film comes after the first lesson's battle, plays its 96.4 seconds
and the first mission's map follows. The title plays its film the same way
when it is left alone (`TITLE_NOTES.md`). The end's are the script's own
choice: the last map's `EV07_01_0100` plays `BRS_ENDROLL2.pmf` if flag 6152 is
set and `BRS_ENDROLL.pmf` if not, and `EV07_99_0000` `BRS_ENDROLL3.pmf`; an
unseen run of the end (`--map E_STG00_05 --variables 10004:101,220:102
--play-events --skip-messages`) has the end roll made ready (288.7 s) and
playing, and without `--play-events` goes on through `s205(1)`, the records
and the question of the clear save to the screen between missions. Not ported:
the PSP's own buttons stopping a film.

## Sound

**The manager** (0x9CEC, at `*0x170CF0`) has a stream for the music and two for what
is spoken (two more can be made), each a decoder of ATRAC3plus with a buffer of its
own (0x11488; the music's 0x180000 bytes, the others' 0x40000). A thread (0x121E4)
goes round them: for each stream that is playing it decodes **a block of 2048
samples**, scales it and hands it to the hardware, which takes it when the block
before has been played. So a block is 46.4 ms, and it is what every fade is counted
in:

- **Stopping** (0x11D5C, `stream, blocks`): the stream's level, 0x8000, falls by
  `0x8000 / blocks` a block (256 blocks at most) and the stream stops when it is
  gone; with no blocks it stops with the next block.
- **Volume** (0x11F60, `stream, volume, frames`): the volume, of 127, moves by
  `(volume - now) / (frames / rate)` a block until it is there, `rate` being 2 while
  the game runs at half the screen's rate (0x850), which is always on the field: a
  fade "of 30 frames" takes 15 blocks, 0.7 s. With no frames the step is the volume
  itself, so it is there in a block; except to nothing, where the step is nothing
  and it never gets there.
- What is played is `sample * level / 0x8000 * volume / 128 * option` for the music
  and `volume / 127` for the rest (0x12E84, 0x12F18, 0x134C4). The option is the
  player's volume for that kind of sound, of 127 (0x9DD8): bits 0-6 of the options
  word (field manager + 0x21E74) the music's, 7-13 the sounds', 14-20 the voices'. A
  new game has them from the one record of `db_resident.lpk/009.xtc` (0x14D8E4):
  **60, 127, 127**: the music at under half of what is spoken.

**Music** is `gamedata/sound/bgm_<name>.at3`, the name from the field's `lst_bgm`
(26 names of 16 bytes; the count is at + 0x0C of `dat_tbl`): 0 `title`, 1 `system1`,
2-7 `field1`-`6`, 8-9 `mission1`-`2`, 10-15 `battle1`-`6`, 16-21 `event1`-`6`, 22
`ending`, 23-24 `kaisou1`-`2`, 25 `song_edit_b`. Playing one (0x9F84, 0x9E84) stops
what is playing at once and starts the new one when the stream is free (the
manager's frame, 0xA6F0), at the volume last asked for, with the file's loop if it
has one (0x11724 sets the stream to loop, 0x12AEC tells the decoder). A file with
no loop, like the jingles, is played once.

**What a map's script does** (table 1):

| function | what |
|----------|------|
| `s57(n)` | tune `n`, unless it is the one the script last asked for (the context's + 0x28), which then goes on as it is (0xC1D08). So a map that asks for the tune of the map before does not start it again |
| `s58(blocks)` | the tune fades out and the script has none (0xC1D64) |
| `s29(n)`, `s202(n)` | recording `n` of the event that is loaded (slot 5, 0xE91CC) on stream 0 or 1, at 127 (0xC277C, 0xC2830). If the stream is playing, that fades out over 8 blocks first and the new one starts when it has stopped (0xABA8). A script that is being run through fast plays none |
| `s30()`, `s203()`, `s264()` | stream 0, or 1, fades out over 8 blocks (0xA2DC) |
| `s122(n)` | jingle `n` of `SYSTEM.VOL/jin_pack.bin` on stream 1 (0xC28C0). The five are the same recordings as `SOUND/BGM_BTL_RESULT`, `BGM_MIS_CLEAR`, `BGM_MIS_RETIRE`, `BGM_MIS_START` and `BGM_MIS_RETIRE2`, in that order; the only one a map's script plays is 2 |
| `s112(volume, frames)` | the music's volume to `volume`, the sounds' to `volume * 100 / 127` percent, and the context's flag 0x800 set (0xC253C) |
| `s113(frames)` | both back to full, and the flag cleared (0xC25C8). `s81` does this over 16 frames before a battle if the flag is set |
| `s27(kind, sound, volume)` | a sound of a bank (0xC23B8): kind 0 the bank that is always there, 1 the map's, 2 that of the event that is loaded. Answers a number to stop or change it by, or -1 (always, when run through fast). `s28(n)` stops it (0xC24CC); `s211(n, what, value)` sets its volume (what 0) or its place between left and right (what 1) (0xC24E8) |
| `s54(place, sound, x, y, z, min, max, range)` | a sound of the map's bank at a place of the map, one of 16 (0xC30E8, 0xD0AA4); `s1(place)` takes it away; `s123(place, off)` silences it, or not. See below |
| `s177()` | waits until the tune is playing (0xC5928: the sound is not busy, 0xA6BC, and the music's stream plays, 0xA6CC); four scenes of the last stage and two more start with it, to go by their tune. `Wait::Tune`, `Mixer::tune_begun` |
| `s83(file)` | a movie |

The messages people send her play their recordings on stream 0 as well (0x38E64),
stopped at once when the line is over (0x39AF4) and over 8 blocks when the window
is closed (0x39078). The window has sounds of its own, of the bank that is always
there: 0x31 as it opens and as it closes (0x39D08, 0x39D80), 3 when cross brings a
line's text up or cuts it short (0x39164).

**The sounds about a map** (the task at 0xD0E3C): each of the 16 places is a
looping sound of the map's bank, started unheard. Every other frame (0xD0FC4) each
is given a volume and a place between left and right. Within `range` of where she
stands it is `min + (max - min) * (1 - distance / range)`, and to the side it is on
as the camera looks: the angle between the camera's line of sight and the way to
the sound, two thirds of it up to a quarter turn (61 at most), and from there back
down. Out of range it is `min`, in the middle. (The volume one sound comes to is
never less than the `min` of the place before it: the original carries the value
over.) All of it is scaled by a level that `s112` and `s113` move with the music's
volume (0xD0DF4), a step a frame. When a battle begins every one of them is made
silent and the task stops (0xD0D74 from 0xD8890), and starts again when the battle
ends.

**The system script and the events' scripts** have the same three banks (0xD4224):
native 0x08 `(kind, sound, volume)` plays a sound and answers its number, 0x0A the
same with a place between left and right, 0x4B stops one. For an event's script
kind 2 is the event's own bank.

**The player's steps**: only her scripts have `update_time`, which
the resident script (`fld_resident.bin/05.sscr`) gives a body: while she walks or
runs (statuses 0 and 1) a step sounds when her animation passes time 0x1800 and
0x5800 (frames 6 and 22; for status 12, 0x5000 and 0x7800). The sound is of the bank
that is always there, by the kind of ground under her (0xFC954): `lst_gnd_se` has
two sounds, one for each foot, for each of the ten kinds (0x15 and 0x14 for kind 0,
up to 0x25 and 0x24; none for kind 9). The kind is the second byte of the attribute
of the collision triangle she stands on (0x13EC8; the first byte's bit 1 says it is
ground), found by a probe 200 up and down from her each frame (0xFD3CC). In the
port: `field/steps.rs` says which foot comes down between two frames of her
animation, `sound_steps` in `field/mod.rs` asks for the sound each frame of the
game (unless her flag 0x28 is set), the engine's `Triangle::kind` and `Ground::kind`
carry the kind of ground, and `FieldTables::steps` is `lst_gnd_se`.

**A battle begins with a sound** of the bank that is always there, played as the
screen's wipe starts (0x11ACDC, from 0xD8890 once the field's flag 0x17 is clear):
0x2B, or 0x2C when the enemy group's flag 2 is set (0x93154: the groups she strikes
her pose over when they are beaten). In the port: `start_battle` in `field/mod.rs`
(there is no wipe yet, so it sounds as the battle comes up).

**Taking up what is in reach has a sound**, 3 of the same bank (0x10D844): the
field's handling of the buttons (0x10D624 on) keeps what she is by (field + 0xE70)
and, when cross is pressed and it has something for cross (0xD3938), or square and
it has something for square (0xD3954), plays the sound and runs what it has. In the
port: `notice_touches` in `field/mod.rs`, for cross (nothing for square is ported).
The same code plays sound 7 in place of the menu that start or triangle would bring
up while the game's value 0x2714 is 0x2C (0x10D9BC, 0x10DA30); neither menu is
ported.

**Coming up to something there is to look at** (not ported). Each frame 0x10DBEC
goes through the contacts of kind 1 (home points, boxes) and, for the character
each belongs to, sees whether she is in its reach (0x10D18C, 0x117208). As she comes
into it (the character's flag 0x4B is clear, and is then set): a character with
flag 0x4A makes her whole (0xD88) with sound 0x10, one with flag 0x4D gives sound 3;
the contact's marks 3 and 4 are switched on (0xD37A8) and, if the character is an
effect, its second effect is set playing or faded in (0x1032C0, 0x10325C,
0x1031B0; none has a second: see "Characters that are effects"). As she leaves,
the flag and the marks are cleared and that effect is faded out. The flags are
settings 42 and 56 of `s52` (0xC794C and 0xC796C in its
switch at 0xC7520, through 0xE4F60 and 0xE5014), and none of the 88 maps' scripts
gives either setting to anything: a home point is made with `s42(id, 60, ..)`,
`s52(id, 2, 1)` and `s224(id, 12, 1)`. So unless a system or event script sets them
the healing and both sounds never happen, and the port leaves them out.

Other sounds of the field's code, all of the bank that is always there unless
said, and where the port has each: 0 as triangle brings up the menu (0xF1988:
`menu/mod.rs`); 0x2D and 0x39 of a leap (0xF832C when her flag 0x72 is set, and
0xF86FC; 0xF868C, 0xF8960: `SOUND_LEAP`, `SOUND_LEAP_EFFECT`); 0x96 with the
burst that shakes the camera (0xF999C: `field/effects.rs`); 0x2F as a symbol
notices her (0xFADDC: `field/symbol.rs`). Three are never heard and are not
ported: the sound a character's script names with `se_destroy`, as it is
destroyed (0xF94E0, its parameter 0xC by 0x102738: no script has one); 0x38
(0xF5890) of the dash on square (0xF77A4, 0xF584C), which takes her figure 0x11
(5) off her figure 0x13 and so needs that to be 5 or more: it is nought from the
game's start, and what adds to it is a consumable of a kind no item is of
(0x56854); and two sounds of the map's bank whose numbers `s96(13 to 16, n)`
would give the field (0xE4B90: + 0x2470 on, none until then, 0x11594C), played
in her status 2 once it has gone on for 1000 (0xF793C) and for 3000 (0xF7B04)
while the field's flag 0x2E is set: no map's script calls `s96` with those.

**Skipping an event stops what is heard.** The menu that offers the skip (its task's
code is from 0x12D52C on) holds the tune (0xA194), both streams (0xA484) and the
banks' sounds (0xD30C8) while it is up, and keeps which of them were playing (bytes
+ 0xC, + 0xD, + 0xE of its data). When the skip is chosen (button 0x4000, 0x12D9CC)
it sets its byte + 8, and as it goes (0x12D474) it lets the tune go on, stops each
stream that was playing over one block (0xA2DC) and, having let the banks' sounds go
on (0xD316C), stops every one of them (0xD3648, the library's 0x15E1C8). The sounds
placed about the map go with the rest; their task does not start them again, and
the scripts place them anew (`s54` comes again in the missions' own functions). In
the port (`skip` in `field/host.rs`, `Request::StopEffects`): the same, but the
sounds about the map are spared, since without `--play-events` every event is
skipped.

**An event's bank goes with the event.** `s59(n)` (0xC40A4) loads an event through
0xE313C, which first frees the one loaded before if the field's flag 0xD says there
is one (0xE9964: its bank is unloaded, 0xD3424, and what was playing of it stops),
and `s61()` (0xC41B8, 0xE327C) frees it without loading another. While an event is
being skipped `s59` loads nothing: it leaves the number (+ 0x650, bit 0x8000) for
the loading to be done once the skip is over. A sound of the event's bank asked for
after the event's end (`return s27(2, 1, 127)` after `EVENT_END2`, as the opening
has it) still plays, the event being loaded until the next `s59` or `s61`. In the
port: functions 59 and 61 in `field/host.rs` push `Request::DropBank`; so do a
change of map and the start of a mission.

**Battles leave the music alone.** The field's code for going into a battle and
coming out of it (0xD8AA8, a state machine at field + 0x1DC0) would stop the tune,
start another from a buffer and put the field's back afterwards, but only while the
field's flag 0x2B is clear, and what it would start from is never loaded (field +
0x1D30 is only ever cleared). Every script sets the flag before a battle with
`s96(9, 1)` (0xC72C8, 0xE3E38). So the tune of a battle is the script's doing: an
event's battle is `s57(10); s96(9, 1); BATTLE_EVENT(..); s58(10)`, a generator's
`s57(10) .. s57(2)`, and a battle with an enemy that goes about has no `s57` at all:
the field's tune plays on through it.

In the port: `crates/brs-game/src/sound.rs` is the mixer (one tune, two streams,
the banks' sounds, fades by the clock in blocks, the options' volumes), fed with
requests that the scripts leave on the scene (`field/host.rs`, `field/comm.rs`,
`field/system.rs`; the sounds about a map are in `field/scene.rs`). The recordings are
decoded from the Ogg Vorbis files `brs-extract --sound` makes of them
(`docs/FORMAT_NOTES.md`). A run with no window plays nothing and times everything
the same; `--mute` plays silence through the real device.

## The scene's own script: its fog and its lights

Slot 5 of a map's inner bundle holds up to eight `SSCR` scripts (one in every map
that has any: 83 of the 88). As a map starts (0x111614) the field clears its flag
0x1A, puts a colour of the scene back to 161 in 255 for each part (field + 0x1E20),
and has each script run its `init_scene` (0x112204) with a table of engine
functions of its own (0x172B80, twenty-five entries, 0 to 3, 11 to 15 and 17 to 24
filled):

| function | what |
|----------|------|
| 19 `(on)` | the fog is on or off: field flag 0x1A (0x111FBC) |
| 20 `(near, far)` | where the fog begins and where it is whole (0x112000: field + 0x1E18, + 0x1E1C) |
| 21 `(red, green, blue)` | its colour, each a part of one (0x11205C: field + 0x1E0C) |
| 17 `(r, g, b, a)` | the scene's ambient colour (0x111ED4, 0xF0174 with 0: field + 0x1D00) |
| 18 `(2, n)` | which of the lights what follows is said of (0x111F60: field + 0x1D20) |
| 11 `(0, kind)` | that light's kind (0x111A14): 0 one that shines from a way, 1 one at a place |
| 12 `(which, r, g, b)` | one of its three colours (0x111AA4): 0, 1, 2 |
| 13 `(r, g, b, r, g, b)` | its first two colours at once (0x111BB0): the first is the first three, the second what the last three are more than those, each part held between 0 and 1 |
| 14 `(x, y, z)` | where it is (0x111D74: its + 0x20) |
| 15 `(a, b, c)` | the way it shines from, in degrees, kept in radians (0x111E10: its + 0x10) |
| 22 `(a, b)`, 23 `(a)` | not read (0x1120D4, 0x112140) |
| 24 `(red, green, blue)` | the colour the screen is cleared to before the scene is drawn (0x11218C: field + 0x1E20, used by 0x10EA90); 161 in 255 of each where a script does not say, which 61 of the 83 leave it at, and near black in the story's opening and the fifth stage's halls |

**The lights.** The field has two sets of eight lights, 0x70 bytes each
(0xF009C: set `s` and light `n` at field + 0x1600 + 0x380 `s` + 0x70 `n`), and
an ambient colour for each set (field + 0x1D00 + 0x10 `s`). A light: its kind
at + 0, its angles at + 0x10, its place at + 0x20, three colours of four parts
from + 0x40. As a map starts every light of both sets is put back (0xEFEC4):
the colours (0.75, 0.65, 1.0), (0.25, 0.35, 0) and white, and the ambient
colour a fifth of white; its kind, angles and place are left. The scene's
script then fills set 0, and set 1 is made a copy to go back to (0xF0264,
which copies two lights and the ambient colour from one set to the other). A
map's script changes set 0 for a scene: `s166(light, r, g, b, r, g, b)`
(0xC55CC) is function 13 in hundredths, without the holding; `s164(r, g, b, a)`
(0xC5520) is function 17 in hundredths, each from 0 to 100; `s165()` (0xC5848)
puts set 0 back from set 1.

A character is lit as it is drawn (0x10F3B4), by one light: the one of set 0
whose number it has (its + 0x164, 0x1072E8). One with flag 0x15 is drawn with
the PSP's lighting off (what `s53` makes for an effect, 0xE41E8). Otherwise
the way to the light is worked out: for kind 0 the vector (0, 0, 1) turned
about the axis across by the light's first angle and then about the upright
one by its second (0x162224 and 0x1621C8: each puts its turn before what the
matrix held, so the first given is the first done), which is (sin b cos a,
-sin a, cos b cos a); for kind 1 the way from the character to the light's
place, made a unit. It is put through the camera's matrix (0xDA048: field +
0x190), since the game gives the PSP its models already in the camera's space,
and given to the PSP as light 0, shining from a way, for diffuse and specular
(0x15AB84 with 0, 0 and 6). The light's three colours are its ambient, its
diffuse and its specular colour (0x15ABE8 with 1, 2 and 4), and the set's
ambient colour is the scene's (0x15AA20). So function 13 and `s166` name the
colour of what is turned away from the light and the colour of what faces it:
the first is added everywhere, and what the second is more than the first is
added by how far a surface faces the light.

Which light: every frame, as a character's model is put where it is (0x106260,
0xFD24C), a ray goes down through it from 300 over it to 200 under it
(0xFD3CC), and what the collision says there (`docs/FORMAT_NOTES.md`, CLIM) is
the kind of ground it is on (its + 0x161, the first kind where nothing says;
the sound of her step is chosen by it) and the number of its light (+ 0x168,
and + 0x164 too unless its flag 0x1E is set, which setting 34 of `s52` sets
with a light's number, 0x10A97C; no map does). The maps mark their shade so:
the first stage's streets have light 0 white and light 1 (0.9, 0.7, 0.4), and
a sheet that says "light 1" lies over the ground wherever a building's shadow
is painted on it.

The colours a character is drawn with are its own (four of them at its +
0x8B8: the scene's ambient colour and its light's three), and only follow the
scene's: what they are to be is set each frame after the ray (0xFFB40; each
part kept as the low byte of what it comes to in 255, 0x105724, so one below
nothing comes out near the top, as the red of the second colour of
`s166(0, 75, 85, 100, 55, 85, 100)` does), and each part goes two hundredths a
frame towards that (0x105868, 0x10580C), but only in a frame in which the
character's status is the one it moves in (0xFFC0C, 0x1096A0; when the field
has no frames, under the pause or the menu, in every one). So she darkens over
half a second as she runs into a shadow, and stays as she was if she stops on
its edge. One that is made, or that a script puts somewhere, has them at once
(0x108BE8).

What a material makes of the light is in its own display list (the lists of
the 721 materials of the field's 243 models, `tools_re/out/_insm_state.py`):
which of its colours are its corners' own (command 0x53: ambient and diffuse
for 575, none for 122, whose colours are then white), the power of its shine
(0x5B: 50 for those, 10 for these), and how its picture is put on (0xC9: by
what the light gives for 611, by it and twice over for 59, added to it for
27). The shine's colour is the light's third, which is nothing in the scripts
of four of the six stages and white in the fifth's. With the first stage's
lights a character in the open is drawn at its full colours, as the port drew
every one before.

**How a material is put on what is behind it** is in the same list. The game
has the PSP mix every model it draws: the character's draw switches the alpha
test and the mixing on (0x10F3E0, 0x10F3E8: states 0 and 4 of 0x15A400) and
sets it to mix by how much of a thing there is (0x10F400: 0x15B13C with 2 and
3), and a scene's frame switches it on too (0xAB914). Each material then says
(counted over the 1027 materials of the 363 models of the whole game):

- **whether it has a picture** (command 0x1E): 39 have none, the arrow that
  shows the way among them (`g_obj16`: no picture, no normals, corners of
  16-bit colours from (34, 68, 170) to (51, 255, 238), all there). The PSP
  puts no picture on such a one whatever function its list names, and the
  arrow's names the adding one; the port used to add white to it;
- **by what it is mixed** (0xDF, with the fixed colours 0xE0 and 0xE1): 413 by
  the fixed colours white and black (0xAA), which is not mixing at all, and
  these leave out what is under half there (0xDB = 0xFF8007); 597 by how much
  of it there is (0x32), leaving out only what is not there at all
  (0xFF0006); 17 added to what is behind by how much of it there is (0xA2, the
  second fixed colour white), which are lights and glows: two things of the
  field (`g_obj23`, `g_obj24`) and pieces of the models that stand about nine
  maps;
- **whether it writes how far it is** (0xE7): 23 do not, the 17 that are added
  among them.

How much of a corner there is, under the PSP's light, is the corner's own
alpha times the alpha given with the light all about, and that is the
character's float 0x1E (its + 0x4BC; 0x10F608 makes a byte of it for
0x15AA20). It is one from when the character is made (0x109A5C) and only one
thing changes it: a character that is destroyed and fades (below, 0xF96E4).
Only a thing of the ordinary kind has colours of its own (0x108BA0: its +
0x1F4 is nought, as 0xFCA00 makes one): the others would be drawn with the
scene's colours and a half for the alpha (0x10F598), and none of them is. Kind
1 is what `s53` makes of an effect (0x1029B4 from 0xC3FBC), drawn with the
PSP's lighting off; kind 2 is a wall (`s185`: 0x1010E0 from 0xC4088) and what
action 5 of `s132` puts in a ring about a character (0x1056A0 from 0xF62D0),
which no map's script asks for: neither has a model.

In the port (`brs_engine::lit`): `Look::of` reads all of it, `Blend` is the
three ways, and `Look::alpha_mode` chooses how bevy draws a material as a
model is built (`ModelAsset::build`): the added ones added, one that is mixed
mixed where anything of it is partly there (a pixel of its picture or a corner
of its pieces: 180 materials of 162 models, 118 of them by their corners) or
where it writes no depth, and every other cut at a half as before. Bevy would
write no depth for what it mixes, so the lit material's pipeline is told to
(`LitExtension::specialize`), and a piece under no light that needs that, or
whose picture is added or taken twice over (the glows of a battle's enemies),
gets a lit material with no light. `Lit::alpha` is the character's, and has
what is mixed drawn so while it is under one. What is mixed is drawn after
everything solid, by how far it is, where the PSP draws a model's pieces in
their order.

In the port: `field/scenery.rs` reads the lights from the scene's script
(`Lights`, `Light::towards`); the scene has them and the copy
(`Scene::lights`, `lights_kept`; natives 164 to 166 in `field/host.rs`);
`Scene::light_characters` is the ray and the colours each frame (`scene::Lit`,
by `CollisionWorld::marks`); and the models of the field's characters, with
what they carry, are drawn by `brs_engine::lit` (`lit.wgsl`), which has the
PSP's sum at every point of a face, not at its corners alone. A light's kind,
place and angles are not put back as a map starts, so one that a script left
unsaid would have the map before's; no script leaves them unsaid for a light a
character is lit by (`Light::UNSET` in `field/scenery.rs` says so), so the
port, which has every light begin unset, shows the same.

Before the scene is drawn (0x10E6A0) the PSP's fog is switched on with those
figures if the flag is set, off if not. What the field starts with (0x118170) is a
fog from 10 to 20000 of (0.5, 0.5, 1.0), which no map is left with: 81 of the 83
scripts turn the fog on and say what it is. The first map's is from 200 to 20000,
(0.7, 0.95, 0.9), a pale green haze; a typical scene's two lights are a white one
and a warm one of (0.9, 0.7, 0.4), both at angles (210, -80).

In the port: `field/scenery.rs` reads the fog as a map is loaded and the field's
camera is given it (`fog_of_the_map`). With it, and with colours mixed as the PSP
mixes them (`brs_engine::texture`), a frame of the first mission's start has the
walkthrough's colours to within three parts in 255 wherever it was measured
(`docs/compare/mission_start.png`: the far buildings 114, 121, 97 against 115,
120, 98). The lights the characters are lit by came later (see "The lights").

**The glare.** The game has a task of its own for it, `Glare`
(0x1AA04, made at 0x31D0 and kept at the game's + 0x20030), which every map's
script speaks to: `s153(n)` (0xC18D4) sets how strong it is, `n` of 255 (95 as a
map begins; 140, 100 and others in scenes), switches it on and clears two of its
flags, and `s254(1, a, b)` (0xC1B4C) sets those two, to 1 for the length of a
scene and back to 0 after it (`s254(0, a, ..)` sets a third). The task's messages
from 0x1003 on: its strength (+ 0x10, with bit 1 of its flags), read it back, a
second strength added to the first (+ 0x1C), on or off, read, bit 2 (it is not
drawn), bit 0x10, read, bit 4, read, bit 8, read. When it is drawn (its message
4, 0x1A6B4; only while it is on and the two strengths come to more than nothing)
the sum, held between 0 and 1, is gone towards by a tenth of what is left each
frame (+ 4 and + 8), and two passes are drawn, lists of the PSP's own drawing
commands that the code writes out each time (read 2026-10-08):

- Before them the screen is copied (0x1AC6C: 512 by 272, 0x15A7A8) so that it
  can be drawn from.
- The first (0x19F4C) draws that copy into a picture a quarter of the screen's
  size (120 by 68; 0x1878C aims the drawing at it), as fifteen upright strips,
  smoothed, each corner's colour the strength in every part with an alpha of
  one, the texture taken by the colour. It is mixed (0x189B4: blend command
  0xA2, fixed colours 0x202020 and eight in each part) as what is drawn by its
  own alpha, added to what the small picture held by 8 in 255: so the small
  picture is the screen by the strength, with a trace of what it was a frame
  before. With bit 4 of the task's flags set (`s254(1, 1, ..)`) the texture's
  alpha counts too and only points whose alpha has any of its low four bits
  set are drawn: the alpha of the screen's picture, which on the PSP is its
  stencil, then says what glares.
- The second (0x18B70) draws the small picture back over the whole screen,
  smoothed, by its own alpha and added to what is there whole (0xA2 again, the
  second fixed colour white): the screen has the small picture added to it.

`s153(n)` clears bit 4 and `s254(1, 1, 1)`, with which every scene of the story
begins, sets it; scenes end with `s254(1, 0, 0)` or `s254(1, 1, 0)`. What the
screen's alpha holds with bit 4 set: the map's pieces and the characters are
drawn through 0x19464, which, while the glare has bit 4 (and for all but the far
piece bit 8, the third figure of `s254`), has the stencil count each point's
drawing up from nothing in its low four bits (0x15B108 with "one more" for what
passes, 0x15B0D4 always, 1, 0xF). So a marked point's alpha is one or two in
255, and what the first pass draws by it is nothing to speak of: with bit 4 set
nothing glares, and with it clear all the screen does, the screen being added to
itself by the strength.

In the port: `scene::Glare` (kept in `Scene::work`, as the task outlasts a map;
natives 153 and 254; a frame of it in `field::step_glare`), and the engine's
`canvas::Glare`, which what lays the scene's picture under the interface adds
(`scene.wgsl`: the small picture's points worked out from the picture as it is
read back, on the game's own numbers). In the opening hall, where the script
has just called `s153(95)`, the port's picture then has the walkthrough's
colours: the backdrop 115, 123, 125 against 114, 122, 125 at 22.4 s, the floor
51, 44, 48 against 50, 45, 49 (it is 37 in 100 lighter than without). Not
ported: the trace of the frame before in the small picture (8 in 255) and the
task's second strength (its + 0x1C, which no map's script sets).

Two others set the same task (every sender of its messages was listed on
2026-10-14). The ride on the bike sets it to its road's figure as the road is
built (`BIKE_NOTES.md`, word 5: 100 on every course), and the walkthrough has
it whole: the ride's far haze, which is the fog's colour alone, is (194, 215,
177) there against the port's (140, 158, 127), 1 + 100/255 times it in every
part; the ride now shows it (`bike/show.rs`, `show_glare`) and its haze comes
out (195, 220, 177) (`docs/compare/bike_glare_*`). A battle's map sets it too,
from its script's seventh figure (0x96EF8 in `BtMapActor`: 95 on the first
stage's), but nothing of it is seen: the same wall of the first stage's battle
map has the same spread of brightness in the walkthrough (quarters 84, 120,
144) as in the port without any glare (89, 124, 148), where a glare of 95 would
lift the darker quarter by a third (`docs/compare/battle_no_glare_*`). Why it
does not show (the task's bit 4 left set by the field's scenes, or a battle's
drawing marking nothing) has not been followed; the port draws no glare over a
battle, as the walkthrough has it. The title turns the glare off (0x1193A8,
message 0x1006).

**Things held in a scene** (`Staging::prop`, `Character::props`, and the
models in `show_characters`). An event's bundle has a slot for the things its
people hold: file 2, a bundle for each thing with its model at
`01.bin/00.insm`, loaded with the event as resources of kind 2 (0x116DD4 finds
one by kind and number). Eleven events have any, fourteen models in all, the
commonest of them something a man she saved gives her. `s71(id, place, n)`
(0xE3554, 0xFDBB8; not in a skipped event) gives a character thing `n` in one of
five places it has for such (0x20 bytes each from its + 0x510: flags, the
resource, a model made of it and that model's animation, 0x105C5C, which none
of the fourteen has). Each frame the character's model is posed (0xFD2E4), a
thing that is there and not put away is given the matrix of a node of that
model: node 0x10 for the first place, which is the right hand, 0x0B for the
second, the left, and node 0 for the rest. `s150(id, place, 0, away)` (0xFE3C4)
sets or clears the bit that has it put away. When the event is let go its
things are taken from everybody (0xE9964, 0xFDC8C). In the port the thing's
model hangs off the node's own entity. Not seen in a run yet: nothing here
starts one of those eleven scenes by itself.

**Things of a map's own** (`scene::object_resource`, `CharacterAssets`). File 2
of a map's scene bundle (`11.bin/02.bin`) has a bundle for each of up to four
things that are the map's alone, sixteen in one mode of the field (0x1108DC,
0x110734): a model at `01.bin/00.insm` and its animations in `02.bin`, with no
script. 23 maps have 42 of them (the one looked at, in the fourth map of the
first stage, is a gate some 730 wide and 800 high). `s110(n)`
loads one as a resource of kind 4 and number `n`, or counts another use of one
that is loaded (0xE4044), and `s111(id, n)` makes character `id` of it if there
is none of that number (0xE4134: class 0x43, animation 0 named, flag 0x15; with
no such resource it would be made of the player's). The model's points are
where the thing is on the map, so most are left where the world begins, and a
script gives one its state with the calls any character has: `s64(id, 0)` holds
a gate shut at the first frame of its animation, `s65(id, 0, 90)` plays it open
over ninety frames and `s64(id, 90)` holds it open, with settings 33 and 27
nought and 47 on. What keeps her out is a wall of the script's own (`s185`,
`s116`). In the port such a character's resource is a number below those of the
field's list, and its bundle is looked for in the map that is up.

## Going on to another map

`s68(map, entrance)` (0xE3474) names the map to go on to and the entrance to come
into it by; `s69()` (0xE34C4) names those of the map-change trap whose callback is
running. Both set field flag 0xF, and the change itself (0x115E88) happens when the
script's event is over. Going to a map of another stage (byte + 0xC of the map's
record) clears flags 3000 to 3585, the enemies beaten on the stage. The next map's
script then runs its `OPEN_EVENT`, which puts her at the entrance (it reads system
values 0 and 1) and walks her in.

**The ways out** are traps of the map-change kind, which every map's script
makes with its `SET_MAP_CHANGE(id, map, entrance, x, y, z, w, h, d, angle,
by_button, margin)`: `s43(id, map, entrance)`, `s44` for its box, `s45(id, 2,
by_button)` and `s45(id, 5, margin)`. Every map gives 0 for the button, so they
are walked into, and their callback (`CALLBACK_MCG_TRAP`) walks her 400 on,
waits ten frames and calls `s69()`. The big blue arrow that stands in the
street on the way to one in the second mission is a character the script puts
there for that mission (`SET_YAJIRUSHI(id, x, y, z, angle, size, moving)`:
`s42` with resource 162, `g_obj16`, 400 over the place given; setting 88, its
size in hundredths, which is float 0x1D, all three of its scales at its +
0xB0; and animation 1 or 0), to show the way. `SET_BATSUJIRUSHI` puts a cross
(resource 163) the same way. Compared with the walkthrough at 1146 to 1151 s
(`docs/compare/arrow_walkthrough.jpg`, `arrow_port.png`): solid blue with
paler stripes.

**What becomes of the screen** (0x116048 on). The field's flag 0x17 is cleared.
Unless the screen counts as covered already (0x29974), the picture the fade
keeps is told to darken (0x2C298 with 1, which clears its flag 1), the fade is
started as kind 0x804 over 0x78 frames of the interface, field + 0x1E34 is
cleared and flag 0x17 set: a picture is taken of the scene alone as the next
frame is drawn, and it stays up, darkening over two seconds, while the next
map is made ready. A screen that is covered is left as it is (0x10EA74 does
nothing). Then the windows the map had up are closed (0xD0C7C), the sign of
what a button would do and the name of the place are sent away (0x115DFC,
0x115E44), and the field goes into its state 3 (0x116850).

State 3 (0x11620C) has a step of its own at field + 0x34, which 0x115E88 chose
by whether the next map is loaded already (0xF2554 and 0xF257C: maps are read
ahead while she walks, 0xF22E0 being whether one is on its way): 0 and 1 wait
for the reading, 2 is for a map that is there and waits only for flag 0x17 to
have been cleared, the picture taken (0x116334). Each ends in 0x115A68, which
enters the map. The next map's script then names the place (`s104(16, 4)`),
puts her at the entrance, and brings the screen back by the kind last used,
`s7(0, -1, 30)`: the picture goes over thirty frames, darkening still.

In the walkthrough (she leaves THE END OF ANKH ST. northwards at 1151 to 1155
s, running through the chevron; the map was read ahead): the picture stands
still from 1154.717 s, ANKH STREET is there at once at 1154.783, and the next
map shows through evenly from then to 1155.75, thirty frames. For the eight
frames from 1154.783 a small bar and a disc are at the bottom right where her
gauge is, and her gauge comes back over the next five: the sign of the disc
being read, which also shows as the map screen opens (1156.4 s).

In the port (`change_map`): `Scene::keep_screen` does the screen's part, and the
map is not left until the picture has come; then the map's pieces and
characters go, the next map and its collision are loaded in their place,
`Scene::onward` keeps what belongs to the field (its flags and figures, the
pop-up work, a window that is up, what covers the screen), the
system script's `sys_fld_enter_map` runs and the map's script is opened.
The player's character does not come along as it was. Leaving a map
(0x116670, by 0x11648C) unmakes every character on the field's list (+ 0xE24:
each is unlinked, 0x1168C8, and unmade, 0x106A04), hers with the rest, and lets
go of every resource but those marked to stay and the one she is made of (the
list at + 0xE38; hers by 0x14BE78); it clears the 0x40 traps and the lists of
what she is in touch with. So she is made anew on the next map, and nothing a
script did to her character on the last one holds there. The fifth mission
shows it: the first map hides her for a radio scene (`s52(0, 0, 1)`) and goes
straight on to another map, whose scene has her turn and walk without showing
her first. Until 2026-10-08 the port carried her character over as it was, and
that scene waited for ever on a walk she, hidden, never made.
Checked by a run from map 6, ANKH STREET, into the first map with a mission
under way (`--map STG01_0A --variables 10004:1,220:110 --script
"wait:3,to9010/11400:4,wait:6"`): the street she left gives way to the one she
comes into over a second, with the new name up from the first frame. Before
this the port cut to black and faded in. Not ported: the sign of the disc
being read (nothing is read while she waits), and her gauge going while it
shows; a trap's margin (`s45(id, 5, ..)`).

## Leaving the field, and the missions' list

`s173(how)` (0xE4B40) leaves the field: 1 a mission cleared, 3 given up, 5 tried
again, and 2 for the ride on the bike, after which the field starts again on the map
and entrance of system values 42 and 43. What the game does next is 0xE6E94: the
field's task is ended, and for 1 and 3 the screen between missions is made
(0x5CEFC), for 2 the task `BikeMiniGame` (0x147360 `startBikeMiniGame`, its
courses the files `MINI_GAME/BMG_*.BIN`), for 0 something else (0x7369C, not
read). A script says which ride with `s96(41, n)` just before: 0 the story's (the
trike of the second stage's `WEB TUBE ENTRANCE`, touched in scenario 21), 5 the
one she is asked "Get on the Black Trike?" for, 3 another (`EX_BIKE`). The map
the ride leads to (`d_stg02_08`, by `s96(42, 20)`) asks how it came out with
`s260()` as its first event starts. In the port the ride is ridden
(`BIKE_NOTES.md`) and says how it came out (`scene::Work::ride_outcome`); a run
that passes battles over has it ridden by a rule of thumb. When
a mission fails its window asks what to do, and the answer (system value 50) becomes
5, 3 or 4 (0xEE8B0). The field then calls `sys_fld_exit_field(how)`, and except for
2 and 5 the game goes to the screen where the next mission is chosen, which calls
`enter_stage_select` on coming up and `mission_select(n)` for the one chosen; the
field then starts anew on the map and entrance `mission_select` has set as system
values 0 and 1.

A mission's state is variable 100 + n: 0 closed, 1 newly open, 2 open, 10 begun, 255
cleared. `enter_stage_select` opens every closed mission that has at least one
required mission, all of them cleared, and whose flag, if it has one, is set. So the
story runs 0, 1, 2, 3, 4, then 5 (a free hunt) and 20 (the second stage) together.
`tools_re/missions.py` lists them all.

The screen itself is in `STAGE_SELECT_NOTES.md`. One thing it does matters
elsewhere (0x5EE64): the first time a stage's missions are shown it sets that
stage's flag, 0x180D for the first to 0x1812 for the sixth (and for any but
the first says that the stage's challenges can now be looked at, 0x5DDD4).
The menu's CHALLENGES shows as many stages as have those flags set one after
another (`MENU_NOTES.md`).

In the port (`crates/brs-game/src/hub/`, `field::leave_field`): the field
tells the system script how it was left (`hub::left`), and unless the
mission is to be tried again the screen comes up over the field, which stands
still under it: none of its scripts has a frame while the screen is there.
When a mission has been chosen on it the field starts again from nothing
(`Scene::afresh`). `--mission n` starts the game on mission n as if it had
been chosen there, and `--stage-select` on the screen itself. What the
system script's value 56 has the field do is under "The field's system script".

## A mission failed

**What a script does about a battle lost.** The library's `GAMEOVER` (in every
map's script; `GAMEOVER_FOR_SPECIFY`, `GAMEOVER_FOR_BOSS` and `M_GAMEOVER` are its
kin for staged battles) waits for the events to be over (`s238`), closes her gauge
(`s104(3, 3)`), sets flag 6167 (nothing of the display is drawn), `s105(15, 0, 1)`,
has the tune fade (`s58(5)`), plays jingle 2 on the second stream (`s122(2)`) and
has an event played by the number of the mission (118 for the first, 130, 145,
157, 167, 317 and so on: someone tells her off over the radio); then she is put
down (`START_MOTION_F(1, 0, 0, 1, 64, 65)`), the screen comes back over 30 frames,
the events and the jingle are let finish (`s238`, `s264`), 30 frames go by and the
system script's entry 403 is run: `mission_fail`, which closes the display's
panels, opens window 13 and waits for it (`n46(13, 4)`, `n47(13)`). What follows
in the script (reading system value 50 and `s173(5)` or `s173(3)`) is never run:
the wait for window 13 (0xD5CA0 to 0xEE838) lasts for as long as there is a field,
and it is the window that leaves it.

**The field's part** (the state at field + 0x5424: a byte of flags, a byte of
state, the window's task). Opening it (0xEE760) sets field + 0x1E34 and the field's
flags 0x17 and 0x56 (the field is no longer free: nothing of its own interface is
drawn) and state 0. Its frame (0xEE8B0, after the scripts' each frame) waits for
flag 0x17 to have been cleared and for the field to be still (0x115D7C, 0xF0754),
stops it (0xF055C: the object at field + 0x53D4, which is what a pause works by),
and once it has stopped (0xF070C) has a picture of the screen shown in place of
the scene (0xEE7CC: the picture the screen's fade keeps, 0x29A1C, drawn each frame
by 0xEE73C) and makes the task (0x42760). When the task has done (0x427D8) it
keeps the choice made as system value 50 (0x117CD8), does away with the task, lets
the field go again (0xF05A8) and leaves it (field + 0x536C and 0xEFA38, which are
all that `s173` is, 0xE4B40): how 5 for the first choice, 3 for the second, 4 for
the third.

**The task** ("ifFldFailedTask", 0x42A6C; its number is 0x400, over all the field
draws and under the screen's fade) loads `gamedata/if/menu/if_mission_failed.gz`
(three textures and a layout), keeps the volume the tune was to have (0xA254),
turns the tune down to 1 of 127 (0xA204), plays jingle 4 of `jin_pack.bin` on the
first stream at full volume (0xABA8) and makes the menu. While the menu asks it
only steps it. The third choice has it make the screen a game is loaded from
(0x4FF8 with 0 and 1) and wait for that; if a game was loaded (0x50FC) it ends,
otherwise the menu asks again (0x6E6C4). When the menu has gone it lets the file
go, stops the jingle if it is still heard (0xA3DC, 0xA2DC) and puts the tune's
volume back.

**The menu** (0x6E59C to 0x6F314; 0x64 bytes: flags, its state at + 4, the cursor
at + 0x5C, a count of frames at + 0x60). Its layout has sixteen textures, of which
it gives eight (0x6EAE8): file 11 of the pack that is always there, the file's own
three (as textures 1, 2 and 13) and files 15, 16, 19 and 20 of the field's pack
(3, 6, 14 and 15). One player plays its sets, each over what it was playing
(0x319BC): 0 brings all of it (a dark sheet over the picture, a pattern drifting
up it, "MISSION FAILED" by the game's emblem with two rings turning about it, the
three choices at the foot on the right); 2, 4 and 6 have the cursor come onto a
choice (blue within two frames) and 3, 5 and 7 leave it (white again over
twelve); 8, 9 and 10 are a choice going once it is made (fifty frames); 11, 12
and 13 bring a choice with the cursor on it already (it blinks, and is the last
thing to come, at frame 80). Set 1, all of it going, is never played.

- Starting (0x6EF70 with 0): sets 0 and 11, and the screen's fade cleared
  (0x297E4 with 4 and 1).
- Coming (state 1, 0x6E718): once the count is 61 or more, sound 0x11 of the bank
  that is always there, once; the count goes up by a frame's worth; when every
  track has played through (0x31C20) the menu asks.
- Asking (state 2, 0x6E854). Cross: on "RETRY MISSION" the choice is made; on
  "STAGE SELECT" too, unless the save's flag 6169 is set (0x6E838), which has
  sound 7 instead; on "LOAD" sound 3 and state 3, which is the task's cue. Up and
  down, while held: when the pad counts the button as pressed (below) the cursor
  moves, round the ends only on a new press, and past "STAGE SELECT" while flag
  6169 is set (0x6F100, 0x6F20C); sound 2 when it has moved. Up comes before down.
- A choice made (0x6EF70 with 1): the screen's fade goes to black over 60 frames
  (0x297E4 with 2 and 0x3C), the choice plays its going, sound 3, the count starts
  again. Going (state 4): when every track has played through and the fade is over
  (0x29948) the menu has gone (state 5).
- Its draw (0x6EC74) draws the player's tracks one by one, each by a colour of the
  menu's choosing (0x32280): white, but "STAGE SELECT" (group 10) by an alpha of
  0x40 while flag 6169 is set, and, as a choice goes, the pattern (group 6) by
  what is left of sixty frames of the count and the rings (groups 19 and 20) by
  what is left of fifty.

Flag 6169 is set by the story's opening and cleared by the first mission's end
(`MIS_SCE01_01_0300`, just before `s173(1)`): until then there is no other mission
to choose. The menu that offers to skip a scene reads it too (0x12D458).

**The pad counts held buttons again** (0x10940, a part of its frame; the menus go
by pad + 0x10 where the game goes by + 4, the buttons newly pressed). The four
directions and L and R do: a new press starts a wait of 24 sixtieths (0x18), after
which every one of them that is held counts as pressed, and again every 4 from
then on (the wait is counted down by 1 or 2 a frame, 0x11184). Letting go of one
of them ends it for all.

**What the save's backup is.** `mission_select` backs the save up (the system
script's function 0x66, 0x2398) and `sys_fld_exit_field` restores it for how 3 and
5 (0x67, 0x23C4) and makes her whole. The backup copies her figures (0x64 bytes,
0xDD4), what she carries (0x400 bytes, 0x1C24) and how far each of the 128
challenges has got (0x14AF08, into flags 9384 on and 9512 on and variables 20200
on); the restoring puts back the challenges alone (0x14AF74). Nothing has been
found to read the other two copies: what she gained in a mission that failed she
keeps.

**In the port**: `field/failed.rs` (the menu's logic, `Scene::open_failed` and
`Scene::step_failed`, run after the scripts in `run_script`), `field/hud/failed.rs`
(its layout and its draw, on layer 0x400), `controls.rs` (`Pad::repeat`),
`Request::KeepMusicVolume` and `MusicVolumeBack` in `sound.rs`. Under the window
nothing of the field moves (`Scene::held_by_failed`: characters are not stepped
and their animations stand still), which stands in for the picture of the screen;
the display and her gauge are not drawn. `--lose-battle <group>` counts a battle
against that group as lost without fighting it, to see all this unattended; the
window is not one that `--skip-messages` passes over. Seen in an unseen run
(`docs/compare/mission_failed.png`): the window over the field, "STAGE SELECT"
faint in the first mission and the cursor passing it, and "RETRY MISSION" starting
the mission again. "LOAD" brings up the list of saves (`SAVE_NOTES.md`), and the
menu asks again when that is left without loading. Not ported: the few
frames the original takes to stop the field and load the file. (The challenges'
progress is put back as the field is left: see "The records (challenges)".)

## A mission cleared

Window 12 (0xEDC0C in the window command, for the mission the save's variable
10004 names, if any): the task "ResultFldTask" is made (0x49F1C; its number is
0x300, the display's), given the mission's title (0x4A1DC: the record's + 4),
told to list and to come (0x49FC8), and the field's flag 0x55 is set. The
system script's `mission_clear` waits for it (0x4A184 answers while it is
neither unmade nor gone). Listings: `tools_re/out/result_fld.txt` (the task,
0x49E80 to 0x4A900), `result_if.txt` and `result_if2.txt` (its interface,
0x44D40 to 0x464F8), `result_layout.txt` (its layout).

**The task** (its object is 0x28 bytes: + 0 its state, + 4 flags, + 0xC the
interface, + 0x14 how many challenges it lists, + 0x24 the tune's volume as it
was to be, 0xA254). Its frame (0x4A550), by state:

| State | What |
| --- | --- |
| 1 | the tune is turned down to 1 (0xA204), the first jingle of `jin_pack.bin`, which is that of a battle won, is played on the first stream at 0x60 (0xABA8); then 2 |
| 2 | until the interface's first place has played its set through (0x450C0): then 4 |
| 3 | never set: it would show the window of each challenge in turn (0x4A338, 0x14B92C); a map's script asks for those itself (`s276`) |
| 4 | cross: the system's sound 3 and it goes (0x4A140: 0x44F74, state 5). Otherwise up and down move the list (0x4520C), with sound 2 when it has moved |
| 5 | until the first place has played its going through: then 6, the jingle is stopped if it is still heard (0xA2DC) and the tune is as loud again as it was to be |
| 6 | gone |

Every frame the three places go on (0x44EA8) by the frames of the game's
frame, two.

**The list** (0x49FC8) is of the records' list 2, those whose counts have gone
up, but only the ones whose count has come to its goal: each with its name, its
number and a share of 1.0 (0x4518C). So it is what the heading says, the
challenges cleared, and the parts of the draw for a challenge only part of the
way (its hundredths in figures, 0x46024, their sign, 0x45FEC, and its bar cut
by the scissor to its share, 0x46144) never show. Four rows are shown; with
five challenges or more up and down move it a row, held or pressed, round its
ends on a new press only.

**Its layout** is the field's pack's files 12 to 14 (`if_fldsys.lpk`) with
eighteen textures (0x45440 on): the shared pack's 11 and 10, an empty place,
the field pack's 15, two empty, its 16, the shared 9 (figures), its 17, the
shared 20 (small pictures) and 7, an empty place, its 18, an empty place, and
its 19 to 22. It is a sister of the pause's: the same sheet, pattern, frame,
strip at the foot and rows. Three places (0x44EF0, 0x44F74): all of it but the
rows (set 0 as it comes, 1 as it goes), a row with a challenge (2, 3) and a
row without (4, 5); with fewer than five challenges the first place plays set
6 (and 7) as well, which has what the list's knob runs in fainter. Set 0 by
its frames: the sheet comes blue (0, 160, 255 at an alpha of 102 by 14) with
the pattern; the star (group 32) grows down to its size by 14, and the words
MISSION and COMPLETE (groups 29 and 44) come in from either side by 24, turned
ten degrees, with a line of three stripes each (30, 31), and blink once; at 80
the lines go and by 88 words and star have gone up sixty; from 96 to 116 the
sheet turns white and the rest comes: the grid, the frame, the heading MISSION
RESULTS at the top left (41 to 43), the title's mark and place (39, 40), the
strip (47 to 52), the rows; from 104 to 116 the words turn level and black; at
114 the star goes and the two rings come in its place (35, 36), which turn for
good (one round in 500 frames, the other the other way in 270). All of it has
come by 124. As it goes (set 1, 32 frames) everything fades; the rings keep
the turn they had (0x44FD8 reads it, and the draw turns them by it, 0x45AF0).

**Its texts** (0x44D24, six): the button's words at the foot, the system's
text 60 ("(cross) NEXT"), in the capitals at ten, thirty-six to a line, to the
right of object 64's box and two lower (0x458C4); the mission's title at
object 51 and the four rows' names at object 16, in the text's own font. A
row's small picture is drawn at object 15 (0x46250). The knob (0x45BA0) is
there for five or more: as tall as four rows are of all the challenges in the
75 it runs in (what it runs in is 77), and as far down as the first row shown.

**In the port**: `field/cleared.rs` (`Cleared`: what the task and its
interface keep; `Scene::cleared`; `system::open_window` makes it,
`SystemScript::update` has its frame, and `run_script` gives it the buttons
and its list) and `field/hud/cleared.rs` (the layout and the draw, on the
display's layer). A run with `--skip-messages` takes it as read two seconds
after it has come, which is the port's own. Compared with the walkthrough at
954.2 s (the words) and 957 s (all of it): the same, to the pixel where the
video lets that be seen, and the colours of the strip and of the tinted sheet
within a few units (`docs/compare/mission_complete_vs_walkthrough.png`,
`mission_complete_words_vs_walkthrough.png`). In the video it comes at 953.3 s
and the frames of its layout are the video's sixtieths: the words go up at
954.77 and the rest has come by 955.2. The field's flag 0x55 says no more than
that the window's task is there: it is set as the window is opened (0xEDC5C),
the field's frame unmakes the task once it is done and clears the flag
(0xEDA48 to 0xEDA80), and a system function reads it for the script to wait
by (0xD5D24); the port's `Scene::cleared` is that. Once the jingle plays, the
task has its stream go round for good (0x4A7D4: 0xA5A8 with stream 0 and 1,
which is 0x11EE8, the stream's bit 2 and the library's call at 0x164890 with
-1); but the jingle (`jin_pack.bin/000.riff`) has no place to go round from in
its file, so it is heard once, as in the port. (Of the five jingles only 001
and 002 have such a place, and a recording that has one goes round in the
port whoever plays it.)

## The sign that names a place

White letters of a sheet of their own at a corner of the screen, in up to three
lines. Two things put one up: the field, for the name of the place she comes
into (window 16), and a map's script, for the title a story scene opens with
(the task "Location"). Listings: `tools_re/out/place_if.txt` and
`place_if2.txt` (the sign, 0x46DB8 to 0x4804C), `place_field.txt`,
`place_field2.txt` and `place_field3.txt` (the field's side, 0xEE9C0 to
0xEEF90), `place_script.txt` (the task, 0xCFEC8).

**Its data** is `if_rsdsys.lpk/022.lpk`, file 22 of the pack that is always
there (0x2CC94, + 0x64). `000.ptm` is a sheet of 128 by 48 in cells of 11 by 11:
A to K, L to V, then W X Y Z, a blank, a colon, a dash, a full stop and a star,
then the ten figures; each letter white with a dark edge to its right and below
it, which fills the cell. `001.xtc` has 84 places of 16 bytes, four texts of
which three are the lines: places 0 to 11 and 79 have three (a date, a position
and a name: "25 SEPTEMBER:2051", "37.77-122.41", "SAN FRANCISCO"), the rest
one, the name of a map. `002.xtc` has 41 letters of 8 bytes: the character (a
text) and three signed bytes, how far right and down of the pen it is put and
how far it moves the pen: 11 for all but the space, which is 5, and the full
stop, which is 5 and put 4 back.

**The sign** (0x46DB8 makes it, 0xF78 bytes): + 0 flags (2 gone, which it is
from the start; 8 busy; 0x20000000 all typed; 0x40000000 told to come), + 0x10
what it is doing (1 coming, 2 going), + 0x14 the place's record, + 0x18 a
clock, + 0x24 and + 0x28 the letter typed next, + 0x30 where it is (0x472A8
sets it: 0 the top left, from (6, 6); 1 the bottom left, from (6, 217)), + 0x34
the star, + 0x5C three rows of 32 letters of 0x28 bytes (alpha, place, place in
the sheet, size, whether it is a space, the lot it goes with), + 0xF5C the
rows' counts, + 0xF68 frames to wait, + 0xF6C and + 0xF70 two counts, + 0xF74
the lots that have gone.

- It is laid out each time it comes (0x47A30): the lines 13 apart, each
  character looked up in the table of letters (0x472B8; one the table does not
  have is the star, 11 wide), put at the pen plus its two bytes, and the pen
  moved by its third and one more. Where in the sheet goes by the character
  (0x47B90 on, with a table at 0x166D88).
- The lots (0x47D9C): five lots of a sixth of the letters each, spaces counted,
  drawn with the game's random numbers (0x93A0) from those not yet taken; the
  neighbours on its line of a letter just drawn are kept back for that lot, so
  no two of a lot stand side by side. What is left over is the sixth lot.
- Coming (0x47058) it is typed, or all there at once. Typed, its frame (0x4737C:
  two of the interface's frames a frame, 0x850, and none while the pause is up)
  goes by two counts that both run 0, 2, 0, 2: on the first of each pair of
  frames the star is at the place of the next letter (not seen over a space),
  on the second the letter is there in its stead, with sound 9 of the bank that
  is always there at 42 of 127 (0xD34A0), which a space has not. Two frames are
  waited after a space and eleven after a line. All typed, the star stands a
  letter past the last: a frame later it is at 229 of 255, and it fades out and
  in over twenty frames each way; when it has faded out the fourth time, 69
  frames on, the sign has come (0x47280).
- Going (0x4717C) it is gone at once, or after four frames a lot at a time: a
  lot is at 255, at 127 and gone in three frames, the left-over lot first,
  eighteen frames in all. Then it has gone (0x47258).
- It is drawn (0x46EBC) unless the save's flag 6167 is set: each letter a
  sprite of its cell, white by its alpha (0x32AD4), and then the star.

**The field's** is an object at field + 0x1D88: + 0 flags (1 made, 2 another
waits, 8 up, 0x10 to go as soon as it has come, 0x20 to go at once), + 8 its
state (0 coming, 1 standing, 2 going), + 0x14 the frames it is to stand and
+ 0x28 those it has, + 0x18 on the one that waits. When window 16 is opened
(0xEDC8C) the one that is up is told to go at once (0xEED78 with 0) and the
place is named (0xEED54): the place of the map's record, its + 0x10, for the
frames of the field's tunables' + 0x14, which are 200. With none up it is made
at the bottom left and shown all at once (0xEEC70, 0xEEA2C, 0xEEAA0); with one
up it waits until that has gone. Its frame (0xEEDF8) is among the display's:
come, it stands its frames and goes a lot at a time (0xEED78 with 1); gone,
the one that waits is put up, or it is done with. It is told to go, a lot at a
time, when a touch or a trap starts an event (0x115E44 at 0xE83AC) and when
the map is left (0x1160E0). The menu and three more places tell it so only
while the field's flag 0x45 is set (0x115C28), which no code found sets. A
map's script opens the window where the map is come into in the ordinary way:
`s104(16, 4)` in its opening, in the branch for when no part of the story
starts there.

**A scene's title** is the task "Location" (0xCFEC8; drawn at 0x601, over the
screen's fade and under the captions), kept at the script context's + 0x454.
`s36(place)` makes it (0xCEDBC; one there was is done away with); `s37()`
waits until nothing is loading and it is ready to be started (0xC2F1C);
`s39(frames)` starts it, typed at the top left (its message 0x101C);
`s129()` tells it to go (0x101D); `s256(where)` puts it elsewhere (0x101F);
`s38()` does away with it; `s25()` waits until there is none (0xC3030). Its
frame (0xCFFEC) is the sign's; when the sign has come it counts two a frame
up to twice the frames it was started with, tells the sign to go, and ends
when that has gone. Choosing SKIP from the pause does away with it (0xCEF48,
at 0x12DAA8). The maps' scripts only use `s36`, `s37`, `s38`, `s39` (with 30
or 60) and `s25`. The first map's opening has it so: the picture goes dark,
the scene is set, `s39(60)` types the three lines over the dark, and
`s190(0); s25(); s191()` waits for it, which is all of that stretch that can
be skipped; then the first cut starts and the picture comes back.

**In the port**: `field/place.rs` has the tables (`Places`, kept as
`FieldTables::places`, with `MapEntry::sign`), the sign (`Sign`), the field's
(`PlaceName`, in `Scene::place_name`: `system::open_window` names a place,
`SystemScript::update` has its frame, an event's start and `Scene::onward`
tell it to go) and the scene's title (`Title`, in `Scene::location`: the
functions are in `field/host.rs`, and `MapScript::update` has its frame);
`field/hud/place.rs` draws either, on the display's layer and on
`layer::LOCATION`. The lots are drawn from a run of random numbers of the
sign's own. The sheet is drawn from a copy in which each cell has its own edge
once more all round it: a letter fills its cell, and enlarged to a window the
pixels at its edge would else be mixed with the letter next to it in the
sheet, which showed as faint lines.

Compared with the walkthrough frame by frame, by reading the video's pixels
(the title is up from 172.83 s to 181.6 s; in thirtieths of a second, which
are the game's frames there): the star alone at 5185; a letter on every other
frame with the star on the frames between; two frames more after each space
and eleven after each line (the lines begin at 5185, 5232 and 5267); all
typed at 5294; the star at 229 at 5296, none at 5305, full at 5315, and so on
to none at 5365; the first lot at half at 5431 and the last gone at 5447,
seven letters to a lot with the spaces counted; and the picture comes back
over the fifteen frames after. The port has every one of those frames the
same (`the_opening_scene_is_staged_cut_by_cut_with_what_is_said` has the title
up 262 frames, which is 5185 to 5447). Pictures:
`docs/compare/place_title_vs_walkthrough.png` (176.55 s) and
`docs/compare/place_name.png`, which is the port's alone: the field's sign
has not been looked up in the video yet.

## The sign of what a button would do

A ring with the cross in it and a word, ACTION, CHECK or TALK, which comes up
over her when she is by something a button does something with, and goes when
she is no longer by it. Listings: `tools_re/out/place_if.txt` (the sign,
0x464FC to 0x468D0), `sign_field.txt` (the field's side, 0xEDCE0 to 0xEE3B0),
`sign_buttons.txt` (the field's frame, 0x10D624 to 0x10DBE8),
`sign_contacts.txt` (the list of what she is in touch with, 0x10DBEC),
`sign_near.txt` and `sign_trap.txt` (a trap's tests).

**Its data** is the signs' layout, `if_rsdsys.lpk/018.lpk` (the battle's cursor
and marks are of it too), sets 6 to 14, three for each word: the set it comes by
(twenty frames: it rises ten and fades in), one that has the button shrink away
in four frames, which nothing plays, and the set it goes by (twenty frames: it
sinks ten and fades out). ACTION is groups 19 and 20, CHECK 21 and 22, TALK 23
and 24: in the first a dark disc, a ring with four ticks, a dark bar and the
word on it (texture 12, `009.ptm`, which has ACTION, TALK and CHECK down it); in
the second the cross (texture 13, `010.ptm`).

**The sign** (0x464FC): + 0x10 a layout object of its own, with a place for each
word (0x30710: sets 6, 9 and 12), + 0x14 flags (1 shown, 2 going, 4 its set is
through), + 0x18 which word, + 0x1C to + 0x24 that word's three sets (0x46774),
+ 0x28 and + 0x2C where it is (0x46768). 0x467F0 shows it (the first set from
its start), 0x46844 has it go (the third), 0x46894 hides it at once; its frame
(0x46688) has the place go on until it has played its set through (0x31C20),
and 0x468B8 says whether it still plays. It is drawn (0x466F8) unless the
save's flag 6167 is set: the place of its word, *about where the layout has
the first object of its first group* (0x32288 takes that object's place from
every track's), with the layout's outer matrix a move to where the sign is
(0x321F8). So the word begins sixteen left of that point, the ring is from 45
to 19 left of it, and all of it is thirteen above and below.

**The field's** is an object at + 0x1D64, kept as the sign that names a place
is: + 0 flags (1 made, 2 another waits, 8 up, 0x10 to go when it has come, 0x20
asked for, 0x40 to go at once, 0x80 behind the camera), + 0x10 its state (0
coming, 1 standing, 2 going), + 0x14 and + 0x18 where on the screen. 0xEDFA4
asks for it with a thing's records (0xEDD64: the word is the first record's
byte); 0xEE050 sends it away, by its set or at once; 0xEE0F0 says whether it
stands and 0xEE11C whether it is asked for. Its frame (0xEE144) first finds
where it is: her place (0x1179D8, 0x106BB8) and fifty up, turned as she is
(0x1090E0), through the camera's two matrices (0xDA07C, 0xDA0B0) to the screen,
x / w × 240 + 240 and −y / w × 136 + 136; with z under −100 it is behind the
camera and not drawn (0xEDF20). Then by state: come, it stands, or goes if
that was asked meanwhile; gone, the one that waits is put up with its word, or
it is done with.

**Who says what a button does.** `s42` makes a character with TALK for cross
(0xE1920: the first record's byte 2, there; the contact's flag 0).
`s52(id, 2, word)` gives cross its word and `s52(id, 3, word)` square
(0xC7578); `s52(id, 0x24, on)` and `(id, 0x25, on)` take what a button does
away or give it back (0xC75CC); `s52(id, 5, on)` says whether a button does
anything with it at all (0xE374C: the contact's flag 0). The maps' helpers
have home points, junk, boxes and the points of special battles CHECK (685
times `s52(id, 2, 1)`), containers ACTION (89 times `s52(id, 2, 0)`), and
arrows, crosses, the walls of bosses and a box once it is opened nothing
(`s52(id, 5, 0)`). `s67` makes an event's trap with ACTION for cross, a button
doing it and walking into it doing it too (0xC43A8); `s45(id, 0, word)` and
`(id, 1, word)` give the words (0xC85D8); `s45(id, 2, on)` says whether a
button does it, and if one does, walking into it does not (0xC8624: the
contact's flag 0, and the trap's flag 2 cleared); `s45(id, 31, degrees)` is
how far from the way its box faces she may be turned (0xC8914: the trap's
+ 0x110); `s45(id, 38, on)` has walking into it do it after all, and then no
button (0xC8644: the trap's flag 2 set, the contact's flag 0 cleared), as two
traps of the tower have it that take her on unasked. `s45(id, 37, n)` gives
it a sign (0xD3D18: a thing of kind 4 of the module that shows small things,
0x3D9AC, kept at the trap's + 0x434, with the trap's flag 0x19), and
`s45(id, 36, on)` hides that (flag 0x1A). The sign is set 15 of the signs'
layout: a soft dark round eighteen across that comes over eight frames, and
four white corners about it that go on turning, a quarter turn in about
seventy-five of the set's frames (two of them to a frame of the game), since
the sign is kept when its set has played through (0x3D0C4) and
has a frame whenever the field does (0x115704). It is drawn with the marks over
characters and under the same flags (0x10F2E8, 0xD3DB8), where the camera has
the middle of the trap's box, while she is no further from that than
`prm[+0x12]`, it is before the camera and on the screen, and field flag 0x5A is
clear. The figure `n` is kept with the sign and nothing was found to use it.
The box faces along its
own z (0xED570) and she along hers
(0x107CEC); the angle between the two, both taken level, must not be more
(0x117274), and a trap that allows half a turn takes any. Of the maps' traps
235 are made to be done by a button and 353 to be walked into; nine are given
CHECK or TALK (`s45(502, 0, 2)` on the second map of the second mission). No
script gives square anything.

**The field's frame** (0x10D624 on), in this order:

1. What she has walked into that starts of itself (0x118330, the first in the
   list): a character with flag 0x78, without 0x79 and not hidden, a container
   (0x76) only while she strikes; a trap with its flag 2. The sign is sent away
   by its set (0x115DFC) and the thing's event starts.
2. If the sign is asked for: when what it is for is no longer in the list it
   is sent away (0x10D958). Else, once it stands, cross with the first record
   there, or square with the second (0x10D7B8): the common bank's sound 3; which
   button it was and the record's byte are kept (0x117684: field + 0xE78 and
   + 0xE7C, which `s47(3)` and `s47(4)` read and no script does); the name of
   the place is sent away (0x115E44); the thing's event starts, a character's
   `CALLBACK_NPC` or a trap's; and the sign is sent away.
3. Start and triangle (the pause and the menu), and square (0xF332C).
4. With no sign asked for, the first thing in the list a button does anything
   with (0x11743C: the contact's flags 3 and 0; a character that is not
   hidden; a trap that is not off) has the sign asked for (0x10DAF4) and is
   remembered at field + 0xE70.

**In the port**: `field/action.rs` has the sign and what the field keeps of it
(`ActionSign`, in `Scene::action_sign`) and the frame's part (`frame`, which
`notice_touches` in `field/mod.rs` calls with the list it keeps,
`FieldState::in_touch`); `SystemScript::update` has the sign's own frame. What
a button does is `Character::prompts` and `by_action`, `Trap::prompts`,
`by_action`, `on_touch` and `facing` (`field/scene.rs`), set by `s52` and
`s45`. `field/hud/action.rs` draws it with the signs' layout
(`interface::signs_layout`), at the place the port's camera has the point
fifty above her. Before this the port took cross only by what had been given
CHECK, at once, and fired every event's trap as it was walked into: so nobody
could be talked to, and what was to be done by a button did itself. Not
ported: the sounds as a thing is come to (a character's flag 0x4A: sound 0x10,
and 0xD88; 0x4D: sound 3); what a character whose + 0x1F4 is 1 does as she
comes to it and leaves it (0x1032C0, 0x10325C); a trap's margin (`s45(id, 5,
..)`, the contact's flag 5: once in, she is in it out to a second box); the
traps that carry her somewhere (kind 3, 0x10E05C); square's other use. A map
change's trap is made with ACTION too (0xE1DC4), but every map's script has it
walked into (`SET_MAP_CHANGE`, under "Going on to another map"), and so she
does in the walkthrough.

Against the walkthrough, at the first map's home point (804.08 to 804.92 s;
`docs/compare/action_sign.png` is the port's): the ring is at x 210.6 and the
word CHECK from 229 to 282, at y 211.5; the port's are 210.5, 228 to 281 and
207, from a camera a little differently tilted. It is first seen two frames
after it is asked for and whole five frames later, which is the layout's set
(nothing for four frames, there by twenty) at two of its frames to one of the
game's.

## The marks over characters

A character can have an object of marks over it (0x468EC, of the signs' layout
again: `tools_re/out/place_if.txt` from 0x468EC, `mark_chr.txt` for the
character's side). It is made for a character while one of its flags 0xB,
0x51, 0x59, 0x5A, 0x75 or 0xA5 is set (0x1074A4, which sets flag 0x52 while it
has one), and every character a behaviour script runs has 0x5A (0xF9B1C), so
every enemy that roams has one. It has two places: the layout's set 0, the
mark of having noticed her, and set 20, a small black tile with a red chevron
(object 43) that stands by it for as long as the character's flag 0xA5 is set,
which its int parameter 0x11 does (0x10A854). Much of what the object once did
is gone: seven of its functions are empty (0x46D60 to 0x46DB0), and what flags
0xB (a script's setting 4), 0x51 and 0x59 chose with them shows nothing.

**The mark of having noticed her** is a red glow of 46 by 46 with a yellow
exclamation mark in it (objects 1 and 2), which pops up to a fifth over its
size, stands, and is gone after 54 frames of the layout. A behaviour script's
function 8 has it shown (0xFAD98): the common bank's sound 0x2F, the mark
(0x108A68, 0x46C68: one that is up goes on as it is), and the character is
after her from then (flag 0x60).

**Where it is** is the character's to say each frame (0x109F4C, after all
the field shows of the mission has had its frame): the top of the character
(0x10731C: its model's node 4, 0xFD88C, if its resource and its flags have
that looked for; else 150 above where it stands, or the field's tunables'
+ 0x10 above it while its model is not ready) and 14 more; then by how far
that is in front of the camera, ten more at 450 and a tenth less for every 135
beyond (0x10A014). The camera's two matrices put that on the screen; behind the
camera the character's flag 0x23 is cleared and nothing is drawn. How big it
is goes by how tall 56 of the world's units are on the screen there: with 48
pixels it is its own size, with less it shrinks by two fifths of what is
missing, with more it grows by a quarter of what is over (0x10A2DC). It is
drawn (0x107748, 0x46B5C; the field draws the characters' marks first of what
it shows, while its flag 0x27 is set and 0x28 is not: `s104(2, ..)` closes and
opens that part, and it is open from the start, 0xED8DC) with that size as the
layout's inner matrix, about the first object of its set, half the glow's
height above the point: the glow's foot is at the point.

**The tile** (set 20) is one thing that does not move, twenty square, to the
left of the mark's point with its foot level with it (the glow's place in the
layout is its own too, so it is put and sized as the mark is). The object's
second place plays it: begun again both as the flag is set (0x46CD0: bit 0x20,
which keeps it) and as it is cleared (0x46D18: bit 0x40, which the object's
frame takes away as soon as the set is through, 0x46AE4, and a set with
nothing moving in it is through at once), so it is there for as long as the
flag is. Setting 122 is what sets it: a mission's script, for the enemy it is
after (`s52(521, 122, 1)` on the first map when mission variable 10004 is 10).

**In the port**: `Character::mark` (`show_mark`, `step_mark`), shown by the
behaviour scripts' `ALERT` in `field/symbol.rs` with its cry; the model's
node 4 is kept as `MarkNode` on the character's entity; `field/hud/mark.rs`
has the place, the size and the draw (`draw`, and `draw_tile` for the tile of
one whose flag 0xA5 is set), and `draw_marks` in `field/hud/mod.rs` goes over
the characters. The other object a character has with it (0x44B10) is a sign
like a trap's, which setting 92 gives it: `Character::sign`, drawn there too.

Against the walkthrough, an Eater noticing her at 656.0 s
(`docs/compare/notice_mark.png` is the port's): the mark is seen from 656.117
to 656.917, and the exclamation mark is out for the one frame at 656.317,
which is its track's alpha going to nothing at the layout's frame 18: two
frames of the layout to one of the game, 27 frames in all. With the Eater far
off the glow is 22 pixels across and the exclamation mark 14 high, seven
tenths of their size, its foot just over the Eater's back. Both textures are
made of two tones of alpha in a pattern of dots, which the original's
smoothing of what it draws small evens out; the port's canvas kept texels
whole and showed the dots, and now smooths what is drawn small as well
(`brs_engine/src/canvas.wgsl`).

## The shadows under characters

A character that is drawn (0x1067F8) has its shadow drawn first (0x10791C) while
its flag 0x16 is set and its flag 0x61 is not. Whatever makes a character sets
0x16 (0xE1954, 0xD6AB8, and 0x1116B0 for her); 0x61 is a script's to set, with
setting 77 at nought (0xC7BA8), which the maps do 472 times for the things a
scene stands about (`setting::SHADOW`).

The shadow is one picture, the first of the field's resident textures
(0x1100BC(0): `SYSTEM.VOL/fld_resident.bin/01.bin/00.ptm`, 32 by 32, white, its
alpha all there in the middle and falling away to the edge), drawn black on a
flat square from -1 to 1 each way (the four corners at 0x172A00), mixed over
what is there by the picture's alpha times a strength: 96 of 255 times the
character's + 0x1F0, which every frame of the character makes one (0xF5F68). The
square is as large as the character's number 0x2B (its + 0x174; 0x10A480 reads
it and 0x107ECC sets it), which is its own script's float `shadow_radius` as the
character is made ready (0x106AF0: 0x1096E0 with 4, 0x1028D0): sixty where the
script has none, so 120 across; 43 of the field's characters say.

Where it lies. Every frame of a character's model (0xFD24C, 0xFD3CC; not while
the field's flag 0x59 is set) a ray goes down the character's upright from 300
over where it is to 200 under (0xFD4B8, 0xFD4DC; 0x13E94 and 0x155E8 against
the map's collision). Where it is, for this, is where its model's node 0 is in
the world once the model is ready (0x107254, 0xFDDA4: so wherever its
animation has taken it), and its own place for one that is no plain model.
What the ray meets says what the ground is there (0x10730C,
for the steps' sounds) and, when it meets anything, where it met it is kept at
the character's + 0x1E0 and its flag 0x17 set. The shadow of a character with
that flag is at the kept place, not at the character's own (0x107B6C; one with
flag 0x65 has it at its own): so it stays on the ground under one that leaps or
hovers, and stays where ground was last found when there is none within reach.
It is 3 above that, with the three numbers at the character's + 0x440 added,
and 3 above that again; and it is turned from lying flat to which way is up
for the character (0x106CB8: its + 0x80, the lie of the ground its last move
found under it, 0x101A9C, or it was dropped on, 0x108694), by the turn about
the axis across the two unless they are all but the same (0x107A60).

In the port: `crate::shadow` (the picture and the square, which the ride on the
bike has too) and `field/shadows.rs` (`lie`: the ground under the model's node
0, `RootNode`, from 300 over to 200 under, kept on the shadow; the ground's own
lie there; `CharacterAsset::shadow_radius`). One whose frame of the field is turned has
it where it stands, lying as the frame does. The three numbers at + 0x440 are
settings 80 to 82 (`Character::shadow_offset`). Not ported: flag 0x65, which
nothing read so far sets.

## The map of the stage

Square on the field brings up a map of the stage (the task "ifFldMiniMap",
0x42D44 to 0x44614; `tools_re/out/mmap_task.txt`, `map_open.txt`,
`mmap_natives.txt`, and the layouts' dumps `mmap_frame_*.txt`,
`mmap_st1_*.txt`). Nothing of it is worked out from where she is: it is a
picture made of pieces, and what is shown of it is in the save, where the
maps' scripts put it.

**Its files** are `IF/EVT/MINI_MAP_STAGEnn.GZ`, one for each of the six stages
(0x42EA4 asks for it by the stage's number, the byte + 0xC of the map's
record; a stage past the sixth has no map and the task is over at once):

| file | what |
|---|---|
| 000 | a pack, the same in every stage: the frame's layout (objects, motions, groups) and its four textures: the panel's grid, the word CLOSE with the button, the words of the first stage's names, and a sheet of marks |
| 001 | a table of records of two shorts, one for each track of the first set of the map's layout: its kind and its number |
| 002 to 004 | the map's layout |
| 005 on | its textures: the pieces, the dotted ways, the stage's name |

The map's layout takes its textures in an order of its own for each stage
(0x43514 to 0x43BCC), with the frame's sheet of marks among them and, for the
first stage, the frame's sheet of words: stage 1 words, marks, 005, 006, 007;
stage 2 005, 006, 007, marks, 008; stage 3 marks, 007, 005, 006; stages 4 and
5 marks, 005, 006, 007; stage 6 005, 006, 007, 008, marks, 009.

**The frame's layout** has six sets, and the task plays five of them at once
(0x43270): set 0 as it comes and set 1 as it goes (the dark over all the
screen, black at 0x8C; the panel at (8, 7), 293 by 258, darker still, with its
grid and its lines; CLOSE and the button), six frames of the layout each; set
2, a pin; set 3, the mark of where the mission is (the word MISSION, a line and
a ring that swells and fades over 90 frames); set 5, another mark; and set 4,
seven squares of 50 by 50 that are never drawn. The squares are colours: each
frame the task takes, for each, the square's own colour by its track's
(0x43D98 to 0x43F30), and those seven are what the map's pieces are drawn in.
So the colour of the piece she is on can pulse, as its square's track does (30,
130, 255, its alpha from 204 to 77 and back over 60 frames).

**The map's layout** has two sets: its pieces, and the stage's name at the
right ("STAGE1 / SAN FRANCISCO"). A piece is a place (kind 0, and any kind
past 2), a way between two (1) or a name with its line (2); the first stage
has fourteen places, numbers 0 to 13, thirteen ways, 24 to 36, and three
names, 51 to 53 (G.G.GATE, TRAILER, CISCO PORTAL). Each is drawn alone, in
its colour by the frame's (0x441B0):

| piece | colour (the square) |
|---|---|
| its number below nought, or its flag 0x15F9 + number set | not drawn |
| a place whose number is one of the three she is on | 4: blue, pulsing |
| a place, its flag 0x157C + number set | 5: 180, 180, 180 at 204 |
| a place, the flag clear | 6: 80, 80, 80 at 204 |
| a way, the flag set, clear | 2: 180s at 204; 3: 80s at 204 |
| a name, the flag set, clear | 0: white; 1: 242, 110, 45 |

Then the stage's name; then the pins, four at most, each the frame's set 2
drawn with its first object at the pin's place (0x23F6C, 0x32288); then the
marks, five at most, each set 3 or set 5 by its kind. Everything but the
frame itself is drawn by the colour the frame's first track has come to, and
so comes and goes with it.

**What the save keeps** (set by the scripts' functions at 0xC6830 on, read as
the map is made, 0x432C0 on; the flags are read as it is drawn):

| what | where | set by |
|---|---|---|
| the pieces she is on | bytes 0, 1 and 2; 0xFF for none | `s270(which, piece)`, -1 for none |
| what is known | flags 0x157C + number | `s267(number, on)` |
| what is not to be seen | flags 0x15F9 + number | `s271(number, on)` |
| the pins | shorts 0x4FD3 to 0x4FDA, x and y each; 0xFFFF and 0xFFFF for none | `s268(pin, x, y)`, -1 and -1 for none |
| the marks | shorts 0x4FD1 and 0x4FD2, then 0x4FDB to 0x4FE2 | `s269(x, y)` the first, `s278(mark, x, y)` any |
| the marks' kinds | bytes 3 to 7 | `s279(mark, kind)`, which no script calls |

`s266()` clears it all: 0xFF for the three pieces, no kinds, nowhere for the
pins and the marks, and the flags from 0x157C to 0x176E. Every map's script
sets the map anew as the map is come into (`INIT_FIELD` calls
`SET_MMAP_STGnn(mission, piece, other piece, 0, 0)`, which is `s266` and then
what that mission has known by then): so the map is of the mission, and of the
map she is on.

**Its frames** (0x43BF0): it asks for its file, with the sign of the disc
being read up until it has come (0x11B310, 0x11B35C); it is made (0x43174) and
set 0 played with the common bank's sound 0; once that has played through,
square or circle (the pad's 0xA000) plays set 1 with sound 1; and when that
has played through it is over. The field's part (0xF32BC to 0xF3478, its
object at field + 0x2230): square, after the pause and the menu have had
their buttons (0x10DA90), has a picture taken of all that is on the screen,
which stands while the map is up (0xF3388: the picture shown whole and held,
field + 0x1E34 set, flag 0x17) and is what the map is drawn over (0xF326C;
the field's own interface is not drawn meanwhile, 0x10E61C); the task is made
when the picture has been taken (0xF3400); nothing of the field has a frame
while it is up (0x10D0F0, 0x108240); and when the task is over the picture is
let go and the field goes on (0xF30A4).

**In the port**: `field/map_screen.rs` has all of it (`MapAssets`, the six
stages' layouts and tables, loaded as the game starts; the functions the
scripts' `s266` to `s271`, `s278` and `s279` are; `MapScreen`, made by
`open` from the save and stepped and drawn; the resource `MapUp`), and
`field/mod.rs` the field's part: `control_player` opens it on square,
`run_map` gives it its frames, and the field's systems do not run while it is
up. It is drawn over the field as that stands, which is what the original's
picture is of. Against the walkthrough (the map of the second mission, 1141.9
to 1144.4 s, `docs/compare/map_screen_vs_walkthrough.png`): the same pieces
are light and dark, with the same colours to within a few parts in 255
wherever they were measured (a known place 147 against the port's 150, one not
known 70 against 68), the pins, the mark and the names are where the port has
them, and it comes and goes over three frames. Not ported: the sign of the
disc being read.

## The records (challenges)

What the game shows as CHALLENGES it calls records, and they are a script of its
data: `SYSTEM.VOL/rec_script.ss` (an `SSCR`; resource 0x1C of the system pack,
0x14B5E8). `tools_re/rec_table.py [--tables]` lists what it holds and
`tools_re/out/rec_script.txt` is `sscr_dec.py`'s reading of its code.

**The table** `lst_record`: 96 records of 60 bytes (`n_record` says how many; the
flags and variables have room for 128).

| at | what |
|----|------|
| 0 | its name ("First Challenge") |
| 4, 6 | the count that clears it, and the span of counts between tellings of how far it is |
| 8, 9 | its category (0 battles, 1 missions) and the stage it is of |
| 10 | flags: 1 open from the start; 2 opened by the first thing that would count; 4 put back by a retry (0x14AF74); 8 only counted on its own stage; 0x10 not shown as it opens; 0x20 every count told of |
| 12, 16, 20 | what is to be done, what kind of thing that is, what it is worth |
| 24, 36, 48 | three rewards of 12 bytes: kind (1 an item and how many, 2 a skill, 5 another record opened, 6 a flag set, 7 a piece of the gallery, 8 a skill made better, 9 words only), two numbers, and words of its own |

**What a record has come to** is kept by its number in the save: flag 6600 + n it
is open, 6800 + n cleared, 9000 + n its count has news, 9128 + n it has been
cleared and not shown so, 9256 + n its count has gone up; variable 20000 + n its
count. Two more flags are not of the save: 100000 + n it has been opened and not
told of, 100128 + n opened and not shown.

**The backup.** The system script has the save's backup made as a mission is
chosen (`mission_select`: its engine function 0x66, 0x2398; the game's start
makes the first, 0x97C) and put back as the field is left for a mission failed
(exits 3 and 5: function 0x67, 0x23C4). The backup keeps her figures and what
she carries as well, but all that is ever put back is the records: for each of
the 128, whether it is cleared and whether it is open (flags 9384 + n and
9512 + n) and its count (variable 20200 + n; 0x14AF08); and what is put back
(0x14AF74) is that of every record that is not cleared and has flag 4. So a
mission failed takes back what the attempt counted towards such a record, and a
record cleared in the attempt stays cleared. Function 0x6A goes with them
(0xECC70): with 1, as a mission is chosen, what there was to tell of every
record's count is forgotten (0x14AEB4: flags 9000, 9128 and 9256 + n); with 0,
at the field's end, something of her skills (0x14D73C); with 2 the records
would be told of a mission cleared (`rec_update_mis_finish`, which this script
does not have).

**The script's functions**, each run by name on a machine of its own with the
field's engine functions (0x14B6DC: the argument, and the address of a word that
says whether it is to print what it does, which is nought). Nothing is run while
flag 6156 is set, as it is through the story's opening.

| function | run | what it does |
|----------|-----|--------------|
| `rec_initialize` | as a game is started (0x13C; not held back by the flag) | opens the records with flag 1 |
| `rec_inc_counter(n)` | by a map's script (`s274`), and for what the functions below match | nothing if cleared or, with flag 8, off its stage; one that is not open is opened first if it has flag 2, else left; the count goes up by one; crossing a span is news (not a first count, unless flag 0x20); reaching the goal clears it |
| `rec_unlock(n)` | by a map's script (`s277`) | opens it, which is news, and to be shown unless flag 0x10 |
| `update_kill_enemy(event)` | as a hit of hers ends an enemy (0x9BDAC) | the kills' table: 45 entries of a record and a kind of enemy, and 9 of a record, a kind of enemy and a skill |
| `rec_update_btl_finish(event)` | as the results of a battle she has come out of are made (0x9BD28) | 13 entries of a record and a group; the first is the "First Challenge" for any group |
| `rec_update_level_up` | as the results give her a level (0x37F24) | one entry, record 94 "Level MAX": it counts if her level (her figure 1, 0x9C4) is not less than her figure 22, which is fifty whatever is asked (0xB58) |
| `rec_clear_reward(n)` | by the window that shows a record (0x14B92C) | shows it and, if it is cleared, gives what it is worth |

The engine asks for `rec_update_guard`, `rec_update_dodge`, `rec_update_overheat`
and `rec_update_mis_finish` as well (0x9BE40, 0x9BE7C, 0x9BEB8, 0x14A5DC), which
this script does not have: a function that is not found is not run.

An event (0x14ADC8) is five words: flags (bit 0 nothing took HP off her in the
battle, which is the battle root's flag 7 being clear; bit 2 the enemy's first
gauge was broken as the hit was done with, which it never is, mended as it falls;
bit 3 the group is one she strikes her pose over), a kind of enemy, an item, a
skill and a group, each -1 unless set. Kills are not told of in the first mission
(0x97380: variable 10004 is 0), and neither they nor a battle's end with the
battle system's value 86 set.

Two engine functions are the records' own: 0x5E says whether a record's count can
go up (0x14A5F8: in range, not cleared, on its stage if it has flag 8), and 0x5F
goes through a table (table, entry size, count, a function of the script, the
event): for each entry whose record's count can go up the script's function is
run with the entry and the event, on the same machine with its state put by, and
if it says yes `rec_inc_counter` is run for the entry's record (0x14A6E0).

**The panels** (`recMsgTask`, 0x14B51C, made as the game starts; its number is
0x700, over everything). The field asks every frame whether there is news of a
record (0xEDA84, in its interface's update), a battle every frame it is not
paused unless its value 86 is set (0x8D038), and the list of missions too
(0x5F284). The asking (0x14B080) does nothing while a panel is up (0x14B29C);
otherwise the first record with news (0x14AA04: one opened before one counted,
the lowest number first) gets its panel and that news is forgotten:

- opened (0x72D48): "CHALLENGE UNLOCKED" and the record's name, on blue;
- counted (0x721D8): "CHALLENGE", the name, how far the count is of its goal in
  hundredths (two figures, 0x72A30) and as a bar, which is drawn cut to that
  share of its width where it lies at rest (0x7297C);
- the same with the count at its goal (0x724FC): "CHALLENGE CLEARED" and
  "CLEAR", with other sets of the layout.

All three are `021.lpk` of the pack that is always there: one layout, sets 0 and
1 the coming and going of a count, 2 and 3 of a count at its goal, 4 and 5 of a
record opened; its seven textures are its own first two, three of the pack (7,
the sheet of small pictures 20, the figures 9), none, and its own third. Groups
10 and 24 are where the record's small picture goes (0x2D4B0), 11 and 25 where
its name is written (a line of 32 glyphs, no shadow; the opened panel has the
group twice, the first a pixel down and right in a dark colour), 6 the bar, 13
the figures. A panel comes (six frames of the interface), stays 120 and one
frame of the game more, is told to go the frame after (0x14B404) and goes in six;
the frame after it has gone another may come: 68 frames of the game in all, 66
of them drawn. As it comes sound 0x0D of the common bank is heard for a count
and 0x66 for a record opened. No button is waited for. What waits for a panel
to be gone: a battle's end (0x9DBB4, with the log) and a battle lost (0x94B00),
the results before they show the records' windows and before they can be
closed (0x37FD8, 0x38090), and a map's script that asks to with `s275`
(0xC6E38, which waits while there is news not yet told as well).

**The window** (`ifRecInfoTask`, 0x6F498 to 0x721D8; `IF/MENU/IF_REC_INFO.GZ`):
the screen headed CHALLENGES, with the sixteen records of a stage as two rows of
squares numbered down the columns. `record_window.rs` has its parts, groups and
states at its head. The script works it: `rec_clear_reward(n)` asks for it
(engine functions 0x6C and 0x6D, which make it and wait), sets variable 100000
to the record and has it shown (0x70 with 0) and waited for (0x71) until cross
is pressed; then, for a record that is cleared, gives each of the up to three
things it is worth and says so; then has it go (0x70 with 1) and waits. What is
said is said in the window a script puts up ("INFORMATION", the one `s252`
uses): function 0x63 with kind 0 and a text of the system script's
(`fld_pack.bin/09.bin/00.bin`: after its first word, the place of each text),
into which the words given beforehand are put where the text has code 0x1A and
two figures (0x12FBE8). The words are given by 0x74 (the script's own, 0xEC7C4)
or 0x1A (the name of a thing, 0xEC5F8: kind 1 an item, 2 a text of the same
table, 8 a skill, 10 a record). Every reward of this script has words of its
own, said with text 9, which is nothing but them; texts 7, 8 and 10 (still in
Japanese) are for rewards without, and none is. Kind 1 of 0x63 would say it in
the opened panel and kind 2 shows the window again; this script uses neither.

**Showing a list.** The results of a battle show the records of list 0
(0x14AAE4: opened and not shown, or of battles and cleared and not shown), as
many as it had when they were made (0x38758), each held back until its turn
(0x380E0, 0x38024) and fetched by its place in the list as it then is
(0x38004). They begin once all is counted and no panel is up, and only when the
last has gone can the results be closed. The results' own picture is not drawn
while a record is shown (0x37DDC). A map's script has list 1 shown (missions)
with `s276` (0xC6E9C): the field loads the window's file (0xF055C), shows each
record in turn, as many as the list has by then (0xEF870), and the script
waits. Either way the list is then forgotten (0x14ACDC). The maps' scripts
come in fours: `s277(n)` open, `s274(n)` count, `s275()` wait for the panels,
`s276()` show the windows.

In the port: `records.rs` runs the script itself, with the engine's functions,
the lists (`Shown` shows one, for the results and for the field) and the system
script's texts; `record_news.rs` is the panels, `record_window.rs` the window
and what is said over it. The battle tells the script of the enemies beaten
(`tell_records`) and of its end and a level gained (`conclude`), a map's script
of what it opens and counts. Seen in an unseen run of the first mission against
the walkthrough (`docs/compare/record_news.png`, `challenges.png`,
`record_reward.png`): the first battle after the opening unlocks "First
Challenge" (panel, then window at 25%), the second and third have the count's
panel (50%, 75%), the fourth the cleared panel, the window with its other
picture, and "Obtained basic skill Dodge"; from the fifth she can dodge, which
is flag 7500 (0x97490; the battle reads it each frame, `Rules::dodge_unlocked`).
The battle of the first mission's own opening counts for nothing: flag 6156 is
still set. A game started in a later mission with `--mission` has the first
record taken as cleared. A reward of kind 7 opens a group of the gallery's
things (`MENU_NOTES.md`, "GALLERY"). Of the three kinds of message (0x130490)
the window a script puts up (0) and the record's window shown again (2) are
ported; the panel (1) is not, and the records' script never asks for it. The backup is `Records::back_up`, `restore` and `forget_news`, which
the system script's host calls (`field/system.rs`).

## Where a new game starts

The game's own start (0x908) gives her **record 0 of the start table** (0x910:
0xE10 with 0, which is 0x14C9D4 on her figures), one each of items 0x85 and
0x81, sets variable 40 and puts her on **map 67** (`d_stg05_0a1`), entrance 0
(0x95C, 0x964), before `init_player`; choosing "new game" at the title then sets
the first mission as begun (0x11A1B4).

The start table (`005.xtc`, `BATTLE_NOTES.md`) has eight records. Record 0 is
999999 experience (level 50), 9999 HP, 999 attack, special attack and defence,
weapon 131, suit 135 and skills 2, 23, 12 and 15 in her slots: who the opening
shows, and who fights its lesson's battle against an enemy of level 99. Record 1
is a new game's: no experience, 372 HP, 29 attack, 41 special attack, 24 defence,
the Rock Cannon and the Normal Suit, nothing in her slots. Giving a record
(0x14C9D4) sets each figure the record has (below nought: left alone; records 2
and 3 have a suit and nothing else), changes to its weapon (0x14D0CC) and its
suit, gives her one of either if she has none (0x14D720, 0x14D594), sets the
flags 7000 + n and 7200 + n of the skills of its slots (0x14D95C, 0x14D994) and
works her level out of her experience (0x14D240). The opening's script ends, its
film over, with `s262(1); s263(); s109(0, 131, -1); s109(0, 135, -1); s109(0, 1,
3)`: record 1, her model made again of what she now has on, the opening's weapon
and suit taken away, three Repairers given. The walkthrough shows it: in the
lesson's battle her HP reads 9998 after one hit (88 s; `docs/compare/
prologue_battle_both.png`), and the first mission begins with 372. That hit is
the damage sum at work: the enemy there (group 1, `COF03` variant 10, level 99)
has 500 attack and its attack 900 is all of that less half her defence, 500 −
499, times a spread of five in a hundred either way: one or nothing.

That map's
`INIT_EVENT` sees a new game (mission 0, variable 220 at 100, no story event yet)
and plays `EV01_01_0100B`: the story so far in captions over staged shots ("In the
year 2032..."), the first lesson and its battle (`s81(1, 504, 0)`), the opening
film (`movie/BRS_OP_final.pmf`), and then goes on to the first mission's map by
its entrance 2 (`s68(0, 2)`), with her made a new game's as above. It sets flags 6156, 6169 and 7500; 7500 it clears
again itself, 6156 is cleared by the first mission's own opening (it is read at
0x14B704, by what announces a record reached), and 6169 stays until the first
mission is cleared.

In the port (`Starts`, `PlayerStats::start`, `field::run_title`): NEW GAME gives
her record 0 and nothing to carry, and `s262` does the rest; a run begun past
the title on the opening's map (`--map STG05_0A1`) has her so too, and one begun
on any other map has record 1 (`PlayerStats::new_game`).

In the port NEW GAME on the title starts there too (since 2026-10-08), and the
opening plays as the map's script has it: the captions over its ground
(`EV01_01_0100B_BG`) and its staged shots, the first lesson ("Battle Basics") and
its battle, and on to the first mission's map; only the film is passed over,
films not being played yet. A run that begins past the title on the first
mission's map (`--map STG01_01`, or no map named) has the opening taken as
played: `Progress::after_the_opening` sets the two flags it leaves. `--map
STG05_0A1` starts on the opening's map. Ported since (see "The lights"): `s166(light,
r, g, b, r, g, b)`, called for lights 0 and 1 with 75, 85, 100 twice, which by
its figures is the scene's lights.

## The first mission

Map `d_stg01_01`, scenario 0. For a new game `INIT_EVENT` plays the opening events
(`EV01_01_…`), in the middle of which is the game's first battle (group 2), then
`S1M1_ST_COMM` (which places the objects and the encounters) and the mission's start:
the player at (1425, 0, 5656) facing −132°, the camera behind her, variable 220 = 101.

Symbols 500-505 are Eaters (resource 21; groups 100 and 101), 521-523 Hornets
(resource 22, group 102, hidden while 220 < 112), 527-531 the trailer's guard (hidden
while 220 < 115). Each battle fought from a symbol adds one to 220 and to 239
(`MIS_SCE01_01_0100`): 102 a tutorial, 105 event trap 501 goes on, 107 the mission
says to go to the home point. Looking at home point 200 at 107 has the Hornets appear
(`MIS_SCE01_01_0050`) and sets 112; at 115 the guard appears and trap 502 goes on
(`MIS_SCE01_01_0200`); walking into 502 is the event battle against group 103
(`MIS_SCE01_01_0500`), after which 116 ends the mission (`MIS_SCE01_01_0300`:
`mission_clear`, then `s173(1)`).

The whole of it can be played without anyone at the keyboard: a scripted run can be
steered on the field (`seek500` runs her to character 500, `to-3500/260` to a place;
see `controls::Script::parse`), `--skip-battles` counts each battle as won and `--skip-messages` takes each window as read. The
sixth Eater stands behind a wall that has to be gone round by the south:

```
brs-game --map STG01_01 --mute --skip-battles --skip-messages --saves tools_re\out\_saves --script "wait:16,seek500:8,seek501:9,seek502:7,seek504:8,seek503:10,to-3500/260:2,to-4100/200:2,seek505:8,to-4100/200:3,to-3500/260:2,to-2000/200:3,to-700/900:3,to-200/2900:4,to100/3900:2,to450/4700:2,to600/5300:2,seek200:3,cross:0.3,wait:6,to600/5300:2,to450/4700:2,to100/3900:2,to-200/2900:3,seek521:6,seek522:8,seek523:8,to-200/2900:5,to100/3900:2,to450/4700:2,to1300/5500:3,wait:28,cross:0.3,wait:4,cross:0.3,wait:3,cross:0.3,wait:14" --screenshot shot.png --screenshot-at 158,166,171,175,180,188,193
```

The first wait is for the opening, whose scenes are skipped but whose fades and
first battle take their time. `seek200:3,cross:0.3` is the home point: she has to
have been by it for the eleven frames its sign takes to come before cross is
taken. The three presses of cross at the end are for the screen between
missions: the stage, the second mission, and "Yes". The pictures are the mission
cleared (158 s), the screen between missions (166, 171), its list with the
second mission open (175), the wipe the mission begins under (180), the map the
second mission opens with (188) and the second mission begun, "Go north to
destroy the Generator" (193): `docs/compare/first_mission_to_second.png`. The
mission itself is a test too, `the_first_mission_can_be_played_to_its_end`.

This run is what showed, on 2026-10-07, that the screen between missions was
black after a mission played to its end, though it was there and working: the
mission's script darkens the scene alone before it ends (`s197(1, 0, 30)`), the
port keeps the scene until the next one is made, and what the field draws of it
was still drawn, over the screen between missions. Nothing of the field is
drawn now while that screen or the title is up (`field_there` in
`field/hud/mod.rs`); a run that starts on the screen (`--stage-select`) had no
such leftovers and never showed it.

## The field's tables

`SYSTEM.VOL/fld_resident.bin`, slot 0, is an `SSCR`-format file whose symbols are data:

- `lst_map`: 115 records of 24 bytes: name (12 bytes, `d_stg01_01`), a byte for the
  stage it is of (from 0) and another, i16 battle map (101 → `STG01_01`), i16 the
  place the sign names it by (+ 0x10: 12, "THE END OF ANKH ST.", for the first map
  and for every map that has no name of its own), u16, pointer to the place's name
  as a save has it ("NONE" for those).
- `lst_fchr`: 183 records of 16 bytes: the resource names (`p_brs00`, `m_egg01`).
- `lst_mission`: 120 records of 0x74 bytes (0x11049C), 73 of them with a title: four
  bytes (the stage, from 0; then 0 on the missions of the story, 2 on the free hunts
  and 1 on the rest; a number from 0 to 13; nothing); at +4 the title, +8 the briefing,
  +0xC what is to be done, +0x10 a hint (all pointers to text); +0x14 a number;
  +0x18 the story event the mission starts from (variable 10003); +0x1C eight bytes,
  the missions that have to be cleared for it to open (0xFF ends them); +0x28 a flag
  that has to be set for it to open, or −1; +0x2C and +0x2E the map and the entrance
  it starts on; from +0x30 pointers to the lines that say what is to be done at each
  point of it (the display's panels pick among them); +0x6C and +0x70 two more
  numbers.
- `lst_pop`: 18 lists of the enemies that pop up, 16 bytes each (0x1104E8): eight
  pairs of a character resource and its share out of 100; resource 0xFF ends a
  list. List 4, which the first map's later missions use, is all Hornets.
- `prm`: 132 bytes of tunables. Beyond those named above: +0x4C what a guarded
  thing loses a second for each enemy near it (10), +0x50 how far she goes between
  pop-ups (3000.0), +0x54 the time between them (300 frames), +0x58 how many can be
  about (3), +0x5C and +0x60 how far ahead of her they come up and how far from
  that (1500.0, 300.0).
- `lst_fevt`: 710 records of 17 bytes: the names of the events (`FEVT_C_S1M1_AD06`).
- also `lst_mgrp`, `lst_gnd_se`, `lst_bgm`, `lst_mesh`, `dat_tbl`: not read.
