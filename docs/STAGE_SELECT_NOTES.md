# The stage select

What the executable has for the screen between missions, as far as it has been
read. Addresses are of `EBOOT.OLD`. The port has it in `crates/brs-game/src/hub/`
(see "In the port" at the end): the root, what is chosen from, the messages, the
interface with its script, the lists, the texts, the world, and her model in
its scene with its effect. Saving and loading are in `SAVE_NOTES.md`; the title is
`docs/TITLE_NOTES.md`.

## What it is

The hub a mission is left for: a world with the six stages on it (San
Francisco, New York City, Moscow, Fuji Jukai, Tokyo, the Moon), a root menu
(save, load, stage select, the MENU, a shop, mission select), the list of a
stage's missions with who each is from, and the messages that are due after a
mission (a window, the records opened or cleared). A script of its own plays
the comings and goings of its 3D scene.

## Its archive: `IF/MENU/IF_STAGE_SELECT.GZ` (46 files)

| Files | What |
| --- | --- |
| 000 to 029 `.ptm` | textures. 000 to 013 are the tiles of the world's map, 128 by 128 each; 016 is the Moon (256 by 192); 022 and 024 to 028 the pictures of the six stages; 023 a sheet of fourteen faces, each with "FROM" and a name under it (PHOBOS, MARION, ROTHCALL, BRS, NANA, XNFE, SAHA, UNKNOWN, XIAOMIN, MEFE, MZMA, SZZU, LLWO, ???): who a mission is from |
| 030 `.sdoa`, 031 `.xbn`, 032 `.bin` | the layout |
| 033 `.xtc` | the six stages: records of 16 bytes, a name ("SAN FRANCISCO", "NEW YORK CITY", "MOSCOW", "FUJI JUKAI", "TOKYO", "THE MOON", in full-width letters), a second text, and two numbers that look like a place on a map ((107, 203), (178, 201), (365, 165), (524, 210), (534, 205), (534, 205)) |
| 034 `.xtc` | what the strip says of the root's entries: "Save game data.", "Load game data.", "Select a stage.", "Open the Main Menu.", "Go to the shop.", "Select a mission." |
| 035 `.xtc` | messages: 0 "Begin Mission "%s"?", 1 "The challenge list is now available for Stage %d.", 2 "Yes", 3 "No", 4 to 9 the lesson about Free Hunt, 10 "Return to Title screen. Are you sure?", 11 "Unsaved data will be lost. Is this okay?" |
| 036 `.sscr` | a script (decompiled by `tools_re/sscr_dec.py IF/MENU/IF_STAGE_SELECT.GZ/036.sscr`): `open`, `close`, `close_m0305`, `to_mission`, `to_menu`, `next`, `prev`, and the tables `table`, `latitude`, `longitude`, `map_tex` and `local_str`. Each function is a run of the screen's own engine functions (`n01` to `n0b`, the table at 0x1711F8; see "The script" below): it is the script that plays the layout's sets, waits for them, and sounds |
| 037 `.efc` | an effect |
| 038 to 045 `.insa`, `.insm` | two models of 128 bytes (041, 044: no more than nodes) and six animations: what the scene's parts are given (her model stands in it, as in the MENU: 0x64028 with 7 hands the models of what she has on to it). Which file is what has not been made out |

## Its code

| Where | What |
| --- | --- |
| 0x5CEFC | makes the task "stage_select_boot" (`menuStageRootTaskMake`): a small task (0x5CF44: made by 0x5CFD4, unmade by 0x5D054, its frame 0x5DF34, which names "menuStageRootTask" and "_boot_update") |
| 0x5D104 | the root's own routine: made by 0x5D39C ("menuStageRootCreate", with "ifLoadingTask", "stage_select_message" and the files' task), unmade by 0x5D570, its frame 0x5E134 (396 instructions: the root's states), drawn by 0x5D760 |
| 0x5D1D4 | makes the MENU from here (`menuMainTask`, told `0x1004` with 3) |
| 0x5DAD4 to 0x5DD28, 0x5F204 | the task "stage_select_message": a window (0x6B5C8) and the records' panels (0x721D8); its frame 0x5F204 shows what is due after a mission (the records' windows through 0x14B92C, among others) |
| 0x5DDD4 | says "The challenge list is now available for Stage %d." |
| 0x5DE44 | asks "Begin Mission "%s"?" with "Yes" and "No" |
| 0x5E764, 0x5E918, 0x5EE64 | the frame of what is chosen from: 0x5E918 the root's entries, 0x5EE64 the stages (it is 0x5EE64 that sets a stage's flag, 0x180D on, the first time the cursor is on it: see `MENU_NOTES.md`, CHALLENGES) |
| 0x60A20 to 0x612C0 | what is listed: 0x60A84 and 0x60D98 fill tables from the missions (0x14A4F8 on) |
| 0x61B40, 0x6246C, 0x62590 | the screen's interface object: made (587 instructions), its frame, its drawing (412 instructions) |
| 0x62C00 to about 0x68600 | its parts: lists (0x65534, 0x6651C, 0x678CC), the scene (0x64028, 0x63BF8) |

### The boot task (0x5DF34)

Its object: + 0 the root's task, + 4 its state, + 8 what the root answered, + 0xC
a count, + 0x10 what follows. As it is made (0x5CFD4) the second tune of the
game's list, `system1`, is started (0x9F84 with 1) at full volume (0xA204): the
screen's own tune.

| State | What |
| --- | --- |
| 1 | makes the root ("menuStageRootTask", routine 0x5D104): 2 |
| 2 | asks the root what it has come to (0x5D0B8) until that is not nought: 3 |
| 3 | ends the root. Answered 2 (a mission was chosen): the music stops over 4 blocks (0xA10C), what follows is 1, and 4. Answered 3 (the title): the same with what follows 3. Answered 4 (a game was loaded): 0x2400, and by what that says the root is made again (1) or it is 4 with what follows 0 |
| 4 | four frames: 5 |
| 5 | by what follows: 0 = 0xE6CB0, 1 = 0xE6C64 (the field is started for the mission: "fldStartCtrlTaskStageSelect"), 3 = 0x7369C (the title) |

### The root's states (0x5E134; the table at 0x167718)

