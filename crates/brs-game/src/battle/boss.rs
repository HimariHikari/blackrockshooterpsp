//! What a boss's battle has that no other has, all of it at its script's word (see
//! "Engine functions an enemy script uses" in `docs/BATTLE_NOTES.md`): scenes of
//! the boss's own with a camera move, with her model made to play its part in them;
//! the card that is put up as it is met (`ifBtlCutinTask`, 0x331B8); a sheet of a
//! colour over the whole picture (the object at 0x171318); and the finish
//! (`BtFinish`, 0x940C8, with `ifBtlFinishTask`, 0x33670), a gauge she fills by
//! hammering cross while it drains.
//!
//! A battle against a boss is also begun by its script: `BtEnter` (0x93D9C) only
//! has the enemies think, and the script lets the screen come, has the display
//! come and says when the fight is on.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{ensure, Context as _, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::{Animator, Clip};
use brs_engine::camera::CameraClip;
use brs_engine::canvas::{Canvas, Seen};
use brs_engine::layout::{Layout, Look, Player};
use brs_engine::model::{Parts, Skeleton};
use brs_engine::step::{Stepped, SIXTIETHS_PER_STEP};
use brs_formats::bxcb;

use super::actor::PcActor;
use super::aim::{self, Aim};
use super::bullet::Bullet;
use super::damage::Random;
use super::dimmer::Dimmer;
use super::effects::OfBattle;
use super::encounter::Encounter;
use super::enemy::actor::{CutIn, JAW_NODE};
use super::enemy::{Enemy, EnemyModel, Horde};
use super::flow::Flow;
use super::projectile::{Beam, Bomb, Projectile, Strike};
use super::tables::FINISH_LAYOUT;
use super::{Arsenal, BattleCamera, BattleSource, Held, Staging};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::player::{Holds, ShownParts};
use crate::sound::{Board, Mixer, Request, COMMON_BANK, FULL_VOLUME};

/// The table of the finishes (0x12A9CC looks a record up by its first word).
const FINISHES: &str = "SYSTEM.VOL/btl_finish_tbl.bin";
/// The sounds of the battle's bank: the second set of a cut-in (0x3325C) and a
/// finish that is full (0x948B8).
const SOUND_CUT_IN: u32 = 0x70;
const SOUND_FINISH_FULL: u32 = 0x63;
/// The frames of a layout a frame of the game is.
const LAYOUT_FRAMES: f32 = 2.0;
/// The frames of the game a recording that has been asked for is given to be
/// heard of before a stream that says nothing is taken to be silent: what plays
/// the sounds has its turn between the game's frames, and not always between two.
const HEARD_WITHIN: u8 = 60;

/// What an enemy's script has asked of the battle as a whole, with what it names
/// looked up, until `direct` carries it out.
#[derive(Clone)]
pub enum Direction {
    /// A scene of the enemy's begins: its camera move, if its archives have it,
    /// in the frame of a model of the enemy's or of a node of that.
    Scene { camera: Option<Arc<CameraClip>>, model: Option<Entity>, node: Option<u8> },
    SceneOver,
    HerAnimation { clip: Arc<Clip>, looping: bool, speed: i32 },
    HerMoved(Vec3),
    HerTurned(f32),
    HerShown(bool),
    HerParts(i32),
    /// A mouth is turned so far: hers with no model named.
    Jaw { model: Option<Entity>, angle: f32 },
    /// Parts of an enemy's model are shown or hidden, each by its number.
    Parts { model: Entity, changes: Vec<(u16, bool)> },
    Lens { list: i32, near: f32, far: f32 },
    /// An enemy has taken the stage, or given it back.
    Stage(bool),
    /// What is behind is dimmed, as the enemy with this number has it.
    Dim { by: usize, to: i32, time: i32 },
    /// The cut-in, with the pack its layout is for one that is to be loaded.
    CutIn(CutIn, Option<String>),
    /// The finish begins: the record of its table, and the pack of its layout.
    Finish(i32, Option<String>),
    Sheet(u32),
    SheetGoes(i32),
    Go,
    Fight,
    Display(bool),
    Hush,
    NoBullets,
    MusicOff(i32),
    /// Every enemy but the one with this number goes.
    Dismiss(usize),
}

