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
- [x] A big dark shape sweeping across the screen about two minutes into the
      opening story (`docs/compare/tail_lash_after_cut_port.png`): looked
      into, and no fault. It is her cannon coming down past the camera as she
      lowers it; the scene's script starts that shot with the camera seeing
      from ten units on, for just that. Her tails are switched off for the
      whole of that scene and follow its own animation. (Still worth a look
      beside the walkthrough.)
- [x] Things coming a frame or more late the first time they are drawn (a
      model as it first appears, the parts of a model drawn another way than
      the rest, a scene's smoke at a cut): the graphics card was being made
      ready for each new way of drawing behind the game's back, and until it
      was, what is drawn that way was simply not there. Found with a check
      that photographs every frame about each camera cut and looks for one
      that does not belong, and settled against the walkthrough frame by
      frame (the smoke of the opening's shot at 276.0 s is there on its first
      frame; in the port it came on the second). Now the frame waits until
      the card is ready, so everything is there on its first frame. This is
      the likeliest cause of models and bits of models blinking on a real
      PC, where it takes longer than here. (To be confirmed in play.)
- [x] Everything skipping ahead after a hitch: when a frame took long (a
      scene being read, the card made ready for something new) the game ran
      all the steps the long frame was worth at once, up to a quarter of a
      second of them, which is seen as animation jumping. Now a long frame
      counts for one step and the game goes on from where it was, as the PSP
      slows when it has much to do; a film's picture and what is heard keep
      to the clock on the wall. (Cannot be seen in an unattended run, which
      has no hitches: to be confirmed in play.)
- [x] A scene's effect made by a script had no time in the frame it was made
      (the original gives it one), so it was a frame behind its shot.
- [x] The black beside the screen checked at other shapes of window (4:3,
      16:10, 21:9; `--size` on the command line): every pixel beside the
      screen is black, in a fade and in scenes.
- [ ] Mouths that sometimes do not move while someone speaks: looked for in
      the first mission's opening, played unattended with the voices on
      (silently) and X pressed every five seconds, and not found. Both kinds
      of mouth work there: the ones a scene's own animation moves (the man
      who shouts "Wake her up, dammit!", his jaw open on the same frames as
      in the walkthrough at 226 s) and the ones the script moves with the
      voice (hers on "...Okay.", the others' in the scene after; each stops
      when its voice ends). Checked over the whole game since: all 1,403
      animations of the story's scenes drive the right nodes of their models
      (`tools_re/motion_match.py`), all 3,383 recordings can be played (a
      test decodes each), and every mouth a script moves has its voice asked
      for in the same frame (7,548 of them). One thing that looks like it
      and is the original's own: a line spoken by someone who is not in the
      picture (a voice over the radio, as Marion's in the first mission's
      scene by the trailer) has no mouth to move, and the scripts give none
      to whoever the camera is on meanwhile; the walkthrough shows the same
      shots. If it was something else, what would find it is the scene or
      the line it was seen in.
- [x] "Model bits bugging out": no fault of a model's own was found (every
      scene's animation drives the right nodes, above); what fits is the parts
      of a model that are drawn another way than the rest coming a frame or
      more after it the first time, which is put right above. (To be
      confirmed in play.)