Its object: + 0x14 its state (0x5D81C sets it), + 0x18 what was chosen, + 0x1C
the archive, + 0x20 the save or load screen while one is up, + 0x24 flags
(0x100, 0x400), + 0x2C the interface object, + 0x30 the messages' task, + 0x38
the state that follows the loading, + 0x3C and + 0x40 the sign of loading and
the files' task, + 0x44 what loads her models, + 0x48 the file asked for,
+ 0x54 the frames the screen takes to go dark, + 0x58 set while the memory
stick is being complained about (0x79C, 0xEB88).

| State | What |
| --- | --- |
| 0, 1 | the screen comes over 30 frames (0x297E4 with 1); when it has: 2 |
| 2 | what she has on is asked for (0x5D824), and the archive (0x3C028): 3 |
| 3 | when the archive is there (0x3C1A4) and its resources (0x5D96C, 0x5D9F4): the messages' task is given its texts (0x5DF18: the archive's file 35), and 4, with 7 to follow |
| 4, 5, 6 | waits for her models (0x14BC90 with 0 and 1, and that what is loaded is what she has on: her figures 0xC and 0xB); then the scene is told 7 (0x64028) and the state is the one that was to follow |
| 7 | the interface is made (0x5D670): what is chosen from is the root's entries (+ 0x18 is 1), flags 8 and 0x10; the interface (0x61B40) is told 7 and 6 (her models, and the script's `open`) and its root cursor is put on the stage, the third entry (0x63BF8 with 0 and 2): 9 |
| 8, 9 | 8 has the screen come again over 30 frames. 9, once it has: the frame of what is chosen from (0x5E764); when + 0x18 is 9: 0x11 |
| 0xA | the MENU is made (0x5D1D4: "menuMainTask", number 0x402, told 0x1004 with 3; the menus' two ways out are given as 0x5D320 and 0x5D364): 0xF, then 0x10, which waits. 0x5D320 ends the MENU's task and has the root at 4 with 8 to follow: her models are waited for again, and the screen comes over 30 frames. 0x5D364 has it at 0x11 |
| 0xB, 0xC | loading: the screen for it is made (0x4FF8 with 1) and waited for (0x5048); if a game was loaded (0x50FC answers 1) flag 0x100 and 0x12, which leaves the hub for the boot task to start it over; else back to 9 |
| 0xD, 0xE | saving, with 0x4F40 and 0x4F90: back to 9 |
| 0x11 | the screen goes dark over + 0x54 frames: 0x12 |
| 0x12 | waits for that if flag 0x400 or 0x100: 0x13 |
| 0x13 to 0x16 | what was loaded is let go (0x3C40C, 0x3C4A8, 0x3B08C, 0x3B14C): 0x17 |
| 0x17 | done: the boot task takes its answer |

After its state's work, in states 1, 9, 0xA, 0xF and 0x11, the interface has
its frame (0x6246C), unless the messages' task is busy (0x5DDA8).

The root is task number 0x100 and the messages' 0x101: both are drawn under
everything the field has. While the root is there the game is drawn sixty
times a second (0x5D51C: 0x808 with nought, and what it was is put back as
the root goes), and her figures are worked out anew as it is made
(0x14D008).

**What the root draws (0x5D760).** In states 4, 5, 6, 8 and 0x10, while the
MENU is there and answers nought to 0x1002, the picture of the screen that
was kept (0x2A0E8); in every other state the interface (0x62590), and after
it, if flag 0x40 is set (cross on the MENU's entry sets it), the picture of
the screen is kept (0x29864) and the flag cleared. The script keeps a
picture the same way: `n01` with 5, 6 or 7 asks for one after the first,
second or third place is drawn, and a place whose word is 2 is drawn as
that picture. `to_mission` does it for the first place once its set is
through, so the world behind the list of missions stands still.

### What is chosen from (0x5E764; the root's + 0x18, the table at 0x1677C0)

Nothing of it runs while the messages' task is busy (0x5DD3C).

| At | What |
| --- | --- |
| 0 | 1 |
| 1 | the root's four entries (0x5E918), once the interface is ready (0x61ADC: the script's flag 1, which `n01(3)` sets and `n01(4)` clears): save, load, the stage, the MENU. **Circle** has the messages' task ask about the title (0x5DF24): 8. **Cross** by the entry the cursor is on: 0 saving (the root's state 0xD), 1 loading (0xB), 2 the stage's missions (if 0x61344 allows: the interface is told 0, and 2), 3 the MENU (flag 0x40, the root's state 0xA); each with sound 3. **Up and down** move the cursor, a step each time the button counts as pressed and round the ends only when it has newly gone down (0x63BF8 with 0), with sound 2. **Left and right**, on the stage's entry only, tell the interface 4 and 3: the stage before and the next, likewise round the ends only on a new press, with sound 2 if the stage is another |
| 2 | flag 8 cleared: 3 |
| 3 | the stage's missions (0x5EE64). First of all, if the stage's flag is not set (0x180D for the first stage to 0x1812 for the sixth) it is set, and for any stage but the first the messages' task says "The challenge list is now available for Stage %d." (0x5DDD4): so a stage counts as seen from when its missions are first shown. Then **cross**: if the mission the cursor is on can be begun (0x61640: its entry's flag 4), "Begin Mission?" is asked with its title (0x61878, 0x5DE44): 5. **Circle**: the interface is told 1, sound 5: 4. **Up and down** move the missions' cursor (0x63BF8 with 4), as the root's, with sound 2 |
| 4 | flag 8 set: 1 |
| 5 | the answer about the mission (0x5DDB4). "Yes": the scene is told 2, the mission is the interface's (0x6182C) and the messages' task begins it (0x5DD6C: the system script's `mission_select`): 7. "No": 3 |
| 7 | 0x5D6EC: once the interface is no longer busy (the script has done with `close`), 9, with no frames for the screen to go dark in: the script's wipe has covered it |
| 8 | the answer about the title. "Yes": 9, with 30 frames for the screen to go dark and flag 0x400. "No": 1 |
| 6, 9 | nothing; at 9 the root's state 9 goes on to 0x11 |

Before any of it (0x5E764): the first time the interface is ready, the
messages' task is started on what is due after a mission (0x5DD5C, and flag
0x10 gives way to 0x20); and when that task is idle again with flag 0x20
set, the interface is told 5: the stages and the missions are listed anew.

### The messages' task (0x5DAD4 to 0x5DF30, its frame 0x5F204; the table at 0x1677E8)

