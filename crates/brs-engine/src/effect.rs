//! Effects as they play: what the game's effect library (about 0x11B600 to
//! 0x129000) does with an effect's script (`brs_formats::esb`), frame by frame,
//! and what there is to draw of it. Nothing here draws.
//!
//! An effect is a timeline of emitters. An emitter has a place, lets particles go
//! from a shape about it, so many at a time and every so many frames, and may hang
//! from another, when it lets them go from each particle of that one. A particle is
//! a flat picture or a model; it goes its way at a speed, and has a size, a colour
//! and a turning over its life. Frames are thirtieths of a second.
//!
//! All the effects that play share one run of random numbers, and no more than 128
//! particles between them (0x128D4C).

use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use bevy::math::{Mat3, Mat4, Vec3};
use brs_formats::esb::{self, Blend, Held, Kind, Run};

/// The particles all the effects may have between them: one that would be let go
/// beyond them is not (0x126620).
pub const MOST_PARTICLES: usize = 0x80;
/// The frame that ends a run of keys, as the time a cursor does not go past.
const END: f32 = 16384.0;
/// The middle of the screen, which what is fixed to the screen is placed from
/// (0x11CE60).
const SCREEN_MIDDLE: Vec3 = Vec3::new(240.0, 136.0, 0.0);
/// The frames of a model's animation that go by in a frame of an effect
/// (0x1226F0): its clock is in sixtieths of a second.
pub const MODEL_FRAMES_A_FRAME: f32 = 2.0;

/// The game's random numbers: the Mersenne Twister, as it has it (0x121818,
/// 0x121888).
#[derive(Clone)]
pub struct Random {
    words: [u32; 624],
    at: usize,
}

impl Random {
    /// What the game starts it from (the word at 0x172CA8, and 5489 more).
    pub const SEED: u32 = 5489;

    pub fn new(seed: u32) -> Random {
        let mut words = [0u32; 624];
        words[0] = seed;
        for index in 1..words.len() {
            let before = words[index - 1];
            words[index] = 0x6c07_8965u32.wrapping_mul(before ^ (before >> 30)).wrapping_add(index as u32);
        }
        Random { words, at: 624 }
    }

    pub fn next(&mut self) -> u32 {
        if self.at >= 624 {
            for index in 0..624 {
                let mixed = (self.words[index] & 0x8000_0000) | (self.words[(index + 1) % 624] & 0x7fff_ffff);
                let turned = (mixed >> 1) ^ if mixed & 1 != 0 { 0x9908_b0df } else { 0 };
                self.words[index] = self.words[(index + 397) % 624] ^ turned;
            }
            self.at = 0;
        }
        let mut word = self.words[self.at];
        self.at += 1;
        word ^= word >> 11;
        word ^= (word << 7) & 0x9d2c_5680;
        word ^= (word << 15) & 0xefc6_0000;
        word ^ (word >> 18)
    }

    /// From nought up to one (0x121A34).
    fn unit(&mut self) -> f32 {
        self.next() as f32 * (1.0 / 4_294_967_296.0)
    }

    /// From one below nought up to one (0x121A74).
    fn either_way(&mut self) -> f32 {
        self.next() as f32 * (1.0 / 2_147_483_648.0) - 1.0
    }

    /// From a half below nought up to a half (0x121AC0).
    fn half(&mut self) -> f32 {
        self.next() as f32 * (1.0 / 4_294_967_296.0) - 0.5
    }

    /// One, or one below nought (0x121B0C).
    fn sign(&mut self) -> f32 {
        if self.next() & 1 != 0 {
            1.0
        } else {
            -1.0
        }
    }

    /// Three of a half either way: x, then y, then z (0x121B4C).
    fn halves(&mut self) -> Vec3 {
        let x = self.half();
        let y = self.half();
        Vec3::new(x, y, self.half())
    }
}

/// What every effect that plays shares.
pub struct Pool {
    pub random: Random,
    /// How many particles all the effects have now: whoever steps the effects
    /// counts them before a frame ([`Effect::particles`]).
    pub particles: usize,
}

impl Default for Pool {
    fn default() -> Pool {
        Pool { random: Random::new(Random::SEED), particles: 0 }
    }
}

/// The camera as the effects go by it (0xA5750).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// How it is turned: its own matrix in the world without its place.
    pub turn: Mat4,
    pub place: Vec3,
    /// Its turn about the upright axis alone (0x24968).
    pub heading: f32,
}

impl Camera {
    /// From the camera's matrix in the world.
    pub fn of(world: Mat4) -> Camera {
        let behind = world.z_axis.truncate();
        let turn = Mat4::from_mat3(Mat3::from_cols(world.x_axis.truncate().normalize_or_zero(), world.y_axis.truncate(), behind.normalize_or_zero()));
        Camera { turn, place: world.w_axis.truncate(), heading: behind.x.atan2(behind.z) }
    }
}

impl Default for Camera {
    fn default() -> Camera {
        Camera::of(Mat4::IDENTITY)
    }
}

/// A turn about x, then y, then z as the game puts three together: z's first
/// (0x121ED4).
fn turned(by: Vec3) -> Mat4 {
    Mat4::from_rotation_y(by.y) * Mat4::from_rotation_x(by.x) * Mat4::from_rotation_z(by.z)
}

/// What undoes a matrix of a turn and a place (0x162100): the game uses it of
/// matrices with sizes in them as well, and so does this.
fn undone(matrix: &Mat4) -> Mat4 {
    let back = Mat3::from_mat4(*matrix).transpose();
    let place = -(back * matrix.w_axis.truncate());
    Mat4::from_cols(back.x_axis.extend(0.0), back.y_axis.extend(0.0), back.z_axis.extend(0.0), place.extend(1.0))
}

/// A matrix's turn with its sizes taken out, as the game takes them (0x162550):
/// its second axis kept, the others squared to it.
fn squared(matrix: &Mat4) -> Mat4 {
    let (x, y) = (matrix.x_axis.truncate(), matrix.y_axis.truncate());
    let z = x.cross(y);
    let x = y.cross(z);
    let whole = |axis: Vec3| (axis * axis.length_squared().sqrt().recip()).extend(0.0);
    Mat4::from_cols(whole(x), whole(y), whole(z), matrix.w_axis)
}

fn without_place(mut matrix: Mat4) -> Mat4 {
    matrix.w_axis = Mat4::IDENTITY.w_axis;
    matrix
}

/// Where a run of keys is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Cursor {
    time: f32,
    index: usize,
    /// There is a key after the first: a run of one key is never stepped.
    more: bool,
    at_last: bool,
}

impl Cursor {
    /// At a run's first key (0x11F094).
    fn start<T>(run: &Run<T>) -> Cursor {
        let more = run.keys.len() > 1;
        Cursor { time: 0.0, index: 0, more, at_last: !more }
    }

    fn stepped<T>(&mut self, run: &Run<T>, by: f32, back: impl Fn(&Cursor, usize) -> Option<u8>) -> Option<usize> {
        if !self.more || self.at_last || !(self.time < END) {
            return None;
        }
        let next = self.index + 1;
        match run.keys.get(next) {
            Some(key) if !(self.time < f32::from(key.frame)) => match back(self, next) {
                Some(to) => {
                    self.index = usize::from(to);
                    self.time = f32::from(run.keys[self.index].frame) + by;
                }
                None => {
                    self.index = next;
                    self.time += by;
                }
            },
            after => {
                self.time += by;
                self.at_last = after.is_none();
            }
        }
        Some(self.index)
    }

    /// Goes on by so much of a frame, and gives the key the run is at then, or
    /// nothing once it is at its last (0x11F3B8). When the frame of a key that
    /// goes back comes, the run goes back, unless the effect is ending.
    fn step<T>(&mut self, run: &Run<T>, by: f32, ending: bool) -> Option<usize> {
        self.stepped(run, by, |_, next| run.keys[next].back_to.filter(|_| !ending))
    }

    /// The same for the run of which picture a particle is (0x11F534): it is the
    /// key the run is at that says where it goes back to as the next one's frame
    /// comes, and it does so though the effect is ending.
    fn step_of<T>(&mut self, run: &Run<T>, by: f32) -> Option<usize> {
        self.stepped(run, by, |cursor, _| run.keys[cursor.index].back_to)
    }

    /// How far it is from the key it is at to the next, from nought to one
    /// (0x11F0F4).
    fn part<T>(&self, run: &Run<T>) -> f32 {
        let (Some(key), Some(next)) = (run.keys.get(self.index), run.keys.get(self.index + 1)) else { return 0.0 };
        let since = self.time - f32::from(key.frame);
        if since == 0.0 {
            return 0.0;
        }
        let part = since * (1.0 / (f32::from(next.frame) - f32::from(key.frame)));
        if part < 0.0 {
            0.0
        } else if part <= 1.0 {
            part
        } else {
            1.0
        }
    }

