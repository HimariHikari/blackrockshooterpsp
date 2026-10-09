//! The interface's layouts: what a screen of the interface is drawn from and how it
//! moves. Three files go together, read as the layout library reads them (0x304C4
//! on, 0x25430, 0x33138):
//!
//! - the objects (`SDOa`, [`Objects`]): rectangles, each with its place on the
//!   screen and the list of the PSP's drawing commands that draws it;
//! - the motions (`XBN`, [`Motions`]): sets of tracks, each of which has a group of
//!   objects play a clip of keys that move, scale, turn and colour it;
//! - the groups (the `.bin` after them, [`Groups`]): which objects each group is.
//!
//! ## Objects
//!
//! `SDOa`, u16 objects, u16 textures; then 16 bytes an object from 8:
//!
//! | off | size | meaning                                              |
//! |-----|------|------------------------------------------------------|
//! | 0   | 2    | bit 0 it is drawn, bit 1 it has a texture            |
//! | 2   | 2    | which of the layout's textures (0xFFFF none)         |
//! | 4   | 2, 2 | where it is on the screen                            |
//! | 8   | 2, 2 | its width and height                                 |
//! | 12  | 4    | offset of its commands                               |
//!
//! Every object that is drawn is one strip of four corners. Its commands are words
//! with the command's number in the top byte, the corners following them: texture
//! on or off (0x1E), dither (0x20), the texture's filter (0xC6), how it is blended
//! (0xDF, with the fixed colour of 0xE1), what a corner is made of (0x12), where the
//! corners are (0x10, 0x01), the strip (0x04) and the end (0x0B). A corner is a
//! texture coordinate (two floats) if it has one, a colour (four bytes) if it has
//! one, and its place (three floats) counted from the object's own.
//!
//! ## Motions
//!
//! `XBN\0`, u32 sets, u32 clips, an offset for each set and then for each clip. A set
//! is u32 tracks and 16 bytes a track: u16 group, u16 clip, u16 frames before the
//! clip starts, u16 flags (bit 0: not drawn), and two floats the group is moved by.
//! A clip is u32 channels; a channel is u32 what it changes ([`Channel`]), u32 keys,
//! a zero, and 16 bytes a key: u32 frame, f32 value, u32 how the way to the next key
//! goes ([`Ease`]), u32 mark ([`Mark`]).
//!
//! ## Groups
//!
//! u32 the file's size, u32 objects, u32 groups; a colour for each object (that of
//! its corners), an offset for each group; a group is u32 how many objects and their
//! numbers.

use anyhow::{bail, ensure, Context, Result};

use crate::reader::Reader;

pub const OBJECTS_MAGIC: &[u8; 4] = b"SDOa";
pub const MOTIONS_MAGIC: &[u8; 4] = b"XBN\0";

const OBJECTS_AT: usize = 8;
const OBJECT_LEN: usize = 16;
const NO_TEXTURE: u16 = 0xffff;
const KIND_DRAWN: u16 = 1;
const KIND_TEXTURED: u16 = 2;
const CORNERS: usize = 4;

// The drawing commands an object's list is made of.
mod command {
    pub const VERTICES_AT: u32 = 0x01;
    pub const PRIMITIVE: u32 = 0x04;
    pub const RETURN: u32 = 0x0b;
    pub const BASE: u32 = 0x10;
    pub const VERTEX_TYPE: u32 = 0x12;
    pub const ORIGIN: u32 = 0x14;
    pub const TEXTURE_ON: u32 = 0x1e;
    pub const DITHER_ON: u32 = 0x20;
    pub const FILTER: u32 = 0xc6;
    pub const BLEND: u32 = 0xdf;
    pub const FIXED_COLOUR: u32 = 0xe1;
}

const PRIMITIVE_STRIP: u32 = 4;
// A corner's parts, in the vertex type: two floats of texture, a colour of four
// bytes, three floats of place.
const VERTEX_TEXTURE_MASK: u32 = 3;
const VERTEX_TEXTURE_FLOAT: u32 = 3;
const VERTEX_COLOUR_MASK: u32 = 7 << 2;
const VERTEX_COLOUR_8888: u32 = 7 << 2;
const VERTEX_PLACE_FLOAT: u32 = 3 << 7;
/// The blend that adds what is drawn to what is there: source by its alpha, plus the
/// destination by the fixed colour, which is white.
const BLEND_ADD: u32 = 0x0a2;

