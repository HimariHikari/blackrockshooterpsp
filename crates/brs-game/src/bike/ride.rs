//! A ride on the bike as it goes: the road, what is on it, the scripts that run
//! it and the eighty-four engine functions they call (the table at 0x172D98;
//! `docs/BIKE_NOTES.md`, "The scripts' functions"). Nothing here draws or
//! sounds: what is to be seen and heard is kept for what does.

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use bevy::log::warn;
use bevy::math::{EulerRot, Quat, Vec3};
use brs_formats::script::{Call, Host, Run, Script};

use super::camera::Camera;
use super::chara::{entry, flag, Character, HitBox, Told, Trap, BLOW, NEAR};
use super::display::Display;
use super::road::Road;
use super::rules::{self, Asked, Ended, Rules, State};
use super::script::{Instance, Looks, Who, Work, OWN_BASE, SHARED_BASE};
use super::tables::{
    self, course_pack, CourseTables, Enemy, Gate, Pack, Resource, RoadTables, SystemTables, ANIMATIONS, CAMERA_SCRIPT, COURSE_ROAD, COURSE_SCRIPT, ENEMY_RECORD, MADE,
    ROW, STATE_LEN, SYSTEM_PACK, SYSTEM_TABLES, TRAP_RECORD,
};
use crate::data::GameData;
use crate::field::scene::{fade_kind, Fade};

/// The time a frame takes (0x149CBC): the game runs at thirty a second, and the
/// interface's frames are two to each of the game's (0x850).
pub const FRAME: f32 = 1.0 / 30.0;
const STEPS: i32 = 2;
/// What the game's time is multiplied by, for each pace (the game's + 0xC).
const PACES: [f32; 3] = [0.0, 1.0, 0.12];
/// A function of a character's that returns at once is begun again at once:
/// this many times in a frame is taken as a script that has gone wrong.
const ROUNDS: usize = 64;
/// How many sounds of the mini-game's own can play at once, each in its slot.
pub const SOUND_SLOTS: usize = 23;

/// How a ride came out, by the numbers the map's script reads (`s260`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Through = 1,
    Failed = 2,
    GivenUp = 3,
}

/// A sound that is to be played or stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundAsked {
    /// One of the mini-game's own, into the slot of its number, so loud (0 to
    /// 0x7f).
    Play(i32, i32),
    Stop(i32),
    /// One of the common game's.
    Common(i32),
    Voice(i32),
    Music(i32),
    MusicOut(i32),
}

/// A motion of the camera's own model that the camera rides along on (`n4d`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraMotion {
    /// Which of the system's models, and which of its animations.
    pub model: usize,
    pub animation: i32,
    /// How far in it is, and how long it is, in seconds.
    pub at: f32,
    pub length: f32,
    /// Which of the ride's motions it is, counted from one.
    pub begun: u32,
}

impl CameraMotion {
    /// Whether it is still playing (0x13601C).
    pub fn plays(&self) -> bool {
        self.at < self.length
    }
}

/// Talk: the field's events that the course's script asks for by number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Talk {
    /// A bit for each one asked for and not yet begun.
    pub asked: u32,
    /// The one that goes on.
    pub going: Option<i32>,
}

/// A thing of scenery beside the road: up to five pieces of it.
#[derive(Clone, Debug)]
pub struct Scenery {
    pub place: Vec3,
    /// How far off it is still seen.
    pub sight: f32,
    /// 0 and 1 for a row's two patterns, 2 for the sky.
    pub kind: i32,
    pub looks: Looks,
}

/// What a character has besides what the rules see of it.
#[derive(Default)]
pub struct Rider {
    pub looks: Looks,
    scripts: [Option<Box<Instance>>; 3],
}

/// The ride.
pub struct Ride {
    pub tables: SystemTables,
    pub road_tables: RoadTables,
    pub road: Road,
    /// The scripts the system's table names, and how long each model's
    /// animations are, in seconds.
    scripts: Vec<Option<Arc<Script>>>,
    lengths: Vec<[f32; ANIMATIONS]>,
    /// What is on the road, the last made first, and what each has besides.
    pub characters: Vec<Character>,
    pub riders: Vec<Rider>,
    pub scenery: Vec<Scenery>,
    pub traps: Vec<Trap>,
    pub rules: Rules,
    /// The course's flags and numbers, then its two tables.
    pub shared: Vec<u8>,
    enemies_at: usize,
    gates_at: usize,
    system: Option<Box<Instance>>,
    course: Option<Box<Instance>>,
    camera_script: Option<Box<Instance>>,
    /// The scripts that run now, the innermost last, and whether each can be
    /// broken in on; and entries asked of them meanwhile.
    running: Vec<(Who, Arc<Script>, bool)>,
    pending: Vec<(Who, u32, i32, i32)>,
    pub camera: Camera,
    /// The camera's own motion, while it rides along on one.
    pub camera_motion: Option<CameraMotion>,
    camera_motions: u32,
    pub camera_held: bool,
    pub display: Display,
    pub fade: Fade,
    pub talk: Talk,
    pub sounds: Vec<SoundAsked>,
    /// Which of the mini-game's sounds are taken as playing.
    pub playing: [bool; SOUND_SLOTS],
    pub loudness: i32,
    /// The time this frame takes, and which of the paces the game goes at.
    pub time: f32,
    pub pace: usize,
    /// The frames counted since the ride began, and by `n0d`.
    pub frames: i32,
    counted: i32,
    next_number: i32,
    next_trap: i32,
    /// A blow has taken something from her (the game's flag 0x1818).
    pub hurt: bool,
    pub outcome: Option<Outcome>,
    /// The course's script is to finish before the ride is over.
    winding_up: bool,
    traps_stopped: bool,
    asked: Asked,
    pressed: u32,
    random: u64,
    /// What has gone wrong with the scripts, for a run to be judged by.
    pub faults: Vec<String>,
}

/// The buttons `n4f` asks after, by its bits.
pub mod pressed {
    pub const LEFT: u32 = 1;
    pub const RIGHT: u32 = 2;
    pub const TRIANGLE: u32 = 4;
    pub const SQUARE: u32 = 8;
    pub const CROSS: u32 = 16;
}

pub(super) fn turn(turns: Vec3) -> Quat {
    Quat::from_euler(EulerRot::ZYX, turns.z.to_radians(), turns.y.to_radians(), turns.x.to_radians())
}

/// The turns that face a way (0x13A744): its heading, and how far it looks up.
fn turns_towards(way: Vec3, turns: &mut Vec3) {
    let Some(way) = way.try_normalize() else { return };
    if let Some(level) = Vec3::new(way.x, 0.0, way.z).try_normalize() {
        let angle = level.z.clamp(-1.0, 1.0).acos().to_degrees();
        turns.y = if level.x < 0.0 { -angle } else { angle };
    }
    let up = Vec3::new(0.0, way.y, 1.0).normalize();
    let angle = up.z.clamp(-1.0, 1.0).acos().to_degrees();
    turns.x = if -up.y > 0.0 { -angle } else { angle };
}

