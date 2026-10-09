//! What rides the road: a character, how it moves from lane to lane, and the
//! boxes it hits with and is hit in (`docs/BIKE_NOTES.md`, "A character" and
//! "What is on the road besides her").

use bevy::math::{EulerRot, Mat4, Quat, Vec3};

use super::road::{mark, Place, Road};

/// A character's flags (its + 0x18).
pub mod flag {
    pub const JUST_MADE: u32 = 1;
    pub const HAS_BOXES: u32 = 2;
    pub const TAKES_NOTHING: u32 = 8;
    pub const CAN_BE_HURT: u32 = 0x80;
    pub const ON_THE_GROUND: u32 = 0x100;
    pub const MAY_TURN: u32 = 0x800;
    pub const CAN_BE_STRUCK: u32 = 0x2000;
}

/// The entries of a character's scripts that the game begins of itself, by
/// their numbers (0x172D68).
pub mod entry {
    pub const INIT: i32 = 0;
    pub const EVENT: i32 = 1;
    pub const START: i32 = 2;
    pub const FINISH: i32 = 3;
    pub const GAME_OVER: i32 = 4;
    pub const JUST_AVOID: i32 = 5;
    pub const TOUCH: i32 = 6;
    pub const TERMINATE: i32 = 7;
    pub const GROUNDED: i32 = 8;
    pub const TURN_LEFT: i32 = 9;
    pub const TURN_RIGHT: i32 = 10;
    pub const BOMB: i32 = 11;
    pub const NAMES: [&str; 12] = [
        "int_init",
        "int_event",
        "int_start",
        "int_finish",
        "int_gameover",
        "int_justavoid",
        "int_touch",
        "int_tarminate",
        "int_grounded",
        "int_turn_left",
        "int_turn_right",
        "int_bomb",
    ];
}

/// Something a character's scripts are to be told: an entry, the event and
/// what comes with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Told {
    pub entry: i32,
    pub event: i32,
    pub with: i32,
}

/// How high up a wall ridden on is, and how far she leans on it.
const BANK_HEIGHT: f32 = 256.0;
const BANK_LEAN: f32 = 45.0;
/// Under this height a character is on the ground.
const GROUND: f32 = 0.1;

/// A box's kinds.
pub const BLOW: u32 = 1;
pub const NEAR: u32 = 2;
/// How many boxes a character has.
pub const BOXES: usize = 5;

/// One of a character's boxes (0x38 bytes).
#[derive(Clone, Copy, Debug)]
pub struct HitBox {
    /// 0 none, 1 a ball, 2 a box.
    pub shape: i32,
    pub on: bool,
    /// Whether it met something when last looked at.
    pub met: bool,
    /// The bone it goes with, or less than 0 for the whole character, and
    /// which of the two models the bone is of.
    pub bone: i32,
    pub model: i32,
    pub middle: Vec3,
    pub radius: f32,
    pub size: Vec3,
    pub kinds: u32,
    /// What a blow of it says, and what it is worth.
    pub says: i32,
    pub worth: i32,
}

impl Default for HitBox {
    fn default() -> Self {
        HitBox { shape: 0, on: false, met: false, bone: 0, model: 0, middle: Vec3::ZERO, radius: 0.0, size: Vec3::ZERO, kinds: 0, says: 0, worth: 0 }
    }
}

impl HitBox {
    fn is_of(&self, kinds: u32) -> bool {
        self.on && kinds & !self.kinds == 0 && (self.shape == 1 || self.shape == 2)
    }
}

/// How a character moves on the road (0x70 bytes at its + 0x770).
#[derive(Clone, Copy, Debug)]
pub struct Mover {
    /// A change of lanes goes on.
    pub changing: bool,
    /// The place it is leaving and the one it is on.
    pub from: Place,
    pub to: Place,
    /// The part of a change's time waited before it begins to cross.
    pub wait: f32,
    /// How long a change takes, and how long this one has gone on.
    pub length: f32,
    pub time: f32,
    /// The curve of a change: `f(x) = a^3 / (x^2 + a^2)` from `c` to `b`.
    pub curve: [f32; 3],
    /// A change that crosses evenly instead.
    pub even: bool,
    pub position: Vec3,
    pub facing: Vec3,
}

