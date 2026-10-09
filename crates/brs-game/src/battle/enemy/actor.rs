//! One enemy: a port of the original's `BtEnActor` (built at 0x91BBC) and of the
//! engine functions its script calls (table 2 of the script machine).
//!
//! An enemy is its script. The engine gives it slots to put models and collision
//! spheres in, a movement block, HP and a few flags; the script loads the model, says
//! where the spheres are, and from then on decides everything the enemy does. Each
//! function below names the address of the code it follows; `docs/BATTLE_NOTES.md`
//! has the longer account.

use std::collections::HashSet;

use anyhow::{Context as _, Result};
use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_formats::bscr::{FunctionKind, Host, Machine, Reply, Script, Table};
use brs_formats::egtb::Member;

use super::mover::{self, flag, Mover};
use super::reserves::{Plan, Reserves, PLACE_CLEARANCE};
use crate::battle::actor::Action;
use crate::battle::damage::{damage, status_factors, Assailant, Random};
use crate::battle::log;
use crate::battle::status::{self, kind, Effect, Effects, Giver, Marks, Status, Taker};
use crate::battle::tables::{Archive, Attack, Shot, Shots, Variant};
use crate::battle::Stage;
use crate::sound::{FULL_VOLUME, NOTE};

/// Slots an actor has for models and collision spheres, and lock-point gauges.
pub const SLOTS: usize = 16;
pub const GAUGES: usize = 3;

/// The signals the engine sends an enemy's script.
pub mod signal {
    /// Build yourself, then enter as a wave.
    pub const ENTER: i32 = 0;
    /// Build yourself, standing where you are.
    pub const SET_UP: i32 = 1;
    /// You have been called into the battle (0xA0E70).
    pub const CALLED: i32 = 2;
    /// The battle is over.
    pub const OVER: i32 = 3;
    pub const HIT: i32 = 100;
    /// The player was hit.
    pub const PLAYER_HIT: i32 = 101;
    /// Think: sent every frame once the battle is under way.
    pub const THINK: i32 = 2000;
}

/// Height above the player's standing place that enemies aim their shots at (0x91308).
const SHOT_AIM_HEIGHT: f32 = 75.0;
/// Logic frames in which an enemy turns to what it will shoot at (0xBF658).
const SHOT_AIM_TURN: f32 = 12.0;
/// How long the flash that warns of an attack lasts, in sixtieths (0x91644).
const WARNING_TIME: f32 = 30.0;
/// The most bullets one shot can chain together. The table's longest chain is 18;
/// this only stops a chain that leads back to itself.
const SHOT_CHAIN: usize = 64;

/// A collision sphere: a slot of its own, fixed to a node of a model slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    /// The model slot and the node of its model (by id; -1 for the model itself)
    /// the sphere is fixed to, and where it is from there.
    pub parent: i32,
    pub node: i32,
    pub offset: Vec3,
    pub radius: f32,
    /// Hurts the player on touching her; the others are where the enemy can be hit.
    pub attack: bool,
    pub enabled: bool,
    /// Which attack a touch is.
    pub attack_id: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Slot {
    /// A model, by its file name in the enemy's archive.
    Model { name: String },
    Sphere(Sphere),
}

/// The gauge of a lock point: a part of an enemy that enough force breaks, stunning
/// it for a while (the original's class at 0x85D38).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gauge {
    /// The strength a fresh gauge has, give or take.
    base: f32,
    /// A worn gauge is whole again in this many sixtieths.
    recovery: f32,
    /// How long it stays broken.
    broken_time: f32,
    max: f32,
    value: f32,
    broken_left: f32,
    pub broken: bool,
    worn: bool,
    /// The lock point is shown and can be aimed at.
    pub active: bool,
    /// The slot and node the lock point is on, and where from there.
    pub slot: i32,
    pub node: i32,
    pub offset: Vec3,
}

impl Gauge {
    /// A fresh gauge with the three figures of the enemy's parameters (0x85DD4). Its
    /// strength starts between seven eighths of the base and the base (0x85B20).
    fn new(figures: [i32; 3], random: &mut Random) -> Gauge {
        let base = figures[0] as f32;
        let low = base * 7.0 * 0.125;
        let strength = low + (base - low) * random.fraction();
        Gauge {
            base,
            recovery: figures[1] as f32,
            broken_time: figures[2] as f32,
            max: strength,
            value: strength,
            broken_left: 0.0,
            broken: false,
            worn: false,
            active: false,
            slot: -1,
            node: -1,
            offset: Vec3::ZERO,
        }
    }

    /// Wears the gauge down by `power`; true if that broke it (0x860E8).
    fn wear(&mut self, power: i32) -> bool {
        if self.broken || power <= 0 {
            return false;
        }
        self.worn = true;
        self.value -= power as f32;
        if self.value > 0.0 {
            return false;
        }
        self.value = 0.0;
        self.worn = false;
        self.broken = true;
        self.broken_left = self.broken_time;
        true
    }

    /// One frame: a worn gauge recovers, a broken one counts down to being whole
    /// again, and stronger than before (0x85E24, 0x86194). True on the frame it is
    /// whole again.
    fn update(&mut self, dt: f32) -> bool {
        if self.worn {
            if self.recovery > 0.0 {
                self.value = (self.value + self.max / self.recovery * dt).min(self.max);
            }
            if self.value >= self.max {
                self.worn = false;
            }
        } else if self.broken {
            self.broken_left = (self.broken_left - dt).max(0.0);
            if self.broken_left <= 0.0 {
                self.mend();
                return true;
            }
        }
        false
    }

    fn mend(&mut self) {
        if !self.broken {
            return;
        }
        self.broken = false;
        self.max = (self.max * 120.0 / 100.0).max(self.base * 200.0 / 100.0);
        self.value = self.max;
    }
}

/// What the player looks like to an enemy's script.
#[derive(Clone, Copy, Debug)]
pub struct PlayerView {
    /// Where she is as the enemies have it: the middle of her body, node 0 of her
    /// model, and not the spot on the ground she stands on (0x91108).
    pub position: Vec3,
    /// Where her right ankle is, node 0x17.
    pub ankle: Vec3,
    pub hp: f32,
    pub max_hp: f32,
    pub action: Action,
    /// The heat gauge, from 0 to 1.
    pub heat: f32,
    /// Which of the stage's two standing places she is at or heading for.
    pub place: u32,
    /// The status effects on her.
    pub status: Marks,
}

/// What every enemy of the battle can see of the others.
#[derive(Clone, Copy, Debug, Default)]
pub struct Peer {
    /// A number the scripts hand round so that the enemies take turns (0xA20).
    pub token: i32,
    /// In the battle: neither dead nor on its way out.
    pub alive: bool,
    /// HP as a percentage, cut to a whole number.
    pub hp_percent: i32,
    /// Where it belongs, and whether it is still there with HP left: nobody else
    /// comes into the battle on that spot then.
    pub home: Vec3,
    pub in_the_way: bool,
}

/// What the enemies of a battle have between them.
#[derive(Clone, Debug, Default)]
pub struct Shared {
    /// Every enemy that has been in the battle, by its number.
    pub peers: Vec<Peer>,
    /// The enemies the group keeps back.
    pub reserves: Reserves,
    /// What beating them has been worth so far.
    pub spoils: Spoils,
    /// How many sounds they have asked for: the next is known by this number.
    pub sounds: u32,
    /// What a boss's script is told of the battle about it.
    pub told: Told,
}

/// What a boss's script asks about the battle as a whole, as the battle last had
/// it: the things its waits go by, and who has the stage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Told {
    /// The screen has come to the battle (0x907F4 answers no).
    pub encounter_over: bool,
    /// Each of the two streams is silent (0xA3DC).
    pub silent: [bool; 2],
    /// A set of the cut-in's layout is still playing (0x3328C).
    pub cut_in_plays: bool,
    /// The finish is on (0x9469C), and how many have been done with (0x946A8).
    pub finishing: bool,
    pub finishes: i32,
    /// The enemy that has the stage (the root's flag 10), by its number.
    pub on_stage: Option<usize>,
}

/// The numbers the enemies' sounds are known to the mixer by start here.
const SOUND_HANDLES: u32 = 0x2000_0000;
/// What a script is answered when the sound it asks for is not played.
const NO_SOUND: i32 = -1;
/// The banks a sound can be of (0xBC3CC): the battle's, the enemy's own, and that
/// of its finishing move.
pub const BANK_OF_BATTLE: i32 = 0;
pub const BANK_OF_ENEMY: i32 = 1;
pub const BANK_OF_FINISH: i32 = 2;
// Sounds of the battle's bank: a guarded hit, a warning, an enemy called in.
pub const SOUND_GUARDED: i32 = 0x6e;
const SOUND_WARNING: i32 = 0x8e;
const SOUND_CALLED_IN: i32 = 0x6b;

/// How many kinds of item a battle can leave her with (0x94C54).
pub const SPOIL_KINDS: usize = 4;

/// What a battle has been worth so far: the stage's figures at +0x94 to +0xC4.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spoils {
    /// Experience counts for half in this battle (0x973B4).
    pub halved: bool,
    /// Enemies beaten.
    pub beaten: i32,
    /// The damage her hits have done them (the root's + 0xA0), all told.
    pub damage_dealt: i32,
    pub experience: i32,
    /// The items left behind, each with how many of it.
    pub items: [(i32, i32); SPOIL_KINDS],
}

impl Spoils {
    /// An enemy is beaten (0x90B20). It is counted; and unless it is one that was
    /// called into the battle, which is worth nothing, its experience is added and
    /// it may leave something behind.
    fn take(&mut self, variant: &Variant, rewards: bool, random: &mut Random) {
        self.beaten += 1;
        if !rewards {
            return;
        }
        self.experience += if self.halved { variant.experience * 50 / 100 } else { variant.experience };

        // What it leaves (0x94C54): its usual item or, on a second roll, its rare
        // one. The chances are out of 255.
        let mut rolls = |chance: i32| (random.below(0x7fff) as f32 / 32767.0) < chance as f32 / 255.0;
        if !rolls(variant.drop.chance) {
            return;
        }
        let item = if rolls(variant.rare.chance) { variant.rare.item } else { variant.drop.item };
        if item == 0 {
            return;
        }
        // It goes with the others of its kind, or else in the first empty place; a
        // fifth kind of item is lost.
        let place = self.items.iter().position(|kind| kind.0 == item).or(self.items.iter().position(|kind| kind.1 <= 0));
        if let Some(place) = place {
            self.items[place] = (item, self.items[place].1.max(0) + 1);
        }
    }

    /// The items left behind, with how many of each.
    pub fn items(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.items.iter().copied().filter(|kind| kind.1 > 0)
    }
}

/// The models an enemy's script has loaded, as far as the game has them ready.
pub trait Models {
    /// The animation player of the model in `slot`.
    fn animator(&mut self, slot: usize) -> Option<&mut Animator>;
    /// Where the node with id `node` of the model in `slot` is, in the model's space.
    fn node(&self, slot: usize, node: u8) -> Option<Transform>;
    /// Turns the node with id `node` of the model in `slot` by `rotation` in its own
    /// frame, on top of its animation (0xAA730).
    fn turn(&mut self, slot: usize, node: u8, rotation: Quat);
}

/// The scene's last draw list, as a script names it for an effect (0xBD460): what
/// is of it is drawn after all else, whatever is nearer.
const LAST_LAYER: i32 = 4;
/// How strongly what is behind is dimmed as an enemy takes the stage, and the
/// sixtieths that takes (0xBE244).
const STAGE_DIM: i32 = 0xc4;
const STAGE_DIM_TIME: i32 = 0x14;

