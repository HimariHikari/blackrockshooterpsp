//! Her skills: the battle system's table of them (`btl_skl_tbl.bin`, read at
//! 0x12AB9C) and the names the menus and the battle show them by
//! (`db_resident.lpk/003.xtc`, 0x12CE7C). See "Skills" in `docs/BATTLE_NOTES.md`.
//!
//! There are four sorts. Those of kind 1 are attacks and those of kind 2 help her
//! for a while: both are put in one of four slots and used in a battle with R and
//! the slot's button, and take time to be ready again. Those of kind 4 act of
//! themselves when something happens, and those of kind 3 add to one of her
//! figures for as long as she has them.
//!
//! Which she has is kept in the save's flags, one for each skill (0x14D95C). A
//! second flag for each (0x14D994) says of one of the first two kinds that it is
//! in a slot, and of one of the others that it is put aside and adds nothing
//! (0x14CFA8).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use bevy::prelude::*;
use brs_engine::animation::Clip;
use brs_engine::camera::CameraClip;
use brs_formats::{bxcb, insm, xtc};

use super::actor::PcActor;
use super::flow::Flow;
use super::tables::{PlayerStats, Shot, Shots};
use super::SIXTIETHS_PER_STEP;
use crate::controls::button;
use crate::data::GameData;
use crate::progress::Progress;
use crate::sound::{Board, COMMON_BANK, FULL_VOLUME};
use crate::Mode;

const TABLE: &str = "SYSTEM.VOL/btl_skl_tbl.bin";
const NAMES: &str = "SYSTEM.VOL/db_resident.lpk/003.xtc";
/// Where the skills' archives are.
const ARCHIVES: &str = "BTL/SKL";

/// The flag that says she has a skill is at its number from this, and the one
/// that says it is in a slot, or put aside, from the other.
pub const HAS: i32 = 7000;
pub const SET: i32 = 7200;
/// The flags have room for so many skills (0x13D0).
const MOST: i32 = 128;

/// The kind of a skill that attacks (0x12B22C), of one that helps her for a while,
/// of one that adds to one of her figures (0x12B1D8, 0x12AEC8), and of one that
/// acts of itself (0x9F824).
const KIND_ATTACK: i32 = 1;
const KIND_HELP: i32 = 2;
const KIND_BONUS: i32 = 3;
const KIND_AUTO: i32 = 4;
/// What the menus sort a skill under (0x12CDBC): one that is put in a slot, or
/// one that is "always in force", which are the kinds that are not.
pub const OF_A_SLOT: u8 = 1;
pub const ALWAYS: u8 = 2;

/// One of her figures that a skill can add to, as the table numbers them
/// (0x12AED4, 0x12AF90, 0x12B04C, 0x12B104).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Figure {
    Hp = 1,
    Attack = 2,
    Special = 3,
    Defence = 4,
}

impl Figure {
    pub const ALL: [Figure; 4] = [Figure::Hp, Figure::Attack, Figure::Special, Figure::Defence];

    /// The most a skill can add to it (0x12AEEC, 0x12AFA8).
    fn most(self) -> i32 {
        if self == Figure::Hp { 9999 } else { 999 }
    }
}

/// A record of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub id: i32,
    pub kind: i32,
    /// The sixtieths of a second it takes to be ready the first time in a battle,
    /// and again after it has been used (0x12AD74, 0x12AD7C).
    pub first: i32,
    pub recharge: i32,
    /// The archive of what it is played with.
    pub archive: String,
    /// The line of the battle's messages that names it.
    pub message: i32,
    /// The status effects one of kind 2 gives her (0x12B1D4).
    pub effects: [i32; 3],
    /// The attacks it does: one for those that shoot, one for the others.
    pub attacks: [i32; 2],
    /// What one of kind 4 goes by (0x12ADB8 to 0x12AE38, and one more): when it
    /// acts, on what condition and with what figure for it, what it does, and
    /// with what two figures. See `auto.rs`.
    pub auto: [i32; 6],
    /// What one of kind 3 adds: to which of her figures (by its number), how
    /// much, and how many hundredths of the figure itself besides.
    bonus: (i32, i32, i32),
    pub name: String,
    pub description: String,
    /// What the menus sort it under.
    category: u8,
}

impl Skill {
    const FIELDS: usize = 20;

    /// What it adds to one of her figures (0x12AEC0 and the three after it).
    /// Nothing in the game's table asks for a share of the figure itself, which
    /// the original would work out from the figure before anything is added.
    pub fn adds(&self, figure: Figure) -> i32 {
        if self.kind != KIND_BONUS || self.bonus.0 != figure as i32 {
            return 0;
        }
        self.bonus.1.clamp(-figure.most(), figure.most())
    }

    /// Whether it is one that is in force for as long as she has it.
    pub fn always(&self) -> bool {
        self.category == ALWAYS
    }

    /// The line of the battle's log that names it as it is used (0x12B1AC): one
    /// of a slot has one.
    pub fn line(&self) -> i32 {
        if matches!(self.kind, KIND_ATTACK | KIND_HELP) { self.message } else { 0 }
    }

    /// The folder its archive is unpacked to.
    fn folder(&self) -> String {
        format!("{ARCHIVES}/{}", self.archive.to_ascii_uppercase())
    }

    /// A skill of a slot, for tests: its number and kind, the sixtieths it takes
    /// to be ready again, its line of the log and what it gives her.
    #[cfg(test)]
    pub fn of_a_slot(id: i32, kind: i32, recharge: i32, message: i32, effects: [i32; 3]) -> Skill {
        Skill {
            id,
            kind,
            first: 0,
            recharge,
            archive: String::new(),
            message,
            effects,
            attacks: [0; 2],
            auto: [0; 6],
            bonus: (0, 0, 0),
            name: format!("Skill {id}"),
            description: String::new(),
            category: 1,
        }
    }

    /// One that acts of itself, for tests: its number and the six figures of its
    /// record.
    #[cfg(test)]
    pub fn of_itself(id: i32, auto: [i32; 6]) -> Skill {
        Skill { auto, category: ALWAYS, ..Skill::of_a_slot(id, KIND_AUTO, 0, 0, [0; 3]) }
    }
}

/// How a skill fires the bullet its record names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Firing {
    /// If it is one that flies straight (0x766B4).
    Straight,
    /// If it is a guided one, and then, in the same frame, each bullet that its
    /// record has fired along with it, and so on down the chain (0x7ABB8).
    Guided,
    /// If it is a bomb, which is thrown (0x797A8).
    Lobbed,
}

/// Where a scene has her model stand in place of where she is: her own update
/// leaves the model alone while she has the stage (her + 0x7FC), so it stays where
/// the scene puts it. Turned half round in both, which the animations of such a
/// scene are made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placing {
    /// At the middle of the map (0x7720C).
    Middle,
    /// Where what she had locked on to stands, on the ground (0x7782C).
    Target,
}

/// What a scene puts an effect it makes by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// Nothing: it is where the world begins, the middle of the stage. (Those
    /// that are made so are fixed to the screen, all but one that is for the
    /// enemies there.)
    Nowhere,
    /// A node of her model, or her model whole, which it goes with (0x74978).
    With(Option<u8>),
    /// Her model, where that is as the effect is made: it is left there
    /// (0x748F8).
    Left,
}

/// An effect a scene makes: one of the battle's own (0x7488C; each skill's code
/// has a copy of its own of this and of what puts it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Made {
    pub id: u32,
    pub hold: Hold,
    /// From what it is put by it is moved so far, in its own frame (0x74A0C),
    /// and then turned by so many degrees: about the upright axis first, then
    /// the one across, then the one along (0x74A90, 0x240C0).
    pub moved: Vec3,
    pub turned: Vec3,
    /// The draw list of the scene it is of, by its number (0x749E0).
    pub list: i32,
}

impl Made {
    pub(super) const fn new(id: u32, hold: Hold, list: i32) -> Made {
        Made { id, hold, moved: Vec3::ZERO, turned: Vec3::ZERO, list }
    }

    const fn moved(self, x: f32, y: f32, z: f32) -> Made {
        Made { moved: Vec3::new(x, y, z), ..self }
    }

    pub(super) const fn turned(self, x: f32, y: f32, z: f32) -> Made {
        Made { turned: Vec3::new(x, y, z), ..self }
    }

    /// Where it is from what it is put by.
    pub fn matrix(&self) -> Mat4 {
        let turned = self.turned * std::f32::consts::PI / 180.0;
        Mat4::from_translation(self.moved) * Mat4::from_euler(EulerRot::YXZ, turned.y, turned.x, turned.z)
    }
}

