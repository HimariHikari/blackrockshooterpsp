//! A frame of a field character: what the action a script gave it does to it, and
//! its movement over the ground. Ports of the original's action updates (the walk at
//! 0xF8A10, the turn, the appearing at 0xF72B4, the end at 0xF93EC); see
//! `docs/FIELD_NOTES.md`.

use bevy::prelude::*;
use brs_engine::collision::{Body, CollisionWorld};

use super::frames::{Frames, Lie};
use super::mover::wrap;
use super::scene::{
    flag, status, Action, Character, LeapPart, Motion, Show, Solid, Wall, APPEAR_RISE, APPEAR_WAIT, BRAKE_HELD, BURST_AT, BURST_GONE, DESTROY_FADES, DESTROY_TIME, DROP_HEIGHT,
    DROP_SPEED, FLIGHT_REACH, FULL_PACE, HALF_GRAVITY, HOP_TIME, LANDING_REACH, LEAP_EFFECT_UP, MOTION_LAND, MOTION_RUSH_STOP, MOTION_SPRING, MOTION_UNKNOWN,
    POP_UP_DEPTH, POP_UP_SPEED, RUSH_REACH, SOUND_LEAP, SOUND_LEAP_EFFECT,
};

/// A walk is over when its target is nearer over the ground than this many frames of
/// walking, and no further above or below than `ARRIVAL_HEIGHT`.
const ARRIVAL_FRAMES: f32 = 2.0;
const ARRIVAL_HEIGHT: f32 = 200.0;
/// A direction shorter than this is no direction.
const EPSILON: f32 = 1e-5;

/// How high a ledge a character steps up or down in its stride. The original sweeps
/// a sphere over the ground instead; this stands in for that.
const STEP_HEIGHT: f32 = 40.0;

/// What a character's resource says about it.
pub trait Resources {
    /// The speed its script gives for `status`, in units a frame.
    fn speed(&self, chr: i32, status: i32) -> f32;
    /// How far below the ground it starts when it appears.
    fn pop_height(&self, chr: i32) -> f32;
    /// The way it is destroyed: its script's `type_destroy` (0x1027CC).
    fn destroy_kind(&self, chr: i32) -> u8;
    /// How long one of its own animations is, in frames of the game, if it has
    /// been read and has it.
    fn motion_frames(&self, chr: i32, motion: i32) -> Option<f32>;
    /// How far into one of its own animations the part of it that goes round
    /// begins, in frames of the game (0xFE164 with its first bit).
    fn motion_lead_in(&self, chr: i32, motion: i32) -> Option<f32>;
}

/// The body characters have for the map's collision: a sphere of the radius the
/// field's tunables give.
pub fn body(radius: f32) -> Body {
    Body { radius, step_height: STEP_HEIGHT }
}

/// What a character moves over and is held in by.
pub struct Ground<'a> {
    pub world: &'a CollisionWorld,
    pub body: &'a Body,
    /// The walls the map's script has put up.
    pub walls: &'a [Wall],
    /// What of the characters stands in the way, as the frame began.
    pub solids: &'a [Solid],
    /// The field's frames, by which one that is in a frame that is turned has
    /// the map; none where the map is as the world has it for everything.
    pub frames: Option<&'a Frames>,
}

impl<'a> Ground<'a> {
    /// The map as `character` has it, by the frame it is in.
    pub fn lie(&self, character: &Character) -> Lie<'a> {
        match self.frames {
            Some(frames) => frames.lie(self.world, character.group()),
            None => Lie::level(self.world),
        }
    }
}

/// One frame of `character` (0x106174): what it is doing goes on, and when that
/// ends, one a trap is taking from place to place sets off for the next of them
/// (0x1061B8); any other stands again, unless it keeps its pose.
pub fn step(character: &mut Character, resources: &dyn Resources, ground: &Ground) {
    // One that is hidden has no frame, nor has one too far from her (0x106050,
    // 0x106060).
    if character.hidden() || character.asleep {
        return;
    }
    let acted = character.acting();
    act_in_its_frame(character, resources, ground);
    if acted && !character.acting() {
        let taken_on = character.taken_on(resources.speed(character.chr, status::RUN));
        if !taken_on && character.action != Action::None && !character.flags.has(flag::KEEPS_POSE) {
            character.stop();
        }
    }
}

/// A frame of what `character` is doing, in the frame it is in. The original has
/// everything of a character that goes by an upright go by its frame's (0x1090E0
/// turns the upright for the ray under it, 0xFD3CC and 0x101490, and for what it is
/// thrown by, 0xF5538; 0x105480 and 0x101B1C take the way it wants to go into its
/// frame). The port has one whose frame is turned act in that frame instead, where
/// its ground is level: it is taken into it with all that is in its way, and out
/// again. One that is going over from one frame to another (0x10930C) is in
/// neither, and acts in the world.
fn act_in_its_frame(character: &mut Character, resources: &dyn Resources, ground: &Ground) {
    let lie = ground.lie(character);
    if lie.is_level() || character.between_frames() {
        return act(character, resources, ground);
    }
    let (into, shown, lay) = (lie.turn.inverse(), character.shows.len(), character.lay);
    character.turn_all(into);
    character.lay = Quat::IDENTITY;
    let walls: Vec<Wall> = ground.walls.iter().map(|wall| Wall { centre: into * wall.centre, ..*wall }).collect();
    let solids: Vec<Solid> = ground.solids.iter().map(|solid| solid.turned(into)).collect();
    act(character, resources, &Ground { world: lie.world, body: ground.body, walls: &walls, solids: &solids, frames: None });
    character.turn_all(lie.turn);
    character.lay = lay;
    for show in &mut character.shows[shown..] {
        show.turn(lie.turn);
    }
}

/// So many frames of the game in the field's units of time; none is no time.
fn units(frames: Option<f32>) -> i32 {
    (frames.unwrap_or(0.0) * FULL_PACE as f32) as i32
}

/// Moves `character` for a frame as one that rushes moves: the way it wants to
/// go, at the speed of the status it is in.
fn rush_on(character: &mut Character, resources: &dyn Resources, ground: &Ground) {
    character.mover.speed = resources.speed(character.chr, character.status);
    character.position = character.mover.rush(character.position, ground.world, ground.body);
}

