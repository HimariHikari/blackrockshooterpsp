//! The enemies of a battle: the group it is fought against, each member an actor run
//! by its own script (the original's `BtEnActor`), and what passes between them, the
//! player and her shots.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::model::{ModelAsset, Skeleton};
use brs_engine::step::{Stepped, SIXTIETHS_PER_STEP};
use brs_engine::AssetStores;
use brs_formats::egtb;

use super::actor::{LockOn, PcActor, Shown};
use super::aim::Target;
use super::boss::Direction;
use super::bullet::{self, Bullet};
use super::damage::{Assailant, Attacker, Random};
use super::effects::{Asked, List, Look};
use super::flow::Flow;
use super::log;
use super::numbers;
use super::shake;
use super::projectile::{self, Beam, Bomb, Fired, Lock, Projectile, ProjectileLook, Ray, SkillRays, SkillShot, Strike};
use super::skills::{Blow, Strikes};
use super::status::{kind, Effects, Giver, Status};
use crate::records;
use super::tables::{self, Archive, PlayerStats, Shot, Shots};
use super::{Battle, BattleSource, Stage};
use crate::data::GameData;
use crate::sound::{Board, Request, COMMON_BANK, FULL_VOLUME};

pub mod actor;
pub mod mover;
pub mod reserves;

use actor::{signal, EnActor, Event, Models, Peer, PlayerView, Shared, Spawn, Spoils, Surroundings, Told, BANK_OF_BATTLE, BANK_OF_ENEMY, BANK_OF_FINISH, SLOTS};
use reserves::{mode, Plan, Reserve, Reserves};

/// The node of the player's model her body's collision sphere is on, and the sphere's
/// radius (set up at 0x99C9C).
const PLAYER_BODY_NODE: u8 = 0;
const PLAYER_BODY_RADIUS: f32 = 45.0;
/// The node at her right ankle, which one of the enemies' moves goes by (0xB5C40).
const PLAYER_ANKLE_NODE: u8 = 0x17;
/// The attack a normal shot is, in the attack table (0x98DE8).
const SHOT_ATTACK: i32 = 5;

/// The group a battle is fought against, with every member's archive loaded.
pub struct Group {
    pub id: u16,
    /// Bits of [`egtb::flag`].
    flags: u16,
    /// The rule its later enemies come in by: see [`mode`].
    mode: u8,
    members: Vec<(egtb::Member, Arc<Archive>)>,
    /// The enemies it keeps back, with their archives, and the places they can come
    /// in at.
    reserves: Vec<Reserve>,
    reserve_archives: Vec<Arc<Archive>>,
    places: Vec<egtb::Member>,
}

impl Group {
    /// Whether the player strikes her pose when this group is beaten.
    pub fn victory_pose(&self) -> bool {
        self.flags & egtb::flag::VICTORY_POSE != 0
    }

    /// Whether a battle won against it has nothing follow it (0x9D5FC): no last
    /// state of hers, none of her skills that act on a battle won, and no
    /// results. So it is with two enemies, each alone in the two groups it has
    /// (9 and 1006, 29 and 1020), when one of them is among those the battle
    /// begins with.
    pub fn no_results(&self) -> bool {
        const WITHOUT: [u16; 2] = [110, 122];
        self.members.iter().any(|(member, _)| WITHOUT.contains(&member.enemy))
    }

    /// What it has for the enemies that come in later.
    fn plan(&self) -> Plan<'_> {
        Plan { mode: self.mode, reserves: &self.reserves, places: &self.places }
    }

    /// The folders of the archives of all its enemies, and of those their scripts
    /// load, each once, in the order they are first named.
    pub fn folders(&self) -> Vec<String> {
        let mut folders: Vec<String> = Vec::new();
        for archive in self.members.iter().map(|(_, archive)| archive).chain(&self.reserve_archives) {
            for folder in archive.folders() {
                if !folders.contains(folder) {
                    folders.push(folder.clone());
                }
            }
        }
        folders
    }

    /// Whether the battle against it is begun by its enemies' script (the group's
    /// flag 4, which `BtEnter` goes by, 0x93DE8): a boss's.
    pub fn scripted(&self) -> bool {
        self.flags & egtb::flag::NO_INTRO != 0
    }

    /// The packs of the layouts its enemies' archives have, each once.
    pub fn layouts(&self) -> Vec<String> {
        let mut packs: Vec<String> = Vec::new();
        for archive in self.members.iter().map(|(_, archive)| archive).chain(&self.reserve_archives) {
            for (_, pack) in archive.layouts() {
                if !packs.iter().any(|known| known == pack) {
                    packs.push(pack.to_owned());
                }
            }
        }
        packs
    }

    pub fn load(data: &GameData, id: u16) -> Result<Group> {
        let groups = tables::groups(data)?;
        let group = groups.get(id).with_context(|| format!("there is no enemy group {id}"))?;
        let mut archives: HashMap<u16, Arc<Archive>> = HashMap::new();
        let mut archive = |enemy: u16| -> Result<Arc<Archive>> {
            if let Some(archive) = archives.get(&enemy) {
                return Ok(archive.clone());
            }
            let archive = Archive::load(data, enemy).with_context(|| format!("loading enemy {enemy}"))?;
            Ok(archives.entry(enemy).or_insert(Arc::new(archive)).clone())
        };
        let mut members = Vec::new();
        for member in tables::members(group) {
            members.push((*member, archive(member.enemy)?));
        }
        // A group with no rule for them never sees the enemies it lists as kept
        // back (0xA0954), so they are left out.
        let (mut reserves, mut reserve_archives) = (Vec::new(), Vec::new());
        if group.mode != mode::NONE {
            for wave in &group.waves {
                let archive = archive(wave.enemy)?;
                reserves.push(Reserve { wave: *wave, flies: archive.figures.flies(), room: archive.figures.room() });
                reserve_archives.push(archive);
            }
        }
        Ok(Group { id, flags: group.flags, mode: group.mode, members, reserves, reserve_archives, places: group.places.clone() })
    }
}

/// One enemy of the battle.
#[derive(Component)]
pub struct Enemy {
    pub actor: EnActor,
    archive: Arc<Archive>,
    /// The entities of the models its script has loaded, by slot.
    models: [Option<Entity>; SLOTS],
    /// The round shadows its script has given nodes of those: the slot, the
    /// node, and what was made for it.
    shadows: Vec<(usize, u8, Entity)>,
    /// What its script was last seen doing, for logging only what changes.
    traced: String,
    /// The logic frame of its last turn.
    stepped: u32,
}

/// On the root of every model an enemy's script has loaded.
#[derive(Component)]
pub struct EnemyModel;

/// A sphere of an enemy that the player's shots can hit.
#[derive(Clone, Copy)]
struct BodySphere {
    enemy: Entity,
    slot: usize,
    centre: Vec3,
    radius: f32,
}

/// A point of an enemy that the cursor can lock onto.
#[derive(Clone, Copy)]
pub struct LockAt {
    pub enemy: Entity,
    /// The enemy's number among those that have joined the battle.
    pub number: usize,
    /// Which of its gauges the point belongs to.
    pub gauge: usize,
    pub at: Vec3,
}

/// The mark over an enemy (0x90A78, 0x3DD0C): where it hangs, at the node of the
/// enemy that its script names, and what it shows: the share of its HP the enemy
/// has, its level, whether it has a second life to come, and the marks of the
/// status effects on it that are up, each by its icon.
#[derive(Clone, Debug)]
pub struct MarkAt {
    pub at: Vec3,
    pub share: f32,
    pub level: i32,
    pub second_life: bool,
    pub icons: Vec<usize>,
}

/// A status effect an enemy's script hands to someone other than the enemy itself,
/// or takes from them.
#[derive(Clone, Debug)]
struct Gift {
    /// The enemy that gives it, and what it counts for as the giver.
    from: usize,
    giver: Giver,
    id: i32,
    give: bool,
    to: Whom,
}

#[derive(Clone, Copy, Debug)]
enum Whom {
    Player,
    /// Every other enemy, and with `itself` the giver too (engine function 120).
    Enemies { itself: bool },
}

/// What the enemies share, and what the rest of the battle sees of them.
#[derive(Resource, Default)]
pub struct Horde {
    /// The number of the group, which scripts can ask for.
    group: i32,
    /// What the enemies have between them: each other, and the group's reserves.
    shared: Shared,
    /// The models built so far, by enemy number and file name.
    assets: HashMap<(u16, String), ModelAsset>,
    /// The bullet classes that are not ported and have been fired, for telling of
    /// each only once.
    unported_shots: HashSet<i32>,
    /// Rays fired this frame, which strike once every enemy has had its turn.
    rays: Vec<Ray>,
    /// Status effects handed out by a script and not yet delivered: that is done as
    /// soon as the enemy whose turn it is has finished it.
    gifts: Vec<Gift>,
    /// Logic frames the enemies have run.
    steps: u32,
    /// The enemies and every bullet have been put out of sight for a staged moment,
    /// and which of them were to be seen before that.
    hidden: bool,
    unseen: Vec<Entity>,
    /// The one enemy that is to be seen while the others are not.
    alone: Option<Entity>,
    /// Nothing of theirs runs this frame: the player has the stage.
    frozen: bool,
    /// The enemies' body spheres as of their last frame.
    bodies: Vec<BodySphere>,
    /// The points of the enemies that can be aimed at.
    pub lock_points: Vec<LockAt>,
    /// Where each enemy's model stands, its first, and whether the enemy has a
    /// body sphere in use: as of their last frame.
    pub places: Vec<(Entity, Vec3, bool)>,
    /// The marks over the enemies: one for each that is not dead (0x92C6C).
    pub marks: Vec<MarkAt>,
    /// How many enemies are left: in the battle, and neither dead nor dying.
    pub left: usize,
    /// How many are left to beat, with those that have yet to come in, or -1 while
    /// that cannot be told (0xA0B7C): what the display counts (0xA05A4).
    pub to_beat: i32,
    /// What the enemies have for the battle's log, until it takes them.
    pub lines: Vec<log::Line>,
    /// The effects their scripts have asked for, and the numbers of those they
    /// have told to end, until the battle's effects take them; and the enemies
    /// all of whose own effects are to be no more.
    pub effects_asked: Vec<Asked>,
    pub effects_ended: Vec<u32>,
    pub effects_dropped: Vec<Entity>,
    /// The numbers there are to put up for the enemies, until the battle takes
    /// them.
    pub numbers: Vec<numbers::Asked>,
    /// The shakes of the camera asked for by what the enemies do and what is
    /// done to them, until the battle takes them.
    pub jolts: Vec<shake::Asked>,
    /// The enemies her hits have ended, as the records are to be told of them
    /// (0x9BDAC), until the battle does so.
    pub beaten: Vec<records::Event>,
    /// What their scripts have asked of the battle as a whole, until it is done
    /// (`boss::direct`).
    pub directions: Vec<Direction>,
    /// The streams they have given something to say since the battle last looked.
    pub said: [bool; 2],
    /// The times one of them has asked for the dying to be made again, each with
    /// whether those made are to be sent the signal for it, until that is done.
    revivals: Vec<bool>,
}