/// One thing the state of a skill does once its clock has come to the time it is
/// down for: the calls of the state's update, in their order (0x81E08 for
/// Defender, 0x7699C for Charge Shot; `tools_re/skill_states.py` and
/// `skill_unit.py` list them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// She shows other shapes of her hands, by one of the modes her model has
    /// for it (0x9B608), which may also empty her hand or put her cannon back.
    Parts(u8),
    /// The weapon of the skill's archive is hers to hold, fixed to this node of
    /// her model (0x7AB48 and its twins, each with the node of its own).
    Weapon(u8),
    /// The log's line that names the skill (0x80670).
    Say,
    /// She takes the stage (0x80784), and gives it back (0x80BE4).
    TakeStage,
    GiveStageBack,
    /// Her model stands somewhere else than she does, until the stage is given
    /// back (0x7720C, 0x7782C).
    Put(Placing),
    /// What is behind her is dimmed (0x8073C), and is as it was again.
    Dim,
    Undim,
    /// It is dimmed that much at once, and it is all black at once: for the
    /// scenes that are nearly all effects.
    DimAtOnce,
    Blacken,
    /// The camera's move of the skill starts (0x80874), and is over (0x808B4).
    Camera,
    CameraBack,
    /// The camera sees from so near and so far from here on (0x808E0).
    Lens(f32, f32),
    /// Her animation of the skill starts (0x80960).
    Animation,
    /// The second move of the camera and the second animation of hers start, of a
    /// scene that has two.
    SecondCamera,
    SecondAnimation,
    /// Her animation goes round the part of it that repeats (0x7BB8C), and plays
    /// on from where it is to its end (0x7BBC0).
    LoopAnimation,
    PlayOut,
    /// A line of hers, of the skill's archive (0x80DD8).
    Voice(&'static str),
    /// A sound of the battle's bank (0x80D58), and one of the skill's own
    /// (0x80D8C).
    Sound(u32),
    OwnSound(u32),
    /// She turns her body to where she aims, or stops (0x76208, 0x7623C).
    Aim(bool),
    /// The skill's bullet leaves.
    Fire(Firing),
    /// The cursor is no longer drawn to a lock point it is not on already, so
    /// that it can be swept over the enemies (0xA15E4), or is again (0xA15F8).
    CursorFree(bool),
    /// A volley of rays (0x7C7D0): while any of the skill's bullet's count are
    /// left she turns to where she aims now and one leaves (0x7C158), and the
    /// state's clock starts over for the wait that follows; with none left that
    /// wait is passed over.
    Volley,
    /// The wait between two rays is over: one fewer is left, and it is the
    /// volley's turn again at once (0x7C81C).
    Reload,
    /// The state's clock starts over (0x744A4): what follows is timed from here.
    Restart,
    /// Her model is not drawn, or is again (her + 0x73C: 0x7F9B0, 0x7F9DC).
    Hide(bool),
    /// The scope of a snipe comes (0x7F86C): the display's sight in place of
    /// the cursor, which is held where it is, and the camera that closes in on
    /// what the skill is for. Or it goes (0x7F964).
    Scope(bool),
    /// Once the clock has come to when the sight starts to lock on: it does, if
    /// what the skill is for has a body to strike; otherwise the log says that
    /// the lock has failed (0x7FF1C).
    SightLock,
    /// The wait for the sight to have come, and with something to strike for it
    /// to have locked on; then the clock starts over (0x7FF80).
    SightWait,
    /// The window to fire (0x7FFF0): square fires, once, which has the skill's
    /// slot start over and what the skill is for struck, if it can be; the
    /// window closes two seconds on, or as cross is pressed.
    Trigger,
    /// The skill gives her its status effects (0x80E14).
    Give,
    /// Every other skill of her slots is ready at once (0x82BE0).
    ReadyOthers,
    /// The enemies are to be seen again, held as they still are (0x746D8).
    ShowEnemies,
    /// Every enemy takes her attack of the skill's record, once (0x74EF0).
    Strike,
    /// What she had locked on to as the skill began is to be seen again, and it
    /// alone (0x778C0); and it takes her attack of the skill's record, once
    /// (0x778F8).
    ShowTarget,
    StrikeTarget,
    /// She stands as she does between actions (0x80B58), and can act again
    /// (0x80CCC).
    Stand,
    Unlock,
    /// Her animation is put back to its start and goes round, for the standing
    /// that follows to take over from at once (0x74B78).
    StandAtOnce,
    /// The scene makes an effect (0x7488C).
    Effect(Made),
    /// Every effect of a number that plays is no more, at once (0x74B5C).
    EndEffects(u32),
    /// The effects of a number are of another draw list from here on (0x749E0,
    /// on one the state has kept).
    EffectList(u32, i32),
}

/// What the state of a skill does: she takes the stage with a move of the camera
/// and an animation, things are heard at their times, and the skill does what it
/// does. One that helps her does it as the scene ends; one that shoots gives the
/// stage back first and fires over the battle as it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Her animation and the camera's move: `chr_p_brs02_<this>_all.anm` and
    /// `_cam.cam` of the skill's archive.
    pub clip: &'static str,
    /// The same of its second part, for a scene that has two.
    pub second: Option<&'static str>,
    /// It is for what she has locked on to, and does not begin without (0x9A728).
    pub aimed: bool,
    /// It takes what she has locked on to, if anything, and its slot starts over
    /// only as it is fired (0x9A900).
    pub scoped: bool,
    /// What it does and when, in order: in sixtieths of a second on the state's
    /// clock, which runs from the scene's start until a step has it start over.
    pub steps: Vec<(f32, Step)>,
    /// A boost she has is spent as its state ends (0x80D2C): by all but the two
    /// that give that boost.
    pub spends_boost: bool,
    /// It is one of those that shoot, which her action is numbered apart for (7
    /// and not 6, 0x9A6C4): an enemy's script can ask whether she is at one.
    pub shoots: bool,
    /// The effects that are no more as its state ends, however it does (0x767C8
    /// for Charge Shot): every one that plays of each number.
    pub ends: Vec<u32>,
}

impl Scene {
    /// The node of hers the weapon it has her take is fixed to, if she takes one.
    pub fn weapon_node(&self) -> Option<u8> {
        self.steps.iter().find_map(|(_, step)| if let Step::Weapon(node) = step { Some(*node) } else { None })
    }
}

#[cfg(test)]
impl Scene {
    pub fn has(&self, step: Step) -> bool {
        self.steps.iter().any(|(_, other)| *other == step)
    }

    /// Whether it fires a bullet.
    pub fn fires(&self) -> bool {
        self.steps.iter().any(|(_, step)| matches!(step, Step::Fire(_)))
    }

    /// When it is over.
    pub fn end(&self) -> f32 {
        self.steps.last().map_or(0.0, |step| step.0)
    }
}

/// The nodes of her model that the weapon of a skill is fixed to: a gun where her
/// cannon is (0x7AB48), a blade at one of its own (0x777BC, 0x74E80), and a sword
/// or a hammer at another (0x84750).
pub const GUN_NODE: u8 = 0x50;
pub const BLADE_NODE: u8 = 0x0B;
pub const SWORD_NODE: u8 = 0x10;

/// The sound a skill that has the others ready is heard with (0x82BC4), of the
/// battle's bank.
pub const SOUND_OTHERS_READY: u32 = 130;
/// How far the camera sees in the scene of a skill, whichever it is.
const LENS_FAR: f32 = 20000.0;

