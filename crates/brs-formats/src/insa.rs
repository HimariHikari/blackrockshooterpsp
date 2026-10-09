//! `INSA` animations (`.anm`, `.insa`, and the second half of a `.cam`).
//!
//! One entry per skeleton node. A node's transform is built from parts, each a record
//! in the file, applied one on top of the other in the file's order: that is how the
//! original's evaluator works (0x20614), and it is why one reader does for characters
//! in battle (scale, quaternion, translation), for event scenes (angles, and the fixed
//! turn the authoring tool gives each joint) and for objects that turn about a pivot
//! (nine records). See `docs/FORMAT_NOTES.md` for the layout.

use anyhow::{bail, ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"INSA";

const NODE_LEN: usize = 0x24;
const TRACK_LEN: usize = 0x18;

const KIND_TRANSLATION: u8 = 0;
const KIND_ANGLES: u8 = 1;
const KIND_SCALE: u8 = 2;
const KIND_UNSCALE: u8 = 3;
const KIND_ROTATION: u8 = 4;

/// Mode bit: one channel does for all three axes (the original tests it at 0x20B88).
const MODE_SHARED: u8 = 2;

/// How a track carries on outside its keys: this value repeats them, any other holds
/// the nearest key (0x1EA88).
const WRAP_REPEAT: u16 = 1;

/// Time units per animation frame. The original's animation player counts in frames
/// and converts with this factor (0xA8CD8 in the executable).
pub const UNITS_PER_FRAME: u32 = 1024;
/// Animation frames per second: the player is advanced by the time step in sixtieths
/// of a second. (The player's walk cycle confirms it: 70 frames, during which the
/// 40/7 units per thirtieth of a second her script asks for cover exactly 200 units.)
pub const FRAMES_PER_SECOND: f32 = 60.0;

/// The first header version that stores its own loop range.
const VERSION_WITH_LOOP: u8 = 2;
/// Node flag: this animation does not drive the node. Body animations set it on the
/// hair, and the hair animation that plays alongside sets it on everything else.
const NODE_NOT_DRIVEN: u8 = 0x01;
/// Node flag: the node has nodes under it.
const NODE_HAS_CHILD: u8 = 0x02;
const NO_NODE: u8 = 0xff;

#[derive(Clone, Debug)]
pub struct Animation {
    /// First and last time of the clip, in the file's time units. Key times are on
    /// the same scale.
    pub start: u32,
    pub end: u32,
    /// The stretch a looping playback repeats; the whole clip unless the file says
    /// otherwise.
    pub loop_start: u32,
    pub loop_end: u32,
    pub nodes: Vec<NodeTracks>,
}

/// What an animation does with one node.
#[derive(Clone, Debug, Default)]
pub struct NodeTracks {
    /// Same identifier as [`crate::insm::Node::id`].
    pub id: u8,
    /// The animation leaves this node to whatever else is posing it.
    pub masked: bool,
    /// The node has nodes under it in the skeleton the animation was made on.
    pub has_child: bool,
    /// The first node after this one and all that is under it.
    pub next: Option<usize>,
    /// Where the node sits under its parent in the skeleton the animation was made
    /// on. On a model built to other proportions the node's translation is measured
    /// against this.
    pub reference: [f32; 3],
    /// What the node's transform is made of, innermost first: each part applies on
    /// top of those before it. A node without parts sits exactly on its parent.
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug)]
pub enum Part {
    Translation([Channel; 3]),
    Scale([Channel; 3]),
    /// The same factor on every axis.
    UniformScale(Channel),
    /// A turn given as three angles in radians.
    Angles { order: Order, channels: [Channel; 3] },
    /// A turn given as quaternions `(x, y, z, w)`.
    Rotation(Rotation),
    /// Takes the parent's scale back out, so that a node under a scaled parent keeps
    /// its own size and only its place under the parent scales.
    UnscaleParent,
}

/// The order a part's three angles turn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// About X, then Y, then Z. Every file uses this one.
    Xyz,
    /// About Z, then X, then Y.
    Zxy,
}

/// One number over time.
#[derive(Clone, Debug, Default)]
pub struct Channel {
    /// In order of time. A channel that never changes has one key.
    pub keys: Vec<Key<f32>>,
    /// Whether the keys repeat before the first of them and after the last, rather
    /// than the nearest holding.
    pub repeat_before: bool,
    pub repeat_after: bool,
}

