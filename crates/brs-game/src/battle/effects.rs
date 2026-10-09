//! The effects of a battle: what the fighters ask for by number (0x9E5D4, 0x9E660)
//! is found among the effect files the battle has (0x9CA6C), put where it is asked
//! for, and played until it is over or told to end (0x9E99C). How an effect goes
//! and is drawn is the engine's (`brs_engine::effect`, `brs_engine::effects`).
//!
//! An effect is put by a node of a model, or by a model whole, with a matrix of its
//! own from there (0xBD3AC). One that is fixed goes with what it was put by from
//! then on (0xBD4A8: it is made that node's child); one that is not stays where
//! that was as it began (0xBD438).
//!
//! # Whose an effect is, and when it has its time
//!
//! An effect has the time its owner gives it, and has it when it is drawn
//! (`Playing`). There are three kinds of owner:
//!
//! - **The battle** (the list at the root's + 0xD0): what she asks for (0x7D0E8),
//!   and the effect at the muzzle of every bullet (0x887B0). The root gives them
//!   their time before anybody has a turn (0x8D094), so one made in a frame is
//!   first seen with the one frame every effect begins with, and they go on
//!   whoever is held.
//! - **An enemy** (the list at its + 0x808): what its script asks for (0xBD400).
//!   It gives them their time in its own frame, before its script runs
//!   (0x921AC), and draws them when it is drawn (0x90EB8): held, they stand
//!   still, and out of sight they are not seen.
//! - **A bullet**: the effect that is its look is its model (0xA78B8), which has
//!   its time when the bullet has a frame of its own (0xA7520): from the frame
//!   after the look was given it. It is drawn while the bullet is to be seen.

use std::sync::Arc;

use anyhow::{Context as _, Result};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::effect::Camera;
use brs_engine::effects::{EffectCamera, EffectFile, EffectSystems, Playing};
use brs_engine::model::Skeleton;
use brs_engine::step::Stepped;
use brs_engine::AssetStores;

use super::actor::PcActor;
use super::dimmer;
use super::enemy::Horde;
use super::projectile;
use super::skills::{Hold, Made};
use super::status;
use super::tables::Shot;
use super::{Battle, BattleCamera, SIXTIETHS_PER_STEP};
use crate::data::GameData;

/// The archive whose effect files every battle has: the common ones, which most of
/// what happens to anybody is shown with.
const SYSTEM: &str = "SYSTEM.VOL";

/// Whether a file of an archive is one of effects (0x9D07C).
fn of_effects(file: &str) -> bool {
    let name = file.to_ascii_lowercase();
    name.ends_with(".efp") || name.ends_with(".efc")
}

/// The effect files of a folder, read: their names and bytes.
pub fn files_of(data: &GameData, folder: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for file in data.list(folder)?.into_iter().filter(|file| of_effects(file)) {
        let path = format!("{folder}/{file}");
        files.push((path.clone(), data.read(&path).with_context(|| format!("reading {path}"))?));
    }
    Ok(files)
}

/// The system's own.
pub fn common_files(data: &GameData) -> Result<Vec<(String, Vec<u8>)>> {
    files_of(data, SYSTEM)
}

/// An effect that is asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Asked {
    pub id: u32,
    /// What it is put by: a model, and the node of it with this id; the model
    /// whole if it has no such node, or none is named (0xBD398).
    pub by: Option<(Entity, Option<u8>)>,
    /// Where it is from that; in the world, with nothing to be put by.
    pub at: Mat4,
    /// It goes with what it is put by.
    pub fixed: bool,
    /// The enemy whose it is; the battle's own with none.
    pub of: Option<Entity>,
    /// The draw list of the scene it is of (0xACDD0).
    pub list: List,
}

/// The draw lists of the scene an effect can be of (0xACDD0). The scene is drawn
/// list by list (0xA6488), so what is of a later one is over what is of an
/// earlier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    /// The first, the sky's: before the map, which has the depth cleared for it
    /// and is drawn over it. When the sheet that is laid over what is behind her
    /// is all black it is the first thing of this list, and the map is not drawn
    /// (0x855AC, 0x97084; see `dimmer`): what else is of the list is then all
    /// there is behind her.
    Behind,
    /// The fourth, with whoever fights: what an effect is of unless it is said
    /// otherwise (0x9E6C4).
    Scene,
    /// The fifth and last, which has the depth cleared for it: after all else,
    /// whatever is nearer.
    Last,
}

