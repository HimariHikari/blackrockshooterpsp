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

### Setting up and sharing
- [x] On GitHub, code only: none of the game's data is in the repository
- [x] `brs-setup`: drop the .iso on it, choose a folder, and the game is made
      there, sound included (tried whole: 3 minutes, 1.4 GB)
- [x] GitHub builds the Windows bundle itself (first build passed)

## Still to do

### Gameplay not yet proven in play
- [ ] A guard mission played through to its win
- [ ] The other timed maps (`STG04_0A` gives 300 seconds)
- [ ] The "mazuma" missions' counter (the system script's own; not looked at in play)

### Remarks in the notes still marked "not ported", to be sorted one by one
- [ ] The flash when she is hit on the field
- [ ] The states and flags the field is not free in
- [ ] A picture kept and drawn moved about (picture kind 0x800)
- [ ] Names of items from 500 on in a text; text code 0x1A
- [ ] The window that opens with sound 7, and choice sound 5
- [ ] The half fades and the wipe of the screen's fade
- [ ] The field's flag 0x55 and the jingle's stream at a mission cleared
- [ ] A light's kind, place and angles kept from the map before
- [ ] The order of drawing of characters (setting 123)
- [ ] Which model kinds are not asked how near the camera they are
- [ ] Battle: the sight of two skills, the log's lines of her own states
- [ ] Battle: an enemy's effects standing still in hit stop
- [ ] Battle: the remarks at about lines 762, 1743, 2363, 2471, 2558, 2639 and
      3054 of `BATTLE_NOTES.md`
- [ ] Menu: the sign of loading and the frozen picture (`MENU_NOTES.md`)
- [ ] `FORMAT_NOTES.md`, "Not done"
- [ ] What the field's flag 0x50 does to the camera (misread once; to be read again)

### To be compared with the walkthrough
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

## Left out on purpose
- The PSP's data install and its memory-stick windows (nothing to install on a PC)
- Things the original's code has and the game never does: see
  `docs/BRS_cut_features.pdf` (the field dash, containers a dash would open, a gun
  emplacement no map has, a test event). The dash could be rebuilt as an extra
  that is off unless asked for; not asked for yet.
- Reading a save made on a PSP (its files are encrypted; the port has its own saves)