/// The figures of a finish: a record of its table (0x94508 copies them out, the
/// times doubled, which are in frames of the game there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figures {
    id: i32,
    /// Whose it is: which of the layout's seven names is shown.
    look: i32,
    /// What the gauge holds, and what it starts with.
    full: f32,
    start: f32,
    /// What a press adds, at the least and at the most.
    gain: [f32; 2],
    /// Between these two amounts of the gauge a press adds less and less: by so
    /// many hundredths at the first and so many at the second.
    eased_from: [f32; 2],
    eased_by: [f32; 2],
    /// What the gauge loses each time it drains, at the least and at the most,
    /// and the sixtieths to the next time: just after a press, and once so many
    /// times have gone by with none.
    drain: [[f32; 2]; 2],
    wait: [[f32; 2]; 2],
    slackens_over: f32,
    /// One time in so many, the first drain after a press is this much instead.
    lapse_chance: i32,
    lapse: f32,
    /// The sixtieths a full gauge stays before it goes.
    stays: f32,
}

impl Figures {
    const FIELDS: usize = 22;

    fn from_record(record: &[u32]) -> Figures {
        let field = |index: usize| record[index] as i32 as f32;
        Figures {
            id: record[0] as i32,
            look: record[1] as i32,
            full: field(2),
            start: field(3),
            gain: [field(4), field(5)],
            eased_from: [field(6), field(7)],
            eased_by: [field(8), field(9)],
            drain: [[field(10), field(11)], [field(15), field(16)]],
            wait: [[field(12) * 2.0, field(13) * 2.0], [field(17) * 2.0, field(18) * 2.0]],
            slackens_over: field(14),
            lapse_chance: record[19] as i32,
            lapse: field(20),
            stays: field(21) * 2.0,
        }
    }

    /// The table, read.
    pub fn load(data: &GameData) -> Result<Vec<Figures>> {
        let table = bxcb::Table::parse(&data.read(FINISHES)?).with_context(|| format!("parsing {FINISHES}"))?;
        ensure!(table.fields == Self::FIELDS, "{FINISHES} has {} fields per record", table.fields);
        Ok(table.records().map(Figures::from_record).collect())
    }
}

/// `a` to `b`, `share` of the way (0x94074).
fn between(a: f32, b: f32, share: f32) -> f32 {
    a + (b - a) * share
}

/// How far `value` is from `a` to `b`, kept within nought and one; one where the
/// two are the same (0x94010).
fn share_of(a: f32, b: f32, value: f32) -> f32 {
    if a == b { 1.0 } else { ((value - a) / (b - a)).clamp(0.0, 1.0) }
}

/// A number from nought to one (0x94084).
fn chance(random: &mut Random) -> f32 {
    random.below(0x8000) as f32 / 32767.0
}

/// How far the finish has got (`BtFinish` + 0x80).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// The gauge is being filled.
    Filling,
    /// It is full, and stays a while.
    Full,
    Over,
}

/// What a frame of the finish has to be heard and seen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Told {
    /// It has just become full.
    pub full: bool,
    /// It has just gone.
    pub gone: bool,
}

/// A finish (0x946F4): the gauge, and what drains it.
#[derive(Clone, Debug, PartialEq)]
pub struct Finish {
    figures: Figures,
    stage: Stage,
    value: f32,
    /// The sixtieths to the next drain, and how many of them have gone by.
    wait: f32,
    waited: f32,
    /// The drains since the last press.
    drains: i32,
    stayed: f32,
}

/// The sixtieths to the first drain (0x94790).
const FIRST_WAIT: f32 = 10.0;

impl Finish {
    pub fn new(figures: Figures) -> Finish {
        Finish { figures, stage: Stage::Filling, value: figures.start, wait: FIRST_WAIT, waited: 0.0, drains: 0, stayed: 0.0 }
    }

    /// How full the gauge is, of one.
    pub fn share(&self) -> f32 {
        if self.stage == Stage::Filling { (self.value / self.figures.full).clamp(0.0, 1.0) } else { 1.0 }
    }

    /// The enemies wait for it no longer (0x9469C): from when it is full.
    pub fn waited_for(&self) -> bool {
        self.stage == Stage::Filling
    }

    /// A press of cross (0x94228): the gauge gains by chance within its bounds,
    /// less of it the fuller it is.
    fn press(&mut self, random: &mut Random) {
        let figures = &self.figures;
        let gain = between(figures.gain[0], figures.gain[1], chance(random));
        let eased = between(figures.eased_by[0], figures.eased_by[1], share_of(figures.eased_from[0], figures.eased_from[1], self.value));
        self.value = (self.value + (gain - gain * eased / 100.0).max(0.0)).min(figures.full);
    }

