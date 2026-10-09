//! Story scenes: what an event brings to stage one, and the state of one as it plays.
//!
//! A map's script loads an event's bundle for a scene's sake (`s59`): the motions of
//! those who take part, kept by the character each slot of the bundle's cast is for,
//! and the camera's cuts. It then starts cuts (`s101`) and motions (`s63`), and times
//! what else happens by a count of frames that it sets itself (`s96(8, ..)`) and
//! waits on (`s103`). Captions say what is spoken (`s242`), and the recordings of it
//! are the event's too (`s29`). See `docs/FIELD_NOTES.md`.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use brs_engine::animation::Clip;
use brs_engine::camera::{CameraClip, CameraPose};
use brs_formats::cam;

use super::tables::FieldTables;
use crate::data::GameData;
use crate::text::Span;

// The slots of an event's bundle that stage a scene.
const CAST: &str = "00.sscr";
const MOTIONS: &str = "01.bin";
/// The things its people hold: a bundle each, of which the model is this file.
const PROPS: &str = "02.bin";
const PROP_MODEL: &str = "01.bin/00.insm";
/// How many of them are looked for: no event has more than four.
const PROPS_AT_MOST: i32 = 0x20;
const CUTS: &str = "03.bin";
const EFFECTS: &str = "04.bin";
const RECORDINGS: &str = "05.bin";
const BANK: &str = "06.bin/00.pphd";
/// What a recording's file ends in; the sound made of it lies beside it.
const RECORDING: &str = ".riff";
/// What an effect's file ends in, and how many effects an event can bring
/// (0xE97F4).
const EFFECT: &str = ".efc";
const EFFECTS_AT_MOST: i32 = 0x20;
/// The table of the bundle's cast: a byte for each slot, the character it is for.
const CAST_TABLE: &str = "lst_fchr";
const NOBODY: u8 = 0xff;

/// Frames of a cut's animation that pass in a frame of the game: its time goes on by
/// 0x800 units, which is two of its frames (0xDCA24).
const CUT_FRAMES_A_STEP: f32 = 2.0;

/// How many captions can be up at once (0xD162C).
const CAPTIONS: usize = 8;

/// What an event brings to stage a scene.
pub struct Staging {
    /// The event's number in the game's list.
    pub event: i32,
    /// The character resource each slot is for.
    cast: Vec<u8>,
    motions: BTreeMap<(usize, i32), Arc<Clip>>,
    /// The model of each thing it has for somebody to hold (`s71`; the
    /// resources of kind 2, 0x116DD4).
    props: BTreeMap<i32, String>,
    cuts: BTreeMap<i32, Arc<CameraClip>>,
    /// The file of each recording of what is spoken and heard.
    recordings: BTreeMap<i32, String>,
    /// The file of each effect it has, which a script makes a character of
    /// (`s53` with kind 2; 0xE9418).
    effects: BTreeMap<i32, String>,
    /// The header of its bank of sounds, if it has one.
    bank: Option<String>,
}

impl std::fmt::Debug for Staging {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (motions, cuts, recordings) = (self.motions.len(), self.cuts.len(), self.recordings.len());
        write!(f, "the staging of event {}: {motions} motions, {cuts} cuts, {recordings} recordings", self.event)
    }
}

