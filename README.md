# Black Rock Shooter: The Game — native PC port (Rust / Bevy)

A hobby project to rebuild the PSP game as a native PC program. There is no source
code for the original, so this is a new engine that reads the original data files.

The game data is **not** part of this project. It is read from your own copy of the
game, kept locally under `game/`, which is ignored by git.

## Playing it

No command line and nothing to install: see [HOW-TO-PLAY.txt](HOW-TO-PLAY.txt).
In short:

1. Take `brs-pc-port-windows.zip` from this project's **Releases** page and unzip it.
   It has the game (`brs-game.exe`), the setup program (`brs-setup.exe`) and the
   how-to; none of the game itself.
2. Drag the disc image (`.iso`) of your own copy of the game onto `brs-setup.exe`,
   or double-click `brs-setup.exe` and choose the `.iso` in the window that opens.
   The port was made against the US release (NPUH-10126).
3. Choose the folder to make the game in (about 1.5 GB) in the window that asks.
4. Wait while it copies the films, unpacks the data (under a minute), makes the
   music and voices playable (the long part) and puts the game in the folder. The
   sound is decoded by [ffmpeg](https://ffmpeg.org): if the PC has none, the setup
   fetches it by itself (about 115 MB, from gyan.dev) into the folder.
5. Open the folder and double-click `brs-game.exe`.

The setup can be closed and started again for the same folder: it goes on where it
was. The `.iso` is only read. None of the game's data is in this repository, and
none should be put in it.

## Building it yourself

The bundle on the Releases page is built by GitHub from this source
(`.github/workflows/build.yml`: by hand from the Actions tab, and for every tag that
begins with `v`, which makes a release of it). To build on your own PC you need
[Rust](https://rustup.rs) with its C++ build tools, and then either double-click
`build.cmd` or run

```bash
cargo build -p brs-game -p brs-extract
```

That makes `brs-game` and `brs-setup` in `target/debug`; `brs-setup` is used as
above, or from a command line as `brs-setup game.iso folder` (`--no-sound` leaves the
sound out, `--jobs <n>` has it decode so many recordings at once).

## Layout

| Path | What it is |
| --- | --- |
| `crates/brs-formats` | Library: readers for the game's file formats, and the script interpreter |
| `crates/brs-extract` | Command-line tools: `brs-extract` unpacks the game data, checks that every known format parses, and makes the recordings playable; `brs-setup` does all of it from a disc image, into a folder of your choosing |
| `crates/brs-engine` | Library: Bevy code shared by the viewer and the game (models, animation, maps, collision, sound) |
| `crates/brs-viewer` | Bevy app: shows one model or map from the game data |
| `crates/brs-game` | Bevy app: the port itself |
| `docs/FORMAT_NOTES.md` | What is known about each format and about the original executable, including unverified guesses |
| `docs/BATTLE_NOTES.md` | How battles work in the original, as read from its executable: addresses, field offsets, tables, the order things happen in |
| `docs/FIELD_NOTES.md` | The same for the field: the map scripts and their engine functions, characters and how they move, the enemies' behaviour, the hand-off to a battle |
| `game/m-brsu.iso` | The game disc image |
| `game/extracted/` | The disc image's files. The install data and firmware updater were removed as unused; `7z x game/m-brsu.iso -ogame/extracted` restores them |
| `game/unpacked/` | Output of `brs-extract` (generated; safe to delete) |
| `tools_re/` | Throwaway Python scripts used while working out the formats |
| `docs/compare/` | Pictures of the port beside the original, which the notes name. Kept locally, like everything under `game/`: they show the game's own art, so they are not in the repository |

## Running the extractor

```bash
cargo run --release -p brs-extract
```

With no arguments it reads `game/extracted/PSP_GAME/USRDIR/GAMEDATA` and writes to
`game/unpacked`. Pass an input and output directory to override either.

In the output, archives become folders and compressed files are written decompressed.
It ends with a report of how many files of each known format parsed, and why any did not.

Add `--png` to also write every texture as a PNG for looking at by hand: a standalone
texture gets a `.png` next to it, and textures found inside other files (models, maps,
effects) go into a `<file>.tex/` folder. That is about 330 MB in 29,000 files, and
nothing else needs them.

### Sound

```bash
cargo run --release -p brs-extract -- --sound
```

The game's music and voices are ATRAC3plus, which the port cannot decode itself. This
has [ffmpeg](https://ffmpeg.org) (which has to be on the `PATH`) make an Ogg Vorbis
file beside each of the 4,154 recordings under `game/unpacked`, about 125 MB in all.
It runs ffmpeg at idle priority, three at a time (`--jobs <n>` changes that), and
skips what is already there, so it can be stopped and run again. Without it the game
plays without sound and says which recording it could not play.

The same run makes an Ogg Vorbis file of the sound of each of the 36 films, in
`game/unpacked/MOVIE` (45 MB). A film's picture the game decodes itself, from the film
on the disc (`game/extracted/PSP_GAME/USRDIR/MOVIE`), with the OpenH264 decoder, which
is built from its source with the game.

## Running the game

```bash
cargo run -p brs-game
```

Once it is built, `play.cmd` in the project's folder starts it by a double click
(with the game's options after it, if any). The program itself
(`target\debug\brs-game.exe`) can be started from anywhere: when the folder it is
started in has no `game/unpacked`, it goes to the folder above itself that has.
`play.cmd` starts `target\debug\brs-play.exe` if there is one: a copy of the
program to play with, which a new build does not have to overwrite while it is
open. A game begun from the title plays its story scenes (start and then cross
skips one); a run that begins past the title skips them unless `--play-events`
is given.

With no options the game begins as the original does, on its title: the maker's
mark, PRESS START (start or cross), and the menu of NEW GAME, LOAD GAME, GALLERY,
OPTIONS and INSTALL (up and down move the cursor, cross chooses). NEW GAME begins a
game: the first map of stage 1, set up and run by the
map's own script from the game data, with battles fought when an enemy on the field
catches her. The controls are the PSP's, on a gamepad or the keyboard (the keys are
listed at the top of `crates/brs-game/src/controls.rs`). Game logic runs at the
original's 30 steps a second whatever the display rate.

### The title

- From the original (`TitleTask` and its layout in `TITLE.VOL`): the mark, which
  comes, stays two seconds (a button sends it away) and goes; the title coming
  over its ground of metal with its tune; the menu, whose cursor and entries move
  and sound as the layout's sets and the code have them; the title coming again
  when it has been left alone for a minute; and the other title a game that has
  been cleared has, a sky with clouds and another tune (`--cleared` shows it).
- NEW GAME leaves under the screen's fade in rows and starts where the original
  does: on the map of the story's opening, whose script plays it (the story so
  far in captions over staged shots, the first lesson and its battle) and goes
  on to the first mission. She begins as the original's start table has her for
  the opening, with every figure at its most (level 50, 9999 HP, another
  cannon, four skills), and the opening's script makes her a new game's when
  its film is over.
  OPTIONS is the menus' own screen, up alone as the title has it. From the
  screen between missions, circle (asked twice) comes back here.
- LOAD GAME lists the saves (below) and loads one.
- Left alone for a minute the title goes dark and plays the opening film, which
  cross or start ends, and comes again.
- GALLERY: the three lists of what the challenges have opened, as cards across
  the screen with their small pictures (left and right, L and R for the list).
  Cross looks at a picture (the D-pad or the stick moves it, triangle with up
  and down makes it larger and smaller, square puts it back, cross hides the
  strip, circle leaves), plays a tune or stops it, or plays a film again
  (cross or circle ends it). What is open is kept with the options, for every
  save alike.
- Left out for good: INSTALL does
  nothing (there is nothing to install on a PC, and its entry comes straight
  back), and the PSP's windows about the memory stick are not shown.
- `--map`, `--mission`, `--start` and `--stage-select` begin a run past the title,
  as before; `--title` begins on it whatever else is asked.

### The field

The stick or WASD runs, the arrow keys, right stick or dragging with the right mouse
button turn the camera, the scroll wheel zooms, cross looks at what she stands by,
turns a window's page and makes a choice (up and down move the cursor, circle goes
to "no"), L and R answer a question asked over the radio, triangle brings up the
MENU and start the pause. `--map STG01_02` starts on another
map, and `--mission 2` on another mission, on the map it begins on
(`tools_re/missions.py` lists them).

- From the original: the map and its collision; the map's script, run by an
  interpreter with the engine functions it calls for the game's progress (flags and
  variables), characters, traps, walls, fades, waits and battles; the characters'
  models, animations and speeds; how a character moves, turns, walks to a place,
  appears and is destroyed; that characters stand in each other's way (each a
  round of the field's radius, or the box a script gives a container, a barrier
  or an opened box), are turned about three axes and are as large as a script
  says; and the enemies, each run by the game's own behaviour
  script: they stand, notice her, cry out, chase, dash, give up when she leaves their
  ground, walk home, start the battle when they reach her, and stand down for a
  moment after one. Those a map's script has patrol go round the route of
  points it gives them, back to it after a chase, and their ground is about
  the route; those it has wander do that about where they belong.
