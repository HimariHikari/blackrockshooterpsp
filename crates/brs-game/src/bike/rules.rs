//! The rules of the ride on the bike (0x140424 and what it calls): the pad, her
//! speed and gears, what she has just avoided, blows, and how it ends
//! (`docs/BIKE_NOTES.md`, "The rules").

use bevy::math::Vec3;

use super::chara::{entry, flag, meet, Character, Told, BLOW, BOXES, NEAR};
use super::road::{mark, Road};
use super::tables::state::{self, *};
use super::tables::STATE_LEN;

/// The course's flags and numbers, where the scripts have them.
pub struct State<'a>(pub &'a mut [u8; STATE_LEN]);

impl State<'_> {
    pub fn word(&self, at: usize) -> i32 {
        i32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    pub fn set_word(&mut self, at: usize, value: i32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn number(&self, at: usize) -> f32 {
        f32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    pub fn set_number(&mut self, at: usize, value: f32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn has(&self, flag: u32) -> bool {
        self.word(FLAGS) as u32 & flag != 0
    }
}

/// What the pad asks of the ride this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    /// Left and right, held.
    pub left: bool,
    pub right: bool,
    /// Square, as it goes down.
    pub special: bool,
}

/// A sound of the mini-game's own that the rules have played or stopped, by its
/// slot, which is its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Play(i32),
    Stop(i32),
    /// One of the voices.
    Voice(i32),
}

/// The sounds the rules play.
pub mod sound {
    pub const RIDING: i32 = 3;
    /// A sign of the display's comes up.
    pub const SIGN: i32 = 0xc;
    pub const ROW_RUNS_OUT: i32 = 0xd;
    pub const GAUGE_FULL: i32 = 0xe;
    pub const TIME_SHORT: i32 = 0x10;
    pub const TIME_OUT: i32 = 0x11;
    pub const GAUGE_FULL_VOICE: i32 = 1;
}

/// How the ride has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    Goal,
    Failed,
}

/// What a frame of the rules has to be done about by the game around them.
#[derive(Debug, Default)]
pub struct Happened {
    /// What characters' scripts are to be told, by their numbers: all three of
    /// a character's, and only its first.
    pub told: Vec<(i32, Told)>,
    pub told_first: Vec<(i32, Told)>,
    /// What the camera's script is told: an event and what comes with it.
    pub camera: Vec<(i32, i32)>,
    pub sounds: Vec<Sound>,
    /// Her special has begun: the display shows it.
    pub special_begun: bool,
    /// A blow took something from her (the game's flag 0x1818).
    pub hurt: bool,
    pub ended: Option<Ended>,
}

/// The rules' own flags.
mod own {
    pub const SPECIAL_TOLD: u32 = 1;
    pub const AVOID_SIGN: u32 = 2;
    pub const SIGNS: [u32; 2] = [4, 8];
    pub const SPECIAL_BEGUN: u32 = 0x10;
    pub const GEAR_TOLD: u32 = 0x40;
    pub const RIDING_SOUNDED: u32 = 0x80;
    pub const GAUGE_FULL: u32 = 0x100;
    pub const FULL_SOUNDED: u32 = 0x200;
    pub const ROW_WARNED: u32 = 0x400;
    pub const TIME_WARNED: u32 = 0x1000;
    pub const TIME_OUT_SOUNDED: u32 = 0x2000;
}

/// How the rules stand.
pub mod standing {
    pub const STOPPED: i32 = 0;
    pub const RIDING: i32 = 1;
    pub const SPECIAL: i32 = 2;
}

/// What the display is given each frame (0x1335D8's numbers 0 to 15).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub avoid_sign: bool,
    pub signs: [bool; 2],
    pub parts: [bool; 4],
    pub come: i32,
    pub part_of_the_way: i32,
    pub left: i32,
    pub could: i32,
    pub speed: i32,
    pub row: i32,
    pub row_time: i32,
    pub time_left: i32,
}

/// The rules' block (0x48 bytes at the game's + 0x850).
#[derive(Clone, Debug)]
pub struct Rules {
    flags: u32,
    /// Her number.
    pub her: i32,
    pub standing: i32,
    /// The turn the pad asked for this frame: 1 left, 2 right.
    turned: i32,
    pub speed: f32,
    gear_time: f32,
    pub gear: i32,
    /// How many she has just avoided in a row, and how long the row holds or
    /// her special lasts.
    pub row: i32,
    pub row_time: f32,
    /// The lane mark the camera was last told.
    lane_mark: u32,
    pub shown: Shown,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { flags: 0, her: 0, standing: standing::RIDING, turned: 0, speed: 0.0, gear_time: 0.0, gear: 0, row: 0, row_time: 0.0, lane_mark: 0, shown: Shown::default() }
    }
}