/// A quaternion over time.
#[derive(Clone, Debug, Default)]
pub struct Rotation {
    pub keys: Vec<Key<[f32; 4]>>,
    pub repeat_before: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Key<T> {
    pub time: u32,
    pub value: T,
}

impl Animation {
    pub fn parse(d: &[u8]) -> Result<Animation> {
        ensure!(d.starts_with(MAGIC), "not an INSA file");
        let r = Reader(d);
        let version = r.u8(0x04)?;
        let node_count = r.u16(0x08)? as usize;
        let (start, end) = (r.u32(0x0C)?, r.u32(0x10)?);
        let nodes_at = r.u32(0x14)? as usize;
        let (loop_start, loop_end) =
            if version >= VERSION_WITH_LOOP { (r.u32(0x20)?, r.u32(0x24)?) } else { (start, end) };

        let mut nodes = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let at = nodes_at + i * NODE_LEN;
            let flags = r.u8(at)?;
            let track_count = r.u8(at + 1)? as usize;
            let next = r.u8(at + 3)?;
            let tracks_at = r.u32(at + 4)? as usize;
            let mut node = NodeTracks {
                id: r.u8(at + 2)?,
                masked: flags & NODE_NOT_DRIVEN != 0,
                has_child: flags & NODE_HAS_CHILD != 0,
                next: (next != NO_NODE).then_some(next as usize),
                reference: r.f32s(at + 8)?,
                parts: Vec::with_capacity(track_count),
            };
            for t in 0..track_count {
                let part = read_part(&r, tracks_at + t * TRACK_LEN).with_context(|| format!("node {i} track {t}"))?;
                node.parts.extend(part);
            }
            nodes.push(node);
        }
        Ok(Animation { start, end, loop_start, loop_end, nodes })
    }

    /// Length in whole frames, rounded down as the original does.
    pub fn frames(&self) -> u32 {
        self.end.saturating_sub(self.start) / UNITS_PER_FRAME
    }

    /// The loop range in whole frames from the start of the clip.
    pub fn loop_frames(&self) -> (u32, u32) {
        let frame = |time: u32| time.saturating_sub(self.start) / UNITS_PER_FRAME;
        (frame(self.loop_start), frame(self.loop_end))
    }
}

impl NodeTracks {
    /// The time of every key of every part.
    pub fn key_times(&self) -> impl Iterator<Item = u32> + '_ {
        self.parts.iter().flat_map(|part| -> Box<dyn Iterator<Item = u32> + '_> {
            match part {
                Part::Translation(channels) | Part::Scale(channels) | Part::Angles { channels, .. } => {
                    Box::new(channels.iter().flat_map(|channel| channel.keys.iter().map(|key| key.time)))
                }
                Part::UniformScale(channel) => Box::new(channel.keys.iter().map(|key| key.time)),
                Part::Rotation(rotation) => Box::new(rotation.keys.iter().map(|key| key.time)),
                Part::UnscaleParent => Box::new(std::iter::empty()),
            }
        })
    }
}

