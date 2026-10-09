//! The field's system script (`SYSTEM.VOL/fld_pack.bin`, slot 8) and the engine as it
//! sees it: the functions of the field's general table (0x1726D0) that it calls. See
//! `docs/FIELD_NOTES.md`.
//!
//! The script is the game's own account of what a mission is: what starting,
//! clearing and failing one do, what happens after every battle, the enemies that pop
//! up out of the ground about the player, and the clocks a mission can run. The
//! engine calls its functions by name; a map's script runs its `script_entry` with
//! `s247` and waits for it.
//!
//! The field's events (`s237`) are scripts of the same kind that call the same
//! functions, and are run here too: the messages people send her are these.

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use bevy::prelude::*;
use brs_engine::collision::CollisionWorld;
use brs_formats::script::{self, Call, Machine, Run, Script};

use super::comm::{Line, Voice, PLACES};
use super::event::EventData;
use super::failed;
use super::cleared::Cleared;
use super::place::PlaceName;
use super::scene::{field_flag, flag, int, setting, value, window, Capture, Character, Motion, Notice, Scene, BANK_OF_EVENT, FULL_PACE};
use super::tables::{FieldTables, RESIDENT_BASE};
use crate::battle::tables::PlayerStats;
use crate::data::GameData;
use crate::progress::Progress;
use crate::sound::{Request, FULL_VOLUME};
use crate::text;

const SCRIPT: &str = "SYSTEM.VOL/fld_pack.bin/08.bin/00.sscr";

/// Where the record `update_player` is handed sits in the script's address space.
const FRAME_RECORD: u32 = 0x3000_0000;

/// The engine functions the script calls, by their numbers in the table.
mod native {
    // What an event's script works with: a wait of so many frames, and the window
    // people speak to her through.
    pub const WAIT: u32 = 0x00;
    pub const COMM_SAY: u32 = 0x01;
    pub const COMM_WAIT: u32 = 0x02;
    pub const COMM_OPEN: u32 = 0x03;
    pub const COMM_CLOSE: u32 = 0x04;
    pub const COMM_FACE: u32 = 0x05;
    pub const COMM_NO_FACE: u32 = 0x06;
    pub const COMM_CHOICES: u32 = 0x5c;
    pub const COMM_ANSWER: u32 = 0x5d;
    pub const SET_VALUE: u32 = 0x07;
    pub const VARIABLE: u32 = 0x13;
    pub const FLAG: u32 = 0x14;
    pub const SET_FLAG: u32 = 0x15;
    pub const VALUE: u32 = 0x16;
    pub const FIGURE: u32 = 0x17;
    pub const SET_VARIABLE: u32 = 0x18;
    pub const OPENED_WINDOW: u32 = 0x19;
    pub const OPENED_TEXT: u32 = 0x1a;
    pub const PRINT: u32 = 0x26;
    pub const CHARACTER: u32 = 0x29;
    pub const ACT: u32 = 0x2a;
    pub const MISSIONS: u32 = 0x31;
    pub const MISSION: u32 = 0x32;
    pub const FUNCTION: u32 = 0x33;
    pub const SET_WORK_FLOAT: u32 = 0x34;
    pub const WORK_FLOAT: u32 = 0x35;
    pub const CREATE: u32 = 0x36;
    pub const COUNT: u32 = 0x37;
    pub const FREE_NUMBER: u32 = 0x38;
    pub const LOG: u32 = 0x39;
    pub const POP_PLACE: u32 = 0x3a;
    pub const PLACE: u32 = 0x3b;
    pub const BEHAVIOUR: u32 = 0x3c;
    pub const SET_CHARACTER_FLAG: u32 = 0x3d;
    pub const SET_INT: u32 = 0x3f;
    pub const DECIMAL: u32 = 0x42;
    pub const REMOVE: u32 = 0x43;
    pub const TUNABLES: u32 = 0x44;
    pub const WINDOW_TEXT: u32 = 0x45;
    pub const WINDOW: u32 = 0x46;
    pub const WAIT_WINDOW: u32 = 0x47;
    pub const NTH: u32 = 0x48;
    pub const SET: u32 = 0x49;
    pub const READ: u32 = 0x4a;
    pub const SET_FIGURE: u32 = 0x4c;
    pub const CAMERA_SET: u32 = 0x4f;
    pub const TARGET: u32 = 0x58;
    pub const CAMERA_KIND: u32 = 0x59;
    pub const CAMERA_MOVE: u32 = 0x5a;
    pub const WAIT_ACTION: u32 = 0x5b;
    pub const PUT: u32 = 0x60;
    pub const NECK: u32 = 0x61;
    pub const SPEAK: u32 = 0x62;
    pub const WINDOW_STRING: u32 = 0x64;
    pub const BACK_UP: u32 = 0x66;
    pub const RESTORE: u32 = 0x67;
    pub const FADE: u32 = 0x68;
    pub const WAIT_FADE: u32 = 0x69;
    pub const BETWEEN_MISSIONS: u32 = 0x6a;
    pub const POP_KIND: u32 = 0x6b;
    pub const HEAL: u32 = 0x72;
    /// A sound of one of the banks, which the script is answered with the number
    /// of; the same at a place between left and right; and one stopped.
    pub const SOUND: u32 = 0x08;
    pub const SOUND_AT: u32 = 0x0a;
    pub const SOUND_STOP: u32 = 0x4b;
    pub const COUNTER: u32 = 0x73;
}

// The player's figures a script reads and sets by number (0x9C4, 0xB98).
const FIGURE_EXPERIENCE: i32 = 0;
const FIGURE_LEVEL: i32 = 1;
const FIGURE_HP: i32 = 2;
const FIGURE_MAX_HP: i32 = 3;

// What `ACT` can have a character do: a map's script uses the same numbers.
const ACT_WALK: i32 = 0;
const ACT_TURN: i32 = 2;
const ACT_STOP: i32 = 7;
const ACT_DASH: i32 = 11;
/// The animation a script has named for it starts (0xF4EEC): here it started when
/// it was named.
const ACT_MOTION: i32 = 13;
/// How far above and below where a character is put its ground is looked for
/// (0x108520).
const DROP_REACH: f32 = 40.0;
/// A battle has been fought: a symbol's script is told, with its `int_battle_escape`.
const ACT_STAND_DOWN: i32 = 8;
/// It comes to where it stands: up out of the ground, in from above, or up
/// through a ring (`Character::come_by`).
const ACT_POP_UP: i32 = 9;
const ACT_DROP_IN: i32 = 10;
const ACT_APPEAR: i32 = 14;
/// Its face goes over to the one its settings name (0x1095D4).
const ACT_SHIFT_FACE: i32 = 12;

// What `READ` reads of a character.
const READ_RESOURCE: i32 = 0x17;
const READ_GROUP: i32 = 0x73;
const READ_STATUS: i32 = 0x82;

// A character's decimal parameters that `DECIMAL` reads: how far it is from the
// player, and how far away it notices her (0xFA300).
const DECIMAL_DISTANCE: i32 = 0x29;
const DECIMAL_NOTICE_RANGE: i32 = 0x2a;

// Which script `FUNCTION` looks a function up in (0x2148): 1 is this one. The
// others are scripts that are not loaded here; a function looked for in one of
// them is not found, which is what the script expects of most missions.
const KIND_SYSTEM: i32 = 1;

// The kinds of text `WINDOW_TEXT` can name (0xECD04): 0 is a mission's title.
const TEXT_MISSION_TITLE: i32 = 0;

// Where the field's tunables hold what the pop-ups go by: how far ahead of her the
// middle of where they come up is, and how far from that they can be.
const PRM_OWN_PACE: usize = 0x44;
const PRM_DASH_PACE: usize = 0x46;
const PRM_ALERT_FRAMES: usize = 0x4a;
const PRM_POP_AHEAD: usize = 0x5c;
const PRM_POP_SPREAD: usize = 0x60;

/// The places tried for an enemy to pop up at, a twelfth of a turn apart (0xD5470).
const POP_TRIES: usize = 6;
/// How far above and below a place its ground is looked for, and the room an enemy
/// needs there: a sphere of this radius, its middle this far above the ground
/// (0xD5260).
const POP_GROUND_REACH: f32 = 200.0;
const POP_ROOM: f32 = 40.0;
const POP_ROOM_HEIGHT: f32 = 50.0;

/// Where the field's tunables hold the frames the name of a place stands
/// (0xEDCC8).
const PRM_PLACE_FRAMES: usize = 0x14;
/// The variable that says which mission the notice of a mission's start is for
/// (0x4833C), the mission whose notice has no jingle, and the stream the jingle
/// is played on.
const NOTICE_MISSION: i32 = 10_004;
const MISSION_WITH_NO_JINGLE: i32 = 0x2c;
const NOTICE_STREAM: usize = 0;

