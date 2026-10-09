//! The field's camera as scripts and the map's traps have it: its kind, its nineteen
//! figures, what moves them, and what a trap gives it while she is inside. Where all
//! that puts the view is worked out in `field/mod.rs`. See `docs/FIELD_NOTES.md`,
//! "The camera".

use bevy::prelude::*;

/// How many figures the camera has (its slots, at the camera's + 0x7A8).
pub const FIGURES: usize = 19;

/// The camera's figures, by the numbers scripts know them by (`s79`, 0xC4484; an
/// event's function 0x4F, 0xD73CC): what the original keeps in the camera's record
/// (0xDA700). Angles are in degrees here, as scripts give them.
pub mod figure {
    /// How wide it sees, from side to side: the record's figure is what the
    /// projection is made with (0xD9DF8 to 0x239BC), which takes it for the screen's
    /// width.
    pub const FIELD_OF_VIEW: i32 = 1;
    /// How near and how far it sees (the record's + 0x6C and + 0x70, 0xE28B4 and
    /// 0xE28EC): the two figures that have no slot, so that nothing moves them
    /// and no trap gives them.
    pub const NEAR: i32 = 2;
    pub const FAR: i32 = 3;
    /// Where a camera that stays put is; one that follows her keeps here where it
    /// has got to.
    pub const PLACE: [i32; 3] = [5, 6, 7];
    pub const PITCH: i32 = 8;
    /// The turn about her of where the camera is: behind her is her facing and a
    /// half turn.
    pub const YAW: i32 = 9;
    pub const ROLL: i32 = 10;
    /// What it looks at, from her feet.
    pub const LOOK_AT: [i32; 3] = [11, 12, 13];
    /// Where it is from what it looks at, before it is turned.
    pub const OFFSET: [i32; 3] = [14, 15, 16];
    /// The place a camera looks at her from, when of a kind that does.
    pub const POINT: [i32; 3] = [17, 18, 19];
    /// How far the picture is moved over, as the camera sees it (0xDE310).
    pub const SHIFT: [i32; 3] = [20, 21, 22];

    /// Which of the camera's figures a script's number is (0xDC2E4): the angle it
    /// sees is the first, and from 5 on they follow one another. 2 and 3 are how
    /// near and how far it sees, which the port does not have.
    pub fn slot(what: i32) -> Option<usize> {
        match what {
            FIELD_OF_VIEW => Some(0),
            5..=22 => Some(what as usize - 4),
            _ => None,
        }
    }

    /// Which of them a script's number is for what a trap gives the camera (`s45`,
    /// 0xC8588): they are numbered otherwise there.
    pub fn slot_of_a_trap(what: i32) -> Option<usize> {
        match what {
            6 => Some(0),
            10..=12 => Some(what as usize - 9),
            7..=9 => Some(what as usize - 3),
            14..=25 => Some(what as usize - 7),
            _ => None,
        }
    }

    pub(super) const PITCH_SLOT: usize = 4;
    pub(super) const YAW_SLOT: usize = 5;
}

/// The kinds of camera (`s49`, 0xDA2A0).
pub mod kind {
    /// Behind her, as far off as its offset, turned by its pitch and its yaw.
    pub const FOLLOW: i32 = 0;
    /// At its place, turned by its angles.
    pub const FIXED: i32 = 1;
    /// At its point, turned to her.
    pub const POINT: i32 = 3;
    /// Where a scene's cut has it.
    pub const CUT: i32 = 4;
    /// Along a path of the map's, as far along as she is.
    pub const PATH: i32 = 5;
    /// Turned to her from the way of its point, as far off her as its offset.
    pub const POINT_AT_A_DISTANCE: i32 = 6;
    /// As the first, by another name (`SET_AROUND_CAM`).
    pub const AROUND: i32 = 7;

    pub fn follows(kind: i32) -> bool {
        matches!(kind, FOLLOW | AROUND)
    }

    /// Whether a camera of `kind` works figure `slot` out for itself (0xDE39C), so
    /// that what it leaves there is nothing a camera of another kind can go by: one
    /// that follows her its place, its yaw and where it has got to; one that looks
    /// from a point its pitch, its yaw and how far off it is; one that stays put
    /// everything from its place to its offset.
    pub fn makes(kind: i32, slot: usize) -> bool {
        match kind {
            FOLLOW | AROUND => matches!(slot, 1 | 2 | 3 | 5 | 13 | 14 | 15),
            FIXED => (1..=12).contains(&slot),
            POINT => matches!(slot, 4 | 5 | 12),
            _ => false,
        }
    }
}

/// How far a change has got that takes `frames` and is `elapsed` of them in, from 0
/// to 1, in one of four ways: evenly, slowly at first, slowly at the end, or both,
/// each half then having half of the frames, a whole number of them (0xDCCD0,
/// 0xDCEE4). None for any other way.
fn part(ease: i32, elapsed: f32, frames: i32) -> Option<f32> {
    let even = elapsed / frames as f32;
    match ease {
        0 => Some(even),
        1 => Some(even * even),
        2 => Some(1.0 - (1.0 - even) * (1.0 - even)),
        3 => {
            let half = (frames / 2) as f32;
            Some(if elapsed < half {
                let part = elapsed / half;
                0.5 * part * part
            } else {
                let left = 1.0 - (elapsed - half) / half;
                0.5 + 0.5 * (1.0 - left * left)
            })
        }
        _ => None,
    }
}

/// What a trap gives the camera while she is inside it (`s45`; the trap's + 0xB4 on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrapCamera {
    /// The figures it has for the camera, by which of the nineteen they are.
    pub figures: [Option<f32>; FIGURES],
    /// The kind of camera it asks for, if it asks for one, and the path of a camera
    /// that goes along one.
    pub kind: Option<i32>,
    pub path: i32,
    /// How many frames the camera takes over what coming into it or leaving it
    /// changes, and how it goes about it.
    pub frames: i32,
    pub ease: i32,
}

impl Default for TrapCamera {
    /// A trap as it is made (0xD3980): a script that says nothing of the frames gets
    /// ninety of them.
    fn default() -> TrapCamera {
        TrapCamera { figures: [None; FIGURES], kind: None, path: 0, frames: 90, ease: 0 }
    }
}

impl TrapCamera {
    /// `s45(trap, what, value)` for what is the camera's: says whether it was.
    pub fn set(&mut self, what: i32, value: i32) -> bool {
        match what {
            13 => self.frames = value,
            26 => self.kind = Some(value & 0xff),
            27 => self.path = value,
            // One of four ways, any other number being the first (0xC8948).
            28 => self.ease = if (0..=3).contains(&value) { value } else { 0 },
            _ => match figure::slot_of_a_trap(what) {
                Some(slot) => self.figures[slot] = Some(value as f32),
                None => return false,
            },
        }
        true
    }
}