/// A corner of an object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    /// From the object's own place, in the screen's pixels.
    pub position: [f32; 2],
    /// In the texture, 0 to 1.
    pub uv: [f32; 2],
    /// Red, green, blue and alpha; white if the corners have no colour.
    pub colour: [u8; 4],
}

/// What draws an object: a strip of four corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub corners: [Corner; CORNERS],
    /// The texture is drawn on it.
    pub textured: bool,
    /// The texture is smoothed between its pixels.
    pub smooth: bool,
    /// It is added to what is behind it rather than laid over it.
    pub additive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Object {
    /// Which of the layout's textures it is drawn with.
    pub texture: Option<usize>,
    /// Where on the screen its corners are counted from.
    pub position: [i16; 2],
    pub size: [i16; 2],
    /// What draws it; an object that only marks a place has none.
    pub quad: Option<Quad>,
}

#[derive(Clone, Debug)]
pub struct Objects {
    pub objects: Vec<Object>,
    /// How many textures the layout is drawn with.
    pub textures: usize,
}

impl Objects {
    pub fn parse(d: &[u8]) -> Result<Objects> {
        ensure!(d.starts_with(OBJECTS_MAGIC), "not a layout's objects");
        let r = Reader(d);
        let (count, textures) = (r.u16(4)? as usize, r.u16(6)? as usize);
        let objects = (0..count)
            .map(|index| object(d, OBJECTS_AT + index * OBJECT_LEN).with_context(|| format!("object {index}")))
            .collect::<Result<Vec<Object>>>()?;
        Ok(Objects { objects, textures })
    }
}

fn object(d: &[u8], at: usize) -> Result<Object> {
    let r = Reader(d);
    let (kind, texture) = (r.u16(at)?, r.u16(at + 2)?);
    let position = [r.i16(at + 4)?, r.i16(at + 6)?];
    let size = [r.i16(at + 8)?, r.i16(at + 10)?];
    let commands = r.u32(at + 12)? as usize;
    let texture = (kind & KIND_TEXTURED != 0 && texture != NO_TEXTURE).then_some(texture as usize);
    let quad = if kind & KIND_DRAWN != 0 { Some(quad(d, commands)?) } else { None };
    Ok(Object { texture, position, size, quad })
}

/// Reads an object's commands, which start at `origin`.
fn quad(d: &[u8], origin: usize) -> Result<Quad> {
    let r = Reader(d);
    let (mut textured, mut smooth, mut additive) = (false, false, false);
    let (mut vertex_type, mut vertices_at) = (0u32, 0usize);
    let mut corners = None;
    let mut at = origin;
    loop {
        let word = r.u32(at)?;
        let (number, argument) = (word >> 24, word & 0x00ff_ffff);
        at += 4;
        match number {
            // Addresses are counted from the list's start, with nothing above them.
            command::ORIGIN | command::DITHER_ON | command::FIXED_COLOUR => {}
            command::BASE => ensure!(argument == 0, "a base address of {argument:#x}"),
            command::TEXTURE_ON => textured = argument != 0,
            command::FILTER => smooth = argument != 0,
            command::BLEND => additive = argument == BLEND_ADD,
            command::VERTEX_TYPE => vertex_type = argument,
            command::VERTICES_AT => vertices_at = origin + argument as usize,
            command::PRIMITIVE => {
                let (kind, count) = ((argument >> 16) & 7, (argument & 0xffff) as usize);
                ensure!(kind == PRIMITIVE_STRIP && count == CORNERS, "a primitive of kind {kind} with {count} corners");
                ensure!(corners.is_none(), "more than one strip");
                corners = Some(read_corners(d, vertices_at, vertex_type)?);
            }
            command::RETURN => break,
            other => bail!("drawing command {other:#x}"),
        }
    }
    Ok(Quad { corners: corners.context("commands that draw nothing")?, textured, smooth, additive })
}

