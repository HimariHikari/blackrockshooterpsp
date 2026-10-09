//! Camera moves: where the camera is and what its lens is over time, for the staged
//! moments of a battle (skills, the victory pose, a defeat) and the cuts of the
//! field's story scenes.
//!
//! A move is an `INSM` model without geometry (a few nodes, one of which is the
//! camera, and a record of its lens) and an `INSA` animation of those nodes with one
//! extra track for the lens. A battle's `.cam` file has the one after the other; a
//! scene's cut has them as the model and the animation of a bundle laid out like a
//! field character's. `docs/FORMAT_NOTES.md` has the layout.

use anyhow::{ensure, Context, Result};

use crate::reader::Reader;
use crate::{container, insa, insm};

// The `INSM` header as a camera file uses it.
const CAMERA_COUNT_AT: usize = 0x07;
const NODE_COUNT_AT: usize = 0x08;
const NODES_AT: usize = 0x18;
const CAMERA_AT: usize = 0x24;
const ANIMATION_AT: usize = 0x34;

// The camera's record.
const CAMERA_NODE: usize = 0x02;
const CAMERA_FOCAL_LENGTH: usize = 0x0c;
const CAMERA_FILM_WIDTH: usize = 0x14;

// The `INSA` header's fields for the lens track, and the track's record.
const LENS_COUNT_AT: usize = 0x0b;
const LENS_AT: usize = 0x1c;
const LENS_COUNTS: usize = 4;
const LENS_OFFSETS: usize = 8;
const KEY_LEN: usize = 8;

#[derive(Clone, Debug)]
pub struct Camera {
    /// The nodes of the rig; a parent comes before its children.
    pub nodes: Vec<insm::Node>,
    /// Which node is the camera. It looks along its own -Z, with +Y up.
    pub node: usize,
    /// The lens at rest: its focal length in millimetres, on a film this many inches
    /// wide.
    pub focal_length: f32,
    pub film_width: f32,
    /// Moves the nodes, matched by index. A camera that stays put has none: the file
    /// ends where its animation would start.
    pub animation: Option<insa::Animation>,
    /// Keys of the focal length, on the animation's time scale.
    pub focal: Vec<insa::Key<f32>>,
    /// Keys of a scale on the picture's width. The game's files all leave it at 1.
    pub zoom: Vec<insa::Key<f32>>,
}

/// Whether an `INSM` file is a camera rather than a model.
pub fn is_camera(d: &[u8]) -> bool {
    d.starts_with(insm::MAGIC) && d.get(CAMERA_COUNT_AT) == Some(&1)
}

impl Camera {
    /// A `.cam` file: the rig, and after it the move.
    pub fn parse(d: &[u8]) -> Result<Camera> {
        ensure!(d.starts_with(insm::MAGIC), "not an INSM file");
        let animation_at = Reader(d).u32(ANIMATION_AT)? as usize;
        let insa = d.get(animation_at..).context("the animation's offset is out of range")?;
        Camera::from_parts(d, insa)
    }

    /// A cut of an event scene: a bundle with the rig as the first of its models
    /// (slot 1) and the move as the first of its animations (slot 2), which is how
    /// the original reads it (0xE73C8, 0xE7458).
    pub fn parse_cut(d: &[u8]) -> Result<Camera> {
        let first = |slot: usize| container::sc_slot(d, slot).and_then(|inner| container::sc_slot(inner, 0));
        let model = first(1).context("the cut has no camera")?;
        Camera::from_parts(model, first(2).unwrap_or(&[]))
    }

