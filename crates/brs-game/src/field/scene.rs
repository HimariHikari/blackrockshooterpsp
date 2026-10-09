//! What a map's script sets up and the field then plays with: the characters, the
//! traps, and what has been asked of the game (a battle, a fade). Plain data, so that
//! the scripts can be run and tested without the rest of the game. See
//! `docs/FIELD_NOTES.md`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use bevy::prelude::*;

use super::action::ActionSign;
use super::camera::{self, Camera, TrapCamera};
use super::cleared::Cleared;
use super::comm::Comm;
use super::failed::Failed;
use super::frames::{turn_of, Frames, FRAMES};
use super::mover::{wrap, Mover};
use super::place::{PlaceName, Title};
use super::scenery::{Lights, LIGHTS};
use super::staging::{Captions, Cut, Staging};
use super::window::{Picture, Window};
use crate::sound::{Request, COMMON_BANK, FULL_VOLUME, MIDDLE, NOTE};

/// The number scripts know the player by.
pub const PLAYER: i32 = 0;

/// What a character's time advances by in a frame at full pace: its timers count in
/// these units, and its pace is how many of them a frame is worth to it.
pub const FULL_PACE: i32 = 0x800;

/// A pace given in percent, in time units a frame (0x10821C).
pub fn pace(percent: i32) -> i32 {
    percent * 0xf000 / 3000
}

/// A character's flags, by the original's numbers (bits of the words at character +
/// 0x44, set with 0x106FFC and read with 0x10703C).
pub mod flag {
    /// Its model is loaded and it can take part.
    pub const READY: u32 = 0;
    /// What `s53` sets of the character it makes or begins again (0xE3844);
    /// what reads it has not been read.
    pub const OF_EFFECT: u32 = 3;
    /// Out of sight and out of play.
    pub const HIDDEN: u32 = 4;
    /// It is carrying out an action (a walk, a turn).
    pub const ACTING: u32 = 0x10;
    /// It stays where it is put, however far from the ground that is.
    pub const NO_DROP: u32 = 0x18;
    /// A behaviour script runs it.
    pub const BEHAVES: u32 = 0x24;
    /// It is an enemy symbol: touching it is an encounter.
    pub const SYMBOL: u32 = 0x27;
    /// Its steps make no sound (0xFC9A0).
    pub const NO_STEPS: u32 = 0x28;
    /// Three marks by which scripts show and hide characters all at once
    /// (`s210`, 0xE56D0).
    pub const MARKED_SECOND: u32 = 0x4d;
    pub const MARKED_FIRST: u32 = 0x4e;
    pub const MARKED_THIRD: u32 = 0x4f;
    /// When what it is doing ends it stays as it is, rather than going back to
    /// standing.
    pub const KEEPS_POSE: u32 = 0x53;
    /// It has its frame and is drawn however far it is from the player and
    /// however near the camera.
    pub const ALWAYS_ACTIVE: u32 = 0x57;
    /// It has no shadow (0x1067F8).
    pub const NO_SHADOW: u32 = 0x61;
    /// Its gimmicks are off (0xFD284, 0x10BEBC): what is layered on its body's
    /// animation is neither gone on with nor put on its model (0x10BC78,
    /// 0x10BD58), and its chains hang as they are (0x10BE04).
    pub const NO_LAYERS: u32 = 0x9d;
    /// Its time stands still (0x10827C; setting 24).
    pub const STOPPED: u32 = 0x1b;
    /// It does not idle when it has stood a while (0xF9030; setting 78 at
    /// nought).
    pub const NO_IDLING: u32 = 0x62;
    /// The tile with the chevron stands by its mark (0x1074A4, 0x46CD0): it is
    /// the one a mission is after.
    pub const POINTED_OUT: u32 = 0xa5;
    /// It is drawn in an order a script gave it among the characters, not by
    /// how far it is (0x1169B4; its + 0x4FC is the order).
    pub const ORDERED: u32 = 0xa6;
    /// The effects it carries are drawn by the lens that sees from one unit
    /// off (0xE7568 with the camera's + 0x150; 0x102FD4, 0x103074), not the
    /// scene's own. Kept; the port draws them by the scene's.
    pub const CLOSE_LENS: u32 = 0xa7;
    /// The weapon it carries is not shown (setting 76 off).
    pub const NO_WEAPON: u32 = 0x5f;
    /// It is after the player.
    pub const CHASING: u32 = 0x60;
    /// It never idles, however long it stands (0xF9030).
    pub const NEVER_IDLES: u32 = 0x62;
    /// Its patrol is waiting at a point of its route.
    pub const ROUTE_PAUSED: u32 = 0x67;
    /// Its mouth stops of itself when what is spoken ends, and it then goes
    /// back to standing (the two bits `s160` is given, 0xFE678).
    pub const MOUTH_WITH_VOICE: u32 = 0x6a;
    pub const STANDS_AFTER_VOICE: u32 = 0x6b;
    /// Of one a trap is taking from place to place (0x109AC8): it has flown to
    /// where it now leaps from, and another place follows the one it is on its
    /// way to.
    pub const FROM_FLIGHT: u32 = 0x72;
    pub const TAKEN_ON: u32 = 0x73;
    /// Touching it calls the map's script.
    pub const TOUCHABLE: u32 = 0x78;
    /// It is counting down to hiding itself (the hide timer).
    pub const HIDES_SOON: u32 = 0x7a;
    /// The first of the flags that show the switchable parts of its model, one for
    /// each entry of the field's list of them (`lst_mesh`): a part is hidden until
    /// its flag is set (0xFE2EC).
    pub const PART: u32 = 0x7b;
    // Two more are only ever read by the behaviour scripts: 0x9C, it comes into
    // being with its "appear" action, and 0x9E, where a chase leaves it becomes its
    // home.
    /// Its walk is a dash: straight ahead, with no stopping at a target.
    pub const DASHING: u32 = 0x9f;
    /// It has burst, of one that is being destroyed (0xF9954).
    pub const BURST: u32 = 0xa8;
    /// It cannot notice the player for a while (the unaware timer counts it down).
    pub const UNAWARE: u32 = 0xa0;
    /// It chases at a walk only, without the dashes.
    pub const NO_DASH: u32 = 0xa9;
    /// What it is doing may be left for the stick (0xF4864): she has pulled up
    /// for long enough.
    pub const FREE_TO_GO: u32 = 0xa1;

    pub const COUNT: u32 = 192;
}

/// Flags are kept as the original keeps them: six words of bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags([u32; 6]);

impl Flags {
    pub fn has(&self, flag: u32) -> bool {
        flag < flag::COUNT && self.0[(flag / 32) as usize] & (1 << (flag % 32)) != 0
    }

    pub fn set(&mut self, flag: u32, on: bool) {
        if flag < flag::COUNT {
            let (word, bit) = (&mut self.0[(flag / 32) as usize], 1 << (flag % 32));
            *word = if on { *word | bit } else { *word & !bit };
        }
    }
}

/// The statuses a character can be in (its int parameter 0x10). A character's script
/// gives a speed for each, and its animation follows it.
pub mod status {
    pub const WALK: i32 = 0;
    pub const RUN: i32 = 1;
    /// The status she is taken to a place in when she is not leaping there
    /// (0xF650C, 0xF6944); her script has it as fast as a run.
    pub const RUSH: i32 = 2;
    pub const STAND: i32 = 3;
    /// A leap (0xF83B4).
    pub const LEAP: i32 = 4;
    pub const TURN: i32 = 6;
    /// Pulling up after a run (0xF573C): its animation is number 12, played
    /// once (0xFDA30, 0xFDAE0), and its steps fall at other frames than a
    /// walk's.
    pub const BRAKE: i32 = 12;
    pub const DESTROYED: i32 = 0x11;
    /// What she moves in while the going is heavy (0x1054E8): her script has it
    /// at one unit a frame, and its animation is number 22 (0xFDA18).
    pub const TRUDGE: i32 = 0x41;
}

/// The flag of the game that says the going is heavy (0x102D; 0x1054D8, 0xFD9C8):
/// the third stage's storm sets it. She trudges then where she would run, and
/// whoever stands does so with animation 5 if it has it.
pub const HEAVY_GOING: i32 = 0x102d;
/// How long she must have run for a stop, or a turn right round, to be a pull-up
/// (0x105594, 0x105648: more than ten frames), and how long into one the stick
/// takes her out of it again (0xF57B4: more than thirteen).
pub const BRAKE_AFTER: i32 = 0x5000;
pub const BRAKE_HELD: i32 = 0x6800;

/// The frames a turn takes, whatever its size: `turn_frame` of the field's data.
pub const TURN_FRAMES: f32 = 34.0;
/// How long a character that is destroyed and fades takes to go (0xF9454).
pub const DESTROY_TIME: i32 = 0x1b800;
/// When one that does not fade bursts, and when it is gone (0xF9788, 0xF99A8).
pub const BURST_AT: i32 = 0xc800;
pub const BURST_GONE: i32 = 0xf000;
/// The way of being destroyed that fades (its script's `type_destroy`, 0xF96D8):
/// the boxes she can break have it. Every other bursts.
pub const DESTROY_FADES: u8 = 3;
/// How far below the ground one that hops up out of it starts (0xF6E50), and how
/// far above one that drops in (0xF7468); how long either takes (0xF7040); and
/// what it is thrown with, by a time of thirty frames and a length of a hundred
/// (0xF6F54, 0xF75C0, and the half of gravity at 0x16D4F0).
pub const POP_UP_DEPTH: f32 = 80.0;
pub const DROP_HEIGHT: f32 = 1000.0;
pub const HOP_TIME: i32 = 0x4800;
pub const POP_UP_SPEED: f32 = 4.137_66;
pub const DROP_SPEED: f32 = 31.862_3;
pub const HALF_GRAVITY: f32 = 4.903_325;
/// How long a character that appears stays below the ground, and how long it then
/// takes to come up (0xF72B4).
pub const APPEAR_WAIT: i32 = 0x5000;
pub const APPEAR_RISE: i32 = 0xf000;
/// A second, in the field's units of time.
pub const A_SECOND: i32 = 30 * FULL_PACE;
/// A leap (0xF7F04, 0xF5538). A place no more than so far above or below is on a
/// level with her, and the leap there takes as long as the way is at the speed
/// she runs at. One to a place further up takes as long as a way would with so
/// much more height to it, or, steeper than so many degrees, as long as that
/// part of its height alone. One to a place further down takes its fall and so
/// many seconds more. Of any but that last she goes on springing for one part in
/// so many before she is seen to come down. And she is thrown as things fall, by
/// so many metres a second squared, on top of the straight way there.
pub const LEAP_LEVEL: f32 = 100.0;
pub const LEAP_HIGHER: f32 = 500.0;
pub const LEAP_STEEP: f32 = 70.0;
pub const LEAP_STEEP_PART: f32 = 0.2;
pub const LEAP_DOWN_MORE: f32 = 0.5;
pub const LEAP_SPRING_PARTS: f32 = 5.0;
pub const GRAVITY: f32 = 9.806_65;
/// How much of that a leap from one of the field's frames into another is thrown
/// against (0xF5590: 0.980665).
pub const BETWEEN_FRAMES_GRAVITY: f32 = 0.1;
/// How near a place is near enough for one that rushes to it (0xF6718) and for
/// one that flies to it (0xF6B54).
pub const RUSH_REACH: f32 = 120.0;
pub const FLIGHT_REACH: f32 = 10.0;
/// How far up and down from where she lands the ground is looked for to stand
/// her on (0x1085E8), and how far above where she springs from or lands what
/// rises there is put (0xF8584).
pub const LANDING_REACH: f32 = 40.0;
pub const LEAP_EFFECT_UP: f32 = 3.0;
/// Her animations for being taken from place to place, by their numbers: she
/// gathers herself and springs (0xF8344: it goes round within itself once she is
/// off the ground), comes down (0xF8778), lands (0xF8974), rushes (0xF65D4) and
/// stops (0xF6838), and flies (0xF6A0C).
pub const MOTION_SPRING: i32 = 0x23;
pub const MOTION_COME_DOWN: i32 = 0x27;
pub const MOTION_LAND: i32 = 0x28;
pub const MOTION_RUSH: i32 = 0x21;
pub const MOTION_RUSH_STOP: i32 = 0x0c;
pub const MOTION_FLY: i32 = 0x1d;
/// The sounds of a leap, of the bank that is always there: as she leaves the
/// ground (0xF86F8), and with what rises where she springs from and where she
/// lands (0xF8688, 0xF895C).
pub const SOUND_LEAP: u32 = 0x2d;
pub const SOUND_LEAP_EFFECT: u32 = 0x39;

/// The ways a trap has her taken to a place of its, by the numbers a script
/// gives (`s234`, 0xC61D4, which keeps them as 1, 2, 0x10 and 0x15): a leap; a
/// leap that takes no heed of a flight before or after it; a rush with an aura
/// about her; and flying straight there with the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakenBy {
    Leap,
    PlainLeap,
    Rush,
    Flight,
}

impl TakenBy {
    pub fn of(number: i32) -> Option<TakenBy> {
        match number {
            0 => Some(TakenBy::Leap),
            1 => Some(TakenBy::PlainLeap),
            2 => Some(TakenBy::Rush),
            3 => Some(TakenBy::Flight),
            _ => None,
        }
    }
}

/// A place a trap has her taken to (a record of the trap's, 0xE5E08): somewhere
/// on the line between two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TakenTo {
    pub by: TakenBy,
    pub line: [Vec3; 2],
}

impl TakenTo {
    /// Where on its line one who is at `from` is taken (0x108DFC): as near as
    /// the line comes.
    pub fn nearest(&self, from: Vec3) -> Vec3 {
        let [first, second] = self.line;
        let along = second - first;
        let (way, length) = (along.normalize_or_zero(), along.length());
        let reached = (from - first).dot(way);
        if reached < 0.0 {
            first
        } else if reached > length {
            second
        } else {
            first + way * reached
        }
    }
}

/// The places a trap is taking her to, one after another, and which of them she
/// is on her way to (the trap at her + 0x504 and the count at + 0x500).
#[derive(Clone, Debug, PartialEq)]
pub struct Taken {
    pub places: Vec<TakenTo>,
    pub at: usize,
}

/// A leap, as its action keeps it (the block at the character's + 0x370).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leap {
    pub part: LeapPart,
    /// How long it has been at this part of it, save that the two parts in the
    /// air count the one time.
    pub time: i32,
    /// Where it leapt from, and the way it faced as it set about it, brought
    /// within half a turn of the way it is to face.
    pub from: Vec3,
    pub facing: f32,
    /// How long it is in the air, and for how much of that it goes on springing.
    pub length: i32,
    pub spring: i32,
    /// It leaps from one of the field's frames into another (the block's + 0x34,
    /// set where the leap begins while its + 0x4C1 names a frame): it is timed
    /// by how far it goes alone, springs for none of that, and is thrown a tenth
    /// as high.
    pub between: bool,
}

/// The parts of a leap (0xF8434): gathering itself, turning to the way it is to
/// face; in the air and still springing; in the air and coming down; landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeapPart {
    Gather,
    Spring,
    ComeDown,
    Land,
}

impl Leap {
    /// Where it is in the air, `time` into the way from where it sprang to
    /// `to` (0xF5538): so far along the straight way there, and as high above
    /// that as a thing thrown to come down when the way is done. `up` is its
    /// upright as its frame has that just then (0x1090E0).
    pub fn place(&self, to: Vec3, time: i32, up: Vec3) -> Vec3 {
        let (now, whole) = (time as f32 / A_SECOND as f32, self.length as f32 / A_SECOND as f32);
        let half = if self.between { GRAVITY * BETWEEN_FRAMES_GRAVITY } else { GRAVITY } * 0.5;
        let height = now * (half * whole) - half * now * now;
        self.from + (to - self.from) * (now / whole) + up * (height * 100.0)
    }
}

/// What a character is doing with its body, of the things a script can have it do
/// (the original's action at character + 0x330).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    /// Leaping to its target (action 1).
    Leap(Leap),
    /// Rushing to its target (action 0x10), and stopping after it (action 0x11)
    /// for as long as the animation it stops with plays.
    Rush,
    RushStop { time: i32 },
    /// Flying straight to its target (action 0x15).
    Flight,
    /// Walking to its target (action 5); a dash keeps going the way it faces.
    Walk,
    /// Turning to face its target (action 3), by this much a frame at full pace.
    Turn { rate: f32 },
    /// Coming up out of the ground to where it stands (action 20): how long it has
    /// been at it, and how far down it started.
    Appear { time: i32, depth: f32 },
    /// Hopping up out of the ground (action 18), and dropping in from above
    /// (action 19): how long it has been at it.
    PopUp { time: i32 },
    DropIn { time: i32 },
    /// Being destroyed (action 15): how long it has been at it.
    Destroy { time: i32 },
    /// Playing through an animation a script set playing (action 4): how long it
    /// has been at it, and how long that takes, if that is known yet.
    Motion { time: i32, end: Option<i32> },
    /// Pulling up after a run (action 10): how long she has been at it.
    Brake { time: i32 },
}

/// How long a character goes on with an animation whose length never comes to be
/// known, which is one its resource has not got.
pub const MOTION_UNKNOWN: i32 = 4 * FULL_PACE;

/// What an action of a character has be seen and heard, until the field takes it
/// (`field/effects.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Show {
    /// The ring on the ground it comes up through, where it is to stand
    /// (0xF71E0), and that ring going once it is up (0xF73A4).
    Ring { at: Vec3, facing: f32 },
    RingGoes,
    /// The smoke it hops out of the ground in (0xF6E30), or goes in (0xF9590).
    Smoke { at: Vec3, facing: f32 },
    /// What rises where it lands (0xF7754).
    Landing { at: Vec3, facing: f32 },
    /// The same where it springs from, half as large (0xF8558).
    Spring { at: Vec3, facing: f32 },
    /// The aura of one that rushes or flies: it comes about it and goes where
    /// it goes (0xF65E4, 0xE7DD8), and it goes out (0xF67D4), which one that
    /// flew has it do where it then is and no longer about it (0xF6CF4).
    Aura,
    AuraGoes { let_go: bool },
    /// A sound of the bank that is always there.
    Heard(u32),
    /// It bursts (0xF9778): the burst before it, the camera's shake and the sound.
    Burst,
}

impl Show {
    /// Where it is to be seen is turned about where the world begins: out of a
    /// frame in which the character acted, into the world.
    pub fn turn(&mut self, by: Quat) {
        if let Show::Ring { at, .. } | Show::Smoke { at, .. } | Show::Landing { at, .. } | Show::Spring { at, .. } = self {
            *at = by * *at;
        }
    }
}

/// One that goes over from one of the field's frames to another (its flag 0xA3,
/// 0x10930C): it lies between the two for so long (its + 0x4F0 of + 0x4F4), by
/// the turn from the one it is in now back to the one it was in (+ 0x4D0), and
/// faces what `ahead` is in the frame as it lies just then (+ 0x4E0, 0x107E34).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turning {
    pub from: Quat,
    /// The turn of the frame it is in now.
    pub to: Quat,
    pub time: i32,
    pub length: i32,
    pub ahead: Vec3,
}

impl Turning {
    /// How far over it is (0x109128): one less the square of what is left.
    pub fn part(&self) -> f32 {
        let left = (1.0 - self.time as f32 / self.length.max(1) as f32).max(0.0);
        1.0 - left * left
    }

    /// How it lies just now: so much of the way back to the frame it was in,
    /// and then as the one it is in lies.
    pub fn lay(&self) -> Quat {
        self.from.slerp(Quat::IDENTITY, self.part()) * self.to
    }
}

/// What a symbol's behaviour script keeps about it (the block that 0x1086EC returns).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mind {
    /// How it moves about when nothing is going on (int parameter 0): 0 stands, 1
    /// patrols its route, 2 wanders.
    pub movement: i32,
    /// Counts down to hiding it, while flag [`flag::HIDES_SOON`] is set (int parameter 6).
    pub hide_timer: i32,
    /// Counts down to its noticing things again (int parameter 7).
    pub unaware_timer: i32,
    /// The character it is after (int parameter 0x18).
    pub target: i32,
    /// Where it belongs: it wanders about this point and comes back to it.
    pub home: Vec3,
    /// How far away it notices the player (decimal parameter 0x2A).
    pub notice_range: f32,
    /// How far from home it wanders, and how long it has been on its way to the
    /// place it last picked, or stopping at a point of its route.
    pub wander_radius: f32,
    pub wander_time: i32,
    /// Its route (the block's + 0x8B, + 0x9C, + 0x120 and + 0x11C): up to
    /// sixteen of the map's points, by their numbers; how many of them there
    /// are; how long it stops at each, in the field's units of time (int
    /// parameter 0x2D); and whether it is on its way to one or stopping at one.
    pub route: [u8; ROUTE_POINTS],
    pub route_length: i32,
    pub route_pause: i32,
    pub on_its_way: bool,
    /// A battle has been fought and its script has yet to be told of it (its
    /// `int_battle_escape`, which the field's system script asks for after every
    /// battle).
    pub stand_down: bool,
}

/// How many points a route can have, and the number that is no point (0xF9A44).
pub const ROUTE_POINTS: usize = 16;
pub const NO_POINT: u8 = 0xff;

/// An animation a script has a character play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// One of the character's own, by its number (`s62`).
    Own(i32),
    /// One an event brings for a scene: the slot of the event's cast it is kept
    /// under, and its number there (`s63`, `s265`).
    Staged(usize, i32),
}

/// The animations of one that has stood a while (0xF8FE0): one it begins with, one
/// that goes round and one it ends with. A character has any of them or none; of
/// the field's own, fifteen of the people have the first and nobody the others.
pub mod idling {
    pub const BEGINS: i32 = 0x10;
    pub const GOES_ROUND: i32 = 0x11;
    pub const ENDS: i32 = 0x12;
}

/// What one that stands does with its time (0xF8FE0; its + 0x198, + 0x41C and
/// + 0x420): when it has stood for so many frames and its stand has come round, it
/// plays what it has for idling, and stands again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Idle {
    state: IdleState,
    /// The animation it plays in place of its stand, and whether that goes round.
    playing: Option<(i32, bool)>,
    /// The frames it has stood, or gone round, and those it is to: none until
    /// they are drawn.
    count: i32,
    until: Option<i32>,
    /// How far through its animation it is, in frames of the game, and whether
    /// this round of it is the last (0xFE3F4 with nought).
    clock: f32,
    last_round: bool,
    random: u32,
    /// The stand it has gone back to came at once, not over the frames one
    /// animation gives way to another in.
    pub at_once: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdleState {
    Standing,
    Beginning,
    GoingRound,
    Ending,
}

impl Idle {
    /// The numbers it draws its waits by are its own. (The original takes the
    /// field's, 0x10E45C, which nothing here shares with it.)
    pub fn new(seed: u32) -> Idle {
        let random = (seed.wrapping_mul(0x85eb_ca6b) ^ 0x9e37_79b9) | 1;
        Idle { state: IdleState::Standing, playing: None, count: 0, until: None, clock: 0.0, last_round: false, random, at_once: false }
    }

    /// The animation it plays in place of its stand, if any, and whether that
    /// goes round.
    pub fn playing(&self) -> Option<(i32, bool)> {
        self.playing
    }

    /// It stands, as after anything it did (0xF4908): the wait is drawn anew.
    pub fn rest(&mut self) {
        *self = Idle { random: self.random, ..Idle::new(0) };
    }

    /// Nobody is to idle from now on (0xF6004): one that stands goes on with its
    /// stand, and one that was idling stands.
    pub fn settle(&mut self) {
        match self.state {
            IdleState::Standing => self.last_round = false,
            _ => self.rest(),
        }
    }

    /// A wait of so many to so many frames (0xF60C4).
    fn draw(&mut self, (least, most): (i32, i32)) {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        let more = if most > least { (self.random % (most - least) as u32) as i32 } else { 0 };
        (self.count, self.until) = (0, Some(least + more));
    }

    fn play(&mut self, number: i32, round: bool) {
        (self.playing, self.clock, self.last_round) = (Some((number, round)), 0.0, false);
    }

    fn stand(&mut self, wait: (i32, i32), at_once: bool) {
        (self.state, self.playing, self.clock, self.last_round, self.at_once) = (IdleState::Standing, None, 0.0, false, at_once);
        self.draw(wait);
    }