fn read_corners(d: &[u8], at: usize, vertex_type: u32) -> Result<[Corner; CORNERS]> {
    let has_uv = match vertex_type & VERTEX_TEXTURE_MASK {
        0 => false,
        VERTEX_TEXTURE_FLOAT => true,
        _ => bail!("corners of type {vertex_type:#x}"),
    };
    let has_colour = match vertex_type & VERTEX_COLOUR_MASK {
        0 => false,
        VERTEX_COLOUR_8888 => true,
        _ => bail!("corners of type {vertex_type:#x}"),
    };
    let rest = vertex_type & !(VERTEX_TEXTURE_MASK | VERTEX_COLOUR_MASK);
    ensure!(rest == VERTEX_PLACE_FLOAT, "corners of type {vertex_type:#x}");

    let r = Reader(d);
    let mut at = at;
    let mut corners = [Corner { position: [0.0; 2], uv: [0.0; 2], colour: [255; 4] }; CORNERS];
    for corner in &mut corners {
        if has_uv {
            corner.uv = r.f32s(at)?;
            at += 8;
        }
        if has_colour {
            corner.colour = r.bytes(at)?;
            at += 4;
        }
        let [x, y, _depth]: [f32; 3] = r.f32s(at)?;
        corner.position = [x, y];
        at += 12;
    }
    Ok(corners)
}

/// What a channel of a clip changes of the group that plays it (the jump table at
/// 0x166658): the numbers are the file's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    X = 1,
    Y = 2,
    ScaleX = 4,
    ScaleY = 5,
    /// In degrees.
    Turn = 9,
    /// The colours are 0 to 255.
    Red = 11,
    Green = 12,
    Blue = 13,
    Alpha = 14,
}

impl Channel {
    /// Every channel, in the order a track keeps their values in.
    pub const ALL: [Channel; 9] =
        [Channel::X, Channel::Y, Channel::ScaleX, Channel::ScaleY, Channel::Turn, Channel::Red, Channel::Green, Channel::Blue, Channel::Alpha];

    fn of(number: u32) -> Result<Channel> {
        Self::ALL.into_iter().find(|channel| *channel as u32 == number).with_context(|| format!("a channel of kind {number}"))
    }

    /// Its place among [`Channel::ALL`].
    pub fn slot(self) -> usize {
        Self::ALL.iter().position(|channel| *channel == self).expect("every channel is listed")
    }
}

/// How a value goes from a key to the next (0x2F8B8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    /// Evenly.
    Even,
    /// Slowly at first.
    In,
    /// Slowly at the end.
    Out,
}

/// What a key is to a clip that goes round (0x2FC4C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    None,
    /// Where the clip goes back to.
    LoopStart,
    /// Where it goes back from.
    LoopEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub frame: u32,
    pub value: f32,
    pub ease: Ease,
    pub mark: Mark,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    /// Which group of objects it moves.
    pub group: u16,
    /// Which clip it plays.
    pub clip: u16,
    /// Frames before the clip starts.
    pub delay: u16,
    /// It is not drawn.
    pub hidden: bool,
    /// How far the group is moved from where its objects are.
    pub offset: [f32; 2],
}

/// A clip: the keys of each channel it has.
pub type Clip = Vec<(Channel, Vec<Key>)>;

#[derive(Clone, Debug)]
pub struct Motions {
    pub sets: Vec<Vec<Track>>,
    pub clips: Vec<Clip>,
}

impl Motions {
    const TRACK_LEN: usize = 16;
    const CHANNEL_HEADER_LEN: usize = 12;
    const KEY_LEN: usize = 16;