impl List {
    /// By the number a list is named with.
    pub fn of(number: i32) -> List {
        match number {
            0 => List::Behind,
            4 => List::Last,
            _ => List::Scene,
        }
    }

    /// The last if it is to be seen whatever is before it, and else the one it
    /// would be of anyway.
    pub fn last_if(on_top: bool) -> List {
        if on_top { List::Last } else { List::Scene }
    }
}

impl Asked {
    /// One of the battle's own that goes with a model, so from it.
    pub fn on(id: u32, model: Entity, at: Mat4, on_top: bool) -> Asked {
        Asked { id, by: Some((model, None)), at, fixed: true, of: None, list: List::last_if(on_top) }
    }

    /// One of the battle's own that stays at a place.
    pub fn at(id: u32, at: Vec3, on_top: bool) -> Asked {
        Asked { id, by: None, at: Mat4::from_translation(at), fixed: false, of: None, list: List::last_if(on_top) }
    }

    /// One a scene of a skill of hers makes (0x7488C and its twins): by `her`
    /// model, as the scene has it put.
    pub fn of_scene(made: &Made, her: Entity) -> Asked {
        let (by, fixed) = match made.hold {
            Hold::Nowhere => (None, false),
            Hold::With(node) => (Some((her, node)), true),
            Hold::Left => (Some((her, None)), false),
        };
        Asked { id: made.id, by, at: made.matrix(), fixed, of: None, list: List::of(made.list) }
    }
}

/// The effect files a battle has, and what has been asked of them since they were
/// last seen to.
#[derive(Resource, Default)]
pub struct BattleEffects {
    /// In the order they are looked in for an effect.
    files: Vec<Arc<EffectFile>>,
    asked: Vec<Asked>,
    /// The numbers whose effects are to end, all that are playing of each
    /// (0x9E99C).
    ended: Vec<u32>,
    /// The numbers whose effects are to be no more, at once: all that are
    /// playing of each (0x9E9F4).
    removed: Vec<u32>,
    /// The numbers whose effects are to be of another draw list.
    relisted: Vec<(u32, List)>,
    /// The numbers asked for that no file has, each told of once.
    missing: Vec<u32>,
}

impl BattleEffects {
    /// Makes the files ready. One that cannot be read is told of and left out: what
    /// is asked of it is then not shown.
    pub fn load(files: &[(String, Vec<u8>)], stores: &mut AssetStores) -> BattleEffects {
        let mut ready = Vec::new();
        for (name, bytes) in files {
            match EffectFile::load(bytes, stores) {
                Ok(file) => ready.push(file),
                Err(error) => warn!("{name}: {error:#}"),
            }
        }
        BattleEffects { files: ready, ..default() }
    }

    pub fn ask(&mut self, asked: Asked) {
        self.asked.push(asked);
    }

    /// Every effect of a number that plays is no more, at once (0x9E9F4): not
    /// told to die away, as one that is ended is.
    pub fn remove(&mut self, id: u32) {
        self.removed.push(id);
    }

    /// Those of a number that play are of another draw list from now on
    /// (0xACDD0 on one that was kept).
    pub fn relist(&mut self, id: u32, list: List) {
        self.relisted.push((id, list));
    }

    /// Asks for the effect at the muzzle of a bullet that leaves from `node`, if
    /// its record names one: it stays where the bullet leaves from, and is the
    /// battle's own (0x887B0).
    pub fn ask_at_muzzle(&mut self, shot: &Shot, node: Transform) {
        if let Some(id) = u32::try_from(shot.muzzle_effect).ok().filter(|id| *id > 0) {
            self.ask(Asked { id, by: None, at: projectile::muzzle(shot, Some(node)), fixed: false, of: None, list: List::Scene });
        }
    }

    /// The effect of a number, from the first file that has it, to play.
    fn play(&mut self, id: u32, what: &str) -> Option<Playing> {
        let found = self.files.iter().find_map(|file| file.play(id));
        if found.is_none() && !self.missing.contains(&id) {
            self.missing.push(id);
            warn!("no effect file of the battle has effect {id}, {what}");
        }
        found
    }
}

/// On an effect of the battle: its number.
#[derive(Component)]
pub struct OfBattle(pub u32);

/// On an effect that is an enemy's own.
#[derive(Component)]
struct OfEnemy(Entity);

