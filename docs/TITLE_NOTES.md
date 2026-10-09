# The title screen

What the game shows first: the maker's mark, the title with PRESS START, and its
menu of five entries. Read from the executable (`TitleTask`, 0x1193A8 to 0x11AB74;
its interface object, 0x6D3D4 to 0x6E588) and from `TITLE.VOL`. Listings:
`tools_re/out/title.txt`, `title_end.txt`, `title_if.txt`, `title_if2.txt`, and the
layout as `brs-extract --layout` lists it, `tools_re/out/title_layout.txt`.

## The data (`TITLE.VOL`)

| File | What |
| --- | --- |
| `ie.zzz` | one texture, 480 by 272: the maker's mark ("Imageepoch"), white on black |
| `if_title.zzz` 000 to 008 `.ptm` | the title's textures |
| 009 `.sdoa`, 010 `.bin`, 011 `.xbn` | its layout: objects, groups, motions (the groups come before the motions here; in other packs they come after) |

The layout's eleven textures (0x6DC0C, the table at 0x168140): file 0, none,
files 1 to 4, file 7 of the pack that is always there (0x2CDC0: a chequer of
128 by 8, which the entries' strokes are cut from: dotted lines), files 5 to 8.

| Texture | File | What |
| --- | --- | --- |
| 0 | 000 | the ground: dark plates of metal, 480 by 272 |
| 2 | 001 | a grain, drawn four times over the ground, added |
| 3 | 002 | a blurred shape, drawn in four quarters, three tracks to each: streaks that run up the screen |
| 4 | 003 | the glow behind the logo |
| 5 | 004 | the logo, the lines of copyright, PRESS START, the glows of the entries, the two moving bars |
| 6 | shared 7 | a chequer: the dotted strokes by the entries |
| 7 | 005 | the entries' words: NEW GAME, OPTIONS, INSTALL, LOAD, GALLERY, and their bullet (GAME is drawn twice) |
| 8 to 10 | 006 to 008 | clouds |

The layout has 69 objects, 59 groups, 35 sets and 50 clips. Its groups: 0 the
ground; 1 to 3 sheets of blue over it; 4 to 7 the grain; 8 to 11 the streaks; 12
the copyright; 13 the logo's glow; 14 the logo; 15 a bar that slides up and down
at the left, cut to object 17's box; 16 to 19 strokes at the left; 20 a bar that
slides along the top right, cut to object 23's box; 21 to 26 strokes there; 27
PRESS START; 42 a sheet of black over everything; 54 to 58 the sky and its
clouds. The five entries, top to bottom, each a word, a stroke under it, a
bullet, the bullet's square and a glow:

| Place | Entry | Code | Word | Stroke | Bullet | Square | Glow |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | NEW GAME | 0 | 28 | 29 | 36 | 37 | 38 |
| 1 | LOAD GAME | 1 | 30 | 31 | 43 | 46 | 39 |
| 2 | GALLERY | 4 | 49 | 50 | 52 | 53 | 51 |
| 3 | OPTIONS | 2 | 32 | 33 | 44 | 47 | 40 |
| 4 | INSTALL | 3 | 34 | 35 | 45 | 48 | 41 |

(The code is what the task goes by, 0x6D57C and 0x6D698: GALLERY was put in
third and kept the last number.)

The sets, by what plays them (0x6DF58; the player has two places, the ground and
logo in the first and what is chosen from in the second):

| Set | What |
| --- | --- |
| 0, 1 | everything of the first place, of the second, with no alpha: nothing shows |
| 2 | the ground of metal |
| 3 | the sky: a blue that deepens down the screen and clouds drifting, for a game that has been cleared |
| 4 | the title comes: sheets, grain, streaks, and over 70 frames the copyright, the logo and its glow; the bars and strokes after 20 to 30 |
| 5 | PRESS START comes over 70 frames and then pulses; the black sheet goes over 20 |
| 6 | PRESS START goes over 2 frames; the five words come, white at alpha 166, and their bullets |
| 7, 8, 26, 24, 25 | an entry as the cursor is on it at first (places 0 to 4): its word blue and whole, flickering in over 16 frames, bullet off, square and stroke on, the glow pulsing |
| 9, 10, 13, 11, 12 | an entry as the cursor comes to it (2 frames) |
| 14, 15, 18, 16, 17 | an entry as the cursor leaves it (10 frames) |
| 19, 20, 23, 21, 22 | an entry that was not chosen goes (2 frames) |
| 27 | the black sheet comes over 20 frames |
| 28 | the title comes at once, for the menu: as 4 with shorter clips |
| 29 | the words and bullets come after 70 frames; the black sheet goes over 20 |
| 30, 31, 34, 32, 33 | as 7 and its fellows, after 70 frames |

## The interface object (0x6D3D4)

Its object (0x58 bytes): + 0 flag 1 it is drawn, + 4 its state, + 8 the frames
nothing has been done for; from + 0xC what plays the layout: + 0 flags (1 the
sky instead of the metal, 2 a sound is due), + 4 the cursor's place (0 to 4),
+ 8 what is waited for (0 nothing, else which telling), + 0xC the player, + 0x1C
the eleven textures, + 0x48 the frames until the sound.

**What it is told (0x6D4BC):** 0 to reset (the tellings' 0; state 0, not
idle), 1 to come (the tellings' 0 and 1; drawn, state 1), 2 to come straight to
the menu (the tellings' 4; drawn, state 6). 0x6D57C sets the cursor from an
entry's code (kind 0), which ground (kind 1) and the idle frames (kind 2);
0x6D698 reads them back. 0x6D774 answers 1 in state 2 (PRESS START), 2 in state
7 (an entry was chosen and has gone), else 0.

**The tellings (0x6DF58), each a few sets played (0x319BC):**

| Telling | Sets | Waits |
| --- | --- | --- |
| 0 | place 0 set 0, place 1 set 1 | nothing |
| 1 | place 0 set 2 (3 with the sky) and then 4; place 1 set 5 | 1 |
| 2 | place 1 set 6, and the cursor's entry its set of 7, 8, 26, 24, 25; sound 0x11 on the next frame | 2 |
| 3 | every entry but the cursor's its set of 19 to 23; sound 3; place 1 set 27 | 3 |
| 4 | place 0 set 2 (or 3) and then 28; place 1 set 29 and the cursor's entry its set of 30 to 34; sound 0x11 after 70 frames | 4 |
| 5, 6 | the cursor goes up, down (round the ends): the entry left plays its set of 14 to 18, the one come to its set of 9 to 13; sound 2 | (nothing) |

A wait is over when the second place has played all its tracks through
(0x31C20). The frame of this part (0x6D7E8) ends the wait, counts down to the
sound (0x850 a frame) and moves both places on (0x30C8C).

**Its states (0x6D9A0; the table at 0x168120).** Pad bits are the PSP's:
0x4000 cross, 8 start, 0x10 up, 0x40 down.

| State | What |
| --- | --- |
| 0, 7 | nothing |
| 1 | when the wait is over: 2 |
| 2 | PRESS START. Cross or start newly down: telling 2, and 3 |
| 3 | when the wait is over: 4 |
| 4 | the menu. Cross newly down: telling 3, and 5. Else with up held: if it counts as pressed again, telling 5, but from the first entry only if it has newly gone down; down likewise with telling 6 and the last entry. A cursor that moved is not idle |
| 5 | when the wait is over: it is no longer drawn, and 7 |
| 6 | when the wait is over: 4 |

Every change of state sets the idle frames to nought; every frame adds 0x850
to them.

**What it draws (0x6DD60):** both places, track by track in their order
(0x30F30), with the layout's own matrix and colour; group 15 cut (0x15B098) to
object 17's box, (19, 16) 3 by 225, and group 20 to object 23's, (368, 19) 96
by 8.

## The task (`TitleTask`, 0x1193A8)

Made by `launchTitle` (0x7369C), which first has the game's own start clear the
save (0x13C), as task "Title", number 0x300. It is launched as the game boots
(0x3194), from the screen between missions (0x5E118) and from the field's start
control (0xE6F6C). Its object (0x3A0 bytes): + 0 its state, + 4 and + 8 the
archive, + 0x10 the mark's texture, + 0x18 a count, + 0x1C the entry's code,
+ 0x20 whether a save was found (not below nought), + 0x24 and + 0x28 the
film is to be shown, + 0x2C the mark's light (0 to 1), + 0x3C the film's task,
+ 0x40 the loading screen, + 0x48 a window, + 0x74 and + 0x7C the interface's
archive, + 0x78 the interface, + 0x88 and + 0x8C the tasks of OPTIONS and
GALLERY, + 0x390 the seconds of idling before the film, + 0x394 frames, + 0x39C
the music's volume kept.

As it is made the screen's fade is put over at once (0x297E4 with 2, 0x2BF5C
with 1). The states (the table at 0x16E1E8):

| State | What |
| --- | --- |
| 0 | `gamedata/title.vol` is asked for, the music stopped (0xA10C with 0), the game drawn sixty times a second (0x808 with 0): 1 |
| 1 | when it is there: the mark's texture and the interface are made; the screen's fade is cleared at once (0x2BF5C with 0); a window ("PopChoWin", number 0x800, the window of the maps' scripts with the system's text 35: "This game has an auto-save function. …") and 2 |
| 2 | when the window is read: 3 |
| 3 to 6 | the PSP's memory stick and its system data (0xDFA0, 0x5278; texts 9, 21 and 33 with "384KB", "Yes" and "No"): then 7 |
| 7 | the title's tune is started (0x119368: the game's tune 0, `title`, or 5 if the game has been cleared, 0x394) and the volumes handed on (0x5E8): 8 |
| 8 | the tune is held (0xA194 with 1); the mark's light goes up a tenth a frame; when it is whole the count is 120 frames: 9 |
| 9 | the count runs down (0x850 a frame); triangle, circle, cross or square newly down ends it. Then the save is looked for ("MEMGETUSERID") and 0xA |
| 0xA | the mark's light goes down a tenth a frame; when it is out (and the save has been looked for): the screen's fade comes off over 30 frames (0x2983C with 1) and 0xB |
| 0xB | the tune goes on (0xA194 with 0); the game is drawn thirty times a second (0x808 with 1); the interface is told which ground (0x394), the idle time is 60 seconds, or 65 for a game cleared; the cursor is put on LOAD GAME if a save was found, else on NEW GAME; the interface is told to come: 0xC |
| 0xC | the menu (below) |
| 0xD | waits for the screen's fade: 0xE |
| 0xE | the task ends itself. NEW GAME: variables 100 to 10, 10004 to 0 and 220 to 100, and the field is started (0xE6C18). A game loaded: by its variable 300 (0x2400) the field (0xE6CB0) or the screen between missions (0x5CEFC) |
| 0xF, 0x10 | the film is made ready: the screen's fade in over 4 frames, 0x11 |
| 0x11 | the film (`MOVIE/BRS_OP_final.pmf`) plays; cross or start ends it (0x118958). When it is over: fade out over 4, 0x12 |
| 0x12 | the tune again, the fade off over 30 frames, the music's volume back, the interface reset: 0xB |
| 0x13 | LOAD GAME: the loading screen (0x4FF8) is waited for (0x5048). A game loaded (0x50FC answers 1): the music stops over 4 blocks, the fade goes over 30 frames (0x2983C with 2), 0xD. Otherwise the interface comes straight to the menu and 0xC |
| 0x14 to 0x17 | INSTALL: the PSP's data install ("MSGETFILELIST", "MII"; texts 40, "%d MB"); every way out of it has the interface come straight to the menu and 0xC |
| 0x18 | OPTIONS: when its task is done (`0x1000`), it is ended, the fade comes off over 30 frames, the interface comes straight to the menu: 0xC |
| 0x19 | GALLERY: the same, and if the gallery says so (`0x1004`) the title's tune is started again |

**The menu (state 0xC).** Each frame two random numbers are drawn (0x93A0).
When the interface answers that an entry has gone (0x6D774 is 2), by its code:
NEW GAME stops the music over 4 blocks, has the screen's fade go over in rows
(0x2983C with 0x200) and is 0xD; LOAD GAME makes the loading screen and is 0x13;
OPTIONS and GALLERY make "menuSingleBoot" (0x5C798, number 0x402) with 0 and 1
and are 0x18 and 0x19; INSTALL is 0x14. Then, unless the film is on its way:
when the interface has been idle for the idle time (sixty frames a second),
the music stops over 8 blocks, the fade goes over in rows, and after 61 frames
more the film's task is made (0xF). Last, the interface has its frame
(0x6D9A0).

**What it draws (0x11A970).** In states 8 to 0xA the mark over the whole
screen, grey by its light (0x13B70). In states 0xB to 0xD, 0x13 and 0x16 the
interface. In every other state nothing.

### OPTIONS and GALLERY alone (`menuSingleBoot`, 0x5C798)

Its object has the menus' ground (from + 0x20) and loads the screen's archive
(`menu_option.gz` or `menu_gallery.gz`), with a sign of loading meanwhile.
Then it makes the screen's task, number 0x403 (`menuOptionTask` told `0x1001`
and `0x1002` with 1: the screen's fade instead of the menus' wipe, and triangle
sounding as circle does; or `menuGalleryTask`, 0x4EC40), and has the screen's
fade come off over 15 frames. When the screen says it is done (`0x1000`) its
archive is let go and the ground ended; after OPTIONS, if the screen says so
(`0x1003`), the system data is saved. The gallery is in `MENU_NOTES.md`,
"GALLERY"; when it is left the title has its tune begin again if the gallery
says it played anything (`0x1004`).