    /// A frame of one that stands and is left to it (0xF8FE0). `pace` is how
    /// much of a frame its frame is; `length` how long one of its animations is
    /// in frames of the game, if it has it; `waits` the frames it stands before
    /// it idles and those its idling goes round for, each from so many to so
    /// many (`prm` + 0x18 to + 0x1E).
    pub fn step(&mut self, pace: f32, length: impl Fn(i32) -> Option<f32>, waits: [(i32, i32); 2]) {
        // Its animation goes on. One that goes round has no end until it is told
        // that this round is its last.
        let (number, round) = self.playing.unwrap_or((0, true));
        let whole = length(number).unwrap_or(0.0);
        self.clock += pace;
        let ended = if round && !self.last_round {
            if whole > 0.0 {
                self.clock %= whole;
            }
            false
        } else {
            self.clock >= whole
        };
        let has = |number: i32| length(number).is_some();
        match self.state {
            IdleState::Standing => {
                if !has(idling::BEGINS) && !has(idling::GOES_ROUND) {
                    return;
                }
                if self.until.is_none() {
                    self.draw(waits[0]);
                }
                if self.count < self.until.unwrap_or(0) {
                    self.count += 1;
                } else if !ended {
                    self.last_round = true;
                } else if has(idling::BEGINS) {
                    self.play(idling::BEGINS, false);
                    self.state = if has(idling::GOES_ROUND) { IdleState::Beginning } else { IdleState::Ending };
                } else {
                    self.play(idling::GOES_ROUND, true);
                    self.draw(waits[1]);
                    self.state = IdleState::GoingRound;
                }
            }
            IdleState::Beginning => {
                if ended {
                    self.play(idling::GOES_ROUND, true);
                    self.draw(waits[1]);
                    self.state = IdleState::GoingRound;
                }
            }
            IdleState::GoingRound => {
                if self.count < self.until.unwrap_or(0) {
                    self.count += 1;
                } else if !ended {
                    self.last_round = true;
                } else if has(idling::ENDS) {
                    self.play(idling::ENDS, false);
                    self.state = IdleState::Ending;
                } else {
                    self.stand(waits[0], false);
                }
            }
            IdleState::Ending => {
                if ended {
                    self.stand(waits[0], true);
                }
            }
        }
    }
}

/// How a character is lit, and what it stands on: what the ray down through it
/// finds each frame (0xFD3CC), and the colours it is drawn with, which are its
/// own and follow those of the scene's light it is under (0xFFB40, 0x105868).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lit {
    /// What kind of ground is under it (its + 0x161), which the sound of a step is
    /// chosen by: the first kind where nothing says.
    pub ground: u8,
    /// The light it is lit by (its + 0x164) and the light of the ground under it
    /// (its + 0x168), which are one unless a script has said which light it is
    /// lit by (setting 34, its flag 0x1E).
    pub light: u8,
    pub under: u8,
    pub pinned: bool,
    /// The four colours it is drawn with (its + 0x8B8 on): the light all about
    /// the scene and its light's three. What they are to be, each part a byte,
    /// and what they are, each part of 0xFF00.
    pub wanted: [[u8; 3]; 4],
    pub now: [[u16; 3]; 4],
    /// They are to be what the scene has at once: of one that has just been made
    /// or put somewhere (0x108BE8).
    pub at_once: bool,
}

/// How far a part of a character's colours goes in a frame, of one (0x10580C).
const LIT_STEP: f32 = 0.02;

impl Default for Lit {
    /// As it is made (0x105780): white.
    fn default() -> Lit {
        Lit { ground: 0, light: 0, under: 0, pinned: false, wanted: [[0xff; 3]; 4], now: [[0xffff; 3]; 4], at_once: true }
    }
}

impl Lit {
    /// What its colours are to be (0xFFB40, 0x105724): each part is kept as the
    /// low byte of what it comes to in 255, so that one below nothing comes out
    /// near the top (a light a script has given less by how a surface faces it
    /// than everywhere, as `s166(0, 75, 85, 100, 55, 85, 100)` does, is lit so).
    pub fn want(&mut self, all_about: Vec3, light: [Vec3; 3]) {
        let colours = [all_about, light[0], light[1], light[2]];
        self.wanted = colours.map(|colour| colour.to_array().map(|part| (part * 255.0) as i32 as u8));
    }

    /// A frame of them (0x105868): each part goes two hundredths towards what it
    /// is to be, and is that when it is nearer.
    pub fn step(&mut self) {
        for (now, wanted) in self.now.iter_mut().zip(self.wanted) {
            for (now, wanted) in now.iter_mut().zip(wanted) {
                let (to, at) = (f32::from(wanted) / 255.0, f32::from(*now) / 65535.0);
                let gap = to - at;
                let next = if gap.abs() < LIT_STEP { to } else { at + LIT_STEP.copysign(gap) };
                *now = (next * 65535.0) as u16;
            }
        }
    }

    /// They are what they are to be (0x105A18).
    pub fn settle(&mut self) {
        self.now = self.wanted.map(|colour| colour.map(|part| u16::from(part) << 8));
        self.at_once = false;
    }

    /// One of them as it is drawn (0x1059A0): 0 the light all about, 1 to 3 its
    /// light's.
    pub fn colour(&self, which: usize) -> Vec3 {
        Vec3::from(self.now[which].map(|part| f32::from(part >> 8) / 255.0))
    }
}

#[derive(Clone, Debug)]
pub struct Character {
    /// What it does with its time when it stands.
    pub idle: Idle,
    /// How it is lit, and what kind of ground it stands on.
    pub lit: Lit,
    /// How much of it there is (its float 0x1E, at its + 0x4BC): all when it is
    /// made (0x109A5C), and less only as one that is destroyed fades (0xF96E4).
    pub alpha: f32,
    /// How long it has been moving in the status it moves in (its + 0x35C,
    /// 0xF5FD0), from when it last set off.
    pub ran: i32,
    /// The resource it was made from: a record of the field's character list.
    pub chr: i32,
    pub position: Vec3,
    /// How the frame it is in lies (0x109128): the turn of that one of the
    /// field's sixteen its setting 115 names, and while it goes over from one
    /// to another, what is between the two. Everything of it that goes by an
    /// upright goes by this; the scene keeps it up ([`Scene::lay_characters`]).
    pub lay: Quat,
    /// The frame a leap is to take it into (its + 0x4C1, int parameter 9), and
    /// its going over to the frame it is in, while it does.
    pub group_next: Option<u8>,
    pub turning: Option<Turning>,
    /// Its movement over the ground, which also holds the way it faces.
    pub mover: Mover,
    pub flags: Flags,
    /// Where an action takes it, and how it is to face when it gets there.
    pub target: Vec3,
    pub target_facing: f32,
    pub action: Action,
    /// The status it is in and the one it moves in (int parameters 0x10 and 0xE).
    pub status: i32,
    pub move_status: i32,
    /// How fast its time runs, in time units a frame (int parameter 0x28).
    pub pace: i32,
    /// The pace it goes about at when left to itself, in percent (int parameter 0x29).
    pub own_pace: i32,
    /// How near the player has to come to touch it (decimal parameter 0xC).
    pub touch_radius: f32,
    /// How far from its home it will notice and follow her (decimal parameter 0x1A).
    pub territory: f32,
    /// The behaviour script that runs it (int parameter 37), if any.
    pub behaviour: Option<i32>,
    pub mind: Mind,
    /// The animation a script has asked for, to be played in place of the one its
    /// status would have, and whether it loops.
    pub motion: Option<(Motion, bool)>,
    /// The animation a script has named (its + 0x184 and + 0x188), which is what
    /// it plays when it is next set playing; how many times it has been set
    /// playing one, each of which begins it from its start; and the frame of
    /// the game its animation is held at, while a script holds it (its flag 9).
    pub named: Option<Motion>,
    pub motions: u32,
    pub held: Option<i32>,
    /// The face a script has given it (`set_face` and `shift_face` of the
    /// field's resident script, 0x10C3B0 and 0x10C444): which, and in how many
    /// frames of the game it comes; and how many it has been given, for whoever
    /// shows it.
    pub expression: Option<(i32, i32)>,
    pub expressions: u32,
    /// How far from where it would lie its shadow is put, along the world's
    /// own ways (its + 0x444, + 0x440 and + 0x448, 0x107B54).
    pub shadow_offset: Vec3,
    /// It is too far from her to have its frame (0x106558), and it is too far
    /// from her or too near the camera to be drawn (0x1066A8, 0x1063B8): as
    /// [`Scene::reach_characters`] last found.
    pub asleep: bool,
    pub unseen: bool,
    /// What cross and square do with it when she is by it (the first two records
    /// of its contact), and whether a button does anything with it at all (the
    /// contact's flag 0).
    pub prompts: [Prompt; 2],
    pub by_action: bool,
    /// The mark that it has noticed her, while that is over it: the frames of the
    /// mark's set that have been played.
    pub mark: Option<f32>,
    /// The sign a script has given it, as a trap has one (setting 92): where
    /// it was when it was given, and the frames the sign has had.
    pub sign: Option<(Vec3, u32)>,
    /// Settings a script has made that nothing here acts on yet, kept to be read back.
    pub other: BTreeMap<i32, i32>,
    /// What its actions have asked to be seen, until the field takes it.
    pub shows: Vec<Show>,
    /// What it is of, if it is an effect and not a model.
    pub effect: Option<EffectOf>,
    /// What its face does of itself.
    pub face: Face,
    /// Where its head is turned.
    pub neck: Neck,
    /// The box it is to those that would walk through it, where a script has
    /// given it one (`s236`): about a place of its own frame, turned in that
    /// frame. Without one it is a round about where it stands.
    pub boxed: Option<Wall>,
    /// Where a trap is taking it, place after place (`s235`).
    pub taken: Option<Taken>,
    /// What an event has given it to hold (`s71`; five things of 0x20 bytes at
    /// its + 0x510): the model of each, and whether each is put away (`s150`).
    pub props: [Option<String>; PROPS],
    pub props_away: [bool; PROPS],
}

/// How many things an event can give a character to hold, and the node of its
/// model each is at (0xFD304): the first in its right hand, the second in its
/// left, the rest where its model begins.
pub const PROPS: usize = 5;
pub const PROP_NODES: [u8; PROPS] = [0x10, 0x0b, 0, 0, 0];

/// Where a character's head is turned (its + 0x6E0 on, kept for every character
/// that has a model). A script says how it is turned now and how it is to be
/// turned next, and has it go over from the one to the other in so many frames
/// (`s209`, 0xFF6BC); the model's frame then turns the head and all that hangs
/// off it, where the head is (0x1004F0, 0xFEAC0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neck {
    /// It turns the head at all (the character's flag 0x54), and is on its way
    /// from how the head was turned to how it is to be (flag 0x55).
    pub on: bool,
    pub moving: bool,
    /// How (+ 0x130): by its angles about the head's own axes as they lie, by its
    /// angles from the way the body faces, at a point, or at a character; and
    /// how when it is next started (+ 0x134).
    pub way: NeckWay,
    pub next_way: NeckWay,
    /// Its angles about the axis across, the upright one and the one ahead, in
    /// radians (+ 0xF0), and those it has when it is next started (+ 0x110).
    pub angles: Vec3,
    pub next_angles: Vec3,
    /// The character it looks at (+ 0x170), and the one it looks at when it is
    /// next started (+ 0x174).
    pub object: i32,
    pub next_object: i32,
    /// How long it has been on its way, how long that takes, in the field's
    /// units of time, and how it goes (+ 0x178, + 0x17C, + 0x180).
    pub time: i32,
    pub length: i32,
    pub ease: i32,
    /// How many times it has been started: each starts from how the head was
    /// last to be turned.
    pub starts: u32,
    /// How many times a script has said how it turns the head now (0xFF31C,
    /// which sets the character's flag 0x6C): one that looks at something far
    /// round to one side then chooses its side anew.
    pub ways: u32,
}

/// The ways a neck turns the head, as the numbers scripts give less one (0xC7A2C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeckWay {
    /// By its angles about the head's own axes as its animation has them (0xFEED0).
    Own,
    /// By its angles from the way the body faces, whatever the animation does
    /// with the head (0xFF088).
    FromBody,
    /// At a point that is written into the game (0x1005C0), and at a character
    /// (0x100610).
    Point,
    Object,
}

impl NeckWay {
    /// The way a script names by a number from 1 on; what is past the last is
    /// the first.
    fn of(value: i32) -> NeckWay {
        match value {
            2 => NeckWay::FromBody,
            3 => NeckWay::Point,
            4 => NeckWay::Object,
            _ => NeckWay::Own,
        }
    }
}

impl Default for Neck {
    fn default() -> Neck {
        Neck {
            on: false,
            moving: false,
            way: NeckWay::Own,
            next_way: NeckWay::Own,
            angles: Vec3::ZERO,
            next_angles: Vec3::ZERO,
            object: 0,
            next_object: 0,
            time: 0,
            length: 0,
            ease: 0,
            starts: 0,
            ways: 0,
        }
    }
}

impl Neck {
    /// `s209(id, frames, ease)` (0xC543C, 0xFF6BC): what was to be next is what
    /// it is now, and the head goes over to it in so many frames of the game,
    /// evenly (0), slowly at first (1), slowly at last (2) or both (3).
    pub fn start(&mut self, frames: i32, ease: i32) {
        self.length = frames * FULL_PACE;
        self.ease = if (0..=3).contains(&ease) { ease } else { 0 };
        self.way = self.next_way;
        self.angles = self.next_angles;
        self.object = self.next_object;
        self.time = 0;
        (self.on, self.moving) = (true, true);
        self.starts = self.starts.wrapping_add(1);
    }

    /// A frame of it (0xFEA30): it is there when its time is up.
    pub fn step(&mut self, time: i32) {
        if self.on && self.moving {
            self.time += time;
            if self.time >= self.length {
                self.moving = false;
            }
        }
    }

    /// How much of the way it has come, of one (0x100674): by the part of its
    /// time that is up, evenly, by its square, by one less the square of what is
    /// left, or by the square to half-way and one less the square from there.
    pub fn part(&self) -> f32 {
        if !self.moving || self.length <= 0 {
            return 1.0;
        }
        let (time, length) = (self.time as f32, self.length as f32);
        let slow_end = |part: f32| 1.0 - (1.0 - part) * (1.0 - part);
        match self.ease {
            0 => time / length,
            1 => (time / length) * (time / length),
            2 => slow_end(time / length),
            3 => {
                let half = self.length / 2;
                if self.time < half {
                    let part = time / half as f32;
                    part * part * 0.5
                } else {
                    slow_end((self.time - half) as f32 / half as f32) * 0.5 + 0.5
                }
            }
            _ => 0.0,
        }
    }
}

/// What stands in the way of a character that moves: each other character that
/// is solid, by the shape it has (0x10231C makes the list for the one that
/// moves, of the shapes 0x108CA0 gives, each in its character's frame; 0xE0FB4
/// then has the mover's sphere swept against the map and against each).
#[derive(Clone, Copy, Debug)]
pub struct Solid {
    /// Where the character it is of stands: nothing is in its own way.
    pub of: Vec3,
    pub shape: SolidShape,
}

#[derive(Clone, Copy, Debug)]
pub enum SolidShape {
    /// What a character is until a script says otherwise (shape kind 0): an
    /// upright round of its radius, a hundred high from where it stands.
    Round(f32),
    /// The box a script has given it (kind 1), as it lies in the world.
    Box(Wall),
}

/// How high a character's round is (0x108D4C).
const ROUND_HEIGHT: f32 = 100.0;

impl Solid {
    /// The same, turned about where the world begins: as something in a frame
    /// that is turned has it. Its own upright is not turned with it.
    pub fn turned(&self, by: Quat) -> Solid {
        let shape = match self.shape {
            SolidShape::Box(wall) => SolidShape::Box(Wall { centre: by * wall.centre, ..wall }),
            round => round,
        };
        Solid { of: by * self.of, shape }
    }

    /// Where a body of `radius` standing at `feet` is moved to so as not to be in
    /// it: straight out from the middle of a round, or out by the nearest side
    /// of a box. A stand-in for the original's sweep of a sphere against the
    /// shape (0x15978, 0x1642C) and its slide along what is hit (0xE1268).
    pub fn push_out(&self, feet: Vec3, radius: f32) -> Vec3 {
        match self.shape {
            SolidShape::Box(wall) => wall.push_out(feet, radius),
            SolidShape::Round(round) => {
                let away = Vec2::new(feet.x - self.of.x, feet.z - self.of.z);
                let (reach, above) = (round + radius, feet.y - self.of.y);
                if away.length() >= reach || above <= -2.0 * radius || above >= ROUND_HEIGHT {
                    return feet;
                }
                // From its very middle one way is as good as another.
                let way = away.try_normalize().unwrap_or(Vec2::Y) * reach;
                Vec3::new(self.of.x + way.x, feet.y, self.of.z + way.y)
            }
        }
    }
}

/// What a character's face does of itself (its + 0x6D4 on). The model's frame
/// has it after the model's pose (0xFD2B4): the jaw goes up and down while its
/// mouth moves (0x100164), and the lids come down now and then (0xFFE9C). Each
/// turns a node of the model that hangs off the head, node 6 and node 5, about
/// the way the head's own third axis lies over the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    /// A script has given it something to do at some time.
    pub used: bool,
    /// Its mouth moves (bit 1 of + 0x6D4): how much (+ 0x6DA), how far round it
    /// is in degrees (+ 0x6D8), and the frames before its voice is listened for
    /// (+ 0x6D6).
    speaks: bool,
    amount: u16,
    mouth: u16,
    grace: i16,
    /// Its eyes blink (bit 2): the frames to the next blink (+ 0x6DE; at 1 they
    /// stay open, at 9999 shut) and how far round this one is (+ 0x6DC).
    blinks: bool,
    blink_in: i16,
    blink: u16,
    random: u32,
}

/// What `s159` is given for eyes that stay shut, and the count at which they
/// stay open.
pub const LIDS_SHUT: i32 = 9999;
const LIDS_OPEN: i16 = 1;
/// The longest a script can have the first blink wait (0xFE5E8).
const BLINK_WAIT_AT_MOST: i32 = 6000;
/// A blink is over when its count is under three, and the next comes this many
/// frames later and up to as many again (0xFFFF8).
const BLINK_OVER: i16 = 3;
const BLINK_EVERY: u32 = 0x78;
/// How far round a blink goes in a frame, in degrees, and where it stops; how
/// far the lids come down at its height, and for eyes that stay shut, in radians
/// (0x100018, 0x100084, 0x1000A4).
const BLINK_STEP: u16 = 0x32;
const HALF_TURN: u16 = 180;
const BLINK_DEPTH: f32 = 1.0 / 3.0;
const SHUT_DEPTH: f32 = 0.35;
/// The most a mouth can be asked to move, the frames a mouth waits before its
/// voice is listened for, the least it goes round in a frame, and how far the
/// jaw comes down at its lowest (0xFE628, 0xFE650, 0x1002BC, 0x100360).
const MOUTH_AT_MOST: i32 = 100;
const MOUTH_GRACE: i16 = 10;
const MOUTH_STEP: u16 = 8;
const MOUTH_DEPTH: f32 = 1.0 / 6.0;

impl Default for Face {
    fn default() -> Face {
        Face { used: false, speaks: false, amount: 0, mouth: 0, grace: 0, blinks: false, blink_in: 0, blink: 0, random: 0x9e37_79b9 }
    }
}

impl Face {
    /// `s159` (0xFE5B4): its eyes blink, the first time after so many frames
    /// (and two) and then every four to eight seconds; with 1 they stay open,
    /// with 9999 shut, and with nought or less they are let be.
    pub fn blink_after(&mut self, frames: i32) {
        self.used = true;
        self.blink = 0;
        let frames = match frames {
            LIDS_SHUT | 0 | 1 => frames,
            other => (other + 2).clamp(0, BLINK_WAIT_AT_MOST),
        };
        self.blinks = frames > 0;
        self.blink_in = if self.blinks { frames as i16 } else { 0 };
    }

    /// `s160` (0xFE620): its mouth moves, so much of a hundred; with nought it
    /// is still.
    pub fn speak(&mut self, amount: i32) {
        self.used = true;
        self.mouth = 0;
        let amount = amount.clamp(0, MOUTH_AT_MOST);
        self.speaks = amount != 0;
        self.amount = amount as u16;
        self.grace = if self.speaks { MOUTH_GRACE } else { 0 };
    }

    /// A number under `below` by chance. (The original takes the game's own
    /// random numbers, 0x93A0, which nothing here shares with it.)
    fn chance(&mut self, below: u32) -> u32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        if below == 0 { 0 } else { (self.random >> 8) % below }
    }

    /// A frame of it: the blink that is going on goes on or the next begins, and
    /// the mouth goes round by eight degrees and something by chance, more the
    /// more it is to move.
    pub fn step(&mut self) {
        if self.blinks && self.blink_in != LIDS_SHUT as i16 && self.blink_in != LIDS_OPEN {
            self.blink_in -= 1;
            if self.blink_in < BLINK_OVER {
                self.blink_in = (self.chance(BLINK_EVERY) + BLINK_EVERY) as i16;
                self.blink = 0;
            } else {
                self.blink = (self.blink + BLINK_STEP).min(HALF_TURN);
            }
        }
        if self.speaks {
            if self.amount < 1 {
                self.amount = 2;
            }
            let amount = self.amount as u32;
            let more = match self.chance(4) {
                0 | 1 => self.chance(amount),
                2 => self.chance(amount * 2),
                _ => self.chance(amount * 4),
            };
            self.mouth = self.mouth.wrapping_add(more as u16 + MOUTH_STEP);
        }
    }

    /// The sine of an angle in degrees as the faces take it: of what is left of
    /// it over the half turns in it.
    fn swing(degrees: u16) -> f32 {
        ((degrees % HALF_TURN) as f32).to_radians().sin()
    }

    /// How far the lids are turned down, in radians.
    pub fn lids(&self) -> f32 {
        match self.blink_in {
            _ if !self.blinks => 0.0,
            shut if shut == LIDS_SHUT as i16 => SHUT_DEPTH,
            LIDS_OPEN => 0.0,
            _ => Face::swing(self.blink) * BLINK_DEPTH,
        }
    }

    /// How far the jaw is turned down, in radians.
    pub fn jaw(&self) -> f32 {
        if self.speaks { Face::swing(self.mouth) * MOUTH_DEPTH } else { 0.0 }
    }
}

/// What a script asks of the camera's shake: that it come over so many frames,
/// as far up and down and from side to side as it says, in radians (`s228`,
/// 0xE5A5C), or that it die away over so many (`s229`, 0xE5A88).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShakeAsked {
    Come { reach: Vec2, frames: i32 },
    DieAway(i32),
}

/// The resource of a character that is not of the field's list of them.
pub const NO_RESOURCE: i32 = -1;
/// How far over where a model stands its distance from the camera is measured
/// (0x10649C).
const EYE_MEASURED_OVER: f32 = 100.0;
/// And those of the characters that are things of the map's own (`s111`; the
/// original keeps them as resources of another kind, 4, with their own
/// numbers): the first of them, the rest going down from it.
const OBJECT_OF_MAP: i32 = -2;

/// The resource that thing `number` of the map's own is, and the number of the
/// thing a resource is, if it is one.
pub fn object_resource(number: i32) -> i32 {
    OBJECT_OF_MAP - number.max(0)
}

pub fn object_of(chr: i32) -> Option<i32> {
    (chr <= OBJECT_OF_MAP).then_some(OBJECT_OF_MAP - chr)
}

/// What makes a character an effect and not a model (the kind made at 0x1029B4,
/// which `s53` makes, 0xE3028). It is a character like any other to the scripts:
/// they put it somewhere, turn it, hide it and have a button do something with
/// it. It has room for eight effects (its + 0x510), and every one the game has
/// is made of a file of a single effect, which is the first of them; nothing
/// sets another playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectOf {
    /// The file its effect is of: one of the map's own, of the field's, or of
    /// those an event has brought.
    pub file: String,
    /// How many times it has been set playing: each begins it afresh (0x1031B0).
    pub begun: u32,
    /// The character, and the node of its model, that it is fastened to (its flag
    /// 0x5C, + 0x414 and + 0x418; 0x108AB0): it is then put from where that node
    /// is and not from where the world begins (0x102A2C).
    pub fastened: Option<(i32, i32)>,
}

/// The frames of the interface the mark of having noticed her plays for: the
/// first set of the signs' layout.
pub const MARK_FRAMES: f32 = 54.0;

/// What a button does with something she is by, as far as the field itself goes
/// (a record of a contact's four, 0xD3974: flags and a byte): whether there is
/// anything for the button to do, and the word of the sign that says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub kind: i32,
    pub on: bool,
}