/// The scene of a skill: of those that help her, and of the attacks that are
/// ported.
pub fn scene(skill: i32) -> Option<Scene> {
    use Step::*;
    // The effects a scene makes: with a node of hers or with her whole, left where
    // she is, or put nowhere; of a draw list each. One that goes with the weapon
    // of a skill is turned as its node is not (0x7AF8C), and of the last list.
    let on = |id: u32, node: u8, list: i32| Effect(Made::new(id, Hold::With(Some(node)), list));
    let on_her = |id: u32, list: i32| Effect(Made::new(id, Hold::With(None), list));
    let left = |id: u32, list: i32| Effect(Made::new(id, Hold::Left, list));
    let nowhere = |id: u32, list: i32| Effect(Made::new(id, Hold::Nowhere, list));
    let on_weapon = |id: u32, node: u8| Effect(Made::new(id, Hold::With(Some(node)), 4).turned(90.0, 0.0, 90.0));
    // One that helps her: the stage, a move of the camera that comes so near, what
    // is heard, and at its end what it does and everything given back at once.
    let helping = |clip: &'static str, near: f32, dimmed: bool, heard: &[(f32, Step)], end: f32, does: &[Step]| {
        let mut steps = vec![(0.0, Say), (0.0, TakeStage)];
        if dimmed {
            steps.push((0.0, Dim));
        }
        steps.extend([(0.0, Camera), (0.0, Lens(near, LENS_FAR)), (0.0, Animation)]);
        steps.extend_from_slice(heard);
        steps.extend(does.iter().map(|&step| (end, step)));
        if dimmed {
            steps.push((end, Undim));
        }
        steps.extend([(end, CameraBack), (end, GiveStageBack), (end, Stand), (end, Unlock)]);
        Scene { clip, second: None, aimed: false, scoped: false, steps, spends_boost: true, shoots: false, ends: Vec::new() }
    };
    // Aggressor, Defender and Absolute 0, and the better of each.
    let raised = |line: &'static str| {
        helping(
            "btl4020",
            50.0,
            true,
            &[(64.0, Voice(line)), (72.0, on(10120, 60, 4)), (74.0, Sound(10)), (76.0, Lens(45.0, LENS_FAR)), (76.0, left(10121, 4))],
            160.0,
            &[Give, EndEffects(10120), EndEffects(10121)],
        )
    };
    // What Skill Boost and Regenerate have, and what Heat-Free and its like end by.
    let renewed: [Step; 3] = [Give, EndEffects(10170), EndEffects(10171)];
    let freed: [Step; 3] = [EndEffects(10180), EndEffects(10181), EndEffects(10182)];
    Some(match skill {
        23 => raised("brs_2100"),
        25 => raised("brs_2400"),
        26 => raised("brs_1400"),
        66 => raised("brs_2300"),
        68 => raised("brs_2500"),
        69 => raised("brs_1900"),
        // Skill Boost and Regenerate.
        24 => Scene {
            spends_boost: false,
            ..helping("btl4007", 35.0, false, &[(0.0, on(10170, 60, 4)), (70.0, Voice("brs_1001")), (72.0, left(10171, 3)), (80.0, Sound(55))], 160.0, &renewed)
        },
        67 => Scene {
            spends_boost: false,
            ..helping("btl4007", 35.0, false, &[(0.0, on(10170, 60, 4)), (72.0, Voice("brs_2000")), (72.0, left(10171, 3)), (98.0, Sound(55))], 160.0, &renewed)
        },
        27 => helping("btl4007", 35.0, true, &[(0.0, on(10170, 60, 4)), (72.0, left(10171, 3)), (72.0, Voice("brs_1500")), (98.0, Sound(55))], 160.0, &renewed),
        70 => helping("btl4007", 35.0, true, &[(0.0, on(10170, 60, 4)), (72.0, left(10171, 3)), (72.0, Voice("brs_1600")), (80.0, Sound(55))], 160.0, &renewed),
        // Heat-Free, Overlimit and Ignition.
        28 => helping(
            "btl4008",
            20.0,
            true,
            &[
                (0.0, nowhere(10182, 4)),
                (20.0, Voice("brs_5200")),
                (50.0, Sound(57)),
                (218.0, Sound(99)),
                (222.0, left(10180, 4)),
                (240.0, left(10181, 3)),
                (240.0, Voice("brs_5000")),
                (250.0, Sound(151)),
            ],
            372.0,
            &[Give, freed[0], freed[1], freed[2]],
        ),
        71 => helping(
            "btl4008",
            20.0,
            true,
            &[
                (0.0, nowhere(10182, 4)),
                (20.0, Voice("brs_5200")),
                (50.0, Sound(57)),
                (218.0, Sound(99)),
                (222.0, left(10180, 4)),
                (240.0, left(10181, 3)),
                (240.0, Voice("brs_4900")),
                (250.0, Sound(163)),
                (270.0, Sound(151)),
            ],
            372.0,
            &[Give, freed[0], freed[1], freed[2]],
        ),
        29 => helping(
            "btl4008",
            20.0,
            true,
            &[
                (0.0, nowhere(10182, 4)),
                (50.0, Sound(57)),
                (90.0, Voice("brs_5101")),
                (218.0, Sound(99)),
                (222.0, left(10180, 4)),
                (240.0, left(10181, 3)),
                (250.0, Voice("brs_2200")),
                (250.0, OwnSound(0)),
            ],
            372.0,
            // (What is seen of it is what a gift of HP is seen by, 0x82B88.)
            &[freed[0], freed[1], freed[2], on_her(998042, 3), ReadyOthers],
        ),
        // Charge Shot (0x7699C): two and a half seconds of her making ready, then
        // the stage is the battle's again and the shot leaves over it, with what
        // is behind her still dimmed for a moment.
        1 => Scene {
            clip: "btl4000",
            second: None,
            aimed: false,
            scoped: false,
            steps: vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Parts(4)),
                (0.0, Parts(5)),
                (0.0, Animation),
                (30.0, Sound(144)),
                (34.0, Voice("brs_1003")),
                (46.0, Lens(20.0, LENS_FAR)),
                (68.0, on(10001, 50, 4)),
                (76.0, Sound(143)),
                (136.0, Voice("brs_2600")),
                (146.0, Sound(176)),
                (150.0, CameraBack),
                (150.0, GiveStageBack),
                (150.0, Aim(true)),
                (150.0, Fire(Firing::Straight)),
                (150.0, on_her(10003, 3)),
                (150.0, EffectList(10001, 3)),
                (170.0, Undim),
                (236.0, Aim(false)),
                (244.0, Stand),
                (244.0, Unlock),
            ],
            spends_boost: true,
            shoots: true,
            ends: vec![10001, 10003],
        },
        // Burst Shot (0x76D84), which is the same with other times.
        2 => Scene {
            clip: "btl4022",
            second: None,
            aimed: false,
            scoped: false,
            steps: vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Parts(4)),
                (0.0, Parts(5)),
                (0.0, Animation),
                (0.0, nowhere(10011, 4)),
                (30.0, Voice("brs_1003")),
                (30.0, Sound(144)),
                (86.0, Sound(143)),
                (88.0, Effect(Made::new(10012, Hold::With(Some(50)), 4).moved(0.0, 2.0, 15.0))),
                (160.0, Sound(176)),
                (160.0, Voice("brs_4900")),
                (168.0, CameraBack),
                (168.0, GiveStageBack),
                (168.0, Aim(true)),
                (168.0, Fire(Firing::Straight)),
                (168.0, on(10013, 50, 3)),
                (180.0, Undim),
                (238.0, Aim(false)),
                (252.0, Stand),
                (252.0, Unlock),
            ],
            spends_boost: true,
            shoots: true,
            ends: vec![10011, 10012, 10013],
        },
        // Photon Shower (0x75284): a long scene that is nearly all effects, with
        // what is behind her gone black. Near its end the enemies are seen again,
        // and every one of them is struck. Its line is not written to the log.
        // The sky of the scene is an effect of the scene's first draw list, which
        // is all there is behind her while the sheet is black.
        3 => Scene {
            clip: "btl4010",
            second: None,
            aimed: false,
            scoped: false,
            steps: vec![
                (0.0, Parts(3)),
                (0.0, Parts(5)),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Lens(50.0, 35000.0)),
                (0.0, Animation),
                (0.0, on(10160, 50, 4)),
                (0.0, nowhere(10161, 0)),
                (0.0, Sound(105)),
                (20.0, Blacken),
                (40.0, Voice("brs_0310")),
                (44.0, Lens(1.0, LENS_FAR)),
                (48.0, Sound(108)),
                (80.0, Sound(108)),
                (144.0, Voice("brs_1007")),
                (190.0, Sound(143)),
                (230.0, Voice("brs_3600")),
                // (Its wait here is for 22, long past.)
                (230.0, Sound(104)),
                (240.0, Sound(164)),
                (240.0, Sound(163)),
                (298.0, Lens(200.0, LENS_FAR)),
                (298.0, DimAtOnce),
                (298.0, EndEffects(10160)),
                (298.0, EndEffects(10161)),
                (298.0, ShowEnemies),
                (298.0, nowhere(10163, 4)),
                (340.0, Effect(Made::new(10162, Hold::Left, 3).moved(-600.0, 1500.0, -570.0).turned(50.0, 0.0, 0.0))),
                (344.0, Sound(163)),
                (344.0, Sound(164)),
                (346.0, Effect(Made::new(10164, Hold::Left, 3).moved(-600.0, 1.0, 700.0))),
                (354.0, Sound(151)),
                (420.0, Sound(163)),
                (460.0, Strike),
                (466.0, Sound(150)),
                (470.0, Sound(181)),
                (520.0, Undim),
                (520.0, EndEffects(10162)),
                (520.0, EndEffects(10163)),
                (520.0, EndEffects(10164)),
                (520.0, CameraBack),
                (520.0, GiveStageBack),
                (520.0, StandAtOnce),
                (520.0, Unlock),
            ],
            spends_boost: true,
            shoots: false,
            ends: vec![10160, 10161, 10162, 10163, 10164],
        },
        // BB Genocide (0x759D0): the same with a blade of the skill's, which she has
        // no more from three seconds in.
        6 => Scene {
            clip: "btl4009",
            second: None,
            aimed: false,
            scoped: false,
            steps: vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, DimAtOnce),
                (0.0, Lens(50.0, LENS_FAR)),
                (0.0, Camera),
                (0.0, Animation),
                (0.0, Parts(0)),
                (0.0, Parts(3)),
                (0.0, Weapon(BLADE_NODE)),
                (0.0, left(10150, 3)),
                (0.0, on(10151, BLADE_NODE, 4)),
                (0.0, on_her(10152, 3)),
                (0.0, nowhere(10153, 3)),
                (50.0, Voice("brs_1000")),
                (76.0, Sound(164)),
                (170.0, Lens(1.0, LENS_FAR)),
                (170.0, EffectList(10151, 3)),
                (170.0, Sound(56)),
                (170.0, Voice("brs_1008")),
                (180.0, Parts(2)),
                (260.0, Voice("brs_2900")),
                (266.0, Lens(200.0, LENS_FAR)),
                (266.0, ShowEnemies),
                (270.0, Sound(181)),
                (296.0, Sound(151)),
                (296.0, Strike),
                (400.0, Undim),
                (400.0, EndEffects(10150)),
                (400.0, EndEffects(10151)),
                (400.0, EndEffects(10152)),
                (400.0, EndEffects(10153)),
                (400.0, GiveStageBack),
                (400.0, CameraBack),
                (400.0, Parts(5)),
                (400.0, Parts(3)),
                (400.0, Stand),
                (400.0, Unlock),
            ],
            spends_boost: true,
            shoots: false,
            ends: Vec::new(),
        },
        // Steel Cutter and War Hammer (0x84B40, 0x85004): a sword or a hammer of
        // the skill's, and one blow that lands on every enemy. (After the stage is
        // given back the state's clock starts over and it waits for twenty more.)
        20 | 22 => {
            let cutter = skill == 20;
            // On the weapon and about her from the start; the blow's own, put
            // nowhere; and one that goes before her, 400 on.
            let first = if cutter { 10131 } else { 10191 };
            let mut steps = vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Animation),
                (0.0, Parts(3)),
                (0.0, Parts(0)),
                (0.0, Weapon(SWORD_NODE)),
                (0.0, on_weapon(first + 3, SWORD_NODE)),
                (0.0, on_her(first, 4)),
                (50.0, Sound(132)),
            ];
            if cutter {
                steps.extend([(102.0, Voice("brs_0020")), (104.0, Sound(144))]);
            } else {
                steps.push((110.0, Voice("brs_0020")));
            }
            steps.extend([
                (122.0, Sound(164)),
                (122.0, Sound(144)),
                (164.0, ShowEnemies),
                if cutter { (190.0, Voice("brs_4600")) } else { (180.0, Voice("brs_3500")) },
                (192.0, nowhere(first + 1, 4)),
                (196.0, Effect(Made::new(first + 2, Hold::With(None), 3).moved(0.0, 0.0, 400.0))),
                (198.0, Sound(if cutter { 185 } else { 105 })),
                (200.0, Strike),
                (300.0, CameraBack),
                (300.0, GiveStageBack),
                (300.0, Parts(3)),
                (300.0, Parts(5)),
                (300.0, Undim),
                (320.0, Stand),
                (320.0, Unlock),
            ]);
            Scene { clip: "btl4006", second: None, aimed: false, scoped: false, steps, spends_boost: true, shoots: false, ends: Vec::new() }
        }
        // Blade Kill and Iksa Blade (0x77D44, 0x781B0), which are for the one enemy
        // she has locked on to. She draws a blade of the skill's at the middle of
        // the map, with nobody else to be seen; then she is where that enemy
        // stands, it is seen again, and a second animation and move of the camera
        // have her cut it down. (The state's clock starts over as what is behind
        // her lights up again, and it waits for twenty more.) Blade Kill asks for
        // two effects its archive has not got: 10132, which is Steel Cutter's and
        // so is there to be had when that skill is in a slot too, and 10142,
        // which is nobody's.
        4 | 5 => {
            let kill = skill == 4;
            let first = if kill { 10041 } else { 10051 };
            let mut steps = vec![
                (0.0, TakeStage),
                (0.0, Put(Placing::Middle)),
                (0.0, Say),
                (0.0, Dim),
                (0.0, Parts(1)),
                (0.0, Parts(3)),
                (0.0, Weapon(BLADE_NODE)),
                (0.0, Camera),
                (0.0, Animation),
                (20.0, Voice(if kill { "brs_1004" } else { "brs_0311" })),
                (24.0, Sound(168)),
                (78.0, Sound(105)),
                (142.0, ShowTarget),
                (142.0, SecondCamera),
                (142.0, SecondAnimation),
                (142.0, on_her(first, 3)),
                (142.0, on(first + 2, BLADE_NODE, 3)),
                (142.0, Put(Placing::Target)),
            ];
            if kill {
                steps.extend([(150.0, Voice("brs_5000")), (154.0, Sound(181)), (162.0, nowhere(10132, 3)), (162.0, StrikeTarget), (166.0, on_her(10142, 3))]);
            } else {
                steps.extend([(150.0, Voice("brs_4900")), (150.0, Sound(181)), (160.0, nowhere(10054, 3)), (160.0, StrikeTarget), (166.0, on_her(10052, 3))]);
            }
            steps.extend([(200.0, Undim), (220.0, CameraBack), (220.0, GiveStageBack), (220.0, Parts(5)), (220.0, Parts(3)), (220.0, StandAtOnce), (220.0, Unlock)]);
            let ends = if kill { vec![10041, 10043, 10132, 10142] } else { vec![10051, 10053, 10054, 10052] };
            Scene { clip: "btl2000", second: Some("btl3100"), aimed: true, scoped: false, steps, spends_boost: true, shoots: false, ends }
        }
        // Tracer Gun and Chaser Gun (0x7AEB0, 0x7B374): she takes a gun of the
        // skill's own for it, and what it fires is guided: three bullets, or four,
        // that leave to her sides and come round to what she had locked on to.
        // (Tracer Gun waits for 78 to take the gun only after it has waited for 80
        // to speak, so it does both at 80; Chaser Gun has them the other way.)
        14 | 15 => {
            let first = if skill == 14 { 10090 } else { 10100 };
            let mut steps = vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Lens(20.0, LENS_FAR)),
                (0.0, Animation),
                (0.0, on_weapon(first, GUN_NODE)),
                (0.0, nowhere(first + 1, 3)),
                (32.0, Sound(132)),
            ];
            if skill == 14 {
                steps.extend([(80.0, Voice("brs_1005")), (80.0, Parts(3)), (80.0, Parts(0)), (80.0, Weapon(GUN_NODE))]);
            } else {
                steps.extend([(78.0, Parts(3)), (78.0, Parts(0)), (78.0, Weapon(GUN_NODE)), (80.0, Voice("brs_1005"))]);
            }
            steps.extend([
                (96.0, Sound(144)),
                (96.0, Sound(164)),
                (162.0, CameraBack),
                (162.0, GiveStageBack),
                (162.0, Aim(true)),
                (188.0, Voice(if skill == 14 { "brs_4400" } else { "brs_3700" })),
                (200.0, on(998003, 50, 3)),
                (200.0, Fire(Firing::Guided)),
                (202.0, Sound(176)),
                (250.0, on_weapon(first + 2, GUN_NODE)),
                (250.0, Sound(108)),
                (250.0, Undim),
                (260.0, Parts(3)),
                (260.0, Parts(5)),
                (270.0, Aim(false)),
                (280.0, Stand),
                (280.0, Unlock),
            ]);
            Scene { clip: "btl4004", second: None, aimed: false, scoped: false, steps, spends_boost: true, shoots: true, ends: Vec::new() }
        }
        // Stun Snipe and G-1 Snipe (0x7FC90, 0x80180): she takes a rifle of the
        // skill's, and then, the stage still hers and she herself not drawn, its
        // scope comes over what she had locked on to. The sight locks on, and
        // for two seconds square fires: the one hit of hers that is on the part
        // of the enemy her lock was on. Only then does the skill's slot start
        // over.
        7 | 8 => Scene {
            clip: "btl4001",
            second: None,
            aimed: false,
            scoped: true,
            steps: vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Animation),
                (0.0, Parts(3)),
                (0.0, Parts(0)),
                (0.0, Weapon(GUN_NODE)),
                (0.0, on_weapon(if skill == 7 { 10020 } else { 10030 }, GUN_NODE)),
                (0.0, nowhere(if skill == 7 { 10021 } else { 10031 }, 3)),
                (10.0, Sound(132)),
                (if skill == 7 { 36.0 } else { 42.0 }, Voice("brs_0001")),
                (50.0, Sound(164)),
                (50.0, Sound(144)),
                (102.0, CameraBack),
                (102.0, ShowTarget),
                (102.0, Hide(true)),
                (102.0, Scope(true)),
                (102.0, Restart),
                (0.0, SightLock),
                (0.0, SightWait),
                (0.0, Trigger),
                (0.0, Scope(false)),
                (0.0, GiveStageBack),
                (0.0, Hide(false)),
                (0.0, Parts(3)),
                (0.0, Parts(5)),
                (0.0, Aim(false)),
                (0.0, Undim),
                (0.0, Restart),
                (20.0, Stand),
                (20.0, Unlock),
            ],
            spends_boost: true,
            shoots: false,
            ends: Vec::new(),
        },
        // Star Comet and Star Comet G (0x79A9C, 0x79EDC): she takes a launcher of
        // the skill's, and with the stage the battle's again she throws its bomb
        // towards where she aims. (The clock starts over as what is behind her
        // lights up again, and it waits for twenty more.)
        18 | 19 => {
            let mut steps = vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, Lens(20.0, LENS_FAR)),
                (0.0, Animation),
                (0.0, on_weapon(10110, GUN_NODE)),
                (0.0, nowhere(10111, 3)),
            ];
            if skill == 18 {
                steps.extend([(30.0, Voice("brs_1006")), (38.0, Sound(132))]);
            } else {
                steps.extend([(38.0, Sound(132)), (56.0, Voice("brs_0021"))]);
            }
            steps.extend([
                (92.0, Parts(3)),
                (92.0, Parts(0)),
                (92.0, Weapon(GUN_NODE)),
                (96.0, Sound(164)),
                (96.0, Sound(144)),
                (152.0, CameraBack),
                (152.0, GiveStageBack),
                (180.0, Voice(if skill == 18 { "brs_4300" } else { "brs_3800" })),
                (192.0, on(998002, 50, 3)),
                (192.0, Sound(175)),
                (192.0, Aim(true)),
                (192.0, Fire(Firing::Lobbed)),
                (240.0, on_weapon(10112, GUN_NODE)),
                (240.0, Parts(3)),
                (240.0, Parts(5)),
                (240.0, Sound(108)),
                (240.0, Undim),
                (260.0, Aim(false)),
                (260.0, Stand),
                (260.0, Unlock),
            ]);
            Scene { clip: "btl4005", second: None, aimed: false, scoped: false, steps, spends_boost: true, shoots: true, ends: vec![10110, 10111, 998002, 10112] }
        }
        // Vulcannon, Volcane and Vortex (0x7C4F8, 0x7C930, 0x7CCF4): she takes a
        // gun of the skill's, and once the stage is the battle's again it fires a
        // volley of rays, one every six sixtieths for as many as the skill's
        // bullet has to its count, each at where she aims as it leaves. What is
        // behind her stays dimmed until the last has gone.
        10..=12 => {
            let near = Lens(20.0, LENS_FAR);
            let (clip, back, before): (_, _, &[(f32, Step)]) = match skill {
                10 => (
                    "btl4002",
                    128.0,
                    &[(16.0, Sound(132)), (28.0, Parts(3)), (28.0, Parts(0)), (28.0, Weapon(GUN_NODE)), (50.0, Voice("brs_2800")), (60.0, Sound(164)), (60.0, Sound(144)), (64.0, near)],
                ),
                11 => (
                    "btl4003",
                    120.0,
                    &[(12.0, Voice("brs_4500")), (20.0, Sound(132)), (22.0, Parts(3)), (22.0, Parts(0)), (22.0, Weapon(GUN_NODE)), (74.0, Sound(164)), (74.0, Sound(144))],
                ),
                _ => (
                    "btl4021",
                    122.0,
                    &[(20.0, Sound(132)), (40.0, Voice("brs_3400")), (56.0, Parts(3)), (56.0, Parts(0)), (56.0, Weapon(GUN_NODE)), (64.0, Sound(164)), (64.0, Sound(144)), (100.0, near)],
                ),
            };
            // On the gun from the start, and again as the volley is over; one put
            // nowhere with the first, which Vulcannon makes a frame on, and it
            // alone has both be no more as the stage is given back (0x7C788).
            let first = match skill {
                10 => 10060,
                11 => 10080,
                _ => 10200,
            };
            let mut steps = vec![
                (0.0, Say),
                (0.0, TakeStage),
                (0.0, Dim),
                (0.0, Camera),
                (0.0, near),
                (0.0, Animation),
                (0.0, LoopAnimation),
                (0.0, on_weapon(first, GUN_NODE)),
                (if skill == 10 { 2.0 } else { 0.0 }, nowhere(first + 1, 3)),
            ];
            steps.extend_from_slice(before);
            if skill == 10 {
                steps.extend([(back, EndEffects(first)), (back, EndEffects(first + 1))]);
            }
            steps.extend([
                (back, CameraBack),
                (back, GiveStageBack),
                (back, CursorFree(true)),
                (back, Volley),
                (6.0, Reload),
                (0.0, CursorFree(false)),
                (0.0, PlayOut),
                (0.0, on_weapon(first + 2, GUN_NODE)),
                (0.0, Parts(3)),
                (0.0, Parts(5)),
                (0.0, Sound(108)),
                (0.0, Aim(false)),
                (0.0, Restart),
                (0.0, Undim),
                (20.0, Stand),
                (20.0, Unlock),
            ]);
            Scene { clip, second: None, aimed: false, scoped: false, steps, spends_boost: true, shoots: true, ends: Vec::new() }
        }
        _ => return None,
    })
}