    /// The gauge drains (0x94308), and the wait to the next time is drawn: both the
    /// more the longer cross has not been pressed.
    fn drain(&mut self, random: &mut Random) {
        let figures = &self.figures;
        let slack = share_of(1.0, figures.slackens_over, self.drains as f32);
        let least = between(figures.drain[0][0], figures.drain[1][0], slack);
        let most = between(figures.drain[0][1], figures.drain[1][1], slack);
        let mut lost = between(least, most, chance(random)).max(0.0);
        if self.drains == 0 && random.below(figures.lapse_chance) == 0 {
            lost = figures.lapse;
        }
        self.value = (self.value - lost).max(0.0);
        let least = between(figures.wait[0][0], figures.wait[1][0], slack);
        let most = between(figures.wait[0][1], figures.wait[1][1], slack);
        self.wait = between(least, most, chance(random));
        self.waited = 0.0;
        self.drains += 1;
    }

    /// A frame of it (0x946F4).
    pub fn step(&mut self, dt: f32, pressed: bool, random: &mut Random) -> Told {
        match self.stage {
            Stage::Filling => {
                if pressed {
                    self.press(random);
                    self.drains = 0;
                }
                self.waited += dt;
                if self.waited >= self.wait {
                    self.drain(random);
                }
                if self.value < self.figures.full {
                    return Told::default();
                }
                self.stage = Stage::Full;
                self.stayed = 0.0;
                Told { full: true, gone: false }
            }
            Stage::Full => {
                self.stayed += dt;
                if self.stayed < self.figures.stays {
                    return Told::default();
                }
                self.stage = Stage::Over;
                Told { full: false, gone: true }
            }
            Stage::Over => Told::default(),
        }
    }
}

/// The sets of the finish's layout its four players play (0x336BC to 0x337DC):
/// the words that come and go, what a full gauge has, the gauge, and what rides
/// its end; each with the set it comes by and the one it goes by.
const FINISH_SETS: [[usize; 2]; 4] = [[3, 4], [5, 6], [9, 10], [11, 12]];
const WORDS: usize = 0;
const FULL: usize = 1;
const GAUGE: usize = 2;
const RIDER: usize = 3;
/// The groups of the gauge's player that are a name, one for each look, in the
/// order the looks have them (0x33FC4 to 0x340A4); the group that is the part of
/// the bar not yet filled and the one that is the part filled (0x33EB8, 0x33DC4).
const NAME_GROUPS: [usize; 7] = [10, 11, 13, 12, 14, 15, 16];
const EMPTY_GROUP: usize = 18;
const FILLED_GROUP: usize = 20;
/// The ticks the bar's pattern takes to go once across (0x33958).
const BAR_CYCLE: f32 = 120.0;

/// A player of the finish's layout that comes by one set and goes by another
/// (the task's + 0x20 on: 0 plays on, 1 plays out and is then gone).
#[derive(Clone, Debug)]
struct Part {
    player: Player,
    going: bool,
}

/// What is seen of a finish.
struct FinishShown {
    pack: String,
    look: i32,
    parts: [Option<Part>; 4],
    /// How far the bar's pattern has gone across, in ticks.
    pattern: f32,
}

impl FinishShown {
    fn comes(&mut self, layout: &Layout, part: usize) {
        self.parts[part] = Some(Part { player: layout.player(FINISH_SETS[part][0]), going: false });
    }

    fn goes(&mut self, layout: &Layout, part: usize) {
        if let Some(shown) = &mut self.parts[part] {
            shown.player.play(layout, FINISH_SETS[part][1]);
            shown.going = true;
        }
    }

    /// A frame of the layout's (0x338B4): each player goes on, and one that was
    /// going is gone when it has played through.
    fn advance(&mut self, layout: &Layout, frames: f32) {
        for part in &mut self.parts {
            if let Some(shown) = part {
                shown.player.advance(layout, frames);
                if shown.going && shown.player.finished(layout) {
                    *part = None;
                }
            }
        }
        self.pattern = if self.pattern < BAR_CYCLE { self.pattern + frames } else { 0.0 };
    }