/// A frame of what `character` is doing.
fn act(character: &mut Character, resources: &dyn Resources, ground: &Ground) {
    // An action goes on for as long as the character's flag says it is acting.
    if !character.acting() {
        character.action = Action::None;
    }
    let pace = character.pace_fraction();
    character.mover.pace = pace;
    match character.action {
        Action::None => {
            // Left to itself it moves if its status is the one it moves in: the
            // player's, while the stick is held; and how long it has is counted
            // (0xF5FC8).
            let moving = character.status == character.move_status;
            if moving {
                character.ran = character.ran.saturating_add(character.time());
            }
            walk_on(character, moving, resources, ground);
        }
        // Pulling up (0xF5798): for as long as its animation plays she comes to
        // a stop, and from a little into it the stick may take her out of it.
        Action::Brake { time } => {
            let time = time + character.time();
            if time > BRAKE_HELD {
                character.flags.set(flag::FREE_TO_GO, true);
            }
            if time <= units(resources.motion_frames(character.chr, MOTION_RUSH_STOP)) {
                walk_on(character, false, resources, ground);
                character.action = Action::Brake { time };
            } else {
                character.flags.set(flag::ACTING, false);
            }
        }
        Action::Walk => {
            let speed = resources.speed(character.chr, character.move_status);
            if !character.flags.has(flag::DASHING) {
                let to_target = character.target - character.position;
                let over_ground = Vec3::new(to_target.x, 0.0, to_target.z);
                if over_ground.length() > EPSILON {
                    character.mover.direction = over_ground.normalize();
                }
                if over_ground.length() < ARRIVAL_FRAMES * speed && to_target.y.abs() < ARRIVAL_HEIGHT {
                    character.stop();
                    return;
                }
            }
            walk_on(character, true, resources, ground);
        }
        Action::Turn { rate } => {
            let left = wrap(character.target_facing - character.facing());
            let turn = rate * pace;
            if left.abs() <= turn.abs() + EPSILON || left * turn <= 0.0 {
                character.mover.facing = wrap(character.target_facing);
                character.stop();
            } else {
                character.mover.facing = wrap(character.facing() + turn);
            }
        }
        Action::Appear { time, depth } => {
            let time = time + character.time();
            if time >= APPEAR_WAIT + APPEAR_RISE {
                character.position = character.target;
                character.mover.facing = character.target_facing;
                character.shows.push(Show::RingGoes);
                character.stop();
                return;
            }
            if time > APPEAR_WAIT {
                // It comes up fast and settles: one minus the square of what is left.
                let left = 1.0 - (time - APPEAR_WAIT) as f32 / APPEAR_RISE as f32;
                character.position.y = character.target.y - depth * left * left;
            }
            character.action = Action::Appear { time, depth };
        }
        // It is thrown up out of the ground, and has come when it would be back
        // at the height it is to stand at (0xF6EE4); or thrown down from above,
        // and something rises where it lands (0xF7544).
        Action::PopUp { time } | Action::DropIn { time } => {
            let time = time + character.time();
            let dropping = matches!(character.action, Action::DropIn { .. });
            if time >= HOP_TIME {
                character.position = character.target;
                character.mover.facing = character.target_facing;
                if dropping {
                    character.shows.push(Show::Landing { at: character.target, facing: character.target_facing });
                }
                character.stop();
                return;
            }
            let part = time as f32 / APPEAR_RISE as f32;
            if dropping {
                character.position.y = character.target.y + DROP_HEIGHT - (DROP_SPEED * part + HALF_GRAVITY * part * part) * 100.0;
                character.action = Action::DropIn { time };
            } else {
                character.position.y = character.target.y - POP_UP_DEPTH + (POP_UP_SPEED * part - HALF_GRAVITY * part * part) * 100.0;
                character.action = Action::PopUp { time };
            }
        }
        // An animation a script set playing is played through (0xF7C08): then it
        // stands again, unless it keeps its pose (0x1061B8).
        Action::Motion { time, end } => {
            let time = time + character.time();
            let own = match character.motion {
                Some((Motion::Own(number), _)) => resources.motion_frames(character.chr, number),
                _ => None,
            };
            let end = end.or(own.map(|frames| (frames * FULL_PACE as f32) as i32));
            if time >= end.unwrap_or(MOTION_UNKNOWN) {
                character.stop();
                if !character.flags.has(flag::KEEPS_POSE) {
                    character.motion = None;
                }
            } else {
                character.action = Action::Motion { time, end };
            }
        }
        // A leap (0xF8434). It gathers itself, turning to the way it is to face,
        // until the animation it springs with comes to the part that goes round;
        // something rises where it springs from, and it is in the air, so far
        // along the way and thrown above it, first still springing and then
        // coming down; where it comes down it is at its target, and if a trap
        // has another place for it that is the end of it; else something rises
        // there too, it is stood on the ground, and it lands for as long as the
        // animation it lands with is. Time one part does not use the next has.
        Action::Leap(mut leap) => {
            let mut time = character.time();
            if leap.part == LeapPart::Gather {
                let gather = units(resources.motion_lead_in(character.chr, MOTION_SPRING));
                let gone = leap.time + time;
                if gone < gather {
                    leap.time = gone;
                    let turned = (character.target_facing - leap.facing) * gone as f32 / gather as f32;
                    character.mover.facing = wrap(leap.facing + turned);
                    character.action = Action::Leap(leap);
                    return;
                }
                character.mover.facing = wrap(character.target_facing);
                let at = character.position + character.up() * LEAP_EFFECT_UP;
                character.shows.extend([Show::Spring { at, facing: character.facing() }, Show::Heard(SOUND_LEAP_EFFECT)]);
                // One that is to be in another frame where it comes down begins
                // to go over to it as it leaves the ground (0xF8694), for as
                // long as it is in the air.
                if let (Some(next), Some(frames)) = (character.group_next.take(), ground.frames) {
                    character.change_group(next, frames, leap.length);
                }
                character.shows.push(Show::Heard(SOUND_LEAP));
                time = gone - gather;
                (leap.part, leap.time) = (LeapPart::Spring, 0);
            }
            if leap.part == LeapPart::Spring {
                let gone = leap.time + time;
                if gone < leap.spring {
                    leap.time = gone;
                    character.position = leap.place(character.target, gone, character.up());
                    character.step_turning(time);
                    character.action = Action::Leap(leap);
                    return;
                }
                character.step_turning(leap.spring - leap.time);
                time = gone - leap.spring;
                (leap.part, leap.time) = (LeapPart::ComeDown, leap.spring);
            }
            if leap.part == LeapPart::ComeDown {
                let gone = leap.time + time;
                if gone < leap.length {
                    leap.time = gone;
                    character.position = leap.place(character.target, gone, character.up());
                    character.step_turning(time);
                    character.action = Action::Leap(leap);
                    return;
                }
                character.position = character.target;
                character.step_turning(leap.length - leap.time);
                character.action = Action::Leap(leap);
                if character.flags.has(flag::TAKEN_ON) {
                    character.flags.set(flag::ACTING, false);
                    return;
                }
                time = gone - leap.length;
                (leap.part, leap.time) = (LeapPart::Land, 0);
                let at = character.position + character.up() * LEAP_EFFECT_UP;
                character.shows.extend([Show::Landing { at, facing: character.facing() }, Show::Heard(SOUND_LEAP_EFFECT)]);
                // Where there is ground near (0x108520), by the frame it is in now.
                if let Some(under) = ground.lie(character).under(character.position, LANDING_REACH, LANDING_REACH) {
                    character.position = under;
                }
            }
            character.step_turning(time);
            let gone = leap.time + time;
            leap.time = gone;
            character.action = Action::Leap(leap);
            if gone >= units(resources.motion_frames(character.chr, MOTION_LAND)) {
                character.flags.set(flag::ACTING, false);
            }
        }
        // A rush (0xF66C4): it is there when its target is near enough, and
        // until then goes the straight way to it.
        Action::Rush => {
            let way = character.target - character.position;
            if way.length() < RUSH_REACH {
                character.end_rush();
                return;
            }
            if way.length() > EPSILON {
                character.mover.direction = way.normalize();
            }
            rush_on(character, resources, ground);
        }
        // Stopping after one (0xF6888): for as long as the animation it stops
        // with plays, it goes on as it was going.
        Action::RushStop { time } => {
            let time = time + character.time();
            if time <= units(resources.motion_frames(character.chr, MOTION_RUSH_STOP)) {
                rush_on(character, resources, ground);
                character.action = Action::RushStop { time };
            } else {
                character.flags.set(flag::ACTING, false);
            }
        }
        // A flight (0xF6AFC): it is there when its target is within reach, and
        // until then is moved straight to it at the speed of the status it is
        // in, lying in the air. (Where a step would take it past its target and
        // leave it no nearer than it has to come, the original goes to and fro
        // about it for ever; the port has it there.)
        Action::Flight => {
            let way = character.target - character.position;
            let (left, step) = (way.length(), resources.speed(character.chr, character.status) * pace);
            let never = left < step && left >= FLIGHT_REACH && step - left >= FLIGHT_REACH;
            if left < FLIGHT_REACH || never {
                if never {
                    character.position = character.target;
                }
                character.end_flight();
                return;
            }
            character.mover.direction = way / left;
            character.lie_in_flight();
            character.position += way / left * step;
        }
        Action::Destroy { time } => {
            let time = time + character.time();
            // One that fades is gone when it has (0xF9730), and until then there
            // is as much of it as is left of its time (0xF96F8); any other
            // bursts once, and is gone a little after (0xF9778).
            let gone = if resources.destroy_kind(character.chr) == DESTROY_FADES {
                character.alpha = (1.0 - time as f32 / DESTROY_TIME as f32).max(0.0);
                time >= DESTROY_TIME
            } else {
                if time > BURST_AT && !character.flags.has(flag::BURST) {
                    character.flags.set(flag::BURST, true);
                    character.shows.push(Show::Burst);
                }
                time > BURST_GONE
            };
            if gone {
                character.stop();
                character.status = status::DESTROYED;
                character.flags.set(flag::HIDDEN, true);
            } else {
                character.action = Action::Destroy { time };
            }
        }
    }
}

