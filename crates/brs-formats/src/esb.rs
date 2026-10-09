//! An effect's script: the `.esb` part of an effect file (`efc`), which says what
//! the effect lets go, when, and how each thing then moves and looks
//! (`docs/FORMAT_NOTES.md`, "Effects"; the game's readers are at 0x127EE0,
//! 0x128234, 0x11E05C and 0x125708).
//!
//! A script is a timeline of entries, each with an emitter. An emitter has a place
//! that moves, and lets particles go from a shape about it, so many at a time and
//! every so many frames; a particle is a flat picture or a model, and has a size, a
//! speed, a colour and a turning that change over its life. Each of those is a run
//! of keys by frame, mixed straight from one key to the next.
//!
//! All offsets in a script are of 16 bits from its start, and nought is none.
//! Records carry some of their fields before the place they are pointed at.

use anyhow::{bail, ensure, Context, Result};

use crate::reader::Reader;

/// The frame that ends a run of keys, and the timeline.
const END: u16 = 0x4000;
/// A key's head: its frame, the key gone back to after it, how the mix to the next
/// is shaped, and a word.
const KEY_HEAD: usize = 8;
/// The most keys a run is taken to have: more is a run that does not end.
const MOST_KEYS: usize = 4096;

/// The size of a timeline entry by the file's version (0x128218).
fn entry_len(version: u16) -> usize {
    if version == 100 {
        0xe
    } else {
        0x10
    }
}

/// A key of a run: from its frame on the run has its value, mixed towards the next
/// key's as that key's frame comes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key<T> {
    pub frame: u16,
    /// The key the run goes back to when this one's frame comes (0x11F454): a run
    /// that goes round, until the effect is told to end. The keys after this one
    /// are what it goes on to then.
    pub back_to: Option<u8>,
    pub value: T,
}

/// A run of keys in the order of their frames. Only a colour's is ever empty, in
/// one emitter of the game's, whose particles then show nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Run<T> {
    pub keys: Vec<Key<T>>,
}

/// What a value of a run is read from: the bytes of a key after its head.
trait Value: Sized {
    const LEN: usize;
    fn read(r: &Reader, at: usize) -> Result<Self>;
}

impl Value for f32 {
    const LEN: usize = 4;
    fn read(r: &Reader, at: usize) -> Result<f32> {
        r.f32(at)
    }
}

impl Value for [f32; 3] {
    const LEN: usize = 12;
    fn read(r: &Reader, at: usize) -> Result<[f32; 3]> {
        r.f32s(at)
    }
}

/// The shape particles come out of: its sizes, and how it is turned about x, y
/// and z in radians (0x11C028).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub size: [f32; 3],
    pub turn: [f32; 3],
}

impl Value for Opening {
    const LEN: usize = 24;
    fn read(r: &Reader, at: usize) -> Result<Opening> {
        Ok(Opening { size: r.f32s(at)?, turn: r.f32s(at + 12)? })
    }
}

/// The way particles are sent: a direction, and the angle each is tipped off it
/// to a random side, which is `tip` and a random part of `more_tip` more, in
/// degrees (0x11D76C).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Way {
    pub direction: [f32; 3],
    pub tip: f32,
    pub more_tip: f32,
}

impl Value for Way {
    const LEN: usize = 20;
    fn read(r: &Reader, at: usize) -> Result<Way> {
        Ok(Way { direction: r.f32s(at)?, tip: r.f32(at + 12)?, more_tip: r.f32(at + 16)? })
    }
}

/// How many particles are let go at a time, and the frames from one time to the
/// next (0x11D934).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lot {
    pub count: u16,
    pub every: f32,
}

impl Value for Lot {
    const LEN: usize = 8;
    fn read(r: &Reader, at: usize) -> Result<Lot> {
        Ok(Lot { count: r.u16(at)?, every: r.f32(at + 4)? })
    }
}

/// Which of the file's textures a flat particle is drawn from, or which of its
/// models a model particle is: by its place among them, none below nought. The
/// second would be another texture over the first; no script has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Of {
    pub first: i16,
    pub second: i16,
}

impl Value for Of {
    const LEN: usize = 4;
    fn read(r: &Reader, at: usize) -> Result<Of> {
        Ok(Of { first: r.i16(at)?, second: r.i16(at + 2)? })
    }
}

