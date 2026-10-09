//! The enemies' bullets, by the classes of the bullet table, and the bullets of
//! her skills, which are of the same table.
//!
//! Classes 1 and 2 fly through the air (built at 0x883B4, run by 0x886F8): one of the
//! first kind goes straight at the point its shooter aimed at; one of the second
//! leaves at an angle and is guided towards the player. Either hits her if it passes
//! through her body. Class 0 is a ray that strikes at once along its line (0x87F8C),
//! and class 3 a beam that stays and strikes again and again (0x89C00). Class 4 is a
//! bomb, lobbed to the ground where it lies and then bursts (0x8A284).

use bevy::prelude::*;
use brs_engine::step::Stepped;

use super::actor::facing;
use super::effects::Look;
use super::tables::Shot;
use super::Battle;

/// The bullet classes of the table that this module has.
pub const RAY: i32 = 0;
pub const STRAIGHT: i32 = 1;
pub const GUIDED: i32 = 2;
pub const BEAM: i32 = 3;
pub const BOMB: i32 = 4;
pub const STRIKE: i32 = 5;

/// A bullet of the sixth class (built at 0x874EC, its frame 0x8AF40), which four of
/// the bosses have: nothing flies. Its look is put on the ground under what its
/// shooter aimed at, and so many sixtieths later whatever of her is within its reach
/// of that spot is hit, once; then it is gone, and what was seen of it plays out by
/// itself.
#[derive(Component, Clone, Debug)]
pub struct Strike {
    pub shot: Shot,
    pub owner: Entity,
    at: Vec3,
    age: f32,
}

impl Strike {
    /// The spot is on the ground under the point aimed at, a step towards +Z of
    /// it (0x8AFD4).
    pub fn new(shot: Shot, owner: Entity, aim: Vec3) -> Strike {
        Strike { shot, owner, at: Vec3::new(aim.x, 0.0, aim.z + 1.0), age: 0.0 }
    }

    /// Where its look is put: at the spot, turned to the point it is under
    /// (0x8B00C).
    pub fn frame(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(std::f32::consts::PI), self.at)
    }

    /// A frame after the one it was made on: its time runs, and it is true once
    /// that has come to the record's delay, which is when it lands and goes.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.age += dt;
        self.age >= self.shot.strike_delay
    }

    /// Where on a sphere it lands, if the sphere is within its reach (0x8B1C4).
    pub fn reaches(&self, centre: Vec3, radius: f32) -> Option<Vec3> {
        (self.at.distance(centre) <= self.shot.strike_reach + radius).then(|| centre + (self.at - centre).normalize_or_zero() * radius)
    }
}

/// Makes a strike: it has no model of its own, only its time.
pub fn spawn_strike(commands: &mut Commands, strike: Strike, frame: u32) {
    commands.spawn((Battle, strike, Fired(frame)));
}

/// How far a ray or a beam reaches (0x869B0, 0x89FB8).
const RAY_RANGE: f32 = 3000.0;
/// A beam first strikes this long after it appears, and is gone this long after its
/// time is up: both in sixtieths of a second (0x89E70, 0x8A20C). A bomb's burst is
/// gone as long after its own (0x8AEC0).
const BEAM_FIRST_STRIKE: f32 = 4.0;
const BEAM_FADE: f32 = 30.0;

/// A bomb comes down to this height and no lower (0x12950C).
const BOMB_FLOOR: f32 = 20.0;
/// The nearest and the furthest an enemy's bomb is thrown, whatever it was aimed at
/// (0x8A5B4, 0x8A5F8), and how far short of that its two hops end (0x8A630).
const BOMB_NEAREST: f32 = 750.0;
const BOMB_FURTHEST: f32 = 1500.0;
const BOMB_SHORT: [f32; 2] = [200.0, 100.0];
/// No hop takes less time than this, in sixtieths (0x8A6A8).
const BOMB_LEAST_TIME: f32 = 10.0;
/// The second hop's speed along the ground, and its height as a share of the
/// first's (0x8A7C0, 0x8A7D4).
const BOMB_SECOND_SPEED: f32 = 3.0;
const BOMB_SECOND_PEAK: f32 = 0.1;
/// The sound of its first landing (0x8A7BC).
pub const BOMB_BOUNCE_SOUND: i32 = 124;
/// The thickness of the bar that stands in for a beam's effect.
const BEAM_LOOK_WIDTH: f32 = 14.0;

/// Height above the player's standing place that bullets make for (0x88B68).
const AIM_HEIGHT: f32 = 75.0;
/// How far a bullet flies when its record gives it no life of its own (0x888CC).
const RANGE: f32 = 8000.0;
/// A bullet that leaves this box is gone (0x89174).
const BOUNDS: (Vec3, Vec3) = (Vec3::new(-2000.0, -500.0, -1600.0), Vec3::new(2000.0, 1000.0, 1000.0));
/// An enemy's bullet can hit the player only once it is past this, on her side of
/// the stage (0x897B4).
const HIT_PLANE: f32 = -1400.0;
/// The radius of the ball that stands in for a bullet with no effect for its look,
/// before its scale.
const LOOK_RADIUS: f32 = 14.0;

#[derive(Component)]
pub struct Projectile {
    shot: Shot,
    /// The enemy that fired it. A hit is that enemy's attack, and once the enemy is
    /// gone the bullet hits nothing.
    pub owner: Entity,
    start: Vec3,
    /// The point it makes for.
    target: Vec3,
    previous: Vec3,
    position: Vec3,
    /// Per sixtieth of a second.
    velocity: Vec3,
    /// Sixtieths of a second it has been flying or waiting, and has left.
    age: f32,
    life: f32,
    /// Its target is kept on the player.
    homing: bool,
    /// It has come level with its target and steers no more.
    passed: bool,
    /// The way its look is turned: from where it started to what it was fired
    /// at, whichever way it goes after that (0x8899C).
    facing: Quat,
}

/// A turn by `yaw` and then, in the turned frame, by `pitch`: both in degrees.
fn turn(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(pitch.to_radians())
}

/// The frame a flying bullet starts in: `from` with its axes brought to unit length,
/// moved and turned as the table says (0x88578).
fn start_frame(shot: &Shot, from: Transform) -> Transform {
    Transform { scale: Vec3::ONE, ..from }
        * Transform { translation: shot.offset, rotation: turn(shot.yaw, shot.pitch), scale: Vec3::ONE }
}

impl Projectile {
    /// A bullet of the kind `shot`, fired by `owner` at `aim`. `node` is the frame of
    /// the node of the shooter's model that the record names, if the model has it.
    ///
    /// The frame it is made on is its first, on which the original has it appear
    /// without moving: a new actor joins the end of the list being run (0xAE53C), so
    /// its first update comes on that same frame.
    pub fn new(shot: Shot, owner: Entity, node: Option<Transform>, aim: Vec3) -> Projectile {
        // Unless the record says it leaves from the node, its offset is from the
        // middle of the stage (0x88508).
        let from = node.filter(|_| shot.from_node).unwrap_or(Transform::IDENTITY);
        let frame = start_frame(&shot, from);
        let start = frame.translation;
        // A straight one heads for the point aimed at; a guided one leaves along its
        // frame turned once more (0x88880, 0x888E4).
        let direction = match shot.kind {
            STRAIGHT => (aim - start).normalize_or_zero(),
            _ => frame.rotation * turn(shot.launch_yaw, shot.launch_pitch) * Vec3::Z,
        };
        Projectile {
            shot,
            owner,
            start,
            target: aim,
            previous: start,
            position: start,
            velocity: direction * shot.speed,
            age: 0.0,
            life: if shot.life <= 0.0 { RANGE / shot.speed } else { shot.life },
            homing: shot.kind != STRAIGHT,
            passed: false,
            facing: facing(aim - start),
        }
    }

    /// The record it was fired from.
    pub fn shot(&self) -> &Shot {
        &self.shot
    }

    /// Where the bullet is, turned as its look is and with the size that is
    /// given (0x89304).
    pub fn transform(&self) -> Transform {
        Transform { translation: self.position, rotation: self.facing, scale: Vec3::splat(self.shot.look_scale()) }
    }

