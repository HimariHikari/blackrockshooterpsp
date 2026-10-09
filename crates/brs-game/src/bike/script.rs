//! A script of the ride on the bike as it runs (0x13F124 to 0x13F490;
//! `docs/BIKE_NOTES.md`, "Scripts"), and what a character shows, which its
//! scripts set and the drawing reads.

use std::sync::Arc;

use bevy::math::Vec3;
use brs_formats::script::{Machine, Script};

/// Where the course's flags and numbers and its two tables lie in every
/// script's memory: the scripts and the game all work on the one copy.
pub const SHARED_BASE: u32 = 0x2000_0000;
/// Where a script's own 0x40 bytes are in its memory (`n23(5)`).
pub const OWN_BASE: u32 = 0x2100_0000;
pub const OWN_LEN: usize = 0x40;

/// Whose a script is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    System,
    Course,
    Camera,
    /// One of a character's three, by the character's number.
    Of(i32, usize),
}

/// What a script reads of itself (`n23`, from its + 0x140).
#[derive(Clone, Copy, Debug, Default)]
pub struct Work {
    /// An entry begun by name is not to break in on it (`n49`).
    pub no_breaking_in: bool,
    /// Its character's number, and the number it was made with.
    pub number: i32,
    pub with: i32,
    /// The event it was last told, and what came with it.
    pub event: i32,
    pub came: i32,
    /// A count for its waits.
    pub count: i32,
}

/// A script that runs.
pub struct Instance {
    pub machine: Machine,
    pub script: Arc<Script>,
    /// A character's goes round its states: the function it is in is begun
    /// again whenever it returns, and what it returns is the next one.
    pub rounds: bool,
    /// The function it is in when nothing else is asked of it.
    pub within: u32,
    pub work: Work,
}

impl Instance {
    pub fn new(script: &Arc<Script>, rounds: bool, number: i32, with: i32) -> Instance {
        let mut machine = Machine::new(script);
        machine.map(OWN_BASE, vec![0; OWN_LEN]);
        Instance { machine, script: script.clone(), rounds, within: 0, work: Work { number, with, ..Work::default() } }
    }
}

/// One of a character's two models as its scripts have it.
#[derive(Clone, Debug)]
pub struct ModelLook {
    /// Which of the system's models, if any.
    pub resource: Option<usize>,
    pub animation: i32,
    pub looped: bool,
    /// How far into its animation it is, in seconds, and how long that is.
    pub at: f32,
    pub length: f32,
    pub speed: f32,
    pub scale: f32,
    /// What `n1b(.., 0, ..)` set of how it is drawn.
    pub drawn: u32,
    pub flag: bool,
    /// How often its animation has been begun: the drawing begins it anew when
    /// this changes.
    pub begun: u32,
    /// Bones turned (`n24`: the bone, which way, degrees) and parts shown or
    /// hidden (`n21`).
    pub turned: Vec<(i32, i32, f32)>,
    pub parts: Vec<(i32, i32)>,
}

impl Default for ModelLook {
    fn default() -> Self {
        ModelLook { resource: None, animation: 0, looped: false, at: 0.0, length: 0.0, speed: 1.0, scale: 1.0, drawn: 0, flag: false, begun: 0, turned: Vec::new(), parts: Vec::new() }
    }
}

impl ModelLook {
    /// Whether its animation still plays (0x13E9E4): one that goes round
    /// always does.
    pub fn plays(&self) -> bool {
        self.resource.is_some() && (self.looped || self.at < self.length)
    }
}

/// One of the five pieces of road a thing of scenery is made of.
#[derive(Clone, Copy, Debug, Default)]
pub struct PieceLook {
    pub resource: Option<usize>,
    pub drawn: u32,
    pub place: Vec3,
    pub heading: f32,
}

/// One of a character's twenty effects.
#[derive(Clone, Copy, Debug)]
pub struct EffectLook {
    pub resource: usize,
    /// It goes with what it is on every frame; else it is put there once.
    pub follows: bool,
    /// It does not slow when the game's time does.
    pub own_time: bool,
    pub held: bool,
    pub flag: bool,
    /// The model whose bone it is on, and the bone, or less than 0 for the
    /// character itself.
    pub model: i32,
    pub bone: i32,
    pub scale: f32,
    pub place: Vec3,
    pub turns: Vec3,
    pub word: i32,
    /// How often an effect has been put in this slot: the drawing makes it
    /// anew when this changes.
    pub made: u32,
}

/// What a character shows.
#[derive(Clone, Debug)]
pub struct Looks {
    pub models: [ModelLook; 2],
    pub pieces: [PieceLook; 5],
    pub effects: [Option<EffectLook>; 20],
    /// How long a new animation takes to come in.
    pub blend: f32,
    /// The three words of its shadow (0x146F4C): how wide and how long it is,
    /// and whether it is left out.
    pub shadow: [i32; 3],
    made: u32,
}

impl Default for Looks {
    /// As it is made (0x146A8C has the shadow sixty each way, and drawn).
    fn default() -> Self {
        Looks { models: Default::default(), pieces: Default::default(), effects: [None; 20], blend: 0.0, shadow: [60, 60, 0], made: 0 }
    }
}

impl Looks {
    /// Begins both models' animations anew (0x13C61C).
    pub fn begin(&mut self, lengths: impl Fn(usize, i32) -> f32) {
        for model in &mut self.models {
            if let Some(resource) = model.resource {
                (model.at, model.length, model.begun) = (0.0, lengths(resource, model.animation), model.begun + 1);
            }
        }
    }

    /// A frame of the models' animations (0x13C9C0, 0).
    pub fn step(&mut self, time: f32) {
        for model in &mut self.models {
            if model.resource.is_some() {
                model.at += time * model.speed;
            }
        }
    }

    /// Puts an effect in a slot, as it is before anything is set of it
    /// (0x13C12C).
    pub fn put_effect(&mut self, slot: usize, resource: usize) {
        self.made += 1;
        self.effects[slot] = Some(EffectLook {
            resource,
            follows: false,
            own_time: false,
            held: false,
            flag: false,
            model: -1,
            bone: -1,
            scale: 1.0,
            place: Vec3::ZERO,
            turns: Vec3::ZERO,
            word: 0,
            made: self.made,
        });
    }
}
