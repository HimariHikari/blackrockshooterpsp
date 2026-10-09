//! Battles: the player at one of two standing places facing the enemies in the middle
//! of a stage, seen over her shoulder.
//!
//! The logic is a port of the original's battle classes, one logic frame per fixed
//! step; see `docs/BATTLE_NOTES.md` for where each piece was read from.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{ensure, Context as _, Result};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::Camera3dDepthLoadOp;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use brs_engine::animation::{Animator, Clip};
use brs_engine::camera::CameraClip;
use brs_engine::canvas::{all_of_the_screen, Canvas, Captured};
use brs_engine::offscreen::Offscreen;
use brs_engine::model::{ModelAsset, Skeleton};
use brs_engine::step::{Stepped, SIXTIETHS_PER_STEP};
use brs_engine::{map, AssetStores, EngineSystems};
use brs_formats::insm;

use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::items::{Inventory, Items};
use crate::pause::{self, Pause, PauseAssets, Paused};
use crate::player::{self, part, Holds, ShownParts};
use crate::progress::Tally;
use crate::sound::{Board, Keeper, Request, COMMON_BANK, FULL_VOLUME};
use crate::Mode;

pub mod actor;
pub mod aim;
pub mod auto;
pub mod boss;
pub mod bullet;
pub mod damage;
pub mod dimmer;
pub mod effects;
pub mod numbers;
pub mod scope;
pub mod shadows;
pub mod shake;
pub mod enemy;
pub mod flow;
pub mod hud;
pub mod item_menu;
pub mod encounter;
pub mod log;
pub mod projectile;
pub mod result;
pub mod skills;
pub mod stage;
pub mod status;
pub mod tables;
pub mod tunables;

use actor::{Clips, Context, Event, PcActor, Rules, Staged};
use aim::{Aim, LockPoint};
use auto::{Autos, Happens};
use bullet::BulletLook;
use damage::Random;
use crate::progress::{variable, Progress};
use crate::record_news::RecordNews;
use crate::record_window::RecordWindow;
use crate::records::{self, Records};
use enemy::actor::Spoils;
use enemy::Horde;
use flow::{Flow, Outcome};
use projectile::{Bomb, ProjectileLook, Ray, SkillRays, SkillShot};
pub use stage::Stage;
use stage::BattleMap;
use dimmer::Dimmer;
use effects::{Asked, BattleEffects, List};
use scope::Scope;
use skills::{Firing, Skills, Slots, Stagings, Strikes};
use status::Effects;
use tables::{Attacks, Levels, PlayerStats, Shots};
use tunables::{id, Tunables};

/// Where the player's battle animations are.
const PLAYER_CLIPS: &str = "SYSTEM.VOL/pack.vol";
/// Where her lines are, and the stream they are spoken on (0x74E30).
const VOICES: &str = "SYSTEM.VOL/pack.vol";
const VOICE_STREAM: usize = 0;
/// The cannon she carries, a model of its own: the first of its four levels.
pub(crate) const WEAPON: &str = "BTL/WEP/ITM_EQU_ASSUALT_LV1.ZZZ";

/// What her model shows in the basic battle states (the original's part modes 4 and
/// 5, at 0x9B608): an empty left hand, and a right hand shaped to hold the cannon,
/// which is the separate weapon model rather than the one built into her own.
const BATTLE_PARTS: [u16; 2] = [part::LEFT_HAND_EMPTY, part::RIGHT_HAND_CANNON];

/// The seed of the battle's random numbers. The original takes its from the clock.
const SEED: u64 = 0x5eed_b25;

/// Everything a battle needs, loaded before the app starts so that a missing or
/// unreadable file is reported plainly.
#[derive(Resource)]
pub struct BattleSource {
    map: BattleMap,
    player: (insm::Model, Vec<u8>),
    weapon: (insm::Model, Vec<u8>),
    clips: Clips,
    /// Plays all the time on top of the body's animation.
    hair: Arc<Clip>,
    /// The camera moves of her victory pose and of her defeat.
    victory_camera: Arc<CameraClip>,
    defeat_camera: Arc<CameraClip>,
    /// What the skills in her slots are played with.
    stagings: Stagings,
    tunables: Tunables,
    attacks: Attacks,
    /// The bullets the enemies fire.
    shots: Shots,
    /// The status effects there are.
    effects: Effects,
    /// The player's figures as she comes into the battle.
    stats: PlayerStats,
    terms: Terms,
    /// The enemies the battle is fought against.
    group: enemy::Group,
    /// The effect files the battle has, in the order they are looked in: those
    /// of the enemies' archives, of her skills', and the system's own.
    effect_files: Vec<(String, Vec<u8>)>,
    /// The stage the map it was walked into on is of, which some of the records
    /// go by; the first for a battle with no field about it.
    pub map_stage: i32,
}

/// What a battle takes from the mission it is fought in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Terms {
    /// The enemies are worth half their experience (0x973B4).
    pub half_experience: bool,
    /// Going up a level does not make her whole (0x37F08).
    pub no_level_heal: bool,
}

impl Terms {
    /// The missions in which experience counts for half, and the one in which a
    /// level gained does not heal.
    const HALF_EXPERIENCE: [i32; 6] = [5, 25, 45, 65, 85, 102];
    const NO_LEVEL_HEAL: i32 = 66;

    /// The terms of the mission with this number (variable 10004).
    pub fn of_scenario(scenario: i32) -> Terms {
        Terms { half_experience: Self::HALF_EXPERIENCE.contains(&scenario), no_level_heal: scenario == Self::NO_LEVEL_HEAL }
    }
}

impl BattleSource {
    /// Loads the battle map named e.g. `STG01_01` and enemy group `group`, for a
    /// player with the given figures and the skills she has in her slots.
    pub fn load(data: &GameData, stage: &str, group: u16, stats: PlayerStats, terms: Terms, skills: &Skills) -> Result<BattleSource> {
        // She is in the suit and has the weapon she has on (0x9BAA4, 0x9BAD0): the
        // suit is a character of the field, whose bundle has her model.
        let bundle = crate::field::tables::suit_bundle(data, stats.suit).context("finding the suit she has on")?;
        let player = data.model(player::model_of(&bundle)).with_context(|| format!("loading her model of {bundle}"))?;
        for id in [actor::MUZZLE_NODE].into_iter().chain(HANDS) {
            ensure!(player.0.nodes.iter().any(|node| node.id == id), "the player's model has no node {id}");
        }
        let clip = |name: &str| data.clip(format!("{PLAYER_CLIPS}/chr_p_brs02_{name}.anm"));
        let camera = |name: &str| data.camera(format!("{PLAYER_CLIPS}/chr_p_brs02_{name}_cam.cam"));

        // What is equipped changes two of the tunables as a battle starts: the
        // weapon's burst (0x8D47C) and the suit's dodge heat (0x7E7B0).
        let mut tunables = Tunables::load(data)?;
        tunables.set(id::BURST_BULLETS[0], stats.burst_bullets as f32);
        tunables.set(id::DODGE_HEAT, stats.dodge_heat as f32);

        let shots = Shots::load(data)?;
        let group = enemy::Group::load(data, group).with_context(|| format!("loading enemy group {group}"))?;
        let stagings = Stagings::load(data, skills, &shots, stats.skills).context("loading what her skills are played with")?;
        let mut effect_files = Vec::new();
        for folder in group.folders().into_iter().chain(stagings.folders()) {
            effect_files.append(&mut effects::files_of(data, &folder)?);
        }
        effect_files.append(&mut effects::common_files(data)?);
        Ok(BattleSource {
            map: BattleMap::load(data, stage)?,
            player,
            weapon: data.model(player::weapon_model(stats.weapon).unwrap_or_else(|| WEAPON.to_owned())).context("loading her weapon")?,
            clips: Clips {
                idle: clip("btl0000")?,
                dodge_out: clip("btl2020")?,
                dodge_back: clip("btl2021")?,
                shoot: clip("btl3000")?,
                hit: [clip("btl5000_all")?, clip("btl5001_all")?],
                guard: clip("btl6000_all")?,
                victory: clip("btl8010_all")?,
                defeat: clip("btl8020_all")?,
            },
            hair: clip("fld9700_hair")?,
            victory_camera: camera("btl8010")?,
            defeat_camera: camera("btl8020")?,
            stagings,
            tunables,
            attacks: Attacks::load(data)?,
            shots,
            effects: Effects::load(data)?,
            stats,
            terms,
            group,
            effect_files,
            map_stage: 0,
        })
    }