    /// One logic frame of flight after its first (0x88AD0). `place` is the standing
    /// place the player is at or on her way to. False once the bullet is spent.
    pub fn advance(&mut self, dt: f32, place: Vec3) -> bool {
        let shot = self.shot;
        let on_player = place + Vec3::Y * AIM_HEIGHT;
        self.age += dt;
        if self.age <= shot.delay {
            // Still waiting where it started: a straight one keeps turning to where
            // she is now.
            if shot.kind == STRAIGHT {
                self.target = on_player;
                self.velocity = (self.target - self.position).normalize_or_zero() * shot.speed;
            }
            return true;
        }
        self.life -= dt;
        if self.life <= 0.0 {
            return false;
        }

        // Its target follows the player until it has been guided for long enough, has
        // come far enough, or is nearly there.
        if self.homing {
            let done = shot.homing_time <= self.age
                || self.start.distance(self.position) > shot.homing_range
                || self.position.distance(self.target) < shot.homing_until;
            self.homing = !done;
        }
        if self.homing {
            self.target = on_player;
        }
        if shot.kind != STRAIGHT && !self.passed {
            let wanted = (self.target - self.position).normalize_or_zero() * shot.speed;
            self.velocity = self.velocity.lerp(wanted, (shot.homing * dt).min(1.0));
        }

        self.previous = self.position;
        self.position += self.velocity * dt;
        let (low, high) = BOUNDS;
        if self.position.cmplt(low).any() || self.position.cmpgt(high).any() {
            return false;
        }
        // Level with its target, it flies on as it is.
        if (self.position.z - self.target.z).abs() < shot.speed {
            self.passed = true;
            self.homing = false;
        }
        true
    }

    /// Whether the bullet, on its way this frame, touched the sphere of `radius`
    /// round `centre`, and if so where the hit is: the point of the sphere that
    /// is towards where the bullet came from, or towards where it has come to
    /// if it is only there that it is within reach (0x897A4, 0x89898, 0x898F0,
    /// 0x89AEC).
    pub fn touches(&self, centre: Vec3, radius: f32) -> Option<Vec3> {
        let reach = self.shot.radius + radius;
        if !(self.position.z < HIT_PLANE && passes_within(self.previous, self.position, reach, centre)) {
            return None;
        }
        let only_now = self.previous.distance(centre) >= reach && self.position.distance(centre) < reach;
        let towards = if only_now { self.position } else { self.previous };
        Some(centre + (towards - centre).normalize_or_zero() * radius)
    }
}

/// Whether a point carried from `from` to `to` comes within `reach` of `centre`: at
/// either end, or where the line between them is nearest to it.
fn passes_within(from: Vec3, to: Vec3, reach: f32, centre: Vec3) -> bool {
    if from.distance(centre) < reach || to.distance(centre) < reach {
        return true;
    }
    let length = from.distance(to);
    if length <= 0.0 {
        return false;
    }
    let along = (to - from) / length;
    let ahead = along.dot(centre - from);
    if centre.distance(from + along * ahead) > reach {
        return false;
    }
    // The nearest point of the line has to lie on the stretch flown. Off either end
    // of it only an exact touch of that end is left to count.
    match ahead / length {
        share if share < 0.0 => centre.distance(from) <= reach,
        share if share <= 1.0 => true,
        _ => centre.distance(to) <= reach,
    }
}

/// A ray: it strikes at once, whatever lies on its line (bullet class 0).
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub shot: Shot,
    /// The enemy that fired it.
    pub owner: Entity,
    from: Vec3,
    /// A unit vector.
    direction: Vec3,
}

impl Ray {
    /// A ray from the node of its shooter that the record names (or the stage's own
    /// frame, if the model has no such node) towards `aim` (0x86854). Unlike a flying
    /// bullet's, its frame is the node's as it stands, scale and all (0x86614), and
    /// whatever the record says of leaving from the node.
    pub fn new(shot: Shot, owner: Entity, node: Option<Transform>, aim: Vec3) -> Ray {
        let from = node.unwrap_or(Transform::IDENTITY).transform_point(shot.offset);
        Ray { shot, owner, from, direction: (aim - from).normalize_or_zero() }
    }

    /// Where the ray first meets the sphere of `radius` round `centre`, if it does
    /// within its reach (0x869DC).
    pub fn meets(&self, centre: Vec3, radius: f32) -> Option<Vec3> {
        let apart = centre - self.from;
        if apart.length_squared() <= radius * radius {
            return Some(self.from);
        }
        let ahead = self.direction.dot(apart);
        if ahead <= 0.0 || ahead - RAY_RANGE > radius {
            return None;
        }
        let inside = radius * radius - (apart.length_squared() - ahead * ahead);
        if inside < 0.0 {
            return None;
        }
        let along = ahead - inside.sqrt();
        (along <= RAY_RANGE).then(|| self.from + self.direction * along)
    }

    /// How far along the ray a point of it is.
    pub fn reach(&self, point: Vec3) -> f32 {
        point.distance(self.from)
    }

    /// Where it ends when it meets nothing: as far as it reaches (0x88154).
    pub fn end(&self) -> Vec3 {
        self.from + self.direction * RAY_RANGE
    }
}

/// The rays her skills have fired this frame, each with its skill (0x7C158). A ray
/// of hers is the same thing as an enemy's (0x86854) with her for its owner: it
/// strikes at once, in the enemies' part of the frame, the one enemy whose body it
/// meets first (0x87B5C, 0x88008).
#[derive(Resource, Default)]
pub struct SkillRays(pub Vec<(i32, Ray)>);

/// A beam: a ray that stays where it was fired, striking along its line every so
/// often until its time is up (bullet class 3).
#[derive(Component)]
pub struct Beam {
    pub ray: Ray,
    /// Sixtieths of a second since it appeared, and since it last struck.
    age: f32,
    since_strike: f32,
    /// How long after the last strike the next one comes.
    interval: f32,
    /// Its time is up: it strikes no more, and its effect dies away.
    spent: bool,
}

impl Beam {
    /// A beam from the shooter's node towards `aim`, both as they are now (0x89C78).
    pub fn new(shot: Shot, owner: Entity, node: Option<Transform>, aim: Vec3) -> Beam {
        Beam { ray: Ray::new(shot, owner, node, aim), age: 0.0, since_strike: 0.0, interval: BEAM_FIRST_STRIKE, spent: false }
    }

    /// Where its look is: at its start, along its line. One with no effect for
    /// its look has a bar along its line stand in.
    pub fn transform(&self) -> Transform {
        let (ray, looks) = (&self.ray, self.ray.shot.effect > 0);
        Transform {
            translation: if looks { ray.from } else { ray.from + ray.direction * (RAY_RANGE * 0.5) },
            rotation: facing(ray.direction),
            scale: Vec3::ONE,
        }
    }

    /// One logic frame after its first (0x89EAC). Returns whether it strikes along
    /// its line now, and whether it is still there afterwards.
    pub fn advance(&mut self, dt: f32) -> (bool, bool) {
        let shot = self.ray.shot;
        self.age += dt;
        self.since_strike += dt;
        // Its effect is told to die away when its time is up; a beam with no effect
        // of its own is never told, and strikes on until it goes.
        if shot.effect > 0 && shot.beam_time <= self.age {
            self.spent = true;
        }
        let strikes = !self.spent && self.interval <= self.since_strike;
        if strikes {
            self.interval = shot.beam_tick;
            self.since_strike = 0.0;
        }
        (strikes, self.age <= shot.beam_time + BEAM_FADE)
    }

    /// Whether the beam is still to be seen.
    pub fn shown(&self) -> bool {
        !self.spent
    }
}

/// The upward speed that takes a thing `peak` above where it is on its way to being
/// `end` above it after `time` (0x87264): the larger root of
/// v² - 4·peak/time·v + 4·peak·end/time² = 0. Where that has no two roots, which is
/// when it would have to start above its own peak, the original's solver hands back
/// nothing (0xA813C), and it sets off level.
fn launch_speed(peak: f32, end: f32, time: f32) -> f32 {
    let (b, c) = (-4.0 * peak / time, 4.0 * peak * end / (time * time));
    let under_root = b * b - 4.0 * c;
    if under_root > 0.0 { (under_root.sqrt() - b) / 2.0 } else { 0.0 }
}

/// How far along a bomb is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lob {
    /// Thrown this frame: it has yet to work out its way.
    Thrown,
    /// On its first hop, and on the short second one.
    First,
    Second,
    /// Lying where it came down, until its fuse runs out.
    Lying,
    Burst,
}

/// What a bomb did in a frame, for whoever shows and sounds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BombStep {
    /// It came down from its first hop.
    pub bounced: bool,
    /// It burst this frame.
    pub burst: bool,
    /// Its burst strikes whatever is within reach now.
    pub strikes: bool,
    /// It is still there afterwards.
    pub there: bool,
}