/// The field's random numbers (0x10E45C), of which the script's functions take
/// twelve bits at a time.
#[derive(Clone)]
struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        // Xorshift: any even spread will do, the original's own sequence is not kept.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    /// A fraction from 0 up to, but not reaching, 1, in steps of 1/4096.
    fn fraction(&mut self) -> f32 {
        (self.next() & 0xfff) as f32 / 4096.0
    }
}

/// What the script's functions work on.
pub struct World<'a> {
    pub progress: &'a mut Progress,
    pub scene: &'a mut Scene,
    pub stats: &'a mut PlayerStats,
    pub tables: &'a FieldTables,
    pub ground: &'a CollisionWorld,
}

/// What a frame of the field in its normal state tells `update_player` (0x10D2C4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The field's two time steps: the second is the one its timers go by.
    pub own_time: i32,
    pub time: i32,
    /// How far the player has moved since the frame before.
    pub moved: f32,
}

impl Frame {
    /// An ordinary frame in which she has moved so far.
    pub fn of(moved: f32) -> Frame {
        Frame { own_time: FULL_PACE, time: FULL_PACE, moved }
    }
}

/// What the script's functions keep between calls.
struct State {
    script: Arc<Script>,
    /// The character they work on (the field's + 0x1D4C), set by `CHARACTER`.
    current: Option<i32>,
    random: Random,
    /// Functions that were called and are not ported, so that each is told of once.
    unported: BTreeSet<u32>,
    /// The words a text it has shown is to have put in, by number (0x12FBB4).
    words: [String; 8],
}

/// An event that is being played: its script on a machine of its own.
struct Event {
    data: EventData,
    machine: Machine,
    /// The frames its script has waited in the wait it is in.
    waited: i32,
}

/// The system script, loaded and running.
#[derive(Resource)]
pub struct SystemScript {
    state: State,
    /// The machine the functions the engine calls run on, to their end.
    machine: Machine,
    /// A function a screen that is not the field's has started, while it
    /// runs: it may wait.
    called: Option<Machine>,
    /// The entry a map's script has asked for, while it runs: it may wait.
    entry: Option<Machine>,
    entry_waited: i32,
    /// The event a map's script has had played, while it runs.
    event: Option<Event>,
    /// Where events are loaded from.
    data: GameData,
}

impl SystemScript {
    pub fn load(data: &GameData, tables: &FieldTables) -> Result<SystemScript> {
        let script = Arc::new(data.script(SCRIPT)?);
        let machine = machine(&script, tables);
        let state = State { script, current: None, random: Random(0x9e37_79b9), unported: BTreeSet::new(), words: Default::default() };
        Ok(SystemScript { state, machine, called: None, entry: None, entry_waited: 0, event: None, data: data.clone() })
    }

    /// Runs one of the script's functions to its end, as the engine does by name
    /// (0x130CE8, 0x130DA8). A function the script does not have is not an error.
    pub fn call(&mut self, name: &str, arguments: &[u32], world: &mut World) -> Result<u32> {
        let Some(function) = self.state.script.symbol(name) else { return Ok(0) };
        let mut engine = Engine { state: &mut self.state, world, event: None, waited: &mut 0 };
        self.machine.call(function, arguments, &mut engine).with_context(|| format!("in the system script's {name}"))
    }

    /// Starts one of the script's functions for a screen that is not the
    /// field's (the task "sysScrTask", 0x130704): it runs on a machine of its
    /// own and may wait. Answers whether it is still running.
    pub fn begin(&mut self, name: &str, arguments: &[u32], world: &mut World) -> Result<bool> {
        let Some(function) = self.state.script.symbol(name) else { return Ok(false) };
        let script = self.state.script.clone();
        let called = self.called.insert(machine(&script, world.tables));
        let mut engine = Engine { state: &mut self.state, world, event: None, waited: &mut 0 };
        let run = called.start(function, arguments, &mut engine).with_context(|| format!("in the system script's {name}"));
        self.ran(run)
    }

    /// Has what was begun so go on a frame (0x130BE0). Answers whether it is
    /// still running.
    pub fn go_on(&mut self, world: &mut World) -> Result<bool> {
        let Some(called) = &mut self.called else { return Ok(false) };
        let mut engine = Engine { state: &mut self.state, world, event: None, waited: &mut 0 };
        let run = called.resume(&mut engine).context("in a function of the system script");
        self.ran(run)
    }

    fn ran(&mut self, run: Result<Run>) -> Result<bool> {
        if !matches!(run, Ok(Run::Waiting)) {
            self.called = None;
        }
        run.map(|run| run == Run::Waiting)
    }

    /// Drops whatever was running: the field is started anew.
    pub fn abandon(&mut self) {
        self.called = None;
        self.entry = None;
        self.event = None;
    }

    /// `update_player`: once every frame the field is in its normal state (0x10DBA4).
    pub fn frame(&mut self, frame: Frame, world: &mut World) -> Result<()> {
        let mut record = Vec::with_capacity(12);
        record.extend(frame.own_time.to_le_bytes());
        record.extend(frame.time.to_le_bytes());
        record.extend(frame.moved.to_le_bytes());
        self.machine.map(FRAME_RECORD, record);
        self.call("update_player", &[FRAME_RECORD], world).map(|_| ())
    }

    /// A frame of what a map's script has asked of the system script: the entry it
    /// named is started, or goes on from where it waited, and when it has run to its
    /// end the map's script is let go on. The mission's display is redone when a
    /// script has changed what it shows.
    pub fn update(&mut self, world: &mut World) -> Result<()> {
        let scene = &mut *world.scene;
        // The sign that names the place has its frame (0xEEDF8), with what the
        // field shows of the mission.
        if scene.place_name.as_mut().is_some_and(|name| !name.step(&world.tables.places)) {
            scene.place_name = None;
        }
        // And so has the sign of what a button would do (0xEE144), before it.
        scene.action_sign.step();
        // The marks over the characters have theirs after all of these (0x109F4C).
        for character in scene.characters.values_mut() {
            character.step_mark();
        }
        // The window of a mission cleared has its frame (0x4A550), and is done
        // with when it has gone.
        if let Some(cleared) = &mut scene.cleared {
            let there = cleared.step();
            scene.sounds.append(&mut cleared.sounds);
            if !there {
                scene.cleared = None;
            }
        }
        if world.scene.notice.as_mut().is_some_and(|notice| !notice.step()) {
            world.scene.notice = None;
        }
        if std::mem::take(&mut world.scene.display_stale) {
            self.call("mission_update_info", &[], world)?;
        }
        let entry = self.update_entry(world);
        let events = self.update_events(world);
        entry.and(events)
    }

    /// A frame of the events that are asked for, for what plays them where the
    /// field is not: the ride on the bike has its talk so (0x149CBC, 0x130778).
    pub fn play_events(&mut self, world: &mut World) -> Result<()> {
        self.update_events(world)
    }

    /// A frame of the events a map's script has asked for (`s237`), which are played
    /// one at a time in the order they were asked for (0xD7C78): an event is loaded,
    /// its script is started, and it goes on a frame at a time until it returns.
    fn update_events(&mut self, world: &mut World) -> Result<()> {
        if std::mem::take(&mut world.scene.stop_events) {
            self.event = None;
        }
        let mut result = Ok(());
        loop {
            let run = match &mut self.event {
                Some(Event { data, machine, waited }) => {
                    let mut engine = Engine { state: &mut self.state, world, event: Some(data), waited };
                    machine.resume(&mut engine)
                }
                None => {
                    let Some((number, argument)) = world.scene.events.pop_front() else { break };
                    let data = match EventData::load(&self.data, world.tables, number) {
                        Ok(Some(data)) => data,
                        Ok(None) => {
                            debug!("event {number} has nothing to run");
                            continue;
                        }
                        Err(error) => {
                            result = Err(error.context(format!("loading event {number}")));
                            continue;
                        }
                    };
                    debug!("event {number} is played: {}", data.bundle);
                    let entry = data.entry;
                    let machine = machine(&data.script, world.tables);
                    let Event { data, machine, waited } = self.event.insert(Event { data, machine, waited: 0 });
                    let mut engine = Engine { state: &mut self.state, world, event: Some(data), waited };
                    // What `s237` was given second is what the script is started with.
                    machine.start(entry, &[argument as u32, 0], &mut engine)
                }
            };
            match run {
                Ok(Run::Waiting) => break,
                Ok(Run::Returned(_)) => self.event = None,
                Err(error) => {
                    self.event = None;
                    result = Err(error.context("in an event's script"));
                }
            }
        }
        world.scene.event_running = self.event.is_some();
        result
    }