/// One track record. A record whose kind of turn the original does not know either
/// comes to nothing.
fn read_part(r: &Reader, at: usize) -> Result<Option<Part>> {
    let (kind, mode) = (r.u8(at)?, r.u8(at + 1)?);
    Ok(Some(match kind {
        KIND_UNSCALE => Part::UnscaleParent,
        KIND_ROTATION => {
            let count = r.u16(at + 4)? as usize;
            // Bits 2-3 say how it carries on before its first key. The original reads
            // the other end's from a bit that can never say "repeat" (0x2086C).
            let repeat_before = u16::from(r.u8(at + 6)? >> 2) & 3 == WRAP_REPEAT;
            let mut keys = Vec::with_capacity(count.max(1));
            if count == 0 {
                // No keys: the record holds the one rotation itself.
                keys.push(Key { time: 0, value: r.f32s(at + 8)? });
            } else {
                let keys_at = r.u32(at + 8)? as usize;
                for k in 0..count {
                    let key_at = keys_at + k * 20;
                    keys.push(Key { time: r.u32(key_at)?, value: r.f32s(key_at + 4)? });
                }
            }
            Part::Rotation(Rotation { keys, repeat_before })
        }
        KIND_TRANSLATION | KIND_ANGLES | KIND_SCALE => {
            // Two bits for each end of each channel, the first channel's highest.
            let wraps = r.u16(at + 10)?;
            let channel = |axis: usize| -> Result<Channel> {
                let count = r.u16(at + 4 + axis * 2)? as usize;
                let wrap = wraps >> (8 - axis * 4);
                let mut keys = Vec::with_capacity(count.max(1));
                if count == 0 {
                    // No keys: the offset's place holds the value itself.
                    keys.push(Key { time: 0, value: r.f32(at + 12 + axis * 4)? });
                } else {
                    let keys_at = r.u32(at + 12 + axis * 4)? as usize;
                    for k in 0..count {
                        let key_at = keys_at + k * 8;
                        keys.push(Key { time: r.u32(key_at)?, value: r.f32(key_at + 4)? });
                    }
                }
                Ok(Channel { keys, repeat_before: (wrap >> 2) & 3 == WRAP_REPEAT, repeat_after: wrap & 3 == WRAP_REPEAT })
            };
            if mode & MODE_SHARED != 0 {
                ensure!(kind == KIND_SCALE, "track kind {kind} with one channel for three axes");
                return Ok(Some(Part::UniformScale(channel(0)?)));
            }
            let channels = [channel(0)?, channel(1)?, channel(2)?];
            match kind {
                KIND_TRANSLATION => Part::Translation(channels),
                KIND_SCALE => Part::Scale(channels),
                // The original takes 0 and 2 for the same order (0x210CC), and turns
                // nothing for a number it does not know.
                _ => match r.u8(at + 2)? {
                    0 | 2 => Part::Angles { order: Order::Xyz, channels },
                    4 => Part::Angles { order: Order::Zxy, channels },
                    _ => return Ok(None),
                },
            }
        }
        _ => bail!("unknown track kind {kind} (mode {mode})"),
    }))
}

impl Channel {
    /// A channel that never changes.
    pub fn constant(value: f32) -> Channel {
        Channel { keys: vec![Key { time: 0, value }], ..Default::default() }
    }

    /// A channel of `(time, value)` keys that holds its ends.
    pub fn keyed(keys: &[(u32, f32)]) -> Channel {
        Channel { keys: keys.iter().map(|&(time, value)| Key { time, value }).collect(), ..Default::default() }
    }

    /// The value at `time`, straight between the keys on either side of it. Without
    /// keys it is zero.
    pub fn at(&self, time: f32) -> f32 {
        let time = wrapped(&self.keys, time, self.repeat_before, self.repeat_after);
        sample(&self.keys, time).unwrap_or(0.0)
    }
}

impl Rotation {
    /// A rotation that never changes.
    pub fn constant(value: [f32; 4]) -> Rotation {
        Rotation { keys: vec![Key { time: 0, value }], repeat_before: false }
    }