/// How a gear's speed comes on (0x14897C).
fn eased(from: f32, to: f32, part: f32) -> f32 {
    from + (to - from) * (part + (part * std::f32::consts::PI).sin() / std::f32::consts::PI)
}

impl Rules {
    fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    fn set(&mut self, flag: u32, on: bool) {
        self.flags = if on { self.flags | flag } else { self.flags & !flag };
    }

    /// A frame (0x140424). `characters` are all that are on the road; she is
    /// the one with the rules' number.
    pub fn step(&mut self, state: &mut State, road: &Road, characters: &mut [Character], asked: Asked, time: f32) -> Happened {
        let mut happened = Happened::default();
        let Some(her) = characters.iter().position(|character| character.number == self.her) else { return happened };
        self.turned = 0;
        self.flags &= !(own::AVOID_SIGN | own::SIGNS[0] | own::SIGNS[1]);

        self.pad(state, road, &characters[her], asked, &mut happened);
        self.speed_and_gauge(state, road, &mut characters[her], time, &mut happened);
        self.just_avoided(state, road, characters, her, &mut happened);
        self.aim(state, characters, her, &mut happened);
        self.blows(state, characters, her, &mut happened);
        self.end(state, road, &characters[her], &mut happened);

        let she = &characters[her];
        let (start, goal) = (state.word(START_CHIP), state.word(GOAL_CHIP));
        let begins = |chip: i32| road.chips.get(chip as usize).map_or(0.0, |chip| chip.begins.z);
        let come = she.place.z - begins(start);
        self.shown = Shown {
            avoid_sign: self.has(own::AVOID_SIGN),
            signs: own::SIGNS.map(|sign| self.has(sign)),
            parts: SHOWN.map(|part| state.has(part)),
            come: come as i32,
            part_of_the_way: (come / (begins(goal) - begins(start)) * 100.0) as i32,
            left: she.left,
            could: she.could,
            speed: she.mover.from.speed as i32,
            row: self.row,
            row_time: (if self.standing != standing::RIDING {
                0.0
            } else if state.number(ROW_TIME) > 0.0 {
                self.row_time / state.number(ROW_TIME)
            } else {
                1.0
            } * 100.0) as i32,
            time_left: (state.number(TIME_LEFT) * 100.0) as i32,
        };
        if self.has(own::SPECIAL_BEGUN) {
            happened.special_begun = true;
            self.set(own::SPECIAL_BEGUN, false);
        }
        happened
    }

    /// The pad (0x13FA58): a change of lanes, and her special begun.
    fn pad(&mut self, state: &mut State, road: &Road, she: &Character, asked: Asked, happened: &mut Happened) {
        if !state.has(PAD) {
            return;
        }
        let chip = she.chip().filter(|_| state.has(PAD_CHANGES_LANES) && she.has(flag::MAY_TURN));
        if let Some(chip) = chip.map(|chip| &road.chips[chip]).filter(|chip| chip.marks & mark::NO_TURNING == 0) {
            let open = |beside: Option<usize>| beside.is_some_and(|beside| road.chips[beside].marks & mark::SHUT == 0);
            if asked.left {
                if open(chip.left) {
                    happened.told.push((she.number, Told { entry: entry::TURN_LEFT, event: 0, with: 0 }));
                    self.turned = 1;
                }
            } else if asked.right && open(chip.right) {
                happened.told.push((she.number, Told { entry: entry::TURN_RIGHT, event: 0, with: 0 }));
                self.turned = 2;
            }
        }
        if self.standing == standing::RIDING && state.has(SPECIAL_CAN_BE_HAD) && self.has(own::GAUGE_FULL) {
            if !self.has(own::FULL_SOUNDED) {
                happened.sounds.extend([Sound::Voice(sound::GAUGE_FULL_VOICE), Sound::Play(sound::GAUGE_FULL)]);
                self.set(own::FULL_SOUNDED, true);
            }
            if asked.special {
                self.standing = standing::SPECIAL;
                self.row_time = state.number(SPECIAL_TIME);
                self.row = 0;
                self.set(own::SPECIAL_BEGUN, true);
                self.set(own::GAUGE_FULL | own::FULL_SOUNDED, false);
            }
        }
    }