    /// A frame of the entry a map's script has asked for and is waiting on; and
    /// a function it has asked for by name is run to its end first.
    fn update_entry(&mut self, world: &mut World) -> Result<()> {
        if let Some(name) = world.scene.system_call.take() {
            self.call(name, &[], world)?;
        }
        let Some(number) = world.scene.system else { return Ok(()) };
        let mut engine = Engine { state: &mut self.state, world, event: None, waited: &mut self.entry_waited };
        let run = match &mut self.entry {
            Some(machine) => machine.resume(&mut engine),
            None => {
                let script = engine.state.script.clone();
                let entry = script.symbol("script_entry").context("the system script has no script_entry")?;
                let machine = self.entry.insert(machine(&script, engine.world.tables));
                machine.start(entry, &[number as u32], &mut engine)
            }
        };
        match run.with_context(|| format!("in entry {number} of the system script")) {
            Ok(Run::Waiting) => Ok(()),
            Ok(Run::Returned(_)) => {
                self.entry = None;
                world.scene.system = None;
                Ok(())
            }
            Err(error) => {
                // The map's script must not be left waiting for an entry that failed.
                self.entry = None;
                world.scene.system = None;
                Err(error)
            }
        }
    }
}

/// A machine for the script with the field's tables in its address space.
fn machine(script: &Script, tables: &FieldTables) -> Machine {
    let mut machine = Machine::new(script);
    machine.map(RESIDENT_BASE, tables.resident.image().to_vec());
    machine
}

/// Reads one of the field's figures (0xD6AF4, 0x1F04, 0x117E14).
pub fn read_value(scene: &Scene, progress: &Progress, tables: &FieldTables, number: i32) -> i32 {
    let kept = |number: i32| scene.values.get(&number).copied().unwrap_or(0);
    let tunable = |at: usize| tables.tunables.u16(at).unwrap_or(0);
    match number {
        value::MAP => scene.map,
        value::ENTRANCE => scene.entrance,
        value::POP_TIMER => scene.work.pop_timer,
        value::POPS => scene.flag(field_flag::POPS) as i32,
        value::LEAVE => scene.flag(field_flag::LEAVE) as i32,
        value::MISSION_SECONDS => progress.variable(variable::MISSION_TIME) / 1000,
        value::POP_SECONDS => progress.variable(variable::POP_INTERVAL) / 0xf000,
        // Three of the tunables, read as the original reads them: two signed.
        45 => tunable(PRM_OWN_PACE) as i16 as i32,
        46 => tunable(PRM_ALERT_FRAMES) as i32,
        48 => tunable(PRM_DASH_PACE) as i16 as i32,
        // The enemies about what she guards (0x10D370): of the map's own (500 to
        // 599) and of those that popped up (3000 to 3099), the ones that are not
        // hidden and no further than 300 from the character a script has named
        // (value 52); none where there is no such character.
        value::ENEMIES_NEAR => {
            let guarded = guarded(scene);
            scene.characters.get(&guarded).map_or(0, |kept| {
                let about = |id: i32, character: &Character| {
                    id != guarded && GUARD_FOES.iter().any(|numbers| numbers.contains(&id)) && !character.hidden() && character.position.distance(kept.position) <= GUARD_REACH
                };
                scene.characters.iter().filter(|(&id, character)| about(id, character)).count() as i32
            })
        }
        value::GUARDED => guarded(scene),
        value::NEXT_MAP | value::NEXT_ENTRANCE | value::OBJECTIVE => kept(number) as i16 as i32,
        value::FAILED_CHOICE => kept(number) as u8 as i32,
        value::ASYNC_TIME | 39 | value::GUARD_TIMER | value::EXIT => kept(number),
        _ => 0,
    }
}

/// How near what she guards an enemy counts against it (0x10D3CC), and the
/// numbers of the characters that are enemies for that: the map's own and those
/// that pop up (0x10D418 to 0x10D43C).
const GUARD_REACH: f32 = 300.0;
const GUARD_FOES: [std::ops::Range<i32>; 2] = [500..600, 3000..3100];

/// The number of the character she guards: none until a script names one
/// (0x1181F0).
fn guarded(scene: &Scene) -> i32 {
    scene.values.get(&value::GUARDED).copied().unwrap_or(-1)
}

/// Sets one of the field's figures (0xD4108, 0x1E78, 0x117CD8). Returns whether the
/// mission's display has to be redone for it.
pub fn write_value(scene: &mut Scene, progress: &mut Progress, number: i32, to: i32) -> bool {
    match number {
        // The map and entrance to go to next: kept, for when maps can be changed.
        value::MAP | value::ENTRANCE => {
            scene.values.insert(number, to);
        }
        value::POP_TIMER => scene.work.pop_timer = to,
        value::POPS => scene.set_flag(field_flag::POPS, to != 0),
        value::LEAVE => scene.set_flag(field_flag::LEAVE, to != 0),
        // Two more of the field's flags, which nothing here reads yet.
        54 => scene.set_flag(0x1a, to != 0),
        59 => scene.set_flag(0x59, to != 0),
        value::MISSION_SECONDS => progress.set_variable(variable::MISSION_TIME, to.wrapping_mul(1000)),
        value::POP_SECONDS => progress.set_variable(variable::POP_INTERVAL, to.wrapping_mul(0xf000)),
        value::NEXT_MAP | value::NEXT_ENTRANCE | value::FAILED_CHOICE | 60 | value::ASYNC_TIME | 39 | value::GUARDED | value::GUARD_TIMER | value::EXIT => {
            scene.values.insert(number, to);
        }
        value::OBJECTIVE => {
            scene.values.insert(number, to);
            return true;
        }
        _ => {}
    }
    false
}

/// The mission's display: two panels, each a line of the mission's that says what is
/// to be done and, by the panel's kind, a number or two. All of it is kept in the
/// game's progress (0xEF084 draws a panel from there).
pub mod display {
    /// The flag that has each panel shown.
    pub const SHOWN: [i32; 2] = [6165, 6166];
    /// The variable that picks each panel's line among the mission's; 255 is none.
    pub const LINE: [i32; 2] = [233, 234];
    pub const NO_LINE: i32 = 0xff;
    /// The variable that says how each panel's numbers are shown, and those that
    /// hold the two numbers.
    pub const KIND: [i32; 2] = [235, 236];
    pub const NUMBERS: [[i32; 2]; 2] = [[10_007, 10_008], [10_009, 10_010]];
}

/// How a panel's numbers are shown (0xEEF90 turns the script's number for it into
/// one of these, and 0xEF198 draws by it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Numbers {
    None,
    /// A clock (the script's 3): the first number, in thousandths of a second, as
    /// minutes, seconds and hundredths.
    One,
    /// A gauge: the first as a share of the second (the script's 2).
    Gauge,
    /// Both (the script's 1): the first in big figures, a stroke, the second in
    /// small ones.
    Both,
    /// Both as two figures each at places of their own, with no line (its 4).
    Other,
}

/// A panel of the mission's display that is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    /// Which of the two it is: the second stands over the first.
    pub slot: usize,
    pub line: String,
    pub numbers: Numbers,
    pub values: [i32; 2],
}

/// The panels of the mission's display that are shown (0xEF084).
pub fn panels(progress: &Progress, tables: &FieldTables) -> Vec<Panel> {
    let mission = progress.variable(crate::progress::variable::SCENARIO).max(0);
    let mut shown = Vec::new();
    for panel in 0..2 {
        if !progress.flag(display::SHOWN[panel]) {
            continue;
        }
        let line = progress.variable(display::LINE[panel]);
        let line = if line == display::NO_LINE { None } else { tables.mission_line(mission, line) };
        let numbers = match progress.variable(display::KIND[panel]) {
            1 => Numbers::Both,
            2 => Numbers::Gauge,
            3 => Numbers::One,
            4 => Numbers::Other,
            _ => Numbers::None,
        };
        let values = display::NUMBERS[panel].map(|number| progress.variable(number));
        shown.push(Panel { slot: panel, line: line.unwrap_or_default(), numbers, values });
    }
    shown
}

