//! A map's script and the engine as it sees it: the functions of its table (the
//! original's table 1, `s<number>` in the notes), and the runner that starts the
//! script's entry points and carries it through its waits. See `docs/FIELD_NOTES.md`.
//!
//! The script works on plain data (the [`Scene`] and the [`Progress`]), which the
//! rest of the field then shows and plays. Functions that are not ported (the
//! camera, sound, what a story scene shows) are reported once, answer zero, and do
//! not make the script wait.
//!
//! An event can be skipped between `s190` and `s191`, as in the original: the
//! script then runs on to the end of that part, and the functions that only show
//! something do nothing meanwhile.
//!
//! A story scene is staged with what an event's bundle brings (`staging.rs`): the
//! script loads it, starts the camera's cuts and the cast's motions, puts captions
//! up, and times the rest by a count of frames.
//!
//! What a script asks of the sound it leaves with the scene, as requests for the
//! game's sound to carry out (`crate::sound`): the tunes, and the recordings of an
//! event's bundle.

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_engine::collision::CollisionWorld;
use brs_formats::bscr::{self, FunctionKind, Reply, Status, Table, Variables};

use super::camera;
use super::frames::FRAMES;
use super::map_screen;
use super::motion::Resources;
use super::place::Title;

use super::scene::{
    field_flag, flag, object_resource, setting, status, value, Ambient, BattleRequest, Capture, Character, Defaults, Fade, FilmAsked, Motion, Prompt, SavesAsked, Scene,
    SceneFade,
    ShakeAsked, Trap, TrapKind, Wall, AMBIENT_PLACES, BANK_OF_MAP, FULL_PACE, NO_TUNE, PLAYER, ROUTE_POINTS,
};
use super::staging::{Captions, Cut, Staging};
use super::system;
use super::tables::FieldTables;
use super::window::{Picture, Window, FRAMES_A_STEP};
use crate::battle::tables::PlayerStats;
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::items::{Inventory, Items};
use crate::progress::Progress;
use crate::records::{self, Records};
use crate::sound::{self, Request, FULL_VOLUME, MAKE_WAY};
use crate::text;

// The types of a map script's entry points.
const OPEN_EVENT: u8 = 0xe7;
const CALLBACK_CHARACTER: u8 = 0xe8;
const CALLBACK_TRAP: u8 = 0xe9;

/// How far above and below where a character is put its ground is looked for
/// (0x108520).
const DROP_REACH: f32 = 40.0;

/// The setting of a trap that switches it off (`s45`).
const TRAP_OFF: i32 = 4;
// More of what `s45` sets of a trap: the word of the sign for what cross does
// with it, and square; whether it is done by a button; and how far from the way
// it faces she may be turned, in degrees.
const TRAP_PROMPT: i32 = 0;
const TRAP_PROMPT_SQUARE: i32 = 1;
const TRAP_BY_ACTION: i32 = 2;
const TRAP_FACING: i32 = 31;
/// And whether walking into it calls the script, which then no button does
/// (0xC8644).
const TRAP_ON_TOUCH: i32 = 38;
/// It is given a sign that shows where it is (0xC8A54, 0xD3D18), and the sign is
/// hidden or shown again (0xC8A44).
const TRAP_SIGN: i32 = 37;
const TRAP_SIGN_HIDDEN: i32 = 36;
const TRAP_MARGIN: i32 = 5;
/// The frame of the field's sixteen its box is in, which is also the one it takes
/// her into; the frame it takes her into, if that is another; and a figure of its
/// own (its + 0x430) that keeps one that is set from being walked into (0xD3F10).
const TRAP_FRAME: i32 = 39;
const TRAP_TAKES_INTO: i32 = 40;
const TRAP_NOT_WALKED_INTO: i32 = 41;

// What `s53` can make a character of (0xC3FBC): one of the map's own effects,
// one of the bundles of them the field always has, or one of those the event
// that is loaded has brought.
const EFFECT_OF_MAP: i32 = 0;
const EFFECT_OF_FIELD: i32 = 1;
const EFFECT_OF_EVENT: i32 = 2;
/// Where a map's bundle has its effects, each a file by its number (0xF1E28),
/// and where the field has its bundles of them, each with its effects in a
/// file of its own (0xEFD44, 0x102684): the first is what a character is of.
const EFFECTS_SLOT: &str = "04.bin";
const FIELD_EFFECTS: &str = "SYSTEM.VOL/fld_resident.bin/03.bin";
const FIELD_EFFECT: &str = "03.bin/00.efc";

/// The frames of an animation that go by in a frame of the game.
const ANIMATION_FRAMES_A_STEP: f32 = 2.0;

/// The function of the system script that `s231(4)` has run, and the variable
/// `s231(5)` sets.
const SYSTEM_START_MAZUMA: &str = "sys_cmn_start_mis_mazuma";
const VARIABLE_OF_A_SAVE: i32 = 300;

// What `s132` can have a character do.
const ACT_WALK: i32 = 0;
const ACT_TURN: i32 = 2;
const ACT_DESTROY: i32 = 6;
/// Its face goes over to the one settings 117 and 118 name (0x1095D4).
const ACT_SHIFT_FACE: i32 = 12;

/// The first of the field's windows that `s104` hands on to the window code
/// (0xC8D14); those below it are parts of the screen with code of their own.
const FIRST_WINDOW: i32 = 5;

/// What `s247` is given first to run an entry of the field's system script.
const SYSTEM_ENTRY: i32 = 1;

/// How many choices the window that asks a question has room for (`s95`).
const CHOICES: usize = 9;
/// The item numbers from here on are names of another kind, which the port does not
/// have yet (0x28520).
const FIRST_OTHER_NAME: i32 = 500;

/// The figure of the field that is the count of frames a scene is timed by (`s96`,
/// 0xC72B4).
const VALUE_EVENT_FRAME: i32 = 8;
/// The stream an event's recordings are played on as a rule, and the one for what
/// is to be heard along with them (0xC277C, 0xC2830).
pub const STREAM_LINES: usize = 0;
const STREAM_OTHER: usize = 1;
/// The frames the sound takes to come back up when a battle starts with it turned
/// down (0xC47F8).
const BATTLE_SOUND_UP_FRAMES: i32 = 0x10;
/// The frames of a fade of the music's volume that go into a block of the sound's:
/// the game runs at half the screen's rate (0x850, 0x11F60).
const FRAMES_A_BLOCK: f32 = 2.0;
/// Where a map's bundle has the header of its bank of sounds.
const BANK_SLOT: &str = "06.bin/00.pphd";
/// What `s27` answers when it plays nothing.
const NO_SOUND: i32 = -1;
/// What `s211` changes of a sound that is playing: its volume, or its place between
/// left and right (0xC24E8).
const SOUND_VOLUME: i32 = 0;
const SOUND_PLACE: i32 = 1;
/// The figure of the field that says the tune is left alone through a battle.
const VALUE_TUNE_KEPT: i32 = 9;
/// How near the camera a character's model may be and still be drawn (0xE4A78),
/// and whether every one is drawn however near it is (0xE4AA8) and has its
/// frame and is drawn however far it is from her (0xE4AF4).
const VALUE_REACH: i32 = 11;
const VALUE_ALL_IN_SIGHT: i32 = 12;
const VALUE_ALL_NEAR: i32 = 21;
/// Whether those who stand idle after a while (0xE5978).
const VALUE_IDLING: i32 = 28;
/// Whether one that `s70` starts an animation for has its matrix worked out there
/// and then (0xE5DD4, 0xE32EC).
const VALUE_PLACED_AT_ONCE: i32 = 30;
/// Which ride on the bike the game goes to when the field is left for one
/// (`s173(2)`, 0xE6F8C): 0 the story's, 3 and 5 others.
const VALUE_RIDE: i32 = 41;
/// The jingles kept in memory, which `s122` plays by number: the same recordings as
/// the tunes `BTL_RESULT`, `MIS_CLEAR`, `MIS_RETIRE`, `MIS_START` and `MIS_RETIRE2`.
const JINGLES: &str = "SYSTEM.VOL/jin_pack.bin";
/// The buttons that pass a caption that waits to be read (0xC2DC0).
const PASS_BUTTONS: u32 = button::L | button::R | button::TRIANGLE | button::CIRCLE | button::CROSS | button::SQUARE;

// The player's figures `s87` reads and `s88` sets (0x9C4, 0xB98).
const FIGURE_EXPERIENCE: i32 = 0;
const FIGURE_LEVEL: i32 = 1;
const FIGURE_HP: i32 = 2;
const FIGURE_MAX_HP: i32 = 3;

/// Something a script's entry point is started for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callback {
    /// A character was touched.
    Character(i32),
    /// The player walked into a trap.
    Trap(i32),
}

/// Whether there is still something to be told of a record (0xC6DC4): news that
/// no panel has told yet, or a panel that is up.
fn news_of_records(field: &Field) -> bool {
    field.records.news(field.progress).is_some() || field.scene.record_news
}

/// What a waiting script is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    /// So many more frames (`s21`).
    Frames(i32),
    /// The end of what a character is doing (`s99`).
    Action(i32),
    /// The end of the fade (`s20`).
    Fade,
    /// The tune's being heard (`s177`).
    Tune,
    /// The end of the battle it asked for (`s81`).
    Battle,
    /// The end of the entry of the system script it asked for (`s247`).
    System,
    /// The end of the field: the script has sent the game somewhere else (`s173`).
    Leaving,
    /// The window it put up to be read (`s252`).
    Window,
    /// The panels that tell of the records to have told all there is (`s275`).
    RecordNews,
    /// The windows of the records it has asked for (`s276`).
    RecordWindows,
    /// The list of saves it has asked for (`s178`, `s204`).
    Saves,
    /// The answer to the question it asked (`s75`).
    Question,
    /// A press of cross (`s258`).
    Press,
    /// The end of the events it has had played (`s238`).
    Events,
    /// The end of the cut the camera is playing (`s100`).
    Cut,
    /// The frame of the scene's count that it named (`s103`).
    EventFrame(i32),
    /// The last of the captions to go (`s240`).
    Captions,
    /// The end of the fade of the scene alone (`s198`).
    SceneFade,
    /// The picture of the screen it asked to have taken (`s145`, `s172`, `s223`);
    /// with `hold` the picture is then put up to stay.
    Picture { hold: bool },
    /// The title of the scene's place to be ready to be started (`s37`).
    TitleMade,
    /// The title of the scene's place to have gone (`s25`).
    Title,
    /// The film it has had made ready to be so (`s84`), and to be over (`s86`).
    FilmReady,
    FilmOver,
}

/// A film is done with (0xC32A4 on): its task ends, and the field has again what
/// it put aside for it (0xE68F8, which clears its flag 0x2C).
fn end_film(scene: &mut Scene) {
    scene.film = None;
    scene.set_flag(field_flag::FILM, false);
}

/// What the script's functions work on.
pub struct Field<'a> {
    pub progress: &'a mut Progress,
    pub stats: &'a mut PlayerStats,
    pub scene: &'a mut Scene,
    pub tables: &'a FieldTables,
    pub world: &'a CollisionWorld,
    pub resources: &'a dyn Resources,
    pub items: &'a Items,
    pub inventory: &'a mut Inventory,
    /// The buttons, for the windows the script puts up.
    pub pad: &'a Pad,
    /// The records, which a script can open and count (0xC6DA8, 0xC6ED8).
    pub records: &'a Records,
}

/// A map's script, running.
pub struct MapScript {
    script: bscr::Script,
    machine: bscr::Machine,
    /// Where an event's bundle is loaded from when the script asks for one.
    data: GameData,
    wait: Option<Wait>,
    /// The part of an event that can be skipped is running: from `s190` to `s191`
    /// (the context's flag 0x100).
    skippable: bool,
    /// The event is being skipped (the context's flag 0x200): waits of frames are
    /// over at once, walks arrive at once, and what only shows something is left
    /// out, until `s191`.
    fast: bool,
    /// Events are skipped as soon as they can be: the port cannot show them yet.
    pub skip_events: bool,
    /// While set no window is put up: each is taken as read, and a question as
    /// answered with what the cursor starts on. For runs with no one at the buttons.
    pub skip_windows: bool,
    /// The player's option that a scene's captions go on of themselves (0x614):
    /// without it those of a scene that does not say otherwise wait for a button.
    pub auto_skip: bool,
    /// What the entry point that is running was started for.
    current: Option<Callback>,
    /// The functions that were called and are not ported, so that each is reported
    /// once; and the figures of the field that were set and are not.
    unported: BTreeSet<usize>,
    unported_values: BTreeSet<i32>,
    /// What the numbers it draws by chance come of (`s130`).
    random: u32,
}

impl MapScript {
    pub fn new(script: bscr::Script, data: GameData) -> MapScript {
        let machine = bscr::Machine::new(&script);
        MapScript {
            script,
            machine,
            data,
            wait: None,
            skippable: false,
            fast: false,
            skip_events: false,
            skip_windows: false,
            auto_skip: false,
            current: None,
            unported: BTreeSet::new(),
            unported_values: BTreeSet::new(),
            random: 0x2545_f491,
        }
    }

    /// True while an entry point is running: the field is in an event, and no other
    /// can be started.
    pub fn busy(&self) -> bool {
        self.machine.suspended()
    }

    /// Whether the event that is running can be skipped now (0xD93DC, 0xD9448):
    /// the part of it that can be is running, and is not being skipped already.
    /// It is then that start brings the pause up over a scene.
    pub fn can_skip(&self) -> bool {
        self.busy() && self.skippable && !self.fast
    }

    /// The event is skipped (0x10C92C, and the field's flag 0x3F): what the pause
    /// does when that is chosen from it.
    pub fn skip_event(&mut self, scene: &mut Scene) {
        if self.can_skip() {
            self.fast = true;
            skip(scene);
        }
    }

    /// The functions called so far that are not ported.
    #[cfg(test)]
    pub fn unported(&self) -> &BTreeSet<usize> {
        &self.unported
    }

    /// Runs what a map's script runs when the map has loaded: its `init` functions,
    /// and then its opening event, as far as its first wait (0xD90AC).
    pub fn open(&mut self, field: &mut Field) -> Result<()> {
        let inits: Vec<usize> = (0..self.script.functions.len())
            .filter(|&index| self.script.functions[index].kind == FunctionKind::Init)
            .collect();
        for function in inits {
            self.run(function, &[], field)?;
            anyhow::ensure!(!self.busy(), "an init function of the map's script waits");
        }
        match self.script.entry(FunctionKind::Event(OPEN_EVENT)) {
            Some(function) => self.run(function, &[0], field),
            None => Ok(()),
        }
    }