/// What the scene of a skill is played with, of the skill's archive.
#[derive(Clone)]
pub struct Staging {
    pub clip: Arc<Clip>,
    pub camera: Arc<CameraClip>,
    /// The same of the scene's second part, if it has two.
    pub second: Option<(Arc<Clip>, Arc<CameraClip>)>,
    folder: String,
    /// The archive's own bank of sounds, if it has one.
    bank: Option<String>,
    /// What one that shoots fires: the bullet its record names (0x12B22C).
    pub shot: Option<Shot>,
    /// The weapon she takes for it, if she takes one: the model of its archive.
    pub weapon: Option<Arc<(insm::Model, Vec<u8>)>>,
    /// The attack of hers that one which strikes the enemies outright strikes
    /// them with (0x12B24C).
    pub strike: Option<i32>,
}

impl Staging {
    /// The file of a line of hers the scene has.
    pub fn voice(&self, name: &str) -> String {
        format!("{}/{name}.at3", self.folder)
    }

    pub fn bank(&self) -> Option<&str> {
        self.bank.as_deref()
    }
}

/// What the skills of her slots are played with in a battle, by the skill.
#[derive(Clone, Default)]
pub struct Stagings(HashMap<i32, Staging>);

impl Stagings {
    /// Loads it for the skills in her slots that have a scene: the original has
    /// the archives of the skills she comes with loaded as the battle is.
    pub fn load(data: &GameData, skills: &Skills, shots: &Shots, equipped: [u8; SLOTS]) -> Result<Stagings> {
        let mut stagings = HashMap::new();
        for id in equipped.into_iter().filter(|&id| id > 0).map(i32::from) {
            let (Some(skill), Some(scene)) = (skills.get(id), scene(id)) else { continue };
            let folder = skill.folder();
            let part = |name: &str| {
                let file = |what: &str| format!("{folder}/chr_p_brs02_{name}_{what}");
                let clip = data.clip(file("all.anm")).with_context(|| format!("the animation of skill {id}"))?;
                let camera = data.camera(file("cam.cam")).with_context(|| format!("the camera's move of skill {id}"))?;
                anyhow::Ok((clip, camera))
            };
            let (clip, camera) = part(scene.clip)?;
            let second = scene.second.map(part).transpose()?;
            let bank = data.list(&folder)?.into_iter().find(|file| file.to_ascii_lowercase().ends_with(".phd")).map(|file| format!("{folder}/{file}"));
            let shot = if skill.kind == KIND_ATTACK { shots.of_player(skill.attacks[0]).copied() } else { None };
            // Each archive has the one weapon its skill takes.
            let weapon = match data.list(&folder)?.into_iter().find(|file| file.to_ascii_lowercase().ends_with(".mdl")) {
                Some(file) if scene.weapon_node().is_some() => Some(Arc::new(data.model(format!("{folder}/{file}")).with_context(|| format!("the weapon of skill {id}"))?)),
                _ => None,
            };
            let strike = (skill.kind == KIND_ATTACK && skill.attacks[1] > 0).then_some(skill.attacks[1]);
            stagings.insert(id, Staging { clip, camera, second, folder, bank, shot, weapon, strike });
        }
        Ok(Stagings(stagings))
    }

