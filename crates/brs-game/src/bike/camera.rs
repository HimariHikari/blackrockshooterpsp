//! The camera of the ride on the bike (0x135C30 to 0x137EB8): it goes with one
//! character, from an eye and to a mark that are each a set distance from where
//! that character is; the camera's script sets the distances, the lens and the
//! roll for the lane she is in and what she is doing, and each comes to what it
//! is set to over a time of its own. The eye and the mark then follow where they
//! should be by a part of the way each frame, a part for each of the world's
//! three ways. See `docs/BIKE_NOTES.md`, "The camera".

use bevy::math::{Quat, Vec2, Vec3};

/// How something comes from what it was to what it is set to (0x136A0C on): 0
/// evenly (0x14896C), 1 slowly at both ends and 2 quickly at first (0x14897C
/// with -100 and 100); anything else leaves it where it is.
fn eased(from: f32, to: f32, part: f32, how: i32) -> Option<f32> {
    let wave = (part * std::f32::consts::PI).sin() / std::f32::consts::PI;
    let part = match how {
        0 => part,
        1 => part - wave,
        2 => part + wave,
        _ => return None,
    };
    Some(from + (to - from) * part)
}

/// A number that comes to what it is set to over a time.
#[derive(Clone, Copy, Debug, Default)]
struct Coming {
    now: f32,
    from: f32,
    to: f32,
    time: f32,
    length: f32,
    how: i32,
}

impl Coming {
    fn part(&self) -> f32 {
        if self.length <= 0.0 {
            1.0
        } else {
            (self.time / self.length).clamp(0.0, 1.0)
        }
    }

    /// Begins from where it is now.
    fn begin(&mut self) {
        (self.from, self.time) = (self.now, 0.0);
    }

    fn step(&mut self, time: f32) {
        if self.time < self.length {
            self.time = (self.time + time).min(self.length);
        }
    }

    fn settle(&mut self) {
        if let Some(now) = eased(self.from, self.to, self.part(), self.how) {
            self.now = now;
        }
    }
}

/// Three numbers that do, all in the one time.
#[derive(Clone, Copy, Debug, Default)]
struct ComingPlace {
    now: Vec3,
    from: Vec3,
    to: Vec3,
    /// What the script has set, which becomes `to` when it says so.
    set: Vec3,
    time: f32,
    length: f32,
    how: i32,
}

impl ComingPlace {
    fn begin(&mut self) {
        (self.from, self.to, self.time) = (self.now, self.set, 0.0);
    }

    fn step(&mut self, time: f32) {
        if self.time < self.length {
            self.time = (self.time + time).min(self.length);
        }
    }

    fn settle(&mut self) {
        let part = if self.length <= 0.0 { 1.0 } else { (self.time / self.length).clamp(0.0, 1.0) };
        for axis in 0..3 {
            if let Some(now) = eased(self.from[axis], self.to[axis], part, self.how) {
                self.now[axis] = now;
            }
        }
    }
}

/// A shake of the picture (0x24 bytes): up and down by turns, a frame each,
/// along a line that leans, dying away towards its end.
#[derive(Clone, Copy, Debug, Default)]
struct Shake {
    on: bool,
    /// How long it lasts, in the interface's frames, and how far in it is.
    length: i32,
    gone: i32,
    /// The part of its length after which it dies away.
    full_for: f32,
    /// How its line leans, in degrees, and by how much more this time.
    leans: i32,
    leans_more: i32,
    size: f32,
    /// How many frames it has had.
    frames: i32,
    /// How far it has the picture off now.
    off: f32,
}

/// The camera (0x480 bytes at the game's + 0x10830).
#[derive(Clone, Debug)]
pub struct Camera {
    /// The character it goes with (or less than 0 for none), and the bone of
    /// which of its models, if any.
    pub follows: i32,
    follows_model: i32,
    follows_bone: i32,
    /// Three more of the script's words that nothing reads.
    words: [i32; 3],
    /// Where it looks from, at what, and which way is up.
    pub eye: Vec3,
    pub mark: Vec3,
    pub up: Vec3,
    /// Where the eye and the mark should be.
    wanted_eye: Vec3,
    wanted_mark: Vec3,
    /// Where the one it goes with is.
    anchor: Vec3,
    /// The eye and the mark, from there.
    eye_from_it: ComingPlace,
    mark_from_it: ComingPlace,
    /// The lens, in degrees, and the roll.
    lens: Coming,
    roll: Coming,
    /// The part of the way the eye and the mark come each frame, for each of
    /// the game's paces, across, up and along (the six threes at + 0xB0).
    eye_follows: [Vec3; 3],
    mark_follows: [Vec3; 3],
    /// The lean a change of lanes gives it: how far, how long, how far in.
    lean: f32,
    lean_length: f32,
    lean_time: f32,
    leaning: bool,
    /// The shake the script has set up, and the two that go on.
    set_shake: Shake,
    leans_within: i32,
    which_shake: usize,
    shakes: [Shake; 2],
    /// How far the shakes have the picture off, across and up.
    pub shaken: Vec2,
    /// It does nothing (flag 8).
    pub stopped: bool,
    flag: bool,
    /// It rides along on its own model's motion: where it is comes from that
    /// (`ride_along`), not from what its script has set.
    pub riding_along: bool,
}