impl Horde {
    /// What the cursor's ray can land on (0x98560): the enemies' body spheres.
    pub fn targets(&self) -> Vec<Target> {
        self.bodies.iter().map(|body| Target { centre: body.centre, radius: body.radius }).collect()
    }

    /// An enemy the cursor is locked on to, as a skill that is for one enemy sees
    /// it: with whether it has a body sphere in use (0x97FC0).
    pub fn lock_on(&self, enemy: Entity, gauge: usize) -> LockOn {
        LockOn { enemy, gauge, open: self.bodies.iter().any(|body| body.enemy == enemy) }
    }

    /// What beating the enemies has been worth so far.
    pub fn spoils(&self) -> &Spoils {
        &self.shared.spoils
    }

    /// Whether an enemy has the stage (the root's flag 10): she does nothing then
    /// but play her animation (0x99EC4), and no other actor has a frame or is
    /// seen (0xBE208).
    pub fn has_the_stage(&self) -> bool {
        self.shared.told.on_stage.is_some()
    }

    /// What the bosses' scripts are told of the battle, for the battle to keep.
    pub fn told(&mut self) -> &mut Told {
        &mut self.shared.told
    }
}

/// The models the enemies' scripts have loaded: their animation players, skeletons
/// and where they stand.
type ModelQuery<'w, 's, 'q> = Query<'w, 's, (&'q mut Animator, &'q Skeleton, &'q mut Stepped), With<EnemyModel>>;

/// The models of one enemy, as the game has them: entities with an animation player.
struct Spawned<'a, 'w, 's, 'q> {
    entities: &'a [Option<Entity>; SLOTS],
    query: &'a mut ModelQuery<'w, 's, 'q>,
}

impl Models for Spawned<'_, '_, '_, '_> {
    fn animator(&mut self, slot: usize) -> Option<&mut Animator> {
        let entity = (*self.entities.get(slot)?)?;
        self.query.get_mut(entity).ok().map(|(animator, _, _)| animator.into_inner())
    }

    fn node(&self, slot: usize, node: u8) -> Option<Transform> {
        let entity = (*self.entities.get(slot)?)?;
        let (animator, skeleton, _) = self.query.get(entity).ok()?;
        Some(animator.model_space(skeleton, skeleton.index_of(node)?))
    }

    fn turn(&mut self, slot: usize, node: u8, rotation: Quat) {
        let Some(entity) = self.entities.get(slot).copied().flatten() else { return };
        let Ok((mut animator, skeleton, _)) = self.query.get_mut(entity) else { return };
        if let Some(index) = skeleton.index_of(node) {
            animator.set_own_turn(index, rotation);
        }
    }
}

/// No models: what an enemy has while its script first sets it up.
struct NoModels;

impl Models for NoModels {
    fn animator(&mut self, _: usize) -> Option<&mut Animator> {
        None
    }

    fn node(&self, _: usize, _: u8) -> Option<Transform> {
        None
    }

    fn turn(&mut self, _: usize, _: u8, _: Quat) {}
}

/// What the player looks like to the enemies right now. `nodes` are her model's
/// animation player and skeleton, where the caller has them: the enemies go by where
/// two nodes of her model are. Without them she is taken to be where she stands.
fn view_of(player: &PcActor, nodes: Option<(&Animator, &Skeleton)>) -> PlayerView {
    let at = |id: u8| {
        let (animator, skeleton) = nodes?;
        Some((player.transform() * animator.model_space(skeleton, skeleton.index_of(id)?)).translation)
    };
    PlayerView {
        position: at(PLAYER_BODY_NODE).unwrap_or(player.position),
        ankle: at(PLAYER_ANKLE_NODE).unwrap_or(player.position),
        hp: player.hp,
        max_hp: player.max_hp,
        action: player.action,
        heat: if player.heat.max > 0.0 { player.heat.value / player.heat.max } else { 0.0 },
        place: player.slot,
        status: player.status.marks(),
    }
}

/// The player as she lands a hit: her figures for the damage, her level, and the
/// status effects on her (the stats 0x92608 reads).
fn player_assailant<'a>(stats: &PlayerStats, status: &'a Status) -> Assailant<'a> {
    Assailant { figures: Attacker { attack: stats.attack, special: stats.special }, level: stats.level, status }
}

/// What an enemy's turn takes besides the enemy itself: the battle's tables, what the
/// enemies share, and the means to put things into the scene.
struct Turn<'a, 'w, 's, 'r> {
    commands: &'a mut Commands<'w, 's>,
    group: &'a Group,
    shots: &'a Shots,
    effects: &'a Effects,
    stage: &'a Stage,
    look: &'a ProjectileLook,
    /// The battle is decided: hits take no HP off any more.
    decided: bool,
    horde: &'a mut Horde,
    random: &'a mut Random,
    stores: &'a mut AssetStores<'r>,
    /// Where what is to be heard is asked for.
    board: &'a mut Board,
    /// The skill of hers whose hit is being carried out, or nought: the records
    /// are told which it was that ended an enemy (0x92C34).
    skill: i32,
}

/// The header of the bank a sound is of: the battle's, or that of the enemy whose
/// archive this is (0xBC3CC).
fn bank_of(kind: i32, archive: &Archive) -> Option<&str> {
    match kind {
        BANK_OF_BATTLE => Some(COMMON_BANK),
        BANK_OF_ENEMY => archive.bank.as_deref(),
        BANK_OF_FINISH => archive.finish_bank.as_deref(),
        _ => None,
    }
}

/// What an attack that lands on the player is seen and heard as.
struct Landing<'a> {
    attack: &'a tables::Attack,
    outcome: &'a super::actor::HitOutcome,
    /// Where it lands.
    at: Vec3,
    /// Her model.
    her: Entity,
}

/// What is seen and heard when an attack lands on the player (0x9B3E8 to 0x9B580).
/// A guarded hit has her guard's effect go with her, before her, and its sound;
/// a skill of hers that acted on the guard has its own on her, over everything
/// (0x9F6D8). Where the hit lands there is the effect of the ailment it gave her
/// or of the attack's look, over everything. Then the attack's own sound, of
/// the battle's bank or of the attacker's.
fn tell_hit(board: &mut Board, asked: &mut Vec<Asked>, landing: Landing, effects: &Effects, attacker: &Archive) {
    let Landing { attack, outcome, at, her } = landing;
    if outcome.guard_acted {
        asked.push(Asked::on(super::auto::EFFECT, her, Mat4::IDENTITY, true));
    }
    if outcome.guarded {
        asked.push(Asked::on(super::actor::EFFECT_GUARD, her, Mat4::from_translation(super::actor::GUARD_EFFECT_AT), false));
        board.play(COMMON_BANK, actor::SOUND_GUARDED as u32, FULL_VOLUME);
    }
    if let Some(id) = super::actor::seen_of_hit(attack, outcome, effects) {
        asked.push(Asked::at(id, at, true));
    }
    if let (Ok(sound), Some(bank)) = (u32::try_from(attack.sound), bank_of(attack.sound_bank, attacker)) {
        board.play(bank, sound, FULL_VOLUME);
    }
}

