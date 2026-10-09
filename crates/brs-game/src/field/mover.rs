//! How a field character moves: a port of the original's mover (0x101F04), which the
//! player and every other character share. See `docs/FIELD_NOTES.md`.
//!
//! A character has a direction it wants to go in and a way it faces. Each frame it
//! moves along the direction, the faster the better it faces it and not at all while
//! it faces away, and then turns towards it.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use brs_engine::collision::{Body, CollisionWorld};

/// The least and most a character turns in a frame, at full speed: the turn grows
/// from one to the other with how far round it has to go (0x1020E8).
const TURN_MIN: f32 = 0.174_533;
const TURN_MAX: f32 = 1.221_73;
/// The speed a character that stops gently loses each frame (0x101C98).
const SLOWING: f32 = 2.5;
/// A direction shorter than this is no direction.
const EPSILON: f32 = 1e-5;

/// The way a character faces, as the unit vector on the ground for angle `facing`
/// about the vertical: zero looks along +Z.
pub fn forward(facing: f32) -> Vec3 {
    Vec3::new(facing.sin(), 0.0, facing.cos())
}

/// An angle brought into (-π, π].
pub fn wrap(angle: f32) -> f32 {
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI { wrapped + TAU } else { wrapped }
}

#[derive(Clone, Copy, Debug)]
pub struct Mover {
    /// The angle it faces, about the vertical, in radians.
    pub facing: f32,
    /// What it moved by in the last frame.
    pub velocity: Vec3,
    /// The way it wants to go: a unit vector on the ground.
    pub direction: Vec3,
    /// Its speed in units a frame, as its script gives it for the status it moves in.
    pub speed: f32,
    /// How fast its time runs: 1 is full speed. Its script sets this as a percentage
    /// (0x10821C), and it scales the turning as well as the moving.
    pub pace: f32,
    /// Set while it is to stop gently rather than at once (the original's flag 0xA4).
    pub slides: bool,
}

impl Mover {
    pub fn new(facing: f32, speed: f32) -> Mover {
        Mover { facing, velocity: Vec3::ZERO, direction: forward(facing), speed, pace: 1.0, slides: false }
    }

    /// One frame of a character at `feet`: it moves if `moving` (its status is the
    /// one it moves in), slows or stops if not, and is kept on the ground and out of
    /// the walls. Returns where it now stands.
    pub fn step(&mut self, feet: Vec3, moving: bool, world: &CollisionWorld, body: &Body) -> Vec3 {
        // The speed is worked out from how it faced before this frame's turn.
        self.velocity = if moving { self.advance() } else { self.slow() };
        let feet = if self.velocity.length() > EPSILON { world.walk(feet, self.velocity, body) } else { feet };
        if moving {
            self.turn();
        }
        feet
    }

    /// One frame of a character that rushes from `feet` the straight way it wants
    /// to go, which need not be level: it moves as far as it would over the
    /// ground, turns as it would, and is on the ground where there is ground
    /// within a step of where it comes to. (The original has one mover for this
    /// and for a walk, 0x101F04, and sweeps a sphere through the map; the port's
    /// walk keeps to the ground, so this stands in for the way up or down.)
    pub fn rush(&mut self, feet: Vec3, world: &CollisionWorld, body: &Body) -> Vec3 {
        self.velocity = self.advance();
        let mut feet = feet + self.velocity;
        if let Some(ground) = world.ground(feet, body.step_height, body.step_height) {
            feet.y = ground.height;
        }
        self.turn();
        feet
    }

    /// What it moves by in a frame when going its way (0x101B1C).
    fn advance(&self) -> Vec3 {
        let alignment = forward(self.facing).dot(self.direction);
        if alignment < 0.0 { Vec3::ZERO } else { self.direction * (self.speed * self.pace * alignment) }
    }

    /// What is left of its movement a frame after it stops going (0x101C68).
    fn slow(&self) -> Vec3 {
        let lost = SLOWING * self.pace;
        let speed = self.velocity.length();
        if self.slides && speed > lost { self.velocity * ((speed - lost) / speed) } else { Vec3::ZERO }
    }