/// Red, green, blue and alpha, each 0 to 255.
impl Value for [u16; 4] {
    const LEN: usize = 8;
    fn read(r: &Reader, at: usize) -> Result<[u16; 4]> {
        Ok([r.u16(at)?, r.u16(at + 2)?, r.u16(at + 4)?, r.u16(at + 6)?])
    }
}

/// A move a particle makes besides its own way: so much a frame along a vector
/// (0x122944).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drift {
    pub along: [f32; 3],
    pub speed: f32,
}

impl Value for Drift {
    const LEN: usize = 16;
    fn read(r: &Reader, at: usize) -> Result<Drift> {
        Ok(Drift { along: r.f32s(at)?, speed: r.f32(at + 12)? })
    }
}

/// What a particle is (0x125960).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A flat picture: one quad of a texture.
    Flat,
    Model,
}

/// How a particle is mixed with what is behind it (0x11C680).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    /// Laid over by its alpha.
    Over,
    /// Added by its alpha.
    Added,
    /// Taken away by its alpha.
    TakenAway,
    /// All of it there, and written to the depth as well.
    Solid,
    /// A number the game sets nothing for, so that what the emitter drawn before
    /// this one had stays. It is so in nine emitters of models.
    AsBefore,
}

/// Which of the matrices of the effect a particle is put by (0x125A4C, 0x124548).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// The effect's as it is now: the particle goes where the effect goes.
    Now,
    /// The effect's as it was when the particle was let go.
    AtBirth,
    /// The effect's as it was when the emitter was made (0x11E564).
    AtStart,
    /// None: the world's.
    Never,
}

/// An emitter: what an entry of the timeline makes, and its particles.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    /// The frames a particle lives; nought, and it never ends of itself.
    pub life: u16,
    /// By how many hundredths a particle's own pace is spread: its time runs at
    /// one and a random part of this, either way (0x1257F8).
    pub pace_spread: u8,
    pub kind: Kind,

    /// Where the emitter is, and what that is fixed to (0x11CAD8). The game's
    /// data has 0 the effect, 6 a particle of the emitter it hangs from, 11 the
    /// screen (in its pixels from its middle), 16 the camera on the flat (turned
    /// with it and about it, the height as given) and 17 the camera whole.
    pub place: Run<[f32; 3]>,
    pub fixed_to: u16,
    /// The shape particles come out of about the place (0x11D254): 0 a ball of a
    /// random radius, 1 a ball's skin, 2 a box, 3 a ring, 4 a round with a height.
    pub opening: Run<Opening>,
    pub shape: u16,
    /// The way they are sent, and the space that is in (0 or 11).
    pub way: Run<Way>,
    pub way_space: u16,
    pub lots: Run<Lot>,

    pub of: Run<Of>,
    /// A particle's size; all of it is multiplied by `size_by`, and each
    /// particle's by one less a random part of the spreads, half of it either way
    /// (0x1200E8): one draw for all three when the second and third are below
    /// nought, else one each.
    pub size: Run<[f32; 3]>,
    pub size_by: f32,
    pub size_spread: [f32; 3],
    /// Its speed along its way, and the spread of that.
    pub speed: Run<f32>,
    pub speed_spread: f32,
    /// Its colour and alpha, with their spreads as the size's are (0x120824): one
    /// draw for red, green and blue when the second and third are below nought.
    pub colour: Run<[u16; 4]>,
    pub blend: Blend,
    pub colour_spread: [f32; 4],
    /// Its turning in radians, and how it faces (0x123FE0, 0x124D68): 0 and 1 the
    /// camera whole, 2 and 3 the camera's heading only, 4 nothing, 5 laid flat, 6
    /// side on, 7 the way it moves. Each particle is turned once by a random part
    /// of `turn_spread`, either way (0x1215C0); with `turn_random` its first turns
    /// are by a random part of themselves, either way.
    pub turn: Run<[f32; 3]>,
    pub faces: u16,
    pub turn_random: bool,
    pub turn_spread: [f32; 3],
    pub drift: Option<Run<Drift>>,
    /// How far its middle is from its place, turned as it is (0x124ED0); in its
    /// own sizes when `off_by_size` is set.
    pub off: Option<Run<[f32; 3]>>,
    pub off_by_size: bool,
}