impl Turn<'_, '_, '_, '_> {
    /// The battle as the script of an enemy of `archive` sees it.
    fn surroundings<'b>(
        &'b mut self,
        player: PlayerView,
        archive: &'b Archive,
        models: &'b mut dyn Models,
        events: &'b mut Vec<Event>,
    ) -> Surroundings<'b> {
        Surroundings {
            staged: self.horde.shared.told.on_stage.is_some(),
            dt: SIXTIETHS_PER_STEP,
            decided: self.decided,
            archive,
            shots: self.shots,
            effects: self.effects,
            stage: self.stage,
            player,
            group: self.horde.group,
            plan: self.group.plan(),
            shared: &mut self.horde.shared,
            random: &mut *self.random,
            models,
            events,
        }
    }

    /// Makes an enemy and has its script set it up, which loads its model (0x90CC0).
    /// One that comes in later is then told it has been called (0xA0E70). Its place
    /// among the peers is already there.
    ///
    /// One that is made again in place of one that was dying (`again`, 0x91830) is
    /// told it has been called whatever it was at first, has its AI on from the
    /// start, and is sent one signal more if whoever had it made says so.
    fn enlist(&mut self, player: PlayerView, spawn: Spawn, archive: &Arc<Archive>, again: Option<bool>) -> Result<()> {
        let mut events = Vec::new();
        let mut none = NoModels;
        let mut world = self.surroundings(player, archive, &mut none, &mut events);
        let mut actor = EnActor::new(spawn, &mut world).with_context(|| format!("setting up {}", archive.folder))?;
        if spawn.wave.is_some() || again.is_some() {
            actor.send(signal::CALLED, &mut world).with_context(|| format!("calling in {}", archive.folder))?;
        }
        if let Some(think) = again {
            actor.body.thinking = true;
            if think {
                actor.send(actor::SIGNAL_MADE_AGAIN, &mut world).with_context(|| format!("making {} again", archive.folder))?;
            }
        }
        let mut enemy = Enemy { actor, archive: archive.clone(), models: [None; SLOTS], shadows: Vec::new(), traced: String::new(), stepped: 0 };
        let entity = self.commands.spawn(Battle).id();
        self.carry_out(player, entity, &mut enemy, events);
        self.note(&mut enemy);
        self.commands.entity(entity).insert(enemy);
        Ok(())
    }

    /// Lets the others see an enemy as its turn has left it, has the sounds of the
    /// effects it was given since played (0x8FC80), and gives the log what there is
    /// to say of them (0x8DE7C, 0x8DEF8).
    fn note(&mut self, enemy: &mut Enemy) {
        for sound in std::mem::take(&mut enemy.actor.body.status.heard) {
            self.board.play(COMMON_BANK, sound, FULL_VOLUME);
        }
        // What is seen of an effect it was given stays where it stands (0x8FD4C).
        let at = enemy.actor.body.placements[0].translation;
        for burst in std::mem::take(&mut enemy.actor.body.status.seen) {
            self.horde.effects_asked.push(Asked::at(super::status::BURSTS_ON_AN_ENEMY[burst], at, false));
        }
        self.horde.lines.append(&mut enemy.actor.body.status.said);
        let body = &enemy.actor.body;
        let peer = &mut self.horde.shared.peers[body.index];
        *peer = body.peer(peer.token);
    }

    /// Delivers the status effects that scripts have handed to the player or to the
    /// other enemies (0xC02D8, 0xBF020). It is done between turns, when no enemy is
    /// in the middle of its own.
    fn deliver(&mut self, player: &mut PcActor, stats: &PlayerStats, enemies: &mut Query<(Entity, &mut Enemy)>) {
        for gift in std::mem::take(&mut self.horde.gifts) {
            let Some(effect) = self.effects.get(gift.id) else {
                warn!("enemy {}: there is no status effect {}", gift.from, gift.id);
                continue;
            };
            match gift.to {
                Whom::Player if gift.give => {
                    let outcome = player.afflict(effect, &gift.giver, stats, self.random);
                    info!("enemy {} gives the player status effect {}: {outcome:?}", gift.from, gift.id);
                }
                Whom::Player => {
                    player.status.take(gift.id);
                }
                Whom::Enemies { itself } => {
                    // Nothing happens unless somebody stands for it to go to; then
                    // it goes to every enemy there is that has a model to show it on.
                    let standing = self.horde.shared.peers.iter().filter(|peer| peer.alive).count();
                    if standing <= usize::from(!itself) {
                        continue;
                    }
                    for (index, entity) in in_order(enemies, |_| true) {
                        let Ok((_, mut enemy)) = enemies.get_mut(entity) else { continue };
                        let body = &mut enemy.actor.body;
                        if (index == gift.from && !itself) || !body.anchored() {
                            continue;
                        }
                        if gift.give {
                            let outcome = body.afflict(effect, &gift.giver, self.random);
                            info!("enemy {} gives enemy {index} status effect {}: {outcome:?}", gift.from, gift.id);
                        } else {
                            body.status.take(gift.id);
                        }
                        self.note(&mut enemy);
                    }
                }
            }
        }
    }

    /// Does what the script of the enemy `entity` asked of the game.
    fn carry_out(&mut self, player: PlayerView, entity: Entity, enemy: &mut Enemy, events: Vec<Event>) {
        let index = enemy.actor.body.index;
        for event in events {
            match event {
                Event::Model { slot, name } => {
                    let Some(model) = enemy.archive.model(&name) else {
                        error!("{}: there is no model {name}", enemy.archive.folder);
                        continue;
                    };
                    let asset = self
                        .horde
                        .assets
                        .entry((enemy.archive.enemy, name))
                        .or_insert_with(|| ModelAsset::build(&model.0, &model.1, self.stores));
                    let at = enemy.actor.body.placements[slot];
                    let root = asset.spawn(self.commands, at);
                    self.commands.entity(root).insert((Battle, EnemyModel, Animator::new(), Stepped::new(at)));
                    if let Some(old) = enemy.models[slot].replace(root) {
                        self.commands.entity(old).despawn();
                    }
                }
                Event::Unload { slot } => {
                    if let Some(model) = enemy.models[slot].take() {
                        self.commands.entity(model).despawn();
                    }
                }
                Event::Show { slot, shown } => {
                    if let Some(model) = enemy.models[slot] {
                        let visibility = if shown { Visibility::Inherited } else { Visibility::Hidden };
                        self.commands.entity(model).insert(visibility);
                    }
                }
                Event::Shadow { slot, node, radius, strength } => {
                    // A node has one shadow: given again, it is as it is now said.
                    if let Some(at) = enemy.shadows.iter().position(|(of, under, _)| (*of, *under) == (slot, node)) {
                        let (_, _, old) = enemy.shadows.swap_remove(at);
                        self.commands.entity(old).try_despawn();
                    }
                    if let Some(model) = enemy.models[slot] {
                        enemy.shadows.push((slot, node, super::shadows::give(self.commands, model, node, radius, strength)));
                    }
                }
                Event::Shoot { shot, node, aim } => {
                    // What the bullet shows and sounds like as it leaves (0x86728).
                    debug!(
                        "enemy {index} fires bullet {} of class {} at {aim:.0}: effect {} at the muzzle, sound {} of bank {}, looking like effect {}",
                        shot.id, shot.kind, shot.muzzle_effect, shot.sound, shot.sound_bank, shot.effect
                    );
                    if let (Ok(sound), Some(bank)) = (u32::try_from(shot.sound), bank_of(shot.sound_bank, &enemy.archive)) {
                        self.board.play(bank, sound, FULL_VOLUME);
                    }
                    // The effect at its muzzle stays where it leaves from (0x86784). A
                    // strike has none: its frame asks for its sound and its look only.
                    if let (Ok(id), true) = (u32::try_from(shot.muzzle_effect), shot.kind != projectile::STRIKE) {
                        if id > 0 {
                            let at = projectile::muzzle(&shot, node);
                            self.horde.effects_asked.push(Asked { id, by: None, at, fixed: false, of: None, list: List::Scene });
                        }
                    }
                    let frame = self.horde.steps;
                    match shot.kind {
                        projectile::STRAIGHT | projectile::GUIDED => {
                            projectile::spawn(self.commands, self.look, Projectile::new(shot, entity, node, aim), frame);
                        }
                        // A ray strikes on the frame it is fired, once every enemy has moved.
                        projectile::RAY => self.horde.rays.push(Ray::new(shot, entity, node, aim)),
                        projectile::BEAM => projectile::spawn_beam(self.commands, self.look, Beam::new(shot, entity, node, aim), frame),
                        projectile::BOMB => projectile::spawn_bomb(self.commands, self.look, Bomb::new(shot, entity, node, aim), frame),
                        // A strike at the spot under what it aims at: its look is
                        // the battle's own from the first, at that spot (0x8B028).
                        projectile::STRIKE => {
                            let strike = Strike::new(shot, entity, aim);
                            if let Some(id) = u32::try_from(shot.effect).ok().filter(|id| *id > 0) {
                                self.horde.effects_asked.push(Asked { id, by: None, at: strike.frame(), fixed: false, of: None, list: List::Scene });
                            }
                            projectile::spawn_strike(self.commands, strike, frame);
                        }
                        kind => {
                            if self.horde.unported_shots.insert(kind) {
                                warn!("{}: bullet {} is of class {kind}, which is not ported", enemy.archive.folder, shot.id);
                            }
                        }
                    }
                }
                Event::Reinforce { index: number, reserve, place } => {
                    // One of the group's reserves comes in (the stage's part is 0xA0E50).
                    let (wave, archive) = (self.group.reserves[reserve].wave, self.group.reserve_archives[reserve].clone());
                    let position = Vec3::from(place.position);
                    let spawn = Spawn { index: number, variant: wave.variant as i32, position, wave: Some(wave.entry as i32) };
                    match self.enlist(player, spawn, &archive, None) {
                        Ok(()) => info!("enemy {index} calls in enemy {number}: {} at {position:.0}", archive.folder),
                        Err(error) => {
                            error!("enemy {number} could not come in: {error:#}");
                            let peer = &mut self.horde.shared.peers[number];
                            (peer.alive, peer.in_the_way) = (false, false);
                        }
                    }
                }
                // Status effects for others wait until its turn is over.
                Event::Status { id, give } => {
                    let giver = enemy.actor.body.giver();
                    self.horde.gifts.push(Gift { from: index, giver, id, give, to: Whom::Player });
                }
                Event::GroupStatus { id, give, itself } => {
                    let giver = enemy.actor.body.giver();
                    self.horde.gifts.push(Gift { from: index, giver, id, give, to: Whom::Enemies { itself } });
                }
                Event::OwnStatus { id, outcome } => info!("enemy {index} takes status effect {id}: {outcome:?}"),
                Event::Sound { handle, bank, id, volume, note } => {
                    if let (Some(bank), Ok(sound)) = (bank_of(bank, &enemy.archive), u32::try_from(id)) {
                        self.board.ask(Request::Effect { handle, bank: bank.to_owned(), sound, volume, note });
                    }
                }
                Event::StopSound(handle) => {
                    if let Ok(handle) = u32::try_from(handle) {
                        self.board.ask(Request::StopEffect(handle));
                    }
                }
                // What a stream is playing stops at once for it (0xA7EAC).
                Event::Voice { stream, name } => match (usize::try_from(stream), enemy.archive.voice(&name)) {
                    (Ok(stream), Some(file)) => {
                        if let Some(said) = self.horde.said.get_mut(stream) {
                            *said = true;
                        }
                        self.board.ask_all([Request::Hush(stream, 0), Request::Say(stream, file)]);
                    }
                    _ => debug!("enemy {index}: there is no recording {name} for stream {stream}"),
                },
                // A line of the log, with its name for all the line's names
                // (0xBE418), or that of a part of it (0x85CBC).
                Event::Log(line) => {
                    let names = log::Names::of(&enemy.actor.body.variant.name);
                    self.horde.lines.push(log::Line { message: line, names });
                }
                Event::Beaten { broken } => self.horde.beaten.push(records::Event::kill(i32::from(enemy.actor.body.enemy), broken, self.skill)),
                Event::PartLog { line, gauge } => {
                    let names = log::Names::of(enemy.actor.body.variant.part_name(gauge));
                    self.horde.lines.push(log::Line { message: line, names });
                }
                // An effect by a node of one of its models (0xBD320); with no such
                // model it is by nothing.
                Event::Effect { id, slot, node, offset, turn, fixed, on_top } => {
                    let model = usize::try_from(slot).ok().and_then(|slot| enemy.models.get(slot).copied().flatten());
                    if let Ok(id) = u32::try_from(id) {
                        let by = model.map(|model| (model, u8::try_from(node).ok()));
                        let at = Mat4::from_rotation_translation(turn, offset);
                        self.horde.effects_asked.push(Asked { id, by, at, fixed, of: Some(entity), list: List::last_if(on_top) });
                    }
                }
                Event::Number { value, at } => self.horde.numbers.push(numbers::Asked { kind: numbers::Kind::Damage, value, at, hers: false }),
                // What is seen of a hit is the battle's own (0x909AC).
                Event::Burst { id, at, on_top } => self.horde.effects_asked.push(Asked::at(id, at, on_top)),
                Event::StopEffect(id) => {
                    if let Ok(id) = u32::try_from(id) {
                        self.horde.effects_ended.push(id);
                    }
                }
                // All it has, playing or asked for until now (0xBF1EC); what it
                // asks for after this is its own again.
                Event::StopEffects => {
                    self.horde.effects_asked.retain(|asked| asked.of != Some(entity));
                    self.horde.effects_dropped.push(entity);
                }
                Event::Shake { kind, degrees, frames } => self.horde.jolts.push(shake::Asked::Script { kind, degrees, frames }),
                Event::HitShake(word) => self.horde.jolts.push(shake::Asked::Attack(word)),
                // What a boss's script asks of the battle as a whole is the
                // battle's to do, with what it names looked up in its archives.
                Event::Scene { camera, slot, node } => {
                    let clip = enemy.archive.camera(&camera).cloned();
                    if clip.is_none() {
                        warn!("{}: there is no camera move {camera}", enemy.archive.folder);
                    }
                    let model = usize::try_from(slot).ok().and_then(|slot| enemy.models.get(slot).copied().flatten());
                    info!("enemy {index} begins a scene with {camera}");
                    self.horde.directions.push(Direction::Scene { camera: clip, model, node: u8::try_from(node).ok() });
                }
                Event::SceneOver => self.horde.directions.push(Direction::SceneOver),
                Event::HerAnimation { name, looping, speed } => match enemy.archive.clip(&name) {
                    Some(clip) => self.horde.directions.push(Direction::HerAnimation { clip: clip.clone(), looping, speed }),
                    None => warn!("{}: there is no animation {name} for her", enemy.archive.folder),
                },
                Event::HerMoved(by) => self.horde.directions.push(Direction::HerMoved(by)),
                Event::HerTurned(angle) => self.horde.directions.push(Direction::HerTurned(angle)),
                Event::HerShown(shown) => self.horde.directions.push(Direction::HerShown(shown)),
                Event::HerParts(mode) => self.horde.directions.push(Direction::HerParts(mode)),
                Event::Jaw { hers, angle } => {
                    let model = if hers { None } else { enemy.models[0] };
                    if hers || model.is_some() {
                        self.horde.directions.push(Direction::Jaw { model, angle });
                    }
                }
                Event::Lens { list, near, far } => self.horde.directions.push(Direction::Lens { list, near, far }),
                // Nothing puts back what the cameras were told (0xA7CE0).
                Event::LensBack => {}
                Event::Stage(taken) => {
                    info!("enemy {index} {} the stage", if taken { "takes" } else { "gives back" });
                    self.horde.directions.push(Direction::Stage(taken));
                }
                Event::Dim { to, time } => self.horde.directions.push(Direction::Dim { by: index, to, time }),
                Event::CutIn(what) => {
                    let pack = match &what {
                        actor::CutIn::Load(name) => enemy.archive.layout(name).map(str::to_owned),
                        _ => None,
                    };
                    self.horde.directions.push(Direction::CutIn(what, pack));
                }
                Event::Finish(record) => {
                    let pack = enemy.archive.layout(super::boss::finish_layout()).map(str::to_owned);
                    self.horde.directions.push(Direction::Finish(record, pack));
                }
                Event::Sheet(colour) => self.horde.directions.push(Direction::Sheet(colour)),
                Event::SheetGoes(time) => self.horde.directions.push(Direction::SheetGoes(time)),
                Event::Go => self.horde.directions.push(Direction::Go),
                Event::Fight => {
                    info!("enemy {index} has the fight begin");
                    self.horde.directions.push(Direction::Fight);
                }
                Event::Display(on) => self.horde.directions.push(Direction::Display(on)),
                Event::Hush => self.horde.directions.push(Direction::Hush),
                Event::NoBullets => self.horde.directions.push(Direction::NoBullets),
                Event::MusicOff(time) => self.horde.directions.push(Direction::MusicOff(time)),
                Event::Dismiss => self.horde.directions.push(Direction::Dismiss(index)),
                Event::Parts(mode) => {
                    if let Some(model) = enemy.models[0] {
                        self.horde.directions.push(Direction::Parts { model, changes: actor::part_mode(mode) });
                    }
                }
                Event::Revive { think } => self.horde.revivals.push(think),
                other => debug!("enemy {index}: {other:?}"),
            }
        }
    }

    /// Sends every enemy a signal.
    fn send_all(&mut self, number: i32, player: PlayerView, enemies: &mut Query<(Entity, &mut Enemy)>, models: &mut ModelQuery) {
        for (index, entity) in in_order(enemies, |_| true) {
            let Ok((_, mut enemy)) = enemies.get_mut(entity) else { continue };
            let enemy = &mut *enemy;
            if enemy.actor.body.remove {
                continue;
            }
            let mut events = Vec::new();
            let mut spawned = Spawned { entities: &enemy.models, query: models };
            let mut world = self.surroundings(player, &enemy.archive, &mut spawned, &mut events);
            if let Err(error) = enemy.actor.send(number, &mut world) {
                error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                enemy.actor.body.remove = true;
            }
            self.carry_out(player, entity, enemy, events);
        }
    }
}