    /// The colour beyond everything, for the window's background.
    pub fn backdrop(&self) -> Color {
        self.map.fog_color
    }

    /// Whether the group fought is one she strikes her pose over when it is beaten
    /// (the group's flag 2): the field begins such a battle with another sound.
    pub fn victory_pose(&self) -> bool {
        self.group.victory_pose()
    }

    /// Whether the records are told nothing of this battle (0x9BDCC, 0x9BD30): the
    /// battle system's value 86, which also has what follows a battle not wait
    /// for the log.
    pub fn unrecorded(&self) -> bool {
        self.tunables.flag(tunables::id::LOG_NOT_WAITED_FOR)
    }

    /// The camera move that goes with one of her staged moments.
    fn camera(&self, staged: Staged) -> Option<&Arc<CameraClip>> {
        match staged {
            Staged::Victory => Some(&self.victory_camera),
            Staged::Defeat => Some(&self.defeat_camera),
            Staged::Skill(skill) => self.stagings.get(skill).map(|staging| &staging.camera),
            Staged::SkillSecond(skill) => self.stagings.get(skill).and_then(|staging| staging.second.as_ref()).map(|(_, camera)| camera),
        }
    }
}

/// Status effects to give her and every enemy as the fight begins (`--afflict`):
/// for a look at their marks and at what the log says of them, without waiting for
/// whoever gives them in the game.
#[derive(Resource, Default)]
pub struct Afflict(pub Vec<i32>);

/// Skills that act of themselves for her to have in a run, whatever the save says
/// of them (`--auto`), as `--skills` puts some in her slots.
#[derive(Resource, Default)]
pub struct Automatic(pub Vec<i32>);

/// What she can have in her hand in a battle: her model's second slot (0xA7444),
/// which holds the cannon she wears, or for a skill's scene the weapon of the
/// skill's archive, fixed to a node of hers that goes with the weapon: a gun to
/// the cannon's (0x7A740), a blade or a sword to another.
#[derive(Resource)]
struct Arsenal {
    /// The nodes of hers that something can hang on, by their identifiers.
    hands: HashMap<u8, Entity>,
    cannon: ModelAsset,
    of_skills: HashMap<i32, ModelAsset>,
    /// What she is to hold from this frame on, if that has changed.
    wanted: Option<Held>,
}

/// The nodes of her model that something can hang on.
const HANDS: [u8; 3] = [actor::WEAPON_NODE, skills::BLADE_NODE, skills::SWORD_NODE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    Nothing,
    Cannon,
    /// The weapon of a skill, at a node of hers.
    OfSkill(i32, u8),
}

/// On what she holds.
#[derive(Component)]
struct InHand;

/// Puts in her hand what she is to hold, in place of what she held.
fn wield(mut commands: Commands, mut arsenal: ResMut<Arsenal>, held: Query<Entity, With<InHand>>) {
    if arsenal.wanted.is_none() {
        return;
    }
    let wanted = arsenal.wanted.take();
    for entity in &held {
        commands.entity(entity).despawn();
    }
    let (asset, node) = match wanted {
        Some(Held::Cannon) => (Some(&arsenal.cannon), actor::WEAPON_NODE),
        Some(Held::OfSkill(skill, node)) => (arsenal.of_skills.get(&skill), node),
        Some(Held::Nothing) | None => (None, actor::WEAPON_NODE),
    };
    if let (Some(asset), Some(&hand)) = (asset, arsenal.hands.get(&node)) {
        let weapon = asset.spawn(&mut commands, Transform::IDENTITY);
        commands.entity(weapon).insert((InHand, ChildOf(hand)));
    }
}

/// A staged camera move in progress: the original's camera 3, which follows the
/// camera player of the actor that started it (0xA7678).
#[derive(Resource, Default)]
struct Staging {
    /// The move, and how many frames of it have played.
    playing: Option<(Arc<CameraClip>, f32)>,
}

/// On every entity that belongs to the battle scene.
#[derive(Component)]
pub struct Battle;

#[derive(Component)]
struct BattleCamera;

/// The sound of the cursor locking on to something (0xA1644), of the battle's bank.
const SOUND_LOCK: u32 = 0x6a;

/// How a battle that is over ended, once what follows its end has played: the
/// player's last state, and after a battle won its results (`result.rs`), which
/// stand until a button closes them.
#[derive(Resource, Default)]
pub struct Conclusion {
    /// Logic frames since the battle's end had finished playing.
    steps: u32,
    /// Her skills that act of themselves have been asked what they make of a
    /// battle won (0x9DA70), and if one of them acted, the sixtieths since: the
    /// battle's end waits some (0x9DB14).
    asked: bool,
    acted_for: Option<f32>,
    /// The display has been taken away (0x94C28): the battle's end does so once
    /// nothing is left for it to wait for.
    pub display_gone: bool,
    /// How the battle is giving way to what follows it, once it has begun to.
    way_back: Option<WayBack>,
    /// Its last picture was taken, for the field to come through.
    pub picture: bool,
    pub outcome: Option<Outcome>,
    /// What a battle won was worth, once she has been given it.
    rewards: Option<Rewards>,
    /// The records the battle has opened or cleared, once its results have come
    /// to showing them (0x37F8C), and whether there is still one to show.
    records: Option<(records::Shown, bool)>,
    /// The figures a battle won leaves for the field; a battle lost leaves none
    /// (0x78E34).
    pub tally: Tally,
}

impl Conclusion {
    /// Whether a record is being shown in its window: the results are not drawn
    /// meanwhile (0x37DDC).
    pub fn showing_a_record(&self) -> bool {
        self.records.as_ref().is_some_and(|(_, showing)| *showing)
    }
}

/// How a battle gives way to what follows it (0x9D758), won or lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WayBack {
    /// A picture of the screen is taken and put up to stay (0xE6B28): the field
    /// comes back under it, and its script has it go. For most groups.
    Picture,
    /// The screen goes dark, over thirty frames of the interface (0x2983C with
    /// 2): for the groups she strikes her pose over. So many frames of the game
    /// have gone by.
    Dark(u32),
}

/// The frames of the game the screen takes to go dark as such a battle ends.
const DARK_FRAMES: u32 = 15;

/// What she came out of a battle won with.
#[derive(Clone, Debug, PartialEq)]
struct Rewards {
    experience: u32,
    /// Her level before and after.
    levels: (i32, i32),
    /// Her experience after, and what the next level takes.
    total: u32,
    next: Option<u32>,
    /// The items left behind, by name, with how many of each.
    items: Vec<(String, i32)>,
}

impl Rewards {
    /// Gives her what the battle was worth, as the result screen does: the
    /// experience, with the levels it comes to (0x37E24), and the items (0x37F44).
    fn give(spoils: &Spoils, terms: Terms, levels: &Levels, items: &Items, stats: &mut PlayerStats, inventory: &mut Inventory) -> Rewards {
        let experience = spoils.experience.max(0) as u32;
        let before = stats.level;
        if let Err(error) = stats.gain(levels, experience, !terms.no_level_heal) {
            error!("the experience could not be given: {error:#}");
        }
        let kept = spoils.items().map(|(item, count)| {
            inventory.add(items, item, count);
            (items.name(item).to_owned(), count)
        });
        Rewards {
            experience,
            levels: (before, stats.level),
            total: stats.experience,
            next: levels.next(stats.level),
            items: kept.collect(),
        }
    }

}