impl Default for Mover {
    fn default() -> Self {
        Mover {
            changing: false,
            from: Place::default(),
            to: Place::default(),
            wait: 0.0,
            length: 0.5,
            time: 0.0,
            curve: [3.0, 8.0, 0.0],
            even: false,
            position: Vec3::ZERO,
            facing: Vec3::ZERO,
        }
    }
}

impl Mover {
    /// How far across a change is, 0 to 1, for the part of its time gone.
    fn across(&self, part: f32) -> f32 {
        if self.even {
            return part;
        }
        let [a, b, c] = self.curve;
        let f = |x: f32| a * a * a / (x * x + a * a);
        let whole = f(c) - f(b);
        if whole <= 0.0 {
            return 1.0;
        }
        (f(c) - f((b - c) * part + c)) / whole
    }

    /// A frame (0x13B644): both places go on, and where it is between them.
    pub fn step(&mut self, road: &Road, time: f32) {
        self.from.step(road, time);
        self.to.step(road, time);
        if self.to.chip.is_none() || self.from.chip.is_none() {
            self.changing = false;
            return;
        }
        let mut across = 1.0;
        if self.changing {
            self.time += time;
            if self.time > self.length {
                self.time = self.length;
                self.changing = false;
            }
            let wait = self.length * self.wait;
            let part = if self.length - wait == 0.0 { 1.0 } else { ((self.time - wait) / (self.length - wait)).clamp(0.0, 1.0) };
            across = self.across(part);
        }
        let (from, to) = (self.from.position(road), self.to.position(road));
        self.position = from + (to - from) * across;
        // Every chip faces straight on.
        self.facing = Vec3::ZERO;

        // The banks: a wall is come on to from half way across, and left by half way.
        let (onto, off) = (road.marks(self.to.chip), road.marks(self.from.chip));
        for (bank, lean) in [(mark::LEFT_BANK, BANK_LEAN), (mark::RIGHT_BANK, -BANK_LEAN)] {
            match (onto & bank != 0, off & bank != 0) {
                (false, false) => {}
                (true, false) => {
                    if across >= 0.5 {
                        self.position.y += (across - 0.5) * 2.0 * BANK_HEIGHT;
                    }
                    self.facing.z += across * lean;
                }
                (false, true) => {
                    if across < 0.5 {
                        self.position.y += (1.0 - across * 2.0) * BANK_HEIGHT;
                    }
                    self.facing.z += (1.0 - across) * lean;
                }
                // Wholly on it, it is her heading the code adds to (0x13BA1C).
                (true, true) => {
                    self.position.y += BANK_HEIGHT;
                    self.facing.y += lean;
                }
            }
        }
    }
}

/// A character (0x820 bytes): she, and whatever is met.
#[derive(Clone, Debug)]
pub struct Character {
    pub number: i32,
    /// Bit 0: its place is the road's (its mover's).
    pub rides: i32,
    pub word: i32,
    /// What it can still take, what it could at first, and how many blows it
    /// has had.
    pub left: i32,
    pub could: i32,
    pub blows: i32,
    pub flags: u32,
    pub was: Vec3,
    pub place: Vec3,
    /// What its place is the sum of: where the road has it, and what is added;
    /// and how fast what is added moves.
    pub on_road: Vec3,
    pub added: Vec3,
    pub adding: Vec3,
    /// Its turns in degrees, likewise.
    pub road_turns: Vec3,
    pub added_turns: Vec3,
    pub turns: Vec3,
    /// How fast it went this frame.
    pub going: Vec3,
    pub boxes: [HitBox; BOXES],
    pub mover: Mover,
    /// How long nothing can touch it.
    pub untouchable: f32,
    /// To be taken off the road at the frame's end.
    pub ending: bool,
}

