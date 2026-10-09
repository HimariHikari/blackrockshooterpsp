//! The aim cursor and the camera it steers.
//!
//! The battle camera sits behind the player's right shoulder and is fixed to her
//! (0x8C334). The cursor moves inside a box in the middle of the screen; pushing it
//! against an edge of the box turns the camera instead (0xA1CC8, 0x8C640), and with
//! the stick let go it is drawn to a lock point near it. What the cursor points at in
//! the world is where shots go (0x989F8).

use bevy::prelude::*;

use super::actor::facing;
use super::tunables::{id, Tunables};
use crate::controls::{Pad, Seek};

/// The PSP's screen, which the cursor's coordinates and speeds are in.
pub const SCREEN: Vec2 = Vec2::new(480.0, 272.0);

const CURSOR_START: Vec2 = Vec2::new(240.0, 136.0);
/// The box the cursor stays in.
const CURSOR_MIN: Vec2 = Vec2::new(180.0, 76.0);
const CURSOR_MAX: Vec2 = Vec2::new(300.0, 196.0);
/// Pixels the cursor moves per sixtieth of a second at full deflection.
const CURSOR_SPEED: f32 = 7.5;
/// How many times faster than that it is drawn to a lock point.
const LOCK_PULL: f32 = 4.0;
/// A lock point draws the cursor from this many pixels away.
const LOCK_RANGE: f32 = 64.0;

// The camera's lens: a 35 mm focal length on a film back 1.417 inches wide.
const FOCAL_LENGTH: f32 = 35.0;
const FILM_WIDTH_INCHES: f32 = 1.417;

/// If the cursor points at nothing nearer, shots go to where its ray meets this plane
/// across the far side of the stage.
const FAR_PLANE_Z: f32 = 5000.0;

/// The camera's vertical field of view in radians: the lens above on the PSP's screen.
pub fn vertical_fov() -> f32 {
    brs_engine::camera::vertical_fov(FOCAL_LENGTH, FILM_WIDTH_INCHES, 1.0, SCREEN.x / SCREEN.y)
}

/// Something on screen the cursor can lock onto: one of an enemy's lock points.
#[derive(Clone, Copy)]
pub struct LockPoint {
    pub entity: Entity,
    /// The enemy's number among those that have joined the battle.
    pub number: usize,
    /// Which of the enemy's lock points.
    pub gauge: usize,
    /// In PSP screen pixels.
    pub screen: Vec2,
}

impl LockPoint {
    fn id(&self) -> (Entity, usize) {
        (self.entity, self.gauge)
    }
}

/// Something in the world a shot can be aimed at: a sphere round a point of an enemy.
#[derive(Clone, Copy)]
pub struct Target {
    pub centre: Vec3,
    pub radius: f32,
}

/// Where the camera is and what it looks at.
#[derive(Clone, Copy)]
pub struct View {
    pub eye: Vec3,
    pub look: Vec3,
}