impl Default for Camera {
    fn default() -> Self {
        // What each follows by at the paces nought, one and the slow one (0x135C3C).
        let follows = [Vec3::ZERO, Vec3::new(0.3, 1.0, 0.9), Vec3::new(0.03, 1.0, 0.3)];
        Camera {
            follows: 0,
            follows_model: -1,
            follows_bone: -1,
            words: [-1; 3],
            eye: Vec3::ZERO,
            mark: Vec3::ZERO,
            up: Vec3::ZERO,
            wanted_eye: Vec3::ZERO,
            wanted_mark: Vec3::ZERO,
            anchor: Vec3::ZERO,
            eye_from_it: ComingPlace::default(),
            mark_from_it: ComingPlace::default(),
            lens: Coming::default(),
            roll: Coming { length: 1.0, ..Coming::default() },
            eye_follows: follows,
            mark_follows: follows,
            lean: 11.0,
            lean_length: 0.5,
            lean_time: 0.0,
            leaning: false,
            set_shake: Shake::default(),
            leans_within: 0,
            which_shake: 0,
            shakes: [Shake::default(); 2],
            shaken: Vec2::ZERO,
            stopped: false,
            flag: false,
            riding_along: false,
        }
    }
}

impl Camera {
    /// The lens: how much it sees, up and down, in degrees.
    pub fn lens(&self) -> f32 {
        self.lens.now
    }

    /// Sets one of its words (0x137328).
    pub fn set_word(&mut self, which: usize, value: i32) {
        match which {
            0 => self.set_shake.length = value.max(0),
            1 => self.set_shake.leans = value,
            2 => self.leans_within = value.max(0),
            3 => self.which_shake = value.clamp(0, 1) as usize,
            4 => self.eye_from_it.how = value,
            5 => self.mark_from_it.how = value,
            6 => self.lens.how = value,
            7 => self.roll.how = value,
            8 => self.follows = value,
            9 => self.follows_model = value.clamp(0, 1),
            10 => self.follows_bone = value,
            11..=13 => self.words[which - 11] = value,
            14 => self.flag = value != 0,
            _ => {}
        }
    }

    /// Sets one of its numbers (0x137454).
    pub fn set_number(&mut self, which: usize, value: f32) {
        // The parts of the way followed are parts of one.
        let part = value.clamp(0.0, 1.0);
        match which {
            0..=2 => self.eye_from_it.set[which] = value,
            3..=5 => self.mark_from_it.set[which - 3] = value,
            6 => self.shaken.x = value,
            7 => self.shaken.y = value,
            8 => self.set_shake.full_for = part,
            9 => self.set_shake.size = value,
            0xa => self.eye_follows[1].x = part,
            0xb => self.eye_follows[2].x = part,
            0xc => self.eye_follows[1].z = part,
            0xd => self.eye_follows[2].z = part,
            0xe => self.eye_follows[1].y = part,
            0xf => self.eye_follows[2].y = part,
            0x10 => self.mark_follows[1].x = part,
            0x11 => self.mark_follows[2].x = part,
            0x12 => self.mark_follows[1].z = part,
            0x13 => self.mark_follows[2].z = part,
            0x14 => self.mark_follows[1].y = part,
            0x15 => self.mark_follows[2].y = part,
            0x16 => self.eye_from_it.length = value,
            0x17 => self.mark_from_it.length = value,
            0x18 => self.lens.to = value,
            0x19 => self.lens.length = value,
            // Three numbers that nothing reads.
            0x1a..=0x1c => {}
            0x1d => self.roll.to = value,
            0x1e => self.roll.length = value,
            0x1f => self.lean = value,
            _ => {}
        }
    }