/// The words of the sign of what a button would do, by their numbers: only the
/// scripts say which a thing has.
#[allow(dead_code)]
pub mod prompt {
    pub const ACTION: i32 = 0;
    pub const CHECK: i32 = 1;
    pub const TALK: i32 = 2;
}

impl Prompt {
    /// None: the button does nothing.
    pub const NONE: Prompt = Prompt { kind: 0, on: false };

    pub const fn of(kind: i32) -> Prompt {
        Prompt { kind, on: true }
    }
}

impl Character {
    pub fn new(chr: i32, position: Vec3, facing: f32, defaults: &Defaults) -> Character {
        let mut flags = Flags::default();
        flags.set(flag::READY, true);
        Character {
            idle: Idle::new(chr as u32 ^ position.x.to_bits() ^ position.z.to_bits().rotate_left(13)),
            lit: Lit::default(),
            alpha: 1.0,
            ran: 0,
            chr,
            position,
            lay: Quat::IDENTITY,
            group_next: None,
            turning: None,
            mover: Mover::new(facing, 0.0),
            flags,
            target: position,
            target_facing: facing,
            action: Action::None,
            status: status::STAND,
            move_status: status::RUN,
            pace: FULL_PACE,
            own_pace: defaults.pace,
            touch_radius: defaults.touch_radius,
            territory: defaults.territory,
            behaviour: None,
            mind: Mind {
                movement: 0,
                hide_timer: 0,
                unaware_timer: 0,
                target: PLAYER,
                home: position,
                notice_range: defaults.notice_range,
                wander_radius: defaults.wander_radius,
                wander_time: 0,
                route: [NO_POINT; ROUTE_POINTS],
                route_length: 0,
                route_pause: 0,
                on_its_way: true,
                stand_down: false,
            },
            motion: None,
            named: None,
            motions: 0,
            held: None,
            expression: None,
            expressions: 0,
            shadow_offset: Vec3::ZERO,
            asleep: false,
            unseen: false,
            // As it is made (0xE1920): cross talks to it.
            prompts: [Prompt::of(prompt::TALK), Prompt::NONE],
            by_action: true,
            mark: None,
            sign: None,
            other: BTreeMap::new(),
            shows: Vec::new(),
            effect: None,
            face: Face::default(),
            neck: Neck::default(),
            boxed: None,
            taken: None,
            props: Default::default(),
            props_away: [false; PROPS],
        }
    }

    /// `s71` (0xFDBB8): it is given a thing of an event's to hold, in one of its
    /// five places for such; and `s150` (0xFE3C4): what is in one of them is
    /// put away, or brought out again.
    pub fn hold(&mut self, place: i32, model: &str) {
        if let Some(held) = usize::try_from(place).ok().and_then(|place| self.props.get_mut(place)) {
            *held = Some(model.to_owned());
        }
    }

    pub fn put_away(&mut self, place: i32, away: bool) {
        if let Some(is_away) = usize::try_from(place).ok().and_then(|place| self.props_away.get_mut(place)) {
            *is_away = away;
        }
    }

    /// One that is an effect, as it is made (0xE3028): where the world begins,
    /// until a script puts it somewhere.
    pub fn of_effect(file: String, defaults: &Defaults) -> Character {
        let mut character = Character::new(NO_RESOURCE, Vec3::ZERO, 0.0, defaults);
        character.effect = Some(EffectOf { file, begun: 0, fastened: None });
        character
    }

    /// What `s53` does with the character of the number it is given, whether it
    /// has just made it or it was there (0xC4038 on): cross checks it, and its
    /// effect is set playing from the start.
    pub fn begin_effect(&mut self) {
        self.flags.set(flag::OF_EFFECT, true);
        self.set(setting::CONTACT, prompt::CHECK);
        self.set(setting::BY_ACTION, 1);
        if let Some(effect) = &mut self.effect {
            effect.begun += 1;
        }
    }

    /// Where it is, how it is turned and how large it is, as its model or its
    /// effect is put (0x10925C, 0x102A2C): turned about the axis across, then
    /// the upright one, then the one ahead (0x162D58), and as large every way
    /// as setting 88 has it, in hundredths; and that turn is turned again by
    /// the one of the frame it is in (0x109128), which only the sixth stage's
    /// maps turn.
    pub fn frame(&self) -> Transform {
        let turn = |what: i32| self.other.get(&what).map_or(0.0, |&degrees| (degrees as f32).to_radians());
        let own = Quat::from_rotation_z(turn(setting::TURN_Z)) * Quat::from_rotation_y(self.facing()) * Quat::from_rotation_x(turn(setting::TURN_X));
        let scale = self.other.get(&setting::SCALE).map_or(1.0, |&hundredths| hundredths as f32 / 100.0);
        Transform { translation: self.position, rotation: self.lay * own, scale: Vec3::splat(scale) }
    }

    /// Which of the field's sixteen frames it is in (its int parameter 8).
    pub fn group(&self) -> u8 {
        self.other.get(&setting::GROUP).map_or(0, |&group| group.clamp(0, FRAMES as i32 - 1) as u8)
    }

    /// Its upright, as its frame lies (0x1090E0).
    pub fn up(&self) -> Vec3 {
        self.lay * Vec3::Y
    }

    /// A way of the world's as its frame has it (0x107E34, 0x105538: by the
    /// frame's turn undone), and the angle about its upright of one that faces
    /// that way.
    pub fn in_frame(&self, way: Vec3) -> Vec3 {
        if self.lay == Quat::IDENTITY { way } else { self.lay.inverse() * way }
    }

    pub fn bearing(&self, way: Vec3) -> f32 {
        let own = self.in_frame(way);
        own.x.atan2(own.z)
    }

    /// Whether it is in none of the field's frames just now: going over from
    /// one to another, or gathering itself for the leap that will take it over.
    pub fn between_frames(&self) -> bool {
        self.turning.is_some() || (self.group_next.is_some() && matches!(self.action, Action::Leap(_)))
    }

    /// How its frame lies is worked out anew (0x109128): the turn of the frame
    /// it is in, and before that, while it goes over, so much of the way back
    /// to the one it was in.
    pub fn lie_in(&mut self, frames: &Frames) {
        self.lay = match &self.turning {
            Some(turning) => turning.lay(),
            None => frames.turn(self.group()),
        };
    }

    /// It goes over into another frame in so much of its time (0x10930C, from a
    /// leap as it leaves the ground): it is in that frame at once, lies as it
    /// did and comes to lie as the frame does; and it faces the way it is going
    /// as that lies over the new frame's ground, all the while.
    pub fn change_group(&mut self, to: u8, frames: &Frames, length: i32) {
        let (was, next) = (frames.turn(self.group()), frames.turn(to));
        let (way, up) = ((self.target - self.position).normalize_or_zero(), next * Vec3::Y);
        let ahead = (way - up * way.dot(up)).try_normalize().unwrap_or(self.lay * super::mover::forward(self.facing()));
        self.other.insert(setting::GROUP, i32::from(to));
        self.turning = Some(Turning { from: was * next.inverse(), to: next, time: 0, length, ahead });
        self.lie_in(frames);
        self.mover.facing = self.bearing(ahead);
    }

    /// So much of its time goes by for one that is going over into another
    /// frame (0xF5E50): it lies further over, faces the way it was to as it
    /// lies now, and is over when the time is more than it was to take.
    pub fn step_turning(&mut self, time: i32) {
        let Some(turning) = &mut self.turning else { return };
        turning.time += time;
        let (over, lay, ahead) = (turning.time > turning.length, turning.lay(), turning.ahead);
        self.lay = lay;
        self.mover.facing = self.bearing(ahead);
        if over {
            self.turning = None;
        }
    }

    /// Everything of it that is a place or a way is turned about where the world
    /// begins: into the frame it is in, where it then acts, or out of it.
    pub fn turn_all(&mut self, by: Quat) {
        self.position = by * self.position;
        self.target = by * self.target;
        self.mover.velocity = by * self.mover.velocity;
        self.mover.direction = by * self.mover.direction;
        if let Action::Leap(leap) = &mut self.action {
            leap.from = by * leap.from;
        }
    }

    /// The mark that it has noticed her comes up over it (0x108A68, 0x46C68), if it
    /// is one that has such a mark, which those a behaviour script runs have
    /// (0xF9B1C); one that is up already goes on as it is.
    pub fn show_mark(&mut self) {
        if self.behaviour.is_some() && self.mark.is_none() {
            self.mark = Some(0.0);
        }
    }

    /// A frame of the mark (0x46A70): its set goes on, two frames of it a frame,
    /// and the mark is gone when that has been played through.
    pub fn step_mark(&mut self) {
        self.mark = self.mark.map(|frames| frames + 2.0).filter(|&frames| frames < MARK_FRAMES);
        if let Some((_, frames)) = &mut self.sign {
            *frames = frames.wrapping_add(1);
        }
    }

    /// Gives it a sign where it stands (0x107710: its flag 0x75, and as the
    /// object of its marks is made for that, 0x1073E4, the sign is put at its
    /// place, 0x44BF8). One that has it keeps it where it was.
    fn give_sign(&mut self) {
        if self.sign.is_none() {
            self.sign = Some((self.position, 0));
        }
    }

    /// The angle it faces about the vertical, in radians.
    pub fn facing(&self) -> f32 {
        self.mover.facing
    }

    pub fn hidden(&self) -> bool {
        self.flags.has(flag::HIDDEN)
    }

    /// Whether the field draws it this frame (0x1067F8): it is not hidden, nor
    /// too far from her or too near the camera.
    pub fn drawn(&self) -> bool {
        !self.hidden() && !self.unseen
    }

    /// The word of the sign for what a button does with it when she is by it, if
    /// there is anything: cross's (0) or square's (1). A character has TALK for
    /// cross from when it is made; scripts give home points and boxes CHECK and
    /// containers ACTION, and say of what is only there to be seen that no
    /// button does anything with it.
    pub fn prompt(&self, slot: usize) -> Option<i32> {
        self.prompts.get(slot).filter(|prompt| self.by_action && prompt.on).map(|prompt| prompt.kind)
    }

    /// Whether it is in the middle of something a script asked of it.
    pub fn acting(&self) -> bool {
        self.flags.has(flag::ACTING)
    }

    /// The time it has of a frame (0x108240): what a full frame is at its pace,
    /// and none while a script has it stopped (its flag 0x1B). Its animations
    /// go on by it (0xFCE90), and all that it does.
    pub fn time(&self) -> i32 {
        if self.asleep || self.flags.has(flag::STOPPED) { 0 } else { self.pace }
    }

    /// How much of a full-pace frame one of its frames is.
    pub fn pace_fraction(&self) -> f32 {
        self.time() as f32 / FULL_PACE as f32
    }

    /// Puts it somewhere, facing some way (0xE19E4 and 0xE1A84); the caller drops it
    /// onto the ground where that applies. It is lit as that place is at once
    /// (0x108BE8).
    pub fn place(&mut self, position: Vec3, facing: f32) {
        self.lit.at_once = true;
        self.position = position;
        self.mover.facing = wrap(facing);
        self.mover.velocity = Vec3::ZERO;
    }

    /// What of it stands in the way of the others, if anything does (0x108B18:
    /// nothing of one that is hidden, that a script has said is not solid, or
    /// that is an effect; 0x108CA0): its box, turned and put as it is, or a round
    /// of the field's own radius about where it stands.
    pub fn solid(&self, radius: f32) -> Option<Solid> {
        if self.hidden() || self.flags.has(flag::NO_DROP) || self.effect.is_some() {
            return None;
        }
        let shape = match self.boxed {
            Some(own) => SolidShape::Box(Wall {
                centre: self.position + Quat::from_rotation_y(self.facing()) * own.centre,
                reach: own.reach,
                turn: self.facing() + own.turn,
                open: false,
            }),
            None => SolidShape::Round(radius),
        };
        Some(Solid { of: self.position, shape })
    }

    /// `s236` (0xE42F8): it is a box to those that would walk through it, about
    /// a place of its own frame; a size that is not positive is a hundred.
    pub fn be_a_box(&mut self, centre: Vec3, reach: Vec3, turn: f32) {
        let mut own = Wall::new();
        own.shape(centre, reach, turn);
        self.boxed = Some(own);
    }

    /// `s160` (0xFE678): its mouth moves so much, or with nought is still; and
    /// of the two bits the first has the mouth stop of itself when what is
    /// spoken ends, the second has the character then go back to standing.
    pub fn speak(&mut self, amount: i32, how: i32) {
        self.flags.set(flag::MOUTH_WITH_VOICE, how & 1 != 0);
        self.flags.set(flag::STANDS_AFTER_VOICE, how & 2 != 0);
        self.face.speak(amount);
    }

    /// A frame of its face (0x100164, 0xFFE9C). `begun` is whether what a scene
    /// has spoken has begun to be heard since it was last hushed (the script's
    /// flag 0x1000), and `heard` whether anything is still being spoken
    /// (0xA3DC): a mouth that moves listens for that once ten frames of it
    /// have gone by, and when nothing is spoken any more it stops if it is to,
    /// and the character stands again if it is to and does not keep its pose.
    pub fn step_face(&mut self, begun: bool, heard: bool) {
        let spoke = self.face.speaks;
        self.face.step();
        if !spoke || !begun {
            return;
        }
        self.face.grace -= 1;
        if self.face.grace >= 0 {
            return;
        }
        self.face.grace = 0;
        if heard {
            return;
        }
        if self.flags.has(flag::MOUTH_WITH_VOICE) {
            self.face.speaks = false;
            self.flags.set(flag::MOUTH_WITH_VOICE, false);
        }
        if self.flags.has(flag::STANDS_AFTER_VOICE) {
            self.flags.set(flag::STANDS_AFTER_VOICE, false);
            if !self.flags.has(flag::KEEPS_POSE) {
                self.stop();
            }
        }
    }

    /// Ends whatever it is doing and has it stand (0xF4908).
    pub fn stop(&mut self) {
        self.action = Action::None;
        self.flags.set(flag::ACTING, false);
        self.flags.set(flag::DASHING, false);
        self.status = status::STAND;
        self.idle.rest();
    }

    /// A frame of its standing (0xF5F78): one that stands with nothing else to do
    /// idles after a while, unless nobody is to (`quiet`) or it never does.
    /// `length` is how long one of its animations is, if it has it.
    pub fn idle_on(&mut self, quiet: bool, length: impl Fn(i32) -> Option<f32>, waits: [(i32, i32); 2]) {
        let stands = self.status == status::STAND && self.action == Action::None && self.motion.is_none() && self.effect.is_none() && !self.hidden();
        if !stands {
            self.idle.rest();
        } else if !quiet && !self.flags.has(flag::NEVER_IDLES) {
            self.idle.step(self.pace_fraction(), length, waits);
        }
    }

    /// Sets it walking to its target (0xF52AC).
    pub fn walk(&mut self) {
        self.flags.set(flag::DASHING, false);
        self.begin_walk();
    }

    /// Sets it dashing: off the way it faces, until something stops it (0xF52E0).
    pub fn dash(&mut self) {
        self.flags.set(flag::DASHING, true);
        self.mover.direction = self.lay * super::mover::forward(self.facing());
        self.begin_walk();
    }

    fn begin_walk(&mut self) {
        self.action = Action::Walk;
        self.status = self.move_status;
        self.flags.set(flag::ACTING, true);
    }

    /// Is at once where its walk was taking it: what a walk comes to when the script
    /// is not waiting for anything (0xE44D4).
    pub fn arrive(&mut self) {
        self.position = self.target;
        self.stop();
    }

    /// Sets it turning to the way its target has it face (0xF4A68). The turn takes the
    /// same time whatever its size.
    pub fn turn(&mut self) {
        self.stop();
        let difference = wrap(self.target_facing - self.facing());
        if difference == 0.0 {
            return;
        }
        self.status = status::TURN;
        self.action = Action::Turn { rate: difference / TURN_FRAMES };
        self.flags.set(flag::ACTING, true);
    }

    /// Has it come up out of the ground where it stands, from `depth` below,
    /// through a ring on the ground there (0xF7090).
    pub fn appear(&mut self, depth: f32) {
        self.come(-depth, Action::Appear { time: 0, depth });
        self.shows.push(Show::Ring { at: self.target, facing: self.target_facing });
    }

    /// Has it hop up out of the ground where it stands, in smoke (0xF6D20).
    pub fn pop_up(&mut self) {
        self.come(-POP_UP_DEPTH, Action::PopUp { time: 0 });
        self.shows.push(Show::Smoke { at: self.target, facing: self.target_facing });
    }

    /// Has it drop in from high above where it stands (0xF7458).
    pub fn drop_in(&mut self) {
        self.come(DROP_HEIGHT, Action::DropIn { time: 0 });
    }

    /// Starts it on its way to where it stands, from so far above that.
    fn come(&mut self, from: f32, action: Action) {
        self.stop();
        self.target = self.position;
        self.target_facing = self.facing();
        self.position.y += from;
        self.action = action;
        self.flags.set(flag::NO_DROP, true);
        self.flags.set(flag::ACTING, true);
    }

    /// Has it come to where it stands in one of the ways scripts name by number
    /// (0xE6068; `ACT` has the same numbers, 0xD4D74): up out of the ground in
    /// smoke, in from above, or up through a ring from as far down as its own
    /// script says. Says whether the number is one of them.
    pub fn come_by(&mut self, way: i32, depth: f32) -> bool {
        match way {
            9 => self.pop_up(),
            10 => self.drop_in(),
            14 => self.appear(depth),
            _ => return false,
        }
        true
    }

    /// Destroys it (0xF93EC): it goes through its end and is then hidden. One
    /// whose script has it fade goes in smoke; any other bursts on its way.
    pub fn destroy(&mut self, way: u8) {
        self.stop();
        self.action = Action::Destroy { time: 0 };
        self.status = status::DESTROYED;
        self.flags.set(flag::NO_DROP, true);
        self.flags.set(flag::BURST, false);
        self.flags.set(flag::ACTING, true);
        if way == DESTROY_FADES {
            self.shows.push(Show::Smoke { at: self.position, facing: self.facing() });
        }
    }

    /// She pulls up (0xF573C): what she was doing ends, and she is in the status
    /// of it for as long as its animation plays, coming to a stop.
    pub fn brake(&mut self) {
        self.end_action();
        self.action = Action::Brake { time: 0 };
        self.status = status::BRAKE;
        self.ran = 0;
        self.flags.set(flag::FREE_TO_GO, false);
        self.flags.set(flag::ACTING, true);
    }

    /// Ends what it is doing as another action that begins ends it (0xF463C,
    /// by the action at 0xF4674), without having it stand: one that rushed has
    /// its aura go out, and so has one that flew, which is upright again.
    pub fn end_action(&mut self) {
        if !self.acting() {
            return;
        }
        match self.action {
            Action::Rush => self.shows.push(Show::AuraGoes { let_go: false }),
            Action::Flight => self.end_flight(),
            _ => {}
        }
        self.flags.set(flag::DASHING, false);
        self.flags.set(flag::ACTING, false);
    }

    /// `s235` (0xE5EC8): a trap has it taken to its places, one after another,
    /// if it has any. What it was doing ends, and it sets off for the first.
    /// `speed` is how fast its script has it run.
    pub fn take_to(&mut self, places: &[TakenTo], speed: f32) {
        if places.is_empty() {
            return;
        }
        self.end_action();
        self.taken = Some(Taken { places: places.to_vec(), at: 0 });
        self.set_off(speed);
    }

    /// What it was doing has ended (0x1061B8): one a trap is taking from place
    /// to place sets off for the next, if there is one, and this says that it
    /// did.
    pub fn taken_on(&mut self, speed: f32) -> bool {
        let Some(taken) = &mut self.taken else { return false };
        taken.at += 1;
        if taken.at < taken.places.len() {
            self.set_off(speed);
            true
        } else {
            self.taken = None;
            false
        }
    }

    /// It sets off for the place it is to be taken to next (0x109AC8): the
    /// nearest of that place's line is where it is going, and the way there the
    /// way it is to face, which is the angle the straight way there makes with
    /// the way a character that is not turned faces, to the side it is on
    /// (0x108DFC: of the whole way, height and all, so that a place far above
    /// or below turns it less than it would have to turn to face it).
    fn set_off(&mut self, speed: f32) {
        let Some(taken) = &self.taken else { return };
        let Some(&place) = taken.places.get(taken.at) else { return };
        let by = |index: Option<usize>| index.and_then(|index| taken.places.get(index)).map(|place| place.by);
        let (before, after) = (by(taken.at.checked_sub(1)), by(Some(taken.at + 1)));
        let leap = place.by == TakenBy::Leap;

        let to = place.nearest(self.position);
        let way = self.in_frame(to - self.position).normalize_or_zero();
        let turned = way.z.clamp(-1.0, 1.0).acos();
        self.target = to;
        self.target_facing = if way.x < 0.0 { -turned } else { turned };
        self.flags.set(flag::TAKEN_ON, after.is_some());
        self.flags.set(flag::FROM_FLIGHT, leap && before == Some(TakenBy::Flight));
        match place.by {
            TakenBy::Leap | TakenBy::PlainLeap => self.leap(speed),
            TakenBy::Rush => self.rush(),
            TakenBy::Flight => self.fly(),
        }
    }

    /// Sets it leaping to its target (0xF7F04). One that takes no part yet is
    /// simply put there. The leap is timed by how far the place is and how far
    /// above or below; and it gathers itself first, unless it has just flown
    /// to where it leaps from, when it is in the air at once and coming down.
    fn leap(&mut self, speed: f32) {
        self.end_action();
        if !self.flags.has(flag::READY) {
            self.position = self.target;
            self.mover.facing = self.target_facing;
            return;
        }
        // The way it faces, counted so that it is no more than half a turn from
        // the way it is to face.
        let mut facing = self.facing();
        if self.target_facing - facing > std::f32::consts::PI {
            facing += std::f32::consts::TAU;
        }
        if self.target_facing - facing < -std::f32::consts::PI {
            facing -= std::f32::consts::TAU;
        }
        let from = self.position;
        let way = self.target - from;
        let a_unit = 1.0 / (speed * 30.0);
        let between = self.group_next.is_some();
        let (seconds, springs) = if between {
            (way.length() * a_unit, false)
        } else if (-LEAP_LEVEL..=LEAP_LEVEL).contains(&way.y) {
            (way.length() * a_unit, true)
        } else if way.y > 0.0 {
            let steep = way.y.atan2(way.with_y(0.0).length()).to_degrees();
            if steep > LEAP_STEEP {
                (a_unit * way.y * LEAP_STEEP_PART, true)
            } else {
                (way.with_y(way.y + LEAP_HIGHER).length() * a_unit, true)
            }
        } else {
            ((way.y.abs() / 100.0 * 2.0 / GRAVITY).sqrt() + LEAP_DOWN_MORE, false)
        };
        let length = (seconds * A_SECOND as f32) as i32;
        let spring = if springs { (seconds / LEAP_SPRING_PARTS * A_SECOND as f32) as i32 } else { 0 };
        let mut leap = Leap { part: LeapPart::Gather, time: 0, from, facing, length, spring, between };
        if self.flags.has(flag::FROM_FLIGHT) {
            self.shows.push(Show::Heard(SOUND_LEAP));
            (leap.part, leap.spring) = (LeapPart::Spring, 0);
        } else {
            self.mover.velocity = Vec3::ZERO;
        }
        self.motions = self.motions.wrapping_add(1);
        self.status = status::LEAP;
        self.action = Action::Leap(leap);
        self.flags.set(flag::ACTING, true);
    }

    /// Sets it rushing to its target (0xF650C), in the status it rushes in, the
    /// straight way there, with an aura about it.
    fn rush(&mut self) {
        self.end_action();
        self.head_for_target();
        self.shows.push(Show::Aura);
        self.motions = self.motions.wrapping_add(1);
        self.mover.velocity = Vec3::ZERO;
        self.action = Action::Rush;
        self.flags.set(flag::ACTING, true);
    }

    /// Sets it flying straight to its target (0xF6944), as it is set rushing.
    fn fly(&mut self) {
        self.end_action();
        self.head_for_target();
        self.shows.push(Show::Aura);
        self.motions = self.motions.wrapping_add(1);
        self.mover.velocity = Vec3::ZERO;
        self.action = Action::Flight;
        self.flags.set(flag::ACTING, true);
    }