    /// Her speed, the gauge, the camera's lane, and the time (0x13FC70).
    fn speed_and_gauge(&mut self, state: &mut State, road: &Road, she: &mut Character, time: f32, happened: &mut Happened) {
        let held = state.number(HELD);
        if held > 0.0 {
            state.set_number(HELD, (held - time).max(0.0));
        } else {
            match self.standing {
                standing::RIDING => {
                    state.set_word(GOING, state::RIDING);
                    if !self.has(own::RIDING_SOUNDED) {
                        happened.sounds.push(Sound::Play(sound::RIDING));
                        self.set(own::RIDING_SOUNDED, true);
                    }
                }
                standing::SPECIAL => state.set_word(GOING, state::SPECIAL),
                _ => {}
            }
            if state.word(GOING) != state::NOT_DRIVEN && (0..4).contains(&self.gear) {
                let gear = self.gear as usize;
                let length = state.number(GEAR_TIMES + gear * 4);
                self.gear_time += time;
                if self.gear_time > length {
                    if !self.has(own::GEAR_TOLD) {
                        happened.told_first.push((she.number, Told { entry: entry::EVENT, event: 1, with: 0 }));
                        self.set(own::GEAR_TOLD, true);
                    }
                    if gear < 3 && state.has(GEARS[gear]) {
                        self.gear += 1;
                        self.gear_time = 0.0;
                        self.set(own::GEAR_TOLD, false);
                    } else {
                        self.gear_time = length;
                    }
                }
            }
        }

        self.speed = if state.word(GOING) == state::NOT_DRIVEN {
            self.speed * state.number(SLOWING)
        } else if (0..4).contains(&self.gear) {
            let gear = self.gear as usize;
            eased(state.number(SPEEDS + gear * 4), state.number(SPEEDS + gear * 4 + 4), self.gear_time / state.number(GEAR_TIMES + gear * 4))
        } else {
            0.0
        };
        she.set_speed(self.speed);

        match self.standing {
            standing::RIDING => {
                if !self.has(own::GAUGE_FULL) && state.has(SPECIAL_CAN_BE_HAD) {
                    if self.row < state.word(ROW_FULL) {
                        if self.row_time <= 0.0 {
                            (self.row, self.row_time) = (0, 0.0);
                        } else {
                            self.row_time -= time;
                            if !self.has(own::ROW_WARNED) && self.row_time < 3.0 {
                                happened.sounds.push(Sound::Play(sound::ROW_RUNS_OUT));
                                self.set(own::ROW_WARNED, true);
                            }
                        }
                    } else {
                        self.set(own::GAUGE_FULL, true);
                    }
                }
            }
            standing::SPECIAL => {
                if !state.has(SPECIAL_CAN_BE_HAD) {
                    self.row_time = 0.0;
                }
                if self.row_time <= 0.0 {
                    if self.has(own::SPECIAL_TOLD) {
                        self.set(own::SPECIAL_TOLD, false);
                        happened.told.push((she.number, Told { entry: entry::BOMB, event: 1, with: 0 }));
                    }
                    self.standing = standing::RIDING;
                } else {
                    if !self.has(own::SPECIAL_TOLD) {
                        happened.told.push((she.number, Told { entry: entry::BOMB, event: 0, with: 0 }));
                        self.set(own::SPECIAL_TOLD, true);
                    }
                    self.row_time -= time;
                }
            }
            _ => {}
        }

        // The camera follows her: it is told the lane she is in when that changes.
        let marks = road.marks(she.chip());
        if marks & mark::FOR_THE_CAMERA != self.lane_mark {
            self.lane_mark = marks & mark::FOR_THE_CAMERA;
            happened.camera.push((0, self.lane_mark as i32));
        }

        if state.word(GOING) != state::NOT_DRIVEN && she.left <= 0 {
            self.standing = standing::STOPPED;
            state.set_word(GOING, state::NOT_DRIVEN);
        }

        if state.has(TIME_RUNS) {
            let mut left = state.number(TIME_LEFT) - time;
            if !self.has(own::TIME_WARNED) {
                if left <= 30.0 {
                    happened.sounds.push(Sound::Play(sound::TIME_SHORT));
                    self.set(own::TIME_WARNED, true);
                }
            } else if left <= 0.0 {
                happened.sounds.push(Sound::Stop(sound::TIME_SHORT));
            }
            if !self.has(own::TIME_OUT_SOUNDED) && left < 0.0 {
                happened.sounds.push(Sound::Play(sound::TIME_OUT));
                self.set(own::TIME_OUT_SOUNDED, true);
            }
            if left < 0.0 {
                left = 0.0;
            }
            state.set_number(TIME_LEFT, left);
        }
        let after = state.number(AFTER_A_BLOW);
        if after > 0.0 {
            state.set_number(AFTER_A_BLOW, (after - time).max(0.0));
        }
        for (sign, bit) in own::SIGNS.into_iter().zip(mark::SIGNS) {
            if marks & bit != 0 {
                self.set(sign, true);
            }
        }
    }

