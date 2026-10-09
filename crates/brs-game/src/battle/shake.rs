//! The shake of a battle's camera (0x9DD44 to 0x9E3F0).
//!
//! A shake does not move the camera: it moves the whole picture by whole pixels
//! (0x8BE90 gives every draw list's camera the same offset, 0xA30C8), to and fro
//! along one line of the screen. It has a strength it keeps for a time and then
//! loses over another, and a swing that goes round once in four sixtieths, so that
//! at the battle's thirty frames a second the picture is to one side on one frame
//! and to the other on the next.
//!
//! The battle's root has the shake's frame before anybody's turn (0x8D0A4), and the
//! picture is moved as it is drawn (0x8D110): so a shake begun in a frame is seen
//! in that frame at its first place, which is the far end of its swing.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::camera::SubCameraView;
use bevy::prelude::*;

use super::actor::PcActor;
use super::aim;
use super::damage::Random;
use super::enemy::Horde;
use super::{BattleCamera, SIXTIETHS_PER_STEP};

/// The sixtieths a swing takes (0x9DF90), and where in it a shake begins: at its
/// far end (0x9E03C).
const PERIOD: f32 = 4.0;
const FIRST_PHASE: f32 = FRAC_PI_2;
/// How far either way of upright the line of a shake that is not given one can
/// lean, in degrees: one of so many (0x9E154).
const LEAN: i32 = 60;

/// A shake that is asked for, before the chance in it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// One of the four the game has by number (0x9E0B8).
    Kind(i32),
    /// That of a hit, by the word of its attack's record (0x9E278).
    Attack(i32),
    /// That of a bomb's burst (0x9E2E8).
    Burst,
    /// One an enemy's script asks for (0xBDBC8): how strong by a number of its
    /// own, the lean of its line in degrees, and the logic frames it dies away
    /// over.
    Script { kind: i32, degrees: i32, frames: i32 },
}

/// A shake: how strong, how its line leans from upright, how long it keeps its
/// strength and how long it takes to lose it, in sixtieths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jolt {
    amplitude: f32,
    angle: f32,
    hold: f32,
    fade: f32,
}

impl Jolt {
    /// The shake asked for, if it is one.
    pub fn of(asked: Asked, random: &mut Random) -> Option<Jolt> {
        let mut lean = || ((random.below(LEAN) - LEAN / 2) as f32).to_radians();
        match asked {
            // A shot of hers: three pixels up and back, in one frame.
            Asked::Kind(0) => Some(Jolt { amplitude: 3.0, angle: 0.0, hold: 2.0, fade: 0.0 }),
            Asked::Kind(1) => Some(Jolt { amplitude: 4.0, angle: lean(), hold: 0.0, fade: 8.0 }),
            Asked::Kind(2) => Some(Jolt { amplitude: 6.0, angle: lean(), hold: 0.0, fade: 8.0 }),
            Asked::Kind(3) => Some(Jolt { amplitude: 8.0, angle: lean(), hold: 0.0, fade: 8.0 }),
            Asked::Kind(_) => None,
            // The record's 1 to 3 are those kinds, and its 4 the first.
            Asked::Attack(word @ 1..=3) => Jolt::of(Asked::Kind(word), random),
            Asked::Attack(4) => Jolt::of(Asked::Kind(0), random),
            Asked::Attack(_) => None,
            Asked::Burst => Some(Jolt { amplitude: 8.0, angle: lean(), hold: 0.0, fade: 32.0 }),
            Asked::Script { kind, degrees, frames } => {
                let amplitude = match kind {
                    10 => 2.0,
                    20 => 5.0,
                    30 => 8.0,
                    _ => return None,
                };
                Some(Jolt { amplitude, angle: (degrees as f32).to_radians(), hold: 0.0, fade: frames as f32 * 2.0 })
            }
        }
    }
}