    /// A step, and the two keys the run is between with how far it is from the
    /// one to the other: nothing when it is on a key, when what the run had
    /// before stays (0x11F880).
    fn between<T: Copy>(&mut self, run: &Run<T>, by: f32, ending: bool) -> Option<(T, T, f32)> {
        let at = self.step(run, by, ending)?;
        let part = self.part(run);
        (part != 0.0).then(|| (run.keys[at].value, run.keys[at + 1].value, part))
    }
}

fn mix(from: f32, to: f32, part: f32) -> f32 {
    (to - from) * part + from
}

fn mix3(from: [f32; 3], to: [f32; 3], part: f32) -> Vec3 {
    Vec3::from(from).lerp(Vec3::from(to), part)
}

/// The matrices of an effect: with its sizes in it, without them, and its sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placing {
    whole: Mat4,
    plain: Mat4,
    scale: Vec3,
}

impl Placing {
    const NONE: Placing = Placing { whole: Mat4::IDENTITY, plain: Mat4::IDENTITY, scale: Vec3::ONE };
}

/// What a frame is given that is the same for all of an effect.
struct Frame<'a> {
    camera: &'a Camera,
    placing: Placing,
    /// The effect was given no matrix, and the game then takes its sizes to be
    /// set (0x12756C).
    sized: bool,
    tint: Option<[f32; 4]>,
    ending: bool,
    by: f32,
}

/// A model particle's model.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Model {
    index: usize,
    /// How far its animation is, in the animation's frames from its start.
    frames: f32,
}

struct Particle {
    id: u32,
    /// The particle of the emitter its emitter hangs from that it was let go from.
    parent: Option<u32>,
    age: f32,
    /// The frames it has left, unless it is endless.
    life: f32,
    endless: bool,
    over: bool,
    /// How fast its own time goes.
    pace: f32,
    /// What its place is taken from when its emitter is fixed to something.
    origin: Vec3,
    place: Vec3,
    before: Vec3,
    direction: Vec3,
    /// The emitters' times at which they last let something go for it, by their
    /// entries; its own emitter's is when it was let go (0x1258D4).
    since: Vec<f32>,

    of: (Cursor, esb::Of),
    size: (Cursor, Vec3),
    size_by: Option<Vec3>,
    speed: (Cursor, f32),
    speed_by: f32,
    /// As the run has it; what is drawn is [`Particle::colour`].
    colour: (Cursor, [u8; 4]),
    colour_by: [f32; 4],
    turn: (Cursor, Vec3),
    turn_more: Option<Vec3>,
    /// What its turns are multiplied by; nought for nothing.
    turn_by: f32,
    drift: Option<(Cursor, Vec3, f32)>,
    /// Its middle's offset, and whether that is anything.
    off: Option<(Cursor, Vec3, bool)>,

    turning: Mat4,
    held: Option<Placing>,
    model: Option<Model>,
    /// As it was last worked out to be drawn: its matrix, its colour, and
    /// whether there is anything of it to see.
    drawn: Mat4,
    drawn_colour: [u8; 4],
    shown: bool,
}

/// What a particle needs to know of its emitter.
struct Of<'a> {
    entry: &'a esb::Entry,
    emitter: &'a esb::Emitter,
    /// It is fixed to the screen (0x11E3EC).
    screen: bool,
    /// An emitter that hangs from it has its particles keep to this one's
    /// (0x128320).
    kept_to: bool,
    /// The middles of the particles of the emitter it hangs from are off by
    /// their sizes.
    parent_off_by_size: bool,
}

impl Of<'_> {
    fn of(script: &esb::Script, index: usize, kept_to: bool) -> Option<Of<'_>> {
        let entry = &script.entries[index];
        let emitter = entry.emitter.as_ref()?;
        let parent = entry.parent.and_then(|parent| script.entries[parent].emitter.as_ref());
        Some(Of {
            entry,
            emitter,
            screen: (11..16).contains(&emitter.fixed_to),
            kept_to,
            parent_off_by_size: parent.is_some_and(|parent| parent.off.is_some() && parent.off_by_size),
        })
    }

    fn by_camera(&self) -> bool {
        matches!(self.emitter.fixed_to, 16 | 17)
    }
}

impl Particle {
    /// A particle let go (0x125708).
    #[allow(clippy::too_many_arguments)]
    fn new(id: u32, of: &Of, frame: &Frame, random: &mut Random, parent: Option<u32>, origin: Vec3, place: Vec3, direction: Vec3, held: Option<Placing>, entries: usize, slot: usize, time: f32) -> Particle {
        let emitter = of.emitter;
        let mut pace = 1.0;
        if emitter.pace_spread != 0 {
            pace += random.either_way() * 0.01 * f32::from(emitter.pace_spread);
        }
        let mut since = vec![0.0; entries];
        since[slot] = time;

        // The size (0x1200E8): each particle's is by one less a part of the
        // spread, one draw for all three when the second and third are below
        // nought.
        let first = |run: &Run<[f32; 3]>| Vec3::from(run.keys[0].value);
        let mut size = first(&emitter.size);
        let [x, y, z] = emitter.size_spread;
        let size_by = if y < 0.0 && z < 0.0 {
            (x != 0.0).then(|| Vec3::splat(1.0 - random.half() * x))
        } else if x == 0.0 && y == 0.0 && z == 0.0 {
            None
        } else {
            let by_x = 1.0 - random.half() * x;
            let by_y = 1.0 - random.half() * y;
            Some(Vec3::new(by_x, by_y, 1.0 - random.half() * z))
        };
        if let Some(by) = size_by {
            size *= by;
        }
        if emitter.size_by != 1.0 {
            size *= emitter.size_by;
        }

        // The speed (0x1204A8): its draw is made whatever the spread.
        let speed_by = 1.0 - random.half() * emitter.speed_spread;
        let speed = emitter.speed.keys[0].value * speed_by;

        // The colour (0x120824): the spreads are kept within nought and one; its
        // first colour is as the run has it.
        let spread = emitter.colour_spread;
        let within = spread.map(|spread| if spread <= 1.0 { spread.max(0.0) } else { 1.0 });
        let colour_by = if spread == [0.0; 4] {
            [1.0; 4]
        } else {
            let [red, green, blue] = if spread[0] != 0.0 && spread[1] < 0.0 && spread[2] < 0.0 {
                [1.0 - random.half() * within[0]; 3]
            } else {
                let red = 1.0 - random.half() * within[0];
                let green = 1.0 - random.half() * within[1];
                [red, green, 1.0 - random.half() * within[2]]
            };
            [red, green, blue, 1.0 - random.half() * within[3]]
        };
        let colour = emitter.colour.keys.first().map_or([0; 4], |key| key.value.map(|part| part as u8));

        let drift = emitter.drift.as_ref().map(|run| (Cursor::start(run), Vec3::from(run.keys[0].value.along), run.keys[0].value.speed));

        // The turning (0x1215C0): turns more by a part of the spreads, of which a
        // particle that faces the camera takes the third alone and one upright
        // to it the first and third; then all by a part of themselves.
        let [x, y, z] = emitter.turn_spread;
        let more = match emitter.faces {
            0 => Vec3::new(0.0, 0.0, random.either_way() * z),
            2 => {
                let more_x = random.either_way() * x;
                Vec3::new(more_x, 0.0, random.either_way() * z)
            }
            _ => {
                let more_x = random.either_way() * x;
                let more_y = random.either_way() * y;
                Vec3::new(more_x, more_y, random.either_way() * z)
            }
        };
        let turn_more = (more != Vec3::ZERO).then_some(more);
        let mut turn = first(&emitter.turn) + turn_more.unwrap_or(Vec3::ZERO);
        let turn_by = if emitter.turn_random { random.either_way() } else { 0.0 };
        if emitter.turn_random {
            turn *= turn_by;
        }

        let off = emitter.off.as_ref().map(|run| {
            let value = first(run);
            (Cursor::start(run), value, value != Vec3::ZERO)
        });

        let which = emitter.of.keys[0].value;
        // A model particle is of the model its run first names (0x1235C0), and
        // stays so.
        let model = (emitter.kind == Kind::Model).then(|| usize::try_from(which.first).ok()).flatten().map(|index| Model { index, frames: 0.0 });
        let held = match of.entry.held {
            Held::Now => None,
            Held::AtBirth => Some(frame.placing),
            Held::AtStart => held,
            // The game leaves its sizes at nought, which is what it then goes by
            // (0x125AEC).
            Held::Never => Some(Placing { whole: Mat4::IDENTITY, plain: Mat4::IDENTITY, scale: Vec3::ZERO }),
        };
        Particle {
            id,
            parent,
            age: 0.0,
            life: f32::from(emitter.life),
            endless: emitter.life == 0,
            over: false,
            pace,
            origin,
            place,
            before: place,
            direction,
            since,
            of: (Cursor::start(&emitter.of), which),
            size: (Cursor::start(&emitter.size), size),
            size_by,
            speed: (Cursor::start(&emitter.speed), speed),
            speed_by,
            colour: (Cursor::start(&emitter.colour), colour),
            colour_by,
            turn: (Cursor::start(&emitter.turn), turn),
            turn_more,
            turn_by,
            drift,
            off,
            turning: Mat4::IDENTITY,
            held,
            model,
            drawn: Mat4::IDENTITY,
            drawn_colour: [0; 4],
            shown: false,
        }
    }

