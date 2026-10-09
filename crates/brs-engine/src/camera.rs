//! Camera moves from `.cam` files, and the game's way of describing a lens.
//!
//! A move says where the camera is and what its lens is at each frame, in the space
//! the move is played in: the original plays one relative to a node of the actor
//! that starts it (its camera player, built at 0xA3E70).

use std::sync::Arc;

use bevy::math::Affine3A;
use bevy::prelude::*;
use brs_formats::{cam, cpmd, insa};

use crate::animation::{transform_of, Clip, Joint, Posed};

/// Inches in a millimetre, as the original has it (0x166048).
const INCHES_PER_MM: f32 = 0.03937;

/// The vertical field of view, in radians, of a lens of `focal_length` millimetres on
/// film `film_width` inches wide, shown on a screen `aspect` times as wide as it is
/// high; `zoom` scales the picture's width (0x24AC0, 0xA31F4).
pub fn vertical_fov(focal_length: f32, film_width: f32, zoom: f32, aspect: f32) -> f32 {
    let half_width = film_width * 0.5 / (focal_length * INCHES_PER_MM) * zoom;
    2.0 * (half_width / aspect).atan()
}

/// How many times as far as just before or just after the camera has to go between
/// two frames, or how many times as much it has to turn, for that to be a cut; and
/// what it has to go or turn besides, which keeps a camera that hardly stirs from
/// being told to have cut.
const CUT: f32 = 4.0;
const CUT_AT_LEAST: (f32, f32) = (0.01, 0.001);

/// The camera at one moment of a move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    /// Where the camera is and how it is turned: looking along its -Z with +Y up, as
    /// Bevy's cameras do.
    pub transform: Transform,
    /// The lens: millimetres of focal length on film this many inches wide, with the
    /// picture's width scaled by `zoom`.
    pub focal_length: f32,
    pub film_width: f32,
    pub zoom: f32,
}

impl CameraPose {
    /// The vertical field of view on a screen `aspect` times as wide as it is high.
    pub fn vertical_fov(&self, aspect: f32) -> f32 {
        vertical_fov(self.focal_length, self.film_width, self.zoom, aspect)
    }
}

/// A camera move, ready to play.
pub struct CameraClip {
    camera: cam::Camera,
    /// The animation of the rig's nodes; a camera that stays put has none.
    clip: Option<Arc<Clip>>,
}

impl CameraClip {
    pub fn new(mut camera: cam::Camera) -> Arc<CameraClip> {
        let clip = camera.animation.take().map(Clip::new);
        Arc::new(CameraClip { camera, clip })
    }

    /// Length in frames.
    pub fn frames(&self) -> f32 {
        self.clip.as_ref().map_or(0.0, |clip| clip.frames())
    }

    /// The camera at `frame`. Past either end of the move it stays at that end.
    pub fn pose(&self, frame: f32) -> CameraPose {
        let frame = frame.clamp(0.0, self.frames());
        // The camera's node and its parents, innermost first.
        let mut chain = vec![self.camera.node];
        while let Some(parent) = self.camera.nodes[chain[chain.len() - 1]].parent {
            chain.push(parent);
        }
        let mut matrix = Affine3A::IDENTITY;
        let mut parent_scale = None;
        for &index in chain.iter().rev() {
            let node = &self.camera.nodes[index];
            let rest = Transform {
                translation: Vec3::from(node.translation),
                rotation: Quat::from_array(node.rotation),
                scale: Vec3::ONE,
            };
            let joint = Joint { rest: rest.translation, fitted: false };
            let posed = self
                .clip
                .as_ref()
                .and_then(|clip| clip.pose(index, frame, &joint, parent_scale, (rest.rotation, rest.translation), None))
                .unwrap_or_else(|| Posed::rest(rest));
            matrix *= posed.local;
            parent_scale = Some(posed.scale);
        }
        // The original then takes the node's axes without their scale (0xA4084) and
        // turns it about to look along +Z, as its own cameras do; Bevy's look along
        // -Z, the way the node does.
        let mut transform = transform_of(&matrix);
        transform.scale = Vec3::ONE;

        let time = self.clip.as_ref().map_or(0.0, |clip| clip.time_at(frame));
        CameraPose {
            transform,
            focal_length: insa::sample(&self.camera.focal, time).unwrap_or(self.camera.focal_length),
            film_width: self.camera.film_width,
            zoom: insa::sample(&self.camera.zoom, time).unwrap_or(1.0),
        }
    }