    pub fn parse(d: &[u8]) -> Result<Motions> {
        ensure!(d.starts_with(MOTIONS_MAGIC), "not a layout's motions");
        let r = Reader(d);
        let (set_count, clip_count) = (r.u32(4)? as usize, r.u32(8)? as usize);
        ensure!(12 + (set_count + clip_count) * 4 <= d.len(), "file ends inside its offsets");
        let offset = |index: usize| Ok::<usize, anyhow::Error>(r.u32(12 + index * 4)? as usize);

        let mut sets = Vec::with_capacity(set_count);
        for set in 0..set_count {
            let at = offset(set)?;
            let count = r.u32(at)? as usize;
            ensure!(at + 4 + count * Self::TRACK_LEN <= d.len(), "set {set} ends outside the file");
            let tracks = (0..count)
                .map(|index| {
                    let at = at + 4 + index * Self::TRACK_LEN;
                    let clip = r.u16(at + 2)?;
                    ensure!((clip as usize) < clip_count, "set {set} plays clip {clip} of {clip_count}");
                    Ok(Track { group: r.u16(at)?, clip, delay: r.u16(at + 4)?, hidden: r.u16(at + 6)? & 1 != 0, offset: r.f32s(at + 8)? })
                })
                .collect::<Result<Vec<Track>>>()?;
            sets.push(tracks);
        }

        let mut clips = Vec::with_capacity(clip_count);
        for clip in 0..clip_count {
            let mut at = offset(set_count + clip)?;
            let count = r.u32(at)? as usize;
            at += 4;
            let mut channels = Vec::with_capacity(count.min(Channel::ALL.len()));
            for _ in 0..count {
                let channel = Channel::of(r.u32(at)?).with_context(|| format!("clip {clip}"))?;
                let keys = r.u32(at + 4)? as usize;
                at += Self::CHANNEL_HEADER_LEN;
                ensure!(at + keys * Self::KEY_LEN <= d.len(), "clip {clip} ends outside the file");
                let keys = (0..keys)
                    .map(|index| {
                        let at = at + index * Self::KEY_LEN;
                        let ease = match r.u32(at + 8)? {
                            0 => Ease::Even,
                            1 => Ease::In,
                            2 => Ease::Out,
                            other => bail!("clip {clip} has a key that goes on in way {other}"),
                        };
                        let mark = match r.u32(at + 12)? {
                            0 => Mark::None,
                            1 => Mark::LoopStart,
                            2 => Mark::LoopEnd,
                            other => bail!("clip {clip} has a key marked {other}"),
                        };
                        Ok(Key { frame: r.u32(at)?, value: r.f32(at + 4)?, ease, mark })
                    })
                    .collect::<Result<Vec<Key>>>()?;
                ensure!(keys.windows(2).all(|pair| pair[0].frame <= pair[1].frame), "clip {clip} has keys out of order");
                at += keys.len() * Self::KEY_LEN;
                channels.push((channel, keys));
            }
            clips.push(channels);
        }
        Ok(Motions { sets, clips })
    }
}

/// Which objects each group is, and the colour of each object.
#[derive(Clone, Debug)]
pub struct Groups {
    pub colours: Vec<[u8; 4]>,
    pub groups: Vec<Vec<usize>>,
}

impl Groups {
    const HEADER_LEN: usize = 12;