/// Logic frames a battle lost takes to give way once its display has gone. The
/// original has no screen of its own for it: the battle's end (0x9D988) makes the
/// results as after a battle won, which draw nothing of themselves when she has
/// no HP (0x36714) and have done as soon as their panels would have come
/// (0x38388: eight frames of their layout); no button is waited for (0x9DCAC),
/// the battle gives way as after one won (0x9D758), and `BtGameOver` (0x94A40)
/// has the tune fade and her HP kept as none. The frames between those steps are
/// counted as one each. What the screen says of a mission failed is the field's
/// to say (`field/failed.rs`).
const DEFEAT_STEPS: u32 = 4 + 7;
/// The blocks the tune takes to fade as a battle lost ends (0x94B10).
const DEFEAT_TUNE_FADE: i32 = 60;

// The tune of a battle won (0x38288), which the results of a battle against a group
// she strikes her pose over begin with: the first jingle of the pack of them, on
// the first stream at 96 of 127, with the music turned down to 1 under it.
const VICTORY_TUNE: &str = "SYSTEM.VOL/jin_pack.bin/000.riff";
const VICTORY_TUNE_STREAM: usize = 0;
const VICTORY_TUNE_VOLUME: i32 = 0x60;
const MUSIC_UNDER_THE_TUNE: i32 = 1;

pub fn plugin(app: &mut App) {
    app.init_resource::<Stage>()
        .init_resource::<Flow>()
        .init_resource::<encounter::Encounter>()
        .init_resource::<Staging>()
        .init_resource::<Dimmer>()
        .init_resource::<Strikes>()
        .init_resource::<SkillRays>()
        .init_resource::<Scope>()
        .init_resource::<Conclusion>()
        .init_resource::<Afflict>()
        .init_resource::<Automatic>()
        .init_resource::<Paused>()
        .insert_resource(Random::new(SEED))
        .add_systems(OnEnter(Mode::Battle), (setup, boss::begin))
        .add_systems(OnExit(Mode::Battle), clear)
        .add_systems(FixedUpdate, pause.before(EngineSystems::Animate).run_if(in_state(Mode::Battle)))
        // The enemies think twice: one called into the battle during the first round
        // takes its first turn in the second, in the same frame.
        .add_systems(
            FixedUpdate,
            (
                shake::step,
                afflict,
                step,
                wield,
                dimmer::shade,
                enemy::begin,
                enemy::think,
                enemy::think,
                enemy::settle,
                boss::direct,
                shake::apply,
                dimmer::follow,
                effects::systems(),
                encounter::systems(),
                numbers::gather,
                tell_records,
                conclude,
            )
                .chain()
                .after(EngineSystems::Animate)
                .run_if(in_state(Mode::Battle).and_then(not_paused)),
        )
        .add_systems(Update, (draw_fade, boss::draw, (shadows::make, shadows::follow).chain()).run_if(in_state(Mode::Battle)));
    hud::plugin(app);
    item_menu::plugin(app);
    effects::plugin(app);
}

/// Takes the battle's scene away, and puts its progress back to the beginning for
/// the next one. The results of a battle won go with it (0x37D30): what the first
/// stream is playing stops there, and the music is as loud as it was when they
/// were made.
fn clear(
    mut commands: Commands,
    scene: Query<Entity, With<Battle>>,
    results: Option<Res<result::Results>>,
    records: Res<Records>,
    progress: Option<ResMut<Progress>>,
    mut window: ResMut<RecordWindow>,
    mut board: ResMut<Board>,
) {
    for entity in &scene {
        commands.entity(entity).try_despawn();
    }
    if results.is_some() {
        board.ask_all([Request::Hush(VICTORY_TUNE_STREAM, 0), Request::MusicVolumeBack(Keeper::Results)]);
        // What the battles' records had to show has been shown (0x37DAC), and
        // their window goes with what worked it (0x130B20).
        if let Some(mut progress) = progress {
            records.shown(records::OF_BATTLES, &mut progress);
        }
        window.shut();
    }
    commands.remove_resource::<result::Results>();
    commands.insert_resource(Flow::default());
    commands.insert_resource(Staging::default());
    commands.insert_resource(Conclusion::default());
}

/// Tells the records of the enemies her hits have ended (0x9BDAC, at the end of
/// 0x92510). They are told of none in the first mission (0x97380), nor in a battle
/// that is not for them.
fn tell_records(source: Res<BattleSource>, records: Res<Records>, mut horde: ResMut<Horde>, progress: Option<ResMut<Progress>>, mut stats: ResMut<PlayerStats>) {
    if horde.beaten.is_empty() {
        return;
    }
    let beaten = std::mem::take(&mut horde.beaten);
    let Some(mut progress) = progress else { return };
    if source.unrecorded() || progress.variable(variable::SCENARIO) == 0 {
        return;
    }
    let mut world = records::World { progress: &mut progress, stats: &mut stats, stage: source.map_stage };
    for event in beaten {
        if let Err(error) = records.enemy_beaten(event, &mut world) {
            error!("the records' script failed: {error:#}");
        }
    }
}