/// The enemies that `wanted` picks out, in the order they joined the battle, which
/// is the order they take their turns in.
fn in_order(enemies: &Query<(Entity, &mut Enemy)>, wanted: impl Fn(&Enemy) -> bool) -> Vec<(usize, Entity)> {
    let chosen = enemies.iter().filter(|(_, enemy)| wanted(enemy));
    let mut order: Vec<(usize, Entity)> = chosen.map(|(entity, enemy)| (enemy.actor.body.index, entity)).collect();
    order.sort();
    order
}

/// Makes the battle's enemies (the stage's part of this is at 0xA0820): each member's
/// actor is built and its script sets it up, which loads its model.
#[allow(clippy::too_many_arguments)]
pub fn spawn_group(
    commands: &mut Commands,
    group: &Group,
    shots: &Shots,
    effects: &Effects,
    stage: &Stage,
    player: &PcActor,
    half_experience: bool,
    random: &mut Random,
    stores: &mut AssetStores,
    look: &ProjectileLook,
    board: &mut Board,
) -> Result<Horde> {
    let peers = group.members.iter().map(|(member, _)| Peer {
        token: 0,
        alive: true,
        hp_percent: 100,
        home: Vec3::from(member.position),
        in_the_way: true,
    });
    let mut horde = Horde {
        group: group.id as i32,
        shared: Shared {
            peers: peers.collect(),
            reserves: Reserves::default(),
            spoils: Spoils { halved: half_experience, ..default() },
            sounds: 0,
            told: Told::default(),
        },
        left: group.members.len(),
        ..default()
    };
    let mut turn = Turn { commands, group, shots, effects, stage, look, decided: false, horde: &mut horde, random, stores, board, skill: 0 };
    for (index, (member, archive)) in group.members.iter().enumerate() {
        let spawn = Spawn { index, variant: member.variant as i32, position: Vec3::from(member.position), wave: None };
        turn.enlist(view_of(player, None), spawn, archive, None)?;
    }
    // The stage counts its towers and its reserves once everybody is there (0xA08E8).
    let families: Vec<i32> = group.members.iter().map(|(_, archive)| archive.family).collect();
    horde.shared.reserves = Reserves::new(&group.plan(), &families);
    horde.to_beat = horde.shared.reserves.to_beat(horde.left);
    Ok(horde)
}

impl Horde {
    /// The logic frame the enemies run next. While her own frame is being run that
    /// is the one they are about to run: what she fires then is fired on it.
    pub fn next_frame(&self) -> u32 {
        self.steps + 1
    }

    /// Nobody but her has a frame: she has the stage.
    pub fn frozen(&self) -> bool {
        self.frozen
    }

    /// An enemy is out of sight for a moment of hers, and what is its own with
    /// it.
    pub fn out_of_sight(&self, enemy: Entity) -> bool {
        self.hidden && self.alone != Some(enemy)
    }
}