/// One of the camera's figures as the field keeps it beside the camera's record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Slot {
    /// What the script has it at: what it last set while no camera was put by.
    script: f32,
    /// Where it is going, or is: the script's value, or that of a trap she is in.
    target: f32,
    /// The trap the target is from, if it is from one.
    given_by: Option<i32>,
    /// The trap she last came into or left that has this figure, and which of the
    /// two she did, until the camera has seen to it.
    marked_by: Option<i32>,
    entered: bool,
    left: bool,
    moving: Option<Moving>,
    /// How it goes when it moves.
    ease: i32,
}

/// A figure on its way to its target.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Moving {
    from: f32,
    /// The frames of the game it takes, and those gone by.
    frames: i32,
    elapsed: i32,
}

/// A camera put by for the time of an event (`s50`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Kept {
    kind: i32,
    now: [f32; FIGURES],
    path: i32,
    sees: [f32; 2],
}

/// The view going over from what it was to what the camera makes of it now (0xDB11C,
/// 0xDCEDC): where it is goes evenly from the one to the other and so does its turn,
/// by the part of the way the frames gone by make.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blend {
    frames: i32,
    elapsed: i32,
    ease: i32,
    /// The view it started from: what the camera showed when it was asked for,
    /// which whoever draws the view fills in.
    pub from: Option<(Vec3, Quat)>,
}

impl Blend {
    /// How much of the new view there is, from 0 to 1, `into` of a frame of the
    /// game after the last one.
    pub fn part(&self, into: f32) -> f32 {
        part(self.ease, (self.elapsed as f32 + into).min(self.frames as f32), self.frames).unwrap_or(0.0)
    }
}

/// How many cameras can be put by at once (0xDA33C).
const CAMERAS_KEPT: usize = 2;
/// The frames a change of the camera's kind is spread over when no trap says
/// (0xDE614).
const FRAMES_OF_A_CHANGE: i32 = 30;

/// How many frames the stick takes to go by a view the camera has cut to (the
/// 16-bit figure at + 0x40 of the field's `prm`, 0xDC904).
pub const STICK_FRAMES: i32 = 45;

#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// Its kind ([`kind`]). A map's camera is one that stays put until a script or
    /// a trap makes it another (0xDC650).
    pub kind: i32,
    /// Its figures as they are (the camera's record).
    now: [f32; FIGURES],
    /// How near and how far it sees (`figure::NEAR`, `figure::FAR`): what the
    /// lens the map and its people are drawn through is made with (0xD9DF8). A
    /// scene's script brings the first in for a close shot, or puts it out past
    /// what stands between the camera and what it is to show.
    sees: [f32; 2],
    /// The path it goes along, when it is of that kind (`s124`).
    pub path: i32,
    kept: Vec<Kept>,
    slots: [Slot; FIGURES],
    /// The kind and the path the script has it at, as with the figures.
    script_kind: i32,
    script_path: i32,
    /// The traps she is in, in the order she came into them (the field's list at
    /// + 0x1DF8).
    inside: Vec<i32>,
    /// The trap the camera has its kind from (the camera's + 0xA54).
    kind_from: Option<i32>,
    /// The view is going over from the last to this one.
    pub blend: Option<Blend>,
    /// The view is to be where the camera has it at once, with none of the lag it
    /// follows its place with: counted up each time (0xDBB58).
    pub cuts: u32,
    /// How often a camera has been made anew or taken up again after being put
    /// by: the counts the view trails its place by start from nothing then
    /// (0xDA3C0).
    pub renewed: u32,
    /// How often a script has set it up: the port's own controls start afresh then.
    pub set: u32,
    /// The view the stick goes by (the camera's + 0x50), as its yaw: the view's
    /// own, but that after a cut it goes over to it from what it was (field flag
    /// 0x44; 0xDBC6C keeps the old one and 0xDFC48 turns from it to the new by
    /// the square of the share of the frames gone by). The yaw it went by at the
    /// cut, and the frames since.
    stick: Option<(f32, i32)>,
    stick_yaw: f32,
    /// How much the pitch of one that follows her is leant by, in degrees: what
    /// the camera's + 0xBDC, its pitch as it leans, is more than the record's.
    /// None until it has been worked out.
    leaning: Option<f32>,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera::new()
    }
}

impl Camera {
    /// The camera a map starts with (0xDC428): it looks down a little, at a point
    /// 110 over her feet, from 492 behind it, and sees 45 degrees from top to bottom
    /// (0x249E8 makes the width of that for the PSP's screen). What the script is
    /// taken to have set is not quite that: no pitch, and a point 150 over her feet.
    pub fn new() -> Camera {
        const WIDTH: f32 = 72.337;
        let mut now = [0.0; FIGURES];
        (now[0], now[4], now[8], now[12]) = (WIDTH, -7.5947, 110.0, 492.0);
        let mut slots = [Slot::default(); FIGURES];
        (slots[0].script, slots[8].script, slots[12].script) = (WIDTH, 150.0, 492.0);
        Camera {
            kind: kind::FIXED,
            now,
            sees: [Self::NEAR, Self::FAR],
            path: 0,
            kept: Vec::new(),
            slots,
            script_kind: kind::FOLLOW,
            script_path: 0,
            inside: Vec::new(),
            kind_from: None,
            blend: None,
            cuts: 0,
            renewed: 0,
            set: 0,
            stick: None,
            stick_yaw: 0.0,
            leaning: None,
        }
    }

    /// How much a camera that follows her leans its pitch by, in degrees: nothing
    /// on level ground, and nothing before it is worked out.
    pub fn leaning(&self) -> f32 {
        self.leaning.unwrap_or(0.0)
    }

    /// A frame of that (0xDE974, from 0xDEC44 on). `slope` is the pitch of the
    /// way from what it looks at to where its own pitch has it, taken as if the
    /// ground under it were as high as her feet; `rates` are how far the pitch
    /// leans to that where it is under the pitch and where it is over (the
    /// camera's + 0xBD0 and + 0xBD4), and `level` how much of the whole is taken
    /// away (+ 0xBD8): what a map's scene says, nought all three where it says
    /// nothing. With no slope to go by it is the pitch itself.
    ///
    /// The original keeps the pitch so leant (its + 0xBDC) and, read to the
    /// letter, adds it to the record's as it works the place out (0xDB60C),
    /// which would have the camera look down twice as far on level ground. It
    /// does not: the walkthrough shows the record's pitch there
    /// (`docs/compare/follow_pitch_walkthrough.jpg`). So the port has the
    /// pitch so leant be the pitch.
    pub fn lean(&mut self, slope: Option<f32>, rates: [f32; 2], level: f32) {
        let pitch = self.now[figure::PITCH_SLOT];
        let leant = match slope {
            Some(slope) => pitch + (slope - pitch) * if slope < pitch { rates[0] } else { rates[1] },
            None => pitch,
        };
        self.leaning = Some((1.0 - level) * leant - pitch);
    }

