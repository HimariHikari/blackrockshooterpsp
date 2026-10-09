//! The field's sixteen frames (the field's + 0x4C60, 0x70 bytes each, made as the
//! world is by 0x117C10). A character is in one of them (its int parameter 8,
//! setting 115), and everything of it that goes by an upright goes by its frame's:
//! how its model is turned (0x10925C), the ray that finds the ground under it
//! (0xFD3CC, 0x108520), the way it moves (0x101490). A map's script turns a frame
//! with `s255`, which only the five maps of the sixth stage do: their floors hang in
//! the air at every angle, and each is level to those of its frame. See
//! `docs/FIELD_NOTES.md`, "The frames".
//!
//! The port keeps a copy of the map's collision for each frame that is turned, with
//! the map turned the other way, so that what is in the frame has its ground level
//! under it there ([`Lie`]).

use std::sync::Arc;

use bevy::prelude::*;
use brs_engine::collision::{CollisionWorld, Marks};

pub const FRAMES: usize = 16;

/// The one turn three turns in radians make (0x162D58): about the axis across,
/// then the upright one, then the one ahead.
pub fn turn_of(turns: Vec3) -> Quat {
    Quat::from_rotation_z(turns.z) * Quat::from_rotation_y(turns.y) * Quat::from_rotation_x(turns.x)
}

#[derive(Clone, Default)]
pub struct Frames {
    /// Each one's three turns, in radians.
    turns: [Vec3; FRAMES],
    /// Each one's place, which scripts give and nothing reads.
    places: [Vec3; FRAMES],
    /// The one `s255` sets (the field's + 0x5364).
    chosen: usize,
    /// The map as each one that is turned has it.
    lying: [Option<Arc<CollisionWorld>>; FRAMES],
}

impl std::fmt::Debug for Frames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frames").field("turns", &self.turns).field("places", &self.places).field("chosen", &self.chosen).finish()
    }
}

impl Frames {
    /// `s255`'s first figure (0xE6314): the frame its others are about.
    pub fn choose(&mut self, frame: i32) {
        self.chosen = usize::try_from(frame).unwrap_or(FRAMES);
    }

    /// `s255`'s other two (0xE633C): one of the chosen frame's three turns, in
    /// radians (0 to 2), or of the three figures of its place (3 to 5). A turn
    /// makes the frame's one turn anew (0x117B28), and here the map as it has it.
    pub fn set(&mut self, what: i32, value: f32, world: &CollisionWorld) {
        if self.chosen >= FRAMES {
            return;
        }
        match what {
            0..=2 => {
                self.turns[self.chosen][what as usize] = value;
                self.lay(self.chosen, world);
            }
            3..=5 => self.places[self.chosen][what as usize - 3] = value,
            _ => {}
        }
    }

    fn lay(&mut self, frame: usize, world: &CollisionWorld) {
        let turn = turn_of(self.turns[frame]);
        self.lying[frame] = (turn != Quat::IDENTITY).then(|| Arc::new(world.turned(turn.inverse())));
    }

    /// The turn of a frame; one there is not is as the world is.
    pub fn turn(&self, frame: u8) -> Quat {
        self.turns.get(usize::from(frame)).map_or(Quat::IDENTITY, |&turns| turn_of(turns))
    }

    /// The map as what is in `frame` has it.
    pub fn lie<'a>(&'a self, world: &'a CollisionWorld, frame: u8) -> Lie<'a> {
        match self.lying.get(usize::from(frame)).and_then(Option::as_deref) {
            Some(lying) => Lie { world: lying, turn: self.turn(frame) },
            None => Lie::level(world),
        }
    }
}

/// The map as something in a frame has it: turned so that the frame's upright is
/// the upright, with the turn that takes a place of it back out into the world.
#[derive(Clone, Copy)]
pub struct Lie<'a> {
    pub world: &'a CollisionWorld,
    pub turn: Quat,
}