/// Before anybody's turn in a logic frame: the enemies are told what the battle's
/// flow has for them, and are put away while the player has the stage.
#[allow(clippy::too_many_arguments)]
pub fn begin(
    mut commands: Commands,
    source: Res<BattleSource>,
    stage: Res<Stage>,
    look: Res<ProjectileLook>,
    mut flow: ResMut<Flow>,
    mut horde: ResMut<Horde>,
    mut random: ResMut<Random>,
    mut stores: AssetStores,
    mut board: ResMut<Board>,
    player: Single<(&PcActor, &Animator, &Skeleton), Without<EnemyModel>>,
    mut enemies: Query<(Entity, &mut Enemy)>,
    mut models: ModelQuery,
    shots: Query<Entity, Or<(With<Bullet>, With<Projectile>, With<Beam>, With<Bomb>, With<SkillShot>, With<Strike>)>>,
    seen: Query<&Visibility>,
) {
    let (player, player_animator, player_skeleton) = player.into_inner();
    let horde = &mut *horde;
    let mut turn = Turn {
        commands: &mut commands,
        group: &source.group,
        shots: &source.shots,
        effects: &source.effects,
        stage: &stage,
        look: &look,
        decided: flow.decided(),
        horde,
        random: &mut random,
        stores: &mut stores,
        board: &mut board,
        skill: 0,
    };

    // The enemies are told when the battle is over (0x91B58).
    for _ in 0..flow.take_announcements() {
        let view = view_of(player, Some((player_animator, player_skeleton)));
        turn.send_all(signal::OVER, view, &mut enemies, &mut models);
    }

    // While the player has the stage for a moment of her own, every other actor is
    // frozen and out of sight (the flags set at 0x7887C), and nothing can hit her.
    // While an enemy has it, every actor but that one is (0xBE208).
    let boss = turn.horde.shared.told.on_stage;
    let boss = boss.and_then(|index| enemies.iter().find(|(_, enemy)| enemy.actor.body.index == index)).map(|(entity, _)| entity);
    turn.horde.frozen = player.staged;
    if !player.staged {
        turn.horde.steps += 1;
    }
    // A held enemy's update is passed over whole (0x92048), its animations with
    // the rest: it stands as it stood.
    for (entity, enemy) in &enemies {
        let held = player.staged || boss.is_some_and(|boss| boss != entity);
        for model in enemy.models.iter().flatten() {
            if let Ok((mut animator, ..)) = models.get_mut(*model) {
                if animator.paused != held {
                    animator.paused = held;
                }
            }
        }
    }
    if let (false, Some(boss)) = (player.staged, boss) {
        // Everybody but the one that has the stage is out of sight, and all that
        // has been fired.
        if !std::mem::replace(&mut turn.horde.hidden, true) {
            turn.horde.alone = Some(boss);
            let others = enemies.iter().filter(|(entity, _)| *entity != boss).flat_map(|(_, enemy)| enemy.models.iter().flatten().copied());
            for entity in others.chain(shots.iter()).collect::<Vec<Entity>>() {
                if seen.get(entity).is_ok_and(|seen| *seen != Visibility::Hidden) {
                    turn.horde.unseen.push(entity);
                }
                turn.commands.entity(entity).insert(Visibility::Hidden);
            }
        }
    } else if !player.staged || player.shown == Shown::Everybody {
        // The stage given back (0x80C68), or the others shown before that for
        // what she does to them to be seen (0x746D8): what was put out of sight
        // is seen again.
        turn.horde.alone = None;
        if std::mem::take(&mut turn.horde.hidden) {
            for entity in std::mem::take(&mut turn.horde.unseen) {
                if let Ok(mut entity) = turn.commands.get_entity(entity) {
                    entity.insert(Visibility::Inherited);
                }
            }
        }
    } else if !std::mem::replace(&mut turn.horde.hidden, true) {
        let models = enemies.iter().flat_map(|(_, enemy)| enemy.models.iter().flatten().copied());
        for entity in models.chain(shots.iter()).collect::<Vec<Entity>>() {
            if seen.get(entity).is_ok_and(|seen| *seen != Visibility::Hidden) {
                turn.horde.unseen.push(entity);
            }
            turn.commands.entity(entity).insert(Visibility::Hidden);
        }
    } else if let Shown::Only(one) = player.shown {
        // The one enemy a skill of hers is for is seen again, and nobody else
        // (0x778C0).
        turn.horde.alone = Some(one);
        let its: Vec<Entity> = enemies.get(one).map(|(_, enemy)| enemy.models.iter().flatten().copied().collect()).unwrap_or_default();
        let unseen = &mut turn.horde.unseen;
        for entity in its {
            if let Some(index) = unseen.iter().position(|other| *other == entity) {
                unseen.swap_remove(index);
                turn.commands.entity(entity).insert(Visibility::Inherited);
            }
        }
    }
}

/// The enemies' turns in a logic frame (0x91F4C), in the order they joined the
/// battle. It runs twice: one that an enemy's script calls into the battle takes its
/// first turn in the same frame, after everybody who was there already, and exists
/// for the second run to find.
#[allow(clippy::too_many_arguments)]
pub fn think(
    mut commands: Commands,
    source: Res<BattleSource>,
    stage: Res<Stage>,
    look: Res<ProjectileLook>,
    flow: Res<Flow>,
    mut horde: ResMut<Horde>,
    mut random: ResMut<Random>,
    mut stores: AssetStores,
    mut board: ResMut<Board>,
    player: Single<(Entity, &mut PcActor, &Animator, &Skeleton), Without<EnemyModel>>,
    mut enemies: Query<(Entity, &mut Enemy)>,
    mut models: ModelQuery,
) {
    if horde.frozen {
        return;
    }
    let steps = horde.steps;
    // While one of them has the stage only that one has a frame.
    let on_stage = horde.shared.told.on_stage;
    let order = in_order(&enemies, |enemy| enemy.stepped != steps && on_stage.is_none_or(|index| index == enemy.actor.body.index));
    if order.is_empty() {
        return;
    }
    let (her, mut player, player_animator, player_skeleton) = player.into_inner();
    let horde = &mut *horde;
    let mut turn = Turn {
        commands: &mut commands,
        group: &source.group,
        shots: &source.shots,
        effects: &source.effects,
        stage: &stage,
        look: &look,
        decided: flow.decided(),
        horde,
        random: &mut random,
        stores: &mut stores,
        board: &mut board,
        skill: 0,
    };

    // Where the player's body can be hit: a sphere round a node of her model.
    let nodes = Some((player_animator, player_skeleton));
    let player_body = player_skeleton
        .index_of(PLAYER_BODY_NODE)
        .map(|node| (player.transform() * player_animator.model_space(player_skeleton, node)).translation);

    let mut player_hit = false;
    for (index, entity) in order {
        {
            let Ok((_, mut enemy)) = enemies.get_mut(entity) else { continue };
            let enemy = &mut *enemy;
            enemy.stepped = steps;
            if flow.thinking() {
                enemy.actor.body.thinking = true;
            }

            let mut events = Vec::new();
            let mut spawned = Spawned { entities: &enemy.models, query: &mut models };
            let mut world = turn.surroundings(view_of(&player, nodes), &enemy.archive, &mut spawned, &mut events);
            if let Err(error) = enemy.actor.update(&mut world) {
                // A script the port cannot run is taken out rather than left half alive.
                error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                enemy.actor.body.remove = true;
            }

            // Its attack spheres against the player's body: the first that touches
            // hits, and is switched off.
            let body = &mut enemy.actor.body;
            let stunned = body.gauges[0].is_some_and(|gauge| gauge.broken);
            if let (Some(player_body), false) = (player_body, body.remove || stunned) {
                let touching = body.spheres().filter(|(_, sphere)| sphere.enabled && sphere.attack).find_map(|(slot, sphere)| {
                    let centre = body.sphere_centre(slot, &spawned)?;
                    (centre.distance(player_body) <= sphere.radius + PLAYER_BODY_RADIUS).then_some((slot, sphere.attack_id, centre))
                });
                if let Some((slot, attack, centre)) = touching {
                    // It lands on the side of her that the sphere is on (0x923E0).
                    let at = player_body + (centre - player_body).normalize_or_zero() * PLAYER_BODY_RADIUS;
                    if let Some(actor::Slot::Sphere(sphere)) = &mut body.slots[slot] {
                        sphere.enabled = false;
                    }
                    match source.attacks.of_enemy(attack, body.enemy as i32, body.variant.id) {
                        Some(attack) => {
                            let by = body.assailant();
                            let outcome = player.take_hit(attack, by, &source.stats, &source.effects, &source.tunables, turn.random);
                            // What the attack drains comes back to the enemy as HP.
                            body.heal(outcome.drained);
                            info!("enemy {index} hits the player with attack {}: {outcome:?}", attack.id);
                            player.show_hit(&outcome, at);
                            let landing = Landing { attack, outcome: &outcome, at, her };
                            tell_hit(turn.board, &mut turn.horde.effects_asked, landing, &source.effects, &enemy.archive);
                            player_hit = true;
                        }
                        None => warn!("{}: there is no attack {attack} for this enemy", enemy.archive.folder),
                    }
                }
            }

            turn.note(enemy);
            turn.carry_out(view_of(&player, nodes), entity, enemy, events);
            place_models(enemy, &mut models);

            let trace = enemy.actor.trace();
            if trace != enemy.traced {
                let token = turn.horde.shared.peers[index].token;
                debug!("enemy {index} step {steps}: {trace} token {token} at {:.0}", enemy.actor.body.mover.position);
                enemy.traced = trace;
            }
        }
        // What its script handed to the player or to the others reaches them now,
        // before the next one's turn.
        turn.deliver(&mut player, &source.stats, &mut enemies);
    }

    // The dying are made again where they belong, for whoever asked (0x91830):
    // each is a new enemy of the same kind and variant, and the old one goes.
    for think in std::mem::take(&mut turn.horde.revivals) {
        let dying: Vec<(Entity, Arc<Archive>, Spawn)> = enemies
            .iter()
            .filter(|(_, enemy)| enemy.actor.body.dying && !enemy.actor.body.remove)
            .map(|(entity, enemy)| {
                let body = &enemy.actor.body;
                let spawn = Spawn { index: 0, variant: body.variant.id, position: body.home, wave: (!body.rewards).then_some(body.entry) };
                (entity, enemy.archive.clone(), spawn)
            })
            .collect();
        for (entity, archive, mut spawn) in dying {
            if let Ok((_, mut old)) = enemies.get_mut(entity) {
                old.actor.body.remove = true;
            }
            spawn.index = turn.horde.shared.peers.len();
            turn.horde.shared.peers.push(Peer { token: 0, alive: true, hp_percent: 100, home: spawn.position, in_the_way: true });
            match turn.enlist(view_of(&player, nodes), spawn, &archive, Some(think)) {
                Ok(()) => info!("enemy {} is made again as enemy {}: {} at {:.0}", archive.folder, spawn.index, archive.folder, spawn.position),
                Err(error) => {
                    error!("enemy {} could not be made again: {error:#}", spawn.index);
                    let peer = &mut turn.horde.shared.peers[spawn.index];
                    (peer.alive, peer.in_the_way) = (false, false);
                }
            }
        }
    }

    // Every enemy is told when the player has been hit (the start of 0x9AD90).
    if player_hit {
        turn.send_all(signal::PLAYER_HIT, view_of(&player, nodes), &mut enemies, &mut models);
    }
}