impl Ride {
    /// Reads a course and makes the ride ready to begin (the task's states 3 to
    /// 11, 0x147C4C): the tables, the road, the system's and the course's
    /// `int_init`, the camera's script, and the course's `int_start`.
    pub fn new(data: &GameData, course: &str) -> Result<Ride> {
        let member = |pack: &str, number: usize| -> Result<Vec<u8>> {
            data.member(pack, number)?.with_context(|| format!("{pack} has nothing at {number}"))
        };
        let script = |pack: &str, number: usize| -> Result<Arc<Script>> {
            Ok(Arc::new(Script::parse(&member(pack, number)?).with_context(|| format!("member {number} of {pack}"))?))
        };
        let pack = course_pack(course);
        let system_script = script(SYSTEM_PACK, SYSTEM_TABLES)?;
        let tables = SystemTables::read(&system_script)?;
        let road_tables = RoadTables::read(&*script(&pack, COURSE_ROAD)?)?;
        let course_script = script(&pack, COURSE_SCRIPT)?;
        let course_tables = CourseTables::read(&course_script)?;
        let road = Road::build(&road_tables);

        let of = |resource: Resource| -> Option<Vec<u8>> {
            let pack = match resource.pack {
                Pack::System => SYSTEM_PACK,
                Pack::Course => &pack,
                Pack::Common => return None,
            };
            data.member(pack, resource.member).ok().flatten()
        };
        let scripts = tables
            .scripts
            .iter()
            .map(|resource| resource.and_then(of).and_then(|bytes| Script::parse(&bytes).ok()).map(Arc::new))
            .collect();
        let lengths = tables
            .models
            .iter()
            .map(|model| {
                // The camera's own model is a camera's rig, whose animations have a
                // track for the lens besides.
                let rig = model.model.and_then(of).filter(|bytes| brs_formats::cam::is_camera(bytes));
                model.animations.map(|animation| {
                    let read = |bytes: Vec<u8>| match &rig {
                        Some(rig) => brs_formats::cam::Camera::from_parts(rig, &bytes).ok().and_then(|camera| camera.animation),
                        None => brs_formats::insa::Animation::parse(&bytes).ok(),
                    };
                    animation.and_then(of).and_then(read).map_or(0.0, |animation| animation.frames() as f32 / brs_formats::insa::FRAMES_PER_SECOND)
                })
            })
            .collect();

        let mut shared = tables.state.to_vec();
        let enemies_at = shared.len();
        shared.extend_from_slice(&course_tables.enemies);
        let gates_at = shared.len();
        shared.extend_from_slice(&course_tables.gates);

        let mut ride = Ride {
            scenery: scenery_of(&road_tables),
            tables,
            road_tables,
            road,
            scripts,
            lengths,
            characters: Vec::new(),
            riders: Vec::new(),
            traps: Vec::new(),
            rules: Rules::default(),
            shared,
            enemies_at,
            gates_at,
            system: Some(Box::new(Instance::new(&system_script, false, -1, 0))),
            course: Some(Box::new(Instance::new(&course_script, false, -1, 0))),
            camera_script: None,
            running: Vec::new(),
            pending: Vec::new(),
            camera: Camera::default(),
            camera_motion: None,
            camera_motions: 0,
            camera_held: false,
            display: Display::default(),
            fade: Fade::DARK,
            talk: Talk::default(),
            sounds: Vec::new(),
            playing: [false; SOUND_SLOTS],
            loudness: 0,
            time: FRAME,
            pace: 1,
            frames: 0,
            counted: 0,
            next_number: 0x100,
            next_trap: 0x100,
            hurt: false,
            outcome: None,
            winding_up: false,
            traps_stopped: false,
            asked: Asked::default(),
            pressed: 0,
            random: 0x2545_f491_4f6c_dd1d,
            faults: Vec::new(),
        };
        ride.loudness = ride.road_tables.loudness.clamp(0, 255);
        // The stage's tune is asked for as its files are (the task's state 8).
        if let Some(stage) = ride.tables.stage(course) {
            ride.sounds.push(SoundAsked::Music(stage.tune));
        }
        ride.begin(Who::System, entry::INIT, 0, 0);
        ride.begin(Who::Course, entry::INIT, 0, 0);
        ride.camera_script = Some(Box::new(Instance::new(&script(SYSTEM_PACK, CAMERA_SCRIPT)?, false, -1, 0)));
        ride.begin(Who::Camera, entry::INIT, 0, 0);
        ride.begin(Who::Course, entry::START, 0, 0);
        Ok(ride)
    }