/// An entry of the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The frames its emitter starts at and stops letting particles go at; with no
    /// end it goes on. For an emitter that hangs from another they are of the age
    /// of each particle of that one (0x11DD50).
    pub start: u16,
    pub end: Option<u16>,
    /// The entry its emitter hangs from: it lets its particles go from each
    /// particle of that one's (0x11DC34).
    pub parent: Option<usize>,
    /// How its particles keep to what the emitter is fixed to once they are let go
    /// (0x1245CC): 0 not at all, 1 to its place, 2 to its turning as well.
    pub keeps: u8,
    /// Its place in the order the emitters are drawn in: the lower first
    /// (0x128794).
    pub order: i8,
    pub held: Held,
    pub emitter: Option<Emitter>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub entries: Vec<Entry>,
}

impl Script {
    /// Reads a script of a file of the version given.
    pub fn parse(d: &[u8], version: u16) -> Result<Script> {
        let r = Reader(d);
        let root = r.u16(0)? as usize;
        ensure!(root != 0, "the script has no root");
        // The root (0x127F94): a count of frames before it starts, which no script
        // has, the timeline, and a flag.
        let timeline = r.u16(root + 2)? as usize;
        let mut entries = Vec::new();
        let mut at = timeline;
        while at != 0 && r.u16(at)? < END {
            entries.push(entry(&r, at, version).with_context(|| format!("the entry at {at:#x}"))?);
            at += entry_len(version);
            ensure!(entries.len() <= MOST_KEYS, "the timeline does not end");
        }
        // An emitter hangs from one made before it (0x12828C): the game looks for
        // it among those it has made so far, and three scripts name an entry that
        // comes later, is the entry itself or is not there, which finds nothing.
        for index in 0..entries.len() {
            let found = entries[index].parent.filter(|&parent| parent < index && entries[parent].emitter.is_some());
            entries[index].parent = found;
        }
        Ok(Script { entries })
    }
}

fn entry(r: &Reader, at: usize, version: u16) -> Result<Entry> {
    let (start, end, record) = (r.u16(at)?, r.u16(at + 2)?, r.u16(at + 4)? as usize);
    let parent = r.u8(at + 6)? as i8;
    let held = match if version == 100 { 0 } else { r.u8(at + 0xe)? } {
        0 => Held::Now,
        1 => Held::AtBirth,
        2 => Held::AtStart,
        3 => Held::Never,
        other => bail!("its particles are held {other}"),
    };
    Ok(Entry {
        start,
        end: (end != 0).then_some(end),
        parent: usize::try_from(parent).ok(),
        keeps: r.u8(at + 7)? & 3,
        order: r.u8(at + 8)? as i8,
        held,
        emitter: if record == 0 { None } else { Some(emitter(r, record).with_context(|| format!("the emitter at {record:#x}"))?) },
    })
}