    /// Draws it (0x33C44) with the gauge so full.
    fn draw(&self, canvas: &mut Canvas, layout: &Layout, share: f32) {
        let plain = Look::default();
        for part in [WORDS, FULL] {
            if let Some(shown) = &self.parts[part] {
                layout.draw(canvas, layer::DISPLAY, &shown.player, &plain);
            }
        }
        let bar = |group: usize| layout.first_of(group).map(|object| &layout.objects.objects[object]);
        if let Some(shown) = &self.parts[GAUGE] {
            for track in 0..shown.player.len() {
                let Some(group) = shown.player.group(track) else { continue };
                // Of the names only hers whose finish it is.
                if let Some(look) = NAME_GROUPS.iter().position(|name| *name == group) {
                    if look as i32 != self.look {
                        continue;
                    }
                }
                // The bar is in two parts, each cut to its share of the bar's own
                // room, its pattern going across: the filled part's one way and
                // the other's the other (0x15AE38, 0x15B098).
                let across = self.pattern / BAR_CYCLE;
                let cut = match group {
                    FILLED_GROUP => bar(group).map(|bar| (0.0, share, 1.0 - across, bar)),
                    EMPTY_GROUP => bar(group).map(|bar| (share, 1.0 - share, across, bar)),
                    _ => None,
                };
                match cut {
                    Some((from, part, shift, bar)) => {
                        let (at, room) = (Vec2::new(bar.position[0] as f32, bar.position[1] as f32), Vec2::new(bar.size[0] as f32, bar.size[1] as f32));
                        let from = at + Vec2::X * (room.x * from).trunc();
                        let width = (room.x * part).trunc();
                        if width > 0.0 {
                            canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(width, room.y))));
                            layout.draw_track(canvas, layer::DISPLAY, &shown.player, track, &Look { shift: Vec2::new(shift, 0.0), ..plain });
                            canvas.cut_to(None);
                        }
                    }
                    None => layout.draw_track(canvas, layer::DISPLAY, &shown.player, track, &plain),
                }
            }
        }
        // What rides the bar's end is moved along it by the bar's width (0x33CE8).
        if let Some(shown) = &self.parts[RIDER] {
            let along = bar(FILLED_GROUP).map_or(0.0, |bar| (bar.size[0] as f32).trunc() * share);
            layout.draw(canvas, layer::DISPLAY, &shown.player, &Look { outer: Affine2::from_translation(Vec2::X * along), ..plain });
        }
    }
}

/// The card put up as a boss is met: a layout whose sets a script plays one after
/// the other (`ifBtlCutinTask`).
struct Card {
    pack: String,
    /// The sets that play, each by its number.
    sets: Vec<(usize, Player)>,
}

/// The sheet of a colour over the whole picture (0xA2268, 0xA249C).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Sheet {
    shown: bool,
    colour: [u8; 3],
    /// The sixtieths its going takes and how many have gone by, once it is going.
    going: Option<(f32, f32)>,
}

impl Sheet {
    /// How much of it is seen, of 255: all of it until it is going, then
    /// `trunc((1 - t / T) x 255)`.
    fn alpha(&self) -> u8 {
        match self.going {
            _ if !self.shown => 0,
            None => 255,
            Some((time, run)) if run < time => ((1.0 - run / time) * 255.0) as u8,
            Some(_) => 0,
        }
    }

    fn step(&mut self, dt: f32) {
        if let (true, Some((time, run))) = (self.shown, &mut self.going) {
            *run = (*run + dt).min(*time);
            if *run >= *time {
                (self.shown, self.going) = (false, None);
            }
        }
    }
}

/// What a boss's battle shows, and what of the battle its script decides.
#[derive(Resource, Default)]
pub struct Show {
    /// The layouts of the enemies' archives, by the pack each is.
    layouts: HashMap<String, Layout>,
    finishes: Vec<Figures>,
    /// The display is on (the root's flag 11): from the start in any other battle,
    /// and when its script says so in a boss's.
    display: bool,
    /// Whoever has the stage has taken the display away (root + 0x194).
    taken: bool,
    card: Option<Card>,
    sheet: Sheet,
    finish: Option<(Finish, FinishShown)>,
    /// How many finishes have been done with (`BtFinish` + 0x84).
    finished: i32,
    /// An enemy's scene is on: the model, and the node of it, whose frame its
    /// camera move plays in.
    scene: Option<(Option<Entity>, Option<u8>)>,
    /// The frames left for a recording asked for on each stream to be heard of.
    awaited: [u8; 2],
}

impl Show {
    /// Whether the battle's display is to be drawn.
    pub fn display(&self) -> bool {
        self.display && !self.taken
    }

    /// Whether a finish has taken the column of her skills away (0x9EEAC).
    pub fn finishing(&self) -> bool {
        self.finish.is_some() || self.finished > 0
    }