    /// A frame of its life (0x125B1C). `orphaned` is that it keeps to a particle
    /// that is no more.
    fn step(&mut self, of: &Of, frame: &Frame, orphaned: bool) {
        if self.over {
            return;
        }
        let emitter = of.emitter;
        let by = frame.by * self.pace;
        // An endless particle of an effect told to end goes once its colour has
        // run out to nothing.
        if frame.ending && self.colour.0.at_last && self.colour.1[3] == 0 {
            self.endless = false;
        }
        if orphaned {
            self.endless = false;
            self.life = 0.0;
        }
        if !self.endless {
            self.life -= by;
            // One that has not had a frame yet has this one.
            if self.life < 0.0 && self.age != 0.0 {
                self.over = true;
                return;
            }
        }

        if let Some(at) = self.of.0.step_of(&emitter.of, by) {
            self.of.1 = emitter.of.keys[at].value;
        }
        if let Some((from, to, part)) = self.size.0.between(&emitter.size, by, frame.ending) {
            let mut size = mix3(from, to, part);
            if let Some(by) = self.size_by {
                size *= by;
            }
            if emitter.size_by != 1.0 {
                size *= emitter.size_by;
            }
            self.size.1 = size;
        }
        if let Some((from, to, part)) = self.speed.0.between(&emitter.speed, by, frame.ending) {
            self.speed.1 = mix(from, to, part) * self.speed_by;
        }
        if let Some((from, to, part)) = self.turn.0.between(&emitter.turn, by, frame.ending) {
            let mut turn = mix3(from, to, part) + self.turn_more.unwrap_or(Vec3::ZERO);
            if self.turn_by != 0.0 {
                turn *= self.turn_by;
            }
            self.turn.1 = turn;
        }
        if let Some((from, to, part)) = self.colour.0.between(&emitter.colour, by, frame.ending) {
            self.colour.1 = std::array::from_fn(|index| mix(f32::from(from[index]), f32::from(to[index]), part) as i32 as u8);
        }
        if let (Some((cursor, along, speed)), Some(run)) = (&mut self.drift, &emitter.drift) {
            // Its speed is taken at every step, its way only between keys
            // (0x12112C).
            if let Some(at) = cursor.step(run, by, frame.ending) {
                let part = cursor.part(run);
                let key = run.keys[at].value;
                match run.keys.get(at + 1).filter(|_| part != 0.0) {
                    Some(next) => {
                        *along = mix3(key.along, next.value.along, part);
                        *speed = mix(key.speed, next.value.speed, part);
                    }
                    None => *speed = key.speed,
                }
            }
        }
        if let (Some((cursor, value, set)), Some(run)) = (&mut self.off, &emitter.off) {
            if let Some((from, to, part)) = cursor.between(run, by, frame.ending) {
                *value = mix3(from, to, part);
                *set = *value != Vec3::ZERO;
            }
        }

        // Its move (0x123C40): along its way at its speed, and by its drift.
        self.before = self.place;
        let speed = if by != 1.0 { self.speed.1 * by } else { self.speed.1 };
        let mut moved = if speed != 0.0 { self.direction * speed } else { Vec3::ZERO };
        let drifted = self.drift.map_or(0.0, |(_, _, speed)| speed * by);
        if drifted != 0.0 {
            let mut drift = self.drift.map_or(Vec3::ZERO, |(_, along, _)| along) * drifted;
            if of.screen {
                drift.y = -drift.y;
            }
            moved += drift;
        }
        if speed != 0.0 || drifted != 0.0 {
            self.place += moved;
        }

        // Its turning is made anew on its first frame, and after that while it
        // moves if it faces its way, or while its turns still change.
        let anew = self.age == 0.0 || if emitter.faces == 7 { self.place != self.before } else { self.turn.0.more };
        if anew {
            self.turning = self.turned(emitter.faces);
        }
        self.age += by;
    }

    /// Its own turning (0x123FE0).
    fn turned(&self, faces: u16) -> Mat4 {
        let turn = self.turn.1;
        let off = self.off.is_some_and(|(_, _, set)| set);
        match faces {
            // It rolls, facing the camera; with its middle off its place it
            // takes all three turns.
            0 | 1 if !off => Mat4::from_rotation_z(turn.z),
            2 | 3 if !off => Mat4::from_rotation_x(turn.x) * Mat4::from_rotation_z(turn.z),
            0..=3 => turned(turn),
            4..=6 => match (turn.x != 0.0, turn.y != 0.0, turn.z != 0.0) {
                (true, false, false) => Mat4::from_rotation_x(turn.x),
                (false, true, false) => Mat4::from_rotation_y(turn.y),
                (false, false, true) => Mat4::from_rotation_z(turn.z),
                _ => turned(turn),
            },
            7 => {
                // Lined up with the way it is going: a picture's upright, a
                // model's forward (0x124098).
                let (line, mirrored) = if self.model.is_some() { (Vec3::Z, Vec3::new(1.0, 1.0, -1.0)) } else { (Vec3::Y, Vec3::new(1.0, -1.0, 1.0)) };
                let way = (self.place - self.before).normalize();
                let along = line.dot(way);
                let lined = if !(along < 0.999_999) {
                    Mat4::IDENTITY
                } else if along <= -0.999_999 {
                    Mat4::from_scale(mirrored)
                } else {
                    let about = line.cross(way);
                    if about == Vec3::ZERO {
                        Mat4::IDENTITY
                    } else {
                        Mat4::from_axis_angle(about.normalize(), along.acos())
                    }
                };
                lined * turned(turn)
            }
            _ => Mat4::IDENTITY,
        }
    }

    /// The colour it is drawn in (0x120D20).
    fn colour(&self, tint: Option<[f32; 4]>) -> [u8; 4] {
        std::array::from_fn(|index| {
            let part = f32::from(self.colour.1[index]) * self.colour_by[index] * tint.map_or(1.0, |tint| tint[index]);
            // Kept within a byte: what is not a number at all comes out as nought.
            if part < 0.0 {
                0
            } else if part <= 255.0 {
                part as u8
            } else {
                255
            }
        })
    }

    /// What it is to be drawn as, worked out once a frame after it has moved
    /// (0x1239E4): its matrix only if there is anything of it to see, or a
    /// particle keeps to it.
    fn look(&mut self, of: &Of, frame: &Frame, parent: Option<&Particle>) {
        self.drawn_colour = self.colour(frame.tint);
        self.shown = self.drawn_colour[3] != 0;
        if of.kept_to || self.shown {
            self.drawn = self.matrix(of, frame, parent);
            // A model's animation goes on with the effect's time, not with the
            // particle's own, and holds at its end (0x1256A4, 0x2359C).
            if let Some(model) = &mut self.model {
                model.frames += MODEL_FRAMES_A_FRAME * frame.by;
            }
        }
    }