Its object: + 0 the task the system script runs on ("sysScrTask"), + 4 its
state, + 8 the records' panels, + 0xC to + 0x18 the records being shown,
+ 0x1C the window (0x6B5C8, the one the maps' scripts put up), + 0x20 set
while the interface is not to have its frame, + 0x24 its texts (file 35),
+ 0x28 and + 0x2C a page and a wait, + 0x30 which question. Drawn (0x5DBFC):
a sheet of 0x90000000 over the screen while + 0x20 is set, the records'
panels in states 5 to 7, and the window.

| State | What |
| --- | --- |
| 0 | idle (0x5DD3C answers so) |
| 1, 2 | the system script's `enter_stage_select` is run (0x130704 with 0): 3 |
| 3 | the records' news (0x14B080); when the script has run to its end: 5 |
| 4, 5 | how many records of missions there are to show is taken (0x14ABAC with 1), and + 0x20 is set if there are any: 6 |
| 6 | each of them in its turn is shown in its window (0x14B92C) and waited for; then 7, and + 0x20 cleared |
| 7 | that list is forgotten (0x14ACDC with 1): 8 |
| 8 | if flag 0x1CEE is set it is cleared, and after 45 frames (9) the six texts about Free Hunt are shown one after another in the window (0xA); then 0xB, which is idle again |
| 0xC, 0xD | "Return to Title screen. Are you sure?" with Yes and No, the cursor on No; on Yes, "Unsaved data will be lost. Is this okay?" the same way; then 0xE, which is idle again. The root reads the last answer |
| 0xF | a window that was put up is waited for: then idle |

0x5DDD4 says "The challenge list is now available for Stage %d." (the stage
counted from one) and 0x5DE44 asks "Begin Mission "%s"?" with Yes and No, the
cursor on Yes and circle for No; both leave it at 0xF. 0x5DD6C has the
system script run `mission_select` with the mission (0x130778).

The system script's `enter_stage_select` tells of each mission newly opened
with its engine functions 0x1A and 0x19: `n1a(0, 0, mission)` has the
mission's title be the first of the words a text can have put in (0xEC5F8),
and `n19(text, a, b)` (0x12FC08) shows a text of the system script's in the
window, with two answers if they are given, and waits for it.

### What the interface answers (0x6168C; the table at 0x1679F0)

0 the root's cursor; 1 how many entries the root has, which is 4; 2 the stage
shown; 3 how many stages there are; 4 the missions' cursor; 5 how many
missions the stage has. 0x61870 gives an entry's kind, which is its number.
0x6182C is the number of the mission the cursor is on and 0x61878 its title:
the stage's missions are entries pointed at from + 0x4FEC, each with its
number at + 4, flags at + 8 (4: it can be begun), its kind at + 0xC (which
colours its bar) and its title at + 0x18. 0x63BF8 sets one of
these figures (0 the root's cursor, 2 the stage shown, 4 the missions'
cursor) and has what goes with it follow.

### What is listed (0x60A20 to 0x612C0)

**The stages (0x60D98)**: six entries of 0x24 bytes, from the missions' table
(0x14A4F8 how many there are, 0x14A514 a mission's record: its byte 0 is its
stage and its byte 1 its kind, 0 for a mission of the story) and the stages'
texts (`033.xtc`).

| At | What |
| --- | --- |
| + 0 | the stage's number |
| + 4 | flags. The first stage has 1 and 2. Each later stage has them once its flag is set: 0x17F4 for the second, 0x17F6, 0x17F8, 0x17FA and 0x17FC for the sixth; and 4 as well ("new") the first time that is seen, which sets the flag after it (0x17F5, 0x17F7 and so on) |
| + 8 | its mark: nought while none of its open missions is cleared; 2 if as many are cleared as the stage has missions; 1 if as many of the story's are cleared as it has of the story |
| + 0xC, + 0x10 | its name and its second text |
| + 0x14, + 0x18 | the two numbers of its record in the texts' table (107 and 203 for San Francisco): what the world is given as figures 0 and 1 |
| + 0x1C, + 0x20 | how many of its missions are cleared, and how many are open (0x14A574, 0x14A530) |

0x60A20 counts the entries that have flag 1 one after another from the first:
how many stages there are.

**The missions (0x60A84)**: entries of 0xA8 bytes, one for each mission (120 at
most): + 0 its stage, + 4 its number, + 8 flags (5 if it is open, 0x14A530:
it is listed and can be begun), + 0xC its kind (the record's byte 1: 0 of
the story, 1 an extra, 2 a free hunt), + 0x10 the record's byte 2 (who it is
from: which face), + 0x14 what it has come to (1 if it has not been begun,
0x14A554, whose mark is "!"; 2 if it is cleared, 0x14A574, a star; and, for a
record that names flags at + 0x6C and + 0x70, 3 if the first is set, "?",
and 4 if the second is, a star in a ring), + 0x18 its title (the record's
+ 4), + 0x1C its briefing (+ 8), + 0x20 its hint (a copy of the text at the
record's + 0x10, 0x14A598), + 0xA0 what is to be done (+ 0xC), + 0xA4 the
record's + 0x14: the level it is meant for, or less than nought. A record
that is not a mission has 0xFF for its stage.

**A stage's missions are listed (0x639B0)**: of all the missions (filled in at
+ 0x12C by 0x60A84) those of the stage shown that have flag 1, in their order,
pointed at from + 0x4FEC, and how many at + 0x51E4. Once flag 0x180A is set
only mission 44 is listed, whatever else the stage has. The list's text 7 is
the script's second `local_str` with the stage's number (in full-width
figures) and its name put in. The first time a stage is listed the cursor is
put on its first mission that has come to 1 (+ 0x14), with the list scrolled
so that it is the sixth row at the lowest; after that the cursor and the
scroll are where they were left for the stage (+ 0x5530, + 0x5548, and
+ 0x5518 that they have been set).

**The rows shown (0x61398)**: six rows, from the one the list is scrolled to
(the missions' list's figure 2): each row's mission is kept at + 0x51CC, and
its text (the list's texts 0 to 5) is the mission's title, or for a row
past the last mission seven long marks ("ーーーーーーー", 0x167968).

**What is told of the cursor's mission (0x61484)**, as texts of the missions'
list (0x65574): 6 is a word for its kind, from the script's `local_str` (its
third, fourth and fifth strings for kinds 0, 1 and 2); 8 the record's + 8; 9
the record's + 0xC; 10 the text built at + 0x20; 11 the record's + 0x14 as a
number, if it is more than nought. The strip then says "Select a mission.".
With no mission under the cursor they are all empty.