impl Staging {
    /// Loads what event `number` of the game's list has for a scene. An event without
    /// a bundle stages nothing.
    pub fn load(data: &GameData, tables: &FieldTables, number: i32) -> Result<Staging> {
        let mut staging = Staging {
            event: number,
            cast: Vec::new(),
            motions: BTreeMap::new(),
            props: BTreeMap::new(),
            cuts: BTreeMap::new(),
            recordings: BTreeMap::new(),
            effects: BTreeMap::new(),
            bank: None,
        };
        let Some(bundle) = tables.event_bundle(number).filter(|bundle| data.has_folder(bundle)) else {
            return Ok(staging);
        };
        let path = format!("{bundle}/{CAST}");
        if data.exists(&path) {
            staging.cast = data.script(&path)?.extent(CAST_TABLE).map(<[u8]>::to_vec).unwrap_or_default();
        }

        // A motion is a file of its slot's folder, named by its number.
        let numbered = |name: &str| name.split('.').next().and_then(|stem| stem.parse::<i32>().ok());
        for slot in 0..staging.cast.len() {
            let folder = format!("{bundle}/{MOTIONS}/{slot:02}.bin");
            if !data.has_folder(&folder) {
                continue;
            }
            for file in data.list(&folder)? {
                if let Some(motion) = numbered(&file) {
                    staging.motions.insert((slot, motion), data.clip(format!("{folder}/{file}"))?);
                }
            }
        }
        // Each thing is a bundle of its own, a folder here, named by its number.
        let folder = format!("{bundle}/{PROPS}");
        if data.has_folder(&folder) {
            for prop in 0..PROPS_AT_MOST {
                let model = format!("{folder}/{prop:02}.bin/{PROP_MODEL}");
                if data.exists(&model) {
                    staging.props.insert(prop, model);
                }
            }
        }
        let folder = format!("{bundle}/{CUTS}");
        if data.has_folder(&folder) {
            for file in data.list(&folder)? {
                let Some(cut) = numbered(&file) else { continue };
                let path = format!("{folder}/{file}");
                let camera = cam::Camera::parse_cut(&data.read(&path)?).with_context(|| format!("parsing {path}"))?;
                staging.cuts.insert(cut, CameraClip::new(camera));
            }
        }
        let folder = format!("{bundle}/{RECORDINGS}");
        if data.has_folder(&folder) {
            for file in data.list(&folder)?.into_iter().filter(|file| file.ends_with(RECORDING)) {
                if let Some(recording) = numbered(&file) {
                    staging.recordings.insert(recording, format!("{folder}/{file}"));
                }
            }
        }
        let folder = format!("{bundle}/{EFFECTS}");
        if data.has_folder(&folder) {
            for file in data.list(&folder)?.into_iter().filter(|file| file.ends_with(EFFECT)) {
                if let Some(effect) = numbered(&file).filter(|effect| (0..EFFECTS_AT_MOST).contains(effect)) {
                    staging.effects.insert(effect, format!("{folder}/{file}"));
                }
            }
        }
        staging.bank = Some(format!("{bundle}/{BANK}")).filter(|bank| data.exists(bank));
        Ok(staging)
    }

    /// The slot of the cast that is for character resource `chr` (0xE9398).
    pub fn slot_of(&self, chr: i32) -> Option<usize> {
        let chr = u8::try_from(chr).ok().filter(|&chr| chr != NOBODY)?;
        self.cast.iter().position(|&cast| cast == chr)
    }

    /// Motion `number` of slot `slot` (0xE9024).
    pub fn motion(&self, slot: usize, number: i32) -> Option<&Arc<Clip>> {
        self.motions.get(&(slot, number))
    }

    pub fn cut(&self, number: i32) -> Option<&Arc<CameraClip>> {
        self.cuts.get(&number)
    }

    /// The header of the event's bank of sounds.
    pub fn bank(&self) -> Option<&str> {
        self.bank.as_deref()
    }

    /// The model of thing `number` that it has for somebody to hold, if it has it.
    pub fn prop(&self, number: i32) -> Option<&str> {
        self.props.get(&number).map(String::as_str)
    }

    /// The file of effect `number` (0xE9418), if the event has it.
    pub fn effect(&self, number: i32) -> Option<&str> {
        self.effects.get(&number).map(String::as_str)
    }

    /// The file of recording `number` (0xE91CC), if the event has it.
    pub fn recording(&self, number: i32) -> Option<&str> {
        self.recordings.get(&number).map(String::as_str)
    }
}

/// A cut the camera is playing, or has played and stays at the end of.
#[derive(Clone)]
pub struct Cut {
    clip: Arc<CameraClip>,
    /// How far into it the camera is, in frames of its animation.
    frame: f32,
    /// False once it has reached its end.
    pub playing: bool,
}

impl std::fmt::Debug for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "a cut at frame {} of {}{}", self.frame, self.clip.frames(), if self.playing { "" } else { ", over" })
    }
}

impl Cut {
    /// A cut at its start (0xDA884).
    pub fn new(clip: &Arc<CameraClip>) -> Cut {
        Cut { clip: clip.clone(), frame: 0.0, playing: true }
    }

    /// A frame of the game (0xDCA10): the cut goes on, and is over when it has
    /// reached its end.
    pub fn step(&mut self) {
        if !self.playing {
            return;
        }
        self.frame += CUT_FRAMES_A_STEP;
        if self.frame >= self.clip.frames() {
            self.frame = self.clip.frames();
            self.playing = false;
        }
    }

    /// Where the camera is, `ahead` of a frame of the game past the last one.
    pub fn pose(&self, ahead: f32) -> CameraPose {
        let ahead = if self.playing { ahead * CUT_FRAMES_A_STEP } else { 0.0 };
        self.clip.pose(self.frame + ahead)
    }
}