    /// The rotation at `time`, turned between the keys on either side of it. Without
    /// keys it is no turn at all.
    pub fn at(&self, time: f32) -> [f32; 4] {
        let time = wrapped(&self.keys, time, self.repeat_before, false);
        match bracket(&self.keys, time) {
            Some((a, b, t)) if t > 0.0 => slerp(a, b, t),
            Some((a, _, _)) => a,
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// `time` brought within the keys of a track that repeats (0x1EA88): before the first
/// key it counts back from the last, after the last on from the first.
fn wrapped<T>(keys: &[Key<T>], time: f32, repeat_before: bool, repeat_after: bool) -> f32 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return time };
    let (first, last) = (i64::from(first.time), i64::from(last.time));
    let at = time as i64;
    if last <= first {
        return time;
    }
    if at < first && repeat_before {
        (last - (first - at) % (last - first)) as f32
    } else if at > last && repeat_after {
        // Landing on the first key exactly counts as the last.
        match (at - first) % (last - first) {
            0 => last as f32,
            over => (first + over) as f32,
        }
    } else {
        time
    }
}

/// Linearly interpolated value of a scalar track at `time`, clamped to its ends.
pub fn sample(keys: &[Key<f32>], time: f32) -> Option<f32> {
    let (a, b, t) = bracket(keys, time)?;
    Some(a + (b - a) * t)
}

/// The original's way between two rotations (0x162CAC): along the shorter arc at an
/// even rate, and straight across when they are nearly the same.
pub fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    /// The cosine above which the original goes straight across.
    const NEAR: f32 = 0.998_046_9;
    /// The sine below which it does not turn at all.
    const NO_TURN: f32 = 5e-5;
    let mix = |b: [f32; 4], from: f32, to: f32| std::array::from_fn(|i| a[i] * from + b[i] * to);
    let cosine: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>().clamp(-1.0, 1.0);
    if cosine >= NEAR {
        // The original leaves this as it comes; the port brings it back to unit
        // length, which it is off by a thousandth at most.
        let q: [f32; 4] = mix(b, 1.0 - t, t);
        let length = q.iter().map(|x| x * x).sum::<f32>().sqrt();
        return if length > 1e-6 { q.map(|x| x / length) } else { a };
    }
    let angle = cosine.abs().acos();
    let sine = angle.sin();
    if sine < NO_TURN {
        return a;
    }
    let b = if cosine < 0.0 { b.map(|x| -x) } else { b };
    mix(b, ((1.0 - t) * angle).sin() / sine, (t * angle).sin() / sine)
}