0x61344 says whether the interface is busy (its state 0 or 3): the stage's
missions cannot be come to then.

**What a mission has come to** is its variable (100 and its number, 0x14A4DC):
0x14A530 says it is open when that is 2 or more (so one that is only newly
open, at 1, is not listed yet); 0x14A554 that it is under 3 (open and not
begun); 0x14A574 that it is cleared, at 201 or more.

**The strip's texts**: 0x60C40 gives the text of a root entry (`034.xtc` 0 to
3), 0x60CFC text 5 ("Select a mission."), 0x60D48 any text of a table by its
number.

### What follows a figure being set (0x63BF8, 0x63C00; the table at 0x167C50)

- **0, the root's cursor**: if it is another entry, the strip says that
  entry's text (0x60C40) from its start (0x67938).
- **2, the stage shown**: if it is another stage, the world is given the new
  stage's two numbers as its figures 2 and 3 and set turning to it (0x64D18);
  text 3 is the stage's second text; text 1 is the script's first `local_str`
  with the stage's number (one more than it is counted by) put in, in
  full-width figures, and text 2 the same of the stage it was; and the
  layout's texture 14 becomes the new stage's (+ 0x5574).
- **4, the missions' cursor** (only with flag 0x200000 of + 0x44): past either
  end it goes round to the other; the list is scrolled so that the cursor is
  among the six rows shown (to the cursor if that is above them, to five above
  it if below), never past the last six; and if the cursor is another mission
  the rows and what is told are made anew (0x61398, 0x61484).
- The others hand a number on: to her model (0x667CC with 2 to 7), to the
  world (0x64C18 with 0 and 1), or set and clear flag 0x100000 of + 0x44 (with
  which the 3D scene is not drawn).

### What the interface is told (0x64028; the table at 0x167C90)

Its own state (+ 0) is 1 at the root's entries, 2 at a stage's missions, 3
once a mission has been chosen and 4 when the script has done with that.

| Told | What |
| --- | --- |
| 0 | from 1: it is 2; the stage's missions are listed (0x639B0, 0x61398, 0x61484), and the script runs `to_mission` |
| 1 | from 2: it is 1; where the missions' cursor is and how far their list is scrolled is kept for the stage (+ 0x5530 and + 0x5548, a word a stage); the strip says the root entry's text again (0x60C40); the script runs `to_menu` |
| 2 | from 2: the script runs `close`, or `close_m0305` once flag 0x180A is set; the stage shown becomes the current one (0x2290); it is 3 |
| 3 | the next stage, round to the first, if flag 0x200000 of + 0x44 is set: the stage shown is set (0x63BF8 with 2), and if it is another the script runs `next` |
| 4 | the stage before, round to the last: likewise with `prev` |
| 5 | the stages and their missions are listed anew (0x60D98, 0x60A20, 0x67B98, 0x691E4 with 2) |
| 6 | the script runs `open` |
| 7 | the models of what she has on, once they are loaded (0x14BC90 with 0 and 1), are handed to her model in the scene (0x664B8, 0x66780, 0x66A0C) |

### The script (the object at + 0x53E8; 0x66C8C to 0x67764)