/// The effect a hit is seen as, by the look its attack has (0x9089C), and the look
/// a guarded hit has in place of each (0x9081C).
const HIT_EFFECTS: [u32; 11] = [0, 999_002, 998_022, 998_021, 998_019, 999_033, 998_046, 998_046, 998_047, 998_047, 998_048];
const GUARDED_LOOKS: [usize; 11] = [0, 0, 1, 3, 0, 4, 5, 7, 8, 9, 4];
/// The effect of a guard that takes a hit (0x92A40: 0x7B4AA), and the burst of
/// an attack that does more to an enemy that is broken (0x92B00).
const EFFECT_GUARDED: u32 = 505_002;
const EFFECT_BROKEN_MORE: u32 = 998_048;

/// The effects a hit on an enemy is seen as, in the order they are made, each with
/// whether it is drawn over everything (0x92A38): for a guarded hit the guard's own
/// and the look a guarded hit of the attack has; or else the burst of an attack
/// that does more to an enemy that is broken, if it is one, and the attack's look.
fn seen_of_hit(look: i32, guarded: bool, broken_more: bool) -> Vec<(u32, bool)> {
    let mut seen = Vec::new();
    if guarded {
        seen.push((EFFECT_GUARDED, false));
    } else if broken_more {
        seen.push((EFFECT_BROKEN_MORE, true));
    }
    let look = usize::try_from(look).ok();
    let look = if guarded { look.and_then(|look| GUARDED_LOOKS.get(look).copied()) } else { look };
    if let Some(id) = look.and_then(|look| HIT_EFFECTS.get(look).copied()).filter(|id| *id > 0) {
        seen.push((id, true));
    }
    seen
}

/// What an enemy asks of the game around it.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Put the model with this file name in a slot.
    Model { slot: usize, name: String },
    /// Take the model in a slot away.
    Unload { slot: usize },
    /// A sound of a bank, known from now on by `handle`, at a volume and a note.
    Sound { handle: u32, bank: i32, id: i32, volume: i32, note: i32 },
    StopSound(i32),
    /// An effect at a node of one of its slots, `offset` from it and turned by
    /// `turn`; `fixed` to the node, or left where the node was. `on_top` if it is
    /// of the scene's last draw list.
    Effect { id: i32, slot: i32, node: i32, offset: Vec3, turn: Quat, fixed: bool, on_top: bool },
    /// An effect of the battle's own at a place: what is seen of a hit.
    Burst { id: u32, at: Vec3, on_top: bool },
    /// What a hit took off it, to be put up where the hit landed (0x92A10).
    Number { value: i32, at: Vec3 },
    StopEffect(i32),
    /// Every effect that is its own ends.
    StopEffects,
    /// What is in a slot is drawn, or no longer.
    Show { slot: usize, shown: bool },
    /// A node of the model in a slot throws a round shadow on the ground, so
    /// large and so strong (of one).
    Shadow { slot: usize, node: u8, radius: f32, strength: f32 },
    /// Its script shakes the camera (0xBDBC8): how strong by a number of its own,
    /// the lean of the shake's line in degrees, and the logic frames it takes to
    /// die away.
    Shake { kind: i32, degrees: i32, frames: i32 },
    /// A hit on it shakes the camera, by the word of its attack (0x92A30).
    HitShake(i32),
    /// A line of the battle log about this enemy.
    Log(i32),
    /// A line of the battle log about the part of it one of its gauges is of.
    PartLog { line: i32, gauge: usize },
    /// A hit of hers has ended it: with whether its first gauge is broken as the hit
    /// is done with, which is what the records are told (0x92C0C).
    Beaten { broken: bool },
    /// A recording, by its file's name, on one of the streams.
    Voice { stream: i32, name: String },
    /// The flash that warns of an attack.
    Warning,
    /// It fires a bullet at `aim`. `node` is the frame of the node of its model that
    /// the bullet's record names, if the model has it.
    Shoot { shot: Shot, node: Option<Transform>, aim: Vec3 },
    /// It gives the player a status effect, or takes one from her.
    Status { id: i32, give: bool },
    /// It gives every other enemy a status effect (and itself with `itself`), or
    /// takes one from them.
    GroupStatus { id: i32, give: bool, itself: bool },
    /// What became of a status effect it gave itself.
    OwnStatus { id: i32, outcome: status::Outcome },
    /// It has called one of the group's reserves into the battle, to be enemy
    /// number `index` at `place`.
    Reinforce { index: usize, reserve: usize, place: Member },
    /// A scene of its own begins (0xBCFEC): a camera move of its archives plays in
    /// the frame of a node of one of its slots, and she is put in the scene's
    /// frame, with whatever her animation was blending from dropped.
    Scene { camera: String, slot: i32, node: i32 },
    /// The scene is over (0xBD284): the camera is the battle's again, and she is
    /// back at her standing place.
    SceneOver,
    /// An animation of her model, by its file's name in what is loaded (0xBD7F8).
    HerAnimation { name: String, looping: bool, speed: i32 },
    /// She is moved by so much in her own frame (0xBEC68).
    HerMoved(Vec3),
    /// She is put at the origin, facing a way (0xBEB78).
    HerTurned(f32),
    /// Her model is drawn, or is not (0xBE118).
    HerShown(bool),
    /// What her model shows (0x9B608), and what its own does (0x92CB0).
    HerParts(i32),
    Parts(i32),
    /// How near and how far the cameras of a draw list see (0xA7CAC), until told
    /// otherwise; and no longer that (0xA7CE0).
    Lens { list: i32, near: f32, far: f32 },
    LensBack,
    /// It takes the stage (0xBE194), or gives it back (0xBE26C).
    Stage(bool),
    /// What is behind is dimmed to a strength over so many sixtieths (0xBE150).
    Dim { to: i32, time: i32 },
    /// The cut-in (`ifBtlCutinTask`).
    CutIn(CutIn),
    /// The finish begins, with a record of its table (0xC0D78).
    Finish(i32),
    /// A sheet of a colour over the whole screen (0xA2590), and its going over so
    /// many sixtieths (0xA25C4).
    Sheet(u32),
    SheetGoes(i32),
    /// The battle is ready for the screen to come to it (0x90800).
    Go,
    /// The fight begins (0x93FA0).
    Fight,
    /// The display comes, or goes (the root's flag 11).
    Display(bool),
    /// A mouth is turned so far about its hinge (0xBC2F0): hers, or its own.
    Jaw { hers: bool, angle: f32 },
    /// Both streams stop, every effect of the battle ends and every bullet goes
    /// (0xBED60); or the bullets alone (0x87AE8).
    Hush,
    NoBullets,
    /// The music stops, over so long if that is more than nothing (0xA10C).
    MusicOff(i32),
    /// Every other enemy goes (0x917CC).
    Dismiss,
    /// Every enemy that is dying is made again where it belongs (0x91830), and
    /// with `think` is sent signal 4 as well.
    Revive { think: bool },
}

/// The signal an enemy that has been made again is sent when whoever had it made
/// says so (0x91894).
pub const SIGNAL_MADE_AGAIN: i32 = 4;

/// What one of the part modes of an enemy that is of her build shows and hides of
/// its model (0x92CB0, by the table at 0x1694C0), by the parts' numbers as
/// `player::part` has them: 0 to 2 a shape of the right hand, with the cannon that
/// is of the model itself out of sight, and 5 the first of them with that cannon;
/// 3, 4 and 6 the left hand, the last with the blade; 7 and 8 hide and show a part
/// of its own.
pub fn part_mode(mode: i32) -> Vec<(u16, bool)> {
    use crate::player::part::*;
    const OWN: u16 = 0x37;
    let right = |shown: u16, cannon: bool| vec![(RIGHT_HAND_CANNON, shown == RIGHT_HAND_CANNON), (RIGHT_HAND_OTHER, shown == RIGHT_HAND_OTHER), (RIGHT_HAND_THIRD, shown == RIGHT_HAND_THIRD), (CANNON, cannon)];
    let left = |hand: u16, blade: bool| vec![(LEFT_HAND_BLADE, hand == LEFT_HAND_BLADE), (LEFT_HAND_EMPTY, hand == LEFT_HAND_EMPTY), (BLADE, blade)];
    match mode {
        0 => right(RIGHT_HAND_CANNON, false),
        1 => right(RIGHT_HAND_OTHER, false),
        2 => right(RIGHT_HAND_THIRD, false),
        3 => left(LEFT_HAND_BLADE, false),
        4 => left(LEFT_HAND_EMPTY, false),
        5 => right(RIGHT_HAND_CANNON, true),
        6 => left(LEFT_HAND_BLADE, true),
        7 => vec![(OWN, false)],
        8 => vec![(OWN, true)],
        _ => Vec::new(),
    }
}

/// What a script has the cut-in do.
#[derive(Clone, Debug, PartialEq)]
pub enum CutIn {
    /// Its layout, by the name of its file (0x8DA60).
    Load(String),
    /// A set of the layout plays (0x33204), or is no longer played or drawn (0x33274).
    Play(i32),
    Stop(i32),
    /// It is dropped (0x8DAC4).
    Drop,
}

/// The node of a model that is its jaw (0xBC314), and what a mouth that moves
/// goes on by each frame besides what chance adds (0xBC220).
pub const JAW_NODE: u8 = 6;
const MOUTH_STEP: i32 = 4;

/// A mouth a script has moving (engine function 118, 0xBEE54).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mouth {
    /// Hers, or its own.
    hers: bool,
    pace: i32,
    span: i32,
    /// How far through its opening and shutting it is, in degrees (block + 0x22).
    turned: i32,
}

/// The enemies whose finish is a record of their own (0xC0DB4, by the table at
/// 0x16B348): each of the first six bosses as she first meets it, with the two
/// that are met again after it one and two records on; the last, and the one met
/// again in its place, go by how many finishes there have been. Anybody else has
/// the first.
fn finish_record(enemy: u16, finishes: i32) -> i32 {
    let nth = |first: i32| first + if (0..3).contains(&finishes) { finishes } else { 0 };
    match enemy {
        116..=121 => (i32::from(enemy) - 116) * 3 + 1,
        98..=109 => (i32::from(enemy) - 98) / 2 * 3 + 2 + (i32::from(enemy) - 98) % 2,
        123 => nth(0x13),
        111 => nth(0x16),
        _ => 1,
    }
}

/// Everything an enemy's script reads and changes outside the enemy itself.
pub struct Surroundings<'a> {
    /// Length of the step in sixtieths of a second.
    pub dt: f32,
    /// The battle is decided.
    pub decided: bool,
    pub archive: &'a Archive,
    pub shots: &'a Shots,
    pub effects: &'a Effects,
    pub stage: &'a Stage,
    pub player: PlayerView,
    /// The number of the group the battle is fought against, and what it has for
    /// the enemies that come in later.
    pub group: i32,
    pub plan: Plan<'a>,
    /// What the enemies have between them: each other, this one included, and the
    /// group's reserves.
    pub shared: &'a mut Shared,
    pub random: &'a mut Random,
    pub models: &'a mut dyn Models,
    pub events: &'a mut Vec<Event>,
    /// Somebody has the stage, she or an enemy (the root's flags 9 and 10).
    pub staged: bool,
}