    /// Starts the entry point for `callback`, as far as its first wait. The script
    /// must not be busy: the field only looks for touches and traps between events.
    pub fn call(&mut self, callback: Callback, field: &mut Field) -> Result<()> {
        anyhow::ensure!(!self.busy(), "the map's script is in the middle of an event");
        let (kind, id) = match callback {
            Callback::Character(id) => (CALLBACK_CHARACTER, id),
            Callback::Trap(id) => (CALLBACK_TRAP, id),
        };
        // The name of the place is not left up over an event (0xE83AC): it goes.
        if let Some(name) = &mut field.scene.place_name {
            name.dismiss(false);
        }
        self.current = Some(callback);
        match self.script.entry(FunctionKind::Event(kind)) {
            Some(function) => self.run(function, &[id], field),
            None => Ok(()),
        }
    }

    /// A frame of a waiting script: it goes on if what it waits for has come (0xC9838).
    /// The windows it has up over the field go on a frame first; its pictures and
    /// its fade of the scene after it, as their tasks come after the field's own
    /// (0x340 and 0x229 after 0x100).
    pub fn update(&mut self, field: &mut Field) -> Result<()> {
        if let Some(window) = &mut field.scene.window {
            let answer = window.step(field.pad);
            field.scene.sounds.extend(window.sounds.drain(..).map(Request::common));
            if let Some(selected) = answer {
                field.scene.selection = selected as i32;
                field.scene.window = None;
            }
        }
        field.scene.comm.step(field.pad, self.skip_windows);
        let result = self.advance(field);
        for picture in field.scene.pictures.values_mut() {
            picture.step();
        }
        if let Some(fade) = &mut field.scene.scene_fade {
            fade.step();
        }
        // The title of the scene's place has its frame (0xCFFEC), and is done
        // with when it has gone.
        if let Some(title) = &mut field.scene.location {
            let there = title.step();
            field.scene.sounds.append(&mut title.sign.sounds);
            if !there {
                field.scene.location = None;
            }
        }
        // What a scene is timed by goes on a frame, script or no script (0xD9224);
        // then the camera's cut (0xDCA10) and the captions (0xD1D58).
        let scene = &mut *field.scene;
        scene.event_frame += 1;
        if let Some(cut) = &mut scene.cut {
            cut.step();
        }
        if let Some(captions) = &mut scene.captions {
            captions.step();
        }
        result
    }

    /// The script's part of a frame.
    fn advance(&mut self, field: &mut Field) -> Result<()> {
        if !self.busy() {
            return Ok(());
        }
        if let Some(wait) = self.wait {
            let over = match wait {
                Wait::Frames(left) if left > 1 && !self.fast => {
                    self.wait = Some(Wait::Frames(left - 1));
                    false
                }
                Wait::Frames(_) => true,
                Wait::Action(id) => field.scene.characters.get(&id).is_none_or(|character| !character.acting()),
                Wait::Fade => !field.scene.fade.running(),
                Wait::Battle => field.scene.battle.is_none(),
                Wait::System => field.scene.system.is_none() && field.scene.system_call.is_none(),
                Wait::Tune => field.scene.tune_heard,
                Wait::Leaving => false,
                Wait::Window => field.scene.window.is_none(),
                Wait::RecordNews => !news_of_records(field),
                Wait::RecordWindows => !field.scene.record_windows,
                Wait::Saves => field.scene.saves.is_none(),
                Wait::Question => {
                    let answered = field.scene.window.is_none();
                    if answered {
                        field.scene.answer = field.scene.selection;
                    }
                    answered
                }
                Wait::Press => self.skip_windows || field.pad.pressed & button::CROSS != 0,
                Wait::Events => field.scene.events.is_empty() && !field.scene.event_running,
                Wait::Cut => self.fast || !field.scene.cut.as_ref().is_some_and(|cut| cut.playing),
                Wait::EventFrame(frame) => self.fast || field.scene.event_frame >= frame,
                Wait::SceneFade => field.scene.scene_fade.is_none_or(|fade| fade.over),
                Wait::Picture { hold } => {
                    let taken = field.scene.capture == Capture::None;
                    if taken && hold {
                        field.scene.fade.hold_picture();
                    }
                    taken
                }
                Wait::Captions => {
                    // Any of these buttons passes a caption that waits to be read
                    // (0xC2D80); with no one at the buttons it is taken as read.
                    let captions = field.scene.captions.as_mut();
                    let up = captions.as_ref().is_some_and(|captions| captions.any());
                    let passed = self.skip_windows || field.pad.pressed & PASS_BUTTONS != 0;
                    if let (Some(captions), true) = (captions, up && passed) {
                        captions.pass();
                    }
                    self.fast || !up
                }
                // Made, it waits to be started (0xC2F1C; nothing is ever still
                // loading here); and its task ends when it has gone (0xC3030).
                Wait::TitleMade => field.scene.location.as_ref().is_none_or(Title::made),
                Wait::Title => field.scene.location.is_none(),
                Wait::FilmReady => field.scene.film.as_ref().is_none_or(|film| film.ready || film.over),
                // Over, its task ends and what was put aside for it comes back
                // (0xC3264).
                Wait::FilmOver => {
                    let over = field.scene.film.as_ref().is_none_or(|film| film.over);
                    if over {
                        end_film(field.scene);
                    }
                    over
                }
            };
            if !over {
                return Ok(());
            }
            self.wait = None;
        }
        self.resume(field)
    }

    fn run(&mut self, function: usize, arguments: &[i32], field: &mut Field) -> Result<()> {
        let mut engine = Engine {
            field,
            data: &self.data,
            wait: &mut self.wait,
            skippable: &mut self.skippable,
            fast: &mut self.fast,
            skip_events: self.skip_events,
            skip_windows: self.skip_windows,
            auto_skip: self.auto_skip,
            current: self.current,
            unported: &mut self.unported,
            unported_values: &mut self.unported_values,
            random: &mut self.random,
        };
        let status = self.machine.call(&self.script, function, arguments, &mut engine)?;
        self.finished(status)
    }

    fn resume(&mut self, field: &mut Field) -> Result<()> {
        let mut engine = Engine {
            field,
            data: &self.data,
            wait: &mut self.wait,
            skippable: &mut self.skippable,
            fast: &mut self.fast,
            skip_events: self.skip_events,
            skip_windows: self.skip_windows,
            auto_skip: self.auto_skip,
            current: self.current,
            unported: &mut self.unported,
            unported_values: &mut self.unported_values,
            random: &mut self.random,
        };
        let status = self.machine.resume(&self.script, &mut engine)?;
        self.finished(status)
    }

    /// An entry point leaves nothing behind; one that returns a value has it dropped.
    fn finished(&mut self, status: Status) -> Result<()> {
        if status == Status::Finished {
            self.machine.abort();
            self.wait = None;
            self.current = None;
            self.skippable = false;
            self.fast = false;
        }
        Ok(())
    }
}

/// The blocks what is spoken takes to stop when an event is skipped (0x12D4B4).
const SKIP_HUSH: i32 = 1;

/// What skipping an event does there and then (0x10C92C, 0x10C9E0): the picture
/// goes dark at once, so that what is passed over is not seen, the camera's cut
/// stops, and the events that were being played are dropped with whatever was
/// being said. What was being heard stops too: the menu that offers the skip
/// notes that it was chosen (0x12D9E0), and as it goes it stops both streams and
/// every sound of the banks (0x12D474); the tune plays on. The original stops the
/// sounds about the map with the rest and its scripts place them again; here they
/// are spared, since without `--play-events` every event is skipped. The title
/// of the scene's place is done away with too (0xCEF48, at 0x12DAA8).
fn skip(scene: &mut Scene) {
    let spared = scene.ambience.values().map(|ambient| ambient.handle).collect();
    scene.sounds.extend([Request::Hush(STREAM_LINES, SKIP_HUSH), Request::Hush(STREAM_OTHER, SKIP_HUSH), Request::StopEffects(spared)]);
    scene.location = None;
    scene.fade = Fade::DARK;
    scene.capture = Capture::None;
    // A film that is playing is stopped (0x118958, from the menu's 0x12D080 on).
    end_film(scene);
    scene.events.clear();
    scene.stop_events = true;
    scene.comm.reset();
    if let Some(cut) = &mut scene.cut {
        cut.playing = false;
    }
    if let Some(captions) = &mut scene.captions {
        captions.clear();
    }
}

/// The engine's side of a run of the script.
struct Engine<'a, 'f> {
    field: &'a mut Field<'f>,
    data: &'a GameData,
    wait: &'a mut Option<Wait>,
    skippable: &'a mut bool,
    fast: &'a mut bool,
    skip_events: bool,
    skip_windows: bool,
    auto_skip: bool,
    /// What the entry point that is running was started for.
    current: Option<Callback>,
    unported: &'a mut BTreeSet<usize>,
    unported_values: &'a mut BTreeSet<i32>,
    random: &'a mut u32,
}

/// What a text of the script's can ask for: the script's variables, and the names
/// of things.
struct Names<'a> {
    variables: Variables<'a>,
    items: &'a Items,
}

impl text::Lookup for Names<'_> {
    fn local(&self, index: usize) -> i32 {
        self.variables.local(index)
    }

    fn global(&self, index: usize) -> i32 {
        self.variables.global(index)
    }

    fn item(&self, number: i32) -> String {
        if number >= FIRST_OTHER_NAME {
            debug!("a text asked for name {number}, of a kind that is not ported");
            return String::new();
        }
        self.items.name(number).to_owned()
    }
}

impl bscr::Host for Engine<'_, '_> {
    fn engine(&mut self, table: Table, number: usize, arguments: &[i32], script: &bscr::Script) -> Result<Reply> {
        self.engine_with(table, number, arguments, script, Variables::NONE)
    }