    /// The course's flags and numbers.
    pub fn state(&mut self) -> State<'_> {
        State((&mut self.shared[..STATE_LEN]).try_into().unwrap())
    }

    /// She.
    pub fn her(&self) -> Option<&Character> {
        self.character(0)
    }

    pub fn character(&self, number: i32) -> Option<&Character> {
        self.characters.iter().find(|character| character.number == number)
    }

    fn index(&self, number: i32) -> Option<usize> {
        self.characters.iter().position(|character| character.number == number)
    }

    /// What a rule of thumb asks of the pad, for a ride nobody plays: away from
    /// whatever is ahead in her lane, by the display's signs past a hole in the
    /// road, and her special whenever it can be had.
    pub fn rule_of_thumb(&self) -> Asked {
        let mut asked = Asked { special: true, ..Asked::default() };
        let Some(she) = self.her() else { return asked };
        let lane = |character: &Character| character.chip().map(|chip| self.road.chips[chip].lane);
        let ahead = |of: Option<usize>| self.characters.iter().any(|other| other.number != 0 && lane(other) == of && (0.0..4000.0).contains(&(other.place.z - she.place.z)));
        if ahead(lane(she)) && !she.mover.changing {
            let left = lane(she).and_then(|lane| lane.checked_sub(1));
            (asked.left, asked.right) = if left.is_some_and(|left| left >= 1 && !ahead(Some(left))) { (true, false) } else { (false, true) };
        }
        match self.rules.shown.signs {
            [true, _] => (asked.left, asked.right) = (true, false),
            [false, true] => (asked.left, asked.right) = (false, true),
            _ => {}
        }
        asked
    }

    /// Has her as whole as she can be, for such a ride.
    pub fn unharmed(&mut self) {
        if let Some(index) = self.index(0) {
            self.characters[index].left = self.characters[index].could;
        }
    }

    /// An effect of a character's has played out: it is taken out of its slot
    /// (0x13C9C0), if it is still the one that was put there.
    pub fn effect_over(&mut self, number: i32, slot: usize, made: u32) {
        let Some(index) = self.index(number) else { return };
        if let Some(effect) = self.riders[index].looks.effects.get_mut(slot).filter(|effect| effect.is_some_and(|effect| effect.made == made)) {
            *effect = None;
        }
    }

    /// Whether the ride is over, for the field to be gone back to (0x1498E4).
    pub fn over(&self) -> bool {
        self.outcome.is_some() && !(self.winding_up && self.course.as_ref().is_some_and(|course| course.machine.waiting()))
    }

    /// The ride is given up from the pause (0x149FCC, 5).
    pub fn give_up(&mut self) {
        (self.outcome, self.winding_up) = (Some(Outcome::GivenUp), false);
    }

    /// A frame of the ride (0x149CBC and the task's state 12): what the pad asks
    /// of it, and the buttons that went down, by `pressed`'s bits.
    pub fn frame(&mut self, asked: Asked, pressed: u32) {
        (self.asked, self.pressed) = (asked, pressed);
        self.frames += STEPS;
        self.time = FRAME * PACES[self.pace.min(PACES.len() - 1)];
        let ended: Vec<i32> = self.characters.iter().filter(|character| character.ending).map(|character| character.number).collect();
        for number in ended {
            if let Some(index) = self.index(number) {
                self.characters.remove(index);
                self.riders.remove(index);
            }
        }
        self.fade.advance();
        // The display's frame, with the sound of a sign that comes (0x132F9C).
        let speeds = std::array::from_fn(|gear| self.state().number(tables::state::SPEEDS + gear * 4));
        if self.display.frame(speeds, STEPS) {
            self.play_sound(rules::sound::SIGN, 0x7f);
        }
        if self.talk.going.is_none() && self.talk.asked != 0 {
            let talk = self.talk.asked.trailing_zeros() as i32;
            self.talk = Talk { asked: self.talk.asked & !(1 << talk), going: Some(talk) };
            if self.playing[rules::sound::TIME_SHORT as usize] {
                self.sounds.push(SoundAsked::Play(rules::sound::TIME_SHORT, 0x20));
            }
            self.stop_sound(rules::sound::ROW_RUNS_OUT);
        }
        if self.over() {
            return;
        }
        self.run_on(Who::System);
        self.run_on(Who::Course);
        if !self.state().has(tables::state::WORLD_STANDS) {
            self.world();
            self.come_upon();
        }
    }

    /// The talk that went on is over: the course's script may go on, and the
    /// sound turned down for it comes up again (0x149CBC).
    pub fn talk_over(&mut self) {
        if self.talk.going.take().is_some() && self.playing[rules::sound::TIME_SHORT as usize] {
            self.sounds.push(SoundAsked::Play(rules::sound::TIME_SHORT, 0x7f));
        }
    }

    /// A frame of the world (0x142574).
    fn world(&mut self) {
        let time = self.time;
        let numbers: Vec<i32> = self.characters.iter().map(|character| character.number).collect();
        for &number in &numbers {
            let Some(index) = self.index(number) else { continue };
            for told in self.characters[index].step(&self.road, time) {
                self.tell(number, told);
            }
        }
        if !self.traps_stopped {
            for index in 0..self.traps.len() {
                let trap = &self.traps[index];
                if trap.gone_off || !self.character(trap.watches).is_some_and(|watched| trap.passed(watched)) {
                    continue;
                }
                let (tells, entry, event, with) = (trap.tells, trap.entry, trap.event, trap.with);
                self.traps[index].gone_off = true;
                match tells {
                    0 => self.begin(Who::System, entry, event, with),
                    1 => self.begin(Who::Course, entry, event, with),
                    3 => self.begin(Who::Camera, entry, event, with),
                    // The road's script has no entry that could be begun.
                    _ => {}
                }
            }
        }
        for rider in &mut self.riders {
            rider.looks.step(time);
        }
        for &number in &numbers {
            for slot in 0..3 {
                self.run_on(Who::Of(number, slot));
            }
        }
        // The camera (0x136054): its script, then where it is.
        if !self.camera.stopped {
            self.run_on(Who::Camera);
            let followed = self.character(self.camera.follows).map(|followed| followed.place);
            self.camera.riding_along = self.camera_motion.is_some();
            self.camera.step(time, self.pace, STEPS, followed);
            if let Some(motion) = &mut self.camera_motion {
                motion.at += time;
            }
        }

        let (asked, winding_up) = (self.asked, self.outcome.is_some());
        let state: &mut [u8; STATE_LEN] = (&mut self.shared[..STATE_LEN]).try_into().unwrap();
        let happened = self.rules.step(&mut State(state), &self.road, &mut self.characters, asked, time);
        self.display.given(&self.rules.shown);
        for (number, told) in happened.told_first {
            self.begin(Who::Of(number, 0), told.entry, told.event, told.with);
        }
        for (number, told) in happened.told {
            self.tell(number, told);
        }
        // The camera is told of her lane while it is she it goes with.
        for (event, with) in happened.camera {
            if self.camera.follows == self.rules.her {
                self.begin(Who::Camera, entry::EVENT, event, with);
            }
        }
        for sound in happened.sounds {
            match sound {
                rules::Sound::Play(slot) => self.play_sound(slot, 0x7f),
                rules::Sound::Stop(slot) => self.stop_sound(slot),
                rules::Sound::Voice(voice) => self.sounds.push(SoundAsked::Voice(voice)),
            }
        }
        if happened.special_begun {
            self.display.special();
        }
        self.hurt |= happened.hurt;
        match happened.ended {
            Some(_) if winding_up => {}
            Some(Ended::Goal) => self.end(Outcome::Through),
            Some(Ended::Failed) => self.end(Outcome::Failed),
            None => {}
        }
    }

    /// The ride's end (0x149FCC, 3 and 4): the course's script has its say
    /// first.
    fn end(&mut self, outcome: Outcome) {
        (self.outcome, self.winding_up) = (Some(outcome), true);
        match outcome {
            Outcome::Through => self.begin(Who::Course, entry::FINISH, 0, 0),
            _ => {
                self.begin(Who::Course, entry::GAME_OVER, 0, 0);
                self.traps_stopped = true;
            }
        }
    }

    /// What the course's tables put on the road as she comes within a row of it
    /// (0x147738, 0x147924).
    fn come_upon(&mut self) {
        let Some(horizon) = self.her().map(|she| she.place.z + ROW) else { return };
        for index in 0..(self.gates_at - self.enemies_at) / ENEMY_RECORD {
            let at = self.enemies_at + index * ENEMY_RECORD;
            let enemy = Enemy::read(&self.shared[at..at + ENEMY_RECORD]);
            let part = enemy.along as f32 / 100.0;
            if self.shared[at] & MADE != 0 || horizon < (enemy.row as f32 + part) * ROW {
                continue;
            }
            self.shared[at] |= MADE;
            let Some(chip) = self.road.chip_of(enemy.lane, false, enemy.row) else { continue };
            let number = self.make(enemy.number, enemy.kind, enemy.with);
            if let Some(index) = self.index(number) {
                let character = &mut self.characters[index];
                character.put(&self.road, chip);
                character.set_part(&self.road, part);
                character.place = self.road.at(chip, part);
            }
        }
        for index in 0..(self.shared.len() - self.gates_at) / TRAP_RECORD {
            let at = self.gates_at + index * TRAP_RECORD;
            let gate = Gate::read(&self.shared[at..at + TRAP_RECORD]);
            let part = gate.along as f32 / 100.0;
            if self.shared[at] & MADE != 0 || horizon < (gate.row as f32 + part) * ROW {
                continue;
            }
            self.shared[at] |= MADE;
            let Some(chip) = self.road.chip_of(gate.lane, false, gate.row) else { continue };
            let number = if gate.number < 0 { self.new_trap_number() } else { gate.number };
            let mut trap = Trap::new(number);
            (trap.watches, trap.tells, trap.entry, trap.event, trap.with) = (0, 1, entry::EVENT, gate.event, gate.with);
            (trap.place, trap.size) = (self.road.at(chip, part), Vec3::from(gate.size));
            self.traps.insert(0, trap);
        }
    }

    fn new_trap_number(&mut self) -> i32 {
        self.next_trap += 1;
        self.next_trap - 1
    }

    /// Makes a character of a kind, with the script the kind has (0x149968).
    /// Its number is answered.
    fn make(&mut self, number: i32, kind: usize, with: i32) -> i32 {
        let number = if number < 0 {
            self.next_number += 1;
            self.next_number - 1
        } else {
            number
        };
        self.characters.insert(0, Character::new(number));
        self.riders.insert(0, Rider::default());
        let script = self.tables.kinds.get(kind).filter(|&&script| script >= 0).and_then(|&script| self.scripts.get(script as usize)).cloned().flatten();
        if let Some(script) = script {
            self.riders[0].scripts[0] = Some(Box::new(Instance::new(&script, true, number, with)));
            self.begin(Who::Of(number, 0), entry::INIT, 0, 0);
        }
        number
    }

    /// Tells all three of a character's scripts something (0x13C6DC).
    fn tell(&mut self, number: i32, told: Told) {
        for slot in 0..3 {
            self.begin(Who::Of(number, slot), told.entry, told.event, told.with);
        }
    }

    fn held(&mut self, who: Who) -> Option<&mut Option<Box<Instance>>> {
        match who {
            Who::System => Some(&mut self.system),
            Who::Course => Some(&mut self.course),
            Who::Camera => Some(&mut self.camera_script),
            Who::Of(number, slot) => {
                let index = self.index(number)?;
                self.riders[index].scripts.get_mut(slot)
            }
        }
    }

    /// Begins an entry of a script by its number, at once, whatever the script
    /// was in the middle of (0x13F1F4); the event and what comes with it are
    /// what the script then reads. A script that is itself running further out
    /// is broken in on as soon as it has the say again.
    pub fn begin(&mut self, who: Who, entry: i32, event: i32, with: i32) {
        let name = entry::NAMES.get(entry as usize).copied().unwrap_or("");
        if let Some((_, script, can_be)) = self.running.iter().find(|(running, ..)| *running == who) {
            if let Some(address) = script.symbol(name).filter(|_| *can_be) {
                self.pending.push((who, address, event, with));
            }
            return;
        }
        let Some(mut instance) = self.held(who).and_then(Option::take) else { return };
        (instance.work.event, instance.work.came) = (event, with);
        if let Some(address) = instance.script.symbol(name).filter(|_| !instance.work.no_breaking_in) {
            self.drive(who, &mut instance, Some(address));
        }
        self.put_back(who, instance);
    }

    /// A frame of a script: it runs on from where it waited (0x13F278).
    fn run_on(&mut self, who: Who) {
        if self.running.iter().any(|(running, ..)| *running == who) {
            return;
        }
        let Some(mut instance) = self.held(who).and_then(Option::take) else { return };
        self.drive(who, &mut instance, None);
        self.put_back(who, instance);
    }

    fn put_back(&mut self, who: Who, instance: Box<Instance>) {
        if let Some(held) = self.held(who) {
            // A script that put another in its own slot is the one that goes.
            held.get_or_insert(instance);
        }
    }

    /// Runs a script that has been taken out of where it is kept: from an
    /// address, or on from where it waited.
    fn drive(&mut self, who: Who, instance: &mut Instance, begin: Option<u32>) {
        self.running.push((who, instance.script.clone(), !instance.work.no_breaking_in));
        let mut begin = begin;
        for round in 0.. {
            if begin.is_none() && !instance.machine.waiting() {
                break;
            }
            instance.machine.map(SHARED_BASE, std::mem::take(&mut self.shared));
            let Instance { machine, work, .. } = &mut *instance;
            let mut host = Functions { ride: self, who, work };
            let run = match begin.take() {
                Some(address) => machine.start(address, &[], &mut host),
                None => machine.resume(&mut host),
            };
            self.shared = instance.machine.mapped(SHARED_BASE).map(<[u8]>::to_vec).unwrap_or_default();
            let asked = self.pending.iter().position(|(pending, ..)| *pending == who).map(|at| self.pending.remove(at));
            match run {
                Err(error) => {
                    warn!("a script of the ride ({who:?}) went wrong and is stopped: {error:#}");
                    self.faults.push(format!("{who:?}: {error:#}"));
                    instance.within = 0;
                    instance.machine = brs_formats::script::Machine::new(&instance.script);
                    instance.machine.map(OWN_BASE, vec![0; super::script::OWN_LEN]);
                    break;
                }
                Ok(_) if asked.is_some() => {
                    let (_, address, event, with) = asked.unwrap();
                    (instance.work.event, instance.work.came) = (event, with);
                    begin = Some(address);
                }
                Ok(Run::Waiting) => break,
                Ok(Run::Returned(value)) => {
                    if !instance.rounds {
                        break;
                    }
                    if value != 0 {
                        instance.within = value;
                    }
                    if instance.within == 0 {
                        break;
                    }
                    if round >= ROUNDS {
                        warn!("a script of the ride ({who:?}) goes round without waiting and is stopped");
                        self.faults.push(format!("{who:?} goes round without waiting"));
                        instance.within = 0;
                        break;
                    }
                    begin = Some(instance.within);
                }
            }
        }
        self.running.pop();
    }

    fn play_sound(&mut self, slot: i32, loudness: i32) {
        // Sound 13 is not begun while there is talk (0x149624).
        if !(0..SOUND_SLOTS as i32).contains(&slot) || (slot == rules::sound::ROW_RUNS_OUT && self.busy_talking()) {
            return;
        }
        self.playing[slot as usize] = true;
        self.sounds.push(SoundAsked::Play(slot, loudness));
    }

    fn stop_sound(&mut self, slot: i32) {
        if (0..SOUND_SLOTS as i32).contains(&slot) && std::mem::take(&mut self.playing[slot as usize]) {
            self.sounds.push(SoundAsked::Stop(slot));
        }
    }

    /// Whether any talk is asked for or goes on (0x14983C).
    fn busy_talking(&self) -> bool {
        self.talk.asked != 0 || self.talk.going.is_some()
    }

    fn length_of(&self, model: usize, animation: i32) -> f32 {
        self.lengths.get(model).and_then(|lengths| lengths.get(animation as usize)).copied().unwrap_or(0.0)
    }

    fn random(&mut self, below: i32) -> i32 {
        chance(&mut self.random, below)
    }
}