/// On an effect that goes with what it was put by.
#[derive(Component)]
struct Fixed {
    by: Entity,
    node: Option<u8>,
    at: Mat4,
    /// It is the look of what it goes with, and goes when that has another look
    /// or none.
    look: bool,
}

/// On a bullet: the effect that is its look (its record's, 0x12940C), which goes
/// with it and has its time from it. It is there to be seen while the bullet is,
/// and goes when the bullet does or is given another look.
#[derive(Component)]
pub struct Look {
    effect: u32,
    scale: f32,
    playing: Option<Entity>,
    /// The time it has been given while there was nothing of it yet to have it;
    /// none before the first frame it is seen to.
    waited: Option<f32>,
    /// It has been told to die away, and its effect has been told so.
    ending: bool,
    told: bool,
}

impl Look {
    /// The look of a record's effect, if it names one.
    pub fn of(effect: i32, scale: f32) -> Option<Look> {
        let effect = u32::try_from(effect).ok().filter(|effect| *effect > 0)?;
        Some(Look { effect, scale, playing: None, waited: None, ending: false, told: false })
    }

    /// Tells it to die away (0xA6020): what of it goes round does so no more, and
    /// it comes to its end.
    pub fn end(&mut self) {
        self.ending = true;
    }
}

type Anchors<'w, 's> = Query<'w, 's, (&'static Stepped, Option<&'static Skeleton>, Option<&'static Animator>)>;

/// Where a model, or a node of it, is in the world as the battle's frame has it.
fn anchor(anchors: &Anchors, by: Entity, node: Option<u8>) -> Option<Mat4> {
    let (stepped, skeleton, animator) = anchors.get(by).ok()?;
    let model = stepped.current.to_matrix();
    let node = node.and_then(|id| {
        let (skeleton, animator) = (skeleton?, animator?);
        Some(animator.model_space(skeleton, skeleton.index_of(id)?).to_matrix())
    });
    Some(node.map_or(model, |node| model * node))
}

/// The effects go by the battle's camera as its frame has it.
fn see(camera: Single<&Stepped, With<BattleCamera>>, mut seen: ResMut<EffectCamera>) {
    seen.0 = Camera::of(camera.current.to_matrix());
}

/// Gives the effects there are the frame's time: the battle's own whatever
/// happens, an enemy's unless nobody but her has a frame; and an enemy's are out
/// of sight when it is. (A bullet's look has its time from the bullet: `dress`.)
fn tick(horde: Res<Horde>, mut playing: Query<(&mut Playing, Option<&OfEnemy>, Option<&Fixed>), With<OfBattle>>) {
    for (mut playing, of, fixed) in &mut playing {
        if fixed.is_some_and(|fixed| fixed.look) {
            continue;
        }
        match of {
            None => playing.give(SIXTIETHS_PER_STEP),
            Some(OfEnemy(enemy)) => {
                if !horde.frozen() {
                    playing.give(SIXTIETHS_PER_STEP);
                }
                let hidden = horde.out_of_sight(*enemy);
                if playing.hidden != hidden {
                    playing.hidden = hidden;
                }
            }
        }
    }
}

/// Gives the bullets their looks and the looks their time, and tells those that
/// are to die away.
fn dress(
    mut commands: Commands,
    horde: Res<Horde>,
    mut effects: ResMut<BattleEffects>,
    mut looks: Query<(Entity, &mut Look, &Visibility, &Stepped)>,
    mut playing: Query<&mut Playing>,
) {
    for (entity, mut look, visibility, stepped) in &mut looks {
        let seen = *visibility != Visibility::Hidden;
        // The frame a look is given in has no time for it, nor has one in which
        // nobody but her has a frame.
        let given = match look.waited {
            None => 0.0,
            Some(_) if horde.frozen() => 0.0,
            Some(_) => SIXTIETHS_PER_STEP,
        };
        match look.playing {
            Some(made) => {
                look.waited = Some(0.0);
                // One that is over has gone, and nothing is left to see to.
                let Ok(mut playing) = playing.get_mut(made) else { continue };
                playing.give(given);
                if playing.hidden == seen {
                    playing.hidden = !seen;
                }
                if look.ending && !look.told {
                    playing.effect.end();
                    look.told = true;
                }
            }
            // Until the bullet is first to be seen nothing draws its look, and the
            // time the look is given waits for it (0xA5E4C).
            None => {
                let waited = look.waited.unwrap_or(0.0) + given;
                look.waited = Some(waited);
                if !seen {
                    continue;
                }
                let Some(mut effect) = effects.play(look.effect, "which a bullet looks like") else { continue };
                let at = Mat4::from_scale(Vec3::splat(look.scale));
                effect.place(stepped.current.to_matrix() * at);
                effect.give(waited);
                if look.ending {
                    effect.effect.end();
                    look.told = true;
                }
                let made = commands.spawn((Battle, OfBattle(look.effect), effect, Fixed { by: entity, node: None, at, look: true })).id();
                look.playing = Some(made);
            }
        }
    }
}

/// What is seen of the status effects she has been given stays where she stands
/// (0x8FCEC).
fn given(mut player: Single<&mut PcActor>, mut effects: ResMut<BattleEffects>) {
    let at = player.transform().translation;
    for burst in std::mem::take(&mut player.status.seen) {
        effects.ask(Asked::at(status::BURSTS_ON_HER[burst], at, false));
    }
}

/// Begins the effects asked for, and tells those that are to end.
fn begin(
    mut commands: Commands,
    mut effects: ResMut<BattleEffects>,
    mut horde: ResMut<Horde>,
    anchors: Anchors,
    mut playing: Query<(Entity, &OfBattle, &mut Playing)>,
    theirs: Query<(Entity, &OfEnemy), With<Playing>>,
) {
    let effects = &mut *effects;
    effects.asked.append(&mut horde.effects_asked);
    effects.ended.append(&mut horde.effects_ended);
    // An enemy's own are no more when its script has said so (0x9E7E0 on its
    // own pool of them): those that were playing, not what it asks for now.
    let dropped = std::mem::take(&mut horde.effects_dropped);
    for (entity, of) in &theirs {
        if dropped.contains(&of.0) {
            commands.entity(entity).despawn();
        }
    }
    for asked in std::mem::take(&mut effects.asked) {
        let Some(mut effect) = effects.play(asked.id, "which was asked for") else { continue };
        let by = asked.by.and_then(|(by, node)| Some((by, node, anchor(&anchors, by, node)?)));
        effect.place(by.map_or(asked.at, |(.., anchor)| anchor * asked.at));
        effect.on_top = asked.list == List::Last;
        effect.behind = (asked.list == List::Behind).then(|| RenderLayers::layer(dimmer::BACKDROP_LAYER));
        let mut made = commands.spawn((Battle, OfBattle(asked.id), effect));
        if let (Some((by, node, _)), true) = (by, asked.fixed) {
            made.insert(Fixed { by, node, at: asked.at, look: false });
        }
        if let Some(enemy) = asked.of {
            made.insert(OfEnemy(enemy));
        }
    }
    for id in std::mem::take(&mut effects.ended) {
        for (_, of, mut playing) in &mut playing {
            if of.0 == id {
                playing.effect.end();
            }
        }
    }
    for (id, list) in std::mem::take(&mut effects.relisted) {
        for (_, of, mut playing) in &mut playing {
            if of.0 == id && playing.on_top != (list == List::Last) {
                playing.on_top = list == List::Last;
            }
        }
    }
    let removed = std::mem::take(&mut effects.removed);
    for (entity, of, _) in &playing {
        if removed.contains(&of.0) {
            commands.entity(entity).despawn();
        }
    }
}

/// Keeps the effects that are fixed to something where that is; one whose something
/// is no more goes with it (0x9E7E0), and so does a look that is no longer its
/// bullet's.
fn follow(mut commands: Commands, anchors: Anchors, looks: Query<&Look>, mut fixed: Query<(Entity, &Fixed, &mut Playing)>) {
    for (entity, fixed, mut playing) in &mut fixed {
        let its = !fixed.look || looks.get(fixed.by).is_ok_and(|look| look.playing == Some(entity));
        match anchor(&anchors, fixed.by, fixed.node).filter(|_| its) {
            Some(anchor) => playing.place(anchor * fixed.at),
            None => commands.entity(entity).despawn(),
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<BattleEffects>();
}

/// The systems of a battle's frame that see to its effects, to run once everybody
/// has had a turn. While the battle is paused they do not run, and its effects
/// stand still: nothing gives them time (0x9E508).
pub fn systems() -> impl bevy::ecs::schedule::IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    (see, tick, dress, given, begin, follow).chain().before(EffectSystems::Step)
}