/// A bomb: lobbed in two hops to just short of where it was thrown, it lies there
/// until its fuse runs out and then bursts, and the burst strikes whatever is within
/// its reach every so often until its time is up (bullet class 4, 0x8A284).
#[derive(Component)]
pub struct Bomb {
    pub shot: Shot,
    /// The enemy that threw it.
    pub owner: Entity,
    /// The skill of hers it is of, if it is hers and no enemy's (0x87454): then it
    /// is the enemies its burst strikes.
    pub of_skill: Option<i32>,
    target: Vec3,
    position: Vec3,
    /// Per sixtieth of a second, and what is added to its upward part each sixtieth.
    velocity: Vec3,
    gravity: f32,
    /// Where its second hop is to end.
    rest: Vec3,
    lob: Lob,
    /// Sixtieths since it was thrown; from its burst on, since that.
    clock: f32,
    bursts_at: f32,
    /// Time since the burst last struck, and how long after that it strikes again.
    since_strike: f32,
    interval: f32,
    /// Its time is up: it strikes no more, and its effect dies away.
    spent: bool,
    /// The way its look is turned, and its burst's: from where it was thrown to
    /// what it was thrown at (0x8A4C0).
    facing: Quat,
}

impl Bomb {
    /// A bomb thrown by `owner` at `aim`, from the node of its model that the record
    /// names: the node's frame as it stands, like a ray's (0x8A308). The frame it is
    /// made on it only appears.
    pub fn new(shot: Shot, owner: Entity, node: Option<Transform>, aim: Vec3) -> Bomb {
        let position = node.unwrap_or(Transform::IDENTITY).transform_point(shot.offset);
        Bomb {
            shot,
            owner,
            of_skill: None,
            target: aim,
            position,
            velocity: Vec3::ZERO,
            gravity: 0.0,
            rest: position,
            lob: Lob::Thrown,
            clock: 0.0,
            bursts_at: 0.0,
            since_strike: 0.0,
            interval: 0.0,
            spent: false,
            facing: facing(aim - position),
        }
    }

    /// A bomb of hers, thrown for a skill at where she aims (0x797A8).
    pub fn hers(shot: Shot, skill: i32, node: Option<Transform>, aim: Vec3) -> Bomb {
        Bomb { of_skill: Some(skill), ..Bomb::new(shot, Entity::PLACEHOLDER, node, aim) }
    }

    /// Where it is: the middle of its burst, once it has burst.
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Where it is, and how big: its look's effect and its burst's are as they
    /// are, whatever size its record gives (0x8A918). With no effect for one of
    /// them a ball stands in: of the record's size for the bomb, as wide as it
    /// reaches for the burst.
    pub fn transform(&self) -> Transform {
        let scale = match self.lob {
            Lob::Burst if self.shot.blast_effect > 0 => 1.0,
            Lob::Burst => self.shot.blast / LOOK_RADIUS,
            _ if self.shot.effect > 0 => 1.0,
            _ => self.shot.look_scale(),
        };
        Transform { translation: self.position, rotation: self.facing, scale: Vec3::splat(scale) }
    }

    /// Whether the burst is still to be seen.
    pub fn shown(&self) -> bool {
        !self.spent
    }

    /// Sets it off on a hop to above `to`: along the ground at `speed`, or slower if
    /// that would take no time at all, rising to `peak` above the ground on the way
    /// to coming down onto it (0x8A68C, 0x8A7C0).
    fn hop(&mut self, to: Vec3, speed: f32, peak: f32) {
        let flat = Vec3::new(to.x - self.position.x, 0.0, to.z - self.position.z);
        let far = flat.length();
        let time = (far / speed).max(BOMB_LEAST_TIME);
        let down = -self.position.y;
        self.velocity = flat.normalize_or_zero() * (far / time);
        self.velocity.y = launch_speed(peak - self.position.y, down, time);
        // What brings it down by that much in that time (0x872C4).
        self.gravity = (down - self.velocity.y * time) / (0.5 * time * time);
    }

    /// One frame of a hop (0x8A734). True as it comes down.
    fn falls(&mut self, dt: f32) -> bool {
        self.velocity.y += self.gravity * dt;
        self.position += self.velocity * dt;
        if self.position.y > BOMB_FLOOR {
            return false;
        }
        self.position.y = BOMB_FLOOR;
        true
    }

    /// Stays where it has come down, its fuse running.
    fn lie(&mut self) {
        self.velocity = Vec3::ZERO;
        self.bursts_at = self.clock + self.shot.fuse;
        self.lob = Lob::Lying;
    }

    /// One logic frame after its first.
    pub fn advance(&mut self, dt: f32) -> BombStep {
        let shot = self.shot;
        let mut step = BombStep { there: true, ..BombStep::default() };
        if self.lob == Lob::Burst {
            self.clock += dt;
            self.since_strike += dt;
            // As with a beam, it is its effect that is told to die away: a burst
            // with none strikes on until it goes.
            if shot.blast_effect > 0 && shot.beam_time <= self.clock {
                self.spent = true;
            }
        } else {
            self.clock += dt;
            if self.lob == Lob::Thrown {
                // An enemy's is thrown no nearer and no further than it can be,
                // along the ground towards what was aimed at; it comes down a
                // little short of that, and its second hop ends less short. One
                // of hers is thrown the nearest it can be, wherever she aims
                // (0x8A5D4).
                let flat = Vec3::new(self.target.x - self.position.x, 0.0, self.target.z - self.position.z);
                let far = if self.of_skill.is_some() { BOMB_NEAREST } else { flat.length().min(BOMB_FURTHEST).max(BOMB_NEAREST) };
                let along = flat.normalize_or_zero();
                self.rest = self.position + along * (far - BOMB_SHORT[1]);
                self.hop(self.position + along * (far - BOMB_SHORT[0]), shot.speed, shot.peak);
                self.lob = Lob::First;
            }
            if self.lob == Lob::First && self.falls(dt) {
                // Down from the first hop it sets off on the second at once.
                step.bounced = true;
                self.hop(self.rest, BOMB_SECOND_SPEED, shot.peak * BOMB_SECOND_PEAK);
                self.lob = Lob::Second;
                if self.falls(dt) {
                    self.lie();
                }
            } else if self.lob == Lob::Second && self.falls(dt) {
                self.lie();
            }
            if self.lob != Lob::Lying || self.clock < self.bursts_at {
                return step;
            }
            // It bursts, and the burst strikes on that same frame (0x8A96C).
            (self.lob, self.clock, self.interval) = (Lob::Burst, 0.0, 0.0);
            step.burst = true;
        }
        step.strikes = !self.spent && self.interval <= self.since_strike;
        if step.strikes {
            self.interval = shot.beam_tick;
            self.since_strike = 0.0;
        }
        step.there = self.clock <= shot.beam_time + BEAM_FADE;
        step
    }

    /// Whether its burst reaches the sphere of `radius` round `centre`, and if
    /// so where the hit is: the point of the sphere that is towards the bomb
    /// (0x8ADC8).
    pub fn reaches(&self, centre: Vec3, radius: f32) -> Option<Vec3> {
        (self.position.distance(centre) <= self.shot.blast + radius).then(|| centre + (self.position - centre).normalize_or_zero() * radius)
    }
}

/// What an enemy's bullet looks like. The original draws the effect its record names;
/// until the effect packs are decoded a glowing ball stands in, a bar for a beam, and
/// a ball you can see through for a bomb's burst.
#[derive(Resource)]
pub struct ProjectileLook {
    mesh: Handle<Mesh>,
    beam: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    burst: Handle<StandardMaterial>,
    /// A bullet of hers, in the colour her own shots stand in with.
    hers: Handle<StandardMaterial>,
}

impl ProjectileLook {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> ProjectileLook {
        ProjectileLook {
            mesh: meshes.add(Sphere::new(LOOK_RADIUS)),
            beam: meshes.add(Cuboid::new(BEAM_LOOK_WIDTH, BEAM_LOOK_WIDTH, RAY_RANGE)),
            material: materials.add(StandardMaterial {
                base_color: brs_engine::texture::values(Color::srgb(1.0, 0.55, 0.3)),
                unlit: true,
                fog_enabled: false,
                ..default()
            }),
            burst: materials.add(StandardMaterial {
                base_color: brs_engine::texture::values(Color::srgba(0.6, 0.9, 0.35, 0.3)),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                fog_enabled: false,
                ..default()
            }),
            hers: materials.add(StandardMaterial {
                base_color: brs_engine::texture::values(Color::srgb(0.75, 0.95, 1.0)),
                unlit: true,
                fog_enabled: false,
                ..default()
            }),
        }
    }