- The things that are a map's own, each a model with its animations in the
  map's bundle (a gate that a mission opens): made, held shut or open and
  played by the map's script as any character is.
- Her jumps between the levels of a map, which is how the game gets her onto
  roofs, across gaps and up its tower: at a place a script marks, cross has
  her taken from point to point as the script lays them out, each in one of
  the original's three ways. A leap: she gathers herself, turning to face the
  way, dust rises where she springs from, she goes the straight way with the
  throw of a fall on top, timed by how far and how high it is, and lands in
  dust; a rush, with an aura about her; or flying straight up a shaft, lying
  in the air with the same aura. The camera is the script's the while, and
  the script waits for her to land. 257 such points in 25 maps. Where one
  starts, a small round with turning corners marks the spot while she is
  near, as in the original.
- The last stage's floors, which hang in the air at every angle: the field's
  sixteen "frames", each a turn that everything in it goes by. She, the enemies
  and what pops up stand and walk on the floor of their frame, a pad's leap
  takes her from one to another, turning her over in the air (a tenth of the
  gravity, timed by the distance alone), the traps' boxes lie as their frame
  does, and every kind of camera is turned with her. Only the stick is a
  stand-in there: up is away from the camera, over her own ground.
- The field's system script, run as it is: what starting, clearing and failing a
  mission do, what follows every battle, the mission's clock, and the enemies that
  pop up out of the ground ahead of her in the missions that have them.
- The mission's display, as the original keeps it and draws it: the line that says
  what is to be done and its numbers ("Defeat all the Eaters 0 / 6"), the notice a
  mission starts with ("BEGIN MISSION" and its title, with its jingle), and her HP
  gauge, blue, amber and red as her HP falls, all from the game's own layouts and
  hidden when its scripts hide them.
