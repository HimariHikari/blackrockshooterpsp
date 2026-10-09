//! Collision queries against the triangles of a map's `CLIM` files.
//!
//! Maps have a few hundred collision triangles, so every query simply tests them all.

use bevy::prelude::*;
use brs_formats::clim;

/// Triangles steeper than this (by the Y of their unit normal) are walls, not ground.
const GROUND_NORMAL_Y: f32 = 0.5;
/// Passes of pushing a character out of walls; more than one handles corners.
const PUSH_OUT_PASSES: usize = 4;
/// The least a sphere that is swept along a path is moved on at a time.
const SWEEP_STEP: f32 = 0.5;

/// The bits of the first byte of an attribute's record (0x13EC8).
pub mod mark {
    /// The triangle stops nothing: it only marks the place under or over it.
    pub const NOT_SOLID: u8 = 1;
    /// The record's second byte says what kind of ground it is.
    pub const KIND: u8 = 2;
    /// The record's third byte says which of the scene's lights falls there.
    pub const LIGHT: u8 = 4;
}

#[derive(Resource, Default)]
pub struct CollisionWorld {
    /// The triangles that stop things.
    triangles: Vec<Triangle>,
    /// The ones that only mark a place: sheets a little over the ground of a shaded
    /// street or of a floor that sounds otherwise.
    regions: Vec<Triangle>,
}

#[derive(Clone, Copy)]
pub struct Triangle {
    pub corners: [Vec3; 3],
    /// Unit normal by the triangle's winding; its sign is not relied on.
    pub normal: Vec3,
    pub attribute: u8,
    /// The first byte of its attribute's record: the bits of [`mark`].
    pub marks: u8,
    /// What kind of ground it is, which the game chooses the sound of a step by: the
    /// record's second byte, which counts when [`mark::KIND`] is set.
    pub kind: u8,
    /// Which light falls there: the record's third byte, which counts when
    /// [`mark::LIGHT`] is set.
    pub light: u8,
    pub layer: Layer,
}

/// Which of a map's collision files a triangle came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The physical world: floors, walls, obstacles.
    World,
    /// Invisible walls fencing in the playable area. They stop characters but not
    /// lines of sight.
    Boundary,
}

/// Where a downward ray met the ground.
#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub height: f32,
    pub attribute: u8,
    /// Which way is up there: straight out of the ground, on its upper side.
    pub normal: Vec3,
}

/// What a place is marked as by the triangles over and under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    /// What kind of ground it is, if anything there says.
    pub kind: Option<u8>,
    /// Which of the scene's lights falls there, if anything there says.
    pub light: Option<u8>,
}

impl CollisionWorld {
    /// Takes every triangle away: the map they were of is gone.
    pub fn clear(&mut self) {
        self.triangles.clear();
        self.regions.clear();
    }

    /// Adds every triangle of `collision`: the ones that stop things, and apart from
    /// them the ones whose attribute says they only mark a place.
    pub fn add(&mut self, collision: &clim::Collision, layer: Layer) {
        for triangle in &collision.triangles {
            let corners = collision.corners(triangle).map(Vec3::from);
            let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
            if normal != Vec3::ZERO {
                let [marks, kind, light, _] = collision.attributes.get(triangle.attribute as usize).copied().unwrap_or_default();
                let triangle = Triangle { corners, normal, attribute: triangle.attribute, marks, kind, light, layer };
                if marks & mark::NOT_SOLID == 0 {
                    self.triangles.push(triangle);
                } else {
                    self.regions.push(triangle);
                }
            }
        }
    }

    /// The same triangles with every one of them turned by `turn` about where the
    /// world begins: the map as something that is itself turned the other way has
    /// it, for which what is ground and what is a wall goes by its own upright.
    pub fn turned(&self, turn: Quat) -> CollisionWorld {
        let turned = |triangle: &Triangle| Triangle { corners: triangle.corners.map(|corner| turn * corner), normal: turn * triangle.normal, ..*triangle };
        CollisionWorld { triangles: self.triangles.iter().map(turned).collect(), regions: self.regions.iter().map(turned).collect() }
    }