    /// What a bomb is seen as once it has burst: its burst's effect, or the
    /// stand-in for one that has none.
    pub fn burst(&self, made: &mut EntityCommands, effect: i32) {
        match Look::of(effect, 1.0) {
            Some(look) => made.insert(look).remove::<Mesh3d>(),
            None => made.insert((Mesh3d(self.mesh.clone()), MeshMaterial3d(self.burst.clone()))).remove::<Look>(),
        };
    }
}

/// What the lock point a bullet of hers was fired at has come to, this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lock {
    /// It is where it is, on an enemy that lives.
    At(Vec3),
    /// Its enemy has no HP left.
    Dead,
    /// There is none: she had locked on to nothing, or it is gone.
    Gone,
}

/// How near the way it goes has to be to the way it should go for a guided bullet
/// of hers to turn faster, and to speed up (0x88F0C, 0x88F50): the distance between
/// the two directions, each of length one.
const TURNS_FASTER_WITHIN: f32 = 0.5;
const SPEEDS_UP_WITHIN: f32 = 0.1;
/// What its share of the turn grows by a frame, what its speed grows by a sixtieth,
/// and the speed past which the original puts that figure, by mistake it seems, in
/// the share (0x88F8C).
const TURN_GROWTH: f32 = 0.1;
const SPEED_GROWTH: f32 = 3.0;
const TOP_SPEED: f32 = 200.0;

/// A flying bullet of hers: what one of her skills fires (0x86E5C). It is made as
/// an enemy's of classes 1 and 2 is, and what it does on its way is her side of
/// the same code (0x88CFC, 0x88EA8, 0x89064, 0x893E0):
///
/// - a straight one is not kept to the stage's box;
/// - a guided one makes for the lock point the cursor was on as it was fired, for
///   as long as that is there and its enemy lives, and steers by a share of its
///   own that grows as it lines up, as its speed does;
/// - it hits the enemy whose body it first sweeps through, as her own shots do.
///   One that pierces goes on, and hits each enemy once.
#[derive(Component)]
pub struct SkillShot {
    shot: Shot,
    /// The skill it is of.
    pub skill: i32,
    /// The point it makes for, and the lock point that is kept on.
    target: Vec3,
    lock: Option<(Entity, usize)>,
    previous: Vec3,
    position: Vec3,
    /// Per sixtieth of a second.
    velocity: Vec3,
    /// Sixtieths of a second it has been there, and has left to fly.
    age: f32,
    life: f32,
    /// A guided one's: its speed, the share of the turn towards its target that it
    /// takes each sixtieth, whether its target is kept on the lock point, and
    /// whether it steers no more.
    speed: f32,
    share: f32,
    homing: bool,
    passed: bool,
    /// The enemies it has gone through, which it does not hit again (0x867CC).
    pub struck: Vec<Entity>,
    /// The way its look is turned: from where it started to what it was fired
    /// at (0x8899C).
    facing: Quat,
}

impl SkillShot {
    /// A bullet of the kind `shot`, fired at `aim` from `node`: the frame of the
    /// node of her model that the record names. `lock` is the enemy and the lock
    /// point of it the cursor is on. Like an enemy's, it only appears on the frame
    /// it is made on.
    pub fn new(shot: Shot, skill: i32, node: Option<Transform>, aim: Vec3, lock: Option<(Entity, usize)>) -> SkillShot {
        let from = node.filter(|_| shot.from_node).unwrap_or(Transform::IDENTITY);
        let frame = start_frame(&shot, from);
        let start = frame.translation;
        let direction = match shot.kind {
            STRAIGHT => (aim - start).normalize_or_zero(),
            _ => frame.rotation * turn(shot.launch_yaw, shot.launch_pitch) * Vec3::Z,
        };
        SkillShot {
            shot,
            skill,
            target: aim,
            lock,
            previous: start,
            position: start,
            velocity: direction * shot.speed,
            age: 0.0,
            life: if shot.life <= 0.0 { RANGE / shot.speed } else { shot.life },
            speed: shot.speed,
            share: shot.homing,
            homing: shot.kind != STRAIGHT,
            passed: false,
            struck: Vec::new(),
            facing: facing(aim - start),
        }
    }

    /// The record it was fired from.
    pub fn shot(&self) -> &Shot {
        &self.shot
    }

    /// The enemy and the lock point of it that it was fired at.
    pub fn lock(&self) -> Option<(Entity, usize)> {
        self.lock
    }

    /// Where the bullet is, with the size its look is given; one with no effect
    /// for its look has a ball the size of its sphere stand in.
    pub fn transform(&self) -> Transform {
        let scale = if self.shot.effect > 0 { self.shot.look_scale() } else { self.shot.radius / LOOK_RADIUS };
        Transform { translation: self.position, rotation: self.facing, scale: Vec3::splat(scale) }
    }

    /// One logic frame of flight after its first (0x88AD0), with what has become
    /// of its lock point. False once it is spent.
    pub fn advance(&mut self, dt: f32, lock: Lock) -> bool {
        self.age += dt;
        if self.age <= self.shot.delay {
            return true;
        }
        self.life -= dt;
        if self.life <= 0.0 {
            return false;
        }
        let guided = self.shot.kind != STRAIGHT;
        // Its target is kept on the lock point while that is there and its enemy
        // lives; after that it is where it last was.
        if self.homing {
            match lock {
                Lock::At(place) => self.target = place,
                Lock::Dead | Lock::Gone => self.homing = false,
            }
        }
        if guided && !self.passed {
            let wanted = (self.target - self.position).normalize_or_zero();
            let off = self.velocity.normalize_or_zero().distance(wanted);
            if off <= TURNS_FASTER_WITHIN {
                self.share = (self.share + TURN_GROWTH).min(1.0);
            }
            if off <= SPEEDS_UP_WITHIN {
                self.speed += dt * SPEED_GROWTH;
                if self.speed > TOP_SPEED {
                    self.share = TOP_SPEED;
                }
            }
            self.velocity = self.velocity.lerp(wanted * self.speed, (self.share * dt).min(1.0));
        }

        self.previous = self.position;
        self.position += self.velocity * dt;
        let (low, high) = BOUNDS;
        if guided && (self.position.cmplt(low).any() || self.position.cmpgt(high).any()) {
            return false;
        }
        // With its enemy dead it flies on as it is.
        match lock {
            Lock::At(_) => {}
            Lock::Dead => (self.passed, self.homing) = (true, false),
            Lock::Gone => self.homing = false,
        }
        true
    }

    /// The stretch it flew this frame, on which it can have hit.
    pub fn flown(&self) -> (Vec3, Vec3) {
        (self.previous, self.position)
    }
}

/// The logic frame a bullet was fired on. On that frame it only appears: a new actor
/// joins the end of the list being run (0xAE53C), so its first update comes that
/// same frame, and what that update does is done here as the bullet is made. It
/// moves, and can hit, from the frame after.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired(pub u32);

/// What a bullet is seen as: the effect its record names (0x12940C), or the
/// stand-in for one that names none.
fn dress(made: &mut EntityCommands, effect: i32, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) {
    match Look::of(effect, 1.0) {
        Some(look) => made.insert(look),
        None => made.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()))),
    };
}

/// Puts a bomb into the battle, on logic frame `frame`.
pub fn spawn_bomb(commands: &mut Commands, look: &ProjectileLook, bomb: Bomb, frame: u32) {
    let at = bomb.transform();
    let material = if bomb.of_skill.is_some() { &look.hers } else { &look.material };
    let effect = bomb.shot.effect;
    let mut made = commands.spawn((Battle, Fired(frame), Stepped::new(at), at, Visibility::Inherited, bomb));
    dress(&mut made, effect, &look.mesh, material);
}

/// Puts a beam into the battle, on logic frame `frame`.
pub fn spawn_beam(commands: &mut Commands, look: &ProjectileLook, beam: Beam, frame: u32) {
    let at = beam.transform();
    let effect = beam.ray.shot.effect;
    let mut made = commands.spawn((Battle, beam, Fired(frame), Stepped::new(at), at, Visibility::Inherited));
    dress(&mut made, effect, &look.beam, &look.material);
}

/// Puts a bullet into the battle, on logic frame `frame`.
pub fn spawn(commands: &mut Commands, look: &ProjectileLook, projectile: Projectile, frame: u32) {
    let at = projectile.transform();
    let effect = projectile.shot.effect;
    let mut made = commands.spawn((Battle, projectile, Fired(frame), Stepped::new(at), at, Visibility::Inherited));
    dress(&mut made, effect, &look.mesh, &look.material);
}