/// Moves `character` over the ground for a frame: on its way if `moving`, else
/// coming to a stop.
fn walk_on(character: &mut Character, moving: bool, resources: &dyn Resources, ground: &Ground) {
    if moving {
        character.mover.speed = resources.speed(character.chr, character.status);
    }
    let before = character.position;
    let mut feet = character.mover.step(before, moving, ground.world, ground.body);
    if feet != before {
        for wall in ground.walls {
            feet = wall.push_out(feet, ground.body.radius);
        }
        // Every other character that is solid, which is every one but the one
        // that stands where this one stood.
        for solid in ground.solids.iter().filter(|solid| solid.of != before) {
            feet = solid.push_out(feet, ground.body.radius);
        }
    }
    character.position = feet;
}

#[cfg(test)]
pub(crate) mod tests {
    use brs_engine::collision::Layer;
    use brs_formats::clim::{Collision, Triangle};

    use super::*;
    use crate::field::scene::{test_defaults, BRAKE_AFTER, FULL_PACE};

    /// Every character walks at 6 and runs at 32, as the player does.
    pub struct Speeds;

    impl Resources for Speeds {
        fn speed(&self, _: i32, status: i32) -> f32 {
            if status == status::WALK { 6.0 } else { 32.0 }
        }

        fn pop_height(&self, _: i32) -> f32 {
            100.0
        }

        /// The character of this number fades as it is destroyed, as a box does.
        fn destroy_kind(&self, chr: i32) -> u8 {
            if chr == FADING { DESTROY_FADES } else { 0 }
        }

        /// Every animation is as many frames long as ten times its number.
        fn motion_frames(&self, _: i32, motion: i32) -> Option<f32> {
            (motion > 0).then_some(motion as f32 * 10.0)
        }

        /// And comes to the part of it that goes round after eight.
        fn motion_lead_in(&self, _: i32, motion: i32) -> Option<f32> {
            (motion > 0).then_some(8.0)
        }
    }

    pub const FADING: i32 = 77;

    /// A flat floor wide enough for every test.
    pub fn floor() -> CollisionWorld {
        let far = 100_000.0;
        let collision = Collision {
            vertices: vec![[-far, 0.0, -far], [-far, 0.0, far], [far, 0.0, -far], [far, 0.0, far]],
            triangles: vec![Triangle { indices: [0, 1, 2], attribute: 0 }, Triangle { indices: [2, 1, 3], attribute: 0 }],
            attributes: Vec::new(),
        };
        let mut world = CollisionWorld::default();
        world.add(&collision, Layer::World);
        world
    }

    fn character() -> Character {
        Character::new(21, Vec3::ZERO, 0.0, &test_defaults())
    }

    /// A frame of a character on ground with no walls put up.
    fn frame(character: &mut Character, world: &CollisionWorld, body: &Body) {
        step(character, &Speeds, &Ground { world, body, walls: &[], solids: &[], frames: None });
    }

    #[test]
    fn a_wall_holds_a_character_that_runs_at_it() {
        let (world, body) = (floor(), body(40.0));
        let mut wall = Wall::new();
        wall.shape(Vec3::new(0.0, 0.0, 300.0), Vec3::new(1000.0, 150.0, 100.0), 0.0);
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        player.status = status::RUN;
        player.mover.direction = Vec3::Z;
        for _ in 0..20 {
            step(&mut player, &Speeds, &Ground { world: &world, body: &body, walls: &[wall], solids: &[], frames: None });
        }
        // The wall's near face is at 200; she is held a body's radius short of it.
        assert_eq!(player.position, Vec3::new(0.0, 0.0, 160.0));
        // Opened, it lets her through.
        wall.open = true;
        for _ in 0..20 {
            step(&mut player, &Speeds, &Ground { world: &world, body: &body, walls: &[wall], solids: &[], frames: None });
        }
        assert!(player.position.z > 700.0);
    }