    /// What she has just avoided (0x140728).
    fn just_avoided(&mut self, state: &mut State, road: &Road, characters: &mut [Character], her: usize, happened: &mut Happened) {
        if state.number(AFTER_A_BLOW) > 0.0 || self.standing == standing::SPECIAL {
            return;
        }
        let mut near = Vec::new();
        for other in 0..characters.len() {
            if other == her {
                continue;
            }
            let (she, it) = pair(characters, her, other);
            if meet(she, it, NEAR, NEAR) && near.len() < 3 {
                near.push(it.number);
            }
        }
        if near.is_empty() {
            return;
        }
        let she = &characters[her];
        if she.has(flag::MAY_TURN) {
            let to_be_left = |beside: Option<usize>| beside.is_some_and(|beside| road.chips[beside].marks & mark::TO_BE_LEFT != 0);
            let beside = she.chip().map(|chip| &road.chips[chip]);
            if !(beside.is_some_and(|chip| to_be_left(chip.left)) && beside.is_some_and(|chip| to_be_left(chip.right))) {
                self.set(own::AVOID_SIGN, true);
            }
        }
        if self.turned == 0 {
            return;
        }
        happened.told.push((she.number, Told { entry: entry::JUST_AVOID, event: 0, with: self.turned }));
        for number in near {
            happened.told.push((number, Told { entry: entry::JUST_AVOID, event: 1, with: self.turned }));
        }
        self.standing = standing::RIDING;
        self.turned = 0;
        self.row += 1;
        self.row_time = state.number(ROW_TIME);
        self.set(own::ROW_WARNED, false);
        happened.sounds.push(Sound::Stop(sound::ROW_RUNS_OUT));
    }

    /// Whom her special strikes (0x13F534): the nearest ahead of her.
    fn aim(&mut self, state: &mut State, characters: &[Character], her: usize, happened: &mut Happened) {
        if !state.has(SPECIAL_OUT) {
            return;
        }
        let she = &characters[her];
        let (low, high) = (she.place - Vec3::new(100.0, 400.0, 0.0), she.place + Vec3::new(100.0, 400.0, 8000.0));
        let within = |at: Vec3| (0..3).all(|axis| at[axis] >= low[axis] && at[axis] <= high[axis]);
        let nearest = characters
            .iter()
            .filter(|other| other.number != she.number && other.has(flag::CAN_BE_STRUCK) && within(other.place))
            .min_by(|one, other| one.place.distance(she.place).total_cmp(&other.place.distance(she.place)));
        if let Some(nearest) = nearest {
            happened.told.push((nearest.number, Told { entry: entry::BOMB, event: 2, with: 0 }));
        }
    }

    /// Blows (0x13F6DC).
    fn blows(&mut self, state: &mut State, characters: &mut [Character], her: usize, happened: &mut Happened) {
        if characters[her].untouchable > 0.0 {
            return;
        }
        let mut taken = 0;
        for other in 0..characters.len() {
            if other == her || characters[other].untouchable > 0.0 {
                continue;
            }
            let (she, it) = pair(characters, her, other);
            if !meet(she, it, BLOW, BLOW) {
                continue;
            }
            for index in 0..BOXES {
                if it.boxes[index].met {
                    let (worth, says) = (it.boxes[index].worth, it.boxes[index].says);
                    happened.told.extend(she.take(worth, says).into_iter().map(|told| (she.number, told)));
                    taken += worth;
                }
            }
            happened.told.push((it.number, Told { entry: entry::TOUCH, event: 0, with: 0 }));
            self.set(own::GEAR_TOLD | own::RIDING_SOUNDED, false);
            self.gear_time = 0.0;
            if state.word(GOING) != state::SPECIAL {
                state.set_word(GOING, state::HIT);
            }
        }
        if taken > 0 {
            happened.hurt = true;
        }
    }