    /// Whether a scene of an enemy's has the camera.
    pub fn scene_on(&self) -> bool {
        self.scene.is_some()
    }
}

/// Makes what a battle's enemies may show ready: the layouts of their archives,
/// and the table of the finishes.
pub(super) fn begin(mut commands: Commands, data: Res<GameData>, source: Res<BattleSource>, mut images: ResMut<Assets<Image>>) {
    let mut show = Show { display: !source.group.scripted(), ..default() };
    for pack in source.group.layouts() {
        match interface::layout(&data, &mut images, &pack, 0) {
            Ok(layout) => {
                show.layouts.insert(pack, layout);
            }
            Err(error) => warn!("{pack}: {error:#}"),
        }
    }
    if !show.layouts.is_empty() {
        match Figures::load(&data) {
            Ok(finishes) => show.finishes = finishes,
            Err(error) => warn!("{error:#}"),
        }
    }
    commands.insert_resource(show);
}

/// The frame of a model, or of a node of it, in the world.
fn frame_of(models: &Query<(&mut Animator, &Skeleton, &Stepped), With<EnemyModel>>, model: Option<Entity>, node: Option<u8>) -> Transform {
    let Some(Ok((animator, skeleton, stepped))) = model.map(|model| models.get(model)) else { return Transform::IDENTITY };
    let on_model = node.and_then(|node| skeleton.index_of(node)).map_or(Transform::IDENTITY, |index| animator.model_space(skeleton, index));
    stepped.current * on_model
}