    fn engine_with(
        &mut self,
        table: Table,
        number: usize,
        arguments: &[i32],
        script: &bscr::Script,
        variables: Variables,
    ) -> Result<Reply> {
        anyhow::ensure!(table == Table::One, "a map's script called function {number} of the battle's table");
        let a = |index: usize| arguments[index];
        let value = match number {
            // Debug output.
            2 => {
                debug!("script: {}", script.string(a(0))?.trim_end());
                0
            }
            35 => {
                debug!("script: {}", a(0));
                0
            }
            7 => {
                self.fade(a(0), a(1), a(2));
                0
            }
            20 => return Ok(self.wait_for(Wait::Fade, self.field.scene.fade.running())),
            // A picture of the scene is taken (0xC5270), and goes over so many
            // frames of the interface (0xC5314): between the two a scene changes
            // its shot, which the old one then gives way to. One being run
            // through fast has neither.
            145 => return Ok(self.take_picture(false, false)),
            146 => {
                if !*self.fast {
                    debug!("the picture of the screen goes over {} frames", a(0) / FRAMES_A_STEP as i32);
                    self.field.scene.fade.dissolve(a(0) / FRAMES_A_STEP as i32);
                }
                0
            }
            // A picture is taken, of the scene alone or of all the screen
            // (0xC52A0); and one that is then put up to stay (0xC5370), which is
            // how a scene's last shot is left for the field to come through.
            172 if matches!(a(0), 0 | 1) => return Ok(self.take_picture(a(0) == 1, false)),
            223 if matches!(a(0), 0 | 1) => return Ok(self.take_picture(a(0) == 1, true)),
            172 | 223 => 0,
            // A wait of so many frames, which a skipped event does not wait (0xC180C).
            21 => return Ok(self.wait_for(Wait::Frames(a(0)), !*self.fast)),
            // The part of an event that can be skipped begins and ends (0xC33C4,
            // 0xC3580).
            190 => {
                *self.skippable = true;
                *self.fast = false;
                if self.skip_events {
                    *self.fast = true;
                    skip(self.field.scene);
                }
                0
            }
            191 => {
                *self.skippable = false;
                *self.fast = false;
                0
            }
            // The title of a scene's place ("Location"): it is made (0xCEDBC),
            // which does away with one there was; waited for until it can be
            // started (0xC2F64); done away with (0xCEE40); started, to stand so
            // many frames once it has been typed (0xCEEC0); told to go
            // (0xCEF08); and told where on the screen it is to be (0xCF018).
            // Its end is waited for (0xC3068). Without one each of them does
            // nothing.
            36 => {
                self.field.scene.location = Some(Title::new(a(0)));
                0
            }
            37 => return Ok(self.wait_for(Wait::TitleMade, self.field.scene.location.is_some())),
            38 => {
                self.field.scene.location = None;
                0
            }
            39 => {
                if let Some(title) = &mut self.field.scene.location {
                    title.start(&self.field.tables.places, a(0));
                }
                0
            }
            129 => {
                if let Some(title) = &mut self.field.scene.location {
                    title.go();
                }
                0
            }
            256 => {
                if let Some(title) = &mut self.field.scene.location {
                    title.sign.put(a(0));
                }
                0
            }
            25 => return Ok(self.wait_for(Wait::Title, self.field.scene.location.is_some())),

            // The game's progress.
            8 => self.field.progress.flag(a(0)) as i32,
            12 => {
                self.field.progress.set_flag(a(0), false);
                0
            }
            13 => {
                self.field.progress.set_flag(a(0), true);
                0
            }
            23 => self.field.progress.variable(a(0)),
            24 => {
                self.field.progress.set_variable(a(0), a(1));
                0
            }

            // Characters.
            40 => {
                *self.field.scene.resources.entry(a(0)).or_default() += 1;
                0
            }
            41 => {
                self.release(a(0));
                0
            }
            42 => {
                self.create(a(0), a(1), position(&arguments[2..5]), degrees(a(5)));
                0
            }
            48 => {
                // A skipped event leaves characters where they are (0xC3AE8): the
                // script puts them where they belong when the event is over.
                if !*self.fast {
                    self.place(a(0), position(&arguments[1..4]), degrees(a(4)));
                }
                0
            }
            144 => {
                self.field.scene.characters.remove(&a(0));
                self.field.scene.walls.remove(&a(0));
                0
            }
            // A character is made of an effect, and its effect set playing
            // (0xC3FBC).
            53 => {
                self.make_effect(a(0), a(1), a(2));
                0
            }
            // One is fastened to a node of another's model (0xC6098), and let go
            // of (0xC60D8).
            220 => {
                if let Some(effect) = self.field.scene.characters.get_mut(&a(0)).and_then(|character| character.effect.as_mut()) {
                    effect.fastened = Some((a(1), a(2)));
                }
                0
            }
            222 => {
                if let Some(effect) = self.field.scene.characters.get_mut(&a(0)).and_then(|character| character.effect.as_mut()) {
                    effect.fastened = None;
                }
                0
            }
            // The events that are played of themselves, and the system script's
            // own frame with them, can be held (the field's flag 0x57, 0xD76F8).
            // With 1 they are let go on and the window people speak through comes
            // back (0xD8220), which the scripts ask for after every battle; the
            // port holds nothing through a battle, the field having no frames
            // then. With 2 those that play and those that wait are dropped with
            // what was being said (0xD8264, 0x12F14C), as when a battle is lost.
            // (0 holds them, which no script asks for.)
            231 => {
                match a(0) {
                    1 => {}
                    2 => {
                        let scene = &mut *self.field.scene;
                        scene.events.clear();
                        scene.stop_events = true;
                        scene.comm.reset();
                    }
                    // What was kept loaded for the map's battles is let go
                    // (0xEAF0C, 0xEAF44: thirty-three of the system's list of
                    // what is loaded): the port reads such things as they are
                    // wanted, and keeps none.
                    3 => {}
                    // The system script's own beginning of the mission against
                    // Mazuma is run (0x130CE8 with its name), and the map's
                    // script goes on when it has.
                    4 => {
                        self.field.scene.system_call = Some(SYSTEM_START_MAZUMA);
                        return Ok(self.wait_for(Wait::System, true));
                    }
                    // The game's variable 300 is one (0x23E0), before a save is
                    // offered.
                    5 => self.field.progress.set_variable(VARIABLE_OF_A_SAVE, 1),
                    // About the opening's film the options' music is made full,
                    // what the player has being kept (0xE64DC: the field's
                    // + 0x1D43), and put back (0xE6508): a film's sound is as
                    // loud as the options have the music.
                    6 => self.field.scene.sounds.push(Request::MusicOptionFull(true)),
                    7 => self.field.scene.sounds.push(Request::MusicOptionFull(false)),
                    other => {
                        if self.unported_values.insert(0x1000 + other) {
                            debug!("script function s231({other}) is not ported");
                        }
                    }
                }
                0
            }
            // Every character that bears one of three marks is shown, or hidden
            // (0xC5C38, 0xE56D0): its flags 0x4E, 0x4D and 0x4F, for the bits 1, 2
            // and 4 of what is asked. (Settings 42 and 56 on give the marks, and
            // no map's script gives any.)
            210 => {
                for (bit, mark) in [(1, flag::MARKED_FIRST), (2, flag::MARKED_SECOND), (4, flag::MARKED_THIRD)] {
                    if a(0) & bit != 0 {
                        for character in self.field.scene.characters.values_mut().filter(|character| character.flags.has(mark)) {
                            character.flags.set(flag::HIDDEN, a(1) == 0);
                        }
                    }
                }
                0
            }
            // Nothing: its function is empty in the game as it was released
            // (0xE526C).
            192 => 0,
            // The camera shakes: it comes over so many frames, so many hundredths
            // of a degree up and down and from side to side (0xC613C), and dies
            // away over so many (0xC619C).
            228 => {
                let reach = Vec2::new(a(0) as f32, a(1) as f32).map(|hundredths| (hundredths / 100.0).to_radians());
                self.field.scene.shakes.push(ShakeAsked::Come { reach, frames: a(2) });
                0
            }
            229 => {
                self.field.scene.shakes.push(ShakeAsked::DieAway(a(0)));
                0
            }
            // A character's head goes over, in so many frames, to how its neck is
            // next to turn it (0xC543C); one that is an effect has no neck.
            209 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)).filter(|character| character.effect.is_none()) {
                    character.neck.start(a(1), a(2));
                }
                0
            }
            // A character's eyes blink, stay open or stay shut (0xC3314), and its
            // mouth moves or is still (0xC3364).
            159 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                    character.face.blink_after(a(1));
                }
                0
            }
            160 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                    character.speak(a(1), a(2));
                }
                0
            }
            // Waits for a character's resource to have been read (0xC5ED8): here
            // it has been by the time it is asked for.
            280 => 0,
            52 => {
                let scene = &mut *self.field.scene;
                if let Some(character) = scene.characters.get_mut(&a(0)) {
                    character.set(a(1), a(2));
                    // In another of the field's frames it lies as that one does.
                    if a(1) == setting::GROUP {
                        character.lie_in(&scene.frames);
                    }
                } else if let (Some(wall), setting::HIDDEN | setting::NO_DROP) = (self.field.scene.walls.get_mut(&a(0)), a(1)) {
                    // Hidden, or told not to be solid, a wall is not in the way.
                    wall.open = a(2) != 0;
                }
                0
            }
            143 => self.field.scene.characters.get(&a(0)).map_or(0, |character| character.get(a(1))),
            133 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                    character.target = position(&arguments[1..4]);
                    character.target_facing = degrees(a(4));
                }
                0
            }
            132 => {
                self.act(a(0), a(1));
                0
            }
            // One of the switchable parts of a character's model is shown or hidden
            // (0xC60FC, 0x109088).
            224 => {
                let part = u32::try_from(a(1)).ok().filter(|&part| (part as usize) < self.field.tables.meshes.len());
                if let (Some(character), Some(part)) = (self.field.scene.characters.get_mut(&a(0)), part) {
                    character.flags.set(flag::PART + part, a(2) != 0);
                }
                0
            }
            // Waits for what a character is doing to end; a skipped event does not
            // (0xC4B38).
            99 => {
                let acting = self.field.scene.characters.get(&a(0)).is_some_and(Character::acting);
                return Ok(self.wait_for(Wait::Action(a(0)), acting && !*self.fast));
            }
            179 => self.bearing(a(0), a(1)),
            // An animation of the script's choosing: `s62` and its like name it and
            // `s70` starts it. A skipped event gives none but by `s62` (0xC4228,
            // 0xC427C, 0xC42FC).
            62 => {
                if *self.fast {
                    if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                        character.name_only(Some(Motion::Own(a(1))));
                    }
                } else {
                    self.motion(a(0), Some(Motion::Own(a(1))));
                }
                0
            }
            // One an event has brought for a scene: the one kept for the character's
            // kind (0xE94EC), or one of a slot of the cast that the script names
            // (0xE9578). A motion the event does not have leaves the character with
            // none of its own choosing.
            63 => {
                if !*self.fast {
                    let scene = &*self.field.scene;
                    let chr = scene.characters.get(&a(0)).map(|character| character.chr);
                    let slot = chr.zip(scene.staging.as_ref()).and_then(|(chr, staging)| staging.slot_of(chr));
                    if let Some(slot) = slot {
                        self.staged(a(0), slot, a(1));
                    }
                }
                0
            }
            265 => {
                if !*self.fast {
                    if let Ok(slot) = usize::try_from(a(1)) {
                        self.staged(a(0), slot, a(2));
                    }
                }
                0
            }
            // It is set playing (0xC42DC, not in a skipped event), which keeps
            // the character busy until an animation that does not loop is
            // through: what `s99` then waits for.
            70 => {
                if !*self.fast {
                    self.play(a(0));
                }
                0
            }
            // The animation is put at a frame and held there (0xC42AC); or set
            // playing for so many frames (0xC4320: the frame it is given to
            // start from is overwritten as the animation is started, 0xF7E34).
            64 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                    character.hold_motion(a(1));
                }
                0
            }
            65 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                    character.play_motion_for(a(2));
                }
                0
            }

            // What an event brings to stage a scene: loaded (0xC40A4), waited for
            // (here it is there at once), let go (0xC41B8). Loading one lets go of
            // the one loaded before, its bank and what was playing of it with it
            // (0xE313C). (While an event is skipped the original puts the loading
            // off until the skip is over; it comes to the same.)
            59 => {
                debug!("event {} is loaded for its scene", a(0));
                let scene = &mut *self.field.scene;
                if let Some(bank) = scene.staging.take().and_then(|staging| staging.bank().map(str::to_owned)) {
                    scene.sounds.push(Request::DropBank(bank));
                }
                scene.drop_props();
                let staging = Staging::load(self.data, self.field.tables, a(0));
                self.field.scene.staging = match staging {
                    Ok(staging) => Some(Arc::new(staging)),
                    Err(error) => {
                        warn!("event {} could not be loaded: {error:#}", a(0));
                        None
                    }
                };
                0
            }
            60 => 0,
            61 => {
                // Its bank goes with it, and what was playing of it; and what it
                // gave its people to hold (0xE9964).
                let scene = &mut *self.field.scene;
                if let Some(bank) = scene.staging.take().and_then(|staging| staging.bank().map(str::to_owned)) {
                    scene.sounds.push(Request::DropBank(bank));
                }
                scene.drop_props();
                0
            }
            // A character is given a thing of the event's to hold, in one of its
            // places for such (0xC443C: not in a skipped event), and what is in
            // one of them is put away or brought out (0xC53E4, whose third
            // figure has to be nought).
            71 => {
                if !*self.fast {
                    let scene = &mut *self.field.scene;
                    let model = scene.staging.as_ref().and_then(|staging| staging.prop(a(2))).map(str::to_owned);
                    if let (Some(character), Some(model)) = (scene.characters.get_mut(&a(0)), model) {
                        character.hold(a(1), &model);
                    }
                }
                0
            }
            150 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)).filter(|_| a(2) == 0) {
                    character.put_away(a(1), a(3) != 0);
                }
                0
            }
            // The camera plays a cut of the event's (0xE2EC4); a skipped event
            // starts none. A cut the event does not have leaves the camera where
            // it is, with nothing to wait for.
            101 => {
                if !*self.fast {
                    let scene = &mut *self.field.scene;
                    let clip = scene.staging.as_ref().and_then(|staging| staging.cut(a(0)));
                    debug!("cut {}: {:?} frames", a(0), clip.map(|clip| clip.frames()));
                    if let Some(clip) = clip {
                        scene.cut = Some(Cut::new(clip));
                    } else if let Some(cut) = &mut scene.cut {
                        cut.playing = false;
                    }
                    scene.camera.set_kind(camera::kind::CUT);
                }
                0
            }
            100 => {
                let playing = self.field.scene.cut.as_ref().is_some_and(|cut| cut.playing);
                return Ok(self.wait_for(Wait::Cut, playing && !*self.fast));
            }
            103 => {
                let early = self.field.scene.event_frame < a(0);
                return Ok(self.wait_for(Wait::EventFrame(a(0)), early && !*self.fast));
            }

            // Captions for what is spoken in a scene: the script has them made
            // (0xC2C30), says a line for so many frames with so many to come and
            // to go (0xC2CE4), waits for the last to go (0xC2E40), and has done
            // with them (0xC2C84). A skipped event says nothing. Unless the
            // player has them go on of themselves, each stays until a button
            // passes it (0xD1634), which the script may yet say otherwise.
            239 => {
                if !*self.fast {
                    self.field.scene.captions = Some(Captions::made(self.auto_skip));
                }
                0
            }
            241 => {
                self.field.scene.captions = None;
                0
            }
            242 => {
                if !*self.fast {
                    let names = Names { variables, items: self.field.items };
                    let line = text::pages(script.string(a(0))?, &names).into_iter().next().unwrap_or_default();
                    if let Some(captions) = &mut self.field.scene.captions {
                        captions.say(line, a(1), a(2), a(3));
                    }
                }
                0
            }
            240 => {
                let up = self.field.scene.captions.as_ref().is_some_and(Captions::any);
                return Ok(self.wait_for(Wait::Captions, up && !*self.fast));
            }
            // Whether a caption stays until a button passes it (0xD1B68), and that
            // they are drawn higher up (0xD1B54).
            243 => {
                if let Some(captions) = &mut self.field.scene.captions {
                    captions.by_hand = a(0) as i8 != 0;
                }
                0
            }
            261 => {
                if let Some(captions) = &mut self.field.scene.captions {
                    captions.raised = true;
                }
                0
            }

            // The records (0xC6DA8, 0xC6ED8): one's count goes up, and one is opened.
            274 | 277 => {
                let field = &mut *self.field;
                let stage = usize::try_from(field.scene.map).ok().and_then(|map| field.tables.maps.get(map)).map_or(0, |map| i32::from(map.stage));
                let mut world = records::World { progress: &mut *field.progress, stats: &mut *field.stats, stage };
                if number == 274 { field.records.count_up(a(0), &mut world)? } else { field.records.open(a(0), &mut world)? };
                0
            }
            // What there is to tell of them is waited for (0xC6E38): the panels,
            // while there is news of a record or one is up.
            275 => {
                let told = !news_of_records(self.field);
                return Ok(self.wait_for(Wait::RecordNews, !told));
            }
            // And the records of missions that have been opened or cleared are
            // shown, each in its window (0xC6E9C): nothing if there is none.
            276 => {
                let field = &mut *self.field;
                field.scene.record_windows = !field.records.list(records::OF_MISSIONS, field.progress).is_empty();
                return Ok(self.wait_for(Wait::RecordWindows, self.field.scene.record_windows));
            }

            // Traps.
            43 => {
                self.field.scene.make_trap(a(0), TrapKind::MapChange { map: a(1), entrance: a(2) });
                0
            }
            67 => {
                self.field.scene.make_trap(a(0), TrapKind::Event);
                0
            }
            118 => {
                self.field.scene.make_trap(a(0), TrapKind::Camera);
                0
            }
            44 => {
                if let Some(trap) = self.field.scene.traps.get_mut(&a(0)) {
                    trap.shape(position(&arguments[1..4]), position(&arguments[4..7]), degrees(a(7)));
                }
                0
            }
            // The same with all three of the box's turns (0xC3CC8): the camera's
            // traps of the sixth stage, whose floors lie every way.
            257 => {
                if let Some(trap) = self.field.scene.traps.get_mut(&a(0)) {
                    let turns = Vec3::new(degrees(a(7)), degrees(a(8)), degrees(a(9)));
                    trap.shape_turned(position(&arguments[1..4]), position(&arguments[4..7]), turns);
                }
                0
            }
            // One of the field's sixteen frames is given one of its three turns,
            // in degrees, or one of the three figures of its place (0xC66D4):
            // what is in it lies as it does from then on.
            255 => {
                let scene = &mut *self.field.scene;
                scene.frames.choose(a(0));
                match a(1) {
                    0..=2 => scene.frames.set(a(1), degrees(a(2)), self.field.world),
                    3..=5 => scene.frames.set(a(1), a(2) as f32, self.field.world),
                    _ => {}
                }
                scene.lay_characters();
                0
            }

            // Walls: a character with no model (0xE30D4), given a box to be (0xE4214).
            185 => {
                if !self.field.scene.characters.contains_key(&a(0)) {
                    self.field.scene.walls.entry(a(0)).or_insert_with(Wall::new);
                }
                0
            }
            116 => {
                if let Some(wall) = self.field.scene.walls.get_mut(&a(0)) {
                    wall.shape(position(&arguments[1..4]), position(&arguments[4..7]), degrees(a(7)));
                }
                0
            }
            // A character is a box to those that would walk through it, about a
            // place of its own frame (0xC4F24): what is on a box she can open, a
            // container, a barrier and a generator.
            236 => {
                if let Some(character) = self.field.scene.characters.get_mut(&a(0)) {
                    character.be_a_box(position(&arguments[1..4]), position(&arguments[4..7]), degrees(a(7)));
                }
                0
            }
            // Something of a trap's (0xC8588): it is switched off or on; what cross
            // or square does with it is given its sign's word (0xC85D8); it is to
            // be done by a button, and then not by walking into it (0xC8624); how
            // she has to be turned to be in it (0xC8914); it is to be done by
            // walking into it after all (0xC8644); or it is given what the camera
            // is to have while she is inside it; or it is given its sign; or the
            // frame of the field's its box is in, and the one it takes her into
            // (0xD3F10). The rest is not ported.
            45 => {
                if let Some(trap) = self.field.scene.traps.get_mut(&a(0)) {
                    match a(1) {
                        TRAP_OFF => trap.off = a(2) != 0,
                        TRAP_PROMPT | TRAP_PROMPT_SQUARE => {
                            trap.prompts[(a(1) - TRAP_PROMPT) as usize] = Prompt::of(a(2));
                            trap.by_action = true;
                        }
                        TRAP_BY_ACTION => {
                            trap.by_action = a(2) != 0;
                            if trap.by_action {
                                trap.on_touch = false;
                            }
                        }
                        TRAP_FACING => trap.facing = degrees(a(2)),
                        TRAP_ON_TOUCH => {
                            trap.on_touch = a(2) != 0;
                            if trap.on_touch {
                                trap.by_action = false;
                            }
                        }
                        TRAP_SIGN => trap.sign = Some(a(2)),
                        TRAP_SIGN_HIDDEN => trap.sign_hidden = a(2) != 0,
                        // How far past its box she still counts as in it
                        // (0xE2280): only the ways out have one, to keep her
                        // in them while their callback walks her on, which
                        // nothing here goes by.
                        TRAP_MARGIN => {}
                        TRAP_FRAME => {
                            let frame = a(2).clamp(0, FRAMES as i32 - 1) as u8;
                            (trap.frame, trap.takes_into) = (frame, Some(frame));
                        }
                        TRAP_TAKES_INTO => trap.takes_into = u8::try_from(a(2)).ok().filter(|&frame| usize::from(frame) < FRAMES),
                        TRAP_NOT_WALKED_INTO => trap.not_walked_into = a(2) != 0,
                        what if !trap.camera.set(what, a(2)) => {
                            debug!("s45({}, {what}, ..), a setting of a trap's, is not ported", a(0));
                        }
                        _ => {}
                    }
                }
                0
            }

            // A trap has her taken somewhere: it is given a place to take her to,
            // on the line between two points, and how (0xC61D4); and she is set
            // off for the first of its places, each of which she goes on from to
            // the next (0xC6384; its second figure, how many there are, is not
            // looked at). What waits for her is `s99`.
            234 => {
                self.field.scene.take_from(a(0), a(1), [position(&arguments[2..5]), position(&arguments[5..8])]);
                0
            }
            235 => {
                let scene = &mut *self.field.scene;
                let places = scene.taken_to.get(&a(0)).cloned().unwrap_or_default();
                // The frame it takes her into, if that is not the one its box is
                // in, is the one her leap is to change to (0xE5F28).
                let into = scene.traps.get(&a(0)).and_then(|trap| trap.takes_into.filter(|&into| into != trap.frame));
                debug!("trap {} takes her to {} places{}", a(0), places.len(), into.map_or(String::new(), |into| format!(", into frame {into}")));
                if let Some(player) = scene.characters.get_mut(&PLAYER) {
                    let speed = self.field.resources.speed(player.chr, status::RUN);
                    if !places.is_empty() {
                        player.group_next = into;
                    }
                    player.take_to(&places, speed);
                }
                0
            }

            // The things of a map's own, a model and its animations each (a gate,
            // a lift): one is made ready (0xC4E18, 0xE4044; here it is read when
            // it is first shown), and a character is made of one (0xC4E34,
            // 0xE4134), where the world begins until a script puts it, unless
            // there is a character of that number already.
            110 => 0,
            111 => {
                let scene = &mut *self.field.scene;
                if !scene.characters.contains_key(&a(0)) {
                    let defaults = scene.defaults;
                    scene.characters.insert(a(0), Character::new(object_resource(a(1)), Vec3::ZERO, 0.0, &defaults));
                }
                0
            }

            // The routes of the enemies that patrol: one of the map's points is
            // given its place and how far about it they may end up (0xC5AC4), and
            // a character's route is given one of them as its n-th (0xC5B00).
            180 => {
                self.field.scene.route_points.insert(a(0), (position(&arguments[1..4]), a(4) as f32));
                0
            }
            181 => {
                let place = usize::try_from(a(1)).ok().filter(|&place| place < ROUTE_POINTS);
                if let (Some(character), Some(place)) = (self.field.scene.characters.get_mut(&a(0)), place) {
                    character.mind.route[place] = a(2) as u8;
                }
                0
            }

            // One of the field's figures: the map, the entrance come in by, which
            // button did the last thing a button did and the word its sign had,
            // and the rest (0xC3E04). No script reads the two of the button.
            47 if a(0) == 3 => self.field.scene.chosen.0,
            47 if a(0) == 4 => self.field.scene.chosen.1,
            47 => system::read_value(self.field.scene, self.field.progress, self.field.tables, a(0)),
            // The field's windows from 5 on, and what the mission's display shows.
            // Below 5 are parts of the screen, each with a flag of the field's that
            // says whether it is up: opened by 0 and 2, closed by 1 and 3 (the odd
            // ones at once in the original, which the port does not tell apart).
            104 => {
                let closed = matches!(a(1), 1 | 3);
                let scene = &mut *self.field.scene;
                match a(0) {
                    1 => scene.set_flag(field_flag::PART_1_SHOWN, !closed),
                    2 => scene.set_flag(field_flag::PART_2_SHOWN, !closed),
                    3 => scene.set_flag(field_flag::GAUGE_HIDDEN, closed),
                    4 => scene.set_flag(field_flag::PART_4_HIDDEN, closed),
                    window if window >= FIRST_WINDOW => {
                        system::open_window(self.field.scene, self.field.progress, self.field.tables, window, a(1));
                    }
                    _ => {}
                }
                0
            }
            105 => {
                system::set_display(self.field.scene, self.field.progress, a(0), a(1), a(2));
                0
            }
            // The map of the stage, which is kept in the save: every map's script
            // sets it as the map is come into (0xC6830 to 0xC6D44).
            266 => {
                map_screen::reset(self.field.progress);
                0
            }
            267 => {
                map_screen::know(self.field.progress, a(0), a(1) != 0);
                0
            }
            268 => {
                map_screen::pin(self.field.progress, a(0), a(1), a(2));
                0
            }
            269 => {
                map_screen::mark(self.field.progress, 0, a(0), a(1));
                0
            }
            270 => {
                map_screen::here(self.field.progress, a(0), a(1));
                0
            }
            271 => {
                map_screen::hide(self.field.progress, a(0), a(1) != 0);
                0
            }
            278 => {
                map_screen::mark(self.field.progress, a(0), a(1), a(2));
                0
            }
            279 => {
                map_screen::mark_kind(self.field.progress, a(0), a(1));
                0
            }
            // On to another map, by one of its entrances, once this event is over:
            // the one named (0xE3474), or the one the trap that was walked into
            // leads to (0xE34C4).
            68 => {
                self.field.scene.map_change = Some((a(0), a(1)));
                0
            }
            69 => {
                let trap = match self.current {
                    Some(Callback::Trap(id)) => self.field.scene.traps.get(&id),
                    _ => None,
                };
                if let Some(Trap { kind: TrapKind::MapChange { map, entrance }, .. }) = trap {
                    self.field.scene.map_change = Some((*map, *entrance));
                }
                0
            }
            // The scene's lights (0xC5520, 0xC5848, 0xC55CC): the light all about,
            // in hundredths; the first two lights and that one put back as the
            // map's own scripts left them; and a light's first two colours.
            164 => {
                self.field.scene.lights.all_about([a(0), a(1), a(2)]);
                0
            }
            165 => {
                let scene = &mut *self.field.scene;
                scene.lights.put_back(&scene.lights_kept);
                0
            }
            166 => {
                self.field.scene.lights.colour(a(0), [a(1), a(2), a(3)], [a(4), a(5), a(6)]);
                0
            }
            // A film (0xC3170 to 0xC32F8): it is made ready, waited for, started,
            // and waited for to be over. One made while another is still there
            // takes its place. A skipped event has none, nor has a run that
            // skips every event it can.
            83 => {
                if !*self.fast && !self.skip_events {
                    let file = script.string(a(0))?.to_owned();
                    let scene = &mut *self.field.scene;
                    scene.film = Some(FilmAsked::new(file));
                    scene.set_flag(field_flag::FILM, true);
                }
                0
            }
            84 => {
                let waits = self.field.scene.film.as_ref().is_some_and(|film| !film.ready && !film.over);
                return Ok(self.wait_for(Wait::FilmReady, waits && !*self.fast));
            }
            85 => {
                if let Some(film) = &mut self.field.scene.film {
                    film.started = true;
                }
                0
            }
            86 => {
                let plays = self.field.scene.film.as_ref().is_some_and(|film| !film.over) && !*self.fast;
                if !plays {
                    end_film(self.field.scene);
                }
                return Ok(self.wait_for(Wait::FilmOver, plays));
            }
            // A battle; the sound comes back up for it if a script has it turned
            // down (0xC47B8).
            81 => {
                if self.field.scene.turned_down {
                    self.sound_up(BATTLE_SOUND_UP_FRAMES);
                }
                self.field.scene.battle = Some(BattleRequest { group: a(0) });
                return Ok(self.wait_for(Wait::Battle, true));
            }

            // Sound. A tune, unless it is the one a script last asked for
            // (0xC1D08); and the tune fades out over so many blocks of the sound's
            // (0xC1D64).
            57 => {
                let scene = &mut *self.field.scene;
                if a(0) != scene.tune {
                    scene.tune = a(0);
                    if let Some(tune) = self.field.tables.tune(a(0)) {
                        scene.sounds.push(Request::Music(sound::tune_file(tune)));
                        scene.tune_heard = false;
                    }
                }
                0
            }
            // Waits until the tune is playing (0xC5928: the sound is not busy,
            // 0xA6BC, and the music's stream plays, 0xA6CC): four scenes of the
            // last stage and two more start with it, to go by their tune.
            177 => return Ok(self.wait_for(Wait::Tune, !*self.fast)),
            58 => {
                self.field.scene.sounds.push(Request::StopMusic(a(0)));
                self.field.scene.tune = NO_TUNE;
                0
            }
            // A recording of the event that is loaded, on one stream or the other
            // (0xC277C, 0xC2830); a skipped event plays none. And a stream is
            // stopped (0xC27EC, 0xC28A0, 0xC29D4).
            29 | 202 => {
                if !*self.fast {
                    let stream = if number == 29 { STREAM_LINES } else { STREAM_OTHER };
                    let scene = &mut *self.field.scene;
                    let file = scene.staging.as_ref().and_then(|staging| staging.recording(a(0)));
                    if let Some(file) = file {
                        scene.sounds.push(Request::Say(stream, file.to_owned()));
                    }
                }
                0
            }
            30 => {
                self.field.scene.sounds.push(Request::Hush(STREAM_LINES, MAKE_WAY));
                self.field.scene.voice_begun = false;
                0
            }
            203 | 264 => {
                self.field.scene.sounds.push(Request::Hush(STREAM_OTHER, MAKE_WAY));
                0
            }
            // One of the jingles (0xC28C0).
            122 => {
                let file = format!("{JINGLES}/{:03}.riff", a(0));
                if a(0) >= 0 && self.data.exists(&file) {
                    self.field.scene.sounds.push(Request::Say(STREAM_OTHER, file));
                }
                0
            }
            // The sound is turned down to so much of 127 over so many frames, and
            // back up (0xC253C, 0xC25C8): the music, and the sounds about the map.
            112 => {
                let volume = a(0).clamp(0, FULL_VOLUME);
                let scene = &mut *self.field.scene;
                scene.sounds.push(Request::MusicVolume(volume, a(1) as f32 / FRAMES_A_BLOCK));
                scene.ambience_level.go(volume * 100 / FULL_VOLUME, a(1));
                scene.turned_down = true;
                0
            }
            // A sound of one of the banks (0xC23B8): the script is answered with
            // the number to stop it by (0xC24CC) or to change it by (0xC24E8). A
            // skipped event plays none.
            27 => {
                let volume = if a(2) < 0x80 { a(2).max(0) } else { FULL_VOLUME };
                match (self.field.scene.bank(a(0)), u32::try_from(a(1))) {
                    (Some(bank), Ok(sound)) if !*self.fast => self.field.scene.effect(bank, sound, volume) as i32,
                    _ => NO_SOUND,
                }
            }
            28 => {
                if let Ok(handle) = u32::try_from(a(0)) {
                    self.field.scene.sounds.push(Request::StopEffect(handle));
                }
                0
            }
            211 => {
                if let Ok(handle) = u32::try_from(a(0)) {
                    match a(1) {
                        SOUND_VOLUME => self.field.scene.sounds.push(Request::EffectVolume(handle, a(2))),
                        SOUND_PLACE => self.field.scene.sounds.push(Request::EffectPan(handle, a(2))),
                        _ => {}
                    }
                }
                0
            }
            // A sound of the map's bank is put at a place of the map, to be heard
            // from so far, between two volumes (0xC30E8, 0xD0AA4); taken away again
            // (0xC3134); made silent, or heard again (0xC3150).
            54 => {
                let place = usize::try_from(a(0)).ok().filter(|&place| place < AMBIENT_PLACES);
                let scene = &mut *self.field.scene;
                if let (Some(place), Some(bank), Ok(sound)) = (place, scene.bank(BANK_OF_MAP), u32::try_from(a(1))) {
                    // It starts unheard: how loud it is comes with the frame.
                    let handle = scene.effect(bank, sound, 0);
                    let ambient = Ambient::new(handle, a(5), a(6), position(&arguments[2..5]), a(7) as f32);
                    if let Some(old) = scene.ambience.insert(place, ambient) {
                        scene.sounds.push(Request::StopEffect(old.handle));
                    }
                }
                0
            }
            1 => {
                let scene = &mut *self.field.scene;
                if let Some(old) = usize::try_from(a(0)).ok().and_then(|place| scene.ambience.remove(&place)) {
                    scene.sounds.push(Request::StopEffect(old.handle));
                }
                0
            }
            123 => {
                let scene = &mut *self.field.scene;
                // Whether any of them is heard goes by the last one a script
                // spoke of, as in the original.
                scene.ambience_on = a(1) == 0;
                if let Some(ambient) = usize::try_from(a(0)).ok().and_then(|place| scene.ambience.get_mut(&place)) {
                    ambient.muted = a(1) != 0;
                    scene.sounds.push(Request::EffectVolume(ambient.handle, 0));
                }
                0
            }
            113 => {
                self.sound_up(a(0));
                0
            }
            156 => self.field.scene.report.outcome,
            157 => self.field.scene.report.second,
            // How the ride on the bike came out.
            260 => self.field.scene.work.ride_outcome,
            // The glare: how strong it is (0xC18D4), and whether only what is
            // marked takes part, which the second of these says with anything
            // but nothing first (0xC1B4C: with every scene of the story).
            153 => {
                self.field.scene.work.glare.set(a(0));
                0
            }
            254 => {
                if a(0) != 0 {
                    self.field.scene.work.glare.marked_only = a(1) != 0;
                }
                0
            }
            173 => {
                self.field.scene.leaving = Some(a(0));
                return Ok(self.wait_for(Wait::Leaving, true));
            }
            // The list of saves, to save the game (0xC59B8) or to load one
            // (0xC5A30): the script waits until it has gone. With no one to use
            // it, it is not put up.
            178 | 204 if self.skip_windows => 0,
            178 | 204 => {
                self.field.scene.saves = Some(if number == 178 { SavesAsked::Save } else { SavesAsked::Load });
                return Ok(self.wait_for(Wait::Saves, true));
            }
            // The window that tells something, and may ask with up to two choices
            // (0xC6618): the script waits for it to be read.
            252 => {
                let pages = text::pages(script.string(a(0))?, &Names { variables, items: self.field.items });
                let choices = [script.string_or_none(a(1))?, script.string_or_none(a(2))?];
                let choices: Vec<&str> = choices.into_iter().flatten().collect();
                return Ok(self.open(Window::new(pages, &choices, 0, 0), Wait::Window));
            }
            // The window that asks a question (0xC1EC4): the text, nine choices of
            // which the unused are blank, the one the cursor starts on and the one
            // circle goes to. `s75` waits for the answer and `s76` reads it.
            95 => {
                let pages = text::pages(script.string(a(0))?, &Names { variables, items: self.field.items });
                let choices: Vec<&str> = arguments[1..1 + CHOICES].iter().map(|&choice| script.string(choice)).collect::<Result<_>>()?;
                let window = Window::new(pages, &choices, a(1 + CHOICES), a(2 + CHOICES));
                if self.skip_windows {
                    self.field.scene.selection = window.selected as i32;
                } else {
                    self.field.scene.window = Some(window);
                }
                0
            }
            75 => {
                if self.field.scene.window.is_none() {
                    self.field.scene.answer = self.field.scene.selection;
                }
                return Ok(self.wait_for(Wait::Question, self.field.scene.window.is_some()));
            }
            76 => self.field.scene.answer,

            // An event of the game's list is played (0xC63B4, 0xD7774): they queue,
            // and the script goes on meanwhile unless it waits for them (0xC644C).
            // An event that is skipped has none played, as one run through fast has
            // not in the original.
            237 => {
                if !*self.fast {
                    self.field.scene.events.push_back((a(0), a(1)));
                }
                0
            }
            238 => {
                let playing = !self.field.scene.events.is_empty() || self.field.scene.event_running;
                return Ok(self.wait_for(Wait::Events, playing));
            }

            // The pictures that go with a lesson, by slot: loaded (0xCF1C0), waited
            // for, brought in or taken out (0xC22E4), let go (0xCF250). A picture is
            // there to be shown as soon as it is named: nothing waits for it here.
            248 => {
                let name = script.string(a(1))?;
                self.field.scene.pictures.entry(a(0)).or_insert_with(|| Picture::new(name));
                0
            }
            249 => 0,
            250 => {
                // A skipped event leaves its pictures as they are.
                let scene = &mut *self.field.scene;
                if !*self.fast && scene.pictures.contains_key(&a(0)) {
                    scene.pictures.get_mut(&a(0)).expect("it is there").fade(a(1), a(2), a(3));
                    // Kinds 0 and 1 have the screen fade instead of the picture, from
                    // or to black and white (0xD0808 on): over the same frames, which
                    // the screen's fade counts as the game's (0x2980C).
                    if let (way @ (0 | 1), kind @ (0 | 1)) = (a(1), a(2)) {
                        scene.fade.restart(way == 1, kind == 1, a(3) / FRAMES_A_STEP as i32);
                    }
                }
                0
            }
            251 => {
                self.field.scene.pictures.remove(&a(0));
                0
            }
            // A press of cross (0xC235C).
            258 => return Ok(self.wait_for(Wait::Press, !self.skip_windows)),

            // The scene alone is faded, under everything of the interface: begun
            // (0xC21C0), waited for (0xC2238) and done with (0xC2254).
            197 => {
                self.field.scene.scene_fade = Some(SceneFade::new(a(0), a(1), a(2)));
                0
            }
            198 => return Ok(self.wait_for(Wait::SceneFade, true)),
            199 => {
                self.field.scene.scene_fade = None;
                0
            }

            // She is made whole (0xC51C4: whatever it is given, 0xD88).
            134 => {
                self.field.stats.hp = self.field.stats.max_hp;
                0
            }
            // She is given so many of an item, or has them taken (0xC4DE4); only
            // for her does it do anything.
            109 => {
                if a(0) == 0 {
                    self.field.inventory.add(self.field.items, a(1), a(2));
                }
                0
            }
            // How many of an item she has (0xC4D7C, 0x1BF4); of anybody else, none.
            107 => {
                if a(0) == 0 {
                    i32::from(self.field.inventory.held(a(1)))
                } else {
                    0
                }
            }
            // A number by chance, from nought up to but not reaching the one given
            // (0xC2620, 0x93A0: the part of one that a fraction of it comes to).
            // The original's own run of numbers is not kept.
            130 => {
                *self.random ^= *self.random << 13;
                *self.random ^= *self.random >> 17;
                *self.random ^= *self.random << 5;
                ((*self.random & 0xffff) as f32 / 65536.0 * a(0) as f32) as i32
            }
            // What the field has loaded is let go to make room for a battle of
            // note, with the sign of the disc up meanwhile (0xC6F7C), and her
            // model is made again of what she now has on (0xC67E4): the port
            // keeps everything, and makes her model again whenever that changes.
            281 | 263 => 0,
            // Her figures become those of a record of the start table (0xC6788,
            // 0xE10, 0x14C9D4). The opening's script gives her the second when
            // its film is over: until then she is as the game's own start made
            // her, with every figure at its most.
            262 => {
                let field = &mut *self.field;
                field.tables.starts.give(a(0), &mut *field.stats, field.items, &mut *field.inventory, &mut *field.progress);
                0
            }
            // The game is cleared (0x340): so many times, which a save has from
            // then on, and by its low bit the system data says so, which the
            // title goes by. The last map's script says it when the story has
            // been played to its end.
            205 => {
                self.field.scene.game_cleared = Some(a(0));
                0
            }
            // An entry of the field's system script, which the script waits for.
            247 => {
                if a(0) == SYSTEM_ENTRY {
                    self.field.scene.system = Some(a(1));
                    return Ok(self.wait_for(Wait::System, true));
                }
                0
            }
            // One of the field's figures (0xC726C, by the table at 0x16B7F0).
            96 => {
                match a(0) {
                    VALUE_EVENT_FRAME => self.field.scene.event_frame = a(1),
                    VALUE_TUNE_KEPT => self.field.scene.set_flag(field_flag::TUNE_KEPT, a(1) != 0),
                    // With nought nobody that stands idles from now on, and
                    // everybody who is not doing something stands (0xE5978).
                    VALUE_IDLING => {
                        let quiet = a(1) == 0;
                        self.field.scene.set_flag(field_flag::NO_IDLING, quiet);
                        if quiet {
                            for character in self.field.scene.characters.values_mut().filter(|character| !character.acting()) {
                                character.idle.settle();
                            }
                        }
                    }
                    // The figure is a whole number made a float (0xC72DC), and
                    // each of the two is so with nought (0xC72F8, 0xC7388).
                    VALUE_REACH => self.field.scene.too_close = Some(a(1) as f32),
                    VALUE_ALL_IN_SIGHT => self.field.scene.set_flag(field_flag::ALL_IN_SIGHT, a(1) == 0),
                    VALUE_ALL_NEAR => self.field.scene.set_flag(field_flag::ALL_NEAR, a(1) == 0),
                    // Nothing here, where every character is placed anew each frame.
                    VALUE_PLACED_AT_ONCE => {}
                    value::POPS | value::NEXT_MAP | value::NEXT_ENTRANCE | value::OBJECTIVE | value::LOSS_EXIT | 54 | value::MISSION_SECONDS | 59 | 60 => {
                        if system::write_value(self.field.scene, self.field.progress, a(0), a(1)) {
                            self.field.scene.display_stale = true;
                        }
                    }
                    // Which ride on the bike the field is about to be left for.
                    VALUE_RIDE => self.field.scene.work.ride = a(1),
                    // The original has figures 5 to 60 and does nothing with any
                    // other (0xC7278); one script gives 62.
                    other if !(5..=60).contains(&other) => {}
                    other => {
                        if self.unported_values.insert(other) {
                            debug!("script function s96({other}, ..) is not ported");
                        }
                    }
                }
                0
            }
            // The player's figures.
            87 => {
                let stats = &*self.field.stats;
                match a(0) {
                    FIGURE_EXPERIENCE => stats.experience as i32,
                    FIGURE_LEVEL => stats.level,
                    FIGURE_HP => stats.hp as i32,
                    FIGURE_MAX_HP => stats.max_hp as i32,
                    _ => 0,
                }
            }
            88 => {
                let stats = &mut *self.field.stats;
                match a(0) {
                    FIGURE_HP => stats.hp = (a(1) as f32).clamp(0.0, stats.max_hp),
                    other => debug!("script function s88({other}, ..) is not ported"),
                }
                0
            }

            // The camera: its kind, and its settings by number.
            // The camera as it is is put by for the time of an event, and taken up
            // again after it (0xDA324, 0xDA40C).
            50 => {
                self.field.scene.keep_camera();
                0
            }
            51 => {
                self.field.scene.take_up_camera();
                0
            }
            49 => {
                self.field.scene.set_camera_kind(a(0));
                0
            }
            79 => {
                if matches!(a(0), camera::figure::NEAR | camera::figure::FAR) {
                    debug!("the camera's figure {} is {}: how near or how far it sees", a(0), a(1));
                }
                self.field.scene.camera.set(a(0), a(1) as f32);
                0
            }
            // The camera goes along a path of the map's (0xE2F34).
            124 => {
                self.field.scene.set_camera_kind(camera::kind::PATH);
                self.field.scene.camera.set_path(a(0));
                0
            }
            // A figure of the camera goes to a value over so many frames (0xC8D30):
            // in one of four ways, any other number being the first.
            139 => {
                let ease = if (0..=3).contains(&a(3)) { a(3) } else { 0 };
                self.field.scene.camera.move_to(a(0), a(1) as f32, a(2), ease);
                0
            }

            _ => {
                if self.unported.insert(number) {
                    debug!("script function s{number} is not ported");
                }
                0
            }
        };
        Ok(Reply::value(value))
    }
}