    /// The status it moves in and the one it is in are the one it rushes in, and
    /// the way it wants to go is the way to its target, unless it is there.
    pub fn head_for_target(&mut self) {
        (self.move_status, self.status) = (status::RUSH, status::RUSH);
        let way = self.target - self.position;
        if way.length() > 1e-5 {
            self.mover.direction = way.normalize();
        }
    }

    /// One that rushed is there (0xF6730): its aura goes out, and it stops
    /// (0xF6818) unless a trap is taking it on to another place.
    pub fn end_rush(&mut self) {
        self.shows.push(Show::AuraGoes { let_go: false });
        self.flags.set(flag::ACTING, false);
        if !self.flags.has(flag::TAKEN_ON) {
            self.motions = self.motions.wrapping_add(1);
            self.action = Action::RushStop { time: 0 };
            self.flags.set(flag::ACTING, true);
        }
    }

    /// While it flies it lies on its back in the air, head first the way a
    /// character that is not turned faces (0xF6BDC: its turns are a quarter
    /// about the axis across, none about the upright one and a half about the
    /// one ahead); and when it is there it is upright again and faces the
    /// other way, its aura going out where it is (0xF6C98).
    pub fn lie_in_flight(&mut self) {
        self.other.insert(setting::TURN_X, 90);
        self.other.insert(setting::TURN_Z, 180);
        self.mover.facing = 0.0;
    }

    pub fn end_flight(&mut self) {
        self.other.remove(&setting::TURN_X);
        self.other.remove(&setting::TURN_Z);
        self.mover.facing = std::f32::consts::PI;
        self.shows.push(Show::AuraGoes { let_go: true });
        self.flags.set(flag::ACTING, false);
    }

    /// Makes one of the settings scripts make by number: the map's script with `s52`
    /// and a behaviour script with its function 0x12 (the switch at 0xC7520). A
    /// setting nothing here acts on yet is kept as it was given.
    pub fn set(&mut self, what: i32, value: i32) {
        let on = value != 0;
        match what {
            setting::HIDDEN => {
                self.stop();
                self.flags.set(flag::HIDDEN, on);
            }
            setting::NO_DROP => self.flags.set(flag::NO_DROP, on),
            setting::TOUCH_RADIUS => self.touch_radius = value as f32,
            // The neck (0xC7A2C): how the head is turned, which also has the neck
            // turn it, or with nought has it turn it no more; how it is to be
            // turned next (nought there too is no more); its angles, in
            // degrees; and whom it looks at.
            setting::NECK | setting::NECK_NEXT if value == 0 => self.neck.on = false,
            setting::NECK => {
                (self.neck.way, self.neck.on) = (NeckWay::of(value), true);
                self.neck.ways = self.neck.ways.wrapping_add(1);
            }
            setting::NECK_NEXT => self.neck.next_way = NeckWay::of(value),
            setting::NECK_X => self.neck.angles.x = (value as f32).to_radians(),
            setting::NECK_Y => self.neck.angles.y = (value as f32).to_radians(),
            setting::NECK_Z => self.neck.angles.z = (value as f32).to_radians(),
            setting::NECK_NEXT_X => self.neck.next_angles.x = (value as f32).to_radians(),
            setting::NECK_NEXT_Y => self.neck.next_angles.y = (value as f32).to_radians(),
            setting::NECK_NEXT_Z => self.neck.next_angles.z = (value as f32).to_radians(),
            setting::NECK_OBJECT => self.neck.object = value,
            setting::NECK_NEXT_OBJECT => self.neck.next_object = value,
            setting::MOVE_STATUS => self.move_status = value,
            setting::X => self.position.x = value as f32,
            setting::Y => self.position.y = value as f32,
            setting::Z => self.position.z = value as f32,
            setting::FACING => self.mover.facing = wrap((value as f32).to_radians()),
            setting::TARGET_X => self.target.x = value as f32,
            setting::TARGET_Y => self.target.y = value as f32,
            setting::TARGET_Z => self.target.z = value as f32,
            setting::TARGET_FACING => self.target_facing = (value as f32).to_radians(),
            setting::PACE => self.pace = pace(value),
            setting::STOPPED => self.flags.set(flag::STOPPED, on),
            setting::BEHAVIOUR => {
                self.behaviour = (value >= 0).then_some(value);
                self.flags.set(flag::BEHAVES, value >= 0);
            }
            setting::SYMBOL => {
                self.flags.set(flag::SYMBOL, on);
                self.flags.set(flag::CHASING, false);
            }
            setting::KEEPS_POSE => self.flags.set(flag::KEEPS_POSE, on),
            setting::ALWAYS_ACTIVE => self.flags.set(flag::ALWAYS_ACTIVE, !on),
            setting::OWN_PACE => self.own_pace = value,
            setting::WEAPON => self.flags.set(flag::NO_WEAPON, !on),
            setting::SHADOW => self.flags.set(flag::NO_SHADOW, !on),
            setting::IDLES => self.flags.set(flag::NO_IDLING, !on),
            setting::SHADOW_UP => self.shadow_offset.y = value as f32,
            setting::SHADOW_X => self.shadow_offset.x = value as f32,
            setting::SHADOW_Z => self.shadow_offset.z = value as f32,
            setting::NO_LAYERS => self.flags.set(flag::NO_LAYERS, on),
            setting::POINTED_OUT => self.flags.set(flag::POINTED_OUT, on),
            setting::SIGN => self.give_sign(),
            setting::DRAWN_ORDER => self.flags.set(flag::ORDERED, value >= 0),
            setting::CLOSE_LENS => self.flags.set(flag::CLOSE_LENS, on),
            setting::FACE => self.show_face(value, 0),
            setting::TOUCHABLE => self.flags.set(flag::TOUCHABLE, on),
            setting::TARGET => self.mind.target = value,
            setting::NOTICE_RANGE => self.mind.notice_range = value as f32,
            // How a symbol moves about when left to itself, and the route of one
            // that patrols: how many points it has, and how many frames it stops
            // at each (0xC7904, 0xC7918, 0xC7FEC).
            setting::MOVEMENT => {
                self.set_int(int::MOVEMENT, value);
            }
            setting::ROUTE_LENGTH => self.mind.route_length = value,
            setting::ROUTE_PAUSE => self.mind.route_pause = value * FULL_PACE,
            setting::WANDER_RADIUS => self.mind.wander_radius = value as f32,
            setting::NO_STEPS => self.flags.set(flag::NO_STEPS, on),
            // Which of the scene's lights it is lit by, or with less than nothing
            // the one of the ground under it again (0x10A97C).
            setting::LIGHT => {
                self.lit.pinned = value >= 0;
                self.lit.light = if value >= 0 { value.min(LIGHTS as i32 - 1) as u8 } else { self.lit.under };
            }
            setting::DASHES => self.flags.set(flag::NO_DASH, !on),
            setting::TERRITORY => self.territory = value as f32,
            // What cross or square does with it is given its sign's word (0xC7578),
            // or taken away and given back (0xC75CC); either has a button do
            // something with it (0xE3724). The first is kept to be read back too.
            setting::CONTACT | setting::CONTACT_SQUARE => {
                self.prompts[(what - setting::CONTACT) as usize] = Prompt::of(value);
                self.by_action = true;
                self.other.insert(what, value);
            }
            setting::PROMPT_ON | setting::PROMPT_ON_SQUARE => {
                self.prompts[(what - setting::PROMPT_ON) as usize].on = on;
                self.by_action = true;
            }
            setting::BY_ACTION => self.by_action = on,
            // What shows nothing: how round it is is what it was, and the flag
            // is not read. Kept as given, like what is not known.
            setting::RADIUS | setting::FLAG_56 => {
                self.other.insert(what, value);
            }
            _ => {
                self.other.insert(what, value);
            }
        }
    }

    /// Its face is another from now on, come to in so many frames of the game.
    fn show_face(&mut self, face: i32, frames: i32) {
        self.expression = Some((face, frames.max(0)));
        self.expressions = self.expressions.wrapping_add(1);
    }

    /// It goes over to the face a script has named for it in the frames named
    /// (action 12, 0x1095D4: `shift_face` with its ints 0xB and 0xC).
    pub fn shift_face(&mut self) {
        let named = |what: i32| self.other.get(&what).copied().unwrap_or(0);
        self.show_face(named(setting::FACE_NEXT), named(setting::FACE_FRAMES));
    }

    /// The animation a script has given it for standing, running or walking
    /// (settings 28 to 30), if one has.
    pub fn motion_of(&self, what: i32) -> Option<usize> {
        self.other.get(&what).and_then(|&number| usize::try_from(number).ok())
    }

    /// Whether the animation a script starts on it loops (setting 27, its flag 8).
    fn motion_loops(&self) -> bool {
        self.other.get(&setting::MOTION_LOOPS).is_some_and(|&value| value != 0)
    }

    /// A script names the animation the character is to play when it has nothing
    /// else to do, or none: it loops if setting 27 says so. (The original only
    /// names it here, 0x1071A0, and plays it when it is set playing; the port
    /// shows it from the first, which is what an event's script goes by.)
    pub fn name_motion(&mut self, motion: Option<Motion>) {
        self.named = motion;
        self.motion = motion.map(|motion| (motion, self.motion_loops()));
    }

    /// A script names an animation and does not set it playing, which is what a
    /// skipped event leaves of one.
    pub fn name_only(&mut self, motion: Option<Motion>) {
        self.named = motion;
    }

    /// Sets the animation that has been named playing from its start (`s70`,
    /// 0xF4EEC, 0xF7CF4): what it was doing ends and it is held no more. One
    /// that loops goes on until something else is asked of it; with any other
    /// it is busy until the animation has played through, `end` of the field's
    /// units of time if that is known, and then stands again unless it keeps
    /// its pose. (One that is hidden has no frames of its own, and is not kept
    /// busy by what it would not come to the end of.)
    pub fn play_motion(&mut self, end: Option<i32>) {
        let loops = self.motion_loops();
        self.begin_motion(loops, loops, end);
    }

    /// Sets it playing for so many frames of the game (`s65`, 0xE3364, 0xF4F20):
    /// the animation is played once through whatever it would do, and the
    /// character is busy with it for that long.
    pub fn play_motion_for(&mut self, frames: i32) {
        self.begin_motion(false, self.motion_loops(), Some(frames * FULL_PACE));
    }

    fn begin_motion(&mut self, looped: bool, free: bool, end: Option<i32>) {
        let hidden = self.hidden();
        self.stop();
        self.held = None;
        self.motions = self.motions.wrapping_add(1);
        self.motion = self.named.map(|motion| (motion, looped));
        if self.motion.is_some() && !free && !hidden {
            self.action = Action::Motion { time: 0, end };
            self.flags.set(flag::ACTING, true);
        }
    }

    /// Has its animation be at a frame of the game and stay there (`s64`,
    /// 0xE3400).
    pub fn hold_motion(&mut self, frame: i32) {
        self.held = Some(frame);
    }

    /// The frames of the game it takes to go over from one animation to the next
    /// (setting 33).
    pub fn blend_frames(&self) -> i32 {
        self.other.get(&setting::BLEND_FRAMES).copied().unwrap_or(DEFAULT_BLEND_FRAMES)
    }

    /// One of its int parameters, by the original's number (0x108018). Those nothing
    /// here keeps read as 0.
    pub fn int(&self, number: i32) -> i32 {
        match number {
            int::MOVEMENT => self.mind.movement,
            int::HIDE_TIMER => self.mind.hide_timer,
            int::UNAWARE_TIMER => self.mind.unaware_timer,
            int::GROUP => self.other.get(&setting::GROUP).copied().unwrap_or(0),
            int::MOVE_STATUS => self.move_status,
            int::STATUS => self.status,
            int::TARGET => self.mind.target,
            int::PACE => self.pace,
            int::OWN_PACE => self.own_pace,
            _ => 0,
        }
    }

    /// Sets one of its int parameters (0x10A720). Returns whether it is one that is
    /// kept here.
    pub fn set_int(&mut self, number: i32, value: i32) -> bool {
        match number {
            // (0xFA124.) It is on its way again, and one that patrols or wanders
            // has its time begin again.
            int::MOVEMENT => {
                self.mind.movement = value;
                self.mind.on_its_way = true;
                if value == 1 || value == 2 {
                    self.mind.wander_time = 0;
                }
            }
            int::HIDE_TIMER => self.mind.hide_timer = value,
            int::UNAWARE_TIMER => self.mind.unaware_timer = value,
            int::GROUP => {
                self.other.insert(setting::GROUP, value);
            }
            int::MOVE_STATUS => self.move_status = value,
            int::STATUS => self.status = value,
            int::TARGET => self.mind.target = value,
            int::PACE => self.pace = value,
            int::OWN_PACE => self.own_pace = value,
            _ => return false,
        }
        true
    }

    /// Reads one of the things scripts read by number with `s143` (0xC8054): places
    /// are cut to whole units and angles to whole degrees.
    pub fn get(&self, what: i32) -> i32 {
        match what {
            setting::X => self.position.x as i32,
            setting::Y => self.position.y as i32,
            setting::Z => self.position.z as i32,
            setting::FACING => self.facing().to_degrees() as i32,
            setting::TURN_X | setting::TURN_Z => self.other.get(&what).copied().unwrap_or(0),
            // Nought while the neck does not turn the head, else the number its
            // way was set by (0xC8258).
            setting::NECK if !self.neck.on => 0,
            setting::NECK => self.neck.way as i32 + 1,
            setting::TARGET_X => self.target.x as i32,
            setting::TARGET_Y => self.target.y as i32,
            setting::TARGET_Z => self.target.z as i32,
            setting::TARGET_FACING => self.target_facing.to_degrees() as i32,
            setting::RESOURCE => self.chr,
            _ => 0,
        }
    }
}

/// The frames of the game a character takes to go over from one animation to the
/// next until a script says (0x4000 of its time units).
const DEFAULT_BLEND_FRAMES: i32 = 8;

/// The numbers scripts make and read a character's settings by.
pub mod setting {
    pub const HIDDEN: i32 = 0;
    /// What cross does with it when she is by it, as the word of the sign that
    /// says so ([`prompt`]): pressed, it calls the map's script. The next is
    /// square's, which no script uses.
    pub const CONTACT: i32 = 2;
    pub const CONTACT_SQUARE: i32 = 3;
    /// Whether a button does anything with it at all (0xE374C): off for what is
    /// only there to be seen, and for a box once it is open.
    pub const BY_ACTION: i32 = 5;
    /// Whether cross, and square, has anything to do with it (0xC75CC).
    pub const PROMPT_ON: i32 = 0x24;
    pub const PROMPT_ON_SQUARE: i32 = 0x25;
    pub const NO_DROP: i32 = 8;
    /// How near she has to come to be in touch with it (0xE43D8).
    pub const TOUCH_RADIUS: i32 = 9;
    /// The status it moves in when it is told to walk somewhere.
    pub const MOVE_STATUS: i32 = 10;
    pub const X: i32 = 11;
    pub const Y: i32 = 12;
    pub const Z: i32 = 13;
    /// Its turn about the vertical, in degrees; 14 and 16 are its other two
    /// turns, about the axis across and the one ahead.
    pub const TURN_X: i32 = 14;
    pub const FACING: i32 = 15;
    pub const TURN_Z: i32 = 16;
    pub const TARGET_X: i32 = 17;
    pub const TARGET_Y: i32 = 18;
    pub const TARGET_Z: i32 = 19;
    pub const TARGET_FACING: i32 = 21;
    /// Read only: the number of the resource it was made from.
    pub const RESOURCE: i32 = 23;
    /// Whether its time stands still (0xE4758: its flag 0x1B, by which it has
    /// none of a frame, 0x10827C): a scene has the fallen lie so.
    pub const STOPPED: i32 = 24;
    /// How fast its time runs, in percent.
    pub const PACE: i32 = 25;
    /// Whether the animation a script starts on it loops.
    pub const MOTION_LOOPS: i32 = 27;
    /// The animations it stands in, runs in and walks in (its ints 0x21 to 0x23,
    /// 0x10A920 and 0xFE4AC: its + 0x6D0, + 0x6CC and + 0x6C8, which 0xFD960
    /// reads by its status). Until a script says they are 0, 11 and 10
    /// (0xFCA1C), no character's own script having a `get_stat_anim` to say
    /// otherwise (0xFFDE0).
    pub const STAND_MOTION: i32 = 28;
    pub const RUN_MOTION: i32 = 29;
    pub const WALK_MOTION: i32 = 30;
    /// How many frames of the game it takes to go over from one animation to the
    /// next (0x1084E8); eight unless a script says.
    pub const BLEND_FRAMES: i32 = 33;
    /// The behaviour script that runs it; none if negative.
    pub const BEHAVIOUR: i32 = 35;
    /// How a symbol moves about when nothing is going on (its int parameter 0,
    /// by way of 0x26): 1 patrols, 2 wanders. And how many points its route has.
    pub const MOVEMENT: i32 = 38;
    pub const ROUTE_LENGTH: i32 = 39;
    pub const SYMBOL: i32 = 40;
    /// Its steps make no sound (its flag 0x28).
    pub const NO_STEPS: i32 = 41;
    pub const LIGHT: i32 = 34;
    pub const KEEPS_POSE: i32 = 47;
    /// Its neck: how its head is turned and the three angles that is by, the
    /// character it looks at, how the head is to be turned next and the angles
    /// and the character of that.
    pub const NECK: i32 = 48;
    pub const NECK_X: i32 = 49;
    pub const NECK_Y: i32 = 50;
    pub const NECK_Z: i32 = 51;
    pub const NECK_OBJECT: i32 = 60;
    pub const NECK_NEXT: i32 = 61;
    pub const NECK_NEXT_X: i32 = 62;
    pub const NECK_NEXT_Y: i32 = 63;
    pub const NECK_NEXT_Z: i32 = 64;
    pub const NECK_NEXT_OBJECT: i32 = 68;
    /// Zero keeps it taking part however far away it is.
    pub const ALWAYS_ACTIVE: i32 = 72;
    /// Its gimmicks are off (its flag 0x9D): a scene has its cast so while it
    /// plays them animations that move their hair and faces themselves, and
    /// lets them go on again to give one a face.
    pub const NO_LAYERS: i32 = 114;
    /// Its face: this one at once (its int 0xA, which calls its script's
    /// `set_face` as it is set, 0x10A7C4); and the face it is to go over to and
    /// in how many frames (its ints 0xB and 0xC), which action 12 has it do.
    pub const FACE: i32 = 116;
    pub const FACE_NEXT: i32 = 117;
    pub const FACE_FRAMES: i32 = 118;
    /// Gives it the sign a trap can have, the round with the corners that
    /// turn about it (0xE5FF8, 0x107710: the figure is kept at its + 0x4B4 and
    /// goes to the sign with its place, where nothing was found to use it).
    /// The library gives every container one.
    pub const SIGN: i32 = 92;
    /// Whether the tile with the chevron stands by its mark (its int 0x11,
    /// 0x10A854: its flag 0xA5): a mission's script has it for the enemy it is
    /// after.
    pub const POINTED_OUT: i32 = 122;
    /// Its place in the order the characters are drawn in (its int 0x12,
    /// 0x10A890: its + 0x4FC and its flag 0xA6; under nought it has none and
    /// goes by how far it is, 0x1169B4). The port draws by how far each pixel
    /// is, so the order shows nothing.
    pub const DRAWN_ORDER: i32 = 123;
    /// Whether the effects it carries are drawn by the lens that sees from
    /// close by (its flag 0xA7).
    pub const CLOSE_LENS: i32 = 124;
    pub const OWN_PACE: i32 = 75;
    /// Whether the weapon it carries is shown (0xE58B8).
    pub const WEAPON: i32 = 76;
    /// Whether it has a shadow (0xC7BA8: its flag 0x61 is set with nought): the
    /// things a scene stands about have none.
    pub const SHADOW: i32 = 77;
    /// How far its shadow is put from where it would lie: up, and along the
    /// world's x and z (0xE5B04 on: its + 0x440, + 0x444 and + 0x448, which the
    /// shadow's draw adds to its place, 0x107B54). A scene lifts one that
    /// would lie in a floor, and pushes one under what leans.
    pub const SHADOW_UP: i32 = 80;
    pub const SHADOW_X: i32 = 81;
    pub const SHADOW_Z: i32 = 82;
    /// How far out it is round, for what it stands in the way of (0xE53F4: its
    /// + 0x170). Every map gives her 40, which is what the field's figures
    /// have for everyone: nothing changes.
    pub const RADIUS: i32 = 70;
    /// Its flag 0x56, set with nought (0xE542C): nothing has been found to
    /// read it.
    pub const FLAG_56: i32 = 71;
    /// Whether it idles when it has stood a while (0xE5924: its flag 0x62 is
    /// set with nought): a scene has its cast not do so.
    pub const IDLES: i32 = 78;
    /// How large it is, in hundredths of its own size (its decimal 0x1D).
    pub const SCALE: i32 = 88;
    pub const TOUCHABLE: i32 = 112;
    /// Its int parameter 8, which an enemy that pops up takes over from the player:
    /// which of the field's sixteen frames it is in (the field's + 0x4C60), by
    /// whose turns everything in it is turned (0x109128). All of them are as
    /// the world is but where a map's script turns one (`s255`), which only the
    /// sixth stage's do.
    pub const GROUP: i32 = 115;
    /// An event's script names an animation with these, as a map's does with `s62`,
    /// `s63` and `s265`: one of the character's own, the one an event keeps for its
    /// kind, and one of a slot of the event's cast, the slot being set first.
    pub const MOTION: i32 = 120;
    pub const MOTION_OF_KIND: i32 = 121;
    pub const MOTION_SLOT: i32 = 127;
    pub const MOTION_OF_SLOT: i32 = 128;
    /// The character a symbol is after.
    pub const TARGET: i32 = 126;
    pub const NOTICE_RANGE: i32 = 129;
    /// How far from home one that wanders goes (decimal 0x2C), and how many
    /// frames one that patrols stops at each point of its route.
    pub const WANDER_RADIUS: i32 = 131;
    pub const ROUTE_PAUSE: i32 = 132;
    /// Zero has a symbol chase without its dashes.
    pub const DASHES: i32 = 133;
    pub const TERRITORY: i32 = 134;
}

/// A character's int parameters, by the original's numbers (0x108018 reads one and
/// 0x10A720 sets one).
pub mod int {
    /// How a symbol moves about when nothing is going on.
    pub const MOVEMENT: i32 = 0;
    /// Counts down to hiding it.
    pub const HIDE_TIMER: i32 = 6;
    /// Counts down to its noticing things again.
    pub const UNAWARE_TIMER: i32 = 7;
    /// What setting 115 sets.
    pub const GROUP: i32 = 8;
    pub const MOVE_STATUS: i32 = 0x0e;
    pub const STATUS: i32 = 0x10;
    /// The character a symbol is after.
    pub const TARGET: i32 = 0x18;
    pub const PACE: i32 = 0x28;
    pub const OWN_PACE: i32 = 0x29;
}

/// What a character starts with, from the field's table of tunables (0x109728 and,
/// for what a behaviour script keeps, 0xF9A00).
#[derive(Clone, Copy, Debug)]
pub struct Defaults {
    pub touch_radius: f32,
    pub territory: f32,
    /// In percent.
    pub pace: i32,
    pub notice_range: f32,
    pub wander_radius: f32,
}