/// A number under `below` (0x93A0 stands behind the original's).
fn chance(state: &mut u64, below: i32) -> i32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    if below > 0 {
        (*state >> 33) as i32 % below
    } else {
        0
    }
}

/// The scenery a road's tables name (the end of 0x139368): the sky, and for
/// each row each of its two patterns that has anything.
fn scenery_of(tables: &RoadTables) -> Vec<Scenery> {
    let mut scenery = Vec::new();
    if tables.sky >= 0 {
        let mut looks = Looks::default();
        (looks.pieces[0].resource, looks.pieces[0].drawn) = (Some(tables.sky as usize), 0x100_0000);
        scenery.push(Scenery { place: Vec3::ZERO, sight: 0.0, kind: 2, looks });
    }
    for (kind, drawn) in [(0, 0x200_0000), (1, 0x400_0000)] {
        for (row, of_row) in tables.rows.iter().enumerate() {
            let Some(pattern) = usize::try_from(of_row.patterns[kind]).ok().and_then(|pattern| tables.patterns.get(pattern)) else { continue };
            if !pattern.iter().any(|piece| piece.piece > 0) {
                continue;
            }
            let mut looks = Looks::default();
            for (slot, piece) in pattern.iter().enumerate().filter(|(_, piece)| piece.piece >= 0) {
                let [x, y, z, heading] = piece.numbers;
                looks.pieces[slot] = super::script::PieceLook { resource: Some(piece.piece as usize), drawn: slot as u32 | drawn, place: Vec3::new(x, y, z), heading };
            }
            scenery.push(Scenery { place: Vec3::new(0.0, 0.0, row as f32 * ROW + ROW / 2.0), sight: 6000.0, kind: kind as i32, looks });
        }
    }
    scenery
}

/// The engine functions, for the script that runs.
struct Functions<'a> {
    ride: &'a mut Ride,
    who: Who,
    work: &'a mut Work,
}

fn vector(call: &Call, address: u32) -> Result<Vec3> {
    let number = |at: u32| call.machine().read_u32(address + at).map(f32::from_bits);
    Ok(Vec3::new(number(0)?, number(4)?, number(8)?))
}

fn answer(call: &Call, address: u32, vector: Vec3) {
    if address != 0 {
        let mut bytes = [0; 12];
        for (axis, value) in vector.to_array().into_iter().enumerate() {
            bytes[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        call.write(address, &bytes);
    }
}

/// How `n1b`, `n1c` and `n1d` make the bit they set of how a thing is drawn.
fn drawn(how: i32) -> u32 {
    match how {
        0 => 0x200_0000,
        1 => 0x800_0000,
        _ => 0,
    }
}

impl Host for Functions<'_> {
    fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
        // The scripts and the game work on one copy of the course's numbers: the
        // script's is taken as it stands, and given back as the game leaves it.
        let before = call.machine().mapped(SHARED_BASE).map(<[u8]>::to_vec).unwrap_or_default();
        self.ride.shared.clone_from(&before);
        let result = self.function(number, call);
        if self.ride.shared != before {
            call.write(SHARED_BASE, &self.ride.shared);
        }
        if let Some(running) = self.ride.running.last_mut() {
            running.2 = !self.work.no_breaking_in;
        }
        // An entry of this script was begun from within it: it stops here.
        if self.ride.pending.iter().any(|(pending, ..)| *pending == self.who) {
            call.wait();
        }
        result
    }
}