impl Engine<'_, '_> {
    /// `s113`: the sound comes back up over so many frames.
    fn sound_up(&mut self, frames: i32) {
        let scene = &mut *self.field.scene;
        scene.sounds.push(Request::MusicVolume(FULL_VOLUME, frames as f32 / FRAMES_A_BLOCK));
        scene.ambience_level.go(100, frames);
        scene.turned_down = false;
    }


    /// Has the script wait if there is something to wait for.
    fn wait_for(&mut self, wait: Wait, needed: bool) -> Reply {
        if needed {
            *self.wait = Some(wait);
        }
        Reply { value: 0, suspend: needed }
    }

    /// Puts a window up and has the script wait for it; or, with no one to read it,
    /// takes it as read.
    fn open(&mut self, window: Window, wait: Wait) -> Reply {
        if self.skip_windows {
            self.field.scene.selection = window.selected as i32;
            return Reply::NONE;
        }
        self.field.scene.window = Some(window);
        self.wait_for(wait, true)
    }

    /// `s7(way, colour, frames)`: 1 takes the screen away, 0 brings it back
    /// (`Fade::script`). A skipped event does not fade (0xC7020).
    fn fade(&mut self, way: i32, colour: i32, frames: i32) {
        if *self.fast {
            return;
        }
        // A picture that is to stay is one of the screen as it is now.
        if self.field.scene.fade.script(way, colour, frames) {
            self.field.scene.capture = Capture::Wanted { whole: false };
        }
    }