/// Asks for a sound of the battle's bank or of the enemy's own, and answers the
/// number it is known by; nothing is played of a bank that is not there.
fn sound(world: &mut Surroundings, bank: i32, id: i32, volume: i32, note: i32) -> i32 {
    let there = match bank {
        BANK_OF_BATTLE => true,
        BANK_OF_ENEMY => world.archive.bank.is_some(),
        BANK_OF_FINISH => world.archive.finish_bank.is_some(),
        _ => false,
    };
    if !there || id < 0 {
        return NO_SOUND;
    }
    let handle = SOUND_HANDLES + world.shared.sounds;
    world.shared.sounds = (world.shared.sounds + 1) % SOUND_HANDLES;
    world.events.push(Event::Sound { handle, bank, id, volume: volume.clamp(0, FULL_VOLUME), note });
    handle as i32
}

/// What the script is waiting for while it is suspended.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    /// Sixtieths of a second left.
    Time(f32),
    /// The animation of a slot to reach its end.
    Animation(usize),
    /// The current move to end.
    Move,
    /// The action clock to reach this many sixtieths.
    Clock(f32),
    /// The screen to have come to the battle (0xBE7C4).
    Encounter,
    /// A stream to be silent (0xBDFB8).
    Silence(usize),
    /// The cut-in's set to have played through (0xBE38C).
    CutIn,
    /// The finish to be over (0xBE560).
    Finish,
}

/// The enemy apart from its script machine: what the engine functions work on.
pub struct Body {
    /// Which of the battle's enemies this is: its place among the peers.
    pub index: usize,
    /// The enemy's number and the variant's figures.
    pub enemy: u16,
    pub variant: Variant,
    /// How it enters when it comes as a wave: 1 drops in, 2 rises.
    pub entry: i32,
    /// Beating it is worth its experience and what it may leave behind. One that was
    /// called into the battle is worth nothing (the word at +0x23A, set at 0xC0C20).
    pub rewards: bool,
    /// Where it belongs.
    pub home: Vec3,
    /// The point its moves aim at.
    pub aim: Vec3,
    /// The point its shots aim at.
    pub shot_aim: Vec3,
    pub max_hp: f32,
    pub hp: f32,
    /// A second life, where the variant has one, has been used.
    revived: bool,
    /// How strong an attack has to be to make it reel.
    pub poise: i32,
    pub dead: bool,
    pub dying: bool,
    /// The script's AI runs.
    pub thinking: bool,
    pub gauge_shown: bool,
    /// The actor is to be taken out of the battle.
    pub remove: bool,
    pub guarding: bool,
    /// Knocked down: set and cleared by the script.
    pub down: bool,
    /// The slot last hit and the damage it took.
    last_hit: (i32, i32),
    /// The HP it has been given back, each time, until the battle has put the
    /// numbers up.
    pub healed: Vec<i32>,
    pub slots: [Option<Slot>; SLOTS],
    /// Where each model slot stands.
    pub placements: [Transform; SLOTS],
    pub gauges: [Option<Gauge>; GAUGES],
    /// Which gauge a hit on each slot wears down.
    slot_gauge: [i32; SLOTS],
    /// The slot, and the node of its model, that what is in a slot is fixed to
    /// (0x97CB8): it is placed where that is each frame.
    attached: [Option<(i32, i32)>; SLOTS],
    /// The slot and node the HP gauge hangs on, and where from there.
    pub hp_gauge: (i32, i32, Vec3),
    pub mover: Mover,
    /// The status effects on it (the original's block at 0x284).
    pub status: Status,
    /// Time the attack warning's flash has run, and its length.
    pub warning: (f32, f32),
    wait: Option<Wait>,
    /// A scene of its own is on: where the movement block was and how it was
    /// turned and turning before it, and the slot that was put in the scene's
    /// frame (block + 0x190, 0x1D0, 0x1D4 and 0x18).
    scene: Option<(Vec3, f32, f32, i32)>,
    /// A mouth it has moving.
    mouth: Option<Mouth>,
    /// A signal asked for the running update to be thrown away.
    cancel: bool,
    /// The time step as the script's waits see it: zero outside the update.
    script_dt: f32,
    /// Engine functions that are not ported and have been called, for telling of
    /// each only once; and kinds of move likewise.
    unported: HashSet<(Table, usize)>,
    unported_moves: HashSet<i32>,
    /// The effect its script last kept in a slot of its own (function 24): the
    /// slot, the effect's number, and which of what it has asked this is.
    kept: Option<(i32, i32, usize)>,
}

impl Body {
    /// Whether it has no HP and no life left (0x91218).
    pub fn is_dead(&self) -> bool {
        self.hp <= 0.0 && self.second_life() <= 0
    }

    /// The HP a second life would start with (0x91174).
    pub fn second_life(&self) -> i32 {
        if self.revived { 0 } else { self.variant.second_hp }
    }

    fn sphere_mut(&mut self, index: i32) -> Option<&mut Sphere> {
        match usize::try_from(index).ok().and_then(|index| self.slots.get_mut(index))? {
            Some(Slot::Sphere(sphere)) => Some(sphere),
            _ => None,
        }
    }

    /// The collision spheres, with the slots they are in.
    pub fn spheres(&self) -> impl Iterator<Item = (usize, &Sphere)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| match slot {
            Some(Slot::Sphere(sphere)) => Some((index, sphere)),
            _ => None,
        })
    }

    /// The frame of what is in `slot`, or of node `node` of its model, in the world.
    /// A sphere's frame is its parent's, moved to the sphere's middle: other things
    /// (a lock point) can hang off a sphere just as off a model.
    fn frame_of(&self, slot: i32, node: i32, models: &dyn Models, depth: usize) -> Option<Transform> {
        let index = usize::try_from(slot).ok().filter(|index| *index < SLOTS)?;
        if let Some(Slot::Sphere(sphere)) = &self.slots[index] {
            // A sphere fixed to itself, or spheres to each other in a ring, would go
            // round for ever.
            let parent = self.frame_of(sphere.parent, sphere.node, models, depth.checked_sub(1)?)?;
            return Some(parent * Transform::from_translation(sphere.offset));
        }
        let on_model = match u8::try_from(node) {
            Ok(node) => models.node(index, node)?,
            Err(_) => Transform::IDENTITY,
        };
        Some(self.placements[index] * on_model)
    }

    /// Where something fixed to node `node` of slot `slot`, `offset` from it, is in
    /// the world.
    pub fn place_of(&self, slot: i32, node: i32, offset: Vec3, models: &dyn Models) -> Option<Vec3> {
        Some(self.frame_of(slot, node, models, SLOTS)?.transform_point(offset))
    }

    /// The middle of the sphere in slot `index`, in the world.
    pub fn sphere_centre(&self, index: usize, models: &dyn Models) -> Option<Vec3> {
        match self.slots.get(index)? {
            Some(Slot::Sphere(_)) => self.place_of(index as i32, -1, Vec3::ZERO, models),
            _ => None,
        }
    }

    /// Which gauge a hit on the sphere in `slot` wears down (0x91A5C).
    #[allow(dead_code)]
    pub fn gauge_of(&self, slot: usize) -> usize {
        self.slot_gauge.get(slot).and_then(|gauge| usize::try_from(*gauge).ok()).unwrap_or(0)
    }

    /// What the other enemies see of this one (the figures 0xA0C80, 0x91A80 and
    /// 0xA0D50 read).
    pub fn peer(&self, token: i32) -> Peer {
        let hp_percent = if self.max_hp > 0.0 { (self.hp / self.max_hp * 100.0) as i32 } else { 100 };
        Peer {
            token,
            alive: self.standing() && !self.remove,
            hp_percent,
            home: self.home,
            in_the_way: !self.remove && self.hp > 0.0,
        }
    }

    /// Neither dead nor dying (the test of 0xA0BFC and 0xA0C80). One that has asked
    /// to be removed still stands until its turn is over and it goes.
    fn standing(&self) -> bool {
        !self.dead && !self.dying
    }

    /// Whether the slot its HP gauge hangs on has anything in it (0xA7F58): what the
    /// engine asks before it gives an enemy a status effect on another's behalf.
    pub fn anchored(&self) -> bool {
        usize::try_from(self.hp_gauge.0).ok().and_then(|slot| self.slots.get(slot)).is_some_and(Option::is_some)
    }

    /// Gives it HP back, up to what it can have, and has the number put up by it
    /// (0x91750).
    pub fn heal(&mut self, amount: i32) {
        if amount > 0 {
            self.healed.push(amount);
        }
        self.hp = (self.hp + amount as f32).min(self.max_hp);
    }

    /// What it counts for as the giver of a status effect: its level, its special
    /// attack and a boost on itself (0x8FE94-0x8FEB0).
    pub fn giver(&self) -> Giver {
        Giver { level: self.variant.level, special: self.variant.special, boost: self.status.boost(), name: self.variant.name.clone() }
    }

    /// Whoever it is as it lands a hit.
    pub fn assailant(&self) -> Assailant<'_> {
        let figures = crate::battle::damage::Attacker { attack: self.variant.attack, special: self.variant.special };
        Assailant { figures, level: self.variant.level, status: &self.status }
    }

    /// Gives it a status effect (0x8FE28), if the chance lets it take, and does what
    /// an effect that acts at once does to an enemy.
    pub fn afflict(&mut self, effect: &Effect, giver: &Giver, random: &mut Random) -> status::Outcome {
        let taker = Taker { level: self.variant.level, resistance: self.variant.resistance(effect.kind), max_hp: self.max_hp as i32 };
        let outcome = self.status.give(effect, giver, &taker, random);
        match outcome {
            status::Outcome::Healed(amount) => self.heal(amount),
            status::Outcome::Figures(by) => {
                // Its own figures change for good, each staying within 0 and 999
                // (0x90514).
                let variant = &mut self.variant;
                for (figure, by) in [&mut variant.attack, &mut variant.special, &mut variant.defence, &mut variant.guard_percent].into_iter().zip(by) {
                    *figure = (*figure + by).clamp(0, 999);
                }
            }
            _ => {}
        }
        outcome
    }
}

/// `part` of `whole` as a percentage cut to a whole number; zero when there is no whole.
fn percent(part: f32, whole: f32) -> i32 {
    if whole > 0.0 { (part * 100.0 / whole) as i32 } else { 0 }
}

pub struct EnActor {
    machine: Machine,
    update: Option<usize>,
    signal: Option<usize>,
    /// The script is running: a signal sent now is dropped.
    running: bool,
    pub body: Body,
}

/// Where and as what an enemy joins the battle.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub index: usize,
    pub variant: i32,
    pub position: Vec3,
    /// It enters as a wave, in this manner, instead of standing there from the start.
    pub wave: Option<i32>,
}

