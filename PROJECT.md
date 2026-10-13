# Where the port stands

The aim is the PSP game as it is, one to one: every part is written from the
original program's own code (read from its disassembly; `docs/*_NOTES.md` say
what was read, with the addresses) and then looked at beside a walkthrough of
the original. This sheet is kept up as the work goes: `[x]` is done, `[ ]` is
still to do. "Tried" means run in the port; "compared" means looked at beside the
walkthrough.

Last brought up to date: 9 October 2026.

## Done

### The whole game, start to end
- [x] Title, new game, load, options, gallery
- [x] The stage select between missions, with her model, its camera and the
      flame at her eye as a mission is left for
- [x] All 66 missions start and run their openings (swept unattended, no faults)
- [x] The field: running, jumping between levels, traps, doors, terminals,
      containers' signs, enemies that roam, notice and chase, enemies that pop up
- [x] Story scenes: cameras, faces, necks, held things, effects, films, voices
- [x] Battles: aiming, shooting, dodging, guarding, heat, all 30 slot skills,
      items, the finish, results, a battle lost
- [x] All seven bosses
- [x] The ride on the bike (all courses), with its sounds, effects and talk
- [x] The MENU and its four screens, the pause, the map of the stage
- [x] Mission cleared, mission failed, the records (challenges)
- [x] Saving and loading (the port's own save files)
- [x] The ending

### The field's finer rules (9 October 2026)
- [x] The camera's three lenses: the scene, its characters, effects and shadows
      are cut at the scene's own near and far distances (tried with forced figures)
- [x] Characters out of her reach: no frame beyond 4000 from her, not drawn
      beyond 3000 or right at the camera; a scene's cast exempt (swept, 66 missions)
- [x] A mission's time running out, or what she guards being lost, starts the
      map script's own callback (0x10D6BC)
- [x] What she guards loses ten a second for each enemy within 300 of it
      (0x10D370; tried with the system script itself in a test)
- [x] A guard mission tried in play to its loss: the bike's gauge runs down
      while enemies stand by it, then MISSION FAILED (`STG02_09`)
- [x] A timed mission tried in play to its time-out: the clock counts down and
      the map's script takes over when it ends (mission 28)

### Battles' finer rules
- [x] Her HP gauge flashes for sixteen frames when a hit takes HP off her
      (0x3DB94; not for hits on enemies, which the walkthrough bears out)

- [x] The sound of a break: as a part of an enemy breaks, and as an attack that
      breaks guards goes through one (0x85BEC, 0x8FA88); not yet heard in play
      (no enemy was stunned in two test fights)

### Setting up and sharing
- [x] On GitHub, code only: none of the game's data is in the repository
- [x] `brs-setup`: drop the .iso on it, choose a folder, and the game is made
      there, sound included (tried whole: 3 minutes, 1.4 GB)
- [x] GitHub builds the Windows bundle itself, and release `v0.1.0` has it

## Seen in play (reported 13 October 2026)

- [x] Bars at the left and right in cutscenes, and the level showing beside a
      still picture or through a fade: the 3D picture filled the whole window,
      which is a little wider than the PSP's screen, and some things covered
      only the screen. Now the world is drawn in the screen's own shape and
      all beside it is black, in every mode and any shape of window. (Tried
      unattended; to be confirmed in play.)
- [x] Characters gone for a split second: the rule that leaves out a model the
      camera is right on was asked with the camera where the frame before had
      it, so a cut could hide someone for a moment. Now asked once the camera
      is placed. (To be confirmed in play.)
- [x] Feet slipping and animations jumping: places were drawn behind the
      game's step and poses ahead of it. The world is now drawn where each of
      the game's thirty steps a second leaves it, as the PSP draws it, at any
      display rate. (Measured in bursts of pictures; to be confirmed in play.)
- [ ] A tail of hers lashing across the screen just after a cut
      (`docs/compare/tail_lash_after_cut_port.png`, about 126 s into the
      opening story). Her tails are chains kept in the world, and a script
      that puts her somewhere else leaves them behind for a moment. The
      original's code has nothing that puts them back either, so what it shows
      there has to be looked at in the walkthrough before this is changed.
- [ ] Mouths that sometimes do not move while someone speaks: not yet seen in
      my own runs; to be looked for with the voices playing.
- [ ] "Model bits bugging out" other than the tails: nothing else found yet.

## Still to do

### Gameplay not yet proven in play
- [ ] A guard mission played through to its win
- [ ] The other timed maps (`STG04_0A` gives 300 seconds)
- [ ] The "mazuma" missions' counter (the system script's own; not looked at in play)