    /// Has a picture of the screen taken and the script wait for it (0xE4790,
    /// 0xE47C0); nothing for a script that is being run through fast.
    fn take_picture(&mut self, whole: bool, hold: bool) -> Reply {
        if *self.fast {
            return Reply::NONE;
        }
        self.field.scene.capture = Capture::Wanted { whole };
        self.wait_for(Wait::Picture { hold }, true)
    }

    /// `s42`: makes a character unless there is one of that number already (0xE1844).
    fn create(&mut self, id: i32, chr: i32, position: Vec3, facing: f32) {
        let scene = &mut *self.field.scene;
        if scene.characters.contains_key(&id) {
            return;
        }
        if self.field.tables.character_bundle(chr).is_none() {
            warn!("character {id} is of resource {chr}, which the field's list does not have");
        }
        let defaults = scene.defaults;
        scene.characters.insert(id, Character::new(chr, position, facing, &defaults));
        self.place(id, position, facing);
        // Where it is made is its home, until its behaviour says otherwise.
        if let Some(character) = self.field.scene.characters.get_mut(&id) {
            character.target = character.position;
            character.mind.home = character.position;
        }
    }

    /// `s53`: makes a character of an effect unless there is one of that number
    /// already (0xE3028), which it is not if there is no such effect; and whether
    /// it has made one or not, has the character of that number be something to
    /// check and its effect begin (0xC4038).
    fn make_effect(&mut self, id: i32, kind: i32, number: i32) {
        let scene = &mut *self.field.scene;
        if !scene.characters.contains_key(&id) {
            let map = usize::try_from(scene.map).ok().and_then(|map| self.field.tables.maps.get(map));
            let file = match kind {
                EFFECT_OF_MAP => map.map(|map| format!("FLD/MAP/MAP_{}.BIN/{EFFECTS_SLOT}/{number:02}.efc", map.name.to_ascii_uppercase())),
                EFFECT_OF_FIELD => Some(format!("{FIELD_EFFECTS}/{number:02}.bin/{FIELD_EFFECT}")),
                EFFECT_OF_EVENT => scene.staging.as_ref().and_then(|staging| staging.effect(number)).map(str::to_owned),
                _ => None,
            };
            match file.filter(|file| number >= 0 && self.data.exists(file)) {
                Some(file) => {
                    debug!("character {id} is made of the effect {file}");
                    let defaults = scene.defaults;
                    scene.characters.insert(id, Character::of_effect(file, &defaults));
                }
                None => debug!("character {id} is not made: there is no effect {number} of kind {kind}"),
            }
        }
        if let Some(character) = scene.characters.get_mut(&id) {
            character.begin_effect();
        }
    }