impl EnActor {
    /// Builds the enemy and has its script set it up (0x91BBC).
    pub fn new(spawn: Spawn, world: &mut Surroundings) -> Result<EnActor> {
        let script = &world.archive.script;
        let variant = world.archive.variant(spawn.variant).clone();
        let mut actor = EnActor {
            machine: Machine::new(script),
            update: script.entry(FunctionKind::Update),
            signal: script.entry(FunctionKind::Signal),
            running: false,
            body: Body {
                index: spawn.index,
                enemy: world.archive.enemy,
                entry: spawn.wave.unwrap_or(0),
                rewards: spawn.wave.is_none(),
                home: spawn.position,
                aim: Vec3::ZERO,
                shot_aim: Vec3::ZERO,
                max_hp: variant.hp as f32,
                hp: variant.hp as f32,
                revived: false,
                poise: variant.poise,
                dead: false,
                dying: false,
                thinking: spawn.wave.is_some(),
                gauge_shown: false,
                remove: false,
                guarding: false,
                down: false,
                last_hit: (0, 0),
                healed: Vec::new(),
                slots: std::array::from_fn(|_| None),
                placements: [Transform::IDENTITY; SLOTS],
                gauges: [None; GAUGES],
                slot_gauge: [0; SLOTS],
                attached: [None; SLOTS],
                hp_gauge: (0, -1, Vec3::ZERO),
                mover: Mover::default(),
                // The log has its name for it (0x910C0), and its marks are by its bar.
                status: Status::of(&variant.name, status::ENEMY_ROOM),
                warning: (WARNING_TIME, WARNING_TIME),
                wait: None,
                scene: None,
                mouth: None,
                cancel: false,
                script_dt: 0.0,
                unported: HashSet::new(),
                unported_moves: HashSet::new(),
                kept: None,
                variant,
            },
        };
        if let Some(init) = script.entry(FunctionKind::Init) {
            let mut host = ScriptHost { body: &mut actor.body, world };
            actor.machine.call(script, init, &[], &mut host).context("the script's start-up function")?;
            actor.machine.abort();
        }
        actor.send(if spawn.wave.is_some() { signal::ENTER } else { signal::SET_UP }, world)?;
        Ok(actor)
    }

    /// Sends the script a signal (0xAE0D8). Returns what its signal function answers.
    pub fn send(&mut self, number: i32, world: &mut Surroundings) -> Result<i32> {
        let Some(function) = self.signal else { return Ok(0) };
        if self.running {
            return Ok(0);
        }
        self.running = true;
        let mut host = ScriptHost { body: &mut self.body, world };
        let answer = self.machine.signal(&world_script(host.world), function, &[number, 0, 0], &mut host);
        self.running = false;
        let answer = answer.with_context(|| format!("signal {number}"))?;
        // A signal can cancel what the update was in the middle of (0xC98E4).
        if std::mem::take(&mut self.body.cancel) {
            self.machine.abort();
            self.body.wait = None;
        }
        Ok(answer)
    }

    /// Runs the script's update for one frame (0xADFBC): carries on from a wait that
    /// is over, and starts the update again whenever it is not in the middle of one.
    fn run_script(&mut self, world: &mut Surroundings) -> Result<()> {
        let Some(update) = self.update else { return Ok(()) };
        if self.running {
            return Ok(());
        }
        self.running = true;
        self.body.script_dt = world.dt;
        let result = (|| -> Result<()> {
            if self.machine.suspended() && self.wait_is_over(world) {
                self.body.wait = None;
                let mut host = ScriptHost { body: &mut self.body, world };
                self.machine.resume(&world_script(host.world), &mut host)?;
            }
            if !self.machine.suspended() {
                let mut host = ScriptHost { body: &mut self.body, world };
                self.machine.call(&world_script(host.world), update, &[], &mut host)?;
            }
            Ok(())
        })();
        self.body.script_dt = 0.0;
        self.running = false;
        result.context("the script's update")
    }

    /// Asks the wait's own check whether the script may carry on (the callbacks at
    /// 0xBC768, 0xBC7F4 and 0xBCF28).
    fn wait_is_over(&mut self, world: &mut Surroundings) -> bool {
        match &mut self.body.wait {
            None => true,
            Some(Wait::Time(left)) => {
                *left -= self.body.script_dt;
                *left <= 0.0
            }
            Some(Wait::Animation(slot)) => match world.models.animator(*slot) {
                Some(animator) => animator.body().time() == animator.body().range().1,
                // A model that is not there has no animation to wait for.
                None => true,
            },
            Some(Wait::Move) => self.body.mover.ended(),
            Some(Wait::Clock(time)) => *time <= self.body.mover.clock,
            // The encounter is told to go each frame it is waited for.
            Some(Wait::Encounter) => {
                world.events.push(Event::Go);
                world.shared.told.encounter_over
            }
            Some(Wait::Silence(stream)) => world.shared.told.silent[*stream],
            Some(Wait::CutIn) => !world.shared.told.cut_in_plays,
            Some(Wait::Finish) => !world.shared.told.finishing,
        }
    }

    /// One logic frame (0x91F4C). The models' animations must already have advanced.
    pub fn update(&mut self, world: &mut Surroundings) -> Result<()> {
        let dt = world.dt;
        let body = &mut self.body;
        if body.warning.0 < body.warning.1 {
            body.warning.0 = (body.warning.0 + dt).min(body.warning.1);
        }
        // A dying enemy waits to be taken away (part of 0x90EE4).
        if body.dying {
            return Ok(());
        }

        for (number, gauge) in body.gauges.iter_mut().enumerate() {
            // The log says so when one is whole again (0x85EEC).
            if gauge.as_mut().is_some_and(|gauge| gauge.update(dt)) {
                world.events.push(Event::PartLog { line: log::PART_MENDED, gauge: number });
            }
        }
        // While somebody has the stage, she or one of them, neither of the next
        // two is looked at (0x92068 to 0x92124).
        if !world.staged {
            // An enemy whose first gauge is broken is stunned: nothing of it runs,
            // its status effects neither.
            if body.gauges[0].is_some_and(|gauge| gauge.broken) {
                return Ok(());
            }
            // Its status effects run on, and a regeneration gives what it gives
            // (0x92110). Poison is the player's alone to suffer.
            body.status.tick();
            if let Some(back) = body.status.regeneration(dt, body.max_hp as i32) {
                body.heal(back);
            }
        }

        // A mouth it has moving goes on (0xAF4B8, as the movement block's frame
        // begins).
        if let Some(mouth) = &mut body.mouth {
            if let Some(angle) = mouth.frame(world.random) {
                world.events.push(Event::Jaw { hers: mouth.hers, angle });
            }
        }

        let mut around = mover::Around {
            dt,
            home: body.home,
            player: world.player.position,
            ankle: world.player.ankle,
            called: !body.rewards,
            aim: &mut body.aim,
            random: &mut *world.random,
        };
        if body.mover.update(&mut around) {
            body.place_moved();
        }
        // A move can turn one node of the first slot's model as well.
        if let Some((node, angle)) = body.mover.node_turn {
            world.models.turn(0, node, Quat::from_rotation_y(angle));
        }
        if let Some(kind) = body.mover.unknown.take() {
            if mover::original_has(kind) && body.unported_moves.insert(kind) {
                warn!("{}: move {kind} is not ported: it ends where it starts", world.archive.folder);
            }
        }

        if !self.body.is_dead() && !self.body.dead && self.body.thinking {
            self.send(signal::THINK, world)?;
        }
        self.run_script(world)?;
        self.body.place_attached(&*world.models);
        Ok(())
    }

    /// A hit on the sphere in `slot`, at the place `at`, wearing gauge `gauge`
    /// (0x92510). Returns the damage done, and the HP the attack gives back to
    /// whoever dealt it.
    pub fn hit(&mut self, slot: usize, at: Vec3, gauge: usize, attack: &Attack, by: Assailant, world: &mut Surroundings) -> Result<(i32, i32)> {
        let body = &mut self.body;
        // A stun ends with the hit (0x8F420).
        body.status.struck();
        let broken = body.gauges[0].is_some_and(|gauge| gauge.broken);
        // A ward makes every hit a guarded one; a broken enemy guards nothing, an
        // attack that cannot be guarded is not, and nor is one whose effect breaks
        // guards (0x92584-0x925F4).
        let effect = world.effects.get(attack.effect);
        let mut guarded = (body.guarding || body.status.has(kind::WARD)) && !broken && !attack.unguardable;
        if effect.is_some_and(Effect::breaks_guard) {
            guarded = false;
        }
        let mut factors = status_factors(by.status, &body.status).to_vec();
        // Some attacks do a great deal more to an enemy that is broken (0x9278C).
        let bonus = effect.map_or(0, Effect::break_bonus);
        if broken && bonus != 0 {
            factors.push((bonus + 100) as f32 / 100.0);
        }
        if guarded {
            factors.push(body.variant.guard_percent as f32 / 100.0);
        }
        let dealt = damage(attack, by.figures, body.variant.defence, &factors, world.random);

        // Once the battle is decided a hit takes nothing off (0x9283C), and no
        // number is put up for it (0x929FC).
        if !world.decided {
            world.shared.spoils.damage_dealt += dealt;
            body.hp -= dealt as f32;
            world.events.push(Event::Number { value: dealt, at });
        }
        let mut killed = false;
        if body.hp <= 0.0 {
            body.hp = 0.0;
            for gauge in body.gauges.iter_mut().flatten() {
                gauge.mend();
            }
            killed = body.is_dead();
        }
        let mut drained = 0;
        if !killed {
            // The log says so when the blow breaks it (0x86170).
            if body.gauges.get_mut(gauge).and_then(Option::as_mut).is_some_and(|worn| worn.wear(attack.break_power)) {
                world.events.push(Event::PartLog { line: log::PART_BROKEN, gauge });
            }
            // What the attack carries (0x92984): a share of the damage back to
            // whoever dealt it, and a status effect for the enemy.
            if let Some(effect) = effect {
                drained = (dealt * effect.drain() / 100).max(0);
                let giver = Giver { level: by.level, special: by.figures.special, boost: by.status.boost(), name: by.status.bearer().to_owned() };
                let outcome = body.afflict(effect, &giver, world.random);
                world.events.push(Event::OwnStatus { id: effect.id, outcome });
            }
        }
        // The records are told of an enemy her hit has ended (0x92C0C), and whether
        // its first gauge is broken then: mended as it fell, it never is.
        if killed {
            world.events.push(Event::Beaten { broken: body.gauges[0].is_some_and(|gauge| gauge.broken) });
        }
        body.last_hit = (slot as i32, dealt);
        // What is seen of it, where it lands (0x92A38): a guard's own effect and
        // the look a guarded hit of the attack has; or else the look of the
        // attack, after the burst of one that does more to an enemy that is
        // broken. All but the guard's are drawn over everything.
        world.events.push(Event::HitShake(attack.shake));
        for (id, on_top) in seen_of_hit(attack.look, guarded, broken && bonus != 0) {
            world.events.push(Event::Burst { id, at, on_top });
        }
        // What is heard of it (0x92A74, 0x92BA0): a guarded hit, and the attack's
        // own sound if that is one of the battle's.
        if guarded {
            sound(world, BANK_OF_BATTLE, SOUND_GUARDED, FULL_VOLUME, NOTE);
        }
        if attack.sound >= 0 && attack.sound_bank == BANK_OF_BATTLE {
            sound(world, BANK_OF_BATTLE, attack.sound, FULL_VOLUME, NOTE);
        }
        self.send(signal::HIT, world)?;
        Ok((dealt, drained))
    }

    /// What the script is up to, for following it in the log: its first two variables
    /// (by convention the state and whether it has run its course), what it waits
    /// for, and the move under way.
    pub fn trace(&self) -> String {
        let body = &self.body;
        format!(
            "state {} ({}) wait {:?} move {} flags {:#x} thinking {}",
            self.machine.globals[0],
            self.machine.globals[1],
            body.wait.map(|wait| match wait {
                Wait::Time(_) => "time",
                Wait::Animation(_) => "animation",
                Wait::Move => "move",
                Wait::Clock(_) => "clock",
                Wait::Encounter => "the encounter",
                Wait::Silence(_) => "silence",
                Wait::CutIn => "the cut-in",
                Wait::Finish => "the finish",
            }),
            body.mover.kind(),
            body.mover.flags,
            body.thinking,
        )
    }

}