fn emitter(r: &Reader, at: usize) -> Result<Emitter> {
    let kind = match r.u16(at + 8)? {
        0 => Kind::Flat,
        2 => Kind::Model,
        other => bail!("a particle of kind {other}"),
    };
    // Eleven offsets, the last two of which may be none.
    let offset = |index: usize| -> Result<usize> { Ok(r.u16(at + 0xa + 2 * index)? as usize) };
    let must = |index: usize| -> Result<usize> {
        let offset = offset(index)?;
        ensure!(offset != 0, "run {index} is not there");
        Ok(offset)
    };
    let (place, opening, way, lots) = (must(0)?, must(1)?, must(2)?, must(3)?);
    let (of, size, speed, colour, turn) = (must(4)?, must(5)?, must(6)?, must(7)?, must(8)?);
    let (drift, off) = (offset(9)?, offset(10)?);
    // The figures before a run's keys, by how far before they are.
    let before = |run: usize, back: usize| run.checked_sub(back).with_context(|| format!("the run at {run:#x} has nothing {back:#x} before it"));
    let short = |run: usize, back: usize| -> Result<u16> { r.u16(before(run, back)?) };
    let float = |run: usize, back: usize| -> Result<f32> { r.f32(before(run, back)?) };
    ensure!(short(of, 4)? == 0 && short(of, 2)? == 0, "the particle has runs for its picture's place, which are not read");
    let blend = match short(colour, 0x14)? {
        0 => Blend::Over,
        1 => Blend::Added,
        2 => Blend::TakenAway,
        4 => Blend::Solid,
        // 3 would take the picture's own colours away from what is behind it; no
        // script has it.
        3 => bail!("a blend no script of the game's has"),
        _ => Blend::AsBefore,
    };
    if drift != 0 {
        ensure!(short(drift, 2)? == 0 && short(drift, 4)? & 1 == 0, "a drift of a kind no script of the game's has");
    }
    let colour_keys = run(r, colour)?;
    let emitter = Emitter {
        life: r.u16(at)?,
        pace_spread: r.u8(at + 2)?,
        kind,
        place: run(r, place)?,
        fixed_to: short(place, 2)?,
        opening: run(r, opening)?,
        shape: short(opening, 4)?,
        way: run(r, way)?,
        way_space: short(way, 2)?,
        lots: run(r, lots)?,
        of: run(r, of)?,
        size: run(r, size)?,
        size_by: float(size, 0x10)?,
        size_spread: [float(size, 0xc)?, float(size, 8)?, float(size, 4)?],
        speed: run(r, speed)?,
        speed_spread: float(speed, 4)?,
        colour: colour_keys,
        blend,
        colour_spread: [float(colour, 0x10)?, float(colour, 0xc)?, float(colour, 8)?, float(colour, 4)?],
        turn: run(r, turn)?,
        faces: short(turn, 0x10)?,
        turn_random: short(turn, 0xe)? != 0,
        turn_spread: [float(turn, 0xc)?, float(turn, 8)?, float(turn, 4)?],
        drift: if drift == 0 { None } else { Some(run(r, drift)?) },
        off: if off == 0 { None } else { Some(run(r, off)?) },
        off_by_size: off != 0 && short(off, 2)? != 0,
    };
    // The game reads a run's first key whether it is there or not. One emitter
    // of its data has no key to its colour, and what is read instead is a colour
    // of nothing; an empty run of anything else would be read as what is not known.
    let filled = [
        !emitter.place.keys.is_empty(),
        !emitter.opening.keys.is_empty(),
        !emitter.way.keys.is_empty(),
        !emitter.lots.keys.is_empty(),
        !emitter.of.keys.is_empty(),
        !emitter.size.keys.is_empty(),
        !emitter.speed.keys.is_empty(),
        !emitter.turn.keys.is_empty(),
        emitter.drift.as_ref().is_none_or(|run| !run.keys.is_empty()),
        emitter.off.as_ref().is_none_or(|run| !run.keys.is_empty()),
    ];
    ensure!(filled.iter().all(|&filled| filled), "a run with no keys");
    Ok(emitter)
}