    /// The yaw of the view the stick is taken by, in degrees.
    pub fn stick_yaw(&self) -> f32 {
        self.stick_yaw
    }

    /// How near and how far a map's camera sees at first (0xDC6E0).
    const NEAR: f32 = 50.0;
    const FAR: f32 = 20_000.0;

    /// A figure as it is now, by a script's number for it.
    pub fn figure(&self, what: i32) -> f32 {
        match what {
            figure::NEAR => self.sees[0],
            figure::FAR => self.sees[1],
            _ => figure::slot(what).map_or(0.0, |slot| self.now[slot]),
        }
    }

    /// Three figures that are one thing: a place, or a way.
    pub fn figures(&self, what: [i32; 3]) -> Vec3 {
        Vec3::from(what.map(|what| self.figure(what)))
    }

    /// Whether a camera is put by, so that what is set is an event's and not what
    /// the script has the field's camera at (0xDA450).
    fn put_by(&self) -> bool {
        !self.kept.is_empty()
    }

    /// `s79(what, value)`, an event's function 0x4F (0xE2864 on): a figure. One that
    /// is on its way somewhere goes on from there the next frame.
    pub fn set(&mut self, what: i32, value: f32) {
        match what {
            figure::NEAR => self.sees[0] = value,
            figure::FAR => self.sees[1] = value,
            _ => {}
        }
        let Some(slot) = figure::slot(what) else { return };
        self.now[slot] = value;
        if !self.put_by() {
            self.slots[slot].script = value;
        }
        if what == figure::YAW {
            self.set += 1;
        }
    }

    /// The camera has worked a figure out for itself: where it has got to, or how
    /// it is turned to her.
    pub fn made(&mut self, what: i32, value: f32) {
        if let Some(slot) = figure::slot(what) {
            self.now[slot] = value;
        }
    }

    /// The player has turned a camera that follows her (0xDEE10): its yaw is that,
    /// and the script is taken to have set it so (0xDF24C).
    pub fn turned(&mut self, yaw: f32) {
        self.now[figure::YAW_SLOT] = yaw;
        self.slots[figure::YAW_SLOT].script = yaw;
    }

    /// `s49(kind)`, an event's function 0x59 (0xDC290): the camera is of another
    /// kind, and whatever was moving its figures stops where it is (0xDA9EC).
    pub fn set_kind(&mut self, kind: i32) {
        self.stop();
        self.kind = kind;
        self.cuts += 1;
        self.set += 1;
        if !self.put_by() {
            self.script_kind = kind;
        }
    }

    /// `s124(path)` (0xE2F34): the path a camera that goes along one goes along.
    pub fn set_path(&mut self, path: i32) {
        self.path = path;
        if !self.put_by() {
            self.script_path = path;
        }
    }

    /// `s139(what, value, frames, ease)`, an event's function 0x5A (0xDC3B0): a
    /// figure goes to a value over so many frames of the game, from what it is.
    pub fn move_to(&mut self, what: i32, to: f32, frames: i32, ease: i32) {
        let Some(slot) = figure::slot(what) else { return };
        self.go(slot, frames, false);
        (self.slots[slot].target, self.slots[slot].ease) = (to, ease);
        if !self.put_by() {
            self.slots[slot].script = to;
        }
    }

    /// The camera as it is is put by for the time of an event (`s50`, 0xDA324): the
    /// event goes on with one like it.
    pub fn keep(&mut self) -> bool {
        let room = self.kept.len() < CAMERAS_KEPT;
        if room {
            self.kept.push(Kept { kind: self.kind, now: self.now, path: self.path, sees: self.sees });
        }
        room
    }

    /// And taken up again after it (`s51`, 0xDA40C): nothing of it is moving then
    /// (0xDA3A8).
    pub fn take_up(&mut self) -> bool {
        let Some(kept) = self.kept.pop() else { return false };
        (self.kind, self.now, self.path, self.sees) = (kept.kind, kept.now, kept.path, kept.sees);
        self.stop();
        self.cuts += 1;
        self.renewed += 1;
        self.set += 1;
        true
    }

    fn stop(&mut self) {
        for slot in &mut self.slots {
            slot.moving = None;
        }
    }

    /// A frame of the camera (0xDCC60): the figures that are on their way move on,
    /// and so does the view if it is going over to another.
    pub fn step(&mut self) {
        for (slot, now) in self.slots.iter_mut().zip(&mut self.now) {
            let Some(moving) = &mut slot.moving else { continue };
            moving.elapsed += 1;
            if moving.elapsed >= moving.frames {
                slot.moving = None;
                *now = slot.target;
            } else if let Some(part) = part(slot.ease, moving.elapsed as f32, moving.frames) {
                // With a way that is none of the four it stays as it is until the
                // frames are up.
                *now = moving.from + (slot.target - moving.from) * part;
            }
        }
        if let Some(blend) = &mut self.blend {
            blend.elapsed += 1;
            if blend.elapsed >= blend.frames {
                self.blend = None;
            }
        }
        // What the stick goes by (0xDC8E4): the view, or after a cut what lies
        // between the old one and it.
        let yaw = self.now[figure::YAW_SLOT];
        self.stick_yaw = match &mut self.stick {
            Some((from, frames)) => {
                *frames += 1;
                let share = *frames as f32 / STICK_FRAMES as f32;
                // The shorter way round.
                let round = (yaw - *from + 180.0).rem_euclid(360.0) - 180.0;
                *from + round * share * share
            }
            None => yaw,
        };
        if self.stick.is_some_and(|(_, frames)| frames >= STICK_FRAMES) {
            (self.stick, self.stick_yaw) = (None, yaw);
        }
    }

    /// A figure goes to its target (0xDB080): over so many frames, or at once.
    fn go(&mut self, slot: usize, frames: i32, at_once: bool) {
        if at_once {
            self.now[slot] = self.slots[slot].target;
        } else {
            self.slots[slot].moving = Some(Moving { from: self.now[slot], frames, elapsed: 0 });
        }
    }

    /// The view goes over to what the camera makes of it from here on (0xDB11C):
    /// over so many frames, or with none at once.
    fn blend_over(&mut self, frames: i32, ease: i32) {
        if frames == 0 {
            self.blend = None;
            self.cuts += 1;
            // The stick is not thrown by it (0xDBC6C).
            self.stick = Some((self.stick_yaw, 0));
        } else {
            self.blend = Some(Blend { frames, elapsed: 0, ease, from: None });
        }
    }

    /// Whether she is in trap `id` as the camera has it.
    pub fn inside(&self, id: i32) -> bool {
        self.inside.contains(&id)
    }