/// What a caption is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    In,
    Held,
    Out,
}

/// A line of what is spoken in a scene, on the screen for a while.
#[derive(Clone, Debug)]
pub struct Caption {
    /// The line, in the pieces of one colour it is made of.
    pub text: Vec<Span>,
    /// How much of it shows, from none to all.
    pub alpha: f32,
    phase: Phase,
    /// Frames spent in the phase it is in.
    count: i32,
    fade_in: i32,
    hold: i32,
    fade_out: i32,
}

/// The sign at the foot of the screen that a caption waits for a button (the bits
/// 0x40 and 0x80 of the captions' flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sign {
    #[default]
    None,
    /// A caption waits to be passed: the sign comes up and blinks.
    Waiting,
    /// It has been passed: the sign shrinks away.
    Leaving,
}

/// The captions of a scene (the original's "telop_auto_task", 0xD13A8).
#[derive(Clone, Debug, Default)]
pub struct Captions {
    /// Those that are up, each in the place it was given.
    slots: [Option<Caption>; CAPTIONS],
    /// A caption stays until a button passes it, rather than for its time (`s243`).
    pub by_hand: bool,
    /// They are drawn higher up the screen (0xD1B54).
    pub raised: bool,
    /// A button has passed the caption that waits.
    passed: bool,
    /// What the sign is doing, and how many times it has been set to do something:
    /// each time starts its motion again.
    pub sign: Sign,
    pub sign_set: u32,
}

impl Captions {
    /// The captions as their task is made (0xD1588): unless the player has them
    /// go on of themselves, a caption stays until a button passes it.
    pub fn made(auto_skip: bool) -> Captions {
        Captions { by_hand: !auto_skip, ..Captions::default() }
    }