/// Opens or closes one of the field's windows (0xEDB24): what the system script's
/// function 0x46 and a map script's `s104` do for windows from 5 on. The panels of
/// the mission's display are shown and hidden; the notice of a mission's start
/// and the windows of its clearing and its failing are the original's, and so is
/// the sign that names the place.
pub fn open_window(scene: &mut Scene, progress: &mut Progress, tables: &FieldTables, number: i32, how: i32) {
    const OPEN: i32 = 4;
    match number {
        _ if window::PANELS.contains(&number) => {
            let panel = (number == window::PANELS[2]) as usize;
            // 0 and 2 show it, 1 and 3 hide it.
            if (0..=3).contains(&how) {
                progress.set_flag(display::SHOWN[panel], how % 2 == 0);
            }
        }
        // The notice with the mission's title (0x48300): it plays its jingle on the
        // stream things are said on, but for one mission, which has none.
        window::MISSION_START if how == OPEN => {
            let silent = progress.variable(NOTICE_MISSION) == MISSION_WITH_NO_JINGLE;
            if !silent {
                scene.sounds.push(Request::Say(NOTICE_STREAM, Notice::JINGLE.into()));
            }
            scene.notice = Some(Notice::new(scene.banner_text.clone(), silent));
        }
        // The window of a mission cleared (0xEDC0C), for the mission the save
        // names, with its title.
        window::MISSION_CLEAR if how == OPEN => {
            let mission = progress.variable(crate::progress::variable::SCENARIO);
            if mission >= 0 {
                scene.cleared = Some(Cleared::new(tables.mission_title(mission).unwrap_or_default()));
            }
        }
        window::MISSION_FAIL if how == OPEN => scene.open_failed(progress.flag(failed::NO_STAGE_SELECT)),
        // The sign that names the place (0xEDC8C): the one the list of maps has
        // for this map, for as long as the field's tunables say.
        window::PLACE_NAME if how == OPEN => {
            let place = usize::try_from(scene.map).ok().and_then(|map| tables.maps.get(map)).map_or(-1, |map| i32::from(map.sign));
            let frames = tables.tunables.u16(PRM_PLACE_FRAMES).map_or(0, i32::from);
            PlaceName::name(&mut scene.place_name, &tables.places, place, frames);
        }
        // 6, 7, 8, 14 and 15 do nothing in the original either.
        _ => {}
    }
}

/// Sets one of the things a panel of the mission's display shows (0xE6650): what
/// the system script's function 0x73 and a map script's `s105` do. For windows 10
/// and 11, the panels: 1 the kind of its numbers, 2 its line, 3 and 4 its numbers.
pub fn set_display(scene: &mut Scene, progress: &mut Progress, number: i32, what: i32, to: i32) {
    match (number, what) {
        (10 | 11, 1..=4) => {
            let panel = (number - 10) as usize;
            match what {
                1 => progress.set_variable(display::KIND[panel], to),
                2 => progress.set_variable(display::LINE[panel], if to < 0 { display::NO_LINE } else { to }),
                _ => progress.set_variable(display::NUMBERS[panel][(what - 3) as usize], to),
            }
        }
        // Two more of the field's flags, which nothing here reads yet.
        (15, 0) => scene.set_flag(0x5a, to != 0),
        (16, 0) => scene.set_flag(0x5b, to != 0),
        _ => {}
    }
}

/// The variables of the game's progress that the system script keeps a mission's
/// clocks in.
pub mod variable {
    // 10011 is what a mission's guarded thing has left, counted down by the enemies
    // about it; only the script itself touches it.
    /// The mission's time left, in thousandths of a second; none while it is zero.
    pub const MISSION_TIME: i32 = 10_012;
    /// The time between enemies popping up, in time units.
    pub const POP_INTERVAL: i32 = 10_014;
    /// Which of the game's lists of pop-ups the map takes its enemies from, and
    /// how many of them there can be at once.
    pub const POP_GROUP: i32 = 231;
    #[cfg(test)]
    pub const POP_LIMIT: i32 = 232;
}

/// The engine's side of a run of the script.
struct Engine<'a, 'w> {
    state: &'a mut State,
    world: &'a mut World<'w>,
    /// The event whose script is running, if it is an event's.
    event: Option<&'a EventData>,
    /// The frames the script has waited in the wait it is in (the original keeps
    /// the count on the script's own stack, 0x113628).
    waited: &'a mut i32,
}

impl Engine<'_, '_> {
    fn current(&mut self) -> Option<&mut Character> {
        self.world.scene.characters.get_mut(&self.state.current?)
    }

    /// Runs another of the script's functions from inside one of the engine's, as
    /// the original does when a figure that the mission's display shows is set.
    fn nested(&mut self, name: &str) -> Result<()> {
        let script = self.state.script.clone();
        let Some(function) = script.symbol(name) else { return Ok(()) };
        machine(&script, self.world.tables).call(function, &[], self).with_context(|| format!("in {name}"))?;
        Ok(())
    }

    fn figure(&self, number: i32) -> i32 {
        let stats = &*self.world.stats;
        match number {
            FIGURE_EXPERIENCE => stats.experience as i32,
            FIGURE_LEVEL => stats.level,
            FIGURE_HP => stats.hp as i32,
            FIGURE_MAX_HP => stats.max_hp as i32,
            _ => 0,
        }
    }

    fn set_figure(&mut self, number: i32, to: i32) {
        let stats = &mut *self.world.stats;
        match number {
            FIGURE_HP => stats.hp = (to as f32).clamp(0.0, stats.max_hp),
            _ => debug!("the system script set the player's figure {number}, which is not ported"),
        }
    }

    /// `CREATE`: makes a character of a resource under a number, unless there is one
    /// of that number already, and makes it the one worked on (0xD69B0).
    fn create(&mut self, id: i32, resource: i32) {
        let scene = &mut *self.world.scene;
        if scene.characters.contains_key(&id) {
            return;
        }
        let defaults = scene.defaults;
        let mut character = Character::new(resource, Vec3::ZERO, 0.0, &defaults);
        character.stop();
        scene.characters.insert(id, character);
        self.state.current = Some(id);
    }

    /// `FREE_NUMBER`: the first number from `first` on that no character that is
    /// not hidden has (0xD51A8).
    fn free_number(&self, first: i32) -> i32 {
        let characters = &self.world.scene.characters;
        let taken = |id: &i32| characters.get(id).is_some_and(|character| !character.hidden());
        (first..).find(|id| !taken(id)).unwrap_or(first)
    }

    /// `POP_PLACE`: looks for somewhere ahead of the player for an enemy to come up
    /// out of the ground, and keeps it if it finds one (0xD5470).
    fn pop_place(&mut self) -> bool {
        let Some(player) = self.world.scene.player() else { return false };
        let tunables = &self.world.tables.tunables;
        let (Ok(ahead), Ok(spread)) = (tunables.f32(PRM_POP_AHEAD), tunables.f32(PRM_POP_SPREAD)) else { return false };
        // Ahead of her as her frame lies (0xD54DC has her frame's turn, 0x109228).
        let facing = player.lay * Quat::from_rotation_y(player.facing());
        let reach = spread * self.state.random.fraction();
        let middle = player.position + facing * Vec3::Z * ahead;
        let first = std::f32::consts::TAU * self.state.random.fraction();
        for attempt in 0..POP_TRIES {
            let turn = first + attempt as f32 * std::f32::consts::PI / POP_TRIES as f32;
            let place = middle + facing * Vec3::new(reach * turn.sin(), 0.0, reach * turn.cos());
            if let Some(found) = self.room_at(place) {
                self.world.scene.pop_place = found;
                return true;
            }
        }
        false
    }

    /// Whether an enemy can come up at `place` (0xD5260): there is ground there, and
    /// room above it, both by the upright of the frame she is in (0xD52A8).
    /// Answers the place on the ground.
    fn room_at(&self, place: Vec3) -> Option<Vec3> {
        let group = self.world.scene.player().map_or(0, Character::group);
        let lie = self.world.scene.frames.lie(self.world.ground, group);
        let on_ground = lie.under(place, POP_GROUND_REACH, POP_GROUND_REACH)?;
        (lie.world.wall_distance(lie.in_frame(on_ground) + Vec3::Y * POP_ROOM_HEIGHT) >= POP_ROOM).then_some(on_ground)
    }

    /// `POP_KIND`: which kind of enemy pops up next, drawn by the shares the map's
    /// list gives each (0xD68C8).
    fn pop_kind(&mut self) -> i32 {
        let kinds = self.world.tables.pops(self.world.progress.variable(variable::POP_GROUP));
        let draw = (self.state.random.next() % 100) as i32;
        let mut share = 0;
        for &(resource, weight) in &kinds {
            share += weight;
            if draw < share {
                return resource;
            }
        }
        // Shares that come to less than a hundred leave the draw past the last
        // kind: the original then reads the entry after the list's end, which is
        // its end mark.
        0xff
    }