    pub fn get(&self, skill: i32) -> Option<&Staging> {
        self.0.get(&skill)
    }

    /// The folders of the archives of the skills, in the order of their numbers.
    pub fn folders(&self) -> Vec<String> {
        let mut skills: Vec<(&i32, &Staging)> = self.0.iter().collect();
        skills.sort_by_key(|(skill, _)| **skill);
        skills.into_iter().map(|(_, staging)| staging.folder.clone()).collect()
    }

    /// The weapons she takes for the skills of her slots, by the skill.
    pub fn weapons(&self) -> impl Iterator<Item = (i32, &(insm::Model, Vec<u8>))> {
        self.0.iter().filter_map(|(&skill, staging)| Some((skill, &**staging.weapon.as_ref()?)))
    }
}

/// Whom a blow of a skill lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    /// Every enemy that has a body to strike (0x74EF0).
    All,
    /// The one enemy the skill is for (0x778F8).
    One(Entity),
    /// The part of an enemy that the lock point of this gauge of it is on: a
    /// snipe's (0x7F7B4).
    Part(Entity, usize),
}

/// The skills that are to strike the enemies outright this frame, each with whom
/// it strikes: her state asks, and the enemies' part of the frame carries it out.
#[derive(Resource, Default)]
pub struct Strikes(pub Vec<(i32, Blow)>);

/// How many slots she has for skills.
pub const SLOTS: usize = 4;
/// The button each slot is on with R held, in the order the save keeps them, and
/// the row of the battle's column each has (0x16A190).
pub const BUTTONS: [u32; SLOTS] = [button::TRIANGLE, button::CIRCLE, button::CROSS, button::SQUARE];
const ROWS: [usize; SLOTS] = [0, 2, 3, 1];

/// A slot of hers in a battle, with the skill in it (0x9F130).
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub skill: Skill,
    /// How far it is to being ready, of one (its + 0x24 of its + 0x20).
    pub share: f32,
    /// The sixtieths it takes to be ready from nothing (its + 0x2C).
    time: f32,
}

impl Slot {
    pub fn ready(&self) -> bool {
        self.share >= 1.0
    }
}

/// Her slots in a battle (`BtSkillPallet`'s part that is not its picture).
#[derive(Resource, Clone, Debug, Default)]
pub struct Slots {
    slots: [Option<Slot>; SLOTS],
    /// The rows whose skills have just come to be ready, for the column to flash.
    pub ready_now: Vec<usize>,
}

impl Slots {
    /// The slots as a battle starts with them (0x9F068): each skill ready at
    /// once unless the table has it wait the first time.
    pub fn new(skills: &Skills, equipped: [u8; SLOTS]) -> Slots {
        let slots = equipped.map(|id| {
            let skill = skills.get(i32::from(id)).filter(|_| id > 0)?.clone();
            let time = skill.first as f32;
            Some(Slot { skill, share: if time <= 0.0 { 1.0 } else { 0.0 }, time })
        });
        let ready_now = (0..SLOTS).filter(|&slot| slots[slot].as_ref().is_some_and(Slot::ready)).map(|slot| ROWS[slot]).collect();
        Slots { slots, ready_now }
    }

    pub fn get(&self, slot: usize) -> Option<&Slot> {
        self.slots.get(slot)?.as_ref()
    }

    /// The name and the share of each row of the column, top to bottom.
    pub fn rows(&self) -> [Option<(&str, f32)>; SLOTS] {
        let mut rows = [None; SLOTS];
        for (slot, row) in ROWS.into_iter().enumerate() {
            rows[row] = self.get(slot).map(|slot| (slot.skill.name.as_str(), slot.share));
        }
        rows
    }

    /// A frame of them (0x9F2E4): each skill that is not ready comes nearer to
    /// it. Says whether any has come to be.
    pub fn tick(&mut self, sixtieths: f32) -> bool {
        let before = self.ready_now.len();
        for (slot, row) in self.slots.iter_mut().zip(ROWS) {
            let Some(slot) = slot.as_mut().filter(|slot| !slot.ready()) else { continue };
            slot.share += 1.0 / slot.time * sixtieths;
            if slot.ready() {
                slot.share = 1.0;
                self.ready_now.push(row);
            }
        }
        self.ready_now.len() > before
    }

    /// A slot's skill has been used (0x9ED3C): it starts over, with the time the
    /// table has it take after a use.
    pub fn used(&mut self, slot: usize) {
        let Some(used) = self.slots.get_mut(slot).and_then(Option::as_mut) else { return };
        used.time = used.skill.recharge as f32;
        used.share = if used.time <= 0.0 { 1.0 } else { 0.0 };
        if used.ready() {
            self.ready_now.push(ROWS[slot]);
        }
    }

    /// The slot a skill is in starts over (0x9EE64 and 0x9ED3C): for a skill
    /// that is used up only as it is fired.
    pub fn used_skill(&mut self, id: i32) {
        if let Some(slot) = (0..SLOTS).find(|&slot| self.get(slot).is_some_and(|held| held.skill.id == id)) {
            self.used(slot);
        }
    }

    /// A slot's skill is ready at once (0x9EEC4). Says whether it was not: it is
    /// heard then.
    pub fn fill(&mut self, slot: usize) -> bool {
        match self.slots.get_mut(slot).and_then(Option::as_mut).filter(|held| !held.ready()) {
            Some(held) => {
                held.share = 1.0;
                self.ready_now.push(ROWS[slot]);
                true
            }
            None => false,
        }
    }

    /// Every skill but one is ready at once (0x9EF90). Says how many were not:
    /// each of those is heard.
    pub fn fill_others(&mut self, but: i32) -> usize {
        let before = self.ready_now.len();
        for (slot, row) in self.slots.iter_mut().zip(ROWS) {
            if let Some(slot) = slot.as_mut().filter(|slot| slot.skill.id != but && !slot.ready()) {
                slot.share = 1.0;
                self.ready_now.push(row);
            }
        }
        self.ready_now.len() - before
    }
}

/// What the names' table has of a skill, which is all the menus go by: it is a
/// table of its own, with room for skills the battle's table has none of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct About {
    pub name: String,
    pub description: String,
    /// Its place in the order the menus list skills in (0x12CD04).
    pub order: u16,
    /// What the menus sort it under (0x12CDBC).
    pub category: u8,
    /// Whether the menus list it at all (0x12CD5C).
    pub listed: bool,
    /// Which kind of picture it has beside its name (0x12CE18).
    pub picture: u8,
}

/// The table of her skills, and what the names' table has of each number from
/// the first.
#[derive(Resource, Clone, Debug, Default)]
pub struct Skills(Vec<Skill>, Vec<About>);

impl Skills {
    pub fn load(data: &GameData) -> Result<Skills> {
        let table = bxcb::Table::parse(&data.read(TABLE)?).with_context(|| format!("parsing {TABLE}"))?;
        ensure!(table.fields == Skill::FIELDS, "{TABLE} has {} fields per record", table.fields);
        let names = xtc::Table::parse(&data.read(NAMES)?).with_context(|| format!("parsing {NAMES}"))?;
        // A record of the names: where its name and what it does are written, and
        // a word of smaller things.
        let mut abouts = Vec::new();
        for record in (0..names.len()).filter_map(|index| names.record(index)) {
            ensure!(record.len() >= 12, "{NAMES} has a record of {} bytes", record.len());
            let word = |at: usize| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            abouts.push(About {
                name: names.string(word(0))?.to_owned(),
                description: names.string(word(4))?.to_owned(),
                order: word(8) as u16,
                category: ((word(8) >> 16) & 7) as u8,
                listed: word(8) & (1 << 19) != 0,
                picture: ((word(8) >> 21) & 0x1f) as u8,
            });
        }
        let mut skills = Vec::new();
        for record in table.records() {
            let field = |index: usize| record[index] as i32;
            // A skill's name is the record before its number's (0x12CF0C).
            let about = usize::try_from(field(0) - 1).ok().and_then(|index| abouts.get(index)).cloned().unwrap_or_default();
            skills.push(Skill {
                id: field(0),
                kind: field(1),
                first: field(2),
                recharge: field(3),
                archive: table.string(record[13])?.to_owned(),
                message: field(14),
                effects: if field(1) == KIND_HELP { [field(15), field(18), field(19)] } else { [0; 3] },
                attacks: [field(16), field(17)],
                auto: if field(1) == KIND_AUTO { [4, 5, 6, 7, 8, 9].map(field) } else { [0; 6] },
                bonus: (field(10), field(11), field(12)),
                name: about.name,
                description: about.description,
                category: about.category,
            });
        }
        Ok(Skills(skills, abouts))
    }

    #[cfg(test)]
    pub fn from_list(skills: Vec<Skill>) -> Skills {
        Skills(skills, Vec::new())
    }

    /// What the menus have of the skill with that number (0x12CE7C).
    pub fn about(&self, id: i32) -> Option<&About> {
        usize::try_from(id - 1).ok().and_then(|index| self.1.get(index))
    }

    /// The skills a list of the menu has (0x4B820): of every number the names'
    /// table has, those it sorts under `category` that she has and that are to
    /// be listed, in the table's order for them.
    pub fn listed(&self, category: u8, progress: &Progress) -> Vec<i32> {
        let of_the_list = |(index, about): (usize, &About)| {
            let id = index as i32 + 1;
            (about.category == category && progress.flag(HAS + id) && about.listed).then_some((about.order, id))
        };
        let mut list: Vec<(u16, i32)> = self.1.iter().enumerate().filter_map(of_the_list).collect();
        list.sort_by_key(|&(order, _)| order);
        list.into_iter().map(|(_, id)| id).collect()
    }