impl Mouth {
    /// A frame of it (0xBC154): how far the jaw is to be turned about its hinge,
    /// if it is to be turned at all. With neither span nor pace the mouth is shut;
    /// with a pace alone it is left as it is; a span with no pace has a pace of two.
    fn frame(&mut self, random: &mut Random) -> Option<f32> {
        if self.span == 0 {
            return (self.pace == 0).then_some(0.0);
        }
        if self.pace == 0 {
            self.pace = 2;
        }
        // One time in four it goes on by up to twice the pace, one in four by up
        // to three times it.
        let bound = match random.below(4) {
            2 => self.pace * 2,
            3 => self.pace * 3,
            _ => self.pace,
        };
        self.turned = (self.turned + random.below(bound) + MOUTH_STEP) % (self.span * 20);
        Some((-(self.turned as f32).to_radians()).sin() / 6.0)
    }
}

impl Body {
    /// Gives it the HP of its second life, if it has one left (0x911A8).
    pub fn revive(&mut self) {
        let hp = self.second_life();
        if hp > 0 {
            self.revived = true;
            self.max_hp = hp as f32;
            self.hp = hp as f32;
        }
    }
}

/// The script of the enemy the surroundings are of.
fn world_script<'a>(world: &Surroundings<'a>) -> &'a Script {
    &world.archive.script
}

impl Body {
    /// Puts what is fixed to a node of another slot where that node is now.
    fn place_attached(&mut self, models: &dyn Models) {
        for slot in 0..SLOTS {
            let Some((parent, node)) = self.attached[slot] else { continue };
            if let Some(frame) = self.frame_of(parent, node, models, SLOTS) {
                self.placements[slot] = frame;
            }
        }
    }

    /// Puts the slots the movement block moves where it has them (the end of 0xB9B50).
    fn place_moved(&mut self) {
        let placement = self.mover.placement();
        for slot in [self.mover.slot, self.mover.second_slot] {
            if let Some(place) = usize::try_from(slot).ok().and_then(|slot| self.placements.get_mut(slot)) {
                *place = placement;
            }
        }
    }
}

/// The engine as an enemy's script sees it.
struct ScriptHost<'a, 'b> {
    body: &'a mut Body,
    world: &'a mut Surroundings<'b>,
}

impl ScriptHost<'_, '_> {
    fn animator(&mut self, slot: i32) -> Option<&mut Animator> {
        self.world.models.animator(usize::try_from(slot).ok()?)
    }

    /// Engine function 7 (0xBC520).
    fn play(&mut self, slot: i32, name: &str, looping: bool, speed: i32, blend: i32, force: bool) {
        let Some(clip) = self.world.archive.clip(name).cloned() else {
            warn!("{}: there is no animation {name}", self.world.archive.folder);
            return;
        };
        let Some(animator) = self.animator(slot) else { return };
        if force || !animator.body().is(&clip) {
            animator.set_blended(&clip, blend as f32);
        }
        if looping {
            animator.body_mut().loop_clip();
        } else {
            animator.body_mut().play_out();
        }
        animator.speed = speed as f32 / 100.0;
    }

    /// Engine function 8 (0xBC684): loop over the animation's own range, or leave
    /// the loop at its end and play the rest.
    fn set_looping(&mut self, slot: i32, on: bool) {
        let Some(animator) = self.animator(slot) else { return };
        let body = animator.body_mut();
        if on {
            body.loop_clip();
        } else {
            let end = body.range().1;
            body.play_out();
            body.set_time(end);
            // The original then steps one logic frame on.
            let time = body.time();
            body.set_time(time + 2.0);
        }
    }

    /// Picks the point the enemy's shots aim at: one of the player's two standing
    /// places, a little above the ground (0x91308).
    fn choose_shot_aim(&mut self, which: i32) {
        let place = match which {
            1 => self.world.random.below(2) as u32,
            2 => 0,
            3 => 1,
            4 => return,
            _ => self.world.player.place,
        };
        self.body.shot_aim = self.world.stage.slot(place) + Vec3::Y * SHOT_AIM_HEIGHT;
    }

    /// The record of the bullet table for one of this enemy's shots.
    fn shot(&self, id: i32) -> Option<Shot> {
        self.world.shots.of_enemy(id, self.body.enemy as i32, self.body.variant.id).copied()
    }

    /// How many enemies stand in the battle as this one's script sees it (0xA0BFC,
    /// 0xA0C80): the others as their last turns left them, itself as it is now.
    fn standing(&self) -> usize {
        let own = self.body.index;
        let others = self.world.shared.peers.iter().enumerate().filter(|(index, peer)| *index != own && peer.alive);
        others.count() + self.body.standing() as usize
    }

    /// Engine function 73 (0xC06E8): calls one of the group's reserves into the
    /// battle, if the group's rule lets it come and there is a place for it.
    fn call_reserve(&mut self, first: i32, last: i32, keep: bool) {
        let standing = self.standing();
        let own = (self.body.index, self.body.home, self.body.hp > 0.0);
        let Shared { peers, reserves, .. } = &mut *self.world.shared;
        // Somebody with HP left whose home is near a place keeps it taken (0xA0D50).
        let taken = |at: Vec3| {
            peers.iter().enumerate().any(|(index, peer)| {
                let (home, there) = if index == own.0 { (own.1, own.2) } else { (peer.home, peer.in_the_way) };
                there && home.distance(at) < PLACE_CLEARANCE
            })
        };
        let family = self.world.archive.family;
        let call = reserves.call(&self.world.plan, family, first, last, keep, standing, &taken, self.world.random);
        let Some(call) = call else { return };
        // It is in the battle from this moment, for whoever counts the enemies next.
        let index = peers.len();
        peers.push(Peer { token: 0, alive: true, hp_percent: 100, home: Vec3::from(call.place.position), in_the_way: true });
        self.world.events.push(Event::Reinforce { index, reserve: call.reserve, place: call.place });
        sound(self.world, BANK_OF_BATTLE, SOUND_CALLED_IN, FULL_VOLUME, NOTE);
    }

    /// Fires bullet `id` at the shot aim, and every bullet its record chains to it
    /// (0x914D4).
    fn shoot(&mut self, mut id: i32) {
        for _ in 0..SHOT_CHAIN {
            let Some(shot) = self.shot(id) else { return };
            // It leaves from a node of the model in the first slot (0x88528, 0x86614).
            let node = self.body.frame_of(0, shot.node, &*self.world.models, SLOTS);
            self.world.events.push(Event::Shoot { shot, node, aim: self.body.shot_aim });
            if shot.next == 0 {
                return;
            }
            id = shot.next;
        }
    }

    /// Engine function 19 (0xBF574): the script's action timer and its shots.
    fn action(&mut self, what: i32, a: i32, b: i32, c: i32) -> i32 {
        let dt = self.body.script_dt;
        match what {
            0 => self.body.mover.action_timer = a as f32,
            1 => {}
            2 => self.body.mover.action_timer -= dt,
            3 => {
                self.body.poise = if a >= 0 { a } else { self.body.variant.poise };
                return 0;
            }
            4 | 5 => {
                self.choose_shot_aim(a);
                if what == 4 {
                    let (from, to) = (self.body.mover.position, self.body.shot_aim);
                    self.body.mover.face(from, to, SHOT_AIM_TURN, self.world.dt);
                }
                return 0;
            }
            // How many bullets a volley of this shot is (0x91458).
            6 => return self.shot(a).map_or(0, |shot| shot.count),
            7 => self.shoot(a),
            8 | 9 => {
                let place = self.world.stage.slot((what - 8) as u32);
                self.body.aim = place + Vec3::new(a as f32, b as f32, c as f32);
                return 0;
            }
            _ => return 0,
        }
        self.body.mover.action_timer as i32
    }

    /// Engine function 26 (0xBF7CC): the enemy's own figures.
    fn get(&mut self, what: i32) -> i32 {
        let body = &self.body;
        match what {
            0 => body.hp as i32,
            1 => body.max_hp as i32,
            2 => body.last_hit.0,
            3 => body.last_hit.1,
            5 => body.down as i32,
            7 => body.gauges[0].is_none_or(|gauge| !gauge.broken) as i32,
            10 => body.variant.id,
            11 => body.variant.ai,
            15 => body.entry,
            16 => body.variant.gauges[0][2],
            54 => (body.mover.flags & flag::SCRIPT_8 != 0) as i32,
            100 => (body.mover.flags & flag::SCRIPT_10 != 0) as i32,
            101 => (body.mover.flags & flag::SCRIPT_20 != 0) as i32,
            // 4 and 12 (the hit broke the gauge, the hit makes it reel) are always
            // answered 0 by the original too; 13 is "in hit stop", which nothing sets.
            _ => 0,
        }
    }

    /// Engine function 27 (0xBF988).
    fn set(&mut self, what: i32, a: i32, b: i32, c: i32) {
        let set_flag = |flags: &mut u32, bit: u32, on: bool| *flags = if on { *flags | bit } else { *flags & !bit };
        match what {
            5 => self.body.down = a != 0,
            50..=52 => {
                self.body.hp_gauge.0 = a;
                self.body.hp_gauge.1 = b;
                self.body.hp_gauge.2[(what - 50) as usize] = c as f32;
            }
            53 => {
                if let Some(sphere) = self.body.sphere_mut(a) {
                    sphere.attack_id = b;
                }
            }
            54 => set_flag(&mut self.body.mover.flags, flag::SCRIPT_8, a & 1 != 0),
            // The script has it dead (0x912A0): the first time, it is counted as
            // beaten and the battle is worth that much more.
            55 if a != 0 => {
                if !std::mem::replace(&mut self.body.dead, true) {
                    self.world.shared.spoils.take(&self.body.variant, self.body.rewards, self.world.random);
                }
            }
            56 if a != 0 => self.body.dying = true,
            100 => set_flag(&mut self.body.mover.flags, flag::SCRIPT_10, a & 1 != 0),
            101 => set_flag(&mut self.body.mover.flags, flag::SCRIPT_20, a & 1 != 0),
            // 8, 9 and 14 call functions of the first gauge that do nothing.
            _ => {}
        }
    }

    /// Engine function 68 (0xC0388): the battle as the AI sees it.
    fn sense(&mut self, what: i32, a: i32, b: i32) -> i32 {
        let player = &self.world.player;
        let doing = |action: Action| (player.action == action) as i32;
        match what {
            1 => percent(self.body.hp, self.body.max_hp),
            2 => percent(player.hp, player.max_hp),
            3 => self.world.group,
            4 => {
                let apart = self.body.placements[0].translation - player.position;
                Vec2::new(apart.x, apart.z).length() as i32
            }
            5 => self.standing() as i32,
            6 => self.world.shared.peers[self.body.index].token,
            7 => self.body.is_dead() as i32,
            20 => doing(Action::Shoot),
            22 => doing(Action::Dodge),
            23 => doing(Action::Guard),
            24 => doing(Action::Overheat),
            25 => (player.heat * 100.0) as i32,
            28 | 29 => self.body.gauges[(what - 27) as usize].is_some_and(|gauge| gauge.broken) as i32,
            30 => {
                let peers = self.world.shared.peers.iter().enumerate();
                let others = peers.filter(|(index, peer)| *index != self.body.index && peer.alive);
                others.map(|(_, peer)| peer.hp_percent).fold(100, i32::min)
            }
            // Whether a status effect is in force, by its id (26) or by its kind (27):
            // on the player for a = 0, on itself for 1 (0xC05D8, 0xC061C).
            26 | 27 => {
                let marks = match a {
                    0 => player.status,
                    1 => self.body.status.marks(),
                    _ => return 0,
                };
                (if what == 26 { marks.has_effect(b) } else { marks.has(b) }) as i32
            }
            // Whether she is using one of the skills that shoot (her action 7,
            // 0xC054C).
            21 => doing(Action::SkillShot),
            // A word of the actor's that nothing ever writes (+ 0xA24, 0xC0408):
            // which of its moves a boss has been told to make, and it never is.
            1000 => 0,
            _ => -1,
        }
    }

