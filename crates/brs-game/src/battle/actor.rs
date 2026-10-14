//! The player in battle: a port of the original's `BtPcActor` and of her basic action
//! states. Each part names the address of the code it follows in the executable, and
//! `docs/BATTLE_NOTES.md` has the longer account.
//!
//! One [`PcActor::step`] is one of the original's logic frames, in its order: the heat
//! gauge cools, the current state runs, she moves, and only then is the pad read to
//! pick the next state, which starts on the following step.

use std::f32::consts::PI;
use std::sync::Arc;

use bevy::prelude::*;
use brs_engine::animation::{Animator, Clip};
use brs_engine::model::Skeleton;

use super::auto::{self, Answer, Autos, Happens, Her};
use super::damage::{damage, status_factors, Assailant, Random};
use super::log::{Line, Names};
use super::numbers;
use super::shake;
use super::skills::{self, Blow, Firing, Placing, Slots, Stagings, Step};
use super::status::{self, kind, Effect, Effects, Giver, Status, Taker};
use super::tables::{Attack, PlayerStats};
use super::tunables::{id, Tunables};
use super::Stage;
use crate::controls::{button, Pad};
use crate::items::Item;

/// What stops her from starting an action. States set these as they begin and clear
/// them as they let go (the flags at 0x750-0x778 of the original's actor).
pub mod lock {
    /// A state is running: do not fall back to standing.
    pub const BUSY: u32 = 1 << 0;
    /// She is in the middle of a dodge.
    pub const EVADING: u32 = 1 << 1;
    pub const DODGE: u32 = 1 << 2;
    pub const GUARD: u32 = 1 << 3;
    pub const SHOOT: u32 = 1 << 4;
    /// Overheating may not interrupt what she is doing yet.
    pub const OVERHEAT: u32 = 1 << 5;
    pub const SKILLS: u32 = 1 << 6;
    pub const ITEMS: u32 = 1 << 7;
    /// The heat gauge does not cool.
    pub const HEAT_RECOVERY: u32 = 1 << 8;

    /// What every action state takes for itself while it runs.
    pub const ACTIONS: u32 = DODGE | GUARD | SHOOT | OVERHEAT | SKILLS;
}

/// The name the battle's log has for her (0x99638): the three letters, in the wide
/// forms the game's font has them in.
pub const NAME: &str = "\u{ff22}\u{ff32}\u{ff33}";

// Ids of the nodes of her model that the battle code looks up.
/// The two nodes that bend her upper body towards what she aims at.
const AIM_NODES: [u8; 2] = [1, 2];
pub const MUZZLE_NODE: u8 = 0x32;
/// Where the cannon is attached.
pub const WEAPON_NODE: u8 = 0x50;

/// The line of the log that says a snipe has nothing to lock on to (0x7FA04), and
/// how long the window to fire is open, in sixtieths (0x7FFF8).
const SNIPE_FAILED_LINE: i32 = 8;
const SNIPE_WINDOW: f32 = 120.0;

/// Height above her feet from which her aim is measured (0x993B8).
const AIM_HEIGHT: f32 = 100.0;

/// Distance a bullet covers per sixtieth of a second (0x98D5C).
const BULLET_SPEED: f32 = 150.0;

/// Distance she covers per sixtieth of a second when dodging (set at 0x99B60).
const DODGE_SPEED: f32 = 45.0;
/// With this far left to go a dodge starts to brake and she can act again (0x7EA24).
const DODGE_BRAKE_DISTANCE: f32 = 200.0;
/// Share of her speed lost per sixtieth of a second while braking (0x7EA5C).
const DODGE_BRAKE: f32 = 0.15;
/// Below this a component of her velocity is taken as zero (0x991C4).
const STOPPED: f32 = 0.1;

// Frames of her animations that the states jump to, hold, and blend over.
const DODGE_BLEND: f32 = 8.0;
const STAND_BLEND: f32 = 16.0;
/// The shooting animation's aimed pose, and the end of one shot's recoil.
const SHOOT_AIMED: f32 = 38.0;
const SHOOT_RECOILED: f32 = 46.0;
/// Where a hit reaction waits for the hit stop to pass.
const HIT_HOLD: f32 = 6.0;
/// Where the guard animation picks up when the guard is let go.
const GUARD_RELEASE: f32 = 22.0;
/// Frames over which she stands down after a battle that ends without a pose, and
/// after a skill (0x80B80).
const STAND_DOWN_BLEND: f32 = 8.0;
/// The sixtieths over which what is behind her dims for a skill, and comes back
/// (0x8213C, 0x82310).
const DIM_TIME: f32 = 20.0;
/// The strength at which nothing of what is behind her is left (0x753C4).
const BLACK: i32 = 255;

// How long her last states take before the battle moves on, in sixtieths of a second.
/// Standing down, and the victory pose (0x789E4, 0x78BDC).
const STAND_DOWN_TIME: f32 = 20.0;
const VICTORY_TIME: f32 = 60.0;
/// Falling, before the battle counts as lost (0x78E08).
const DEFEAT_TIME: f32 = 180.0;

/// What she says as she falls; `brs_0200.at3` instead once flag 6014 of the save
/// data is set (0x974AC).
const VOICE_DEFEAT: &str = "brs_0201.at3";

// Sounds and effects, by their numbers in the game's sound bank and effect packs.
const SOUND_DODGE: u32 = 0x78;
const SOUND_SHOT: u32 = 0xa0;
const EFFECT_OVERHEAT: u32 = 700_001;
const EFFECT_MUZZLE_FLASH: u32 = 999_004;
const EFFECT_DODGE_END: u32 = 608_005;
/// The node of her model that effect is left by.
const DODGE_END_NODE: u8 = 0x3c;
/// Dust kicked up at the start of a dodge: one for each direction, one for both, and
/// one that travels with her.
const EFFECT_DODGE_BACK: u32 = 608_006;
const EFFECT_DODGE_START: u32 = 608_007;
const EFFECT_DODGE_OUT: u32 = 608_008;
const EFFECT_DODGE_TRAIL: u32 = 608_010;

/// Her battle animations (`SYSTEM.VOL/pack.vol/chr_p_brs02_btlNNNN.anm`).
pub struct Clips {
    /// `btl0000`: standing ready.
    pub idle: Arc<Clip>,
    /// `btl2020` and `btl2021`: dodging to the second standing place and back.
    pub dodge_out: Arc<Clip>,
    pub dodge_back: Arc<Clip>,
    /// `btl3000`: aiming and firing.
    pub shoot: Arc<Clip>,
    /// `btl5000_all` and `btl5001_all`: the two hit reactions.
    pub hit: [Arc<Clip>; 2],
    /// `btl6000_all`: guarding.
    pub guard: Arc<Clip>,
    /// `btl8010_all`: the victory pose.
    pub victory: Arc<Clip>,
    /// `btl8020_all`: falling in defeat.
    pub defeat: Arc<Clip>,
}

/// The staged moments that come with a camera move of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Staged {
    /// `chr_p_brs02_btl8010_cam.cam`.
    Victory,
    /// `chr_p_brs02_btl8020_cam.cam`.
    Defeat,
    /// The move of the skill with this number, of its archive, and the second
    /// move of one whose scene has two.
    Skill(i32),
    SkillSecond(i32),
}

/// Who else is to be seen while she has the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Nobody,
    /// The one enemy a skill is for (0x778C0).
    Only(Entity),
    /// Everybody again, held as they still are (0x746D8).
    Everybody,
}

/// What the cursor is locked on to, as a skill that is for one enemy sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockOn {
    pub enemy: Entity,
    /// Which of the enemy's gauges the lock point is of.
    pub gauge: usize,
    /// Whether the enemy has a body sphere in use (0x97FC0): one that has none is
    /// nothing to use such a skill on.
    pub open: bool,
}

/// The flag that lets her dodge (0x97490): the story's opening sets it for its
/// lesson, and what the first of the records is worth sets it for good.
pub const DODGE_FLAG: i32 = 7500;

/// What does not depend on the player: whether the battle is under way, and what the
/// story has unlocked.
#[derive(Clone, Copy)]
pub struct Rules {
    /// The battle has started and is not over or paused.
    pub running: bool,
    /// She can dodge: the flag that lets her is set.
    pub dodge_unlocked: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { running: true, dodge_unlocked: true }
    }
}

/// What a step did that the rest of the battle has to carry out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Sound(u32),
    /// An effect from the game's effect packs (0x7D0E8), put somewhere; `on_top`
    /// if it is of the scene's last draw list, which is drawn over all else
    /// (0x7D28C with 4).
    Effect { id: u32, place: Place, on_top: bool },
    /// A bullet leaves the muzzle; the velocity is per sixtieth of a second.
    Shot { from: Vec3, velocity: Vec3 },
    /// A staged moment begins: its camera move plays from where she stands.
    Camera(Staged),
    /// A staged moment that is not her last is over: the camera is the one over
    /// her shoulder again (0xA7E58).
    CameraBack,
    /// What is behind her dims to so much of 255, over so many sixtieths (0x85814).
    Dim { to: i32, time: f32 },
    /// The camera sees from so near and so far, from here on (0xA7CAC).
    Lens { near: f32, far: f32 },
    /// One of her voice lines, by its file's name.
    Voice(&'static str),
    /// A line of hers and a sound of the archive of a skill.
    SkillVoice { skill: i32, name: &'static str },
    SkillSound { skill: i32, sound: u32 },
    /// A skill gives her a status effect (0x80E7C).
    Give(i32),
    /// The bullet of a skill leaves (0x766B4, 0x7ABB8).
    SkillShot { skill: i32, firing: Firing },
    /// Her hands change by one of her model's part modes (0x9B608).
    Parts(u8),
    /// The weapon of a skill's archive is hers to hold, at this node of her model
    /// (0x7AB48).
    Weapon { skill: i32, node: u8 },
    /// A skill strikes the enemies outright: all of them (0x74EF0), the one it
    /// is for (0x778F8), or a part of that one (0x7F7B4).
    Strike { skill: i32, blow: Blow },
    /// The scope of a snipe comes over the lock point her skill is for, if it has
    /// one (0x7F86C), and goes (0x7F964); and its sight starts to lock on
    /// (0x35D5C).
    Scope(Option<(Entity, usize)>),
    ScopeOff,
    SightLock,
    /// Her model is not drawn, or is again (her + 0x73C).
    Hide(bool),
    /// A ray of a skill's volley leaves, at where she aims now (0x7C158).
    SkillRay { skill: i32 },
    /// The cursor is not drawn to lock points it is not on (0xA15E4), or is again
    /// (0xA15F8).
    CursorFree(bool),
    /// The scene of a skill makes an effect (0x7488C), has every effect of a
    /// number be no more at once (0x74B5C), or has those of a number be of
    /// another draw list (0x749E0).
    SceneEffect(skills::Made),
    EffectsOver(u32),
    EffectList(u32, i32),
    /// She has fallen, and the battle is lost.
    Lost,
}

/// Where an effect of hers is put.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// On a node of her model, and it goes with the node (0x7D1D4).
    Node(u8),
    /// On her model whole, moved and turned so from it, and it goes with her
    /// (0x7D1D4 with no node, 0x7D23C).
    Actor(Vec3, Quat),
    /// Where a node of her model is as the effect begins, and it stays there
    /// (0x7D154).
    WhereNode(u8),
    /// At a place, turned so, and it stays there (0x7D23C).
    At(Vec3, Quat),
}

impl Place {
    /// On her whole as she is.
    pub const HER: Place = Place::Actor(Vec3::ZERO, Quat::IDENTITY);
}

/// Everything a step reads and writes besides the actor itself.
pub struct Context<'a> {
    /// Length of the step in sixtieths of a second.
    pub dt: f32,
    pub pad: &'a Pad,
    pub tunables: &'a Tunables,
    pub stage: &'a Stage,
    pub rules: Rules,
    pub clips: &'a Clips,
    pub animator: &'a mut Animator,
    pub skeleton: &'a Skeleton,
    /// Where the aim cursor points in the world, if at anything.
    pub aim_point: Option<Vec3>,
    /// What the cursor is locked on to, if anything; and of each enemy where its
    /// model stands and whether it has a body sphere in use.
    pub lock: Option<LockOn>,
    pub places: &'a [(Entity, Vec3, bool)],
    /// Her slots, and what their skills are played with.
    pub slots: &'a mut Slots,
    pub stagings: &'a Stagings,
    /// The battle's chance, which her skills that act of themselves go by.
    pub random: &'a mut Random,
    pub events: &'a mut Vec<Event>,
}

/// The heat gauge.
#[derive(Clone, Copy, Debug)]
pub struct Heat {
    pub max: f32,
    pub value: f32,
    /// Time left before it starts to cool.
    pub wait: f32,
    /// Set when the gauge fills; she can do nothing until it has emptied again.
    pub overheated: bool,
}

impl Heat {
    /// Adds heat and restarts the wait before cooling (0x97658). `spared` is a status
    /// effect that lets nothing add heat, under which none of this happens.
    pub fn add(&mut self, amount: f32, spared: bool, tunables: &Tunables) {
        if spared {
            return;
        }
        if !tunables.flag(id::NO_OVERHEAT) {
            self.value = (self.value + amount.abs()).min(self.max);
        }
        if self.value >= self.max {
            self.overheated = true;
        }
        self.wait = tunables.float(id::HEAT_RECOVERY_WAIT);
    }

    /// All of it is gone at once, and her overheating with it (0x978A8).
    pub fn cool(&mut self) {
        (self.value, self.overheated) = (0.0, false);
    }

    /// Cools the gauge over `dt` once the wait has passed (0x974CC).
    pub fn recover(&mut self, dt: f32, tunables: &Tunables) {
        if self.wait > 0.0 {
            self.wait = (self.wait - dt).max(0.0);
            if self.wait > 0.0 {
                return;
            }
        }
        if self.value <= 0.0 {
            return;
        }
        let per_second = if self.overheated { id::HEAT_RECOVERY_OVERHEATED } else { id::HEAT_RECOVERY };
        self.value = (self.value - dt * (tunables.float(per_second).abs() / 60.0)).max(0.0);
        if self.value <= 0.0 {
            self.overheated = false;
        }
    }

    /// How hot the gauge shows: 0 to 2 by thirds, 3 when overheated (0x97770).
    pub fn level(&self) -> u32 {
        let filled = self.value / self.max;
        match () {
            _ if self.overheated => 3,
            _ if filled <= 0.33 => 0,
            _ if filled <= 0.66 => 1,
            _ => 2,
        }
    }

    /// Which of the five burst stages the heat puts her at, counted from 0: the hotter,
    /// the fewer bullets a burst has (0x99644).
    fn burst_stage(&self, tunables: &Tunables) -> usize {
        let heat = self.value as i32;
        id::BURST_STAGES.iter().position(|&threshold| heat < tunables.int(threshold)).unwrap_or(id::BURST_STAGES.len())
    }
}

/// What she is doing, as the original numbers it (its field 0x744).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Defeated,
    /// The battle is won: her pose, or standing down.
    Finish,
    Idle,
    Shoot,
    AimIdle,
    /// A skill of hers (6).
    Skill,
    /// One of her skills that shoot, which are numbered apart (7).
    SkillShot,
    Guard,
    Dodge,
    /// Reeling from a hit.
    Hit,
    Overheat,
}

/// The running state and what it keeps between steps.
#[derive(Clone, Copy, Debug)]
enum State {
    Idle { phase: u32 },
    AimIdle { phase: u32 },
    Overheat { phase: u32 },
    Hit { phase: u32, reaction: usize },
    Guard { phase: u32 },
    /// `back`: returning to the first standing place. `free`: a skill waives the heat.
    Dodge { phase: u32, back: bool, free: bool },
    Shoot { phase: u32, bullets: i32, interval: f32, timer: f32, free: bool },
    /// `time`: how long the state has run, in sixtieths of a second.
    Finish { phase: u32, pose: bool, time: f32 },
    Defeated { phase: u32, time: f32 },
    /// A skill of her slots: its number, the line of the log that names it, the
    /// status effects it gives, the enemy it is for if it is for one, with the
    /// gauge of it her lock point was of (its + 0x9C), and what it counts.
    Skill { phase: u32, id: i32, line: i32, effects: [i32; 3], target: Option<(Entity, usize)>, clock: SkillClock },
}

/// What the state of a skill counts as it runs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SkillClock {
    /// The state's clock, in sixtieths: from its first step, or from when a step
    /// last had it start over.
    time: f32,
    /// How many are left of the volley it fires, if it fires one (its + 0x98).
    shots: i32,
    /// A snipe has been fired (its + 0x9C).
    fired: bool,
}