    /// `s48`: puts a character somewhere, dropped onto the ground if there is ground
    /// near enough under or over the place (0xE19E4), and stops it.
    fn place(&mut self, id: i32, position: Vec3, facing: f32) {
        let scene = &mut *self.field.scene;
        let Some(character) = scene.characters.get_mut(&id) else { return };
        let mut position = position;
        if !character.flags.has(flag::NO_DROP) {
            // The ground is looked for along its own upright, as the frame it
            // is in lies (0x108520).
            let lie = scene.frames.lie(self.field.world, character.group());
            if let Some(on) = lie.under(position, DROP_REACH, DROP_REACH) {
                position = on;
            }
        }
        character.place(position, facing);
    }

    /// `s41`: lets go of a character resource once nothing has asked for it.
    fn release(&mut self, chr: i32) {
        let resources = &mut self.field.scene.resources;
        if let Some(count) = resources.get_mut(&chr) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                resources.remove(&chr);
            }
        }
    }


    /// `s132`: has a character do something (0xC5064).
    fn act(&mut self, id: i32, action: i32) {
        let Some(character) = self.field.scene.characters.get_mut(&id) else { return };
        let depth = self.field.resources.pop_height(character.chr);
        let way = self.field.resources.destroy_kind(character.chr);
        match action {
            // In a skipped event a walk is over at once, where it was going
            // (0xC50AC); everything else is done as ever.
            ACT_WALK if *self.fast => character.arrive(),
            ACT_WALK => character.walk(),
            ACT_TURN => character.turn(),
            ACT_DESTROY => character.destroy(way),
            ACT_SHIFT_FACE => {
                character.shift_face();
                debug!("character {id} goes over to a face: {:?}", character.expression);
            }
            // From seven on it is one of the ways of coming to where it stands,
            // its face (above) or nothing (0xE6068): 3, 4, 7, 8, 11 and 13 do
            // nothing in the original either.
            _ => {
                character.come_by(action, depth);
            }
        }
    }

    /// `s179`: the way from one character to another, in whole degrees.
    fn bearing(&self, from: i32, to: i32) -> i32 {
        let characters = &self.field.scene.characters;
        let (Some(from), Some(to)) = (characters.get(&from), characters.get(&to)) else { return 0 };
        from.bearing(to.position - from.position).to_degrees() as i32
    }

    /// Gives a character an animation of the script's choosing, or takes it away.
    fn motion(&mut self, id: i32, motion: Option<Motion>) {
        if let Some(character) = self.field.scene.characters.get_mut(&id) {
            character.name_motion(motion);
        }
    }

    /// `s70`: sets the animation a character has had named playing. How long
    /// one an event brought is, is known here; one of its own may yet have to
    /// be read, and is asked after as it plays.
    fn play(&mut self, id: i32) {
        let scene = &mut *self.field.scene;
        let Some(character) = scene.characters.get_mut(&id) else { return };
        let frames = match character.named {
            Some(Motion::Staged(slot, number)) => scene.staging.as_ref().and_then(|staging| staging.motion(slot, number)).map(|clip| clip.frames() / ANIMATION_FRAMES_A_STEP),
            Some(Motion::Own(number)) => self.field.resources.motion_frames(character.chr, number),
            None => None,
        };
        character.play_motion(frames.map(|frames| (frames * FULL_PACE as f32) as i32));
    }

    /// Gives a character motion `number` of slot `slot` of the event that is loaded,
    /// if the event has it (0xE9024).
    fn staged(&mut self, id: i32, slot: usize, number: i32) {
        let has = self.field.scene.staging.as_ref().is_some_and(|staging| staging.motion(slot, number).is_some());
        debug!("character {id}: motion {number} of slot {slot}{}", if has { "" } else { ", which the event does not have" });
        self.motion(id, has.then_some(Motion::Staged(slot, number)));
    }
}

/// A place given as three whole numbers.
fn position(arguments: &[i32]) -> Vec3 {
    Vec3::new(arguments[0] as f32, arguments[1] as f32, arguments[2] as f32)
}

/// An angle given in whole degrees, in radians.
fn degrees(value: i32) -> f32 {
    (value as f32).to_radians()
}

/// The header of the bank of sounds of map `number`, if the map has one.
pub fn map_bank(data: &GameData, tables: &FieldTables, number: i32) -> Option<String> {
    let map = usize::try_from(number).ok().and_then(|number| tables.maps.get(number))?;
    Some(format!("FLD/MAP/MAP_{}.BIN/{BANK_SLOT}", map.name.to_ascii_uppercase())).filter(|bank| data.exists(bank))
}

/// Loads the script of the map called `name` (as in `d_stg01_01`), if it has one.
pub fn load(data: &crate::data::GameData, name: &str) -> Result<Option<MapScript>> {
    let path = format!("FLD/MAP/MAP_{}.BIN/10.bscr", name.to_ascii_uppercase());
    if !data.exists(&path) {
        return Ok(None);
    }
    let script = bscr::Script::parse(&data.read(&path)?).with_context(|| format!("parsing {path}"))?;
    Ok(Some(MapScript::new(script, data.clone())))
}

/// What a new scene starts from: the field's tunables (0x109728 and 0xF9A00).
pub fn defaults(tables: &FieldTables) -> Result<Defaults> {
    let tunables = &tables.tunables;
    Ok(Defaults {
        touch_radius: tunables.u16(0x0a)? as f32,
        territory: tunables.u16(0x42)? as f32,
        pace: tunables.u16(0x44)? as i32,
        notice_range: tunables.u16(0x48)? as f32,
        wander_radius: WANDER_RADIUS,
    })
}