/// Notes when a decided battle has played out, and has her take away from it what
/// she does: the HP she has left (0x9DC10; none from a battle lost, 0x94B34) and,
/// from one won, what it was worth, which its results then tell (0x9DC54) until
/// cross or circle closes them (0x9D6A8).
#[allow(clippy::too_many_arguments)]
pub(crate) fn conclude(
    mut commands: Commands,
    pad: Res<Pad>,
    flow: Res<Flow>,
    source: Res<BattleSource>,
    horde: Res<Horde>,
    (levels, items): (Res<Levels>, Res<Items>),
    (display, fonts, colours): (Res<hud::Hud>, Res<crate::writing::Fonts>, Res<crate::text::TextColours>),
    mut player: Single<&mut PcActor>,
    log: Option<Res<log::Log>>,
    results: Option<ResMut<result::Results>>,
    (mut captured, offscreen): (ResMut<Captured>, Option<Res<Offscreen>>),
    (mut board, mut random, mut effects): (ResMut<Board>, ResMut<Random>, ResMut<BattleEffects>),
    mut stats: ResMut<PlayerStats>,
    mut inventory: ResMut<Inventory>,
    mut conclusion: ResMut<Conclusion>,
    (records, mut progress, mut window, news): (Res<Records>, Option<ResMut<Progress>>, ResMut<RecordWindow>, Res<RecordNews>),
) {
    let Some(outcome) = flow.outcome() else { return };
    if conclusion.outcome.is_some() {
        return;
    }
    let fallen = outcome == Outcome::Lost || player.hp <= 0.0;
    // A battle she has won against any but a few groups has her last state, her
    // skills that act of themselves, and results (0x9D5FC).
    let results_follow = !fallen && !source.group.no_results();
    if results_follow {
        // Once her last state has begun those skills are asked what they make of
        // the battle (0x9DA70): one that gives her HP back does so at once
        // (0x9F9E0), and if any acted the end waits from here (0x9DB14).
        if !conclusion.asked {
            if !flow.moved_on() {
                return;
            }
            conclusion.asked = true;
            let answer = player.ask(Happens::BattleWon, &mut random);
            if answer.heals > 0 {
                player.heal(answer.heals);
                board.play(COMMON_BANK, auto::SOUND, FULL_VOLUME);
                // What is seen of it stays where she stands (0x9FA40).
                effects.ask(Asked::at(auto::EFFECT_HEALS, player.position, false));
                info!("a skill of hers gives her {} HP back as the battle is won: she has {}", answer.heals, player.hp);
            }
            conclusion.acted_for = (answer.acted > 0).then_some(0.0);
        } else if let Some(time) = conclusion.acted_for.as_mut() {
            *time += SIXTIETHS_PER_STEP;
        }
        // The battle's end waits for that, and for her last state to have played;
        // either end for the log to have said all it has (0x9DB3C to 0x9DBAC) and
        // for a panel that tells of a record to be gone (0x9DBB4, 0x94B00); then
        // the display goes.
        if conclusion.acted_for.is_some_and(|time| time < auto::END_WAIT) || !player.finished {
            return;
        }
    }
    let waited_for = !source.tunables.flag(tunables::id::LOG_NOT_WAITED_FOR);
    if conclusion.steps == 0 && (waited_for && log.is_some_and(|log| log.busy()) || news.busy()) {
        return;
    }
    conclusion.display_gone = true;
    if conclusion.steps == 0 {
        if fallen {
            stats.hp = 0.0;
        } else {
            stats.hp = player.hp.trunc().min(stats.max_hp);
            let spoils = horde.spoils();
            conclusion.tally = Tally {
                seconds: flow.seconds(),
                damage_taken: player.damage_taken,
                damage_dealt: spoils.damage_dealt,
                enemies_beaten: spoils.beaten,
            };
            if !results_follow {
                info!("the battle is won, and nothing follows it: {:?}", conclusion.tally);
            }
        }
        if results_follow {
            let spoils = horde.spoils();
            // The results count what was earned onto what she had; she is given it
            // here, which the original does only once they have (0x38720).
            let (level, had) = (stats.level, stats.experience as i32);
            let rewards = Rewards::give(spoils, source.terms, &levels, &items, &mut stats, &mut inventory);
            info!("the battle is won: {:?}, {rewards:?}; she is now {:?}", conclusion.tally, *stats);
            let told = result::Results::new(display.results(), &levels, &items, &fonts, &colours, level, had, rewards.experience as i32, &spoils.items);
            commands.insert_resource(told);
            // The records are told of a battle she has come out of (0x9BD28, as
            // the results are made), and of a level gained (0x37F24, which the
            // original does as the results give her the experience).
            if let Some(progress) = progress.as_deref_mut() {
                let mut world = records::World { progress: &mut *progress, stats: &mut stats, stage: source.map_stage };
                let fought = records::Event::battle(i32::from(source.group.id), source.victory_pose(), !player.harmed);
                let told = if source.unrecorded() { Ok(()) } else { records.battle_finished(fought, &mut world) };
                let levelled = if rewards.levels.1 > rewards.levels.0 { records.level_up(&mut world) } else { Ok(()) };
                if let Err(error) = told.and(levelled) {
                    error!("the records' script failed: {error:#}");
                }
                info!("the records that are open: {}", records.summary(progress));
            }
            conclusion.rewards = Some(rewards);
            // The results note how loud the music is as they are made (0x37B94),
            // and those of a battle against a group of note begin with the tune of
            // a battle won (0x38288); the music is as loud again when that is over
            // (0x38810), or when the results are done with (0x37D30).
            board.ask(Request::KeepMusicVolume(Keeper::Results));
            if source.victory_pose() {
                board.ask_all([
                    Request::MusicVolume(MUSIC_UNDER_THE_TUNE, 0.0),
                    Request::SayAt(VICTORY_TUNE_STREAM, VICTORY_TUNE.into(), VICTORY_TUNE_VOLUME),
                    Request::MusicVolumeBackAfter(VICTORY_TUNE_STREAM, Keeper::Results),
                ]);
            }
        }
    }
    conclusion.steps += 1;
    // The way back, once it has begun (0x9DCE4 to 0x9DD28): the picture is waited
    // for, or the dark.
    if let Some(way_back) = &mut conclusion.way_back {
        let (there, picture) = match way_back {
            WayBack::Picture => (!captured.pending(), true),
            WayBack::Dark(frames) => {
                *frames += 1;
                (*frames >= DARK_FRAMES, false)
            }
        };
        if there {
            if fallen {
                board.ask(Request::StopMusic(DEFEAT_TUNE_FADE));
            }
            conclusion.picture = picture;
            conclusion.outcome = Some(if fallen { Outcome::Lost } else { Outcome::Won });
        }
        return;
    }
    let over = match (fallen, results) {
        (true, _) => conclusion.steps >= DEFEAT_STEPS,
        // With no results there is nothing to wait for (0x9DC78, 0x9DCA4).
        (false, _) if !results_follow => true,
        (false, Some(mut results)) => {
            // A press that has the count done at once does not close them too.
            let told = results.told();
            let heard = results.step(display.results(), &levels, SIXTIETHS_PER_STEP, pad.pressed & button::CROSS != 0);
            if heard.level {
                board.play(COMMON_BANK, result::SOUND_LEVEL, FULL_VOLUME);
            }
            if heard.count {
                board.play(COMMON_BANK, result::SOUND_COUNT, FULL_VOLUME);
            }
            // With all of it counted and given, the records the battle has opened
            // or cleared are shown, each in its window and each held back until
            // its turn (0x38758, 0x380E0); only then is all told (0x3807C). A
            // panel that tells of a record is waited for first (0x37FD8), and
            // again before all is told.
            let mut shown = !news.busy();
            if let (true, Some(progress)) = (told && (conclusion.records.is_some() || !news.busy()), progress.as_deref_mut()) {
                let (state, showing) = conclusion.records.get_or_insert_with(|| {
                    let list = records.list(records::OF_BATTLES, progress);
                    window.hold(&list);
                    (records::Shown::begin(records::OF_BATTLES, Some(list.len())), true)
                });
                let mut world = records::World { progress: &mut *progress, stats: &mut stats, stage: source.map_stage };
                let mut screen =
                    records::Screen { window: &mut window, inventory: &mut inventory, items: &items, fonts: &fonts, colours: &colours, board: &mut board };
                *showing = state.step(&records, &mut world, &mut screen);
                shown &= !*showing;
            }
            window.step(&pad, &mut board);
            told && shown && pad.pressed & (button::CROSS | button::CIRCLE) != 0
        }
        // They are there from the next frame on.
        (false, None) => false,
    };
    if over {
        conclusion.way_back = Some(if source.victory_pose() {
            WayBack::Dark(0)
        } else {
            captured.ask(&mut commands, all_of_the_screen(offscreen.as_deref()));
            WayBack::Picture
        });
    }
}

/// Draws what the screen's fade has over a battle: as it begins the field's
/// picture, which goes, or the white it comes out of (0x11B588, on the fade's own
/// layer, over the battle's display); and the dark a battle ends by going into,
/// where it does (the fade with kind 2).
fn draw_fade(
    conclusion: Res<Conclusion>,
    encounter: Res<encounter::Encounter>,
    captured: Res<Captured>,
    seen: Res<brs_engine::canvas::Seen>,
    mut canvas: ResMut<Canvas>,
) {
    let layer = crate::interface::layer::FADE;
    let sheet = |canvas: &mut Canvas, shade: u8, level: f32| {
        let alpha = (level.clamp(0.0, 1.0) * 255.0) as u8;
        if alpha != 0 {
            canvas.fill(layer, seen.0, [shade, shade, shade, alpha]);
        }
    };
    let (picture, white) = encounter.shown();
    if let Some(colour) = picture {
        captured.draw(&mut canvas, &seen, layer, colour);
    }
    // The effect of a battle of note is over the picture: while it plays the
    // scenes' cameras draw nothing else, and what they drew is laid over it.
    if encounter.effect_plays() {
        canvas.scene_here(layer);
    }
    sheet(&mut canvas, 255, white);
    if let Some(WayBack::Dark(frames)) = conclusion.way_back {
        sheet(&mut canvas, 0, frames as f32 / DARK_FRAMES as f32);
    }
}