/// How far either way, across and up and down, what is seen of a ray's hit is from
/// where the ray meets what it hits: one of so many places (0x8808C).
const RAY_HIT_SPREAD: i32 = 200;
/// The effect that is seen where a ray of hers ends when it has met nothing
/// (0x8820C).
const EFFECT_RAY_ENDS: u32 = 998_021;

/// The rest of the enemies' logic frame, once each has had its turn: the player's
/// shots (0x8B310) and their own (0x886F8, 0x87F8C, 0x89C00), those who are gone,
/// and the stage's count of who is left (0xA05F0).
#[allow(clippy::too_many_arguments)]
pub fn settle(
    mut commands: Commands,
    source: Res<BattleSource>,
    stage: Res<Stage>,
    look: Res<ProjectileLook>,
    (mut flow, mut strikes, mut rays): (ResMut<Flow>, ResMut<Strikes>, ResMut<SkillRays>),
    mut horde: ResMut<Horde>,
    mut random: ResMut<Random>,
    mut stores: AssetStores,
    mut board: ResMut<Board>,
    player: Single<(Entity, &mut PcActor, &Animator, &Skeleton), Without<EnemyModel>>,
    mut enemies: Query<(Entity, &mut Enemy)>,
    mut models: ModelQuery,
    (mut bullets, mut skill_shots, mut ground_strikes): (
        Query<(Entity, &mut Bullet, &mut Stepped, &mut Visibility, &Fired), (Without<EnemyModel>, Without<Projectile>, Without<Beam>, Without<SkillShot>)>,
        Query<(Entity, &mut SkillShot, &mut Stepped, &Fired), (Without<EnemyModel>, Without<Bullet>, Without<Projectile>, Without<Bomb>)>,
        Query<(Entity, &mut Strike, &Fired)>,
    ),
    mut projectiles: Query<(Entity, &mut Projectile, &mut Stepped, &Fired), (Without<EnemyModel>, Without<Bullet>, Without<SkillShot>)>,
    mut beams: Query<(Entity, &mut Beam, &mut Visibility, &Fired, Option<&mut Look>), Without<Bullet>>,
    mut bombs: Query<
        (Entity, &mut Bomb, &mut Stepped, &mut Visibility, &Fired, Option<&mut Look>),
        (Without<EnemyModel>, Without<Bullet>, Without<Projectile>, Without<Beam>, Without<SkillShot>),
    >,
) {
    let dt = SIXTIETHS_PER_STEP;
    let (her, mut player, player_animator, player_skeleton) = player.into_inner();
    let horde = &mut *horde;
    let mut turn = Turn {
        commands: &mut commands,
        group: &source.group,
        shots: &source.shots,
        effects: &source.effects,
        stage: &stage,
        look: &look,
        decided: flow.decided(),
        horde,
        random: &mut random,
        stores: &mut stores,
        board: &mut board,
        skill: 0,
    };
    let order = in_order(&enemies, |_| true);
    let nodes = Some((player_animator, player_skeleton));
    let player_body = player_skeleton
        .index_of(PLAYER_BODY_NODE)
        .map(|node| (player.transform() * player_animator.model_space(player_skeleton, node)).translation);

    // What a skill of hers strikes the enemies with outright (0x74EF0, 0x847C0),
    // held as they may be: each that has a body sphere in use takes her attack
    // of the skill's record once, at the first of its spheres. One that is for
    // the enemy she had locked on to strikes that one alone (0x778F8), and a
    // snipe the part of it her lock point was on (0x7F7B4).
    for (skill, blow) in std::mem::take(&mut strikes.0) {
        let Some(attack) = source.stagings.get(skill).and_then(|staging| staging.strike).and_then(|id| source.attacks.of_player(id)) else {
            warn!("skill {skill} has no attack of hers to strike with");
            continue;
        };
        // Whom it lands on: each at the first of its body spheres and wearing its
        // first gauge, but a snipe's at the sphere its lock point is on, wearing
        // the gauge that sphere has (0x91A5C). Where it lands, for what is seen
        // of it: at the middle of that sphere (0x74FA4), no higher than a
        // blade's hit is seen for the one that is for one enemy (0x779C0); a
        // snipe's at her lock point (0x7F7D8).
        let mut landing: Vec<(Entity, usize, Option<usize>, Vec3)> = Vec::new();
        match blow {
            Blow::Part(enemy, gauge) => {
                let slot = enemies.get(enemy).ok().and_then(|(_, enemy)| usize::try_from(enemy.actor.body.gauges.get(gauge)?.as_ref()?.slot).ok());
                let at = turn.horde.lock_points.iter().find(|lock| lock.enemy == enemy && lock.gauge == gauge).map_or(Vec3::ZERO, |lock| lock.at);
                landing.extend(slot.map(|slot| (enemy, slot, None, at)));
            }
            Blow::All | Blow::One(_) => {
                let top = source.tunables.int(super::tunables::id::BLADE_HIT_TOP) as f32;
                for body in body_spheres(&order, &enemies, &models) {
                    let wanted = blow == Blow::All || blow == Blow::One(body.enemy);
                    if wanted && !landing.iter().any(|landed| landed.0 == body.enemy) {
                        let at = if blow == Blow::All { body.centre } else { body.centre.with_y(body.centre.y.min(top)) };
                        landing.push((body.enemy, body.slot, Some(0), at));
                    }
                }
            }
        }
        turn.skill = skill;
        for (struck, slot, gauge, at) in landing {
            let body = BodySphere { enemy: struck, slot, centre: Vec3::ZERO, radius: 0.0 };
            let Ok((_, mut enemy)) = enemies.get_mut(body.enemy) else { continue };
            let enemy = &mut *enemy;
            let index = enemy.actor.body.index;
            let gauge = gauge.unwrap_or_else(|| enemy.actor.body.gauge_of(slot));
            let mut events = Vec::new();
            let mut spawned = Spawned { entities: &enemy.models, query: &mut models };
            let view = view_of(&player, nodes);
            let mut world = turn.surroundings(view, &enemy.archive, &mut spawned, &mut events);
            match enemy.actor.hit(body.slot, at, gauge, attack, player_assailant(&source.stats, &player.status), &mut world) {
                Ok((dealt, drained)) => {
                    player.heal(drained);
                    info!("skill {skill} strikes enemy {index} for {dealt} with attack {}: {:.0} HP left", attack.id, enemy.actor.body.hp);
                }
                Err(error) => {
                    error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                    enemy.actor.body.remove = true;
                }
            }
            turn.note(enemy);
            turn.carry_out(view, body.enemy, enemy, events);
        }
        turn.skill = 0;
        turn.deliver(&mut player, &source.stats, &mut enemies);
    }
    // Nor does anything that has been fired have a frame while an enemy has the
    // stage.
    if turn.horde.frozen || turn.horde.has_the_stage() {
        return;
    }

    // The player's shots: each moves, and the first body sphere on its way is hit.
    // One fired this frame does not move yet (0x8B374), and hits only what it
    // is already in.
    let now = Fired(turn.horde.steps);
    let bodies = body_spheres(&order, &enemies, &models);
    for (bullet_entity, mut bullet, mut stepped, mut visibility, fired) in &mut bullets {
        let from = stepped.current.translation;
        if *fired != now && !bullet.advance(dt, &mut stepped, &mut visibility) {
            turn.commands.entity(bullet_entity).despawn();
            continue;
        }
        let to = stepped.current.translation;
        let nearest = bodies
            .iter()
            .filter_map(|body| Some((bullet::sweep(from, to, bullet::RADIUS, body.centre, body.radius)?, body)))
            .min_by(|a, b| a.0.0.total_cmp(&b.0.0));
        // A shot that has reached the ground is spent, whatever else it met.
        if from.y <= 0.0 || to.y <= 0.0 {
            turn.commands.entity(bullet_entity).despawn();
            continue;
        }
        let Some(((_, at), body)) = nearest else { continue };
        turn.commands.entity(bullet_entity).despawn();
        let (Ok((_, mut enemy)), Some(attack)) = (enemies.get_mut(body.enemy), source.attacks.of_player(SHOT_ATTACK)) else {
            continue;
        };
        let enemy = &mut *enemy;
        let index = enemy.actor.body.index;
        let mut events = Vec::new();
        let mut spawned = Spawned { entities: &enemy.models, query: &mut models };
        let view = view_of(&player, nodes);
        let mut world = turn.surroundings(view, &enemy.archive, &mut spawned, &mut events);
        match enemy.actor.hit(body.slot, at, 0, attack, player_assailant(&source.stats, &player.status), &mut world) {
            Ok((dealt, drained)) => {
                player.heal(drained);
                info!("shot hits enemy {index} for {dealt}: {:.0} HP left", enemy.actor.body.hp);
            }
            Err(error) => {
                error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                enemy.actor.body.remove = true;
            }
        }
        turn.note(enemy);
        turn.carry_out(view, body.enemy, enemy, events);
    }
    turn.deliver(&mut player, &source.stats, &mut enemies);

    // The bullets of her skills (0x893E0): each hits the enemy whose body it first
    // sweeps through, of those it has not gone through, as her attack of the
    // bullet's record; one that pierces goes on. One fired this frame only appears.
    for (shot_entity, mut flying, mut stepped, fired) in &mut skill_shots {
        if *fired == now {
            continue;
        }
        // What it was fired at, as the enemies' turns have just left it (0xA159C):
        // the lock point is there while its enemy is, and its gauge is in use.
        let lock = match flying.lock().and_then(|(entity, gauge)| Some((enemies.get(entity).ok()?.1, gauge))) {
            None => Lock::Gone,
            Some((enemy, gauge)) => {
                let body = &enemy.actor.body;
                let view = Seen { entities: &enemy.models, query: &models };
                let gauge = body.gauges.get(gauge).copied().flatten().filter(|gauge| gauge.active && !body.remove);
                match gauge.and_then(|gauge| body.place_of(gauge.slot, gauge.node, gauge.offset, &view)) {
                    None => Lock::Gone,
                    Some(_) if body.hp <= 0.0 => Lock::Dead,
                    Some(at) => Lock::At(at),
                }
            }
        };
        if !flying.advance(dt, lock) {
            turn.commands.entity(shot_entity).despawn();
            continue;
        }
        stepped.current = flying.transform();
        let ((from, to), shot) = (flying.flown(), *flying.shot());
        let nearest = bodies
            .iter()
            .filter(|body| !flying.struck.contains(&body.enemy))
            .filter_map(|body| Some((bullet::sweep(from, to, shot.radius, body.centre, body.radius)?, body)))
            .min_by(|a, b| a.0.0.total_cmp(&b.0.0));
        let Some(((_, at), body)) = nearest else { continue };
        if shot.pierces {
            flying.struck.push(body.enemy);
        } else {
            turn.commands.entity(shot_entity).despawn();
        }
        let (Ok((_, mut enemy)), Some(attack)) = (enemies.get_mut(body.enemy), source.attacks.of_player(shot.attack)) else {
            warn!("skill {}: there is no attack {} of hers, or its enemy is gone", flying.skill, shot.attack);
            continue;
        };
        let enemy = &mut *enemy;
        let index = enemy.actor.body.index;
        let mut events = Vec::new();
        let mut spawned = Spawned { entities: &enemy.models, query: &mut models };
        let view = view_of(&player, nodes);
        let mut world = turn.surroundings(view, &enemy.archive, &mut spawned, &mut events);
        match enemy.actor.hit(body.slot, at, 0, attack, player_assailant(&source.stats, &player.status), &mut world) {
            Ok((dealt, drained)) => {
                player.heal(drained);
                info!("skill {} hits enemy {index} for {dealt} with attack {}: {:.0} HP left", flying.skill, attack.id, enemy.actor.body.hp);
            }
            Err(error) => {
                error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                enemy.actor.body.remove = true;
            }
        }
        turn.note(enemy);
        turn.skill = flying.skill;
        turn.carry_out(view, body.enemy, enemy, events);
        turn.skill = 0;
    }
    turn.deliver(&mut player, &source.stats, &mut enemies);

    // The rays of her skills (0x88008): each strikes at once, and of the enemies'
    // bodies on its line the one it meets first, as her attack of the bullet's
    // record. (A bullet that can be shot down would be met in the same way, and
    // taken away: only bosses fire any.)
    for (skill, ray) in std::mem::take(&mut rays.0) {
        let nearest = bodies
            .iter()
            .filter_map(|body| {
                let met = ray.meets(body.centre, body.radius)?;
                Some((ray.reach(met), met, body))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, met, body)) = nearest else {
            // One that meets nothing is seen where it ends, up to a hundred off
            // each way (0x88154).
            let mut off = || (turn.random.below(RAY_HIT_SPREAD) - RAY_HIT_SPREAD / 2) as f32;
            let at = ray.end() + Vec3::new(off(), off(), off());
            turn.horde.effects_asked.push(Asked::at(EFFECT_RAY_ENDS, at, false));
            continue;
        };
        // What is seen of it is not just where the ray meets it: up to a hundred
        // off across, and up or down (0x88088).
        let across = (turn.random.below(RAY_HIT_SPREAD) - RAY_HIT_SPREAD / 2) as f32;
        let up = (turn.random.below(RAY_HIT_SPREAD) - RAY_HIT_SPREAD / 2) as f32;
        let at = met + Vec3::new(across, up, 0.0);
        let (Ok((_, mut enemy)), Some(attack)) = (enemies.get_mut(body.enemy), source.attacks.of_player(ray.shot.attack)) else {
            warn!("skill {skill}: there is no attack {} of hers, or its enemy is gone", ray.shot.attack);
            continue;
        };
        let enemy = &mut *enemy;
        let index = enemy.actor.body.index;
        let mut events = Vec::new();
        let mut spawned = Spawned { entities: &enemy.models, query: &mut models };
        let view = view_of(&player, nodes);
        let mut world = turn.surroundings(view, &enemy.archive, &mut spawned, &mut events);
        match enemy.actor.hit(body.slot, at, 0, attack, player_assailant(&source.stats, &player.status), &mut world) {
            Ok((dealt, drained)) => {
                player.heal(drained);
                info!("a ray of skill {skill} hits enemy {index} for {dealt} with attack {}: {:.0} HP left", attack.id, enemy.actor.body.hp);
            }
            Err(error) => {
                error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                enemy.actor.body.remove = true;
            }
        }
        turn.note(enemy);
        turn.skill = skill;
        turn.carry_out(view, body.enemy, enemy, events);
        turn.skill = 0;
    }
    turn.deliver(&mut player, &source.stats, &mut enemies);

    // The enemies' bullets. Whatever its class, one that reaches the player's body
    // hits her as an attack of the enemy that fired it, and with that enemy gone it
    // can hit no more (0x897A4, 0x882F8, 0x89FC4).
    let mut player_shot = false;
    let mut lands = |shot: &Shot, owner: Entity, at: Vec3, player: &mut PcActor, random: &mut Random, board: &mut Board, asked: &mut Vec<Asked>| {
        let Ok((_, mut owner)) = enemies.get_mut(owner) else { return };
        let owner = &mut *owner;
        let body = &mut owner.actor.body;
        match source.attacks.of_enemy(shot.attack, body.enemy as i32, body.variant.id) {
            Some(attack) => {
                let outcome = player.take_hit(attack, body.assailant(), &source.stats, &source.effects, &source.tunables, random);
                // What the attack drains comes back to the enemy as HP.
                body.heal(outcome.drained);
                info!("bullet {} of enemy {} hits the player with attack {}: {outcome:?}", shot.id, body.index, attack.id);
                player.show_hit(&outcome, at);
                tell_hit(board, asked, Landing { attack, outcome: &outcome, at, her }, &source.effects, &owner.archive);
                player_shot = true;
            }
            None => warn!("{}: there is no attack {} for this enemy", owner.archive.folder, shot.attack),
        }
    };

    // A bullet fired this frame only appears (see `Fired`); the others have their
    // frame. Those that fly: each goes on its way, and hits if it passes through her.
    let place = stage.slot(player.slot);
    for (bullet_entity, mut projectile, mut stepped, fired) in &mut projectiles {
        if *fired == now {
            continue;
        }
        if !projectile.advance(dt, place) {
            turn.commands.entity(bullet_entity).despawn();
            continue;
        }
        stepped.current = projectile.transform();
        if let Some(at) = player_body.and_then(|centre| projectile.touches(centre, PLAYER_BODY_RADIUS)) {
            turn.commands.entity(bullet_entity).despawn();
            lands(projectile.shot(), projectile.owner, at, &mut player, turn.random, turn.board, &mut turn.horde.effects_asked);
        }
    }
    // Rays strike at once along their line.
    for ray in std::mem::take(&mut turn.horde.rays) {
        if let Some(met) = player_body.and_then(|centre| ray.meets(centre, PLAYER_BODY_RADIUS)) {
            // What is seen of it is up to fifty off where the ray meets her,
            // across and up or down (0x88288).
            let across = (turn.random.below(RAY_HIT_SPREAD / 2) - RAY_HIT_SPREAD / 4) as f32;
            let up = (turn.random.below(RAY_HIT_SPREAD / 2) - RAY_HIT_SPREAD / 4) as f32;
            let at = met + Vec3::new(across, up, 0.0);
            lands(&ray.shot, ray.owner, at, &mut player, turn.random, turn.board, &mut turn.horde.effects_asked);
        }
    }
    // Beams stay, and strike along theirs every so often.
    for (beam_entity, mut beam, mut visibility, fired, its_look) in &mut beams {
        if *fired == now {
            continue;
        }
        let (strikes, there) = beam.advance(dt);
        if let Some(at) = player_body.filter(|_| strikes).and_then(|centre| beam.ray.meets(centre, PLAYER_BODY_RADIUS)) {
            lands(&beam.ray.shot, beam.ray.owner, at, &mut player, turn.random, turn.board, &mut turn.horde.effects_asked);
        }
        // When its time is up its effect is told to die away (0x89F24), and
        // what stands in for one goes.
        if !there {
            turn.commands.entity(beam_entity).despawn();
        } else if !beam.shown() {
            match its_look {
                Some(mut look) => look.end(),
                None => _ = visibility.set_if_neq(Visibility::Hidden),
            }
        }
    }
    // A strike lands when its time has come, on her if she is within its reach of
    // its spot, and is gone (0x8B0B4).
    for (strike_entity, mut strike, fired) in &mut ground_strikes {
        if *fired == now || !strike.advance(dt) {
            continue;
        }
        if let Some(at) = player_body.and_then(|centre| strike.reaches(centre, PLAYER_BODY_RADIUS)) {
            lands(&strike.shot, strike.owner, at, &mut player, turn.random, turn.board, &mut turn.horde.effects_asked);
        }
        turn.commands.entity(strike_entity).despawn();
    }
    // Bombs hop to the ground, lie there and burst, and the burst strikes her
    // whenever it is due while she is within its reach (0x8A284); its first
    // landing and its burst shake the camera (0x8A7A8, 0x8A9D4). A bomb of hers
    // does the same, and its burst strikes the enemies.
    let mut bomb_sounds: Vec<(Entity, i32, u32)> = Vec::new();
    let mut her_bursts: Vec<(i32, Shot, Vec3)> = Vec::new();
    for (bomb_entity, mut bomb, mut stepped, mut visibility, fired, its_look) in &mut bombs {
        if *fired == now {
            continue;
        }
        let step = bomb.advance(dt);
        stepped.current = bomb.transform();
        if step.bounced {
            debug!("bomb {} lands at {:.0}: sound {}", bomb.shot.id, bomb.position(), projectile::BOMB_BOUNCE_SOUND);
            turn.board.play(COMMON_BANK, projectile::BOMB_BOUNCE_SOUND as u32, FULL_VOLUME);
            turn.horde.jolts.push(shake::Asked::Kind(1));
        }
        if step.burst {
            debug!(
                "bomb {} bursts at {:.0}: effect {}, sound {}",
                bomb.shot.id,
                bomb.position(),
                bomb.shot.blast_effect,
                bomb.shot.blast_sound
            );
            // The burst's sound is of the bank the bullet's own is of (0x86568):
            // the port has the battle's, and the shooter's as long as it is there.
            if let Ok(sound) = u32::try_from(bomb.shot.blast_sound) {
                bomb_sounds.push((bomb.owner, bomb.shot.sound_bank, sound));
            }
            look.burst(&mut turn.commands.entity(bomb_entity), bomb.shot.blast_effect);
            turn.horde.jolts.push(shake::Asked::Burst);
        }
        match bomb.of_skill {
            Some(skill) if step.strikes => her_bursts.push((skill, bomb.shot, bomb.position())),
            Some(_) => {}
            None => {
                if let Some(at) = player_body.filter(|_| step.strikes).and_then(|centre| bomb.reaches(centre, PLAYER_BODY_RADIUS)) {
                    lands(&bomb.shot, bomb.owner, at, &mut player, turn.random, turn.board, &mut turn.horde.effects_asked);
                }
            }
        }
        // As a beam's, its burst's effect is told to die away when its time is up
        // (0x8AA8C).
        if !step.there {
            turn.commands.entity(bomb_entity).despawn();
        } else if !bomb.shown() {
            match its_look {
                Some(mut look) => look.end(),
                None => _ = visibility.set_if_neq(Visibility::Hidden),
            }
        }
    }
    for (owner, kind, sound) in bomb_sounds {
        let bank = match enemies.get(owner) {
            Ok((_, owner)) => bank_of(kind, &owner.archive).map(str::to_owned),
            Err(_) => (kind == BANK_OF_BATTLE).then(|| COMMON_BANK.to_owned()),
        };
        if let Some(bank) = bank {
            turn.board.play(&bank, sound, FULL_VOLUME);
        }
    }
    // A strike of the burst of a bomb of hers (0x8AB54): every enemy that has a
    // body sphere in use whose middle is within the burst's reach and its own
    // radius takes her attack of the bullet's record once, at the first such
    // sphere of it.
    for (skill, shot, at) in her_bursts {
        let Some(attack) = source.attacks.of_player(shot.attack) else {
            warn!("skill {skill}: there is no attack {} of hers", shot.attack);
            continue;
        };
        let mut struck: Vec<Entity> = Vec::new();
        turn.skill = skill;
        for body in body_spheres(&order, &enemies, &models) {
            if struck.contains(&body.enemy) || at.distance(body.centre) > shot.blast + body.radius {
                continue;
            }
            struck.push(body.enemy);
            // It lands on the side of the sphere that the bomb is on (0x8AC10).
            let landed = body.centre + (at - body.centre).normalize_or_zero() * body.radius;
            let Ok((_, mut enemy)) = enemies.get_mut(body.enemy) else { continue };
            let enemy = &mut *enemy;
            let index = enemy.actor.body.index;
            let mut events = Vec::new();
            let mut spawned = Spawned { entities: &enemy.models, query: &mut models };
            let view = view_of(&player, nodes);
            let mut world = turn.surroundings(view, &enemy.archive, &mut spawned, &mut events);
            match enemy.actor.hit(body.slot, landed, 0, attack, player_assailant(&source.stats, &player.status), &mut world) {
                Ok((dealt, drained)) => {
                    player.heal(drained);
                    info!("the bomb of skill {skill} strikes enemy {index} for {dealt} with attack {}: {:.0} HP left", attack.id, enemy.actor.body.hp);
                }
                Err(error) => {
                    error!("enemy {index} ({}): {error:#}", enemy.archive.folder);
                    enemy.actor.body.remove = true;
                }
            }
            turn.note(enemy);
            turn.carry_out(view, body.enemy, enemy, events);
        }
        turn.skill = 0;
        turn.deliver(&mut player, &source.stats, &mut enemies);
    }
    if player_shot {
        turn.send_all(signal::PLAYER_HIT, view_of(&player, nodes), &mut enemies, &mut models);
    }

    // Enemies whose scripts have had them removed go, with their models.
    for &(index, entity) in &order {
        let Ok((_, mut enemy)) = enemies.get_mut(entity) else { continue };
        if !enemy.actor.body.remove {
            continue;
        }
        for model in enemy.models.iter_mut().filter_map(Option::take) {
            turn.commands.entity(model).despawn();
        }
        turn.commands.entity(entity).despawn();
        let peer = &mut turn.horde.shared.peers[index];
        (peer.alive, peer.in_the_way) = (false, false);
        info!("enemy {index} is gone");
    }
    // The stage's look at who is left, and who may still come (0xA05F0).
    let horde = &mut *turn.horde;
    horde.left = horde.shared.peers.iter().filter(|peer| peer.alive).count();
    horde.to_beat = horde.shared.reserves.to_beat(horde.left);
    flow.count(dt, horde.left, horde.to_beat);

    // The HP each has been given back this frame is put up by it: at the node
    // its mark hangs from (0x916F4).
    for &(_, entity) in &order {
        let Ok((_, mut enemy)) = enemies.get_mut(entity) else { continue };
        let healed = std::mem::take(&mut enemy.actor.body.healed);
        let body = &enemy.actor.body;
        let (slot, node, _) = body.hp_gauge;
        let Some(at) = body.place_of(slot, node, Vec3::ZERO, &Seen { entities: &enemy.models, query: &models }) else { continue };
        horde.numbers.extend(healed.into_iter().map(|value| numbers::Asked { kind: numbers::Kind::Heal, value, at, hers: false }));
    }

    // What the player's cursor and shots can find next frame.
    horde.bodies = body_spheres(&order, &enemies, &models);
    horde.lock_points.clear();
    horde.places.clear();
    horde.marks.clear();
    for &(number, entity) in &order {
        let Ok((_, enemy)) = enemies.get(entity) else { continue };
        let body = &enemy.actor.body;
        horde.places.push((entity, body.placements[0].translation, horde.bodies.iter().any(|sphere| sphere.enemy == entity)));
        if !body.remove && !body.is_dead() {
            let (slot, node, offset) = body.hp_gauge;
            let view = Seen { entities: &enemy.models, query: &models };
            if let Some(at) = body.place_of(slot, node, offset, &view).filter(|_| body.anchored()) {
                let share = if body.max_hp > 0.0 { (body.hp / body.max_hp).clamp(0.0, 1.0) } else { 0.0 };
                horde.marks.push(MarkAt { at, share, level: body.variant.level as i32, second_life: body.second_life() > 0, icons: body.status.icons() });
            }
        }
        // The cursor passes over an enemy under either of two status effects
        // (0xA14D0): it cannot be locked onto.
        if body.remove || body.status.has(kind::UNSEEN) || body.status.has(kind::CLOUD) {
            continue;
        }
        let view = Seen { entities: &enemy.models, query: &models };
        for (index, gauge) in body.gauges.iter().enumerate() {
            let Some(gauge) = gauge.filter(|gauge| gauge.active) else { continue };
            if let Some(at) = body.place_of(gauge.slot, gauge.node, gauge.offset, &view) {
                horde.lock_points.push(LockAt { enemy: entity, number, gauge: index, at });
            }
        }
    }
}