#[derive(Component)]
pub struct PcActor {
    pub max_hp: f32,
    pub hp: f32,
    /// The damage she has taken in this battle (the root's + 0x9C), all told.
    pub damage_taken: i32,
    /// Something has taken HP off her in this battle, however little (the root's
    /// flag 7): a battle she comes through without is one for the records.
    pub harmed: bool,
    /// The battle is decided (the root's + 0x88): from then on nothing takes HP off
    /// her (0x98C3C).
    pub spared: bool,
    /// Nothing gives her HP back: the mission is "Survival Limit" (0x97418).
    pub unhealed: bool,
    /// She has asked for her items (L, 0x9A534): the battle's item menu takes it.
    pub items_asked: bool,
    /// Set when a state ends with no HP left; the next step then starts her defeat
    /// (the original clears its field 0x748, and the defeat state sets it again).
    fallen: bool,
    /// A staged moment of hers is playing: everyone else is frozen and out of sight,
    /// and nothing can hit her (0x787D4).
    pub staged: bool,
    /// Who of the others is to be seen while she has the stage: held, and not
    /// hidden.
    pub shown: Shown,
    /// The scope of a snipe is up (her + 0x7F0): the cursor is held where it is
    /// (0x9A018).
    pub scoped: bool,
    /// Where her model stands for a scene, if not where she is (0x7720C): her own
    /// update leaves the model be while she has the stage (her + 0x7FC), and
    /// puts it back where she is as the stage is given back (0x99950).
    pub placed: Option<Transform>,
    /// Her last state of a won battle has run its course (0x774).
    pub finished: bool,
    /// Bits of [`lock`].
    pub locks: u32,
    pub heat: Heat,
    /// Which of the stage's two standing places she is at, or on her way to.
    pub slot: u32,
    pub position: Vec3,
    /// The way she faces: a level unit vector.
    pub direction: Vec3,
    /// A dodge is carrying her along.
    moving: bool,
    /// Per sixtieth of a second.
    velocity: Vec3,
    /// Share of the velocity lost per sixtieth of a second; zero while not braking.
    brake: f32,
    distance_left: f32,
    /// Her upper body follows the aim cursor.
    pub aiming: bool,
    /// Attacks that hit her are guarded.
    pub guarding: bool,
    /// Set by a guarded hit, for the guard state to react to.
    pub guard_hit: bool,
    /// Time left of the pause a hit causes; zero when there is none.
    pub hit_stop: f32,
    /// How strong an attack has to be to make her reel (the original's field 0x708).
    pub poise: i32,
    /// The status effects on her (the original's block at 0x1E8).
    pub status: Status,
    /// Her skills that act of themselves (the list at 0x1712E8), and the frames
    /// of hers that have gone by, which one of them counts in (0xA6E28).
    pub autos: Autos,
    frame: u32,
    pub action: Action,
    state: Option<State>,
    /// The numbers there are to put up for her, until the battle takes them:
    /// what each is of, how much, and where, or by her own place with none
    /// (0x98B30).
    pub numbers: Vec<(numbers::Kind, i32, Option<Vec3>)>,
    /// The shakes of the camera she has asked for, until the battle takes them.
    pub jolts: Vec<shake::Asked>,
    /// The numbers of the effects that are to be no more as a state of hers
    /// ended, until the battle takes them.
    pub effects_over: Vec<u32>,
}

/// What an enemy's hit did to her.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitOutcome {
    pub damage: i32,
    pub guarded: bool,
    pub reeled: bool,
    pub defeated: bool,
    /// HP the attack gives back to whoever dealt it.
    pub drained: i32,
    /// The status effect the attack carries, and what became of it.
    pub effect: Option<(i32, status::Outcome)>,
    /// A skill of hers acted on the guard the hit landed on.
    pub guard_acted: bool,
}

/// The effect of her guard taking a hit, which goes with her, and where it is from
/// her (0x9B3F0: 0x7B4A9; 0x9B430). It is of the system's own file.
pub const EFFECT_GUARD: u32 = 505_001;
pub const GUARD_EFFECT_AT: Vec3 = Vec3::new(0.0, 75.0, 50.0);
/// The effect a hit on her is seen as, by the look its attack has (0x97E90), and the
/// look a guarded hit has in place of each (0x97E10).
const HIT_EFFECTS: [u32; 11] = [0, 999_003, 999_024, 999_008, 998_024, 999_029, 998_023, 105_051, 0, 0, 0];
const GUARDED_LOOKS: [usize; 11] = [0, 0, 1, 3, 0, 4, 5, 7, 8, 9, 4];
/// What is seen in place of that when an ailment the attack carries takes: poison,
/// a stun, any other (0x9B3A8).
const EFFECT_POISONED: u32 = 998_025;
const EFFECT_STUNNED: u32 = 998_026;
const EFFECT_AILING: u32 = 998_027;

/// The effect that is seen where a hit lands on her, over everything (0x9B300 to
/// 0x9B4F8): that of an ailment the attack has just given her, or else the look of
/// the attack, which is another's when the hit was guarded.
pub fn seen_of_hit(attack: &Attack, outcome: &HitOutcome, effects: &Effects) -> Option<u32> {
    let ailment = match outcome.effect {
        Some((id, status::Outcome::Given)) => effects.get(id).and_then(|effect| match effect.kind {
            kind::POISON => Some(EFFECT_POISONED),
            kind::STUN => Some(EFFECT_STUNNED),
            kind::NO_SHOT..=kind::LAST_AILMENT => Some(EFFECT_AILING),
            _ => None,
        }),
        _ => None,
    };
    ailment.or_else(|| {
        let look = usize::try_from(attack.look).ok();
        let look = if outcome.guarded { look.and_then(|look| GUARDED_LOOKS.get(look).copied()) } else { look };
        look.and_then(|look| HIT_EFFECTS.get(look).copied()).filter(|id| *id > 0)
    })
}

/// The rotation that turns +Z to `forward` and keeps X level, as the original builds
/// its facing matrices (0xA8340): X is up cross forward, Y is forward cross X.
pub fn facing(forward: Vec3) -> Quat {
    let z = forward.normalize_or_zero();
    let x = Vec3::Y.cross(z).normalize_or_zero();
    if x == Vec3::ZERO {
        return Quat::IDENTITY;
    }
    Quat::from_mat3(&Mat3::from_cols(x, z.cross(x), z))
}

impl PcActor {
    /// The actor as a battle starts: at the first standing place, facing the middle,
    /// with a cold gauge (0x999E8).
    pub fn new(max_hp: f32, hp: f32, stage: &Stage, tunables: &Tunables) -> PcActor {
        let position = stage.slot(0);
        PcActor {
            max_hp,
            hp,
            damage_taken: 0,
            harmed: false,
            spared: false,
            unhealed: false,
            items_asked: false,
            fallen: false,
            staged: false,
            shown: Shown::Nobody,
            scoped: false,
            placed: None,
            finished: false,
            // Everything is locked until her first state lets go.
            locks: lock::DODGE | lock::GUARD | lock::SHOOT | lock::EVADING | lock::OVERHEAT | lock::ITEMS | lock::HEAT_RECOVERY,
            heat: Heat { max: tunables.float(id::HEAT_MAX), value: 0.0, wait: 0.0, overheated: false },
            slot: 0,
            position,
            direction: (Stage::CENTRE - position).normalize(),
            moving: false,
            velocity: Vec3::ZERO,
            brake: 0.0,
            distance_left: 0.0,
            aiming: false,
            guarding: false,
            guard_hit: false,
            hit_stop: 0.0,
            poise: 1,
            status: Status::of(NAME, status::HER_ROOM),
            autos: Autos::default(),
            frame: 0,
            action: Action::None,
            state: None,
            numbers: Vec::new(),
            jolts: Vec::new(),
            effects_over: Vec::new(),
        }
    }

    /// Has her skills that act of themselves look at something that has happened
    /// (0x9FF58).
    pub fn ask(&mut self, what: Happens, random: &mut Random) -> Answer {
        let her = Her { hp: self.hp, max_hp: self.max_hp, frame: self.frame };
        self.autos.ask(what, her, random)
    }

    /// Adds heat, unless a status effect spares her it (0x97658).
    fn warm(&mut self, amount: f32, tunables: &Tunables) {
        self.heat.add(amount, self.status.has(kind::NO_HEAT), tunables);
    }

    /// Gives her HP back, up to what she can have, and puts the number up by her
    /// (0x98B8C). In the mission "Survival Limit" (the save's variable 10004 is
    /// 66, 0x97418) nothing is given and nothing put up: what gave it, an item
    /// used up, a skill's sound and look, is as it would be.
    pub fn heal(&mut self, amount: i32) {
        if self.unhealed {
            return;
        }
        if amount > 0 {
            self.numbers.push((numbers::Kind::Heal, amount, None));
        }
        self.hp = (self.hp + amount as f32).min(self.max_hp);
    }

    /// Takes damage off her HP, down to none, and counts it (0x98C24). Once the
    /// battle is decided nothing is taken, and the answer is that.
    fn hurt(&mut self, damage: i32) -> bool {
        if self.spared {
            return false;
        }
        self.hp = (self.hp - damage as f32).max(0.0);
        self.damage_taken += damage;
        self.harmed = true;
        true
    }

    /// Puts up what a hit took off her, where it landed (0x98CB4): the number of
    /// a guarded hit if she guarded it and stands. Nothing while nothing can be
    /// taken off her.
    pub fn show_hit(&mut self, outcome: &HitOutcome, at: Vec3) {
        if !self.spared {
            let kind = if outcome.guarded && !outcome.defeated { numbers::Kind::Guarded } else { numbers::Kind::Damage };
            self.numbers.push((kind, outcome.damage, Some(at)));
        }
    }

    /// She uses an item she chose from the battle's menu (0x981BC). One that gives
    /// HP back gives so much and so many hundredths of all she can have, half as
    /// much again if a skill of hers that acts of itself has it so; any other
    /// gives her its status effect as she would give one herself. What is to be
    /// heard and seen of it is handed back: sounds of the battle's bank, and
    /// effects on her (true) or where she stands.
    pub fn use_item(&mut self, item: &Item, effects: &Effects, stats: &PlayerStats, tunables: &Tunables, slots: Option<&mut Slots>, random: &mut Random) -> (Vec<u32>, Vec<(u32, bool)>) {
        let (mut heard, mut seen) = (Vec::new(), Vec::new());
        if self.ask(Happens::ItemBegins, random).acted > 0 {
            heard.push(auto::SOUND);
            seen.push((auto::EFFECT, true));
        }
        let (flat, hundredths) = item.gives;
        if flat != 0 || hundredths != 0 {
            let mut amount = (flat as f32 + self.max_hp * hundredths as f32 / 100.0) as i32;
            if let Some(of_it) = self.ask(Happens::ItemGives, random).item_hundredths {
                amount += amount * of_it / 100 - amount;
            }
            heard.push(auto::SOUND);
            seen.push((auto::EFFECT_HEALS, false));
            self.heal(amount);
        } else if let Some(effect) = effects.get(item.effect).filter(|_| item.effect != 0) {
            let giver = Giver { level: stats.level, special: stats.special, boost: self.status.boost(), name: NAME.to_owned() };
            match self.afflict(effect, &giver, stats, random) {
                status::Outcome::Charged(which) => {
                    // The slots as the save keeps them: triangle, circle, cross, square.
                    let filled = slots.map_or(0, |slots| [(2, 0), (4, 1), (8, 2), (1, 3)].into_iter().filter(|&(bit, slot)| which & bit != 0 && slots.fill(slot)).count());
                    heard.extend(std::iter::repeat_n(skills::SOUND_READY, filled));
                }
                status::Outcome::Cooled => self.heat.cool(),
                _ => {}
            }
        }
        self.warm(tunables.float(id::HEAT_ITEM), tunables);
        self.ask(Happens::ItemEnds, random);
        (heard, seen)
    }

    /// Gives her a status effect by someone's doing other than a hit: an enemy's
    /// script (0xC02D8). What it came to is handed back.
    pub fn afflict(&mut self, effect: &Effect, giver: &Giver, stats: &PlayerStats, random: &mut Random) -> status::Outcome {
        let taker = Taker { level: stats.level, resistance: 0, max_hp: self.max_hp as i32 };
        let outcome = self.status.give(effect, giver, &taker, random);
        if let status::Outcome::Healed(amount) = outcome {
            self.heal(amount);
        }
        outcome
    }

    /// Where her model stands and which way it faces (0x992DC): where she is,
    /// unless a scene has put it somewhere else.
    pub fn transform(&self) -> Transform {
        self.placed.unwrap_or(Transform { translation: self.position, rotation: facing(self.direction), scale: Vec3::ONE })
    }

    /// Whether she can start anything at all (0x98EE8).
    pub fn can_act(&self, rules: Rules) -> bool {
        rules.running && self.hp > 0.0
    }

    /// One logic frame (0x99E50). The animator must already have advanced by the step.
    pub fn step(&mut self, ctx: &mut Context) {
        self.frame += 1;
        // A hit's pause runs down (0x742D8).
        self.hit_stop = (self.hit_stop - ctx.dt).max(0.0);
        // None of this while somebody has the stage (0x99EF8: the root's flag 9),
        // herself for a skill's scene included: nothing of hers runs down or out
        // in it.
        if ctx.rules.running && self.hp > 0.0 && !self.staged {
            if self.locks & lock::HEAT_RECOVERY == 0 {
                self.heat.recover(ctx.dt, ctx.tunables);
            }
            // Her status effects run on (0x99F54): each one's time, then what a
            // regeneration gives and what a poison takes. A poison cannot quite
            // kill: it leaves her one HP.
            self.status.tick();
            let most = self.max_hp as i32;
            if let Some(back) = self.status.regeneration(ctx.dt, most) {
                self.heal(back);
            }
            if let Some(off) = self.status.poison(ctx.dt, most) {
                // What it takes off is put up by her (0x99FA0).
                if self.hurt(off) {
                    self.numbers.push((numbers::Kind::Damage, off, None));
                }
                if self.hp <= 0.0 {
                    self.hp = 1.0;
                }
            }
            // Her skills that act of themselves look at the frame (0x99FD4): one
            // that acts on how she is gives her its effect.
            let answer = self.ask(Happens::Frame, ctx.random);
            ctx.events.extend(answer.gives.iter().copied().map(Event::Give));
            show_acting(&answer, ctx.events);
        }

        // She always has a state; the first one is standing, entered without being
        // counted as an action (0x9936C).
        let state = self.state.take().unwrap_or_else(|| self.enter_idle());
        self.state = Some(self.run(state, ctx));
        self.travel(ctx.dt);
        self.choose(ctx);

        // The bend towards the aim is dropped as soon as she stops aiming (0x9A184).
        if !self.aiming {
            ctx.animator.clear_twists();
        }
    }

    /// An enemy's attack lands on her (0x9AD90): the damage comes off, less through a
    /// guard, and unless she guarded it or is in the middle of a dodge she reels if
    /// the attack is strong enough. Then the status effect the attack carries, if
    /// it has one, takes or does not.
    ///
    /// Her guard has a record of its own among the status effects, which says how
    /// much more than guarding itself it keeps off her and what guarding a hit
    /// with it gives her: the usual one neither. One of her skills that act of
    /// themselves can have it be another's for a hit.
    pub fn take_hit(
        &mut self,
        attack: &Attack,
        by: Assailant,
        stats: &PlayerStats,
        effects: &Effects,
        tunables: &Tunables,
        random: &mut Random,
    ) -> HitOutcome {
        // A stun ends with the hit (0x8F420).
        self.status.struck();
        // A ward makes every hit a guarded one. An attack that cannot be guarded is
        // not, and nor is one whose effect breaks guards (0x9ADFC-0x9AE44).
        let effect = effects.get(attack.effect);
        let mut guarded = (self.guarding || self.status.has(kind::WARD)) && !attack.unguardable;
        if guarded && effect.is_some_and(Effect::breaks_guard) {
            guarded = false;
            self.status.heard.push(super::enemy::actor::SOUND_BREAK as u32);
        }
        // The record of her guard, while she holds it up, whatever the attack
        // (0x9AE8C): her skills are asked as the hit lands on it, and then, guard
        // or none, whether her guard is another than the usual (0x9AEF0).
        let mut shield = None;
        let mut guard_acted = false;
        debug!("a hit lands on her on frame {}: guarding {}", self.frame, self.guarding);
        if self.guarding {
            shield = effects.get(auto::USUAL_GUARD).and_then(Effect::guard);
            if self.ask(Happens::GuardedHit, random).acted > 0 {
                self.status.heard.push(auto::SOUND);
                guard_acted = true;
            }
        }
        if let Some(record) = self.ask(Happens::WhichGuard, random).guard {
            shield = effects.get(record).and_then(Effect::guard).or(shield);
            info!("a skill of hers has her guard be that of status effect {record}: {shield:?}");
        }
        // What guarding a hit with it gives her, if that is HP back (0x9AF50): she
        // is both who gives it and who is given it, before the hit takes anything.
        if let Some(gift) = shield.and_then(|(_, gift)| effects.get(gift)).filter(|gift| gift.kind == kind::HEAL) {
            let giver = Giver { level: stats.level, special: stats.special, boost: self.status.boost(), name: NAME.to_owned() };
            self.afflict(gift, &giver, stats, random);
        }
        let mut factors = status_factors(by.status, &self.status).to_vec();
        if let Some((kept_off, _)) = shield {
            factors.push((100 - kept_off) as f32 / 100.0);
        }
        if guarded {
            factors.push(tunables.float(id::GUARD_PERCENT) / 100.0);
        }
        let dealt = damage(attack, by.figures, stats.defence, &factors, random);
        self.hurt(dealt);
        let defeated = self.hp <= 0.0;
        if defeated {
            guarded = false;
        }

        let reels = defeated || (!guarded && (attack.unguardable || self.poise - attack.strength <= 0));
        // The hit stop is given in logic frames; a negative one is none (0x129178).
        let hit_stop = if reels { attack.hit_stop.max(0) as f32 * 2.0 } else { 0.0 };
        if guarded {
            self.warm(tunables.int(id::GUARDED_HIT_HEAT) as f32, tunables);
            self.guard_hit = true;
        } else if reels && self.locks & lock::EVADING == 0 {
            self.hit(usize::from(attack.reaction == 1), hit_stop);
        } else if reels {
            self.hit_stop = hit_stop;
        }

        // What the attack carries (0x9B300), guarded or not, unless it felled her:
        // a share of the damage back to whoever dealt it, and an effect for her.
        // The camera shakes as the attack has it (0x9B3E0).
        self.jolts.push(shake::Asked::Attack(attack.shake));
        let mut outcome = HitOutcome { damage: dealt, guarded, reeled: reels && !guarded, defeated, drained: 0, effect: None, guard_acted };
        if let (Some(effect), false) = (effect, defeated) {
            outcome.drained = (dealt * effect.drain() / 100).max(0);
            let giver = Giver { level: by.level, special: by.figures.special, boost: by.status.boost(), name: by.status.bearer().to_owned() };
            outcome.effect = Some((effect.id, self.afflict(effect, &giver, stats, random)));
        }
        outcome
    }