fn setup(
    mut commands: Commands,
    source: Res<BattleSource>,
    stage: Res<Stage>,
    (skills, progress, automatic): (Res<Skills>, Option<Res<Progress>>, Res<Automatic>),
    mut random: ResMut<Random>,
    mut stores: AssetStores,
    mut sheets: ResMut<Assets<ColorMaterial>>,
    (mut board, mut encounter, mut flow): (ResMut<Board>, ResMut<encounter::Encounter>, ResMut<Flow>),
) {
    encounter.alone_against(source.victory_pose());
    // A boss's battle is begun by its script (0x93DE8): the enemies think from the
    // first frame, and the screen comes when the script has its scene set.
    if source.group.scripted() {
        encounter.hold();
        flow.leave_to_script();
    }
    commands.insert_resource(ClearColor(source.backdrop()));
    commands.insert_resource(Dimmer::default());
    commands.insert_resource(Scope::default());
    commands.insert_resource(BattleEffects::load(&source.effect_files, &mut stores));
    commands.insert_resource(numbers::Numbers::default());
    commands.insert_resource(shake::Shake::default());
    // The map's models, in the order of the layers that draw them: sky first. They
    // are drawn by a camera of their own, before everything else (see `dimmer`).
    let mut models: Vec<_> = source.map.models.iter().collect();
    models.sort_by_key(|model| model.layer);
    let mut drawn = 0;
    for model in models {
        let turned = Transform::from_rotation(Quat::from_rotation_y(model.turn.to_radians()));
        for entity in map::spawn(&mut commands, &model.map, &model.data, turned, map::Piece::Scene, &mut drawn, &mut stores).entities {
            commands.entity(entity).insert((Battle, RenderLayers::layer(dimmer::STAGE_LAYER)));
        }
    }

    // The player, with her hair's animation layered over whatever her body does.
    info!("the player's figures: {:?}", source.stats);
    let mut actor = PcActor::new(source.stats.max_hp, source.stats.hp, &stage, &source.tunables);
    // Her skills that act of themselves, for this battle (0x9F79C).
    let of_themselves = skills.of_themselves(progress.as_deref(), &automatic.0);
    if !of_themselves.is_empty() {
        let names: Vec<String> = of_themselves.iter().map(|skill| format!("{} {}", skill.id, skill.name)).collect();
        info!("her skills that act of themselves: {}", names.join(", "));
    }
    actor.autos = Autos::new(of_themselves);
    let at = actor.transform();
    let aim = Aim::new(&source.tunables, actor.position, actor.direction);
    let asset = ModelAsset::build(&source.player.0, &source.player.1, &mut stores);
    let (player, hands) = asset.spawn_with_nodes(&mut commands, at, HANDS);
    let hands: HashMap<u8, Entity> = HANDS.into_iter().zip(hands).filter_map(|(id, node)| Some((id, node?))).collect();
    let mut animator = Animator::with_slots(2);
    animator.set(&source.clips.idle);
    animator.body_mut().loop_clip();
    animator.set_slot(1, &source.hair);
    animator.slot_mut(1).loop_clip();

    // The enemies: each one's script sets it up, which is what loads its model.
    let look = ProjectileLook::new(&mut stores.meshes, &mut stores.materials);
    let half = source.terms.half_experience;
    let horde =
        enemy::spawn_group(&mut commands, &source.group, &source.shots, &source.effects, &stage, &actor, half, &mut random, &mut stores, &look, &mut board);
    let horde = horde.unwrap_or_else(|error| {
        error!("the enemies could not be set up: {error:#}");
        Horde::default()
    });
    commands.insert_resource(horde);
    commands.insert_resource(look);
    commands.entity(player).insert((Battle, actor, animator, Stepped::new(at), ShownParts(BATTLE_PARTS.to_vec())));
    // Her shadows on the ground (0x999E8).
    shadows::give_hers(&mut commands, player);

    // The cannon is a model of its own, held at a node of hers.
    let cannon = ModelAsset::build(&source.weapon.0, &source.weapon.1, &mut stores);
    if let Some(&hand) = hands.get(&actor::WEAPON_NODE) {
        let weapon = cannon.spawn(&mut commands, Transform::IDENTITY);
        commands.entity(weapon).insert((InHand, ChildOf(hand)));
    }
    // And the weapons she takes for the skills in her slots, in place of it.
    let of_skills = source.stagings.weapons().map(|(skill, (model, data))| (skill, ModelAsset::build(model, data, &mut stores))).collect();
    commands.insert_resource(Arsenal { hands, cannon, of_skills, wanted: None });

    let view = aim.view.transform();
    let lens = Projection::Perspective(PerspectiveProjection {
        fov: aim::vertical_fov(),
        near: source.map.near,
        far: source.map.far,
        ..default()
    });
    let fog = DistanceFog {
        color: brs_engine::texture::values(source.map.fog_color),
        falloff: FogFalloff::Linear { start: source.map.fog_start, end: source.map.fog_end },
        ..default()
    };
    let camera = commands.spawn((
        Battle,
        BattleCamera,
        // The battle's display is drawn over this camera's picture, whatever other
        // camera there is.
        IsDefaultUiCamera,
        // The map has been drawn when this camera's turn comes: it draws over
        // that, and behind what of the map is nearer.
        Camera3d { depth_load_op: Camera3dDepthLoadOp::Load, ..default() },
        Camera { clear_color: ClearColorConfig::None, ..default() },
        brs_engine::texture::camera_of_values(),
        lens.clone(),
        fog.clone(),
        view,
        Stepped::new(view),
    ));
    let camera = camera.id();
    dimmer::spawn(&mut commands, camera, lens, fog, &mut stores.meshes, &mut sheets);
    encounter::spawn(&mut commands);
    commands.insert_resource(aim);
    commands.insert_resource(BulletLook::new(&mut stores.meshes, &mut stores.materials));
}

/// Whether the battle has its frames: not while the pause is up (0x97BB8, which
/// all that moves in a battle asks before it does), nor while she has her items
/// open (0x99620, which they ask too).
pub(crate) fn not_paused(paused: Res<Paused>, items: Res<item_menu::ItemMenu>) -> bool {
    !paused.up() && !items.holding
}

/// The battle's pause ("BtPause", 0x97A30). While the fight is on (its flag 4,
/// set as the fight begins, 0x93FE4, and cleared as the battle is decided,
/// 0x8D79C) start brings the pause up, with nothing to do from it but go on;
/// until it has gone nothing of the battle has a frame, and what it animates
/// stands still.
#[allow(clippy::too_many_arguments)]
fn pause(
    pad: Res<Pad>,
    assets: Res<PauseAssets>,
    flow: Res<Flow>,
    records: Res<Records>,
    progress: Option<Res<Progress>>,
    items: Res<item_menu::ItemMenu>,
    mut paused: ResMut<Paused>,
    mut board: ResMut<Board>,
    mut animators: Query<&mut Animator>,
) {
    let mut hold = |held: bool| {
        for mut animator in &mut animators {
            animator.paused = held;
        }
    };
    if let Some(shown) = &mut paused.0 {
        let over = shown.step(&assets, &pad).is_some();
        board.ask_all(shown.sounds.drain(..));
        if over {
            paused.0 = None;
            hold(false);
            board.ask(Request::LetGo);
        }
    } else if pad.pressed & button::START != 0 && flow.running() && !items.holding {
        let listed = progress.map(|progress| pause::entries(&records, &progress)).unwrap_or_default();
        paused.0 = Some(Pause::new(&assets, pause::Kind::Battle, listed, false));
        hold(true);
    }
}