    #[test]
    fn a_character_cannot_walk_through_another_that_is_solid() {
        let (world, body) = (floor(), body(40.0));
        let mut other = Character::new(60, Vec3::new(0.0, 0.0, 300.0), 0.0, &test_defaults());
        let run = |other: &Character, from: Vec3| {
            let mut player = Character::new(0, from, 0.0, &test_defaults());
            player.status = status::RUN;
            player.mover.direction = Vec3::Z;
            for _ in 0..30 {
                // What stands in the way is everybody as the frame begins,
                // herself among them: nothing is in its own way.
                let solids: Vec<Solid> = [&player, other].iter().filter_map(|character| character.solid(body.radius)).collect();
                step(&mut player, &Speeds, &Ground { world: &world, body: &body, walls: &[], solids: &solids, frames: None });
            }
            player.position
        };
        // A character is a round of the field's radius: one that runs straight at
        // it is held the two radii short of its middle.
        assert_eq!(run(&other, Vec3::ZERO), Vec3::new(0.0, 0.0, 220.0));
        // One that runs at it a little to the side slides round it and goes on.
        let past = run(&other, Vec3::new(30.0, 0.0, 0.0));
        assert!(past.z > 700.0 && past.x > 60.0, "{past}");
        // What is hidden, told not to be solid, or an effect is in nobody's way.
        other.flags.set(flag::HIDDEN, true);
        assert!(other.solid(40.0).is_none() && run(&other, Vec3::ZERO).z > 700.0);
        other.flags.set(flag::HIDDEN, false);
        other.flags.set(flag::NO_DROP, true);
        assert!(other.solid(40.0).is_none());
        let effect = Character::of_effect("some.efc".to_owned(), &test_defaults());
        assert!(effect.solid(40.0).is_none());
        // And it is in nobody's way from above or below.
        other.flags.set(flag::NO_DROP, false);
        other.position.y = -150.0;
        assert!(run(&other, Vec3::ZERO).z > 700.0);

        // One a script has made a box is that box, about a place of its own
        // frame and turned as it is: a container a hundred and forty to either
        // side of sixty above where it stands, set a quarter turn round.
        let mut container = Character::new(150, Vec3::new(0.0, 0.0, 500.0), std::f32::consts::FRAC_PI_2, &test_defaults());
        container.be_a_box(Vec3::new(0.0, 60.0, 30.0), Vec3::new(140.0, 200.0, 70.0), 0.0);
        // Turned so, its thirty ahead is thirty to the side, and it is seventy
        // deep across her way from there: she is held a radius before it.
        let held = run(&container, Vec3::new(30.0, 0.0, 0.0));
        assert!((held.z - (500.0 - 140.0 - 40.0)).abs() < 1e-3 && held.x == 30.0, "{held}");
        let beside = run(&container, Vec3::new(150.0, 0.0, 0.0));
        assert!(beside.z > 700.0, "{beside}");
    }

    /// The field's frames with the second turned a quarter about the axis ahead,
    /// as the sixth stage's first map has it: its upright is the world's -X.
    fn a_quarter_turned(world: &CollisionWorld) -> Frames {
        let mut frames = Frames::default();
        frames.choose(2);
        frames.set(2, std::f32::consts::FRAC_PI_2, world);
        frames
    }