    /// A hit that was not guarded: she reels (state built at 0x7D83C).
    pub fn hit(&mut self, reaction: usize, hit_stop: f32) {
        self.action = Action::Hit;
        self.hit_stop = hit_stop;
        self.aiming = false;
        self.locks |= lock::BUSY | lock::ACTIONS;
        self.locks &= !lock::HEAT_RECOVERY;
        self.begin(State::Hit { phase: 0, reaction: reaction.min(1) });
    }

    /// The battle is won, and whatever she was doing gives way to her last state
    /// (0x9996C): with `pose` her victory pose, otherwise she just stands down.
    pub fn finish_battle(&mut self, pose: bool) {
        self.finished = false;
        self.action = Action::Finish;
        self.locks |= lock::BUSY | lock::ACTIONS | lock::ITEMS;
        self.begin(State::Finish { phase: 0, pose, time: 0.0 });
    }

    /// Her state gives way to another (0x98E84, 0xA28A4): what the old one does as
    /// it ends is done first. A skill that helps her has a boost of hers spent
    /// (0x80D2C), unless it is one that gives that boost.
    fn begin(&mut self, state: State) {
        self.end_state();
        self.state = Some(state);
    }

    /// What her state does as it is done with (0xA28A4), which is before the one
    /// that follows it is made (0x9AC4C, 0x9AB84, 0x9AA54). A guard, a burst
    /// and a dodge tell her skills that act of themselves that they are over
    /// (0x7D9F4, 0x7DFF8, 0x7DC0C).
    fn end_state(&mut self) {
        match self.state.take() {
            // The effects of a skill's scene that are not to outlast it are no
            // more (0x767D8), and a boost is spent (0x80D2C).
            Some(State::Skill { id, .. }) => {
                if let Some(scene) = skills::scene(id) {
                    self.effects_over.extend(scene.ends);
                    if scene.spends_boost {
                        self.status.spend_boost();
                    }
                }
            }
            Some(State::Guard { .. }) => self.autos.ends(Happens::GuardEnds),
            Some(State::Shoot { .. }) => self.autos.ends(Happens::ShootEnds),
            Some(State::Dodge { .. }) => self.autos.ends(Happens::DodgeEnds),
            _ => {}
        }
    }

    // --- The states. Each `enter_` is the original's constructor, run when the pad is
    // read; the matching arm of `run` is its update, first run on the next step.

    fn run(&mut self, state: State, ctx: &mut Context) -> State {
        match state {
            State::Idle { phase } => State::Idle { phase: self.run_idle(phase, ctx) },
            State::AimIdle { phase } => State::AimIdle { phase: self.run_aim_idle(phase) },
            State::Overheat { phase } => State::Overheat { phase: self.run_overheat(phase, ctx) },
            State::Hit { phase, reaction } => State::Hit { phase: self.run_hit(phase, reaction, ctx), reaction },
            State::Guard { phase } => State::Guard { phase: self.run_guard(phase, ctx) },
            State::Dodge { phase, back, free } => State::Dodge { phase: self.run_dodge(phase, back, free, ctx), back, free },
            State::Shoot { .. } => self.run_shoot(state, ctx),
            // A state's clock starts on its first step and counts the ones after
            // (0x74418).
            State::Finish { phase, pose, time } => {
                let time = if phase == 0 { 0.0 } else { time + ctx.dt };
                State::Finish { phase: self.run_finish(phase, pose, time, ctx), pose, time }
            }
            State::Defeated { phase, time } => {
                let time = if phase == 0 { 0.0 } else { time + ctx.dt };
                State::Defeated { phase: self.run_defeated(phase, time, ctx), time }
            }
            State::Skill { phase, id, line, effects, target, mut clock } => {
                clock.time = if phase == 0 { 0.0 } else { clock.time + ctx.dt };
                let phase = self.run_skill(phase, &mut clock, (id, line, effects, target), ctx);
                State::Skill { phase, id, line, effects, target, clock }
            }
        }
    }

    /// Clears the stage for a moment of her own (0x787D4): she stops aiming, everyone
    /// else freezes out of sight, and nothing can hit her.
    fn take_the_stage(&mut self) {
        self.poise = 1;
        self.aiming = false;
        self.staged = true;
        self.shown = Shown::Nobody;
    }

    /// Gives the stage back after a moment that is not her last (0x80BE4): the
    /// others are let go and seen again, she can be hit, and her model is where
    /// she is again (0x99950).
    fn give_the_stage_back(&mut self) {
        self.staged = false;
        self.shown = Shown::Nobody;
        self.placed = None;
    }

    /// A skill of her slots begins (0x80EC8 and its twins): everything of hers is
    /// locked for it (0x806EC). One that is for an enemy keeps which (0x77A8C; it
    /// also sends that enemy's script signal 103, which none of them answers),
    /// and one that fires a volley how many its bullet's record has to it
    /// (0x7C21C).
    fn enter_skill(&mut self, id: i32, line: i32, effects: [i32; 3], target: Option<(Entity, usize)>, shots: i32) -> State {
        self.locks = lock::BUSY | lock::EVADING | lock::ACTIONS | lock::ITEMS | lock::HEAT_RECOVERY;
        State::Skill { phase: 0, id, line, effects, target, clock: SkillClock { shots, ..default() } }
    }

