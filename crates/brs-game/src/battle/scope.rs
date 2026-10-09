//! The scope of a snipe (0x7F86C): while it is up the battle's camera is the one
//! that closes in (the root's camera kind 2, 0x8C844), the cursor is held where it
//! is (0xA1A5C), and the display has a sight in place of the cursor.
//!
//! The camera stays where the one over her shoulder was and looks where that
//! looked; only its lens changes, and the picture is moved so that what the
//! cursor is on comes to the middle of the screen, where the sight is.

use bevy::prelude::*;

use super::aim;

/// How high the picture is to be at what the scope is on (0x8C8C0), and the lens
/// it makes for with nothing to be on (0x8C8EC: fifteen degrees).
const HEIGHT_SEEN: f32 = 400.0;
const LENS_ON_NOTHING: f32 = 0.261_799;
/// Once the camera has closed in its lens goes on a hundredth of the way more
/// over this many sixtieths (0x8C944, 0x8C970).
const DRIFT_TIME: f32 = 120.0;
const DRIFT: f32 = 0.01;

/// The sets of the sight's layout (0x422EC, 0x42340, 0x420AC, 0x42118): it comes,
/// it goes, it locks on, it lets go.
pub mod set {
    pub const COMES: usize = 0;
    pub const GOES: usize = 1;
    pub const LOCKS_ON: usize = 2;
}

/// The sounds it comes and locks on with, of the common bank (0x42328, 0x420D4).
pub const SOUND_COMES: u32 = 0x7b;
pub const SOUND_LOCKS_ON: u32 = 0x2f;

/// The sight the display has in place of the cursor (its snipe part, 0x41E18).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sight {
    /// Which set of its layout is playing over those before, and for how many
    /// sixtieths it has.
    pub set: usize,
    pub time: f32,
    /// It has locked on: the marks about the target and the word to shoot are
    /// drawn, and the mark of a sight that has not is not (0x42260).
    pub locked: bool,
    /// The scope has gone, and it is playing its going out.
    pub going: bool,
}

#[derive(Resource, Default)]
pub struct Scope {
    /// It is up.
    pub on: bool,
    /// The sight, from when the scope comes until its going has played out.
    pub sight: Option<Sight>,
    /// Sixtieths since it came (the camera's + 0x164).
    clock: f32,
    /// How far from where the camera was as it came (0x8C788) what it is on is
    /// (0x8C7E4): the place of the lock point the skill is for, if it has one.
    distance: Option<f32>,
}

impl Scope {
    /// It comes, with the camera at `eye` and the skill's lock point at `on`.
    pub fn come(&mut self, eye: Vec3, on: Option<Vec3>) {
        let sight = Sight { set: set::COMES, time: 0.0, locked: false, going: false };
        *self = Scope { on: true, sight: Some(sight), clock: 0.0, distance: on.map(|on| on.distance(eye)) };
    }

    /// It goes (0x7F964): the camera is the usual one at once, and the sight
    /// plays its going.
    pub fn go(&mut self) {
        self.on = false;
        if let Some(sight) = &mut self.sight {
            (sight.set, sight.time, sight.going) = (set::GOES, 0.0, true);
        }
    }

    /// The sight locks on (0x42380 and 0x42098). Says whether it had not yet.
    pub fn lock(&mut self) -> bool {
        match &mut self.sight {
            Some(sight) if !sight.locked && !sight.going => {
                (sight.set, sight.time, sight.locked) = (set::LOCKS_ON, 0.0, true);
                true
            }
            _ => false,
        }
    }

    /// A frame of it (0x8C828, and the sight's own).
    pub fn step(&mut self, dt: f32) {
        if self.on {
            self.clock += dt;
        }
        if let Some(sight) = &mut self.sight {
            sight.time += dt;
        }
    }