    /// `ACT`: has the character worked on do something (0xD4D74).
    fn act(&mut self, action: i32) {
        let Some(id) = self.state.current else { return };
        // The ways of coming to where it stands are a map's script's own
        // (0xF6D20, 0xF7458, 0xF7090): the field has it come, which knows how
        // far down its resource has it come up from.
        if matches!(action, ACT_POP_UP | ACT_DROP_IN | ACT_APPEAR) {
            if self.world.scene.characters.contains_key(&id) {
                self.world.scene.coming.push((id, action));
            }
            return;
        }
        let Some(character) = self.world.scene.characters.get_mut(&id) else { return };
        match action {
            ACT_WALK => character.walk(),
            ACT_TURN => character.turn(),
            ACT_DASH => character.dash(),
            ACT_MOTION => {}
            ACT_STOP => character.stop(),
            // Its behaviour script is told, the next time it thinks (0x109054).
            ACT_STAND_DOWN => {
                if character.behaviour.is_some() {
                    character.mind.stand_down = true;
                }
            }
            ACT_SHIFT_FACE => character.shift_face(),
            _ => debug!("the system script asked for action {action}, which is not ported"),
        }
    }

    /// Whether a window a script waits for is still busy (0xD5CA0). The window of
    /// a mission failed is for as long as there is a field (0xEE838): the field is
    /// left by it, not by the script that waits.
    fn window_busy(&self, number: i32) -> bool {
        let scene = &self.world.scene;
        number == window::MISSION_START && scene.notice.is_some()
            || number == window::MISSION_FAIL && scene.failed.is_some()
            || number == window::MISSION_CLEAR && scene.cleared.is_some()
    }

    /// Plays a sound of a bank (0xD4224): of the one that is always there, the
    /// map's, or the bank of the event whose script this is (or, for the system
    /// script, of the event a scene has loaded).
    fn sound(&mut self, kind: i32, sound: i32, volume: i32) -> i32 {
        let scene = &mut *self.world.scene;
        let bank = match (kind, self.event.and_then(|event| event.bank.clone())) {
            (BANK_OF_EVENT, Some(bank)) => Some(bank),
            _ => scene.bank(kind),
        };
        match (bank, u32::try_from(sound)) {
            (Some(bank), Ok(sound)) => scene.effect(bank, sound, volume.clamp(0, FULL_VOLUME)) as i32,
            _ => -1,
        }
    }

    /// `COMM_SAY`: has a line said (0x12FDC0, 0x12E600): by the face in one of the
    /// two places, one of the event's texts, with one of its recordings if the
    /// number of one is given, and for so many frames if a time is.
    fn say(&mut self, speaker: i32, text: i32, voice: i32, time: i32) {
        let Some(event) = self.event else { return };
        let Some(words) = usize::try_from(text).ok().and_then(|index| event.texts.get(index)) else {
            debug!("an event's script asked for text {text}, which {} does not have", event.bundle);
            return;
        };
        let recording = event.voices.get(&voice);
        let voice = match recording {
            _ if voice < 0 => Voice::None,
            Some(recording) => Voice::Frames(recording.frames),
            None => Voice::Missing,
        };
        // The window shows what fits its text and no more: the first page.
        let page = text::pages(words, &text::Plain).swap_remove(0);
        let line = Line::new(speaker.clamp(0, PLACES as i32 - 1) as usize, page, voice, time);
        let line = match recording.filter(|_| voice != Voice::None) {
            Some(recording) => line.spoken(&recording.file),
            None => line,
        };
        self.world.scene.comm.say(line);
    }
}