    /// The state of a skill (0x81E08 and its twins for those that help her, 0x7699C
    /// for Charge Shot): each thing it does waits for the state's clock to have
    /// come to its time, and more than one can come in a frame. The log names the
    /// skill, she takes the stage with a move of the camera and an animation of
    /// the skill's own, what is behind her dims, and things are heard; then the
    /// skill does what it does, all of that is undone, and she stands as before.
    /// `clock` has the state's clock, which a step can have start over.
    fn run_skill(&mut self, phase: u32, clock: &mut SkillClock, (id, line, effects, target): (i32, i32, [i32; 3], Option<(Entity, usize)>), ctx: &mut Context) -> u32 {
        let Some(scene) = skills::scene(id) else { return phase };
        let SkillClock { time, shots, fired } = clock;
        let mut phase = phase as usize;
        // Whether what the skill is for has a body sphere in use (0x7F6DC).
        let open = target.is_some_and(|(enemy, _)| ctx.places.iter().any(|place| place.0 == enemy && place.2));
        // A step that another sends the state back to is not waited for.
        let mut at_once = false;
        while let Some(&(at, step)) = scene.steps.get(phase) {
            if *time < at && !std::mem::take(&mut at_once) {
                break;
            }
            phase += 1;
            match step {
                Step::Restart => *time = 0.0,
                // While any are left a ray leaves at where she aims now, and the
                // wait that follows is counted from here; with none left that
                // wait is passed over (0x7C7D0).
                Step::Volley => {
                    if *shots > 0 {
                        self.aiming = true;
                        self.bend_to_aim(ctx);
                        ctx.events.push(Event::SkillRay { skill: id });
                        *time = 0.0;
                    } else {
                        phase += 1;
                    }
                }
                // One fewer is left, and it is the volley's turn again at once
                // (0x7C81C).
                Step::Reload => {
                    *shots -= 1;
                    phase -= 2;
                    at_once = true;
                }
                Step::CursorFree(free) => ctx.events.push(Event::CursorFree(free)),
                Step::Hide(hidden) => ctx.events.push(Event::Hide(hidden)),
                Step::Scope(true) => {
                    self.scoped = true;
                    ctx.events.push(Event::Scope(target));
                }
                Step::Scope(false) => {
                    self.scoped = false;
                    ctx.events.push(Event::ScopeOff);
                }
                // The three that follow wait on the clock themselves, by the
                // battle's own figures: a step that is not through stays put.
                Step::SightLock => {
                    if *time < ctx.tunables.float(id::SNIPE_LOCK_START) {
                        phase -= 1;
                        break;
                    }
                    if open {
                        ctx.events.push(Event::SightLock);
                    } else {
                        self.status.said.push(Line { message: SNIPE_FAILED_LINE, names: Names::of(NAME) });
                    }
                }
                Step::SightWait => {
                    let locked = ctx.tunables.float(id::SNIPE_LOCK_START) + ctx.tunables.float(id::SNIPE_LOCK_WAIT);
                    if *time < ctx.tunables.float(id::SNIPE_UI_ENTER) || open && *time < locked {
                        phase -= 1;
                        break;
                    }
                    *time = 0.0;
                }
                // Square fires, once: the slot starts over and the part her lock
                // was on is struck, if there is anything to strike. From then,
                // and whenever square is not pressed, the window lasts until its
                // time is up or cross is pressed.
                Step::Trigger => {
                    if !*fired && ctx.pad.pressed & button::SQUARE != 0 {
                        if let (true, Some((enemy, gauge))) = (open, target) {
                            ctx.slots.used_skill(id);
                            ctx.events.push(Event::Strike { skill: id, blow: Blow::Part(enemy, gauge) });
                        }
                        *fired = true;
                        phase -= 1;
                        break;
                    }
                    if *time < SNIPE_WINDOW && ctx.pad.pressed & button::CROSS == 0 {
                        phase -= 1;
                        break;
                    }
                }
                Step::Effect(made) => ctx.events.push(Event::SceneEffect(made)),
                Step::EndEffects(id) => ctx.events.push(Event::EffectsOver(id)),
                Step::EffectList(id, list) => ctx.events.push(Event::EffectList(id, list)),
                Step::LoopAnimation => ctx.animator.body_mut().loop_clip(),
                Step::PlayOut => ctx.animator.body_mut().play_out(),
                Step::Say => {
                    if line > 0 {
                        self.status.said.push(Line { message: line, names: Names::of(NAME) });
                    }
                }
                Step::TakeStage => self.take_the_stage(),
                Step::GiveStageBack => self.give_the_stage_back(),
                // Her model is put at the map's origin, or where the enemy's
                // stands but on the ground, turned half round about the upright.
                Step::Put(placing) => {
                    let at = match placing {
                        Placing::Middle => Some(Vec3::ZERO),
                        Placing::Target => {
                            let place = target.and_then(|(enemy, _)| ctx.places.iter().find(|place| place.0 == enemy));
                            place.map(|&(_, at, _)| Vec3::new(at.x, 0.0, at.z))
                        }
                    };
                    if let Some(at) = at {
                        self.placed = Some(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(PI)));
                    }
                }
                Step::Dim => ctx.events.push(Event::Dim { to: ctx.tunables.int(id::SKILL_DIM), time: DIM_TIME }),
                Step::Undim => ctx.events.push(Event::Dim { to: 0, time: DIM_TIME }),
                Step::DimAtOnce => ctx.events.push(Event::Dim { to: ctx.tunables.int(id::SKILL_DIM), time: 0.0 }),
                Step::Blacken => ctx.events.push(Event::Dim { to: BLACK, time: 0.0 }),
                Step::ShowEnemies => self.shown = Shown::Everybody,
                Step::Strike => ctx.events.push(Event::Strike { skill: id, blow: Blow::All }),
                Step::ShowTarget => {
                    if let Some((enemy, _)) = target {
                        self.shown = Shown::Only(enemy);
                    }
                }
                Step::StrikeTarget => {
                    if let Some((enemy, _)) = target {
                        ctx.events.push(Event::Strike { skill: id, blow: Blow::One(enemy) });
                    }
                }
                Step::StandAtOnce => {
                    ctx.animator.body_mut().set_time(0.0);
                    ctx.animator.body_mut().loop_clip();
                }
                Step::Camera => ctx.events.push(Event::Camera(Staged::Skill(id))),
                Step::SecondCamera => ctx.events.push(Event::Camera(Staged::SkillSecond(id))),
                Step::CameraBack => ctx.events.push(Event::CameraBack),
                Step::Lens(near, far) => ctx.events.push(Event::Lens { near, far }),
                Step::Animation => {
                    if let Some(staging) = ctx.stagings.get(id) {
                        ctx.animator.set(&staging.clip);
                    }
                }
                Step::SecondAnimation => {
                    if let Some((clip, _)) = ctx.stagings.get(id).and_then(|staging| staging.second.as_ref()) {
                        ctx.animator.set(clip);
                    }
                }
                Step::Voice(name) => ctx.events.push(Event::SkillVoice { skill: id, name }),
                Step::Sound(sound) => ctx.events.push(Event::Sound(sound)),
                Step::OwnSound(sound) => ctx.events.push(Event::SkillSound { skill: id, sound }),
                // Her body is turned to where she aims now, and stays so until she
                // stops: nothing turns it again in between (0x76208).
                Step::Aim(true) => {
                    self.aiming = true;
                    self.bend_to_aim(ctx);
                }
                Step::Aim(false) => self.aiming = false,
                Step::Fire(firing) => ctx.events.push(Event::SkillShot { skill: id, firing }),
                Step::Parts(mode) => ctx.events.push(Event::Parts(mode)),
                Step::Weapon(node) => ctx.events.push(Event::Weapon { skill: id, node }),
                Step::Give => ctx.events.extend(effects.into_iter().filter(|&effect| effect > 0).map(Event::Give)),
                Step::ReadyOthers => {
                    ctx.events.push(Event::Sound(skills::SOUND_OTHERS_READY));
                    for _ in 0..ctx.slots.fill_others(id) {
                        ctx.events.push(Event::Sound(skills::SOUND_READY));
                    }
                }
                // She stands as she does when a battle ends without a pose.
                Step::Stand => {
                    ctx.animator.set_blended(&ctx.clips.idle, STAND_DOWN_BLEND);
                    ctx.animator.body_mut().set_time(0.0);
                    ctx.animator.body_mut().set_delay(STAND_DOWN_BLEND);
                    ctx.animator.body_mut().loop_clip();
                }
                // With no HP left she falls instead.
                Step::Unlock => {
                    self.locks &= !(lock::BUSY | lock::ACTIONS | lock::ITEMS);
                    if self.hp <= 0.0 {
                        self.fallen = true;
                    }
                }
            }
        }
        phase as u32
    }

    /// Whether she can use a skill now (0x99050): not overheated, stunned or kept
    /// from skills; those that shoot not while she is kept from shooting, and four
    /// of them not while she is kept from dodging.
    fn can_use(&self, skill: i32) -> bool {
        const SHOOTING: [i32; 17] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 14, 15, 18, 19, 20, 22];
        !(self.heat.overheated
            || self.status.has(kind::STUN)
            || self.status.has(kind::NO_SKILL)
            || SHOOTING.contains(&skill) && self.status.has(kind::NO_SHOT)
            || (3..7).contains(&skill) && self.status.has(kind::NO_DODGE))
    }

    /// The end of a won battle: the victory pose (0x78B44), or standing down
    /// (0x7896C).
    fn run_finish(&mut self, phase: u32, pose: bool, time: f32, ctx: &mut Context) -> u32 {
        match phase {
            0 => {
                if pose {
                    self.take_the_stage();
                    ctx.events.push(Event::Camera(Staged::Victory));
                    ctx.animator.set(&ctx.clips.victory);
                } else if !ctx.animator.body().is(&ctx.clips.idle) {
                    // If she was aiming she stays turned that way: this state does
                    // not touch the bend of her upper body.
                    ctx.animator.set_blended(&ctx.clips.idle, STAND_DOWN_BLEND);
                    ctx.animator.body_mut().set_time(0.0);
                    ctx.animator.body_mut().set_delay(STAND_DOWN_BLEND);
                    ctx.animator.body_mut().loop_clip();
                }
                1
            }
            1 if time >= if pose { VICTORY_TIME } else { STAND_DOWN_TIME } => {
                self.finished = true;
                2
            }
            _ => phase,
        }
    }

    /// Her defeat (0x78D38): she falls, with a camera move of its own, and three
    /// seconds on the battle is lost.
    fn run_defeated(&mut self, phase: u32, time: f32, ctx: &mut Context) -> u32 {
        match phase {
            0 => {
                self.take_the_stage();
                ctx.events.push(Event::Camera(Staged::Defeat));
                ctx.animator.set(&ctx.clips.defeat);
                ctx.events.push(Event::Voice(VOICE_DEFEAT));
                1
            }
            1 if time >= DEFEAT_TIME => {
                ctx.events.push(Event::Lost);
                2
            }
            _ => phase,
        }
    }

    /// Standing ready (0x7D324).
    fn enter_idle(&mut self) -> State {
        self.locks = lock::BUSY;
        self.guarding = false;
        State::Idle { phase: 0 }
    }

    /// 0x7D410.
    fn run_idle(&mut self, phase: u32, ctx: &mut Context) -> u32 {
        if phase == 0 {
            self.aiming = false;
            stand(ctx);
        }
        1
    }

    /// Standing with the cannon still raised after a burst, until the gauge overheats
    /// or she does something else (0x7DC94).
    fn enter_aim_idle(&mut self) -> State {
        self.locks &= !(lock::ACTIONS | lock::EVADING);
        self.locks |= lock::BUSY;
        State::AimIdle { phase: 0 }
    }

    /// 0x7DD60.
    fn run_aim_idle(&mut self, phase: u32) -> u32 {
        if phase > 1 || !self.heat.overheated {
            return phase.max(1);
        }
        self.aiming = false;
        self.finish();
        2
    }

    /// The gauge has filled: one step with everything locked, then she stands, unable
    /// to act, until it has emptied (0x7D608).
    fn enter_overheat(&mut self) -> State {
        self.locks |= lock::BUSY | lock::ACTIONS;
        self.locks &= !lock::HEAT_RECOVERY;
        State::Overheat { phase: 0 }
    }

    /// 0x7E080.
    fn run_overheat(&mut self, phase: u32, ctx: &mut Context) -> u32 {
        match phase {
            0 => {
                self.aiming = false;
                stand(ctx);
                // The effect is begun twice on her (0x7E21C): the second is of the
                // scene's last draw list (0x7E25C), so that whatever she stands
                // behind it is seen through that.
                ctx.events.push(Event::Effect { id: EFFECT_OVERHEAT, place: Place::HER, on_top: false });
                ctx.events.push(Event::Effect { id: EFFECT_OVERHEAT, place: Place::HER, on_top: true });
                1
            }
            1 => {
                self.finish();
                self.locks &= !lock::ACTIONS;
                2
            }
            _ => phase,
        }
    }

    /// 0x7E2F4.
    fn run_hit(&mut self, phase: u32, reaction: usize, ctx: &mut Context) -> u32 {
        let body = ctx.animator.body().time();
        match phase {
            0 => {
                ctx.animator.set(&ctx.clips.hit[reaction]);
                ctx.animator.body_mut().set_time(0.0);
                ctx.animator.body_mut().loop_between(HIT_HOLD, HIT_HOLD);
                1
            }
            1 if self.hit_stop > 0.0 => 1,
            1 => {
                ctx.animator.body_mut().play_out();
                2
            }
            _ => {
                if body >= ctx.animator.body().length() {
                    self.finish();
                    self.locks &= !lock::ACTIONS;
                }
                2
            }
        }
    }

    /// Guarding for as long as the button is held (0x7D8AC).
    fn enter_guard(&mut self, ctx: &mut Context) -> State {
        self.aiming = false;
        self.warm(ctx.tunables.int(id::GUARD_HEAT) as f32, ctx.tunables);
        self.locks |= lock::BUSY | lock::ACTIONS | lock::HEAT_RECOVERY;
        self.guard_hit = false;
        self.guarding = true;
        // Her skills that act of themselves are told (0x7D990): one of them counts
        // the frames from here.
        self.ask(Happens::GuardBegins, ctx.random);
        debug!("her guard begins on frame {}", self.frame);
        State::Guard { phase: 0 }
    }

    /// 0x7E4CC.
    fn run_guard(&mut self, mut phase: u32, ctx: &mut Context) -> u32 {
        if phase == 0 {
            ctx.animator.set(&ctx.clips.guard);
            ctx.animator.body_mut().loop_between(0.0, 0.0);
            phase = 1;
        }
        if phase == 1 {
            // A guarded hit plays the recoil, up to the end of the clip's loop range.
            if std::mem::take(&mut self.guard_hit) {
                let recoiled = ctx.clips.guard.loop_range().1;
                ctx.animator.body_mut().set_time(0.0);
                ctx.animator.body_mut().loop_between(recoiled, recoiled);
            }
            if ctx.pad.held & button::CROSS != 0 {
                return 1;
            }
            phase = 2;
        }
        let body = ctx.animator.body();
        if body.time() < body.range().0 {
            return phase;
        }
        if phase == 2 {
            // Let go, and any recoil has played: lower the guard.
            self.guarding = false;
            ctx.animator.body_mut().play_out();
            ctx.animator.body_mut().set_time(GUARD_RELEASE);
            self.locks &= !lock::ACTIONS;
            // The clip now has to run out before the state is over.
            let body = ctx.animator.body();
            if body.time() < body.range().0 {
                return 3;
            }
        }
        self.finish();
        3
    }

    /// A dash to the other standing place (0x7DA7C).
    fn enter_dodge(&mut self, ctx: &mut Context) -> State {
        let back = self.slot > 0;
        self.slot = if back { self.slot - 1 } else { self.slot + 1 };
        self.locks |= lock::BUSY | lock::EVADING | lock::ACTIONS | lock::HEAT_RECOVERY;
        self.guarding = false;
        State::Dodge { phase: 0, back, free: self.spared_heat(Happens::DodgeBegins, ctx) }
    }

    /// As a dodge or a burst begins her skills that act of themselves are told,
    /// and then asked whether it is to cost her no heat (0x7DB74 and 0x7DBAC,
    /// 0x7DF60 and 0x7DF98). The answer is yes while any of them that spares
    /// heat is acting; the state before has been done with by then, and has had
    /// its own stop.
    fn spared_heat(&mut self, begins: Happens, ctx: &mut Context) -> bool {
        let begun = self.ask(begins, ctx.random);
        show_acting(&begun, ctx.events);
        let spared = self.ask(Happens::HeatSpared, ctx.random).heat_spared;
        if spared {
            info!("a skill of hers spares her the heat of what she begins ({begins:?}, frame {})", self.frame);
        }
        spared
    }

    /// 0x7E704.
    fn run_dodge(&mut self, phase: u32, back: bool, free: bool, ctx: &mut Context) -> u32 {
        match phase {
            0 => {
                if !free {
                    // The original takes this cost from the record of an equipped item;
                    // until items are ported the tunable of the same meaning stands in.
                    self.warm(ctx.tunables.int(id::DODGE_HEAT) as f32, ctx.tunables);
                }
                ctx.events.push(Event::Sound(SOUND_DODGE));
                self.aiming = false;
                let clip = if back { &ctx.clips.dodge_back } else { &ctx.clips.dodge_out };
                ctx.animator.set_blended(clip, DODGE_BLEND);
                ctx.animator.body_mut().set_time(0.0);
                ctx.animator.body_mut().set_delay(DODGE_BLEND);
                ctx.animator.body_mut().loop_clip();

                let destination = ctx.stage.slot(self.slot);
                self.velocity = (destination - self.position).normalize_or_zero() * DODGE_SPEED;
                self.brake = 0.0;
                self.distance_left = destination.distance(self.position);
                self.moving = true;

                // The dust is left on the ground where she sets off, turned to
                // where she goes (0x7E940); what travels with her is turned so
                // from her (0x7E9F8).
                let ground = Vec3::new(self.position.x, 0.0, self.position.z);
                let turn = facing(destination - self.position);
                let dust = if back { EFFECT_DODGE_BACK } else { EFFECT_DODGE_OUT };
                ctx.events.push(Event::Effect { id: dust, place: Place::At(ground, turn), on_top: false });
                ctx.events.push(Event::Effect { id: EFFECT_DODGE_START, place: Place::At(ground, turn), on_top: false });
                ctx.events.push(Event::Effect { id: EFFECT_DODGE_TRAIL, place: Place::Actor(Vec3::ZERO, turn), on_top: false });
                1
            }
            1 if self.distance_left <= DODGE_BRAKE_DISTANCE => {
                // What she stops with is left where one of her feet is (0x7EA54).
                ctx.events.push(Event::Effect { id: EFFECT_DODGE_END, place: Place::WhereNode(DODGE_END_NODE), on_top: false });
                self.brake = DODGE_BRAKE;
                ctx.animator.set_blended(&ctx.clips.idle, STAND_BLEND);
                ctx.animator.body_mut().set_time(0.0);
                ctx.animator.body_mut().set_delay(STAND_BLEND);
                ctx.animator.body_mut().loop_clip();
                self.finish();
                self.locks &= !(lock::ACTIONS | lock::EVADING | lock::HEAT_RECOVERY);
                2
            }
            _ => phase,
        }
    }

    /// A burst from the cannon (0x7DEA0).
    fn enter_shoot(&mut self, ctx: &mut Context) -> State {
        self.locks |= lock::BUSY | lock::ACTIONS | lock::HEAT_RECOVERY;
        self.guarding = false;
        State::Shoot { phase: 0, bullets: 0, interval: 0.0, timer: 0.0, free: self.spared_heat(Happens::ShootBegins, ctx) }
    }

    /// 0x7EB68.
    fn run_shoot(&mut self, state: State, ctx: &mut Context) -> State {
        let State::Shoot { mut phase, mut bullets, mut interval, mut timer, free } = state else { return state };
        timer = (timer - ctx.dt).max(0.0);
        if self.aiming {
            self.bend_to_aim(ctx);
        }

        if phase == 0 {
            ctx.animator.set(&ctx.clips.shoot);
            ctx.animator.body_mut().set_time(SHOOT_AIMED);
            ctx.animator.body_mut().loop_between(SHOOT_AIMED, SHOOT_AIMED);
            self.aiming = true;
            self.bend_to_aim(ctx);
            // The hotter the gauge, the shorter the burst.
            let stage = self.heat.burst_stage(ctx.tunables);
            bullets = ctx.tunables.int(id::BURST_BULLETS[stage]);
            interval = ctx.tunables.int(id::BURST_INTERVAL[stage]) as f32;
            if !free {
                self.warm(ctx.tunables.int(id::SHOT_HEAT) as f32, ctx.tunables);
            }
            phase = 1;
        }
        match phase {
            // One shot: the recoil plays from the aimed pose, and unless the burst's
            // heat has just overheated her a bullet leaves.
            1 => {
                ctx.animator.body_mut().play_out();
                ctx.animator.body_mut().set_time(SHOOT_AIMED);
                ctx.animator.body_mut().loop_between(SHOOT_RECOILED, SHOOT_RECOILED);
                if !self.heat.overheated {
                    // The shot kicks the picture (0x7ED08).
                    self.jolts.push(shake::Asked::Kind(0));
                    ctx.events.push(Event::Sound(SOUND_SHOT));
                    ctx.events.push(Event::Effect { id: EFFECT_MUZZLE_FLASH, place: Place::Node(MUZZLE_NODE), on_top: false });
                    self.fire(ctx);
                }
                timer = interval;
                bullets -= 1;
                phase = 2;
            }
            // Between shots; after the last one, she is stiff for a moment.
            2 if timer > 0.0 => {}
            2 if bullets > 0 => phase = 1,
            2 => {
                timer = ctx.tunables.int(id::SHOT_STIFFNESS) as f32;
                phase = 3;
            }
            _ => {}
        }
        if phase == 3 && timer <= 0.0 {
            self.locks &= !(lock::HEAT_RECOVERY | lock::SHOOT | lock::SKILLS);
            phase = 4;
        }
        // Once the last recoil has played out she can do anything again.
        if phase == 4 && ctx.animator.body().time() >= ctx.animator.body().range().0 {
            self.finish();
            self.locks &= !lock::ACTIONS;
        }
        State::Shoot { phase, bullets, interval, timer, free }
    }

    /// What every state does as it ends: she is free for the next one, and if she has
    /// no HP left her defeat follows.
    fn finish(&mut self) {
        self.locks &= !lock::BUSY;
        if self.hp <= 0.0 {
            self.fallen = true;
        }
    }

    /// Bends her upper body from where she faces towards the aim point: each of two
    /// nodes takes half of the turn (0x993B8).
    fn bend_to_aim(&mut self, ctx: &mut Context) {
        ctx.animator.clear_twists();
        let Some(aim) = ctx.aim_point else { return };
        let from = Vec3::new(self.position.x, AIM_HEIGHT, self.position.z);
        let turn = facing(self.direction).inverse() * facing(aim - from);
        let half = Quat::IDENTITY.slerp(turn, 0.5);
        for id in AIM_NODES {
            if let Some(index) = ctx.skeleton.index_of(id) {
                ctx.animator.set_twist(index, half);
            }
        }
    }

    /// Sends a bullet from the muzzle towards the aim point (0x98D5C).
    fn fire(&mut self, ctx: &mut Context) {
        let Some(aim) = ctx.aim_point else { return };
        let muzzle = match ctx.skeleton.index_of(MUZZLE_NODE) {
            Some(index) => self.transform() * ctx.animator.model_space(ctx.skeleton, index),
            None => self.transform(),
        };
        let from = muzzle.translation;
        ctx.events.push(Event::Shot { from, velocity: (aim - from).normalize_or_zero() * BULLET_SPEED });
    }

    /// Carries her along a dodge and keeps her facing the middle of the stage (0x99130).
    fn travel(&mut self, dt: f32) {
        if !self.moving {
            return;
        }
        if self.brake > 0.0 {
            let lost = self.brake * dt;
            if lost < 1.0 {
                self.velocity -= self.velocity * lost;
                for axis in 0..3 {
                    if self.velocity[axis].abs() < STOPPED {
                        self.velocity[axis] = 0.0;
                    }
                }
            } else {
                self.velocity = Vec3::ZERO;
            }
        }
        if self.velocity == Vec3::ZERO {
            self.moving = false;
            return;
        }
        let mut step = self.velocity * dt;
        if self.distance_left < step.length() {
            step = step.normalize() * self.distance_left;
        }
        self.distance_left -= step.length();
        self.position += step;
        self.direction = (Stage::CENTRE - self.position).normalize();
    }

    /// Reads the pad and picks the next state (0x9A474). Her items (L) come first;
    /// then her skills, with R held, which keeps the other buttons from acting.
    fn choose(&mut self, ctx: &mut Context) {
        if std::mem::take(&mut self.fallen) {
            // Her defeat (the state built at 0x78C94).
            self.action = Action::Defeated;
            self.locks |= lock::BUSY | lock::ACTIONS;
            self.begin(State::Defeated { phase: 0, time: 0.0 });
            return;
        }
        let pad = ctx.pad;
        // L has her items come up, when she can use any (0x98FA0): the battle
        // stands still until their menu has gone, and nothing else is chosen.
        if self.can_act(ctx.rules) && self.locks & lock::ITEMS == 0 && !self.status.has(kind::STUN) && pad.pressed & button::L != 0 {
            self.items_asked = true;
            return;
        }
        // Stunned she can start nothing; two lesser ailments each take one action
        // from her (0x9A9D8, 0x9AAA0, 0x9ABC8).
        let free = self.can_act(ctx.rules) && pad.held & button::R == 0 && !self.heat.overheated && !self.status.has(kind::STUN);
        let open = |locks: u32, lock: u32| locks & lock == 0;

        // With R held, the button of a slot whose skill is ready uses it (0x9A568).
        if self.can_act(ctx.rules) && open(self.locks, lock::SKILLS) && pad.held & button::R != 0 {
            for slot in 0..skills::SLOTS {
                let Some(held) = ctx.slots.get(slot) else { continue };
                let (id, line, effects) = (held.skill.id, held.skill.line(), held.skill.effects);
                let ready = held.ready() || ctx.tunables.flag(id::SKILLS_ALWAYS_READY);
                if !ready || pad.pressed & skills::BUTTONS[slot] == 0 {
                    continue;
                }
                // One she cannot use now is the end of the choosing (0x9A60C).
                if !self.can_use(id) {
                    return;
                }
                let Some(scene) = skills::scene(id) else {
                    warn_once!("skill {id} has a state of its own, which is not ported: nothing comes of its button");
                    continue;
                };
                // One that is for an enemy needs her to have locked on to one that
                // can be struck: without, nothing comes of its button (0x9A728).
                // A snipe takes whatever she has locked on to, if anything, and
                // its slot does not start over yet (0x9A900).
                let lock = ctx.lock.filter(|lock| lock.open || !scene.aimed).filter(|_| scene.aimed || scene.scoped);
                let target = lock.map(|lock| (lock.enemy, lock.gauge));
                if scene.aimed && target.is_none() {
                    continue;
                }
                if !scene.scoped {
                    ctx.slots.used(slot);
                }
                self.action = if scene.shoots { Action::SkillShot } else { Action::Skill };
                let shots = ctx.stagings.get(id).and_then(|staging| staging.shot).map_or(0, |shot| shot.count);
                let state = self.enter_skill(id, line, effects, target, shots);
                self.begin(state);
                return;
            }
        }

        if free && open(self.locks, lock::GUARD) && pad.held & button::CROSS != 0 {
            self.action = Action::Guard;
            self.end_state();
            let state = self.enter_guard(ctx);
            self.begin(state);
        } else if free
            && open(self.locks, lock::DODGE)
            && !self.status.has(kind::NO_DODGE)
            && (ctx.rules.dodge_unlocked || ctx.tunables.flag(id::DODGE_ALWAYS))
            && pad.pressed & button::CIRCLE != 0
        {
            let extra = ctx.tunables.float(id::DODGE_RECOVERY_WAIT);
            if self.heat.wait < extra {
                self.heat.wait += extra;
            }
            self.action = Action::Dodge;
            self.end_state();
            let state = self.enter_dodge(ctx);
            self.begin(state);
        } else if free && open(self.locks, lock::SHOOT) && !self.status.has(kind::NO_SHOT) && pad.pressed & button::SQUARE != 0 {
            self.action = Action::Shoot;
            self.end_state();
            let state = self.enter_shoot(ctx);
            self.begin(state);
        } else if open(self.locks, lock::OVERHEAT) && self.heat.overheated {
            if self.action != Action::Overheat {
                self.action = Action::Overheat;
                let state = self.enter_overheat();
                self.begin(state);
            }
        } else if open(self.locks, lock::BUSY) {
            let next = if self.aiming { Action::AimIdle } else { Action::Idle };
            if self.action != next {
                self.action = next;
                let state = if self.aiming { self.enter_aim_idle() } else { self.enter_idle() };
                self.begin(state);
            }
        }
    }
}