impl Character {
    pub fn new(number: i32) -> Character {
        Character {
            number,
            rides: 0,
            word: 0,
            left: 0,
            could: 0,
            blows: 0,
            flags: 0x83,
            was: Vec3::ZERO,
            place: Vec3::ZERO,
            on_road: Vec3::ZERO,
            added: Vec3::ZERO,
            adding: Vec3::ZERO,
            road_turns: Vec3::ZERO,
            added_turns: Vec3::ZERO,
            turns: Vec3::ZERO,
            going: Vec3::ZERO,
            boxes: [HitBox::default(); BOXES],
            mover: Mover::default(),
            untouchable: 0.0,
            ending: false,
        }
    }

    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    pub fn set(&mut self, flag: u32, on: bool) {
        self.flags = if on { self.flags | flag } else { self.flags & !flag };
    }

    /// The chip it is on.
    pub fn chip(&self) -> Option<usize> {
        self.mover.to.chip
    }

    /// Puts it on a chip (0x13A334).
    pub fn put(&mut self, road: &Road, chip: usize) {
        self.mover.from.put(road, chip);
        self.mover.to.put(road, chip);
    }

    /// Its speed, and the part of the way along its chip it is (0x139E40,
    /// 0x139EE4).
    pub fn set_speed(&mut self, speed: f32) {
        (self.mover.from.speed, self.mover.to.speed) = (speed, speed);
    }

    pub fn set_part(&mut self, road: &Road, part: f32) {
        self.mover.from.set_part(road, part);
        self.mover.to.set_part(road, part);
    }

    /// Moves it along the road by a length, as the road's wrapping does
    /// (0x13A650).
    pub fn shift(&mut self, road: &Road, by: f32) {
        self.mover.from.go(road, by);
        self.mover.to.go(road, by);
        self.place.z += by;
        self.on_road.z += by;
        self.was.z += by;
    }

    /// Begins a change to the lane beside (0x13A390): the place it leaves is
    /// where it was, the one it is on goes to the chip that side. Whether there
    /// was a lane to change to.
    pub fn change_lane(&mut self, road: &Road, right: bool) -> bool {
        self.mover.from = self.mover.to;
        self.mover.to.beside(road, right);
        if self.mover.to.chip == self.mover.from.chip {
            return false;
        }
        (self.mover.changing, self.mover.time) = (true, 0.0);
        true
    }

    /// A frame of it (0x13A958). What its scripts are to be told is answered.
    pub fn step(&mut self, road: &Road, time: f32) -> Vec<Told> {
        let mut told = Vec::new();
        self.mover.step(road, time);
        if self.untouchable > 0.0 {
            self.untouchable = (self.untouchable - time).max(0.0);
        }
        if self.rides & 1 != 0 {
            if let Some(chip) = self.chip() {
                let marks = road.chips[chip].marks;
                if marks & mark::NO_TURNING == 0 && self.has(flag::MAY_TURN) && marks & mark::TO_BE_LEFT != 0 {
                    // How far off each side has a chip with the mark, if any.
                    let reach = |mut beside: Option<usize>, next: fn(&super::road::Chip) -> Option<usize>| {
                        let mut steps = 0;
                        while let Some(chip) = beside {
                            if road.chips[chip].marks & mark::SHUT != 0 {
                                return Some(steps);
                            }
                            (beside, steps) = (next(&road.chips[chip]), steps + 1);
                        }
                        None
                    };
                    let (left, right) = (reach(road.chips[chip].left, |chip| chip.left), reach(road.chips[chip].right, |chip| chip.right));
                    if left.is_some() || right.is_some() {
                        let (left, right) = (left.unwrap_or(100), right.unwrap_or(100));
                        let entry = if right < left { entry::TURN_RIGHT } else { entry::TURN_LEFT };
                        told.push(Told { entry, event: 0, with: 0 });
                    }
                }
            }
            (self.on_road, self.road_turns) = (self.mover.position, self.mover.facing);
        }
        self.added += self.adding * time;
        self.was = self.place;
        self.place = self.on_road + self.added;
        self.turns = self.road_turns + self.added_turns;
        self.going = if time > 0.0 { (self.place - self.was) / time } else { Vec3::ZERO };
        if self.has(flag::ON_THE_GROUND) {
            if self.place.y > GROUND {
                self.set(flag::ON_THE_GROUND, false);
            }
        } else if self.place.y < GROUND {
            told.push(Told { entry: entry::GROUNDED, event: 0, with: 0 });
            self.set(flag::ON_THE_GROUND, true);
        }
        self.set(flag::JUST_MADE, false);
        told
    }