    /// Turns towards the direction it wants to go in (the second half of 0x101F04).
    fn turn(&mut self) {
        // The direction as it lies in the character's own frame, where it faces +Z.
        let local = Quat::from_rotation_y(-self.facing) * self.direction;
        let local = Vec3::new(local.x, 0.0, local.z);
        if local.length() <= EPSILON {
            return;
        }
        let local = local.normalize();
        let least = (TURN_MIN * self.pace).min(PI);
        let most = (TURN_MAX * self.pace).min(PI);
        let angle = local.z.clamp(-1.0, 1.0).acos();
        let turn = if angle <= least {
            // Near enough: it faces the direction outright.
            local.x.atan2(local.z)
        } else {
            let turn = (most - least) * (angle / PI) + least;
            // Straight behind counts as a turn to the left.
            if local.x < 0.0 { -turn } else { turn }
        };
        self.facing = wrap(self.facing + turn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: Body = Body { radius: 10.0, step_height: 5.0 };

    /// A flat floor wide enough for every test.
    fn floor() -> CollisionWorld {
        use brs_formats::clim::{Collision, Triangle};
        let far = 100_000.0;
        let collision = Collision {
            vertices: vec![[-far, 0.0, -far], [-far, 0.0, far], [far, 0.0, -far], [far, 0.0, far]],
            triangles: vec![Triangle { indices: [0, 1, 2], attribute: 0 }, Triangle { indices: [2, 1, 3], attribute: 0 }],
            attributes: Vec::new(),
        };
        let mut world = CollisionWorld::default();
        world.add(&collision, brs_engine::collision::Layer::World);
        world
    }

    #[test]
    fn a_character_facing_its_way_moves_at_its_speed() {
        let world = floor();
        let mut mover = Mover::new(0.0, 32.0);
        mover.direction = Vec3::Z;
        let feet = mover.step(Vec3::ZERO, true, &world, &BODY);
        assert!((feet - Vec3::new(0.0, 0.0, 32.0)).length() < 1e-3, "moved to {feet}");
        // At 95 percent it covers 95 percent of the ground.
        mover.pace = 0.95;
        let feet = mover.step(Vec3::ZERO, true, &world, &BODY);
        assert!((feet.z - 30.4).abs() < 1e-3);
    }

    #[test]
    fn it_does_not_move_while_it_faces_away_and_speeds_up_as_it_comes_round() {
        let world = floor();
        let mut mover = Mover::new(0.0, 32.0);
        mover.direction = Vec3::NEG_Z;
        // Straight behind: it only turns, by the most it can in a frame, to the left.
        let feet = mover.step(Vec3::ZERO, true, &world, &BODY);
        assert_eq!(feet, Vec3::ZERO);
        assert!((mover.facing - TURN_MAX).abs() < 1e-5, "faces {}", mover.facing);

        // Still more than a quarter turn off: no movement yet.
        let feet = mover.step(feet, true, &world, &BODY);
        assert_eq!(feet, Vec3::ZERO);
        // Past the quarter turn it starts to move, slower than its speed.
        let feet = mover.step(feet, true, &world, &BODY);
        let moved = feet.length();
        assert!(moved > 0.0 && moved < 32.0, "moved {moved}");
        assert!(feet.z < 0.0);
        // A few frames on it faces its way and moves at full speed.
        let mut feet = feet;
        for _ in 0..4 {
            feet = mover.step(feet, true, &world, &BODY);
        }
        assert!((wrap(mover.facing - PI)).abs() < 1e-4);
        assert!((mover.velocity.length() - 32.0).abs() < 1e-3);
    }

    #[test]
    fn the_turn_grows_with_how_far_round_it_has_to_go() {
        // A quarter turn off: 10 degrees plus half of the 60 between least and most.
        let mut mover = Mover::new(0.0, 32.0);
        mover.direction = Vec3::X;
        mover.turn();
        assert!((mover.facing - 40f32.to_radians()).abs() < 1e-4);
        // To the other side it turns the other way.
        let mut mover = Mover::new(0.0, 32.0);
        mover.direction = Vec3::NEG_X;
        mover.turn();
        assert!((mover.facing + 40f32.to_radians()).abs() < 1e-4);
        // Within the least turn it snaps.
        let mut mover = Mover::new(0.0, 32.0);
        mover.direction = forward(0.1);
        mover.turn();
        assert!((mover.facing - 0.1).abs() < 1e-5);
    }

    #[test]
    fn a_character_stops_at_once_unless_it_is_to_slide() {
        let world = floor();
        let mut mover = Mover::new(0.0, 32.0);
        mover.direction = Vec3::Z;
        let feet = mover.step(Vec3::ZERO, true, &world, &BODY);
        let stopped = mover.step(feet, false, &world, &BODY);
        assert_eq!(stopped, feet);
        assert_eq!(mover.velocity, Vec3::ZERO);

        // Sliding, it loses 2.5 units of speed a frame.
        let mut feet = mover.step(feet, true, &world, &BODY);
        mover.slides = true;
        feet = mover.step(feet, false, &world, &BODY);
        assert!((mover.velocity.length() - 29.5).abs() < 1e-3);
        let mut frames = 1;
        while mover.velocity != Vec3::ZERO {
            feet = mover.step(feet, false, &world, &BODY);
            frames += 1;
        }
        assert_eq!(frames, 13);
        assert!(feet.z > 64.0 + 150.0);
    }

    #[test]
    fn angles_wrap_into_one_turn() {
        assert!((wrap(3.0 * PI) - PI).abs() < 1e-5);
        assert!((wrap(-PI) - PI).abs() < 1e-5);
        assert!((wrap(0.5) - 0.5).abs() < 1e-6);
        assert!((wrap(TAU + 0.25) - 0.25).abs() < 1e-5);
    }
}