impl Functions<'_> {
    fn character(&mut self, number: i32) -> Option<&mut Character> {
        self.ride.characters.iter_mut().find(|character| character.number == number)
    }

    fn looks(&mut self, number: i32) -> Option<&mut Looks> {
        let index = self.ride.index(number)?;
        Some(&mut self.ride.riders[index].looks)
    }

    /// Waits until a count of the interface's frames has come to `frames`.
    fn count(&mut self, call: &Call, frames: i32, by: i32) {
        if !call.resumed() {
            self.work.count = 0;
            if frames > 0 {
                call.wait();
            }
        } else {
            self.work.count += by;
            if self.work.count < frames {
                call.wait();
            }
        }
    }

    fn function(&mut self, number: u32, call: &Call) -> Result<u32> {
        let int = |index: u32| call.int(index);
        let float = |index: u32| call.float(index);
        let yes = |yes: bool| Ok(yes as u32);
        let is_camera = self.who == Who::Camera && !self.ride.camera_held;
        match number {
            // Traces and what does nothing.
            0x00 | 0x02 | 0x16 => {}
            0x01 => {
                if int(0)? == 0 {
                    self.ride.hurt = int(1)? != 0;
                }
            }
            0x03 => match int(0)? {
                0 => self.ride.end(Outcome::Through),
                1 => self.ride.end(Outcome::Failed),
                _ => {}
            },
            0x04 => {
                let (lane, from_end, steps) = (int(0)?, int(1)?, int(2)?.max(0));
                let chip = usize::try_from(lane).ok().filter(|_| (0..2).contains(&from_end)).and_then(|lane| self.ride.road.chip_of(lane, from_end == 1, steps as usize));
                return Ok(chip.map_or(-1, |chip| chip as i32) as u32);
            }
            0x05 => self.ride.traps.insert(0, Trap::new(int(0)?)),
            0x06 => {
                let number = int(0)?;
                self.ride.traps.retain(|trap| trap.number != number);
            }
            0x07 => {
                let (number, what, value) = (int(0)?, int(1)?, int(2)?);
                if let Some(trap) = self.ride.traps.iter_mut().find(|trap| trap.number == number) {
                    match what {
                        0 => trap.watches = value,
                        1 => trap.tells = value,
                        2 => trap.entry = value,
                        3 => trap.event = value,
                        4 => trap.with = value,
                        _ => {}
                    }
                }
            }
            0x08 => {
                if let Some(character) = self.character(int(0)?) {
                    character.ending = true;
                }
            }
            0x09 => {
                let (number, what, value) = (int(0)?, int(1)?, int(2)?);
                let Some(index) = self.ride.index(number) else { return Ok(0) };
                let (character, looks) = (&mut self.ride.characters[index], &mut self.ride.riders[index].looks);
                match what {
                    0 => character.rides = value,
                    1 => looks.models.iter_mut().filter(|model| model.resource.is_some()).for_each(|model| model.animation = value),
                    2 => looks.models.iter_mut().filter(|model| model.resource.is_some()).for_each(|model| model.looped = value != 0),
                    3 => character.word = value,
                    4 => {
                        if let Some(chip) = usize::try_from(value).ok().filter(|&chip| chip < self.ride.road.chips.len()) {
                            character.put(&self.ride.road, chip);
                        }
                    }
                    5 => character.set(flag::TAKES_NOTHING, value != 0),
                    6 => character.left = value,
                    7 => character.could = value,
                    8 => character.set(flag::MAY_TURN, value != 0),
                    9 if (0..2).contains(&value) => character.mover.even = value == 1,
                    10 => looks.shadow[2] = value,
                    11 => looks.shadow[0] = value,
                    12 => looks.shadow[1] = value,
                    13 => character.set(flag::CAN_BE_STRUCK, value != 0),
                    _ => {}
                }
            }
            0x0a => {
                let (number, what, value) = (int(0)?, int(1)?, float(2)?);
                let Some(index) = self.ride.index(number) else { return Ok(0) };
                let (character, looks) = (&mut self.ride.characters[index], &mut self.ride.riders[index].looks);
                match what {
                    0 => character.set_speed(value),
                    1 => character.set_part(&self.ride.road, value),
                    2 => looks.blend = value,
                    3 => character.mover.length = value.max(0.0),
                    4 => character.mover.wait = value.max(0.0),
                    5 => character.untouchable = value,
                    _ => {}
                }
            }
            0x0b => {
                let (number, model) = (int(0)?, int(1)?);
                let plays = self.looks(number).is_some_and(|looks| match usize::try_from(model) {
                    Ok(model) => looks.models.get(model).is_some_and(|model| model.plays()),
                    Err(_) => looks.models.iter().any(|model| model.plays()),
                });
                if plays {
                    call.wait();
                }
            }
            0x0c => {
                let (number, what) = (int(0)?, int(1)?);
                let Some(index) = self.ride.index(number) else { return Ok(0) };
                match what {
                    0 => {
                        let lengths = &self.ride.lengths;
                        self.ride.riders[index].looks.begin(|model, animation| lengths.get(model).and_then(|lengths| lengths.get(animation as usize)).copied().unwrap_or(0.0));
                    }
                    1 | 2 => {
                        self.ride.characters[index].change_lane(&self.ride.road, what == 2);
                    }
                    _ => {}
                }
            }
            0x0d => {
                let frames = int(0)?;
                if !call.resumed() {
                    self.ride.counted = 0;
                    if frames > 0 {
                        call.wait();
                    }
                } else {
                    self.ride.counted += STEPS;
                    if self.ride.counted < frames {
                        call.wait();
                    }
                }
            }
            0x0e => {
                // The fade's frames are the interface's.
                let (kind, frames) = (int(0)?, int(1)? / STEPS);
                let covered = self.ride.fade.covered();
                match kind {
                    0 if covered => self.ride.fade.begin(fade_kind::IN, frames),
                    1 if !covered => self.ride.fade.begin(fade_kind::OUT, frames),
                    2 if covered => self.ride.fade.begin(fade_kind::IN_WHITE, frames),
                    3 if !covered => self.ride.fade.begin(fade_kind::OUT_WHITE, frames),
                    4 => self.ride.display.fade_out(int(1)?),
                    _ => {}
                }
            }
            0x0f => {
                if self.ride.fade.running() || self.ride.display.fading() {
                    call.wait();
                }
            }
            0x10 => {
                let (number, what) = (int(0)?, int(1)?);
                let value = self.character(number).map_or(0.0, |character| match what {
                    0 => character.mover.from.speed,
                    1 => character.mover.from.part,
                    3 => character.mover.length,
                    5 => character.untouchable,
                    _ => 0.0,
                });
                return Ok(value.to_bits());
            }
            0x11 => {
                let (number, what) = (int(0)?, int(1)?);
                let value = self.character(number).map_or(0, |character| match what {
                    0 => character.rides,
                    4 => character.chip().map_or(0, |chip| chip as i32),
                    5 => character.has(flag::TAKES_NOTHING) as i32,
                    6 => character.left,
                    7 => character.could,
                    8 => character.has(flag::MAY_TURN) as i32,
                    13 => character.has(flag::CAN_BE_STRUCK) as i32,
                    _ => 0,
                });
                return Ok(value as u32);
            }
            0x12 => {
                let (chip, what) = (int(0)?, int(1)?);
                let road = &self.ride.road;
                let Some(chip) = usize::try_from(chip).ok().and_then(|chip| road.chips.get(chip)) else { return Ok(0) };
                let beside = |beside: Option<usize>| beside.map_or(-1, |beside| beside as i32);
                let value = match what {
                    1 => chip.lane as i32,
                    2 => chip.row as i32,
                    3 => beside(chip.left),
                    4 => beside(chip.right),
                    5 => chip.marks as i32,
                    _ => 0,
                };
                return Ok(value as u32);
            }
            0x13 => {
                if self.character(int(0)?).is_some_and(|character| character.mover.changing) {
                    call.wait();
                }
            }
            0x14 => {
                return Ok(match int(0)? {
                    1 => {
                        self.ride.next_number += 1;
                        self.ride.next_number - 1
                    }
                    2 => self.ride.new_trap_number(),
                    _ => 0,
                } as u32)
            }
            0x15 if is_camera => {
                let bits = int(0)?;
                for (bit, asked) in [(1, 2), (2, 1), (4, 6), (8, 4), (0x10, 5), (0x20, 7), (0x40, 3)] {
                    if bits & bit != 0 {
                        let ride = &mut *self.ride;
                        let mut random = ride.random;
                        ride.camera.apply(asked, |below| chance(&mut random, below));
                        ride.random = random;
                    }
                }
            }
            0x17 => {
                let value = float(1)?;
                return Ok(match int(0)? {
                    0 => value.sin(),
                    1 => value.cos(),
                    2 => value.sqrt(),
                    _ => 0.0,
                }
                .to_bits());
            }
            0x18 => {
                let (number, slot, resource) = (int(0)?, int(1)?, int(2)?);
                if let (Some(looks), true, Ok(resource)) = (self.looks(number), (0..2).contains(&slot), usize::try_from(resource)) {
                    looks.models[slot as usize] = super::script::ModelLook { resource: Some(resource), ..Default::default() };
                }
            }
            0x19 => {
                let (number, slot, resource) = (int(0)?, int(1)?, int(2)?);
                if let (Some(looks), true, Ok(resource)) = (self.looks(number), (0..5).contains(&slot), usize::try_from(resource)) {
                    looks.pieces[slot as usize].resource = Some(resource);
                }
            }
            0x1a => {
                let (number, slot, resource) = (int(0)?, int(1)?, int(2)?);
                if let (Some(looks), true, Ok(resource)) = (self.looks(number), (0..20).contains(&slot), usize::try_from(resource)) {
                    looks.put_effect(slot as usize, resource);
                }
            }
            0x1b => {
                let (number, model, what, value) = (int(0)?, int(1)?, int(2)?, int(3)?);
                let Some(model) = self.looks(number).and_then(|looks| looks.models.get_mut(model as usize)).filter(|model| model.resource.is_some()) else { return Ok(0) };
                match what {
                    0 => model.drawn = drawn(value),
                    1 => model.scale = value as f32 / 100.0,
                    2 => model.speed = (value as f32 / 100.0).max(0.0),
                    3 => model.flag = value != 0,
                    4 => model.at = model.length * (value as f32 / 100.0).clamp(0.0, 1.0),
                    _ => {}
                }
            }
            0x1c => {
                let (number, piece, what, value) = (int(0)?, int(1)?, int(2)?, int(3)?);
                if let Some(piece) = self.looks(number).and_then(|looks| looks.pieces.get_mut(piece as usize)).filter(|piece| piece.resource.is_some() && what == 0) {
                    piece.drawn = drawn(value);
                }
            }
            0x1d => {
                let (number, effect, what, value) = (int(0)?, int(1)?, int(2)?, int(3)?);
                let Some(effect) = self.looks(number).and_then(|looks| looks.effects.get_mut(effect as usize)).and_then(Option::as_mut) else { return Ok(0) };
                match what {
                    0 => effect.word = value,
                    1 => effect.bone = value,
                    2 => effect.follows = value != 0,
                    3 => effect.model = value,
                    4 => effect.own_time = value != 0,
                    5 => effect.flag = value != 0,
                    6 => effect.held = value != 0,
                    _ => {}
                }
            }
            0x1e => {
                let (number, slot) = (int(0)?, int(1)?);
                if let Some(model) = self.looks(number).and_then(|looks| looks.models.get_mut(slot as usize)) {
                    *model = Default::default();
                }
            }
            0x1f => {
                let (number, slot) = (int(0)?, int(1)?);
                if let Some(piece) = self.looks(number).and_then(|looks| looks.pieces.get_mut(slot as usize)) {
                    *piece = Default::default();
                }
            }
            0x20 => {
                let (number, slot) = (int(0)?, int(1)?);
                if let Some(effect) = self.looks(number).and_then(|looks| looks.effects.get_mut(slot as usize)) {
                    *effect = None;
                }
            }
            0x21 => {
                let (number, slot, part, what, shown) = (int(0)?, int(1)?, int(2)?, int(3)?, int(4)?);
                if let Some(model) = self.looks(number).and_then(|looks| looks.models.get_mut(slot as usize)).filter(|model| model.resource.is_some() && what == 0) {
                    model.parts.retain(|(other, _)| *other != part);
                    model.parts.push((part, shown));
                }
            }
            0x22 => {
                let (number, slot) = (int(0)?, int(1)?);
                if let (Some(index), true) = (self.ride.index(number), (0..3).contains(&slot)) {
                    self.ride.riders[index].scripts[slot as usize] = None;
                }
            }
            0x23 => {
                return Ok(match int(0)? {
                    0 => self.work.number,
                    1 => self.work.with,
                    2 => STEPS,
                    3 => self.work.event,
                    4 => self.work.came,
                    5 => OWN_BASE as i32,
                    _ => 0,
                } as u32)
            }
            0x24 => {
                let (number, slot, bone, what, degrees) = (int(0)?, int(1)?, int(2)?, int(3)?, int(4)?);
                if let Some(model) = self.looks(number).and_then(|looks| looks.models.get_mut(slot as usize)).filter(|model| model.resource.is_some() && (0..2).contains(&what)) {
                    // A bone has one turn each way: a new one takes the old one's place.
                    model.turned.retain(|(other, way, _)| (*other, *way) != (bone, what));
                    model.turned.push((bone, what, degrees as f32));
                }
            }
            0x25 => self.count(call, int(0)?, STEPS),
            0x26 => {
                // Sixtieths of a second of the game's time, in 1024ths of them.
                let by = (self.ride.time * 61440.0) as i32;
                self.count(call, int(0)? << 10, by);
            }
            0x27 => return Ok(0f32.to_bits()),
            0x28 => self.ride.display.says = Some(call.string(0)?),
            0x29 => {
                if self.ride.display.says.is_some() {
                    call.wait();
                }
            }
            0x2a => return Ok(if int(0)? == 0 { self.ride.time } else { 0.0 }.to_bits()),
            0x2b => {
                let (number, index, what, value) = (int(0)?, int(1)?, int(2)?, int(3)?);
                let Some(hit_box) = self.character(number).and_then(|character| usize::try_from(index).ok().and_then(|index| character.boxes.get_mut(index))) else { return Ok(0) };
                set_box(hit_box, what, value);
            }
            0x2c => return yes(self.ride.index(int(0)?).is_some()),
            0x2d => {
                let (who, entry, event, with) = (int(0)?, int(1)?, int(2)?, int(3)?);
                match who {
                    0 => self.ride.begin(Who::System, entry, event, with),
                    1 => self.ride.begin(Who::Course, entry, event, with),
                    3 => self.ride.begin(Who::Camera, entry, event, with),
                    _ => {}
                }
            }
            0x2e => match (int(0)?, int(1)?) {
                (0, sound) => self.ride.sounds.push(SoundAsked::Common(sound)),
                (1, sound) => self.ride.play_sound(sound, 0x7f),
                _ => {}
            },
            0x2f if is_camera => {
                const WORDS: [usize; 14] = [0, 1, 2, 3, 8, 9, 10, 11, 12, 13, 4, 5, 6, 7];
                if let Some(&word) = WORDS.get(int(0)? as usize) {
                    self.ride.camera.set_word(word, int(1)?);
                }
            }
            0x30 if is_camera => {
                const NUMBERS: [usize; 21] = [0, 1, 2, 3, 4, 5, 0x18, 0x19, 0x1d, 0x1e, 0x1f, 8, 9, 10, 12, 14, 16, 18, 20, 22, 23];
                if let Some(&number) = NUMBERS.get(int(0)? as usize) {
                    self.ride.camera.set_number(number, float(1)?);
                }
            }
            0x31 => {
                let by = float(0)?;
                let road = &self.ride.road;
                self.ride.characters.iter_mut().for_each(|character| character.shift(road, by));
                self.ride.camera.shift(by);
            }
            0x32 => {
                let (number, by) = (int(0)?, float(1)?);
                let Some(index) = self.ride.index(number) else { return Ok(0) };
                self.ride.characters[index].shift(&self.ride.road, by);
            }
            0x33 => return Ok(self.ride.random(int(0)?) as u32),
            0x34 => {
                if let Some(bit) = 1u32.checked_shl(int(0)? as u32) {
                    self.ride.talk.asked |= bit;
                }
            }
            0x35 => {
                if self.ride.busy_talking() {
                    call.wait();
                }
            }
            0x36 => return yes(self.ride.busy_talking()),
            0x37 => {
                let (number, kind, with) = (int(0)?, int(1)?, int(2)?);
                self.ride.make(number, kind.max(0) as usize, with);
            }
            0x38 => return Ok(SHARED_BASE + (self.ride.enemies_at + int(0)? as usize * ENEMY_RECORD) as u32),
            0x39 => return Ok(SHARED_BASE + (self.ride.gates_at + int(0)? as usize * TRAP_RECORD) as u32),
            0x3a => {
                let (number, what, from) = (int(0)?, int(1)?, int(2)? as u32);
                if from == 0 {
                    return Ok(0);
                }
                let value = vector(call, from)?;
                let Some(character) = self.character(number) else { return Ok(0) };
                match what {
                    3 => character.on_road = value,
                    4 => character.added = value,
                    5 => character.adding = value,
                    6 => character.road_turns = value,
                    7 => turns_towards(value, &mut character.road_turns),
                    8 => character.added_turns = value,
                    9 => turns_towards(value, &mut character.added_turns),
                    _ => {}
                }
            }
            0x3b => {
                let (number, what, to) = (int(0)?, int(1)?, int(2)? as u32);
                let Some(character) = self.character(number) else { return Ok(0) };
                let value = match what {
                    0 => character.place,
                    2 => character.going,
                    3 => character.on_road,
                    4 => character.added,
                    5 => character.adding,
                    6 => character.road_turns,
                    7 => turn(character.road_turns) * Vec3::Z,
                    8 => character.added_turns,
                    9 => turn(character.added_turns) * Vec3::Z,
                    _ => return Ok(0),
                };
                answer(call, to, value);
            }
            0x3c => {
                let (chip, part, what, to) = (int(0)?, float(1)?, int(2)?, int(3)? as u32);
                if let Some(chip) = usize::try_from(chip).ok().filter(|&chip| chip < self.ride.road.chips.len()) {
                    match what {
                        0 => answer(call, to, self.ride.road.at(chip, part)),
                        // Every chip faces straight on.
                        1 => answer(call, to, Vec3::ZERO),
                        _ => {}
                    }
                }
            }
            0x3d => {
                // Where a bone is: the port has no bones here yet, and answers
                // where the character is.
                let (number, what, to) = (int(0)?, int(3)?, int(4)? as u32);
                if let (Some(place), 0) = (self.character(number).map(|character| character.place), what) {
                    answer(call, to, place);
                }
            }
            0x3e => {
                let (to, from) = (int(0)? as u32, int(1)? as u32);
                if to != 0 && from != 0 {
                    answer(call, to, vector(call, from)? * float(2)?);
                }
            }
            0x3f => {
                let (to, from) = (int(0)? as u32, int(1)? as u32);
                if to != 0 && from != 0 {
                    answer(call, to, vector(call, from)?.normalize_or_zero());
                }
            }
            0x40 | 0x41 => {
                let (to, one, other) = (int(0)? as u32, int(1)? as u32, int(2)? as u32);
                if to != 0 && one != 0 && other != 0 {
                    let (one, other) = (vector(call, one)?, vector(call, other)?);
                    answer(call, to, if number == 0x40 { one + other } else { one - other });
                }
            }
            0x42 => {
                let (to, from) = (int(0)? as u32, int(1)? as u32);
                let turns = Vec3::new(float(2)?, float(3)?, float(4)?);
                answer(call, to, turn(turns) * vector(call, from)?);
            }
            0x43 => {
                let (number, slot) = (int(0)?, int(1)?);
                return yes(self.looks(number).is_some_and(|looks| looks.effects.get(slot as usize).is_some_and(Option::is_some)));
            }
            0x44 | 0x45 => {
                let (number_of, slot, what, at) = (int(0)?, int(1)?, int(2)?, int(3)? as u32);
                let given = if number == 0x44 && at != 0 { Some(vector(call, at)?) } else { None };
                let Some(effect) = self.looks(number_of).and_then(|looks| looks.effects.get_mut(slot as usize)).and_then(Option::as_mut) else { return Ok(0) };
                let field = match what {
                    0 => &mut effect.place,
                    1 => &mut effect.turns,
                    _ => return Ok(0),
                };
                match given {
                    Some(given) => *field = given,
                    None if number == 0x45 => answer(call, at, *field),
                    None => {}
                }
            }
            0x46 => {
                let (number, slot, with, script) = (int(0)?, int(1)?, int(2)?, int(3)?);
                let script = self.ride.scripts.get(script as usize).cloned().flatten();
                if let (Some(index), Some(script), true) = (self.ride.index(number), script, (0..3).contains(&slot)) {
                    let (who, slot) = (Who::Of(number, slot as usize), slot as usize);
                    if self.ride.running.iter().any(|(running, ..)| *running == who) {
                        bail!("a script gives its own slot another script");
                    }
                    self.ride.riders[index].scripts[slot] = Some(Box::new(Instance::new(&script, true, number, with)));
                    self.ride.begin(who, entry::INIT, 0, 0);
                }
            }
            0x47 => {
                let (number, slot, entry, event, with) = (int(0)?, int(1)?, int(2)?, int(3)?, int(4)?);
                if (0..3).contains(&slot) {
                    self.ride.begin(Who::Of(number, slot as usize), entry, event, with);
                }
            }
            0x48 => return Ok(SHARED_BASE),
            0x49 => self.work.no_breaking_in = int(0)? != 0,
            0x4a if is_camera => {
                const THREES: [[usize; 3]; 4] = [[0, 1, 2], [3, 4, 5], [10, 14, 12], [16, 20, 18]];
                let (what, from) = (int(0)?, int(1)? as u32);
                if let (Some(numbers), true) = (THREES.get(what as usize), from != 0) {
                    let value = vector(call, from)?;
                    for (axis, &number) in numbers.iter().enumerate() {
                        self.ride.camera.set_number(number, value[axis]);
                    }
                }
            }
            0x4b => {
                if int(0)? == 1 {
                    self.ride.stop_sound(int(1)?);
                }
            }
            0x4c => return yes(int(0)? == 1 && usize::try_from(int(1)?).ok().and_then(|slot| self.ride.playing.get(slot)).copied().unwrap_or(false)),
            0x4d if is_camera => {
                let (model, animation) = (int(0)?, int(1)?);
                self.ride.camera_motions += 1;
                self.ride.camera_motion = usize::try_from(model).ok().map(|model| CameraMotion {
                    model,
                    animation,
                    at: 0.0,
                    length: self.ride.length_of(model, animation),
                    begun: self.ride.camera_motions,
                });
            }
            0x4e => {
                if self.ride.camera_motion.is_some_and(|motion| motion.plays()) {
                    call.wait();
                }
            }
            0x4f => {
                let (pad, bits) = (int(0)?, int(1)? as u32);
                return Ok(if pad == 0 { self.ride.pressed & bits } else { 0 });
            }
            0x50 => return yes(self.ride.camera_motion.is_some_and(|motion| motion.plays())),
            0x51 => self.ride.sounds.push(SoundAsked::Voice(int(0)?)),
            0x52 => self.ride.sounds.push(SoundAsked::MusicOut(int(0)?)),
            0x53 => self.ride.sounds.push(SoundAsked::Music(int(0)?)),
            // The camera's functions, asked of what is not the camera or while
            // the camera is held, do nothing.
            0x15 | 0x2f | 0x30 | 0x4a | 0x4d => {}
            _ => bail!("the ride has no function {number:#x}"),
        }
        Ok(0)
    }

}