    /// The end (0x13F8CC).
    fn end(&mut self, state: &mut State, road: &Road, she: &Character, happened: &mut Happened) {
        if state.has(GOAL) {
            let begins = |chip: i32| road.chips.get(chip as usize).map_or(0.0, |chip| chip.begins.z);
            let (start, goal) = (begins(state.word(START_CHIP)), begins(state.word(GOAL_CHIP)));
            if ((goal - start) - (she.place.z - start)).max(0.0) <= 0.0 {
                happened.ended = Some(Ended::Goal);
                return;
            }
        }
        if state.has(CAN_DIE) && she.left <= 0 {
            happened.ended = Some(Ended::Failed);
            return;
        }
        if state.has(TIME_CAN_RUN_OUT) && state.number(TIME_LEFT) <= 0.0 {
            happened.ended = Some(Ended::Failed);
        }
    }
}

/// Two of the characters at once.
fn pair(characters: &mut [Character], one: usize, other: usize) -> (&mut Character, &mut Character) {
    if one < other {
        let (head, tail) = characters.split_at_mut(other);
        (&mut head[one], &mut tail[0])
    } else {
        let (head, tail) = characters.split_at_mut(one);
        (&mut tail[0], &mut head[other])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bike::chara::HitBox;
    use crate::bike::road::tests::normal;
    use crate::bike::tables::{self, SystemTables, SYSTEM_PACK, SYSTEM_TABLES};

    const FRAME: f32 = 1.0 / 30.0;

    fn game() -> Option<(Road, [u8; STATE_LEN], Vec<Character>)> {
        let road = normal()?;
        let data = tables::tests::data()?;
        let mut bytes = SystemTables::read(&tables::tests::script(&data, SYSTEM_PACK, SYSTEM_TABLES)).unwrap().state;
        let mut state = State(&mut bytes);
        // As the course's script has it once the ride has begun.
        state.set_word(FLAGS, (PAD | GOAL | CAN_DIE | TIME_CAN_RUN_OUT | TIME_RUNS | PAD_CHANGES_LANES | SPECIAL_CAN_BE_HAD) as i32);
        state.set_word(START_CHIP, 12);
        state.set_word(GOAL_CHIP, 77 * 5 + 2);
        state.set_number(TIME_LEFT, 240.0);
        let mut she = Character::new(0);
        she.rides = 1;
        (she.left, she.could) = (1000, 1000);
        she.set(flag::MAY_TURN | flag::CAN_BE_HURT, true);
        she.put(&road, 12);
        let ball = |kinds: u32, height: f32, radius: f32| HitBox { shape: 1, on: true, bone: -1, middle: Vec3::new(0.0, height, 0.0), radius, kinds, ..HitBox::default() };
        (she.boxes[0], she.boxes[1]) = (ball(BLOW, 60.0, 40.0), ball(NEAR, 40.0, 70.0));
        Some((road, bytes, vec![she]))
    }

    fn frame(rules: &mut Rules, bytes: &mut [u8; STATE_LEN], road: &Road, characters: &mut [Character], asked: Asked) -> Happened {
        for character in characters.iter_mut() {
            character.step(road, FRAME);
        }
        rules.step(&mut State(bytes), road, characters, asked, FRAME)
    }

    #[test]
    fn she_comes_up_through_the_first_gear_and_stays_in_it_until_the_next_can_be_come_to() {
        let Some((road, mut bytes, mut characters)) = game() else { return };
        let mut rules = Rules::default();
        let first = frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        assert!(first.sounds.contains(&Sound::Play(sound::RIDING)));
        assert!(rules.speed > 80.0 && rules.speed < 85.0, "{}", rules.speed);
        let mut told = Vec::new();
        for _ in 0..90 {
            told.extend(frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default()).told_first);
        }
        // Two seconds take her to 140 km/h, where she is told of the gear, once.
        assert_eq!((rules.gear, rules.speed), (0, 140.0));
        assert_eq!(told, [(0, Told { entry: entry::EVENT, event: 1, with: 0 })]);
        assert_eq!(rules.shown.speed, 140);
        let flags = State(&mut bytes).word(FLAGS);
        State(&mut bytes).set_word(FLAGS, flags | GEARS[0] as i32);
        frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        assert_eq!(rules.gear, 1);
        // The time runs.
        assert!((State(&mut bytes).number(TIME_LEFT) - (240.0 - 92.0 * FRAME)).abs() < 1e-3);
    }

    #[test]
    fn the_pad_turns_her_only_on_to_a_lane_that_is_not_shut() {
        let Some((road, mut bytes, mut characters)) = game() else { return };
        let mut rules = Rules::default();
        let left = Asked { left: true, ..Asked::default() };
        let happened = frame(&mut rules, &mut bytes, &road, &mut characters, left);
        assert_eq!(happened.told, [(0, Told { entry: entry::TURN_LEFT, event: 0, with: 0 })]);
        // Her script would change her lane; do it for it, and wait the change out.
        characters[0].change_lane(&road, false);
        for _ in 0..20 {
            frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        }
        assert_eq!(characters[0].chip().map(|chip| road.chips[chip].lane), Some(1));
        // Lane 0 is shut: the pad does not take her on to it.
        assert_eq!(frame(&mut rules, &mut bytes, &road, &mut characters, left).told, []);
    }

    #[test]
    fn avoiding_five_in_a_row_fills_the_gauge_and_square_begins_her_special() {
        let Some((road, mut bytes, mut characters)) = game() else { return };
        let mut rules = Rules::default();
        frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        for count in 1..=5 {
            // Something near her, in the lane beside, as she turns away from it.
            let mut other = Character::new(0x100 + count);
            other.place = characters[0].place + Vec3::new(120.0, 0.0, 0.0);
            other.boxes[1] = HitBox { shape: 1, on: true, bone: -1, middle: Vec3::ZERO, radius: 70.0, kinds: NEAR, ..HitBox::default() };
            let mut both = vec![characters[0].clone(), other];
            let happened = rules.step(&mut State(&mut bytes), &road, &mut both, Asked { right: true, ..Asked::default() }, FRAME);
            assert!(happened.told.contains(&(0, Told { entry: entry::JUST_AVOID, event: 0, with: 2 })), "{:?}", happened.told);
            assert!(happened.told.contains(&(0x100 + count, Told { entry: entry::JUST_AVOID, event: 1, with: 2 })));
            assert_eq!((rules.row, rules.row_time), (count, 10.0));
        }
        let full = frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        assert_eq!(full.sounds, []);
        let announced = frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        assert_eq!(announced.sounds, [Sound::Voice(sound::GAUGE_FULL_VOICE), Sound::Play(sound::GAUGE_FULL)]);
        let begun = frame(&mut rules, &mut bytes, &road, &mut characters, Asked { special: true, ..Asked::default() });
        assert!(begun.special_begun && rules.standing == standing::SPECIAL);
        assert!(begun.told.contains(&(0, Told { entry: entry::BOMB, event: 0, with: 0 })));
        assert_eq!(State(&mut bytes).word(GOING), state::SPECIAL);
        // It lasts five seconds, and she is told as it ends.
        let mut told = Vec::new();
        for _ in 0..155 {
            told.extend(frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default()).told);
        }
        assert!(told.contains(&(0, Told { entry: entry::BOMB, event: 1, with: 0 })));
        assert_eq!(rules.standing, standing::RIDING);
    }

    #[test]
    fn a_blow_takes_its_worth_and_the_ride_ends_at_the_goal_or_with_nothing_left() {
        let Some((road, mut bytes, mut characters)) = game() else { return };
        let mut rules = Rules::default();
        frame(&mut rules, &mut bytes, &road, &mut characters, Asked::default());
        let mut wall = Character::new(0x100);
        wall.place = characters[0].place + Vec3::new(0.0, 0.0, 60.0);
        wall.boxes[0] = HitBox { shape: 2, on: true, bone: -1, middle: Vec3::new(0.0, 50.0, 0.0), size: Vec3::new(200.0, 100.0, 50.0), kinds: BLOW, says: 1, worth: 400, ..HitBox::default() };
        characters.push(wall);
        let happened = rules.step(&mut State(&mut bytes), &road, &mut characters, Asked::default(), FRAME);
        assert!(happened.hurt && characters[0].left == 600);
        assert_eq!(happened.told, [(0, Told { entry: entry::TOUCH, event: 1, with: 0 }), (0x100, Told { entry: entry::TOUCH, event: 0, with: 0 })]);
        assert_eq!(State(&mut bytes).word(GOING), state::HIT);
        assert_eq!(happened.ended, None);
        // With nothing left she has failed.
        characters[0].left = 0;
        characters.truncate(1);
        assert_eq!(rules.step(&mut State(&mut bytes), &road, &mut characters, Asked::default(), FRAME).ended, Some(Ended::Failed));
        // At the goal chip's beginning she has arrived.
        characters[0].left = 600;
        characters[0].place.z = 770000.0;
        assert_eq!(rules.step(&mut State(&mut bytes), &road, &mut characters, Asked::default(), FRAME).ended, Some(Ended::Goal));
    }
}