/// Gives her and every enemy the status effects a run was asked to begin with, once
/// the fight is on. They are given as by somebody of a level no chance goes against.
fn afflict(
    mut wanted: ResMut<Afflict>,
    source: Res<BattleSource>,
    flow: Res<Flow>,
    mut random: ResMut<Random>,
    mut player: Single<&mut PcActor>,
    mut enemies: Query<&mut enemy::Enemy>,
) {
    if wanted.0.is_empty() || !flow.running() {
        return;
    }
    let giver = status::Giver { level: 99, special: 100, boost: [0; 3], name: "Somebody".into() };
    for id in std::mem::take(&mut wanted.0) {
        let Some(effect) = source.effects.get(id) else {
            warn!("there is no status effect {id} to give");
            continue;
        };
        let hers = player.afflict(effect, &giver, &source.stats, &mut random);
        let theirs: Vec<status::Outcome> = enemies.iter_mut().map(|mut enemy| enemy.actor.body.afflict(effect, &giver, &mut random)).collect();
        info!("status effect {id} is given to her ({hers:?}) and to the enemies ({theirs:?})");
    }
}

/// A skill gives her a status effect as she would give one to another: by her own
/// level and special attack, and by a boost if she has one (0x80E14, 0xA00F8).
fn give_her(actor: &mut PcActor, id: i32, source: &BattleSource, random: &mut Random, seconds: f32) {
    match source.effects.get(id) {
        Some(effect) => {
            let giver = status::Giver { level: source.stats.level, special: source.stats.special, boost: actor.status.boost(), name: actor::NAME.to_owned() };
            let outcome = actor.afflict(effect, &giver, &source.stats, random);
            info!("her skill gives her status effect {id}, {seconds:.3} s in: {outcome:?}");
        }
        None => warn!("there is no status effect {id} for her skill to give"),
    }
}