    /// The skill with that number (0x12AB9C).
    pub fn get(&self, id: i32) -> Option<&Skill> {
        self.0.iter().find(|skill| skill.id == id)
    }

    /// Those that act of themselves in a battle (0x9F79C): the ones of that kind
    /// that she has and has not put aside, and any that a run was asked to have
    /// `also`.
    pub fn of_themselves(&self, progress: Option<&Progress>, also: &[i32]) -> Vec<Skill> {
        let hers = |id: i32| progress.is_some_and(|progress| (1..MOST).contains(&id) && progress.flag(HAS + id) && !progress.flag(SET + id));
        self.0.iter().filter(|skill| skill.kind == KIND_AUTO && (also.contains(&skill.id) || hers(skill.id))).cloned().collect()
    }

    /// What her skills add to her figures (0x14CF18 to 0x14CFD0): those in her
    /// slots, and those always in force that she has and has not put aside.
    pub fn bonuses(&self, slots: [u8; 4], progress: &Progress) -> [i32; 4] {
        let in_slots = slots.iter().filter(|&&id| id > 0).filter_map(|&id| self.get(i32::from(id)));
        let always = self.0.iter().filter(|skill| {
            (1..MOST).contains(&skill.id) && skill.always() && progress.flag(HAS + skill.id) && !progress.flag(SET + skill.id)
        });
        let mut sums = [0; 4];
        for skill in in_slots.chain(always) {
            for (sum, figure) in sums.iter_mut().zip(Figure::ALL) {
                *sum += skill.adds(figure);
            }
        }
        sums
    }
}

impl PlayerStats {
    /// Her figures are worked out anew (0x14D008), which the original does as a
    /// flag of a skill that is always in force is set or cleared (0x13DC, 0x1418):
    /// what her skills add now takes the place of what they added before, and
    /// she has no more HP than she can have.
    pub fn take_bonuses(&mut self, bonuses: [i32; 4]) {
        if bonuses == self.skill_bonus {
            return;
        }
        let by = |figure: Figure| bonuses[figure as usize - 1] - self.skill_bonus[figure as usize - 1];
        self.max_hp = (self.max_hp + by(Figure::Hp) as f32).clamp(0.0, Self::HP_CAP);
        self.attack = (self.attack + by(Figure::Attack)).clamp(0, Self::FIGURE_CAP);
        self.special = (self.special + by(Figure::Special)).clamp(0, Self::FIGURE_CAP);
        self.defence = (self.defence + by(Figure::Defence)).clamp(0, Self::FIGURE_CAP);
        self.hp = self.hp.min(self.max_hp);
        self.skill_bonus = bonuses;
    }
}

/// Keeps her figures to what her skills add, whatever has set or cleared a
/// skill's flag: a script of a map, the records' script as it gives what a record
/// is worth, or the port's own options.
fn keep_bonuses(skills: Res<Skills>, progress: Option<Res<Progress>>, mut stats: ResMut<PlayerStats>) {
    let Some(progress) = progress.filter(|progress| progress.is_changed()) else { return };
    let bonuses = skills.bonuses(stats.skills, &progress);
    if bonuses != stats.skill_bonus {
        stats.take_bonuses(bonuses);
        info!("her skills add {bonuses:?} to her HP, attack, special attack and defence: she is now {:?}", *stats);
    }
}

/// The sound of a skill that has come to be ready (0x9F370), of the battle's bank.
pub const SOUND_READY: u32 = 0x65;