/// The shake of the battle's camera, and how far the picture is moved besides.
#[derive(Resource, Default, Debug)]
pub struct Shake {
    on: bool,
    /// Where in its swing it is, in radians.
    phase: f32,
    angle: f32,
    /// How strong it is now.
    strength: f32,
    /// The sixtieths it has gone on for.
    time: f32,
    amplitude: f32,
    hold: f32,
    fade: f32,
    /// How far it has the picture moved.
    moved: Vec2,
    /// How far the scope of a snipe has the picture moved, while it is up
    /// (0x8BEF0).
    pub scope: Option<Vec2>,
}

impl Shake {
    /// Begins a shake (0x9DFA0), unless one that is stronger now is going on.
    pub fn begin(&mut self, jolt: Jolt) {
        if jolt.amplitude <= 0.0 || (self.on && jolt.amplitude < self.strength) {
            return;
        }
        (self.amplitude, self.angle, self.hold, self.fade) = (jolt.amplitude, jolt.angle, jolt.hold, jolt.fade);
        (self.phase, self.time, self.on) = (FIRST_PHASE, 0.0, true);
        self.settle();
    }

    /// A frame of it (0x9DED8): its time and its swing go on.
    pub fn step(&mut self, dt: f32) {
        if !self.on {
            return;
        }
        self.time += dt;
        self.phase += dt * TAU / PERIOD;
        while self.phase >= TAU {
            self.phase -= TAU;
        }
        self.settle();
    }

    /// How strong it is by its time (0x9DE28), and where that has the picture
    /// (0x9DD44): along its line by the sine of where its swing is.
    fn settle(&mut self) {
        self.strength = if self.time < self.hold {
            self.amplitude
        } else if self.time < self.hold + self.fade {
            self.amplitude * (1.0 - (self.time - self.hold) / self.fade)
        } else {
            self.on = false;
            0.0
        };
        self.moved = if self.strength > 0.0 { self.strength * self.phase.sin() * Vec2::new(-self.angle.sin(), self.angle.cos()) } else { Vec2::ZERO };
    }

    /// How far the picture is moved, in the PSP's whole pixels (0x8BEC8): by the
    /// shake, and under the scope of a snipe by that too.
    pub fn picture(&self) -> Vec2 {
        let shaken = self.moved.trunc();
        self.scope.map_or(shaken, |scope| (shaken + scope).trunc())
    }
}

/// The shake's frame, before anybody's turn.
pub fn step(mut shake: ResMut<Shake>) {
    shake.step(SIXTIETHS_PER_STEP);
}