    /// Its matrix in the world (0x12448C).
    fn matrix(&self, of: &Of, frame: &Frame, parent: Option<&Particle>) -> Mat4 {
        let (emitter, camera) = (of.emitter, frame.camera);
        let Placing { whole, plain, scale } = self.held.unwrap_or(frame.placing);
        let (keeps, fixed_to, by_camera) = (of.entry.keeps, emitter.fixed_to, of.by_camera());
        let mut place = self.place;
        // Its place has had the effect's sizes; the effect's matrix is not for
        // it; what it is turned by besides.
        let (mut sized, mut own, mut turned_by) = (false, false, None);
        if keeps != 0 {
            sized = true;
            if !by_camera {
                place *= scale;
            }
            match (fixed_to, parent) {
                (6, Some(parent)) => {
                    let mut at = parent.drawn.w_axis.truncate();
                    if of.parent_off_by_size {
                        let off = parent.off.map_or(Vec3::ZERO, |(_, value, _)| value);
                        at += parent.turning.transform_vector3(off * parent.size.1);
                    }
                    place += at;
                    if keeps == 2 && emitter.faces != 7 {
                        turned_by = Some(squared(&parent.drawn));
                    }
                    own = true;
                }
                (17, _) if keeps == 1 => place += camera.place,
                (17, _) if keeps == 2 => {
                    place = camera.turn.transform_vector3(place);
                    turned_by = Some(camera.turn);
                }
                _ => {}
            }
        } else if fixed_to != 0 {
            sized = true;
            let gone = place - self.origin;
            place = self.origin + if by_camera { gone } else { gone * scale };
        }

        // A model's size is given to the model, not put in its matrix (0x125688).
        let mut size = if emitter.kind == Kind::Flat { self.size.1 } else { Vec3::ONE };
        let faces = if of.screen { 4 } else { emitter.faces };
        let facing = match faces {
            0 | 1 => Some(camera.turn),
            2 | 3 => Some(Mat4::from_rotation_y(camera.heading)),
            _ => None,
        };
        let laid = match faces {
            5 => Some(Mat4::from_rotation_x(-FRAC_PI_2)),
            6 => Some(Mat4::from_rotation_y(FRAC_PI_2)),
            _ => None,
        };
        if matches!(faces, 4..=6) && sized && !by_camera {
            size *= scale;
        }
        let off = self.off.filter(|(_, _, set)| *set).map(|(_, value, _)| if emitter.off_by_size { value * self.size.1 } else { value });
        let effects = |sized: bool| if sized { plain } else { whole };
        // The effect's own matrix is for what is fixed to nothing of the screen
        // or the camera.
        let by_effect = fixed_to != 11 && !own && !by_camera;

        let mut out;
        if let Some(facing) = facing {
            if frame.sized && !by_camera {
                size *= scale;
            }
            let mut body = self.turning * Mat4::from_scale(size);
            if matches!(faces, 0 | 2) {
                if frame.sized && !sized && !by_camera {
                    place *= scale;
                }
                if let Some(off) = off {
                    place += self.turning.transform_vector3(off);
                }
            } else if let Some(off) = off {
                // Its middle's offset is turned to the camera with it.
                body.w_axis = self.turning.transform_vector3(off).extend(1.0);
            }
            out = facing * body;
            if let Some(by) = turned_by {
                out = without_place(by) * out;
            }
            out.w_axis += place.extend(0.0);
            // Only its place goes by the effect's matrix: it keeps facing the
            // camera.
            if by_effect && !(matches!(faces, 1 | 3) && fixed_to == 3) {
                out.w_axis = effects(sized) * out.w_axis;
            }
        } else {
            let sizes = Mat4::from_scale(size);
            out = match (off, laid) {
                (Some(off), laid) => {
                    let body = self.turning * Mat4::from_translation(off) * sizes;
                    laid.map_or(body, |laid| body * laid)
                }
                (None, Some(laid)) => self.turning * laid * sizes,
                // A model's own matrix is its turning alone.
                (None, None) if self.model.is_some() => self.turning,
                (None, None) => self.turning * sizes,
            };
            if let Some(by) = turned_by {
                out = without_place(by) * out;
            }
            out.w_axis += place.extend(0.0);
            if by_effect && fixed_to != 3 {
                out = effects(sized) * out;
            }
        }
        if of.screen {
            out.w_axis.z = 0.0;
        }
        out
    }
}

struct Emitter {
    time: f32,
    /// When it last let particles go, and where it was then.
    last: f32,
    last_place: Vec3,
    first: bool,
    endless: bool,
    finished: bool,
    hangs: bool,
    kept_to: bool,
    place: (Cursor, Vec3),
    /// The shape's sizes and its turns.
    opening: (Cursor, Vec3, Vec3),
    /// The way, and the two angles it is tipped by.
    way: (Cursor, Vec3, f32, f32),
    /// The frames between lots, and how many in a lot.
    lots: (Cursor, f32, u16),
    /// The effect's matrices as they were when it was made, for the particles of
    /// an emitter that are held so (0x11E564).
    held: Option<Placing>,
    particles: Vec<Particle>,
}

impl Emitter {
    /// An emitter made of an entry (0x11E05C).
    fn new(entry: &esb::Entry, emitter: &esb::Emitter, placing: Placing) -> Emitter {
        let first = |run: &Run<[f32; 3]>| Vec3::from(run.keys[0].value);
        let (opening, way, lot) = (emitter.opening.keys[0].value, emitter.way.keys[0].value, emitter.lots.keys[0].value);
        Emitter {
            time: 0.0,
            last: 0.0,
            last_place: Vec3::ZERO,
            first: true,
            endless: entry.end.is_none(),
            finished: false,
            hangs: entry.parent.is_some(),
            kept_to: false,
            place: (Cursor::start(&emitter.place), first(&emitter.place)),
            opening: (Cursor::start(&emitter.opening), Vec3::from(opening.size), Vec3::from(opening.turn)),
            way: (Cursor::start(&emitter.way), Vec3::from(way.direction), way.tip, way.more_tip),
            lots: (Cursor::start(&emitter.lots), lot.every, lot.count),
            held: (entry.held == Held::AtStart).then_some(placing),
            particles: Vec::new(),
        }
    }

    /// Its four runs a frame on (0x11D900).
    fn step_runs(&mut self, emitter: &esb::Emitter, frame: &Frame) {
        let (by, ending) = (frame.by, frame.ending);
        if let Some((from, to, part)) = self.place.0.between(&emitter.place, by, ending) {
            self.place.1 = mix3(from, to, part);
        }
        if let Some((from, to, part)) = self.opening.0.between(&emitter.opening, by, ending) {
            self.opening.1 = mix3(from.size, to.size, part);
            self.opening.2 = mix3(from.turn, to.turn, part);
        }
        if let Some((from, to, part)) = self.way.0.between(&emitter.way, by, ending) {
            self.way.1 = mix3(from.direction, to.direction, part);
            self.way.2 = mix(from.tip, to.tip, part);
            self.way.3 = mix(from.more_tip, to.more_tip, part);
        }
        if let Some((from, to, part)) = self.lots.0.between(&emitter.lots, by, ending) {
            self.lots.2 = mix(f32::from(from.count), f32::from(to.count), part) as i32 as u16;
            self.lots.1 = mix(from.every, to.every, part);
        }
    }

    /// The shape's own turn (0x11C028).
    fn opening_turn(&self) -> Mat4 {
        let turn = self.opening.2;
        match (turn.x != 0.0, turn.y != 0.0, turn.z != 0.0) {
            (false, false, false) => Mat4::IDENTITY,
            (true, false, false) => Mat4::from_rotation_x(turn.x),
            (false, true, false) => Mat4::from_rotation_y(turn.y),
            (false, false, true) => Mat4::from_rotation_z(turn.z),
            _ => turned(turn),
        }
    }

    /// Where a particle comes out about the emitter's place (0x11D254).
    fn opening(&self, emitter: &esb::Emitter, random: &mut Random, turn: &Mat4) -> Vec3 {
        let size = self.opening.1;
        let on_ball = |random: &mut Random, radius: f32| random.halves().normalize() * radius;
        let at = match emitter.shape {
            0 => {
                let radius = random.unit() * size.x;
                on_ball(random, radius)
            }
            1 => on_ball(random, size.x),
            2 => random.halves() * size,
            // A ring on the flat.
            3 => {
                let angle = (random.unit() * 360.0).to_radians();
                Vec3::new(angle.cos() * size.x, 0.0, angle.sin() * size.x)
            }
            // A round as wide as a ball seen from above, with a height
            // (0x11BF20).
            4 => {
                let at = loop {
                    let at = random.halves() * (size.x * 2.0);
                    if at.length_squared() <= size.x * size.x {
                        break at;
                    }
                };
                Vec3::new(at.x, random.unit() * size.y, at.z)
            }
            _ => Vec3::ZERO,
        };
        turn.transform_vector3(at)
    }

    /// The way a particle is sent (0x11D4A0): the emitter's, tipped off it to
    /// any side by an angle and a part of another more.
    fn way(&self, of: &Of, frame: &Frame, random: &mut Random, parent: Option<&Particle>) -> Vec3 {
        let (_, way, tip, more) = self.way;
        let mut way = match of.emitter.way_space {
            0 | 11 | 12 => way,
            6 => match parent.filter(|_| of.entry.keeps == 2) {
                Some(parent) => parent.turning.transform_vector3(way),
                None => way,
            },
            17 => frame.camera.turn.transform_vector3(way),
            // The spaces that go by whose the effect is are not known here, and
            // the others have no way at all.
            _ => Vec3::ZERO,
        };
        if tip == 0.0 && more == 0.0 {
            return way;
        }
        let side = random.sign();
        let angle = side * (tip + random.unit() * more);
        if angle == 0.0 {
            return way;
        }
        // An axis square to the way, turned about it to any side, and the way
        // tipped about that.
        let across = if Vec3::X.dot(way).abs() < 0.9999 { Vec3::X.cross(way) } else { Vec3::Y.cross(way) };
        let spin = Mat4::from_axis_angle(way.normalize(), random.either_way() * PI);
        let across = spin.transform_vector3(across);
        way = Mat4::from_axis_angle(across.normalize(), angle.to_radians()).transform_vector3(way);
        way
    }