    /// Engine function 104 (0xC11B4): a condition of the AI.
    fn condition(&mut self, condition: i32, a: i32, b: i32) -> bool {
        match condition {
            0 => true,
            1 => self.sense(1, 0, 0) <= a,
            2 => self.sense(1, 0, 0) > a,
            3 => self.sense(2, 0, 0) <= a,
            4 => self.sense(2, 0, 0) > a,
            6 => self.sense(26, a, b) == 1,
            7 => self.sense(26, a, b) == 0,
            8 => self.sense(27, a, b) == 1,
            9 => self.sense(27, a, b) == 0,
            10 => self.sense(30, 0, 0) <= a,
            22 => self.sense(5, 0, 0) - 1 == a,
            23 => self.sense(5, 0, 0) - 1 <= a,
            24 => self.sense(5, 0, 0) - 1 > a,
            25 => self.sense(3, 0, 0) == a,
            31 => self.sense(4, 0, 0) <= a,
            32 => self.sense(4, 0, 0) > a,
            81 => self.sense(6, 0, 0) == a,
            100..=104 => self.sense(condition - 80, 0, 0) != 0,
            105 => self.sense(25, 0, 0) > a,
            106 => self.sense(25, 0, 0) <= a,
            107 => self.sense(28, 0, 0) != 0,
            108 => self.sense(29, 0, 0) != 0,
            109 => self.sense(28, 0, 0) == 0,
            110 => self.sense(29, 0, 0) == 0,
            111 => self.sense(28, 0, 0) != 0 && self.sense(29, 0, 0) != 0,
            112 => self.sense(28, 0, 0) == 0 && self.sense(29, 0, 0) == 0,
            113 => (self.sense(28, 0, 0) != 0) != (self.sense(29, 0, 0) != 0),
            1000 => self.sense(1000, 0, 0) == a,
            _ => false,
        }
    }

    /// Engine functions 34 and 119 (0xBCFEC, 0xBD138): a scene of its own begins.
    /// The movement block is put in the scene's frame, the origin facing half way
    /// round, with what it was put by; `slot` is placed there, the HP gauge goes,
    /// and the camera move plays in the frame of a node of a slot.
    fn scene(&mut self, camera: &str, slot: i32, frame: (i32, i32)) {
        let body = &mut *self.body;
        let (position, yaw, yaw_target) = body.mover.enter_scene();
        body.scene = Some((position, yaw, yaw_target, slot));
        if let Some(place) = usize::try_from(slot).ok().and_then(|slot| body.placements.get_mut(slot)) {
            *place = body.mover.placement();
        }
        body.gauge_shown = false;
        self.world.events.push(Event::Scene { camera: camera.to_owned(), slot: frame.0, node: frame.1 });
    }

    /// Engine function 35 (0xBD284): the scene is over, and all that is put back.
    fn scene_over(&mut self) {
        let body = &mut *self.body;
        if let Some((position, yaw, yaw_target, slot)) = body.scene.take() {
            body.mover.leave_scene(position, yaw, yaw_target);
            if let Some(place) = usize::try_from(slot).ok().and_then(|slot| body.placements.get_mut(slot)) {
                *place = body.mover.placement();
            }
        }
        body.gauge_shown = true;
        self.world.events.push(Event::SceneOver);
    }

    /// Tells once of an engine function the port does not have.
    fn unported(&mut self, table: Table, number: usize, arguments: &[i32]) {
        if self.body.unported.insert((table, number)) {
            warn!("{}: engine function {number} of {table:?} is not ported (called with {arguments:?})", self.world.archive.folder);
        }
    }
}

/// Engine function 105 (0xC0F30): compares `a` with one of eight of the script's own
/// variables. Conditions 33 to 80 come in six groups of eight: equal, then "the
/// variable is at most a", then "at least a", each for two sets of variables that the
/// script tells apart by which eight it passes.
fn compare_variable(condition: i32, a: i32, variables: &[i32]) -> bool {
    let Some(index) = condition.checked_sub(33).filter(|index| (0..48).contains(index)) else { return false };
    let Some(&variable) = variables.get((index % 8) as usize) else { return false };
    match index / 16 {
        0 => a == variable,
        1 => a >= variable,
        _ => a <= variable,
    }
}