/// The models of one enemy, for looking only.
struct Seen<'a, 'w, 's, 'q> {
    entities: &'a [Option<Entity>; SLOTS],
    query: &'a ModelQuery<'w, 's, 'q>,
}

impl Models for Seen<'_, '_, '_, '_> {
    fn animator(&mut self, _: usize) -> Option<&mut Animator> {
        None
    }

    fn node(&self, slot: usize, node: u8) -> Option<Transform> {
        let entity = (*self.entities.get(slot)?)?;
        let (animator, skeleton, _) = self.query.get(entity).ok()?;
        Some(animator.model_space(skeleton, skeleton.index_of(node)?))
    }

    fn turn(&mut self, _: usize, _: u8, _: Quat) {}
}

/// The body spheres of every enemy that can be hit right now.
fn body_spheres(
    order: &[(usize, Entity)],
    enemies: &Query<(Entity, &mut Enemy)>,
    models: &ModelQuery,
) -> Vec<BodySphere> {
    let mut bodies = Vec::new();
    for &(_, entity) in order {
        let Ok((_, enemy)) = enemies.get(entity) else { continue };
        let body = &enemy.actor.body;
        if body.remove {
            continue;
        }
        let view = Seen { entities: &enemy.models, query: models };
        for (slot, sphere) in body.spheres().filter(|(_, sphere)| sphere.enabled && !sphere.attack) {
            if let Some(centre) = body.sphere_centre(slot, &view) {
                bodies.push(BodySphere { enemy: entity, slot, centre, radius: sphere.radius });
            }
        }
    }
    bodies
}

/// Puts an enemy's models where its script has them.
fn place_models(enemy: &Enemy, models: &mut ModelQuery) {
    for (slot, model) in enemy.models.iter().enumerate() {
        if let Some((_, _, mut stepped)) = model.and_then(|model| models.get_mut(model).ok()) {
            stepped.current = enemy.actor.body.placements[slot];
        }
    }
}