/// What is heard and seen as a skill of hers begins to act of itself (0x9F6D8):
/// the effect goes with her, and is drawn over everything.
fn show_acting(answer: &Answer, events: &mut Vec<Event>) {
    if answer.acted > 0 {
        events.push(Event::Sound(auto::SOUND));
        events.push(Event::Effect { id: auto::EFFECT, place: Place::HER, on_top: true });
    }
}

/// Puts her in the standing animation unless she is in it already.
fn stand(ctx: &mut Context) {
    if !ctx.animator.body().is(&ctx.clips.idle) {
        ctx.animator.set(&ctx.clips.idle);
        ctx.animator.body_mut().set_time(0.0);
        ctx.animator.body_mut().loop_clip();
    }
}

#[cfg(test)]
mod tests {
    use brs_formats::insa;

    use super::*;
    use crate::battle::damage::Attacker;
    use crate::battle::skills::{Hold, Made};

    /// An effect the scene of a skill makes, as the battle is told of it; and one
    /// that goes with the weapon of a skill, turned as its node is not.
    fn made(id: u32, hold: Hold, list: i32) -> Event {
        Event::SceneEffect(Made::new(id, hold, list))
    }

    fn on_weapon(id: u32, node: u8) -> Event {
        Event::SceneEffect(Made::new(id, Hold::With(Some(node)), 4).turned(90.0, 0.0, 90.0))
    }

    /// The values of `btl_system.bin` that the actor reads.
    fn tunables() -> Tunables {
        Tunables::from_values(&[
            (id::NO_OVERHEAT, 0.0),
            (id::HEAT_RECOVERY, 22.05),
            (id::HEAT_RECOVERY_OVERHEATED, 27.56),
            (id::HEAT_RECOVERY_WAIT, 30.0),
            (id::DODGE_RECOVERY_WAIT, 0.0),
            (id::DODGE_ALWAYS, 0.0),
            (id::SKILLS_ALWAYS_READY, 0.0),
            (id::SKILL_DIM, 196.0),
            (id::SNIPE_UI_ENTER, 10.0),
            (id::SNIPE_LOCK_START, 10.0),
            (id::SNIPE_LOCK_WAIT, 10.0),
            (id::SHOT_STIFFNESS, 10.0),
            (id::SHOT_HEAT, 10.0),
            (id::DODGE_HEAT, 15.0),
            (id::GUARD_HEAT, 0.0),
            (id::HEAT_MAX, 100.0),
            (id::GUARDED_HIT_HEAT, 5.0),
            (id::GUARD_PERCENT, 50.0),
            (55, 39.0),
            (56, 59.0),
            (57, 69.0),
            (58, 89.0),
            (59, 6.0),
            (60, 6.0),
            (61, 3.0),
            (62, 6.0),
            (63, 3.0),
            (64, 6.0),
            (65, 2.0),
            (66, 6.0),
            (67, 1.0),
            (68, 6.0),
        ])
    }

    fn clip(frames: u32, loop_range: (u32, u32)) -> Arc<Clip> {
        let unit = insa::UNITS_PER_FRAME;
        Clip::new(insa::Animation {
            start: 0,
            end: frames * unit,
            loop_start: loop_range.0 * unit,
            loop_end: loop_range.1 * unit,
            nodes: Vec::new(),
        })
    }

    /// Clips with the lengths and loop ranges of the real ones.
    fn clips() -> Clips {
        Clips {
            idle: clip(72, (0, 72)),
            dodge_out: clip(2, (0, 2)),
            dodge_back: clip(2, (0, 2)),
            shoot: clip(72, (38, 46)),
            hit: [clip(40, (0, 40)), clip(40, (0, 40))],
            guard: clip(40, (0, 20)),
            victory: clip(360, (0, 360)),
            defeat: clip(320, (0, 320)),
        }
    }

    /// A battle with only the player in it, driven one step at a time.
    struct Bench {
        actor: PcActor,
        animator: Animator,
        skeleton: Skeleton,
        tunables: Tunables,
        clips: Clips,
        stage: Stage,
        slots: Slots,
        /// What her cursor is locked on to, and where the enemies stand.
        lock: Option<LockOn>,
        places: Vec<(Entity, Vec3, bool)>,
        random: Random,
        events: Vec<Event>,
    }

    #[test]
    fn in_survival_limit_nothing_gives_her_hp_back() {
        let mut bench = Bench::new();
        bench.actor.hp = 40.0;
        bench.actor.heal(25);
        assert_eq!((bench.actor.hp, bench.actor.numbers.len()), (65.0, 1));
        bench.actor.unhealed = true;
        bench.actor.heal(25);
        assert_eq!((bench.actor.hp, bench.actor.numbers.len()), (65.0, 1), "no HP and no number");
    }
    impl Bench {
        fn new() -> Bench {
            let (tunables, stage) = (tunables(), Stage::default());
            Bench {
                lock: None,
                places: Vec::new(),
                actor: PcActor::new(100.0, 100.0, &stage, &tunables),
                animator: Animator::new(),
                skeleton: Skeleton {
                    nodes: Vec::new(),
                    rest: Vec::new(),
                    parents: Vec::new(),
                    fitted: Vec::new(),
                    has_child: Vec::new(),
                    after: Vec::new(),
                    ids: Vec::new(),
                },
                tunables,
                clips: clips(),
                stage,
                slots: Slots::default(),
                random: Random::new(9),
                events: Vec::new(),
            }
        }

        /// Runs one step with `held` buttons, of which `pressed` went down on it.
        fn step(&mut self, held: u32, pressed: u32) {
            let pad = Pad { held, pressed, ..default() };
            self.animator.step(2.0);
            self.actor.step(&mut Context {
                dt: 2.0,
                pad: &pad,
                tunables: &self.tunables,
                stage: &self.stage,
                rules: Rules::default(),
                clips: &self.clips,
                animator: &mut self.animator,
                skeleton: &self.skeleton,
                aim_point: Some(Vec3::new(0.0, 100.0, 0.0)),
                lock: self.lock,
                places: &self.places,
                slots: &mut self.slots,
                stagings: &Stagings::default(),
                random: &mut self.random,
                events: &mut self.events,
            });
        }

        fn idle(&mut self, steps: usize) {
            for _ in 0..steps {
                self.step(0, 0);
            }
        }

        /// Steps on which a bullet left, counted from the last call.
        fn shots(&mut self, steps: usize) -> Vec<usize> {
            let mut fired = Vec::new();
            for step in 0..steps {
                self.events.clear();
                self.step(0, 0);
                if self.events.iter().any(|event| matches!(event, Event::Shot { .. })) {
                    fired.push(step);
                }
            }
            fired
        }
    }

    /// Status effects as the table has them, and attacks that carry them.
    fn effects() -> Effects {
        let effect = |id, kind, duration, values| Effect { id, kind, chance: 100, duration, values, ..Effect::default() };
        Effects::from_records(vec![
            effect(101, kind::HEAL, 0, [0, 0, 10, 0]),
            effect(500, kind::GUARD_SKILL, 0, [0, 50, 0, 0]),
            effect(501, kind::GUARD_SKILL, 0, [80, 75, 50, 101]),
            effect(602, kind::POISON, 900, [180, 0, 3, 0]),
            effect(800, kind::NO_DODGE, 450, [0; 4]),
            effect(900, kind::STUN, 300, [0; 4]),
            effect(1101, kind::ATTACK_DOWN, 900, [20, 0, 0, 0]),
            effect(1200, kind::DEFENCE_DOWN, 900, [25, 0, 0, 0]),
            effect(2100, kind::WARD, 300, [200, 255, 255, 0]),
            effect(2200, kind::NO_HEAT, 180, [0; 4]),
            effect(2600, kind::GUARD_BREAK, 0, [0; 4]),
            effect(2800, kind::DRAIN, 0, [100, 0, 0, 0]),
        ])
    }

    fn stats() -> PlayerStats {
        PlayerStats {
            level: 5,
            experience: 843,
            max_hp: 100.0,
            hp: 100.0,
            attack: 44,
            special: 41,
            defence: 24,
            burst_bullets: 6,
            dodge_heat: 15,
            skill_bonus: [0; 4],
            skills: [0; 4],
            weapon: 129,
            suit: 133,
        }
    }

    /// An attack that takes 30 off her before anything scales it, carrying `effect`.
    fn blow(effect: i32) -> Attack {
        Attack { id: 400, attack_percent: 100, defence_percent: 50, dealt_percent: 100, effect, strength: 0, ..Attack::default() }
    }

    impl Bench {
        /// An enemy of her own level hits her with `attack`.
        fn struck(&mut self, attack: &Attack, status: &Status) -> HitOutcome {
            let by = Assailant { figures: Attacker { attack: 42, special: 50 }, level: 5, status };
            self.actor.take_hit(attack, by, &stats(), &effects(), &self.tunables, &mut Random::new(9))
        }

        /// Gives her an effect outright.
        fn afflicted(&mut self, id: i32) {
            let effect = *effects().get(id).unwrap();
            let giver = Giver { level: 5, special: 50, boost: [0; 3], name: "Eater".into() };
            assert_eq!(self.actor.afflict(&effect, &giver, &stats(), &mut Random::new(4)), status::Outcome::Given);
        }
    }

    #[test]
    fn an_attack_gives_her_the_effect_it_carries() {
        let mut bench = Bench::new();
        bench.idle(2);
        let nobody = Status::default();
        let outcome = bench.struck(&blow(602), &nobody);
        assert_eq!((outcome.damage, outcome.drained), (30, 0));
        assert_eq!(outcome.effect, Some((602, status::Outcome::Given)));
        assert!(bench.actor.status.has(kind::POISON));
        // A plain attack carries nothing.
        assert_eq!(bench.struck(&blow(0), &nobody).effect, None);
        // One that fells her gives her nothing more.
        bench.actor.hp = 10.0;
        let outcome = bench.struck(&blow(800), &nobody);
        assert!(outcome.defeated);
        assert_eq!(outcome.effect, None);
        assert!(!bench.actor.status.has(kind::NO_DODGE));
    }

    #[test]
    fn a_hit_on_her_is_seen_as_its_attacks_look_or_as_the_ailment_it_gave_her() {
        let mut bench = Bench::new();
        bench.idle(2);
        let nobody = Status::default();
        // A plain attack of the third look.
        let plain = Attack { look: 3, ..blow(0) };
        let outcome = bench.struck(&plain, &nobody);
        assert_eq!(seen_of_hit(&plain, &outcome, &effects()), Some(999_008));
        // Guarded, it has the look of a guarded hit of that look, which is the same.
        let guarded = HitOutcome { guarded: true, ..outcome };
        assert_eq!(seen_of_hit(&plain, &guarded, &effects()), Some(999_008));
        // A guarded hit of the first look shows nothing where it lands.
        assert_eq!(seen_of_hit(&Attack { look: 1, ..blow(0) }, &guarded, &effects()), None);
        assert_eq!(seen_of_hit(&Attack { look: 1, ..blow(0) }, &outcome, &effects()), Some(999_003));
        // Poison that takes is seen in the look's place; kept off, the look is.
        let poisonous = Attack { look: 3, ..blow(602) };
        let outcome = bench.struck(&poisonous, &nobody);
        assert_eq!(outcome.effect, Some((602, status::Outcome::Given)));
        assert_eq!(seen_of_hit(&poisonous, &outcome, &effects()), Some(EFFECT_POISONED));
        let outcome = bench.struck(&poisonous, &nobody);
        assert_eq!(outcome.effect, Some((602, status::Outcome::Kept)));
        assert_eq!(seen_of_hit(&poisonous, &outcome, &effects()), Some(999_008));
        // A stun has its own, and every other ailment the third.
        let stun = HitOutcome { effect: Some((900, status::Outcome::Given)), ..outcome };
        assert_eq!(seen_of_hit(&plain, &stun, &effects()), Some(EFFECT_STUNNED));
        let slowed = HitOutcome { effect: Some((1200, status::Outcome::Given)), ..outcome };
        assert_eq!(seen_of_hit(&plain, &slowed, &effects()), Some(EFFECT_AILING));
        // What is no ailment leaves the look.
        let warded = HitOutcome { effect: Some((2100, status::Outcome::Given)), ..outcome };
        assert_eq!(seen_of_hit(&plain, &warded, &effects()), Some(999_008));
    }

    #[test]
    fn the_damage_she_takes_is_counted_until_the_battle_is_decided() {
        let mut bench = Bench::new();
        bench.idle(2);
        let nobody = Status::default();
        bench.struck(&blow(0), &nobody);
        assert_eq!((bench.actor.hp, bench.actor.damage_taken), (70.0, 30));
        // A blow that takes more than she has left is counted whole.
        bench.actor.hp = 10.0;
        assert!(bench.struck(&blow(0), &nobody).defeated);
        assert_eq!((bench.actor.hp, bench.actor.damage_taken), (0.0, 60));

        // Once the battle is decided a blow takes nothing off her.
        let mut bench = Bench::new();
        bench.idle(2);
        bench.actor.spared = true;
        assert!(!bench.struck(&blow(0), &nobody).defeated);
        assert_eq!((bench.actor.hp, bench.actor.damage_taken), (100.0, 0));
    }

    #[test]
    fn poison_wears_her_down_but_cannot_kill() {
        let mut bench = Bench::new();
        bench.idle(2);
        bench.afflicted(602);
        // Three in a hundred of her 100 HP, every 91 steps.
        bench.idle(90);
        assert_eq!(bench.actor.hp, 100.0);
        bench.idle(1);
        assert_eq!(bench.actor.hp, 97.0);
        // With next to nothing left it leaves her one HP.
        bench.actor.hp = 2.0;
        bench.idle(91);
        assert_eq!(bench.actor.hp, 1.0);
        bench.idle(91);
        assert_eq!(bench.actor.hp, 1.0);
        // Its 900 sixtieths over, it is gone.
        bench.idle(180);
        assert!(!bench.actor.status.has(kind::POISON));
    }

    #[test]
    fn a_stun_takes_every_action_until_it_ends_or_she_is_hit() {
        let mut bench = Bench::new();
        bench.idle(2);
        bench.afflicted(900);
        for button in [button::SQUARE, button::CIRCLE, button::CROSS] {
            bench.step(button, button);
            assert_eq!(bench.actor.action, Action::None, "button {button:#x}");
            bench.step(0, 0);
        }
        // A hit ends it, strong enough to make her reel or not.
        let outcome = bench.struck(&Attack { strength: -5, ..blow(0) }, &Status::default());
        assert!(!outcome.reeled);
        assert!(!bench.actor.status.has(kind::STUN));
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.actor.action, Action::Shoot);