impl<'a> Lie<'a> {
    /// The map as the world has it.
    pub fn level(world: &'a CollisionWorld) -> Lie<'a> {
        Lie { world, turn: Quat::IDENTITY }
    }

    pub fn is_level(&self) -> bool {
        self.turn == Quat::IDENTITY
    }

    /// A place or a way of the world's, as the frame has it; and back.
    pub fn in_frame(&self, of_the_world: Vec3) -> Vec3 {
        if self.is_level() { of_the_world } else { self.turn.inverse() * of_the_world }
    }

    pub fn in_world(&self, of_the_frame: Vec3) -> Vec3 {
        if self.is_level() { of_the_frame } else { self.turn * of_the_frame }
    }

    /// The ground within `above` over a place and `below` under it, by the frame's
    /// upright (0x108520: the ray is cast along what its upright is turned to,
    /// 0x1090E0): where on it that is.
    pub fn under(&self, place: Vec3, above: f32, below: f32) -> Option<Vec3> {
        let own = self.in_frame(place);
        let ground = self.world.ground(own, above, below)?;
        Some(self.in_world(own.with_y(ground.height)))
    }

    /// What a place is marked as by what lies over and under it (0xFD3CC).
    pub fn marks(&self, place: Vec3, above: f32, below: f32) -> Marks {
        self.world.marks(self.in_frame(place), above, below)
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;
    use crate::field::motion::tests::floor;

    #[test]
    fn three_turns_make_one_as_the_original_makes_it() {
        // 0x162D58 with turns of 0.2, 0.3 and 0.4: the four figures it leaves, worked
        // out from the halves' sines and cosines as it does.
        let turns = Vec3::new(0.2, 0.3, 0.4);
        let (s, c) = ((turns * 0.5).map(f32::sin), (turns * 0.5).map(f32::cos));
        let made = Quat::from_xyzw(
            c.y * c.z * s.x - s.y * s.z * c.x,
            c.z * c.x * s.y + s.z * s.x * c.y,
            c.x * c.y * s.z - s.x * s.y * c.z,
            c.x * c.y * c.z + s.x * s.y * s.z,
        );
        assert!(turn_of(turns).abs_diff_eq(made, 1e-6), "{:?} {made:?}", turn_of(turns));
    }

    #[test]
    fn a_frame_that_is_turned_has_its_own_ground_under_it() {
        // The floor of the tests is level at nought. A frame turned a quarter about
        // the axis ahead has the world's -X for its upright... so the floor is a
        // wall to it, and nothing is under what stands "over" it there.
        let world = floor();
        let mut frames = Frames::default();
        assert!(frames.lie(&world, 2).is_level());
        assert_eq!(frames.lie(&world, 2).under(Vec3::new(10.0, 5.0, 10.0), 20.0, 20.0), Some(Vec3::new(10.0, 0.0, 10.0)));

        frames.choose(2);
        frames.set(2, FRAC_PI_2, &world);
        let lie = frames.lie(&world, 2);
        assert!(!lie.is_level());
        assert!((lie.turn * Vec3::Y).abs_diff_eq(Vec3::NEG_X, 1e-6));
        assert!(lie.under(Vec3::new(10.0, 5.0, 10.0), 20.0, 20.0).is_none());
        // The others are as they were, and a place goes into the frame and out again.
        assert!(frames.lie(&world, 1).is_level());
        let place = Vec3::new(3.0, 4.0, 5.0);
        assert!(lie.in_frame(place).abs_diff_eq(Vec3::new(4.0, -3.0, 5.0), 1e-5), "{}", lie.in_frame(place));
        assert!(lie.in_world(lie.in_frame(place)).abs_diff_eq(place, 1e-5));

        // Turned back, it is level again.
        frames.set(2, 0.0, &world);
        assert!(frames.lie(&world, 2).is_level());
        // A frame there is not is as the world is, and setting one is nothing.
        frames.choose(16);
        frames.set(2, 1.0, &world);
        assert_eq!(frames.turn(16), Quat::IDENTITY);
    }
}