impl Host for ScriptHost<'_, '_> {
    fn engine(&mut self, table: Table, number: usize, arguments: &[i32], script: &Script) -> Result<Reply> {
        if table != Table::Two {
            self.unported(table, number, arguments);
            return Ok(Reply::NONE);
        }
        let on = |value: i32| value != 0;
        let value = match (number, arguments) {
            (1 | 2, [text]) | (3, [text, _]) => {
                // What the scripts' authors left in for themselves.
                if let Ok(text) = script.string(*text) {
                    debug!("enemy {}: {}", self.body.index, text.trim_end());
                }
                0
            }
            // A sound, at a volume and a note; the script is answered with what to
            // stop it by (0xBC3CC, 0xBC4FC).
            (4, [bank, id, volume, note]) => sound(self.world, *bank, *id, *volume, *note),
            (5, [handle]) => {
                self.world.events.push(Event::StopSound(*handle));
                0
            }
            (6 | 13, [bound]) => self.world.random.below(*bound),
            (7, [slot, name, looping, speed, blend, force]) => {
                self.play(*slot, script.string(*name)?, on(*looping), *speed, *blend, on(*force));
                0
            }
            (8, [slot, looping]) => {
                self.set_looping(*slot, on(*looping));
                0
            }
            (9, [frames]) => {
                self.body.wait = Some(Wait::Time((*frames * 2) as f32));
                return Ok(Reply { value: 0, suspend: true });
            }
            (10, [slot]) => {
                // A looping animation has no end to wait for (0xAA544).
                let endless = self.animator(*slot).is_none_or(|animator| animator.body().looping());
                if endless {
                    return Ok(Reply::NONE);
                }
                self.body.wait = Some(Wait::Animation(*slot as usize));
                return Ok(Reply { value: 0, suspend: true });
            }
            (11, [index, enabled, _]) => {
                if *index == -1 {
                    for slot in self.body.slots.iter_mut().flatten() {
                        if let Slot::Sphere(sphere) = slot {
                            sphere.enabled = on(*enabled);
                        }
                    }
                } else if let Some(sphere) = self.body.sphere_mut(*index) {
                    sphere.enabled = on(*enabled);
                }
                0
            }
            (12, [_mode]) => {
                let (home, aim) = (self.body.home, self.body.aim);
                self.body.mover.init(home, aim);
                self.body.placements[0] = self.body.mover.placement();
                0
            }
            (14, [slot, first, last]) => {
                if let Some(animator) = self.animator(*slot) {
                    animator.body_mut().loop_between((*first * 2) as f32, (*last * 2) as f32);
                }
                0
            }
            (15, [what]) => {
                let place = |index: u32| self.world.stage.slot(index);
                match *what {
                    6 => self.world.player.place as i32,
                    7..=9 => place(0)[(*what - 7) as usize] as i32,
                    10..=12 => place(1)[(*what - 10) as usize] as i32,
                    _ => 0,
                }
            }
            (17, [_]) => !self.body.mover.ended() as i32,
            (18, [mode, slot, node, layer, id]) => {
                let (offset, turn, fixed) = (Vec3::ZERO, Quat::IDENTITY, mode & 1 != 0);
                self.world.events.push(Event::Effect { id: *id, slot: *slot, node: *node, offset, turn, fixed, on_top: *layer == LAST_LAYER });
                0
            }
            (19, [what, a, b, c]) => self.action(*what, *a, *b, *c),
            (22, [slot, name]) => {
                let index = usize::try_from(*slot).ok().filter(|slot| *slot < SLOTS).context("model slot out of range")?;
                let name = script.string(*name)?.to_owned();
                self.body.slots[index] = Some(Slot::Model { name: name.clone() });
                self.world.events.push(Event::Model { slot: index, name });
                0
            }
            (23, [index, parent, node, x, y, z, radius, kind, _]) => {
                let slot = usize::try_from(*index).ok().filter(|slot| *slot < SLOTS).context("sphere slot out of range")?;
                self.body.slots[slot] = Some(Slot::Sphere(Sphere {
                    parent: *parent,
                    node: *node,
                    offset: Vec3::new(*x as f32, *y as f32, *z as f32),
                    radius: *radius as f32,
                    attack: kind & 1 != 0,
                    enabled: false,
                    attack_id: 1,
                }));
                0
            }
            (24, [slot, parent, node, id]) => {
                // An effect kept in a slot of its own, fixed to a node of another
                // (0xBCC74). The slot matters only to the functions that show, hide
                // or layer what is in it.
                let (offset, turn, fixed) = (Vec3::ZERO, Quat::IDENTITY, true);
                self.body.kept = Some((*slot, *id, self.world.events.len()));
                self.world.events.push(Event::Effect { id: *id, slot: *parent, node: *node, offset, turn, fixed, on_top: false });
                0
            }
            // What is in a slot is put in one of the scene's draw lists (0xBCCEC,
            // 0xACDD0). Every script that does names the last, for the effect it
            // has just kept in the slot: that is then drawn after all else.
            (42, [slot, layer]) => {
                if let Some((_, id, at)) = self.body.kept.filter(|(kept, ..)| kept == slot) {
                    if let Some(Event::Effect { id: asked, on_top, .. }) = self.world.events.get_mut(at) {
                        if *asked == id {
                            *on_top = *layer == LAST_LAYER;
                        }
                    }
                }
                0
            }
            (25, [stop]) => {
                if on(*stop) {
                    self.body.mover.stop();
                }
                self.body.cancel = true;
                0
            }
            (26, [what]) => self.get(*what),
            (27, [what, a, b, c]) => {
                self.set(*what, *a, *b, *c);
                0
            }
            (28, []) => {
                self.body.mover.flags &= !flag::MOVING;
                self.body.remove = true;
                self.body.cancel = true;
                0
            }
            (29, [slot]) => {
                // The slot's object is deleted (0xA7444), whatever it is.
                if let Some(of_slot) = usize::try_from(*slot).ok().and_then(|slot| self.body.slots.get_mut(slot)) {
                    if matches!(of_slot.take(), Some(Slot::Model { .. })) {
                        self.world.events.push(Event::Unload { slot: *slot as usize });
                    }
                    self.body.attached[*slot as usize] = None;
                }
                0
            }
            (30, [slot, kind, frames]) => {
                self.body.mover.start(*slot, *kind, *frames);
                0
            }
            (31, []) => {
                self.body.wait = Some(Wait::Move);
                return Ok(Reply { value: 0, suspend: true });
            }
            (33, [hold]) => {
                let flags = &mut self.body.mover.flags;
                *flags = if hold & 1 != 0 { *flags | flag::HOLD } else { *flags & !flag::HOLD };
                0
            }
            (36, [slot, frame]) => {
                if let Some(animator) = self.animator(*slot) {
                    animator.body_mut().play_out();
                    animator.body_mut().set_time((*frame * 2) as f32);
                }
                0
            }
            (50, [guard]) => {
                self.body.guarding = on(*guard);
                0
            }
            // The action clock (0xBDAB8, 0xBDAF4, 0xBDB9C): set in logic frames, kept
            // and read in sixtieths.
            (52, [frames]) => {
                self.body.mover.clock = (*frames * 2) as f32;
                0
            }
            (53, []) => self.body.mover.clock as i32,
            (56, [kind, degrees, frames]) => {
                self.world.events.push(Event::Shake { kind: *kind, degrees: *degrees, frames: *frames });
                0
            }
            (62, [id]) => self.world.archive.figures.get(*id),
            (63 | 64, [id]) => {
                self.world.events.push(Event::StopEffect(*id));
                0
            }
            // What the enemies and the stage keep between them (0xBDE60, 0xBDEEC):
            // a word of flags, and the number of towers standing.
            (65, [which, bits, value]) => {
                let reserves = &mut self.world.shared.reserves;
                match *which {
                    0 if on(*value) => reserves.flags |= *bits as u32,
                    0 => reserves.flags &= !(*bits as u32),
                    1 => reserves.towers = *value as u8,
                    _ => {}
                }
                0
            }
            (66, [which, bits]) => match *which {
                0 => (self.world.shared.reserves.flags & *bits as u32 != 0) as i32,
                1 => self.world.shared.reserves.towers as i32,
                _ => 0,
            },
            (67, [what, a, b, c]) => {
                match *what {
                    6 if *b == 1 => {
                        let own = self.body.index;
                        for (index, peer) in self.world.shared.peers.iter_mut().enumerate() {
                            if index != own {
                                peer.token = *a;
                            }
                        }
                    }
                    6 => self.world.shared.peers[self.body.index].token = *a,
                    // A status effect given or taken away (0xC02D8): the player's
                    // for a = 0, its own for 1.
                    8 if *a == 0 => self.world.events.push(Event::Status { id: *b, give: on(*c) }),
                    8 if *a == 1 && on(*c) => {
                        if let Some(effect) = self.world.effects.get(*b) {
                            let giver = self.body.giver();
                            let outcome = self.body.afflict(effect, &giver, self.world.random);
                            self.world.events.push(Event::OwnStatus { id: *b, outcome });
                        }
                    }
                    8 if *a == 1 => {
                        self.body.status.take(*b);
                    }
                    31 => self.choose_shot_aim(*a),
                    _ => {}
                }
                0
            }
            (68, [what, a, b, _]) => self.sense(*what, *a, *b),
            // A recording of its archive, by name, on a stream (0xA7EAC).
            (69, [stream, name]) => {
                self.world.events.push(Event::Voice { stream: *stream, name: script.string(*name)?.to_owned() });
                0
            }
            (70, [frames]) => {
                self.body.wait = Some(Wait::Clock((*frames * 2) as f32));
                return Ok(Reply { value: 0, suspend: true });
            }
            (73, [first, last, keep]) => {
                self.call_reserve(*first, *last, on(*keep));
                0
            }
            (74, [slot, shown]) => {
                if let Some(slot) = usize::try_from(*slot).ok().filter(|slot| *slot < SLOTS) {
                    self.world.events.push(Event::Show { slot, shown: on(*shown) });
                }
                0
            }
            (80, [line]) => {
                self.world.events.push(Event::Log(*line));
                0
            }
            (81, []) => {
                self.body.warning = (0.0, WARNING_TIME);
                self.world.events.push(Event::Warning);
                sound(self.world, BANK_OF_BATTLE, SOUND_WARNING, FULL_VOLUME, NOTE);
                0
            }
            (82, [mode @ (0 | 1), slot, node, x, y, z, layer, id]) => {
                // Like function 18, with the effect moved off its node (0xBD320).
                let (offset, turn) = (Vec3::new(*x as f32, *y as f32, *z as f32), Quat::IDENTITY);
                self.world.events.push(Event::Effect { id: *id, slot: *slot, node: *node, offset, turn, fixed: *mode == 1, on_top: *layer == LAST_LAYER });
                0
            }
            // The same, turned as well: about Y, then X, then Z (0xBFFB8).
            (90, [mode @ (0 | 1), slot, node, x, y, z, rx, ry, rz, layer, id]) => {
                let offset = Vec3::new(*x as f32, *y as f32, *z as f32);
                let angle = |degrees: &i32| (*degrees as f32).to_radians();
                let turn = Quat::from_rotation_y(angle(ry)) * Quat::from_rotation_x(angle(rx)) * Quat::from_rotation_z(angle(rz));
                self.world.events.push(Event::Effect { id: *id, slot: *slot, node: *node, offset, turn, fixed: *mode == 1, on_top: *layer == LAST_LAYER });
                0
            }
            (101, [shown]) => {
                self.body.gauge_shown = on(*shown);
                0
            }
            // A number scaled by the status effects that change the pace of its waits
            // (0xBEAEC).
            (103, [number]) => *number * self.body.status.pace_percent() / 100,
            (104, [condition, a, b, _]) => self.condition(*condition, *a, *b) as i32,
            (105, [condition, a, _, _, variables @ ..]) => compare_variable(*condition, *a, variables) as i32,
            (114, [gauge, slot, node, x, y, z]) => {
                let index = usize::try_from(*gauge).ok().filter(|gauge| *gauge < GAUGES);
                if let Some(index) = index {
                    let figures = self.body.variant.gauges[index];
                    let gauge = self.body.gauges[index].get_or_insert_with(|| Gauge::new(figures, self.world.random));
                    gauge.slot = *slot;
                    gauge.node = *node;
                    gauge.offset = Vec3::new(*x as f32, *y as f32, *z as f32);
                    gauge.active = true;
                }
                0
            }
            (115, [gauge, active]) => {
                let gauge = usize::try_from(*gauge).ok().and_then(|gauge| self.body.gauges.get_mut(gauge)?.as_mut());
                if let Some(gauge) = gauge {
                    gauge.active = on(*active);
                }
                0
            }
            (116, [slot, gauge]) => {
                if let Some(of_slot) = usize::try_from(*slot).ok().and_then(|slot| self.body.slot_gauge.get_mut(slot)) {
                    if (0..GAUGES as i32).contains(gauge) {
                        *of_slot = *gauge;
                    }
                }
                0
            }
            (120, [id, give, itself]) => {
                // A status effect for every other enemy, or the end of one (0xBF020).
                self.world.events.push(Event::GroupStatus { id: *id, give: on(*give), itself: on(*itself) });
                0
            }
            // What follows is the bosses' (see "Engine functions an enemy script
            // uses" in the notes). A scene of its own, and its end.
            (34, [camera, slot, node]) => {
                self.scene(script.string(*camera)?, *slot, (*slot, *node));
                0
            }
            (119, [camera, slot, _, frame]) => {
                self.scene(script.string(*camera)?, *slot, (*frame, -1));
                0
            }
            (35, []) => {
                self.scene_over();
                0
            }
            (45, [_slot, name, looping, speed]) => {
                self.world.events.push(Event::HerAnimation { name: script.string(*name)?.to_owned(), looping: on(*looping), speed: *speed });
                0
            }
            // A slot is moved by so much in its own frame (0xBEC68), or she is.
            (55, [slot, x, y, z]) => {
                let by = Vec3::new(*x as f32, *y as f32, *z as f32);
                if *slot == -1 {
                    self.world.events.push(Event::HerMoved(by));
                } else if let Some(place) = usize::try_from(*slot).ok().and_then(|slot| self.body.placements.get_mut(slot)) {
                    place.translation += place.rotation * by;
                }
                0
            }
            // A slot is put at the origin facing a way, which the block is then
            // turned and turning to (0xBEB78); or she is.
            (111, [slot, degrees]) => {
                let angle = (*degrees as f32).to_radians();
                if *slot == -1 {
                    self.world.events.push(Event::HerTurned(angle));
                } else {
                    self.body.mover.turned(angle);
                    if let Some(place) = usize::try_from(*slot).ok().and_then(|slot| self.body.placements.get_mut(slot)) {
                        *place = Transform::from_rotation(Quat::from_rotation_y(angle));
                    }
                }
                0
            }
            (71, [list, near, far]) => {
                self.world.events.push(Event::Lens { list: (*list != 0) as i32, near: *near as f32, far: *far as f32 });
                0
            }
            (72, []) => {
                self.world.events.push(Event::LensBack);
                0
            }
            // It takes the stage (0xBE194): everybody else is held and out of
            // sight, the display goes, and what is behind is dimmed.
            (75, [_, _, _]) => {
                self.world.shared.told.on_stage = Some(self.body.index);
                self.world.events.push(Event::Stage(true));
                self.world.events.push(Event::Dim { to: STAGE_DIM, time: STAGE_DIM_TIME });
                0
            }
            // And gives it back (0xBE26C): the cut-in is dropped and she is seen.
            (76, []) => {
                self.world.shared.told.on_stage = None;
                self.world.events.push(Event::CutIn(CutIn::Drop));
                self.world.events.push(Event::HerShown(true));
                self.world.events.push(Event::Stage(false));
                0
            }
            (96, [name]) => {
                self.world.events.push(Event::CutIn(CutIn::Load(script.string(*name)?.to_owned())));
                0
            }
            (77, [set]) => {
                self.world.shared.told.cut_in_plays = true;
                self.world.events.push(Event::CutIn(CutIn::Play(*set)));
                0
            }
            (78, [set]) => {
                self.world.events.push(Event::CutIn(CutIn::Stop(*set)));
                0
            }
            (79, [_set]) => {
                self.body.wait = Some(Wait::CutIn);
                return Ok(Reply { value: 0, suspend: true });
            }
            (113, []) => {
                self.world.events.push(Event::CutIn(CutIn::Drop));
                0
            }
            // The finish (0xC0D78): with the record that goes with what it is.
            (83, [_, _, _, _]) => {
                let told = &mut self.world.shared.told;
                let record = finish_record(self.body.enemy, told.finishes);
                told.finishing = true;
                self.world.events.push(Event::Finish(record));
                0
            }
            (84, [_]) => {
                self.body.wait = Some(Wait::Finish);
                return Ok(Reply { value: 0, suspend: true });
            }
            (89, []) => 1,
            // Its archives are all there from the start: there is nothing to
            // load, to wait for or to let go (0xBE5D8, 0xBE684, 0xBE6DC). And
            // three that do nothing in the original either.
            (86 | 88, [_, _]) | (87 | 91, [_]) | (85 | 92, []) => 0,
            // The screen's own (0xBE71C): the encounter let go, a sheet of a
            // colour over everything, and the sheet's going.
            (93, [what, value]) => {
                match *what {
                    0 => self.world.events.push(Event::Go),
                    1 => self.world.events.push(Event::Sheet(*value as u32 & 0xff_ffff)),
                    2 => self.world.events.push(Event::SheetGoes(*value * 2)),
                    _ => {}
                }
                0
            }
            (94, []) => {
                self.body.wait = Some(Wait::Encounter);
                return Ok(Reply { value: 0, suspend: true });
            }
            (97, []) => {
                self.world.events.push(Event::Fight);
                0
            }
            (98 | 99, [_]) => {
                self.world.events.push(Event::Display(number == 98));
                0
            }
            (100, [shown]) => {
                self.world.events.push(Event::HerShown(on(*shown)));
                0
            }
            // Its second life (0x911A8).
            (102, []) => {
                self.body.revive();
                0
            }
            (106, [stream @ (0 | 1)]) => {
                self.body.wait = Some(Wait::Silence(*stream as usize));
                return Ok(Reply { value: 0, suspend: true });
            }
            (106, [_]) => 0,
            (107, [_]) => {
                self.world.events.push(Event::Dismiss);
                0
            }
            (108, [think]) => {
                self.world.events.push(Event::Revive { think: on(*think) });
                0
            }
            // A model in a slot (0xA7830), fixed to a node of another slot's
            // (0x97CB8).
            (110, [slot, name, parent, node]) => {
                let index = usize::try_from(*slot).ok().filter(|slot| *slot < SLOTS).context("model slot out of range")?;
                let name = script.string(*name)?.to_owned();
                self.body.slots[index] = Some(Slot::Model { name: name.clone() });
                self.body.attached[index] = Some((*parent, *node));
                self.world.events.push(Event::Model { slot: index, name });
                0
            }
            (112, [_]) => {
                self.world.events.push(Event::Hush);
                0
            }
            (117, [first, second]) => {
                self.world.events.push(Event::HerParts(*first));
                self.world.events.push(Event::HerParts(*second));
                0
            }
            (121, [first, second]) => {
                for mode in [first, second].into_iter().filter(|mode| **mode >= 0) {
                    self.world.events.push(Event::Parts(*mode));
                }
                0
            }
            // A mouth that moves, or is shut (0xBEE54): it has its first frame here.
            (118, [who, pace, span, moving]) => {
                let mut mouth = Mouth { hers: *who == 0, pace: *pace, span: *span, turned: self.body.mouth.map_or(0, |mouth| mouth.turned) };
                if let Some(angle) = mouth.frame(self.world.random) {
                    self.world.events.push(Event::Jaw { hers: mouth.hers, angle });
                }
                self.body.mouth = (*moving > 0).then_some(mouth);
                0
            }
            // Back to where it belongs (0xBF148): turned no way, placed plainly at
            // home; and every bullet goes.
            (122, [_]) => {
                let body = &mut *self.body;
                body.mover.set_down(body.home);
                body.place_moved();
                self.world.events.push(Event::NoBullets);
                0
            }
            (125, [time]) => {
                self.world.events.push(Event::MusicOff(*time));
                0
            }
            // What is behind is dimmed, or is not (0xBE150).
            (109, [to, time]) => {
                self.world.events.push(Event::Dim { to: *to, time: *time });
                0
            }
            // A node of one of its models throws a round shadow (0xBDA28): its
            // radius, and its strength of 255.
            (51, [slot, node, radius, strength]) => {
                if let (Some(slot), Ok(node)) = (usize::try_from(*slot).ok().filter(|slot| *slot < SLOTS), u8::try_from(*node)) {
                    self.world.events.push(Event::Shadow { slot, node, radius: *radius as f32, strength: *strength as f32 / 255.0 });
                }
                0
            }
            // Every effect of its own is told to end (0xBF1EC: 0xA77A4 and 0x9E7E0
            // on its own pool of them): each script does as its enemy is beaten.
            (123, []) => {
                self.world.events.push(Event::StopEffects);
                0
            }
            // Functions 54 and 57 do nothing in the original either; 82 with a way
            // other than its two does nothing.
            (54 | 57 | 82, _) => 0,
            _ => {
                self.unported(table, number, arguments);
                0
            }
        };
        Ok(Reply::value(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_boss_has_the_finish_that_is_its_own() {
        // The six that are met first, in the order of their stages, and each met
        // again twice over.
        assert_eq!([116, 117, 118, 119, 120, 121].map(|enemy| finish_record(enemy, 0)), [1, 4, 7, 10, 13, 16]);
        assert_eq!([98, 99, 100, 101, 108, 109].map(|enemy| finish_record(enemy, 0)), [2, 3, 5, 6, 17, 18]);
        // The last, and the one met again in its place, by how many there have been.
        assert_eq!([0, 1, 2, 3].map(|finishes| finish_record(123, finishes)), [19, 20, 21, 19]);
        assert_eq!([0, 1, 2, 3].map(|finishes| finish_record(111, finishes)), [22, 23, 24, 22]);
        // Anybody else has the first.
        assert_eq!([110, 122, 44].map(|enemy| finish_record(enemy, 1)), [1, 1, 1]);
    }

    #[test]
    fn a_boss_of_her_build_shows_its_hands_as_she_does() {
        use crate::player::part::*;
        // As its script first has it: the cannon of its own model in its right
        // hand, and an empty left hand.
        assert_eq!(part_mode(5), [(RIGHT_HAND_CANNON, true), (RIGHT_HAND_OTHER, false), (RIGHT_HAND_THIRD, false), (CANNON, true)]);
        assert_eq!(part_mode(4), [(LEFT_HAND_BLADE, false), (LEFT_HAND_EMPTY, true), (BLADE, false)]);
        // Another shape of the right hand lets that cannon go; the blade in the left.
        assert_eq!(part_mode(1)[3], (CANNON, false));
        assert_eq!(part_mode(6), [(LEFT_HAND_BLADE, true), (LEFT_HAND_EMPTY, false), (BLADE, true)]);
        // And a part of its own, hidden and shown.
        assert_eq!((part_mode(7), part_mode(8), part_mode(9)), (vec![(0x37, false)], vec![(0x37, true)], vec![]));
    }

    #[test]
    fn a_mouth_that_moves_opens_and_shuts_and_one_with_no_span_is_shut() {
        let mut random = Random::new(3);
        let mut mouth = Mouth { hers: false, pace: 5, span: 9, turned: 0 };
        let angles: Vec<f32> = (0..200).map(|_| mouth.frame(&mut random).unwrap()).collect();
        // It goes round within its span of 180 degrees, so the jaw is only ever
        // turned one way, by up to a sixth of a radian.
        assert!(angles.iter().all(|angle| (-1.0 / 6.0..=0.0).contains(angle)));
        assert!(angles.iter().any(|angle| *angle < -0.15) && angles.iter().any(|angle| *angle > -0.02));
        // With neither span nor pace it is shut; with a pace alone it is left be.
        assert_eq!(Mouth { hers: true, pace: 0, span: 0, turned: 40 }.frame(&mut random), Some(0.0));
        assert_eq!(Mouth { hers: true, pace: 5, span: 0, turned: 40 }.frame(&mut random), None);
    }

    #[test]
    fn a_gauge_breaks_only_under_one_blow_as_strong_as_it_is() {
        let mut random = Random::new(5);
        let mut gauge = Gauge::new([300, 1, 480], &mut random);
        assert!((262.5..=300.0).contains(&gauge.value));
        // Normal shots have no break power.
        assert!(!gauge.wear(0));
        // A blow that does not break it is shrugged off within a frame.
        assert!(!gauge.wear(200));
        gauge.update(2.0);
        assert_eq!(gauge.value, gauge.max);
        assert!(gauge.wear(999));
        assert!(gauge.broken);
        // It stays broken for its 480 sixtieths, then comes back stronger.
        for _ in 0..239 {
            gauge.update(2.0);
        }
        assert!(gauge.broken);
        gauge.update(2.0);
        assert!(!gauge.broken);
        assert_eq!(gauge.value, 600.0);
    }

    #[test]
    fn a_hit_is_seen_as_the_effect_of_its_attacks_look() {
        // Her own shots have the fifth look (the attack table's record 5).
        assert_eq!(seen_of_hit(5, false, false), vec![(999_033, true)]);
        // Guarded, the guard's own effect comes first, under what is drawn over
        // everything, and the look is another's.
        assert_eq!(seen_of_hit(5, true, false), vec![(EFFECT_GUARDED, false), (998_019, true)]);
        // A guarded hit of the first look has nothing but the guard's.
        assert_eq!(seen_of_hit(1, true, false), vec![(EFFECT_GUARDED, false)]);
        // An attack that does more to an enemy that is broken bursts first.
        assert_eq!(seen_of_hit(6, false, true), vec![(EFFECT_BROKEN_MORE, true), (998_046, true)]);
        // No look, or one the game has no effect for, shows nothing.
        assert_eq!(seen_of_hit(0, false, false), vec![]);
        assert_eq!(seen_of_hit(11, false, false), vec![]);
        assert_eq!(seen_of_hit(-1, false, false), vec![]);
    }

    /// An enemy worth 25 experience that leaves item 1, or item 2 for its rare one,
    /// with these chances out of 255.
    fn worth(chance: i32, rare_chance: i32) -> Variant {
        use crate::battle::tables::Drop;
        Variant { experience: 25, drop: Drop { chance, item: 1 }, rare: Drop { chance: rare_chance, item: 2 }, ..Variant::default() }
    }

    #[test]
    fn an_enemy_beaten_is_worth_its_experience_unless_it_was_called_in() {
        let mut random = Random::new(7);
        let mut spoils = Spoils::default();
        spoils.take(&worth(0, 0), true, &mut random);
        spoils.take(&worth(0, 0), false, &mut random);
        assert_eq!((spoils.beaten, spoils.experience), (2, 25));
        // In some missions experience counts for half, cut to a whole number.
        let mut halved = Spoils { halved: true, ..Spoils::default() };
        halved.take(&worth(0, 0), true, &mut random);
        assert_eq!(halved.experience, 12);
        assert_eq!(spoils.items().count() + halved.items().count(), 0);
    }

    #[test]
    fn what_an_enemy_leaves_goes_by_two_rolls() {
        let mut random = Random::new(7);
        let mut spoils = Spoils::default();
        // A chance of 255 always comes up, one of 0 never.
        for _ in 0..3 {
            spoils.take(&worth(255, 0), true, &mut random);
        }
        spoils.take(&worth(255, 255), true, &mut random);
        spoils.take(&worth(0, 255), true, &mut random);
        // One called in leaves nothing, whatever its chances.
        spoils.take(&worth(255, 255), false, &mut random);
        assert_eq!(spoils.items().collect::<Vec<_>>(), vec![(1, 3), (2, 1)]);
        // An enemy with no item to leave leaves none.
        let mut bare = worth(255, 0);
        bare.drop.item = 0;
        spoils.take(&bare, true, &mut random);
        assert_eq!(spoils.items().count(), 2);
    }

    #[test]
    fn a_fifth_kind_of_item_is_lost() {
        let mut random = Random::new(7);
        let mut spoils = Spoils::default();
        for item in [10, 11, 12, 13, 14, 12] {
            let mut variant = worth(255, 0);
            variant.drop.item = item;
            spoils.take(&variant, true, &mut random);
        }
        assert_eq!(spoils.items, [(10, 1), (11, 1), (12, 2), (13, 1)]);
    }

    #[test]
    fn the_ais_comparisons_come_in_groups_of_eight() {
        let variables = [5, 6, 7, 8, 9, 10, 11, 12];
        // Equal, for either set of variables.
        assert!(compare_variable(33, 5, &variables));
        assert!(!compare_variable(34, 5, &variables));
        assert!(compare_variable(48, 12, &variables));
        // The variable is at most the number.
        assert!(compare_variable(49, 5, &variables));
        assert!(compare_variable(49, 9, &variables));
        assert!(!compare_variable(50, 5, &variables));
        // The variable is at least the number.
        assert!(compare_variable(65, 5, &variables));
        assert!(compare_variable(66, 5, &variables));
        assert!(!compare_variable(65, 6, &variables));
        assert!(compare_variable(80, 12, &variables));
        // Numbers outside the six groups are never true.
        assert!(!compare_variable(32, 5, &variables));
        assert!(!compare_variable(81, 5, &variables));
    }
}