    /// She has come into a trap (0x11709C, 0xDAF78): the figures it has for the
    /// camera are marked as come into.
    pub fn enter(&mut self, id: i32, trap: &TrapCamera) {
        self.inside.push(id);
        for (slot, given) in self.slots.iter_mut().zip(trap.figures) {
            if given.is_some() {
                (slot.entered, slot.marked_by, slot.ease) = (true, Some(id), trap.ease);
            }
        }
    }

    /// She has left one (0xDAFF8, 0x1170F0): the figures the camera has from it are
    /// marked as left.
    pub fn leave(&mut self, id: i32, trap: &TrapCamera) {
        for (slot, given) in self.slots.iter_mut().zip(trap.figures) {
            if given.is_some() && slot.given_by == Some(id) {
                (slot.left, slot.marked_by) = (true, Some(id));
            }
        }
        self.forget(id);
    }

    /// A trap she is in has been switched off: it is out of the camera's list, and
    /// the camera is told nothing (0x10DE68).
    pub fn forget(&mut self, id: i32) {
        self.inside.retain(|&other| other != id);
    }

    /// The camera takes what the traps she is in have for it (0xDE460), after she
    /// has come into one or left one. Each figure is the first's of those traps that
    /// has it, the one she came into first, or else what the script has it at; and
    /// so with the camera's kind. If the kind is to change, every figure is that at
    /// once and the view goes over to the new camera's; if not, the figures of the
    /// trap come into or left go to what they now are over that trap's frames, but
    /// for the yaw of a camera that follows her, which stays the player's. `at_once`
    /// is after an event, when nothing takes any time.
    pub fn resolve(&mut self, trap: impl Fn(i32) -> Option<TrapCamera>, at_once: bool) {
        for slot in &mut self.slots {
            (slot.target, slot.given_by) = (slot.script, None);
        }
        let (mut wanted, mut path, mut kind_from) = (self.script_kind, self.script_path, None);
        for &id in &self.inside {
            let Some(trap) = trap(id) else { continue };
            for (slot, given) in self.slots.iter_mut().zip(trap.figures) {
                if let (None, Some(value)) = (slot.given_by, given) {
                    (slot.target, slot.given_by) = (value, Some(id));
                }
            }
            if let (None, Some(kind)) = (kind_from, trap.kind) {
                (wanted, path, kind_from) = (kind, trap.path, Some(id));
            }
        }
        let frames_of = |id: Option<i32>| id.and_then(&trap).map(|trap| (trap.frames, trap.ease));
        // What a change of kind or of path goes by: the trap the camera had its
        // kind from, or else the one it is to have it from.
        let change = frames_of(self.kind_from).or(frames_of(kind_from)).unwrap_or((FRAMES_OF_A_CHANGE, 0));

        if self.kind != wanted && !at_once {
            for slot in 0..FIGURES {
                self.go(slot, 0, true);
            }
            self.blend_over(change.0, change.1);
            self.kind = wanted;
            self.cuts += 1;
            if wanted == kind::PATH {
                self.path = path;
            }
            self.kind_from = kind_from;
        } else {
            for index in 0..FIGURES {
                let slot = self.slots[index];
                let marked = frames_of(slot.marked_by).map_or(0, |(frames, _)| frames);
                let come_into = slot.entered && slot.marked_by == slot.given_by;
                if self.kind == wanted {
                    if kind::follows(wanted) && index == figure::YAW_SLOT {
                        continue;
                    }
                    if come_into || slot.left {
                        self.go(index, marked, at_once);
                    }
                } else if come_into || (slot.left && !kind::makes(wanted, index)) {
                    self.go(index, marked, at_once);
                } else if kind::makes(self.kind, index) && !kind::makes(wanted, index) {
                    self.go(index, change.0, at_once);
                }
            }
            if self.path != path {
                self.blend_over(change.0, change.1);
                self.path = path;
            }
            if self.kind != wanted {
                self.kind = wanted;
                self.cuts += 1;
                self.kind_from = kind_from;
            } else if wanted == kind::PATH {
                self.cuts += 1;
            }
        }
        for slot in &mut self.slots {
            (slot.entered, slot.left) = (false, false);
        }
    }

    /// The field is taken up again after a battle (0xE8A84 on): its camera is made
    /// anew, of the kind the script had the old one at, with what the script had of
    /// its angles, of its place or of its point, by its kind.
    pub fn resumed(&self) -> Camera {
        let mut next = Camera::new();
        (next.kind, next.script_kind) = (self.script_kind, self.script_kind);
        let kept: &[usize] = match self.script_kind {
            kind::FOLLOW | kind::AROUND => &[4, 5, 6],
            kind::POINT | kind::POINT_AT_A_DISTANCE => &[13, 14, 15],
            kind::FIXED => &[1, 2, 3, 4, 5, 6],
            _ => &[],
        };
        for &slot in kept {
            // A camera that follows her is not rolled.
            let value = if kind::follows(self.script_kind) && slot == 6 { 0.0 } else { self.slots[slot].script };
            (next.now[slot], next.slots[slot].script) = (value, value);
        }
        // How near and how far it sees are a new camera's own again: fifty and
        // twenty thousand (0xDC428, which 0xEFB60 calls for it).
        next.set = self.set + 1;
        next.cuts = self.cuts + 1;
        next.renewed = self.renewed + 1;
        next
    }

    /// What a camera is made anew from, as the game keeps it with where she is
    /// (0x1D64, each frame of the camera, 0xDCC28: the block's + 9 and six
    /// floats, which a save has with the rest).
    pub fn record(&self) -> Record {
        Record { kind: self.script_kind, figures: RECORD_SLOTS.map(|slot| self.slots[slot].script) }
    }

    /// The camera a record gives back (0x1DC0, 0xE8A90): made anew from it as
    /// after a battle, which is how a game loaded on the field has its camera.
    pub fn restored(&self, record: &Record) -> Camera {
        let mut from = Camera::new();
        from.script_kind = record.kind;
        for (slot, value) in RECORD_SLOTS.into_iter().zip(record.figures) {
            from.slots[slot].script = value;
        }
        (from.set, from.cuts, from.renewed) = (self.set, self.cuts, self.renewed);
        from.resumed()
    }
}

/// The figures a camera's record has, whatever its kind: its place, its
/// angles and its point (the original keeps the six that go with the kind).
const RECORD_SLOTS: [usize; 9] = [1, 2, 3, 4, 5, 6, 13, 14, 15];

/// The kind a script has a camera at and what it has of its figures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub kind: i32,
    pub figures: [f32; 9],
}