        // Left alone it lasts its 300 sixtieths.
        let mut bench = Bench::new();
        bench.idle(2);
        bench.afflicted(900);
        bench.idle(149);
        assert!(bench.actor.status.has(kind::STUN));
        bench.idle(1);
        assert!(!bench.actor.status.has(kind::STUN));
    }

    #[test]
    fn the_lesser_ailments_each_take_one_action() {
        let mut bench = Bench::new();
        bench.idle(2);
        bench.afflicted(800);
        bench.step(button::CIRCLE, button::CIRCLE);
        assert_eq!(bench.actor.action, Action::None);
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.actor.action, Action::Shoot);
        // Without it the same press sends her dodging.
        let mut bench = Bench::new();
        bench.idle(2);
        bench.step(button::CIRCLE, button::CIRCLE);
        assert_eq!(bench.actor.action, Action::Dodge);
    }

    #[test]
    fn status_effects_on_either_side_scale_what_a_hit_takes() {
        let give = |status: &mut Status, id: i32| {
            let effect = *effects().get(id).unwrap();
            status.give(&effect, &Giver::default(), &Taker::default(), &mut Random::new(1));
        };
        // The enemy's attack is down by a fifth: 24 instead of 30.
        let mut weakened = Status::default();
        give(&mut weakened, 1101);
        let mut bench = Bench::new();
        bench.idle(2);
        assert_eq!(bench.struck(&blow(0), &weakened).damage, 24);
        // Her defence is down by a quarter: a quarter more, 37.
        bench.afflicted(1200);
        assert_eq!(bench.struck(&blow(0), &Status::default()).damage, 37);

        // Under a ward every hit is a guarded one that takes nothing, and adds the
        // heat of a guarded hit.
        let mut bench = Bench::new();
        bench.idle(2);
        bench.afflicted(2100);
        let outcome = bench.struck(&blow(0), &Status::default());
        assert_eq!((outcome.damage, outcome.guarded, outcome.reeled), (0, true, false));
        assert_eq!(bench.actor.heat.value, 5.0);
        // Unless nothing can add heat.
        bench.afflicted(2200);
        bench.struck(&blow(0), &Status::default());
        assert_eq!(bench.actor.heat.value, 5.0);
        // An attack that breaks guards is not guarded, ward or no; the ward still
        // lets none of it through.
        let outcome = bench.struck(&blow(2600), &Status::default());
        assert_eq!((outcome.damage, outcome.guarded), (0, false));
    }

    #[test]
    fn a_draining_attack_gives_its_damage_back_to_the_enemy() {
        let mut bench = Bench::new();
        bench.idle(2);
        let outcome = bench.struck(&blow(2800), &Status::default());
        assert_eq!((outcome.damage, outcome.drained), (30, 30));
        assert_eq!(outcome.effect, Some((2800, status::Outcome::Nothing)));
        // Guarded, it drains what got through.
        bench.step(button::CROSS, 0);
        bench.step(button::CROSS, 0);
        assert!(bench.actor.guarding);
        let outcome = bench.struck(&blow(2800), &Status::default());
        assert_eq!((outcome.damage, outcome.drained, outcome.guarded), (15, 15, true));
    }

    #[test]
    fn a_cold_burst_is_six_bullets_four_steps_apart() {
        let mut bench = Bench::new();
        bench.idle(2);
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.actor.action, Action::Shoot);
        assert_eq!(bench.shots(40), vec![0, 4, 8, 12, 16, 20]);
        assert_eq!(bench.actor.heat.value, 10.0);
        // Afterwards she keeps the cannon raised.
        assert_eq!(bench.actor.action, Action::AimIdle);
        assert!(bench.actor.aiming);
        assert_eq!(bench.animator.body().time(), SHOOT_RECOILED);
    }

    #[test]
    fn each_shot_plays_the_whole_recoil() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::SQUARE, button::SQUARE);
        bench.step(0, 0);
        assert_eq!(bench.animator.body().time(), SHOOT_AIMED);
        bench.idle(3);
        assert_eq!(bench.animator.body().time(), 44.0);
        // The step of the next shot first reaches the end of the recoil, then restarts.
        bench.idle(1);
        assert_eq!(bench.animator.body().time(), SHOOT_AIMED);
    }

    #[test]
    fn shooting_again_is_possible_only_after_the_stiffness() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::SQUARE, button::SQUARE);
        // Last bullet on step 20 of the burst; then the interval (3 steps) and the
        // stiffness (5 steps) pass before the lock opens.
        bench.idle(28);
        assert_ne!(bench.actor.locks & lock::SHOOT, 0);
        bench.idle(1);
        assert_eq!(bench.actor.locks & lock::SHOOT, 0);
    }

    #[test]
    fn a_hot_gauge_shortens_the_burst() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.actor.heat.value = 60.0;
        bench.actor.heat.wait = 1000.0;
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.shots(40).len(), 3);
        bench.actor.heat.value = 89.0;
        bench.actor.heat.wait = 1000.0;
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.shots(40).len(), 1);
    }

    #[test]
    fn the_burst_that_overheats_fires_nothing_and_locks_her_until_cool() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.actor.heat.value = 95.0;
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.shots(12), Vec::<usize>::new());
        assert!(bench.actor.heat.overheated);
        assert_eq!(bench.actor.action, Action::Overheat);
        assert!(!bench.actor.aiming);

        // Pressing things does nothing while the gauge empties...
        bench.step(button::SQUARE, button::SQUARE);
        bench.step(button::CIRCLE, button::CIRCLE);
        assert_eq!(bench.actor.action, Action::Overheat);
        // ...which takes the wait and then 100 heat at 27.56 a second.
        bench.idle(110);
        assert!(bench.actor.heat.overheated);
        bench.idle(20);
        assert!(!bench.actor.heat.overheated);
        assert_eq!(bench.actor.action, Action::Idle);
    }

    #[test]
    fn a_dodge_crosses_to_the_other_place_and_back() {
        let mut bench = Bench::new();
        bench.idle(1);
        let (first, second) = (bench.stage.slot(0), bench.stage.slot(1));
        bench.step(button::CIRCLE, button::CIRCLE);
        assert_eq!(bench.actor.action, Action::Dodge);
        assert_ne!(bench.actor.locks & lock::EVADING, 0);
        bench.idle(1);
        assert_eq!(bench.actor.heat.value, 15.0);
        bench.idle(39);
        assert!(bench.actor.position.distance(second) < 1e-2, "ended at {}", bench.actor.position);
        assert_eq!(bench.actor.action, Action::Idle);
        // She still faces the middle of the stage.
        assert!(bench.actor.direction.dot((Stage::CENTRE - second).normalize()) > 0.9999);

        bench.step(button::CIRCLE, button::CIRCLE);
        bench.idle(40);
        assert!(bench.actor.position.distance(first) < 1e-2, "ended at {}", bench.actor.position);
    }

    #[test]
    fn she_can_act_again_while_the_dodge_is_still_braking() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::CIRCLE, button::CIRCLE);
        let mut steps = 0;
        while bench.actor.locks & lock::DODGE != 0 {
            bench.step(0, 0);
            steps += 1;
            assert!(steps < 40, "the dodge never let go");
        }
        // 790 units at 90 a step: the brakes come on with 200 or less to go.
        assert_eq!(steps, 8);
        assert!(bench.actor.position.distance(bench.stage.slot(1)) > 50.0);
    }

    #[test]
    fn the_guard_holds_while_the_button_is_down() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::CROSS, button::CROSS);
        assert_eq!(bench.actor.action, Action::Guard);
        assert!(bench.actor.guarding);
        for _ in 0..30 {
            bench.step(button::CROSS, 0);
        }
        assert!(bench.actor.guarding);
        assert_eq!(bench.animator.body().time(), 0.0);

        // Letting go lowers the guard from frame 22 to the end of the 40-frame clip.
        bench.step(0, 0);
        assert!(!bench.actor.guarding);
        assert_eq!(bench.animator.body().time(), GUARD_RELEASE);
        assert_eq!(bench.actor.action, Action::Guard);
        bench.idle(10);
        assert_eq!(bench.actor.action, Action::Idle);
    }

    #[test]
    fn a_guarded_hit_plays_the_recoil_before_the_guard_can_drop() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::CROSS, button::CROSS);
        bench.step(button::CROSS, 0);
        bench.actor.guard_hit = true;
        bench.step(button::CROSS, 0);
        // Released at once, but the recoil (20 frames) plays first.
        bench.idle(5);
        assert!(bench.actor.guarding);
        bench.idle(6);
        assert!(!bench.actor.guarding);
    }

    #[test]
    fn holding_r_keeps_the_other_buttons_from_acting() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::R | button::SQUARE, button::SQUARE);
        bench.step(button::R | button::CROSS, button::CROSS);
        assert_eq!(bench.actor.action, Action::None);
    }

    #[test]
    fn a_hit_reels_until_the_hit_stop_and_the_animation_are_over() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.actor.hit(0, 12.0);
        bench.idle(5);
        // Held on frame 6 while the hit stop lasts.
        assert_eq!(bench.animator.body().time(), HIT_HOLD);
        assert!(bench.actor.hit_stop > 0.0);
        bench.step(button::SQUARE, button::SQUARE);
        assert_ne!(bench.actor.action, Action::Shoot);
        assert_eq!(bench.animator.body().time(), HIT_HOLD);
        // The rest of the 40-frame reaction plays, and she is free again.
        bench.idle(16);
        assert_ne!(bench.actor.locks & lock::SHOOT, 0);
        bench.idle(2);
        assert_eq!(bench.actor.locks & lock::SHOOT, 0);
    }

    #[test]
    fn a_won_battle_ends_with_her_standing_down() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.step(button::SQUARE, button::SQUARE);
        bench.idle(3);
        assert!(bench.actor.aiming);
        bench.actor.finish_battle(false);
        assert_eq!(bench.actor.action, Action::Finish);
        // Twenty sixtieths after its first step the state has run its course.
        bench.idle(10);
        assert!(!bench.actor.finished);
        bench.idle(1);
        assert!(bench.actor.finished);
        // She has gone back to standing, with no camera move and the stage as it was.
        assert!(bench.animator.body().is(&bench.clips.idle));
        assert!(!bench.actor.staged);
        assert!(!bench.events.iter().any(|event| matches!(event, Event::Camera(_))));
        // Nothing can be started any more.
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.actor.action, Action::Finish);
    }

    #[test]
    fn the_victory_pose_takes_the_stage_for_a_second() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.actor.finish_battle(true);
        bench.events.clear();
        bench.idle(1);
        assert_eq!(bench.events, vec![Event::Camera(Staged::Victory)]);
        assert!(bench.actor.staged);
        assert!(bench.animator.body().is(&bench.clips.victory));
        bench.idle(29);
        assert!(!bench.actor.finished);
        bench.idle(1);
        assert!(bench.actor.finished);
    }

    #[test]
    fn with_no_hp_left_she_falls_once_her_state_ends_and_the_battle_is_lost_three_seconds_on() {
        let mut bench = Bench::new();
        bench.idle(1);
        bench.actor.hp = 0.0;
        bench.actor.hit(0, 0.0);
        bench.events.clear();
        // The hit reaction plays out first.
        let mut steps = 0;
        while bench.actor.action != Action::Defeated {
            bench.idle(1);
            steps += 1;
            assert!(steps < 60, "she never fell");
        }
        assert!(steps > 15, "the reaction was cut short after {steps} steps");
        assert!(bench.events.is_empty());
        bench.idle(1);
        assert_eq!(bench.events, vec![Event::Camera(Staged::Defeat), Event::Voice(VOICE_DEFEAT)]);
        assert!(bench.actor.staged);
        assert!(bench.animator.body().is(&bench.clips.defeat));
        bench.events.clear();
        bench.idle(89);
        assert!(bench.events.is_empty());
        bench.idle(1);
        assert_eq!(bench.events, vec![Event::Lost]);
        bench.events.clear();
        bench.idle(30);
        assert!(bench.events.is_empty());
        assert_eq!(bench.actor.action, Action::Defeated);
    }

    /// Her slots with these skills in them, of a table of ten: Charge Shot, Blade
    /// Kill, Stun Snipe, Vulcannon, Tracer Gun, Star Comet and Steel Cutter, which
    /// attack; Skill Boost and Defender, which give her something; and Ignition,
    /// which has the others ready.
    fn slots(equipped: [u8; 4]) -> Slots {
        let skills = skills::Skills::from_list(vec![
            skills::Skill::of_a_slot(1, 1, 1200, 10, [0; 3]),
            skills::Skill::of_a_slot(4, 1, 3600, 13, [0; 3]),
            skills::Skill::of_a_slot(7, 1, 3600, 16, [0; 3]),
            skills::Skill::of_a_slot(10, 1, 2400, 19, [0; 3]),
            skills::Skill::of_a_slot(14, 1, 3600, 20, [0; 3]),
            skills::Skill::of_a_slot(18, 1, 2100, 27, [0; 3]),
            skills::Skill::of_a_slot(20, 1, 5400, 26, [0; 3]),
            skills::Skill::of_a_slot(24, 2, 3600, 33, [1900, 0, 0]),
            skills::Skill::of_a_slot(25, 2, 1800, 34, [1600, 0, 0]),
            skills::Skill::of_a_slot(29, 2, 7200, 38, [0; 3]),
        ]);
        Slots::new(&skills, equipped)
    }

    impl Bench {
        /// Uses the skill on triangle and runs its scene to the step it ends on,
        /// which is the `last`th of its state. Says what the scene asked for, with
        /// the step each thing came on.
        fn scene(&mut self, last: usize) -> Vec<(usize, Event)> {
            self.step(button::R | button::TRIANGLE, button::TRIANGLE);
            assert_eq!(self.actor.action, Action::Skill);
            let mut asked = Vec::new();
            for step in 0..=last {
                assert_eq!(self.actor.staged, step > 0, "step {step}");
                self.events.clear();
                // Nothing she presses in it is anything, until the step that ends
                // it lets her act again.
                let pressing = if step < last { button::SQUARE | button::CROSS } else { 0 };
                self.step(pressing, pressing);
                asked.extend(self.events.iter().map(|&event| (step, event)));
            }
            self.events.clear();
            asked
        }
    }

    #[test]
    fn a_skill_of_a_slot_is_used_with_r_held_and_its_button() {
        let mut bench = Bench::new();
        bench.slots = slots([25, 0, 0, 0]);
        bench.idle(2);
        // Neither its button alone nor R alone uses it.
        bench.step(button::TRIANGLE, button::TRIANGLE);
        bench.step(button::R, button::R);
        assert_ne!(bench.actor.action, Action::Skill);
        assert!(bench.slots.get(0).is_some_and(|slot| slot.ready()));
        // Nothing comes of the button of a slot with nothing in it.
        bench.step(button::R | button::CIRCLE, button::CIRCLE);
        bench.step(button::R | button::CROSS, button::CROSS);
        assert_ne!(bench.actor.action, Action::Skill);
        // With R held, triangle uses what is on it: the skill starts over, and
        // everything of hers is locked for its scene.
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_eq!(bench.actor.action, Action::Skill);
        assert!(matches!(bench.actor.state, Some(State::Skill { phase: 0, id: 25, line: 34, effects: [1600, 0, 0], .. })));
        assert!(bench.slots.get(0).is_some_and(|slot| !slot.ready() && slot.share == 0.0));
        assert_eq!(bench.actor.locks, lock::BUSY | lock::EVADING | lock::ACTIONS | lock::ITEMS | lock::HEAT_RECOVERY);

        // One that is not ready is not used, unless the battle's values have every
        // skill ready always.
        let mut bench = Bench::new();
        bench.slots = slots([25, 0, 0, 0]);
        bench.slots.used(0);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_ne!(bench.actor.action, Action::Skill);
        bench.tunables.set(id::SKILLS_ALWAYS_READY, 1.0);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_eq!(bench.actor.action, Action::Skill);
    }

    #[test]
    fn she_cannot_use_a_skill_overheated_stunned_or_kept_from_it() {
        let mut bench = Bench::new();
        bench.slots = slots([25, 0, 0, 0]);
        bench.idle(2);
        assert!((1..72).all(|skill| bench.actor.can_use(skill)));
        // Overheated she can use none, and the one whose button she presses stays
        // ready.
        bench.actor.heat.overheated = true;
        assert!(!bench.actor.can_use(25) && !bench.actor.can_use(1));
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_ne!(bench.actor.action, Action::Skill);
        assert!(bench.slots.get(0).is_some_and(|slot| slot.ready()));
        bench.actor.heat.overheated = false;
        // Nor can she stunned.
        bench.afflicted(900);
        assert!(!bench.actor.can_use(25));
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_ne!(bench.actor.action, Action::Skill);

        // Kept from dodging she cannot use the four that dodge, and can use the
        // others.
        let mut bench = Bench::new();
        bench.idle(2);
        bench.afflicted(800);
        assert_eq!((1..=8).filter(|&skill| !bench.actor.can_use(skill)).collect::<Vec<i32>>(), [3, 4, 5, 6]);
        assert!(bench.actor.can_use(25));
        // Kept from shooting she cannot use those that shoot, and kept from her
        // skills none of them.
        let ailment = |id, kind| Effect { id, kind, chance: 100, duration: 300, ..Effect::default() };
        let giver = Giver { level: 5, special: 50, boost: [0; 3], name: "Eater".into() };
        let mut bench = Bench::new();
        assert_eq!(bench.actor.afflict(&ailment(700, kind::NO_SHOT), &giver, &stats(), &mut Random::new(4)), status::Outcome::Given);
        assert!(!bench.actor.can_use(1) && !bench.actor.can_use(22) && bench.actor.can_use(9) && bench.actor.can_use(25));
        let mut bench = Bench::new();
        assert_eq!(bench.actor.afflict(&ailment(1000, kind::NO_SKILL), &giver, &stats(), &mut Random::new(4)), status::Outcome::Given);
        assert!((1..72).all(|skill| !bench.actor.can_use(skill)));
    }

    #[test]
    fn a_skill_that_helps_her_is_a_scene_of_its_own() {
        let mut bench = Bench::new();
        bench.slots = slots([25, 0, 0, 0]);
        bench.idle(2);
        // Defender: its state's first step takes the stage, and the last of its
        // hundred and sixty sixtieths gives it back.
        let asked = bench.scene(80);
        assert_eq!(
            asked,
            vec![
                (0, Event::Dim { to: 196, time: DIM_TIME }),
                (0, Event::Camera(Staged::Skill(25))),
                (0, Event::Lens { near: 50.0, far: 20000.0 }),
                (32, Event::SkillVoice { skill: 25, name: "brs_2400" }),
                // Its effects: one that goes with a node of hers, of the last
                // draw list, and one left where she stands. Both are no more as
                // the scene ends.
                (36, made(10120, Hold::With(Some(60)), 4)),
                (37, Event::Sound(10)),
                (38, Event::Lens { near: 45.0, far: 20000.0 }),
                (38, made(10121, Hold::Left, 4)),
                (80, Event::Give(1600)),
                (80, Event::EffectsOver(10120)),
                (80, Event::EffectsOver(10121)),
                (80, Event::Dim { to: 0, time: DIM_TIME }),
                (80, Event::CameraBack),
            ]
        );
        // The log has named it, once.
        assert_eq!(bench.actor.status.said, vec![Line { message: 34, names: Names::of(NAME) }]);
        // The step that ends it has her standing again, free to act, with the
        // standing animation coming over what she was in.
        assert!(!bench.actor.staged);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
        assert!(bench.animator.body().is(&bench.clips.idle));
        // And she can do something else at once.
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.actor.action, Action::Shoot);

        // Skill Boost is not dimmed, and the camera comes closer for it.
        let mut bench = Bench::new();
        bench.slots = slots([24, 0, 0, 0]);
        bench.idle(2);
        let asked = bench.scene(80);
        assert_eq!(
            asked,
            vec![
                (0, Event::Camera(Staged::Skill(24))),
                (0, Event::Lens { near: 35.0, far: 20000.0 }),
                (0, made(10170, Hold::With(Some(60)), 4)),
                (35, Event::SkillVoice { skill: 24, name: "brs_1001" }),
                (36, made(10171, Hold::Left, 3)),
                (40, Event::Sound(55)),
                (80, Event::Give(1900)),
                (80, Event::EffectsOver(10170)),
                (80, Event::EffectsOver(10171)),
                (80, Event::CameraBack),
            ]
        );

        // Ignition takes more than six seconds, gives her nothing, and has the
        // skills of her other slots ready, each of which is heard.
        let mut bench = Bench::new();
        bench.slots = slots([29, 25, 1, 0]);
        bench.slots.used(1);
        bench.slots.used(2);
        bench.slots.ready_now.clear();
        bench.idle(2);
        let asked = bench.scene(186);
        assert_eq!(asked.len(), 20);
        assert_eq!(asked[..3], [(0, Event::Dim { to: 196, time: DIM_TIME }), (0, Event::Camera(Staged::Skill(29))), (0, Event::Lens { near: 20.0, far: 20000.0 })]);
        // One effect put nowhere from the start, and two left where she stands.
        assert_eq!(asked[3], (0, made(10182, Hold::Nowhere, 4)));
        assert_eq!(asked[7..9], [(111, made(10180, Hold::Left, 4)), (120, made(10181, Hold::Left, 3))]);
        assert_eq!(asked[9..11], [(125, Event::SkillVoice { skill: 29, name: "brs_2200" }), (125, Event::SkillSound { skill: 29, sound: 0 })]);
        assert_eq!(
            asked[11..],
            [
                // They are no more, and what is seen of a gift of HP goes with
                // her in their stead.
                (186, Event::EffectsOver(10180)),
                (186, Event::EffectsOver(10181)),
                (186, Event::EffectsOver(10182)),
                (186, made(998042, Hold::With(None), 3)),
                (186, Event::Sound(skills::SOUND_OTHERS_READY)),
                (186, Event::Sound(skills::SOUND_READY)),
                (186, Event::Sound(skills::SOUND_READY)),
                (186, Event::Dim { to: 0, time: DIM_TIME }),
                (186, Event::CameraBack),
            ]
        );
        assert!(bench.slots.get(1).is_some_and(|slot| slot.ready()) && bench.slots.get(2).is_some_and(|slot| slot.ready()));
        assert!(bench.slots.get(0).is_some_and(|slot| !slot.ready()));
        assert_eq!(bench.slots.ready_now, [2, 3]);
    }

    #[test]
    fn charge_shot_is_a_scene_and_then_a_shot_over_the_battle() {
        let mut bench = Bench::new();
        bench.slots = slots([1, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        // Her action is the one the skills that shoot have, which an enemy's
        // script can ask after.
        assert_eq!(bench.actor.action, Action::SkillShot);
        let mut asked = Vec::new();
        for step in 0..=122 {
            // She has the stage until the shot leaves, a hundred and fifty
            // sixtieths in, and not after.
            assert_eq!(bench.actor.staged, (1..=75).contains(&step), "step {step}");
            bench.events.clear();
            bench.step(0, 0);
            asked.extend(bench.events.iter().map(|&event| (step, event)));
            // From then she is turned to where she aims, until a little before
            // she stands again.
            assert_eq!(bench.actor.aiming, (75..118).contains(&step), "step {step}");
        }
        assert_eq!(
            asked,
            vec![
                (0, Event::Dim { to: 196, time: DIM_TIME }),
                (0, Event::Camera(Staged::Skill(1))),
                (0, Event::Parts(4)),
                (0, Event::Parts(5)),
                (15, Event::Sound(144)),
                (17, Event::SkillVoice { skill: 1, name: "brs_1003" }),
                (23, Event::Lens { near: 20.0, far: 20000.0 }),
                // What gathers at her muzzle is of the last draw list while the
                // stage is hers, and of the fighters' own once the shot has
                // left, with what is seen about her then.
                (34, made(10001, Hold::With(Some(MUZZLE_NODE)), 4)),
                (38, Event::Sound(143)),
                (68, Event::SkillVoice { skill: 1, name: "brs_2600" }),
                (73, Event::Sound(176)),
                (75, Event::CameraBack),
                (75, Event::SkillShot { skill: 1, firing: Firing::Straight }),
                (75, made(10003, Hold::With(None), 3)),
                (75, Event::EffectList(10001, 3)),
                (85, Event::Dim { to: 0, time: DIM_TIME }),
            ]
        );
        // Neither outlasts the skill's state.
        assert_eq!(bench.actor.effects_over, [10001, 10003]);
        assert_eq!(bench.actor.status.said, vec![Line { message: 10, names: Names::of(NAME) }]);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
        assert!(!bench.slots.get(0).unwrap().ready());

        // A hit after the stage is given back takes its HP off her and does not
        // make her reel: a skill's state has her as hard to move as a dodge does
        // (the lock at her + 0x754), so what is still to come does come.
        let mut bench = Bench::new();
        bench.slots = slots([1, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        bench.idle(78);
        assert!(!bench.actor.staged && bench.actor.aiming);
        bench.events.clear();
        let outcome = bench.struck(&Attack { strength: 5, ..blow(0) }, &Status::default());
        assert_eq!((outcome.damage, outcome.guarded, bench.actor.action), (30, false, Action::SkillShot));
        bench.idle(50);
        assert_eq!(bench.events, vec![Event::Dim { to: 0, time: DIM_TIME }]);
        assert_eq!((bench.actor.action, bench.actor.hp), (Action::Idle, 70.0));
    }

    #[test]
    fn tracer_gun_has_her_take_its_gun_and_fire_guided_bullets() {
        let mut bench = Bench::new();
        bench.slots = slots([14, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_eq!(bench.actor.action, Action::SkillShot);
        let mut asked = Vec::new();
        for step in 0..=140 {
            assert_eq!(bench.actor.staged, (1..=81).contains(&step), "step {step}");
            bench.events.clear();
            bench.step(0, 0);
            asked.extend(bench.events.iter().map(|&event| (step, event)));
            assert_eq!(bench.actor.aiming, (81..135).contains(&step), "step {step}");
        }
        assert_eq!(
            asked,
            vec![
                (0, Event::Dim { to: 196, time: DIM_TIME }),
                (0, Event::Camera(Staged::Skill(14))),
                (0, Event::Lens { near: 20.0, far: 20000.0 }),
                (0, on_weapon(10090, skills::GUN_NODE)),
                (0, made(10091, Hold::Nowhere, 3)),
                (16, Event::Sound(132)),
                // She speaks, and has the gun: her left hand closed for it, her
                // right hand empty of the cannon and then with the gun in it.
                (40, Event::SkillVoice { skill: 14, name: "brs_1005" }),
                (40, Event::Parts(3)),
                (40, Event::Parts(0)),
                (40, Event::Weapon { skill: 14, node: skills::GUN_NODE }),
                (48, Event::Sound(144)),
                (48, Event::Sound(164)),
                (81, Event::CameraBack),
                (94, Event::SkillVoice { skill: 14, name: "brs_4400" }),
                (100, made(998003, Hold::With(Some(MUZZLE_NODE)), 3)),
                (100, Event::SkillShot { skill: 14, firing: Firing::Guided }),
                (101, Event::Sound(176)),
                (125, on_weapon(10092, skills::GUN_NODE)),
                (125, Event::Sound(108)),
                (125, Event::Dim { to: 0, time: DIM_TIME }),
                // And the cannon back.
                (130, Event::Parts(3)),
                (130, Event::Parts(5)),
            ]
        );
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
    }

    #[test]
    fn vulcannon_fires_a_volley_of_rays_at_where_she_aims_once_the_stage_is_the_battles_again() {
        let mut bench = Bench::new();
        bench.slots = slots([10, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_eq!(bench.actor.action, Action::SkillShot);
        // (The bench has no table of bullets for the count of the volley to come
        // from: it is put in by hand. The game's has thirty-five.)
        match &mut bench.actor.state {
            Some(State::Skill { id: 10, clock, .. }) => clock.shots = 4,
            other => panic!("{other:?}"),
        }
        let mut asked = Vec::new();
        for step in 0..=86 {
            // She has the stage for a hundred and twenty-eight sixtieths.
            assert_eq!(bench.actor.staged, (1..=64).contains(&step), "step {step}");
            bench.events.clear();
            bench.step(0, 0);
            asked.extend(bench.events.iter().map(|&event| (step, event)));
            // From the first ray until the last has gone she is turned to where
            // she aims, anew for each.
            assert_eq!(bench.actor.aiming, (64..76).contains(&step), "step {step}");
        }
        assert_eq!(
            asked,
            vec![
                (0, Event::Dim { to: 196, time: DIM_TIME }),
                (0, Event::Camera(Staged::Skill(10))),
                (0, Event::Lens { near: 20.0, far: 20000.0 }),
                (0, on_weapon(10060, skills::GUN_NODE)),
                (1, made(10061, Hold::Nowhere, 3)),
                (8, Event::Sound(132)),
                (14, Event::Parts(3)),
                (14, Event::Parts(0)),
                (14, Event::Weapon { skill: 10, node: skills::GUN_NODE }),
                (25, Event::SkillVoice { skill: 10, name: "brs_2800" }),
                (30, Event::Sound(164)),
                (30, Event::Sound(144)),
                (32, Event::Lens { near: 20.0, far: 20000.0 }),
                // The stage is given back with the map still dimmed, and the
                // cursor is let go of the lock points for the volley: a ray every
                // six sixtieths.
                (64, Event::EffectsOver(10060)),
                (64, Event::EffectsOver(10061)),
                (64, Event::CameraBack),
                (64, Event::CursorFree(true)),
                (64, Event::SkillRay { skill: 10 }),
                (67, Event::SkillRay { skill: 10 }),
                (70, Event::SkillRay { skill: 10 }),
                (73, Event::SkillRay { skill: 10 }),
                // Six after the last, it is over: the cannon back, and the map
                // lit again over twenty, after which she stands.
                (76, Event::CursorFree(false)),
                (76, on_weapon(10062, skills::GUN_NODE)),
                (76, Event::Parts(3)),
                (76, Event::Parts(5)),
                (76, Event::Sound(108)),
                (76, Event::Dim { to: 0, time: DIM_TIME }),
            ]
        );
        assert_eq!(bench.actor.status.said, vec![Line { message: 19, names: Names::of(NAME) }]);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
        // With nothing to its volley the end comes as the stage is given back, and
        // she stands twenty sixtieths on.
        let mut bench = Bench::new();
        bench.slots = slots([10, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        bench.idle(74);
        assert_eq!(bench.actor.action, Action::SkillShot);
        bench.idle(1);
        assert_eq!(bench.actor.action, Action::Idle);
    }

    /// A bench with Stun Snipe on triangle, used with the cursor on gauge 1 of an
    /// enemy, or on nothing; and the steps of its state run so far.
    fn sniping(locked: bool) -> (Bench, Entity) {
        let mut world = World::new();
        let enemy = world.spawn_empty().id();
        let mut bench = Bench::new();
        bench.slots = slots([7, 0, 0, 0]);
        bench.places = vec![(enemy, Vec3::new(100.0, 0.0, 300.0), true)];
        bench.lock = locked.then_some(LockOn { enemy, gauge: 1, open: true });
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        // It is a skill like the others, but its slot has not started over.
        assert_eq!(bench.actor.action, Action::Skill);
        assert!(bench.slots.get(0).is_some_and(|slot| slot.ready()));
        (bench, enemy)
    }

    #[test]
    fn a_snipe_has_its_scope_come_over_what_she_had_locked_on_to_and_waits_for_square() {
        let (mut bench, enemy) = sniping(true);
        let mut asked = Vec::new();
        let mut run = |bench: &mut Bench, steps: std::ops::Range<usize>, pressed: u32| {
            for step in steps {
                bench.events.clear();
                bench.step(pressed, pressed);
                asked.extend(bench.events.iter().map(|&event| (step, event)));
                // The stage is hers all through it, the scope up from a hundred
                // and two sixtieths in, and only her enemy to be seen from then.
                let up = (51..121).contains(&step);
                assert_eq!((bench.actor.staged, bench.actor.scoped), (step < 121, up), "step {step}");
                assert_eq!(bench.actor.shown, if up { Shown::Only(enemy) } else { Shown::Nobody }, "step {step}");
            }
        };
        // Square before the window is open is nothing.
        run(&mut bench, 0..60, 0);
        run(&mut bench, 60..61, button::SQUARE);
        run(&mut bench, 61..71, 0);
        assert!(bench.slots.get(0).is_some_and(|slot| slot.ready()));
        // In the window it fires: the slot starts over, and the part her lock
        // was on is struck. Once only.
        run(&mut bench, 71..72, button::SQUARE);
        assert!(bench.slots.get(0).is_some_and(|slot| !slot.ready()));
        run(&mut bench, 72..73, button::SQUARE);
        // The window is open for two seconds from when the sight has locked on.
        run(&mut bench, 73..132, 0);
        assert_eq!(
            asked,
            vec![
                (0, Event::Dim { to: 196, time: DIM_TIME }),
                (0, Event::Camera(Staged::Skill(7))),
                (0, Event::Parts(3)),
                (0, Event::Parts(0)),
                (0, Event::Weapon { skill: 7, node: skills::GUN_NODE }),
                (0, on_weapon(10020, skills::GUN_NODE)),
                (0, made(10021, Hold::Nowhere, 3)),
                (5, Event::Sound(132)),
                (18, Event::SkillVoice { skill: 7, name: "brs_0001" }),
                (25, Event::Sound(164)),
                (25, Event::Sound(144)),
                // The camera's move is over, she is not drawn, and the scope
                // comes; ten sixtieths on the sight starts to lock on, and ten
                // after that the window opens.
                (51, Event::CameraBack),
                (51, Event::Hide(true)),
                (51, Event::Scope(Some((enemy, 1)))),
                (56, Event::SightLock),
                (71, Event::Strike { skill: 7, blow: Blow::Part(enemy, 1) }),
                (121, Event::ScopeOff),
                (121, Event::Hide(false)),
                (121, Event::Parts(3)),
                (121, Event::Parts(5)),
                (121, Event::Dim { to: 0, time: DIM_TIME }),
            ]
        );
        assert_eq!(bench.actor.status.said, vec![Line { message: 16, names: Names::of(NAME) }]);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
    }

    #[test]
    fn a_snipe_that_is_not_fired_costs_nothing_and_cross_leaves_it_early() {
        // Not fired: the window closes of itself, and the skill is as ready as it
        // was.
        let (mut bench, _) = sniping(true);
        bench.idle(132);
        assert_eq!(bench.actor.action, Action::Idle);
        assert!(bench.slots.get(0).is_some_and(|slot| slot.ready()));

        // Cross closes the window, fired or not.
        let (mut bench, _) = sniping(true);
        bench.idle(70);
        assert!(bench.actor.scoped);
        bench.step(button::CROSS, button::CROSS);
        assert!(!bench.actor.scoped && !bench.actor.staged);
        bench.idle(10);
        assert_eq!(bench.actor.action, Action::Idle);
        // (Before the window is open cross is nothing.)
        let (mut bench, _) = sniping(true);
        bench.idle(55);
        bench.step(button::CROSS, button::CROSS);
        assert!(bench.actor.scoped);

        // With nothing locked on to the scope still comes, over nothing: the log
        // says that the lock has failed, the window opens at once, and square
        // fires nothing and uses nothing up.
        let (mut bench, _) = sniping(false);
        bench.events.clear();
        bench.idle(56);
        assert!(bench.events.contains(&Event::Scope(None)) && !bench.events.contains(&Event::SightLock));
        assert_eq!(bench.actor.shown, Shown::Nobody);
        assert_eq!(bench.actor.status.said.len(), 1);
        bench.idle(1);
        assert_eq!(bench.actor.status.said.last(), Some(&Line { message: SNIPE_FAILED_LINE, names: Names::of(NAME) }));
        bench.events.clear();
        bench.step(button::SQUARE, button::SQUARE);
        assert!(bench.events.is_empty() && bench.slots.get(0).is_some_and(|slot| slot.ready()));
        // Sixty steps after the window opened it closes.
        bench.idle(58);
        assert!(bench.actor.scoped);
        bench.idle(1);
        assert!(!bench.actor.scoped);
    }

    #[test]
    fn star_comet_throws_its_bomb_once_the_stage_is_the_battles_again() {
        let mut bench = Bench::new();
        bench.slots = slots([18, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_eq!(bench.actor.action, Action::SkillShot);
        let mut asked = Vec::new();
        for step in 0..=130 {
            assert_eq!(bench.actor.staged, (1..=76).contains(&step), "step {step}");
            bench.events.clear();
            bench.step(0, 0);
            asked.extend(bench.events.iter().map(|&event| (step, event)));
            // She is turned to where she aims as she throws, and until she stands.
            assert_eq!(bench.actor.aiming, (96..130).contains(&step), "step {step}");
        }
        let at = |event: Event| asked.iter().find(|(_, other)| *other == event).map(|(step, _)| *step);
        assert_eq!(at(Event::Weapon { skill: 18, node: skills::GUN_NODE }), Some(46));
        assert_eq!(at(Event::CameraBack), Some(76));
        // Forty sixtieths after the stage is given back, with a sound of its own.
        assert_eq!((at(Event::Sound(175)), at(Event::SkillShot { skill: 18, firing: Firing::Lobbed })), (Some(96), Some(96)));
        assert_eq!(at(Event::Dim { to: 0, time: DIM_TIME }), Some(120));
        assert_eq!(bench.actor.status.said, vec![Line { message: 27, names: Names::of(NAME) }]);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
    }

    #[test]
    fn steel_cutter_shows_the_enemies_again_and_strikes_them_all_while_she_has_the_stage() {
        let mut bench = Bench::new();
        bench.slots = slots([20, 0, 0, 0]);
        bench.idle(2);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        // It is not one of those that shoot.
        assert_eq!(bench.actor.action, Action::Skill);
        let mut asked = Vec::new();
        for step in 0..=160 {
            assert_eq!(bench.actor.staged, (1..=150).contains(&step), "step {step}");
            bench.events.clear();
            bench.step(0, 0);
            asked.extend(bench.events.iter().map(|&event| (step, event)));
            // The enemies are held all the while, and out of sight until a little
            // before the blow.
            assert_eq!(bench.actor.shown, if (82..150).contains(&step) { Shown::Everybody } else { Shown::Nobody }, "step {step}");
            // She stands where she stood all through it.
            assert_eq!(bench.actor.placed, None);
        }
        let at = |event: Event| asked.iter().find(|(_, other)| *other == event).map(|(step, _)| *step);
        // Its sword is at a node of her model that is not her cannon's.
        assert_eq!(at(Event::Weapon { skill: 20, node: skills::SWORD_NODE }), Some(0));
        assert_eq!(at(Event::Strike { skill: 20, blow: Blow::All }), Some(100));
        assert_eq!(at(Event::CameraBack), Some(150));
        assert_eq!(asked[asked.len() - 3..], [(150, Event::Parts(3)), (150, Event::Parts(5)), (150, Event::Dim { to: 0, time: DIM_TIME })]);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
    }

    #[test]
    fn blade_kill_is_for_the_enemy_she_has_locked_on_to_and_cuts_it_down_where_it_stands() {
        let mut world = World::new();
        let (enemy, other) = (world.spawn_empty().id(), world.spawn_empty().id());
        let mut bench = Bench::new();
        bench.slots = slots([4, 0, 0, 0]);
        bench.places = vec![(other, Vec3::new(-300.0, 0.0, 200.0), true), (enemy, Vec3::new(250.0, 40.0, 600.0), true)];
        bench.idle(2);
        let home = bench.actor.transform();
        // With nothing locked on to nothing comes of its button, nor with an
        // enemy that has nothing of it to be struck; and it stays ready.
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        bench.lock = Some(LockOn { enemy, gauge: 0, open: false });
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_ne!(bench.actor.action, Action::Skill);
        assert!(bench.slots.get(0).is_some_and(|slot| slot.ready()));
        bench.lock = Some(LockOn { enemy, gauge: 0, open: true });
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        assert_eq!(bench.actor.action, Action::Skill);
        assert!(matches!(bench.actor.state, Some(State::Skill { id: 4, target: Some((target, 0)), .. }) if target == enemy));
        // The scene is for the enemy it began with, whatever her cursor goes to.
        bench.lock = Some(LockOn { enemy: other, gauge: 0, open: true });
        let mut asked = Vec::new();
        for step in 0..=110 {
            assert_eq!(bench.actor.staged, (1..=110).contains(&step), "step {step}");
            bench.events.clear();
            bench.step(0, 0);
            asked.extend(bench.events.iter().map(|&event| (step, event)));
            // Her model is at the middle of the map as she draws the blade, and
            // where the enemy stands, on the ground, from the second part of the
            // scene on: turned half round in both. Only that enemy is seen again.
            let stands = bench.actor.transform();
            match step {
                0..=70 => assert_eq!((stands.translation, bench.actor.shown), (Vec3::ZERO, Shown::Nobody), "step {step}"),
                71..=109 => assert_eq!((stands.translation, bench.actor.shown), (Vec3::new(250.0, 0.0, 600.0), Shown::Only(enemy)), "step {step}"),
                _ => assert_eq!((stands, bench.actor.shown), (home, Shown::Nobody), "step {step}"),
            }
            if step < 110 {
                assert!((stands.rotation * Vec3::Z).distance(Vec3::NEG_Z) < 1e-5 && (stands.rotation * Vec3::Y).distance(Vec3::Y) < 1e-5, "step {step}");
            }
        }
        assert_eq!(
            asked,
            vec![
                (0, Event::Dim { to: 196, time: DIM_TIME }),
                // Her right hand is shaped for the blade, which is at a node of
                // its own.
                (0, Event::Parts(1)),
                (0, Event::Parts(3)),
                (0, Event::Weapon { skill: 4, node: skills::BLADE_NODE }),
                (0, Event::Camera(Staged::Skill(4))),
                (10, Event::SkillVoice { skill: 4, name: "brs_1004" }),
                (12, Event::Sound(168)),
                (39, Event::Sound(105)),
                (71, Event::Camera(Staged::SkillSecond(4))),
                (71, made(10041, Hold::With(None), 3)),
                (71, made(10043, Hold::With(Some(skills::BLADE_NODE)), 3)),
                (75, Event::SkillVoice { skill: 4, name: "brs_5000" }),
                (77, Event::Sound(181)),
                // (Two that its own archive has not got.)
                (81, made(10132, Hold::Nowhere, 3)),
                (81, Event::Strike { skill: 4, blow: Blow::One(enemy) }),
                (83, made(10142, Hold::With(None), 3)),
                (100, Event::Dim { to: 0, time: DIM_TIME }),
                (110, Event::CameraBack),
                (110, Event::Parts(5)),
                (110, Event::Parts(3)),
            ]
        );
        assert_eq!(bench.actor.status.said, vec![Line { message: 13, names: Names::of(NAME) }]);
        assert_eq!(bench.actor.effects_over, [10041, 10043, 10132, 10142]);
        assert_eq!((bench.actor.action, bench.actor.locks), (Action::Idle, lock::BUSY));
        assert!(!bench.slots.get(0).unwrap().ready());
    }

    #[test]
    fn a_boost_is_spent_by_the_skill_it_is_used_on_and_nothing_of_hers_runs_out_in_a_scene() {
        let boost = Effect { id: 1900, kind: kind::BOOST, chance: 100, values: [50, 50, 50, 0], ..Effect::default() };
        let giver = Giver { level: 5, special: 50, boost: [0; 3], name: NAME.into() };
        // Defender spends the boost she has, as its state gives way to the next.
        let mut bench = Bench::new();
        bench.slots = slots([25, 0, 0, 0]);
        bench.idle(2);
        assert_eq!(bench.actor.afflict(&boost, &giver, &stats(), &mut Random::new(4)), status::Outcome::Given);
        // Three seconds of having no heat, which the scene's two and two thirds
        // would all but use up if they counted.
        bench.afflicted(2200);
        bench.step(button::R | button::TRIANGLE, button::TRIANGLE);
        for step in 0..80 {
            bench.step(0, 0);
            assert!(bench.actor.status.has(kind::BOOST), "step {step}");
        }
        bench.step(0, 0);
        assert!(!bench.actor.status.has(kind::BOOST));
        // What she had before the scene has lost only the two frames at its start.
        assert!(bench.actor.status.has(kind::NO_HEAT));
        bench.idle(80);
        assert!(bench.actor.status.has(kind::NO_HEAT));
        bench.idle(10);
        assert!(!bench.actor.status.has(kind::NO_HEAT));

        // Skill Boost, which gives that boost, does not spend one.
        let mut bench = Bench::new();
        bench.slots = slots([24, 0, 0, 0]);
        bench.idle(2);
        bench.actor.afflict(&boost, &giver, &stats(), &mut Random::new(4));
        bench.scene(80);
        assert_eq!(bench.actor.action, Action::Idle);
        assert!(bench.actor.status.has(kind::BOOST));
    }

    /// One of her skills that act of themselves, as the game's table has it;
    /// `always` has it act whenever it can instead of by chance.
    fn of_itself(id: i32, always: bool) -> skills::Skill {
        let mut auto = match id {
            34 => [2, 3, 25, 6, 1608, 0],
            36 => [3, 1, 10, 4, 0, 0],
            37 => [6, 4, 0, 5, 501, 0],
            38 => [4, 1, 10, 4, 0, 0],
            _ => panic!("skill {id} is not one of them"),
        };
        if always {
            auto[1] = 4;
        }
        skills::Skill::of_itself(id, auto)
    }

    /// A battle in which she has those skills, standing ready.
    fn with_autos(skills: Vec<skills::Skill>) -> Bench {
        let mut bench = Bench::new();
        bench.actor.autos = Autos::new(skills);
        bench.idle(2);
        bench.events.clear();
        bench
    }

    const SHOWN_ACTING: [Event; 2] = [Event::Sound(auto::SOUND), Event::Effect { id: auto::EFFECT, place: Place::HER, on_top: true }];

    #[test]
    fn a_skill_of_hers_can_spare_her_the_heat_of_a_burst_or_of_a_dodge() {
        // As it is a burst heats her by ten, and a dodge by fifteen.
        let mut bench = with_autos(Vec::new());
        bench.step(button::SQUARE, button::SQUARE);
        bench.idle(1);
        assert_eq!(bench.actor.heat.value, 10.0);
        let mut bench = with_autos(Vec::new());
        bench.step(button::CIRCLE, button::CIRCLE);
        bench.idle(1);
        assert_eq!(bench.actor.heat.value, 15.0);

        // Attack-Free acting, the burst costs none: it is heard and seen as she
        // begins, and the bullets leave as ever.
        let mut bench = with_autos(vec![of_itself(36, true)]);
        bench.step(button::SQUARE, button::SQUARE);
        assert_eq!(bench.events, SHOWN_ACTING);
        assert_eq!(bench.shots(40).len(), 6);
        assert_eq!(bench.actor.heat.value, 0.0);
        // It is nothing to a dodge.
        let mut bench = with_autos(vec![of_itself(36, true)]);
        bench.step(button::CIRCLE, button::CIRCLE);
        assert!(bench.events.is_empty());
        bench.idle(1);
        assert_eq!(bench.actor.heat.value, 15.0);

        // Sway Three is the same for a dodge, and nothing to a burst.
        let mut bench = with_autos(vec![of_itself(38, true)]);
        bench.step(button::CIRCLE, button::CIRCLE);
        assert_eq!(bench.events, SHOWN_ACTING);
        bench.idle(1);
        assert_eq!((bench.actor.action, bench.actor.heat.value), (Action::Dodge, 0.0));
        let mut bench = with_autos(vec![of_itself(38, true)]);
        bench.step(button::SQUARE, button::SQUARE);
        bench.idle(1);
        assert_eq!(bench.actor.heat.value, 10.0);

        // A dodge straight out of a burst that cost nothing costs what a dodge
        // does: the burst is done with before the dodge is made.
        let mut bench = with_autos(vec![of_itself(36, true)]);
        bench.step(button::SQUARE, button::SQUARE);
        for _ in 0..80 {
            if bench.actor.action == Action::Dodge {
                break;
            }
            bench.step(button::CIRCLE, button::CIRCLE);
        }
        bench.idle(1);
        assert_eq!((bench.actor.action, bench.actor.heat.value), (Action::Dodge, 15.0));

        // By chance, one burst in ten or so.
        let mut bench = with_autos(vec![of_itself(36, false)]);
        let mut spared = 0;
        for _ in 0..300 {
            bench.actor.heat.value = 0.0;
            bench.step(button::SQUARE, button::SQUARE);
            bench.idle(1);
            spared += usize::from(bench.actor.heat.value == 0.0);
            bench.idle(60);
            assert_eq!(bench.actor.action, Action::AimIdle);
        }
        assert!((15..=45).contains(&spared), "{spared}");
    }

    #[test]
    fn a_guard_begun_two_frames_before_a_hit_is_a_better_one_if_she_has_the_skill() {
        let nobody = Status::default();
        // A guard of so many frames, counting the one it began on.
        let guard = |bench: &mut Bench, frames: usize| {
            bench.step(button::CROSS, button::CROSS);
            for _ in 1..frames {
                bench.step(button::CROSS, 0);
            }
            assert_eq!(bench.actor.action, Action::Guard);
        };
        // The usual guard keeps half of a hit's thirty off her, whenever it lands.
        let mut bench = with_autos(Vec::new());
        guard(&mut bench, 3);
        let outcome = bench.struck(&blow(0), &nobody);
        assert_eq!((outcome.damage, outcome.guarded), (15, true));
        assert!(bench.actor.status.heard.is_empty());

        // With Danger Guard a hit on the second frame after the guard began takes
        // a fifth of that, and she is given a tenth of her HP before it does.
        let mut bench = with_autos(vec![of_itself(37, false)]);
        bench.actor.hp = 50.0;
        guard(&mut bench, 3);
        let outcome = bench.struck(&blow(0), &nobody);
        assert_eq!((outcome.damage, outcome.guarded, outcome.reeled), (3, true, false));
        assert_eq!(bench.actor.hp, 57.0);
        assert_eq!(bench.actor.status.heard, [auto::SOUND]);
        // Another on that frame is guarded as well, and not heard again; a frame
        // later her guard is the usual one.
        assert_eq!(bench.struck(&blow(0), &nobody).damage, 3);
        assert_eq!((bench.actor.hp, bench.actor.status.heard.len()), (64.0, 1));
        bench.step(button::CROSS, 0);
        assert_eq!(bench.struck(&blow(0), &nobody).damage, 15);
        assert_eq!(bench.actor.hp, 49.0);

        // A frame too soon, or too late, it is the usual one from the first.
        for frames in [1, 2, 4, 9] {
            let mut bench = with_autos(vec![of_itself(37, false)]);
            guard(&mut bench, frames);
            assert_eq!(bench.struck(&blow(0), &nobody).damage, 15, "after {frames} frames");
        }
        // An attack that cannot be guarded is still met with that record: it
        // takes a fifth and gives her the HP, and she reels all the same.
        let mut bench = with_autos(vec![of_itself(37, false)]);
        bench.actor.hp = 50.0;
        guard(&mut bench, 3);
        let outcome = bench.struck(&Attack { unguardable: true, ..blow(0) }, &nobody);
        assert_eq!((outcome.damage, outcome.guarded, outcome.reeled), (6, false, true));
        assert_eq!(bench.actor.hp, 54.0);
        // Letting the guard go ends it: a new one counts its own frames.
        let mut bench = with_autos(vec![of_itself(37, false)]);
        guard(&mut bench, 2);
        bench.idle(30);
        assert_ne!(bench.actor.action, Action::Guard);
        guard(&mut bench, 3);
        assert_eq!(bench.struck(&blow(0), &nobody).damage, 3);
    }

    #[test]
    fn a_skill_of_hers_gives_her_its_effect_once_her_hp_is_low() {
        let gives = |bench: &Bench| bench.events.iter().filter(|event| matches!(event, Event::Give(_))).count();
        let mut bench = with_autos(vec![of_itself(34, false)]);
        bench.actor.hp = 26.0;
        bench.idle(5);
        assert!(bench.events.is_empty());
        // At a quarter of her HP Dead Guard gives her its effect, heard and seen.
        bench.actor.hp = 25.0;
        bench.idle(1);
        assert_eq!(bench.events[0], Event::Give(1608));
        assert_eq!(bench.events[1..], SHOWN_ACTING);
        // Once in a battle, however low she goes.
        bench.actor.hp = 5.0;
        bench.idle(20);
        assert_eq!(gives(&bench), 1);

        // Not while somebody has the stage.
        let mut bench = with_autos(vec![of_itself(34, false)]);
        bench.actor.hp = 10.0;
        bench.actor.staged = true;
        bench.idle(5);
        assert_eq!(gives(&bench), 0);
        bench.actor.staged = false;
        bench.idle(1);
        assert_eq!(gives(&bench), 1);
    }

    #[test]
    fn heat_cools_after_the_wait() {
        let tunables = tunables();
        let mut heat = Heat { max: 100.0, value: 0.0, wait: 0.0, overheated: false };
        heat.add(10.0, false, &tunables);
        for _ in 0..14 {
            heat.recover(2.0, &tunables);
        }
        assert_eq!(heat.value, 10.0);
        // The step on which the wait runs out already cools.
        heat.recover(2.0, &tunables);
        assert!((heat.value - (10.0 - 2.0 * 22.05 / 60.0)).abs() < 1e-4);
    }

    #[test]
    fn facing_matches_a_turn_about_the_vertical() {
        let forward = Vec3::new(0.6, 0.0, 0.8);
        let turned = facing(forward) * Vec3::Z;
        assert!(turned.distance(forward) < 1e-5);
        // Up cross forward: +X for a character facing +Z.
        assert!((facing(Vec3::Z) * Vec3::X).distance(Vec3::X) < 1e-5);
    }
}