    pub fn parse(d: &[u8]) -> Result<Groups> {
        let r = Reader(d);
        let (size, objects, count) = (r.u32(0)? as usize, r.u32(4)? as usize, r.u32(8)? as usize);
        ensure!(size == d.len(), "the file says it is {size} bytes but is {}", d.len());
        let offsets_at = Self::HEADER_LEN + objects * 4;
        ensure!(offsets_at + count * 4 <= d.len(), "file ends inside its offsets");
        let colours = (0..objects).map(|index| r.bytes(Self::HEADER_LEN + index * 4)).collect::<Result<Vec<[u8; 4]>>>()?;
        let groups = (0..count)
            .map(|group| {
                let at = r.u32(offsets_at + group * 4)? as usize;
                let members = r.u32(at)? as usize;
                ensure!(at + 4 + members * 4 <= d.len(), "group {group} ends outside the file");
                (0..members)
                    .map(|index| {
                        let object = r.u32(at + 4 + index * 4)? as usize;
                        ensure!(object < objects, "group {group} has object {object} of {objects}");
                        Ok(object)
                    })
                    .collect::<Result<Vec<usize>>>()
            })
            .collect::<Result<Vec<Vec<usize>>>>()?;
        Ok(Groups { colours, groups })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// An object's record and, at `commands`, its list: a strip with a texture and
    /// colours, as most are.
    fn objects() -> Vec<u8> {
        let mut d = OBJECTS_MAGIC.to_vec();
        d.extend([2u16, 1].iter().flat_map(|value| value.to_le_bytes()));
        // A mark at (48, 229), and a rectangle of 101 by 34 at (375, 237).
        d.extend([0u16, 0xffff].iter().flat_map(|value| value.to_le_bytes()));
        d.extend([48i16, 229, 0, 0].iter().flat_map(|value| value.to_le_bytes()));
        d.extend(0u32.to_le_bytes());
        d.extend([3u16, 0].iter().flat_map(|value| value.to_le_bytes()));
        d.extend([375i16, 237, 101, 34].iter().flat_map(|value| value.to_le_bytes()));
        d.extend(0x28u32.to_le_bytes());
        assert_eq!(d.len(), 0x28);
        d.extend(words(&[0x1400_0000, 0x1e00_0001, 0x2000_0001, 0xc600_0101, 0xdf00_0032, 0x1200_019f, 0x1000_0000, 0x0100_0028, 0x0404_0004, 0x0b00_0000]));
        for (u, v, colour, x, y) in [(0.0f32, 0.0f32, 0xff14_1414u32, 0.0f32, 0.0f32), (0.0, 0.5, 0xff14_1414, 0.0, 34.0), (0.75, 0.0, 0x8056_483a, 101.0, 0.0), (0.75, 0.5, 0x8056_483a, 101.0, 34.0)] {
            d.extend(u.to_le_bytes());
            d.extend(v.to_le_bytes());
            d.extend(colour.to_le_bytes());
            d.extend([x, y, 0.0].iter().flat_map(|value| value.to_le_bytes()));
        }
        d
    }

    #[test]
    fn an_object_is_a_place_and_the_strip_its_commands_draw() {
        let parsed = Objects::parse(&objects()).unwrap();
        assert_eq!((parsed.objects.len(), parsed.textures), (2, 1));
        let mark = &parsed.objects[0];
        assert_eq!((mark.position, mark.quad, mark.texture), ([48, 229], None, None));
        let rectangle = &parsed.objects[1];
        assert_eq!((rectangle.position, rectangle.size, rectangle.texture), ([375, 237], [101, 34], Some(0)));
        let quad = rectangle.quad.unwrap();
        assert!(quad.textured && quad.smooth && !quad.additive);
        assert_eq!(quad.corners[1], Corner { position: [0.0, 34.0], uv: [0.0, 0.5], colour: [0x14, 0x14, 0x14, 0xff] });
        assert_eq!(quad.corners[2], Corner { position: [101.0, 0.0], uv: [0.75, 0.0], colour: [0x3a, 0x48, 0x56, 0x80] });
    }

    #[test]
    fn corners_may_lack_a_texture_or_a_colour_and_a_blend_may_add() {
        let mut d = objects();
        // No texture, no filter, the adding blend, and corners that are a colour and
        // a place.
        let list = 0x28;
        d.truncate(list);
        d.extend(words(&[0x1400_0000, 0x1e00_0000, 0xc600_0000, 0xdf00_00a2, 0xe1ff_ffff, 0x1200_019c, 0x1000_0000, 0x0100_0028, 0x0404_0004, 0x0b00_0000]));
        for (x, y) in [(0.0f32, 0.0f32), (0.0, 1.0), (61.0, 0.0), (61.0, 1.0)] {
            d.extend(0x80e6_e6e6u32.to_le_bytes());
            d.extend([x, y, 0.0].iter().flat_map(|value| value.to_le_bytes()));
        }
        let quad = Objects::parse(&d).unwrap().objects[1].quad.unwrap();
        assert!(!quad.textured && !quad.smooth && quad.additive);
        assert_eq!(quad.corners[3], Corner { position: [61.0, 1.0], uv: [0.0, 0.0], colour: [0xe6, 0xe6, 0xe6, 0x80] });

        // A command the lists do not have, and a list that draws nothing.
        let mut unknown = d.clone();
        unknown[list + 3] = 0x15;
        assert!(Objects::parse(&unknown).is_err());
        let mut empty = d.clone();
        empty[list..list + 4].copy_from_slice(&0x0b00_0000u32.to_le_bytes());
        assert!(Objects::parse(&empty).is_err());
    }

    fn motions() -> Vec<u8> {
        // One set of two tracks; two clips, the first empty, the second an alpha
        // that goes round.
        let mut d = MOTIONS_MAGIC.to_vec();
        d.extend(words(&[1, 2, 0x18, 0x3c, 0x40]));
        assert_eq!(d.len(), 0x18);
        d.extend(2u32.to_le_bytes());
        d.extend([2u16, 1, 4, 0].iter().flat_map(|value| value.to_le_bytes()));
        d.extend([1.5f32, -2.0].iter().flat_map(|value| value.to_le_bytes()));
        d.extend([0u16, 0, 0, 1].iter().flat_map(|value| value.to_le_bytes()));
        d.extend([0.0f32, 0.0].iter().flat_map(|value| value.to_le_bytes()));
        assert_eq!(d.len(), 0x3c);
        d.extend(0u32.to_le_bytes());
        d.extend(words(&[1, 14, 3, 0]));
        for (frame, value, ease, mark) in [(4u32, 0.0f32, 0u32, 0u32), (8, 255.0, 1, 1), (82, 100.0, 2, 2)] {
            d.extend(frame.to_le_bytes());
            d.extend(value.to_le_bytes());
            d.extend(ease.to_le_bytes());
            d.extend(mark.to_le_bytes());
        }
        d
    }

    #[test]
    fn a_set_is_tracks_and_a_clip_is_keys_by_channel() {
        let parsed = Motions::parse(&motions()).unwrap();
        assert_eq!(parsed.sets.len(), 1);
        assert_eq!(parsed.sets[0][0], Track { group: 2, clip: 1, delay: 4, hidden: false, offset: [1.5, -2.0] });
        assert_eq!(parsed.sets[0][1], Track { group: 0, clip: 0, delay: 0, hidden: true, offset: [0.0, 0.0] });
        assert!(parsed.clips[0].is_empty());
        let (channel, keys) = &parsed.clips[1][0];
        assert_eq!((*channel, keys.len()), (Channel::Alpha, 3));
        assert_eq!(keys[1], Key { frame: 8, value: 255.0, ease: Ease::In, mark: Mark::LoopStart });
        assert_eq!(keys[2], Key { frame: 82, value: 100.0, ease: Ease::Out, mark: Mark::LoopEnd });
        assert_eq!(Channel::Alpha.slot(), 8);
        assert_eq!(Channel::Turn.slot(), 4);

        // A channel of a kind there is not, and a track that plays a clip there is not.
        let mut d = motions();
        d[0x44] = 3;
        assert!(Motions::parse(&d).is_err());
        let mut d = motions();
        d[0x1e] = 2;
        assert!(Motions::parse(&d).is_err());
    }

    #[test]
    fn a_group_is_a_list_of_objects() {
        let mut d = words(&[0, 3, 2]);
        d.extend([0xff, 0xff, 0x0f, 0xff, 0x86, 0x64, 0x0a, 0xa5, 0xff, 0xff, 0xff, 0xff]);
        d.extend(words(&[0x20, 0x28]));
        d.extend(words(&[1, 2]));
        d.extend(words(&[2, 0, 1]));
        let size = d.len() as u32;
        d[..4].copy_from_slice(&size.to_le_bytes());
        let parsed = Groups::parse(&d).unwrap();
        assert_eq!(parsed.colours, [[0xff, 0xff, 0x0f, 0xff], [0x86, 0x64, 0x0a, 0xa5], [0xff; 4]]);
        assert_eq!(parsed.groups, [vec![2], vec![0, 1]]);

        // An object the layout has not, and a size that is not the file's.
        let mut bad = d.clone();
        bad[0x24] = 3;
        assert!(Groups::parse(&bad).is_err());
        d.push(0);
        assert!(Groups::parse(&d).is_err());
    }
}