    /// Whether the move cuts to another shot between two frames, rather than goes
    /// from the one place to the other. The original shows a move frame by frame
    /// and is never between two; what shows it between them as well must not at a
    /// cut, where the camera is in neither shot. A cut is told by the camera going
    /// several times as far in that time, or turning several times as much, as it
    /// does in as much time just before or just after.
    pub fn cuts(&self, from: f32, to: f32) -> bool {
        let step = to - from;
        if step <= 0.0 || to > self.frames() {
            return false;
        }
        let at = |frame: f32| self.pose(frame).transform;
        let gone = |a: &Transform, b: &Transform| (a.translation.distance(b.translation), a.rotation.angle_between(b.rotation));
        let (here, there) = (at(from), at(to));
        // What it does on either side, where the move goes on that far.
        let before = (from - step >= 0.0).then(|| gone(&at(from - step), &here));
        let after = (to + step <= self.frames()).then(|| gone(&there, &at(to + step)));
        let calm = match (before, after) {
            (Some(before), Some(after)) => (before.0.min(after.0), before.1.min(after.1)),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => return false,
        };
        let (far, turned) = gone(&here, &there);
        far > CUT * calm.0 + CUT_AT_LEAST.0 || turned > CUT * calm.1 + CUT_AT_LEAST.1
    }
}

/// A camera that goes along a path of a field map's: a move, and a map of the ground
/// that says how far along the move the camera is for wherever she stands (0xDAA48).
pub struct CameraPath {
    map: cpmd::PathMap,
    clip: Arc<CameraClip>,
}

impl CameraPath {
    pub fn new(map: cpmd::PathMap, camera: cam::Camera) -> Arc<CameraPath> {
        Arc::new(CameraPath { map, clip: CameraClip::new(camera) })
    }

    /// The camera with her at (`x`, `z`): the move's first frame where the map has
    /// nought, its last where it has the whole, and evenly between.
    pub fn pose(&self, x: f32, z: f32) -> CameraPose {
        self.clip.pose(self.map.along(x, z) * self.clip.frames())
    }
}

#[cfg(test)]
mod tests {
    use brs_formats::insm;

    use super::*;

    fn node(parent: Option<usize>, translation: [f32; 3], rotation: Quat) -> insm::Node {
        insm::Node { flags: 0, id: 0xff, parent, next: None, translation, rotation: rotation.to_array() }
    }

    /// A rig of a mount turned a quarter to the left with the camera 10 in front of
    /// it, whose mount slides 100 along X over 60 frames while the lens goes from 35
    /// to 70 millimetres.
    fn clip() -> Arc<CameraClip> {
        sliding(&[(0, 0.0), (60, 100.0)])
    }

    /// The same rig, with its mount at these places along X at these frames, the
    /// last of which is the move's last.
    fn sliding(places: &[(u32, f32)]) -> Arc<CameraClip> {
        use insa::{Channel, Part};
        let unit = insa::UNITS_PER_FRAME;
        let key = |frame: u32, value: f32| insa::Key { time: frame * unit, value };
        let quarter = Quat::from_rotation_y(90f32.to_radians()).to_array();
        let places: Vec<(u32, f32)> = places.iter().map(|&(frame, place)| (frame * unit, place)).collect();
        let end = places[places.len() - 1].0;
        let mount = insa::NodeTracks {
            id: 0xff,
            parts: vec![
                Part::Rotation(insa::Rotation::constant(quarter)),
                Part::Translation([Channel::keyed(&places), Channel::constant(50.0), Channel::constant(0.0)]),
            ],
            ..Default::default()
        };
        // A node the move leaves alone stays as the rig has it.
        let camera = insa::NodeTracks { id: 0xff, masked: true, ..Default::default() };
        let animation = insa::Animation { start: 0, end, loop_start: 0, loop_end: end, nodes: vec![mount, camera] };
        CameraClip::new(cam::Camera {
            nodes: vec![
                node(None, [0.0, 50.0, 0.0], Quat::from_rotation_y(90f32.to_radians())),
                node(Some(0), [0.0, 0.0, -10.0], Quat::IDENTITY),
            ],
            node: 1,
            focal_length: 35.0,
            film_width: 1.417,
            animation: Some(animation),
            focal: vec![key(0, 35.0), key(60, 70.0)],
            zoom: vec![key(0, 1.0)],
        })
    }