/// Carries out what the enemies' scripts have asked of the battle as a whole this
/// frame, has what a boss's battle shows go on by a frame, and tells the scripts
/// what their waits go by.
#[allow(clippy::too_many_arguments)]
pub(super) fn direct(
    mut commands: Commands,
    pad: Res<Pad>,
    (mut horde, mut show, mut flow, mut encounter): (ResMut<Horde>, ResMut<Show>, ResMut<Flow>, ResMut<Encounter>),
    (mut staging, mut dimmer, mut arsenal, aim): (ResMut<Staging>, ResMut<Dimmer>, ResMut<Arsenal>, Res<Aim>),
    (mut board, mixer, mut random): (ResMut<Board>, Res<Mixer>, ResMut<Random>),
    player: Single<(&mut PcActor, &mut Animator, &Skeleton, &mut Stepped, &mut ShownParts, &mut Visibility), (Without<BattleCamera>, Without<EnemyModel>)>,
    camera: Single<(&mut Projection, &mut Stepped), (With<BattleCamera>, Without<PcActor>, Without<EnemyModel>)>,
    mut models: Query<(&mut Animator, &Skeleton, &Stepped), With<EnemyModel>>,
    (mut enemies, their_parts): (Query<&mut Enemy>, Query<&Parts, With<EnemyModel>>),
    shots: Query<Entity, Or<(With<Bullet>, With<Projectile>, With<Beam>, With<Bomb>, With<Strike>)>>,
    effects: Query<Entity, With<OfBattle>>,
) {
    let dt = SIXTIETHS_PER_STEP;
    let (mut actor, mut animator, skeleton, mut stepped, mut parts, mut seen) = player.into_inner();
    let (mut projection, mut view) = camera.into_inner();
    let show = &mut *show;
    let mut cut = false;
    for direction in std::mem::take(&mut horde.directions) {
        match direction {
            // She is put in the scene's frame (0xBD0BC) with her animation's blends
            // dropped (0xAA9A0), and the camera move plays.
            Direction::Scene { camera, model, node } => {
                actor.placed = Some(Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)));
                stepped.snap(actor.transform());
                animator.body_mut().drop_blend();
                if let Some(clip) = camera {
                    staging.playing = Some((clip, 0.0));
                    show.scene = Some((model, node));
                    cut = true;
                }
            }
            // She is back where she stands (0x99950) and the camera is the
            // battle's again (0xA7E58).
            Direction::SceneOver => {
                actor.placed = None;
                stepped.snap(actor.transform());
                if show.scene.take().is_some() {
                    staging.playing = None;
                    cut = true;
                }
            }
            Direction::HerAnimation { clip, looping, speed } => {
                if !animator.body().is(&clip) {
                    animator.set(&clip);
                }
                if looping {
                    animator.body_mut().loop_clip();
                } else {
                    animator.body_mut().play_out();
                }
                animator.speed = speed as f32 / 100.0;
            }
            Direction::HerMoved(by) => {
                let mut placed = actor.transform();
                placed.translation += placed.rotation * by;
                actor.placed = Some(placed);
                stepped.snap(placed);
            }
            Direction::HerTurned(angle) => {
                actor.placed = Some(Transform::from_rotation(Quat::from_rotation_y(angle)));
                stepped.snap(actor.transform());
            }
            Direction::HerShown(shown) => *seen = if shown { Visibility::Inherited } else { Visibility::Hidden },
            Direction::HerParts(mode) => match u8::try_from(mode).map(|mode| parts.set_mode(mode)) {
                Ok(Holds::Nothing) => arsenal.wanted = Some(Held::Nothing),
                Ok(Holds::Cannon) => arsenal.wanted = Some(Held::Cannon),
                _ => {}
            },
            Direction::Jaw { model, angle } => {
                let turn = Quat::from_rotation_z(angle);
                match model {
                    None => {
                        if let Some(index) = skeleton.index_of(JAW_NODE) {
                            animator.set_own_turn(index, turn);
                        }
                    }
                    Some(model) => {
                        if let Ok((mut animator, skeleton, _)) = models.get_mut(model) {
                            if let Some(index) = skeleton.index_of(JAW_NODE) {
                                animator.set_own_turn(index, turn);
                            }
                        }
                    }
                }
            }
            Direction::Parts { model, changes } => {
                let Ok(of_model) = their_parts.get(model) else { continue };
                for (part, shown) in changes {
                    for &mesh in of_model.entities(part) {
                        commands.entity(mesh).insert(if shown { Visibility::Inherited } else { Visibility::Hidden });
                    }
                }
            }
            // How near and how far the cameras of all but the sky see (0xA6724); the
            // sky's own are the map's to say here.
            Direction::Lens { list, near, far } => {
                if let (1, Projection::Perspective(perspective)) = (list, &mut *projection) {
                    (perspective.near, perspective.far) = (near, far);
                }
            }
            Direction::Stage(taken) => show.taken = taken,
            Direction::Dim { by, to, time } => dimmer.go_for(by, to, time as f32),
            Direction::CutIn(what, pack) => match what {
                CutIn::Load(name) => match pack {
                    Some(pack) => show.card = Some(Card { pack, sets: Vec::new() }),
                    None => warn!("there is no cut-in {name}"),
                },
                CutIn::Play(set) => {
                    let (Some(card), Ok(set)) = (&mut show.card, usize::try_from(set)) else { continue };
                    let Some(layout) = show.layouts.get(&card.pack) else { continue };
                    card.sets.retain(|(playing, _)| *playing != set);
                    card.sets.push((set, layout.player(set)));
                    if set == 1 {
                        board.play(COMMON_BANK, SOUND_CUT_IN, FULL_VOLUME);
                    }
                }
                CutIn::Stop(set) => {
                    if let Some(card) = &mut show.card {
                        card.sets.retain(|(playing, _)| Ok(*playing) != usize::try_from(set));
                    }
                }
                CutIn::Drop => show.card = None,
            },
            // The finish (0x94508): its words, its gauge and what rides the
            // gauge's end come (0x947B0).
            Direction::Finish(record, pack) => {
                let figures = show.finishes.iter().find(|figures| figures.id == record).copied();
                let layout = pack.as_ref().and_then(|pack| show.layouts.get(pack));
                match (figures, pack.clone(), layout) {
                    (Some(figures), Some(pack), Some(layout)) => {
                        let mut shown = FinishShown { pack, look: figures.look, parts: [const { None }; 4], pattern: 0.0 };
                        for part in [WORDS, GAUGE, RIDER] {
                            shown.comes(layout, part);
                        }
                        info!("the finish begins: record {record}");
                        show.finish = Some((Finish::new(figures), shown));
                    }
                    _ => warn!("there is no finish {record} to show ({pack:?})"),
                }
            }
            Direction::Sheet(colour) => {
                // The colour is the PSP's: red in its low byte.
                show.sheet = Sheet { shown: true, colour: [colour as u8, (colour >> 8) as u8, (colour >> 16) as u8], going: None };
            }
            Direction::SheetGoes(time) => {
                if show.sheet.shown {
                    show.sheet.going = Some((time as f32, 0.0));
                }
            }
            Direction::Go => encounter.let_go(),
            Direction::Fight => flow.begin(),
            Direction::Display(on) => show.display = on,
            // Both streams stop, every effect of the battle is no more and every
            // bullet goes (0xBED60).
            Direction::Hush => {
                board.ask_all([Request::Hush(0, 0), Request::Hush(1, 0)]);
                for entity in effects.iter().chain(shots.iter()) {
                    commands.entity(entity).try_despawn();
                }
            }
            Direction::NoBullets => {
                for entity in &shots {
                    commands.entity(entity).try_despawn();
                }
            }
            Direction::MusicOff(time) => board.ask(Request::StopMusic(time.max(0))),
            Direction::Dismiss(by) => {
                for mut enemy in &mut enemies {
                    if enemy.actor.body.index != by {
                        enemy.actor.body.remove = true;
                    }
                }
            }
        }
    }

    // What is shown goes on by a frame: the card's sets, the sheet, the finish.
    if let Some(card) = &mut show.card {
        if let Some(layout) = show.layouts.get(&card.pack) {
            for (_, player) in &mut card.sets {
                player.advance(layout, LAYOUT_FRAMES);
            }
        }
    }
    show.sheet.step(dt);
    dimmer.step_theirs(dt);
    if let Some((finish, shown)) = &mut show.finish {
        let layout = show.layouts.get(&shown.pack);
        let told = finish.step(dt, pad.pressed & button::CROSS != 0, &mut random);
        if let Some(layout) = layout {
            // Full, its words go and what a full gauge has comes (0x948C4 on);
            // and when that has stayed its time all of it goes (0x94938).
            if told.full {
                board.play(COMMON_BANK, SOUND_FINISH_FULL, FULL_VOLUME);
                shown.goes(layout, WORDS);
                shown.comes(layout, FULL);
            }
            if told.gone {
                for part in [GAUGE, RIDER, FULL] {
                    shown.goes(layout, part);
                }
            }
            shown.advance(layout, LAYOUT_FRAMES);
        }
        if told.full {
            show.finished += 1;
            info!("the finish is full");
        }
        if finish.stage == Stage::Over && shown.parts.iter().all(Option::is_none) {
            show.finish = None;
        }
    }

    // What the scripts' waits go by, as it is now. A stream that has just been
    // given something to say is not silent, though nothing is heard of it yet.
    for (stream, said) in std::mem::take(&mut horde.said).into_iter().enumerate() {
        if said {
            show.awaited[stream] = HEARD_WITHIN;
        }
    }
    let silent = [0, 1].map(|stream| {
        if mixer.saying(stream).is_some() {
            show.awaited[stream] = 0;
            return false;
        }
        show.awaited[stream] = show.awaited[stream].saturating_sub(1);
        show.awaited[stream] == 0
    });
    let told = horde.told();
    told.encounter_over = encounter.over();
    told.silent = silent;
    let playing = |card: &Card| show.layouts.get(&card.pack).is_some_and(|layout| card.sets.iter().any(|(_, player)| !player.finished(layout)));
    told.cut_in_plays = show.card.as_ref().is_some_and(playing);
    told.finishing = show.finish.as_ref().is_some_and(|(finish, _)| finish.waited_for());
    told.finishes = show.finished;

    // The camera of an enemy's scene: its move in the frame of what the scene
    // named (0xA400C), with the move's own lens. It begins and ends with a cut.
    if let (Some((model, node)), Some((clip, frame))) = (show.scene, &staging.playing) {
        let pose = clip.pose(*frame);
        let placed = frame_of(&models, model, node) * pose.transform;
        if let Projection::Perspective(perspective) = &mut *projection {
            perspective.fov = pose.vertical_fov(aim::SCREEN.x / aim::SCREEN.y);
        }
        if cut || clip.cuts(*frame - dt, *frame) {
            view.snap(placed);
        } else {
            view.current = placed;
        }
    } else if cut {
        if let Projection::Perspective(perspective) = &mut *projection {
            perspective.fov = aim::vertical_fov();
        }
        view.snap(aim.view.transform());
    }
}