/// Sets a thing of a box (`n2b`, 0x1463D8).
fn set_box(hit_box: &mut HitBox, what: i32, value: i32) {
    let number = value as f32;
    match what {
        0 => hit_box.kinds = if value & 3 != 0 { BLOW } else { 0 } | if value & 4 != 0 { NEAR } else { 0 },
        1 => hit_box.shape = value.clamp(0, 2),
        2 => hit_box.middle.x = number,
        3 => hit_box.middle.y = number,
        4 => hit_box.middle.z = number,
        5 => hit_box.radius = number.max(0.0),
        6 => hit_box.size.x = number.max(0.0),
        7 => hit_box.size.y = number.max(0.0),
        8 => hit_box.size.z = number.max(0.0),
        9 => hit_box.bone = value,
        10 => hit_box.on = value != 0,
        11 => hit_box.worth = value,
        12 => hit_box.says = value,
        13 => hit_box.model = value.clamp(0, 1),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bike::tables::state::*;

    fn normal() -> Option<Ride> {
        let data = tables::tests::data()?;
        Some(Ride::new(&data, "normal").unwrap())
    }

    #[test]
    fn a_ride_is_made_with_her_on_the_road_and_waits_for_its_start() {
        let Some(mut ride) = normal() else { return };
        // The course's script has set the goal and the start, and put her there.
        let state = ride.state();
        assert_eq!((state.word(START_CHIP), state.word(GOAL_CHIP), state.number(TIME_LEFT)), (12, 77 * 5 + 2, 240.0));
        assert!(state.has(GOAL) && state.has(CAN_DIE) && !state.has(PAD));
        let she = ride.her().unwrap().clone();
        assert_eq!((she.chip(), she.left, she.could, she.rides), (Some(12), 1000, 1000, 1));
        // Her script has given her two models, her two balls and a second script.
        let looks = &ride.riders[ride.index(0).unwrap()];
        assert_eq!(looks.looks.models.clone().map(|model| model.resource), [Some(0), Some(1)]);
        assert!(looks.scripts[0].is_some() && looks.scripts[1].is_some());
        assert_eq!((she.boxes[0].kinds, she.boxes[0].radius, she.boxes[1].kinds, she.boxes[1].radius), (BLOW, 40.0, NEAR, 70.0));
        // The screen comes up from black.
        assert!(!ride.fade.covered() && ride.fade.running());
        // The camera rides along on the sixth motion of its own model until the
        // start, which is a few seconds long.
        for _ in 0..3 {
            ride.frame(Asked::default(), 0);
        }
        let motion = ride.camera_motion.expect("the camera has no motion");
        assert_eq!((motion.model, motion.animation), (11, 5));
        assert!(motion.length > 1.0 && motion.plays(), "{motion:?}");
    }

    #[test]
    fn she_rides_from_the_start_through_the_first_gate_and_the_pad_then_changes_her_lane() {
        let Some(mut ride) = normal() else { return };
        // Held until the start's gate, a row on, she goes at the first gear's speed.
        for _ in 0..30 {
            ride.frame(Asked::default(), 0);
        }
        let she = ride.her().unwrap();
        assert!((she.mover.from.speed - 80.0).abs() < 1e-3 && she.place.z > 20000.0 && she.place.z < 30000.0, "{:?}", she.place);
        assert!(!ride.state().has(PAD));
        for _ in 0..150 {
            ride.frame(Asked::default(), 0);
        }
        // Through the gate at row 3 the ride has begun: the pad is heeded.
        assert!(ride.her().unwrap().place.z > 30000.0);
        assert!(ride.state().has(PAD) && ride.state().has(PAD_CHANGES_LANES) && ride.state().has(TIME_RUNS));
        assert!(ride.traps.iter().any(|trap| trap.gone_off));
        // Left, held: her script changes her lane, once, and she is in lane 1.
        for _ in 0..40 {
            ride.frame(Asked { left: true, ..Asked::default() }, 0);
        }
        let she = ride.her().unwrap();
        assert_eq!(she.chip().map(|chip| ride.road.chips[chip].lane), Some(1), "{:?}", she.place);
        assert!((she.place.x - 250.0).abs() < 1.0);
        // What is met has come on to the road by now.
        assert!(ride.characters.len() > 1, "{} on the road", ride.characters.len());
    }

    /// Rides a course with the pad worked by a rule of thumb: away from
    /// whatever is ahead in her lane. Talk is taken as lasting four seconds.
    fn ridden(course: &str, steers: bool, sturdy: bool) -> Option<Ride> {
        let data = tables::tests::data()?;
        let mut ride = Ride::new(&data, course).unwrap();
        let mut talking = 0;
        for frame in 0..30 * 600 {
            if sturdy {
                ride.unharmed();
            }
            let asked = if steers { ride.rule_of_thumb() } else { Asked::default() };
            ride.frame(asked, 0);
            if ride.talk.going.is_some() {
                talking += 1;
                if talking >= 120 {
                    talking = 0;
                    ride.talk_over();
                }
            }
            if frame % 300 == 0 || ride.over() {
                let time_left = ride.state().number(TIME_LEFT);
                let she = ride.her().unwrap();
                eprintln!(
                    "{course} {:5.1} s: row {:5.2} lane {:?} at {:3.0} km/h, {} left, row of {}, {} on the road, {:.0} s left",
                    frame as f32 / 30.0,
                    she.place.z / ROW,
                    she.chip().map(|chip| ride.road.chips[chip].lane),
                    she.mover.from.speed,
                    she.left,
                    ride.rules.row,
                    ride.characters.len(),
                    time_left,
                );
            }
            if ride.over() {
                break;
            }
        }
        Some(ride)
    }

    #[test]
    fn the_storys_course_is_ridden_to_an_end_with_nothing_going_wrong() {
        let Some(ride) = ridden("normal", true, false) else { return };
        assert_eq!(ride.faults, Vec::<String>::new());
        assert!(ride.over(), "the ride did not end");
        eprintln!("ended {:?}", ride.outcome);
        // One whom nothing wears down comes to the goal, within the time.
        let ride = ridden("normal", true, true).unwrap();
        assert_eq!((ride.faults.clone(), ride.outcome), (Vec::new(), Some(Outcome::Through)));
        assert!(ride.her().unwrap().place.z >= 770000.0);
    }

    #[test]
    fn every_course_runs_its_scripts_with_nothing_going_wrong() {
        let Some(data) = tables::tests::data() else { return };
        let names: Vec<String> = SystemTables::read(&tables::tests::script(&data, SYSTEM_PACK, SYSTEM_TABLES)).unwrap().stages.into_iter().map(|stage| stage.name).collect();
        for name in names {
            let ride = ridden(&name, false, false).unwrap();
            assert_eq!(ride.faults, Vec::<String>::new(), "{name}");
            eprintln!("{name}: ended {:?}", ride.outcome);
        }
    }
}