    /// What the place is marked as, by a ray straight down from `above` over it to
    /// `below` under it. The game keeps the nearest triangle that says a kind and the
    /// nearest that says a light apart from each other and from what stops the ray
    /// (0x13EC8), so a sheet that marks a place counts though the ground is under it,
    /// and so does a roof over her head.
    pub fn marks(&self, position: Vec3, above: f32, below: f32) -> Marks {
        let origin = position + Vec3::Y * above;
        let mut kind: Option<(f32, u8)> = None;
        let mut light: Option<(f32, u8)> = None;
        let marked = self.triangles.iter().chain(&self.regions).filter(|t| t.layer == Layer::World && t.marks & (mark::KIND | mark::LIGHT) != 0);
        for triangle in marked {
            let Some(distance) = ray_triangle(origin, Vec3::NEG_Y, &triangle.corners).filter(|&d| d <= above + below) else { continue };
            if triangle.marks & mark::KIND != 0 && kind.is_none_or(|(d, _)| distance < d) {
                kind = Some((distance, triangle.kind));
            }
            if triangle.marks & mark::LIGHT != 0 && light.is_none_or(|(d, _)| distance < d) {
                light = Some((distance, triangle.light));
            }
        }
        Marks { kind: kind.map(|(_, kind)| kind), light: light.map(|(_, light)| light) }
    }

    /// The triangles that only mark a place.
    pub fn regions(&self) -> &[Triangle] {
        &self.regions
    }

    /// Distance from `point` to the nearest wall of either layer.
    pub fn wall_distance(&self, point: Vec3) -> f32 {
        self.triangles
            .iter()
            .filter(|t| t.normal.y.abs() < GROUND_NORMAL_Y)
            .map(|t| closest_point(point, &t.corners).distance(point))
            .fold(f32::MAX, f32::min)
    }

    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }

    /// The highest ground under `position` between `position.y + above` and
    /// `position.y - below`.
    pub fn ground(&self, position: Vec3, above: f32, below: f32) -> Option<Ground> {
        let origin = position + Vec3::Y * above;
        let mut best: Option<(f32, &Triangle)> = None;
        for triangle in self.triangles.iter().filter(|t| t.normal.y.abs() >= GROUND_NORMAL_Y) {
            if let Some(distance) = ray_triangle(origin, Vec3::NEG_Y, &triangle.corners) {
                if distance <= above + below && best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, triangle));
                }
            }
        }
        best.map(|(distance, triangle)| Ground {
            height: origin.y - distance,
            attribute: triangle.attribute,
            normal: triangle.normal * triangle.normal.y.signum(),
        })
    }

    /// Distance along `direction` (a unit vector) to the first piece of the physical
    /// world within `max`. Boundary walls are invisible and do not block the ray.
    pub fn sight(&self, origin: Vec3, direction: Vec3, max: f32) -> Option<f32> {
        self.triangles
            .iter()
            .filter(|t| t.layer == Layer::World)
            .filter_map(|t| ray_triangle(origin, direction, &t.corners))
            .filter(|&d| d <= max)
            .min_by(f32::total_cmp)
    }

    /// How far along `path` a sphere of `radius` whose middle starts at `origin` gets
    /// before it touches a triangle of `layer` that is ahead of it: a part of the
    /// path, from 0 to 1, or nothing if it gets to the end of it.
    pub fn sweep(&self, origin: Vec3, path: Vec3, radius: f32, layer: Layer) -> Option<f32> {
        let length = path.length();
        if length <= f32::EPSILON {
            return None;
        }
        let direction = path / length;
        let mut gone = 0.0;
        while gone < length {
            let centre = origin + direction * gone;
            // The room there is before the nearest of the triangles ahead.
            let ahead = self.triangles.iter().filter(|triangle| triangle.layer == layer).filter_map(|triangle| {
                let nearest = closest_point(centre, &triangle.corners);
                ((nearest - centre).dot(direction) > 0.0).then(|| nearest.distance(centre) - radius)
            });
            let room = ahead.fold(f32::MAX, f32::min);
            if room <= 0.0 {
                return Some(gone / length);
            }
            gone += room.max(SWEEP_STEP);
        }
        None
    }

    /// Moves a sphere at `centre` sideways until it no longer overlaps any wall.
    pub fn push_out_of_walls(&self, mut centre: Vec3, radius: f32) -> Vec3 {
        for _ in 0..PUSH_OUT_PASSES {
            let mut moved = false;
            for triangle in self.triangles.iter().filter(|t| t.normal.y.abs() < GROUND_NORMAL_Y) {
                let closest = closest_point(centre, &triangle.corners);
                let mut away = centre - closest;
                let distance = away.length();
                if distance >= radius {
                    continue;
                }
                // Push horizontally only: walls should never lift or sink the character.
                away.y = 0.0;
                let direction = if away.length_squared() > 1e-8 {
                    away.normalize()
                } else {
                    Vec3::new(triangle.normal.x, 0.0, triangle.normal.z).normalize_or_zero()
                };
                centre += direction * (radius - distance);
                moved = true;
            }
            if !moved {
                break;
            }
        }
        centre
    }

    /// Walks a character standing at `feet` by `step` (horizontal), keeping it out of
    /// walls and on the ground. Returns the new feet position; if there is no ground
    /// to stand on at the destination the character stays where it was.
    pub fn walk(&self, feet: Vec3, step: Vec3, body: &Body) -> Vec3 {
        let lift = Vec3::Y * (body.step_height + body.radius);
        // Advance in pieces shorter than the body, so a long step cannot land wholly on
        // the far side of a wall without ever touching it.
        let pieces = (step.length() / (body.radius * 0.5)).ceil().max(1.0);
        let mut centre = feet + lift;
        for _ in 0..pieces as usize {
            centre = self.push_out_of_walls(centre + step / pieces, body.radius);
        }
        let moved = centre - lift;
        match self.ground(moved, body.step_height, body.step_height) {
            Some(ground) => Vec3::new(moved.x, ground.height, moved.z),
            None => feet,
        }
    }
}