### Remarks in the notes still marked "not ported", to be sorted one by one
- [ ] The states and flags the field is not free in
- [ ] A picture kept and drawn moved about (picture kind 0x800)
- [x] Names of items from 500 on in a text; text code 0x1A: no text in the
      data uses either, the records' rewards have their words by another way,
      and the port warns if a script ever asks for such a name
- [x] The window that opens with sound 7, and choice sound 5: of no window a
      map's script makes
- [x] The half fades and the wipe of the screen's fade: the save tasks' and the
      title's alone; the title has its own, the save list its own sheet
- [ ] The port's windows have their frame before the script's, the original's
      after it (a frame's difference at the most)
- [x] The field's flag 0x55 and the jingle's stream at a mission cleared: the
      flag is the window's being there, and the jingle's file has nowhere to
      go round from, so it is heard once as in the port
- [x] A light's kind, place and angles kept from the map before: no script
      leaves them unsaid for a light that lights anyone
- [x] The order of drawing a script gives the effects of a scene (setting 123;
      tried, not yet compared with the walkthrough's opening)
- [ ] Which model kinds are not asked how near the camera they are
- [x] Battle: the sight of the skills that snipe and the log's lines: looked
      into, all in (the one line never written, "No longer Overheated.", the
      original never writes either)
- [ ] Battle: the enemies' marks while she has the stage and some of them are
      shown again; the depth figures of the scope's camera
- [ ] Battle: an enemy's effects standing still in hit stop
- [ ] Battle: the effect's show of a status effect given (its sound is in)
- [ ] Battle: how the picture is drawn while the effect of a battle of note
      plays (a strip of triangles from the picture kept, 0x11AF48)
- [ ] Battle: the sky's own near and far planes (no boss's script asks for them)
- [ ] Menu: the sign of loading and the frozen picture (`MENU_NOTES.md`)
- [ ] `FORMAT_NOTES.md`, "Not done"
- [ ] What the field's flag 0x50 does to the camera (misread once; to be read again)

### To be compared with the walkthrough
- [ ] The gauge's flash at a hit on her (a moment she is hit has to be found)
- [ ] The flame at her eye on the stage select
- [ ] The timing of the bosses' moves
- [ ] The guard's display ("Bike endurance" and its gauge): not found in the
      walkthrough yet (mission 22 is at 6740 to 7869 s; 7300 to 7780 was looked
      through at twenty-second steps, with advertisements on almost every jump)
- [ ] A timed mission's clock: mission 28 is not one of the story's, so the
      walkthrough may not have it; another timed mission has to be looked for

### For a person to try (cannot be done unattended)
- [ ] The release bundle from GitHub, unzipped and used
- [ ] `brs-setup`'s two windows (choose the .iso, choose the folder)
- [ ] `brs-setup` fetching ffmpeg on a PC that has none

### Known difference
- The PSP's menus are drawn sixty times a second and its world thirty. The
  port draws its window as often as the display allows and moves the world
  thirty times a second, so the menus are at least as smooth as the PSP's.

## Left out on purpose
- The PSP's data install and its memory-stick windows (nothing to install on a PC)
- Things the original's code has and the game never does: see
  `docs/BRS_cut_features.pdf` (the field dash, containers a dash would open, a gun
  emplacement no map has, a test event). The dash could be rebuilt as an extra
  that is off unless asked for; not asked for yet.
- Reading a save made on a PSP (its files are encrypted; the port has its own saves)