## The screen's fade in rows (kinds 0x100 and 0x200)

Each of the screen's 272 rows has a place of 0 to 480 and a pace (the fade's
+ 0x54, a word a row). Started (0x2B92C, 0x2B99C) every row's pace is a random
0 to 31 and twice 480 over the frames, and its place 480 (0x100: the sheet
comes off) or 0 (0x200: it goes over). Each frame (0x2A61C) a row's place moves
by its pace, down to 0 or up to 480; when none moved the fade is there. While
it is on its way each row is drawn as two lines (0x2AFA4): black from the left
edge to twice its place less 480, and from there over 480 pixels from black to
nothing. Once it is there the whole screen is the sheet.

## In the port

`crates/brs-game/src/title/`: `mod.rs` the task (`Title`), `interface.rs` the
interface object, `rows.rs` the fade in rows. The game begins on it unless a
place to begin is named (`--map`, `--mission`, `--start`, `--stage-select`;
`--title` says so outright, and `--cleared` has the game count as cleared).
It lives in the field's state like the screen between missions
(`FieldState::title`): while it is up nothing of the field runs or is drawn,
and the scenes' cameras are put out. The mark is counted in its own sixtieths,
two a step; the title itself has a frame a step.

- NEW GAME (`field::run_title`): the save is made anew (`Progress::start`, her
  figures and what she carries as a new game has them, the records, the system
  script's `init_player`), the first mission set as begun, and the field
  started afresh on the map of the story's opening (map 67, `d_stg05_0a1`, by
  its entrance 0, as 0x95C has it), whose script plays the opening and goes on
  to the first mission's map. A game begun from the title plays its events.