/// A frame of her slots (0x9F254): their skills come nearer to being ready while
/// the battle is on and nobody has the stage (the root's flags 8, 9 and 10).
fn charge(flow: Res<Flow>, player: Option<Single<&PcActor>>, slots: Option<ResMut<Slots>>, mut board: ResMut<Board>) {
    let (Some(player), Some(mut slots)) = (player, slots) else { return };
    if flow.running() && !player.staged && slots.tick(SIXTIETHS_PER_STEP) {
        board.play(COMMON_BANK, SOUND_READY, FULL_VOLUME);
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(FixedPreUpdate, keep_bonuses).add_systems(FixedUpdate, charge.after(super::conclude).run_if(in_state(Mode::Battle).and_then(super::not_paused)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Option<Skills> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        Some(Skills::load(&data).unwrap())
    }

    #[test]
    fn the_table_has_her_skills_with_their_names_and_times() {
        let Some(skills) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(skills.0.len(), 66);
        // The names' table has room for 128, of which 71 are anything; the menus
        // list those of a slot and those always in force apart, each in an order
        // that has the better of two skills after the other.
        assert_eq!(skills.1.len(), 128);
        assert_eq!(skills.1.iter().filter(|about| about.listed).count(), 71);
        let mut progress = Progress::start();
        assert!(skills.listed(OF_A_SLOT, &progress).is_empty());
        for id in [1, 20, 22, 23, 24, 66, 30, 42, 72] {
            progress.set_flag(HAS + id, true);
        }
        assert_eq!(skills.listed(OF_A_SLOT, &progress), [1, 22, 20, 23, 66, 24]);
        assert_eq!(skills.listed(ALWAYS, &progress), [30, 42]);
        let about = |id: i32| skills.about(id).map(|about| (about.name.as_str(), about.picture));
        assert_eq!([about(1), about(23), about(30), about(42)], [Some(("Charge Shot", 1)), Some(("Aggressor", 2)), Some(("Aftercare", 6)), Some(("ATK Bonus", 4))]);
        assert_eq!((about(0), about(129)), (None, None));
        // The first: an attack that shoots, ready from the start and again
        // twenty seconds after it is used.
        let charge = skills.get(1).unwrap();
        assert_eq!((charge.name.as_str(), charge.kind, charge.first, charge.recharge), ("Charge Shot", 1, 0, 1200));
        assert_eq!((charge.archive.as_str(), charge.message, charge.attacks), ("bskl_assult_00.vol", 10, [10, 0]));
        assert!(charge.description.ends_with("Cooldown: 20 seconds.") && !charge.always());
        // One that helps her gives status effects.
        let aggressor = skills.get(23).unwrap();
        assert_eq!((aggressor.name.as_str(), aggressor.kind, aggressor.effects, aggressor.recharge), ("Aggressor", 2, [1500, 1600, 0], 1800));
        assert_eq!(skills.get(25).map(|skill| (skill.name.as_str(), skill.effects, skill.line())), Some(("Defender", [1600, 0, 0], 34)));
        // One that attacks gives none, and one that only adds to a figure has no
        // line in the log.
        assert_eq!((charge.effects, charge.line(), skills.get(54).map(Skill::line)), ([0; 3], 10, Some(0)));
        assert_eq!(skills.get(68).map(|skill| skill.name.as_str()), Some("Defender +"));
        // No skill is ready later the first time than nought.
        assert!(skills.0.iter().all(|skill| skill.first == 0));
        // Those that add to a figure are always in force, and none adds a share
        // of the figure itself.
        let hp = skills.get(54).unwrap();
        assert_eq!((hp.name.as_str(), hp.kind, hp.always()), ("HP Bonus", 3, true));
        assert_eq!(Figure::ALL.map(|figure| hp.adds(figure)), [70, 0, 0, 0]);
        assert_eq!(skills.get(42).map(|skill| Figure::ALL.map(|figure| skill.adds(figure))), Some([0, 8, 0, 0]));
        assert_eq!(skills.get(48).map(|skill| Figure::ALL.map(|figure| skill.adds(figure))), Some([0, 0, 0, 10]));
        assert_eq!(skills.get(60).map(|skill| Figure::ALL.map(|figure| skill.adds(figure))), Some([0, 0, 5, 0]));
        assert!(skills.0.iter().filter(|skill| skill.kind == KIND_BONUS).all(|skill| skill.bonus.2 == 0 && skill.always()));
        assert!(skills.get(0).is_none() && skills.get(9).is_none());
    }

    #[test]
    fn a_skill_that_adds_to_a_figure_does_so_while_she_has_it_and_has_not_put_it_aside() {
        let Some(skills) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut progress = Progress::new_game();
        let mut stats = PlayerStats { level: 1, experience: 0, max_hp: 372.0, hp: 372.0, attack: 44, special: 41, defence: 24, burst_bullets: 6, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 };
        assert_eq!(skills.bonuses(stats.skills, &progress), [0; 4]);
        // The second of the records is worth "HP Bonus": seventy more HP to have,
        // which she has not got yet.
        progress.set_flag(HAS + 54, true);
        stats.take_bonuses(skills.bonuses(stats.skills, &progress));
        assert_eq!((stats.max_hp, stats.hp, stats.skill_bonus), (442.0, 372.0, [70, 0, 0, 0]));
        // Another of them, and one each for her attack and her defence.
        for id in [55, 42, 48] {
            progress.set_flag(HAS + id, true);
        }
        stats.take_bonuses(skills.bonuses(stats.skills, &progress));
        assert_eq!((stats.max_hp, stats.attack, stats.special, stats.defence), (512.0, 52, 41, 34));
        // Put aside, one adds nothing, and she has no more HP than she can have.
        stats.hp = 512.0;
        progress.set_flag(SET + 54, true);
        stats.take_bonuses(skills.bonuses(stats.skills, &progress));
        assert_eq!((stats.max_hp, stats.hp), (442.0, 442.0));
        // A skill of a slot adds nothing to a figure, and having an attack is
        // nothing to them either.
        progress.set_flag(HAS + 1, true);
        stats.skills = [1, 0, 25, 0];
        assert_eq!(skills.bonuses(stats.skills, &progress), [70, 8, 0, 10]);
    }

    #[test]
    fn those_that_act_of_themselves_are_the_ones_she_has_and_has_not_put_aside() {
        let Some(skills) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The nine of them, by when each acts, on what condition, and what it
        // does with what: Aftercare and its better, Max Up, Rocket Starter, Dead
        // Guard, Six Pocket, Attack-Free, Danger Guard and Sway Three.
        let table: Vec<(i32, &str, [i32; 6])> = skills.0.iter().filter(|skill| skill.kind == KIND_AUTO).map(|skill| (skill.id, skill.name.as_str(), skill.auto)).collect();
        assert_eq!(
            table,
            [
                (30, "Aftercare", [7, 1, 20, 1, 25, 0]),
                (31, "Aftercare +", [7, 1, 30, 1, 25, 0]),
                (32, "Max Up", [8, 2, 100, 2, 125, 0]),
                (33, "Rocket Starter", [1, 1, 25, 6, 1507, 0]),
                (34, "Dead Guard", [2, 3, 25, 6, 1608, 0]),
                (35, "Six Pocket", [5, 1, 30, 3, 150, 200]),
                (36, "Attack-Free", [3, 1, 10, 4, 0, 0]),
                (37, "Danger Guard", [6, 4, 0, 5, 501, 0]),
                (38, "Sway Three", [4, 1, 10, 4, 0, 0]),
            ]
        );
        // None has a line in the log (0x12B1AC), and all are always in force.
        assert!(skills.0.iter().filter(|skill| skill.kind == KIND_AUTO).all(|skill| skill.line() == 0 && skill.always()));
        // No other kind has those figures.
        assert!(skills.0.iter().filter(|skill| skill.kind != KIND_AUTO).all(|skill| skill.auto == [0; 6]));

        let ids = |progress: Option<&Progress>, also: &[i32]| skills.of_themselves(progress, also).iter().map(|skill| skill.id).collect::<Vec<i32>>();
        let mut progress = Progress::new_game();
        assert!(ids(None, &[]).is_empty() && ids(Some(&progress), &[]).is_empty());
        for id in [1, 33, 36, 54] {
            progress.set_flag(HAS + id, true);
        }
        assert_eq!(ids(Some(&progress), &[]), [33, 36]);
        progress.set_flag(SET + 33, true);
        assert_eq!(ids(Some(&progress), &[]), [36]);
        // A run can be asked to have some, whatever she has; one of another kind
        // is none of them.
        assert_eq!(ids(None, &[37, 1, 54]), [37]);
        assert_eq!(ids(Some(&progress), &[33]), [33, 36]);
    }

    #[test]
    fn the_skills_of_her_slots_are_ready_at_first_and_take_their_time_after_a_use() {
        let Some(skills) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // Charge Shot on triangle and Defender on cross: the rows of the column
        // are triangle, square, circle, cross.
        let mut slots = Slots::new(&skills, [1, 0, 25, 0]);
        assert_eq!(slots.rows(), [Some(("Charge Shot", 1.0)), None, None, Some(("Defender", 1.0))]);
        assert_eq!(std::mem::take(&mut slots.ready_now), [0, 3]);
        assert!(slots.get(0).is_some_and(Slot::ready) && slots.get(1).is_none());
        // Ready, time is nothing to them.
        assert!(!slots.tick(600.0) && slots.ready_now.is_empty());
        // Used, Charge Shot takes its twenty seconds again, and says so when it
        // has.
        slots.used(0);
        assert_eq!(slots.rows()[0], Some(("Charge Shot", 0.0)));
        for _ in 0..599 {
            assert!(!slots.tick(2.0));
        }
        assert!(slots.get(0).is_some_and(|slot| !slot.ready() && slot.share > 0.99));
        // (The shares are added up as the original adds them, which leaves the
        // six hundredth frame a hair short.)
        assert!(slots.tick(2.0) || slots.tick(2.0));
        assert_eq!(slots.rows()[0], Some(("Charge Shot", 1.0)));
        assert_eq!(std::mem::take(&mut slots.ready_now), [0]);
        // One skill can have all the others ready at once.
        slots.used(0);
        slots.used(2);
        assert_eq!(slots.fill_others(25), 1);
        assert_eq!((slots.rows()[0], slots.rows()[3]), (Some(("Charge Shot", 1.0)), Some(("Defender", 0.0))));
        // A slot with nothing in it is nothing to use.
        slots.used(1);
        assert_eq!(slots.rows()[2], None);
    }

    #[test]
    fn every_skill_with_a_scene_has_what_it_is_played_with() {
        let Some(skills) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        let shots = Shots::load(&data).unwrap();
        let helping: Vec<i32> = skills.0.iter().filter(|skill| skill.kind == KIND_HELP).map(|skill| skill.id).collect();
        assert_eq!(helping, [23, 66, 24, 67, 25, 68, 26, 69, 27, 70, 28, 71, 29]);
        // Her model has the nodes that the weapons of the skills are fixed to.
        let her = data.model(crate::player::model()).unwrap().0;
        assert!([GUN_NODE, BLADE_NODE, SWORD_NODE].iter().all(|id| her.nodes.iter().any(|node| node.id == *id)));
        // Those, and the attacks that are ported.
        for &id in helping.iter().chain(&[1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 14, 15, 18, 19, 20, 22]) {
            let scene = scene(id).unwrap_or_else(|| panic!("skill {id} has no scene"));
            // What it does comes in the order of its times, which start over
            // only where the state's clock does.
            let restarts = |step: Step| matches!(step, Step::Volley | Step::Reload | Step::Restart);
            assert!(scene.steps.windows(2).all(|pair| pair[0].0 <= pair[1].0 || restarts(pair[0].1)), "skill {id}");
            // Its animation, its camera's move and its lines are in its archive.
            let stagings = Stagings::load(&data, &skills, &shots, [id as u8, 0, 0, 0]).unwrap();
            let staging = stagings.get(id).unwrap();
            for &(_, step) in &scene.steps {
                match step {
                    Step::Voice(name) => assert!(data.exists(staging.voice(name)), "skill {id}: {name}"),
                    Step::OwnSound(_) => assert!(staging.bank().is_some(), "skill {id} has no bank of its own"),
                    // The camera sees from no farther off than a battle's own does,
                    // but for the two that end far from her.
                    Step::Lens(near, far) => assert!((near <= 50.0 || matches!(id, 3 | 6)) && far >= 20000.0, "skill {id}"),
                    _ => {}
                }
            }
            // It names itself and takes the stage, gives the stage and the camera
            // back, and ends with her free to act; what it dims it lights again.
            assert!(scene.steps.iter().take_while(|step| step.0 == 0.0).any(|step| step.1 == Step::TakeStage), "skill {id}");
            assert_eq!(scene.has(Step::Say), id != 3, "skill {id}");
            assert!(scene.has(Step::Camera) && scene.has(Step::CameraBack) && scene.has(Step::GiveStageBack), "skill {id}");
            assert_eq!(scene.steps.last(), Some(&(scene.end(), Step::Unlock)), "skill {id}");
            assert_eq!(scene.has(Step::Dim) || scene.has(Step::DimAtOnce), scene.has(Step::Undim), "skill {id}");
            // One that helps her gives status effects, all but the one that has the
            // others ready; only the two that give a boost do not spend one; and
            // one that attacks fires the bullet its record names.
            let record = skills.get(id).unwrap();
            let attack = record.kind == KIND_ATTACK;
            assert_eq!(scene.has(Step::Give), record.effects[0] != 0, "skill {id}");
            assert_eq!(scene.spends_boost, !matches!(id, 24 | 67), "skill {id}");
            let strikes = scene.has(Step::Strike) || scene.has(Step::StrikeTarget) || scene.has(Step::Trigger);
            assert_eq!(attack, scene.fires() || scene.has(Step::Volley) || strikes, "skill {id}");
            // A snipe takes what she has locked on to without needing it, has
            // only that seen once its scope is up, and gives the stage back only
            // when the scope has gone.
            assert_eq!((scene.scoped, scene.has(Step::Trigger)), (matches!(id, 7 | 8), matches!(id, 7 | 8)), "skill {id}");
            if scene.scoped {
                assert!(!scene.aimed && staging.strike == Some(id + 19), "skill {id}");
                let at = |step: Step| scene.steps.iter().position(|(_, other)| *other == step).unwrap();
                let order = [
                    Step::CameraBack,
                    Step::ShowTarget,
                    Step::Hide(true),
                    Step::Scope(true),
                    Step::Restart,
                    Step::SightLock,
                    Step::SightWait,
                    Step::Trigger,
                    Step::Scope(false),
                    Step::GiveStageBack,
                    Step::Hide(false),
                ];
                assert!(order.windows(2).all(|pair| at(pair[0]) + 1 == at(pair[1])), "skill {id}");
            }
            assert_eq!(scene.shoots, matches!(id, 1 | 2 | 10..=12 | 14 | 15 | 18 | 19), "skill {id}");
            // One that throws a bomb has a bomb for its bullet: slow, with a fuse
            // of a second and a half, and a burst that reaches far and strikes
            // three times.
            assert_eq!(scene.has(Step::Fire(Firing::Lobbed)), matches!(id, 18 | 19), "skill {id}");
            if scene.has(Step::Fire(Firing::Lobbed)) {
                let shot = staging.shot.unwrap();
                assert_eq!((shot.kind, shot.attack, shot.node, shot.sound), (4, id + 19, 50, -1), "skill {id}");
                assert_eq!((shot.speed, shot.peak, shot.fuse, shot.blast, shot.beam_time, shot.beam_tick, shot.blast_sound), (5.0, 300.0, 90.0, 300.0, 45.0, 16.0, 151), "skill {id}");
            }
            // One that fires a volley has a ray for its bullet, with a count: the
            // rays leave her cannon's muzzle node, thirty-five of them. Her
            // animation goes round until the last has gone, and the cursor is
            // free meanwhile; what is behind her is lit again only then.
            assert_eq!(scene.has(Step::Volley), matches!(id, 10..=12), "skill {id}");
            if scene.has(Step::Volley) {
                let shot = staging.shot.unwrap();
                assert_eq!((shot.kind, shot.count, shot.node, shot.sound, shot.attack), (0, 35, 50, 174, id + 19), "skill {id}");
                let at = |step: Step| scene.steps.iter().position(|(_, other)| *other == step).unwrap();
                let order = [Step::Animation, Step::LoopAnimation, Step::GiveStageBack, Step::CursorFree(true), Step::Volley, Step::Reload, Step::CursorFree(false), Step::PlayOut];
                assert!(order.windows(2).all(|pair| at(pair[0]) < at(pair[1])), "skill {id}");
                assert!(at(Step::Reload) == at(Step::Volley) + 1 && at(Step::Volley) < at(Step::Undim) && at(Step::Restart) < at(Step::Stand), "skill {id}");
                assert_eq!(scene.steps[at(Step::Reload)].0, 6.0, "skill {id}");
            }
            assert!(!scene.fires() || staging.shot.is_some(), "skill {id}");
            // One that strikes the enemies outright has an attack of hers for it,
            // and has them seen again first.
            assert_eq!(scene.has(Step::Strike), matches!(id, 3 | 6 | 20 | 22), "skill {id}");
            if scene.has(Step::Strike) {
                assert!(staging.strike.is_some_and(|attack| attack > 0), "skill {id}");
                let at = |step: Step| scene.steps.iter().position(|(_, other)| *other == step).unwrap();
                assert!(at(Step::ShowEnemies) < at(Step::Strike) && at(Step::Strike) < at(Step::GiveStageBack), "skill {id}");
            }
            // One that is for the enemy she has locked on to has that enemy alone
            // seen again and struck, where she has been put: at the middle of the
            // map from the first, and where the enemy stands as the second part of
            // the scene begins.
            assert_eq!(scene.has(Step::StrikeTarget), matches!(id, 4 | 5), "skill {id}");
            assert_eq!((scene.aimed, scene.second.is_some(), staging.second.is_some()), (matches!(id, 4 | 5), matches!(id, 4 | 5), matches!(id, 4 | 5)), "skill {id}");
            if scene.aimed {
                assert!(staging.strike.is_some_and(|attack| attack > 0), "skill {id}");
                let at = |step: Step| scene.steps.iter().position(|(_, other)| *other == step).unwrap();
                assert_eq!(at(Step::Put(Placing::Middle)), at(Step::TakeStage) + 1, "skill {id}");
                assert!(at(Step::ShowTarget) < at(Step::StrikeTarget) && at(Step::StrikeTarget) < at(Step::GiveStageBack), "skill {id}");
                let second: Vec<f32> = [Step::SecondCamera, Step::SecondAnimation, Step::Put(Placing::Target)].map(|step| scene.steps[at(step)].0).to_vec();
                assert_eq!(second, [142.0; 3], "skill {id}");
            }
            // One that takes a weapon has it in its archive, to hold at the node of
            // hers that goes with it once her hand is empty, and gives her cannon
            // back to her before it ends.
            let node = match id {
                7 | 8 | 10..=12 | 14 | 15 | 18 | 19 => Some(GUN_NODE),
                4..=6 => Some(BLADE_NODE),
                20 | 22 => Some(SWORD_NODE),
                _ => None,
            };
            assert_eq!((scene.weapon_node(), staging.weapon.is_some()), (node, node.is_some()), "skill {id}");
            if let Some(node) = node {
                let at = |step: Step| scene.steps.iter().position(|(_, other)| *other == step).unwrap();
                let emptied = scene.steps.iter().position(|(_, step)| matches!(step, Step::Parts(0 | 1))).unwrap();
                assert!(emptied < at(Step::Weapon(node)) && at(Step::Weapon(node)) < at(Step::Parts(5)), "skill {id}");
            }
        }
        assert!(scene(29).unwrap().has(Step::ReadyOthers));
        // The three moves of those that help her come closer to her one than the
        // other, and the first of them sees from nearer once its flash has come.
        let near = |id: i32| scene(id).unwrap().steps.iter().find_map(|(_, step)| if let Step::Lens(near, _) = step { Some(*near) } else { None });
        assert_eq!([25, 27, 28].map(near), [Some(50.0), Some(35.0), Some(20.0)]);
        assert!(scene(25).unwrap().steps.contains(&(76.0, Step::Lens(45.0, 20000.0))));
        assert_eq!((scene(25).unwrap().end(), scene(29).unwrap().end(), scene(1).unwrap().end()), (160.0, 372.0, 244.0));
        // Steel Cutter's move ends high up behind her and to one side, beyond the
        // walls at the stage's edge, which are seen through from there: the map's
        // faces that have culling on are drawn from their front only.
        let far = Stagings::load(&data, &skills, &shots, [20, 0, 0, 0]).unwrap().get(20).unwrap().camera.pose(200.0).transform;
        assert!(far.translation.z < -500.0 && far.translation.y > 900.0 && far.translation.x < -1200.0, "{}", far.translation);
        assert!((far.rotation * Vec3::NEG_Z).z > 0.6);
        // Defender's move cuts once, where the walkthrough has it cut: from all of
        // her to her head, a second in. From there the lens closes in on her.
        let stagings = Stagings::load(&data, &skills, &shots, [25, 1, 2, 14]).unwrap();
        let camera = &stagings.get(25).unwrap().camera;
        let cuts: Vec<f32> = (0..80).map(|step| step as f32 * 2.0).filter(|&from| camera.cuts(from, from + 2.0)).collect();
        assert_eq!(cuts, [58.0]);
        assert!(camera.pose(70.0).focal_length > 1.5 * camera.pose(60.0).focal_length);
        // Charge Shot's bullet leaves her cannon's muzzle, flies straight and fast
        // and goes through what it hits; Burst Shot's is the same and bigger.
        let bullet = stagings.get(1).unwrap().shot.unwrap();
        assert_eq!((bullet.id, bullet.kind, bullet.speed, bullet.radius, bullet.attack), (10, 1, 120.0, 50.0, 10));
        assert!(bullet.pierces && bullet.from_node && bullet.node == 50 && bullet.life == 0.0 && bullet.delay == 0.0);
        assert_eq!(stagings.get(2).unwrap().shot.map(|shot| (shot.id, shot.radius, shot.attack)), Some((11, 75.0, 11)));
        // Tracer Gun's are three, each fired along with the one before, that leave
        // from above her feet to her sides; Chaser Gun's are four.
        let chain = |first: i32| {
            let mut bullets = vec![shots.of_player(first).copied().unwrap()];
            while let Some(next) = shots.of_player(bullets[bullets.len() - 1].next).filter(|_| bullets.len() < 8) {
                bullets.push(*next);
            }
            bullets
        };
        let tracers = chain(stagings.get(14).unwrap().shot.unwrap().id);
        assert_eq!(tracers.iter().map(|shot| (shot.id, shot.launch_yaw, shot.launch_pitch)).collect::<Vec<_>>(), [(33, 60.0, 0.0), (34, 0.0, -60.0), (35, -60.0, 0.0)]);
        assert!(tracers.iter().all(|shot| shot.kind == 2 && shot.node == -1 && shot.offset == Vec3::new(0.0, 65.0, 0.0) && shot.speed == 25.0 && shot.homing == 0.05));
        assert!(tracers.iter().all(|shot| shot.attack == 33 && shot.radius == 25.0 && !shot.pierces && shot.from_node));
        let chasers = chain(skills.get(15).map(|skill| skill.attacks[0]).unwrap());
        assert_eq!(chasers.iter().map(|shot| shot.id).collect::<Vec<_>>(), [36, 37, 38, 39]);
        assert!(chasers.iter().all(|shot| shot.kind == 2 && shot.attack == 34));
        // The attacks that are not ported have no scene, and nor has a skill that
        // only adds to a figure.
        assert!(scene(9).is_none() && scene(54).is_none());
        assert_eq!((scene(3).unwrap().end(), scene(6).unwrap().end(), scene(20).unwrap().end()), (520.0, 400.0, 320.0));
        assert_eq!((scene(4).unwrap().end(), scene(5).unwrap().end()), (220.0, 220.0));
        // A skill that is in no slot has nothing loaded for it.
        assert!(Stagings::load(&data, &skills, &shots, [0; 4]).unwrap().get(25).is_none());
    }

    #[test]
    fn an_effect_of_a_scene_is_moved_and_then_turned_from_what_it_is_put_by() {
        // The rays of Photon Shower: left where she is, far to one side, high up
        // and behind, and tipped half over about the axis across.
        let rays = Made::new(10162, Hold::Left, 3).moved(-600.0, 1500.0, -570.0).turned(50.0, 0.0, 0.0).matrix();
        assert!(rays.w_axis.truncate().abs_diff_eq(Vec3::new(-600.0, 1500.0, -570.0), 1e-3));
        let tip = 50f32.to_radians();
        assert!(rays.transform_vector3(Vec3::Y).abs_diff_eq(Vec3::new(0.0, tip.cos(), tip.sin()), 1e-5));
        assert!(rays.transform_vector3(Vec3::X).abs_diff_eq(Vec3::X, 1e-5));
        // On a weapon: a quarter round about the axis across, and before that a
        // quarter round about the one along.
        let on_gun = Made::new(10090, Hold::With(Some(GUN_NODE)), 4).turned(90.0, 0.0, 90.0).matrix();
        assert!(on_gun.w_axis.truncate() == Vec3::ZERO);
        assert!(on_gun.transform_vector3(Vec3::X).abs_diff_eq(Vec3::Z, 1e-5));
        assert!(on_gun.transform_vector3(Vec3::Y).abs_diff_eq(Vec3::NEG_X, 1e-5));
        assert!(on_gun.transform_vector3(Vec3::Z).abs_diff_eq(Vec3::NEG_Y, 1e-5));
        // And one that is only made is where what it is put by is.
        assert_eq!(Made::new(10011, Hold::Nowhere, 4).matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn the_effects_of_a_scene_are_its_skills_own_or_the_systems() {
        let Some(skills) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        let shots = Shots::load(&data).unwrap();
        let numbers = |folder: &str| -> Vec<u32> {
            let files = super::super::effects::files_of(&data, folder).unwrap();
            files.iter().flat_map(|(_, bytes)| brs_formats::efc::File::parse(bytes).unwrap().effects().map(|(number, _)| number).collect::<Vec<u32>>()).collect()
        };
        let system = numbers("SYSTEM.VOL");
        let her = data.model(crate::player::model()).unwrap().0;
        let mut behind = Vec::new();
        for id in [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 14, 15, 18, 19, 20, 22, 23, 24, 25, 26, 27, 28, 29, 66, 67, 68, 69, 70, 71] {
            let scene = scene(id).unwrap();
            let stagings = Stagings::load(&data, &skills, &shots, [id as u8, 0, 0, 0]).unwrap();
            let own: Vec<u32> = stagings.folders().iter().flat_map(|folder| numbers(folder)).collect();
            let mut made: Vec<u32> = Vec::new();
            for &(_, step) in &scene.steps {
                match step {
                    Step::Effect(effect) => {
                        // Blade Kill asks for two that are not in its archive: one
                        // is Steel Cutter's, and nothing has the other.
                        let lacking = id == 4 && matches!(effect.id, 10132 | 10142);
                        assert_eq!(own.contains(&effect.id) || system.contains(&effect.id), !lacking, "skill {id}: effect {}", effect.id);
                        assert!(matches!(effect.list, 0 | 3 | 4), "skill {id}: effect {}", effect.id);
                        if let Hold::With(Some(node)) = effect.hold {
                            assert!(her.nodes.iter().any(|other| other.id == node), "skill {id}: node {node}");
                        }
                        if effect.list == 0 {
                            behind.push(effect.id);
                        }
                        made.push(effect.id);
                    }
                    // What it has be no more, or be of another draw list, it has
                    // made by then.
                    Step::EndEffects(over) | Step::EffectList(over, _) => assert!(made.contains(&over), "skill {id}: effect {over}"),
                    _ => {}
                }
            }
            assert!(!made.is_empty(), "skill {id}");
            assert!(scene.ends.iter().all(|over| made.contains(over)), "skill {id}");
        }
        // One alone is of the scene's first draw list: the sky of Photon Shower.
        assert_eq!(behind, [10161]);
        assert!(numbers("BTL/SKL/BSKL_SWORD_00.VOL").contains(&10132) && !system.contains(&10142));
    }
}