- The sign that names a place, in the letters of its own sheet: the name of the
  place she comes into at the foot of the screen on the left ("THE END OF ANKH
  ST."), which stands a while and then goes a few letters at a time; and the
  title a story scene opens with, typed out over the dark behind a blinking star
  with a tick for each letter ("25 SEPTEMBER:2051", "37.77-122.41", "SAN
  FRANCISCO"). The title was checked against the walkthrough frame by frame.
- The sign of what a button would do: a ring with the cross in it and ACTION,
  CHECK or TALK, which comes up over her when she is by something there is
  something to do with: people can be talked to, home points and boxes checked,
  and the places a script means to be worked by a button wait for cross
  instead of going off as she walks into them, some only while she faces the
  right way.
- The map of the stage, which square brings up over the field: the stage's
  places as the save has them, the ones the mission has shown light and the
  rest dark, the one she is on pulsing blue, the home points' pins, the mark
  of where the mission is and the names (G.G.GATE, CISCO PORTAL, TRAILER), all
  set by the maps' own scripts; square or circle closes it. It was checked
  against the walkthrough.
- An enemy that notices her cries out, and a red exclamation mark pops up over
  its head, as big as the enemy is on the screen. Its place, its size and its
  27 frames agree with the walkthrough, as do the place and the coming of the
  sign of what a button would do.
- A mission cleared, as the original has it: MISSION COMPLETE comes in over the
  field under the jingle of a battle won, goes up to head the screen of the
  mission's results, and under it are the mission's title and the challenges
  that were cleared in it, four at a time with up and down for more; cross
  goes on. It was checked against the walkthrough frame by frame.
- A mission failed, as the original has it: a battle lost goes back to the field,
  whose script has her told off over the radio and put down, and then the
  original's window comes over the field as it stood, with its jingle: "MISSION
  FAILED" and the choices to try the mission again, to go and choose another
  ("STAGE SELECT", out of reach until the first mission has been cleared) or to
  load a game. Up and down move the cursor (held, they go on moving it, as the
  original's pad has them), cross chooses. "LOAD" brings up the list of saves.
- The camera, from the original's own: the figures scripts give it (where it
  stands from the point over her that it looks at, its angles, how wide it sees),
  moved over time as they ask, kept in by the walls the map has for it and getting
  to its place with the original's lag; the camera that stays where a script puts
  it, which is what the opening's talk over the radio is shot with; the ones that
  look at her from a point; and the one that goes along a path of the map's as she
  goes along the ground under it, which is how most of the game's set views are
  made. The traps of a map give the camera its figures and its kind while she is
  inside them, as in the original: over the frames the trap asks for when she walks
  in or out, the view going over from one camera to the next, and at once after an
  event. After a battle the camera is a new one, as the original's is.
- The windows a script puts up: a lesson's pictures (the game's own, filling the
  screen) and its texts, page by page, with what the game marks out in the game's
  colour; the questions it asks (the terminal's "Save Game Data?"), answered with
  the cursor; the terminal making her whole; items given by the script. The
  window is the original's: its layout and its motions as it opens, grows by the
  line and closes, the game's font, its sounds. A lesson's pictures cover what the
  field shows of the mission and of her, as in the original, whose order of
  drawing the port keeps; and a script can darken the scene alone under all of it.
- The messages people send her as she goes: each is an event of the game with its
  own script, run as it is, through the original's window along the top of the
  screen: a face at each end coming in and going out, the lines and the noise
  over them, the speaker's name, what is said in the game's font for as long as its
  recording lasts (cross cuts it short), and the two answers of a line that asks,
  given with L and R.
- The interface is drawn on a screen of 480 by 272 of its own, scaled to the
  window with its pixels kept whole, and mixed with the scene as the PSP mixes: on
  the values of the pixels, which is what makes a dark panel as dark as it was.
  What the original draws smaller than it is made (the mission's line, written
  at two thirds of the font's size; a mark over an enemy far off) is smoothed, as
  the original smooths everything: its textures count on it, a glow being made
  of dots that only so come out even.
- The scene's colours are the PSP's too: its textures and its baked lighting are
  multiplied as numbers (a map's lighting can double a texture's brightness), mixed
  with the fog and with what is behind as numbers, and shown without any mapping.
  Each map's fog is the one its own scene script sets. A frame of the first
  mission's start has the walkthrough's colours to within about one part in a
  hundred wherever it was measured.
- Going on from one map to the next through the ways out the map's script sets
  up, as the original does it: the street she leaves stays on the screen as it
  was, the next map shows through it over a second, and its name is up at once.
- From mission to mission, on the original's screen for it (STAGE SELECT), from
  the game's own layout and run by its own script: when a mission is cleared
  the system script opens what follows from it and says so in a window; the
  world's map lies behind with the stage shown at its middle and the other
  stages' marks on it, and she stands before it, in what she has on, seen by
  the screen's own camera. Up and down go over SAVE, LOAD, the stage and MENU;
  left and right change the stage, the map sliding to it; cross on the stage
  lists its missions, each with its mark (not begun, cleared), its kind, the
  level it is meant for, what is to be done, a hint, and its briefing beside
  the face of who it is from; cross on one asks "Begin Mission?", and "Yes"
  has the camera close in on her, the screen wiped, and the mission start on
  its own map. Cross on MENU brings up the MENU, and circle asks, twice, about
  going back to the title, which then comes. `--stage-select` begins a run
  there. The first
  mission can be played from its opening to its end this way, and the second
  begun. SAVE and LOAD list the saves (below). (Not yet: the
  effect that goes with her as a mission is begun.)
- Saving: twenty saves, as the original has, kept as `save/DATA00.sav` to
  `DATA19.sav` beside where the game is run from (`--saves <folder>` names
  another place). The original hands over to the PSP's own dialog for it; the
  port lists the saves on a screen of its own that says what that dialog
  says of each: the game's title, when it was written, her level, the time
  played and where it was saved. A game can be saved at a terminal on the field (it asks "Save Game
  Data?") and on the screen between missions, and loaded there, with LOAD GAME
  on the title, and with LOAD after a mission failed. A game saved on the
  field comes back on its map with her where she stood, facing as she faced,
  and the camera turned as it was. The options are kept
  from run to run (`save/SYSTEM.sav`).
- A battle walked into is fought on the battle map that goes with the field map,
  against the group the script names, and its outcome goes back to the script, which
  takes the enemy away and counts the mission on. She comes out of it with the HP
  she had left, the experience and the items it was worth, and the levels that
  experience comes to; all of that stays with her into the next battle.
- Story scenes, with `--play-events`: the camera's cuts, the cast's motions, the
  captions, the scene's tune and its voices, timed as the script times them, one
  shot dissolving into the next where the script has it, and a scene's last shot
  standing until what follows is ready. A caption waits for a button (any of
  the face or shoulder buttons) with its sign at the foot of the screen, as in
  a new game of the original, unless the scene is cut to its voices or the
  menu's OPTIONS has AUTO SCROLL on. Without the option each scene is skipped
  the way the original skips one when asked to, as soon as it can be. A scene's
  effects are in it: the scripts make them as characters of their own (smoke,
  fire, the flash at a gun's muzzle fastened to the gun), which are put, turned
  and sized as any character is, and the port plays each one's effect there.
  Mouths move while their characters speak and stop with the voice, and eyes
  blink, as the scripts ask (a jaw and a pair of lids turned on the head); and
  heads turn as the scripts have them, over the frames and with the easing
  they say, or to look at whoever is spoken to, as far round and as far up
  and down as the game lets a head go. What an event gives somebody to hold
  is in the hand it is given to. A motion a script starts keeps its character busy until it has
  played through, so that a scene waits for it, and the cast then stands again;
  the camera shakes when a script says. The people who stand about a map look
  about them when they have stood a while (four to eight seconds, as the game's
  figures say), and not while a scene has them.
- The screen's fade and the picture it keeps, as the original has them: black and
  white sheets, and a picture of the screen that stands while something is made
  ready under it and then goes. That is how a battle begins (the field's picture,
  with all it shows, goes over the battle that has come up under it; a battle of
  note comes out of white) and how it ends (its last picture, the results, goes
  over the field, whose own display is already there).
- Sound: the tune each map's script asks for, the tunes of scenes and of the battles
  that have one of their own, what is spoken in scenes and over the radio, the
  jingles; the sounds of the banks that scripts play (a scene's noises, the sounds
  placed about a map, louder as she nears them and to the side they are on); her
  steps, by the kind of ground under her, the sound of looking at something and
  the one a battle begins with; with the original's fades, and the volumes the
  options give them (in a new game the music at under half of the voices; the
  menu's OPTIONS changes them). Skipping a
  scene stops what it was sounding, as it does in the original. `--mute` plays it
  all as silence.
- The ride on the trike in the second stage, which is a game of its own
  (`MINI_GAME`, `crates/brs-game/src/bike`, `docs/BIKE_NOTES.md`): where the story
  goes to it the field is put by, the ride is ridden, and the field starts anew on
  the map it leads to with how it came out. Its road of five lanes, what is met on
  it and the scripts that run it all, the rules (left and right change lanes, a
  change of lanes close by something destroys it and fills the gauge for her
  special on square, three gears, the time), its own camera, fog and light, its
  display (the dial, her life, the way come, the signs), its sounds, voices, tune
  and effects, the talk over the radio and the pause (START; triangle gives the
  ride up). `--ride easy` rides a course alone: `easy`, `normal` and `hard` are
  the story's three, `ex00` (over the bridge), `ex01`, `ex_easy`, `ex_normal` and
  `ex_hard` the others.
- The round shadow on the ground under every character of the field, and under
  what is on the road in the ride (`crates/brs-game/src/shadow.rs`); in a
  battle under her and under every enemy, as its script asks
  (`crates/brs-game/src/battle/shadows.rs`).
- A model's materials are put on the screen as their own lists have the PSP do
  it (`crates/brs-engine/src/lit.rs`): solid, mixed by how much of a thing
  there is (and still hiding what is behind it), or added, as lights and glows
  are; with a picture or without one (the blue arrows that show the way have
  none); and a character can fade (`Lit::alpha`).
- She pulls up after a run (let go of the stick, or sent back the way she
  came, after more than ten frames of running), and trudges through the third
  stage's storm at a thirty-second of a run.
- A push of the stick held across a cut of the camera carries her on the way
  she was going, and comes round to the new view over 45 frames.
- The faces of the story's people change as the scenes' scripts have them (her
  own included): each face is a pose of the character's own animation of faces,
  put on at once or come over to in the frames the script gives.
- Her two tails of hair swing: each is a chain of points hung from her head
  that is late in following her, comes back to where her animation has it and
  is slowed by the air, as her script describes it (`brs_engine::chain`); they
  trail as she runs and settle over four seconds of standing.
- A character stands, walks and runs in the animations a script gives it, where
  one does, and the things a scene stands about have no shadow where the script
  says so.
- Every one of the 81 settings the maps' scripts make of a character is
  accounted for (`tools_re/out/_settings_check.py`): among the last, one whose
  time stands still (the fallen of a scene), one that does not idle, a shadow
  put off its place, the sign at a container's foot, and the tile that marks
  the enemy a mission is after.
- The round shadows lie on the ground under whoever casts them, tilted with it,
  stay there while she leaps, and are as large as each character's own script
  says.
- The camera sees from as near as the scene's script says (50 unless it says:
  nearer for a close shot, further to see past what stands in the way), and
  the scene, its characters and its effects are drawn only as far as its
  script says (twenty thousand unless it says, which the maps' scripts do 236
  times): beyond that there is the sky and what stands far off, which have a
  lens of their own. The effects of the enemies of a story scene that ask for
  it are seen from right before the camera, as on the PSP.
- As on the PSP, a character more than 4000 from her stands still until she
  comes nearer, one more than 3000 from her is not drawn, and one the camera
  comes right up to (150, until a scene's script says otherwise) is left out
  so that the camera never looks out of its inside. A scene's cast is exempt,
  as its scripts ask.
- On the eight maps whose scenes ask for it, the camera that follows her leans
  its pitch with the ground under it, riding up where the ground behind her is
  higher than where she stands.
- The events that speak to her over the radio have everything their scripts
  call (`tools_re/fevt_coverage.py`: 64 of 71 functions, the other seven only
  in a test event that is never played), down to enemies coming up through a
  ring as the camera shows them.
- Not yet: the fades that move the picture kept, and a window's other sound of
  opening.
- A map is drawn as the PSP drew it: strip after strip in its own list's order,
  each mixed with what was drawn before it and hiding what comes after it and
  lies behind (a polished floor drawn first of its hall, over the colour the
  screen is cleared to, which is the scene's own), with the fog by depth; the
  sky and what is far off first, the map proper after it.
- The glare the story's scenes have: the screen made small and soft and added to
  itself, as strong as the scene's script says (the opening hall's colours are
  the walkthrough's with it, to two or three parts in 255).
- The light the field's characters stand in: each is lit as the PSP lit it, by the
  one of the scene's lights that the map's collision says falls where it stands
  (the first stage's streets have a white one for the open and a dim warm one
  for wherever a building's shadow is painted on the ground), by its own colours,
  which go over to that light's as it moves; the scenes of the story change the
  lights and put them back. What each material of a model makes of the light
  (whose colours are its corners', its shine, its picture taken twice over or
  added) is read from the model.
- The story's films: the opening and the films its events cut to are played
  where the scripts play them, picture and sound, over the screen as the PSP's
  own player has them, and stop with the scene when the pause skips it. A film
  is as loud as the options have the music, and the opening film is heard in
  full whatever they say.

`--skip-battles` counts every battle as won without fighting it, `--lose-battle 100`
counts a battle against that enemy group as lost, `--skip-messages` takes every
window as read and every caption that waits as passed (but not the window of a
mission failed, which waits for its choice), and `--start x,z` puts the player
somewhere else once the opening is
over; all of them are for checking the field's own flow.

### Battles

```bash
cargo run -p brs-game -- --battle STG01_01 --group 2
```

A battle can also be run alone: `--battle` names a battle map and `--group` one of
the game's 484 enemy groups (2, the default, is the first battle of the game: two
Eaters and two Hornets). `--level 20` gives the player the figures of that level
instead of a new game's, which the later groups need; it works for a game started on
the field too. `--afflict 602,1101` gives her and every enemy those status effects
(numbered as the game's table has them) as the first fight begins, for a look at
their marks and at what the log says of them. `--skills 25,1,27,29` puts those
skills (numbered as the game's table has them: Defender, Charge Shot, Regenerate
and Ignition) on triangle, circle, cross and square from the start, as the
menu's CUSTOMIZE does in a game, and `--auto 33,36,37` has her have skills that
act of themselves (30 to 38), which a game gets from its challenges. On the
field `--has 1,2,3,23,30,42` has her have those skills from the start, for
CUSTOMIZE to list, `--carry 1:3,6,132` has her carry those items (three of
the first), for ITEMS, and `--wear 132,136` has her have that weapon and that
suit on; `--flags 6157,6158,6600,6802` sets flags of the save and
`--variables 20000:3` gives variables of it from the start (the first two
stages seen, the first challenge unlocked with three counted and the third
cleared), for CHALLENGES. The stick or D-pad moves the
aim cursor, which settles on an enemy's lock point when let go near one; square fires
a burst, circle dodges to the other standing place, cross guards, and with R held
the button of a slot uses the skill in it.

Battles are ported from the original's code rather than imitated, piece by piece:

- The player: the camera over her shoulder, the aim cursor and lock-on, standing,
  shooting, guarding, dodging, overheating, reeling from a hit, the heat gauge, and
  the figures of a new game or of any level.
- Enemies: each one is run by its own script from the game data, through an
  interpreter and the engine functions the scripts call. Every enemy's script has
  all it asks for (all 88 of them, the bosses' 22 included): collision spheres,
  lock points and their gauges, every kind of move, attacks at close quarters, and
  bullets of six kinds: straight, guided, rays, beams, lobbed bombs, and strikes
  at the spot she stands on.
- The bosses: a boss's battle is begun by the boss's own script, which the port
  runs as the original does: the scene the boss is met in, with a camera move of
  its own and its line spoken with its mouth moving; the card that names the two
  of them; scenes in the middle of the fight (an attack, the change when its first
  HP is gone); and the finish, a gauge she fills by hammering cross while it
  drains, with the scenes around it and her own animations for them. The first
  two play through from the meeting to the battle's results
  (`--battle STG01_02 --group 3 --level 30`, `--battle STG02_02 --group 4
  --level 40`); groups 5 to 10 are the others. The first's meeting is framed as
  the walkthrough has it (`docs/compare/boss1_intro_port.png`, lower left, and
  `boss1_intro_walkthrough.jpg`).
- Enemies that come in later: a group's reserves, called in by the enemies' own
  scripts under the group's rule, at the places the group has for them.
- Damage: the game's formula and attack tables.
- Status effects, for both sides: poison, stun, the ailments that take away the dodge
  or the shot, attack and defence up and down, regeneration, heals, wards, attacks
  that drain or break a guard, and the cloud that keeps the cursor off an enemy. The
  chance of one taking counts the levels as the original does. Each that stays
  shows as the original's icon, by her gauges or over the enemy's bar, in the
  original's order (the one soonest over first), eight at a time for her and three
  for an enemy, a page of them every two seconds when there are more; and the
  battle's log says so as one is given and as its time runs out ("BRS Shocked
  (unable to perform any actions for a time).").
- The battle's course: the wait as it comes into view, the count of enemies left to
  beat, winning (she stands down, or strikes her victory pose with its own camera
  move, as the enemy group says), and losing (her fall, with its camera move).
- The display, from the game's own layouts and drawn as the original draws it: her
  HP and heat gauges, the count of enemies, the level and HP bar over each enemy,
  the aim cursor (which changes with her heat) and the mark on what she has locked
  on to, the column of her skills that comes in while R is held (each row with
  its skill's name, dim until the skill is ready again), and the line of
  information along the top, which is the battle's log: the enemies' own scripts
  write to it ("BRS has defeated Eater!").
- The results of a battle won, as the original shows them: the experience earned
  counted onto hers with the bar of her level and what is wanting to the next, the
  band that says a level was reached, and the items found with their icons. Cross
  has the count done at once; cross or circle then leaves them.
- What a battle is worth: each enemy's experience (none for those called in as
  reinforcements, half in six of the missions), the items they leave behind by the
  game's two rolls, the levels the experience comes to with each level's gains from
  the level table, and all her HP back on gaining a level. The items go into what she
  carries, up to each item's limit.
- The challenges, which are a script of the game's data that the port runs: it
  is told of every battle she comes out of, of the enemies she beats and of the
  levels she gains, and of what the maps' scripts open and count. What it has to
  tell is told as the original tells it: a panel at the foot of the screen for a
  challenge unlocked, for how far one has come and for one cleared, and after a
  battle's results the CHALLENGES screen for each one opened or cleared, with
  what a cleared one is worth said in a window and given. The first is worth
  the dodge, as in the original: she cannot dodge until her fourth battle of
  the first mission is won. (A battle run alone lets her from the start.)
- Her skills, from the game's table of them. Those that add to one of her
  figures do so from when a challenge gives them. Those of her four slots take
  their time to be ready again after a use. The thirteen that help her for a
  while (Aggressor, Defender, Absolute 0, Regenerate, Heat-Free, Skill Boost,
  Ignition and the better ones of each) are used as in the original: each is a
  scene of its own, with its camera move and her animation from the skill's
  archive, her line and its sounds at their times, the map dimmed behind her
  (to the strength and over the frames the walkthrough has, measured), and at
  its end what it gives her, which the log says and a mark by her gauges shows.
  A boost she has is spent by the next skill, and nothing of hers runs out
  while a scene lasts. All seventeen of the attacks: Charge Shot and Burst Shot (the
  scene, and then the shot over the battle, the game's own bullet for it flying
  straight through every enemy on its way); Tracer Gun and Chaser Gun (she
  takes the skill's own gun, and three or four guided bullets come round to
  what she had locked on to); Photon Shower, BB Genocide, Steel Cutter and
  War Hammer, whose blow lands on every enemy at once; Blade Kill and Iksa
  Blade, which need an enemy locked on to: she draws the blade at the middle
  of the map and cuts that enemy down where it stands; Vulcannon, Volcane
  and Vortex, which fire thirty-five rays over three and a half seconds at
  wherever she aims, the cursor free of the lock points meanwhile; Star
  Comet and Star Comet G, whose bomb is lobbed a fixed way towards where she
  aims, lies a second and a half and bursts over whoever has come near; and
  Stun Snipe and G-1 Snipe, whose scope closes in on what she had locked on to
  (the camera's lens, the sight with its lens and its lock, she herself out of
  the picture) and waits two seconds for square, the one hit of hers that is
  on the part of the enemy she chose; not fired, it costs nothing. The weapon
  of a skill is held in the hand the original fixes it to, and an enemy a
  skill ends is told to the challenges as that skill's. Those that act of
  themselves do, when what they wait for happens: Rocket Starter (one battle
  in four begins with her attack up), Dead Guard (her defence up once her HP
  is down to a quarter), Attack-Free and Sway Three (one burst or dodge in ten
  adds no heat), Danger Guard (a guard raised two frames before a hit lands
  lets a tenth of it through instead of half, and gives her a tenth of her HP)
  and Aftercare (a quarter of her HP back after some battles won). As in the
  original nothing names them in the log; Max Up never acts there either.
- The MENU that triangle brings up on the field, from the game's own layouts:
  the wipe of chequers it comes under, CUSTOMIZE, ITEMS, CHALLENGES and OPTIONS
  with the cursor, her level, experience, what is wanting to the next level and
  its bar, her attack, defence and special attack, all her HP and her HP gauge,
  and along the foot what the entry is for; and her model stands in it as in
  the original, with her cannon, in the menu's own pose, turned by L and R.
  Circle or triangle leaves it.
- CUSTOMIZE, the first of the screens behind its entries (cross), which comes
  and goes under the wipe: the skills she has, in the game's order, in two lists
  that L and R go between. Cross on a weapon skill has a slot chosen for it
  (triangle, square, circle or cross), or, if it is in one, a window ask whether
  it is to come out; cross on a passive skill switches it off or on, and her
  figures follow at once. Six rows that scroll, with the bar for it; what the
  cursor's skill does; her model in the screen's own pose. Circle goes back to
  the MENU, triangle out of the menus.
- ITEMS, the second: what she carries, in three lists (consumables, equipment,
  rare items) in the game's order, with how many of each. Cross on a Repairer
  or a Nano Repair uses it, after the window has asked, and says how much HP
  she got back (or that it would do nothing); what is for battles cannot be
  used there. Cross on a weapon or a suit changes to it: her attack, the
  bullets of a burst and the heat of a dodge follow, and beside a weapon her
  attack is shown with what it would be. Her model wears what the cursor is
  on: the four cannons and the six suits. Her HP gauge is under the first
  list.
- CHALLENGES, the third: the challenges of a stage at a time, sixteen squares
  in two rows of eight that the D-pad goes over, with the stages she has seen
  along the top for L and R to go through (the one of the map she is on
  first) and how many of the sixteen are cleared. One not unlocked yet is a
  square that says UNKNOWN with its number, and all that is told of it is what
  unlocks it; one unlocked has its picture, what is to be done for it and
  what it is worth, with how far its count has come as a bar and in
  hundredths; one cleared has its picture in colour and says CLEAR.
- OPTIONS, the fourth: how loud the music, the sound effects and the voices
  are, each a bar that left and right move (a step at a time, then quickly
  when held), heard at once; cross plays something to judge the sounds and
  the voices by. AUTO SCROLL says whether the captions of a scene go on of
  themselves or wait for a button, which is how a new game has them (the
  scenes that are cut to their voices go on regardless). Square asks whether
  everything is to be as a new game has it. DATA INSTALL is there and cannot
  be switched on. The options are kept from one run to the next, with the
  saves.
- The pause that start brings up, from the game's own layout: a white sheet
  over the game as it stood, the word PAUSE, and nothing moving or sounding
  under it but its own sounds. On the field it lists the challenges whose
  counts have gone up, four at a time with up and down to move the list: each
  with its small picture, its name, how far it has come in hundredths with a
  bar of that, or CLEAR. Triangle there asks whether the mission is to be
  given up, and "Yes" goes to the screen between missions as the screen goes
  dark; square puts up the list of saves to load a game from; start goes on.
  Over a story scene it offers cross to skip the scene instead, and in a
  battle there is start and nothing else.
- What she has on is what she is seen in everywhere: the field and a battle
  load the suit's model and hang the weapon's on her hand, as the menus do
  (`--wear 132,136` starts a run in the Schoolgirl suit with the Hyperion).
- The effects of a battle, played by a port of the game's own effect library
  from the effect files of the enemies', her skills' and the system's archives:
  the flash and the glowing shot of her cannon, her dodge's streaks, her gun
  overheating, the looks of the enemies' bullets with what is seen at their
  muzzles, what the enemies' scripts ask for, what is seen of a hit on an enemy
  or on her (and of a guarded one), of a status effect given, and everything
  the scenes of her thirty skills make: the flame at her eye, the glow of a
  weapon coming to her hand, the speed lines and flashes that are fixed to the
  screen, and the sky Photon Shower has in place of the map. Each is given its
  time and drawn in the draw list the original has it in. The numbers a hit
  takes off (or a heal gives back) go up where it lands, and the camera shakes
  as the original's does for a shot, a hit, a bomb and what a script asks.
- On the field, the effects of its own: the ring an enemy comes up through, the
  smoke and the landing of the two other ways it has of coming, the burst a
  beaten enemy goes in (with the shake of the field's camera and its sound),
  the smoke a broken box goes in; and what a battle of note begins with, over
  the field's picture, before the screen goes white. And the effects the maps'
  scripts make: the ring of light about a home point, what swirls about her as
  it heals her, a yellow terminal's glow, and everything of a story scene.
- Her items: L brings up the original's window of what she carries that can be
  used in a battle, and the battle stands still for it; up and down, cross to use
  one, circle or L to leave. The Repairers and Nano Repairs give HP back (half as
  much again when Six Pocket acts), the Charges have a slot's skill ready at
  once, Body Cooler takes her heat away, and the rest cure her or boost her.
- Nothing is left out there on purpose but what the game never does: a dash
  of hers on the field that nothing begins, and a light for a battle's
  characters, which the game does not give them. Sound is in: her shots and dodges,
  what the enemies' scripts play (at the pitch they ask), bullets, bombs, hits and
  guarded hits, a heal's or a boon's sound as it takes, her line as she falls; the
  tune the field's script chose plays
  through the battle, as in the original, and the results of a battle against a
  group she strikes her pose over begin with the tune of a battle won, the music
  turned down under it. A warning names anything a
  script asks for that is not ported, the first time it is asked.

`tools_re/groups_with.py <enemy number>` finds the groups an enemy is in,
`tools_re/enemy_variants.py` lists the enemies by the names the game gives them, with
what each is worth, and `tools_re/items.py` lists the items.

### Checking changes without the keyboard

`--script wait:1.2,right:0.1,wait:0.3,square:0.1,wait:3` plays scripted input (the
names are in `controls.rs`), `--screenshot out.png` saves a frame when the script
ends and exits, and `--verbose` logs what the game's logic does: every change in what
an enemy's script is up to, every hit, every bullet fired. In a battle the name
`seek` works the stick to bring the cursor to the nearest lock point (`seek2` to that
of enemy 2, as the log numbers them); a burst is fired on the press of square, so a
script that is to keep firing has to press again and again:
`seek+square:0.1,seek:1.3` repeated. On the field `seek` runs her to the nearest
enemy, `seek502` to the character the map's script knows by that number, and
`to-400/2200` to that place; `--screenshot-at 20,45,60` saves a frame at each of
those seconds (`out_1.png`, `out_2.png`, ...). `docs/FIELD_NOTES.md` has a script that
plays the whole first mission.

A run that saves pictures opens **no window**: it draws to a picture in memory
(1280 by 720) and saves from that, so it can go on while the machine is used for
something else, a game in full screen included. `--background` does the same for a
run that saves nothing. The viewer works the same way. Such a run puts **no sound
out** either; with `--mute` it plays everything as silence instead, through the
sound device, which checks the way sound takes without anything to hear.

What only a real window shows has to be checked in one: `--window` has a run
that saves pictures open its window after all (the pictures are then of the
window). The scene is drawn to a picture of the window's size and that picture
shown by the canvas, so a change of the window's size is the thing to try:
maximising it on the title once left the scene black behind the interface from
then on, which no run without a window could show.

```bash
python tools_re/trial.py 411 --level 25 --seconds 40
```

runs a battle against a group unattended and sums up its log: the states each
enemy's script went through, the moves it made, the bullets fired, the hits either
way, and every warning. `tools_re/bscr_coverage.py` tells what each enemy's script
still needs that the port does not have.

## Running the model viewer

```bash
cargo run -p brs-viewer -- game/unpacked/BTL/CHR/BCHR_APL00.VOL/chr_m_apl00.mdl
```

Drag with the left mouse button to orbit and scroll to zoom. Any `.mdl` file works, as
do the `.ZZZ` files in `BTL/WEP` and `BTL/DBG`. Models are drawn unlit and double-sided,
because the game's lighting and blending are not decoded yet.

To play an animation, add `--anim` with an `.anm` file from the same archive folder:

```bash
cargo run -p brs-viewer -- game/unpacked/BTL/CHR/BCHR_APL00.VOL/chr_m_apl00.mdl --anim game/unpacked/BTL/CHR/BCHR_APL00.VOL/chr_m_apl00_btl0000.anm
```

Other options: `--time <seconds>` freezes the animation at that point, `--parts
60,71` shows only those of a model's switchable parts, `--camera <file.cam>` looks
through one of the game's camera moves instead of orbiting (the player's victory pose
is `chr_p_brs02_btl8010_all.anm` with `chr_p_brs02_btl8010_cam.cam`, both in
`SYSTEM.VOL/pack.vol`), `--distance 2` stands the orbiting camera twice as far off,
and `--screenshot out.png` saves one frame and exits, without opening a window.

An event scene's animations play on the field characters' models: `python
tools_re/fevt_cast.py FEVT_1_1_0200B_1` lists who is in a scene's slots and the
motions it brings for each, as in

```bash
cargo run -p brs-viewer -- game/unpacked/FLD/FCHR/FCHR_S_MOB00.BIN/01.bin/00.insm --anim game/unpacked/FLD/FEVT/FEVT_1_1_0200B_1.BIN/01.bin/03.bin/21.insa
```

The viewer also opens map geometry (`.stcm` files, and the `map_b_*.bin` files in
`BTL/MAP`). Maps use a free-flying camera: hold the right mouse button to look around,
WASD to move, Q/E for down/up, Shift to go faster, and the scroll wheel to change speed.
`--collision <file>` overlays a collision file on the map, and can be given more than
once. `--pitch` and `--yaw` set the starting camera angle in degrees:

```bash
cargo run -p brs-viewer -- game/unpacked/FLD/MAP/MAP_D_STG01_01.BIN/11.bin/01.bin/00.stcm --pitch 35
```

## Format status

| Format | Used for | Status |
| --- | --- | --- |
| gzip, `CZAA` | Compression wrappers | Done |
| `RTDP` (`.VOL`) | Named archive, contents XOR 0x55 | Done |
| `LPK` | Unnamed archive | Done |
| `PDK` (`MINI_GAME/BMG_*.BIN`) | Numbered archive of the ride on the bike | Done: unpacked, and read as it is where it was unpacked before the extractor knew it (`docs/BIKE_NOTES.md`) |
| `PTMD` | Textures | Done: all 29,427 in the game decode |
| `SC` | Field bundles (maps, characters, events) | Done: unpacked recursively into numbered slots |
| `STCM` | Field and battle map geometry | Done for geometry and textures: all 204 parse and display. Blend state not decoded |
| `CLIM` | Field map collision | Done: all 123 parse; used by the game for walking and the camera |
| `CPMD` | The maps of the camera's paths on field maps | Done: all 91 parse, with their camera rigs and moves; the game's camera goes along them |
| `INSM` | Models | Done: all 363 models parse and display in `brs-viewer`, with skinning. Lighting and blend state not decoded |
| `INSM` (`.cam`) | Camera moves | Done: all 263 parse; played in battle and in `brs-viewer --camera` |
| `INSA` | Animations | Done for the nodes: all 3,459 parse and are posed the way the original's evaluator does it (characters, event scenes, objects with pivots; fitting to another skeleton, matching nodes by id, blending). Material tracks not decoded |
| `SSCR` | Character, event, behaviour and system scripts, the field's tables | Loader and interpreter done: all 1,597 parse. Run for the characters' speeds, the behaviour of the enemies on the field, the field's system script and the field's events; `tools_re/sscr_dec.py` decompiles one |
| `bscr`, `.bms` | Battle scripts (enemies, battle maps) and field map scripts | Loader and interpreter done: all 203 parse. Of the enemies' 126 engine functions 62 are ported, which is all that the ordinary enemies use; of the map scripts' 281, those for progress, characters, traps, walls, fades, waits and battles |
| `BXCB` | Battle tables (tunables, attacks, bullets, enemy variants) | Done: all 183 parse; the ones a battle uses are read into typed tables |
| `EGTB` | Enemy groups | Done |
| `XTC` | Items, equipment, levels, starting figures | Done: all 34 parse; the port reads the starting figures, the level table and the three item tables |
| `EDXD` | Per-enemy figures | Done: all 88 parse |
| `EFP`, `EFC` | Effects | Done: all 730 files parse (1,546 effects, 7,628 emitters); the library that plays them is ported and they are drawn, in battles and in `brs-viewer <file> --effect <number>` |
| `RIFF` AT3 (`.AT3`, `.at3`, `.riff`) | Music, voices, jingles (ATRAC3plus) | Done: all 4,154 are read for their length and loop, and played from the Ogg Vorbis files `brs-extract --sound` has ffmpeg make |
| `PPHD8` (`.phd` + `.pbd`, `.pphd`) | Sound banks | Done: all 248 parse and all 779 sounds decode (ADPCM), with their loops |
| `MOVIE/*.PMF` | Films (PSMF: H.264 and ATRAC3plus in an MPEG program stream) | Done: all 36 are taken apart; the picture is decoded as it plays (OpenH264), the sound played from the Ogg Vorbis file `brs-extract --sound` makes of it |
| `INSDIR/INSTDAT.VLD` | Install data | Not needed: a scrambled copy of files already in `GAMEDATA` |

Format details are in `docs/FORMAT_NOTES.md` and in the source of `crates/brs-formats`.

Known assumption in the texture decoder: textures narrower than 16 bytes per row or
shorter than 8 rows are treated as not swizzled. This has not been checked by eye.