/// The size of a walking character for collision purposes.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub radius: f32,
    /// How high a ledge the character steps up or down without stopping.
    pub step_height: f32,
}

/// Distance along the ray to the triangle, if it hits (either side).
fn ray_triangle(origin: Vec3, direction: Vec3, corners: &[Vec3; 3]) -> Option<f32> {
    let edge1 = corners[1] - corners[0];
    let edge2 = corners[2] - corners[0];
    let p = direction.cross(edge2);
    let det = edge1.dot(p);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let t = origin - corners[0];
    let u = t.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = t.cross(edge1);
    let v = direction.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge2.dot(q) * inv;
    (distance >= 0.0).then_some(distance)
}

/// The point of the triangle nearest to `p` (Ericson, "Real-Time Collision Detection").
fn closest_point(p: Vec3, [a, b, c]: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (*a, *b, *c);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(triangles: &[[Vec3; 3]]) -> CollisionWorld {
        let triangles = triangles
            .iter()
            .map(|&corners| Triangle {
                corners,
                normal: (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize(),
                attribute: 0,
                marks: 0,
                kind: 0,
                light: 0,
                layer: Layer::World,
            })
            .collect();
        CollisionWorld { triangles, regions: Vec::new() }
    }

    const FLOOR: [Vec3; 3] = [Vec3::new(-100.0, 0.0, -100.0), Vec3::new(-100.0, 0.0, 300.0), Vec3::new(300.0, 0.0, -100.0)];
    /// A wall in the plane x = 50, standing on the floor.
    const WALL: [Vec3; 3] = [Vec3::new(50.0, 0.0, -100.0), Vec3::new(50.0, 200.0, 0.0), Vec3::new(50.0, 0.0, 100.0)];
    const BODY: Body = Body { radius: 10.0, step_height: 5.0 };

    #[test]
    fn ground_is_found_under_and_missed_beside() {
        let world = world(&[FLOOR]);
        let hit = world.ground(Vec3::new(10.0, 3.0, 10.0), 5.0, 5.0).unwrap();
        assert!((hit.height - 0.0).abs() < 1e-4);
        assert!(world.ground(Vec3::new(-500.0, 0.0, 0.0), 5.0, 5.0).is_none());
        // Too far below the allowed range.
        assert!(world.ground(Vec3::new(10.0, 50.0, 10.0), 5.0, 5.0).is_none());
    }

    #[test]
    fn a_wall_is_ground_to_what_is_turned_to_stand_on_it() {
        // Something turned a quarter about the axis ahead has the wall's side for
        // its upright: with the map turned the other way, the wall lies under it.
        let world = world(&[FLOOR, WALL]);
        assert!(world.ground(Vec3::new(60.0, 50.0, 0.0), 20.0, 20.0).is_none());
        let frame = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
        let lying = world.turned(frame.inverse());
        let stood = frame.inverse() * Vec3::new(60.0, 50.0, 0.0);
        let under = lying.ground(stood, 20.0, 20.0).expect("the wall should be under it");
        let on = frame * Vec3::new(stood.x, under.height, stood.z);
        assert!(on.distance(Vec3::new(50.0, 50.0, 0.0)) < 1e-3, "{on}");
    }

    #[test]
    fn a_sheet_that_marks_a_place_is_not_stood_on_and_says_what_is_under_it() {
        // The streets' own way: the ground says a kind and the open light, and a sheet
        // fifteen over a part of it says the light of the shade.
        let sheet = FLOOR.map(|corner| corner * 0.5 + Vec3::Y * 15.0);
        let mut both = world(&[FLOOR, sheet]);
        (both.triangles[0].marks, both.triangles[0].kind) = (mark::KIND | mark::LIGHT, 5);
        (both.triangles[1].marks, both.triangles[1].light) = (mark::NOT_SOLID | mark::LIGHT, 1);
        let sheet = both.triangles.pop().unwrap();
        both.regions.push(sheet);

        let shaded = Vec3::new(10.0, 0.0, 10.0);
        let open = Vec3::new(140.0, 0.0, 10.0);
        assert!(both.ground(shaded + Vec3::Y * 20.0, 5.0, 30.0).unwrap().height.abs() < 1e-4);
        assert_eq!(both.walk(shaded, Vec3::X, &Body { radius: 10.0, step_height: 20.0 }).y, 0.0);
        assert_eq!(both.marks(shaded, 300.0, 200.0), Marks { kind: Some(5), light: Some(1) });
        assert_eq!(both.marks(open, 300.0, 200.0), Marks { kind: Some(5), light: Some(0) });
        // Out of the ray's reach, and off the ground altogether.
        assert_eq!(both.marks(shaded + Vec3::Y * 400.0, 300.0, 200.0), Marks::default());
        assert_eq!(both.marks(Vec3::new(-500.0, 0.0, 0.0), 300.0, 200.0), Marks::default());
    }

    #[test]
    fn walking_into_a_wall_stops_at_its_surface() {
        let world = world(&[FLOOR, WALL]);
        let feet = world.walk(Vec3::new(30.0, 0.0, 0.0), Vec3::new(30.0, 0.0, 0.0), &BODY);
        assert!((feet.x - 40.0).abs() < 1e-3, "stopped at x = {}", feet.x);
        assert!(feet.y.abs() < 1e-4);
    }

    #[test]
    fn walking_along_a_wall_slides() {
        let world = world(&[FLOOR, WALL]);
        let feet = world.walk(Vec3::new(40.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 20.0), &BODY);
        assert!((feet.x - 40.0).abs() < 1e-3 && (feet.z - 20.0).abs() < 1e-3, "ended at {feet}");
    }

    #[test]
    fn boundary_walls_stop_walking_but_not_sight() {
        let mut world = world(&[FLOOR, WALL]);
        world.triangles[1].layer = Layer::Boundary;
        let feet = world.walk(Vec3::new(30.0, 0.0, 0.0), Vec3::new(30.0, 0.0, 0.0), &BODY);
        assert!((feet.x - 40.0).abs() < 1e-3, "stopped at x = {}", feet.x);
        assert_eq!(world.sight(Vec3::new(0.0, 50.0, 0.0), Vec3::X, 200.0), None);

        world.triangles[1].layer = Layer::World;
        let hit = world.sight(Vec3::new(0.0, 50.0, 0.0), Vec3::X, 200.0).unwrap();
        assert!((hit - 50.0).abs() < 1e-3);
    }

    #[test]
    fn a_swept_sphere_stops_where_it_touches_a_wall_of_its_layer() {
        let mut world = world(&[FLOOR, WALL]);
        world.triangles[1].layer = Layer::Boundary;
        // The wall is at x = 50: a sphere of ten is stopped with its middle at 40.
        let part = world.sweep(Vec3::new(0.0, 50.0, 0.0), Vec3::X * 200.0, 10.0, Layer::Boundary).unwrap();
        assert!((part * 200.0 - 40.0).abs() <= SWEEP_STEP, "stopped at {}", part * 200.0);
        // It goes by what is beside it, and by what is of another layer.
        assert_eq!(world.sweep(Vec3::new(45.0, 50.0, -50.0), Vec3::Z * 100.0, 10.0, Layer::Boundary), None);
        assert_eq!(world.sweep(Vec3::new(0.0, 50.0, 0.0), Vec3::X * 200.0, 10.0, Layer::World), None);
        // And a path that ends before the wall is free.
        assert_eq!(world.sweep(Vec3::new(0.0, 50.0, 0.0), Vec3::X * 30.0, 10.0, Layer::Boundary), None);
    }

    #[test]
    fn wall_distance_is_measured_to_the_nearest_wall() {
        let world = world(&[FLOOR, WALL]);
        assert!((world.wall_distance(Vec3::new(20.0, 10.0, 0.0)) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn walking_off_the_ground_is_refused() {
        let world = world(&[FLOOR]);
        let start = Vec3::new(-95.0, 0.0, 0.0);
        assert_eq!(world.walk(start, Vec3::new(-20.0, 0.0, 0.0), &BODY), start);
    }
}