/// A box in the map that the player can walk into (0xED4CC).
#[derive(Clone, Copy, Debug)]
pub struct Trap {
    pub kind: TrapKind,
    pub centre: Vec3,
    /// How far it reaches to either side of its centre along its own axes.
    pub reach: Vec3,
    /// Its turn about the vertical, in radians.
    pub turn: f32,
    /// Its other two turns, about the axis across and the one ahead, which only
    /// `s257` gives a box (the box's + 4 and + 0xC).
    pub tilt: Vec2,
    /// Which of the field's frames its box is in (the box's + 1, `s45(id, 39,
    /// n)`): its turns are turned again by that frame's (0x117B6C). And the
    /// frame it takes her into when it has her leap (the trap's + 8, which
    /// setting 39 gives too and `s45(id, 40, n)` after it; 0xD3F10).
    pub frame: u8,
    pub takes_into: Option<u8>,
    /// A figure of its own that a script can set (its + 0x430, `s45(id, 41, n)`):
    /// while it is, the trap is passed over where the one she has walked into is
    /// looked for (0x118484).
    pub not_walked_into: bool,
    /// Off: walking into it does nothing (`s45(id, 4, 1)`).
    pub off: bool,
    /// The player was inside it at the last look, so that it fires on the way in only.
    pub inside: bool,
    /// What it gives the camera while she is inside it.
    pub camera: TrapCamera,
    /// The traps of a map are gone through in the order they were made.
    pub order: u32,
    /// Walking into it calls the map's script (its flag 2). A script that has a
    /// button do it instead (`s45(id, 2, 1)`) takes this away.
    pub on_touch: bool,
    /// A button does something with it while she is in it (its contact's flag
    /// 0), and what: the first two records of its contact, cross's and square's.
    pub by_action: bool,
    pub prompts: [Prompt; 2],
    /// How far from the way its box faces she may be turned and still be taken
    /// to be in it, in radians (`s45(id, 31, degrees)`, its + 0x110); half a turn
    /// is any way.
    pub facing: f32,
    /// It has a sign that shows where it is while she is near (`s45(id, 37,
    /// n)`, its flag 0x19, with the figure it was given), and that sign is not
    /// to be shown for now (`s45(id, 36, on)`, its flag 0x1A).
    pub sign: Option<i32>,
    pub sign_hidden: bool,
    /// How many frames of the game the sign has had (0xD3D80): its set goes
    /// on from when it is made, seen or not.
    pub sign_frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapKind {
    /// Leads to another map by one of its entrances.
    MapChange { map: i32, entrance: i32 },
    Event,
    Camera,
}

/// A box that nothing walks through, put up by the map's script: in the original a
/// character with no model whose collision shape is the box (`s185`, `s116`).
#[derive(Clone, Copy, Debug)]
pub struct Wall {
    pub centre: Vec3,
    /// How far it reaches to either side of its centre along its own axes.
    pub reach: Vec3,
    /// Its turn about the vertical, in radians.
    pub turn: f32,
    /// Not in the way for now: it is hidden, or told not to be solid (0x108B18).
    pub open: bool,
}

impl Wall {
    pub fn new() -> Wall {
        Wall { centre: Vec3::ZERO, reach: Vec3::splat(DEFAULT_REACH), turn: 0.0, open: false }
    }

    /// Gives the wall its box; a size that is not positive is taken as 100 (0xC4E54).
    pub fn shape(&mut self, centre: Vec3, reach: Vec3, turn: f32) {
        self.centre = centre;
        self.reach = Vec3::select(reach.cmpgt(Vec3::ZERO), reach, Vec3::splat(DEFAULT_REACH));
        self.turn = turn;
    }

    /// Where a body of `radius` standing at `feet` is moved to so as not to be in
    /// the wall: out by the nearest of its four sides. A stand-in for the original's
    /// test of the two collision shapes against each other.
    pub fn push_out(&self, feet: Vec3, radius: f32) -> Vec3 {
        let local = Quat::from_rotation_y(-self.turn) * (feet - self.centre);
        let (reach_x, reach_z) = (self.reach.x + radius, self.reach.z + radius);
        if self.open || local.y.abs() > self.reach.y + radius || local.x.abs() >= reach_x || local.z.abs() >= reach_z {
            return feet;
        }
        let (out_x, out_z) = (reach_x - local.x.abs(), reach_z - local.z.abs());
        let moved = if out_x < out_z {
            Vec3::new(reach_x.copysign(local.x), local.y, local.z)
        } else {
            Vec3::new(local.x, local.y, reach_z.copysign(local.z))
        };
        self.centre + Quat::from_rotation_y(self.turn) * moved
    }
}

/// The reach a trap's box has along an axis it was given no positive size for.
const DEFAULT_REACH: f32 = 100.0;

impl Trap {
    pub fn new(kind: TrapKind) -> Trap {
        Trap {
            kind,
            centre: Vec3::ZERO,
            reach: Vec3::splat(DEFAULT_REACH),
            turn: 0.0,
            tilt: Vec2::ZERO,
            frame: 0,
            takes_into: None,
            not_walked_into: false,
            off: false,
            inside: false,
            camera: TrapCamera::default(),
            order: 0,
            // As an event's trap is made (0xC43A8): walked into it calls the
            // script, and cross has ACTION. The other kinds are left to be
            // walked into, as the port has had them.
            on_touch: true,
            by_action: kind == TrapKind::Event,
            prompts: [Prompt::of(prompt::ACTION), Prompt::NONE],
            facing: std::f32::consts::PI,
            sign: None,
            sign_hidden: false,
            sign_frames: 0,
        }
    }

    /// The word of the sign for what a button does with it, if anything: cross's
    /// (0) or square's (1).
    pub fn prompt(&self, slot: usize) -> Option<i32> {
        self.prompts.get(slot).filter(|prompt| self.by_action && prompt.on).map(|prompt| prompt.kind)
    }

    /// Whether one who faces `facing` is turned the way the box does, near enough
    /// (0x117274): the angle between the two, both taken level, is not more than
    /// what the trap allows. A trap that allows half a turn takes any.
    pub fn faced(&self, facing: f32) -> bool {
        let level = |turn: f32| Quat::from_rotation_y(turn) * Vec3::Z;
        self.facing >= std::f32::consts::PI || level(facing).dot(level(self.turn)).clamp(-1.0, 1.0).acos() <= self.facing
    }

    /// Whether she is in touch with it (0x10DF38): it is on, she is in its box,
    /// and she is turned as it asks.
    pub fn touched(&self, at: Vec3, facing: f32, frames: &Frames) -> bool {
        !self.off && self.contains(at, frames) && self.faced(facing)
    }

    /// Gives the trap its box; a size that is not positive is taken as 100 (0xC3C00).
    pub fn shape(&mut self, centre: Vec3, reach: Vec3, turn: f32) {
        self.shape_turned(centre, reach, Vec3::new(0.0, turn, 0.0));
    }

    /// `s257` (0xC3CC8, 0xE1FB4): the same with all three of the box's turns, about
    /// the axis across, the upright one and the one ahead.
    pub fn shape_turned(&mut self, centre: Vec3, reach: Vec3, turns: Vec3) {
        self.centre = centre;
        self.reach = Vec3::select(reach.cmpgt(Vec3::ZERO), reach, Vec3::splat(DEFAULT_REACH));
        (self.turn, self.tilt) = (turns.y, Vec2::new(turns.x, turns.z));
    }

    /// Whether `point` is inside the box (0xED640): taken into the box's own frame,
    /// each of its coordinates is within the box's reach. The box is turned by its
    /// three turns, and that by the turn of the field's frame it is in (0x117B6C).
    pub fn contains(&self, point: Vec3, frames: &Frames) -> bool {
        let turn = frames.turn(self.frame) * turn_of(Vec3::new(self.tilt.x, self.turn, self.tilt.y));
        let local = turn.inverse() * (point - self.centre);
        (local / self.reach).abs().cmple(Vec3::ONE).all()
    }
}

/// A battle the map's script has asked for (`s81`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleRequest {
    /// The enemy group to fight.
    pub group: i32,
}

/// How the last battle ended, as the script reads it (`s156` and `s157`: the words at
/// + 4 and + 8 of the record at 0x2EAF8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BattleReport {
    /// 1 won, 2 lost, 3 got away.
    pub outcome: i32,
    /// 3 after a battle won (0xA022C), 2 after one lost (0x78E34); the scripts only
    /// look at it after getting away, for 1.
    pub second: i32,
}

impl BattleReport {
    pub const WON: BattleReport = BattleReport { outcome: OUTCOME_WON, second: 3 };
    pub const LOST: BattleReport = BattleReport { outcome: OUTCOME_LOST, second: 2 };
}

pub const OUTCOME_WON: i32 = 1;
pub const OUTCOME_LOST: i32 = 2;
/// The port's battles cannot be got away from yet, so only tests report this.
#[cfg(test)]
pub const OUTCOME_ESCAPED: i32 = 3;

/// The kinds of the screen's fade that are ported, by the numbers the original
/// starts them with (0x297E4, 0x2B6EC). A sheet of black comes off the screen or
/// goes over it, or one of white; a sheet of black comes off from half, or goes
/// over by half; and the picture of the screen that the fade keeps goes, showing
/// what is under it, or stays while it darkens. (Not here: 0x100 and 0x200,
/// which come and go in rows and are the title's, `title/rows.rs`; not ported,
/// the family of 0x800, in which the picture kept is moved about.)
pub mod fade_kind {
    pub const IN: i32 = 1;
    pub const OUT: i32 = 2;
    pub const FROM_HALF: i32 = 4;
    pub const TO_HALF: i32 = 8;
    pub const IN_WHITE: i32 = 0x11;
    pub const OUT_WHITE: i32 = 0x12;
    pub const PICTURE_GOES: i32 = 0x803;
    pub const PICTURE_STAYS: i32 = 0x804;
}

/// The most the fade's own level goes to, and how far a sheet that goes over by
/// half does (0x2A5E8).
const WHOLE: f32 = 255.0;
const HALF: f32 = 128.0;
/// The frames of the interface over which the picture of a map that is left
/// darkens (0x11608C); a frame of the game is two of them.
const LEFT_DARKENS: i32 = 0x78;

/// The picture of the screen the fade keeps (the object at 0x2C100, a part of the
/// "Trans" task): the screen as it was when the picture was taken, drawn over the
/// scene and under the field's own interface (0x298DC). What is here is how much
/// of it is shown; the picture itself is the engine's (`canvas::Captured`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kept {
    state: KeptState,
    /// It is as bright as it was taken, whatever its darkening has come to (its
    /// flag 1, 0x2C298).
    whole: bool,
    /// The frames its going takes and those gone by (+ 8, + 0x10).
    going: (i32, i32),
    /// The frames its darkening takes and those gone by (+ 0xC, + 0x14).
    darkening: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeptState {
    Off,
    /// It goes: there is less and less of it (0x2C244).
    Going,
    /// It stands as it is (0x2C2C0).
    Held,
    /// It stays, and darkens (0x2C258).
    Darkening,
}

impl Kept {
    /// As the game starts (0x2C100).
    pub const OFF: Kept = Kept { state: KeptState::Off, whole: false, going: (5, 0), darkening: (5, 0) };

    /// It is shown as it was taken and stands (0x2C298 with 0, 0x2C2C0 with 1).
    fn hold(&mut self) {
        self.whole = true;
        self.going = (1, 0);
        self.state = KeptState::Held;
    }

    fn go(&mut self, frames: i32) {
        self.going = (frames, 0);
        self.state = KeptState::Going;
    }

    fn darken(&mut self, frames: i32) {
        self.darkening = (frames, 0);
        self.going.1 = 0;
        self.state = KeptState::Darkening;
    }

    /// Whether it is on its way somewhere (0x2C270).
    fn moving(&self) -> bool {
        matches!(self.state, KeptState::Going | KeptState::Darkening)
    }

    /// A frame of it (0x2C13C).
    fn step(&mut self) {
        match self.state {
            KeptState::Going => {
                self.darkening.1 = (self.darkening.1 + 1).min(self.darkening.0);
                self.going.1 += 1;
                if self.going.1 >= self.going.0 {
                    self.going.1 = self.going.0;
                    self.state = KeptState::Off;
                }
            }
            KeptState::Darkening => {
                self.darkening.1 += 1;
                if self.darkening.1 >= self.darkening.0 {
                    self.darkening.1 = self.darkening.0;
                    self.state = KeptState::Held;
                }
            }
            KeptState::Off | KeptState::Held => {}
        }
    }

    /// The colour it is drawn by, if it is drawn (0x2C430 to 0x2C494): how bright
    /// in its red, green and blue, and how much of it there is in its alpha.
    fn colour(&self) -> Option<[u8; 4]> {
        if self.state == KeptState::Off {
            return None;
        }
        let left = |(frames, gone): (i32, i32)| ((frames - gone) * 255).checked_div(frames).unwrap_or(0).clamp(0, 255) as u8;
        let bright = if self.whole { 255 } else { left(self.darkening) };
        Some([bright, bright, bright, left(self.going)])
    }
}

/// What the list of saves is asked for (0x111420): to save the game, or to load
/// one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavesAsked {
    Save = 0,
    Load = 1,
}

/// A picture of the screen that a script has asked to be taken (the field's flag
/// 0x17, set by 0xE4790 and 0xE47C0 and cleared where the picture is taken,
/// 0x10F358 and 0x10EA18): of the scene alone, or of everything on the screen.
/// The script waits until it has been.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Capture {
    #[default]
    None,
    Wanted {
        whole: bool,
    },
    /// It has been asked of the graphics card and has yet to come.
    Pending,
}

/// A film a script has had made ready, and what has become of it. The original
/// makes a task for it, "Movie" (0xC3170), which hands the film to the PSP's own
/// player (0x11863C); the script then waits for the player to be ready (`s84`,
/// 0x1188A4), starts it (`s85`, 0x1188E4) and waits for it to be over (`s86`,
/// 0x1189F0). Here the script says what it wants in this, and what plays films
/// (`crate::film`) says in it how far it has got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilmAsked {
    /// Its file as the script names it: `movie/BRS_OP_final.pmf`.
    pub file: String,
    /// It can be started.
    pub ready: bool,
    /// The script has started it.
    pub started: bool,
    /// It has played through, or cannot be played.
    pub over: bool,
}

impl FilmAsked {
    pub fn new(file: String) -> FilmAsked {
        FilmAsked { file, ready: false, started: false, over: false }
    }
}

/// The screen's fade (the original's "Trans" task, 0x2A30C): a sheet of black or
/// of white over everything but a scene's captions, and the picture of the screen
/// it keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    /// How much of the screen the sheet covers: from 0, the picture as it is, to
    /// 1, nothing but the sheet's colour.
    pub level: f32,
    /// What the level changes by each frame until it gets to where it is going.
    pub step: f32,
    /// The same two as the original keeps them (+ 4, + 0xC), the level from 0 to
    /// 255: it is these that are added up, so that a fade takes the frames the
    /// original's takes; the two above are made of them.
    value: f32,
    pace: f32,
    pub white: bool,
    /// The kind it was last started as: it is what says whether the screen counts
    /// as covered (0x29974), and what a fade of the kind last used is.
    kind: i32,
    picture: Kept,
}

impl Fade {
    /// Nothing over the screen.
    pub const CLEAR: Fade = Fade { level: 0.0, step: 0.0, value: 0.0, pace: 0.0, white: false, kind: fade_kind::IN, picture: Kept::OFF };
    /// The screen all black, as a fade out leaves it.
    pub const DARK: Fade = Fade { level: 1.0, step: 0.0, value: WHOLE, pace: 0.0, white: false, kind: fade_kind::OUT, picture: Kept::OFF };

    /// True while it is on its way somewhere (0x29948 answers the other way).
    pub fn running(&self) -> bool {
        match self.kind {
            fade_kind::PICTURE_GOES | fade_kind::PICTURE_STAYS => self.picture.moving(),
            _ => self.step != 0.0,
        }
    }

    /// The colour the picture of the screen is drawn by, if it is drawn: it is
    /// only while the fade is of one of the picture's kinds (0x298DC).
    pub fn picture(&self) -> Option<[u8; 4]> {
        match self.kind {
            fade_kind::PICTURE_GOES | fade_kind::PICTURE_STAYS => self.picture.colour(),
            _ => None,
        }
    }

    /// Whether the screen counts as covered (0x29974): by the kind last started,
    /// however far that has come.
    pub fn covered(&self) -> bool {
        matches!(self.kind, fade_kind::OUT | fade_kind::TO_HALF | fade_kind::OUT_WHITE | fade_kind::PICTURE_STAYS)
    }

    /// Starts a fade of a kind over so many frames of the game (0x297E4, which is
    /// given the interface's frames and makes the game's of them; 0x2B6EC). Each
    /// starts from all or from nothing whatever was there. A kind that is not
    /// ported starts nothing.
    pub fn begin(&mut self, kind: i32, frames: i32) {
        let frames = frames.max(1);
        // The pace is the whole of the level over the frames, whichever way and
        // however far it goes (0x2B7F0 on).
        let sheet = |from: f32, out: bool, white: bool| (from, if out { WHOLE / frames as f32 } else { -(WHOLE / frames as f32) }, white);
        let (value, pace, white) = match kind {
            fade_kind::IN => sheet(WHOLE, false, false),
            fade_kind::OUT => sheet(0.0, true, false),
            fade_kind::IN_WHITE => sheet(WHOLE, false, true),
            fade_kind::OUT_WHITE => sheet(0.0, true, true),
            fade_kind::FROM_HALF => sheet(HALF, false, false),
            fade_kind::TO_HALF => sheet(0.0, true, false),
            // With a kind of the picture's no sheet is drawn (0x2ADC0).
            fade_kind::PICTURE_GOES => {
                self.picture.go(frames);
                (0.0, 0.0, false)
            }
            fade_kind::PICTURE_STAYS => {
                self.picture.darken(frames);
                (0.0, 0.0, false)
            }
            _ => return,
        };
        (self.value, self.pace, self.white, self.kind) = (value, pace, white, kind);
        (self.level, self.step) = (value / WHOLE, pace / WHOLE);
    }

    /// Starts the sheet coming off the screen, or going over it, whatever it was
    /// at: black or white, over `frames` frames.
    pub fn restart(&mut self, out: bool, white: bool, frames: i32) {
        let kind = match (out, white) {
            (false, false) => fade_kind::IN,
            (true, false) => fade_kind::OUT,
            (false, true) => fade_kind::IN_WHITE,
            (true, true) => fade_kind::OUT_WHITE,
        };
        self.begin(kind, frames);
    }

    /// `s7(way, colour, frames)` (0xC6FE0): way 0 brings the screen back and way 1
    /// takes it away, the first only if it counts as covered and the second only
    /// if it does not. Colour 0 is the black sheet, 1 the white one and 2 the
    /// picture of the screen, which a fade out has stay and darken and a fade in
    /// has go; -1 is whichever was last used. Answers whether a picture of the
    /// screen is to be taken for it.
    pub fn script(&mut self, way: i32, colour: i32, frames: i32) -> bool {
        use fade_kind::*;
        let kind = match (way, colour, self.covered()) {
            (0, 0, true) => IN,
            (0, 1, true) => IN_WHITE,
            (0, 2, true) => PICTURE_GOES,
            (0, -1, true) => match self.kind {
                PICTURE_STAYS => PICTURE_GOES,
                OUT_WHITE => IN_WHITE,
                OUT => IN,
                _ => return false,
            },
            (1, 0, false) => OUT,
            (1, 1, false) => OUT_WHITE,
            (1, 2, false) => PICTURE_STAYS,
            (1, -1, false) => match self.kind {
                PICTURE_GOES => PICTURE_STAYS,
                IN_WHITE => OUT_WHITE,
                IN => OUT,
                _ => return false,
            },
            _ => return false,
        };
        self.begin(kind, frames);
        kind == PICTURE_STAYS
    }

    /// The picture of the screen is put up as it was taken, and stands (0xE6B28,
    /// 0xE4838): what a battle leaves the field with, and a scene its last shot,
    /// for a script to have go.
    pub fn hold_picture(&mut self) {
        *self = Fade { kind: fade_kind::PICTURE_STAYS, ..Fade::CLEAR };
        self.picture.hold();
    }

    /// The picture of the screen, as it was taken, goes over so many frames of the
    /// game (0xE47F4): what is under it shows through more and more, which is how
    /// one shot of a scene gives way to the next.
    pub fn dissolve(&mut self, frames: i32) {
        self.picture.whole = true;
        self.begin(fade_kind::PICTURE_GOES, frames);
    }

    /// The picture of the screen stays up and darkens over so many frames of the
    /// game, whatever it was last put up as (0x11607C: 0x2C298 with 1, and then
    /// the kind 0x804): what the screen is left with as a map is left.
    pub fn darken_picture(&mut self, frames: i32) {
        self.picture.whole = false;
        self.begin(fade_kind::PICTURE_STAYS, frames);
    }

    /// A frame of it (0x2A4A4).
    pub fn advance(&mut self) {
        if matches!(self.kind, fade_kind::PICTURE_GOES | fade_kind::PICTURE_STAYS) {
            self.picture.step();
            return;
        }
        if self.pace != 0.0 {
            let most = if self.kind == fade_kind::TO_HALF { HALF } else { WHOLE };
            self.value += self.pace;
            if self.value <= 0.0 || self.value >= most {
                self.value = self.value.clamp(0.0, most);
                self.pace = 0.0;
            }
            (self.level, self.step) = (self.value / WHOLE, self.pace / WHOLE);
        }
    }
}

/// The fade of the scene alone that a script asks for (`s197`; the original's task
/// "Fade Priority", 0xD01CC): a sheet of black or of white over the scene and under
/// everything of the interface, which comes or goes evenly over its frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneFade {
    /// The sheet comes (the script's 1) or goes (0); any other number leaves the
    /// scene clear.
    way: i32,
    /// Black (0) or white (1); any other number draws nothing.
    colour: i32,
    /// The frames it takes and those gone by, in the interface's.
    frames: i32,
    elapsed: i32,
    /// It has had a frame since it had all its frames: what `s198` waits for.
    pub over: bool,
}

impl SceneFade {
    /// The interface's frames in one of the game's (0x850).
    const FRAMES_A_STEP: i32 = 2;

    /// `s197(way, colour, frames)`, the frames being the game's (0xC21C0).
    pub fn new(way: i32, colour: i32, frames: i32) -> SceneFade {
        SceneFade { way, colour, frames: frames * Self::FRAMES_A_STEP, elapsed: 0, over: false }
    }

    /// A frame of it (0xD0344).
    pub fn step(&mut self) {
        if self.over {
            return;
        }
        if self.elapsed >= self.frames {
            self.over = true;
        } else {
            self.elapsed += Self::FRAMES_A_STEP;
        }
    }

    /// The colour of the sheet, if it has one (0xD0398): its alpha goes evenly from
    /// none to all, or from all to none.
    pub fn colour(&self) -> Option<[u8; 4]> {
        let (from, to) = match self.way {
            0 => (255.0, 0.0),
            1 => (0.0, 255.0),
            _ => (0.0, 0.0),
        };
        let alpha = (from + (to - from) * (self.elapsed as f32 / self.frames as f32)) as u8;
        match self.colour {
            0 => Some([0, 0, 0, alpha]),
            1 => Some([255, 255, 255, alpha]),
            _ => None,
        }
    }
}

/// The field's own flags, by the original's numbers (0x1176FC sets one and 0x11773C
/// reads one).
pub mod field_flag {
    // Parts of the screen a script opens and closes with `s104` (0xC8AB0): two that
    // are shown while their flag is set, two that are hidden while theirs is. The
    // second is the marks over characters (0x10F278) and the third her gauge
    // (0xEDAAC draws it unless its flag is set); nothing has been found to read the
    // fourth's flag, and the first's is not read where the field draws.
    pub const PART_1_SHOWN: u32 = 0x25;
    pub const PART_2_SHOWN: u32 = 0x27;
    pub const GAUGE_HIDDEN: u32 = 0x51;
    pub const PART_4_HIDDEN: u32 = 0x52;
    /// The tune is left alone when a battle begins and ends: a script sees to it
    /// (`s96(9, ..)`, 0xD8AA8).
    pub const TUNE_KEPT: u32 = 0x2b;
    /// A film is up (0xE68AC sets it as one is made, 0xE68F8 clears it when it is
    /// over): the pause then has no picture of the screen to show (0x10CCA0).
    pub const FILM: u32 = 0x2c;
    /// Nobody that stands idles (`s96(28, 0)`, 0xE5978; read by 0xF9020).
    pub const NO_IDLING: u32 = 0x47;
    /// Every character is drawn however near the camera it is (`s96(12, 0)`,
    /// 0xE4AA8; read by 0x1063B8).
    pub const ALL_IN_SIGHT: u32 = 0x31;
    /// Every character has its frame and is drawn however far from her it is
    /// (`s96(21, 0)`, 0xE4AF4; read by 0x106558 and 0x1066A8).
    pub const ALL_NEAR: u32 = 0x32;
    /// Enemies pop up out of the ground about her as she goes.
    pub const POPS: u32 = 0x54;
    /// The field is to be left: the mission's time has run out, or what she was to
    /// keep safe has been lost.
    pub const LEAVE: u32 = 0x58;
}