- OPTIONS: `menu::Single` is `menuSingleBoot` for it, the menus' ground with
  the screen and the strip, and `OptionScreen::alone` the screen as it is then:
  a frame of it is two of the interface's (the title has the game drawn thirty
  times a second), it comes when the screen's fade is off (7 frames of the game)
  and goes under it. `Title::take_wanted` and `Title::screen_done` are how the
  two meet.
- From the screen between missions the answer "Yes" to both questions ends it
  with `Outcome::Title`, and the title is made.
- The screen's fade does its sums as the original does, on a level of 0 to
  255 (`Fade::advance`): with a level of 0 to 1 a fade over 5, 7, 15, 45, 60 or
  120 frames took a frame more than the original's.

Left out, as a PC has no use for them: the window about the auto-save and the
memory stick's light, the system data's windows, and the data install (INSTALL
is still an entry: chosen, it comes straight back to the menu, as it does on
the PSP when the install is not gone through with). GALLERY is the menus'
screen up alone, as OPTIONS is (`Menu::open_gallery`).

The film is played (since 2026-10-08): `Title::showing` is the film it has made
ready, which `field::run_title` hands to what plays films through the scene
(`FIELD_NOTES.md`, "Films"). The title waits for it to be ready (states 0xF and
0x10), starts it with the screen's fade coming off over four of the interface's
frames, and when it has played through, or cross or start is pressed, has the
fade go over in four more and comes again (0x12).

The walkthrough begins after the title, so there is nothing to compare it with
but the code and the data. `docs/compare/title.png`: the mark, PRESS START, the
menu, and the cursor moved; `title_cleared.png`: the title of a game cleared;
`title_options.png`: OPTIONS chosen, its screen, and the menu come back to.