    /// The rig of `model` moved by the animation `insa`, which is empty for a camera
    /// that stays put.
    pub fn from_parts(model: &[u8], insa: &[u8]) -> Result<Camera> {
        let d = model;
        ensure!(d.starts_with(insm::MAGIC), "not an INSM file");
        let r = Reader(d);
        ensure!(r.u8(4)? == 1 && is_camera(d), "INSM file is not a camera");

        let node_count = r.u8(NODE_COUNT_AT)? as usize;
        let nodes = insm::read_nodes(&r, r.u32(NODES_AT)? as usize, node_count)?;
        let record = r.u32(CAMERA_AT)? as usize;
        let node = r.u8(record + CAMERA_NODE)? as usize;
        ensure!(node < node_count, "the camera is node {node} of {node_count}");

        let (animation, [focal, zoom]) = if insa.is_empty() {
            (None, [Vec::new(), Vec::new()])
        } else {
            let animation = insa::Animation::parse(insa).context("the camera's animation")?;
            // The moves of the camera of the ride on the bike leave the rig's last
            // node alone: with none of the nodes named, the animation's go to the
            // rig's first ones (the walk of 0x21454).
            ensure!(
                animation.nodes.len() <= node_count,
                "the animation moves {} nodes and the rig has {node_count}",
                animation.nodes.len()
            );
            (Some(animation), lens_track(insa).context("the lens track")?)
        };

        Ok(Camera {
            nodes,
            node,
            focal_length: r.f32(record + CAMERA_FOCAL_LENGTH)?,
            film_width: r.f32(record + CAMERA_FILM_WIDTH)?,
            animation,
            focal,
            zoom,
        })
    }
}