    /// Takes a blow's worth from it (0x13A58C): nothing unless it can be hurt,
    /// and nothing if it takes nothing. Its scripts are told of the blow, and
    /// that it is over when nothing is left.
    pub fn take(&mut self, mut worth: i32, says: i32) -> Vec<Told> {
        if !self.has(flag::CAN_BE_HURT) || self.has(flag::TAKES_NOTHING) {
            worth = worth.min(0);
        }
        self.left -= worth;
        let mut told = vec![Told { entry: entry::TOUCH, event: says, with: 0 }];
        if self.left <= 0 {
            told.push(Told { entry: entry::TERMINATE, event: 0, with: 0 });
        }
        if worth > 0 {
            self.blows += 1;
        }
        told
    }

    /// The matrix its place and turns make (0x13C9C0): its turns about x, then
    /// y, then z, and its place.
    pub fn matrix(&self) -> Mat4 {
        let turn = Quat::from_euler(EulerRot::ZYX, self.turns.z.to_radians(), self.turns.y.to_radians(), self.turns.x.to_radians());
        Mat4::from_rotation_translation(turn, self.place)
    }
}

/// Whether one of `a`'s balls of its kinds, swept from where it was a frame ago
/// to where it is, touches one of `b`'s balls or boxes of its kinds
/// (0x13ADC0). The two that touch are flagged as having met. Boxes are taken as
/// going with the whole character: none of the game's is tied to a bone that
/// the port has moving yet.
pub fn meet(a: &mut Character, b: &mut Character, kinds_of_a: u32, kinds_of_b: u32) -> bool {
    for hit_box in a.boxes.iter_mut().chain(b.boxes.iter_mut()) {
        hit_box.met = false;
    }
    if !a.has(flag::HAS_BOXES) || !b.has(flag::HAS_BOXES) {
        return false;
    }
    if !a.boxes.iter().any(|hit_box| hit_box.is_of(kinds_of_a)) || !b.boxes.iter().any(|hit_box| hit_box.is_of(kinds_of_b)) {
        return false;
    }
    let (matrix_a, matrix_b) = (a.matrix(), b.matrix());
    let moved = a.place - a.was;
    for of_a in a.boxes.iter_mut().filter(|hit_box| hit_box.is_of(kinds_of_a) && hit_box.shape == 1) {
        let now = matrix_a.transform_point3(of_a.middle);
        let before = now - moved;
        for of_b in b.boxes.iter_mut().filter(|hit_box| hit_box.is_of(kinds_of_b)) {
            let touches = match of_b.shape {
                1 => {
                    let middle = matrix_b.transform_point3(of_b.middle);
                    distance_to_segment(middle, before, now) <= of_a.radius + of_b.radius
                }
                _ => {
                    let into = matrix_b.inverse();
                    let (before, now) = (into.transform_point3(before) - of_b.middle, into.transform_point3(now) - of_b.middle);
                    segment_touches_box(before, now, of_b.size * 0.5, of_a.radius)
                }
            };
            if touches {
                (of_a.met, of_b.met) = (true, true);
                return true;
            }
        }
    }
    false
}

