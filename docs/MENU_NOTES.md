# The menus

What the executable does for the menu that triangle brings up on the field (and
the stage select has too), as far as it has been read. Addresses are of
`EBOOT.OLD`. The MENU screen itself is ported (`crates/brs-game/src/menu/`), her
model in it too, and all four of its screens: CUSTOMIZE, ITEMS, CHALLENGES and
OPTIONS.

## The tasks

| Task | Made by | Its routine | What |
| --- | --- | --- | --- |
| `menuMainTask` | 0xF19B8 (from the field, then told `0x1004` with 1) and 0x5D1D4 (from the stage select's side, told `0x1004` with 3) | 0x59DCC | the MENU screen: four entries and her figures |
| `menuCustomTask` | 0x5A398, entry 0 | 0x4AFE0 | CUSTOMIZE (`IF/MENU/MENU_CUSTOM.GZ`) |
| `menuItemTask` | 0x5A398, entry 1 | 0x52DE0 | ITEMS (`MENU_ITEM.GZ`) |
| `menuLibraryTask` | 0x5A398, entry 2 | 0x56B50 | CHALLENGES (`MENU_LIBRARY.GZ`) |
| `menuOptionTask` | 0x5A398, entry 3; also 0x5CAE0 | 0x5AFCC | OPTIONS (`MENU_OPTION.GZ`) |
| `menuGalleryTask` | 0x4EC40 (made at 0x5CAE0) | | the gallery (`MENU_GALLERY.GZ`), from the title: see "GALLERY" |
| `menuBgTask` | 0x60958 | | what is behind a menu |
| `menuFadeTask` | 0x60898 | | |
| (access) | 0x608F8 `menuWorkCreateAccess` | | |
| `menuStageRootTask` | 0x5CEFC, 0x5DF34 | | the stage select (`IF/MENU/IF_STAGE_SELECT.GZ`) |
| `ifResFileMng` | 0x5A228 | 0x3BD44 | loads the four screens' archives for the main menu |

A task's routine is called with a message: 0 make, 2 unmake, 3 a frame, 4 draw,
and from 0x1000 up what others ask of it (0xB0C0 sends one).

## Where the data is

- The MENU screen itself is of the pack that is always there, `SYSTEM.VOL/if_rsdsys.lpk`
  (0x2CC94, its entry at + 0x58): `019.lpk`. In it `000.sdoa` (67 objects, 3 sets,
  16 clips) is the screen: the word MENU, the list CUSTOMIZE, ITEMS, CHALLENGES,
  OPTIONS, and a panel with her picture and LEVEL, EXP, NEXT, MAX HP, ATTACK,
  DEFENSE, SKILL. `010.sdoa` (26 objects, 2 sets) is what is behind it, a
  chequered ground with streaks, and a strip along the foot. `008.insa` and
  `009.insa` are two animations, and `015.xtc` the texts the strip has for the
  entries: "Check and set skills.", "Check and use items.", "Check all known
  challenges.", "Change in-game settings." and an empty one.
- The four screens' archives are named by the four pointers at 0x1711E8 and loaded
  as the menu is made (0x5A228), each with 0x5A0B8 to take its files in by their
  kind. `MENU_CUSTOM.GZ` has an animation (`000.insa`), texts (`001.xtc`), a layout
  (`002.sdoa`: 84 objects, 26 sets, 47 clips) with six textures of its own and one
  that is not; `MENU_ITEM.GZ` and `MENU_OPTION.GZ` are made the same way
  (`MENU_OPTION.GZ` begins with a sound).

## The MENU screen (`menuMainTask`, 0x59DCC)

Its object is 0xC0 bytes (0x59F64): the task at + 0xC, what 0x830 gave at + 0x10,
its state at + 0x14, what was asked of it at + 0x18 (1 at first; 2 and 4 leave),
flags at + 0x1C, the entry the cursor is on at + 0x20, the menus' common work at
+ 0x24 (whose + 4 and + 8, the object's + 0x28 and + 0x2C, are two tasks it is
told things through), the files' task at + 0x8C with the four archives' handles
from + 0x90 (eight bytes each), the panel at + 0xB0, the screen that is open at
+ 0xB4, and two counts of frames at + 0xB8 and + 0xBC.

Its states (0x5A9C0, by the table at 0x1674F0):

| State | What |
| --- | --- |
| 1 | the first: tells the task at + 0x2C `0x1001`; then 2 |
| 2 | sets the screen up (0x5A7DC: the ground behind from the pack's `010.sdoa`, the panel 0x5FAA4 with the cursor's entry, everything at no alpha, flag 8); tells + 0x28 `0x1000`; then 0 |
| 0 | a frame of the screen (0x5ACC4); when the screen's fade is over (0x29948) and + 0x28 answers `0x1004`: 3 |
| 3 | a frame of the screen; when leaving was asked for (+ 0x18 is 2 or 4): sound 1, + 0x28 is told `0x1001`, the screen's fade begins (0x297E4 with 2, 15 frames), and 8 |
| 4 | an entry was chosen: waits for + 0x28 to answer `0x1005`; then, if that entry's archive is loaded (0x5A550), makes its screen (0x5A398) and 6; while it is not, + 0x2C is told `0x1000` and + 0x28 `0x1006` with 1 (flag 0x20000 keeps that from being told twice, and both are taken back once it is) |
| 6 | the entry's screen has its turn; it ends by 0x5A5EC (back to this screen: 5) or 0x5A718 (out of the menus: 7), each of which ends its task |
| 5 | back from a screen (0x5A660): the panel and the ground at full alpha again; 3 |
| 8 | a frame of the screen; when the screen's fade is over: 7 |
| 7 | waits for what is being said (0x2D6E8) and for the files' task (0x3C4A8), lets the four archives go (0x5A2FC), 0x60814; 9 |
| 9 | waits for 0x60840; then 0xA |
| 0xA | done: the task answers `0x1000` with 1 |

A frame of the screen (0x5ACC4), in state 3 and once the files' task is not busy
and the panel is ready (0x5F918). The pad is the system's own (0x10B1C: + 4 what
has just gone down, + 0 what is held, in the PSP's bits):

- circle: leaving is asked for (+ 0x18 = 2); triangle the same with 4;
- cross: + 0x28 is told `0x1003`, sound 3, state 4;
- up and down move the cursor round the four entries (0x2E314 with 0x10, 0x40
  and 4: a step each time the button counts as pressed, by the pad's + 0x10, and
  round from the last to the first or back only when it has newly gone down):
  the panel is told the entry (0x5F910) and gives the strip its text for it
  (0x5F924), sound 2;
- L and R held (0x400 and 0x800: the pad's frame, 0x10BE4, makes those of the
  PSP's 0x100 and 0x200): 0x5FA28 with -1 and 1, which is added to the panel's
  + 0x44, the turn of her model.

The menu has the game drawn sixty times a second while it is up: as it is made
it keeps what 0x830 answers (the word at the system's + 4, which 0x850 goes by:
nought is one of the interface's frames to a frame of the game, anything else
two) and sets it to nought (0x808), and puts it back as it is unmade. So every
count here is in sixtieths.

Until the panel is ready a count runs (+ 0xBC), and after three frames sound 0x11
is played, once (flag 0x10000). While flag 8 is set the screen comes: a count of
frames (+ 0xB8) from 12 to 26 takes the alpha of the panel (0x5F9B8) and of the
ground behind (0x609B8, 0x609D4) from 0 to 255 (0x2DD70), and at 27 the flag is
cleared (0x609F0 with 1). Each frame ends with the panel's own (0x5F6A4).

It draws (0x5A8B8) the panel (0x5FE00) in states 0, 3, 4 and 8 only.

### The panel (0x5FAA4 makes it, 0x5F6A4 is its frame, 0x5FE00 draws it)

A pack's file is at the pack and the word at 0xC + 4 × its number. Of
`if_rsdsys.lpk/019.lpk` the panel takes: the layout (files 0, 1, 2), its
textures in this order: files 3, 4, 5, 6, then `if_rsdsys.lpk/009.ptm` (the
figures, 0x2CD24), then file 7, a place left empty, and `if_rsdsys.lpk/012.ptm`
(0x2CE28, which the three pieces down the right edge are drawn from); the
animations `008.insa` (0x5F850, set with 0x3B8C8) and `009.insa` (0x2CEE8, set
with 0x3B92C); and the texts `015.xtc` (+ 0x34), of which the one for the entry
the cursor is on is given to the strip along the foot (0x5F924: 0x2C54C,
0x4A8EC; entries 0 to 3, and 4 for none).

The panel's object: the pack at + 0, the layout's player at + 0x14 and the set
it plays at + 0x1C (0), flags at + 0x18 (4 at first; bit 0 = the layout's first
motion is over, which is what "ready" means, 0x5F918; bit 3 = it has been coming
for 22 frames, counted at + 0x24), the entry at + 0x20, the menus' work at + 0x28
and its model viewer at + 0x2C (the work's + 0xC), how the model is turned at
+ 0x40 (-1, 5 and -1 degrees about x, y and z, 0x5FB2C; the + 0x44 of it, the
turn about the upright, is what L and R add to: 0x5FA28), the HP
gauge at + 0x50 (the one of `if_rsdsys.lpk/013.sdoa`, 0x3C988, as on the field),
a camera at + 0x58, and her attack, defence and special attack at + 0x64, + 0x68
and + 0x6C (0x1008, 0x1030, 0x1058, after her figures have been worked out anew,
0x14D008).

Her model is shown in it: parts 0x33, 0x46, 0x32 and 0x3C off and 0x47 and 0x3D
on (0x3BB04), seen by a camera at (11, 148, 341) that looks at (1, 93, 16) with
(0, 1, 0) up, a lens of 21 degrees, from 100 to 1000 (0x32794, 0x32768). It is
drawn with the middle of the picture at (0x860, 0x804) of the PSP's own space
(0x15A8EC), which is 96 to the right of the middle of the screen and 4 below it.

**The model viewer** (0xA0 bytes; 0x3B404 makes it of a model, 0x3BCA0 gives it a
second one; the work makes it of what 0x6075C answers for 0 and for 1, which
goes by what she has on: herself, and what she holds). It keeps the two models
(+ 0x20, + 0x24), an animation for the first with another over it (+ 0x28 by
0x3B8C8, + 0x3C by 0x3B92C, which sets flag 2 of + 0x18) and one for the second
(+ 0x50), how far a frame takes them (+ 0x1C, set each frame by 0x3B5C0: 0x400
when the game is drawn sixty times a second, 0x800 at thirty), three turns in
degrees (+ 0x90, + 0x94, + 0x98, by 0x3BAA4), a light's way (+ 0x78: 0.853, 0.5,
-0.150) and three colours for it (+ 0x6C, + 0x70, + 0x74, by 0x3B9C4; at first
0xFFE5E5E5 and two blacks), and flags at + 0x14 (0x3BADC: bit 0 has it lit, bit
1 has the first animation stand still).

As it is drawn (0x3B7E4, after 0x3B624): the first model's own matrix is the
turn about z, then y, then x (0x24070, 0x24020, 0x23FD0); its animation goes on
by the step and is put on it, and then the one over it; the second model is put
where the first one's node 0x50 is (0x1E20C: the node her cannon hangs on), with
its own animation; and the step is spent. Lit, it has one light from that way
with those colours (0x15AB84, 0x15ABE8) before both models are drawn (0x22F64).

The panel has it lit (0x3BADC with 1), gives it the pack's `008.insa` and over
it `009.insa`, both to go round (0x5F850), and the colours 0xFFF0FFF5, 0 and 0
(0x3B9C4 at 0x5FCB8): all that reaches her is a light of (245, 255, 240) from
everywhere.

The frame (0x5F6A4): the layout's player goes on (0x30C8C); the HP gauge is told
her HP as a share of all of it and as a figure (0x5F764: 0xF1C over 0xFAC), and
steps; until 22 frames have gone by they are counted, and from then on the model
viewer steps (0x3B5C0); and when the track of group 13 of the set has played
its clip through (0x31CBC: at frame 52) the panel is ready.

Drawing (0x5FE00) goes through the tracks of the set, each by its group (the
table at 0x167830, for groups 6 to 54):

| Group | What |
| --- | --- |
| 6, 7, 48, 8 | the entries CUSTOMIZE, ITEMS, CHALLENGES, OPTIONS: of each group's tracks the first (grey) is drawn when the cursor is not on the entry, and the other (blue, twelve to the right) when it is |
| 13, 14, 15, 16 | the blue line under an entry: drawn only when the cursor is on entry 0, 1, 2, 3 |
| 23, 24, 25 | her attack, defence and special attack, as figures (digits of group 20, 21, 22) |
| 31 | her experience (0xE40; digits of group 29) |
| 32 | what is wanting to the next level (0xE9C less her experience, nought if there is no next; digits of group 30) |
| 38 | her level (0xEC0; digits of group 37) |
| 54 | all her HP (0xFAC; digits of group 53), set to the right |
| 50 | the bar of her experience: drawn whole when there is no next level, and otherwise only so much of its width as she has come of the way from where her level began (0x12B3D0) to the next (the scissor, 0x15B098) |
| 20, 21, 22, 29, 30, 37, 53 | not drawn of themselves |
| the others | drawn as they are |

A figure is drawn digit by digit: first the track of the group that goes under
digits (0x312D4: a faint square), then the track of its own group with the
texture moved to the digit (0x2D080 with the figures' texture, 0x15AE38), both
moved by a step that is the width of the digit's object and one more: to the
right from where it is for each digit after the first, or for all her HP so that
the last digit is where the object is.

Then, only once the panel has been coming for 22 frames: her model (0x3B7E4;
not if the panel's flag 2 is set), the HP gauge (0x3D114) at its place 2
(0x3D7DC: a pixel left of where its layout has it and ten higher), and the
strip along the foot (0x4A8D0).


### The menus' common work (0x68 bytes; 0x60620 to 0x60A20)

| At | What |
| --- | --- |
| + 0 | `menuBgTask` (0x60958; its routine 0x4A804, layer 0x401), made with the pack whose `010.sdoa` it shows |
| + 4 | `menuFadeTask` (0x60898; 0x4E734, layer 0x404) |
| + 8 | `ifLoadingTask` (0x608F8; 0x3AD74, layer 0x405): the sign that something is being loaded |
| + 0xC | the model viewer (0x606B8: 0x3B404 with what 0x6075C gives for 0, and 0x3BCA0 with what it gives for 1: what she has on, by 0x14BDC8 and 0x14BDEC) |
| + 0x10, + 0x14, + 0x18 | three words others set and read (0x609F0 to 0x60A18) |
| + 0x1C | flags; bit 0 = what is at + 0x20 has been started (0x607E0) |

The MENU screen's own layer is 0x402 (its maker hands 0x402 to 0xAE3C), so it
is drawn over the ground and under the wipe.

**The ground behind** (`menuBgTask`). Its layout is the pack's files 10, 11 and
12, with the textures: the pack's file 13, `if_rsdsys.lpk/012.ptm` (0x2CE28), the
pack's file 14, a place left empty and `if_rsdsys.lpk/010.ptm` (0x2CD58). Each
frame it plays sets 0 and 1 on (0x4AAF0); it draws set
0, the ground, in its first colour (+ 0x18: white at the alpha 0x4A908 sets),
and nothing when that alpha is nought (0x4AB38). Set 1, the strip along the
foot, is drawn by whoever calls 0x4A8D0 (the panel does, last of all), in its
second colour (+ 0x1C, by 0x4A924): every track as it is, but the one of group
22, in whose place the text that 0x4A8EC gave it is written (0x4AC08: at the
object's place moved as the track is, in the track's colour times the object's
times the second colour; the text is of the kind 0x2D788 makes with 0, 0x30, 6,
1 and 4).

**The wipe** (`menuFadeTask`). Its layout is `SYSTEM.VOL/if_fldsys.lpk` files 5,
6 and 7 (142 objects, 3 sets, no textures: squares of a chequer). What it is
told:

| Message | What |
| --- | --- |
| `0x1000` | plays set 0 (0x4EB38 with 1): it is "coming" (flag 1) |
| `0x1001` | plays set 1 (0x4EB80 with 1): it is "going" (flag 2) |
| `0x1002` | plays set 2, coming |
| `0x1003` | plays set 2, going |
| `0x1004` | answers whether it is neither coming nor going any more (0x4EBD4) |
| `0x1005` | answers whether the set has been playing for 14 frames or more (0x4EBFC: the count at its + 0x1C, which telling it a set puts at nought) |
| `0x1006` | 0x4EC18: with 1 it stands still (flag 0x40), with 0 it goes on |

A frame of it (0x4EA08) plays the set on while either flag is set, counts the
frame, and clears the flag when the set is through; it draws the set only while
a flag is set (0x4EAF0). So the MENU screen comes under set 0, leaves under set 1 (with the
screen's own fade), and goes to one of its four screens under set 2.

## CUSTOMIZE (`menuCustomTask`, 0x4AFE0 to 0x4E734)

The screen her skills are put in her four slots with, and those that are always
in force switched on and off. Its routine takes 0 to make (0x4B0EC, which makes
the object with 0x4BB98 and, like the MENU screen, keeps what 0x830 answers and
has the game drawn sixty times a second), 2 to unmake (0x4B1FC), 3 for a frame
(0x4B978), 4 to draw (0x4C1C8), and `0x1000`, which answers whether its state is
7: done. Its task is made on layer 0x403, over the MENU's and under the wipe's.

### Going to it and coming back

The wipe's third set (35 frames) covers the screen and uncovers it again: all
its squares are there from its twelfth frame to about its twentieth. The wipe
counts the frames a set has been playing (its + 0x1C), and what it answers to
`0x1005`, "has it done going", is whether that count is 14 or more (0x4EBFC);
to `0x1004`, whether it is neither coming nor going any more (0x4EBD4). Told
`0x1006` with 1 it stands still (flag 0x40) until told it with 0.

- On the MENU, cross: the wipe is told `0x1003` (the third set, going), sound 3,
  and the MENU's state is 4, in which its screen has no frame (the panel stands
  still) but is still drawn.
- When the wipe answers `0x1005`, 14 frames on, the entry's archive is asked for
  (0x5A550); if it is not loaded yet the loading sign is told `0x1000` and the
  wipe to stand still, until it is. Then the entry's task is made (0x5A398) and
  the MENU's state is 6: it does nothing and draws nothing.
- CUSTOMIZE begins in its state 0 with everything made, and is in its state 3,
  where it takes the pad, once the screen's fade is over and the wipe answers
  `0x1004`: when the set's other 21 frames are through.
- Circle leaves it for the MENU (its + 0x18 = 3, sound 5): the wipe is told
  `0x1003` again and its state is 6; when the wipe answers `0x1005` her figures
  are worked out anew (0x14D008), the MENU is told (0x5C738, which calls the
  0x5A5EC the MENU gave: its state 5, and the entry's task is ended; for OPTIONS
  it first asks the task `0x1003` and keeps the answer as its flag 0x80000000),
  and its state is 7.
- The MENU's state 5 (0x5A660) has the panel and the strip at full alpha, gives
  the model viewer its models anew (0x60708), and the panel its own again
  (0x5F7D4: the viewer's turns from the panel's, its two animations and parts,
  the strip's text for the entry, her figures; and the panel's "ready" is
  cleared, which its next frame sets again since its layout has long played
  through); then its state is 3. So the MENU takes the pad again in the second
  frame after, while the wipe is still uncovering it.
- Triangle leaves all the menus (its + 0x18 = 5, sound 1): the wipe is told
  `0x1001` and the screen's fade begins (0x297E4 with 2, 15 frames), state 6;
  when the fade is over her figures are worked out anew, the MENU is told
  (0x5C768, which calls 0x5A718: its state 7, the end of the menus), state 7.

### Its object (0x7F8 bytes and a table of its textures)

| At | What |
| --- | --- |
| + 0x10 | what 0x830 answered |
| + 0x14 | its state (0 at first) |
| + 0x18 | what it is at: 0 the list, 1 choosing a slot, 2 the question, 3 back to the MENU, 5 out of the menus |
| + 0x1C | flags: 0x80 her model is not drawn, 0x100 ready, 0x10000 its sound has been played |
| + 0x20, + 0x24 | the menus' work, and the archive |
| + 0x3C | the layout's player, with sixteen places |
| + 0x40 | the model viewer (the work's) |
| + 0x44, + 0x48 | the ground's task, and the texts (the archive's file 1) |
| + 0x4C | the window that asks (0x6B5C8, the one the maps' scripts have) |
| + 0x50 | a camera; + 0x54 and + 0x58 the middle of its picture, 0x881 and 0x7EB |
| + 0x5C | the text of what a skill does (0x2D788 with 0, 0x24, 2, 0, 0) |
| + 0x60 | six rows of 0x90 bytes: a text (0x2D788 with 0, 0x18, 1, 0, 0), what is written in it (+ 4), which slot the skill is in (+ 0x84: 1 circle, 2 cross, 3 triangle, 4 square, 0 none), whether it is in force (+ 0x88: 1 when it is not put aside), the skill (+ 0x8C) |
| + 0x3C0 | the four slots: a text like a row's, and the skill |
| + 0x3E0 | the list: 256 skills |
| + 0x7E0 | which list is shown: 0 those of a slot, 1 those always in force |
| + 0x7E4, + 0x7E8, + 0x7EC | the row the cursor is on (0 to 5), how far the list is scrolled, how many it has |
| + 0x7F0 | the slot the cursor is on |
| + 0x7F4 | a count of frames |

The four slots are, from the top, triangle, square, circle and cross: her stats
0xB, 0xE, 0xC and 0xD (0x14CC00 reads one, 0x14CCF0 writes it).

### How it is made (0x4BB98)

- The layout is the archive's files 2, 3 and 4, with the textures: files 5, 6,
  7 and 8, `if_rsdsys.lpk/020.ptm` (0x2CE5C: the small pictures), files 9 and
  10, two places left empty, and `if_rsdsys.lpk/012.ptm` (0x2CE28).
- The model viewer is lit (0x3BADC), given the archive's file 0 as its
  animation, not to go round (0x3B8C8 with 0) and put at its end (0x3B9A8 with
  the animation's + 0x10: it is two frames, a pose), and over it the
  `009.insa` the MENU has (0x2CEE8), going round; the same parts as on the
  MENU (0x33, 0x46, 0x32, 0x3C off; 0x47, 0x3D on); turned 14, 10 and 8
  degrees about x, y and z (0x3BAA4).
- The strip's alpha is 255 (0x609D4).
- The camera is at (-37, 124, 119) and looks at (-19, 91, 3) with (0, 1, 0)
  up, a lens of 65 degrees, from 10 to 500 (0x32794, 0x32768). The middle of
  its picture, (0x881, 0x7EB), is 129 to the right of the middle of the screen
  and 21 above it.
- The player's places are given their sets (0x30710): place 0 set 0, 1 set 2,
  2 set 0xB, 3 to 6 set 0x15, 7 set 0x17, 8 and 9 set 0xD, 0xA set 3, 0xB set 4,
  0xC set 5, 0xD set 6, 0xE set 7, 0xF set 8.
- The text of what a skill does is made with its pen the colour of the object
  of group 21 at full alpha (0x26E30: black); nothing that is written in it
  changes that, so it is written in black. The other texts are white.
- The first list is built (0x4B740 with 0), the rows are filled in (0x4E248),
  the window is made, and the slots are given what her stats have (0x4E4F0 for
  each).

### The lists (0x4B740, 0x4B820)

A list is every skill of the names' table (`db_resident.lpk/003.xtc`, from the
first for as many as it has) whose kind there (0x12CDBC: bits 16 to 18 of the
record's third word) is 1 for the first list and 2 for the second, that she
has (0x14D940: flag 7000 and its number) and that is to be listed (0x12CD5C:
bit 19), sorted (0x14E828) by its place in the order (0x12CD04: the low half of
that word). The cursor's row is kept if the list has a skill for it, and is
otherwise the last that has one; the list is shown from its top.

In the game's table the skills of a slot are 1 to 29 and 66 to 71 (the better
ones of 23 to 28 come after each: Aggressor, Aggressor +, Skill Boost, ...),
those always in force 30 to 65; the pictures (0x12CE18: bits 21 to 25) are 1
for one that attacks, 2 for one that helps, 6 for one that acts of itself and 4
for one that adds to a figure.

### A frame (0x4B2EC)

In state 3: if it is ready, what the pad does (0x4DC50). If it is not, it is
ready once place 0xA has played its set through (0x31C20: 54 frames after it
was made), and meanwhile, if the list has anything, the count at + 0x7F4 runs
to 20 and then sound 0x11 is played, once. In every state the sixteen places
are played on (0x30C8C), the window has its frame while it is not closed
(0x6BDE0, 0x6BA60), and the model viewer steps unless the screen is being left.

### The pad (0x4DC50)

**At the list** (+ 0x18 = 0), the first of these that applies:

- triangle: out of the menus (5), sound 1;
- up and down (0x4B454): the cursor goes through the six rows and then scrolls
  the list, a step each time the button counts, and from either end to the
  other only when it has newly gone down (to the last row with a skill, the
  list scrolled to its end; or to the first, unscrolled); sound 2 if it is
  somewhere else than it was; the rows are filled in anew;
- L and R (0x2E314 with 0x400, 0x800 and two values): the other list. Place 1
  plays set 0xA for R and 9 for L (the arrow by the button nudges), then 0x4B5EC
  gives the places their sets for the list (the first list: 2 set 0xB, 3 to 6
  set 0x15, 7 set 0x17, 8 and 9 set 0xD; the second: 2 set 0xC, 3 to 6 set
  0x16, 7 set 0x18, 8 and 9 set 0xE) and builds it; the rows are filled in;
  sound 2;
- circle: back to the MENU (3), sound 5;
- cross, on the second list: the flag that puts the cursor's skill aside
  (0x14D978, 0x14D994: flag 7200 and its number) is turned over, and the row's
  + 0x88 with it; sound 3. (Nothing asks whether the list has anything: with
  an empty list it is the flag of skill nought that is turned over.)
- cross, on the first list, if it has anything: if the cursor's skill is in a
  slot the window asks (0x6BAFC with the archive's text 0, "Remove skill
  “%s”?", with the skill's name in it by 0x14E50C, and the choices of texts 6
  and 7, "Yes" and "No"; the cursor on "No", and circle goes to "No": 0x6BC94
  and 0x6BC84 with 1; opened with 0x6BE38), and + 0x18 is 2. If it is not,
  place 8 plays set 0xF, place 0xA set 0x11 and, if any other row has a skill,
  place 0xB set 0x12; + 0x18 is 1; sound 3.

**Choosing a slot** (1):

- cross: the slot the cursor is on is given the skill (0x4E4F0), the rows are
  filled in, sound 3, back to the list;
- circle: back to the list, sound 5;
- up and down move the slots' cursor round the four (0x2E314), sound 2.

As it goes back to the list either way, place 8 plays set 0x10, the place of
the slot the cursor is on (3 and its number) set 0x19, place 0xA set 0x13 and
place 0xB set 0x14.

**The question** (2): once the window has closed, if its cursor was on "Yes"
(0x6BCA8 answers 0) the slot that has the cursor's skill is emptied (0x4E4F0
with no skill) and the rows are filled in; back to the list.

**Giving a slot a skill** (0x4E4F0): the slot's text is the skill's name, or
seven dashes (the string at 0x166E60) for none, and her stat for the slot is
written; any other slot that had the same skill is emptied the same way.

**Filling the rows in** (0x4E248): each row's text is its skill's name
(0x12CC4C, through the "%s" at 0x166E5C) or the seven dashes if the list has
no skill for it; its + 0x88 is whether the skill is not put aside, its + 0x84
which slot has it. The text of what a skill does is that of the cursor's
(0x12CC90), or nothing. The strip is given the archive's text 8 for the first
list ("Set/Remove", after the picture of cross) and 9 for the second ("Switch
on/off").

### What it draws (0x4C1C8)

The layout has two matrices, of which 0x32174 sets the one that moves an
object's corners before its track does; "moved down" below is by that. A text
is drawn at its group's first object's place, moved as its track and that
matrix have it (each cut to a whole number), in the track's colour times the
object's.

1. Place 0, whole: the dark frame down the right.
2. Her model (unless flag 0x80), by the camera, with the picture's middle
   where the object has it.
3. Place 1, track by track, by its group (the table at 0x166E98, for groups 18
   to 44):

   | Group | What |
   | --- | --- |
   | 41 | the picture of the first list's tab, dark: only while the second list is shown |
   | 38, 42 | the first list's title, and its tab lit: only while it is shown |
   | 43 | the second list's tab, dark: only while it is not shown |
   | 29, 44 | the second list's title, and its tab lit: only while it is shown |
   | 21 | the text of what the skill does: in white at the alpha of the track's times the object's, which with the black it is written in is black at six tenths |
   | 18 | the scroll bar: only when the list has seven or more |
   | 19 | its knob, likewise: with the bar's height less 2 over the number in the list as a step, it is moved down by a step for each the list is scrolled, and made six steps high (scaled by that over its own height) |
   | the others | as they are: the panel, the title CUSTOMIZE, L and R, the three pieces down the right |

4. The six rows, each 20 lower than the one before. A row with no skill is
   drawn with place 0xC (first list) or 0xF (second): the tracks of group 24
   as the row's text, the others as they are. A row with a skill of the first
   list is drawn with place 0xA if the cursor is on it and 0xB if not (the
   table at 0x166F08, for groups 24 to 37):

   | Group | What |
   | --- | --- |
   | 24 | the row's text |
   | 32 | "SET": only if the skill is in a slot |
   | 33, 34, 35, 36 | the picture of circle, cross, triangle, square: only if that is the slot it is in |
   | 37 | the skill's picture (0x2D320) |
   | the others | as they are (30 and 31 are the row's ground) |

   and one of the second list with place 0xD if the cursor is on it and 0xE if
   not:

   | Group | What |
   | --- | --- |
   | 24 | the row's text |
   | 25 | the skill's picture |
   | 26 | the word OFF: the first of its two tracks, which has it lit, when the skill is put aside, and the second, dim, when it is in force |
   | 27 | the word ON: the first when it is in force, the second when it is put aside |
   | the others | as they are (22 and 23 are the row's ground, 28 the picture of cross) |

5. Place 2, whole: the ground of the slots.
6. The four slots: for each, its own place (3 and its number), whole, moved
   down 18 for each slot before it (the bar behind it); then place 8 if the
   slots' cursor is on it and 9 if not, not moved at all, of which only these
   tracks: of groups 59, 60, 61 and 62 (the pictures of triangle, square,
   circle and cross) the one that is the slot's; of group 58 the track whose
   number among that group's (0x31BE4) is the slot's, as the slot's text (the
   four tracks of the group are 18 apart themselves).
7. Place 7, whole.
8. The window (0x6C0F8), and the strip along the foot (0x4A8D0).

**A skill's picture** (0x2D320): by the kind the names' table has for it, 1 to
7, picture 0xA, 0x17, 0xB, 0x1D, 0x3A, 0xB or 0xB of the sheet of small
pictures (`if_rsdsys.lpk/020.ptm`: 15 by 15 in cells of 16, four to a row, a
pixel in), at the group's place in the track's colour.

### The layout (`MENU_CUSTOM.GZ/002.sdoa`: 84 objects, 68 groups, 26 sets)

| Set | Groups | What |
| --- | --- | --- |
| 0, 1 | 0 to 14 | the dark frame down the right (1 is not used) |
| 2 | 15 to 21, 29, 38 to 54, 65 to 67 | the screen itself: 44 frames |
| 3 | 30, 31, 24, 37, 32 to 36 | a row of the first list as the screen comes (54 frames): place 0xA's at first, whose end is "ready" |
| 4 | the same | a row of the first list at rest |
| 5 | 30, 31, 24, 63 | a row of the first list with no skill |
| 6, 7 | 22, 23, 25, 24, 27 twice, 26 twice, 28 | a row of the second list: the cursor's, and the others |
| 8 | 22, 23, 64, 24 | a row of the second list with no skill |
| 9, 10 | 39, 40 | L's and R's arrows nudged (12 frames) |
| 0xB, 0xC | 55 | the slots' ground, for the first list and for the second |
| 0xD, 0xE | 58 four times, 59 to 62 | the slots' texts and pictures, for the first list and for the second |
| 0xF, 0x10 | the same | as a slot is being chosen, and after |
| 0x11, 0x12 | 24, 37, 32 | the rows while a slot is being chosen: the cursor's and the others |
| 0x13, 0x14 | the same | and after |
| 0x15, 0x16 | 56 | the bar behind a slot, for the first list and for the second |
| 0x17, 0x18 | 57 | the shade over the slots |
| 0x19 | 56 | the bar of the slot that was chosen, flashing (10 frames) |

Its texts (`001.xtc`): 0 "Remove skill “\c(01)%s\c(00)”?", 6 "Yes", 7 "No",
8 "\b(01)Set/Remove", 9 "\b(01)Switch on/off"; 1 to 5 are empty.

## ITEMS (`menuItemTask`, 0x52DE0 to 0x56B50)

What she carries, in three lists that L and R go round: what is used up
(consumables), what she wears (equipment) and what is kept for the story's sake
(rare items). Cross uses one of the first and changes to one of the second, each
after the window has asked. Its routine takes 0 to make (0x52EEC, the object by
0x5411C; sixty frames a second like the others), 2 to unmake (0x52FEC), 3 for a
frame (0x53DD4), 4 to draw (0x551A8), and `0x1000`, which answers whether its
state is 8. It is made by the MENU as CUSTOMIZE is, on layer 0x403.

### The items' tables, as far as the screen goes by them

`db_resident.lpk/000.xtc`, `001.xtc` and `002.xtc`, of 128 records each, one
after the other: item numbers start at 1. Every record begins with where its
name and what is said of it are written (0x12C05C, 0x12C110). A word of flags
(at + 0xC of a consumable, + 8 of the others) has: in its low half the item's
place in the order the lists are in (0x12C1C4); bit 16, it is in the game
(0x12C24C); bits 17 to 20, which list it is of (0x12C2F8: 1, 2, 3); bits 21 to
25, what kind of thing it is (0x12C3A4).

| Kind | What | Picture (0x2D198) |
| --- | --- | --- |
| 1 | gives HP back: Repairer 200 to 1500, Nano Repair 50% and 100% | 4 |
| 2 | for battles: the Charges, Body Cooler, the Breaks, ATK and DEF Boost, Rest Up | 5 |
| 3 | would give back to the "Raptor" (stat 0x13, of at most stat 0x11 times stat 0x14); no item is of it | 4 |
| 4, 5, 6 | 5 would end the mission for the stage select, after asking twice (texts 10); no item is of these | 9 |
| 7 | a weapon: Rock Cannon, Blue Square, Dynehalcyon, Hyperion | 6 |
| 8, 9 | none | 6 |
| 10 | a suit: Normal Suit, Lost All, Booster Unit, Schoolgirl, Poolside, Moon Rabbit | 7 |
| 11 | a rare item | 8 |

A consumable's word at + 0x10 has in its low seven bits how many she can carry,
in bits 7 to 9 where it can be used (0x12C520: 2 is in battles only, which all
of kind 2 are; those of kind 1 have 3), and in its high half how much it gives
back (0x12C640: 200 to 1500); the byte at + 0x14 is how many hundredths of all
there is it gives back besides (0x12C698: 50 and 100). The word at + 0x18 is
the status effect a battle item gives or takes.

A piece of equipment has three words from + 0xC: what a weapon adds to her
attack (0x12C748), a heat figure, and the bullets of a burst (15 / 10 / 6 for
the Rock Cannon, 25 / 10 / 7, 50 / 10 / 8, 60 / 8 / 8); for a suit the second is
the heat a dodge costs (15 for the Normal Suit, 10 for the others).

**What she has on** are her stats 9 (the weapon) and 0xA (the suit); 0x9C4 with
0xB and 0xC reads them. Changing (0x14D0CC): a suit is only written to its
stat; a weapon that is not the one she has takes the old one's addition out of
her attack (stat 5, by 0x14D2EC, which keeps it within bounds), is written to
its stat and adds its own.

**The models.** A weapon's is `gamedata/btl/wep/itm_equ_assualt_lv1.zzz` to
`lv4.zzz` for items 0x81 to 0x84 (0x14BF58). A suit's is the field character's
bundle `gamedata/fld/fchr/fchr_%s.bin` (0x1105C4) of the character the suit
stands for (0x14BE78: items 0x85 to 0x89 are characters 0 to 4, and 0x8A, the
Moon Rabbit, character 0x98; anything else is character 0), by the name the
field's table of characters has for it (0x1103A4): `p_brs00` to `p_brs05`.

What is loaded of them is kept by a list of 39 things of 16 bytes each at the
system's + 0x2076C (0x14BA30 answers one: flags, what it is of, the data and
its size; 0x14BC24 fills one in): the first is her suit's bundle, the second
her weapon's model (0x6075C gives a viewer the models out of the two; the
field has them shown and put away with 0x14BE34; a battle takes them at
0x9BAA4 and 0x9BAD0), the next four the archives of the skills in her slots
(0xEAD5C keeps those still wanted after CUSTOMIZE), then thirty-two by the
variables from 0x4FB0, and one more. So wherever she is seen she has on what
her stats say.

A weapon's second figure, its "heat", is read by nothing: the one caller of
0x12C7A4 (0x7E7B8, the dodge) asks it of her suit. The Hyperion's "Reduced
Heat Cost" is only words.

### Its object (0x830 bytes and a table of its textures)

Like CUSTOMIZE's, with these differences: its own model viewer at + 0x40
(0x3B404 with what 0x6075C gives for 0, and 0x3BCA0 with what it gives for 1),
the pose at + 0x44 and the animation over it at + 0x48, the ground's task and
the texts at + 0x4C and + 0x50, her HP gauge at + 0x54 (0x3C988), the window at
+ 0x58, the camera at + 0x5C with its picture's middle at + 0x60 and + 0x64, the
text of what an item is at + 0x84, the six rows from + 0x88 (0x90 bytes: a
text, what is written, how many she has at + 0x84, the item's kind at + 0x88
for the equipment's list, nought for the first list and 0xB for the third, and
the item at + 0x8C), the list at + 0x3E8, which list is shown at + 0x7E8, the
row, the scroll and the count at + 0x7EC, + 0x7F0 and + 0x7F4, her attack at
+ 0x7F8 and what it would be with the cursor's weapon at + 0x7FC, a count of
frames at + 0x800, a task that loads files at + 0x804 (`ifResFileMng`), and two
things it has loaded at + 0x80C (a suit's bundle and a weapon's model, each
with the item it is of).

Its + 0x18: 0 at the list, 1 the window asks, 2 it asks again (kind 5), 3 the
thing is done, 4 the window tells what was done, 5 the mission is being ended,
6 back to the MENU, 8 out of the menus. Flags: 0x80 her model is not drawn,
0x100 ready, 0x400 her model is to be changed for what the cursor is on, 0x800
nothing can be used (the mission is 0x42), 0x10000 sounded.

### How it is made (0x5411C)

- The layout is the archive's files 2, 3 and 4 with the textures: files 5, 6
  and 7, a place left empty, file 8, another empty place, the small pictures
  (0x2CE5C) and the figures (0x2CD24).
- The viewer is lit, has the archive's file 0 for its pose (not going round,
  at its end: two frames, as CUSTOMIZE's) and the MENU's `009.insa` over it,
  the same parts as the other screens and part 0x48 off besides (0x5319C), is
  told to keep the first animation still (0x3BADC with 2), and is turned 13, 27
  and -2 degrees.
- The camera is at (135, 243, 363) and looks at (-42, 77, 23), up (0, 1, 0), a
  lens of 20 degrees, from 100 to 3000; its picture's middle (0x856, 0x7E0) is
  86 right of the screen's and 32 above it.
- Places 0 to 5 play sets 1, 0, 2, 3, 4 and 5.
- If the mission (variable 0x2714) is 0x42, flag 0x800.
- The first list is built (0x53298) and the rows filled in (0x5657C); the window
  is made; her attack is read (0x5322C).

### The lists (0x53298, 0x533AC)

Every item of one of the three tables that she has any of (0x1BF4) and that is
in the game, sorted by its place in the order. With fewer than seven the list
is shown from its top, and with more it is not scrolled past its last six; the
cursor's row is kept if the list has an item for it, and is otherwise its last.
Changing to the list of equipment sets flag 0x400 (0x53254).

### A frame (0x5463C)

Nothing at all while the files' task is at work (0x3C4F4). In state 3, ready:
what the pad does, below. Not ready: it is when place 2 (the cursor's row) has
played its set through, 38 frames; and if the list has anything, sound 0x11
after four frames, once. Then the six places are played on, her gauge is told
her HP (0x53110), the window has its frame while it is not closed, her model
is seen to (0x534E8), and the viewer steps unless her model is to be changed or
the screen is being left.

**Her model** (0x534E8, 0x53768, 0x53890, 0x56A08): while flag 0x400 is set on
the list of equipment, the cursor's item is tried on. If it is what she has on,
the viewer is given her own two models again; if not, its file is loaded (a
weapon's model, or a suit's bundle) and, once it is, the viewer has that in
place of the one of hers it stands for. Giving the viewer models (0x53890)
sets the pose, the animation over it and the parts anew and clears the flag.
On the other lists she has what she has on. While the flag is set she is not
drawn.

### The pad, at the list

- triangle: out of the menus (8); circle: back to the MENU (6). Neither sounds
  yet: see the states;
- up and down (0x563CC): as in CUSTOMIZE, and on the list of equipment flag
  0x400; the rows are filled in;
- L and R (0x2E314 with three values): the next list or the one before; place 0
  plays set 7 for R and 6 for L; the list is built, the rows filled in, sound 2;
- cross, with nothing in the list: sound 7, which is "no";
- cross on a consumable: if it is one that can be used here (where it can be
  used is 1 or 3) and flag 0x800 is not set, then if it would do anything
  (0x14D804: one of kind 1 if her HP is not all there) the window asks (text
  0, "Use “%s”?", with the row's text for the name, and "Yes" and "No" of
  texts 11 and 12, the cursor on "No"; opened with sound 0), + 0x18 = 1; and if
  it would not, the window tells so (text 1; opened with sound 7, 0x6BE90),
  + 0x18 = 4. One that cannot be used here: sound 7;
- cross on a piece of equipment: the window asks (text 2 for a weapon, "Change
  weapon to “%s”?", 3 for a suit), + 0x18 = 1;
- cross on a rare item: sound 7.

**The window has asked** (1): once it has closed, "No" is back to the list; "Yes"
is + 0x18 = 3 (for the kinds there are).

**The thing is done** (3). A consumable (0x56854 first, which gives her what it
gives and answers how much HP she has more): she has one fewer (0x14D594), the
list is built and the rows filled in anew, and the window tells (0x6BCB0, no
sound of its own) with sound 0x10: text 4, "BRS's HP healed by %d.", if her HP
is still not all there, text 5, "... healed to maximum.", if it is; + 0x18 = 4.
What it gives (kind 1): so much, her HP kept between nought and all of it; then
so many hundredths of all her HP (cut to a whole number), likewise.

A piece of equipment, once the files' task is idle and her model has been
changed for it: what was loaded becomes hers (0x5398C), the list is built, she
changes to it (0x14D0CC), her figures are worked out anew (0x14D008), her
attack is read, the rows are filled in, and the window tells with sound 3:
text 8, "Weapon has been changed to “%s”.", or 9 for a suit; + 0x18 = 4.

**The window tells** (4): once it has closed, back to the list.

**Filling the rows in** (0x5657C): on the list of equipment, what her attack
would be with the cursor's item is worked out on a copy of her stats (changed
to it, figures anew, stat 0x16) and kept at + 0x7FC. A row's text is the item's
name, in the texts' second colour ("\c(02)%s\c(00)") for a consumable that can
only be used in battles or while flag 0x800 is set; a row with no item has the
string at 0x1670E8. The text of what an item is, is the cursor's. The strip
says text 13, 14 or 15 by the list.

### The states (0x53DD4)

0 a frame, until the screen's fade is over and the wipe is idle: 3. 3 a frame;
when + 0x18 is 6 or 8, and the files' task is idle: 6, and for 8 the screen's
fade begins (15 frames). 6 a frame; then, for 6, the wipe is told `0x1003` and
sound 5 is played, and for 8, sound 1 and the wipe `0x1001`: 7. 7 a frame; for
6, when the wipe answers `0x1005`, what was loaded is let go (0x53AA0) and the
MENU is told it is to come back; for 8 the same when the screen's fade is
over, and the MENU that the menus are done with: 8.

### What it draws (0x551A8)

1. Place 1, whole: the panels down the right.
2. Her model, unless flag 0x80 or 0x400.
3. Place 0, by its tracks' groups (the table at 0x167128, for groups 7 to 29):

   | Group | What |
   | --- | --- |
   | 10 | the text of what the cursor's item is, as in CUSTOMIZE (its pen is the colour of this group's object, black) |
   | 24, 25, 26 | the three lists' tabs, dark: each only while its list is not the one shown; and the second 13 to the left while the third list is shown |
   | 19, 21, 20 | the titles of the first, second and third list: only while it is shown |
   | 27, 28, 29 | the tabs lit: only while their list is shown |
   | 7, 8 | the scroll bar and its knob, for a list of seven or more, as in CUSTOMIZE |
   | the others | as they are |

4. The six rows, 20 apart. One with no item: place 4, the track of group 12 as
   its text. One with an item: place 2 for the cursor's row, 3 for the others
   (the table at 0x167188, for groups 11 to 47):

   | Group | What |
   | --- | --- |
   | 12 | the row's text |
   | 13 | the item's picture (0x2D198), by its kind, of the sheet of small pictures, uncoloured |
   | 15 | how many she has, set to the right with the last figure where the group is, in at least two figures (a nought before one), each over the track of group 14 (0x312D4) |
   | 14 | not of itself |
   | 47 | on the list of equipment only: the mark of what she has on, for the row of her weapon and of her suit |
   | the others | as they are |

5. On the first list, her HP gauge at its place 2, as on the MENU. On the
   second, if the cursor's item is a weapon, place 5 (the table at 0x167220,
   for groups 31 to 42):

   | Group | What |
   | --- | --- |
   | 36 | her attack (+ 0x7F8), from the left, each figure over the track of group 34 |
   | 37 | what it would be (+ 0x7FC), over group 35: the first of its two tracks if that is more, the second if it is less, neither if it is the same |
   | 38, 39 | drawn if it is more, and if it is less |
   | 40, 41, 42 | the label, by the row's kind: 7, 9 and 8 |
   | 34, 35 | not of themselves |
   | the others | as they are |

6. The window, and the strip.

The layout (`MENU_ITEM.GZ/002.sdoa`: 66 objects, 49 groups, 8 sets): set 0 the
panels down the right, 1 the screen (28 frames), 2 a row coming (38 frames), 3
a row at rest, 4 a row with no item, 5 the panel of her attack, 6 and 7 the
arrows by L and R nudged. Its texts (`001.xtc`) are quoted above; 6 and 7 are
the Raptor's.

## CHALLENGES (`menuLibraryTask`, 0x56B50 to 0x59DCC)

The third entry: the records of a stage at a time, sixteen squares in two rows
of eight, with what is told of the one the cursor is on over them and under
them. It is the screen the record's window is a cut of (`ifRecInfoTask`, see
"The records (challenges)" in `FIELD_NOTES.md`): its archive
`IF/MENU/MENU_LIBRARY.GZ` is `IF/MENU/IF_REC_INFO.GZ` byte for byte but for the
motions (`002.xbn`), and what it draws of a square and of a record is the same
code over again. It changes nothing: cross does nothing in it.

Its routines: 0x56C5C makes it (the object by 0x57700), 0x56DC0 unmakes it,
0x57500 is its frame, 0x57E34 draws it, and `0x1000` answers whether its state
is 7. It is made, given its frames and left as CUSTOMIZE is (the MENU's states
4, 6 and 5; callbacks 0x5C738 back and 0x5C768 out).

### Its object (0x528 bytes and a table of its textures)

| At | What |
| --- | --- |
| + 0x14 | its state |
| + 0x18 | what it is at: 3 being left for the MENU, 5 all the menus being left |
| + 0x1C | flags: 0x100 it takes the pad, 0x10000 it has sounded, 0x10 the strip is coming back |
| + 0x20 | the menus' common work |
| + 0x38 | the layout's player, with 20 places |
| + 0x48 | a window (0x6B5C8), which nothing puts anything in |
| + 0x4C, + 0x74 | the ten pictures of a record that is open, and of one cleared (the archive's files 22 to 31 and 12 to 21) |
| + 0x9C | six text boxes: the hint, what is to be done, what it is worth, the name (all 0x2D788 with 0, 1.0, 0x60, 4.0, 1); the total and the count of those cleared (kinds 2 and 3, a line of 4 glyphs, both with 0x271D4 and 1) |
| + 0xB4 | the frames of the strip's coming back |
| + 0xB8 | the frames it has waited to sound in |
| + 0xFC, + 0x100, + 0x104 | the square the cursor is on, how many records the page has, how far it is scrolled (never: a page is sixteen) |
| + 0x108, + 0x10C | how many stages there are to show, and the one shown |
| + 0x110 | how much of its goal the cursor's record has come to |
| + 0x114 | what is told of it: 0 nothing (a square with no record), 1 not known, 2 open, 3 cleared |
| + 0x118 | how many of the page are cleared |
| + 0x11C | the page's records, by number |
| + 0x51C, + 0x520 | 10 less the width of the total as written, and 15 less the width of the count, both cut to whole numbers |
| + 0x524 | 16, the total that is written |

### How it is made (0x57700)

The layout is the archive's files 1 to 3 with the record window's fourteen
textures (the archive's 4 to 11, 0x2CE5C, the archive's 12, 0x2CDC0, 0x2CD24,
0x2CE28, 0x2CD58). The strip says the archive's text 0 ("Check achievable
challenges for the stage.") at full alpha. The places play: 0 set 0 (all that
is about the squares), 1 set 8 (what is told of a record that is known), 2 set
7 (of one that is not), 3 set 1 (the square the cursor is on at first) and 4 to
18 set 2 (the other fifteen squares): a square's place is its number and 3.
Then the stages (0x56E84), the page (0x5714C) and what is told (0x56FEC).

**The stages (0x56E84).** There are as many as the flags 0x180D to 0x1812 are
set one after another from the first, and at least one. Those flags are the
stage select's: 0x5EE64 sets a stage's the first time its missions are shown. If
the menus were come to from the field (0x60A18 answers 1), the stage shown is
that of the map she is on (0x1294, 0x110368: the map's byte + 0xC), kept
within the stages there are; otherwise the first.

**The page (0x5714C, 0x571D0).** The records from sixteen times the stage, for
sixteen or as many as there are (0x14B658); how many of them are cleared
(0x14A978); the total is written as "%d" of 16 and the count as "%d" of those
cleared, and each is measured for + 0x51C and + 0x520. The cursor is kept, or
put on the last record if the page has fewer.

**What is told (0x56FEC)** of the cursor's record. A square past the page's
records: nothing (0). One that is open (0x14AD8C): what is to be done (its
+ 0xC), its name (+ 0), and what it is worth if it has that (+ 0x14); cleared
(0x14A978), it is told as cleared (3) and its share is 1.0; otherwise as open
(2), and its share is its count (0x14A9E8) over its goal (0x14B6CC). One that
is not open: the hint (+ 0x10) if it has one, and it is not known (1).

### A frame (0x57B28) and the states (0x57500)

The states are CUSTOMIZE's: 0 a frame, until the screen's fade is over and
the wipe is idle: 3. 3 a frame; at 3 the wipe is told `0x1003`, at 5 `0x1001`
and the screen's fade begins (2, 15 frames): 6. 6 a frame; at 3, when the wipe
answers `0x1005`, the MENU is told to come back; at 5, when the fade is over,
that the menus are done with: 7.

A frame, in state 3 only:

- Once it takes the pad: **circle** leaves for the MENU (sound 5), **triangle**
  leaves the menus (sound 1). Otherwise **L and R** go through the stages
  (0x2E314); if they did anything, place 0 plays set 6 for R or 5 for L (the
  arrow is nudged), the page and what is told are made anew, and sound 2 is
  played if the stage is another. Otherwise the cursor (0x5971C); if that did
  anything, what is told is made anew and sound 2 is played.
- Until then: it takes the pad once place 3 has played its set through
  (0x31C20); and if the page has any record and it has not sounded, it waits
  26 frames and plays sound 0x11.

Then, in every state: the strip's coming back if flag 0x10 is set (over 14
frames, as in the other screens), the twenty places' frames, and the window's.

**The cursor (0x5971C)** goes over two rows of eight, down the columns: the
square's number is twice its column and its row. With `n` the page's count
made even:

- **left**, each time it counts as pressed: two back; from the first column,
  only when it has newly gone down, to the same row of the last column that
  has a record (`n` - 2 and the row, at most 14 and the row);
- **right**: two on, while that is short of `n`; from the last column, on a new
  press, to the same row of the first;
- **up**: from the lower row to the upper; from the upper, on a new press, to
  the lower (if that is short of `n`);
- **down** (only if up does not count as pressed): from the upper row to the
  lower; from the lower, on a new press, to the upper.

Left or right and up or down are both taken in one frame. If the cursor is
somewhere else for it, the place of the square it left plays set 3 and that of
the one it came to set 4. It answers that it did something even when the
cursor is where it was (a page of one column), which is then heard all the
same. With an odd count the last column's lower square can be come to, though
it has no record.

### What it draws (0x57E34)

1. Place 0, by its tracks' groups (the table at 0x167278, for groups 23 to 65):

   | Group | What |
   | --- | --- |
   | 48 to 59 | the stages' tabs (0x59AC0), see below |
   | 60 to 65 | the stages' names: only the one shown (0x57360) |
   | 28 | the number of the cursor's square and one, in figures of the sheet at 0x2CD24 set to the right: the last where the group is, each one before it a figure's width and one to the left |
   | 29 | what stands before that number: 6 to the left for each figure past the first |
   | 23 | the count of those cleared, written at the group's object moved by + 0x520 and + 0x51C |
   | 24 | the total, written at its object moved by + 0x51C |
   | 25 | the stroke between them, moved by + 0x51C |
   | 31 | R's arrow: only with two stages or more, 8 to the left for each stage short of six |
   | 30 | L's arrow: only with two or more, 8 to the right for each stage short of six |
   | the others | as they are |

   The tabs (0x59AC0, with the table at 0x1711B8 of each stage's lit and dark
   group: 54 and 48, 55 and 49, and so on): all of them stand 8 to the right
   for each stage short of six. A stage has a tab if there are at least two
   stages and it is one of them. Its lit tab is drawn while it is the stage
   shown, its dark one while it is not; and the dark tabs of the stages
   between the first and the one shown, when that is the third or a later one,
   stand 13 to the left.

2. What is told. Of a record that is open or cleared, place 1 (the table at
   0x167328, for groups 7 to 20):

   | Group | What |
   | --- | --- |
   | 7, 8, 16 | what is to be done, what it is worth, and its name, written at the group's object |
   | 12 | the mark of one cleared: only for that |
   | 17 | the bar, for one not cleared: cut (0x15B098) to as much of its width as the share |
   | 18, 20 | only for one not cleared |
   | 19 | for one not cleared, the hundredths of its share (cut to a whole number) in two figures, as group 28 |
   | 13 | its small picture (0x2D4B0) |
   | the others | as they are |

   Otherwise place 2: group 9 is where the hint is written and groups 3 and 6
   are drawn, all three only for a record that is not known (not for a square
   with none); the others as they are.

3. The sixteen squares, each by its place (its number and 3), moved 51 across
   for each column and 48 down for the lower row (0x321F8). What a square is:
   nothing (no record), not known, open, or cleared (the table at 0x167360,
   for groups 41 to 47):

   | Group | What |
   | --- | --- |
   | 41 | the picture of its record's kind (0x573D4), cleared or open, a pixel up and to the left of the object: for one open or cleared |
   | 42 | for one not known or open |
   | 44 | for any square with a record |
   | 45 | the word UNKNOWN: for one not known |
   | 46 | its number, for one not known: its figures about the middle of the object |
   | 47 | the frame of one cleared |
   | the others (39 the cursor's light, 43 its frame) | as they are |

4. The window, and the strip.

The layout's sets (`MENU_LIBRARY.GZ/002.xbn`; 9 sets, 109 objects, 80 groups):
0 all that is about the squares (48 frames), 1 the square the cursor is on at
first, 2 any other square, 3 a square the cursor has left, 4 one it has come
to (each 120 frames long, of which the cursor's light goes round), 5 and 6 L's
and R's arrow nudged (12 frames), 7 what is told of a record not known, 8 of
one that is. Groups the record's window has no use for: 23 to 25 the count,
the total and the stroke at the top right, 30 and 31 the arrows, 48 to 53 the
six tabs dark and 54 to 59 lit, 47 the frame of a square cleared.

## OPTIONS (`menuOptionTask`, 0x5AFCC to 0x5C718)

The fourth entry: five rows, of which three are volumes with a bar and two
are switches.

| Row | What | Read by | Set by | Its text on the strip |
| --- | --- | --- | --- | --- |
| 0 | the music's volume, 0 to 127 | 0x428 | 0x460 | "Change the music volume. (square) Default" |
| 1 | the sounds' volume | 0x4B4 | 0x4F0 | "Change the sound effect volume. (square) Default (cross) Play" |
| 2 | the voices' volume | 0x548 | 0x58C | "Change the voiceover volume. (square) Default (cross) Play" |
| 3 | captions go on of themselves | 0x614 | 0x648 | "Automatically skip through dialogue. (square) Default" |
| 4 | the data install | 0x698 | 0x6E8 | "Turn on Data Install. (square) Default" |

**The options are one word** of the system's (the field manager's + 0x21E74):
bits 0 to 6 the music's volume, 7 to 13 the sounds', 14 to 20 the voices', bit
21 the captions, bit 22 the data install, bit 23 whether a data install is to
be had at all (0xD800 finds that out, 0xD76C; without it row 4 reads 0 and
cannot be changed). 0x9DD8 (through 0x5E8) hands the music's volume to the
music's stream and the voices' to the two streams of what is spoken. What a
new game has, and what square puts back (0x14D8E4: the three volumes and the
captions, not the install), is the one record of `db_resident.lpk/009.xtc`:
60, 127, 127 and bit 0 of its fourth byte, which is 0.

**What the captions' bit does** (0xD1634, where the task of a scene's
captions is made, `s239`): with the bit clear the task is made with its flag 4
set, which is "a caption stays until a button passes it"; with it set a
caption goes when its time is up. A script's `s243(n)` then sets that flag
(`n` not 0) or clears it for the scene, as the scenes that are cut to their
voices do (`s239(); s243(0)`). So in a new game the captions of every scene
that does not say otherwise wait for a button.

Its routines: 0x5B154 makes it (the object by 0x5B6E4), 0x5B1B4 unmakes it,
0x5B498 is its frame, 0x5BC98 draws it; `0x1000` answers whether its state is
5, `0x1001` sets + 0x54 (it was not come to under the menus' wipe: the screen's
fade is used instead), `0x1002` sets + 0x58 (1: triangle sounds as circle
does), `0x1003` answers + 0x70 (what 0x2D72C said of a data install that was
switched on). `0x1001` and `0x1002` are what the title's `menuSingleBoot` tells
it (see `TITLE_NOTES.md`): it is then up alone, at the title's thirty frames a
second. Its counts (the wait for its sound at 0x5BB80, the frames a direction
has been held at 0x5C2F4, the strip's at 0x5BC2C) each add 0x850 a frame, so
they keep their pace there; only a volume held past its ninety frames moves
half as fast, a step a frame. In the port that is `OptionScreen::alone`.

Its object (0x74 bytes and a table of four textures: the archive's 5, none,
6 and 7): + 0x14 its state, + 0x18 what it is at (0 coming, 1 the rows, 2 the
window asking, 3 left for the MENU, 5 the menus left), + 0x1C flags (0x10000
it has sounded, 0x200 the window asked about the defaults), + 0x38 the
layout's player with six places, + 0x48 the window, + 0x4C the strip's frames,
+ 0x50 the frames waited to sound in, + 0x5C the row the cursor is on, + 0x60
the row it was on, + 0x64 the frames left or right has been held for, + 0x68
and + 0x6C the archive's first file and its length: a recording, which cross
on the voices' row plays.

**Made (0x5B6E4):** the layout is the archive's files 2 to 4; the strip says
text 0; place 0 plays set 2 (the panel and the screen's name) and places 1 to
5 sets 3, 6, 9, 12 and 15, a row each. Each row has three sets, one after the
other: as it is at first, as the cursor comes to it, and as the cursor leaves
it.

**The states (0x5B498):** 0 a frame, until the wipe is idle (or the screen's
fade over): 1. 1 a frame; at 3 the wipe is told `0x1003` (or the screen's fade
begins), at 5 the wipe `0x1001` and the screen's fade: 3. 3 a frame; at 3 when
the wipe answers `0x1005`, at 5 when the fade is over: the data install is
seen to (0x698, 0xD800, 0x2D72C) and 4. 4, in which nothing of it is drawn:
the MENU is told to come back (0x5C738) or that the menus are done with
(0x5C768): 5.

**A frame (0x5B8D4)**, in state 1 only, by what it is at:

- 0: it takes the pad (1) when place 1 has played its set through (0x31C20);
  and unless it has sounded it waits 12 frames and plays sound 0x11.
- 1: **circle** leaves for the MENU (sound 5); **triangle** leaves the menus
  (sound 1, or 5 with + 0x58); **square** has the window ask text 5 ("Restore
  to default?") with texts 6 and 7 for "Yes" and "No", its cursor on "No", and
  it is at 2. Otherwise **up and down** (0x2E314 over the five rows): the row
  come to plays its second set and the strip says its text, the row left plays
  its third, sound 2. Otherwise left and right and cross (0x5C108, below).
- 2: when the window has closed, on its first answer the defaults are put
  back (0x14D8E4); it is at 1 again.

Then in every state the strip's coming back, the six places' frames and the
window's.

**Left and right (0x5C108)** change the cursor's row, from 0 to 127 or
between 0 and 1. While neither is held the count of frames held is 0. For
the first 90 frames of holding, a step each time the button counts as
pressed, and that is heard; from then on a step every frame, heard only on
the frames the button counts as pressed. A step that would go past either end
changes nothing (but in the first 90 frames is heard all the same). When the
value has changed:

- rows 0 to 2: it is set; the row's place plays the set that nudges its left
  arrow or its right (0x12 and 0x13, 0x14 and 0x15, 0x16 and 0x17), by which is
  held; rows 0 and 2 hand the volumes on (0x5E8); and if it is to be heard,
  sound 0xB;
- row 3: it is set; the place plays set 0x18 if left has newly gone down or
  0x19 if right has; sound 2;
- row 4: the same with sets 0x1A and 0x1B, if a data install is to be had;
  otherwise only sound 7.

And **cross**, newly down: on row 1 sound 0x66 of the bank that is always
there, on row 2 the archive's recording on stream 1 at full volume (0xAA04).

**What it draws (0x5BC98)**, unless its state is 4: place 0 whole; places 1
to 3, a row each, of which group 9 is the bar, cut (0x15B098) to as much of
its width as the volume is of 127, at its object's place and 22 lower for each
row; places 4 and 5, the two switches, of which groups 12 and 15 have two
tracks each: with the switch on the first of group 12's and the second of
group 15's are drawn, with it off the second of group 12's and the first of
group 15's; the window; the strip. (Group 12 is the word OFF, to the left of
the arrows, and group 15 the word ON, to their right; the first track of each
has the word dim and the second has it lit. The sets that nudge an arrow have
the dim track of the word that has just stopped being so fade to that.)

The layout (`MENU_OPTION.GZ/002.sdoa`: 25 objects, 24 groups, 28 sets): groups
0 to 2 the panel and what heads it, 3 to 7 the rows' names, 8 a bar's ground,
9 the bar (256 by 5 at (180, 112)), 10 and 11 the arrows either end of it, 12
and 15 a switch's two words (OFF and ON) with 13 and 14 the arrows between
them, 16 to 23 the screen's name. The rows' names as the layout draws them:
BGM VOLUME, SFX VOLUME, VOICE VOLUME, AUTO SCROLL, DATA INSTALL. Sets 0 and 1 are all of it coming and going (not used), 2
is place 0's, 3 to 17 the rows' (three each), 18 to 27 the arrows nudged. Its
texts (`001.xtc`) are quoted above.

## GALLERY (`menuGalleryTask`, 0x4E734 to 0x52DE0)

Read and ported on 2026-10-08 (see "In the port" at the end of this section). The
title makes it alone (`menuSingleBoot`, see `TITLE_NOTES.md`). Listings:
`tools_re/out/gallery_*.txt`.

**What there is to see** is three lists, which the game keeps as three tables of
the pack that is always there (`SYSTEM.VOL/db_resident.lpk` files 6, 7 and 8, set
up by 0x12B6B8 at the game's + 0x2333C; the archive's own files 1 to 3 are the same
tables) and reads through the functions at 0x12B768 to 0x12BED4:

| list | table | records | of them real | a record |
|------|-------|---------|--------------|----------|
| 0 pictures | `XTC`, 20 bytes each | 128 | 83 | name, what it is, the three figures of its file (`gamedata/gallery/glr_art_%s.bin`); byte 12; byte 13; byte 14 how many pieces across its picture is; byte 15 its small picture; u16 height, u16 width |
| 1 music | 16 bytes | 40 | 30 | name, what it is, its file (`gamedata/sound/bgm_%s.at3`); byte 12; byte 13; byte 14 its small picture |
| 2 films | 16 bytes | 40 | 33 | name, what it is, its file (`movie/%s.pmf`); byte 12; byte 13; byte 14 its small picture |

Byte 12 has bit 0x20 for a record that is real (0x12BB74) and in its low five bits
the *group* it is opened with (0x12BAE0); byte 13 is its place in its list's order
(0x12BA58). The small pictures are the 146 of the archive's file 11, each 32 by 88,
numbered from one (0x12BC14). A thing's *number* counts through all three tables
from one (0x12BE4C: pictures 1 to 128, music 129 to 168, films 169 to 208).

**What is open.** A thing is open while the system data's flag `11000 + number` is
set (0x4F49C: real, and 0x1964 of 0x2AF8 + number). 0x19AC(group, on), which a
record's reward of kind 7 calls (0x14A82C), sets that flag and the game's own flag
`8100 + number` for every thing of the group; 0x1A34, after a game is loaded
(0x2744), sets the system's flag for every thing whose game flag is set.

**The screen** (object of 0x344 bytes and a word for each of the layout's objects,
0x4FF74): the layout is the archive's file 4 with motions 5 and colours 6 and
eleven textures (the archive's 7 to 10, three places left empty for the small
pictures, then `if_rsdsys.lpk` 7, 9 (the figures), 20 and 12); four texts
(0x4EEE8: the name and what it is in the text's own font, the total and the count
in the capitals at ten and fifteen, as CHALLENGES has them); the window that asks
(0x6B5C8, never opened); the strip along the foot with the archive's text for the
list ("View", "Play/Stop", "Play/Stop"; 0x4A8EC). + 0x80 is the list, + 0x7C how
many real things it has, + 0xA4 on their numbers in order (0x4F978: the real ones,
sorted by byte 13), + 0x78 the first of the twelve shown and + 0x74 which of the
twelve the cursor is on; + 0x2D8 how many of the list are open.

The task (0x4EDCC): state 0 waits for the screen's fade to be off, then 1; in 1 a
wish to leave (+ 0x18: 1 by circle, 3 by triangle, sound 5 either way) has the
screen's fade go over in 15 frames, state 4; when that is there the MENU is told
(0x5C738 for circle, if there is a MENU; 0x5C768 for triangle) and the state is 5,
which is what it answers `0x1000` by. In state 3 the frame is the viewer's
(0x504F4), in every other the list's (0x4F0DC).

**The list's frame** (0x4F0DC, state 1). Until the layout's first place has played
its first set through (0x31C20) nothing is taken, and twenty-two frames after the
screen was made sound 0x11 is heard once. Then, of what has newly gone down:

- cross: the thing under the cursor (0x50294). One that is not open: sound 7.
  A picture: its file is asked for (0x3C028), the sign of loading comes (`0x1000`
  to the single boot's sign), state 3, sound 3. Music: if it is the one playing
  it is stopped (0xA10C) and nothing plays; else it is played (0x9E84 by its
  file, at full volume) and is the one playing (+ 0x2EC); sound 3 either way,
  and the list goes on being the list. A film: the task "Movie" is made
  (0x118E70), the film made ready and started (0x11863C, 0x1188E4), the music's
  volume kept and set full, the strip hidden, state 3, sound 3.
- circle and triangle: as above.
- L and R (0x2E314 with 0x400 and 0x800 over the three lists): the list before
  or the next, round; the frame plays set 9 for L or 10 for R, the lists are
  made anew (0x4F784, 0x522D4: the cursor and the first shown are kept within
  the new list), every card set and begun again (0x4FC38), sound 2.
- left and right (0x51DB0, by what counts as pressed): the cursor one back or one
  on. Within the twelve it just moves; at the end of the twelve the list moves
  under it by one (0x4FAA4 hands each card's set and frame on to its neighbour,
  so that what plays goes with the thing); from the first thing left goes to the
  last, and from the last right goes to the first, but only on a new press (the
  frame plays set 0xB or 0xC). Sound 2 when it moved, and what is told is made
  anew (0x4F7FC).

A card's set (0x4FB74) is 8 under the cursor and open, 6 under the cursor and not,
7 open, 5 not; its place of the layout is one more than which of the twelve it is.

**The viewer's frame** (0x504F4, state 3), by the list:

- *A picture.* While it loads the layout goes on. When the file is there
  (0x3C1A4): it is an archive of pieces, each a texture whose header's u16 at 6
  has the power of two of its width in its top four bits and its height in the
  rest; the first 0x133333 bytes of them are copied into the PSP's video memory
  (0x40CC000 on). The strip says text 3 ("Move", "Zoom in", "Zoom out", "Reset",
  "View"). The pieces lie in rows of as many as the record's byte 14 (0x52560),
  each row as high as its pieces. The view (0x4FC8C): its middle is the middle of
  the picture (half the record's width and height, cut to whole pixels), its size
  the smaller of 480 over the width and 272 over the height; a pixel of the
  picture is on the screen at `(pixel - middle) x size + (240, 136)`. Then, each
  frame: square puts the view back; circle lets the file go (sound 5) and the
  list is back, with its text on the strip; cross hides the strip, or shows it
  again; with triangle held, up (or the stick up) has the size grow by 0.01 a
  frame and down shrink it, between the size it began with and 2 (0x4FDDC);
  without it the D-pad, or else the stick, moves the middle by 8 pixels of the
  picture a frame each way, kept within the picture (0x4FE38).
- *A film.* Cross or circle stops it (0x118958, sound 5). When it is over
  (0x1189F0) its task is ended; and once what it had loaded is gone (0x12154) the
  strip is shown, the music's volume put back, the list is back, and 0x2780 with
  0x10 has the title's tune begin again.

**What is drawn** (0x4F4E4): in state 3 with a picture up, the picture (0x4F5D4,
its pieces as two-cornered sprites, 0x52830) and, unless cross hid it, the strip;
else the list (0x50BB0). The list is the layout's place 0 and then the twelve
cards, places 1 to 12, each moved 32 to the right of the one before. By the group
of each track:

| place 0, group | what |
|----------------|------|
| 6 | what the thing is, written where the group is (text 2) |
| 19, 21, 20 | the total, the count of those open, and the stroke between: each moved right by ten less the total's width, the count besides by fifteen less its own |
| 22, 23, 24 | only for list 0, 1, 2 |
| 25 | moved left by the name's width, or by the width of group 27 if the thing is not open |
| 26 | the cursor's number in the list, in the figures' sheet, set to the right of that same place: each figure a group's width and one to the left of the next |
| 27 | only for a thing that is not open |
| 28 | the name, written where the group is, only for one that is open |
| 31, 32, 33 | not for list 0; not for list 1, and moved 13 left for list 2; not for list 2 |
| 7 to 18, 29, 30 | always |

| a card, group | what |
|---------------|------|
| 8 | the thing's small picture in the group's rectangle, if it is open (0x4F678) |
| 10 | only if open; 11 only if open and the list is music; 12 only if not open |
| 18 | the thing's number in its list, in figures, set to the right |
| 9, 13 to 17 | always |

Last the window that asks and the strip. Not read: 0x52830's clipping of a piece
to the screen (the port draws the pieces whole), and the first object of the
module (0x4E734 to 0x4EC3F, the single boot's sign of loading).

**In the port.** `crates/brs-game/src/gallery.rs` is the tables (`Gallery`, a
`Thing` each) and what is open (`Opened`, the system data's flags as bits, kept in
`SYSTEM.sav` after the options; a file written before has none open). A record's
reward opens a group in the game's own flags (`Gallery::open_group`, from the
records' script's function 0x6F and where a record is taken as cleared), and
`field::open_gallery` has the system data follow whenever the game's flags change,
which is also what a game loaded comes to. `menu/gallery.rs` is the screen
(`GalleryScreen`): `menu::Single` has it up alone as it has OPTIONS, the title asks
for it by `take_wanted`, and when it is done with the title's tune begins again if
it played anything (`Menu::tune_again`). A picture's pieces are read when it is
asked for (`Menu::read_picture`, from `field::run_title`) and let go with it; a tune
is the sound's `Request::Music`; a film is asked of what plays films through the
scene, as the title's own is, and while it is up nothing of the menus is drawn.
Stands in: the sign of loading is not shown (a picture is read at once); a film
stops what was playing rather than having the film's task do it; the pieces of a
picture are drawn from the middle of their first pixel to the middle of their last,
so that the seams between them do not show when it is made larger.
Checked unseen from the title with some of each list open (`docs/compare/gallery_port.png`,
`gallery_picture_port.png`): the lists with their small pictures and counts, the
cursor and L and R, a picture looked at, made larger, moved and put back, a tune
played and stopped, the opening film played and ended by cross. Not compared with
the walkthrough, which does not go into it. Recipe: `python tools_re\field_sheet.py
gallery 4 15 17 19.5 22 25 28 31 34 -- --title --saves tools_re\out\_saves_gallery_some
--script "wait:8,start:0.2,wait:2,down:0.1,wait:0.4,down:0.1,wait:0.4,cross:0.2,wait:3,dright:0.1,..."`
(`dright` and `dleft` are the D-pad's; `right` and `left` are the stick's, which the
list does not go by; `_saves_gallery` has everything open). Tests: `gallery::tests`
and `menu::gallery::tests`.

## From the field and back

It is **triangle** that brings the MENU up on the field, not START. Where she is
free to be moved the field looks at what has just gone down (0x10D968 on, in
the PSP's bits):

- START (8), unless 0xD9448 says no: the pause (0x10CE08, another thing than
  this menu; "The start window" below); in mission 44 only sound 7 of the
  system's, which is "no";
- triangle (0x1000): the MENU (0xF18F8); in mission 44 sound 7 likewise.

0xF18F8 sets the field's flags 0x17 and 0x15, plays the system's sound 0, puts
the menu's own state (the field's + 0x34) at 0 and the field's mode at 6
(0x116850), and has no task yet (+ 0x1DD8). While flag 0x15 is set the field's
frame is 0xF1BC4 and nothing else of it (0x11526C). Its states:

| State | What |
| --- | --- |
| 0 | 0x11B310 if 0xF1B00 or 0xEB634 say so; then 1 |
| 1 | 0xEB99C; waits for 0x115D7C and 0xEB634 to be nought; 0xEAED4, 0xF055C; then 2 |
| 2 | waits for 0x115D7C and 0xF1B00; 0x11B35C; makes the menu (0xF19B8: `menuMainTask`, told `0x1004` with 1; the field's flags 0x1B and 0x1C are set); then 3 |
| 3 | asks the menu `0x1000` each frame (0xF1AAC); when it is done, asks it `0x1003` for what is to follow, ends it, clears flags 0x1B (and 0x1C), and by the answer: 1 = 0xF1B98 and the field's + 0x536C is set (0xEFA38 with 1); then 4 |
| 4 | waits for 0x115D7C; puts things back (0x11B35C, 0xEB8B4, 0x115C88, what she has on by 0x14BE34 with 0 and with 1, 0xF1B98, 0xEAD5C, 0xEB2E8), clears flag 0x15, 0x10CFC4, and has the screen come back (0x297E4 with 1, 15 frames) |

Of the routines named there only those of states 0 to 2 have been looked at:
they wait for things of the field that are being loaded or said, which the port
has none of, so the menu is made in the frame after triangle's.

What is seen on the way in: 0xF18F8 has the screen's fade keep a picture of the
screen (0x2C298, 0x2C2C0; the field's flag 0x17 has it taken as the field is
next drawn, 0x10F358). The field's own drawing (0x10F0F8) then goes by the
menu's state: in state 2, and for the one frame after the menu is made in which
flag 0x1C is still set, that picture alone (0x10EFB4); after that nothing at
all. So behind the menu there is nothing of the field: black, until the menu's
ground has come.

## In the port

`crates/brs-game/src/menu/mod.rs`: `MenuAssets` (the panel's, the ground's and the
wipe's layouts, her gauge, the strip's texts), `MainMenu` (the task's states and
a frame of the screen, at the menu's sixty a second: `Menu::step` is a frame of
the port's logic and runs two) and its drawing, on the layers 0x401, 0x402 and
0x404. `field/mod.rs` asks for it in `control_player` (triangle; "no" in
mission 44), has nothing of the field's frame run while it is up and its
animations stand still (`run_menu`), and has the screen come back over seven
frames when it is done. Run unseen: `--mute --skip-battles --skip-messages
--script "wait:30,triangle:0.1,wait:3,down:0.1,wait:1,down:0.1,wait:1.5,circle:0.1,wait:3"`
(`docs/compare/menu_main.png`: the wipe, the screen coming, the cursor on
CUSTOMIZE and on ITEMS). It has not been compared with the walkthrough yet.

Her model (`show_her`, `docs/compare/menu_main_her.png`): for as
long as the menu is made she stands far from anything of a map, with the
cannon on the node it hangs on, the pack's two animations and the parts the
panel shows, turned as the panel has her; a camera of the menu's own, with the
viewer's eye, lens and picture's middle, draws her once the panel has been
coming for 22 frames. All that the scenes' cameras draw goes to one picture,
which the canvas lays under the interface; for her to be over the panel and
under her gauge the canvas can be told where in its order that picture is to
be drawn instead (`Canvas::scene_here` in `brs-engine/src/canvas.rs`), by how
much of it there is; and for the picture to be clear of everything but her the
field's cameras are put out while hers draws (a camera that is not the first to
draw to a picture draws over what is there, whatever it is told to clear to).
The light the viewer has on her, (245, 255, 240) from everywhere, is not put on
her: the port draws every model as it is.

CUSTOMIZE (`menu/custom.rs`, `docs/compare/menu_custom.png`): `Custom` is the
task's states, what the pad does and its drawing, on layer 0x403; the MENU's
states 4, 6 and 5 make it under the wipe's third set, give it its frames and
come back from it. Its sixteen places are sixteen players of the one layout
(`Player::play` is 0x319BC: a set played in a place takes over the tracks of
its own groups and leaves the rest); its window is the one the maps' scripts
put up (`field/window.rs`), stepped once in a frame of the port's logic as it
is on the field and drawn on the screen's layer. It reads the names' table
through `Skills::about` and `Skills::listed` (`battle/skills.rs`), writes her
slots into `PlayerStats::skills` and the flags that put a skill aside into
`Progress`, and has her figures worked out anew as the original does
(`Hers::figure`); the field is told that they changed only when they did. Her
model has the viewer's second view (`custom::VIEW`): the archive's pose at its
end under the MENU's second animation, the screen's own camera and turns.
Run unseen with skills for it to show: `--has 1,2,3,7,10,14,18,23,30,33,42,54
--skills 1,23,0,0 --script "wait:30,triangle:0.1,wait:3,cross:0.1,wait:2.5,..."`
(cross on the MENU's first entry; it takes the pad 34 frames of the port's
logic later). It has not been compared with the walkthrough yet.

ITEMS (`menu/item.rs`, `docs/compare/menu_items.png`): `ItemScreen`, made as
`Custom` is (the MENU keeps whichever is up as a `Screen`). The tables' fields
it goes by are in `items.rs` (`Item::order`, `description`, `usage`, `gives`,
`figures`; `Items::carried` is a list), what she has on in `PlayerStats::weapon`
and `suit`, with `PlayerStats::helped_by`, `take` and `equip` (0x14D804,
0x56854, 0x14D0CC) in `battle/tables.rs`. The numbers (how many she has, her
attack) are drawn by `draw_number`, which the MENU's panel uses too. The
original loads the model of what the cursor is on with a task of its own and
hides her until it has; the port reads it at once: `show_her` makes her model
anew whenever what she has on differs from what it was made with, of the
suit's bundle (`player::suit_character`, `FieldSource::character_bundle`) and
the weapon's model (`player::weapon_model`), on every screen of the menus.
The two kinds of item the screen has code for and the tables have none of (3,
the Raptor's; 5, which ends the mission) are left out. Run unseen with things
for it to show: `--carry 1:3,2:1,6:2,14:1,20:1,130,132,134,136,257:2,258
--script "wait:30,triangle:0.1,wait:3,down:0.1,wait:0.5,cross:0.1,wait:2.5,..."`.
It has not been compared with the walkthrough yet.

The field and a battle show what she has on as well. On the field the
player's resource is the bundle of her suit instead of the one the list has
for its number (`CharacterAssets::wear`, as `load_characters` finds her suit
changed), and her model is made anew for another suit or weapon
(`show_characters`: `Actor::worn`), carrying the weapon's model on the node it
hangs on (`FieldWeapon`, put away by setting 76) where the cannon that is part
of her own model stood in for it before. A battle takes the suit's model and
the weapon's as it is loaded (`BattleSource::load`, `field::tables::suit_bundle`).
`--wear 132,136` has her have things on from the start of a run.

CHALLENGES (`menu/library.rs`, `docs/compare/menu_library.png`): `Library`,
made as the other two are. It is the record window's drawing over again
(`record_window.rs`, whose figures and small pictures it shares) with nineteen
places for the window's six: one for each of the sixteen squares, so that the
one the cursor leaves and the one it comes to each play a set of their own.
It reads the records' table and the save (`Hers::records`, `Hers::progress`)
and changes nothing; the stage it shows first is that of the map she is on
(`Hers::stage`, which `run_menu` takes from the game's list of maps). The
flags it counts its stages by (`menu::STAGE_SEEN`, 0x180D on) are the stage
select's to set (`hub::Hub`, as a stage's missions are first shown). What it
has that could not be tried: a page of an
odd count (the game's ninety-six records make six full pages) and a page
with none. Run unseen with stages and records for it to show:
`--flags 6157,6158,6159,6600,6601,6602,6603,6801,6803,6616,6617,6817,6632,6634,6834
--variables 20000:3,20002:1 --script "wait:30,triangle:0.1,wait:3,down:0.1,wait:0.5,down:0.1,wait:0.5,cross:0.1,wait:2.5,dright:0.1,wait:0.8,down:0.1,wait:0.8,r:0.1,wait:1.0,..."`
(cross on the MENU's third entry; it takes the pad 37 frames of the port's
logic later). It has not been compared with the walkthrough yet.

OPTIONS (`menu/option.rs`, `docs/compare/menu_options.png`): `OptionScreen`,
made as the others are; it has one state more than they, a frame at its end
in which nothing of it is drawn. What it changes is the resource
`options::Options` (`options.rs`: the three volumes of 127 and the captions'
way, a new game's from `db_resident.lpk/009.xtc`), which `Hers` hands it.
The sound reads that resource every frame (`sound::Volumes::of`), so a volume
is heard as it is moved; the field hands the captions' way to the map's
script (`MapScript::auto_skip`), which makes a scene's captions with it
(`Captions::made`). That changed what the port does in a scene: until now its
captions always went on of themselves, which is the option switched on, and a
new game has it off. A run with no one at the buttons (`--skip-messages`)
takes a caption that waits as passed. The data install is not in the
resource: the row reads as off and answers "no", as on a PSP that has none
to be had. What the screen has for being come to from elsewhere than the
MENU (+ 0x54, + 0x58: the title's options, 0x5CAE0) is left out. Run unseen:
`--script "wait:30,triangle:0.1,wait:3,down:0.1,wait:0.4,down:0.1,wait:0.4,down:0.1,wait:0.4,cross:0.1,wait:2.2,dright:1.2,wait:0.5,..."`
(cross on the MENU's fourth entry; it takes the pad 28 frames of the port's
logic later). It has not been compared with the walkthrough yet.

Not ported: the sign of loading (nothing is waited for), the frozen picture
of the field for the frame or two before the menu is made (the field's scene
stands still there instead), whatever 0x1869C does to the sound as the menu
comes and goes, and the options being kept from one run to the next (they are
of the system's save, which the port has none of yet).

Not read yet: the sign of loading. (The stage select is
`STAGE_SELECT_NOTES.md`, the title `TITLE_NOTES.md`, saving `SAVE_NOTES.md`.)

### The start window: the pause

Listing: `tools_re/out/start_window.txt` (0x10CD00 to 0x10D200). It is what
START brings up on the field (0x10CE08), and is ported (`pause.rs`; see the
end of this section). One routine opens it,
0x10CD04, with a kind: 0 (0x10CDD0), 1 (0x10CDEC) or 2 (0x10CE08, the field's
START). Its object is the field's + 0x221C: + 0 flags (1 it is up, 4, 8 it has
its own screen, 0x10), + 8 its state, + 0xC a count, + 0x10 the kind. Opening
keeps the picture of the screen (0x29A1C, 0x2C2E8) and asks 0x10CCA0: if that
answers, the picture is put up as it was taken and stands, the field's
+ 0x1E34 is set and its flag 0x17 (0x10CE64, state 0); otherwise the game is
drawn sixty times a second (0x808 with 0), something is made with the kind
(0x12CFF0 with 0 and the kind) and the field's flag 0x3D is set (0x10CEE8,
state 1). 0x10CE24 answers whether it is up, 0x10CF5C ends it (state 2: the
frame rate back unless the field's flag 0x2C is set, 0xEFA38). The system's
texts that look to be its own (`msg_system.bin`): 55 "(cross) Back to Title
Screen", 57 "(cross) SKIP", 58 "(triangle) RETURN (square) LOAD", 59 "Finish
mission and return to the Stage Select screen?", 60 "(cross) NEXT", 61
"(triangle) RETURN", 62 "(square) LOAD". 0x10CCA0 answers 1 (the picture's
way) unless the field's flag 0x2C is set or the screen's fade is covering the
screen and has done so (0x29974, 0x29948).

What 0x12CFF0 makes is a task whose routine is the 564 instructions from
0x12D85C to 0x12E128 (listing `tools_re/out/pause.txt`, 0x12CF80 to 0x12E600;
made by 0x12D5B0, let go by 0x12D474): it has an interface object of its own
(0x69810 to about 0x6A200: 0x69810 makes it, 0x69960, 0x699A0 and 0x69A20
tell it things, 0x69888 and 0x698C8 are its frame and draw, 0x6A09C), the
window of the maps' scripts for its questions (0x6B5C8 with a system text by
0x2C54C, the cursor and circle's answer by 0x6BC94 and 0x6BC84), the loading
screen (0x4FF8, 0x5048, 0x50FC), the film's stop (0x118958: it is what offers
to skip a scene's film) and the screen's fade.

**It is the game's pause.** The task's object (0x2C bytes): + 4 the interface
object, + 8 a byte set when a scene is to be skipped, + 0xA, + 0xB to + 0xE
bytes for what it stopped and has to let go on again (a film, 0x118924; the
tune, 0xA194 with 1; the two streams of what is spoken, 0xA484; with the
sounds, 0xD30C8), + 0x10 its kind (0 to 4), + 0x14 what was chosen, + 0x18 the
window (kinds 2 to 4), + 0x1C the system's texts, + 0x20 its state, + 0x24
the loading screen, + 0x28 another task (0x12F1B8). As it is made (0x12D5EC)
the interface is told which it is (0x69960): nothing for kind 0, 1 for kind
1, 2 for kind 2 (5 instead if 0x12D458 answers), 3 for kind 3, 4 for kind 4.
Its frame (0x12D8A4), by kind, with the pad's buttons newly down (8 start,
0x4000 cross, 0x1000 triangle, 0x8000 square):

- every kind: state 0 until the interface has come (0x699A0): 1. State 5
  until it has gone (0x69A20): 7, which is the end.
- kind 0: in state 1 start has it go (0x12D080; state 5).
- kind 1, a scene: in state 1 cross skips (+ 8 and + 0x14 are 1; a film
  playing is stopped, 0x118958) and start goes on (+ 0x14 is 0); either has
  it go. When it has gone after a skip the screen is black at once (0x297E4
  with 2 and none, 0x2BF5C with 1) and the scene is ended through the map
  script's context (its + 0x460 told `0x1007`, 0xCEF48 for its + 0x454).
- kind 2, the field (states by the table at 0x16E5A8): in state 1 triangle,
  unless 0x12D458 answers, has the window ask text 59 ("Finish mission and
  return to the Stage Select screen?") with "Yes" and "No", the cursor on
  "No" and circle for "No" (state 2); square makes the loading screen
  (0x4FF8 with 1, told 0x50B4 with 1; state 3); start has it go with
  nothing chosen. State 2: the window has its frame (0x6BA60) until it is
  answered (0x6BDE0); "Yes" is + 0x14 at 2, + 0xA set, the task of + 0x28
  told to darken over 60 frames (0x12F204 with 2 and 0x3C) and state 6;
  "No" is state 1 again. State 3: when the loading screen is done with
  (0x5048), a game loaded (0x50FC is 1) is + 0x14 at 3, the going sound, the
  screen black at once and state 4, which waits for the fade and ends;
  otherwise state 1 again (and 0x12F204 with 4 if 0x50D8 says so). State 6
  waits for the darkening (0x12F2C4), has the screen black and ends. The
  interface has no frame in states 2, 3 and 4.
- kind 3: as kind 2 without the loading screen (triangle asks, start goes).
- kind 4: start goes; a question that is up is answered as in kind 2, "Yes"
  being + 0x14 at 2 and going.

So + 0x14 says how it ended: 0 the game goes on, 1 a scene is skipped, 2 the
mission is given up for the screen between missions, 3 a game was loaded.

The interface object is made by 0x6A274 (from 0x69810): a layout with
textures of the pack that is always there (0x2CDF4, 0x2CD58, 0x2CCC4,
0x2CD24, 0x2CE5C, 0x2CDC0), a player of several places; 0x69CD0 writes
figures of two digits ("%02d": a clock) and 0x6A09C is its frame, 0x69E8C
plays its sets with sounds. Listing: `tools_re/out/pause_if.txt` (0x69330 to
0x6A400; the first routines there, to 0x69810, are another small object).

**Its layout** is the field's pack's (`SYSTEM.VOL/if_fldsys.lpk`, which is
what 0x2CCC4 gives) files 26 to 28, with eleven textures by the table at
0x167F80: the shared pack's 11 and 10, four places left empty, the field
pack's 16, the shared 9 (the figures), the field pack's 17, the shared 20
(0x2CE5C, the small pictures) and the shared 7. It has one place. The field
pack's texture 17 has the words PAUSE, "CLEAR % RECORD" and CHALLENGES
CLEARED; the layout (`tools_re/out/pause_layout.txt`: 65 objects, 41 groups,
5 sets) is a sheet over the screen with a pattern of squares and lines, the
word PAUSE (group 6), a frame, and on the right a list of records, each with
its small picture (group 16), the place of its name (17, 150 by 15), figures
of its count (18 to 21), and a bar (22 to 25, 40), with a knob for the list
(26, 27); at the foot two buttons' pictures with the places of what they do
(31 to 34) and the strip (35 to 39). So the pause shows the challenges of
where she is, with how far each has come, besides what the buttons do.

What the interface is told (0x6AF94, through 0x69960 for its way and 0x69980
for its flag 8): its way is kept at + 0x5C and gives the words at the foot
(0x69F90 with place 0), system texts all: way 1 text 57 "(cross) SKIP" and
flag 2 set; way 2 text 58 "(triangle) RETURN (square) LOAD" and flag 2
clear; way 3 text 61 "(triangle) RETURN", flag 2 set; way 4 no words, flag 2
set; way 5 text 62 "(square) LOAD", flag 2 clear; no way at all: no words,
flag 2 clear. (So way 5 is the field's while the screen between missions is
not to be had: 0x12D458 will be what says that.)

The interface's object, as far as read: + 0 flags (1 the list has moved, 2 the
list is not the pad's, 8), + 4 its state (1 coming, 2 there, 3 going), + 0x44
the player, + 0x48 the list's entries (twelve bytes each, 0x6A048) and + 0x4C
how many, + 0x54 the first row shown, + 0x58 where the list is, + 0x5C its
way, + 0x60 on its texts, + 0x84 the system's texts. It comes (0x69E8C with
0) by set 0, and set 2 as well when the list has five entries or more, with
sound 0; it goes (with 1) by set 1, set 3 with five or more, and set 4 if
flag 8 is set. Its frame (0x6A09C): while it is there, has a way and flag 2
is clear, up and down move the list a row (0x69FE4: from nought to four less
than there are entries, so four rows show; round the ends only on a new
press; sound 2 when it has moved); then 0x69CD0.

The list: 0x69A90 (0x69B98) makes room for so many entries and puts the list
at its top; 0x69AB4 (0x69C54) adds one: a name (+ 0), a second word (+ 4)
and a byte of 0 to 100 (+ 8). The task fills it as it is made (0x12D36C,
from the records: 0x14ABAC, 0x14AC24, 0x14B688, 0x14B6CC, 0x14A9E8). The
rest of the frame (0x69CD0): when the list has moved, the four rows' texts
are set anew, text 1 to 4 the names and 5 to 8 the bytes as two figures
("%02d": it is no clock but how far a record has come, the "CLEAR %"),
empty past the last entry; coming takes 20 frames (+ 0x88, by 0x850) and
then it is there; going is over when the place has played its sets through;
and the place is moved on.

The list is the records' list 2 (0x12D36C: 0x14ABAC and 0x14AC24 with 2,
which is every record whose count has gone up; `Records::list(2, ..)` in the
port): an entry's name is the record's, its second word the record's number,
and its byte the count over the goal as hundredths, cut to a whole number
(0x14A9E8 over 0x14B6CC). 0x12D458 is the save's flag 0x1819 (6169): the
screen between missions is not to be had yet.

**Its texts** (0x6A3F8 to 0x6A528, by the table at 0x167FB0): nine text
objects. Text 0, the words at the foot, is of kind 2 (the capitals at ten),
eighty to a line, two between glyphs, set to the right of its box (0x271D4);
texts 1 to 4, the rows' names, of kind 0 (the text's own font), forty to a
line, nothing between glyphs; texts 5 to 8, the rows' hundredths, of kind 1
(the capitals at five), five to a line, one between glyphs, set to the right.
A text is drawn by 0x6AC98 at the place of an object, moved as its track is,
in the object's colour (0x33190) by the track's, in a box of the object's
size: text 0 at object 57 (104 by 10 at (365, 253), grey), the names at
object 40 (150 by 15 at (267, 161)) and the hundredths at object 42 (5 by 5
at (433, 166), so that two figures end where the sign for hundredths begins),
each row twenty lower than the one before.

**Its draw** (0x6A5D0) goes through the tracks of its one place and by each
one's group either draws it as it is or does something of its own. With flag
2 set (a scene, the ride, a battle) groups 15 to 30 and 40 are not drawn at
all: that is the list. Otherwise, the rows being the four from the first
shown, each twenty lower (the layout's matrix, 0x32174):

| Group | What |
| --- | --- |
| 16, 17, 18 | once for each row with a record: its small picture (0x2D4B0 at object 39's place), its name, and the mark before its figures. Each of these groups has two tracks, and which is drawn goes by the track's number in its group (0x31BE4): the first only for a record cleared, the second only for one that is not |
| 19, 20 | for a row whose record is not cleared: its hundredths (text) and their sign |
| 21 | for a row whose record is cleared: the word CLEAR |
| 22, 24, 25 | for a row with a record: the bar's ground and what shines on it |
| 23 | for a row whose record is cleared: the whole bar |
| 40 | for a row whose record is not: the bar made as wide as the hundredths say (the matrix's first figure), from its left |
| 28, 29, 30 | for a row without a record |
| 31, 32, 33 | never drawn: the buttons' pictures, which the words have in them |
| 34 | the words at the foot (text 0) |
| 27 | the list's knob: as tall as four rows are of all the records (no taller than what it runs in, object 50's 75), and as far down as the first row shown is of them |
| the rest | as they are |

The knob and what it runs in are there by set 0 with the knob at no alpha;
set 2, played as well when there are five records or more, brings both up.
Set 4, played as it goes when flag 8 is set, takes the track of the sheet
(group 1, the white sheet at half alpha that is over the screen) and has it
go black over twenty frames: a scene that is skipped is not seen again.

**How it goes** (0x12D080, the task's `0x1000`, 0x12E068): flag 8 is set if a
scene is to be skipped and cleared if not; then by how it ended the interface
goes (0x698C8) with the system's sound 1 (the game goes on, or the mission is
given up), sound 3 (a scene skipped) or none (a game loaded). The task's other
messages: `0x1001` whether it is over (state 7; 0x12D0D8), `0x1002` whether a
scene is to be skipped (0x12D1B4), `0x1004` how it ended (0x12D304). As the
task is made what plays is held still: a film if one plays (0x118924), or
else the tune (0xA194 with 1), the two streams (0xA484) and the sounds
(0xD30C8); 0x12D474 lets them go on as it is let go, and with a scene skipped
has the streams stop (0xA2DC with 1) and every sound (0xD3648).

**What darkens the screen** when the mission is given up is a task of its own
("sysFadeTask", 0x12F1B8, drawn over the pause: its number is 0x6FE to the
pause's 0x610, and the screen's fade is 0x600, under both). Told a kind and a
count of frames (0x12F204: 1 black to clear, 2 clear to black, 4 half to
clear, 8 clear to half), it has a sheet go there, a frame at a time; 0x12F2C4
answers when the count has run out. The pause tells it 2 and 60 at "Yes", and
4 and 60 when the list of saves comes back without a game (that list, at
0x6FF, is over it).

**The field's side** (0x10CB0C, once a frame while the pause's object is up).
Opening (0x10CD04) has a picture of the screen taken, which stands under the
pause while the game is not drawn, and the next frame makes the task with the
game at sixty frames a second. When the task is over: a scene to be skipped
sets the object's flag 4; the task is let go (0x12D144); and by how it ended
the game goes on (0x10C998: with flag 4 the scene is skipped there, 0x10C92C,
and the screen made black; the picture is let go, the game is at thirty
frames again, and the field's flag 0x3E keeps start from being taken in the
same frame), or the field is left (0x10CF5C: its + 0x536C is 3 for a mission
given up, which is what `s173` is given for that, or 4 for a game loaded).

**Who asks for it.** On the field, where she is free to be moved, START
(0x10D968; in mission 44 only the system's sound 7): kind 2. In an event
(0xE87E8), START while the part of it that can be skipped is running
(0xD93DC: the script context's bit 0x100) and it is not being skipped already
(0xD9448: bit 0x200): kind 1, or kind 0 if the field's flag 0x48 is set (a
script's function sets and clears that, through 0xE5DA0; which has not been
looked for). In a battle ("BtPause", 0x97958, its frame 0x97A30), START while
its flag 4 is set, which is from when the fight begins (0x93FE4) until the
battle is decided (0x8D79C): kind 4, once the battle's loader ("BtLoad") is
idle; while its flag 1 is set everything of the battle that moves stands
still (thirteen routines ask 0x97BB8 first). In the ride (the game within the
game, `bmg_`, 0x147BA8): kind 3.

**In the port** it is `crates/brs-game/src/pause.rs`: `Pause` is the task and
its interface, a frame of the port's logic being two of its own; `Paused` is
the resource that has it while it is up, and nothing of the field (or of a
battle) has a frame meanwhile (`field::run_pause`, `battle::pause`). What
plays is held still by the sound's `Request::Hold` and let go by `LetGo`. The
game is not drawn from a picture: the scene simply stands still. Kinds 1, 2
and 4 are asked for as above; kind 0 and the ride's are there and not used.
Square puts up the port's own list of saves (`SaveDesk`), and a game loaded
there takes the place of this one at once. Pictures: `docs/compare/pause.png`
(the field's, its question, the screen going dark, the screen between
missions come to) and `docs/compare/pause_kinds.png` (a list of six records
moved down, a scene's, a battle's). It has not been compared with the
walkthrough, which does not show it.