    /// Has it do one of the things the script asks for (0x1375D8, 1 to 7): what
    /// has been set begins to come, or is there at once. A shake's line leans
    /// by a chance amount more: `chance` answers a number under what it is given.
    pub fn apply(&mut self, what: i32, mut chance: impl FnMut(i32) -> i32) {
        match what {
            1 => {
                self.eye_from_it.begin();
                self.mark_from_it.begin();
            }
            2 => {
                let slot = &mut self.shakes[self.which_shake];
                // One that is no smaller is not broken in on.
                if slot.on && self.set_shake.size < slot.off {
                    return;
                }
                let leans_more = (chance(self.leans_within) as f32 - self.leans_within as f32 * 0.5) as i32;
                *slot = Shake { on: true, gone: 0, frames: 0, leans_more, off: self.set_shake.size, ..self.set_shake };
            }
            3 => self.shakes[self.which_shake].on = false,
            4 => self.roll.begin(),
            5 => (self.lean_time, self.leaning) = (0.0, true),
            6 => {
                for place in [&mut self.eye_from_it, &mut self.mark_from_it] {
                    (place.now, place.time) = (place.to, place.length);
                }
                (self.wanted_mark, self.wanted_eye) = (self.anchor + self.mark_from_it.now, self.anchor + self.eye_from_it.now);
                (self.eye, self.mark) = (self.wanted_eye, self.wanted_mark);
                for number in [&mut self.lens, &mut self.roll] {
                    (number.now, number.time) = (number.to, number.length);
                }
            }
            7 => self.lens.begin(),
            _ => {}
        }
    }

    /// Where the one it goes with is.
    pub fn anchor(&self) -> Vec3 {
        self.anchor
    }

    /// Has it be where its own model's motion has it (0x1365C8 to 0x136988):
    /// what its script has set is made the same, so that it goes on from there
    /// when the motion is taken away.
    pub fn ride_along(&mut self, eye: Vec3, mark: Vec3, up: Vec3, lens: f32) {
        (self.eye, self.mark, self.up) = (eye, mark, up);
        for (place, at) in [(&mut self.eye_from_it, eye - self.anchor), (&mut self.mark_from_it, mark - self.anchor)] {
            (place.now, place.from, place.to, place.set) = (at, at, at, at);
        }
        (self.roll.now, self.roll.from, self.roll.to) = (0.0, 0.0, 0.0);
        (self.lens.now, self.lens.from, self.lens.to) = (lens, lens, lens);
    }

    /// Moves it along the road with everything else (0x135F24).
    pub fn shift(&mut self, by: f32) {
        for place in [&mut self.eye, &mut self.mark, &mut self.wanted_eye, &mut self.wanted_mark, &mut self.anchor] {
            place.z += by;
        }
    }