    /// Where the emitter is, by what it is fixed to, and what a particle let go
    /// there takes its place from (0x11CAD8).
    fn fixed(&self, of: &Of, frame: &Frame, parent: Option<&Particle>) -> (Vec3, Vec3) {
        let (emitter, camera, at) = (of.emitter, frame.camera, self.place.1);
        let keeps = of.entry.keeps;
        let out = match emitter.fixed_to {
            0 | 12 => at,
            // A particle of the emitter it hangs from: where that was last
            // drawn, brought into the effect's own space.
            6 => match parent.filter(|_| keeps == 0) {
                Some(parent) => {
                    let base = self.held.map_or(frame.placing.whole, |held| held.whole);
                    let mut from = undone(&base).transform_point3(parent.drawn.w_axis.truncate());
                    if of.parent_off_by_size {
                        let off = parent.off.map_or(Vec3::ZERO, |(_, value, _)| value);
                        from += parent.turning.transform_vector3(off * parent.size.1);
                    }
                    from + at
                }
                None => at,
            },
            11 => at + SCREEN_MIDDLE,
            // About the camera on the flat, at the height given.
            16 => {
                let flat = camera.turn.transform_vector3(Vec3::new(at.x, 0.0, at.z));
                Vec3::new(flat.x + camera.place.x, at.y, flat.z + camera.place.z)
            }
            // Before the camera, which looks the other way along its own axis.
            17 => match keeps {
                0 => (camera.turn * Mat4::from_rotation_y(PI)).transform_vector3(at) + camera.place,
                1 => (camera.turn * Mat4::from_rotation_y(PI)).transform_vector3(at),
                _ => at,
            },
            _ => Vec3::ZERO,
        };
        (out, if out == at { out } else { out - at })
    }

    /// Lets a lot of particles go at a place.
    #[allow(clippy::too_many_arguments)]
    fn let_go(&mut self, of: &Of, frame: &Frame, pool: &mut Pool, ids: &mut u32, entries: usize, slot: usize, at: Vec3, mut origin: Vec3, turn: &Mat4, parent: Option<&Particle>) {
        for _ in 0..self.lots.2 {
            let mut place = at + self.opening(of.emitter, &mut pool.random, turn);
            let mut direction = self.way(of, frame, &mut pool.random, parent);
            if of.screen {
                (origin.z, place.z, direction.z) = (0.0, 0.0, 0.0);
            }
            // The draws for its place and its way are made though there is no
            // room for it.
            if pool.particles >= MOST_PARTICLES {
                continue;
            }
            pool.particles += 1;
            *ids += 1;
            self.particles.push(Particle::new(*ids, of, frame, &mut pool.random, parent.map(|parent| parent.id), origin, place, direction, self.held, entries, slot, self.time));
        }
    }

    /// Whether a lot may go at an instant of the frame: before the emitter's end,
    /// or the first of the frame for one that has just begun.
    fn may(&self, entry: &esb::Entry, at: f32, begun: bool) -> bool {
        self.endless || self.time + at < f32::from(entry.end.unwrap_or(0)) || (begun && at == 0.0)
    }

    /// The lots of a frame of an emitter that hangs from nothing (0x11D88C).
    fn emit(&mut self, of: &Of, frame: &Frame, pool: &mut Pool, ids: &mut u32, entries: usize, slot: usize) {
        let since = self.time - self.last;
        let begun = std::mem::take(&mut self.first);
        if !begun {
            self.step_runs(of.emitter, frame);
        }
        if self.lots.2 == 0 || (!begun && since < self.lots.1) {
            return;
        }
        let turn = self.opening_turn();
        let (now, origin) = self.fixed(of, frame, None);
        let (every, mut at) = (self.lots.1, 0.0);
        while at < frame.by && self.may(of.entry, at, begun) {
            // The later lots of a frame go from places back towards where the
            // emitter was.
            let from = if begun { now } else { self.last_place.lerp(now, 1.0 - at / frame.by) };
            self.let_go(of, frame, pool, ids, entries, slot, from, origin, &turn, None);
            at += every;
        }
        self.last_place = now;
        self.last = self.time;
    }

    /// The lots of a frame of an emitter that hangs from another: from each of
    /// that one's particles, whose age is what this one's start and end are of
    /// (0x11DC34).
    #[allow(clippy::too_many_arguments)]
    fn emit_from(&mut self, of: &Of, frame: &Frame, pool: &mut Pool, ids: &mut u32, entries: usize, slot: usize, parent: &mut Emitter, parent_slot: usize) {
        if !std::mem::take(&mut self.first) {
            self.step_runs(of.emitter, frame);
        }
        let turn = self.opening_turn();
        let every = self.lots.1;
        let (start, end) = (f32::from(of.entry.start), f32::from(of.entry.end.unwrap_or(0)));
        let before = parent.time - frame.by;
        for index in 0..parent.particles.len() {
            let from = &parent.particles[index];
            let age = before - from.since[parent_slot];
            if age < start || (!(age < end) && !self.endless) {
                continue;
            }
            // Lots that come every nought frames would go as the particle ends,
            // and nothing in the game says that it has.
            if every <= 0.0 {
                continue;
            }
            let begun = age - start < frame.by;
            if !begun && before - from.since[slot] < every {
                continue;
            }
            let (now, origin) = self.fixed(of, frame, Some(from));
            let mut at = 0.0;
            while at < frame.by && self.may(of.entry, at, true) {
                self.let_go(of, frame, pool, ids, entries, slot, now, origin, &turn, Some(from));
                at += every;
            }
            parent.particles[index].since[slot] = before;
        }
    }
}

/// What there is to draw of a particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Body {
    /// A square of one by one about its middle, of a texture of the effect's
    /// file by its place among them, or plain.
    Flat { texture: Option<usize> },
    /// A model of the effect's file by its place among them, with how far its
    /// animation is in the animation's frames: it plays once and holds.
    Model { model: usize, frames: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub body: Body,
    pub matrix: Mat4,
    /// Red, green, blue and alpha.
    pub colour: [u8; 4],
    /// Never [`Blend::AsBefore`].
    pub blend: Blend,
    /// It is on the screen and not in the world: in the screen's pixels from its
    /// bottom left corner, upwards, facing out of it (0x128C78).
    pub screen: bool,
    /// Its emitter's place in the order the effect's emitters are drawn in: what
    /// is drawn later is over what was drawn before, however far either is.
    pub turn: usize,
}

/// An effect that is playing.
pub struct Effect {
    script: Arc<esb::Script>,
    placing: Placing,
    placed: bool,
    /// What every particle's colour is multiplied by (0x120D78).
    pub colour: [f32; 4],
    /// How fast its time goes (0x127C28).
    pub speed: f32,
    ending: bool,
    made: bool,
    over: bool,
    emitters: Vec<Option<Emitter>>,
    /// The entries in the order their emitters are drawn in.
    order: Vec<usize>,
    ids: u32,
}

impl Effect {
    pub fn new(script: Arc<esb::Script>) -> Effect {
        Effect { script, placing: Placing::NONE, placed: false, colour: [1.0; 4], speed: 1.0, ending: false, made: false, over: false, emitters: Vec::new(), order: Vec::new(), ids: 0 }
    }

    /// Puts it by a matrix (0x127960): its sizes are those of the matrix's axes.
    pub fn place(&mut self, matrix: Mat4) {
        let axes = [matrix.x_axis, matrix.y_axis, matrix.z_axis].map(|axis| axis.truncate());
        let whole = |axis: Vec3| (axis * axis.length_squared().sqrt().recip()).extend(0.0);
        let plain = Mat4::from_cols(whole(axes[0]), whole(axes[1]), whole(axes[2]), matrix.w_axis);
        self.placing = Placing { whole: matrix, plain, scale: Vec3::from(axes.map(Vec3::length)) };
        self.placed = true;
    }

    /// Puts it by a matrix taken as it is (0x127914).
    pub fn place_as_is(&mut self, matrix: Mat4) {
        self.placing = Placing { whole: matrix, plain: matrix, scale: self.placing.scale };
        self.placed = true;
    }

    /// Tells it to end (0x127908): its runs no longer go round, so that what
    /// goes on for ever comes to its last keys.
    pub fn end(&mut self) {
        self.ending = true;
    }

    /// Nothing is left of it (0x127C6C).
    pub fn over(&self) -> bool {
        self.over
    }

    pub fn particles(&self) -> usize {
        self.emitters.iter().flatten().map(|emitter| emitter.particles.len()).sum()
    }