/// How a camera that follows her gets to where its figures have it (0xDF28C): not at
/// once, but by gathering speed towards the place and losing it again in time to
/// stop there. It is then kept on the side of her that the place is on
/// (0xDF6D4): it lags in how far off and how high it is, never in the way round.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lag {
    /// Where it is; nowhere yet after a cut, when it starts where it is to be.
    place: Option<Vec3>,
    speed: Vec3,
    /// For how many frames the walls have left the camera alone, to 32, going
    /// down again while they stop it (the camera's + 0xC58, 0xDE9EC); and for
    /// how many it has been more than 18 behind its place, to 16, going down
    /// while it is nearer (+ 0xC5C, 0xDF340).
    clear: i32,
    behind: i32,
}

impl Lag {
    /// The most its speed changes by in a frame, and how far from its place it
    /// starts to hurry (0xDC548, 0xDC558).
    const MOST: f32 = 36.0;
    const SLACK: f32 = 10.0;
    /// How much further from what it looks at than its figures have it it may be
    /// (0xD9B94), and how much where the walls stopped the camera (0xD9B98).
    const GIVE: f32 = 0.5;
    const GIVE_STOPPED: f32 = 5.0;
    /// What the two counts go up to, and how far behind it must be for the
    /// second to go up (0xDEA18, 0xDF360, 0xDF340).
    const CLEAR: i32 = 32;
    const BEHIND: i32 = 16;
    const BEHIND_BY: f32 = 18.0;

    /// The next frame starts from where the camera is to be (0xDBB58): the
    /// counts are kept.
    pub fn cut(&mut self) {
        (self.place, self.speed) = (None, Vec3::ZERO);
    }

    /// The camera is a new one, or one taken up again after being put by
    /// (0xDA3C0): the counts start from nothing, so the view is where the
    /// camera is and comes to trail it again.
    pub fn renew(&mut self) {
        (self.clear, self.behind) = (0, 0);
    }

    /// A frame: where the view is from, the camera being after `goal` and
    /// looking at `target`. `up` is the upright of her frame, and `stopped` that
    /// the walls kept the camera from where its figures would have it.
    ///
    /// The lagging place goes after `goal` and is kept on its side of her. The
    /// view (0xD9B14) is from `goal` where that is no further from what is
    /// looked at than the lagging place, give or take a little, and else from
    /// the lagging place; but from `goal` it is drawn back towards the lagging
    /// place by the two counts, all the way when the walls have left the camera
    /// alone for long and it has been well behind for long. So in the open the
    /// camera trails her both ways, and by a wall it does not.
    pub fn step(&mut self, goal: Vec3, target: Vec3, up: Vec3, stopped: bool) -> Vec3 {
        self.clear = if stopped { (self.clear - 1).max(0) } else { (self.clear + 1).min(Self::CLEAR) };
        if let Some(place) = self.place {
            let near = goal.distance(place) < Self::BEHIND_BY;
            self.behind = if near { (self.behind - 1).max(0) } else { (self.behind + 1).min(Self::BEHIND) };
        }
        let lagging = Self::kept_behind(self.chase(goal), goal, target, up, stopped);
        self.place = Some(lagging);
        let give = if stopped { Self::GIVE_STOPPED } else { Self::GIVE };
        let from = if goal.distance(target) < lagging.distance(target) + give { goal } else { lagging };
        let back = (self.clear as f32 / Self::CLEAR as f32) * (self.behind as f32 / Self::BEHIND as f32);
        from + (lagging - from) * back
    }

    /// Where the lagging `place` is put when it has moved (0xDF6D4). Seen from
    /// what the camera looks at, with what is along the upright left out, it is
    /// turned the way `goal` lies: about what is square to both, or about the
    /// world's upright when it lies right behind. And where the walls stopped
    /// the camera it is no further off than `goal` is.
    fn kept_behind(place: Vec3, goal: Vec3, target: Vec3, up: Vec3, stopped: bool) -> Vec3 {
        const NONE: f32 = 1e-5;
        const AS_ONE: f32 = 0.99999;
        let (to_goal, mut to_place) = (goal - target, place - target);
        let (far, lagging) = (to_goal.length(), to_place.length());
        if far < NONE {
            return goal;
        }
        let across = |way: Vec3| way - up * up.dot(way);
        let (flat_goal, flat_place) = (across(to_goal / far), across(to_place.normalize_or_zero()));
        if flat_goal.length() < NONE || flat_place.length() < NONE {
            return goal;
        }
        let (flat_goal, flat_place) = (flat_goal.normalize(), flat_place.normalize());
        let cosine = flat_goal.dot(flat_place);
        let about = if cosine <= -AS_ONE {
            Some(Vec3::Y)
        } else if cosine < AS_ONE {
            flat_goal.cross(flat_place).try_normalize()
        } else {
            None
        };
        if let Some(about) = about {
            to_place = Quat::from_axis_angle(about, -cosine.clamp(-1.0, 1.0).acos()) * to_place;
        }
        if stopped && far < lagging {
            to_place = to_place.normalize() * far;
        }
        target + to_place
    }