    /// The camera's lens while it is up (0x8C844): its field of view up and down,
    /// and how far the picture is moved, in the PSP's pixels, for what is at
    /// `cursor` on the screen to come to the middle. `usual` is the lens of the
    /// camera over her shoulder and `enter` the sixtieths the closing in takes.
    pub fn lens(&self, usual: f32, enter: f32, cursor: Vec2) -> (f32, Vec2) {
        let wanted = match self.distance {
            Some(distance) if distance > 0.0 => 2.0 * (HEIGHT_SEEN * 0.5 / distance).atan(),
            _ => LENS_ON_NOTHING,
        };
        // The way in is eased; what follows it is counted from before it is over,
        // so the lens starts a hair wider than the usual one.
        let closed = (self.clock / enter).min(1.0);
        let eased = 1.0 - (1.0 - closed).powi(3);
        let drift = ((self.clock - enter) / DRIFT_TIME).min(1.0);
        let share = eased + DRIFT * drift;
        let fov = usual * (1.0 - share) + wanted * share;
        // Where the cursor's point of the picture would be with the new lens
        // (0x8BA6C, 0x8BAC8), which is how far the picture is moved, eased in the
        // same way.
        let grown = (usual * 0.5).tan() / (fov * 0.5).tan();
        (fov, (cursor - aim::SCREEN * 0.5) * grown * eased)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_closes_in_on_what_the_scope_is_on_and_brings_it_to_the_middle() {
        let usual = aim::vertical_fov();
        let mut scope = Scope::default();
        assert!(!scope.on);
        scope.come(Vec3::new(0.0, 100.0, -1700.0), Some(Vec3::new(300.0, 300.0, 200.0)));
        let cursor = Vec2::new(290.0, 96.0);
        // As it comes the lens is the usual one, all but, and nothing is moved.
        let (fov, moved) = scope.lens(usual, 30.0, cursor);
        assert!((fov - usual).abs() < 0.002 && fov > usual && moved == Vec2::ZERO, "{fov} {moved}");
        // Half a second on it shows four hundred units up and down at what it
        // is on, and that is at the middle of the screen.
        for _ in 0..15 {
            scope.step(2.0);
        }
        let (fov, moved) = scope.lens(usual, 30.0, cursor);
        let distance = Vec3::new(300.0, 200.0, 1900.0).length();
        let wanted = 2.0 * (200.0 / distance).atan();
        assert!((fov - wanted).abs() < 1e-5, "{fov} {wanted}");
        let grown = (usual * 0.5).tan() / (wanted * 0.5).tan();
        assert!(grown > 2.5 && moved.distance((cursor - Vec2::new(240.0, 136.0)) * grown) < 1e-3, "{grown} {moved}");
        // On the way there it is eased: most of it is done in the first half.
        let mut half = Scope::default();
        half.come(Vec3::new(0.0, 100.0, -1700.0), Some(Vec3::new(300.0, 300.0, 200.0)));
        for _ in 0..7 {
            half.step(2.0);
        }
        let (midway, _) = half.lens(usual, 30.0, cursor);
        assert!((usual - midway) / (usual - wanted) > 0.8);
        // From there the lens goes on a hundredth of the way over two seconds.
        for _ in 0..60 {
            scope.step(2.0);
        }
        let (later, _) = scope.lens(usual, 30.0, cursor);
        assert!((later - (usual + (wanted - usual) * 1.01)).abs() < 1e-5);
        // With nothing to be on it makes for fifteen degrees.
        let mut bare = Scope::default();
        bare.come(Vec3::ZERO, None);
        for _ in 0..15 {
            bare.step(2.0);
        }
        assert!((bare.lens(usual, 30.0, cursor).0 - 15f32.to_radians()).abs() < 1e-5);
        // Gone, its clock stands.
        scope.go();
        scope.step(2.0);
        assert!(!scope.on);
    }

    #[test]
    fn the_sight_comes_locks_on_once_and_plays_its_going() {
        let mut scope = Scope::default();
        assert_eq!(scope.sight, None);
        assert!(!scope.lock());
        scope.come(Vec3::ZERO, None);
        scope.step(2.0);
        assert_eq!(scope.sight, Some(Sight { set: set::COMES, time: 2.0, locked: false, going: false }));
        assert!(scope.lock() && !scope.lock());
        scope.step(2.0);
        assert_eq!(scope.sight, Some(Sight { set: set::LOCKS_ON, time: 2.0, locked: true, going: false }));
        // Its going starts over from nought, and it stays locked as it goes.
        scope.go();
        scope.step(2.0);
        assert_eq!(scope.sight, Some(Sight { set: set::GOES, time: 2.0, locked: true, going: true }));
        assert!(!scope.lock());
        // Coming again it is as new.
        scope.come(Vec3::ZERO, None);
        assert_eq!(scope.sight.map(|sight| (sight.set, sight.locked, sight.going)), Some((set::COMES, false, false)));
    }
}