0x66D4C runs one of its functions by number: 0 `open`, 1 `close`, 2
`close_m0305`, 3 `to_mission`, 4 `to_menu`, 5 `next`, 6 `prev`. It is stepped
each frame (0x66D00), and 0x66E38 says whether one is still running. Its
engine functions (the object's + 4 is the interface, + 8 the world, + 0xC the
scene's camera and models, + 0x10 her model, + 0x14 the effect, + 0x18 the
list, + 0x1C the layout's player):

| Function | What |
| --- | --- |
| `n01(what)` | 0 the scene's 0x646F8 with 0; 1 the effect's 0x64984 with 0; 2 her model's 0x66A0C with 1; 3 sets and 4 clears the script's flag 1; 5, 6 and 7 set its flag 2 with + 0x84 at 0, 1 and 2 |
| `n02(which, value)` | a whole number given to a part: 0 and 1 the effect (0x64A00), 2 and 3 her model (0x667CC with 0 and 1), 6 and 7 the scene (0x64678), 8 and 9 the list (0x691E4 with 3 and 5), 10 the world (0x64C18 with 4), 11 to 13 the script's own + 0x78, + 0x7C and + 0x80, 14 the stage shown (0x63BF8 with 2) |
| `n03(which, value)` | a fraction given to a part (3 to 5 are kept within 0 and 1): 0 to 2 her model (0x664DC), 3 to 5 her model as that of 255 (0x667CC with 2, 3 and 4), 6 to 8 the effect (0x64A48), 9 the list (0x67888) |
| `n04(frames)` | waits so many frames: the script goes on in the frame the count comes to it, and at once for none |
| `n05(place, set)` | **has one of the layout's three places play a set** (0x319BC) |
| `n06(place)` | waits until that place has played its set through (0x31C20); not at all if it has |
| `n07(mode, a, b)` | starts the alpha of the panel down the right going: mode 1 has it come and 2 go, over `a` frames after `b` (0x67BA4; `tools_re/sscr_dec.py` prints it as `set_sys_value`, the field's name for a function of that number) |
| `n08()` | waits for her model (0x6669C) |
| `n09(which)` | answers: 14 the stage the cursor is on (the list's figure 1), 15 the list's + 0x58 (the stage it was on), 16 whether the script's flag 4 is set; nought for any other |
| `n0b(which)` | a sound of the bank that is always there: 0 is 0x11, 1 is 0x12, 2 is 0xA |

So `open` has places 0, 1 and 2 play sets 7, 8 and 1 and then 13, 14 and 0;
`close` 15, 16 and 2; `to_mission` 17 and 18 (and 21 unless the stage is the
Moon, stage 5); `to_menu` 19 and 20 (and 22); `next` 9 and 10 and `prev` 11
and 12, each with sets 5 and 6 and sound 0x12 where the Moon is come to or
left. The whole of it is in `tools_re/out/stage_select_script.txt`.

### The interface object, as far as its making has been read (0x61B40)

0x5590 bytes. An archive's file `n` is at the archive's + 0xC + 4n.

- + 0xC on: the archive's files 0 to 13, fourteen textures of 128 by 128 (what
  they are of has not been looked at).
- The layout (+ 0x54B0 on) is files 30 to 32, with twenty-one textures
  (+ 0x54BC on; the table at 0x167A58):

  | Texture | From |
  | --- | --- |
  | 0 | file 11 of the pack that is always there (0x2CDF4) |
  | 1 | the archive's 15 |
  | 2, 3, 4, 8, 18 | none |
  | 5 | the archive's 16 (the Moon) |
  | 6 | the archive's 17 |
  | 7 | file 12 of the pack that is always there (0x2CE28) |
  | 9 | the figures (0x2CD24) |
  | 10, 11, 12, 13, 14 | the archive's 18, 19, 20, 21, 22 |
  | 15 | the archive's 20 again |
  | 16 | file 10 of the pack that is always there (0x2CD58) |
  | 17 | the archive's 23 (the faces of who a mission is from) |
  | 19 | the archive's 14 |
  | 20 | the archive's 29 |

- + 0x5574 on: a texture for each stage, the archive's 22, 24, 25, 26, 27 and 28.
- + 0x5510: the layout's player, with three places (0x308A8).
- + 0x5560 and + 0x5564: the stages' texts (file 33) and the strip's (file 34).
- + 0x4C: what is listed (0x60D98 with 6 and the stages' texts).
- + 0x53E8: the script (0x66C8C, 0x66D2C with file 36; its `local_str` is
  looked up by name, 0x66E64, and kept at + 0x5568).
- + 0x51F0: her model in the scene (0x66098, 0x664A8, 0x667A4), with
  animations that are other files of the archive once flag 0x180A is set than
  before; + 0x5270 the scene's camera and models (0x643AC, 0x64764, 0x647EC),
  likewise.
- + 0x5324 (0x657D8, 0x655C8 with the layout's player) and + 0x5368
  (0x67BF8, 0x691E4 with 4 and 0x1E0): lists.
- + 0x545C: whether it is ready (0x61ADC reads it).
- + 0x52B8 (0x64A88; given the fourteen textures at + 0xC with 0x64C0C): the
  world. It is told figures by number (0x64C18): 5 and 6 the place of the
  layout's object 27 (group 14, at (269, 127)); 0 and 1 the two numbers of
  the stage shown (the list's + 0x14 and + 0x18); 7 the first word of the
  script's `table`; and it is given the script's `latitude`, `longitude` and
  `map_tex` tables with the next words of `table` (0x64D48, 0x64D54,
  0x64D60). See "The world" below.
- + 0x5470 (0x647FC, 0x64964 with the archive's file 37, the effect).
- The stage shown at first is the current one (0x22B8), kept within those
  there are (the list at + 0x5368: its figure 2 is how many stages, 1 the
  cursor's). The stages' own list entries are made with 0x677B8; the text of
  entry 1 is the script's `local_str` with the stage's number put in by
  0x618BC (which writes it in full-width figures: "%s%c").
- The six stage entries are 0x24 bytes each from + 0x4C: + 0 the stage's
  number, + 4 flags (bit 0 it is there, bit 2 it is new: then the interface's
  + 0x545C has bit 2, and the stage shown at first is put forward by one,
  0x2290), + 0x10 its name, + 0x14 and + 0x18 two figures.

Its frame (0x6246C): its own state (+ 0: 0 becomes 1 and flag 8 of + 0x44 is
set; 3 waits for the script to be done, 0x66E38, and is then 4), then the
frames of its parts (0x6446C the models, 0x66160 the animations, 0x64AFC the
world, 0x65518 and 0x67E38 the lists, 0x66D00 the script, 0x6485C the
effect), of the layout's three places, and two counters that go round (+ 0x556C
up to 500, + 0x5570 up to 270).

### Its parts, by where their code is

| Part | At | Code | Its routines made out so far |
| --- | --- | --- | --- |
| the scene's camera and models | + 0x5270 | 0x643AC to 0x647E8 | 0x643AC made, 0x64408 unmade, 0x6446C its frame, 0x64500 its camera set up, 0x64624 its drawing begun, 0x64678 a figure set, 0x646F8 an animation played, 0x64764 and 0x647EC a model and an animation given |
| the effect | + 0x5470 | 0x647FC to 0x64A7C | 0x647FC made, 0x64834 unmade, 0x6485C its frame, 0x6491C drawn, 0x64964 given its file, 0x64984 started, 0x64A00 and 0x64A48 figures set |
| the world | + 0x52B8 | 0x64A88 to 0x654C0 | 0x64A88 made, 0x64AFC its frame, 0x64C0C given its textures, 0x64C18 a figure set, 0x64CAC one read, 0x64D18 set moving, 0x64D48 to 0x64D60 the script's tables given, 0x64D70 and 0x64E0C drawn, 0x64D7C where a place of its map is on the screen |
| the missions' list | + 0x5324 | 0x654C4 to 0x6608C | 0x657D8 made (its text boxes), 0x654C4 unmade, 0x65518 its frame, 0x65534 a figure read (0 how many, 1 the cursor, 2 how far it is scrolled), 0x65574 a text set, 0x655C8 given the layout's player, 0x655D0 a text drawn with a track, 0x65ADC drawn (319 instructions), 0x65FD8 |
| her model | + 0x51F0 | 0x66098 to 0x66C88 | 0x66098 made, 0x660E8 unmade, 0x66160 its frame, 0x66318 drawn, 0x664A8 to 0x667CC figures and animations given, 0x66A0C built from the models of what she has on |
| the script | + 0x53E8 | 0x66C8C to 0x67764 | above |
| the texts and figures | + 0x5368 | 0x67764 to 0x69330 | 0x67BF8 made, 0x67764 unmade, 0x67E38 its frame, 0x677B8 a text set, 0x678CC a figure read, 0x691E4 a figure set, 0x67888 the strip's speed, 0x67938 the strip started over, 0x6798C given the layout's player, 0x67B98 given the stages, 0x67BA4 the panel's alpha set going, 0x67994 a text drawn at an object, 0x68000 and 0x6852C what it draws |

`tools_re/out/stage_objs_a.txt` (0x64500 to 0x66C8C), `stage_objs_b.txt`
(0x67800 to 0x69400) and `stage_script_obj.txt` are their code.

### What it draws (0x62590)

Each of the layout's three places has a word of its own (+ 0x5460, + 0x5464,
+ 0x5468): 1 has the place drawn track by track, by each track's group (the
table at 0x167AB0, for groups 2 to 78); 2 has 0x2A0E8 instead (not read); with
anything else nothing of the place is drawn. After a place, if flag 2 of
+ 0x545C is set and + 0x546C is that place, 0x29864 and the flag is cleared.
Last of all the script's 0x66D1C.

For every track the tint is white and the inner matrix none, and then by its
group:

| Group | What |
| --- | --- |
| 2 | not the layout's object but the world (+ 0x52B8), given the track's matrix and colour (0x64D70, 0x64E0C): its map is drawn as big as the track has the group, whatever the track moves it by |
| 3 | as it is (the Moon's picture) |
| 28, 29 | only while the root's cursor is on the stage (2); at an alpha of 0x4D if there are fewer than two stages or flag 0x200000 of + 0x44 is clear |
| 47 | the 3D scene, unless flag 0x100000: the models (0x64624), her model's animations (0x66318) and the effect (0x6491C), between 0x2DC74 and 0x2DAA8 |
| 22 | moved down 20 for each entry the root's cursor is past the first: the bar under the entry |
| 23 | only while the root's cursor is on the stage |
| 73, 74, 75 | only while the root's cursor is on SAVE, on LOAD and on the MENU |
| 50 | moved down 20 for each row the missions' cursor is past the first shown (the list's figure 1 less its figure 2) |
| 49 | a row's bar, drawn six times, 20 apart: tinted by the kind of the row's mission (white, 0xFF808080, 0xFFC0C0C0 for kinds 0, 1 and 2), or 0xFF404040 for a row with none |
| 56 | tinted by the kind of the cursor's mission, likewise (white with none) |
| 76 | the knob of the missions' list, only with seven missions or more: its height that of object 196 made `(119 / missions) * 6 / height` of itself, and moved down `(119 / missions) * scroll` |
| 77, 78 | only if the cursor's mission has a number to show (its + 0xA4 more than nought), and only if it has not |
| 10 | the stages' marks: see below |
| 65 | the mark of the fourth stage: see below |
| 30 to 45 | the panel down the right: all of it at the alpha the texts keep (+ 0x53D4, a part of 1); and groups 31, 33, 34 and 45 are cut (0x15B098) to the box of the layout's objects 71, 75, 77 and 81 while they are drawn |
| 14 | turned about its middle, once round against the clock for every 500 frames (+ 0x556C) |
| 15 | turned the other way, once round for every 270 frames (+ 0x5570) |
| the others | as they are, if the track is to be seen (0x31B48: flag 1 of its object's + 0x52 clear) |

**The stages' marks (groups 10 and 65)**. Group 10 has three tracks: the
first is for the stage shown, the second for the stage it was (the texts'
+ 0x58), the third for every other stage. For each stage there is that the
track is for, the world says where on the screen the stage is (0x64D7C, from
the two numbers of its record), and the track is drawn moved from where the
layout's object 22 is to there; then the stage's name is written with the
same move: box 4 for the first stage, 5 for the second, 6 for the third and 8
for the fifth, each at object 23 (0x6852C with 2, 3, 4 and 6). The fourth and
the sixth stages are left out. Group 65 does the same for the fourth stage
alone, from object 176, with its name in box 7 at object 177, which is set to
the right (0x6852C with 5): Fuji Jukai stands beside Tokyo, and has its mark
and name on the other side. The Moon has no mark.

**After a track is drawn, the texts that go with its group**, each given the
track's matrix (of which only how far it is moved is used) and its colour:

| Group | What |
| --- | --- |
| 20 | 0x6852C with 1: the stage's second text (box 3 at object 43); the stage's number as one figure of 20 by 20 from the layout's texture 9, at `(20 * number, 0)` with the first stage as 1, at object 44; and how many of its missions are cleared and how many are open, each as two figures of 5 by 5 from texture 12 (at `(5 * figure, 0)`), 6 apart, from object 45, the second 10 lower |
| 26 | 0x68000 with nought: box 1 (the word for the stage with its number) set to the right at object 60; and before it, if the stage has a mark (its entry's + 8), the mark at object 63: 15 by 15 from texture 13, at (1, 0x42) if every mission of the story is cleared and (0x11, 0x42) if every mission is |
| 70 | only while the root's cursor is on the stage. Its first track: 0x68000 with 1, which is box 1 centred, at object 186, or with a mark at object 187 and the mark at object 188. Its second: 0x6852C with 8, the same of the stage it was, from box 2 |
| 69 | 0x6852C with nought: the strip (box 0 at object 183, cut to that object's box). While the text scrolls it is written again and again, its width and the gap apart, from where it has got to until the box is full |
| 54 | the titles of the six rows but the cursor's (0x65ADC with the row) |
| 71 | the title of the cursor's row |
| 57 | the face of who the cursor's mission is from, at object 155 and of its size: from texture 17, where the faces stand five to a row, each the object's width and one more apart from 1, and a row its height apart, with one more for the second and third rows; which face is the mission's + 0x10. Then the list's texts 7 to 11 |
| 62 | the list's text 6 |

A text is written with no shadow (0x2D900), at where its object is, moved as
the track is, each cut to a whole number; its box is first given the object's
size; and its colour is the track's by the object's.

### The world (the object at + 0x52B8; 0x64A88 to 0x654C0)

Not a globe: a flat map of tiles with a grid over it, which is moved so that
the stage shown is at the middle of where it is drawn. 0x6C bytes: + 0 its
tiles' textures; + 8 flag 1 while it is moving; + 0xC and + 0x10 the place on
the map that is at the middle now (figures 0 and 1); + 0x18 and + 0x1C where
it is going (2 and 3); + 0x20 and + 0x24 where it came from; + 0x28 the
frames a move takes (figure 4: the script gives 15) and + 0x2C those it has
taken; + 0x30 and + 0x34 the middle on the screen (5 and 6: (269, 127));
+ 0x44 the colour of the grid (7); + 0x48 to + 0x60 the script's tables and
their counts; + 0x64 how big it is drawn; + 0x68 the layout's player.

The script's `table` is 0x1A000000, 9, 42, 5 and 3: the colour of the grid
(black, at an alpha of 26) and how many entries `latitude` and `longitude`
have and how many across and down `map_tex` is. `map_tex` says which tile is
where, five across and three down: 0 to 9, none, 10 to 13; so the map is 640
by 384 of its pixels. `latitude` is nine places down the map (83, 148, 189,
219, 246, 270, 294, 321, 351) and `longitude` forty-two across it (10 to 812,
24 apart but for two of 29, and -14 to -182).

- **Moved (0x64D18, 0x64AFC).** Setting the stage shown (0x63BF8 with 2)
  gives the stage's two numbers as where to go and starts a move from where
  it is; each frame of a move the place is so much of the way as the frames
  taken are of those a move takes, cut to whole pixels of the map.
- **Where a place of the map is on the screen (0x64D7C).** The middle, and
  how far the place is from the one at the middle, by how big the map is
  drawn.
- **Drawn (0x64E0C).** How big is the first of the numbers of the track's
  matrix (0x64D70 takes it): how much wider the track has its group. Each
  tile is drawn 128 by that across and down (cut to a whole number) from
  where it falls, in the colour of the layout's object 4 by the track's; a
  tile that is off the screen is not drawn. Then the grid, with no texture,
  in its colour by the track's: a line across the whole screen for each
  latitude, and one down it for each longitude.

### The texts and figures (the object at + 0x5368; 0x67764 to 0x69330)

0x80 bytes: ten boxes of text and the figures the rest of the screen goes by.

| Box | How it is written (0x67BF8, the table at 0x167EA0) | What |
| --- | --- | --- |
| 0 | the text font: letters 1 apart, 50 to a line, one line | the strip |
| 1, 2 | capitals of 10: letters 2 apart, 20 to a line, one line | the word for the stage with its number, of the stage shown and of the stage it was |
| 3 | capitals of 5: letters 1 apart, 20 to a line, three lines 1 apart | the second text of the stage shown |
| 4 to 9 | the same; box 7 set to the right | the second texts of the six stages: their names on the world |

Its figures (0x678CC reads, 0x691E4 sets): 0 the root's cursor (+ 0x4C, kept
within 0 and 3; + 0x50 what it was), 1 the stage shown (+ 0x54, kept within
those there are; + 0x58 what it was), 2 how many stages (+ 0x5C), 3 how long
the strip waits before it scrolls (+ 0x40; + 0x3C is what is left of it), 4
how wide the strip is (+ 0x48: 480), 5 the gap between one writing of the
strip's text and the next (+ 0x44). 0x67888 sets how fast it scrolls (+ 0x38,
always to the left).

**The strip.** Setting box 0 (0x677B8) measures the text: it starts at the
right of object 183's width if it fits (+ 0x30, + 0x2C where it is now), and
0x67938 has it scroll if it is wider than the strip (flag 1 of + 0x28) and
starts the wait over. Each frame (0x67E38) of a text that scrolls the wait
counts down, and once it is out the text moves on by its speed; when it is
further left than its own width and the gap, it is put that much to the
right. The script gives a wait of 60, a speed of 1 and a gap of 50.

**The panel's alpha** (+ 0x6C, at first 1): `n07(mode, a, b)` starts it going
(0x67BA4: + 0x70 the mode, + 0x74 `b`, + 0x7C `a + b`, + 0x78 a count from
nought). Each frame the count goes up to + 0x7C; mode 1 has the alpha at
`(count - b) / a` and mode 2 at one less that, kept within 0 and 1; and when
the count is there the mode is nought. `to_mission` has `n07(2, 10, 0)`: gone
over ten frames; `to_menu` has `n07(1, 10, 10)`: ten frames of nothing, then
there over ten.

### The missions' list (the object at + 0x5324; 0x654C4 to 0x66098)

0x44 bytes: twelve boxes of text, the stage's missions (+ 0x30, the pointers
at the interface's + 0x4FEC), the cursor (+ 0x34), how far it is scrolled
(+ 0x38), how many missions (+ 0x3C) and the layout's player (+ 0x40).
0x65534 reads a figure (0 how many, 1 the cursor, 2 the scroll) and 0x65FD8
sets one (the cursor within the missions, the scroll within all but the last
six).

| Box | How it is written (0x657D8, the table at 0x167CF8) | What | Drawn (0x65ADC, the table at 0x167D28) |
| --- | --- | --- | --- |
| 0 to 5 | the text font: letters no further apart, 50 to a line, one line | the titles of the six rows | with 0 to 5: at object 111, or 189 for the cursor's row, 20 lower for each row; at seven tenths of the alpha for a row past the last mission. Then the mark of what the row's mission has come to, 15 by 15 from texture 13 at object 110: (0x70, 0x52) for 1, (0x11, 0x42) for 2, (0x70, 0x72) for 3, (0x70, 0x62) for 4 |
| 6 | capitals of 10: letters 1 apart, 50 to a line, one line, centred | the word for the mission's kind | with 7: at object 120 |
| 7 | capitals of 10: letters 1 apart, 40 to a line, one line | the stage's number and name | with 6: at object 173 |
| 8 | the text font: 50 to a line, five lines 2 apart | the mission's record + 8 | with 8: at object 170 |
| 9 | the same, two lines | the record's + 0xC | with 10: at object 143 |
| 10 | the same, two lines | the text built at the entry's + 0x20 | with 9: at object 154 |
| 11 | capitals of 10: letters 2 apart, 20 to a line, one line, set to the right | the record's + 0x14 as a number | with 11: at object 198 |

The script's `local_str` is five strings: "ＳＴＡＧＥ %s" (box 1 and 2 of the
texts), "ＳＴＡＧＥ %s：%s" (box 7 of the list) and the words for the kinds,
"ＳＴＯＲＹ", "ＥＸＴＲＡ" and "ＨＵＮＴ". The stages' texts (`033.xtc`) are a
name in full-width letters ("ＳＡＮ　ＦＲＡＮＣＩＳＣＯ"), a second text in plain
ones with line breaks ("SAN\nFRANCISCO", "NEW\nYORK\nCITY", "FUJI\\NJUKAI")
and two numbers: where the stage is on the world's map, in its pixels.

### The scene (the layout's group 47; 0x643AC to 0x647E8, 0x66098 to 0x66C88)

What is drawn for group 47 is not of the layout: her model, seen by a camera
of the screen's own, and an effect.

**The effect (the object at + 0x5470; 0x647FC to 0x64A7C).** It has the
engine's object for an effect that plays (+ 4, made by 0x2C580), the scene's
camera (+ 8) and her model (+ 0xC), which of its files it plays (+ 0x10, which
0x64A00 keeps at nought whatever the script gives; the files from + 0x18, of
which there is the one, the archive's file 37), a node of her model (+ 0x14,
`n02(1, n)`) and three floats (+ 0x20, `n03(6 to 8, x)`). `n01(1)` starts it
(0x64984 with nought: 0x2C5E0 with the file, and its flag 1 is set). Each frame
while that flag is set (0x6485C) it is given the scene's camera (0x2C794), put
where that node of her model is (0x66620) by the matrix of the three floats as
a way from it (0x16205C, 0x2C79C), and has a frame (0x2C694); when it has played
out (0x2C824) the flag is cleared. It is drawn with the scene (0x6491C).

The script starts it once, in `close()`, as a mission has been chosen and the
screen goes: her second animation and the camera's are started, 110 frames
are waited, and then `n02(0, 0)`, `n02(1, 60)`, `n03(6 to 8, 0)`, `n01(1)` and
a sound: node 60 of her model is her left eye, and the effect is the blue
flame there as the camera has come up to her face. In the port:
`interface::SceneEffect` and `show_effect` in `hub/scene.rs`
(`docs/compare/stage_select_effect_port.png`); not yet looked at beside the
walkthrough.

**The camera (the object at + 0x5270).** Its rig is a camera file (`041.insm`:
one node and a lens, as the cuts of story scenes have theirs) and it has two
animations of that rig (0x647EC: `043.insa` and `042.insa`). `n02(6, which)`
says which of the two is to be played, `n02(7, flag)` whether it goes round
(0x64678), and `n01(0)` starts it (0x646F8). Each frame (0x6446C) the
animation goes on by a frame of sixty to the second; the view is the node's
place and turn (0x23360, 0x32A4C), and its angle from top to bottom comes
from the lens's angle from side to side by the shape of the screen, 480 by
272 (0x64510 on); it sees from 10 to 500.

**Her model (the object at + 0x51F0).** Made of the model of the suit she has
on, with the model of her weapon on the node it hangs on (0x50), when the
interface is told 7 (0x66A0C with nought); the cannon and the blade that are
of her model itself and two of the shapes of her right hand are hidden
(0x665B8 with 0x33, 0x32, 0x3D and 0x48). She has two animations (0x664A8:
`038.insa` and `039.insa`): `n02(2, which)` and `n02(3, flag)` say which and
whether it goes round (0x667CC with 0 and 1), `n01(2)` starts it (0x66A0C
with 1), and `n08()` waits until it is no longer playing (0x6669C). Three
angles turn her (`n03` with 0 to 2; the script leaves them at nought). She
is drawn in one light that is all ambient (0x66398): the colour `n03` with 3
to 5 gives it, 0xE5 of 0xFF each as the script has it.

**As the script has them.** `open`: the camera's first animation, going
round, and hers, going round. `close`: hers is let run to its end and waited
for, then her second is played once and the camera's second with it; 110
frames on the effect is started and sound 0xA played, and 60 frames after
that the wipe covers the screen. Once flag 0x180A is set the archive's other
files are used instead (0x61EC4, 0x61F98): `040.insa` for both of hers, and
the rig `044.insm` with `045.insa` for both of the camera's.

### What the archive's last files are (0x61B40)

| Files | What |
| --- | --- |
| 037 | the effect |
| 038, 039 | the two animations her model has here (slots 0 and 1); 040 is both once flag 0x180A is set |
| 041 | the scene's model (nodes: its camera), with the animations 043 and 042; 044 with 045 for both once flag 0x180A is set |

### The layout (`030.sdoa`: 203 objects, 79 groups, 25 sets)

`tools_re/out/stage_layout.txt` has all of it. In short: group 63 a sheet over
the whole screen and 64 the squares of a wipe (sets 0 to 2); groups 2 and 3
the two worlds at (269, 125) (the Earth's texture and the Moon's) with 12 to
15 the cursor's rings about them, and 10 and 65 marks (sets 3 to 6, 9, 11, 17,
19, 21 to 24: the comings and goings of a stage); the root's entries 24, 25,
26 and 27 (and 73, 74, 75), four lines 20 apart from (62, 82); 30 to 45 a
panel down the right with a gauge of ten bars; 48 to 62 the list of missions
and what is told of one (group 55 is its frame, of forty-odd objects; 57 and
58 the face of who it is from, 47 by 77 at (25, 160)); 66 to 69 the strip
along the foot; 16 to 23 lines and corners of the frame. Sets 7 and 8 are all
of each half at rest, 13 and 14 their coming, 16, 18 and 20 the list's.

The module that makes OPTIONS and the gallery for the title (`menuSingleBoot`,
0x5C798 to 0x5CEF8, with `menu_option.gz` and `menu_gallery.gz`) stands just
before it and is another thing: see `docs/TITLE_NOTES.md`.

## In the port

`crates/brs-game/src/hub/mod.rs` is the root with its states, what is chosen
from, and the messages' task; `hub/interface.rs` the interface: the stages and
missions as it lists them, the layout's three places, the texts and figures,
the list of missions, the world and the stages' marks, and the script, which
runs on the port's machine for such scripts with the engine functions above.
`field::leave_field` makes the screen when the field has been left and gives
it its frames, two to a frame of the game's logic; `--stage-select` begins a
run on it. The system script's `enter_stage_select` is run a frame at a time
(`SystemScript::begin`, `go_on`), and what it says of a mission newly opened is
put up in the screen's window.

Stand-ins and gaps:

- The files are there at once, so the root's states 2 to 6 are three frames
  and no more.
- A picture of the screen is not kept: while the script has the first place
  drawn as the picture, it is drawn as it was in the frame the picture was
  asked for, which comes to the same; and the MENU made from here comes up
  over the screen as it is for the frame the picture would show.
- The scene of group 47 is `hub/scene.rs`: her model and the camera, each
  kept to the frame the interface has its animation at. Not the effect, and
  not the light she is drawn in.
- SAVE and LOAD answer with the sound of "no". "Yes" to both questions
  about the title has the screen go dark, and the title comes (`title/`).
Compared with the walkthrough (`docs/compare/stage_select.png` and the
frames `stage_select_walkthrough.jpg`, 1019.5 s, and
`stage_select_missions_walkthrough.jpg`, 1034.5 s): the root and the list of
missions are as there, down to the blue of what the cursor is on, the
counts in the stage's picture and the long marks in the rows that have no
mission, and she stands where she does there, under the strip's band.

## What the system script has to do with it

`enter_stage_select` on coming up and `mission_select(n)` for the mission
chosen (see "Leaving the field, and the missions' list" in `FIELD_NOTES.md`),
through the screen's "sysScrTask".