/// Once everybody has had a turn: the shakes asked for begin, and the picture is
/// moved to where the shake and the scope have it.
pub(super) fn apply(
    mut shake: ResMut<Shake>,
    mut horde: ResMut<Horde>,
    mut random: ResMut<Random>,
    mut player: Single<&mut PcActor>,
    mut lens: Single<&mut Camera, With<BattleCamera>>,
) {
    let asked: Vec<Asked> = player.jolts.drain(..).chain(horde.jolts.drain(..)).collect();
    for jolt in asked.into_iter().filter_map(|asked| Jolt::of(asked, &mut random)) {
        shake.begin(jolt);
    }
    let (moved, screen) = (shake.picture(), aim::SCREEN.as_uvec2());
    let view = (moved != Vec2::ZERO).then_some(SubCameraView { full_size: screen, offset: moved, size: screen });
    if lens.sub_camera_view != view {
        lens.sub_camera_view = view;
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    fn jolt(asked: Asked) -> Option<Jolt> {
        Jolt::of(asked, &mut Random::new(3))
    }

    #[test]
    fn a_shot_of_hers_moves_the_picture_three_pixels_for_one_frame() {
        let mut shake = Shake::default();
        shake.begin(jolt(Asked::Kind(0)).unwrap());
        // Seen on the frame it begins: upright, at the far end of its swing.
        assert_eq!(shake.picture(), Vec2::new(0.0, 3.0));
        // The frame after, its time is up.
        shake.step(2.0);
        assert_eq!(shake.picture(), Vec2::ZERO);
        assert!(!shake.on);
        // With none going on a frame changes nothing.
        shake.step(2.0);
        assert_eq!(shake.picture(), Vec2::ZERO);
    }

    #[test]
    fn a_hits_shake_swings_to_and_fro_as_it_dies_away() {
        let mut shake = Shake::default();
        let hit = Jolt { angle: 0.0, ..jolt(Asked::Kind(3)).unwrap() };
        assert_eq!((hit.amplitude, hit.hold, hit.fade), (8.0, 0.0, 8.0));
        shake.begin(hit);
        // Eight pixels one way, then six the other, four, two, and it is over: at
        // thirty frames a second each frame is half a swing on.
        let mut seen = vec![shake.picture().y];
        for _ in 0..5 {
            shake.step(2.0);
            seen.push(shake.picture().y);
        }
        assert_eq!(seen, vec![8.0, -6.0, 4.0, -2.0, 0.0, 0.0]);
        assert!(!shake.on);
    }

    #[test]
    fn a_weaker_shake_does_not_take_the_place_of_a_stronger_one() {
        let mut shake = Shake::default();
        let strong = Jolt { amplitude: 8.0, angle: 0.0, hold: 0.0, fade: 32.0 };
        let weak = Jolt { amplitude: 4.0, angle: 0.0, hold: 0.0, fade: 8.0 };
        shake.begin(strong);
        shake.begin(weak);
        assert_eq!(shake.amplitude, 8.0);
        // Once the strong one has lost enough of its strength the weak one does.
        for _ in 0..9 {
            shake.step(2.0);
        }
        assert!(shake.strength < 4.0 && shake.on);
        shake.begin(weak);
        assert_eq!((shake.amplitude, shake.time), (4.0, 0.0));
    }

    #[test]
    fn the_line_of_a_shake_leans_and_the_picture_goes_along_it() {
        // A quarter turn's lean has the picture go across, to the left first.
        let mut shake = Shake::default();
        shake.begin(Jolt { amplitude: 5.0, angle: FRAC_PI_2, hold: 4.0, fade: 0.0 });
        assert!(shake.moved.abs_diff_eq(Vec2::new(-5.0, 0.0), 1e-5));
        // What is not given a lean has one of up to thirty degrees either way.
        for seed in 0..40 {
            let burst = Jolt::of(Asked::Burst, &mut Random::new(seed)).unwrap();
            assert!(burst.angle.abs() <= 30_f32.to_radians() + 1e-6);
            assert_eq!((burst.amplitude, burst.fade), (8.0, 32.0));
        }
    }

    #[test]
    fn shakes_are_asked_for_by_number_by_an_attacks_word_and_by_scripts() {
        assert_eq!(jolt(Asked::Kind(1)).map(|jolt| jolt.amplitude), Some(4.0));
        assert_eq!(jolt(Asked::Kind(2)).map(|jolt| jolt.amplitude), Some(6.0));
        assert_eq!(jolt(Asked::Kind(4)), None);
        // An attack's word: 1 to 3 are those, 4 is her shot's, anything else none.
        assert_eq!(jolt(Asked::Attack(3)).map(|jolt| jolt.amplitude), Some(8.0));
        assert_eq!(jolt(Asked::Attack(4)), jolt(Asked::Kind(0)));
        assert_eq!(jolt(Asked::Attack(0)), None);
        assert_eq!(jolt(Asked::Attack(5)), None);
        // A script's: its strength by ten, twenty or thirty, its lean in degrees,
        // and its time in logic frames.
        let scripted = jolt(Asked::Script { kind: 20, degrees: 90, frames: 10 }).unwrap();
        assert_eq!((scripted.amplitude, scripted.fade, scripted.hold), (5.0, 20.0, 0.0));
        assert!((scripted.angle - PI / 2.0).abs() < 1e-6);
        assert_eq!(jolt(Asked::Script { kind: 15, degrees: 0, frames: 10 }), None);
    }

    #[test]
    fn under_a_scope_the_picture_is_moved_by_that_too_in_whole_pixels() {
        let mut shake = Shake { scope: Some(Vec2::new(10.6, -4.2)), ..Shake::default() };
        assert_eq!(shake.picture(), Vec2::new(10.0, -4.0));
        shake.begin(Jolt { amplitude: 3.0, angle: 0.0, hold: 2.0, fade: 0.0 });
        assert_eq!(shake.picture(), Vec2::new(10.0, -1.0));
    }
}