/// How far from home a symbol wanders: a constant of the original's (0xF9A00).
const WANDER_RADIUS: f32 = 300.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::field::motion::tests::Speeds;
    use crate::field::scene::{field_flag, flag, status, Action, BattleReport, NO_RESOURCE, OUTCOME_ESCAPED, OUTCOME_WON, PLAYER};
    use crate::controls::{button, Pad};
    use crate::field::scene::value;
    use crate::field::system::{SystemScript, World};
    use crate::items::{Inventory, Items};
    use crate::progress::variable;
    use crate::sound::Request;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");
    const FIRST_MAP: &str = "d_stg01_01";
    /// The map a new game starts on, whose script plays the opening.
    const OPENING_MAP: &str = "d_stg05_0a1";
    /// What a mission's progress stands at once the system script has started it.
    const MISSION_STARTED: i32 = 101;

    struct Loaded {
        tables: FieldTables,
        script: MapScript,
        system: SystemScript,
        scene: Scene,
        progress: Progress,
        stats: PlayerStats,
        world: CollisionWorld,
        items: Items,
        inventory: Inventory,
        /// What the player is pressing, for the windows the script puts up.
        pad: Pad,
        records: Records,
    }

    /// The first map as a new game finds it, if the game's data has been unpacked.
    fn first_map() -> Option<Loaded> {
        on_map(FIRST_MAP)
    }

    /// A map as a game that has just begun finds it.
    fn on_map(name: &str) -> Option<Loaded> {
        let data = GameData::new(DATA).ok()?;
        let tables = FieldTables::load(&data).expect("the field's tables load");
        let script = load(&data, name).expect("the map's script parses").expect("the map has a script");
        let system = SystemScript::load(&data, &tables).expect("the system script loads");
        let map = tables.map_number(name).expect("the map is in the list") as i32;
        let scene = Scene::new(map, 0, defaults(&tables).unwrap());
        let stats = PlayerStats::new_game(&data).expect("the start table loads");
        let items = Items::load(&data).expect("the item tables load");
        let inventory = Inventory::new_game(&items);
        let mut game = Loaded {
            tables,
            script,
            system,
            scene,
            progress: Progress::start(),
            stats,
            world: CollisionWorld::default(),
            items,
            inventory,
            pad: Pad::default(),
            records: Records::load(&data).expect("the records' script loads"),
        };
        // With no one at the buttons, windows are taken as read; the tests that are
        // about them put them up.
        game.script.skip_windows = true;
        // As a new game does it: the system script's part, the first mission, the map.
        game.system(|system, world| system.call("init_player", &[], world).map(|_| ()));
        game.progress.begin_first_mission();
        game.system(|system, world| system.call("sys_fld_enter_map", &[], world).map(|_| ()));
        Some(game)
    }

    impl Loaded {
        fn with<T>(&mut self, run: impl FnOnce(&mut MapScript, &mut Field) -> Result<T>) -> T {
            let mut field = Field {
                progress: &mut self.progress,
                stats: &mut self.stats,
                scene: &mut self.scene,
                tables: &self.tables,
                world: &self.world,
                resources: &Speeds,
                items: &self.items,
                inventory: &mut self.inventory,
                pad: &self.pad,
                records: &self.records,
            };
            run(&mut self.script, &mut field).expect("the script runs")
        }

        fn system(&mut self, run: impl FnOnce(&mut SystemScript, &mut World) -> Result<()>) {
            let mut world = World {
                progress: &mut self.progress,
                scene: &mut self.scene,
                stats: &mut self.stats,
                tables: &self.tables,
                ground: &self.world,
            };
            run(&mut self.system, &mut world).expect("the system script runs")
        }

        /// A frame of the map's script and then of the system script, as the field
        /// runs them.
        fn frame(&mut self) {
            self.with(|script, field| script.update(field));
            self.system(|system, world| system.update(world));
            // With no one at the buttons the window of a mission cleared is taken
            // as read.
            if let Some(cleared) = &mut self.scene.cleared {
                cleared.read();
            }
            // Standing in for the field: a picture of the screen that is asked for
            // has been taken by the next frame.
            self.scene.capture = Capture::None;
            // And for what plays films: one is ready as soon as it is asked for,
            // and over as soon as it is started.
            if let Some(film) = &mut self.scene.film {
                (film.ready, film.over) = (true, film.started);
            }
            // And what there is to tell of a record has been told, and the
            // windows a script asks for shown.
            while self.records.take_news(&mut self.progress).is_some() {}
            if std::mem::take(&mut self.scene.record_windows) {
                self.records.shown(records::OF_MISSIONS, &mut self.progress);
            }
        }

        /// A frame with the field stood in for: the picture's fade goes on, and
        /// whatever a character is doing ends at once.
        fn stood_in_frame(&mut self) {
            self.scene.fade.advance();
            for character in self.scene.characters.values_mut() {
                if character.acting() {
                    let destroyed = matches!(character.action, Action::Destroy { .. });
                    character.stop();
                    character.flags.set(flag::HIDDEN, destroyed);
                }
            }
            self.frame();
        }

        /// Runs frames until the script is done or waits for a battle.
        fn settle(&mut self) {
            for _ in 0..100_000 {
                if !self.script.busy() || self.scene.battle.is_some() || self.scene.leaving.is_some() {
                    return;
                }
                self.stood_in_frame();
            }
            panic!("the script never came to rest");
        }
    }

    /// The first map with its opening event skipped through: a new game as it stands
    /// when the player is first given control.
    fn opened() -> Option<Loaded> {
        let mut game = first_map()?;
        // The field makes the player before the script runs.
        let player = Character::new(0, Vec3::ZERO, 0.0, &game.scene.defaults);
        game.scene.characters.insert(PLAYER, player);
        game.script.skip_events = true;
        game.with(|script, field| script.open(field));
        game.settle();
        // The opening has the game's first battle in it, which is never skipped.
        assert_eq!(game.scene.battle, Some(BattleRequest { group: 2 }));
        game.scene.battle = None;
        game.scene.report.outcome = OUTCOME_WON;
        game.settle();
        assert!(!game.script.busy(), "the opening event should have run to its end");
        game.script.skip_events = false;
        // What is said as the mission starts is let run out, so that the tests
        // begin with nothing going on.
        let mut frames = 0;
        while game.scene.event_running || !game.scene.events.is_empty() {
            game.frame();
            frames += 1;
            assert!(frames < 10_000, "the events after the opening never came to an end");
        }
        eprintln!("the mission's first words took {frames} frames");
        Some(game)
    }

    #[test]
    fn the_opening_shows_her_at_her_most_and_leaves_her_as_a_new_game_has_her() {
        use crate::battle::tables::Starts;
        let Some(mut game) = on_map(OPENING_MAP) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // As the game's own start leaves her (0x13C): the start table's first
        // record, and nothing carried.
        game.stats = PlayerStats::NONE;
        game.stats.start(game.tables.starts.get(Starts::OPENING).unwrap(), &game.items);
        game.inventory = Inventory::default();
        let player = Character::new(0, Vec3::ZERO, 0.0, &game.scene.defaults);
        game.scene.characters.insert(PLAYER, player);
        game.script.skip_events = true;
        game.with(|script, field| script.open(field));
        game.settle();
        // The first lesson's battle, which she fights as the opening shows her.
        assert!(game.scene.battle.is_some(), "the opening should have come to its battle");
        assert_eq!((game.stats.level, game.stats.max_hp, game.stats.attack, game.stats.weapon), (50, 9999.0, 999, 131));
        game.scene.battle = None;
        game.scene.report.outcome = OUTCOME_WON;
        game.settle();
        eprintln!("not ported: {:?}", game.script.unported());

        // Its film over, she is who a new game has, with what a new game has:
        // the Rock Cannon, the Normal Suit and three Repairers.
        let data = GameData::new(DATA).unwrap();
        assert_eq!(game.stats, PlayerStats::new_game(&data).unwrap());
        assert_eq!(game.inventory.all().collect::<Vec<_>>(), [(1, 3), (129, 1), (133, 1)]);
    }

    #[test]
    fn the_opening_leaves_the_story_where_the_first_mission_begins() {
        let Some(game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        eprintln!("not ported: {:?}", game.script.unported());

        let player = game.scene.player().unwrap();
        assert_eq!(player.position, Vec3::new(1425.0, 0.0, 5656.0));
        assert!((player.facing().to_degrees() + 132.0).abs() < 1e-3);
        // The opening has been seen, and the mission has just been started.
        assert_eq!(game.progress.variable(10_003), 1_010_550);
        assert_eq!(game.progress.variable(variable::MISSION_PROGRESS), MISSION_STARTED);
        assert!(game.progress.flag(5000));
        // The camera is behind her.
        assert_eq!((game.scene.camera.kind, game.scene.camera.figure(camera::figure::YAW)), (camera::kind::FOLLOW, 47.0));

        // Six Eaters roam the streets; the three Hornets and the trailer's guard are
        // made but hidden until the mission gets to them.
        let symbols: Vec<i32> = game.scene.characters.keys().copied().filter(|id| (500..600).contains(id)).collect();
        assert_eq!(symbols, [500, 501, 502, 503, 504, 505, 521, 522, 523, 527, 528, 529, 530, 531]);
        for id in 500..=505 {
            let eater = &game.scene.characters[&id];
            assert_eq!(eater.chr, 21);
            assert!(eater.flags.has(flag::SYMBOL) && !eater.hidden());
            assert_eq!(eater.behaviour, Some(0));
        }
        assert_eq!(game.scene.characters[&502].mind.notice_range, 2500.0);
        assert_eq!(game.scene.characters[&500].mind.notice_range, 1800.0);
        for id in [521, 522, 523, 527, 531] {
            assert!(game.scene.characters[&id].hidden(), "{id} should be hidden");
        }
        assert_eq!(game.scene.characters[&521].chr, 22);
        // The standing kind is only touchable.
        let guard = &game.scene.characters[&527];
        assert!(guard.flags.has(flag::TOUCHABLE) && !guard.flags.has(flag::SYMBOL));
        assert_eq!(guard.behaviour, None);

        // The story's people have come and gone.
        assert!(!game.scene.characters.contains_key(&600));
        // The barricade across the street to the south, and the wall by the trailer.
        let barricade = &game.scene.walls[&800];
        assert_eq!((barricade.centre, barricade.reach), (Vec3::new(-400.0, 0.0, -1470.0), Vec3::new(1000.0, 150.0, 100.0)));
        assert!(!barricade.open && game.scene.walls.contains_key(&810));
        // The way out of the map, the event gates across the street, and the picture
        // is back.
        assert!(matches!(game.scene.traps[&0].kind, TrapKind::MapChange { map: 6, entrance: 1 }));
        assert!(!game.scene.traps[&500].off && game.scene.traps[&501].off && game.scene.traps[&502].off);
        assert_eq!(game.scene.fade.level, 0.0);
    }

    #[test]
    fn the_opening_scene_is_staged_cut_by_cut_with_what_is_said() {
        let Some(mut game) = first_map() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let player = Character::new(0, Vec3::ZERO, 0.0, &game.scene.defaults);
        game.scene.characters.insert(PLAYER, player);
        game.with(|script, field| script.open(field));

        // When the picture has gone dark the scene's event is loaded and the scene
        // set; the title of its place is typed out over the dark, three lines of
        // it, stands and goes; and then the camera's first cut starts.
        let (mut frames, mut typed, mut titled) = (0, 0, 0);
        while game.scene.cut.is_none() {
            game.frames(1);
            frames += 1;
            assert!(frames < 300, "the first cut never started");
            if let Some(title) = game.scene.location.as_ref().filter(|title| title.shown()) {
                titled += 1;
                typed = typed.max(title.sign.rows.iter().flatten().filter(|cell| cell.alpha == 0xff).count());
                assert_eq!((title.sign.rows[0][0].at, game.scene.fade.level), ([6, 6], 1.0));
            }
        }
        eprintln!("the title of the scene's place was up for {titled} frames");
        assert_eq!((typed, game.scene.location.is_none()), (17 + 12 + 13, true));
        assert!((250..290).contains(&titled), "{titled}");
        let letters = game.scene.sounds.iter().filter(|sound| matches!(sound, Request::Play { sound: 9, volume: 0x2a, .. })).count();
        assert_eq!(letters, 17 + 12 + 13 - 2);
        assert_eq!(game.scene.staging.as_ref().map(|staging| staging.event), Some(169));
        assert_eq!(game.scene.camera.kind, camera::kind::CUT);
        // The camera she is followed with has been put by, and the mission's display
        // is down for the scene.
        let mut put_by = game.scene.camera.clone();
        assert!(put_by.take_up() && !put_by.take_up());
        assert!(game.scene.flag(field_flag::GAUGE_HIDDEN));
        // Everyone in the scene is in the hands the scenes use, and she is hidden.
        let soldier = &game.scene.characters[&104];
        assert_eq!(soldier.chr, 17);
        let part = |index: u32| soldier.flags.has(flag::PART + index);
        assert_eq!([part(2), part(3), part(4), part(5)], [false, true, false, true]);
        assert!(game.scene.player().unwrap().hidden());

        // The first cut is 658 frames of animation, two to a frame of the game, the
        // first two in the frame it starts in; the second starts the frame after it
        // ends.
        let first = game.scene.cut.as_ref().unwrap().pose(0.0);
        game.frames(327);
        assert!(game.scene.cut.as_ref().unwrap().playing);
        game.frames(1);
        assert!(!game.scene.cut.as_ref().unwrap().playing);
        let last = game.scene.cut.as_ref().unwrap().pose(0.0);
        assert!(first.transform.translation.distance(last.transform.translation) > 100.0);
        game.frames(1);
        assert!(game.scene.cut.as_ref().unwrap().playing, "the second cut should have started");
        assert_ne!(game.scene.cut.as_ref().unwrap().pose(0.0), last);

        // On through the scene to the battle it ends in, noting what is said and
        // which events stage it.
        let (mut said, mut events): (Vec<String>, Vec<i32>) = (Vec::new(), vec![169]);
        let mut staged = false;
        while game.scene.battle.is_none() {
            game.frames(1);
            frames += 1;
            assert!(frames < 30 * 240, "the opening never came to its battle");
            let event = game.scene.staging.as_ref().map(|staging| staging.event);
            if let Some(event) = event.filter(|event| events.last() != Some(event)) {
                events.push(event);
            }
            staged |= game.scene.characters.values().any(|character| matches!(character.motion, Some((Motion::Staged(..), _))));
            for caption in game.scene.captions.iter().flat_map(|captions| captions.shown()) {
                let line = text::plain(&caption.text);
                if !said.contains(&line) {
                    said.push(line);
                }
            }
        }
        eprintln!("the opening took {frames} frames to its battle; said: {said:#?}");
        assert_eq!(events, [169, 170, 171]);
        assert!(staged, "the cast should have been given the event's motions");
        assert_eq!(
            said[..9],
            [
                "Come on, wake up!",
                "Jon!",
                "We're surrounded!",
                "Wake her up, dammit!",
                "I've done all I can!\nWe just have to wait!",
                "We need a prince to kiss her or something.",
                "Pipe down, noodle head!",
                "Okay, then\u{2015}\u{2015}!",
                "Tonio!",
            ]
        );
        assert_eq!(game.scene.battle, Some(BattleRequest { group: 2 }));

        // What is heard: the first line, then the scene's tune, then something on
        // the other stream; and the battle's tune as the battle is asked for.
        let sounds = &game.scene.sounds;
        let line = |number: usize| format!("FLD/FEVT/FEVT_1_1_0200B_1.BIN/05.bin/{number:02}.riff");
        let at = |request: Request| sounds.iter().position(|sound| *sound == request).unwrap_or_else(|| panic!("{request:?} was not asked for"));
        let (first, tune, other) = (at(Request::Say(0, line(0))), at(Request::Music("SOUND/BGM_EVENT4.AT3".into())), at(Request::Say(1, line(2))));
        assert!(first < tune && tune < other);
        let tunes: Vec<&Request> = sounds.iter().filter(|sound| matches!(sound, Request::Music(_))).collect();
        assert_eq!(tunes.last(), Some(&&Request::Music("SOUND/BGM_BATTLE1.AT3".into())));
        assert_eq!(game.scene.tune, 10);
        // Every recording the script asks for is one the scene's three events have.
        let lines = sounds.iter().filter(|sound| matches!(sound, Request::Say(..))).count();
        assert_eq!((lines, tunes.len()), (44, 2));
        // The scene's noises are sounds of its events' banks, six of them, each
        // known by a number of its own; a bank goes when its event does.
        let first_bank = "FLD/FEVT/FEVT_1_1_0200B_1.BIN/06.bin/00.pphd";
        let noises: Vec<(u32, &str, u32)> = sounds
            .iter()
            .filter_map(|sound| match sound {
                Request::Effect { handle, bank, sound, .. } => Some((*handle, bank.as_str(), *sound)),
                _ => None,
            })
            .collect();
        assert!(noises.iter().any(|&(_, bank, sound)| bank == first_bank && sound == 0));
        let handles: BTreeSet<u32> = noises.iter().map(|&(handle, ..)| handle).collect();
        assert_eq!((noises.len(), handles.len()), (6, 6));
        assert!(sounds.contains(&Request::DropBank(first_bank.into())));
    }

    #[test]
    fn a_tune_asked_for_again_goes_on_and_the_sound_comes_back_up_for_a_battle() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The map's script has had the field's tune played as it set the map up.
        assert_eq!(game.scene.tune, 2);
        assert!(game.scene.sounds.contains(&Request::Music("SOUND/BGM_FIELD1.AT3".into())));
        game.scene.sounds.clear();

        // A battle with an enemy that goes about leaves the tune alone.
        game.with(|script, field| script.call(Callback::Character(502), field));
        assert_eq!(game.scene.battle, Some(BattleRequest { group: 101 }));
        assert!(game.scene.sounds.iter().all(|sound| !matches!(sound, Request::Music(_) | Request::StopMusic(_))));
        assert_eq!(game.scene.tune, 2);
    }

    #[test]
    fn touching_an_enemy_asks_for_its_battle_and_winning_counts_towards_the_mission() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.with(|script, field| script.call(Callback::Character(502), field));
        assert_eq!(game.scene.battle, Some(BattleRequest { group: 101 }));
        assert!(game.script.busy());
        // The script waits for as long as the battle lasts.
        for _ in 0..10 {
            game.with(|script, field| script.update(field));
        }
        assert!(game.script.busy() && game.scene.battle.is_some());

        game.scene.battle = None;
        game.scene.report.outcome = OUTCOME_WON;
        game.with(|script, field| script.update(field));
        // The symbol is gone for good.
        assert!(game.progress.flag(3003));
        let beaten = &game.scene.characters[&502];
        assert!(matches!(beaten.action, Action::Destroy { .. }) && beaten.status == status::DESTROYED);
        assert!(!beaten.flags.has(flag::SYMBOL) && beaten.behaviour.is_none());
        // What follows every battle is the system script's to do, and the map's
        // script waits for it: the enemies still about are told to stand down.
        assert_eq!(game.scene.system, Some(300));
        assert_eq!(game.progress.variable(variable::MISSION_PROGRESS), MISSION_STARTED);
        game.frame();
        assert_eq!(game.scene.system, None);
        assert!(game.scene.characters[&500].mind.stand_down && !game.scene.characters[&502].mind.stand_down);
        // Then the mission moves on by one.
        game.frame();
        assert_eq!(game.progress.variable(variable::MISSION_PROGRESS), MISSION_STARTED + 1);
        assert_eq!(game.progress.variable(239), 1);
        // The picture, which the battle left dark, is coming back.
        assert!(game.scene.fade.step <= 0.0);
        // The first enemy beaten brings a lesson on after a pause of two seconds,
        // which the script waits out before it starts on the lesson's own.
        for _ in 0..60 {
            assert!(game.script.busy() && !game.progress.flag(7402));
            game.frame();
        }
        game.settle();
        assert!(!game.script.busy());
        assert!(game.progress.flag(7402), "the lesson has been given");
    }

    #[test]
    fn traps_call_the_script_which_may_wait_for_frames_to_pass() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The first event gate switches itself off and sets a flag, with no waiting.
        game.with(|script, field| script.call(Callback::Trap(500), field));
        assert!(!game.script.busy());
        assert!(game.scene.traps[&500].off && game.progress.flag(5005));

        // Walking out of the map by its trap walks the player on for ten frames first.
        game.with(|script, field| script.call(Callback::Trap(0), field));
        assert!(game.script.busy());
        let player = game.scene.player().unwrap();
        assert!(player.acting());
        assert_eq!(player.target, Vec3::new(1425.0, 0.0, 5256.0));
        for _ in 0..9 {
            game.with(|script, field| script.update(field));
            assert!(game.script.busy() && game.scene.map_change.is_none());
        }
        game.with(|script, field| script.update(field));
        assert!(!game.script.busy());
        // And then names the map the trap leads to, and the entrance to come in by.
        assert_eq!(game.scene.map_change, Some((6, 1)));
    }

    impl Loaded {
        /// Has the player touch a character, or walk into a trap, and wins whatever
        /// battles come of it; answers the groups fought.
        fn play(&mut self, callback: Callback) -> Vec<i32> {
            self.with(|script, field| script.call(callback, field));
            let mut fought = Vec::new();
            loop {
                self.settle();
                let Some(request) = self.scene.battle.take() else { return fought };
                fought.push(request.group);
                self.scene.report = BattleReport::WON;
            }
        }

        fn progress(&self) -> i32 {
            self.progress.variable(variable::MISSION_PROGRESS)
        }

        /// What follows the field being left: the system script is told how, and
        /// the screen between missions then has it run `enter_stage_select`,
        /// whose windows are taken as read here. Answers what they said.
        fn leave(&mut self) -> Vec<String> {
            let how = self.scene.leaving.expect("the field has been left");
            let mut world = World {
                progress: &mut self.progress,
                scene: &mut self.scene,
                stats: &mut self.stats,
                tables: &self.tables,
                ground: &self.world,
            };
            assert_eq!(crate::hub::left(how, &mut self.system, &mut world).expect("the system script runs"), None);
            let mut said = Vec::new();
            let mut running = self.system.begin("enter_stage_select", &[], &mut world).expect("the system script runs");
            while running {
                said.extend(world.scene.said.take());
                running = self.system.go_on(&mut world).expect("the system script runs");
            }
            said
        }

        /// A mission is chosen on that screen: answers the map and the entrance
        /// the field starts anew on.
        fn choose(&mut self, mission: i32) -> (i32, i32) {
            let mut world = World {
                progress: &mut self.progress,
                scene: &mut self.scene,
                stats: &mut self.stats,
                tables: &self.tables,
                ground: &self.world,
            };
            self.system.call("mission_select", &[mission as u32], &mut world).expect("the system script runs");
            crate::hub::named(world.scene, value::MAP, value::ENTRANCE)
        }
    }

    #[test]
    fn the_scripts_make_characters_of_effects() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let effects = |game: &Loaded| -> Vec<(i32, String)> {
            game.scene.characters.iter().filter_map(|(&id, character)| Some((id, character.effect.as_ref()?.file.clone()))).collect()
        };
        // What the opening's scenes made of their events' effects has gone with
        // them. The ring of light about the terminal is a character made of one
        // of the map's own effects, where the terminal is; no button does
        // anything with it.
        assert_eq!(effects(&game), [(2049, "FLD/MAP/MAP_D_STG01_01.BIN/04.bin/12.efc".to_owned())]);
        let (ring, terminal) = (&game.scene.characters[&2049], &game.scene.characters[&200]);
        assert_eq!((ring.position, ring.facing()), (terminal.position, terminal.facing()));
        assert_eq!((ring.chr, ring.prompt(0), ring.hidden()), (NO_RESOURCE, None, false));
        assert_eq!(ring.effect.as_ref().map(|effect| (effect.begun, effect.fastened)), Some((1, None)));

        // Looking at the terminal has another made where she stands, which is
        // taken away again sixty frames on, and then she is whole.
        game.stats.hp = 1.0;
        game.with(|script, field| script.call(Callback::Character(200), field));
        let mut frames = 0;
        while !game.scene.characters.contains_key(&2050) {
            game.stood_in_frame();
            frames += 1;
            assert!(frames < 1000, "looking at the terminal made nothing");
        }
        let (made, player) = (&game.scene.characters[&2050], game.scene.player().unwrap());
        assert_eq!(made.effect.as_ref().map(|effect| effect.file.as_str()), Some("FLD/MAP/MAP_D_STG01_01.BIN/04.bin/13.efc"));
        assert_eq!(made.position, player.position);
        assert!((made.facing() - player.facing()).abs() < 0.02, "{} and {}", made.facing(), player.facing());
        assert_eq!(game.stats.hp, 1.0);
        let mut frames = 0;
        while game.scene.characters.contains_key(&2050) {
            game.stood_in_frame();
            frames += 1;
            assert!(frames < 1000, "what looking at the terminal made was never taken away");
        }
        assert_eq!(frames, 60);
        game.settle();
        assert_eq!(game.stats.hp, game.stats.max_hp);
        assert_eq!(effects(&game).len(), 1);
    }

    #[test]
    fn the_first_mission_can_be_played_to_its_end() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The six Eaters of the streets, one battle each.
        for (count, id) in (500..=505).enumerate() {
            let fought = game.play(Callback::Character(id));
            assert_eq!(fought.len(), 1, "symbol {id}");
            assert_eq!(game.progress(), MISSION_STARTED + 1 + count as i32);
            assert!(!game.script.busy());
        }
        // With all of them beaten she is sent to the terminal, and the line that says
        // what is to be done changes.
        assert_eq!(game.progress(), 107);
        assert_eq!(system::panels(&game.progress, &game.tables)[0].line, "Heal at the Terminal");

        // Looking at the terminal brings the Hornets out.
        assert_eq!(game.scene.characters[&200].prompt(0), Some(crate::field::scene::prompt::CHECK));
        assert!(game.play(Callback::Character(200)).is_empty());
        assert_eq!(game.progress(), 112);
        for id in 521..=523 {
            assert!(!game.scene.characters[&id].hidden(), "{id} should have appeared");
            assert_eq!(game.play(Callback::Character(id)).len(), 1);
        }
        // Then the trailer's guard, and the way to it opens.
        assert_eq!(game.progress(), 115);
        assert!(!game.scene.characters[&527].hidden() && !game.scene.traps[&502].off);
        let fought = game.play(Callback::Trap(502));
        assert_eq!(fought, [103]);

        // The mission is cleared: the system script says so, and the field is left.
        assert_eq!(game.progress(), 200);
        assert_eq!(game.scene.leaving, Some(1));
        // She has been at the terminal and is whole.
        assert_eq!(game.stats.hp, game.stats.max_hp);

        // The field left, the system script closes the mission and opens what
        // follows from it, which it tells of: a mission's state is the variable
        // of its number past a hundred.
        let said = game.leave();
        let state = |game: &Loaded, mission: i32| game.progress.variable(100 + mission);
        assert_eq!((state(&game, 0), state(&game, 1), state(&game, 2)), (255, 2, 0));
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("Generator Destruction"), "{said:?}");
        // No mission is under way while they are chosen from.
        assert_eq!(game.progress.variable(variable::SCENARIO), -1);
        // Choosing one begins it: the field starts anew on its map, with the
        // story where the mission takes it up.
        assert_eq!(game.choose(1), (0, 0));
        assert_eq!(game.progress.variable(variable::SCENARIO), 1);
        assert_eq!(game.progress(), 100);
        assert_eq!(state(&game, 1), 10);
        assert_eq!(game.progress.variable(10_003), 1_010_550);
        // What the last mission had set on its maps is forgotten.
        assert!(!game.progress.flag(5000) && !game.progress.flag(3003));
    }

    /// Of the scenes a new game opens with, the first says that its captions go
    /// on of themselves (`s243(0)`: it is cut to its voices); one after its
    /// battle (`EV01_01_0400C`) does not say, and its captions wait for a button
    /// unless the player's options have them go on of themselves (0xD1634).
    #[test]
    fn a_scenes_captions_wait_for_a_button_unless_the_player_has_them_go_on() {
        use crate::field::staging::Sign;
        for auto_skip in [false, true] {
            let Some(mut game) = first_map() else {
                eprintln!("skipped: game data not unpacked");
                return;
            };
            let player = Character::new(0, Vec3::ZERO, 0.0, &game.scene.defaults);
            game.scene.characters.insert(PLAYER, player);
            game.script.auto_skip = auto_skip;
            game.with(|script, field| script.open(field));
            // With no one at the buttons a caption that waits is taken as read,
            // the frame after it is all there.
            let (mut said, mut waited, mut signed, mut fought, mut frames) = (false, false, false, false, 0);
            while game.script.busy() {
                if game.scene.battle.take().is_some() {
                    game.scene.report.outcome = OUTCOME_WON;
                    fought = true;
                }
                game.stood_in_frame();
                if let Some(captions) = &game.scene.captions {
                    said |= captions.any();
                    waited |= captions.any() && captions.by_hand;
                    signed |= captions.sign != Sign::None;
                }
                frames += 1;
                assert!(frames < 200_000, "the opening never came to an end");
            }
            assert!(fought && said);
            assert_eq!((waited, signed), (!auto_skip, !auto_skip), "with the option {auto_skip}");
        }
    }

    impl Loaded {
        /// So many frames with nothing pressed.
        fn frames(&mut self, count: usize) {
            self.pad = Pad::default();
            for _ in 0..count {
                self.scene.fade.advance();
                self.frame();
            }
        }

        /// A frame in which cross goes down.
        fn press(&mut self, buttons: u32) {
            self.pad = Pad { pressed: buttons, held: buttons, ..Pad::default() };
            self.frame();
            self.pad = Pad::default();
        }

        /// The page of the window that is up, as plain text.
        fn page(&self) -> String {
            let window = self.scene.window.as_ref().expect("a window is up");
            text::plain(window.page())
        }
    }

    #[test]
    fn a_lesson_is_shown_with_its_picture_and_read_page_by_page() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.script.skip_windows = false;
        // The first enemy beaten brings the lesson about challenges.
        game.with(|script, field| script.call(Callback::Character(502), field));
        game.scene.battle = None;
        game.scene.report = BattleReport::WON;
        game.frames(70);
        // Its picture is loaded and fades in over half a second (thirty frames of
        // the interface); the script then waits two seconds, and for cross.
        assert_eq!(game.scene.pictures[&0].name, "tut_pit011");
        assert!(game.scene.pictures[&0].alpha() > 0.0 && game.scene.pictures[&0].alpha() < 1.0);
        game.frames(120);
        assert_eq!(game.scene.pictures[&0].alpha(), 1.0);
        assert!(game.script.busy() && game.scene.window.is_none(), "it waits for cross with no window up");
        game.press(button::CROSS);

        // Five windows, one after the other; each is closed with cross, once its
        // text has had the time to come up, and takes its time to go.
        let pages = [
            "In your last battle, the challenge\n\u{201c}First Challenge\u{201d} was unlocked.",
            "Tutorial:\n\u{201c}Unlocking Challenges\u{201d}",
            "BRS, who has just awakened,\nhas forgotten how to fight.",
            "In order for her to remember,\nshe will need to clear challenges.",
            "First, engage in battle 4 times\nto clear this challenge.",
        ];
        for page in pages {
            assert_eq!(game.page(), page);
            game.press(button::CROSS);
            assert_eq!(game.page(), page, "a window just put up does not take a button yet");
            game.frames(10);
            assert!(game.script.busy());
            game.press(button::CROSS);
            assert_eq!(game.page(), page, "it is closing");
            game.frames(10);
        }
        // What the game marks out is in the colour it marks things out in.
        assert!(game.scene.window.is_none());
        // The picture fades out again, and the lesson has been given.
        game.frames(15);
        assert!(game.scene.pictures[&0].alpha() < 1.0);
        game.frames(30);
        assert!(game.scene.pictures.is_empty() && !game.script.busy());
        assert!(game.progress.flag(7402));
    }

    #[test]
    fn the_terminal_makes_her_whole_and_asks_whether_to_save() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        for id in 500..=505 {
            game.play(Callback::Character(id));
        }
        assert_eq!(game.progress(), 107);
        // What was being said when the last of them fell is let run out: the
        // terminal's lesson waits for it.
        game.frames(5);
        assert!(!game.scene.event_running && !game.scene.comm.busy());
        game.script.skip_windows = false;
        game.stats.hp = 10.0;
        game.with(|script, field| script.call(Callback::Character(200), field));
        // The first time, the lesson about it comes first.
        for page in ["Tutorial:\n\u{201c}The Terminal\u{201d}", "You can restore HP and save\nat the Terminal."] {
            assert_eq!(game.page(), page);
            game.frames(11);
            game.press(button::CROSS);
            game.frames(10);
        }
        assert!(game.progress.flag(7404));
        // The terminal works for two seconds before she is whole and it asks.
        game.frames(30);
        assert!(game.scene.window.is_none() && game.stats.hp == 10.0);
        game.frames(40);
        assert_eq!(game.stats.hp, game.stats.max_hp);
        assert_eq!(game.page(), "HP has been restored.\nSave Game Data?");
        let window = game.scene.window.as_ref().unwrap();
        assert_eq!((window.choices.as_slice(), window.selected, window.cancel), (["Yes".to_string(), "No".to_string()].as_slice(), 0, 1));
        // Circle goes to "No" once the choices are up; cross then answers with it,
        // and nothing is saved.
        game.frames(22);
        game.press(button::CIRCLE);
        assert_eq!(game.scene.window.as_ref().unwrap().selected, 1);
        game.press(button::CROSS);
        game.frames(10);
        assert!(game.scene.window.is_none());
        assert_eq!(game.scene.answer, 1);
        assert!(!game.script.unported().contains(&178), "the game is not saved when the answer is no");

        // The Hornets come out, and when the script is done with that Phobos has a
        // word to say about it: the script does not wait for him.
        for _ in 0..400 {
            if !game.script.busy() {
                break;
            }
            game.frames(1);
            for character in game.scene.characters.values_mut() {
                if character.acting() {
                    character.stop();
                }
            }
        }
        assert!(!game.script.busy());
        assert_eq!(game.progress(), 112);
        game.frames(2);
        assert_eq!(game.scene.comm.faces()[0], Some(1));
        // The line waits for his face to have come in.
        assert!(game.scene.comm.line().is_none());
        game.frames(8);
        let line = game.scene.comm.line().expect("a line is being said");
        assert_eq!(text::plain(&line.text), "More enemies inbound! Be quick\nor they'll just keep coming!");
        // Cross cuts the line short, and the window is closed.
        game.press(button::CROSS);
        game.frames(2);
        assert!(!game.scene.comm.open && !game.scene.event_running);
    }

    #[test]
    fn a_map_come_into_by_an_entrance_brings_her_in_there() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // On to Ankh Street by its first entrance, as the field does it: the scene
        // goes on, and the next map's script sets it up.
        let data = GameData::new(DATA).unwrap();
        const ANKH_STREET: i32 = 6;
        let name = game.tables.maps[ANKH_STREET as usize].name.clone();
        assert_eq!(name, "d_stg01_0a");
        // Nothing covers the screen, so a picture of it is taken first, which
        // stays up; once there is one the map is left, and not again asked for.
        assert!(!game.scene.fade.covered() && game.scene.keep_screen());
        assert_eq!(game.scene.capture, Capture::Wanted { whole: false });
        game.scene.capture = Capture::None;
        assert!(game.scene.fade.covered() && !game.scene.keep_screen());
        game.scene = game.scene.onward(ANKH_STREET, 1);
        assert_eq!(game.scene.fade.picture(), Some([255; 4]));
        game.script = load(&data, &name).expect("the map's script parses").expect("the map has a script");
        game.system(|system, world| system.call("sys_fld_enter_map", &[], world).map(|_| ()));
        game.with(|script, field| script.open(field));
        // She is put at the entrance and walks in, the picture coming back as she
        // does; the script waits for her.
        let player = game.scene.player().unwrap();
        assert_eq!(player.position, Vec3::new(9010.0, 0.0, 10890.0));
        assert_eq!(player.target, Vec3::new(9010.0, 0.0, 10390.0));
        assert!(player.acting() && game.script.busy());
        // The picture of the map she left goes over thirty frames, darkening as
        // it would have for sixty had the map kept her waiting.
        assert!(game.scene.fade.running() && game.scene.fade.level == 0.0);
        game.frames(15);
        assert_eq!(game.scene.fade.picture(), Some([191, 191, 191, 127]));
        game.frames(15);
        assert!(!game.scene.fade.running() && game.scene.fade.picture().is_none() && !game.scene.fade.covered());
        game.settle();
        assert!(!game.script.busy());
        // The map has its own ways out, and the mission's enemies for this map.
        assert!(matches!(game.scene.traps[&0].kind, TrapKind::MapChange { map: 0, entrance: 1 }));
        assert!(matches!(game.scene.traps[&1].kind, TrapKind::MapChange { map: 7, entrance: 1 }));
        // The mission's display is still up, and the place has been named.
        assert_eq!(system::panels(&game.progress, &game.tables).len(), 1);
        assert_eq!(game.scene.map, ANKH_STREET);
    }

    /// Not a check: prints where the first map's script puts everything, for
    /// working out a scripted run (`cargo test -p brs-game layout -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn layout_of_the_first_map() {
        let Some(game) = opened() else { return };
        for (id, character) in &game.scene.characters {
            let hidden = if character.hidden() { " hidden" } else { "" };
            eprintln!("character {id}: resource {} at {}{hidden}", character.chr, character.position);
        }
        for (id, trap) in &game.scene.traps {
            let off = if trap.off { " off" } else { "" };
            eprintln!("trap {id}: {:?} at {} reach {} turn {:.0}{off}", trap.kind, trap.centre, trap.reach, trap.turn.to_degrees());
        }
        for (id, wall) in &game.scene.walls {
            let open = if wall.open { " open" } else { "" };
            eprintln!("wall {id}: at {} reach {} turn {:.0}{open}", wall.centre, wall.reach, wall.turn.to_degrees());
        }
    }

    #[test]
    fn an_enemy_that_was_got_away_from_stays_where_it_is() {
        let Some(mut game) = opened() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.with(|script, field| script.call(Callback::Character(500), field));
        assert_eq!(game.scene.battle, Some(BattleRequest { group: 100 }));
        game.scene.battle = None;
        game.scene.report = BattleReport { outcome: OUTCOME_ESCAPED, second: 1 };
        game.settle();
        assert!(!game.script.busy());
        assert!(!game.progress.flag(3001));
        assert!(!game.scene.characters[&500].hidden());
    }
}