    fn of(&self, index: usize) -> Option<Of<'_>> {
        Of::of(&self.script, index, self.emitters[index].as_ref().is_some_and(|emitter| emitter.kept_to))
    }

    /// Makes the emitters of all the entries (0x128234), and sets the order they
    /// are drawn in: by their entries' places in it, the lower first (0x128768).
    fn make(&mut self) {
        let entries = &self.script.entries;
        self.emitters = entries.iter().map(|entry| entry.emitter.as_ref().map(|emitter| Emitter::new(entry, emitter, self.placing))).collect();
        for entry in entries {
            if let (Some(parent), true) = (entry.parent, entry.keeps != 0) {
                if let Some(parent) = &mut self.emitters[parent] {
                    parent.kept_to = true;
                }
            }
        }
        // The game's own sort, which leaves entries of one place in an order of
        // its own.
        self.order = (0..entries.len()).filter(|&index| self.emitters[index].is_some()).collect();
        for at in 0..self.order.len().saturating_sub(1) {
            let mut least = at;
            for other in at + 1..self.order.len() {
                if entries[self.order[other]].order < entries[self.order[least]].order {
                    least = other;
                }
            }
            self.order.swap(at, least);
        }
        self.made = true;
    }

    /// A frame of it, or so much of one (0x127BA0). An effect has its first
    /// frame, a whole one, as it is made (0xA5D48).
    pub fn step(&mut self, pool: &mut Pool, camera: &Camera, by: f32) {
        if self.over {
            return;
        }
        if !self.made {
            self.make();
        }
        if self.emitters.iter().all(Option::is_none) {
            self.over = true;
            return;
        }
        let frame = Frame {
            camera,
            placing: self.placing,
            sized: !self.placed,
            tint: (self.colour != [1.0; 4]).then_some(self.colour),
            ending: self.ending,
            by: by * self.speed,
        };
        for index in 0..self.emitters.len() {
            if self.emitters[index].is_none() {
                continue;
            }
            self.step_emitter(index, &frame, pool);
            if self.emitters[index].as_ref().is_some_and(|emitter| emitter.finished) {
                self.remove(index, pool);
            }
        }
    }

    /// A frame of an emitter (0x11C57C).
    fn step_emitter(&mut self, index: usize, frame: &Frame, pool: &mut Pool) {
        let script = Arc::clone(&self.script);
        let kept_to = self.emitters[index].as_ref().is_some_and(|emitter| emitter.kept_to);
        let Some(of) = Of::of(&script, index, kept_to) else { return };
        let entries = script.entries.len();
        let parent_slot = of.entry.parent;
        let (before, after) = self.emitters.split_at_mut(index);
        let Some(emitter) = &mut after[0] else { return };
        let mut parent = parent_slot.and_then(|parent| before[parent].as_mut());
        let by = frame.by;

        if emitter.time < f32::from(of.entry.start) && !emitter.hangs {
            emitter.time += by;
            return;
        }
        let going = emitter.endless || emitter.hangs || emitter.first || emitter.time < f32::from(of.entry.end.unwrap_or(0));
        if going {
            // Its lots (0x11C20C): once its run of them has come to a last key
            // of none it lets nothing more go, and has an end like any other.
            if by > 0.0 {
                if emitter.lots.0.at_last && emitter.lots.2 == 0 {
                    emitter.endless = false;
                    emitter.hangs = false;
                } else if emitter.hangs {
                    if let (Some(parent), Some(parent_slot)) = (parent.as_deref_mut(), parent_slot) {
                        emitter.emit_from(&of, frame, pool, &mut self.ids, entries, index, parent, parent_slot);
                    } else if !std::mem::take(&mut emitter.first) {
                        emitter.step_runs(of.emitter, frame);
                    }
                } else {
                    emitter.emit(&of, frame, pool, &mut self.ids, entries, index);
                }
            }
        } else if emitter.particles.is_empty() {
            emitter.finished = true;
            emitter.time += by;
            return;
        }

        // Its particles' frames (0x11C2A0), and how each is to be drawn
        // (0x11C7D8).
        let parent = parent.as_deref();
        let its = |id: Option<u32>| parent.and_then(|parent| parent.particles.iter().find(|from| Some(from.id) == id));
        let keeps_to_one = of.entry.keeps != 0 && of.emitter.fixed_to == 6;
        for particle in &mut emitter.particles {
            let orphaned = keeps_to_one && its(particle.parent).is_none();
            particle.step(&of, frame, orphaned);
        }
        let had = emitter.particles.len();
        emitter.particles.retain(|particle| !particle.over);
        pool.particles = pool.particles.saturating_sub(had - emitter.particles.len());
        for particle in &mut emitter.particles {
            let from = its(particle.parent);
            particle.look(&of, frame, from);
        }
        emitter.time += by;
    }

    /// Takes an emitter that has finished away, unless one that hangs from it
    /// still has particles (0x12838C).
    fn remove(&mut self, index: usize, pool: &mut Pool) -> bool {
        let mut free = true;
        for other in index + 1..self.emitters.len() {
            if self.script.entries[other].parent != Some(index) {
                continue;
            }
            let Some(child) = &self.emitters[other] else { continue };
            if child.kept_to {
                self.remove(other, pool);
                free = false;
            } else if !child.particles.is_empty() {
                free = false;
            } else {
                self.emitters[other] = None;
            }
        }
        if free {
            if let Some(emitter) = self.emitters[index].take() {
                pool.particles = pool.particles.saturating_sub(emitter.particles.len());
            }
        }
        free
    }

    /// What there is to draw of it, in the order it is drawn (0x1288B4).
    pub fn drawn(&self) -> Vec<Drawn> {
        let mut out = Vec::new();
        // An effect whose own alpha is nought shows nothing.
        if self.over || self.colour[3] == 0.0 {
            return out;
        }
        let mut blend = Blend::Over;
        for (turn, &index) in self.order.iter().enumerate() {
            let (Some(emitter), Some(of)) = (&self.emitters[index], self.of(index)) else { continue };
            if emitter.particles.is_empty() {
                continue;
            }
            // A blend the game sets nothing for leaves what was set last
            // (0x11C770).
            if of.emitter.blend != Blend::AsBefore {
                blend = of.emitter.blend;
            }
            for particle in emitter.particles.iter().filter(|particle| particle.shown) {
                let (body, matrix) = match (of.emitter.kind, particle.model) {
                    (Kind::Flat, _) => (Body::Flat { texture: usize::try_from(particle.of.1.first).ok() }, particle.drawn),
                    // The model has the particle's size (0x1226AC).
                    (Kind::Model, Some(model)) => (Body::Model { model: model.index, frames: model.frames }, particle.drawn * Mat4::from_scale(particle.size.1)),
                    (Kind::Model, None) => continue,
                };
                out.push(Drawn { body, matrix, colour: particle.drawn_colour, blend, screen: of.screen, turn });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use brs_formats::esb::{Drift, Entry, Key, Lot, Of as Which, Opening, Script, Way};

    use super::*;

    fn run<T>(keys: Vec<(u16, Option<u8>, T)>) -> Run<T> {
        Run { keys: keys.into_iter().map(|(frame, back_to, value)| Key { frame, back_to, value }).collect() }
    }

    fn one<T>(value: T) -> Run<T> {
        run(vec![(0, None, value)])
    }

    /// An emitter of one plain square a frame, let go at its place and sent
    /// nowhere, that lives ten frames.
    fn emitter() -> esb::Emitter {
        esb::Emitter {
            life: 10,
            pace_spread: 0,
            kind: Kind::Flat,
            place: one([0.0; 3]),
            fixed_to: 0,
            opening: one(Opening { size: [0.0; 3], turn: [0.0; 3] }),
            shape: 2,
            way: one(Way { direction: [0.0, 0.0, 1.0], tip: 0.0, more_tip: 0.0 }),
            way_space: 0,
            lots: one(Lot { count: 1, every: 1.0 }),
            of: one(Which { first: 0, second: -1 }),
            size: one([1.0; 3]),
            size_by: 1.0,
            size_spread: [0.0, -1.0, -1.0],
            speed: one(0.0),
            speed_spread: 0.0,
            colour: one([255; 4]),
            blend: Blend::Added,
            colour_spread: [0.0; 4],
            turn: one([0.0; 3]),
            faces: 4,
            turn_random: false,
            turn_spread: [0.0; 3],
            drift: None,
            off: None,
            off_by_size: false,
        }
    }

    fn entry(start: u16, end: Option<u16>, emitter: esb::Emitter) -> Entry {
        Entry { start, end, parent: None, keeps: 0, order: 0, held: Held::Now, emitter: Some(emitter) }
    }

    fn play(entries: Vec<Entry>) -> (Effect, Pool, Camera) {
        (Effect::new(Arc::new(Script { entries })), Pool::default(), Camera::default())
    }

    fn places(effect: &Effect) -> Vec<Vec3> {
        effect.drawn().iter().map(|drawn| drawn.matrix.w_axis.truncate()).collect()
    }

    #[test]
    fn the_random_numbers_are_the_twisters() {
        // The first of the Mersenne Twister from its usual seed.
        let mut random = Random::new(Random::SEED);
        assert_eq!([random.next(), random.next(), random.next()], [3_499_211_612, 581_869_302, 3_890_346_734]);
        // And the 10000th, past several turns of its state.
        let mut random = Random::new(Random::SEED);
        assert_eq!((0..10_000).map(|_| random.next()).last(), Some(4_123_659_995));
        // The ways they are read keep to their bounds.
        for _ in 0..1000 {
            assert!((0.0..=1.0).contains(&random.unit()));
            assert!((-1.0..=1.0).contains(&random.either_way()));
            assert!((-0.5..=0.5).contains(&random.half()));
            assert_eq!(random.sign().abs(), 1.0);
        }
    }

    #[test]
    fn a_run_is_mixed_between_its_keys_and_holds_at_its_last() {
        let keys = run(vec![(0, None, 10.0f32), (4, None, 30.0), (6, None, 0.0)]);
        let mut cursor = Cursor::start(&keys);
        let mut value = keys.keys[0].value;
        let mut seen = Vec::new();
        for _ in 0..9 {
            if let Some((from, to, part)) = cursor.between(&keys, 1.0, false) {
                value = mix(from, to, part);
            }
            seen.push(value);
        }
        // A quarter of the way a frame to the second key, half to the third,
        // and then it stays.
        assert_eq!(seen, [15.0, 20.0, 25.0, 30.0, 15.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(cursor.at_last);
        // A run of one key is never stepped.
        let alone = one(5.0f32);
        let mut cursor = Cursor::start(&alone);
        assert_eq!(cursor.between(&alone, 1.0, false), None);
    }

    #[test]
    fn a_run_goes_round_until_the_effect_is_ending() {
        // The key at frame 2 goes back to the first; the one after it is what
        // the run goes on to once the effect is told to end.
        let keys = run(vec![(0, None, 0.0f32), (2, Some(0), 2.0), (4, None, 10.0)]);
        let mut cursor = Cursor::start(&keys);
        let mut value = 0.0;
        let mut seen = Vec::new();
        for frame in 0..12 {
            if let Some((from, to, part)) = cursor.between(&keys, 1.0, frame >= 6) {
                value = mix(from, to, part);
            }
            seen.push(value);
        }
        assert_eq!(seen, [1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 6.0, 10.0, 10.0, 10.0, 10.0, 10.0]);
    }

    #[test]
    fn an_emitter_lets_a_particle_go_each_frame_between_its_start_and_end() {
        let mut flying = emitter();
        flying.speed = one(2.0);
        flying.place = one([1.0, 0.0, 0.0]);
        let (mut effect, mut pool, camera) = play(vec![entry(2, Some(4), flying)]);
        let mut counts = Vec::new();
        for _ in 0..16 {
            effect.step(&mut pool, &camera, 1.0);
            counts.push(effect.particles());
        }
        // Nothing for its first two frames, one at each of the two after, and
        // each for the ten frames it lives; then the effect is over.
        assert_eq!(counts, [0, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0]);
        assert!(effect.over());
        assert_eq!(pool.particles, 0);

        // Each has gone two a frame along its way from the frame it was let go.
        let (mut effect, mut pool, camera) = play(vec![entry(2, Some(4), {
            let mut flying = emitter();
            flying.speed = one(2.0);
            flying.place = one([1.0, 0.0, 0.0]);
            flying
        })]);
        for _ in 0..5 {
            effect.step(&mut pool, &camera, 1.0);
        }
        assert_eq!(places(&effect), [Vec3::new(1.0, 0.0, 6.0), Vec3::new(1.0, 0.0, 4.0)]);
    }

    #[test]
    fn a_particle_has_its_size_colour_and_turn_over_its_life() {
        let mut growing = emitter();
        growing.lots = run(vec![(0, None, Lot { count: 1, every: 1.0 }), (1, None, Lot { count: 0, every: 1.0 })]);
        growing.size = run(vec![(0, None, [1.0, 1.0, 1.0]), (4, None, [5.0, 3.0, 1.0])]);
        growing.size_by = 2.0;
        growing.colour = run(vec![(0, None, [200, 100, 50, 255]), (4, None, [0, 100, 250, 55])]);
        growing.turn = run(vec![(0, None, [0.0; 3]), (4, None, [0.0, 0.0, PI])]);
        let (mut effect, mut pool, camera) = play(vec![entry(0, None, growing)]);
        effect.colour = [1.0, 0.5, 1.0, 1.0];
        for _ in 0..2 {
            effect.step(&mut pool, &camera, 1.0);
        }
        let drawn = effect.drawn();
        assert_eq!(drawn.len(), 1);
        // Two frames of the four: half the way from each first key to the
        // second; the effect's own colour halves the green.
        assert_eq!(drawn[0].colour, [100, 50, 150, 155]);
        assert_eq!((drawn[0].blend, drawn[0].screen, drawn[0].body), (Blend::Added, false, Body::Flat { texture: Some(0) }));
        let (scale, turn, _) = drawn[0].matrix.to_scale_rotation_translation();
        assert!(scale.abs_diff_eq(Vec3::new(6.0, 4.0, 2.0), 1e-4), "{scale}");
        assert!(turn.abs_diff_eq(bevy::math::Quat::from_rotation_z(FRAC_PI_2), 1e-5), "{turn}");
    }

    #[test]
    fn an_effect_is_put_by_a_matrix_and_what_faces_the_camera_keeps_facing_it() {
        let matrix = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)) * Mat4::from_rotation_y(FRAC_PI_2) * Mat4::from_scale(Vec3::splat(2.0));
        let camera = Camera::of(Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0)) * Mat4::from_rotation_y(0.3));
        assert!((camera.heading - 0.3).abs() < 1e-6);
        let at = |faces: u16| {
            let mut still = emitter();
            still.place = one([0.0, 0.0, 3.0]);
            still.faces = faces;
            let (mut effect, mut pool, _) = play(vec![entry(0, None, still)]);
            effect.place(matrix);
            effect.step(&mut pool, &camera, 1.0);
            effect.drawn()[0].matrix
        };
        // The place is the effect's matrix's whatever it faces: three before it,
        // turned a quarter round and doubled.
        for faces in [0, 2, 4, 5] {
            assert!(at(faces).w_axis.truncate().abs_diff_eq(Vec3::new(16.0, 0.0, 0.0), 1e-4), "{faces}");
        }
        // What faces the camera has the camera's turn and its own size; what
        // faces nothing has the effect's turn and its sizes.
        assert!(Mat3::from_mat4(at(0)).abs_diff_eq(Mat3::from_mat4(camera.turn), 1e-5));
        assert!(Mat3::from_mat4(at(2)).abs_diff_eq(Mat3::from_rotation_y(0.3), 1e-5));
        assert!(Mat3::from_mat4(at(4)).abs_diff_eq(Mat3::from_mat4(matrix), 1e-5));
        // One laid flat has its face up.
        assert!(at(5).transform_vector3(Vec3::Z).abs_diff_eq(Vec3::new(0.0, 2.0, 0.0), 1e-4));
    }

    #[test]
    fn particles_come_out_of_the_shape_and_go_within_the_tip() {
        let mut spray = emitter();
        spray.opening = one(Opening { size: [4.0, 2.0, 6.0], turn: [0.0; 3] });
        spray.lots = one(Lot { count: 50, every: 1.0 });
        spray.way = one(Way { direction: [0.0, 1.0, 0.0], tip: 10.0, more_tip: 20.0 });
        spray.speed = one(1.0);
        let (mut effect, mut pool, camera) = play(vec![entry(0, Some(1), spray)]);
        effect.step(&mut pool, &camera, 1.0);
        let emitter = effect.emitters[0].as_ref().unwrap();
        assert_eq!(emitter.particles.len(), 50);
        for particle in &emitter.particles {
            // In a box of the sizes about the place, and sent off the upright by
            // ten to thirty degrees.
            let from = particle.before;
            assert!(from.x.abs() <= 2.0 && from.y.abs() <= 1.0 && from.z.abs() <= 3.0, "{from}");
            let tip = particle.direction.angle_between(Vec3::Y).to_degrees();
            assert!((9.99..=30.01).contains(&tip), "{tip}");
            assert!((particle.direction.length() - 1.0).abs() < 1e-4);
        }
        // To every side.
        let sides: Vec<bool> = emitter.particles.iter().map(|particle| particle.direction.x > 0.0).collect();
        assert!(sides.contains(&true) && sides.contains(&false));

        // A ring is on the flat at its width, a ball's skin at its radius.
        for (shape, check) in [(3, (|at: Vec3| at.y == 0.0 && (at.length() - 4.0).abs() < 1e-3) as fn(Vec3) -> bool), (1, |at: Vec3| (at.length() - 4.0).abs() < 1e-3), (4, |at: Vec3| (0.0..=2.0).contains(&at.y) && Vec3::new(at.x, 0.0, at.z).length() <= 4.0)] {
            let mut shaped = self::emitter();
            shaped.opening = one(Opening { size: [4.0, 2.0, 6.0], turn: [0.0; 3] });
            shaped.shape = shape;
            shaped.lots = one(Lot { count: 20, every: 1.0 });
            let (mut effect, mut pool, camera) = play(vec![entry(0, Some(1), shaped)]);
            effect.step(&mut pool, &camera, 1.0);
            assert!(effect.emitters[0].as_ref().unwrap().particles.iter().all(|particle| check(particle.place)), "shape {shape}");
        }
    }

    #[test]
    fn all_the_effects_have_no_more_than_their_particles_between_them() {
        let mut many = emitter();
        many.lots = one(Lot { count: 100, every: 1.0 });
        let (mut effect, mut pool, camera) = play(vec![entry(0, None, many)]);
        effect.step(&mut pool, &camera, 1.0);
        effect.step(&mut pool, &camera, 1.0);
        assert_eq!((effect.particles(), pool.particles), (MOST_PARTICLES, MOST_PARTICLES));
        // Another effect gets none while they are all taken.
        let (mut other, _, _) = play(vec![entry(0, None, emitter())]);
        other.step(&mut pool, &camera, 1.0);
        assert_eq!(other.particles(), 0);
    }

    #[test]
    fn an_emitter_that_hangs_from_another_lets_go_from_each_of_its_particles() {
        // One carrier, going one a frame along z; from it a spark every two
        // frames once it is two frames old, each living three frames.
        let mut carrier = emitter();
        carrier.speed = one(1.0);
        let mut spark = emitter();
        spark.fixed_to = 6;
        spark.life = 3;
        spark.lots = one(Lot { count: 1, every: 2.0 });
        let mut hanging = entry(2, None, spark);
        hanging.parent = Some(0);
        let (mut effect, mut pool, camera) = play(vec![entry(0, Some(1), carrier), hanging]);
        let mut sparks = Vec::new();
        for _ in 0..6 {
            effect.step(&mut pool, &camera, 1.0);
            sparks.push(effect.emitters[1].as_ref().map_or(0, |emitter| emitter.particles.len()));
        }
        // The carrier's age is taken as of the frame's beginning: it is two
        // frames old on its third frame.
        assert_eq!(effect.emitters[0].as_ref().unwrap().particles.len(), 1);
        assert_eq!(sparks, [0, 0, 1, 1, 2, 1]);
        // A spark is let go where its carrier was last drawn, and stays there.
        let last = &effect.emitters[1].as_ref().unwrap().particles[0];
        assert_eq!((last.parent, last.place), (Some(1), Vec3::new(0.0, 0.0, 5.0)));
    }

    #[test]
    fn particles_that_keep_to_one_go_with_it_and_end_with_it() {
        let mut carrier = emitter();
        carrier.speed = one(1.0);
        carrier.life = 4;
        carrier.lots = run(vec![(0, None, Lot { count: 1, every: 1.0 }), (1, None, Lot { count: 0, every: 1.0 })]);
        let mut rider = emitter();
        rider.fixed_to = 6;
        rider.life = 0;
        rider.place = one([0.0, 2.0, 0.0]);
        rider.lots = run(vec![(0, None, Lot { count: 1, every: 100.0 }), (1, None, Lot { count: 0, every: 100.0 })]);
        let mut hanging = entry(0, None, rider);
        (hanging.parent, hanging.keeps) = (Some(0), 1);
        let (mut effect, mut pool, camera) = play(vec![entry(0, Some(1), carrier), hanging]);
        let mut seen = Vec::new();
        for _ in 0..8 {
            effect.step(&mut pool, &camera, 1.0);
            seen.push(places(&effect));
        }
        // The rider is let go on the carrier's second frame, two above it, and
        // is drawn there as the carrier goes; when the carrier ends so does it.
        assert_eq!(seen[1], [Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 2.0, 2.0)]);
        assert_eq!(seen[3], [Vec3::new(0.0, 0.0, 4.0), Vec3::new(0.0, 2.0, 4.0)]);
        assert!(seen[6].is_empty() && effect.over());
    }

    #[test]
    fn an_effect_that_goes_round_ends_once_it_is_told_to() {
        // Lots that go round for ever, with a key of none after the one that
        // goes back; particles that never end of themselves, whose colour goes
        // round too, with a key of nothing after.
        let mut endless = emitter();
        endless.life = 0;
        endless.lots = run(vec![(0, None, Lot { count: 1, every: 4.0 }), (4, Some(0), Lot { count: 1, every: 4.0 }), (5, None, Lot { count: 0, every: 4.0 })]);
        endless.colour = run(vec![(0, None, [255; 4]), (2, Some(0), [255; 4]), (4, None, [255, 255, 255, 0])]);
        let (mut effect, mut pool, camera) = play(vec![entry(0, None, endless)]);
        for _ in 0..40 {
            effect.step(&mut pool, &camera, 1.0);
        }
        assert_eq!((effect.over(), effect.particles()), (false, 10));
        effect.end();
        let mut left = Vec::new();
        for _ in 0..12 {
            effect.step(&mut pool, &camera, 1.0);
            left.push(effect.particles());
        }
        assert_eq!(left.last(), Some(&0), "{left:?}");
        assert!(effect.over());
    }

    #[test]
    fn emitters_are_drawn_in_the_order_of_their_entries_places() {
        let mut entries: Vec<Entry> = [3i8, 1, 2, 1].into_iter().enumerate().map(|(index, order)| {
            let mut placed = emitter();
            placed.place = one([index as f32, 0.0, 0.0]);
            let mut entry = entry(0, None, placed);
            entry.order = order;
            entry
        }).collect();
        // One whose blend the game sets nothing for is drawn as the one before
        // it was.
        entries[2].emitter.as_mut().unwrap().blend = Blend::AsBefore;
        entries[3].emitter.as_mut().unwrap().blend = Blend::TakenAway;
        let (mut effect, mut pool, camera) = play(entries);
        effect.step(&mut pool, &camera, 1.0);
        let drawn: Vec<(f32, Blend, usize)> = effect.drawn().iter().map(|drawn| (drawn.matrix.w_axis.x, drawn.blend, drawn.turn)).collect();
        assert_eq!(drawn, [(1.0, Blend::Added, 0), (3.0, Blend::TakenAway, 1), (2.0, Blend::TakenAway, 2), (0.0, Blend::Added, 3)]);
    }

    #[test]
    fn a_model_particle_plays_its_models_animation_and_has_its_size() {
        let mut shown = emitter();
        shown.kind = Kind::Model;
        shown.of = one(Which { first: 3, second: -1 });
        shown.size = one([2.0, 2.0, 2.0]);
        shown.lots = run(vec![(0, None, Lot { count: 1, every: 1.0 }), (1, None, Lot { count: 0, every: 1.0 })]);
        shown.drift = Some(one(Drift { along: [0.0, 1.0, 0.0], speed: 0.5 }));
        let (mut effect, mut pool, camera) = play(vec![entry(0, None, shown)]);
        for _ in 0..3 {
            effect.step(&mut pool, &camera, 1.0);
        }
        let drawn = effect.drawn();
        assert_eq!(drawn[0].body, Body::Model { model: 3, frames: 6.0 });
        let (scale, _, place) = drawn[0].matrix.to_scale_rotation_translation();
        assert!(scale.abs_diff_eq(Vec3::splat(2.0), 1e-5));
        // It has drifted half a unit up a frame.
        assert_eq!(place, Vec3::new(0.0, 1.5, 0.0));
    }

    #[test]
    fn what_is_fixed_to_the_screen_is_placed_from_its_middle() {
        let mut flash = emitter();
        flash.fixed_to = 11;
        flash.place = one([-40.0, 36.0, 5.0]);
        flash.size = one([480.0, 272.0, 1.0]);
        flash.faces = 0;
        let (mut effect, mut pool, _) = play(vec![entry(0, None, flash)]);
        effect.place(Mat4::from_translation(Vec3::new(100.0, 100.0, 100.0)));
        effect.step(&mut pool, &Camera::of(Mat4::from_rotation_y(1.0)), 1.0);
        let drawn = effect.drawn();
        // In the screen's pixels whatever the effect's matrix and the camera
        // are, flat on it, and facing nothing.
        assert!(drawn[0].screen);
        assert_eq!(drawn[0].matrix, Mat4::from_translation(Vec3::new(200.0, 172.0, 0.0)) * Mat4::from_scale(Vec3::new(480.0, 272.0, 1.0)));
    }
}