    /// Puts a caption up in the first free place, to be there for `frames` in all, of
    /// which it takes `fade_in` to come and `fade_out` to go (0xD1A88, 0xD1C18). With
    /// no place free it is dropped.
    pub fn say(&mut self, text: Vec<Span>, frames: i32, fade_in: i32, fade_out: i32) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(Caption {
                text,
                alpha: 0.0,
                phase: Phase::In,
                count: 0,
                fade_in,
                hold: frames - (fade_in + fade_out),
                fade_out,
            });
        }
    }

    /// Those that are up.
    pub fn shown(&self) -> impl Iterator<Item = &Caption> {
        self.slots.iter().flatten()
    }

    pub fn any(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }

    /// Takes them all down at once.
    pub fn clear(&mut self) {
        self.slots = Default::default();
        self.passed = false;
        self.sign = Sign::None;
    }

    /// A button passes the caption that waits to be read, if one does (0xD1B0C).
    pub fn pass(&mut self) {
        let waits = self.shown().any(|caption| caption.phase == Phase::Held);
        self.passed = self.by_hand && waits;
    }

    /// A frame (0xD1D58): each caption comes, stays its time or until it is passed,
    /// and goes.
    pub fn step(&mut self) {
        let passed = std::mem::take(&mut self.passed);
        let part = |count: i32, of: i32| if of > 0 { count as f32 / of as f32 } else { 1.0 };
        let mut sign = None;
        for slot in &mut self.slots {
            let Some(caption) = slot else { continue };
            if caption.phase == Phase::In {
                if caption.count >= caption.fade_in {
                    (caption.phase, caption.count) = (Phase::Held, 0);
                    // One that waits for a button says so (0xD1E9C).
                    if self.by_hand {
                        sign = Some(Sign::Waiting);
                    }
                } else {
                    caption.alpha = part(caption.count, caption.fade_in);
                }
            }
            if caption.phase == Phase::Held {
                caption.alpha = 1.0;
                let over = if self.by_hand { passed } else { caption.count >= caption.hold };
                if over {
                    (caption.phase, caption.count) = (Phase::Out, 0);
                    if self.by_hand {
                        sign = Some(Sign::Leaving);
                    }
                }
            }
            if caption.phase == Phase::Out {
                if caption.count >= caption.fade_out {
                    *slot = None;
                    continue;
                }
                caption.alpha = 1.0 - part(caption.count, caption.fade_out);
            }
            caption.count += 1;
        }
        if let Some(sign) = sign {
            self.sign = sign;
            self.sign_set += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    /// The event that stages the game's opening scene.
    const OPENING: i32 = 169;

    fn words(text: &str) -> Vec<Span> {
        vec![Span { text: text.to_owned(), colour: 0 }]
    }

    #[test]
    fn the_openings_bundle_has_its_cast_its_motions_and_its_cuts() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        let staging = Staging::load(&data, &tables, OPENING).unwrap();
        // The capsule she sleeps in, the officer, the soldiers.
        assert_eq!((staging.slot_of(46), staging.slot_of(16), staging.slot_of(17)), (Some(1), Some(2), Some(3)));
        assert_eq!(staging.slot_of(0), None);
        assert_eq!(staging.slot_of(255), None);
        // A soldier's motion 21, which he runs in for almost seven seconds.
        assert_eq!(staging.motion(3, 21).unwrap().frames(), 418.0);
        assert!(staging.motion(3, 8).is_none());
        // Nine cuts; the first runs from frame 2 of its animation to frame 660.
        assert_eq!(staging.cuts.len(), 9);
        assert_eq!(staging.cut(1).unwrap().frames(), 658.0);
        assert!(staging.cut(0).is_none());
        // Twenty-five recordings, numbered with gaps: there is no second.
        assert_eq!(staging.recordings.len(), 25);
        assert_eq!(staging.recording(0), Some("FLD/FEVT/FEVT_1_1_0200B_1.BIN/05.bin/00.riff"));
        assert_eq!(staging.recording(1), None);
        assert_eq!(staging.bank(), Some("FLD/FEVT/FEVT_1_1_0200B_1.BIN/06.bin/00.pphd"));
        // An event with no bundle stages nothing.
        assert!(Staging::load(&data, &tables, 9999).unwrap().cut(1).is_none());
        // The opening gives nobody anything to hold; the scene in which a man
        // she saved gives her something has the thing, a model, as its first.
        assert_eq!(staging.prop(0), None);
        let thanks = Staging::load(&data, &tables, 705).unwrap();
        let thing = thanks.prop(0).expect("the event has a thing to be held");
        assert_eq!(thing, "FLD/FEVT/FEVT02_02_0217E.BIN/02.bin/00.bin/01.bin/00.insm");
        assert!(data.model(thing).is_ok() && thanks.prop(1).is_none());
    }

    #[test]
    fn a_cut_runs_two_frames_of_its_animation_a_frame_and_then_stays() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        let staging = Staging::load(&data, &tables, OPENING).unwrap();
        let mut cut = Cut::new(staging.cut(1).unwrap());
        let start = cut.pose(0.0);
        for _ in 0..328 {
            cut.step();
        }
        assert!(cut.playing);
        // Half a frame on, between two steps, the camera is on its way.
        assert_ne!(cut.pose(0.5), cut.pose(0.0));
        cut.step();
        assert!(!cut.playing);
        let end = cut.pose(0.0);
        assert_ne!(start.transform.translation, end.transform.translation);
        cut.step();
        assert_eq!(cut.pose(0.5), end);
    }

    #[test]
    fn a_caption_comes_stays_and_goes_in_its_time() {
        let mut captions = Captions::default();
        captions.say(words("Come on, wake up!"), 48, 4, 4);
        let alpha = |captions: &Captions| captions.shown().next().map(|caption| caption.alpha);
        let mut seen = Vec::new();
        for _ in 0..50 {
            captions.step();
            seen.push(alpha(&captions));
        }
        // Four frames to come, forty there, four to go: forty-eight in all.
        assert_eq!(seen[..5], [Some(0.0), Some(0.25), Some(0.5), Some(0.75), Some(1.0)]);
        assert!(seen[4..45].iter().all(|&alpha| alpha == Some(1.0)));
        assert_eq!(seen[45..], [Some(0.75), Some(0.5), Some(0.25), None, None]);
    }

    #[test]
    fn a_caption_that_waits_to_be_read_stays_until_a_button_passes_it() {
        // As they are made, captions wait unless the player's options have them
        // go on of themselves.
        assert!(Captions::made(false).by_hand && !Captions::made(true).by_hand);
        let mut captions = Captions::made(false);
        captions.say(words("one"), 10, 1, 1);
        // A button before it is all there does nothing.
        captions.pass();
        for _ in 0..200 {
            captions.step();
        }
        assert_eq!(captions.shown().count(), 1);
        captions.pass();
        captions.step();
        captions.step();
        assert!(!captions.any());
        // Nine at once is one too many.
        for _ in 0..9 {
            captions.say(words("x"), 10, 1, 1);
        }
        assert_eq!(captions.shown().count(), 8);
        captions.clear();
        assert!(!captions.any());
    }
}