impl script::Host for Engine<'_, '_> {
    fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
        let int = |index: u32| call.int(index);
        Ok(match number {
            // A wait of so many frames (0xD4030): the script goes on when it has
            // been called back that many times.
            native::WAIT => {
                let frames = int(0)?;
                *self.waited = if call.resumed() { *self.waited + 1 } else { 0 };
                if *self.waited < frames {
                    call.wait();
                }
                0
            }
            // The window people speak to her through (0x12FDC0 to 0x12FFCC): opened
            // and closed, a face put in one of its two places or taken out, a line
            // said, and a wait until everything said has been heard.
            native::COMM_OPEN => {
                let bundle = self.event.map(|event| event.bundle.clone()).unwrap_or_default();
                self.world.scene.comm.open_for(bundle);
                0
            }
            native::COMM_CLOSE => {
                self.world.scene.comm.close();
                0
            }
            // The second of the three it is given is not looked at (0x12ED3C).
            native::COMM_FACE => {
                let (place, face) = (int(0)?, int(2)?);
                if let Ok(place) = usize::try_from(place) {
                    self.world.scene.comm.put_face(place, face);
                }
                0
            }
            native::COMM_NO_FACE => {
                if let Ok(place) = usize::try_from(int(0)?) {
                    self.world.scene.comm.take_face(place);
                }
                0
            }
            // The line's parts are kept as bytes, the recording's number signed,
            // and its time as sixteen bits (0x12FE34).
            native::COMM_SAY => {
                self.say(int(0)? as u8 as i32, int(1)? as u8 as i32, int(2)? as i8 as i32, int(3)? as i16 as i32);
                0
            }
            native::COMM_WAIT => {
                if self.world.scene.comm.busy() {
                    call.wait();
                }
                0
            }
            // The next line said asks, with two of the event's texts for answers
            // (0x13000C); and a wait for the answer, which is what it returns
            // (0x130088): 0 for the first, given with L, 1 for the second, with R.
            native::COMM_CHOICES => {
                let texts = self.event.map(|event| &event.texts);
                let text = |index: i32| usize::try_from(index).ok().and_then(|index| texts?.get(index)).cloned().unwrap_or_default();
                self.world.scene.comm.choices = Some([text(int(0)?), text(int(1)?)]);
                0
            }
            native::COMM_ANSWER => match self.world.scene.comm.answer {
                Some(answer) => answer as u32,
                None => {
                    call.wait();
                    0
                }
            },

            native::VALUE => read_value(self.world.scene, self.world.progress, self.world.tables, int(0)?) as u32,
            native::SET_VALUE => {
                if write_value(self.world.scene, self.world.progress, int(0)?, int(1)?) {
                    self.nested("mission_update_info")?;
                }
                0
            }
            native::VARIABLE => self.world.progress.variable(int(0)?) as u32,
            native::SET_VARIABLE => {
                self.world.progress.set_variable(int(0)?, int(1)?);
                0
            }
            native::FLAG => self.world.progress.flag(int(0)?) as u32,
            native::SET_FLAG => {
                self.world.progress.set_flag(int(0)?, int(1)? != 0);
                0
            }
            native::FIGURE => self.figure(int(0)?) as u32,
            native::SET_FIGURE => {
                self.set_figure(int(0)?, int(1)?);
                0
            }
            // What the script has to say for itself.
            native::PRINT => {
                debug!("system script: {}", call.string(0)?.trim_end());
                0
            }
            native::LOG => {
                debug!("system script: {}", call.string(1)?.trim_end());
                0
            }

            // The game's tables, by address.
            native::TUNABLES => self.world.tables.address("prm"),
            native::MISSIONS => self.world.tables.address("lst_mission"),
            native::MISSION => self.world.tables.mission_address(int(0)?),
            // A function of a script, by name (0xEC934).
            native::FUNCTION => match int(1)? {
                KIND_SYSTEM => self.state.script.symbol(&call.string(0)?).unwrap_or(0),
                _ => 0,
            },

            // What the game keeps for the field outside any map (0x202C, 0x2068).
            native::SET_WORK_FLOAT => {
                if int(0)? == 0 {
                    self.world.scene.work.pop_distance = call.float(1)?;
                }
                0
            }
            native::WORK_FLOAT => match int(0)? {
                0 => self.world.scene.work.pop_distance.to_bits(),
                _ => 0f32.to_bits(),
            },

            // Characters. One is picked to work on; the rest act on that one.
            native::CHARACTER => {
                let id = int(0)?;
                self.state.current = self.world.scene.characters.contains_key(&id).then_some(id);
                self.state.current.is_some() as u32
            }
            native::COUNT => self.world.scene.shown(int(0)?, int(1)?).count() as u32,
            native::NTH => {
                let found = usize::try_from(int(0)?).ok().and_then(|index| self.world.scene.shown(int(1).ok()?, int(2).ok()?).nth(index));
                found.unwrap_or(-1) as u32
            }
            native::FREE_NUMBER => self.free_number(int(0)?) as u32,
            native::CREATE => {
                self.create(int(0)?, int(1)?);
                0
            }
            native::REMOVE => {
                if let Some(id) = self.state.current.take() {
                    self.world.scene.characters.remove(&id);
                }
                0
            }
            native::PLACE => {
                let place = self.world.scene.pop_place;
                if let Some(character) = self.current() {
                    character.position = place;
                    character.target = place;
                    character.mind.home = place;
                }
                0
            }
            native::BEHAVIOUR => {
                let behaviour = int(0)?;
                if let Some(character) = self.current() {
                    character.set(setting::BEHAVIOUR, behaviour);
                }
                0
            }
            native::SET_CHARACTER_FLAG => {
                let (flag, on) = (int(0)? as u32, int(1)? != 0);
                if let Some(character) = self.current() {
                    character.flags.set(flag, on);
                }
                0
            }
            native::SET_INT => {
                let (number, to) = (int(0)?, int(1)?);
                if let Some(character) = self.current() {
                    if !character.set_int(number, to) {
                        debug!("the system script set int parameter {number}, which is not ported");
                    }
                }
                0
            }
            // A setting of the character's (0xD6ED4). An animation of a script's
            // choosing is named by three of them, as a map's script names one with
            // `s62`, `s63` and `s265`: one of its own (0x1071A0), the one an event
            // keeps for its kind (0xE94EC), or one of the slot of the event's cast
            // that setting 127 names (0xE9578).
            native::SET => {
                let (what, to) = (int(0)?, int(1)?);
                let scene = &mut *self.world.scene;
                let staging = scene.staging.clone();
                if let Some(character) = self.state.current.and_then(|id| scene.characters.get_mut(&id)) {
                    let staged = |slot: Option<usize>| {
                        let slot = slot?;
                        staging.as_ref()?.motion(slot, to).map(|_| Motion::Staged(slot, to))
                    };
                    match what {
                        setting::MOTION => character.name_motion(Some(Motion::Own(to))),
                        setting::MOTION_OF_KIND => {
                            let slot = staging.as_ref().and_then(|staging| staging.slot_of(character.chr));
                            character.name_motion(staged(slot));
                        }
                        setting::MOTION_OF_SLOT => {
                            let slot = character.other.get(&setting::MOTION_SLOT).and_then(|&slot| usize::try_from(slot).ok());
                            character.name_motion(staged(slot));
                        }
                        _ => character.set(what, to),
                    }
                }
                0
            }
            // Where the character is to go and how it is to face there, in whole
            // degrees (0xD6374), as a map's script says it with `s133`.
            native::TARGET => {
                let (place, facing) = (Vec3::new(call.float(0)?, call.float(1)?, call.float(2)?), call.float(3)?);
                if let Some(character) = self.current() {
                    (character.target, character.target_facing) = (place, facing.to_radians());
                }
                0
            }
            // It is put somewhere, on the ground if there is ground near enough,
            // facing a way, and stopped (0xD6680), as with `s48`.
            native::PUT => {
                let (mut place, facing) = (Vec3::new(call.float(0)?, call.float(1)?, call.float(2)?), call.float(3)?);
                let ground = self.world.ground;
                let scene = &mut *self.world.scene;
                if let Some(character) = self.state.current.and_then(|id| scene.characters.get_mut(&id)) {
                    // In another of the field's frames than it was made in, it
                    // lies as that one does, and its ground is found so.
                    character.lie_in(&scene.frames);
                    if !character.flags.has(flag::NO_DROP) {
                        if let Some(on) = scene.frames.lie(ground, character.group()).under(place, DROP_REACH, DROP_REACH) {
                            place = on;
                        }
                    }
                    character.place(place, facing.to_radians());
                }
                0
            }
            // Waits for what the character is doing to end (0xD65DC, by 0x10819C),
            // as a map's script does with `s99`.
            native::WAIT_ACTION => {
                if self.current().is_some_and(|character| character.acting()) {
                    call.wait();
                }
                0
            }
            // Its head goes over, in so many frames and eased so, to how its neck
            // is next to turn it (0xD6784), as with `s209`; one that is an effect
            // has no neck.
            native::NECK => {
                let (frames, ease) = (int(0)?, int(1)?);
                if let Some(character) = self.current().filter(|character| character.effect.is_none()) {
                    character.neck.start(frames, ease);
                }
                0
            }
            // Its lips move while it speaks (0xD6868), as with `s160`: so much, and
            // by the two bits whether they stop of themselves when what is spoken
            // ends and whether it then goes back to standing.
            native::SPEAK => {
                let (amount, how) = (int(0)?, int(1)?);
                if let Some(character) = self.current() {
                    character.speak(amount, how);
                }
                0
            }
            // The screen's fade (0xED1DC), which is a map's `s7(way, colour,
            // frames)`: the screen comes back or is taken away, by the black sheet,
            // the white one or the picture of the screen, only if it is not so
            // already; and the wait for it to be over (0xECBE8, `s20`).
            native::FADE => {
                let (way, colour, frames) = (int(0)?, int(1)?, int(2)?);
                if self.world.scene.fade.script(way, colour, frames) {
                    self.world.scene.capture = Capture::Wanted { whole: false };
                }
                0
            }
            native::WAIT_FADE => {
                if self.world.scene.fade.running() {
                    call.wait();
                }
                0
            }
            native::READ => {
                let what = int(0)?;
                let character = self.state.current.and_then(|id| self.world.scene.characters.get(&id));
                character.map_or(0, |character| match what {
                    READ_STATUS => character.int(int::STATUS),
                    READ_GROUP => character.int(int::GROUP),
                    READ_RESOURCE => character.chr,
                    _ => 0,
                }) as u32
            }
            native::DECIMAL => {
                let what = int(0)?;
                let scene = &*self.world.scene;
                let character = self.state.current.and_then(|id| scene.characters.get(&id));
                let decimal = match (character, what) {
                    // Only a character that a behaviour script runs has these two.
                    (Some(character), DECIMAL_DISTANCE) if character.behaviour.is_some() => {
                        scene.player().map_or(0.0, |player| player.position.distance(character.position))
                    }
                    (Some(character), DECIMAL_NOTICE_RANGE) if character.behaviour.is_some() => character.mind.notice_range,
                    _ => 0.0,
                };
                decimal.to_bits()
            }
            native::ACT => {
                self.act(int(0)?);
                0
            }
            native::POP_PLACE => self.pop_place() as u32,
            native::POP_KIND => self.pop_kind() as u32,

            // Sounds of the banks (0xD42E0, 0xD4354, 0xD43F4).
            native::SOUND => self.sound(int(0)?, int(1)?, int(2)?) as u32,
            native::SOUND_AT => {
                let handle = self.sound(int(0)?, int(1)?, int(2)?);
                if let Ok(handle) = u32::try_from(handle) {
                    self.world.scene.sounds.push(Request::EffectPan(handle, int(3)?));
                }
                handle as u32
            }
            native::SOUND_STOP => {
                if let Ok(handle) = u32::try_from(int(0)?) {
                    self.world.scene.sounds.push(Request::StopEffect(handle));
                }
                0
            }

            // The mission's windows.
            native::WINDOW_TEXT => {
                let (number, kind, of) = (int(0)?, int(1)?, int(2)?);
                if number == window::MISSION_START && kind == TEXT_MISSION_TITLE {
                    self.world.scene.banner_text = self.world.tables.mission_title(of).unwrap_or_default();
                }
                0
            }
            native::WINDOW_STRING => {
                if int(0)? == window::MISSION_START {
                    self.world.scene.banner_text = call.string(2)?;
                }
                0
            }
            native::WINDOW => {
                open_window(self.world.scene, self.world.progress, self.world.tables, int(0)?, int(1)?);
                0
            }
            native::WAIT_WINDOW => {
                if self.window_busy(int(0)?) {
                    call.wait();
                }
                0
            }
            native::COUNTER => {
                set_display(self.world.scene, self.world.progress, int(0)?, int(1)?, int(2)?);
                0
            }

            // The camera, which an event has as a map's script has it (`s79`, `s49`,
            // `s139`): a figure of it set (0xD73CC), 26 being its kind and what is
            // left things of it the port does not have (2 and 3 how near and how
            // far it sees, 23 and 24 whom it goes by); its kind chosen (0xD6470); a
            // figure moved over so many frames (0xD6494), to a decimal.
            native::CAMERA_SET => {
                let (what, value) = (int(0)?, int(1)?);
                match what {
                    1 | 5..=22 => self.world.scene.camera.set(what, value as f32),
                    26 => self.world.scene.set_camera_kind(value),
                    _ => {}
                }
                0
            }
            native::CAMERA_KIND => {
                self.world.scene.set_camera_kind(int(0)?);
                0
            }
            native::CAMERA_MOVE => {
                let (what, to, frames, ease) = (int(0)?, call.float(1)?, int(2)?, int(3)?);
                if matches!(what, 1 | 5..=22) {
                    self.world.scene.camera.move_to(what, to, frames, ease);
                }
                0
            }

            // She is made whole (0xD88).
            native::HEAL => {
                self.world.stats.hp = self.world.stats.max_hp;
                0
            }
            // The save's backup, made as a mission is chosen (0x2398) and put
            // back as one that was failed is left (0x23C4). It keeps her figures
            // and what she carries too, but only the records are ever put back.
            native::BACK_UP => {
                self.world.tables.records.back_up(self.world.progress);
                0
            }
            native::RESTORE => {
                self.world.tables.records.restore(self.world.progress);
                0
            }
            // What a mission's start and end do besides (0xECC70): at its end
            // something to her skills (0x14D73C; there are none yet); at its
            // start the records' news is forgotten (0x14AEB4); and the records
            // are told of a mission cleared (0x14A5DC), which their script has no
            // function for.
            native::BETWEEN_MISSIONS => {
                if int(0)? == 1 {
                    self.world.tables.records.forget_news(self.world.progress);
                }
                0
            }
            // A mission newly opened is told of on the screen between missions.
            // Its title is one of the words a text can have put in (0xEC5F8:
            // a thing's name by its kind, of which nought is a mission); and
            // a text of the script's is said in that screen's window and
            // waited for (0x12FC08; its two answers are never asked for).
            native::OPENED_TEXT => {
                let (which, kind, number) = (int(0)?, int(1)?, int(2)?);
                if let (Some(words), 0) = (usize::try_from(which).ok().and_then(|which| self.state.words.get_mut(which)), kind) {
                    *words = self.world.tables.mission_title(number).unwrap_or_default();
                }
                0
            }
            native::OPENED_WINDOW => {
                if !call.resumed() {
                    let text = self.world.tables.records.text(int(0)?).unwrap_or_default();
                    self.world.scene.said = Some(crate::records::with_words(text, &self.state.words));
                    call.wait();
                } else if self.world.scene.said.is_some() || self.world.scene.saying {
                    call.wait();
                }
                0
            }
            _ => {
                if self.state.unported.insert(number) {
                    warn!("the system script called engine function {number:#x}, which is not ported");
                }
                0
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::host::defaults;
    use crate::field::scene::PLAYER;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    struct Game {
        system: SystemScript,
        tables: FieldTables,
        scene: Scene,
        progress: Progress,
        stats: PlayerStats,
        ground: CollisionWorld,
    }

    /// A new game on the first map with the player standing in the open, if the
    /// game's data has been unpacked.
    fn game() -> Option<Game> {
        let data = GameData::new(DATA).ok()?;
        let tables = FieldTables::load(&data).expect("the field's tables load");
        let system = SystemScript::load(&data, &tables).expect("the system script loads");
        let mut scene = Scene::new(0, 0, defaults(&tables).unwrap());
        let player = Character::new(0, Vec3::ZERO, 0.0, &scene.defaults);
        scene.characters.insert(PLAYER, player);
        let stats = PlayerStats::new_game(&data).expect("the start table loads");
        Some(Game { system, tables, scene, progress: Progress::new_game(), stats, ground: CollisionWorld::default() })
    }

    impl Game {
        fn with<T>(&mut self, run: impl FnOnce(&mut SystemScript, &mut World) -> Result<T>) -> T {
            let mut world = World {
                progress: &mut self.progress,
                scene: &mut self.scene,
                stats: &mut self.stats,
                tables: &self.tables,
                ground: &self.ground,
            };
            run(&mut self.system, &mut world).expect("the system script runs")
        }

        /// Runs an entry as a map's script would, to its end.
        fn entry(&mut self, number: i32) -> u32 {
            self.scene.system = Some(number);
            let mut frames = 0;
            while self.scene.system.is_some() {
                self.with(|system, world| system.update(world));
                // With no one at the buttons the window of a mission cleared
                // is taken as read.
                if let Some(cleared) = &mut self.scene.cleared {
                    cleared.read();
                }
                frames += 1;
                assert!(frames < 10_000, "the entry never came to its end");
            }
            frames
        }
    }

    #[test]
    fn a_function_a_maps_script_asks_for_by_name_is_run_in_the_next_frame() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.with(|system, world| system.call("init_player", &[], world));
        // `s231(4)`: the beginning of the mission against Mazuma, which gives it
        // its time (0x5A000 in the variable 10013) and sets flag 4701.
        game.progress.set_variable(237, 5);
        game.scene.system_call = Some("sys_cmn_start_mis_mazuma");
        game.with(|system, world| system.update(world));
        assert!(game.scene.system_call.is_none());
        assert_eq!((game.progress.variable(237), game.progress.variable(10_013)), (0, 0x5a000));
        assert!(game.progress.flag(4701));
    }

    #[test]
    fn starting_a_mission_counts_it_started_and_puts_its_window_up() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.with(|system, world| system.call("init_player", &[], world));
        // A new player is in no mission, and enemies pop up every ten seconds.
        assert_eq!(game.progress.variable(crate::progress::variable::SCENARIO), -1);
        assert_eq!(game.progress.variable(variable::POP_INTERVAL), 300 * FULL_PACE);
        assert_eq!(game.scene.work.pop_timer, 300 * FULL_PACE);

        game.with(|system, world| system.call("mission_select", &[0], world));
        assert_eq!(game.progress.variable(crate::progress::variable::SCENARIO), 0);
        assert_eq!(game.progress.variable(crate::progress::variable::MISSION_PROGRESS), 100);
        // The story is at the mission's first event, and the mission counts as begun.
        assert_eq!(game.progress.variable(10_003), 1_010_100);
        assert_eq!(game.progress.variable(100), 10);

        let frames = game.entry(400);
        assert_eq!(game.progress.variable(crate::progress::variable::MISSION_PROGRESS), 101);
        // The entry waited for the mission's notice, which named the mission: five
        // frames to come, the jingle's hundred and twenty, and three to go.
        assert_eq!(frames as i32, Notice::JINGLE_FRAMES + 5);
        assert_eq!(game.scene.banner_text, "The Final Weapon Awakens");
        assert!(game.scene.cleared.is_none() && game.scene.notice.is_none());
        assert!(game.scene.sounds.contains(&Request::Say(NOTICE_STREAM, Notice::JINGLE.into())));
        // And what is to be done is on show.
        let shown = panels(&game.progress, &game.tables);
        assert_eq!(shown, [Panel { slot: 0, line: "Defeat all the Eaters".into(), numbers: Numbers::None, values: [0, 0] }]);
    }

    #[test]
    fn clearing_and_failing_a_mission_close_its_display() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.with(|system, world| system.call("mission_select", &[0], world));
        game.entry(400);
        game.entry(401);
        assert_eq!(game.progress.variable(crate::progress::variable::MISSION_PROGRESS), 200);
        assert!(panels(&game.progress, &game.tables).is_empty());
        // Failing leaves the progress where it was, and puts up its window, which
        // the entry waits for from then on: the field is left by the window's doing
        // (`field/failed.rs`), and the entry never comes to its end.
        game.entry(400);
        game.scene.system = Some(403);
        for _ in 0..300 {
            game.with(|system, world| system.update(world));
        }
        assert_eq!(game.progress.variable(crate::progress::variable::MISSION_PROGRESS), 101);
        assert!(panels(&game.progress, &game.tables).is_empty());
        assert!(game.scene.failed.is_some() && game.scene.system == Some(403));
        // "STAGE SELECT" is to be had unless the save says otherwise.
        assert_eq!(game.scene.failed.as_ref().map(|failed| failed.no_stage_select), Some(false));
    }