- [x] Found while looking (not reported): a picture of the screen that the
      game keeps (for a map's dissolve, a battle's beginning, a scene's cut)
      could go on being drawn from an older one once the window had changed
      size, as when going to full screen. What draws it is now made anew each
      time a picture comes. (Not tried with a real window: to be confirmed in
      play.)

## Still to do

### Gameplay not yet proven in play
- [x] A guard mission played through to its win: mission 22's wave about the
      bike, unattended (`docs/compare/guard_mission_won_port.png`): the count
      goes to 4 of 4 before the bike's gauge is out, the scene after it plays
      and the mission goes on to its next objective; left alone, the gauge
      runs out and the mission fails, as before
- [x] The other timed maps: mission 69 on `STG04_0A` played unattended through
      its 300 seconds (`docs/compare/timed_mission_300_port.png`): the clock
      counts down from 5:00 on the display, the time-up scene plays as it
      reaches nought (300.5 s after the start) and MISSION FAILED comes with
      its three choices
- [x] The "mazuma" mission's counter: mission 68 played unattended through its
      three minutes (`docs/compare/mazuma_mission_port.png`): the display
      reads the clock and "BRS 00 vs nn MZMA", Mazuma's count goes up by one
      every six seconds (29 at the end), and as the time runs out the scene
      for it plays and MISSION FAILED comes. Her own count was not seen to
      rise: nothing was fought in that run (the mission's fights are at its
      smoke signals, which a script of key presses did not find)

### Remarks in the notes still marked "not ported", to be sorted one by one
- [x] The states and flags the field is not free in: read one by one (the
      MENU, the pause, the map of the stage, the list of saves, a film, a
      battle, a mission failed, a map being left, the field told to end; two
      that nothing ever sets). Each is a screen over a picture of the field,
      which the port has by the field standing still, with the display in the
      picture or out of it as the original takes it. Nothing left to port
      (`FIELD_NOTES.md`, "What the field is not free in")
- [x] A picture kept and drawn moved about (the fade's kinds 0x800 with 0x1000
      to 0x5000): every start of a fade in the executable was listed, and none
      asks for one of them (the kinds asked for are 1, 2, 4, 0x11, 0x12, 0x803
      and 0x804, a script's `s7` among them), so there is nothing to port
- [x] Names of items from 500 on in a text; text code 0x1A: no text in the
      data uses either, the records' rewards have their words by another way,
      and the port warns if a script ever asks for such a name
- [x] The window that opens with sound 7, and choice sound 5: of no window a
      map's script makes
- [x] The half fades and the wipe of the screen's fade: the save tasks' and the
      title's alone; the title has its own, the save list its own sheet
- [x] The port's windows have their frame before the script's, the original's
      after it: at most a frame's difference in when a window comes, kept as
      it is and listed under "Known difference"
- [x] The field's flag 0x55 and the jingle's stream at a mission cleared: the
      flag is the window's being there, and the jingle's file has nowhere to
      go round from, so it is heard once as in the port
- [x] A light's kind, place and angles kept from the map before: no script
      leaves them unsaid for a light that lights anyone
- [x] The order of drawing a script gives the effects of a scene (setting 123;
      tried, not yet compared with the walkthrough's opening)
- [x] Which model kinds are not asked how near the camera they are: all but
      the characters of the field's list (things of a map and effects are of
      other kinds), as the port has it
- [x] Battle: the sight of the skills that snipe and the log's lines: looked
      into, all in (the one line never written, "No longer Overheated.", the
      original never writes either)
- [x] Battle: the enemies' marks while she has the stage: an enemy that a
      moment of hers shows again has its mark again
- [ ] Battle: the scope's blur of what is behind the enemy. Read: the
      "depth figures" are a filter that blurs the picture by depth (sharp up
      to 100 behind the enemy, wholly blurred from 1100 behind), which only
      the two snipes use (`BATTLE_NOTES.md`, "The filter that blurs by
      depth"). It wants a pass over the picture with its depth, which the
      port does not have yet
- [x] Battle: an enemy's effects standing still in hit stop: an enemy is
      never put in hit stop (nothing sets it), so there is nothing to port
- [x] Battle: what is shown as a status effect is given: it was in already
- [x] Battle: how the picture is drawn while the effect of a battle of note
      plays (0x11AF48): read, tried another way, and measured against the
      walkthrough pixel by pixel. The picture stands whole under the effect's
      own dark sheet (0.37 of it shows, there and in the port), as the port
      had it all along; nothing to change
- [x] Battle: the sky's own near and far planes: every enemy's script was
      gone through, and the 89 calls of that function all ask for the other
      thing it does; none asks for the sky's planes, so there is nothing to port
- [x] Menu: the sign of loading and the frozen picture: nothing loads slowly
      enough on a PC to show a sign for, and the field standing still is the
      frozen picture. The routine noted there as doing something to the sound
      switches the picture's filters for the menu instead, and the port has
      its glare off while the menu is up; and the options are kept between
      runs (the note was out of date)
- [x] `FORMAT_NOTES.md`, "Not done": it was the fog, which is the scene
      script's to set and has been in since the field's and the battle's were
      ported; the heading is put right
- [x] What the field's flag 0x50 does to the camera: read again. It would have
      the camera come in on her and look straight at her, but nothing in the game
      ever sets the flag, so there is nothing to port

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
- A window a script puts up has its frame before the script's, where the
  original's has it after: it can come a thirtieth of a second sooner or
  later than on the PSP.
- When the PC cannot keep up (under thirty pictures a second, or a frame that
  takes long) the game slows for that moment, as a PSP does, and does not skip.
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