/// The field's figures that scripts read and set by number: a map's script with
/// `s96`, the system script with its functions 7 and 0x16 (0xD4108, 0xD6AF4). Most
/// are kept by the field as plain numbers; the rest are worked out where they are
/// read (`field/system.rs`).
pub mod value {
    /// The map she is on; setting it names the map to go to.
    pub const MAP: i32 = 0;
    pub const ENTRANCE: i32 = 1;
    /// Time that goes on while the field is in its normal state.
    pub const ASYNC_TIME: i32 = 37;
    /// The time left until an enemy may pop up.
    pub const POP_TIMER: i32 = 38;
    /// Whether enemies pop up at all ([`super::field_flag::POPS`]).
    pub const POPS: i32 = 40;
    /// The map and entrance the field starts again on when it is left for them
    /// (`s173(2)`).
    pub const NEXT_MAP: i32 = 42;
    pub const NEXT_ENTRANCE: i32 = 43;
    /// Which of the mission's lines says what is to be done now.
    pub const OBJECTIVE: i32 = 49;
    /// The choice made in the window of a mission failed.
    pub const FAILED_CHOICE: i32 = 50;
    /// How the field is left when what she guards is lost.
    pub const LOSS_EXIT: i32 = 52;
    /// The enemies about what she guards.
    pub const ENEMIES_NEAR: i32 = 53;
    pub const GUARD_TIMER: i32 = 55;
    /// The field is to be left ([`super::field_flag::LEAVE`]), and how.
    pub const LEAVE: i32 = 56;
    pub const EXIT: i32 = 57;
    /// The mission's time left, in seconds (variable 10012 holds it in thousandths).
    pub const MISSION_SECONDS: i32 = 58;
    /// The time between enemies popping up, in seconds (variable 10014).
    pub const POP_SECONDS: i32 = 62;
}

/// What the game keeps for the field outside any one map (the work area that 0x241C
/// returns).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Work {
    /// Time units until an enemy may pop up (+ 0x20).
    pub pop_timer: i32,
    /// How far she has gone since the last one did (+ 0x4C).
    pub pop_distance: f32,
    /// Which ride on the bike the field is left for (`s96(41, n)`), and how the
    /// last ride came out, which the script of the map it leads to asks
    /// (`s260`; a byte of the game's, + 0x2073D): [`RIDDEN_THROUGH`], 3 for a ride
    /// given up, anything else for one that failed.
    pub ride: i32,
    pub ride_outcome: i32,
    /// The glare over the scene's picture.
    pub glare: Glare,
    /// Whether the going is heavy, as the game's flag has it ([`HEAVY_GOING`]).
    pub heavy: bool,
}

/// The glare over the scene's picture: a task of the game's own (`Glare`,
/// 0x1AA04), which outlasts any one map and which the maps' scripts speak to. It
/// adds the screen, made small and soft, to the screen by its strength (0x1A6B4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Glare {
    /// How strong a script has said it is, in parts of one (`s153`; the task's
    /// + 0x10). Nothing until one says.
    pub strength: f32,
    /// Only what is marked as it is drawn takes part (`s254(1, 1, ..)`; the
    /// task's bit 4, by which the screen's alpha, its stencil, says what glares:
    /// 0x19F4C). The marks are a count of one or two in 255 (0x19464), and what
    /// is drawn by so little is not seen: nothing glares then.
    pub marked_only: bool,
    /// How strong it is as it is drawn (the task's + 4): it goes a tenth of the
    /// way to what it is to be every frame.
    pub level: f32,
}

impl Glare {
    /// `s153(n)` (0xC18D4): it is `n` in 255 strong, all of it if less than
    /// nothing is given, and all the screen takes part.
    pub fn set(&mut self, strength: i32) {
        self.strength = if strength < 0 { 1.0 } else { strength as f32 / 255.0 };
        self.marked_only = false;
    }

    /// A frame of it (0x1A768): its level goes towards its strength, held
    /// between nothing and one.
    pub fn step(&mut self) {
        self.level += (self.strength.clamp(0.0, 1.0) - self.level) * 0.1;
    }

    /// How strong it is over the picture now.
    pub fn shown(&self) -> f32 {
        if self.marked_only || self.strength <= 0.0 {
            0.0
        } else {
            self.level
        }
    }
}

/// How a ride on the bike that was ridden to its end came out (`s260`).
pub const RIDDEN_THROUGH: i32 = 1;

/// The notice a mission starts with (window 9; 0x48300 puts it up and 0x481A0 is its
/// frame): a band across the middle of the screen with the mission's title. It
/// comes in, stands for as long as its jingle is heard, and goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub text: String,
    /// It is on its way out.
    pub going: bool,
    /// The interface's frames it has been coming, or going, for.
    pub frames: f32,
    /// The frames of its jingle still to be heard.
    jingle_left: i32,
    /// With no jingle: the interface's frames it has stood.
    stood: Option<i32>,
}

impl Notice {
    /// How long it takes to come and to go, in the interface's frames: what the
    /// sets of its layout come to (`field/hud` checks them against it).
    pub const COMING: f32 = 10.0;
    pub const GOING: f32 = 6.0;
    /// The jingle it stands for: the fourth of `jin_pack.bin` (0x48398), and the
    /// frames that lasts.
    pub const JINGLE: &'static str = "SYSTEM.VOL/jin_pack.bin/003.riff";
    pub const JINGLE_FRAMES: i32 = 120;
    /// How long it stands when it has no jingle (0x48244).
    const STANDS: i32 = 180;
    /// The interface's frames that go by in one of the game's (0x850).
    const FRAMES_A_STEP: i32 = 2;

    /// A notice just put up; one that is `silent` has no jingle and stands its time.
    pub fn new(text: String, silent: bool) -> Notice {
        Notice { text, going: false, frames: 0.0, jingle_left: if silent { 0 } else { Self::JINGLE_FRAMES }, stood: silent.then_some(0) }
    }

    /// A frame of it; says whether it is still there.
    pub fn step(&mut self) -> bool {
        let moving = self.frames < if self.going { Self::GOING } else { Self::COMING };
        if !moving {
            if self.going {
                return false;
            }
            let over = match &mut self.stood {
                Some(stood) if *stood < Self::STANDS => {
                    *stood += Self::FRAMES_A_STEP;
                    false
                }
                Some(_) => true,
                None => self.jingle_left <= 0,
            };
            if over {
                self.going = true;
                self.frames = 0.0;
            }
        }
        if self.frames < if self.going { Self::GOING } else { Self::COMING } {
            self.frames += Self::FRAMES_A_STEP as f32;
        }
        self.jingle_left = (self.jingle_left - 1).max(0);
        true
    }
}

/// The windows of the field that scripts open and close by number (0xEDB24).
pub mod window {
    /// The first panel of the mission's display, under either of two numbers, and
    /// its second.
    pub const PANELS: [i32; 3] = [5, 10, 11];
    pub const MISSION_START: i32 = 9;
    pub const MISSION_CLEAR: i32 = 12;
    pub const MISSION_FAIL: i32 = 13;
    pub const PLACE_NAME: i32 = 16;
}

#[derive(Clone, Debug)]
pub struct Scene {
    /// The number of the map in the game's list, and the entrance it was come in by.
    pub map: i32,
    pub entrance: i32,
    pub defaults: Defaults,
    pub characters: BTreeMap<i32, Character>,
    /// The character resources that are loaded, with how many times each was asked for.
    pub resources: BTreeMap<i32, u32>,
    pub traps: BTreeMap<i32, Trap>,
    /// How many traps the map's script has made.
    traps_made: u32,
    /// The places a trap has her taken to, in the order a script gave them
    /// (`s234`: the trap's + 0x128 on), by the trap's number.
    pub taken_to: BTreeMap<i32, Vec<TakenTo>>,
    /// The map's points that the routes of those that patrol go by (`s180`; the
    /// field's + 0x2484): a place, and how far about it one that walks there
    /// may end up.
    pub route_points: BTreeMap<i32, (Vec3, f32)>,
    pub walls: BTreeMap<i32, Wall>,
    /// A battle the script is waiting for.
    pub battle: Option<BattleRequest>,
    pub report: BattleReport,
    pub fade: Fade,
    /// A picture of the screen is to be taken, or is being.
    pub capture: Capture,
    /// The film a script has had made ready, from then (`s83`) until the script
    /// has seen it over (`s86`).
    pub film: Option<FilmAsked>,
    /// The scene's lights (the field's + 0x1600 on): as they are, which a script
    /// changes for a scene of the story (`s164`, `s166`), and as the map's own
    /// scripts left them, which is what such a script puts back (`s165`).
    pub lights: Lights,
    pub lights_kept: Lights,
    /// The field's sixteen frames, which the sixth stage's maps turn (`s255`).
    pub frames: Frames,
    /// The fade of the scene alone, from when a script asks for one (`s197`) until
    /// it has done with it (`s199`).
    pub scene_fade: Option<SceneFade>,
    /// Where the script has sent the game on to, if it has: the number it gave `s173`.
    pub leaving: Option<i32>,
    /// The map and entrance a script has named to go on to (`s68`, `s69`): the field
    /// goes there when the script's event is over.
    pub map_change: Option<(i32, i32)>,
    /// The camera as scripts and the map's traps have it.
    pub camera: Camera,
    /// The cuts of the cameras put by for the time of an event (`s50`, `s51`).
    cuts_kept: Vec<Option<Cut>>,
    /// The field's own flags that are set ([`field_flag`]).
    pub flags: BTreeSet<u32>,
    /// The figures scripts keep with the field by number ([`value`]).
    pub values: BTreeMap<i32, i32>,
    pub work: Work,
    /// Where the next enemy to pop up has been found a place (the field's + 0x5390).
    pub pop_place: Vec3,
    /// An entry of the system script that the map's script has asked for and is
    /// waiting on (`s247`).
    pub system: Option<i32>,
    /// A function of the system script that the map's script has asked for by
    /// its name (`s231(4)`, 0x130CE8), until it has been run.
    pub system_call: Option<&'static str>,
    /// The window of a mission cleared, while it is up; and the title the system
    /// script has given the notice of a mission's start.
    pub cleared: Option<Cleared>,
    pub banner_text: String,
    /// The notice of a mission's start, while it is up.
    pub notice: Option<Notice>,
    /// The window of a mission failed, from when it is opened.
    pub failed: Option<Failed>,
    /// The sign that names the place she has come into, while it is up (window
    /// 16); and the title of a scene's place, which a map's script has made
    /// (`s36`), from then until it has gone.
    pub place_name: Option<PlaceName>,
    pub location: Option<Title>,
    /// The sign of what a button would do with what she is by; and of the last
    /// thing a button did, which button it was (cross 0, square 1) and the word
    /// its sign had (the field's + 0xE78 and + 0xE7C, which `s47` reads).
    pub action_sign: ActionSign,
    pub chosen: (i32, i32),
    /// A map's script has set the figure that has the system script redo the
    /// mission's display (its `mission_update_info`, 0x117D2C).
    pub display_stale: bool,
    /// How near the camera a character's model may be and still be drawn, once a
    /// script has said (`s96(11, n)`, 0xE4A78: the field's + 0x246C); until then
    /// it is the tunables' (0x118238).
    pub too_close: Option<f32>,
    /// The window a script has up for the player to read or to answer, the choice
    /// the cursor was on when the last one closed, and what the last question was
    /// answered with (`s76`).
    pub window: Option<Window>,
    pub selection: i32,
    pub answer: i32,
    /// The pictures a script has over the screen, by slot (`s248`).
    pub pictures: BTreeMap<i32, Picture>,
    /// The events a script has asked to have played (`s237`: the event's number
    /// and what its script is started with), and whether one is being played.
    pub events: VecDeque<(i32, i32)>,
    pub event_running: bool,
    /// The one being played is to be dropped: the event that asked for it has been
    /// skipped.
    pub stop_events: bool,
    /// The window people speak to her through.
    pub comm: Comm,
    /// What an event has brought to stage a scene with (`s59`), until it is let go
    /// (`s61`).
    pub staging: Option<Arc<Staging>>,
    /// The cut the camera is playing, or has played and stays at the end of; none
    /// when the camera is of another kind.
    pub cut: Option<Cut>,
    /// The count of frames a scene is timed by: a script sets it (`s96(8, ..)`),
    /// usually to 1 as it starts a cut, it goes up by one every frame, and the script
    /// waits for it to reach a number (`s103`). The field's + 0x2178.
    pub event_frame: i32,
    /// What is spoken in a scene, while a script has captions up (`s239` to `s241`).
    pub captions: Option<Captions>,
    /// What scripts have asked of the camera's shake, until the field takes it.
    pub shakes: Vec<ShakeAsked>,
    /// What a scene has spoken has begun to be heard (the script's flag 0x1000,
    /// which the sound sets as a recording begins, 0xA8A8), until the script
    /// hushes it (`s30`, 0xC27EC).
    pub voice_begun: bool,
    /// The tune a script last asked for is playing, as the sound has said since
    /// (`s177` waits for it).
    pub tune_heard: bool,
    /// The characters an event's script has told to come to where they stand,
    /// each with the way (`ACT` 9, 10 and 14), until the field has had them do
    /// it: how far down one comes up from is its resource's to say.
    pub coming: Vec<(i32, i32)>,
    /// A script has said the game is cleared, and how many times (`s205`), until
    /// the field has taken it.
    pub game_cleared: Option<i32>,
    /// A panel that tells of a record is up (0x14B29C), which a script can wait
    /// for (`s275`); and the windows of the records of missions are being shown,
    /// from when a script asks for them (`s276`, 0xEF8E8) until all have been.
    pub record_news: bool,
    pub record_windows: bool,
    /// The list of saves a script has asked for (`s178`, `s204`; the field's
    /// + 0x220C and + 0x2218): to save in or to load from, until it has gone.
    pub saves: Option<SavesAsked>,
    /// What the system script has to say on the screen between missions
    /// (0x12FC08), until that screen's window has taken it; and that the
    /// window is still up with it.
    pub said: Option<String>,
    pub saying: bool,
    /// The tune a script last asked for (`s57`), by its number in the game's list,
    /// which asking for it again does not start anew; [`NO_TUNE`] once a script
    /// has stopped it (`s58`). The script context's + 0x28.
    pub tune: i32,
    /// A script has turned the sound down (`s112`) and not back up (`s113`): the
    /// context's flag 0x800.
    pub turned_down: bool,
    /// What scripts have asked of the sound and has yet to be handed on.
    pub sounds: Vec<Request>,
    /// How many sounds of the banks scripts have asked for: the next is known by
    /// this number.
    pub effects_asked: u32,
    /// The header of the map's bank of sounds, if it has one.
    pub map_bank: Option<String>,
    /// The sounds the map has about it, by their places of sixteen (`s54`); whether
    /// they are heard at all, which they are not during a battle; and how much of
    /// them.
    pub ambience: BTreeMap<usize, Ambient>,
    pub ambience_on: bool,
    pub ambience_level: AmbienceLevel,
}

/// What [`Scene::tune`] is when no tune is playing by a script's doing.
pub const NO_TUNE: i32 = -1;

/// The banks a script plays sounds of, by the kind it names (0xC23B8, 0xD4224): the
/// one that is always there, the map's (slot 6 of its bundle, 0xF1E28), and that of
/// the event that is loaded (slot 6 of its, 0xE95D4).
pub const BANK_COMMON: i32 = 0;
pub const BANK_OF_MAP: i32 = 1;
pub const BANK_OF_EVENT: i32 = 2;

/// How many sounds a map can have about it (0xD0AA4).
pub const AMBIENT_PLACES: usize = 16;

/// A sound a map has at a place (`s54`): it loops, and is as loud as she is near it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ambient {
    /// The number the mixer knows it by.
    pub handle: u32,
    /// Its volume, of 127, at the edge of its range and at its heart.
    pub min: i32,
    pub max: i32,
    pub position: Vec3,
    pub range: f32,
    /// A script has it silent (`s123`).
    pub muted: bool,
    /// The volume and the place between left and right it was last given.
    heard: Option<(i32, i32)>,
}

impl Ambient {
    pub fn new(handle: u32, min: i32, max: i32, position: Vec3, range: f32) -> Ambient {
        Ambient { handle, min, max, position, range, muted: false, heard: None }
    }
}

/// How much of the sounds about a map is heard: scripts turn it down and back up
/// over a number of frames (0xD0DF4, 0xD0FF8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbienceLevel {
    pub now: f32,
    to: f32,
    step: f32,
    frames: i32,
}

impl AmbienceLevel {
    pub const FULL: AmbienceLevel = AmbienceLevel { now: 1.0, to: 1.0, step: 0.0, frames: 0 };

    /// Has it go to so many percent over so many frames; with none it is there the
    /// next frame.
    pub fn go(&mut self, percent: i32, frames: i32) {
        self.to = percent as f32 / 100.0;
        self.frames = frames;
        self.step = if frames > 0 { (self.to - self.now) / frames as f32 } else { f32::INFINITY };
    }

    /// A frame.
    pub fn step(&mut self) {
        if self.step != 0.0 {
            self.frames -= 1;
            if self.frames < 0 {
                (self.now, self.step, self.frames) = (self.to, 0.0, 0);
            } else {
                self.now += self.step;
            }
        }
    }
}

/// How far to one side a sound about the map is heard, for the angle in degrees,
/// from 0 up to 360, between the way the camera looks and the way to the sound,
/// counted to the left (0xD11C8): from the middle, 0x40, less to the left.
fn place_of(angle: f32) -> i32 {
    let offset = if angle < 180.0 {
        let mut offset = (angle * 2.0 / 3.0) as i32;
        if angle > 90.0 {
            offset = 0x80 - offset;
        }
        -offset.min(0x3d)
    } else {
        let back = 360.0 - angle;
        let mut offset = (back * -2.0 / 3.0) as i32;
        if back > 90.0 {
            offset = -(offset + 0x80);
        }
        -offset.max(-0x3e)
    };
    MIDDLE + offset
}

// What `s173` is given (0xE4B40): 1 when a mission has been cleared, 3 when it has
// been given up, and the two below. The game then goes to the screen where the next
// mission is chosen, except for these two.
/// The mission has been cleared.
pub const LEAVING_CLEARED: i32 = 1;
/// The field is left for the ride on the bike (0xE6F8C: `startBikeMiniGame`), and
/// starts again after it on the map and entrance two of its figures name.
pub const LEAVING_ELSEWHERE: i32 = 2;
/// The mission has been given up: from the pause (0x10CBF8), or from the window
/// of a mission failed.
pub const LEAVING_GIVEN_UP: i32 = 3;
/// The mission is tried again from its start.
pub const LEAVING_RETRY: i32 = 5;

impl Scene {
    pub fn new(map: i32, entrance: i32, defaults: Defaults) -> Scene {
        Scene {
            map,
            entrance,
            defaults,
            characters: BTreeMap::new(),
            resources: BTreeMap::new(),
            traps: BTreeMap::new(),
            taken_to: BTreeMap::new(),
            route_points: BTreeMap::new(),
            traps_made: 0,
            walls: BTreeMap::new(),
            battle: None,
            report: BattleReport::default(),
            fade: Fade::CLEAR,
            capture: Capture::None,
            film: None,
            lights: Lights::default(),
            lights_kept: Lights::default(),
            frames: Frames::default(),
            scene_fade: None,
            leaving: None,
            map_change: None,
            camera: Camera::new(),
            cuts_kept: Vec::new(),
            // As the field's interface is made the marks over characters are
            // shown (0xED8DC).
            flags: BTreeSet::from([field_flag::PART_2_SHOWN]),
            values: BTreeMap::new(),
            work: Work::default(),
            pop_place: Vec3::ZERO,
            system: None,
            cleared: None,
            banner_text: String::new(),
            notice: None,
            failed: None,
            place_name: None,
            location: None,
            action_sign: ActionSign::default(),
            chosen: (0, 0),
            display_stale: false,
            too_close: None,
            window: None,
            selection: 0,
            answer: 0,
            pictures: BTreeMap::new(),
            events: VecDeque::new(),
            event_running: false,
            system_call: None,
            stop_events: false,
            comm: Comm::default(),
            staging: None,
            cut: None,
            event_frame: 0,
            captions: None,
            shakes: Vec::new(),
            voice_begun: false,
            tune_heard: true,
            coming: Vec::new(),
            game_cleared: None,
            record_news: false,
            record_windows: false,
            saves: None,
            said: None,
            saying: false,
            tune: NO_TUNE,
            turned_down: false,
            sounds: Vec::new(),
            effects_asked: 0,
            map_bank: None,
            ambience: BTreeMap::new(),
            ambience_on: true,
            ambience_level: AmbienceLevel::FULL,
        }
    }

    /// Every character lies as the frame it is in does (0x109128, which the
    /// original asks each time it needs to know).
    pub fn lay_characters(&mut self) {
        for character in self.characters.values_mut() {
            character.lie_in(&self.frames);
        }
    }

    /// The lights are a map's, as its own scripts have left them (0x111614).
    pub fn light_as(&mut self, lights: Lights) {
        (self.lights, self.lights_kept) = (lights, lights);
    }

    /// A frame of how the characters are lit. Each one's colours go on towards
    /// what they were to be, but only while it is going about (0xFFC0C: its
    /// status is the one it moves in); then the ray down through it says what
    /// kind of ground it is on and which light falls there (0xFD3CC: `marks`,
    /// the first of either where nothing says), and its colours are to be that
    /// light's (0xFFB40), at once if it has just been made or put there. The
    /// ray goes by the character's own upright, as its frame lies (0xFD4AC).
    pub fn light_characters(&mut self, marks: impl Fn(&Character, &Frames) -> (Option<u8>, Option<u8>)) {
        let (lights, frames) = (&self.lights, &self.frames);
        for character in self.characters.values_mut() {
            if character.status == character.move_status {
                character.lit.step();
            }
            let (kind, light) = marks(character, frames);
            let lit = &mut character.lit;
            lit.ground = kind.unwrap_or(0);
            lit.under = light.unwrap_or(0);
            if !lit.pinned {
                lit.light = lit.under;
            }
            let by = &lights.lights[usize::from(lit.light).min(LIGHTS - 1)];
            lit.want(lights.ambient, by.colours);
            if lit.at_once {
                lit.settle();
            }
        }
    }

    /// `s49(kind)`, an event's function 0x59: the camera is of another kind, and
    /// one that is not a cut's has done with the cut.
    pub fn set_camera_kind(&mut self, kind: i32) {
        self.camera.set_kind(kind);
        if kind != camera::kind::CUT {
            self.cut = None;
        }
    }

    /// The camera as it is is put by for the time of an event, with the cut it is
    /// at (`s50`, 0xDA324), and taken up again after it (`s51`, 0xDA40C).
    pub fn keep_camera(&mut self) {
        if self.camera.keep() {
            self.cuts_kept.push(self.cut.clone());
        }
    }

    pub fn take_up_camera(&mut self) {
        if self.camera.take_up() {
            self.cut = self.cuts_kept.pop().flatten();
        }
    }

    /// What an event gave its people to hold goes with the event (0xE9964).
    pub fn drop_props(&mut self) {
        for character in self.characters.values_mut() {
            character.props = Default::default();
        }
    }

    /// A trap of the map's, made by its script (`s43`, `s67`, `s118`).
    pub fn make_trap(&mut self, id: i32, kind: TrapKind) {
        self.traps_made += 1;
        self.traps.insert(id, Trap { order: self.traps_made, ..Trap::new(kind) });
        self.taken_to.remove(&id);
    }

    /// `s234(trap, way, x, y, z, x, y, z)` (0xC61D4, 0xE5E08): the trap has one
    /// more place to take her to, somewhere between two points, in one of four
    /// ways; a way that is none of them, or a trap that is not there, is nothing.
    pub fn take_from(&mut self, trap: i32, way: i32, line: [Vec3; 2]) {
        if let (true, Some(by)) = (self.traps.contains_key(&trap), TakenBy::of(way)) {
            self.taken_to.entry(trap).or_default().push(TakenTo { by, line });
        }
    }

    /// The camera is told which of the map's traps she, standing at `at`, has come
    /// into and which she has left (0x10DBEC), the traps being gone through in the
    /// order they were made: each time it takes what the traps she is in have for
    /// it. A trap that is switched off she is in no longer, and the camera is not
    /// told. `at_once` is after an event, when nothing of the camera's takes time.
    pub fn walk_traps(&mut self, at: Vec3, at_once: bool) {
        let mut made: Vec<(u32, i32)> = self.traps.iter().map(|(&id, trap)| (trap.order, id)).collect();
        made.sort_unstable();
        for (_, id) in made {
            let trap = self.traps[&id];
            if trap.off {
                self.camera.forget(id);
                continue;
            }
            match (trap.contains(at, &self.frames), self.camera.inside(id)) {
                (true, false) => self.camera.enter(id, &trap.camera),
                (false, true) => self.camera.leave(id, &trap.camera),
                _ => continue,
            }
            let traps = &self.traps;
            self.camera.resolve(|id| traps.get(&id).map(|trap| trap.camera), at_once);
        }
    }