impl View {
    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.eye).looking_at(self.look, Vec3::Y)
    }

    /// The ray from the camera through the point `screen` (in PSP pixels) of a picture
    /// `aspect` times as wide as it is high.
    pub fn ray(&self, screen: Vec2, aspect: f32) -> (Vec3, Vec3) {
        let forward = (self.look - self.eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        let half_height = (vertical_fov() * 0.5).tan();
        let across = (screen.x / SCREEN.x * 2.0 - 1.0) * half_height * aspect;
        let above = (1.0 - screen.y / SCREEN.y * 2.0) * half_height;
        (self.eye, (forward + right * across + up * above).normalize())
    }

    /// Where the world point `at` appears, in PSP pixels; `None` behind the camera.
    pub fn project(&self, at: Vec3, aspect: f32) -> Option<Vec2> {
        let forward = (self.look - self.eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        let to = at - self.eye;
        let depth = to.dot(forward);
        if depth <= 0.0 {
            return None;
        }
        let half_height = (vertical_fov() * 0.5).tan();
        let across = to.dot(right) / depth / (half_height * aspect);
        let above = to.dot(up) / depth / half_height;
        Some(Vec2::new((across + 1.0) * 0.5 * SCREEN.x, (1.0 - above) * 0.5 * SCREEN.y))
    }
}

#[derive(Resource)]
pub struct Aim {
    /// Where the cursor is on the PSP's screen.
    pub cursor: Vec2,
    /// Where it was a step ago, for drawing it between steps.
    pub previous_cursor: Vec2,
    /// How far pushing the cursor at the box's edges has tilted the camera down and
    /// turned it to the left, in radians.
    tilt: f32,
    turn: f32,
    /// The cursor has settled on a lock point.
    pub locked: bool,
    /// The lock point it has settled on: the enemy and which of its points.
    pub target: Option<(Entity, usize)>,
    /// No lock point draws it but the one it has settled on already (0x171308,
    /// which 0xA146C looks at): a skill of hers that fires a volley sets it so
    /// that the cursor can be swept over the enemies.
    pub free: bool,
    /// The camera as of the last step. The cursor's ray is cast from here: the original
    /// works its camera out when it draws, so its logic sees the one from the frame
    /// before.
    pub view: View,
}

impl Aim {
    pub fn new(tunables: &Tunables, position: Vec3, direction: Vec3) -> Aim {
        let mut aim = Aim {
            cursor: CURSOR_START,
            previous_cursor: CURSOR_START,
            tilt: tunables.float(id::CAMERA_TILT_START).to_radians(),
            turn: tunables.float(id::CAMERA_TURN_START).to_radians(),
            locked: false,
            target: None,
            free: false,
            view: View { eye: Vec3::ZERO, look: Vec3::Z },
        };
        aim.follow(tunables, position, direction);
        aim
    }

    /// How a scripted run that seeks pushes the stick: towards the one of
    /// `lock_points` it wants, and not at all once the cursor has settled on that
    /// or is near enough for its own pull to take it there. Nothing of the
    /// original's.
    pub fn seek(&self, lock_points: &[LockPoint], wanted: Seek, dt: f32) -> Vec2 {
        let distance = |point: &&LockPoint| point.screen.distance_squared(self.cursor);
        let nearest = |points: &mut dyn Iterator<Item = &LockPoint>| points.min_by(|a, b| distance(a).total_cmp(&distance(b))).copied();
        let of_enemy = match wanted {
            Seek::Enemy(number) => nearest(&mut lock_points.iter().filter(|point| point.number == number)),
            Seek::Nearest | Seek::Point(..) => None,
        };
        let Some(point) = of_enemy.or_else(|| nearest(&mut lock_points.iter())) else { return Vec2::ZERO };
        // The pull is to whatever is nearest, so one wanted in particular is steered
        // to until the cursor is on it.
        let near_enough = match (wanted, self.target) {
            (Seek::Enemy(_), target) if target != Some(point.id()) => 2.0,
            _ => LOCK_RANGE * 0.5,
        };
        if point.screen.distance(self.cursor) > near_enough {
            // Gently over the last stretch, so as to stop on it and not beyond.
            ((point.screen - self.cursor) / (CURSOR_SPEED * dt)).clamp_length_max(1.0)
        } else {
            Vec2::ZERO
        }
    }

    /// Moves the cursor by the pad, or towards a lock point (0xA1CC8). `lock_points` are
    /// the ones that can be locked onto right now.
    pub fn step(&mut self, pad: &Pad, dt: f32, tunables: &Tunables, lock_points: &[LockPoint]) {
        self.previous_cursor = self.cursor;

        // The point already locked onto stays the target; otherwise the nearest one
        // does, if it is close enough.
        let held = self.target.and_then(|target| lock_points.iter().find(|point| point.id() == target));
        let near = || {
            if self.free {
                return None;
            }
            let distance = |point: &&LockPoint| point.screen.distance_squared(self.cursor);
            let nearest = lock_points.iter().min_by(|a, b| distance(a).total_cmp(&distance(b)))?;
            (nearest.screen.distance(self.cursor) <= LOCK_RANGE).then_some(nearest)
        };
        let target = held.or_else(near);

        // The D-pad moves the cursor, or the stick if the D-pad is not used.
        let input = if pad.dpad != Vec2::ZERO { pad.dpad } else { pad.stick };
        let mut movement = Vec2::ZERO;
        if input != Vec2::ZERO {
            movement = input * CURSOR_SPEED * dt;
            self.locked = false;
        } else if let Some(point) = target {
            movement = (point.screen - self.cursor).clamp_length_max(CURSOR_SPEED * LOCK_PULL * dt);
            self.locked = true;
        } else {
            self.locked = false;
        }
        self.target = target.filter(|_| self.locked).map(LockPoint::id);

        // What would carry the cursor out of its box turns the camera instead.
        let moved = self.cursor + movement;
        self.cursor = moved.clamp(CURSOR_MIN, CURSOR_MAX);
        let overflow = moved - self.cursor;
        let angle = |pixels: f32| 2.0 * ((vertical_fov() * 0.5).tan() * pixels / SCREEN.y).atan();
        let degrees = |id: u32| tunables.float(id).to_radians();
        self.turn = (self.turn - angle(overflow.x)).clamp(degrees(id::CAMERA_TURN_MIN), degrees(id::CAMERA_TURN_MAX));
        self.tilt = (self.tilt + angle(overflow.y)).clamp(degrees(id::CAMERA_TILT_MIN), degrees(id::CAMERA_TILT_MAX));
    }

    /// Puts the camera where it belongs for a player standing at `position` and facing
    /// along `direction` (0x8C334): behind her and to one side, tilted and turned about a
    /// point above her feet.
    pub fn follow(&mut self, tunables: &Tunables, position: Vec3, direction: Vec3) {
        let pivot = tunables.float(id::CAMERA_PIVOT_HEIGHT);
        let offset = Vec3::new(
            tunables.float(id::CAMERA_X),
            tunables.float(id::CAMERA_Y) - pivot,
            tunables.float(id::CAMERA_Z),
        );
        let swing = facing(direction) * Quat::from_rotation_y(self.turn) * Quat::from_rotation_x(self.tilt);
        let eye = position + facing(direction) * (Vec3::Y * pivot) + swing * offset;
        let gaze = swing
            * Quat::from_rotation_y(tunables.float(id::CAMERA_YAW).to_radians())
            * Quat::from_rotation_x(tunables.float(id::CAMERA_PITCH).to_radians());
        self.view = View { eye, look: eye + gaze * Vec3::Z };
    }

    /// What the cursor points at in the world (0x989F8): the first target its ray
    /// passes through, else the ground, else a plane across the far side of the stage.
    pub fn point(&self, aspect: f32, targets: &[Target]) -> Option<Vec3> {
        let (origin, direction) = self.view.ray(self.cursor, aspect);
        // A target is hit where the ray passes closest to its centre, if that is
        // within its radius.
        let on_target = targets.iter().find_map(|target| {
            let nearest = origin + direction * direction.dot(target.centre - origin);
            (nearest.distance(target.centre) <= target.radius).then_some(nearest)
        });
        let on_ground = || {
            let along = -origin.y / direction.y;
            (direction.y.abs() > f32::EPSILON && along > 0.0).then(|| origin + direction * along)
        };
        let on_far_plane = || {
            let at = origin + direction * ((FAR_PLANE_Z - origin.z) / direction.z);
            (direction.z.abs() > f32::EPSILON && at.y > 0.0).then_some(at)
        };
        on_target.or_else(on_ground).or_else(on_far_plane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::button;

    /// The camera values of `btl_system.bin`.
    fn tunables() -> Tunables {
        Tunables::from_values(&[
            (id::CAMERA_TILT_MIN, 0.0),
            (id::CAMERA_TILT_MAX, 10.0),
            (id::CAMERA_TURN_MIN, -40.0),
            (id::CAMERA_TURN_MAX, 20.0),
            (id::CAMERA_PIVOT_HEIGHT, 100.0),
            (id::CAMERA_X, -145.0),
            (id::CAMERA_Y, 35.0),
            (id::CAMERA_Z, -265.0),
            (id::CAMERA_PITCH, -10.0),
            (id::CAMERA_YAW, 12.0),
            (id::CAMERA_TILT_START, 0.0),
            (id::CAMERA_TURN_START, 0.0),
        ])
    }

    const ASPECT: f32 = 480.0 / 272.0;

    fn aim() -> Aim {
        Aim::new(&tunables(), Vec3::new(0.0, 0.0, -1500.0), Vec3::Z)
    }

    #[test]
    fn the_lens_gives_the_field_of_view_the_original_uses() {
        // The original's fixed overview camera is given this angle outright.
        assert!((vertical_fov() - 0.567023).abs() < 1e-3, "{}", vertical_fov());
    }

    #[test]
    fn the_camera_sits_behind_her_right_shoulder_looking_up_and_inwards() {
        let aim = aim();
        // Facing +Z, her left is +X: the camera is on her right, low and behind.
        assert!(aim.view.eye.distance(Vec3::new(-145.0, 35.0, -1765.0)) < 1e-3, "{}", aim.view.eye);
        let gaze = aim.view.look - aim.view.eye;
        assert!(gaze.x > 0.0 && gaze.y > 0.0 && gaze.z > 0.9, "{gaze}");
        // So she appears in the left part of the picture, near the bottom.
        let feet = aim.view.project(Vec3::new(0.0, 0.0, -1500.0), ASPECT).unwrap();
        assert!(feet.x < 140.0 && feet.y > 250.0, "{feet}");
    }

    #[test]
    fn a_ray_through_a_projected_point_passes_through_it() {
        let aim = aim();
        let at = Vec3::new(200.0, 300.0, 0.0);
        let screen = aim.view.project(at, ASPECT).unwrap();
        let (origin, direction) = aim.view.ray(screen, ASPECT);
        let nearest = origin + direction * direction.dot(at - origin);
        assert!(nearest.distance(at) < 0.5, "{nearest}");
    }

    #[test]
    fn pushing_past_the_box_turns_the_camera_until_its_limit() {
        let tunables = tunables();
        let mut aim = aim();
        let right = Pad { dpad: Vec2::X, held: button::RIGHT, ..default() };
        // 60 pixels to the edge at 15 a step.
        for _ in 0..4 {
            aim.step(&right, 2.0, &tunables, &[]);
        }
        assert_eq!(aim.cursor.x, CURSOR_MAX.x);
        assert_eq!(aim.turn, 0.0);
        aim.step(&right, 2.0, &tunables, &[]);
        // To the right is a negative turn.
        let expected = -2.0 * ((vertical_fov() * 0.5).tan() * 15.0 / 272.0).atan();
        assert!((aim.turn - expected).abs() < 1e-6);
        for _ in 0..100 {
            aim.step(&right, 2.0, &tunables, &[]);
        }
        assert!((aim.turn - (-40f32).to_radians()).abs() < 1e-6);
        assert_eq!(aim.tilt, 0.0);
    }

    #[test]
    fn a_free_cursor_is_drawn_to_a_lock_point_nearby() {
        let tunables = tunables();
        let mut aim = aim();
        let point = LockPoint { entity: Entity::PLACEHOLDER, number: 0, gauge: 0, screen: CURSOR_START + Vec2::new(40.0, -30.0) };
        aim.step(&Pad::default(), 2.0, &tunables, &[point]);
        assert!(aim.locked);
        // 50 pixels away, at 60 a step.
        assert!(aim.cursor.distance(point.screen) < 1e-4);
        assert_eq!(aim.target, Some((point.entity, 0)));

        // Moving the cursor lets go of it.
        aim.step(&Pad { stick: Vec2::X, ..default() }, 2.0, &tunables, &[point]);
        assert!(!aim.locked);
        assert_eq!(aim.target, None);

        // One too far away is left alone.
        let mut aim = self::aim();
        let far = LockPoint { entity: Entity::PLACEHOLDER, number: 0, gauge: 0, screen: CURSOR_START + Vec2::new(60.0, 30.0) };
        aim.step(&Pad::default(), 2.0, &tunables, &[far]);
        assert!(!aim.locked);
        assert_eq!(aim.cursor, CURSOR_START);

        // While a volley of hers has the cursor free, none draws it that it is not
        // on already; one it has settled on it stays on.
        let mut aim = self::aim();
        aim.free = true;
        aim.step(&Pad::default(), 2.0, &tunables, &[point]);
        assert!(!aim.locked && aim.cursor == CURSOR_START);
        aim.free = false;
        aim.step(&Pad::default(), 2.0, &tunables, &[point]);
        aim.free = true;
        aim.step(&Pad::default(), 2.0, &tunables, &[point]);
        assert!(aim.locked && aim.target == Some((point.entity, 0)));
    }

    #[test]
    fn a_scripted_run_steers_the_cursor_onto_the_enemy_it_wants() {
        let tunables = tunables();
        let mut aim = aim();
        let entity = Entity::PLACEHOLDER;
        let near = LockPoint { entity, number: 0, gauge: 0, screen: CURSOR_START + Vec2::new(20.0, 0.0) };
        let other = LockPoint { entity, number: 1, gauge: 1, screen: CURSOR_START + Vec2::new(-52.0, 31.0) };
        let points = [near, other];
        // The nearest needs no pushing: the cursor's own pull takes it there.
        assert_eq!(aim.seek(&points, Seek::Nearest, 2.0), Vec2::ZERO);
        // The other is pushed for until the cursor has settled on it, and no longer.
        let mut steps = 0;
        while aim.target != Some(other.id()) {
            let stick = aim.seek(&points, Seek::Enemy(1), 2.0);
            aim.step(&Pad { stick, ..default() }, 2.0, &tunables, &points);
            steps += 1;
            assert!(steps < 12, "the cursor is at {} after {steps} steps", aim.cursor);
        }
        assert!(aim.cursor.distance(other.screen) < 1e-3);
        assert_eq!(aim.seek(&points, Seek::Enemy(1), 2.0), Vec2::ZERO);
        // An enemy that cannot be locked onto leaves the nearest thing to seek.
        assert_eq!(aim.seek(&points, Seek::Enemy(7), 2.0), Vec2::ZERO);
        let far = LockPoint { screen: CURSOR_MIN, ..near };
        let pushed = aim.seek(&[far], Seek::Enemy(7), 2.0);
        assert!(pushed.distance((far.screen - aim.cursor).normalize()) < 1e-5, "pushed {pushed}");
    }

    #[test]
    fn the_cursor_points_at_a_target_then_the_ground_then_the_far_plane() {
        let mut aim = aim();
        // Straight ahead of the camera the view rises, so the ray ends on the far plane.
        let far = aim.point(ASPECT, &[]).unwrap();
        assert!((far.z - FAR_PLANE_Z).abs() < 1e-2 && far.y > 0.0, "{far}");

        // A target on the ray is hit at the ray's closest approach to its centre.
        let centre = aim.view.eye + (far - aim.view.eye) * 0.25 + Vec3::X * 30.0;
        let hit = aim.point(ASPECT, &[Target { centre, radius: 50.0 }]).unwrap();
        assert!(hit.distance(centre) <= 30.0 + 1e-2 && hit.z < 0.0, "{hit}");

        // With the camera tilted down, the bottom of the box points at the ground.
        aim.tilt = 10f32.to_radians();
        aim.follow(&tunables(), Vec3::new(0.0, 0.0, -1500.0), Vec3::Z);
        aim.cursor = CURSOR_MAX;
        let ground = aim.point(ASPECT, &[]).unwrap();
        assert!(ground.y.abs() < 1e-2 && ground.z > -1765.0, "{ground}");
    }
}
