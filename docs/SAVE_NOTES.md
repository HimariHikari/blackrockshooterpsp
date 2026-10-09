# Saving and loading

What the executable has for the game's saves, as far as it has been read.
Addresses are of `EBOOT.OLD`. Listings: `tools_re/out/save_entry.txt` (0x4E00 to
0x5300), `save_tasks.txt` (0x5300 to 0x6680), `savedata.txt` (0xE100 to 0xF500),
`savelist.txt` (0xD260 to 0xE100). In the port (see the end) a game can be
saved at a terminal on the field and on the screen between missions, and
loaded there, from the title and after a mission failed; the options are
kept from run to run.

## What a player sees

The game has no screen of its own for it: it hands over to the PSP's own
savedata dialog (`sceUtilitySavedata`), which lists the saves with their icon,
title and details over the game's dimmed picture, asks, saves and says "Save
completed." (`docs/compare/save_walkthrough.jpg`, 1023 s of the walkthrough:
the list as SAVE is chosen on the screen between missions;
`save_done_walkthrough.jpg`, 1024.4 s). The game's own part is the texts it
gives the dialog and a few windows of its own before and after ("PopChoWin"
with the system's texts: `SYSTEM.VOL/msg_system.bin`, 63 texts; 5 "Game
saved.", 11 "Unable to read save data.", 12 "Cancel load?", 28 "Cancel
save?", …). A PC port has to draw the list itself.

## Where and what

| What | Value |
| --- | --- |
| Game's name for saves | `NPUH10126` |
| A save | directory `NPUH10126` + `DATA00` to `DATA19` (0xF7E0: twenty names, "DATA%.2d"), file `BRSS.BIN` |
| The system data | `NPUH10126SYS` + `DATA`, file `BRSC.BIN`: "System Data", "This is the System Data for BLACK★ROCK SHOOTER THE GAME." |
| The install data | `NPUH10126INS` (not for a PC) |
| The dialog's entry for a new save | "Create new Save Data" (0x165B88) |

The texts of a save (0xE698): its title is "BLACK★ROCK SHOOTER THE GAME"; its
second title "Level      %d" with her level (0xEC0: her figure 0x10); its
details "Play Time: %.2d:%.2d:%.2d %s" (hours, minutes, seconds by 0x278,
0x2B0, 0x2F8; then "cleared" if the game has been, 0x400, else nothing) and
under it "Save Point: STAGE %d:%s" (the stage of the map she is on, counted
from one, and the map's place name: the map's record + 0xC and + 0x14, by
0x1294 and 0x110368) when it was saved on the field (variable 300 is 0, 0x2400),
or "Save Point: STAGE SELECT" (variable 300 is 1).

## The block that is saved

One block of the system's (0x26C is the system, a pointer at 0x170C90): from
+ 0x209DC, **0x1498 bytes** (0x2550 copies it out, 0x24BC back in). As it is
copied out its first byte is set to 1 and its halfword at + 2 to 0x1498
(0x4EB4); a block read is taken only if they are so (0x4EF0, which looks at
the buffer + 0x10: the file has sixteen bytes before the block). Loading also
empties the table of what is loaded for her (0x14C910, 0x14C994 with the
system's + 0x20740).

| Block offset | System offset | Size | What |
| --- | --- | --- | --- |
| 0 | 0x209DC | 4 | 1, and the block's size at + 2 |
| 4 | 0x209E0 | 4 | the time played, in sixtieths of a second (an hour is 0x34BC0) |
| 8 | 0x209E4 | 1 | how many times the game has been cleared (0x400; 0x394 goes by it too) |
| 0xC | 0x209E8 | 0xB4E | the progress (0x2444; `FIELD_NOTES.md`): 32 words, the flags from + 0x80, the bytes from + 0x564, the halfwords from + 0x764 |
| 0xB5C | 0x21538 | 0x64 | her figures (0xDAC; read by number with 0x14CC00) |
| 0xBC0 | 0x2159C | 0x400 | what she carries (0x246C; 0x1B78 to 0x1C24) |
| 0xFC0 | 0x2199C | 0x70 | where she is (0x241C: the map by 0x1294, and more by 0x126C to 0x2068) |
| 0x1030 | 0x21A0C | 4 | the stage the screen between missions shows (0x2494; 0x2290, 0x22B8) |
| 0x1034 | 0x21A10 | 0x64 | the backup of her figures (0xDD4) |
| 0x1098 | 0x21A74 | 0x400 | the backup of what she carries (0x1C24) |

The block ends at the system's + 0x21E74, where the options' word is: that
word is of the system data, with the bit that says the game has been cleared
(its byte 3, bit 0: 0x394, 0x340).

Her figures (the block's + 0xB5C; 0x14CC00 reads one by its number through the
table at 0x16F2E8, 0x14CCF0 writes one): numbers 0 to 8 are words at + 0 to
+ 0x20 (0 the most HP, 1 attack, 2 defence, 3 special attack, 4 to 7 what is
added to those, 8 experience), 9 and 0xA words at + 0x28 and + 0x2C (the weapon
and the suit she has on), 0xB to 0xE at + 0x30 to + 0x3C (the skills in her
slots), 0xF at + 0x40 (HP), 0x10 the byte at + 0x44 (her level), 0x11 the byte
at + 0x45, 0x12 and 0x13 at + 0x48 and + 0x4C, 0x14 to 0x18 at + 0x50 to + 0x60
(0x15 to 0x18 are the totals 0x14CDE0 works out). The word at + 0x24 has no
number.

What she carries (+ 0xBC0): a byte for each item by its number; the low seven
bits are how many (0x1BA4 sets, 0x1BF4 reads), the top bit is something else.

Where she is (+ 0xFC0, 0x70 bytes; 0x1C68 clears it): + 0 the map (0x126C,
0x1294), + 4 the entrance (0x12B4, 0x12DC), + 9 a byte and + 0x1C to + 0x34
six floats that go together (0x1D64 keeps them, 0x1DC0 gives them back: a
camera's figures), + 0xA a halfword of bits (0x1E44), + 0xC to + 0x18 four
floats (0x1CA4, 0x1CFC: a place and a facing), + 0x20 a word (0x1E1C; the
field's value 0x26), + 0x4C a float (0x202C, 0x2068).

The time played is counted by a routine that runs at every one of the
screen's frames (0x2A80, set up at 0x2C50): one more in the word at 0x170CC0,
up to 0x14996C4 (99:59:59), unless the system's + 0x20044 is set; the game's
main loop copies that word into the block every frame (0x333C), the game's
own start clears it (0x160) and loading sets it from the block (0x2524).

## The tasks

| Task | Made by | Routine | What |
| --- | --- | --- | --- |
| "MS_DataSave" | 0x4F40 | 0x5330 | saving a game; 0x4F90 asks whether it is done (`0x1000`) and takes its answer (`0x1001`) |
| "MS_DataLoad" | 0x4FF8 (with 0 from the title, 1 from the screen between missions) | 0x5804 | loading one; 0x5048 likewise (`0x1002`, `0x1003`); 0x50FC is the answer kept, 1 when a game was loaded |
| "MS_DataDelete" | 0x5108 | 0x5F28 | |
| "MS_AutoSave", "MS_AutoLoad" | 0x51C0, 0x5278 | 0x6678, … | the system data, saved and loaded without asking |
| "MEMSAVE", "MEMLOAD", "MEMDELETE", "MEMGETUSERID", "MSGETFILELIST", "MII" | by the tasks above and by the title | 0xF498, 0xDAD0 | what talks to the PSP's dialog and memory stick |

## In the port

`crates/brs-game/src/save.rs`. A save is a file of the port's own,
`save/DATA00.sav` to `DATA19.sav` under the folder the game is run from
(`Saves`): `BRSSPORT`, the layout's number, and then parts, each a name of four
letters, its length and its bytes. `TIME` is the time played and the times
cleared; `PROG` the progress exactly as the block has it (0xB4E bytes,
`Progress::to_block`); `HUBS` the stage the screen between missions shows;
`ITEM` what she carries exactly as the block has it (0x400 bytes,
`Inventory::to_block`); `HERS` her figures as the port keeps them
(`PlayerStats`: they are not the block's hundred bytes, which hold her figures
before what she has on and her skills add to them); `PLAC` where it was saved
(on the field with its map and entrance, or on the screen between missions).
A part that is not known is passed over. A save is written beside its file
first and then put in its place. `About` gives the lines the original gives
the PSP's dialog ("Level      3", "Play Time: 00:15:18 ", "Save Point: STAGE
1:THE END OF ANKH ST.", as the walkthrough shows them). `Played` is the time
played and the times cleared.

**The list of saves** is `save_screen.rs`: the port's own screen in place of
the PSP's dialog, made to say what that says (`docs/compare/save.png` beside
`save_walkthrough.jpg`). Over a dark sheet: "Save" or "Load" at the top left;
the game's picture for a save (`PSP_GAME/ICON0.PNG`, which is beside the
game's data on the disc: `game/extracted`; a plain panel without it); the
title, the file's name and size, her level, the time played and the save
point; "Enter" and "Back" at the foot. Up and down go through the saves there
are (and, to save, the first that is free: "Create new Save Data", the
original's own words for it); cross saves at once in a new one, asks before
writing over one ("No" first), or loads; then "Save completed." or "Load
completed." until a button; circle goes back. It is drawn over everything
(layer 0x900) and has its frames from `SaveDesk`, a resource: whoever wants
it opens it and `field::after_saves` sees how it ended.

**Where it is used.** The screen between missions' SAVE and LOAD (the root's
states 0xB to 0xE: `Root::Saves`, `Hub::take_wanted`, `Hub::saves_closed`):
what is saved there is `Place::StageSelect`, with variable 300 at 1. The
title's LOAD GAME (state 0x13: `Title::take_wanted`, `Title::loaded`). A game
loaded takes the place of the one there was (`after_saves`: the progress, her
figures, what she carries, the time played; the system script let go), and
by where it was saved the screen between missions is made anew or the field
starts afresh on the save's map and entrance. The time played is counted two
sixtieths a frame of the game's logic while no title is up (`count_time`);
NEW GAME starts it from nothing. `--saves <folder>` names another folder than
`save`, which every run that is only a check should use.

**On the field.** A script asks for the list with `s178()` (0xC59B8: to save;
a terminal's does, after "Save Game Data?" is answered "Yes") or `s204()`
(0xC5A30: to load) and waits until it has gone: the field's + 0x220C has bit
0 set and + 0x2218 the kind (0x111420), a picture of the screen is kept, and
0x111464 says when it is over. In the port `Scene::saves` is that, `ask_saves`
puts the list up and `after_saves` lets the script go on. What is saved there
is `Place::Field` with the map, the entrance and where she stands, and
variable 300 at 0; a game loaded from it starts the field afresh on that map
and puts her where she stood once the map's script has set the map up. "LOAD"
of the window of a mission failed puts the list up the same way
(`Failed::take_load`, `load_closed`): the window waits, and asks again if no
game was loaded. With `--skip-messages` a script's list is not put up at all.

**The system data** (`System`, `save/SYSTEM.sav`): the three volumes, whether
captions go on of themselves, and whether the game has been cleared. It is
read as the game starts and written when the menus are left, when something
of the gallery is opened and after every save (the original writes it when
OPTIONS is left from the title, and after every save).

**The game cleared.** The last map's script says so with `s205(1)` (0x340): the
bit of the system's options word is set and the save's count of the times
(the block's + 8) becomes what it was given. Whether the game has been cleared
(0x394, which the title's ground and idle time go by) is that bit, or the count
not being nought, or variable 10001 being above nought. As the system data is
written its bit is made what 0x394 answers (0x264C), so a save that was
cleared marks the system data when it is next written; as it is read, the bit
it has is given to 0x340 if the game does not already count as cleared
(0x258C), which makes the count 1 too. In the port: `Scene::game_cleared` and
`field::mark_cleared` (the count in `Played`, the bit in `FieldState::cleared`),
and `Played::game_cleared` wherever the system data is written. Not ported: the
count being made 1 as the system data is read.

Checked by unseen runs (all with `--saves tools_re\out\_saves`): saved at the
first mission's terminal and on the screen between missions, each loaded from
the title; the list says of the first what the walkthrough's dialog says of
its save, "Save Point: STAGE 1:THE END OF ANKH ST.".

The list says when each save was written, after its number and before its
size, as the PSP's dialog does: the day and the time of its file by this
machine's clock (`clock.rs`: the system is asked what a moment is here, on
Windows by `FileTimeToSystemTime` and `SystemTimeToTzSpecificLocalTime`, the
port's only code that is not safe Rust; elsewhere the world's own time is
given).

A game saved on the field comes back with her facing as she faced and with
its camera as it was. The block's + 9 and its six floats are the camera's
record: each frame of the field's camera keeps the kind the script has it at
and the figures that go with the kind there (0xDCC28, 0x1D64: a place or a
point and three angles), and a field that is taken up again makes its camera
from what 0x1DC0 gives back (0xE8A90), as after a battle. The yaw she turned
the view to with the stick is one of those figures (the script is taken to
have set it so, 0xDF24C), so the view is the one she saved with. In the port
the record is a part of the save's file of its own (`VIEW`: the kind and nine
figures, the place, the angles and the point; `Camera::record` and
`restored`), which a save made before it was kept does not have.

Run end to end on 2026-10-09 (a game saved at the first map's terminal into a
folder of its own, then LOAD GAME from the title;
`docs/compare/field_save_loaded_port.png`): she stands at the terminal with
the view she saved with. Until then a game loaded from the title came back at
the mission's start: her place was given her in the frame of the load, on the
scene that was about to go, and not on the map started anew (`FieldState`'s
`opening` is now left alone while `fresh` is set).

Still to do: the block's own hundred bytes for her figures, which would let a
save of the PSP's be read.