    #[test]
    fn the_original_lens_gives_its_field_of_view() {
        // 35 mm on 1.417 inches: the angle the original gives its fixed overview
        // camera outright.
        assert!((vertical_fov(35.0, 1.417, 1.0, 480.0 / 272.0) - 0.567023).abs() < 1e-3);
        // Twice the focal length halves the picture.
        let (wide, long) = (vertical_fov(35.0, 1.417, 1.0, 1.5), vertical_fov(70.0, 1.417, 1.0, 1.5));
        assert!(((wide * 0.5).tan() - 2.0 * (long * 0.5).tan()).abs() < 1e-6);
        // And the zoom scales it back.
        assert!((vertical_fov(70.0, 1.417, 2.0, 1.5) - wide).abs() < 1e-6);
    }

    #[test]
    fn the_camera_follows_its_rig_and_its_lens_keys() {
        let clip = clip();
        assert_eq!(clip.frames(), 60.0);
        let start = clip.pose(0.0);
        // The mount faces -X after its quarter turn, so the camera is 10 to that side.
        assert!(start.transform.translation.distance(Vec3::new(-10.0, 50.0, 0.0)) < 1e-4, "{}", start.transform.translation);
        assert!((start.transform.rotation * Vec3::NEG_Z).distance(Vec3::NEG_X) < 1e-5);
        assert_eq!(start.focal_length, 35.0);

        let middle = clip.pose(30.0);
        assert!(middle.transform.translation.distance(Vec3::new(40.0, 50.0, 0.0)) < 1e-3);
        assert_eq!(middle.focal_length, 52.5);
        assert_eq!((middle.film_width, middle.zoom), (1.417, 1.0));

        // It stays at the end of the move.
        assert_eq!(clip.pose(500.0), clip.pose(60.0));
        assert!(clip.pose(60.0).transform.translation.distance(Vec3::new(90.0, 50.0, 0.0)) < 1e-3);
    }

    #[test]
    fn a_move_that_jumps_to_another_shot_is_told_from_one_that_goes_there() {
        // A unit a frame for thirty frames, then somewhere else two frames later,
        // and on from there as before.
        let clip = sliding(&[(0, 0.0), (30, 30.0), (32, 500.0), (60, 528.0)]);
        assert!(clip.cuts(30.0, 32.0));
        for from in [0.0, 2.0, 26.0, 28.0, 32.0, 34.0, 56.0, 58.0] {
            assert!(!clip.cuts(from, from + 2.0), "from {from}");
        }
        // Nothing is past the end of a move, and nothing between a frame and itself.
        assert!(!clip.cuts(60.0, 62.0) && !clip.cuts(30.0, 30.0));
        // A move that only speeds up does not cut: twice as fast is not a jump.
        let faster = sliding(&[(0, 0.0), (30, 30.0), (60, 90.0)]);
        assert!((0..30).all(|step| !faster.cuts(step as f32 * 2.0, step as f32 * 2.0 + 2.0)));
        // Neither does one that stays where it is.
        let still = sliding(&[(0, 5.0), (60, 5.0)]);
        assert!(!still.cuts(10.0, 12.0));
    }

    #[test]
    fn a_camera_on_a_path_is_as_far_along_its_move_as_she_is_along_the_map() {
        // The move of the test above, over a strip of ground 600 long: at its near
        // end the camera is at the move's start, at its far end at the move's end.
        let map = cpmd::PathMap { columns: 1, rows: 1, x: [0.0, 100.0], z: [0.0, 600.0], points: vec![0, 0, 0xffff, 0xffff] };
        let path = CameraPath { map, clip: clip() };
        assert!(path.pose(50.0, -40.0).transform.translation.distance(Vec3::new(-10.0, 50.0, 0.0)) < 1e-3);
        assert!(path.pose(50.0, 300.0).transform.translation.distance(Vec3::new(40.0, 50.0, 0.0)) < 0.01);
        assert!(path.pose(50.0, 700.0).transform.translation.distance(Vec3::new(90.0, 50.0, 0.0)) < 1e-3);
    }

    #[test]
    fn a_camera_without_an_animation_stays_at_rest() {
        let clip = CameraClip::new(cam::Camera {
            nodes: vec![node(None, [1.0, 2.0, 3.0], Quat::IDENTITY)],
            node: 0,
            focal_length: 50.0,
            film_width: 1.417,
            animation: None,
            focal: Vec::new(),
            zoom: Vec::new(),
        });
        assert_eq!(clip.frames(), 0.0);
        let pose = clip.pose(12.0);
        assert_eq!(pose.transform.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!((pose.focal_length, pose.zoom), (50.0, 1.0));
    }
}