    /// The header of the bank of the kind a script names, if there is such a bank.
    pub fn bank(&self, kind: i32) -> Option<String> {
        match kind {
            BANK_COMMON => Some(COMMON_BANK.to_owned()),
            BANK_OF_MAP => self.map_bank.clone(),
            BANK_OF_EVENT => self.staging.as_ref().and_then(|staging| staging.bank().map(str::to_owned)),
            _ => None,
        }
    }

    /// A sound of a bank is asked for: the number it is known by from now on.
    pub fn effect(&mut self, bank: String, sound: u32, volume: i32) -> u32 {
        let handle = self.effects_asked;
        self.effects_asked = (handle + 1) & i32::MAX as u32;
        self.sounds.push(Request::Effect { handle, bank, sound, volume, note: NOTE });
        handle
    }

    /// The sounds about the map are made silent, or let be heard again (0xD0D74).
    pub fn hear_ambience(&mut self, on: bool) {
        self.ambience_on = on;
        for ambient in self.ambience.values_mut() {
            ambient.heard = None;
            self.sounds.push(Request::EffectVolume(ambient.handle, 0));
        }
    }

    /// The frame's work for the sounds about the map (0xD0FC4): each that is in
    /// range of where she stands is as loud as she is near it, and to the side it
    /// is on from where the camera looks; the rest are as loud as they are at
    /// their range's edge. `yaw` is the angle about what it looks at that the
    /// camera stands at.
    pub fn sound_ambience(&mut self, listener: Vec3, camera: Vec3, yaw: f32) {
        if !self.ambience_on {
            return;
        }
        let level = self.ambience_level.now;
        // The volume one sound comes to is not let fall below what the one before
        // it starts from: the original carries it over from place to place.
        let mut carried = self.ambience.get(&0).map_or(0, |first| first.min);
        let mut any_in_range = false;
        for place in 0..AMBIENT_PLACES {
            let Some(ambient) = self.ambience.get_mut(&place) else {
                if any_in_range {
                    carried = 0;
                }
                continue;
            };
            let mut heard = None;
            if !ambient.muted {
                let distance = ambient.position.distance(listener);
                if distance < ambient.range {
                    let span = (ambient.max - ambient.min) as f32;
                    let near = (span - span * (distance / ambient.range) + ambient.min as f32) as i32;
                    carried = carried.max(near.clamp(0, FULL_VOLUME)).min(ambient.max).max(ambient.min);
                    let way = camera - ambient.position;
                    let angle = (way.x.atan2(way.z).to_degrees() - yaw.to_degrees()).rem_euclid(360.0);
                    heard = Some(((carried as f32 * level) as i32, place_of(angle)));
                    any_in_range = true;
                } else {
                    heard = Some(((ambient.min as f32 * level) as i32, MIDDLE));
                }
            }
            if any_in_range {
                carried = ambient.min;
            }
            if let Some((volume, place)) = heard.filter(|heard| ambient.heard != Some(*heard)) {
                ambient.heard = heard;
                self.sounds.extend([Request::EffectVolume(ambient.handle, volume), Request::EffectPan(ambient.handle, place)]);
            }
        }
    }

    pub fn player(&self) -> Option<&Character> {
        self.characters.get(&PLAYER)
    }

    /// Says of each character whether it is too far from her to have its frame
    /// (0x106558, asked by 0x10600C before the frame: further than
    /// `frames_within`, the tunables' + 0xC) and whether it is not drawn
    /// (0x1067F8's first questions): further from her than `drawn_within`
    /// (0x1066A8, + 0xE), or a model nearer the camera than the field's figure,
    /// measured from a hundred over where it stands (0x1063B8; `too_close`
    /// until a script gives another). A character that keeps taking part (its
    /// flag 0x57) is asked none of it, nor is any while a script has it so (the
    /// field's flags 0x32 and 0x31), nor how far from her while she is not there.
    /// What is an effect or a thing of the map's own is not asked how near the
    /// camera it is: 0x1063B8 asks only a model of the first kind, and the port
    /// takes those two for other kinds.
    pub fn reach_characters(&mut self, eye: Vec3, frames_within: f32, drawn_within: f32, too_close: f32) {
        let her = self.player().map(|player| player.position).filter(|_| !self.flag(field_flag::ALL_NEAR));
        let too_close = (!self.flag(field_flag::ALL_IN_SIGHT)).then(|| self.too_close.unwrap_or(too_close));
        for character in self.characters.values_mut() {
            let kept = character.flags.has(flag::ALWAYS_ACTIVE);
            let from_her = her.filter(|_| !kept).map(|her| character.position.distance(her));
            let model = character.effect.is_none() && character.chr >= 0;
            let by_the_eye = too_close.filter(|_| !kept && model).is_some_and(|least| (character.position + Vec3::Y * EYE_MEASURED_OVER).distance(eye) < least);
            character.asleep = from_her.is_some_and(|far| far > frames_within);
            character.unseen = by_the_eye || from_her.is_some_and(|far| far > drawn_within);
        }
    }

    /// What becomes of the screen as the map is left (0x116048): unless something
    /// covers it already, a picture is taken of the scene as it is, which stays up
    /// and darkens while the next map is made ready, for that map's script to have
    /// go (`s7(0, -1, n)`). Answers whether the picture has yet to come: the map
    /// is not left before it has (0x116334).
    pub fn keep_screen(&mut self) -> bool {
        if !self.fade.covered() {
            self.fade.darken_picture(LEFT_DARKENS / 2);
            self.capture = Capture::Wanted { whole: false };
        }
        self.capture != Capture::None
    }

    /// The scene as another map is gone on to: what the map's script had set up is
    /// gone; what belongs to the field itself stays as it is. So does what covers
    /// the screen, the sheet or the picture of the map that is left
    /// ([`Scene::keep_screen`]), until the next map's script brings the screen
    /// back. Every character is unmade as a map is left, hers too (0x11648C: all
    /// of the field's list, and every resource but the one she is made of), so
    /// she is made anew for the next map, where she stood and facing as she did
    /// until its script puts her at its entrance: nothing a script did to her on
    /// the map that is left comes with her (one that hid her for a scene and
    /// went on to another map leaves her to be seen there).
    pub fn onward(&self, map: i32, entrance: i32) -> Scene {
        let mut next = Scene::new(map, entrance, self.defaults);
        if let Some(player) = self.player() {
            next.characters.insert(PLAYER, Character::new(player.chr, player.position, player.facing(), &self.defaults));
        }
        next.fade = self.fade;
        next.flags = self.flags.clone();
        next.values = self.values.clone();
        next.work = self.work;
        next.cleared = self.cleared.clone();
        // The name of the place that was left goes as the map does (0x1160E0).
        next.place_name = self.place_name.clone();
        if let Some(name) = &mut next.place_name {
            name.dismiss(false);
        }
        next.banner_text = self.banner_text.clone();
        next.notice = self.notice.clone();
        next.display_stale = self.display_stale;
        // The tune goes on from map to map, unless the next map's script asks for
        // another; the sounds about the map end with it.
        next.tune = self.tune;
        next.turned_down = self.turned_down;
        next.sounds = self.sounds.clone();
        next.sounds.extend(self.ambience.values().map(|ambient| Request::StopEffect(ambient.handle)));
        next.sounds.extend(self.banks().map(Request::DropBank));
        next.effects_asked = self.effects_asked;
        next.ambience_level = self.ambience_level;
        next
    }

    /// The banks that go when the map is left: its own, and that of an event it
    /// had loaded.
    fn banks(&self) -> impl Iterator<Item = String> {
        [self.bank(BANK_OF_MAP), self.bank(BANK_OF_EVENT)].into_iter().flatten()
    }

    /// The scene as the field is started anew on a map: nothing is left of the old
    /// one but what the game keeps outside the field, and a player who has just
    /// been made. The picture is dark until the map's script brings it up.
    pub fn afresh(&self, map: i32, entrance: i32) -> Scene {
        let mut next = Scene::new(map, entrance, self.defaults);
        next.characters.insert(PLAYER, Character::new(0, Vec3::ZERO, 0.0, &self.defaults));
        next.fade = Fade::DARK;
        next.work = self.work;
        // A field that is left stops its tune (0xEFC24), and the sounds about its
        // map.
        next.sounds = self.sounds.clone();
        next.sounds.extend(self.ambience.values().map(|ambient| Request::StopEffect(ambient.handle)));
        next.sounds.extend(self.banks().map(Request::DropBank));
        next.sounds.push(Request::StopMusic(0));
        next.effects_asked = self.effects_asked;
        next
    }

    pub fn flag(&self, flag: u32) -> bool {
        self.flags.contains(&flag)
    }

    pub fn set_flag(&mut self, flag: u32, on: bool) {
        if on {
            self.flags.insert(flag);
        } else {
            self.flags.remove(&flag);
        }
    }

    /// The characters that are not hidden whose numbers are from `first` to `last`.
    /// The original goes through its list of characters for this, in the order they
    /// were made; here they come by number, which only matters to a script that
    /// treats them differently, and none does.
    pub fn shown(&self, first: i32, last: i32) -> impl Iterator<Item = i32> + '_ {
        self.characters.range(first..=last).filter(|(_, character)| !character.hidden()).map(|(&id, _)| id)
    }
}