    /// A frame of it (0x136054, after its script's): `followed` is where the
    /// one it goes with is, if there is such a one; `pace` which of the game's
    /// paces time goes at, and `steps` the interface's frames in this one.
    pub fn step(&mut self, time: f32, pace: usize, steps: i32, followed: Option<Vec3>) {
        if self.stopped {
            return;
        }
        self.anchor = followed.unwrap_or(Vec3::ZERO);

        // The shakes: up and down by turns, along a line that leans.
        self.shaken = Vec2::ZERO;
        for shake in self.shakes.iter_mut().filter(|shake| shake.on) {
            shake.gone += steps;
            shake.frames += 1;
            if shake.gone > shake.length {
                (shake.gone, shake.on) = (shake.length, false);
            }
            let full_for = shake.length as f32 * shake.full_for;
            let strength = if shake.gone as f32 <= full_for {
                1.0
            } else {
                let dying = (shake.length as f32 - full_for).trunc();
                if dying == 0.0 {
                    0.0
                } else {
                    1.0 - (shake.gone as f32 - full_for).trunc() / dying
                }
            };
            let high = (shake.frames as f32 * std::f32::consts::PI).cos() * strength * shake.size;
            let off = Vec2::from_angle(((shake.leans + shake.leans_more) as f32).to_radians()).rotate(Vec2::new(0.0, high));
            shake.off = off.length();
            self.shaken += off;
        }

        self.eye_from_it.step(time);
        self.mark_from_it.step(time);
        self.roll.step(time);
        self.lens.step(time);
        if self.leaning {
            if self.lean_length <= 0.0 {
                self.leaning = false;
            } else {
                self.lean_time += time;
                if self.lean_time > self.lean_length {
                    (self.lean_time, self.leaning) = (0.0, false);
                }
            }
        }

        if self.riding_along {
            return;
        }
        self.eye_from_it.settle();
        self.mark_from_it.settle();
        self.roll.settle();
        // Up is the world's, rolled about the way it looks.
        let looks = (self.mark_from_it.now - self.eye_from_it.now).normalize_or_zero();
        let mut up = Quat::from_axis_angle(looks, self.roll.now.to_radians()) * Vec3::Y;
        self.lens.settle();
        if self.leaning {
            let part = (self.lean_time / self.lean_length).clamp(0.0, 1.0);
            let quick = part * 3.0;
            let lean = self.lean * 2.0 * (quick / (quick * quick + 1.0)) * (1.0 - part);
            up = Quat::from_axis_angle(looks, lean.to_radians()) * up;
        }

        self.wanted_eye = self.anchor + self.eye_from_it.now;
        self.wanted_mark = self.anchor + self.mark_from_it.now;
        // Each comes a part of the way to where it should be, a part for each
        // of the world's ways.
        let pace = pace.min(2);
        self.mark += (self.wanted_mark - self.mark) * self.mark_follows[pace];
        self.eye += (self.wanted_eye - self.eye) * self.eye_follows[pace];
        self.up = up;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As the camera's script sets it up for the middle lane (`val_lst` 0 and
    /// `ip_lst` 0).
    fn set_up() -> Camera {
        let mut camera = Camera::default();
        for (which, value) in [-55.0, 114.0, -250.0, -35.0, 107.0, 60.0].into_iter().enumerate() {
            camera.set_number(which, value);
        }
        camera.set_number(0x18, 30.0);
        camera.set_number(0x1d, -3.0);
        for (which, value) in [(0x16, 0.4), (0x17, 0.4), (0x19, 0.4), (0x1e, 0.4), (0xa, 0.3), (0xe, 0.3), (0xc, 0.9), (0x10, 0.5), (0x14, 0.5), (0x12, 0.9)] {
            camera.set_number(which, value);
        }
        for what in [1, 4, 7] {
            camera.apply(what, |_| 0);
        }
        camera
    }

    #[test]
    fn it_comes_to_what_is_set_over_its_time_and_follows_her_by_parts() {
        let mut camera = set_up();
        let her = Vec3::new(0.0, 0.0, 20000.0);
        for _ in 0..6 {
            camera.step(1.0 / 30.0, 1, 2, Some(her));
        }
        // Half its time gone, half the lens and half the roll.
        assert!((camera.lens() - 15.0).abs() < 1e-3 && (camera.roll.now + 1.5).abs() < 1e-3, "{} {}", camera.lens(), camera.roll.now);
        for _ in 0..60 {
            camera.step(1.0 / 30.0, 1, 2, Some(her));
        }
        assert!(camera.eye.distance(her + Vec3::new(-55.0, 114.0, -250.0)) < 0.01, "{:?}", camera.eye);
        assert!(camera.mark.distance(her + Vec3::new(-35.0, 107.0, 60.0)) < 0.01, "{:?}", camera.mark);
        assert_eq!(camera.lens(), 30.0);
        // Rolled three degrees about the way it looks.
        assert!((camera.up.angle_between(Vec3::Y).to_degrees() - 3.0).abs() < 0.05, "{:?}", camera.up);
        // She goes a metre to the side at once: the eye comes three tenths of
        // the way across in a frame, the mark half.
        let beside = her + Vec3::new(100.0, 0.0, 0.0);
        camera.step(1.0 / 30.0, 1, 2, Some(beside));
        assert!((camera.eye.x - (-55.0 + 30.0)).abs() < 0.01 && (camera.mark.x - (-35.0 + 50.0)).abs() < 0.01, "{:?} {:?}", camera.eye, camera.mark);
        // Moved along the road with her, it is where it was from her.
        camera.shift(-10000.0);
        assert!((camera.eye.z - (her.z - 10000.0 - 250.0)).abs() < 0.01);
    }

    #[test]
    fn a_change_of_lanes_leans_it_and_a_shake_goes_up_and_down_and_dies_away() {
        let mut camera = set_up();
        // With no roll of its own, for the lean to be seen by itself.
        camera.set_number(0x1d, 0.0);
        camera.apply(4, |_| 0);
        for _ in 0..60 {
            camera.step(1.0 / 30.0, 1, 2, Some(Vec3::ZERO));
        }
        camera.set_number(0x1f, 6.0);
        camera.apply(5, |_| 0);
        let mut leans = Vec::new();
        for _ in 0..16 {
            camera.step(1.0 / 30.0, 1, 2, Some(Vec3::ZERO));
            leans.push(camera.up.angle_between(Vec3::Y).to_degrees());
        }
        // Most a tenth of a second in, and over in half a second.
        assert!(leans[2] > 3.0 && leans[2] > leans[6] && leans[6] > leans[12], "{leans:?}");
        assert!(leans[15].abs() < 0.05, "{leans:?}");

        // The second of the script's shakes: 15 frames, 4 high, dying from half.
        camera.set_word(0, 15);
        camera.set_number(9, 4.0);
        camera.set_number(8, 0.5);
        camera.apply(2, |_| 0);
        let mut shaken = Vec::new();
        for _ in 0..9 {
            camera.step(1.0 / 30.0, 1, 2, Some(Vec3::ZERO));
            shaken.push(camera.shaken.y);
        }
        assert_eq!(shaken[..3], [-4.0, 4.0, -4.0]);
        assert!(shaken[4].abs() < 4.0 && shaken[6].abs() < shaken[4].abs() && shaken[8] == 0.0, "{shaken:?}");
    }
}