/// Puts a bullet of hers into the battle, on logic frame `frame`.
pub fn spawn_skill_shot(commands: &mut Commands, look: &ProjectileLook, shot: SkillShot, frame: u32) {
    let at = shot.transform();
    let effect = shot.shot.effect;
    let mut made = commands.spawn((Battle, shot, Fired(frame), Stepped::new(at), at, Visibility::Inherited));
    dress(&mut made, effect, &look.mesh, &look.hers);
}

/// Where a bullet leaves from, which is where the effect at its muzzle is put.
/// For one that flies that is the frame it starts in (0x887F0). For a ray, a
/// beam and a bomb it is the node's frame as it stands, its sizes and all, moved
/// and turned as the table says (0x86784, 0x89DB0, 0x8A450).
pub fn muzzle(shot: &Shot, node: Option<Transform>) -> Mat4 {
    match shot.kind {
        STRAIGHT | GUIDED => start_frame(shot, node.filter(|_| shot.from_node).unwrap_or(Transform::IDENTITY)).to_matrix(),
        _ => {
            let own = Transform { translation: shot.offset, rotation: turn(shot.yaw, shot.pitch), scale: Vec3::ONE };
            node.unwrap_or(Transform::IDENTITY).to_matrix() * own.to_matrix()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Charge Shot's bullet, and the first of Tracer Gun's three, as the bullet
    /// table has them.
    fn charge_shot() -> Shot {
        Shot { id: 10, kind: STRAIGHT, speed: 120.0, radius: 50.0, scale: 1.0, from_node: true, attack: 10, pierces: true, ..Shot::default() }
    }

    fn tracer_shot() -> Shot {
        Shot {
            id: 33,
            node: -1,
            kind: GUIDED,
            offset: Vec3::new(0.0, 65.0, 0.0),
            speed: 25.0,
            launch_yaw: 60.0,
            homing: 0.05,
            homing_until: 1.0,
            radius: 25.0,
            scale: 1.0,
            from_node: true,
            attack: 33,
            next: 34,
            ..Shot::default()
        }
    }

    #[test]
    fn a_bullet_of_hers_flies_straight_at_the_point_aimed_at_for_as_long_as_it_has() {
        let muzzle = Transform::from_xyz(100.0, 120.0, -1500.0);
        let mut shot = SkillShot::new(charge_shot(), 1, Some(muzzle), Vec3::new(100.0, 120.0, 0.0), None);
        // The frame it is made on it is only there.
        assert_eq!(shot.flown(), (muzzle.translation, muzzle.translation));
        assert!(shot.advance(2.0, Lock::Gone));
        assert_eq!(shot.flown(), (Vec3::new(100.0, 120.0, -1500.0), Vec3::new(100.0, 120.0, -1260.0)));
        // Its stand-in is as big as the sphere it hits with.
        assert_eq!(shot.transform().scale, Vec3::splat(50.0 / LOOK_RADIUS));
        // It has 8000 to fly at 240 a frame, and is not kept to the stage's box,
        // which an enemy's bullet would have left at 1000.
        let mut frames = 1;
        while shot.advance(2.0, Lock::Gone) {
            frames += 1;
        }
        assert_eq!(frames, 33);
        assert_eq!(shot.flown().1, Vec3::new(100.0, 120.0, -1500.0 + 33.0 * 240.0));
    }

    #[test]
    fn a_guided_bullet_of_hers_turns_to_what_she_locked_on_to_and_speeds_up_as_it_lines_up() {
        // She stands at her place facing the middle of the stage, with an enemy
        // locked on to ahead of her and a little to the side.
        let her = Transform::from_xyz(0.0, 0.0, -1500.0);
        let enemy = Vec3::new(200.0, 150.0, 0.0);
        let lock = Some((Entity::PLACEHOLDER, 0));
        let mut shot = SkillShot::new(tracer_shot(), 14, Some(her), enemy, lock);
        assert_eq!(shot.lock(), lock);
        // It starts above her feet and leaves sixty degrees to one side.
        assert_eq!(shot.flown().0, Vec3::new(0.0, 65.0, -1500.0));
        assert!((shot.velocity.length() - 25.0).abs() < 1e-3);
        assert!((shot.velocity.normalize().dot(Vec3::Z) - 0.5).abs() < 1e-4 && shot.velocity.x > 0.0);
        // At first it takes a tenth of the turn a frame, at the speed it left at.
        assert!(shot.advance(2.0, Lock::At(enemy)));
        assert_eq!((shot.share, shot.speed), (0.05, 25.0));
        // Once it goes nearly the way it should, the share grows by a tenth a
        // frame; once it goes that way, it speeds up, six a frame.
        let mut frames = 1;
        while shot.share == 0.05 {
            assert!(shot.advance(2.0, Lock::At(enemy)));
            frames += 1;
        }
        assert!((3..12).contains(&frames), "{frames}");
        let off = |shot: &SkillShot| shot.velocity.normalize().distance((enemy - shot.position).normalize());
        assert!(off(&shot) < 0.5);
        while shot.speed == 25.0 {
            assert!(shot.advance(2.0, Lock::At(enemy)));
        }
        assert!(off(&shot) < 0.1 && shot.speed == 31.0 && shot.share > 0.05);
        // It follows the lock point as that moves.
        let moved = enemy + Vec3::new(-300.0, 0.0, 0.0);
        for _ in 0..3 {
            assert!(shot.advance(2.0, Lock::At(moved)));
        }
        assert_eq!(shot.target, moved);
        assert!(off(&shot) < 0.5);
        // With the enemy dead it makes for where the point last was, and steers no
        // more: it goes on as it goes, until it leaves the stage's box.
        assert!(shot.advance(2.0, Lock::Dead));
        assert!(shot.passed && !shot.homing && shot.target == moved);
        let velocity = shot.velocity;
        let mut last = shot.flown().1;
        while shot.advance(2.0, Lock::Dead) {
            assert_eq!(shot.velocity, velocity);
            last = shot.flown().1;
        }
        assert!(last.z > 0.0 && last.z <= 1000.0, "{last}");

        // Fired with nothing locked on to, it makes for the point she aimed at.
        let mut shot = SkillShot::new(tracer_shot(), 14, Some(her), enemy, None);
        assert!(shot.advance(2.0, Lock::Gone));
        assert!(!shot.homing && !shot.passed && shot.target == enemy);
        for _ in 0..30 {
            shot.advance(2.0, Lock::Gone);
        }
        assert!(shot.speed > 25.0);

        // Past its top speed the share is given the speed's figure, which is as
        // much as to say that it turns all the way at once.
        let mut shot = SkillShot::new(Shot { launch_yaw: 0.0, ..tracer_shot() }, 14, Some(her), Vec3::new(0.0, 65.0, 900.0), None);
        shot.speed = 199.0;
        assert!(shot.advance(2.0, Lock::Gone));
        assert_eq!((shot.speed, shot.share), (205.0, 200.0));
        assert!(shot.advance(2.0, Lock::Gone));
        assert_eq!(shot.share, 200.0);
    }

    /// The Hornet's plain shot, as the bullet table has it.
    fn hornet_shot() -> Shot {
        Shot { id: 300, kind: STRAIGHT, speed: 100.0, radius: 50.0, scale: 1.0, from_node: true, ..Shot::default() }
    }

    /// The Hornet's beam, as the bullet table has it.
    fn hornet_beam() -> Shot {
        Shot { id: 320, kind: BEAM, effect: 203_005, beam_time: 30.0, beam_tick: 8.0, from_node: true, ..Shot::default() }
    }

    /// The Hornet's volley, flown as a guided bullet.
    fn guided_shot() -> Shot {
        Shot {
            id: 310,
            kind: GUIDED,
            from_node: true,
            speed: 70.0,
            launch_yaw: 45.0,
            homing: 0.05,
            homing_time: 10_000.0,
            homing_range: 10_000.0,
            homing_until: 750.0,
            radius: 50.0,
            scale: 1.0,
            ..Shot::default()
        }
    }

    const PLACE: Vec3 = Vec3::new(395.0, 0.0, -1447.0);

    fn fire(shot: Shot, from: Vec3, aim: Vec3) -> Projectile {
        Projectile::new(shot, Entity::PLACEHOLDER, Some(Transform::from_translation(from)), aim)
    }

    #[test]
    fn a_straight_shot_flies_at_the_point_aimed_at() {
        let (from, aim) = (Vec3::new(0.0, 200.0, 0.0), PLACE + Vec3::Y * 75.0);
        let mut projectile = fire(hornet_shot(), from, aim);
        assert!(projectile.advance(2.0, PLACE));
        // 100 a sixtieth is 200 a frame, along the line to the aim.
        assert!((projectile.position.distance(from) - 200.0).abs() < 1e-3);
        assert!((projectile.position - from).normalize().dot((aim - from).normalize()) > 0.99999);
        // The player moving away does not turn it.
        let elsewhere = Vec3::new(-395.0, 0.0, -1447.0);
        let before = projectile.velocity;
        assert!(projectile.advance(2.0, elsewhere));
        assert_eq!(projectile.velocity, before);
    }

    #[test]
    fn a_shot_starts_at_its_node_moved_and_turned_as_the_table_says() {
        // A node at (100, 50, 0) facing +X, and a shot that leaves 10 ahead of it.
        let node = Transform {
            translation: Vec3::new(100.0, 50.0, 0.0),
            rotation: Quat::from_rotation_y(90f32.to_radians()),
            scale: Vec3::splat(3.0),
        };
        let shot = Shot { offset: Vec3::new(0.0, 0.0, 10.0), ..guided_shot() };
        let projectile = Projectile::new(shot, Entity::PLACEHOLDER, Some(node), Vec3::ZERO);
        // The node's scale is left out of it.
        assert!(projectile.position.distance(Vec3::new(110.0, 50.0, 0.0)) < 1e-4, "{}", projectile.position);
        // A guided shot leaves along the node's Z turned by the launch yaw: 45
        // degrees on from +X, towards -Z.
        let wanted = Vec3::new(1.0, 0.0, -1.0).normalize() * 70.0;
        assert!(projectile.velocity.distance(wanted) < 1e-3, "{}", projectile.velocity);

        // A record that does not leave from the node measures its offset from the
        // middle of the stage, as does one whose node the model lacks.
        let loose = Shot { from_node: false, ..shot };
        assert_eq!(Projectile::new(loose, Entity::PLACEHOLDER, Some(node), Vec3::ZERO).position, shot.offset);
        assert_eq!(Projectile::new(shot, Entity::PLACEHOLDER, None, Vec3::ZERO).position, shot.offset);
    }

    #[test]
    fn a_ray_strikes_the_first_thing_on_its_line_within_reach() {
        let from = Vec3::new(0.0, 200.0, 0.0);
        let body = PLACE + Vec3::Y * 90.0;
        let ray = Ray::new(Shot { kind: RAY, ..hornet_shot() }, Entity::PLACEHOLDER, Some(Transform::from_translation(from)), body);
        // Aimed at her middle, it meets the near side of her sphere.
        let met = ray.meets(body, 45.0).unwrap();
        assert!((met.distance(body) - 45.0).abs() < 1e-2);
        assert!(met.distance(from) < body.distance(from));
        // Had she stepped to her other place it would pass her by.
        let elsewhere = Vec3::new(-395.0, 90.0, -1447.0);
        assert!(ray.meets(elsewhere, 45.0).is_none());
        // Nothing behind it is met, nor anything out of reach.
        assert!(ray.meets(from - ray.direction * 500.0, 45.0).is_none());
        assert!(ray.meets(from + ray.direction * 3050.0, 45.0).is_none());
        assert!(ray.meets(from + ray.direction * 3040.0, 45.0).is_some());
        // What it starts inside of is struck where it starts.
        assert_eq!(ray.meets(from + Vec3::X * 20.0, 45.0), Some(from));

        // Its offset is measured in the node's frame as it stands: turned, and scaled
        // with a model that is. The record's word on leaving from the node is not asked.
        let node = Transform {
            translation: Vec3::new(100.0, 50.0, 0.0),
            rotation: Quat::from_rotation_y(90f32.to_radians()),
            scale: Vec3::splat(3.0),
        };
        let shot = Shot { kind: RAY, offset: Vec3::new(0.0, 0.0, 10.0), from_node: false, ..hornet_shot() };
        let ray = Ray::new(shot, Entity::PLACEHOLDER, Some(node), body);
        assert!(ray.from.distance(Vec3::new(130.0, 50.0, 0.0)) < 1e-4, "{}", ray.from);
        assert_eq!(Ray::new(shot, Entity::PLACEHOLDER, None, body).from, shot.offset);
    }

    #[test]
    fn a_beam_strikes_every_few_frames_until_its_time_is_up() {
        let mut beam = Beam::new(hornet_beam(), Entity::PLACEHOLDER, Some(Transform::IDENTITY), Vec3::NEG_Z * 1500.0);
        let mut strikes = Vec::new();
        let mut frames = 0;
        loop {
            frames += 1;
            let (strikes_now, there) = beam.advance(2.0);
            if strikes_now {
                strikes.push(frames);
            }
            if !there {
                break;
            }
        }
        // First after 4 sixtieths, then every 8, and not once its 30 are up.
        assert_eq!(strikes, vec![2, 6, 10, 14]);
        assert!(!beam.shown());
        // It lingers another 30 while its effect dies away.
        assert_eq!(frames, 31);

        // A beam with no effect to die away strikes for as long as it is there.
        let mut bare = Beam::new(Shot { effect: 0, ..hornet_beam() }, Entity::PLACEHOLDER, None, Vec3::NEG_Z);
        let steps = (0..40).map(|_| bare.advance(2.0)).take_while(|(_, there)| *there);
        assert_eq!(steps.filter(|(strikes, _)| *strikes).count(), 8);
    }

    /// The bomb one enemy lobs, as the bullet table has it.
    #[test]
    fn a_strike_lands_under_what_was_aimed_at_when_its_time_has_come() {
        // The third boss's, as the table has it: fifty-five sixtieths, a reach of 200.
        let shot = Shot { id: 1910, kind: STRIKE, strike_delay: 55.0, strike_reach: 200.0, ..Shot::default() };
        let mut strike = Strike::new(shot, Entity::PLACEHOLDER, Vec3::new(395.0, 75.0, -1447.0));
        // Its spot is on the ground, a step on from under the point.
        assert_eq!(strike.frame().transform_point3(Vec3::ZERO), Vec3::new(395.0, 0.0, -1446.0));
        // Twenty-seven frames go by with nothing, and on the next it lands.
        for frame in 1..=27 {
            assert!(!strike.advance(2.0), "frame {frame}");
        }
        assert!(strike.advance(2.0));
        // On her if the middle of her body is within its reach and her own radius of
        // the spot: 75 up where she stood, and not when she has gone to her other place.
        let body = Vec3::new(395.0, 75.0, -1447.0);
        let at = strike.reaches(body, 45.0).unwrap();
        assert!((at.distance(body) - 45.0).abs() < 1e-3 && at.y < body.y);
        assert!(strike.reaches(body + Vec3::X * 300.0, 45.0).is_none());
    }

    fn lobbed_bomb() -> Shot {
        Shot {
            id: 410,
            kind: BOMB,
            speed: 20.0,
            scale: 1.0,
            beam_time: 270.0,
            beam_tick: 45.0,
            peak: 550.0,
            fuse: 60.0,
            blast: 200.0,
            blast_effect: 998_018,
            blast_sound: -1,
            from_node: true,
            ..Shot::default()
        }
    }

    fn on_ground(from: Vec3, to: Vec3) -> f32 {
        Vec3::new(to.x - from.x, 0.0, to.z - from.z).length()
    }

    #[test]
    fn a_lobbed_thing_rises_to_its_peak_and_comes_down_where_it_should() {
        // Level ground: up at four times the height over the time, and down as fast.
        let (peak, time) = (100.0, 20.0);
        let up = launch_speed(peak, 0.0, time);
        assert_eq!(up, 20.0);
        let gravity = (0.0 - up * time) / (0.5 * time * time);
        assert_eq!(gravity, -2.0);
        assert_eq!(up * 10.0 + 0.5 * gravity * 100.0, peak);
        // Thrown from a height it still tops out at the peak, and ends that far down.
        let (peak, end) = (400.0, -150.0);
        let up = launch_speed(peak, end, time);
        let gravity = (end - up * time) / (0.5 * time * time);
        let top = -up / gravity;
        assert!((up * top + 0.5 * gravity * top * top - peak).abs() < 1e-2);
        assert!(top > 0.0 && top < time);
        // From above its own peak there is no such arc: it sets off level and falls.
        assert_eq!(launch_speed(-50.0, -200.0, time), 0.0);
    }

    #[test]
    fn a_bomb_hops_twice_to_short_of_its_target_then_lies_and_bursts() {
        let from = Vec3::new(0.0, 150.0, 0.0);
        let aim = PLACE + Vec3::Y * 75.0;
        let far = on_ground(from, aim);
        assert!(far > BOMB_NEAREST && far < BOMB_FURTHEST);
        let mut bomb = Bomb::new(lobbed_bomb(), Entity::PLACEHOLDER, Some(Transform::from_translation(from)), aim);
        assert_eq!(bomb.position(), from);

        // The first hop: over the top, down to just above the ground about 200 short,
        // along the line to what was aimed at. It is moved a whole frame at a time,
        // its upward speed losing the frame's worth before it is used: so it tops
        // out lower than the record's 550 by about what it set off with, and comes
        // down a little sooner than the arc was laid for.
        let up = launch_speed(550.0 - from.y, -from.y, (far - 200.0) / 20.0);
        let mut highest = from.y;
        let mut frames = 0;
        let bounce = loop {
            let step = bomb.advance(2.0);
            frames += 1;
            highest = highest.max(bomb.position().y);
            assert!(!step.burst && !step.strikes && step.there);
            if step.bounced {
                break bomb.position();
            }
            assert!(frames < 200, "it never came down");
        };
        assert!((highest - (550.0 - up)).abs() < 3.0, "topped out at {highest}");
        // The frame it comes down on it is already off on its second hop.
        assert!(bounce.y > BOMB_FLOOR && bounce.y < 35.0, "it is {} up as it bounces", bounce.y);
        let out = on_ground(from, bounce);
        assert!(out > far - 270.0 && out < far - 195.0, "came down {out} out of {far}");
        let line = Vec3::new(aim.x - from.x, 0.0, aim.z - from.z).normalize();
        assert!(Vec3::new(bounce.x, 0.0, bounce.z).normalize().dot(line) > 0.9999);

        // The second: low and slow, ending about 100 short, where it stays.
        let mut highest = 0.0f32;
        let mut before = bounce;
        let mut landed = 0;
        loop {
            let step = bomb.advance(2.0);
            frames += 1;
            assert!(!step.bounced && step.there);
            // It strikes only once it has burst, and at once then.
            assert_eq!(step.strikes, step.burst);
            highest = highest.max(bomb.position().y);
            if step.burst {
                break;
            }
            if bomb.position() == before && landed == 0 {
                // The frame before this one it came to rest.
                landed = frames - 1;
            }
            before = bomb.position();
            assert!(frames < 400, "it never burst");
        }
        let rest = bomb.position();
        assert!(highest < 60.0 && highest > 30.0, "the second hop topped out at {highest}");
        assert_eq!(rest.y, BOMB_FLOOR);
        let out = on_ground(from, rest);
        assert!(out > on_ground(from, bounce) && out < far - 99.0 && out > far - 140.0, "it lies {out} out");
        // The fuse is 60 sixtieths from coming to rest.
        assert_eq!(frames - landed, 30);
    }

    #[test]
    fn a_bomb_of_hers_is_thrown_the_nearest_a_bomb_can_be_wherever_she_aims() {
        // Star Comet's, as the table has it: slow, a fuse of ninety, a burst of
        // forty-five that strikes every sixteen.
        let comet = Shot { id: 40, speed: 5.0, peak: 300.0, fuse: 90.0, blast: 300.0, beam_time: 45.0, beam_tick: 16.0, blast_effect: 998_011, ..lobbed_bomb() };
        let from = Vec3::new(0.0, 120.0, -1400.0);
        let thrown = |aim: Vec3| {
            let mut bomb = Bomb::hers(comet, 18, Some(Transform::from_translation(from)), aim);
            assert_eq!(bomb.of_skill, Some(18));
            let (mut frames, mut strikes) = (0, Vec::new());
            let burst = loop {
                frames += 1;
                if bomb.advance(2.0).burst {
                    break frames;
                }
            };
            strikes.push(0);
            for frame in 1..40 {
                if bomb.advance(2.0).strikes {
                    strikes.push(frame);
                }
            }
            (bomb.position(), burst, strikes)
        };
        // Aimed at an enemy far off or at the ground at her feet, it comes to rest
        // as far along the ground from where it was thrown, towards what she
        // aimed at: a little short of the 650 its second hop is laid to, as
        // every bomb comes down a little early.
        let (far, burst, strikes) = thrown(Vec3::new(0.0, 200.0, 600.0));
        let went = on_ground(from, far);
        assert!((600.0..=650.0).contains(&went) && far.x == 0.0 && far.y == BOMB_FLOOR && far.z > from.z, "{far}");
        let (near, ..) = thrown(Vec3::new(0.0, 0.0, -1300.0));
        assert!(near.distance(far) < 1e-3, "{near}");
        let (aside, ..) = thrown(from + Vec3::new(300.0, -120.0, 400.0));
        assert!(aside.distance(Vec3::new(from.x + 0.6 * went, BOMB_FLOOR, from.z + 0.8 * went)) < 1e-2, "{aside}");
        // It takes a little under four seconds to burst: a hop of 550 at five a
        // sixtieth, one of 100 at three, and its fuse, in frames of two
        // sixtieths. The burst strikes three times.
        assert!((105..=117).contains(&burst), "{burst}");
        assert_eq!(strikes, vec![0, 8, 16]);
    }

    #[test]
    fn a_burst_strikes_what_is_in_reach_every_so_often_until_its_time_is_up() {
        let mut bomb = Bomb::new(lobbed_bomb(), Entity::PLACEHOLDER, None, Vec3::new(0.0, 0.0, -1000.0));
        let mut step = bomb.advance(2.0);
        while !step.burst {
            step = bomb.advance(2.0);
        }
        // It strikes as it bursts, and then every 45 sixtieths that have passed in
        // full, for as long as its 270 last.
        assert!(step.strikes && step.there);
        let mut strikes = vec![0];
        let mut frames = 0;
        loop {
            frames += 1;
            let step = bomb.advance(2.0);
            if step.strikes {
                strikes.push(frames);
            }
            if !step.there {
                break;
            }
        }
        assert_eq!(strikes, vec![0, 23, 46, 69, 92, 115]);
        assert!(!bomb.shown());
        // It lingers another 30 while its effect dies away.
        assert_eq!(frames, 151);

        // Its reach and the sphere's radius together are how near counts.
        let at = bomb.position();
        // The hit is on the side of the sphere that the bomb is on.
        let landed = bomb.reaches(at + Vec3::X * 244.0, 45.0).unwrap();
        assert!(landed.abs_diff_eq(at + Vec3::X * 199.0, 1e-3));
        assert!(bomb.reaches(at + Vec3::X * 246.0, 45.0).is_none());
        // The burst's effect is as it is, turned the way the bomb was thrown.
        assert_eq!(bomb.transform().scale, Vec3::ONE);
        assert!((bomb.transform().rotation * Vec3::Z).abs_diff_eq(Vec3::NEG_Z, 1e-6));

        // A burst with no effect to die away strikes for as long as it is there.
        let mut bare = Bomb::new(Shot { blast_effect: 0, ..lobbed_bomb() }, Entity::PLACEHOLDER, None, Vec3::new(0.0, 0.0, -1000.0));
        let mut strikes = 0;
        loop {
            let step = bare.advance(2.0);
            strikes += step.strikes as i32;
            if !step.there {
                break;
            }
        }
        assert_eq!(strikes, 7);
        assert!(bare.shown());
        // The ball that stands in for its burst is as wide as the burst reaches.
        assert_eq!(bare.transform().scale, Vec3::splat(200.0 / LOOK_RADIUS));
    }

    #[test]
    fn a_bullets_look_is_turned_to_what_it_was_fired_at_and_sized_by_its_record() {
        // A flying bullet's look faces the point it was fired at from where it
        // started, and keeps facing that way wherever it goes.
        let from = Transform::from_xyz(0.0, 100.0, 0.0);
        let aim = Vec3::new(300.0, 100.0, -400.0);
        let mut bullet = Projectile::new(Shot { scale: 2.5, ..hornet_shot() }, Entity::PLACEHOLDER, Some(from), aim);
        let way = (aim - from.translation).normalize();
        assert!((bullet.transform().rotation * Vec3::Z).abs_diff_eq(way, 1e-6));
        assert_eq!(bullet.transform().scale, Vec3::splat(2.5));
        assert!(bullet.advance(2.0, Vec3::new(-500.0, 0.0, -1500.0)));
        assert!((bullet.transform().rotation * Vec3::Z).abs_diff_eq(way, 1e-6));
        // A record that gives no size leaves the look as it is.
        let plain = Projectile::new(Shot { scale: 0.0, ..hornet_shot() }, Entity::PLACEHOLDER, Some(from), aim);
        assert_eq!(plain.transform().scale, Vec3::ONE);

        // A bomb's look is not sized by its record.
        let bomb = Bomb::new(Shot { effect: 998_017, scale: 2.5, ..lobbed_bomb() }, Entity::PLACEHOLDER, Some(from), aim);
        assert_eq!(bomb.transform().scale, Vec3::ONE);
        assert!((bomb.transform().rotation * Vec3::Z).abs_diff_eq(way, 1e-6));
    }

    #[test]
    fn the_effect_at_a_muzzle_is_put_by_the_frame_the_bullet_leaves_from() {
        let node = Transform { translation: Vec3::new(10.0, 20.0, 30.0), rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), scale: Vec3::splat(3.0) };
        let shot = Shot { offset: Vec3::new(0.0, 0.0, 5.0), ..hornet_shot() };
        // A flying bullet's start frame has the node's sizes left out of it.
        let flying = muzzle(&shot, Some(node));
        assert!(flying.transform_point3(Vec3::ZERO).abs_diff_eq(Vec3::new(15.0, 20.0, 30.0), 1e-4));
        assert!((flying.x_axis.truncate().length() - 1.0).abs() < 1e-6);
        // A beam's is the node's frame as it stands.
        let beam = muzzle(&Shot { kind: BEAM, ..shot }, Some(node));
        assert!(beam.transform_point3(Vec3::ZERO).abs_diff_eq(Vec3::new(25.0, 20.0, 30.0), 1e-4));
        assert!((beam.x_axis.truncate().length() - 3.0).abs() < 1e-5);
        // With no node it is the stage's own.
        assert!(muzzle(&Shot { kind: BOMB, ..shot }, None).transform_point3(Vec3::ZERO).abs_diff_eq(Vec3::new(0.0, 0.0, 5.0), 1e-6));
    }

    #[test]
    fn an_enemys_bomb_is_thrown_no_nearer_and_no_further_than_it_can_be() {
        let lands = |aim: Vec3| {
            let mut bomb = Bomb::new(lobbed_bomb(), Entity::PLACEHOLDER, None, aim);
            while !bomb.advance(2.0).burst {}
            on_ground(Vec3::ZERO, bomb.position())
        };
        // Aimed at something close it still goes most of 750 out; at something far,
        // most of 1500.
        let near = lands(Vec3::new(0.0, 0.0, -100.0));
        assert!(near > 610.0 && near < 651.0, "it lies {near} out");
        let far = lands(Vec3::new(0.0, 0.0, -3000.0));
        assert!(far > 1360.0 && far < 1401.0, "it lies {far} out");
    }

    #[test]
    fn a_waiting_shot_keeps_turning_to_the_player() {
        let shot = Shot { delay: 4.0, ..hornet_shot() };
        let from = Vec3::new(0.0, 200.0, 0.0);
        let mut projectile = fire(shot, from, PLACE + Vec3::Y * 75.0);
        let elsewhere = Vec3::new(-395.0, 0.0, -1447.0);
        // Two frames of waiting: it stays put and turns to where she is.
        for _ in 0..2 {
            assert!(projectile.advance(2.0, elsewhere));
            assert_eq!(projectile.position, from);
        }
        let to_her = (elsewhere + Vec3::Y * 75.0 - from).normalize();
        assert!(projectile.velocity.normalize().dot(to_her) > 0.99999);
        // Then it leaves, and turns no more.
        assert!(projectile.advance(2.0, PLACE));
        assert!((projectile.position - from).normalize().dot(to_her) > 0.99999);
    }

    #[test]
    fn a_shot_is_spent_when_its_life_or_the_stage_runs_out() {
        // With no life of its own, 8000 units' worth: 80 sixtieths at 100.
        let mut projectile = fire(Shot { speed: 10.0, life: 12.0, ..hornet_shot() }, Vec3::ZERO, Vec3::new(0.0, 0.0, -1500.0));
        let mut frames = 0;
        while projectile.advance(2.0, PLACE) {
            frames += 1;
        }
        // Twelve sixtieths are six frames, and on the sixth it goes instead of moving.
        assert_eq!(frames, 5);
        assert_eq!(projectile.position, Vec3::new(0.0, 0.0, -100.0));

        // A fast one is gone as it leaves the stage, well before 8000 units.
        let mut projectile = fire(hornet_shot(), Vec3::ZERO, Vec3::new(0.0, 0.0, -1500.0));
        let mut frames = 0;
        while projectile.advance(2.0, PLACE) {
            frames += 1;
        }
        assert_eq!(frames, 8);
        assert_eq!(projectile.position.z, -1800.0);
    }

    #[test]
    fn a_guided_shot_turns_towards_the_player_until_it_is_near() {
        let from = Vec3::new(0.0, 300.0, 0.0);
        let mut projectile = fire(guided_shot(), from, PLACE + Vec3::Y * 75.0);
        // It leaves 45 degrees off the stage's +Z, and each frame takes a tenth of
        // the turn towards her.
        let to_her = |projectile: &Projectile| {
            let wanted = (PLACE + Vec3::Y * 75.0 - projectile.position).normalize();
            projectile.velocity.normalize().dot(wanted)
        };
        let mut facing = to_her(&projectile);
        assert!(facing < 0.0);
        for _ in 0..10 {
            assert!(projectile.advance(2.0, PLACE));
            assert!(to_her(&projectile) > facing);
            facing = to_her(&projectile);
        }
        assert!(projectile.homing);

        // Once it is within 750 of her it no longer follows where she goes, though it
        // still steers for where she was.
        while projectile.homing {
            assert!(projectile.advance(2.0, PLACE));
        }
        assert!(projectile.position.distance(projectile.target) < 750.0);
        let elsewhere = Vec3::new(-395.0, 0.0, -1447.0);
        assert!(projectile.advance(2.0, elsewhere));
        assert_eq!(projectile.target, PLACE + Vec3::Y * 75.0);
        // Level with that point it stops steering altogether.
        while !projectile.passed {
            assert!(projectile.advance(2.0, elsewhere));
        }
        let before = projectile.velocity;
        projectile.advance(2.0, elsewhere);
        assert_eq!(projectile.velocity, before);
    }

    #[test]
    fn only_a_bullet_on_the_players_side_of_the_stage_can_hit() {
        let body = PLACE + Vec3::Y * 90.0;
        let mut projectile = fire(hornet_shot(), Vec3::new(0.0, 200.0, 0.0), body);
        let mut hit_on = None;
        for frame in 1..=10 {
            assert!(projectile.advance(2.0, PLACE));
            // A sphere it flies straight through, were it anywhere on its way.
            let on_its_way = projectile.position;
            if projectile.position.z >= HIT_PLANE {
                assert!(projectile.touches(on_its_way, 45.0).is_none());
            }
            if let Some(at) = projectile.touches(body, 45.0) {
                hit_on = Some((frame, at));
                break;
            }
        }
        // About 1500 units at 200 a frame. It lands on her body's sphere, on the
        // side the bullet came from: it had not reached her the frame before.
        let (frame, at) = hit_on.unwrap();
        assert_eq!(frame, 8);
        assert!((at.distance(body) - 45.0).abs() < 1e-3);
        assert!((at - body).dot(projectile.previous - body) > 0.0);
    }

    #[test]
    fn a_fast_bullet_hits_what_it_passes_between_two_frames() {
        let (from, to) = (Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -100.0));
        // Within reach of the line, between its ends.
        assert!(passes_within(from, to, 95.0, Vec3::new(94.0, 0.0, 0.0)));
        assert!(!passes_within(from, to, 95.0, Vec3::new(96.0, 0.0, 0.0)));
        // Within reach of an end.
        assert!(passes_within(from, to, 95.0, Vec3::new(0.0, 0.0, 190.0)));
        assert!(passes_within(from, to, 95.0, Vec3::new(0.0, 0.0, -190.0)));
        // Beside the line but beyond its ends.
        assert!(!passes_within(from, to, 95.0, Vec3::new(50.0, 0.0, 190.0)));
        assert!(!passes_within(from, to, 95.0, Vec3::new(0.0, 0.0, -196.0)));
        // A bullet that has not moved.
        assert!(!passes_within(from, from, 95.0, Vec3::new(96.0, 0.0, 100.0)));
        assert!(passes_within(from, from, 95.0, Vec3::new(94.0, 0.0, 100.0)));
    }
}