/// The two key values around `time` and the blend factor between them.
fn bracket<T: Copy>(keys: &[Key<T>], time: f32) -> Option<(T, T, f32)> {
    let (first, last) = (keys.first()?, keys.last()?);
    if time <= first.time as f32 {
        return Some((first.value, first.value, 0.0));
    }
    if time >= last.time as f32 {
        return Some((last.value, last.value, 0.0));
    }
    let next = keys.partition_point(|k| k.time as f32 <= time);
    let (a, b) = (&keys[next - 1], &keys[next]);
    let span = (b.time - a.time) as f32;
    Some((a.value, b.value, if span > 0.0 { (time - a.time as f32) / span } else { 0.0 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of one node with the given track records, whose keys follow them.
    fn file(records: &[[u8; TRACK_LEN]], keys: &[u8]) -> Vec<u8> {
        let mut d = vec![0u8; 0x28];
        d[..4].copy_from_slice(MAGIC);
        d[4] = 3;
        d[8] = 1;
        d[0x10..0x14].copy_from_slice(&(60 * UNITS_PER_FRAME).to_le_bytes());
        d[0x14..0x18].copy_from_slice(&0x28u32.to_le_bytes());
        d[0x24..0x28].copy_from_slice(&(60 * UNITS_PER_FRAME).to_le_bytes());
        let mut node = [0u8; NODE_LEN];
        node[0] = NODE_HAS_CHILD;
        node[1] = records.len() as u8;
        node[2] = 7;
        node[3] = NO_NODE;
        node[4..8].copy_from_slice(&((0x28 + NODE_LEN) as u32).to_le_bytes());
        node[8..12].copy_from_slice(&1.5f32.to_le_bytes());
        d.extend_from_slice(&node);
        for record in records {
            d.extend_from_slice(record);
        }
        d.extend_from_slice(keys);
        d
    }

    /// Where the keys of a file from [`file`] with `records` records begin.
    fn keys_at(records: usize) -> u32 {
        (0x28 + NODE_LEN + records * TRACK_LEN) as u32
    }

    fn record(kind: u8, mode: u8) -> [u8; TRACK_LEN] {
        let mut record = [0u8; TRACK_LEN];
        (record[0], record[1]) = (kind, mode);
        record
    }

    fn put_f32(record: &mut [u8; TRACK_LEN], at: usize, value: f32) {
        record[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn a_scene_node_is_read_part_by_part() {
        // What an event scene gives a joint: one scale for all axes, keyed angles,
        // the joint's fixed turn, the parent's scale taken out, a place.
        let mut scale = record(KIND_SCALE, 3);
        scale[4] = 1;
        scale[12..16].copy_from_slice(&keys_at(5).to_le_bytes());
        let mut angles = record(KIND_ANGLES, 1);
        angles[4] = 2;
        angles[12..16].copy_from_slice(&(keys_at(5) + 8).to_le_bytes());
        put_f32(&mut angles, 16, 0.25);
        let mut orient = record(KIND_ANGLES, 0);
        put_f32(&mut orient, 12, -1.5);
        let mut place = record(KIND_TRANSLATION, 0);
        put_f32(&mut place, 20, 9.0);
        let mut keys = Vec::new();
        for (time, value) in [(0u32, 2.0f32), (0, 0.0), (2048, 1.0)] {
            keys.extend_from_slice(&time.to_le_bytes());
            keys.extend_from_slice(&value.to_le_bytes());
        }

        let animation = Animation::parse(&file(&[scale, angles, orient, record(KIND_UNSCALE, 0), place], &keys)).unwrap();
        let node = &animation.nodes[0];
        assert_eq!((node.id, node.masked, node.has_child, node.next), (7, false, true, None));
        assert_eq!(node.reference, [1.5, 0.0, 0.0]);
        let [Part::UniformScale(scale), Part::Angles { order, channels }, Part::Angles { channels: orient, .. }, Part::UnscaleParent, Part::Translation(place)] =
            &node.parts[..]
        else {
            panic!("{:?}", node.parts)
        };
        assert_eq!(scale.at(500.0), 2.0);
        assert_eq!(*order, Order::Xyz);
        // Half way between its two keys; the axes without keys hold what the record says.
        assert_eq!([channels[0].at(1024.0), channels[1].at(1024.0), channels[2].at(1024.0)], [0.5, 0.25, 0.0]);
        assert_eq!(orient[0].at(0.0), -1.5);
        assert_eq!([place[0].at(0.0), place[2].at(9999.0)], [0.0, 9.0]);
    }

    #[test]
    fn a_character_node_has_quaternion_keys() {
        let mut rotation = record(KIND_ROTATION, 1);
        rotation[4] = 2;
        rotation[8..12].copy_from_slice(&keys_at(1).to_le_bytes());
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let mut keys = Vec::new();
        for (time, value) in [(0u32, [0.0, 0.0, 0.0, 1.0f32]), (1024, [0.0, half, 0.0, half])] {
            keys.extend_from_slice(&time.to_le_bytes());
            for part in value {
                keys.extend_from_slice(&part.to_le_bytes());
            }
        }
        let animation = Animation::parse(&file(&[rotation], &keys)).unwrap();
        let [Part::Rotation(rotation)] = &animation.nodes[0].parts[..] else { panic!() };
        // Half way through a quarter turn about Y is an eighth of a turn.
        let middle = rotation.at(512.0);
        let eighth = [0.0, (std::f32::consts::PI / 8.0).sin(), 0.0, (std::f32::consts::PI / 8.0).cos()];
        assert!(middle.iter().zip(&eighth).all(|(a, b)| (a - b).abs() < 1e-5), "{middle:?}");
        assert_eq!(rotation.at(5000.0), [0.0, half, 0.0, half]);
    }

    #[test]
    fn a_rotation_is_met_along_the_shorter_arc() {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        // The same quarter turn with its sign the other way round: no further off.
        let q = slerp([0.0, 0.0, 0.0, 1.0], [0.0, -half, 0.0, -half], 0.5);
        let eighth = (std::f32::consts::PI / 8.0).sin();
        assert!((q[1] - eighth).abs() < 1e-5 && q[3] > 0.0, "{q:?}");
        // Nearly the same: straight across, and still of unit length.
        let near = slerp([0.0, 0.0, 0.0, 1.0], [0.0, 0.02, 0.0, 0.9998], 0.5);
        assert!((near.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-6);
        assert!((near[1] - 0.01).abs() < 1e-4);
    }

    #[test]
    fn a_track_can_repeat_outside_its_keys() {
        let mut channel = Channel::keyed(&[(1000, 0.0), (2000, 10.0)]);
        // Held at its ends unless it says otherwise.
        assert_eq!((channel.at(0.0), channel.at(2500.0)), (0.0, 10.0));
        channel.repeat_after = true;
        assert_eq!(channel.at(2500.0), 5.0);
        // Landing exactly on a whole turn is the last key, not the first.
        assert_eq!(channel.at(3000.0), 10.0);
        channel.repeat_before = true;
        assert_eq!(channel.at(750.0), 7.5);
    }
}