/// A run of keys from its offset to the key whose frame ends it.
fn run<T: Value>(r: &Reader, mut at: usize) -> Result<Run<T>> {
    let mut keys = Vec::new();
    loop {
        let frame = r.u16(at)?;
        if frame == END {
            break;
        }
        let (back_to, shape) = (r.u8(at + 2)? as i8, r.u8(at + 3)?);
        // Every key of the game's has a straight mix to the next; the shaped ones
        // (0x11F190) and the curves through four keys are not read.
        ensure!(shape == 0, "a key shaped {shape:#x} at {at:#x}");
        keys.push(Key { frame, back_to: u8::try_from(back_to).ok(), value: T::read(r, at + KEY_HEAD)? });
        ensure!(keys.len() <= MOST_KEYS, "a run of keys does not end");
        at += KEY_HEAD + T::LEN;
    }
    ensure!(keys.iter().all(|key| key.back_to.is_none_or(|back| usize::from(back) < keys.len())), "a key goes back to one the run does not have");
    Ok(Run { keys })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script with one entry and its emitter, built as the files have them: the
    /// runs first, each with its figures before it, then the emitter's record, the
    /// timeline and the root.
    fn script(version: u16) -> Vec<u8> {
        script_of(version, &[-1])
    }

    /// The same with an entry for each of the parents given, all of the one
    /// emitter.
    fn script_of(version: u16, parents: &[i8]) -> Vec<u8> {
        let mut d = vec![0u8; 8];
        let key = |d: &mut Vec<u8>, frame: u16, back: i8, values: &[u8]| {
            d.extend(frame.to_le_bytes());
            d.extend([back as u8, 0, 0, 0, 0, 0]);
            d.extend(values);
        };
        let floats = |values: &[f32]| values.iter().flat_map(|value| value.to_le_bytes()).collect::<Vec<u8>>();
        let shorts = |values: &[u16]| values.iter().flat_map(|value| value.to_le_bytes()).collect::<Vec<u8>>();
        let end = |d: &mut Vec<u8>| d.extend([0, 0x40, 0, 0]);
        let mut offsets = Vec::new();

        // The place: fixed to 6; two keys, the second going back to the first.
        d.extend(shorts(&[0, 6]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[100.0, 5.0, 500.0]));
        key(&mut d, 20, 0, &floats(&[100.0, 5.0, -30.0]));
        end(&mut d);
        // The opening: a box.
        d.extend(shorts(&[2, 0]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[30.0, 0.0, 30.0, 0.0, 0.0, 0.0]));
        end(&mut d);
        // The way, in space 11.
        d.extend(shorts(&[0, 11]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[0.0, 0.0, -1.0, 0.0, 50.0]));
        end(&mut d);
        // One every four frames.
        offsets.push(d.len());
        key(&mut d, 0, -1, &[1, 0, 0, 0, 0, 0, 0x80, 0x40]);
        end(&mut d);
        // Of the third texture.
        d.extend(shorts(&[0, 0]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &[2, 0, 0xff, 0xff]);
        end(&mut d);
        // The size, by 15, spread by 0.6 all together.
        d.extend(floats(&[15.0, 0.6, -1.0, -1.0]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[8.0, 8.0, 1.0]));
        key(&mut d, 28, -1, &floats(&[20.0, 20.0, 1.0]));
        end(&mut d);
        // The speed.
        d.extend(floats(&[0.5]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[20.0]));
        end(&mut d);
        // The colour, added.
        d.extend(shorts(&[1, 0]));
        d.extend(floats(&[0.7, -1.0, -1.0, 0.7]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &shorts(&[162, 175, 200, 80]));
        key(&mut d, 30, -1, &shorts(&[162, 175, 200, 0]));
        end(&mut d);
        // The turning: facing the way it moves, its first turns random.
        d.extend(shorts(&[7, 1]));
        d.extend(floats(&[0.0, 0.0, 6.25]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[0.0, 0.0, 1.5]));
        end(&mut d);
        // A drift, and its middle off its place by its own sizes.
        d.extend(shorts(&[0, 0]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[0.0, 1.0, 0.0, 2.0]));
        end(&mut d);
        d.extend(shorts(&[0, 1]));
        offsets.push(d.len());
        key(&mut d, 0, -1, &floats(&[0.0, 0.5, 0.0]));
        end(&mut d);

        // The emitter's record: life 30, pace spread 40, a flat picture.
        let record = d.len();
        d.extend(shorts(&[30, 40]));
        d.extend(floats(&[0.0]));
        d.extend(shorts(&[0]));
        d.extend(shorts(&offsets.iter().map(|&offset| offset as u16).collect::<Vec<u16>>()));
        // The timeline: frames 5 to 30, hanging from nothing, drawn third, its
        // particles keeping to it in the second way and held as at their birth.
        let timeline = d.len();
        for &parent in parents {
            d.extend(shorts(&[5, 30, record as u16]));
            d.extend([parent as u8, 2, 3, 0, 0, 0, 0xff, 0]);
            if version != 100 {
                d.extend([1, 0]);
            }
        }
        end(&mut d);
        // The root, with its two half-words before it.
        d.extend(shorts(&[0, 0]));
        let root = d.len();
        d.extend(shorts(&[0, timeline as u16, 0]));
        d[..2].copy_from_slice(&(root as u16).to_le_bytes());
        d
    }

    #[test]
    fn a_script_is_a_timeline_of_emitters_with_their_runs() {
        let read = Script::parse(&script(101), 101).unwrap();
        assert_eq!(read.entries.len(), 1);
        let entry = &read.entries[0];
        assert_eq!((entry.start, entry.end, entry.parent, entry.keeps, entry.order, entry.held), (5, Some(30), None, 2, 3, Held::AtBirth));
        let emitter = entry.emitter.as_ref().unwrap();
        assert_eq!((emitter.life, emitter.pace_spread, emitter.kind), (30, 40, Kind::Flat));
        // The place goes round: its second key goes back to its first.
        let place: Vec<_> = emitter.place.keys.iter().map(|key| (key.frame, key.back_to, key.value)).collect();
        assert_eq!(place, [(0, None, [100.0, 5.0, 500.0]), (20, Some(0), [100.0, 5.0, -30.0])]);
        assert_eq!((emitter.fixed_to, emitter.shape, emitter.way_space), (6, 2, 11));
        assert_eq!(emitter.opening.keys[0].value, Opening { size: [30.0, 0.0, 30.0], turn: [0.0, 0.0, 0.0] });
        assert_eq!(emitter.way.keys[0].value, Way { direction: [0.0, 0.0, -1.0], tip: 0.0, more_tip: 50.0 });
        assert_eq!(emitter.lots.keys[0].value, Lot { count: 1, every: 4.0 });
        assert_eq!(emitter.of.keys[0].value, Of { first: 2, second: -1 });
        assert_eq!((emitter.size.keys.len(), emitter.size_by, emitter.size_spread), (2, 15.0, [0.6, -1.0, -1.0]));
        assert_eq!((emitter.speed.keys[0].value, emitter.speed_spread), (20.0, 0.5));
        assert_eq!((emitter.colour.keys[1].value, emitter.blend, emitter.colour_spread), ([162, 175, 200, 0], Blend::Added, [0.7, -1.0, -1.0, 0.7]));
        assert_eq!((emitter.faces, emitter.turn_random, emitter.turn_spread), (7, true, [0.0, 0.0, 6.25]));
        assert_eq!(emitter.drift.as_ref().unwrap().keys[0].value, Drift { along: [0.0, 1.0, 0.0], speed: 2.0 });
        assert_eq!((emitter.off.as_ref().unwrap().keys[0].value, emitter.off_by_size), ([0.0, 0.5, 0.0], true));

        // A file of version 100 has shorter entries, whose particles are held by
        // the effect as it is.
        let old = Script::parse(&script(100), 100).unwrap();
        assert_eq!((old.entries.len(), old.entries[0].held), (1, Held::Now));
        assert_eq!(old.entries[0].emitter, read.entries[0].emitter);
    }

    #[test]
    fn an_emitter_hangs_only_from_one_made_before_it() {
        // The second hangs from the first; the third names itself, the fourth one
        // that comes after it and the fifth one that is not there, as three
        // scripts of the game's do, and the game finds none of them.
        let read = Script::parse(&script_of(101, &[-1, 0, 2, 4, 9]), 101).unwrap();
        let parents: Vec<Option<usize>> = read.entries.iter().map(|entry| entry.parent).collect();
        assert_eq!(parents, [None, Some(0), None, None, None]);
    }

    #[test]
    fn only_a_colour_may_have_no_keys() {
        let good = script(101);
        let root = u16::from_le_bytes([good[0], good[1]]) as usize;
        let timeline = u16::from_le_bytes([good[root + 2], good[root + 3]]) as usize;
        let record = u16::from_le_bytes([good[timeline + 4], good[timeline + 5]]) as usize;
        let run = |index: usize| u16::from_le_bytes([good[record + 0xa + 2 * index], good[record + 0xb + 2 * index]]) as usize;
        // The colour is the eighth run and the speed the seventh: the end put
        // where the first key is.
        let mut dark = good.clone();
        dark[run(7)..run(7) + 2].copy_from_slice(&END.to_le_bytes());
        let read = Script::parse(&dark, 101).unwrap();
        assert!(read.entries[0].emitter.as_ref().unwrap().colour.keys.is_empty());
        let mut still = good.clone();
        still[run(6)..run(6) + 2].copy_from_slice(&END.to_le_bytes());
        assert!(Script::parse(&still, 101).is_err());
    }

    #[test]
    fn what_is_not_read_is_refused() {
        let good = script(101);
        // A shaped key, a kind of particle the game's data has none of, and a
        // script cut short.
        let mut shaped = good.clone();
        shaped[8 + 4 + 3] = 2;
        assert!(Script::parse(&shaped, 101).is_err());
        let mut odd = good.clone();
        let root = u16::from_le_bytes([good[0], good[1]]) as usize;
        let timeline = u16::from_le_bytes([good[root + 2], good[root + 3]]) as usize;
        let record = u16::from_le_bytes([good[timeline + 4], good[timeline + 5]]) as usize;
        odd[record + 8] = 5;
        assert!(Script::parse(&odd, 101).is_err());
        assert!(Script::parse(&good[..root], 101).is_err());
        assert!(Script::parse(&[0, 0], 101).is_err());
    }
}