/// One logic frame of the player, in the original's order (0x99E50); the enemies and
/// her shots follow in `enemy::step`.
#[allow(clippy::too_many_arguments)]
fn step(
    mut commands: Commands,
    (pad, time): (Res<Pad>, Res<Time>),
    source: Res<BattleSource>,
    stage: Res<Stage>,
    (look, shot_look, mut effects, show): (Res<BulletLook>, Res<ProjectileLook>, ResMut<BattleEffects>, Option<Res<boss::Show>>),
    horde: Res<Horde>,
    (mut flow, mut encounter): (ResMut<Flow>, ResMut<encounter::Encounter>),
    (mut staging, mut dimmer, mut arsenal, mut strikes, mut rays): (ResMut<Staging>, ResMut<Dimmer>, ResMut<Arsenal>, ResMut<Strikes>, ResMut<SkillRays>),
    (mut aim, mut scope, mut shake): (ResMut<Aim>, ResMut<Scope>, ResMut<shake::Shake>),
    mut board: ResMut<Board>,
    (progress, mut slots): (Option<Res<Progress>>, Option<ResMut<Slots>>),
    mut random: ResMut<Random>,
    player: Single<(Entity, &mut PcActor, &mut Animator, &Skeleton, &mut Stepped, &mut ShownParts, &mut Visibility), Without<BattleCamera>>,
    camera: Single<(&mut Projection, &mut Stepped), (With<BattleCamera>, Without<PcActor>)>,
) {
    let dt = SIXTIETHS_PER_STEP;
    let (her, mut actor, mut animator, skeleton, mut stepped, mut shown, mut seen) = player.into_inner();
    let (mut projection, mut camera) = camera.into_inner();
    let aspect = match &*projection {
        Projection::Perspective(perspective) => perspective.aspect_ratio,
        _ => aim::SCREEN.x / aim::SCREEN.y,
    };

    // The battle's own progress. Once it is decided, a player who has won and is
    // still on her feet goes into her last state (0x9DA28).
    flow.clock(dt);
    // The screen comes to the battle from the field, and the fight begins the
    // frame after it has (0x93E20).
    let come = encounter.over();
    encounter.step();
    let waiting = !flow.under_way();
    // Against a few groups a battle won has nothing follow it: she has no last
    // state then (0x9D9F0).
    if flow.advance(come) == Some(Outcome::Won) && actor.hp > 0.0 && !source.group.no_results() {
        actor.finish_battle(source.group.victory_pose());
    }
    // As the fight begins her skills that act of themselves are asked (0x93FB4),
    // and one that acts on that gives her its effect there and then.
    if waiting && flow.under_way() {
        let answer = actor.ask(Happens::BattleBegins, &mut random);
        for &id in &answer.gives {
            give_her(&mut actor, id, &source, &mut random, time.elapsed_secs());
        }
        if answer.acted > 0 {
            board.play(COMMON_BANK, auto::SOUND, FULL_VOLUME);
            effects.ask(Asked::on(auto::EFFECT, her, Mat4::IDENTITY, true));
        }
    }
    // She can act once the battle has come into view and until it is decided.
    // She can dodge once the first of the records is cleared, which sets the flag
    // that lets her (0x97490).
    // While an enemy has the stage (the root's flag 10) she can do nothing
    // (0x98F28), and nothing of hers has a frame but her animation (0x99EC4).
    let held = horde.has_the_stage();
    let rules = Rules { running: flow.running() && !held, dodge_unlocked: progress.is_none_or(|progress| progress.flag(actor::DODGE_FLAG)) };
    // The sheet over what is behind her goes on its way (0x99FF8).
    if !held {
        dimmer.step(dt);
    }
    // A camera move that is playing goes on by the step (0xA7598), before her state
    // runs and may start another. Where it cuts to another shot the camera is not
    // shown on its way there.
    let mut cut = false;
    if let Some((clip, frame)) = &mut staging.playing {
        cut = clip.cuts(*frame, *frame + dt);
        *frame += dt;
    }

    // While the scope of a snipe is up the cursor is held where it is (0xA1A5C in
    // place of 0xA1CC8), and the scope's own clock runs (0x8C828).
    scope.step(dt);
    if actor.can_act(rules) && !actor.scoped {
        // The lock points the cursor can settle on: those in front of the camera.
        let lock_points: Vec<LockPoint> = horde
            .lock_points
            .iter()
            .filter_map(|lock| {
                let screen = aim.view.project(lock.at, aspect)?;
                Some(LockPoint { entity: lock.enemy, number: lock.number, gauge: lock.gauge, screen })
            })
            .collect();
        let (before, was_locked) = (aim.target, aim.locked);
        // A scripted run can have the stick worked for it, to check what follows a
        // hit without the way to each enemy being written out.
        match pad.seek {
            Some(wanted) => {
                let steered = Pad {
                    stick: aim.seek(&lock_points, wanted, dt),
                    dpad: Vec2::ZERO,
                    held: pad.held,
                    pressed: pad.pressed,
                    released: pad.released,
                    repeat: pad.repeat,
                    seek: pad.seek,
                };
                aim.step(&steered, dt, &source.tunables, &lock_points);
            }
            None => aim.step(&pad, dt, &source.tunables, &lock_points),
        }
        if aim.target != before {
            let points: Vec<String> = lock_points.iter().map(|point| format!("{:.0}", point.screen)).collect();
            debug!("the cursor at {:.0} settles on {:?} of the lock points at {points:?}", aim.cursor, aim.target);
        }
        // Locking on is heard (0xA1608); going from one point to another is not.
        if aim.locked && !was_locked {
            board.play(COMMON_BANK, SOUND_LOCK, FULL_VOLUME);
        }
    }
    let aim_point = aim.point(aspect, &horde.targets());
    // The enemy the cursor has settled on (the root's 0x171314).
    let lock = aim.target.filter(|_| aim.locked).map(|(enemy, gauge)| horde.lock_on(enemy, gauge));

    let mut events = Vec::new();
    let mut no_slots = Slots::default();
    if !held {
        actor.step(&mut Context {
            dt,
            pad: &pad,
            tunables: &source.tunables,
            stage: &stage,
            rules,
            clips: &source.clips,
            animator: &mut animator,
            skeleton,
            aim_point,
            lock,
            places: &horde.places,
            slots: slots.as_deref_mut().unwrap_or(&mut no_slots),
            stagings: &source.stagings,
            random: &mut random,
            events: &mut events,
        });
    }
    stepped.current = actor.transform();
    aim.follow(&source.tunables, actor.position, actor.direction);

    for event in events {
        match event {
            Event::Shot { from, velocity } => bullet::spawn(&mut commands, &look, from, velocity, horde.next_frame()),
            Event::Camera(staged) => {
                match staged {
                    Staged::Skill(skill) => info!("her skill {skill} takes the stage at {:.0}, {:.3} s in", actor.transform().translation, time.elapsed_secs()),
                    Staged::SkillSecond(skill) => info!("her skill {skill} goes on at {:.0}, {:.3} s in", actor.transform().translation, time.elapsed_secs()),
                    Staged::Victory | Staged::Defeat => {}
                }
                if let Some(clip) = source.camera(staged) {
                    staging.playing = Some((clip.clone(), 0.0));
                    cut = true;
                }
            }
            Event::CameraBack => {
                staging.playing = None;
                cut = true;
            }
            Event::Dim { to, time } => dimmer.go(to, time),
            // How near and how far the cameras of the ground and of all that
            // fights see (0xA7678). Nothing puts it back once a skill's scene is
            // over (0xA7CE0 only has it no longer set each frame), so it stays
            // until the next battle's map sets its own.
            Event::Lens { near, far } => {
                if let Projection::Perspective(perspective) = &mut *projection {
                    (perspective.near, perspective.far) = (near, far);
                }
            }
            // A skill's line of hers is spoken as her others are, and its own
            // sounds are of its archive's bank (0x80D8C).
            Event::SkillVoice { skill, name } => {
                if let Some(staging) = source.stagings.get(skill) {
                    board.ask_all([Request::Hush(VOICE_STREAM, 0), Request::Say(VOICE_STREAM, staging.voice(name))]);
                }
            }
            Event::SkillSound { skill, sound } => {
                if let Some(bank) = source.stagings.get(skill).and_then(skills::Staging::bank) {
                    board.play(bank, sound, FULL_VOLUME);
                }
            }
            // Her hands, and what a change of them does to what she holds.
            Event::Parts(mode) => match shown.set_mode(mode) {
                Holds::AsBefore => {}
                Holds::Nothing => arsenal.wanted = Some(Held::Nothing),
                Holds::Cannon => arsenal.wanted = Some(Held::Cannon),
            },
            Event::Weapon { skill, node } => arsenal.wanted = Some(Held::OfSkill(skill, node)),
            // The enemies' part of the frame has it land (`enemy::settle`).
            Event::Strike { skill, blow } => strikes.0.push((skill, blow)),
            // The scope of a snipe (0x7F86C): its camera is told where the camera
            // is now and, if the skill is for one, where that lock point is.
            Event::Scope(on) => {
                let place = on.and_then(|(enemy, gauge)| horde.lock_points.iter().find(|lock| lock.enemy == enemy && lock.gauge == gauge));
                scope.come(aim.view.eye, place.map(|lock| lock.at));
                board.play(COMMON_BANK, scope::SOUND_COMES, FULL_VOLUME);
                info!("the scope of her snipe comes over {:?}, {:.3} s in", place.map(|lock| lock.at.round()), time.elapsed_secs());
            }
            Event::ScopeOff => scope.go(),
            Event::SightLock => {
                if scope.lock() {
                    board.play(COMMON_BANK, scope::SOUND_LOCKS_ON, FULL_VOLUME);
                }
            }
            Event::Hide(hidden) => *seen = if hidden { Visibility::Hidden } else { Visibility::Inherited },
            // A ray of a skill's volley (0x7C158): made only if the skill's bullet
            // is a ray, from the node of hers its record names towards what the
            // cursor points at now, with the bullet's sound (0x86728). It strikes
            // in the enemies' part of this same frame.
            Event::SkillRay { skill } => {
                let shot = source.stagings.get(skill).and_then(|staging| staging.shot).filter(|shot| shot.kind == projectile::RAY);
                match (shot, aim_point) {
                    (Some(shot), Some(aim)) => {
                        let node = u8::try_from(shot.node).ok().and_then(|id| skeleton.index_of(id));
                        let from = actor.transform() * node.map_or(Transform::IDENTITY, |index| animator.model_space(skeleton, index));
                        if let (Ok(sound), 0) = (u32::try_from(shot.sound), shot.sound_bank) {
                            board.play(COMMON_BANK, sound, FULL_VOLUME);
                        }
                        effects.ask_at_muzzle(&shot, from);
                        rays.0.push((skill, Ray::new(shot, Entity::PLACEHOLDER, Some(from), aim)));
                    }
                    _ => warn!("her skill {skill} fires no ray: she aims at {aim_point:?}"),
                }
            }
            Event::CursorFree(free) => aim.free = free,
            // The bullet of a skill leaves (0x766B4, 0x7ABB8): from the node of
            // hers its record names, at what the cursor points at and with the
            // lock point the cursor is on, with its sound (0x86568). A guided one
            // has the bullets fired along with it follow. Each first moves on the
            // frame after this one.
            // The bomb of a skill is thrown (0x797A8): only if the skill's bullet
            // is one, from the node of hers its record names, towards what the
            // cursor points at. It first moves on the frame after this one.
            Event::SkillShot { skill, firing: Firing::Lobbed } => {
                let shot = source.stagings.get(skill).and_then(|staging| staging.shot).filter(|shot| shot.kind == projectile::BOMB);
                match (shot, aim_point) {
                    (Some(shot), Some(aim)) => {
                        let node = u8::try_from(shot.node).ok().and_then(|id| skeleton.index_of(id));
                        let from = actor.transform() * node.map_or(Transform::IDENTITY, |index| animator.model_space(skeleton, index));
                        if let (Ok(sound), 0) = (u32::try_from(shot.sound), shot.sound_bank) {
                            board.play(COMMON_BANK, sound, FULL_VOLUME);
                        }
                        info!("her skill {skill} throws bomb {} from {:.0} towards {aim:.0}, {:.3} s in", shot.id, from.translation, time.elapsed_secs());
                        effects.ask_at_muzzle(&shot, from);
                        projectile::spawn_bomb(&mut commands, &shot_look, Bomb::hers(shot, skill, Some(from), aim), horde.next_frame());
                    }
                    _ => warn!("her skill {skill} throws nothing: she aims at {aim_point:?}"),
                }
            }
            Event::SkillShot { skill, firing } => {
                let class = if firing == Firing::Guided { projectile::GUIDED } else { projectile::STRAIGHT };
                let lock = aim.target.filter(|_| aim.locked);
                let mut next = source.stagings.get(skill).and_then(|staging| staging.shot);
                let mut fired = 0;
                // (No chain is as long as this; one that came back on itself
                // would be.)
                for _ in 0..16 {
                    let Some(shot) = next.take() else { break };
                    if let (true, Some(aim)) = (shot.kind == class, aim_point) {
                        let node = u8::try_from(shot.node).ok().and_then(|id| skeleton.index_of(id));
                        let from = actor.transform() * node.map_or(Transform::IDENTITY, |index| animator.model_space(skeleton, index));
                        if let (Ok(sound), 0) = (u32::try_from(shot.sound), shot.sound_bank) {
                            board.play(COMMON_BANK, sound, FULL_VOLUME);
                        }
                        info!("her skill {skill} fires bullet {} from {:.0} at {aim:.0}, locked on to {lock:?}, {:.3} s in", shot.id, from.translation, time.elapsed_secs());
                        effects.ask_at_muzzle(&shot, from);
                        projectile::spawn_skill_shot(&mut commands, &shot_look, SkillShot::new(shot, skill, Some(from), aim, lock), horde.next_frame());
                        fired += 1;
                    }
                    if firing == Firing::Guided && shot.next != 0 {
                        next = source.shots.of_player(shot.next).copied();
                    }
                }
                if fired == 0 {
                    warn!("her skill {skill} fires nothing: she aims at {aim_point:?}");
                }
            }
            Event::Give(id) => give_her(&mut actor, id, &source, &mut random, time.elapsed_secs()),
            Event::Lost => flow.lose(),
            // A sound of the battle's bank (0x8D33C).
            Event::Sound(number) => {
                board.play(COMMON_BANK, number, FULL_VOLUME);
            }
            // One of her lines, on the first stream, in place of what it was
            // playing (0xA7EAC).
            Event::Voice(name) => board.ask_all([Request::Hush(VOICE_STREAM, 0), Request::Say(VOICE_STREAM, format!("{VOICES}/{name}"))]),
            // An effect of hers (0x7D0E8): on a node of her model or on her whole
            // it goes with her; at a place, or where a node is, it stays there.
            Event::Effect { id, place, on_top } => effects.ask(match place {
                actor::Place::Node(node) => Asked { id, by: Some((her, Some(node))), at: Mat4::IDENTITY, fixed: true, of: None, list: List::last_if(on_top) },
                actor::Place::Actor(at, turn) => Asked::on(id, her, Mat4::from_rotation_translation(turn, at), on_top),
                actor::Place::WhereNode(node) => Asked { id, by: Some((her, Some(node))), at: Mat4::IDENTITY, fixed: false, of: None, list: List::last_if(on_top) },
                actor::Place::At(at, turn) => Asked { id, by: None, at: Mat4::from_rotation_translation(turn, at), fixed: false, of: None, list: List::last_if(on_top) },
            }),
            // What the scene of a skill does with its effects.
            Event::SceneEffect(made) => effects.ask(Asked::of_scene(&made, her)),
            Event::EffectsOver(id) => effects.remove(id),
            Event::EffectList(id, list) => effects.relist(id, List::of(list)),
        }
    }
    // The effects that a state of hers has be no more as it ends (0x767D8).
    for id in std::mem::take(&mut actor.effects_over) {
        effects.remove(id);
    }
    // The sounds of the effects she was given (0x8FC80).
    for sound in std::mem::take(&mut actor.status.heard) {
        board.play(COMMON_BANK, sound, FULL_VOLUME);
    }
    // Once the battle is decided nothing takes HP off her any more.
    actor.spared = flow.decided();

    // The camera of an enemy's scene is that enemy's to place, once it has had
    // its frame (`boss::direct`).
    if show.is_some_and(|show| show.scene_on()) {
        return;
    }
    // The camera: a staged move plays in the frame of her model (0xA400C), with a
    // lens of its own; otherwise it is the one over her shoulder.
    let (view, fov) = match &staging.playing {
        Some((clip, frame)) => {
            let pose = clip.pose(*frame);
            (actor.transform() * pose.transform, pose.vertical_fov(aim::SCREEN.x / aim::SCREEN.y))
        }
        None => (aim.view.transform(), aim::vertical_fov()),
    };
    // Under the scope of a snipe the camera closes in on what the cursor is on,
    // which comes to the middle of the screen (0x8C844).
    let (fov, moved) = match scope.on {
        true => scope.lens(fov, source.tunables.float(id::SNIPER_ENTER), aim.cursor),
        false => (fov, Vec2::ZERO),
    };
    // The picture is moved once everybody has had a turn, by that and by the
    // camera's shake (`shake::apply`).
    shake.scope = scope.on.then_some(moved);
    if let Projection::Perspective(perspective) = &mut *projection {
        if perspective.fov != fov {
            perspective.fov = fov;
        }
    }
    // A move starts with a cut, not a glide from where the camera was.
    if cut {
        camera.snap(view);
    } else {
        camera.current = view;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Class, Item};

    #[test]
    fn a_few_missions_have_terms_of_their_own() {
        assert_eq!(Terms::of_scenario(0), Terms::default());
        for scenario in [5, 25, 45, 65, 85, 102] {
            assert_eq!(Terms::of_scenario(scenario), Terms { half_experience: true, no_level_heal: false });
        }
        assert_eq!(Terms::of_scenario(66), Terms { half_experience: false, no_level_heal: true });
    }

    #[test]
    fn every_suit_and_weapon_she_can_have_on_has_a_model_a_battle_can_use() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The six suits are six characters of the field's list, the last far
        // down it; anything else is the Normal Suit's.
        let bundles: Vec<String> = (0x85..=0x8a).map(|suit| crate::field::tables::suit_bundle(&data, suit).unwrap()).collect();
        let named: Vec<&str> = bundles.iter().map(String::as_str).collect();
        assert_eq!(named[..5], ["FLD/FCHR/FCHR_P_BRS00.BIN", "FLD/FCHR/FCHR_P_BRS01.BIN", "FLD/FCHR/FCHR_P_BRS02.BIN", "FLD/FCHR/FCHR_P_BRS03.BIN", "FLD/FCHR/FCHR_P_BRS04.BIN"]);
        assert_eq!(named[5], "FLD/FCHR/FCHR_P_BRS05.BIN");
        assert_eq!(crate::field::tables::suit_bundle(&data, 0).unwrap(), bundles[0]);
        // Each has the nodes a battle goes by: where her shots come from, her
        // hands, and where her weapon hangs.
        for bundle in &bundles {
            let (model, _) = data.model(player::model_of(bundle)).unwrap();
            for id in [actor::MUZZLE_NODE, actor::WEAPON_NODE].into_iter().chain(HANDS) {
                assert!(model.nodes.iter().any(|node| node.id == id), "{bundle} has no node {id}");
            }
        }
        // The four cannons.
        for weapon in 0x81..=0x84 {
            data.model(player::weapon_model(weapon).unwrap()).unwrap();
        }
        assert_eq!((player::weapon_model(0x81).as_deref(), player::weapon_model(0x85), player::weapon_model(0)), (Some(WEAPON), None, None));
    }
    #[test]
    fn a_battle_won_leaves_her_with_what_it_was_worth() {
        let levels = Levels::for_tests();
        let items = Items::from_list(vec![Item { name: "Repairer 200".into(), class: Class::Consumable, real: true, limit: 5, ..Item::default() }]);
        let mut stats = PlayerStats {
            level: 1,
            experience: 10,
            max_hp: 372.0,
            hp: 120.0,
            attack: 44,
            special: 41,
            defence: 24,
            burst_bullets: 6,
            dodge_heat: 15,
            skill_bonus: [0; 4],
            skills: [0; 4],
            weapon: 129,
            suit: 133,
        };
        let mut inventory = Inventory::default();
        let spoils = Spoils { beaten: 4, experience: 20, items: [(1, 2), (0, 0), (0, 0), (0, 0)], ..Spoils::default() };

        let rewards = Rewards::give(&spoils, Terms::default(), &levels, &items, &mut stats, &mut inventory);
        assert_eq!(rewards.levels, (1, 2));
        assert_eq!((rewards.experience, rewards.total, rewards.next), (20, 30, Some(119)));
        assert_eq!(rewards.items, vec![("Repairer 200".to_owned(), 2)]);
        // The level gained has made her whole, and the items are hers.
        assert_eq!((stats.level, stats.experience, stats.hp, stats.max_hp), (2, 30, 434.0, 434.0));
        assert_eq!(inventory.held(1), 2);
    }
}