    #[test]
    fn after_a_battle_every_enemy_about_is_told_to_stand_down() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let defaults = game.scene.defaults;
        for id in [500, 501, 3000] {
            let mut symbol = Character::new(21, Vec3::new(500.0, 0.0, 0.0), 0.0, &defaults);
            symbol.set(setting::BEHAVIOUR, 0);
            game.scene.characters.insert(id, symbol);
        }
        // One that stands still has no behaviour to tell.
        game.scene.characters.insert(527, Character::new(21, Vec3::ZERO, 0.0, &defaults));
        game.scene.work.pop_timer = 10;
        game.entry(300);
        assert!(game.scene.characters[&500].mind.stand_down && game.scene.characters[&501].mind.stand_down);
        assert!(!game.scene.characters[&527].mind.stand_down);
        // One that had popped up is put away, and the next is not due for four
        // seconds at the least.
        assert!(game.scene.characters[&3000].hidden());
        assert_eq!(game.scene.work.pop_timer, 120 * FULL_PACE);
    }

    #[test]
    fn enemies_pop_up_ahead_of_her_as_she_goes() {
        use crate::field::scene::flag;

        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        game.ground = crate::field::motion::tests::floor();
        game.with(|system, world| system.call("init_player", &[], world));
        game.with(|system, world| system.call("sys_fld_enter_map", &[], world));
        assert_eq!(game.progress.variable(variable::POP_LIMIT), 3);

        // Nothing comes up until a map's script says enemies may.
        for _ in 0..60 {
            game.with(|system, world| system.frame(Frame::of(100.0), world));
        }
        assert_eq!(game.scene.characters.len(), 1);
        assert_eq!(game.scene.work.pop_distance, 0.0);

        // The first map's later missions: Hornets, four at the most.
        write_value(&mut game.scene, &mut game.progress, value::POPS, 1);
        game.progress.set_variable(variable::POP_GROUP, 4);
        game.progress.set_variable(variable::POP_LIMIT, 4);
        // One comes up for every 3000 she goes.
        for _ in 0..29 {
            game.with(|system, world| system.frame(Frame::of(100.0), world));
        }
        assert_eq!(game.scene.characters.len(), 1);
        game.with(|system, world| system.frame(Frame::of(100.0), world));
        let hornet = &game.scene.characters[&3000];
        assert_eq!(hornet.chr, 22);
        // Ahead of her by 1500, give or take 300, and on the ground.
        let ahead = hornet.position - Vec3::new(0.0, 0.0, 1500.0);
        assert!(ahead.length() <= 300.0 && hornet.position.y == 0.0, "{}", hornet.position);
        assert_eq!(hornet.mind.home, hornet.position);
        // It is a symbol that comes up out of the ground, and goes again in five
        // seconds unless it finds her.
        assert_eq!(hornet.behaviour, Some(0));
        for flag in [flag::SYMBOL, flag::HIDES_SOON, 0x9c, 0x9e] {
            assert!(hornet.flags.has(flag), "flag {flag:#x}");
        }
        assert_eq!(hornet.mind.hide_timer, 150 * FULL_PACE);
        // The count begins again.
        assert_eq!(game.scene.work.pop_distance, 0.0);
        assert_eq!(game.scene.work.pop_timer, 300 * FULL_PACE);

        // No more come than the map allows.
        for _ in 0..400 {
            game.with(|system, world| system.frame(Frame::of(100.0), world));
        }
        assert_eq!(game.scene.shown(3000, 3099).count(), 4);
        // After a battle those that came up are put away.
        game.entry(300);
        assert_eq!(game.scene.shown(3000, 3099).count(), 0);
    }

    #[test]
    fn an_event_opens_the_window_says_its_line_and_closes_it_again() {
        use crate::controls::Pad;

        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // What a map's script asks for when the Hornets come: Phobos has a word.
        game.scene.events.push_back((116, 0));
        game.with(|system, world| system.update(world));
        assert!(game.scene.event_running && game.scene.events.is_empty());
        let comm = &game.scene.comm;
        assert!(comm.open && comm.busy());
        assert_eq!((comm.faces(), comm.bundle.as_str()), ([Some(1), None], "FLD/FEVT/FEVT_C_S1M1_AD06.BIN"));
        // The line waits for his face to have come in.
        assert!(comm.line().is_none());

        // The script waits for as long as the line's voice lasts, and then takes
        // the face out and closes the window.
        let (mut frames, mut said) = (0, None);
        while game.scene.event_running {
            game.scene.comm.step(&Pad::default(), false);
            if let (None, Some(line)) = (&said, game.scene.comm.line()) {
                said = Some((frames, text::plain(&line.text), line.speaker, line.voice, line.time));
            }
            game.with(|system, world| system.update(world));
            frames += 1;
            assert!(frames < 1000, "the event never came to its end");
        }
        let words = "More enemies inbound! Be quick\nor they'll just keep coming!".to_string();
        assert_eq!(said, Some((7, words, 0, Voice::Frames(103), -1)));
        assert_eq!(frames, 112);
        // The window and the face are on their way out, which takes them ten of
        // the interface's frames.
        let comm = &mut game.scene.comm;
        assert!(!comm.open && comm.busy() && comm.window.open && comm.window.closing);
        for _ in 0..6 {
            comm.step(&Pad::default(), false);
        }
        assert!(!comm.busy() && !comm.window.open && comm.faces() == [None, None]);

        // Events wait their turn: a second one asked for at once starts when the
        // first is over, and one the list has no bundle for is passed by.
        game.scene.events.extend([(116, 0), (101, 0), (118, 0)]);
        game.with(|system, world| system.update(world));
        assert_eq!(game.scene.events.len(), 2);
        for _ in 0..112 {
            game.scene.comm.step(&Pad::default(), false);
            game.with(|system, world| system.update(world));
        }
        assert!(game.scene.event_running && game.scene.events.is_empty());
        assert_eq!(game.scene.comm.bundle, "FLD/FEVT/FEVT_C_S1M1_GO00.BIN");
    }

    #[test]
    fn an_event_has_her_take_a_pose_for_the_call() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The first call of the first mission: she stands at ease for it, going
        // over to that in twenty frames, and it loops.
        let call = game.tables.events.iter().position(|name| name == "FEVT_C_S1M1_ST00").expect("the game has the event");
        game.scene.events.push_back((call as i32, 0));
        game.with(|system, world| system.update(world));
        assert!(game.scene.event_running);
        let player = game.scene.player().unwrap();
        assert_eq!(player.motion, Some((Motion::Own(1), true)));
        assert_eq!(player.blend_frames(), 20);
        // Five frames on, the script has what follows go over in the usual eight.
        for _ in 0..6 {
            game.with(|system, world| system.update(world));
        }
        assert_eq!(game.scene.player().unwrap().blend_frames(), 8);
        assert_eq!(game.scene.player().unwrap().motion, Some((Motion::Own(1), true)));
    }

    /// `update_weakness` of the system script, with the count the engine gives it
    /// (0x10D370): each second what she guards loses ten (the tunables' + 0x4C)
    /// for each enemy about it, and when it has nothing left the field is to be
    /// left with the guarded character's own number.
    #[test]
    fn what_she_guards_loses_as_enemies_stand_about_it_and_its_loss_has_the_field_left() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let defaults = crate::field::scene::test_defaults();
        for (id, x) in [(600, 0.0), (500, 100.0), (501, 400.0), (3000, 299.0), (550, 50.0), (700, 10.0), (3100, 10.0)] {
            game.scene.characters.insert(id, Character::new(1, Vec3::new(x, 0.0, 0.0), 0.0, &defaults));
        }
        game.scene.characters.get_mut(&550).unwrap().flags.set(flag::HIDDEN, true);
        let about = |game: &Game| read_value(&game.scene, &game.progress, &game.tables, value::ENEMIES_NEAR);
        // Nobody is guarded until a script names someone.
        assert_eq!((read_value(&game.scene, &game.progress, &game.tables, value::GUARDED), about(&game)), (-1, 0));
        write_value(&mut game.scene, &mut game.progress, value::GUARDED, 600);
        // Of the map's enemies and those that popped up, the two that are not
        // hidden and within 300: not the one 400 away, the hidden one, nor those
        // whose numbers are no enemy's.
        assert_eq!(about(&game), 2);
        const LEFT: i32 = 10_011;
        game.progress.set_variable(LEFT, 25);
        let frame = Frame { own_time: 0x800, time: 0x800, moved: 0.0 };
        let mut seconds = |game: &mut Game, frames: usize| {
            for _ in 0..frames {
                game.with(|system, world| system.frame(frame, world));
            }
        };
        // A second is over once more than 0xF000 has gone by: thirty-one frames.
        seconds(&mut game, 30);
        assert_eq!(game.progress.variable(LEFT), 25);
        seconds(&mut game, 1);
        assert_eq!(game.progress.variable(LEFT), 5);
        assert!(!game.scene.flag(field_flag::LEAVE));
        seconds(&mut game, 31);
        assert_eq!(game.progress.variable(LEFT), 0);
        assert!(game.scene.flag(field_flag::LEAVE));
        assert_eq!(read_value(&game.scene, &game.progress, &game.tables, value::EXIT), 600);
    }

    #[test]
    fn the_missions_clock_runs_down_by_the_time_that_passes() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // A map's script gives the mission three minutes.
        write_value(&mut game.scene, &mut game.progress, value::MISSION_SECONDS, 180);
        assert_eq!(game.progress.variable(variable::MISSION_TIME), 180_000);
        assert_eq!(read_value(&game.scene, &game.progress, &game.tables, value::MISSION_SECONDS), 180);
        // A second of frames takes a second off, give or take the script's rounding.
        for _ in 0..30 {
            game.with(|system, world| system.frame(Frame::of(0.0), world));
        }
        let left = game.progress.variable(variable::MISSION_TIME);
        assert!((178_900..=179_000).contains(&left), "{left}");
        // The second panel of the mission's display has it.
        assert_eq!(game.progress.variable(display::NUMBERS[1][0]), left);
        // A battle's time comes off it in one go (0xD8A14).
        game.with(|system, world| system.call("update_mis_time", &[20 * 0xf000], world));
        let after = game.progress.variable(variable::MISSION_TIME);
        assert!((158_800..=159_000).contains(&after), "{after}");
        // When it runs out the field is to be left.
        game.with(|system, world| system.call("update_mis_time", &[200 * 0xf000], world));
        assert_eq!(game.progress.variable(variable::MISSION_TIME), 0);
        assert!(game.scene.flag(field_flag::LEAVE));
    }
}