fn distance_to_segment(point: Vec3, from: Vec3, to: Vec3) -> f32 {
    let along = to - from;
    let length = along.length_squared();
    let part = if length > 0.0 { ((point - from).dot(along) / length).clamp(0.0, 1.0) } else { 0.0 };
    point.distance(from + along * part)
}

/// Whether a ball of `radius` going from `from` to `to` touches the box of
/// those half sizes about the origin. The box is grown by the radius, which
/// rounds nothing at its corners: near enough for what a road has on it.
pub fn segment_touches_box(from: Vec3, to: Vec3, half: Vec3, radius: f32) -> bool {
    let half = half + Vec3::splat(radius);
    let along = to - from;
    let (mut first, mut last) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        if along[axis].abs() < 1e-6 {
            if from[axis].abs() > half[axis] {
                return false;
            }
        } else {
            let (one, other) = ((-half[axis] - from[axis]) / along[axis], (half[axis] - from[axis]) / along[axis]);
            (first, last) = (first.max(one.min(other)), last.min(one.max(other)));
            if first > last {
                return false;
            }
        }
    }
    true
}

/// A trap (0x70 bytes): a box that tells a script, once, when the one it
/// watches passes through it.
#[derive(Clone, Debug)]
pub struct Trap {
    pub number: i32,
    pub place: Vec3,
    pub turns: Vec3,
    pub size: Vec3,
    pub middle: Vec3,
    pub gone_off: bool,
    /// Whom it watches: a character's number.
    pub watches: i32,
    /// Whom it tells: 0 the system's script, 1 the course's, 2 the road's, 3
    /// the camera's.
    pub tells: i32,
    pub entry: i32,
    pub event: i32,
    pub with: i32,
}

impl Trap {
    pub fn new(number: i32) -> Trap {
        Trap { number, place: Vec3::ZERO, turns: Vec3::ZERO, size: Vec3::splat(100.0), middle: Vec3::ZERO, gone_off: false, watches: 0, tells: 0, entry: 0, event: 0, with: 0 }
    }