    #[test]
    fn one_in_a_frame_that_is_turned_walks_over_that_frames_ground() {
        use crate::field::scene::setting;
        // All the ground there is stands upright in the world: a wall where x is
        // nought, which is level to the frame.
        let world = floor().turned(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        let (frames, body) = (a_quarter_turned(&world), body(40.0));
        let run_up_it = |frames: Option<&Frames>| {
            let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
            if let Some(frames) = frames {
                player.set(setting::GROUP, 2);
                player.lie_in(frames);
            }
            player.status = status::RUN;
            // The way she wants to go is a way of the world's: up the wall.
            player.mover.direction = Vec3::Y;
            for _ in 0..10 {
                step(&mut player, &Speeds, &Ground { world: &world, body: &body, walls: &[], solids: &[], frames });
            }
            player
        };
        let player = run_up_it(Some(&frames));
        assert!(player.up().abs_diff_eq(Vec3::NEG_X, 1e-6));
        assert!(player.position.x.abs() < 1e-2 && player.position.y > 200.0 && player.position.z.abs() < 1e-2, "{}", player.position);
        // She has turned to face the way she goes, which in her frame is to the
        // side; her model faces up the wall, and the way is the world's still.
        assert!((player.facing() - std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{}", player.facing());
        assert!((player.frame().rotation * Vec3::Z).abs_diff_eq(Vec3::Y, 1e-3));
        assert!(player.mover.direction.abs_diff_eq(Vec3::Y, 1e-5));
        // One in a frame that is as the world is has no ground there to go over.
        assert_eq!(run_up_it(None).position, Vec3::ZERO);
    }

    #[test]
    fn a_leap_into_another_frame_turns_her_over_on_the_way() {
        use crate::field::scene::{TakenBy, TakenTo};
        let (world, body) = (floor(), body(40.0));
        let frames = a_quarter_turned(&world);
        let ground = Ground { world: &world, body: &body, walls: &[], solids: &[], frames: Some(&frames) };
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        // A trap whose box is in one frame and that takes her into another has
        // her leap be one that changes to it (`s235`).
        let to = Vec3::new(960.0, 480.0, 0.0);
        player.group_next = Some(2);
        player.take_to(&[TakenTo { by: TakenBy::Leap, line: [to, to] }], 32.0);
        // Though the place is far above her it is timed by how far off it is
        // alone, at her run's thirty-two a frame, and she springs for none of it.
        let Action::Leap(leap) = player.action else { panic!("she should be leaping") };
        let long = (to.length() / 32.0 * FULL_PACE as f32) as i32;
        assert!(leap.between && leap.spring == 0 && (leap.length - long).abs() <= 1, "{leap:?}");
        // She gathers herself in the frame she is in.
        for _ in 0..7 {
            step(&mut player, &Speeds, &ground);
            assert!(player.group() == 0 && player.turning.is_none() && player.between_frames());
        }
        // As she leaves the ground she is in the other, and lies as she did.
        step(&mut player, &Speeds, &ground);
        assert!(player.group() == 2 && player.group_next.is_none() && player.turning.is_some());
        assert!(player.lay.abs_diff_eq(Quat::IDENTITY, 1e-5), "{:?}", player.lay);
        // Half-way she is half-way there and half turned over, and more: it goes
        // fast at first. She is thrown a tenth as high as on a leap of the kind
        // that stays in its frame, and that by her upright as it is just then.
        let half = (leap.length / FULL_PACE / 2) as usize;
        for _ in 0..half {
            step(&mut player, &Speeds, &ground);
        }
        let part = half as f32 * FULL_PACE as f32 / leap.length as f32;
        let over = player.up().angle_between(Vec3::Y).to_degrees();
        assert!((over - 90.0 * (1.0 - (1.0 - part) * (1.0 - part))).abs() < 1.0, "{over}");
        let thrown = player.position - to * part;
        assert!(thrown.length() > 10.0 && thrown.length() < 16.0 && thrown.normalize().abs_diff_eq(player.up(), 0.1), "{thrown}");
        // She comes down where she was to, lying as the frame does, and facing
        // the way she came as that lies over its ground: up the world's upright,
        // which is to the side in the frame.
        for _ in 0..half + 20 {
            step(&mut player, &Speeds, &ground);
        }
        assert!(player.turning.is_none() && !player.between_frames());
        assert!(player.position.abs_diff_eq(to, 1e-3) && player.up().abs_diff_eq(Vec3::NEG_X, 1e-5), "{} {}", player.position, player.up());
        assert!((player.facing() - std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{}", player.facing());
    }

    #[test]
    fn a_trap_takes_her_from_place_to_place_by_leaps() {
        use crate::field::scene::{Leap, TakenBy, TakenTo, A_SECOND};
        let (world, body) = (floor(), body(40.0));
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        // A place on a level with her, on a line across her way, and one further
        // on that is higher.
        let places = [
            TakenTo { by: TakenBy::Leap, line: [Vec3::new(-100.0, 0.0, 640.0), Vec3::new(100.0, 0.0, 640.0)] },
            TakenTo { by: TakenBy::Leap, line: [Vec3::new(0.0, 300.0, 1040.0), Vec3::new(0.0, 300.0, 1040.0)] },
        ];
        // A trap with no places takes her nowhere.
        player.take_to(&[], 32.0);
        assert!(!player.acting() && player.taken.is_none());
        player.take_to(&places, 32.0);
        // The nearest of the first line is straight ahead: at her run's thirty-two
        // a frame that is twenty frames off, of which she springs for a fifth.
        let Action::Leap(leap) = player.action else { panic!("she should be leaping") };
        assert_eq!((leap.part, leap.length, leap.spring), (LeapPart::Gather, 20 * FULL_PACE, 4 * FULL_PACE));
        assert_eq!((player.target, player.target_facing, player.status), (Vec3::new(0.0, 0.0, 640.0), 0.0, status::LEAP));
        assert!(player.acting() && player.flags.has(flag::TAKEN_ON) && !player.flags.has(flag::FROM_FLIGHT));
        // She gathers herself for the eight frames her animation takes to come
        // round, and something rises where she springs from, with two sounds.
        for _ in 0..7 {
            frame(&mut player, &world, &body);
            assert!(player.position == Vec3::ZERO && player.shows.is_empty());
        }
        frame(&mut player, &world, &body);
        let spring = Show::Spring { at: Vec3::new(0.0, LEAP_EFFECT_UP, 0.0), facing: 0.0 };
        assert_eq!(std::mem::take(&mut player.shows), [spring, Show::Heard(SOUND_LEAP_EFFECT), Show::Heard(SOUND_LEAP)]);
        // Half-way she is half-way there, and as high as a thing thrown to come
        // down in two thirds of a second is after one: 54.5.
        for _ in 0..10 {
            frame(&mut player, &world, &body);
        }
        assert!((player.position - Vec3::new(0.0, 54.4814, 320.0)).length() < 1e-2, "{}", player.position);
        let Action::Leap(Leap { part, time, .. }) = player.action else { panic!("she should be in the air") };
        assert_eq!((part, time), (LeapPart::ComeDown, 10 * FULL_PACE));
        // Ten frames on she is there, and is off for the next place at once.
        for _ in 0..10 {
            frame(&mut player, &world, &body);
        }
        assert!(player.position == Vec3::new(0.0, 0.0, 640.0) && player.acting() && player.shows.is_empty());
        assert!(!player.flags.has(flag::TAKEN_ON));
        let Action::Leap(leap) = player.action else { panic!("she should be leaping on") };
        // The way up there is timed as one with five hundred more height to it,
        // and the way she is to face is the angle of the whole way from the way
        // ahead, though the place is straight ahead of her.
        let seconds = Vec3::new(0.0, 800.0, 400.0).length() / (32.0 * 30.0);
        assert_eq!(leap.part, LeapPart::Gather);
        assert!((leap.length - (seconds * A_SECOND as f32) as i32).abs() <= 1 && (leap.spring - leap.length / 5).abs() <= 1, "{leap:?}");
        assert!((player.target_facing - 0.6f32.asin()).abs() < 1e-5, "{}", player.target_facing);
        // She comes down there, something rises, and she lands for as long as
        // the animation she lands with is; then she stands.
        let mut frames = 0;
        while player.acting() {
            frame(&mut player, &world, &body);
            frames += 1;
            assert!(frames < 1000, "she should come to rest");
        }
        let airborne = 8 + (leap.length + FULL_PACE - 1) / FULL_PACE;
        assert_eq!(frames, airborne + 400);
        assert_eq!((player.position, player.status, player.action), (Vec3::new(0.0, 300.0, 1040.0), status::STAND, Action::None));
        assert!((player.facing() - 0.6f32.asin()).abs() < 1e-5 && player.taken.is_none());
        let landing = Show::Landing { at: Vec3::new(0.0, 300.0 + LEAP_EFFECT_UP, 1040.0), facing: player.facing() };
        assert!(player.shows.contains(&landing) && player.shows.iter().filter(|show| **show == Show::Heard(SOUND_LEAP_EFFECT)).count() == 2);

        // A place far below takes its fall and half a second; one far above and
        // nearly overhead a fifth of its height at her run.
        let leap_to = |from: Vec3, to: Vec3| {
            let mut player = Character::new(0, from, 0.0, &test_defaults());
            player.take_to(&[TakenTo { by: TakenBy::PlainLeap, line: [to, to] }], 32.0);
            let Action::Leap(leap) = player.action else { panic!("she should be leaping") };
            (leap.length as f32 / A_SECOND as f32, leap.spring)
        };
        let (down, spring) = leap_to(Vec3::new(0.0, 500.0, 0.0), Vec3::new(0.0, 0.0, 300.0));
        assert!((down - ((10.0f32 / 9.806_65).sqrt() + 0.5)).abs() < 1e-4 && spring == 0, "{down}");
        let (up, spring) = leap_to(Vec3::ZERO, Vec3::new(0.0, 1000.0, 100.0));
        assert!((up - 1000.0 * 0.2 / 960.0).abs() < 1e-4 && spring == 2560, "{up} {spring}");
        // Where she lands near the ground she is stood on it.
        let mut player = Character::new(0, Vec3::new(0.0, 30.0, 0.0), 0.0, &test_defaults());
        player.take_to(&[TakenTo { by: TakenBy::Leap, line: [Vec3::new(0.0, 20.0, 320.0); 2] }], 32.0);
        for _ in 0..30 {
            frame(&mut player, &world, &body);
        }
        assert!((player.position - Vec3::new(0.0, 0.0, 320.0)).length() < 1e-3, "{}", player.position);
    }

    #[test]
    fn a_trap_has_her_rush_and_fly_with_an_aura_about_her() {
        use crate::field::scene::{setting, TakenBy, TakenTo};
        let (world, body) = (floor(), body(40.0));
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        let at = |by: TakenBy, place: Vec3| TakenTo { by, line: [place, place] };
        let places = [at(TakenBy::Rush, Vec3::new(0.0, 0.0, 600.0)), at(TakenBy::Flight, Vec3::new(0.0, 1000.0, 600.0)), at(TakenBy::Leap, Vec3::new(0.0, 1000.0, 1200.0))];
        player.take_to(&places, 32.0);
        assert_eq!((player.action, player.status, player.move_status), (Action::Rush, status::RUSH, status::RUSH));
        assert_eq!(std::mem::take(&mut player.shows), [Show::Aura]);
        // She rushes until the place is within a hundred and twenty of her, which
        // at thirty-two a frame is sixteen frames on; the aura goes out, and
        // another comes as she flies on from where she is.
        let mut frames = 0;
        while player.action == Action::Rush {
            frame(&mut player, &world, &body);
            frames += 1;
            assert!(frames < 100, "she should get there");
        }
        assert_eq!((frames, player.position, player.action), (17, Vec3::new(0.0, 0.0, 512.0), Action::Flight));
        assert_eq!(std::mem::take(&mut player.shows), [Show::AuraGoes { let_go: false }, Show::Aura]);
        // She flies straight there, lying in the air, thirty-two a frame.
        frame(&mut player, &world, &body);
        let way = (Vec3::new(0.0, 1000.0, 600.0) - Vec3::new(0.0, 0.0, 512.0)).normalize();
        assert!((player.position - (Vec3::new(0.0, 0.0, 512.0) + way * 32.0)).length() < 1e-3);
        assert_eq!((player.get(setting::TURN_X), player.get(setting::TURN_Z), player.facing()), (90, 180, 0.0));
        let mut frames = 1;
        while player.action == Action::Flight {
            frame(&mut player, &world, &body);
            frames += 1;
            assert!(frames < 100, "she should get there");
        }
        // The thirty-second step would have taken her past it and left her no
        // nearer than ten: she is there. Upright again and turned half round,
        // she leaps on without gathering herself, to a sound, and comes down.
        assert_eq!((frames, player.position), (32, Vec3::new(0.0, 1000.0, 600.0)));
        assert_eq!((player.get(setting::TURN_X), player.get(setting::TURN_Z), player.facing()), (0, 0, std::f32::consts::PI));
        assert_eq!(std::mem::take(&mut player.shows), [Show::AuraGoes { let_go: true }, Show::Heard(SOUND_LEAP)]);
        let Action::Leap(leap) = player.action else { panic!("she should be leaping") };
        assert_eq!((leap.part, leap.spring, leap.length), (LeapPart::Spring, 0, 600 / 32 * FULL_PACE + FULL_PACE * 3 / 4));
        assert!(player.flags.has(flag::FROM_FLIGHT) && !player.flags.has(flag::TAKEN_ON));
        frame(&mut player, &world, &body);
        let Action::Leap(leap) = player.action else { panic!("she should be in the air") };
        assert_eq!((leap.part, leap.time), (LeapPart::ComeDown, FULL_PACE));
        assert!(player.shows.is_empty() && player.facing() == std::f32::consts::PI);

        // A rush that is the last of them stops for as long as the animation it
        // stops with plays, going on as it went.
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        player.take_to(&[at(TakenBy::Rush, Vec3::new(0.0, 0.0, 200.0))], 32.0);
        for _ in 0..3 {
            frame(&mut player, &world, &body);
        }
        assert_eq!(player.position.z, 96.0);
        frame(&mut player, &world, &body);
        assert_eq!((player.action, player.position.z), (Action::RushStop { time: 0 }, 96.0));
        let mut frames = 0;
        while player.acting() {
            frame(&mut player, &world, &body);
            frames += 1;
            assert!(frames < 1000, "she should come to rest");
        }
        assert_eq!((frames, player.status, player.action), (121, status::STAND, Action::None));
    }

    #[test]
    fn an_animation_set_playing_keeps_a_character_busy_until_it_is_through() {
        let (world, body) = (floor(), body(40.0));
        // How many frames it is busy for, once the animation has been named and
        // set playing as `start` does.
        let busy = |character: &mut Character| {
            let mut frames = 0;
            while character.acting() {
                frame(character, &world, &body);
                frames += 1;
                assert!(frames < 1000, "it should come to an end");
            }
            frames
        };
        // One of its own that does not loop: for as long as it is (the third is
        // thirty frames here), which is asked after as it plays; then it stands.
        let mut character = character();
        character.name_only(Some(Motion::Own(3)));
        assert_eq!(character.motion, None);
        character.play_motion(None);
        assert_eq!((character.motion, character.motions), (Some((Motion::Own(3), false)), 1));
        assert_eq!(busy(&mut character), 30);
        assert_eq!((character.motion, character.status, character.named), (None, status::STAND, Some(Motion::Own(3))));
        // At half its pace it takes twice as long.
        character.pace = FULL_PACE / 2;
        character.play_motion(None);
        assert_eq!(busy(&mut character), 60);
        character.pace = FULL_PACE;
        // One whose length is given, as an event's is: that long.
        character.name_motion(Some(Motion::Staged(2, 7)));
        character.play_motion(Some(45 * FULL_PACE));
        assert_eq!(busy(&mut character), 45);
        // One that keeps its pose is left in it.
        character.flags.set(flag::KEEPS_POSE, true);
        character.name_motion(Some(Motion::Own(2)));
        character.play_motion(None);
        assert_eq!(busy(&mut character), 20);
        assert_eq!(character.motion, Some((Motion::Own(2), false)));
        // One that loops goes on and keeps nobody waiting.
        character.set(crate::field::scene::setting::MOTION_LOOPS, 1);
        character.play_motion(None);
        assert_eq!((character.acting(), character.motion, character.motions), (false, Some((Motion::Own(2), true)), 5));
        // Played for so many frames it is busy for those, and is played once
        // through whatever it would do; held at a frame it is held until it is
        // next set playing.
        character.set(crate::field::scene::setting::MOTION_LOOPS, 0);
        character.hold_motion(60);
        character.play_motion_for(12);
        assert_eq!((character.held, character.motion), (None, Some((Motion::Own(2), false))));
        assert_eq!(busy(&mut character), 12);
        character.hold_motion(60);
        assert_eq!(character.held, Some(60));
        // An animation its resource has not got does not keep it long, and one
        // that is hidden is not kept at all.
        character.name_motion(Some(Motion::Own(0)));
        character.play_motion(None);
        assert_eq!(busy(&mut character), 4);
        character.flags.set(flag::HIDDEN, true);
        character.name_motion(Some(Motion::Own(3)));
        character.play_motion(None);
        assert!(!character.acting());
    }

    #[test]
    fn a_walk_goes_to_its_target_and_ends_there() {
        let (world, body) = (floor(), body(40.0));
        let mut character = character();
        character.target = Vec3::new(0.0, 0.0, 500.0);
        character.walk();
        assert!(character.acting());
        assert_eq!(character.status, status::RUN);
        let mut frames = 0;
        while character.acting() {
            frame(&mut character, &world, &body);
            frames += 1;
            assert!(frames < 100, "the walk should end");
        }
        // It stops within two frames' walk of the target, and stands.
        let left = character.target.distance(character.position);
        assert!(left < 64.0 && left > 0.0, "{left} from the target");
        assert_eq!(character.status, status::STAND);
        assert_eq!(character.action, Action::None);
        // 500 units at 32 a frame.
        assert_eq!(frames, 15);
    }

    #[test]
    fn a_walk_turns_towards_a_target_that_is_behind() {
        let (world, body) = (floor(), body(40.0));
        let mut character = character();
        character.target = Vec3::new(0.0, 0.0, -400.0);
        character.walk();
        frame(&mut character, &world, &body);
        // Facing away it does not move yet, but it has begun to turn.
        assert_eq!(character.position, Vec3::ZERO);
        assert!(character.facing().abs() > 1.0);
        for _ in 0..40 {
            frame(&mut character, &world, &body);
        }
        assert!(!character.acting());
        assert!(character.position.z < -330.0);
    }

    #[test]
    fn a_dash_goes_straight_on_until_it_is_stopped() {
        let (world, body) = (floor(), body(40.0));
        let mut character = character();
        character.mover.facing = std::f32::consts::FRAC_PI_2;
        character.target = Vec3::new(0.0, 0.0, 10.0);
        character.dash();
        for _ in 0..15 {
            frame(&mut character, &world, &body);
        }
        assert!(character.acting());
        assert!((character.position - Vec3::new(480.0, 0.0, 0.0)).length() < 1e-2);
        character.stop();
        frame(&mut character, &world, &body);
        assert!((character.position.x - 480.0).abs() < 1e-2);
    }

    #[test]
    fn a_slower_pace_makes_everything_take_longer() {
        let (world, body) = (floor(), body(40.0));
        let mut character = character();
        character.pace = FULL_PACE / 2;
        character.target_facing = 1.0;
        character.turn();
        let mut frames = 0;
        while character.acting() {
            frame(&mut character, &world, &body);
            frames += 1;
        }
        assert_eq!(frames, 68);
        assert!((character.facing() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn appearing_waits_below_the_ground_and_then_comes_up() {
        let (world, body) = (floor(), body(40.0));
        let mut character = character();
        character.position = Vec3::new(10.0, 0.0, 20.0);
        character.appear(150.0);
        assert_eq!(character.position.y, -150.0);
        for _ in 0..10 {
            frame(&mut character, &world, &body);
        }
        assert_eq!(character.position.y, -150.0);
        frame(&mut character, &world, &body);
        let after_one = character.position.y;
        assert!(after_one > -150.0 && after_one < -135.0);
        // Half way through the rise it is three quarters of the way up.
        for _ in 0..14 {
            frame(&mut character, &world, &body);
        }
        assert!((character.position.y + 37.5).abs() < 1e-3);
        // The ring it comes up through is there from the first, and is not told
        // to go until it is up.
        assert_eq!(character.shows, [Show::Ring { at: Vec3::new(10.0, 0.0, 20.0), facing: character.target_facing }]);
        for _ in 0..15 {
            frame(&mut character, &world, &body);
        }
        assert!(!character.acting());
        assert_eq!(character.position, Vec3::new(10.0, 0.0, 20.0));
        assert_eq!(character.shows.last(), Some(&Show::RingGoes));
    }

    #[test]
    fn one_hops_up_out_of_the_ground_in_smoke_and_another_drops_in_from_above() {
        let (world, body) = (floor(), body(40.0));
        let at = Vec3::new(10.0, 0.0, 20.0);
        let mut character = character();
        character.position = at;
        assert!(character.come_by(9, 150.0));
        assert_eq!((character.position.y, character.action), (-POP_UP_DEPTH, Action::PopUp { time: 0 }));
        assert_eq!(character.shows, [Show::Smoke { at, facing: character.target_facing }]);
        // Thrown up, it rises fast at first and is there nine frames on.
        let mut heights = Vec::new();
        for _ in 0..8 {
            frame(&mut character, &world, &body);
            heights.push(character.position.y);
        }
        assert!(heights.windows(2).all(|pair| pair[1] > pair[0] && pair[1] - pair[0] < 14.0), "{heights:?}");
        assert!((heights[0] + POP_UP_DEPTH - 13.25).abs() < 0.05 && heights[7] < 0.0 && heights[7] > -6.0, "{heights:?}");
        frame(&mut character, &world, &body);
        assert!(!character.acting() && character.position == at && character.shows.len() == 1);

        // The other falls a thousand in the same nine frames, faster and faster,
        // and something rises where it lands.
        let mut character = self::character();
        character.position = at;
        assert!(character.come_by(10, 150.0));
        assert_eq!((character.position.y, character.shows.len()), (DROP_HEIGHT, 0));
        let mut heights = vec![DROP_HEIGHT];
        for _ in 0..8 {
            frame(&mut character, &world, &body);
            heights.push(character.position.y);
        }
        let falls: Vec<f32> = heights.windows(2).map(|pair| pair[0] - pair[1]).collect();
        assert!(falls.windows(2).all(|pair| pair[1] > pair[0]) && (falls[0] - 106.75).abs() < 0.1, "{falls:?}");
        frame(&mut character, &world, &body);
        assert!(!character.acting() && character.position == at);
        assert_eq!(character.shows, [Show::Landing { at, facing: character.target_facing }]);
        // Any other number is not a way of coming.
        assert!(!character.come_by(11, 150.0) && !character.come_by(12, 150.0));
    }

    #[test]
    fn a_script_gives_a_character_a_face_at_once_or_over_some_frames() {
        use crate::field::scene::setting;
        let mut character = character();
        assert_eq!((character.expression, character.expressions), (None, 0));
        // Setting 116 is `set_face`: that face, now.
        character.set(setting::FACE, 3);
        assert_eq!((character.expression, character.expressions), (Some((3, 0)), 1));
        // Settings 117 and 118 only name a face and the frames; action 12 is
        // what has it go over to it (`shift_face`).
        character.set(setting::FACE_NEXT, 8);
        character.set(setting::FACE_FRAMES, 20);
        assert_eq!((character.expression, character.expressions), (Some((3, 0)), 1));
        character.shift_face();
        assert_eq!((character.expression, character.expressions), (Some((8, 20)), 2));
        // And back to the first face, in as many.
        character.set(setting::FACE_NEXT, 0);
        character.shift_face();
        assert_eq!((character.expression, character.expressions), (Some((0, 20)), 3));
    }

    #[test]
    fn a_script_stops_a_characters_time_and_gives_it_a_sign_and_moves_its_shadow() {
        use crate::field::scene::setting;
        let (world, body) = (floor(), body(40.0));
        let mut character = character();
        // Setting 24 has its time stand still: it has none of a frame, so
        // what it is playing (thirty frames long) does not go on until it is
        // let go again.
        character.name_only(Some(Motion::Own(3)));
        character.play_motion(None);
        assert_eq!((character.time(), character.pace_fraction()), (FULL_PACE, 1.0));
        character.set(setting::STOPPED, 1);
        assert_eq!((character.time(), character.pace_fraction()), (0, 0.0));
        for _ in 0..100 {
            frame(&mut character, &world, &body);
        }
        assert!(character.acting());
        character.set(setting::STOPPED, 0);
        assert_eq!(character.time(), FULL_PACE);
        let mut frames = 0;
        while character.acting() {
            frame(&mut character, &world, &body);
            frames += 1;
            assert!(frames < 1000, "it should come to an end");
        }
        assert_eq!(frames, 30);
        // Settings 78 and 122 are flags: whether it idles, and whether the tile
        // of one a mission is after stands by its mark.
        character.set(setting::IDLES, 0);
        character.set(setting::POINTED_OUT, 1);
        assert!(character.flags.has(flag::NO_IDLING) && character.flags.has(flag::POINTED_OUT));
        character.set(setting::IDLES, 1);
        character.set(setting::POINTED_OUT, 0);
        assert!(!character.flags.has(flag::NO_IDLING) && !character.flags.has(flag::POINTED_OUT));
        // Settings 80 to 82 put its shadow off its place: up, and along x and z.
        character.set(setting::SHADOW_UP, 1);
        character.set(setting::SHADOW_X, -20);
        character.set(setting::SHADOW_Z, -15);
        assert_eq!(character.shadow_offset, Vec3::new(-20.0, 1.0, -15.0));
        // Setting 92 gives it a sign where it stands, which stays there and
        // has a frame with its mark.
        character.position = Vec3::new(794.0, 0.0, -931.0);
        character.set(setting::SIGN, 1);
        character.position = Vec3::ZERO;
        character.set(setting::SIGN, 1);
        character.step_mark();
        assert_eq!(character.sign, Some((Vec3::new(794.0, 0.0, -931.0), 1)));
    }

    #[test]
    fn a_destroyed_character_bursts_and_is_hidden_or_goes_in_smoke_as_its_script_has_it() {
        let (world, body) = (floor(), body(40.0));
        // Most burst twenty-six frames in, once, and are gone five after.
        let mut character = character();
        character.destroy(0);
        assert!(character.shows.is_empty());
        for _ in 0..25 {
            frame(&mut character, &world, &body);
        }
        assert!(character.shows.is_empty() && character.acting());
        frame(&mut character, &world, &body);
        assert_eq!(character.shows, [Show::Burst]);
        for _ in 0..4 {
            frame(&mut character, &world, &body);
        }
        assert!(character.acting() && !character.hidden());
        frame(&mut character, &world, &body);
        assert!(!character.acting() && character.hidden());
        assert_eq!(character.shows, [Show::Burst]);

        // One that fades goes in smoke from the first, and takes fifty-five.
        let mut character = self::character();
        character.chr = FADING;
        character.destroy(DESTROY_FADES);
        assert_eq!(character.shows, [Show::Smoke { at: character.position, facing: character.facing() }]);
        assert_eq!(character.alpha, 1.0);
        for _ in 0..11 {
            frame(&mut character, &world, &body);
        }
        // There is less of it frame by frame: four fifths after eleven.
        assert!((character.alpha - 0.8).abs() < 1e-6);
        for _ in 11..54 {
            frame(&mut character, &world, &body);
        }
        assert!(character.acting() && !character.hidden());
        assert!((character.alpha - 1.0 / 55.0).abs() < 1e-6);
        frame(&mut character, &world, &body);
        assert!(!character.acting() && character.hidden());
        assert_eq!((character.shows.len(), character.alpha), (1, 0.0));
    }

    #[test]
    fn the_player_runs_while_her_status_is_the_one_she_moves_in() {
        let (world, body) = (floor(), body(40.0));
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        player.status = status::RUN;
        player.mover.direction = Vec3::Z;
        frame(&mut player, &world, &body);
        assert!((player.position.z - 32.0).abs() < 1e-3);
        player.status = status::STAND;
        frame(&mut player, &world, &body);
        assert!((player.position.z - 32.0).abs() < 1e-3);
    }

    #[test]
    fn she_pulls_up_after_a_run_and_the_stick_may_take_her_out_of_it() {
        let (world, body) = (floor(), body(40.0));
        let mut player = Character::new(0, Vec3::ZERO, 0.0, &test_defaults());
        player.status = status::RUN;
        player.mover.direction = Vec3::Z;
        // How long she has run is counted while she does: eleven frames are
        // enough for a stop to be a pull-up.
        for _ in 0..11 {
            frame(&mut player, &world, &body);
        }
        assert_eq!(player.ran, 11 * FULL_PACE);
        assert!(player.ran > BRAKE_AFTER);
        let stood = player.position;
        player.brake();
        assert_eq!((player.status, player.action, player.ran), (status::BRAKE, Action::Brake { time: 0 }, 0));
        // She goes nowhere, and from the fourteenth frame may be taken out of it.
        for _ in 0..13 {
            frame(&mut player, &world, &body);
        }
        assert!(player.acting() && !player.flags.has(flag::FREE_TO_GO));
        frame(&mut player, &world, &body);
        assert!(player.acting() && player.flags.has(flag::FREE_TO_GO));
        assert_eq!(player.position, stood);
        // Left to it she stands again when its animation has played: number 12,
        // which is 120 frames long here.
        for _ in 14..120 {
            frame(&mut player, &world, &body);
        }
        assert!(player.acting());
        frame(&mut player, &world, &body);
        assert!(!player.acting());
        assert_eq!((player.status, player.action), (status::STAND, Action::None));
        // Taken out of it, she is no longer at it; what begins next clears the
        // leave she had.
        player.brake();
        assert!(!player.flags.has(flag::FREE_TO_GO));
        player.end_action();
        assert!(!player.acting());
    }
}
