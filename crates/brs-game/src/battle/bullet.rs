//! The player's shots (the original's `BtAssultBullet`, built at 0x87684): straight
//! lines from the muzzle to where the cursor pointed when they left, each a sphere
//! that hits the first enemy it sweeps through (0x8B310).

use bevy::prelude::*;
use brs_engine::step::Stepped;

use super::actor::facing;
use super::effects::Look;
use super::projectile::Fired;
use crate::battle::Battle;

/// How far a bullet flies before it disappears.
const RANGE: f32 = 8000.0;
/// A bullet is not drawn until it is this old, in sixtieths of a second (0x87A2C):
/// by then it has left the muzzle.
const HIDDEN_FOR: f32 = 2.0;
/// The radius of a bullet's collision sphere (0x98DF4).
pub const RADIUS: f32 = 70.0;

#[derive(Component)]
pub struct Bullet {
    /// Per sixtieth of a second.
    velocity: Vec3,
    age: f32,
    /// Time left, in sixtieths of a second.
    life: f32,
}

/// The effect a bullet of hers looks like (0x87774), and how much it is scaled up
/// (0x87A04). It is turned the way the bullet flies (0x879DC).
const LOOK: i32 = 999_036;
const LOOK_SCALE: f32 = 2.0;

/// What stands in for a bullet's look where there is no effect for it: a plain
/// bright streak.
#[derive(Resource)]
pub struct BulletLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl BulletLook {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> BulletLook {
        BulletLook {
            mesh: meshes.add(Cuboid::new(10.0, 10.0, 220.0)),
            material: materials.add(StandardMaterial {
                base_color: brs_engine::texture::values(Color::srgb(0.75, 0.95, 1.0)),
                unlit: true,
                fog_enabled: false,
                ..default()
            }),
        }
    }
}

/// Starts a bullet at `from` with `velocity` per sixtieth of a second, on logic
/// frame `frame`: on that frame it does not fly yet (0x8B374), and it is first
/// seen on the one after, a frame's flight from the muzzle.
pub fn spawn(commands: &mut Commands, look: &BulletLook, from: Vec3, velocity: Vec3, frame: u32) {
    let speed = velocity.length();
    if speed <= 0.0 {
        return;
    }
    let at = Transform { translation: from, rotation: facing(velocity), scale: Vec3::ONE };
    let mut made = commands.spawn((Battle, Bullet { velocity, age: 0.0, life: RANGE / speed }, Fired(frame), Stepped::new(at), at, Visibility::Hidden));
    match Look::of(LOOK, LOOK_SCALE) {
        Some(look) => made.insert(look),
        None => made.insert((Mesh3d(look.mesh.clone()), MeshMaterial3d(look.material.clone()))),
    };
}

impl Bullet {
    /// One logic frame of flight after its first (0x8789C). False once its range
    /// has run out.
    pub fn advance(&mut self, dt: f32, stepped: &mut Stepped, visibility: &mut Visibility) -> bool {
        self.age += dt;
        self.life -= dt;
        if self.life <= 0.0 {
            return false;
        }
        stepped.current.translation += self.velocity * dt;
        if self.age >= HIDDEN_FOR && *visibility == Visibility::Hidden {
            // It appears where it is, not sliding in from where it was hidden.
            let at = stepped.current;
            stepped.snap(at);
            *visibility = Visibility::Inherited;
        }
        true
    }
}

/// Whether a sphere of `radius` carried from `from` to `to` meets the sphere of
/// `target_radius` round `centre`, and if so how far from `from` the point of the
/// target's surface it first meets is, and that point: the distance is the
/// measure by which the original picks the nearest of several, and the point is
/// where the hit is (0x8B4A0).
pub fn sweep(from: Vec3, to: Vec3, radius: f32, centre: Vec3, target_radius: f32) -> Option<(f32, Vec3)> {
    let reach = radius + target_radius;
    let length = from.distance(to);
    let along = (to - from).normalize_or_zero();
    let apart = centre - from;
    let ahead = along.dot(apart);

    let contact = if apart.length_squared() <= reach * reach {
        // Already touching where it starts.
        from
    } else {
        if ahead <= 0.0 || ahead - length > reach {
            return None;
        }
        let inside = reach * reach - (apart.length_squared() - ahead * ahead);
        if inside < 0.0 {
            return None;
        }
        let travelled = ahead - inside.sqrt();
        if travelled > length {
            return None;
        }
        from + along * travelled
    };
    let surface = centre + (contact - centre).normalize_or_zero() * target_radius;
    Some((from.distance(surface), surface))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_through_a_sphere_meets_its_near_side() {
        // Flying along +Z past a sphere of 60 whose middle is 100 ahead: the two
        // touch when the middles are 130 apart, 30 short of the sphere's middle.
        let met = sweep(Vec3::new(0.0, 0.0, -100.0), Vec3::new(0.0, 0.0, 200.0), RADIUS, Vec3::new(0.0, 0.0, 100.0), 60.0);
        // The point met on the sphere's surface is 60 this side of its middle.
        let (distance, point) = met.unwrap();
        assert!((distance - 140.0).abs() < 1e-3, "{met:?}");
        assert!(point.abs_diff_eq(Vec3::new(0.0, 0.0, 40.0), 1e-3), "{met:?}");
    }

    #[test]
    fn a_shot_misses_what_is_off_its_line_behind_it_or_out_of_reach() {
        let (from, to) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 300.0));
        // 131 to the side of the line is just clear of 70 + 60.
        assert!(sweep(from, to, RADIUS, Vec3::new(131.0, 0.0, 150.0), 60.0).is_none());
        assert!(sweep(from, to, RADIUS, Vec3::new(129.0, 0.0, 150.0), 60.0).is_some());
        // Behind the start.
        assert!(sweep(from, to, RADIUS, Vec3::new(0.0, 0.0, -200.0), 60.0).is_none());
        // Too far ahead for this frame's flight.
        assert!(sweep(from, to, RADIUS, Vec3::new(0.0, 0.0, 431.0), 60.0).is_none());
        assert!(sweep(from, to, RADIUS, Vec3::new(0.0, 0.0, 429.0), 60.0).is_some());
    }

    #[test]
    fn a_shot_that_starts_inside_a_sphere_hits_it_there() {
        let distance = sweep(Vec3::ZERO, Vec3::new(0.0, 0.0, 300.0), RADIUS, Vec3::new(0.0, 50.0, 20.0), 60.0);
        assert!(distance.is_some());
    }

    #[test]
    fn of_two_spheres_on_the_line_the_nearer_is_met_first() {
        let (from, to) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 300.0));
        let near = sweep(from, to, RADIUS, Vec3::new(0.0, 0.0, 200.0), 60.0).unwrap();
        let far = sweep(from, to, RADIUS, Vec3::new(0.0, 0.0, 280.0), 60.0).unwrap();
        assert!(near.0 < far.0);
    }

    #[test]
    fn a_bullet_flies_until_its_range_runs_out() {
        let mut bullet = Bullet { velocity: Vec3::new(0.0, 0.0, 150.0), age: 0.0, life: RANGE / 150.0 };
        let mut stepped = Stepped::new(Transform::IDENTITY);
        let mut visibility = Visibility::Hidden;
        assert!(bullet.advance(2.0, &mut stepped, &mut visibility));
        assert_eq!(stepped.current.translation.z, 300.0);
        assert_eq!(visibility, Visibility::Inherited);
        let mut steps = 1;
        while bullet.advance(2.0, &mut stepped, &mut visibility) {
            steps += 1;
        }
        // 8000 at 300 a frame.
        assert_eq!(steps, 26);
    }
}