    /// Whether the way the watched one came this frame passes through it
    /// (0x1486A0).
    pub fn passed(&self, watched: &Character) -> bool {
        let turn = Quat::from_euler(EulerRot::ZYX, self.turns.z.to_radians(), self.turns.y.to_radians(), self.turns.x.to_radians());
        let into = Mat4::from_rotation_translation(turn, self.place).inverse();
        segment_touches_box(into.transform_point3(watched.was) - self.middle, into.transform_point3(watched.place) - self.middle, self.size * 0.5, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bike::road::tests::normal;

    fn on(road: &Road, lane: usize, row: usize) -> Character {
        let mut character = Character::new(0);
        character.rides = 1;
        character.put(road, road.chip_of(lane, false, row).unwrap());
        character
    }

    #[test]
    fn a_change_of_lanes_takes_half_a_second_and_is_quickest_in_its_middle() {
        let Some(road) = normal() else { return };
        let mut she = on(&road, 2, 2);
        she.step(&road, 1.0 / 30.0);
        assert_eq!(she.place, Vec3::new(0.0, 0.0, 20000.0));
        assert!(she.change_lane(&road, false));
        let mut across = Vec::new();
        for _ in 0..16 {
            she.step(&road, 1.0 / 30.0);
            across.push(she.place.x);
        }
        // Lane 1 is 250 to the left: a quarter of the way after a fifth of the
        // time, seven tenths after less than half of it.
        assert!((across[2] - 63.1).abs() < 1.0 && (across[6] - 173.0).abs() < 2.0, "{across:?}");
        assert!(across.windows(2).all(|pair| pair[1] >= pair[0]), "{across:?}");
        assert!((across[14] - 250.0).abs() < 1e-3 && !she.mover.changing, "{across:?}");
        assert_eq!(she.chip().map(|chip| road.chips[chip].lane), Some(1));
        // There is no lane to the left of lane 0.
        let mut edge = on(&road, 0, 2);
        assert!(!edge.change_lane(&road, false));
    }

    #[test]
    fn one_that_may_turn_is_turned_off_a_lane_that_is_to_be_left() {
        let Some(road) = normal() else { return };
        let mut outer = on(&road, 0, 2);
        outer.set(flag::MAY_TURN, true);
        assert!(outer.step(&road, 1.0 / 30.0).contains(&Told { entry: entry::TURN_RIGHT, event: 0, with: 0 }));
        let mut other = on(&road, 4, 2);
        other.set(flag::MAY_TURN, true);
        assert!(other.step(&road, 1.0 / 30.0).contains(&Told { entry: entry::TURN_LEFT, event: 0, with: 0 }));
        // In the middle there is nothing to be told but that it is on the ground.
        let mut middle = on(&road, 2, 2);
        middle.set(flag::MAY_TURN, true);
        assert_eq!(middle.step(&road, 1.0 / 30.0), [Told { entry: entry::GROUNDED, event: 0, with: 0 }]);
        assert_eq!(middle.step(&road, 1.0 / 30.0), []);
    }

    #[test]
    fn a_swept_ball_meets_a_ball_and_a_box_and_a_blow_takes_its_worth() {
        let ball = |kinds: u32, height: f32, radius: f32| HitBox { shape: 1, on: true, bone: -1, middle: Vec3::new(0.0, height, 0.0), radius, kinds, ..HitBox::default() };
        let mut she = Character::new(0);
        she.boxes[0] = ball(BLOW, 60.0, 40.0);
        she.boxes[1] = ball(NEAR, 40.0, 70.0);
        (she.was, she.place) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 300.0));
        let mut other = Character::new(0x100);
        other.place = Vec3::new(60.0, 0.0, 150.0);
        other.boxes[0] = ball(BLOW, 60.0, 30.0);
        other.boxes[1] = HitBox { shape: 2, on: true, bone: -1, middle: Vec3::new(0.0, 50.0, 0.0), size: Vec3::new(200.0, 100.0, 50.0), kinds: NEAR, ..HitBox::default() };
        // She passes 60 beside it in one frame: the balls of 40 and 30 touch.
        assert!(meet(&mut she, &mut other, BLOW, BLOW));
        assert!(she.boxes[0].met && other.boxes[0].met && !she.boxes[1].met);
        assert!(meet(&mut she, &mut other, NEAR, NEAR));
        other.place.x = 300.0;
        assert!(!meet(&mut she, &mut other, BLOW, BLOW));
        assert!(!meet(&mut she, &mut other, NEAR, NEAR));
        // A blow: only what can be hurt loses by it.
        she.left = 150;
        assert_eq!(she.take(100, 1), [Told { entry: entry::TOUCH, event: 1, with: 0 }]);
        assert_eq!((she.left, she.blows), (50, 1));
        she.set(flag::TAKES_NOTHING, true);
        she.take(100, 1);
        assert_eq!((she.left, she.blows), (50, 1));
        she.set(flag::TAKES_NOTHING, false);
        assert_eq!(she.take(100, 2).last(), Some(&Told { entry: entry::TERMINATE, event: 0, with: 0 }));
    }

    #[test]
    fn a_gate_goes_off_as_she_passes_through_it() {
        let mut gate = Trap::new(1);
        (gate.place, gate.size) = (Vec3::new(0.0, 0.0, 30000.0), Vec3::new(8000.0, 400.0, 1.0));
        let mut she = Character::new(0);
        (she.was, she.place) = (Vec3::new(250.0, 0.0, 29000.0), Vec3::new(250.0, 0.0, 29900.0));
        assert!(!gate.passed(&she));
        (she.was, she.place) = (she.place, Vec3::new(250.0, 0.0, 30800.0));
        assert!(gate.passed(&she));
    }
}