#[cfg(test)]
pub(crate) fn test_defaults() -> Defaults {
    Defaults { touch_radius: 120.0, territory: 2000.0, pace: 95, notice_range: 1800.0, wander_radius: 300.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tunables' waits: 120 to 240 frames stood, 60 to 120 gone round.
    const WAITS: [(i32, i32); 2] = [(120, 240), (60, 120)];

    #[test]
    fn one_that_has_stood_a_while_looks_about_and_stands_again() {
        // As the people of the field are: a stand of thirty frames, and the one
        // animation to idle with, of fifty.
        let length = |number: i32| match number {
            0 => Some(30.0),
            idling::BEGINS => Some(50.0),
            _ => None,
        };
        let mut idle = Idle::new(7);
        let mut stood = 0;
        while idle.playing().is_none() {
            idle.step(1.0, length, WAITS);
            stood += 1;
            assert!(stood < 400, "it should idle");
        }
        // It waited its 120 to 240 frames, then for its stand to come round:
        // which it does every thirty.
        assert!((121..=272).contains(&stood), "{stood}");
        assert_eq!(stood % 30, 0);
        assert_eq!(idle.playing(), Some((idling::BEGINS, false)));
        // The animation plays through once, and it stands again, at once.
        let mut played = 0;
        while idle.playing().is_some() {
            idle.step(1.0, length, WAITS);
            played += 1;
        }
        assert_eq!(played, 50);
        assert!(idle.at_once);
        // What it does next ends that: it stands as after anything.
        idle.rest();
        assert!(!idle.at_once && idle.playing().is_none());
    }

    #[test]
    fn one_with_all_three_begins_goes_round_and_ends() {
        let length = |number: i32| match number {
            0 => Some(30.0),
            idling::BEGINS => Some(20.0),
            idling::GOES_ROUND => Some(40.0),
            idling::ENDS => Some(10.0),
            _ => None,
        };
        let mut idle = Idle::new(3);
        let mut seen = vec![idle.playing()];
        for _ in 0..800 {
            idle.step(1.0, length, WAITS);
            if seen.last() != Some(&idle.playing()) {
                seen.push(idle.playing());
            }
            if seen.len() == 5 {
                break;
            }
        }
        assert_eq!(seen, [None, Some((idling::BEGINS, false)), Some((idling::GOES_ROUND, true)), Some((idling::ENDS, false)), None]);
    }

    #[test]
    fn one_with_nothing_to_idle_with_stands_and_a_script_has_the_others_stand_too() {
        let mut idle = Idle::new(1);
        for _ in 0..1000 {
            idle.step(1.0, |number| (number == 0).then_some(30.0), WAITS);
        }
        assert!(idle.playing().is_none());

        // One that is idling when a script says nobody is to stands at once.
        let length = |number: i32| matches!(number, 0 | idling::BEGINS).then_some(30.0);
        let mut idle = Idle::new(1);
        while idle.playing().is_none() {
            idle.step(1.0, length, WAITS);
        }
        idle.settle();
        assert!(idle.playing().is_none() && !idle.at_once);

        // A character that walks, or is hidden, or has an animation of a
        // script's, is not left to it; and with nobody to idle nothing counts.
        let mut character = Character::new(21, Vec3::ZERO, 0.0, &test_defaults());
        character.status = status::STAND;
        for _ in 0..1000 {
            character.idle_on(true, length, WAITS);
        }
        assert!(character.idle.playing().is_none());
        let mut stood = 0;
        while character.idle.playing().is_none() {
            character.idle_on(false, length, WAITS);
            stood += 1;
            assert!(stood < 400, "it should idle");
        }
        character.status = status::WALK;
        character.idle_on(false, length, WAITS);
        assert!(character.idle.playing().is_none());
    }

    #[test]
    fn a_traps_box_is_turned_by_its_three_turns_and_by_the_frame_it_is_in() {
        use std::f32::consts::FRAC_PI_2;
        // The field's frames with the second turned a quarter about the axis ahead.
        let mut frames = Frames::default();
        frames.choose(2);
        frames.set(2, FRAC_PI_2, &brs_engine::collision::CollisionWorld::default());
        let (centre, reach) = (Vec3::new(100.0, 0.0, 0.0), Vec3::new(10.0, 200.0, 50.0));
        let (above, beside) = (Vec3::new(100.0, 150.0, 0.0), Vec3::new(250.0, 0.0, 0.0));
        // As the world has it the box is tall.
        let mut trap = Trap::new(TrapKind::Camera);
        trap.shape_turned(centre, reach, Vec3::ZERO);
        assert!(trap.contains(above, &frames) && !trap.contains(beside, &frames));
        // In the frame its tall side lies along the world's X (0x117B6C).
        trap.frame = 2;
        assert!(!trap.contains(above, &frames) && trap.contains(beside, &frames));
        // And a turn of its own the other way about the axis ahead, which only
        // s257 gives a box, has it stand as it did.
        trap.shape_turned(centre, reach, Vec3::new(0.0, 0.0, -FRAC_PI_2));
        assert!(trap.contains(above, &frames) && !trap.contains(beside, &frames));
        assert_eq!((trap.turn, trap.tilt), (0.0, Vec2::new(0.0, -FRAC_PI_2)));
    }

    #[test]
    fn a_trap_is_a_turned_box_about_its_centre() {
        // The second event trap of the first map: across the street, turned 35 degrees.
        let mut trap = Trap::new(TrapKind::Event);
        trap.shape(Vec3::new(349.0, 0.0, 4819.0), Vec3::new(400.0, 100.0, 100.0), 35f32.to_radians());
        assert!(trap.contains(Vec3::new(349.0, 0.0, 4819.0), &Frames::default()));
        // Along its long axis, which runs across the way the street goes there.
        let along = Quat::from_rotation_y(35f32.to_radians()) * Vec3::X;
        assert!((along - Vec3::new(0.819, 0.0, -0.574)).length() < 1e-3);
        assert!(trap.contains(trap.centre + along * 399.0, &Frames::default()));
        assert!(!trap.contains(trap.centre + along * 401.0, &Frames::default()));
        // Across it, it is thin.
        let across = Quat::from_rotation_y(35f32.to_radians()) * Vec3::Z;
        assert!(trap.contains(trap.centre + across * 99.0, &Frames::default()));
        assert!(!trap.contains(trap.centre + across * 101.0, &Frames::default()));
        // And it reaches as far down as up.
        assert!(trap.contains(trap.centre - Vec3::Y * 99.0, &Frames::default()) && !trap.contains(trap.centre + Vec3::Y * 101.0, &Frames::default()));
    }

    #[test]
    fn sizes_that_are_not_positive_become_a_hundred() {
        let mut trap = Trap::new(TrapKind::Event);
        trap.shape(Vec3::ZERO, Vec3::new(800.0, 0.0, -5.0), 0.0);
        assert_eq!(trap.reach, Vec3::new(800.0, 100.0, 100.0));
    }

    #[test]
    fn a_trap_may_ask_her_to_be_turned_the_way_it_faces() {
        // A trap of the second map's sixth mission: turned half a turn, and she
        // has to face within a quarter turn of the way it does.
        let mut trap = Trap::new(TrapKind::Event);
        trap.shape(Vec3::new(709.0, 1000.0, -4759.0), Vec3::new(180.0, 100.0, 100.0), 180f32.to_radians());
        let inside = trap.centre + Vec3::new(50.0, 0.0, 20.0);
        // As it is made she is in it whichever way she is turned, walking into it
        // calls the script, and cross has ACTION.
        assert!(trap.on_touch && trap.by_action && trap.prompt(0) == Some(prompt::ACTION) && trap.prompt(1).is_none());
        for degrees in [0.0f32, 90.0, 180.0, -135.0] {
            assert!(trap.touched(inside, degrees.to_radians(), &Frames::default()), "{degrees}");
        }
        trap.facing = 90f32.to_radians();
        for (degrees, faced) in [(180.0f32, true), (100.0, true), (-100.0, true), (80.0, false), (0.0, false), (-45.0, false)] {
            assert_eq!(trap.touched(inside, degrees.to_radians(), &Frames::default()), faced, "{degrees}");
        }
        // Outside its box, or switched off, she is not in it however she faces.
        assert!(!trap.touched(trap.centre + Vec3::X * 200.0, 180f32.to_radians(), &Frames::default()));
        trap.off = true;
        assert!(!trap.touched(inside, 180f32.to_radians(), &Frames::default()));
        // The other kinds of trap are walked into.
        assert!(!Trap::new(TrapKind::Camera).by_action && Trap::new(TrapKind::Camera).prompt(0).is_none());
    }

    #[test]
    fn what_a_button_does_with_a_character_is_what_scripts_say_of_it() {
        let mut character = Character::new(17, Vec3::ZERO, 0.0, &test_defaults());
        // As it is made cross talks to it, and square does nothing.
        assert_eq!((character.prompt(0), character.prompt(1)), (Some(prompt::TALK), None));
        // A home point: cross checks it.
        character.set(setting::CONTACT, prompt::CHECK);
        assert_eq!(character.prompt(0), Some(prompt::CHECK));
        // An arrow, or a box that has been opened: no button does anything with it.
        character.set(setting::BY_ACTION, 0);
        assert_eq!((character.prompt(0), character.by_action), (None, false));
        // A container: giving it its word has a button do something with it again.
        character.set(setting::CONTACT, prompt::ACTION);
        assert_eq!((character.prompt(0), character.by_action), (Some(prompt::ACTION), true));
        // What cross does can be taken away and given back, and square given one.
        character.set(setting::PROMPT_ON, 0);
        character.set(setting::CONTACT_SQUARE, prompt::TALK);
        assert_eq!((character.prompt(0), character.prompt(1)), (None, Some(prompt::TALK)));
        character.set(setting::PROMPT_ON, 1);
        assert_eq!(character.prompt(0), Some(prompt::ACTION));
    }

    #[test]
    fn a_face_blinks_now_and_then_and_its_mouth_moves_while_it_is_to() {
        let mut face = Face::default();
        assert_eq!((face.lids(), face.jaw(), face.used), (0.0, 0.0, false));
        // The first blink is at once: three frames of it, the lids a third of a
        // radian down at the most, and then they are up.
        face.blink_after(60);
        let mut blink = Vec::new();
        for _ in 0..5 {
            face.step();
            blink.push(face.lids());
        }
        let of = |degrees: f32| degrees.to_radians().sin() / 3.0;
        for (seen, wanted) in blink.iter().zip([of(50.0), of(100.0), of(150.0), 0.0, 0.0]) {
            assert!((seen - wanted).abs() < 1e-6, "{blink:?}");
        }
        // The next comes when the count it was given has run down (and two), and
        // those after it four to eight seconds apart.
        let mut frames = 5;
        while face.lids() == 0.0 {
            face.step();
            frames += 1;
            assert!(frames < 100, "it never blinked again");
        }
        assert_eq!(frames, 61);
        let mut gaps = Vec::new();
        let mut since = 0;
        for _ in 0..3000 {
            let was = face.lids();
            face.step();
            since += 1;
            if was == 0.0 && face.lids() != 0.0 {
                gaps.push(since);
                since = 0;
            }
        }
        // (A count of 120 to 239, which is run down to under three.)
        assert!(gaps.len() > 10 && gaps.iter().all(|gap| (118..=237).contains(gap)), "{gaps:?}");
        // Eyes that are to stay open or shut stay so, and those let be are let be.
        face.blink_after(1);
        face.step();
        assert_eq!((face.lids(), face.blink_in), (0.0, 1));
        face.blink_after(LIDS_SHUT);
        face.step();
        assert_eq!(face.lids(), 0.35);
        face.blink_after(0);
        assert_eq!((face.lids(), face.blinks), (0.0, false));
        face.blink_after(100_000);
        assert_eq!(face.blink_in, 6000);

        // A mouth that moves goes round by at least eight degrees a frame, and
        // the jaw is never more than a sixth of a radian down; one that is to
        // move no more is still.
        face.speak(10);
        let mut lowest: f32 = 0.0;
        for _ in 0..200 {
            let was = face.mouth;
            face.step();
            let more = face.mouth.wrapping_sub(was);
            assert!((8..8 + 40).contains(&more), "{more}");
            assert!((0.0..=1.0 / 6.0 + 1e-6).contains(&face.jaw()));
            lowest = lowest.max(face.jaw());
        }
        assert!(lowest > 0.15, "{lowest}");
        face.speak(0);
        face.step();
        assert_eq!((face.jaw(), face.speaks), (0.0, false));
        // It is asked for no more than a hundred.
        face.speak(1000);
        assert_eq!(face.amount, 100);
    }

    #[test]
    fn a_mouth_stops_and_the_character_stands_when_what_is_spoken_ends() {
        let mut character = Character::new(17, Vec3::ZERO, 0.0, &test_defaults());
        character.walk();
        character.speak(10, 3);
        assert!(character.face.speaks && character.flags.has(flag::MOUTH_WITH_VOICE) && character.flags.has(flag::STANDS_AFTER_VOICE));
        // Until what is spoken has begun to be heard nothing is listened for.
        for _ in 0..30 {
            character.step_face(false, false);
        }
        assert!(character.face.speaks && character.acting());
        // Once it has, the mouth goes on for the ten frames it waits and for as
        // long as anything is heard.
        for _ in 0..40 {
            character.step_face(true, true);
        }
        assert!(character.face.speaks && character.acting());
        // And when nothing is heard any more it stops, and the character stands.
        character.step_face(true, false);
        assert!(!character.face.speaks && !character.acting());
        assert!(!character.flags.has(flag::MOUTH_WITH_VOICE) && !character.flags.has(flag::STANDS_AFTER_VOICE));
        assert_eq!(character.face.jaw(), 0.0);

        // The ten frames are counted from when the voice begins.
        character.speak(10, 1);
        for frame in 0..10 {
            character.step_face(true, false);
            assert!(character.face.speaks, "frame {frame}");
        }
        character.step_face(true, false);
        assert!(!character.face.speaks);
        // A mouth that is not to stop of itself goes on until its script stills
        // it, and one that keeps its pose does not stand.
        character.speak(10, 0);
        for _ in 0..50 {
            character.step_face(true, false);
        }
        assert!(character.face.speaks);
        character.speak(0, 0);
        assert!(!character.face.speaks);
        character.walk();
        character.flags.set(flag::KEEPS_POSE, true);
        character.speak(10, 2);
        for _ in 0..20 {
            character.step_face(true, false);
        }
        assert!(character.face.speaks && character.acting() && !character.flags.has(flag::STANDS_AFTER_VOICE));
    }

    #[test]
    fn a_neck_goes_over_to_what_it_is_next_to_be_in_the_frames_it_is_given() {
        let mut character = Character::new(17, Vec3::ZERO, 0.0, &test_defaults());
        assert_eq!((character.neck, character.get(setting::NECK)), (Neck::default(), 0));
        // As the scripts' own function turns a head over thirty frames: the neck
        // is switched on by its angles, which are nought, and what it is next to
        // be is named and started.
        character.set(setting::NECK, 1);
        assert_eq!((character.get(setting::NECK), character.neck.on, character.neck.way), (1, true, NeckWay::Own));
        for (what, degrees) in [(setting::NECK_NEXT_X, 10), (setting::NECK_NEXT_Y, -20), (setting::NECK_NEXT_Z, 30)] {
            character.set(what, degrees);
        }
        character.set(setting::NECK_NEXT, 1);
        assert_eq!(character.neck.angles, Vec3::ZERO);
        character.neck.start(30, 0);
        let wanted = Vec3::new(10.0, -20.0, 30.0).map(f32::to_radians);
        assert_eq!((character.neck.angles, character.neck.moving, character.neck.starts, character.neck.part()), (wanted, true, 1, 0.0));
        for _ in 0..15 {
            character.neck.step(FULL_PACE);
        }
        assert_eq!((character.neck.moving, character.neck.part()), (true, 0.5));
        for _ in 0..15 {
            character.neck.step(FULL_PACE);
        }
        assert_eq!((character.neck.on, character.neck.moving, character.neck.part()), (true, false, 1.0));
        // It goes evenly, slowly at first, slowly at last, or both.
        let part = |ease: i32, frames: i32| {
            let mut neck = Neck::default();
            neck.start(40, ease);
            neck.step(frames * FULL_PACE);
            neck.part()
        };
        // (A way of going there is none of is the even one.)
        assert_eq!([part(0, 10), part(1, 20), part(2, 20), part(9, 20)], [0.25, 0.25, 0.75, 0.5]);
        assert_eq!([part(3, 10), part(3, 20), part(3, 30)], [0.125, 0.5, 0.875]);
        // The other ways of turning a head, and whom it looks at.
        character.set(setting::NECK, 2);
        assert_eq!((character.neck.way, character.get(setting::NECK)), (NeckWay::FromBody, 2));
        character.set(setting::NECK_NEXT, 4);
        character.set(setting::NECK_NEXT_OBJECT, 602);
        character.neck.start(8, 3);
        assert_eq!((character.neck.way, character.neck.object, character.get(setting::NECK)), (NeckWay::Object, 602, 4));
        // With nought the neck turns the head no more.
        character.set(setting::NECK_X, 45);
        character.set(setting::NECK, 0);
        assert_eq!((character.neck.on, character.get(setting::NECK)), (false, 0));
        character.set(setting::NECK, 1);
        character.set(setting::NECK_NEXT, 0);
        assert!(!character.neck.on);
    }

    #[test]
    fn a_character_that_is_an_effect_is_one_scripts_do_with_as_with_any() {
        let mut effect = Character::of_effect("some/04.bin/11.efc".to_owned(), &test_defaults());
        // It is made where the world begins, of no model, and has yet to play.
        assert_eq!((effect.chr, effect.position, effect.facing()), (NO_RESOURCE, Vec3::ZERO, 0.0));
        assert_eq!(effect.effect, Some(EffectOf { file: "some/04.bin/11.efc".to_owned(), begun: 0, fastened: None }));
        // What makes it sets it playing, and has cross check it: a heap of junk
        // is nothing but that.
        effect.begin_effect();
        assert_eq!(effect.effect.as_ref().map(|effect| effect.begun), Some(1));
        assert_eq!((effect.prompt(0), effect.by_action, effect.flags.has(flag::OF_EFFECT)), (Some(prompt::CHECK), true, true));
        // What is only there to be seen has that taken away by its script; made
        // again, it plays from the start and is something to check once more.
        effect.set(setting::BY_ACTION, 0);
        assert_eq!(effect.prompt(0), None);
        effect.begin_effect();
        assert_eq!((effect.effect.as_ref().map(|effect| effect.begun), effect.prompt(0)), (Some(2), Some(prompt::CHECK)));
        // How near she has to be to be in touch with it is a setting.
        effect.set(setting::TOUCH_RADIUS, 600);
        assert_eq!(effect.touch_radius, 600.0);
        // A character that is a model is none of this, and what makes an effect
        // of its number only has it be something to check.
        let mut model = Character::new(17, Vec3::ZERO, 0.0, &test_defaults());
        model.begin_effect();
        assert_eq!((model.effect.as_ref(), model.prompt(0)), (None, Some(prompt::CHECK)));
    }

    #[test]
    fn a_character_is_turned_about_three_axes_in_their_order_and_as_large_as_it_is_set() {
        let mut character = Character::new(17, Vec3::new(10.0, 20.0, 30.0), 0.0, &test_defaults());
        // As it is made: where it is, turned no way, its own size.
        assert_eq!(character.frame(), Transform::from_xyz(10.0, 20.0, 30.0));
        // Three times its size (setting 88 is in hundredths).
        character.set(setting::SCALE, 300);
        assert_eq!(character.frame().scale, Vec3::splat(3.0));
        // A quarter turn about the axis across and then one about the upright
        // one: what pointed ahead points down after the first, and stays so.
        character.set(setting::TURN_X, 90);
        character.set(setting::FACING, 90);
        let ahead = character.frame().rotation * Vec3::Z;
        assert!(ahead.abs_diff_eq(Vec3::NEG_Y, 1e-5), "{ahead}");
        // What pointed up points ahead after the first and to the side after the
        // second; a quarter turn about the axis ahead then has it point up again.
        let up = character.frame().rotation * Vec3::Y;
        assert!(up.abs_diff_eq(Vec3::X, 1e-5), "{up}");
        character.set(setting::TURN_Z, 90);
        let up = character.frame().rotation * Vec3::Y;
        assert!(up.abs_diff_eq(Vec3::Y, 1e-5), "{up}");
        // The turns are read back as they were given.
        assert_eq!((character.get(setting::TURN_X), character.get(setting::TURN_Z)), (90, 90));
    }

    #[test]
    fn a_wall_pushes_a_body_out_by_its_nearest_side() {
        // The barricade of the first mission: across the street, a thousand to
        // either side and a hundred through.
        let mut wall = Wall::new();
        wall.shape(Vec3::new(-400.0, 0.0, -1470.0), Vec3::new(1000.0, 150.0, 100.0), 0.0);
        // Running into its long side from the north she is held at its face.
        let held = wall.push_out(Vec3::new(-300.0, 0.0, -1350.0), 40.0);
        assert_eq!(held, Vec3::new(-300.0, 0.0, -1330.0));
        // Away from it nothing happens, nor past its end, nor far above it.
        for clear in [Vec3::new(-300.0, 0.0, -1300.0), Vec3::new(700.0, 0.0, -1470.0), Vec3::new(-300.0, 400.0, -1470.0)] {
            assert_eq!(wall.push_out(clear, 40.0), clear);
        }
        // A turned wall pushes out square to itself.
        wall.shape(Vec3::ZERO, Vec3::new(1000.0, 150.0, 100.0), std::f32::consts::FRAC_PI_2);
        let held = wall.push_out(Vec3::new(120.0, 0.0, 0.0), 40.0);
        assert!((held - Vec3::new(140.0, 0.0, 0.0)).length() < 1e-3, "held at {held}");
        // And an open one is not there.
        wall.open = true;
        assert_eq!(wall.push_out(Vec3::new(120.0, 0.0, 0.0), 40.0), Vec3::new(120.0, 0.0, 0.0));
    }

    #[test]
    fn flags_keep_to_their_own_bits() {
        let mut flags = Flags::default();
        flags.set(flag::SYMBOL, true);
        flags.set(flag::UNAWARE, true);
        assert!(flags.has(flag::SYMBOL) && flags.has(flag::UNAWARE) && !flags.has(flag::HIDDEN));
        flags.set(flag::SYMBOL, false);
        assert!(!flags.has(flag::SYMBOL) && flags.has(flag::UNAWARE));
        // A number that is no flag is never set.
        flags.set(1000, true);
        assert!(!flags.has(1000));
    }

    #[test]
    fn a_pace_in_percent_is_so_many_time_units_a_frame() {
        assert_eq!(pace(100), FULL_PACE);
        assert_eq!(pace(95), 1945);
        // The sixty frames a script counts as 0x1E000.
        assert_eq!(60 * FULL_PACE, 0x1e000);
    }

    #[test]
    fn a_turn_takes_the_same_time_whatever_its_size() {
        let mut character = Character::new(21, Vec3::ZERO, 0.0, &test_defaults());
        character.target_facing = 1.7;
        character.turn();
        assert_eq!(character.status, status::TURN);
        assert!(character.acting());
        let Action::Turn { rate } = character.action else { panic!("it should be turning") };
        assert!((rate * TURN_FRAMES - 1.7).abs() < 1e-5);
        // The short way round.
        character.target_facing = -3.0;
        character.mover.facing = 3.0;
        character.turn();
        let Action::Turn { rate } = character.action else { panic!("it should be turning") };
        assert!(rate > 0.0 && rate * TURN_FRAMES < 0.3);
        // Facing the right way already, there is nothing to do.
        character.target_facing = 3.0;
        character.turn();
        assert_eq!(character.action, Action::None);
        assert!(!character.acting());
    }

    #[test]
    fn a_fade_runs_to_its_end_and_stops() {
        let mut fade = Fade::CLEAR;
        assert!(!fade.covered());
        // A script that brings the screen back when it is not covered does nothing.
        assert!(!fade.script(0, 0, 30));
        assert_eq!(fade, Fade::CLEAR);
        fade.script(1, 0, 4);
        assert!(fade.running() && fade.covered(), "it counts as covered from when it starts to be");
        for _ in 0..3 {
            fade.advance();
            assert!(fade.running());
        }
        fade.advance();
        assert!(!fade.running());
        assert_eq!(fade.level, 1.0);
        // Asked for what it already is, it has nothing to do; and the colour last
        // used is the one it comes back by.
        fade.script(1, 1, 30);
        assert!(!fade.running() && !fade.white);
        fade.script(0, -1, 0);
        assert_eq!((fade.level, fade.white, fade.running()), (1.0, false, true));
        fade.advance();
        assert_eq!((fade.level, fade.covered()), (0.0, false));
        // Told to come in from white, it is all white first, whatever it was.
        fade.restart(false, true, 2);
        assert_eq!((fade.level, fade.white, fade.running()), (1.0, true, true));
        fade.advance();
        assert_eq!(fade.level, 0.5);
        fade.advance();
        assert_eq!((fade.level, fade.running()), (0.0, false));
        // And told to go to black it starts from clear.
        fade.level = 0.7;
        fade.restart(true, false, 0);
        assert_eq!((fade.level, fade.white), (0.0, false));
        fade.advance();
        assert_eq!((fade.level, fade.running()), (1.0, false));
        // A sheet that goes over by half stops at half, and comes off from there.
        fade.begin(fade_kind::TO_HALF, 2);
        fade.advance();
        fade.advance();
        assert_eq!((fade.level, fade.running(), fade.covered()), (128.0 / 255.0, false, true));
        fade.begin(fade_kind::FROM_HALF, 1);
        assert_eq!(fade.level, 128.0 / 255.0);
        fade.advance();
        assert_eq!((fade.level, fade.running(), fade.covered()), (0.0, false, false));
    }

    #[test]
    fn the_picture_the_fade_keeps_stands_goes_and_darkens() {
        // A battle leaves its last picture up: the screen counts as covered, no
        // sheet is drawn, and the picture is all there.
        let mut fade = Fade::DARK;
        fade.hold_picture();
        assert_eq!((fade.level, fade.covered(), fade.running()), (0.0, true, false));
        assert_eq!(fade.picture(), Some([255, 255, 255, 255]));
        // The script brings the screen back by the kind last used: the picture goes,
        // over the frames it gives, and the field shows through it.
        assert!(!fade.script(0, -1, 30));
        assert!(fade.running() && !fade.covered());
        for _ in 0..15 {
            fade.advance();
        }
        assert_eq!(fade.picture(), Some([255, 255, 255, 127]));
        for _ in 0..15 {
            fade.advance();
        }
        assert_eq!((fade.picture(), fade.running()), (None, false));
        // Taken away by the kind last used, a picture is wanted of the screen as it
        // is, which then stays; it was last put up whole, and whole it stays.
        assert!(fade.script(1, -1, 15));
        assert!(fade.covered() && fade.running());
        fade.advance();
        assert_eq!(fade.picture(), Some([255, 255, 255, 255]));
        for _ in 0..14 {
            fade.advance();
        }
        assert!(!fade.running());
        // One that was never put up whole darkens as it stays.
        let mut fade = Fade::CLEAR;
        assert!(fade.script(1, 2, 10));
        for _ in 0..5 {
            fade.advance();
        }
        assert_eq!(fade.picture(), Some([127, 127, 127, 255]));
        for _ in 0..5 {
            fade.advance();
        }
        assert_eq!((fade.picture(), fade.running()), (Some([0, 0, 0, 255]), false));
        // And a shot of a scene gives way to the next by its picture going.
        fade.dissolve(10);
        fade.advance();
        assert_eq!(fade.picture(), Some([255, 255, 255, 229]));
        // A sheet started over a picture that stands takes its place: the picture
        // is no longer drawn.
        fade.hold_picture();
        assert!(fade.picture().is_some());
        assert!(!fade.script(0, 0, 30));
        assert_eq!((fade.picture(), fade.level, fade.running()), (None, 1.0, true));
    }

    #[test]
    fn the_camera_takes_what_the_traps_she_walks_into_have_for_it() {
        use camera::{figure, kind};
        let mut scene = empty();
        // The first map's two: a wide one in which the camera follows her, and
        // beside it one in which it goes along a path.
        scene.make_trap(100, TrapKind::Camera);
        scene.traps.get_mut(&100).unwrap().shape(Vec3::new(2200.0, 440.0, 1000.0), Vec3::new(3650.0, 600.0, 6600.0), 0.0);
        for (what, value) in [(26, 0), (19, 500), (7, 0), (8, 19), (15, 140), (13, 0)] {
            assert!(scene.traps.get_mut(&100).unwrap().camera.set(what, value));
        }
        scene.make_trap(101, TrapKind::Camera);
        scene.traps.get_mut(&101).unwrap().shape(Vec3::new(-3619.0, 0.0, -107.0), Vec3::new(2200.0, 1400.0, 1650.0), 0.0);
        for (what, value) in [(26, 5), (27, 0), (13, 0)] {
            assert!(scene.traps.get_mut(&101).unwrap().camera.set(what, value));
        }

        // After the event the map starts with she is found in the wide one.
        scene.walk_traps(Vec3::new(0.0, 0.0, 0.0), true);
        assert_eq!(scene.camera.kind, kind::FOLLOW);
        assert_eq!((scene.camera.figure(figure::OFFSET[2]), scene.camera.figure(figure::LOOK_AT[1])), (500.0, 140.0));
        // Where the two overlap the one she came into first still has the say.
        scene.walk_traps(Vec3::new(-1430.0, 0.0, 0.0), false);
        assert_eq!(scene.camera.kind, kind::FOLLOW);
        // Out of it, the camera goes along the path: at once, the trap it had its
        // kind from giving the change no frames.
        scene.walk_traps(Vec3::new(-1500.0, 0.0, 0.0), false);
        assert_eq!((scene.camera.kind, scene.camera.path), (kind::PATH, 0));
        assert!(scene.camera.blend.is_none());
        // A trap that is switched off she is in no longer.
        scene.traps.get_mut(&101).unwrap().off = true;
        scene.walk_traps(Vec3::new(-1500.0, 0.0, 0.0), false);
        assert!(!scene.camera.inside(101));
        assert_eq!(scene.camera.kind, kind::PATH);
    }

    #[test]
    fn the_scene_alone_fades_evenly_and_is_over_a_frame_after_its_last() {
        // Three frames of the game are six of the interface's.
        let mut fade = SceneFade::new(1, 0, 3);
        assert_eq!(fade.colour(), Some([0, 0, 0, 0]));
        let mut alphas = Vec::new();
        for _ in 0..3 {
            fade.step();
            assert!(!fade.over);
            alphas.push(fade.colour().unwrap()[3]);
        }
        assert_eq!(alphas, [85, 170, 255]);
        // What waits for it goes on a frame after it is all there, and it stays.
        fade.step();
        assert!(fade.over);
        fade.step();
        assert_eq!(fade.colour(), Some([0, 0, 0, 255]));
        // Going, in white: from all of it to none.
        let mut fade = SceneFade::new(0, 1, 2);
        fade.step();
        assert_eq!(fade.colour(), Some([255, 255, 255, 127]));
        fade.step();
        assert_eq!(fade.colour(), Some([255, 255, 255, 0]));
        // A colour that is neither draws nothing, a way that is neither leaves the
        // scene clear.
        assert_eq!(SceneFade::new(1, 2, 2).colour(), None);
        let mut fade = SceneFade::new(2, 0, 2);
        fade.step();
        assert_eq!(fade.colour(), Some([0, 0, 0, 0]));
    }

    /// A scene with nothing in it.
    fn empty() -> Scene {
        Scene::new(0, 0, Defaults { touch_radius: 120.0, territory: 2000.0, pace: 95, notice_range: 1800.0, wander_radius: 300.0 })
    }

    /// The tunables' figures (0x1065EC, 0x10673C, 0x118218): a frame within 4000
    /// of her, drawn within 3000, and a model no nearer the camera than 150.
    #[test]
    fn one_far_from_her_has_no_frame_and_one_further_or_by_the_camera_is_not_drawn() {
        let mut scene = empty();
        let defaults = test_defaults();
        let at = |x: f32| Character::new(1, Vec3::new(x, 0.0, 0.0), 0.0, &defaults);
        scene.characters.insert(PLAYER, at(0.0));
        for (id, x) in [(10, 2900.0), (11, 3500.0), (12, 4500.0), (13, 4500.0)] {
            scene.characters.insert(id, at(x));
        }
        scene.characters.get_mut(&13).unwrap().flags.set(flag::ALWAYS_ACTIVE, true);
        // The camera is 140 from a hundred over where she stands.
        let eye = Vec3::new(0.0, 100.0, -140.0);
        let reach = |scene: &mut Scene| {
            scene.reach_characters(eye, 4000.0, 3000.0, 150.0);
            [PLAYER, 10, 11, 12, 13].map(|id| (scene.characters[&id].asleep, scene.characters[&id].unseen))
        };
        // She is too near the camera to be drawn. One 3500 from her has its frame
        // and is not drawn, one 4500 from her has neither and no time goes by for
        // it, but for the one that keeps taking part.
        assert_eq!(reach(&mut scene), [(false, true), (false, false), (false, true), (true, true), (false, false)]);
        assert_eq!(scene.characters[&12].time(), 0);
        assert!(scene.characters[&11].time() > 0 && !scene.characters[&11].drawn() && scene.characters[&10].drawn());
        // A script lets the camera come nearer, and has everyone seen and near.
        scene.too_close = Some(100.0);
        assert_eq!(reach(&mut scene)[0], (false, false));
        scene.too_close = None;
        scene.set_flag(field_flag::ALL_IN_SIGHT, true);
        scene.set_flag(field_flag::ALL_NEAR, true);
        assert_eq!(reach(&mut scene), [(false, false); 5]);
        // What is an effect is not asked how near the camera it is.
        scene.set_flag(field_flag::ALL_IN_SIGHT, false);
        assert!(reach(&mut scene)[0].1);
        scene.characters.get_mut(&PLAYER).unwrap().chr = NO_RESOURCE;
        assert!(!reach(&mut scene)[0].1);
    }

    /// The volume and the place a sound was last given, of what a scene has asked.
    fn given(scene: &Scene, handle: u32) -> (Option<i32>, Option<i32>) {
        let volume = scene.sounds.iter().rev().find_map(|sound| match sound {
            Request::EffectVolume(of, volume) if *of == handle => Some(*volume),
            _ => None,
        });
        let place = scene.sounds.iter().rev().find_map(|sound| match sound {
            Request::EffectPan(of, place) if *of == handle => Some(*place),
            _ => None,
        });
        (volume, place)
    }

    #[test]
    fn a_sound_about_the_map_is_as_loud_as_she_is_near_it() {
        let mut scene = empty();
        // A fire burning at the origin, heard from 800 away, from 60 up to 127.
        scene.ambience.insert(1, Ambient::new(7, 60, 127, Vec3::ZERO, 800.0));
        // The camera stands 500 behind her as she looks along +z: at the angle of
        // a half turn about what it looks at.
        let behind = |she: Vec3| she - Vec3::Z * 500.0;
        let half_turn = std::f32::consts::PI;

        // Out of range it is as loud as at its range's edge, in the middle.
        let far = Vec3::new(0.0, 0.0, -1000.0);
        scene.sound_ambience(far, behind(far), half_turn);
        assert_eq!(given(&scene, 7), (Some(60), Some(MIDDLE)));
        // Asked again with nothing changed, nothing is asked of the sound.
        let asked = scene.sounds.len();
        scene.sound_ambience(far, behind(far), half_turn);
        assert_eq!(scene.sounds.len(), asked);

        // Half way in, with the fire straight ahead: half way up, in the middle.
        let near = Vec3::new(0.0, 0.0, -400.0);
        scene.sound_ambience(near, behind(near), half_turn);
        assert_eq!(given(&scene, 7), (Some(93), Some(MIDDLE)));
        // At the fire it is as loud as it gets.
        scene.sound_ambience(Vec3::new(0.0, 0.0, -0.5), Vec3::new(0.0, 0.0, -500.0), half_turn);
        assert_eq!(given(&scene, 7).0, Some(126));

        // Looking along +z, what has more x is to the left. With the fire off to
        // her left as the camera looks it is heard to the left; to her right, to
        // the right.
        let beside = Vec3::new(-300.0, 0.0, 0.0);
        scene.sound_ambience(beside, behind(beside), half_turn);
        let (_, place) = given(&scene, 7);
        assert!(place.unwrap() < MIDDLE, "{place:?}");
        let beside = Vec3::new(300.0, 0.0, 0.0);
        scene.sound_ambience(beside, behind(beside), half_turn);
        let (_, place) = given(&scene, 7);
        assert!(place.unwrap() > MIDDLE, "{place:?}");

        // Turned down to half over ten frames, it is half as loud.
        scene.ambience_level.go(50, 10);
        for _ in 0..11 {
            scene.ambience_level.step();
        }
        assert_eq!(scene.ambience_level.now, 0.5);
        scene.sound_ambience(far, behind(far), half_turn);
        assert_eq!(given(&scene, 7).0, Some(30));
        // During a battle none of it is heard, and nothing is worked out.
        scene.hear_ambience(false);
        assert_eq!(given(&scene, 7).0, Some(0));
        let asked = scene.sounds.len();
        scene.sound_ambience(near, behind(near), half_turn);
        assert_eq!(scene.sounds.len(), asked);
    }

    #[test]
    fn a_sound_is_heard_to_the_side_it_is_on() {
        // Straight ahead it is in the middle; to the left of that the place falls
        // to 3, and comes back up towards the middle behind; to the right it is
        // the same the other way.
        assert_eq!(place_of(0.0), MIDDLE);
        assert_eq!(place_of(45.0), MIDDLE - 30);
        assert_eq!(place_of(90.0), MIDDLE - 60);
        assert_eq!(place_of(91.0), MIDDLE - 0x3d);
        assert_eq!(place_of(179.0), MIDDLE - 9);
        assert_eq!(place_of(315.0), MIDDLE + 30);
        assert_eq!(place_of(270.0), MIDDLE + 60);
        assert_eq!(place_of(269.0), MIDDLE + 0x3e);
        assert_eq!(place_of(181.0), MIDDLE + 9);
    }

    #[test]
    fn the_loudness_of_the_sounds_about_the_map_moves_by_the_frame() {
        let mut level = AmbienceLevel::FULL;
        level.go(50, 2);
        level.step();
        assert_eq!(level.now, 0.75);
        level.step();
        assert_eq!(level.now, 0.5);
        level.step();
        level.step();
        assert_eq!(level.now, 0.5);
        // With no frames to take it is there the next frame.
        level.go(100, 0);
        assert_eq!(level.now, 0.5);
        level.step();
        assert_eq!(level.now, 1.0);
    }
}

#[cfg(test)]
mod glare_tests {
    use super::*;

    #[test]
    fn the_glare_goes_towards_what_a_script_says_and_only_all_the_screen_s_is_seen() {
        let mut glare = Glare::default();
        glare.step();
        assert_eq!(glare.shown(), 0.0);
        // The opening's hall: 95 in 255, a tenth of the way each frame.
        glare.set(95);
        glare.step();
        assert!((glare.shown() - 95.0 / 2550.0).abs() < 1e-6);
        for _ in 0..90 {
            glare.step();
        }
        assert!((glare.shown() - 95.0 / 255.0).abs() < 1e-4);
        // With only what is marked taking part there is nothing to see of it,
        // though it keeps its level for when all the screen takes part again.
        glare.marked_only = true;
        assert_eq!(glare.shown(), 0.0);
        glare.set(-1);
        assert_eq!((glare.strength, glare.marked_only), (1.0, false));
        assert!(glare.shown() > 0.3);
    }
}