    fn chase(&mut self, goal: Vec3) -> Vec3 {
        let Some(place) = self.place else {
            self.speed = Vec3::ZERO;
            return goal;
        };
        let way = goal - place;
        let far = way.length();
        let speed = self.speed.length();
        // There, or there within this frame's move.
        if far < 0.1 || far < speed {
            self.speed = Vec3::ZERO;
            return goal;
        }
        let toward = way / far;
        let along = toward.dot(self.speed);
        // Its speed towards the place, and the rest of it, which is across.
        let mut forth = toward * along;
        let mut across = self.speed - forth;
        let (going, drift) = (forth.length(), across.length());

        // The frames it would take from rest, at the most it can gather.
        let frames = (far * 2.0 / Self::MOST).sqrt();
        let covered = going * frames;
        if far < covered * 2.0 || far < Self::SLACK {
            // Near enough to be slowing down: by what stops it at the place.
            let slowing = (going * going / (far * 2.0)).min(Self::MOST);
            forth += toward * if along <= 0.0 { slowing } else { -slowing };
        } else {
            let room = (far - covered).min(Self::SLACK);
            let gathered = Self::MOST * frames;
            let more = (gathered * gathered - going * going).sqrt() / (room * 2.0);
            // A number that is none (it was going faster than it can gather) is the
            // most, as the original's comparison has it.
            forth += toward * if more <= Self::MOST { more } else { Self::MOST };
        }
        // What it has of speed across the way dies out, by no more a frame than
        // the most.
        if Self::MOST < drift {
            across -= across / drift * Self::MOST;
        } else {
            across = Vec3::ZERO;
        }
        self.speed = forth + across;
        place + self.speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_gathers_speed_to_where_it_is_to_be_and_stops_there() {
        let target = Vec3::new(0.0, 0.0, -1000.0);
        let mut lag = Lag::default();
        // After a cut it is where it is to be.
        assert_eq!(lag.step(Vec3::ZERO, target, Vec3::Y, false), Vec3::ZERO);
        // Its place moves 300 further from what it looks at: it sets off after it,
        // faster each frame at first, slows, and is all but there within twenty
        // frames without having gone past; a few more and it is there.
        let goal = Vec3::new(0.0, 0.0, 300.0);
        let mut seen = vec![0.0];
        for _ in 0..20 {
            seen.push(lag.step(goal, target, Vec3::Y, false).z);
        }
        assert!(seen.windows(2).all(|pair| pair[1] >= pair[0]), "{seen:?}");
        assert!(seen[1] > 5.0 && seen[1] < 10.0 && seen[2] - seen[1] > seen[1], "{seen:?}");
        assert!(300.0 - *seen.last().unwrap() < 0.2, "{seen:?}");
        assert!(seen.iter().all(|&z| z <= 300.0), "{seen:?}");
        // At rest a while, it counts as behind no longer; its place then moves
        // nearer what it looks at, and the view is from all but there at once: a
        // sixteenth of the way back to the lagging place, which has just set off.
        for _ in 0..16 {
            lag.step(goal, target, Vec3::Y, false);
        }
        assert_eq!((lag.behind, lag.place), (0, Some(goal)));
        let view = lag.step(Vec3::new(0.0, 0.0, -200.0), target, Vec3::Y, false);
        assert!((view.z - (-200.0 + (lag.place.unwrap().z + 200.0) / 16.0)).abs() < 1e-3, "{view}");
        // And after a cut it starts from its place again.
        lag.step(Vec3::new(0.0, 0.0, -100.0), target, Vec3::Y, false);
        lag.cut();
        assert_eq!(lag.step(Vec3::new(50.0, 0.0, 0.0), target, Vec3::Y, false), Vec3::new(50.0, 0.0, 0.0));
        // A cut leaves the counts; a camera taken up again has them at nothing.
        assert!(lag.clear > 0);
        lag.renew();
        assert_eq!((lag.clear, lag.behind), (0, 0));
    }

    #[test]
    fn in_the_open_the_view_trails_her_when_the_camera_comes_in_too() {
        let (target, up) = (Vec3::new(0.0, 0.0, -1000.0), Vec3::Y);
        let mut lag = Lag::default();
        lag.step(Vec3::ZERO, target, up, false);
        // The walls leave it alone for long, and it is kept well behind for long:
        // its place runs away from it.
        let mut goal = Vec3::ZERO;
        for _ in 0..40 {
            goal.z += 60.0;
            lag.step(goal, target, up, false);
        }
        assert_eq!((lag.clear, lag.behind), (Lag::CLEAR, Lag::BEHIND));
        // Now its place comes in past it, nearer what it looks at: the view is
        // still from the lagging place, which is yet to turn back after it.
        let lagging = lag.place.unwrap();
        let nearer = Vec3::new(0.0, 0.0, lagging.z - 400.0);
        let view = lag.step(nearer, target, up, false);
        assert_eq!(view, lag.place.unwrap());
        assert!(view.z > nearer.z + 300.0, "{view}");
        // With the counts half run down the view is half way between the two.
        (lag.clear, lag.behind) = (Lag::CLEAR / 2, Lag::BEHIND + 1);
        let view = lag.step(nearer, target, up, false);
        let share = (Lag::CLEAR / 2 + 1) as f32 / Lag::CLEAR as f32;
        assert!((view.z - (nearer.z + (lag.place.unwrap().z - nearer.z) * share)).abs() < 1e-3, "{view}");
        // By a wall it does not trail: the lagging place is brought in to where
        // the walls stopped the camera, and the view is from there.
        for _ in 0..Lag::CLEAR {
            assert_eq!(lag.step(nearer, target, up, true), nearer);
        }
        assert_eq!(lag.clear, 0);
    }

    #[test]
    fn the_lagging_camera_is_kept_on_the_side_of_her_its_place_is_on() {
        let (target, up) = (Vec3::new(0.0, 100.0, 0.0), Vec3::Y);
        // The place has gone a quarter round her and further off; the lagging one
        // is still where it was, nearer and lower. It is turned to the place's
        // side, as far off and as high as it was.
        let (goal, lagging) = (Vec3::new(600.0, 160.0, 0.0), Vec3::new(0.0, 130.0, 400.0));
        let kept = Lag::kept_behind(lagging, goal, target, up, false);
        assert!((kept - Vec3::new(400.0, 130.0, 0.0)).length() < 1e-3, "{kept}");
        // Right behind where it should be, it is turned half round.
        let kept = Lag::kept_behind(Vec3::new(-400.0, 130.0, 0.0), goal, target, up, false);
        assert!((kept - Vec3::new(400.0, 130.0, 0.0)).length() < 1e-2, "{kept}");
        // On the right side already it is left alone.
        let there = Vec3::new(700.0, 130.0, 0.0);
        assert_eq!(Lag::kept_behind(there, goal, target, up, false), there);
        // Where the walls stopped the camera it is no further off than its place.
        let kept = Lag::kept_behind(Vec3::new(900.0, 100.0, 0.0), Vec3::new(300.0, 100.0, 0.0), target, up, true);
        assert!((kept - Vec3::new(300.0, 100.0, 0.0)).length() < 1e-3, "{kept}");
        // Straight over what it looks at there is no side to be on.
        let over = Vec3::new(0.0, 500.0, 0.0);
        assert_eq!(Lag::kept_behind(lagging, over, target, up, false), over);
    }

    /// A camera that follows her, as a script sets one up.
    fn following() -> Camera {
        let mut camera = Camera::new();
        camera.set_kind(kind::FOLLOW);
        camera
    }

    fn trap(figures: &[(i32, i32)]) -> TrapCamera {
        let mut trap = TrapCamera::default();
        for &(what, value) in figures {
            assert!(trap.set(what, value));
        }
        trap
    }

    #[test]
    fn a_figure_moves_to_a_value_over_its_frames() {
        let mut camera = following();
        // What the camera starts with, until a script says otherwise.
        assert_eq!(camera.figures(figure::OFFSET), Vec3::new(0.0, 0.0, 492.0));
        assert_eq!(camera.figures(figure::LOOK_AT), Vec3::new(0.0, 110.0, 0.0));
        camera.set(figure::FIELD_OF_VIEW, 32.0);

        camera.move_to(figure::FIELD_OF_VIEW, 40.0, 4, 0);
        let mut seen = Vec::new();
        for _ in 0..5 {
            camera.step();
            seen.push(camera.figure(figure::FIELD_OF_VIEW));
        }
        assert_eq!(seen, [34.0, 36.0, 38.0, 40.0, 40.0]);

        for (ease, expected) in [(1, [1.0, 4.0, 9.0, 16.0]), (2, [7.0, 12.0, 15.0, 16.0]), (3, [2.0, 8.0, 14.0, 16.0]), (9, [0.0, 0.0, 0.0, 16.0])] {
            camera.set(figure::PITCH, 0.0);
            camera.move_to(figure::PITCH, 16.0, 4, ease);
            let mut seen = Vec::new();
            for _ in 0..4 {
                camera.step();
                seen.push(camera.figure(figure::PITCH));
            }
            assert_eq!(seen, expected, "ease {ease}");
        }

        // A camera of another kind stops what was moving, where it had got to.
        camera.move_to(figure::FIELD_OF_VIEW, 80.0, 4, 0);
        camera.step();
        camera.set_kind(kind::FIXED);
        camera.step();
        assert_eq!(camera.figure(figure::FIELD_OF_VIEW), 50.0);
        // A figure set while it moves goes on from there.
        camera.move_to(figure::YAW, 90.0, 3, 0);
        camera.step();
        camera.set(figure::YAW, 0.0);
        camera.step();
        assert_eq!(camera.figure(figure::YAW), 60.0);
    }

    #[test]
    fn a_trap_gives_the_camera_its_figures_while_she_is_inside() {
        // `SET_CAMTRAP_FOLLOW(100, 0, 0, 500, 0, 19, 0, 140, 0, 0)` of the first map.
        let wide = trap(&[(26, 0), (17, 0), (18, 0), (19, 500), (7, 0), (8, 19), (14, 0), (15, 140), (16, 0), (13, 0)]);
        let traps = |id: i32| (id == 100).then_some(wide);

        // The map's camera is one that stays put until she is found to be in the
        // trap, after the event the map starts with: it follows her then, with all
        // the trap has for it at once, its yaw too.
        let mut camera = Camera::new();
        assert_eq!(camera.kind, kind::FIXED);
        camera.enter(100, &wide);
        camera.resolve(traps, true);
        assert_eq!(camera.kind, kind::FOLLOW);
        assert_eq!(camera.figures(figure::OFFSET), Vec3::new(0.0, 0.0, 500.0));
        assert_eq!(camera.figures(figure::LOOK_AT), Vec3::new(0.0, 140.0, 0.0));
        assert_eq!((camera.figure(figure::PITCH), camera.figure(figure::YAW)), (0.0, 19.0));
        assert!(camera.blend.is_none());

        // The player turns it; leaving the trap, its figures go back to what the
        // script has them at over the trap's frames, but its yaw stays the player's.
        camera.turned(75.0);
        let slow = TrapCamera { frames: 4, ..wide };
        let traps = |id: i32| (id == 100).then_some(slow);
        camera.leave(100, &slow);
        camera.resolve(traps, false);
        assert_eq!(camera.figure(figure::LOOK_AT[1]), 140.0);
        camera.step();
        // The script is taken to have the point it looks at 150 over her feet.
        assert_eq!(camera.figure(figure::LOOK_AT[1]), 142.5);
        assert_eq!(camera.figure(figure::OFFSET[2]), 498.0);
        for _ in 0..3 {
            camera.step();
        }
        assert_eq!((camera.figure(figure::LOOK_AT[1]), camera.figure(figure::OFFSET[2])), (150.0, 492.0));
        assert_eq!(camera.figure(figure::YAW), 75.0);

        // Coming into it again they go to the trap's, and the yaw is still left be.
        camera.enter(100, &slow);
        camera.resolve(traps, false);
        for _ in 0..4 {
            camera.step();
        }
        assert_eq!((camera.figure(figure::LOOK_AT[1]), camera.figure(figure::OFFSET[2])), (140.0, 500.0));
        assert_eq!(camera.figure(figure::YAW), 75.0);
    }

    #[test]
    fn a_trap_of_another_kind_changes_the_camera_and_the_view_goes_over() {
        // `SET_CAMTRAP_FIX(7, 100, 200, 300, -10, 45, 0, 20)`.
        let fixed = trap(&[(26, 1), (10, 100), (11, 200), (12, 300), (7, -10), (8, 45), (9, 0), (13, 20)]);
        let traps = |id: i32| (id == 7).then_some(fixed);
        let mut camera = following();
        camera.set(figure::YAW, 30.0);

        camera.enter(7, &fixed);
        camera.resolve(traps, false);
        assert_eq!(camera.kind, kind::FIXED);
        assert_eq!(camera.figures(figure::PLACE), Vec3::new(100.0, 200.0, 300.0));
        assert_eq!((camera.figure(figure::PITCH), camera.figure(figure::YAW)), (-10.0, 45.0));
        // The view goes over in the trap's twenty frames.
        assert_eq!(camera.blend.unwrap().part(0.0), 0.0);
        for _ in 0..10 {
            camera.step();
        }
        assert_eq!(camera.blend.unwrap().part(0.0), 0.5);
        assert_eq!(camera.blend.unwrap().part(0.5), 0.525);
        for _ in 0..10 {
            camera.step();
        }
        assert!(camera.blend.is_none());

        // Out of it the camera is the script's again, with the script's figures:
        // the yaw the script set, and the view goes over in the frames of the trap
        // the camera had its kind from.
        camera.leave(7, &fixed);
        camera.resolve(traps, false);
        assert_eq!(camera.kind, kind::FOLLOW);
        assert_eq!((camera.figure(figure::PITCH), camera.figure(figure::YAW)), (0.0, 30.0));
        assert!(camera.blend.is_some());
        for _ in 0..20 {
            camera.step();
        }
        assert!(camera.blend.is_none());
    }

    #[test]
    fn after_a_cut_the_stick_comes_to_go_by_the_new_view_in_forty_five_frames() {
        // A trap whose change takes no frames: the view cuts.
        let fixed = trap(&[(26, 1), (10, 100), (11, 200), (12, 300), (7, -10), (8, 120), (9, 0), (13, 0)]);
        let traps = |id: i32| (id == 7).then_some(fixed);
        let mut camera = following();
        camera.set(figure::YAW, 30.0);
        camera.step();
        assert_eq!(camera.stick_yaw(), 30.0);

        camera.enter(7, &fixed);
        camera.resolve(traps, false);
        assert!(camera.blend.is_none());
        assert_eq!(camera.figure(figure::YAW), 120.0);
        // The stick still goes by the old view, and comes to the new by the
        // square of the share of the frames that have gone by.
        camera.step();
        assert!((camera.stick_yaw() - (30.0 + 90.0 / 2025.0)).abs() < 1e-3);
        for _ in 1..30 {
            camera.step();
        }
        assert!((camera.stick_yaw() - 70.0).abs() < 1e-3);
        for _ in 30..44 {
            camera.step();
        }
        assert!(camera.stick_yaw() < 120.0);
        camera.step();
        assert_eq!(camera.stick_yaw(), 120.0);

        // A change that takes its frames is no cut: the stick goes by the view.
        let slow = trap(&[(26, 1), (8, 200), (13, 20)]);
        camera.leave(7, &fixed);
        camera.resolve(|_| None, false);
        for _ in 0..90 {
            camera.step();
        }
        let before = camera.figure(figure::YAW);
        assert_eq!(camera.stick_yaw(), before);
        camera.enter(8, &slow);
        camera.resolve(|id| (id == 8).then_some(slow), false);
        camera.step();
        assert_eq!(camera.stick_yaw(), camera.figure(figure::YAW));
    }

    #[test]
    fn the_pitch_of_a_camera_that_follows_leans_as_a_scene_has_it() {
        let mut camera = following();
        camera.set(figure::PITCH, -7.0);
        // Before it is worked out, and on level ground whatever the rates: by
        // nothing.
        assert_eq!(camera.leaning(), 0.0);
        camera.lean(Some(-7.0), [1.0, 0.3], 0.0);
        assert_eq!(camera.leaning(), 0.0);
        // The ground under the camera is higher than her feet: the way to it is
        // steeper than the pitch, and it leans by the first rate, all the way
        // here; lower, by the second.
        camera.lean(Some(-17.0), [1.0, 0.3], 0.0);
        assert_eq!(camera.leaning(), -10.0);
        camera.lean(Some(3.0), [1.0, 0.3], 0.0);
        assert_eq!(camera.leaning(), 3.0);
        // A scene that says nothing leans it by nothing, and what a scene has
        // taken away is taken off the whole pitch.
        camera.lean(Some(-17.0), [0.0, 0.0], 0.0);
        assert_eq!(camera.leaning(), 0.0);
        camera.lean(Some(-17.0), [1.0, 0.3], 0.25);
        assert_eq!(camera.leaning(), -5.75);
        camera.lean(None, [1.0, 0.3], 1.0);
        assert_eq!(camera.leaning(), 7.0);
    }

    #[test]
    fn the_trap_she_came_into_first_comes_first() {
        let first = trap(&[(15, 140), (13, 0)]);
        let second = trap(&[(15, 200), (19, 800), (13, 0)]);
        let traps = |id: i32| match id {
            1 => Some(first),
            2 => Some(second),
            _ => None,
        };
        let mut camera = following();
        camera.enter(2, &second);
        camera.resolve(traps, true);
        camera.enter(1, &first);
        camera.resolve(traps, true);
        // The second was come into first: its height stands, the other's does not.
        assert_eq!((camera.figure(figure::LOOK_AT[1]), camera.figure(figure::OFFSET[2])), (200.0, 800.0));
        camera.leave(2, &second);
        camera.resolve(traps, true);
        assert_eq!((camera.figure(figure::LOOK_AT[1]), camera.figure(figure::OFFSET[2])), (140.0, 492.0));
        // A trap switched off while she is in it is out of the list, and the camera
        // is not told: it has what it had until she next comes into or leaves one.
        camera.forget(1);
        assert!(!camera.inside(1));
        assert_eq!(camera.figure(figure::LOOK_AT[1]), 140.0);
    }

    #[test]
    fn a_camera_put_by_for_an_event_comes_back_as_it_was() {
        let mut camera = following();
        camera.set(figure::YAW, 47.0);
        assert_eq!((camera.figure(figure::NEAR), camera.figure(figure::FAR)), (50.0, 20_000.0));
        assert!(camera.keep());
        // What an event sets is not what the script has the field's camera at.
        camera.set_kind(kind::FIXED);
        camera.set(figure::YAW, 5.0);
        camera.move_to(figure::PITCH, 30.0, 10, 0);
        // A scene sees past what stands close by.
        camera.set(figure::NEAR, 200.0);
        assert_eq!(camera.figure(figure::NEAR), 200.0);
        assert!(camera.keep());
        assert!(!camera.keep());
        assert!(camera.take_up());
        assert!(camera.take_up());
        assert!(!camera.take_up());
        assert_eq!((camera.kind, camera.figure(figure::YAW)), (kind::FOLLOW, 47.0));
        assert_eq!(camera.figure(figure::NEAR), 50.0);
        camera.step();
        assert_eq!(camera.figure(figure::PITCH), -7.5947);

        // After a battle the camera is a new one, of the script's kind and with the
        // script's angles.
        camera.set(figure::FIELD_OF_VIEW, 62.0);
        camera.set(figure::PITCH, -12.0);
        camera.turned(90.0);
        let resumed = camera.resumed();
        assert_eq!(resumed.kind, kind::FOLLOW);
        assert_eq!((resumed.figure(figure::PITCH), resumed.figure(figure::YAW)), (-12.0, 90.0));
        assert_eq!(resumed.figure(figure::FIELD_OF_VIEW), 72.337);
    }

    #[test]
    fn a_script_names_what_a_trap_gives_by_its_own_numbers() {
        let mut given = TrapCamera::default();
        assert_eq!(given.frames, 90);
        for (what, slot) in [(6, 0), (10, 1), (12, 3), (7, 4), (9, 6), (14, 7), (19, 12), (22, 15), (25, 18)] {
            assert!(given.set(what, what * 10));
            assert_eq!(given.figures[slot], Some(what as f32 * 10.0), "what {what}");
        }
        assert!(given.set(28, 7) && given.ease == 0);
        assert!(given.set(28, 2) && given.ease == 2);
        // The rest of what `s45` sets is not the camera's.
        assert!(!given.set(4, 1) && !given.set(2, 1) && !given.set(31, 90));
        // And a script's numbers for the camera's own figures.
        assert_eq!((figure::slot(1), figure::slot(5), figure::slot(22), figure::slot(2)), (Some(0), Some(1), Some(18), None));
    }

    #[test]
    fn a_camera_is_made_anew_from_the_record_a_save_keeps() {
        // The view she turned the camera to is what the script is taken to
        // have it at, and so is in its record.
        let mut camera = Camera::new();
        camera.turned(135.0);
        let record = camera.record();
        let yaw = RECORD_SLOTS.iter().position(|&slot| slot == figure::YAW_SLOT).unwrap();
        assert_eq!((record.kind, record.figures[yaw]), (kind::FOLLOW, 135.0));
        // A camera made from the record follows her at that yaw, and is one
        // made anew: its view is where it is at once.
        let old = Camera::new();
        let again = old.restored(&record);
        assert_eq!((again.kind, again.now[figure::YAW_SLOT]), (kind::FOLLOW, 135.0));
        assert_eq!(again.record(), record);
        assert_eq!((again.set, again.cuts, again.renewed), (old.set + 1, old.cuts + 1, old.renewed + 1));
        // One that stays put comes back at its place with its angles.
        let fixed = Record { kind: kind::FIXED, figures: [10.0, 20.0, 30.0, -15.0, 90.0, 0.0, 0.0, 0.0, 0.0] };
        let again = old.restored(&fixed);
        assert_eq!((again.kind, &again.now[1..=5]), (kind::FIXED, &[10.0, 20.0, 30.0, -15.0, 90.0][..]));
    }
}