/// The two channels of the animation's lens track; empty when it has none.
fn lens_track(insa: &[u8]) -> Result<[Vec<insa::Key<f32>>; 2]> {
    let r = Reader(insa);
    let count = r.u8(LENS_COUNT_AT)?;
    ensure!(count <= 1, "{count} lens tracks, where one camera has one");
    let mut channels = [Vec::new(), Vec::new()];
    if count == 0 {
        return Ok(channels);
    }
    let at = r.u32(LENS_AT)? as usize;
    for (channel, keys) in channels.iter_mut().enumerate() {
        let count = r.u16(at + LENS_COUNTS + channel * 2)? as usize;
        let keys_at = r.u32(at + LENS_OFFSETS + channel * 4)? as usize;
        for k in 0..count {
            let key_at = keys_at + k * KEY_LEN;
            keys.push(insa::Key { time: r.u32(key_at)?, value: r.f32(key_at + 4)? });
        }
    }
    Ok(channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSA_HEADER: usize = 0x28;
    const INSA_NODE: usize = 0x24;
    const INSM_NODE: usize = 0x20;

    /// A camera file with one node at `place`, a lens of `focal` millimetres, and an
    /// animation that only moves the lens.
    fn file(place: [f32; 3], focal: f32, focal_keys: &[(u32, f32)]) -> Vec<u8> {
        let (nodes_at, record_at, animation_at) = (0x38usize, 0x60usize, 0x80usize);
        let mut d = vec![0u8; animation_at];
        let word = |d: &mut Vec<u8>, at: usize, value: u32| d[at..at + 4].copy_from_slice(&value.to_le_bytes());
        let float = |d: &mut Vec<u8>, at: usize, value: f32| d[at..at + 4].copy_from_slice(&value.to_le_bytes());

        d[..4].copy_from_slice(insm::MAGIC);
        d[4] = 1;
        d[CAMERA_COUNT_AT] = 1;
        d[NODE_COUNT_AT] = 1;
        word(&mut d, NODES_AT, nodes_at as u32);
        word(&mut d, CAMERA_AT, record_at as u32);
        word(&mut d, ANIMATION_AT, animation_at as u32);
        // The node: no parent, at `place`, not turned.
        d[nodes_at + 1] = 0xff;
        d[nodes_at + 2] = 0xff;
        for (axis, value) in place.iter().enumerate() {
            float(&mut d, nodes_at + 4 + axis * 4, *value);
        }
        float(&mut d, nodes_at + INSM_NODE - 4, 1.0);
        float(&mut d, record_at + CAMERA_FOCAL_LENGTH, focal);
        float(&mut d, record_at + CAMERA_FILM_WIDTH, 1.417);

        // The animation: one node without tracks, then the lens track and its keys.
        let lens_at = INSA_HEADER + INSA_NODE;
        let keys_at = lens_at + 16;
        let mut a = vec![0u8; keys_at];
        a[..4].copy_from_slice(insa::MAGIC);
        a[4] = 3;
        a[8] = 1;
        a[LENS_COUNT_AT] = 1;
        word(&mut a, 0x10, 60 * insa::UNITS_PER_FRAME);
        word(&mut a, 0x14, INSA_HEADER as u32);
        word(&mut a, LENS_AT, lens_at as u32);
        word(&mut a, 0x24, 60 * insa::UNITS_PER_FRAME);
        a[INSA_HEADER + 2] = 0xff;
        a[lens_at] = 3;
        a[lens_at + LENS_COUNTS..lens_at + LENS_COUNTS + 2].copy_from_slice(&(focal_keys.len() as u16).to_le_bytes());
        a[lens_at + LENS_COUNTS + 2..lens_at + LENS_COUNTS + 4].copy_from_slice(&1u16.to_le_bytes());
        word(&mut a, lens_at + LENS_OFFSETS, keys_at as u32);
        word(&mut a, lens_at + LENS_OFFSETS + 4, (keys_at + focal_keys.len() * KEY_LEN) as u32);
        for (time, value) in focal_keys.iter().copied().chain([(0, 1.0)]) {
            a.extend(time.to_le_bytes());
            a.extend(value.to_le_bytes());
        }
        d.extend(a);
        d
    }

    #[test]
    fn a_camera_file_gives_the_rig_the_lens_and_the_lens_keys() {
        let d = file([-34.0, 52.0, 197.0], 43.6, &[(0, 43.6), (30 * 1024, 60.0)]);
        let camera = Camera::parse(&d).unwrap();
        assert_eq!(camera.nodes.len(), 1);
        assert_eq!(camera.node, 0);
        assert_eq!(camera.nodes[0].translation, [-34.0, 52.0, 197.0]);
        assert_eq!((camera.focal_length, camera.film_width), (43.6, 1.417));
        assert_eq!(camera.animation.as_ref().unwrap().frames(), 60);
        assert_eq!(camera.focal.len(), 2);
        assert_eq!((camera.focal[1].time, camera.focal[1].value), (30 * 1024, 60.0));
        assert_eq!(camera.zoom.len(), 1);
        assert_eq!(camera.zoom[0].value, 1.0);
    }

    #[test]
    fn a_scenes_cut_has_the_rig_and_the_move_in_a_bundle() {
        // The same camera, split up the way an event's bundle holds a cut: slot 1 a
        // bundle of models, slot 2 a bundle of animations, and the words of the slots
        // it does not use left at zero.
        let whole = file([-34.0, 52.0, 197.0], 43.6, &[(0, 43.6), (30 * 1024, 60.0)]);
        let (model, animation) = whole.split_at(0x80);
        let inner = |part: &[u8]| {
            let mut bundle = vec![0u8; 0x10];
            bundle[..4].copy_from_slice(&[b'S', b'C', 1, 0]);
            bundle[4] = 0x10;
            bundle.extend_from_slice(part);
            bundle.resize(bundle.len().next_multiple_of(16), 0);
            bundle
        };
        let (models, animations) = (inner(model), inner(animation));
        let mut cut = vec![0u8; 0x30];
        cut[..4].copy_from_slice(&[b'S', b'C', 8, 0]);
        cut[8..12].copy_from_slice(&0x30u32.to_le_bytes());
        cut[12..16].copy_from_slice(&((0x30 + models.len()) as u32).to_le_bytes());
        cut.extend_from_slice(&models);
        cut.extend_from_slice(&animations);

        let camera = Camera::parse_cut(&cut).unwrap();
        assert_eq!(camera.nodes[0].translation, [-34.0, 52.0, 197.0]);
        assert_eq!(camera.animation.as_ref().unwrap().frames(), 60);
        assert_eq!(camera.focal.len(), 2);
        // A bundle without a camera in it is not a cut.
        assert!(Camera::parse_cut(&cut[..0x30]).is_err());
    }

    #[test]
    fn a_camera_that_stays_put_has_no_animation() {
        // The stage-select menu's cameras: the file ends after the rig.
        let mut d = file([0.0, 10.0, 0.0], 35.0, &[(0, 35.0)]);
        d.truncate(0x80);
        let camera = Camera::parse(&d).unwrap();
        assert!(camera.animation.is_none());
        assert!(camera.focal.is_empty() && camera.zoom.is_empty());
        assert_eq!(camera.focal_length, 35.0);
    }

    #[test]
    fn a_model_is_not_a_camera_and_a_camera_must_be_one_of_its_nodes() {
        let mut d = file([0.0; 3], 35.0, &[(0, 35.0)]);
        d[0x60 + CAMERA_NODE] = 1;
        assert!(Camera::parse(&d).is_err());
        d[0x60 + CAMERA_NODE] = 0;
        assert!(Camera::parse(&d).is_ok());
        d[CAMERA_COUNT_AT] = 0;
        assert!(Camera::parse(&d).is_err());
    }
}