/// Draws what a boss's battle shows over the scene: the sheet under all of the
/// interface, and the card and the finish with the battle's display.
pub(super) fn draw(show: Option<Res<Show>>, seen: Res<Seen>, mut canvas: ResMut<Canvas>) {
    let Some(show) = show else { return };
    let alpha = show.sheet.alpha();
    if alpha != 0 {
        let [red, green, blue] = show.sheet.colour;
        canvas.fill(layer::SCENE_FADE, seen.0, [red, green, blue, alpha]);
    }
    if let Some(card) = &show.card {
        if let Some(layout) = show.layouts.get(&card.pack) {
            for (_, player) in &card.sets {
                layout.draw(&mut canvas, layer::DISPLAY, player, &Look::default());
            }
        }
    }
    if let Some((finish, shown)) = &show.finish {
        if let Some(layout) = show.layouts.get(&shown.pack) {
            shown.draw(&mut canvas, layout, finish.share());
        }
    }
}

/// The name the finish's layout has in every archive that has one.
pub fn finish_layout() -> &'static str {
    FINISH_LAYOUT
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn the_table_of_the_finishes_has_one_gauge_for_all_of_them() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let finishes = Figures::load(&data).unwrap();
        assert_eq!(finishes.len(), 24);
        let first = finishes[0];
        assert_eq!((first.id, first.look, first.full, first.start), (1, 0, 10000.0, 0.0));
        assert_eq!((first.gain, first.eased_from, first.eased_by), ([250.0, 300.0], [6000.0, 9000.0], [0.0, 10.0]));
        assert_eq!((first.drain, first.wait), ([[10.0, 20.0], [25.0, 30.0]], [[4.0, 16.0], [4.0, 6.0]]));
        assert_eq!((first.slackens_over, first.lapse_chance, first.lapse, first.stays), (5.0, 10, 30.0, 26.0));
        // They differ in whose they are and in how long a full gauge stays.
        for figures in &finishes {
            assert_eq!(Figures { id: 1, look: 0, stays: 26.0, ..*figures }, first);
        }
        let looks: Vec<i32> = finishes.iter().map(|figures| figures.look).collect();
        assert_eq!(looks, [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6, 6, 6]);
    }

    fn figures() -> Figures {
        Figures {
            id: 1,
            look: 0,
            full: 10000.0,
            start: 0.0,
            gain: [250.0, 300.0],
            eased_from: [6000.0, 9000.0],
            eased_by: [0.0, 10.0],
            drain: [[10.0, 20.0], [25.0, 30.0]],
            wait: [[4.0, 16.0], [4.0, 6.0]],
            slackens_over: 5.0,
            lapse_chance: 10,
            lapse: 30.0,
            stays: 26.0,
        }
    }

    #[test]
    fn a_finish_is_filled_by_hammering_cross_and_drains_when_left() {
        let mut random = Random::new(7);
        let mut finish = Finish::new(figures());
        assert_eq!((finish.share(), finish.waited_for()), (0.0, true));
        // A press adds between 250 and 300 of the 10000.
        finish.step(2.0, true, &mut random);
        assert!((250.0..=300.0).contains(&finish.value), "{}", finish.value);
        // Left alone it drains: first after ten sixtieths, then every few, by ten
        // to thirty a time, and never below nothing.
        let had = finish.value;
        for _ in 0..4 {
            finish.step(2.0, false, &mut random);
        }
        assert!(finish.value < had && finish.value >= had - 30.0, "{} of {had}", finish.value);
        for _ in 0..600 {
            finish.step(2.0, false, &mut random);
        }
        assert_eq!(finish.value, 0.0);
        // Pressed every other frame it is full in well under a hundred presses, a
        // press adding less once it is past six tenths.
        let mut presses = 0;
        while !finish.step(2.0, true, &mut random).full {
            assert!(!finish.step(2.0, false, &mut random).full);
            presses += 1;
            assert!(presses < 100);
        }
        assert!((34..=45).contains(&presses), "{presses}");
        assert_eq!((finish.share(), finish.waited_for()), (1.0, false));
        // Full, it stays its thirteen frames and goes.
        let frames = (0..40).position(|_| finish.step(2.0, false, &mut random).gone).unwrap();
        assert_eq!(frames, 12);
    }

    #[test]
    fn the_sheet_over_the_picture_goes_evenly_over_its_time() {
        let mut sheet = Sheet { shown: true, colour: [255; 3], going: None };
        sheet.step(2.0);
        assert_eq!(sheet.alpha(), 255);
        // Thirty frames of the game are sixty sixtieths.
        sheet.going = Some((60.0, 0.0));
        let seen: Vec<u8> = (0..30)
            .map(|_| {
                sheet.step(2.0);
                sheet.alpha()
            })
            .collect();
        assert_eq!(&seen[..3], [246, 238, 229]);
        // (Cut to a whole number as the original cuts it: a hair under 17 is 16.)
        assert_eq!(&seen[27..], [16, 8, 0]);
        assert!(!sheet.shown);
    }
}
