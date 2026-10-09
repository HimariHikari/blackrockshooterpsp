//! The field: a map with its characters, run by the map's script, with the player
//! running about it under a following camera. Touching an enemy hands over to a
//! battle and takes the result back.
//!
//! The scene the script works on is plain data ([`scene::Scene`]); the systems here
//! step it once a logic frame and show it. See `docs/FIELD_NOTES.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use brs_engine::animation::{Animator, Clip};
use brs_engine::camera::{vertical_fov, CameraPath};
use brs_engine::canvas::{all_of_the_screen, Canvas, Captured, Screen, Seen};
use brs_engine::offscreen::Offscreen;
use brs_engine::collision::{Body, CollisionWorld, Layer};
use brs_engine::lit::Lit;
use brs_engine::model::{ModelAsset, Parts, Skeleton};
use brs_engine::step::{Stepped, STEPS_PER_SECOND};
use brs_engine::{map, AssetStores, EngineSystems};
use brs_formats::{clim, stcm};

use crate::battle::actor::WEAPON_NODE;
use crate::battle::skills::Skills;
use crate::battle::tables::{PlayerStats, Starts};
use crate::battle::{self, BattleSource, Terms};
use crate::controls::{button, Look, Pad, Seek};
use crate::data::GameData;
use crate::gallery::{Gallery, Opened};
use crate::hub::{self, Hub, HubAssets};
use crate::title::{self, Title, TitleAssets};
use crate::interface::layer;
use crate::items::{Inventory, Items};
use crate::menu::{self, Menu, MenuAssets};
use crate::pause::{self, Pause, PauseAssets, Paused};
use crate::progress::{variable, Progress};
use crate::record_news::RecordNews;
use crate::record_window::RecordWindow;
use crate::records::{self, Records};
use crate::save::{Place, Played, Saved, Saves};
use crate::save_screen::{self, SaveDesk, SaveScreen};
use crate::sound::{Board, Request};
use crate::text::TextColours;
use crate::writing::Fonts;
use crate::{player, Mode};

pub mod action;
pub mod assets;
pub mod camera;
pub mod cleared;
pub mod comm;
pub mod effects;
pub mod event;
pub mod failed;
pub mod frames;
pub mod host;
pub mod hud;
pub mod map_screen;
pub mod motion;
pub mod mover;
pub mod place;
pub mod scene;
pub mod scenery;
mod shadows;
pub mod staging;
pub mod steps;
mod tails;
pub mod symbol;
pub mod system;
pub mod tables;
pub mod window;

use action::By;
use assets::{animation, CharacterAsset, CharacterAssets};
use camera::{figure, kind, Lag};
use host::{Callback, Field, MapScript};
use map_screen::{MapAssets, MapScreen, MapUp};
use motion::Ground;
use scene::{flag, status, value, Action, BattleReport, Character, LeapPart, Motion, NeckWay, Scene, Wall, FULL_PACE, PLAYER};
use scenery::Scenery;
use staging::Staging;
use symbol::Brains;
use system::{Frame, SystemScript};
use tables::FieldTables;

/// The frames of an animation that go by in a frame of the game.
const STEP_ANIMATION_FRAMES: f32 = 2.0;
/// How many frames of the game apart the faces of a character's animation of
/// faces are (the resident script's `sub_6bc`: five, of 0x800 each).
const FRAMES_A_FACE: f32 = 5.0;

/// How far the stick has to be pushed for the player to move. The original takes any
/// deflection its pad reading lets through; this stands in for that reading's dead zone.
const STICK_DEAD_ZONE: f32 = 0.2;

/// The map a new game starts on, for the story's opening (0x95C): `d_stg05_0a1`,
/// whose script plays it and goes on to the first mission's map.
const OPENING_MAP: i32 = 67;
/// The first mission's map: it is its stage the records of a new game are set up
/// for.
const FIRST_MAP: &str = "STG01_01";

/// How near a place a scripted run that is steered to it stops.
const SEEK_REACH: f32 = 30.0;
/// The numbers a map's script gives the enemies that go about it.
const SYMBOLS: std::ops::RangeInclusive<i32> = 500..=599;

/// How far above or below a character the player can be and still touch it. A
/// stand-in: the original goes by the two characters' collision shapes (0x118330).
const TOUCH_HEIGHT: f32 = 200.0;

// Offsets in the field's tunables (`prm`): the radius of a character's collision
// sphere, and the camera's turning rates in degrees per logic step: how fast the turn
// speeds up while L or R is held, how fast it is cancelled when the other button is
// pressed, and how fast it dies away when neither is.
const PRM_BODY_RADIUS: usize = 0x08;
/// How far from her a character still has its frame, and how far it is still
/// drawn (0x1065EC, 0x10673C: 4000 and 3000); and how near the camera a model
/// may be and be drawn until a script says (0x118218: 150).
const PRM_FRAMES_WITHIN: usize = 0x0c;
const PRM_DRAWN_WITHIN: usize = 0x0e;
const PRM_TOO_CLOSE: usize = 0x3c;
/// The frames one stands before it idles, and those its idling goes round for:
/// each the least, with the most two bytes on (0xF60C4).
const PRM_IDLE_AFTER: usize = 0x18;
const PRM_IDLE_ROUND: usize = 0x1c;
/// The frames of the game a turn's animation is come to over (0xF4C6C).
const TURN_BLEND_FRAMES: f32 = 8.0;
const PRM_CAMERA_TURN_GAIN: usize = 0x2c;
const PRM_CAMERA_TURN_REVERSE: usize = 0x30;
const PRM_CAMERA_TURN_DECAY: usize = 0x34;
/// The fastest the camera turns, in radians per logic step (7 degrees; a constant in
/// the original's camera code, at 0xDEFE0).
const CAMERA_MAX_TURN: f32 = 0.122173;

/// The size of the camera for the walls that keep it in (0xDC480): it is a sphere
/// swept from what it looks at to where it would stand (0xDB6A4), against the second
/// of a map's two collision files alone (0x1110D4), and stops where it touches.
const CAMERA_RADIUS: f32 = 10.0;

/// How close to the point it looks at the camera can be and still look at it
/// (0xDDA4C).
const CAMERA_LOOK_FROM: f32 = 100.0;

// What the port adds: the mouse and the right stick tilt it and bring it nearer or
// further.
const CAMERA_ZOOM: std::ops::RangeInclusive<f32> = 0.3..=6.0;
const CAMERA_MAX_TILT: f32 = 1.3;
/// How far the port's one lens reaches: as far as the lens the original draws
/// what stands far off through (0xD9E1C: from 50 out to 200000).
const CAMERA_FAR: f32 = 200_000.0;
/// And from how near it sees: as near as the lens the original draws effects
/// through (the camera's + 0x150: from 1 out to the scene's far figure). The
/// scene and its characters are drawn by the scene's own lens, so what draws
/// them leaves out what is nearer than its near figure and further than its far
/// one (`end_the_map`, `light_of`); effects are drawn from here on.
const CAMERA_NEAR: f32 = 1.0;
/// The shape of the screen a scene's cuts were made for: the PSP's.
const SCREEN_ASPECT: f32 = 480.0 / 272.0;

/// How far above and below a starting point to look for ground.
const START_SEARCH: f32 = 5000.0;
/// Points tried along each side of the map when looking for open ground to start on.
const START_GRID: usize = 48;

/// The flags of the enemies beaten on a stage, which are cleared when the game goes
/// on to a map of another stage (0x115EDC).
const STAGE_FLAGS: std::ops::Range<i32> = 3000..3586;

/// The most seconds of a battle that count towards a mission's clock, and the time
/// units a second is (0xD8A14).
const BATTLE_SECONDS_MOST: i32 = 10_800;
const TIME_PER_SECOND: u32 = 0xf000;

/// The sounds a battle begins with, of the bank that is always there (0x11ACDC):
/// the second for the groups she strikes her pose over when they are beaten (the
/// group's flag 2, 0x93154).
const SOUND_BATTLE: u32 = 0x2b;
const SOUND_BATTLE_OF_NOTE: u32 = 0x2c;
/// What is heard when start cannot bring the pause up (0x10D9B8), of the same
/// bank: "no".
const SOUND_REFUSED: u32 = 7;

/// The colour the screen is cleared to before a map has said its own (0x11170C).
pub const BACKDROP: Color = Color::linear_rgb(scenery::BACKDROP_UNSET[0], scenery::BACKDROP_UNSET[1], scenery::BACKDROP_UNSET[2]);

/// The colour a scene has the screen cleared to (0x10EA90), as the game's own
/// numbers.
fn backdrop_of(scenery: &Scenery) -> ClearColor {
    let [red, green, blue] = scenery.backdrop;
    ClearColor(brs_engine::texture::values(Color::srgb(red, green, blue)))
}

/// Everything the field needs, loaded before the app starts so that a missing or
/// unreadable file is reported plainly instead of from inside a system.
#[derive(Resource)]
pub struct FieldSource {
    /// The map the game starts on, until the field takes it over.
    first: Option<MapData>,
    tables: FieldTables,
    /// The scripts that run the enemies on the field, and the field's own, likewise.
    brains: Option<Brains>,
    system: Option<SystemScript>,
    camera_turn: CameraTurn,
    body: Body,
    start: Option<Vec2>,
    options: Options,
    /// Skills she has from the start, besides what the game gives her.
    has: Vec<i32>,
    /// Flags of the save that are set from the start, and variables of it with
    /// what each is.
    flags: Vec<i32>,
    variables: Vec<(i32, i32)>,
}

/// How a run of the field departs from the game as it is played.
#[derive(Clone, Copy, Default)]
pub struct Options {
    /// Story scenes take the time they take, with nothing to see, rather than being
    /// skipped as soon as they can be.
    pub play_events: bool,
    /// Battles are not fought: each counts as won the moment it is asked for. For
    /// checking the field's own flow unattended.
    pub skip_battles: bool,
    /// A battle against this enemy group is not fought either, and counts as lost:
    /// for checking what the field does about a mission failed.
    pub lose_battle: Option<i32>,
    /// The battle against this enemy group is fought, in a run in which battles
    /// are not.
    pub fight: Option<i32>,
    /// The mission the game starts on instead of a new game's first, begun as it is
    /// when it is chosen from the list; nothing before it counts as done.
    pub mission: Option<i32>,
    /// The windows a script puts up are taken as read, and its questions as
    /// answered with what the cursor starts on. For unattended runs.
    pub skip_messages: bool,
    /// The run begins on the screen between missions, as it is come to when
    /// the mission the run would have begun has been cleared.
    pub stage_select: bool,
    /// The run begins on the title, as the game does.
    pub title: bool,
    /// The game counts as one that has been cleared (the system data's bit that
    /// 0x394 reads): the title shows it.
    pub cleared: bool,
}

/// The camera's turning rates, in radians per logic step.
#[derive(Clone, Copy)]
struct CameraTurn {
    gain: f32,
    reverse: f32,
    decay: f32,
}

/// A field map as its bundle has it: what is drawn, what is walked on, and the script
/// that sets it up and runs it.
struct MapData {
    /// Its number in the game's list.
    number: i32,
    pieces: Vec<(stcm::Map, Vec<u8>)>,
    collisions: Vec<(clim::Collision, Layer)>,
    /// The paths its camera can go along, by their numbers.
    paths: Vec<Option<Arc<CameraPath>>>,
    /// What its scene is like: its fog.
    scenery: Scenery,
    script: Option<MapScript>,
}

/// The paths the camera of the map that is up can go along (slot 8 of the map's
/// bundle), by the numbers scripts know them by.
#[derive(Resource, Default)]
struct CameraPaths(Vec<Option<Arc<CameraPath>>>);

impl MapData {
    fn load(data: &GameData, tables: &FieldTables, number: i32) -> Result<MapData> {
        let entry = usize::try_from(number).ok().and_then(|number| tables.maps.get(number));
        let entry = entry.with_context(|| format!("the game's list has no map {number}"))?;
        let bundle = format!("FLD/MAP/MAP_{}.BIN/11.bin", entry.name.to_ascii_uppercase());
        let mut pieces = vec![data.map(format!("{bundle}/01.bin/00.stcm"))?];
        // A second, small piece of geometry accompanies most maps.
        let extra = format!("{bundle}/01.bin/01.stcm");
        if data.exists(&extra) {
            pieces.push(data.map(extra)?);
        }
        // Slot 3 is the physical world, slot 6 the walls fencing in the playable area.
        let mut collisions = Vec::new();
        for (slot, layer) in [("03.clim", Layer::World), ("06.clim", Layer::Boundary)] {
            let path = format!("{bundle}/{slot}");
            if data.exists(&path) {
                collisions.push((data.collision(path)?, layer));
            }
        }
        // Slot 8 has the paths the camera can go along, each a bundle of its own.
        let mut paths = Vec::new();
        let folder = format!("{bundle}/08.bin");
        if data.has_folder(&folder) {
            for name in data.list(&folder)? {
                let Some(path) = name.split('.').next().and_then(|stem| stem.parse::<usize>().ok()) else { continue };
                match data.camera_path(format!("{folder}/{name}")) {
                    Ok(found) => {
                        paths.resize(paths.len().max(path + 1), None);
                        paths[path] = Some(found);
                    }
                    Err(error) => warn!("path {path} of the map's camera cannot be read: {error:#}"),
                }
            }
        }
        let scenery = Scenery::load(data, &bundle).with_context(|| format!("the scene of map {number}"))?;
        info!("map {number} is {}: {}, {scenery:?}", entry.name, entry.place);
        Ok(MapData { number, pieces, collisions, paths, scenery, script: host::load(data, &entry.name)? })
    }

    /// Puts the map into the world in place of whatever map was there: its geometry
    /// under the field's root, its collision into the collision world.
    fn build(&self, commands: &mut Commands, root: Entity, world: &mut CollisionWorld, stores: &mut AssetStores) {
        // The pieces are drawn one after the other, each in its list's order: the
        // second of the bundle first, which is what is far off (0x10EC28).
        let mut turn = 0;
        let far_off = self.pieces.iter().skip(1).map(|piece| (piece, map::Piece::FarOff));
        for ((piece, data), which) in far_off.chain(self.pieces.first().map(|piece| (piece, map::Piece::Scene))) {
            for entity in map::spawn(commands, piece, data, Transform::IDENTITY, which, &mut turn, stores).entities {
                commands.entity(entity).insert((MapPiece, ChildOf(root)));
            }
        }
        world.clear();
        for (collision, layer) in &self.collisions {
            world.add(collision, *layer);
        }
        commands.insert_resource(CameraPaths(self.paths.clone()));
        commands.insert_resource(self.scenery);
    }
}

/// Gives the camera the fog of the map that is up, or takes it from it (0x10E6A0),
/// and has the screen cleared to the scene's colour. The colours are the game's
/// own numbers, as everything of the scene is.
fn fog_of_the_map(scenery: Res<Scenery>, mode: Res<State<Mode>>, cameras: Query<Entity, With<FollowCamera>>, mut commands: Commands) {
    if !scenery.is_changed() {
        return;
    }
    if *mode.get() == Mode::Field {
        commands.insert_resource(backdrop_of(&scenery));
    }
    for camera in &cameras {
        match scenery.fog {
            Some(fog) => {
                let [red, green, blue] = fog.colour;
                commands.entity(camera).insert(DistanceFog {
                    color: brs_engine::texture::values(Color::srgb(red, green, blue)),
                    falloff: FogFalloff::Linear { start: fog.near, end: fog.far },
                    ..default()
                });
            }
            None => {
                commands.entity(camera).remove::<DistanceFog>();
            }
        }
    }
}

impl FieldSource {
    /// Loads the field, to start on the map named e.g. `STG01_01`, or on the map of
    /// the mission the options name. The five maps that are only scenes are
    /// named whole, e.g. `E_STG00_05`.
    pub fn load(data: &GameData, stage: &str, start: Option<Vec2>, options: Options) -> Result<FieldSource> {
        let stage = stage.to_ascii_lowercase();
        let name = if stage.starts_with("e_") { stage } else { format!("d_{stage}") };
        let tables = FieldTables::load(data)?;
        let map = match options.mission {
            Some(mission) => {
                let named = tables.mission_title(mission).is_some_and(|title| !title.is_empty());
                let start = tables.mission_start(mission).filter(|_| named);
                start.with_context(|| format!("the game has no mission {mission}"))?.0
            }
            None => tables.map_number(&name).with_context(|| format!("the game's list has no map {name}"))? as i32,
        };
        let first = Some(MapData::load(data, &tables, map)?);
        let degrees = |offset: usize| -> Result<f32> { Ok(tables.tunables.f32(offset)?.to_radians()) };
        let camera_turn = CameraTurn {
            gain: degrees(PRM_CAMERA_TURN_GAIN)?,
            reverse: degrees(PRM_CAMERA_TURN_REVERSE)?,
            decay: degrees(PRM_CAMERA_TURN_DECAY)?,
        };
        let body = motion::body(tables.tunables.u16(PRM_BODY_RADIUS)? as f32);
        let brains = Some(Brains::load(data)?);
        let system = Some(SystemScript::load(data, &tables)?);
        Ok(FieldSource { first, tables, brains, system, camera_turn, body, start, options, has: Vec::new(), flags: Vec::new(), variables: Vec::new() })
    }

    /// The bundle of one of the field's characters, by its number in the list
    /// of them: a suit she wears is one.
    pub fn character_bundle(&self, number: i32) -> Option<String> {
        self.tables.character_bundle(number)
    }

    /// The same, with her having these skills from the start: the game gives
    /// them out as it goes, and a run that is to show them cannot wait for that.
    pub fn having(mut self, skills: Vec<i32>) -> FieldSource {
        self.has = skills;
        self
    }

    /// The same, with these flags of the save set and these variables of it
    /// given from the start: for a run that is to show what a game comes to
    /// only after hours, such as records cleared.
    pub fn with_save(mut self, flags: Vec<i32>, variables: Vec<(i32, i32)>) -> FieldSource {
        (self.flags, self.variables) = (flags, variables);
        self
    }
}

/// The field as it stands: the scene and the script that runs it.
#[derive(Resource)]
pub struct FieldState {
    pub scene: Scene,
    script: Option<MapScript>,
    /// The characters the player is in touch with: one does not call the script
    /// again until she has been away from it.
    touching: BTreeSet<i32>,
    /// What she is in touch with, characters and traps, in the order she came
    /// to them (the list at the field's + 0x1DF8): the sign of what a button
    /// would do is for the first of them a button does anything with.
    in_touch: Vec<By>,
    /// The characters whose behaviour has asked for their event in this frame.
    asked: Vec<i32>,
    /// The character whose event the script is running, if it is one's.
    event_of: Option<i32>,
    body: Body,
    skip_battles: bool,
    lose_battle: Option<i32>,
    fight: Option<i32>,
    skip_messages: bool,
    /// The player's option that a scene's captions go on of themselves, as it
    /// was when the script last had its frame.
    auto_skip: bool,
    /// Events are skipped as soon as they can be.
    skip_events: bool,
    /// The opening event is still running.
    opening: bool,
    /// Where the player is put when the opening event is over, if she was asked to
    /// be put somewhere.
    start: Option<Vec3>,
    /// And which way she is to face then, and the camera's record, of a game
    /// loaded on the field.
    start_view: Option<(Option<f32>, Option<camera::Record>)>,
    /// How many times the script has set its camera, as last seen.
    camera_set: u32,
    /// An event has run since the camera last looked at which traps she is in: it
    /// takes what they have for it at once then (0xE8C6C).
    event_over: bool,
    /// Where the player was a frame ago: the system script goes by how far she has
    /// moved (the field's + 0x2150).
    last_position: Vec3,
    /// The field has been left: the screen between missions is up, on which
    /// the next is chosen.
    hub: Option<Hub>,
    /// The title is up: there is no game yet, or the one there was has been
    /// left for it.
    title: Option<Title>,
    /// The list of saves is up for something of the field: a script, or the
    /// window of a mission failed.
    saves_for: Option<SavesFor>,
    /// The game counts as cleared, for the title.
    cleared: bool,
    /// The map and entrance the field is to be started on anew: a mission has been
    /// chosen, or is being tried again.
    fresh: Option<(i32, i32)>,
    /// The field has been left for the ride on the bike: where it is to be
    /// started anew when the ride is over.
    riding: Option<(i32, i32)>,
}

impl FieldState {
    /// The screen between missions, while it is up.
    pub fn hub(&self) -> Option<&Hub> {
        self.hub.as_ref()
    }

    /// The title, while it is up.
    pub fn title(&self) -> Option<&Title> {
        self.title.as_ref()
    }

    fn busy(&self) -> bool {
        self.script.as_ref().is_some_and(MapScript::busy)
    }

    /// The field is taken up again after a battle. The original makes its camera
    /// anew then, from what the script had the old one at (0xEFB60, 0xE8A84), and
    /// finds the traps she is in again as after an event.
    fn back_from_battle(&mut self) {
        self.scene.camera = self.scene.camera.resumed();
        self.event_over = true;
    }
}

/// On the entity everything of the field hangs from, so that it can be put out of
/// sight for a battle.
#[derive(Component)]
struct FieldRoot;

/// On every piece of the map's geometry, which goes when the map is left.
#[derive(Component)]
struct MapPiece;

/// On the model of a character of the scene: which it is, the resource it was
/// made of, and for the player's the suit and the weapon it was made with (the
/// suit as the character of the field's list it is, the weapon as its item).
#[derive(Component)]
struct Actor {
    id: i32,
    chr: i32,
    worn: (i32, i32),
    /// How many animations its character had been set playing when it was
    /// last seen to, and how many faces it had been given.
    motions: u32,
    expressions: u32,
    /// What its neck has come to, and that its head was last seen to.
    neck: NeckPose,
    headed: bool,
    /// What it holds of an event's: the file each was made of, its model, and
    /// whether that is put away.
    props: [Option<(String, Entity, bool)>; scene::PROPS],
}

/// On the model of the weapon the player carries.
#[derive(Component)]
struct FieldWeapon;

/// The node of a character's model that the mark of its having noticed her is
/// put over (0xFD88C).
const MARK_NODE: u8 = 4;

/// On the model of a character other than her: the node of it that its mark is
/// put over, if the model has it.
#[derive(Component)]
struct MarkNode(Option<Entity>);

/// The node of a character's model that its shadow is under (0xFDDA4: the one
/// numbered nought), which goes where the model's animation takes it.
const ROOT_NODE: u8 = 0;

/// On the model of a character: that node, if the model has it.
#[derive(Component)]
pub struct RootNode(pub Option<Entity>);

/// On the camera the field is seen through. Where it is and how it looks are the
/// figures scripts keep with the scene; what is here is the rest.
#[derive(Component)]
pub struct FollowCamera {
    /// What the port's own controls have added: how much higher it stands, in
    /// radians, and how many times further off.
    tilt: f32,
    zoom: f32,
    /// How fast the L and R buttons are turning it, in radians per logic step.
    spin: f32,
    turn: CameraTurn,
    /// How a camera that follows her gets to its place, and how far from it that
    /// left it at each of the last two frames of the game.
    lag: Lag,
    behind: [Vec3; 2],
    /// How often the camera has cut to its place, as last seen; and how often
    /// it has been made anew or taken up again.
    cuts: u32,
    renewed: u32,
}

/// The angle from top to bottom of a view that is so many degrees from side to
/// side on the PSP's screen, in radians: the camera's figure is the width (0x239BC
/// makes the projection of it, and of the shape of the screen for the height).
fn view_height(width: f32) -> f32 {
    2.0 * ((width.to_radians() * 0.5).tan() / SCREEN_ASPECT).atan()
}

/// What a mission's progress stands at once the system script has it cleared
/// (`mission_clear`).
const MISSION_CLEARED: i32 = 200;

pub fn plugin(app: &mut App) {
    hud::plugin(app);
    map_screen::plugin(app);
    effects::plugin(app);
    app.init_resource::<CollisionWorld>()
        .init_resource::<CharacterAssets>()
        .init_resource::<CameraPaths>()
        .init_resource::<Scenery>()
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            (
                run_script,
                show_records,
                take_pictures,
                leave_field,
                change_map,
                control_player,
                (reach_characters, step_characters).chain(),
                step_faces,
                notice_touches,
                update_player,
                light_characters,
                step_glare,
                load_characters,
                (show_characters, tails::swing).chain(),
                effects::systems(),
                spin_camera,
                start_battle,
                sound_steps,
                sound_ambience,
                hand_sounds_on,
            )
                .chain()
                .before(EngineSystems::Animate)
                .run_if(in_state(Mode::Field).and_then(|menu: Res<Menu>, paused: Res<Paused>, map: Res<MapUp>| !menu.is_open() && !paused.up() && !map.up())),
        )
        .add_systems(FixedUpdate, run_map.before(run_pause).before(EngineSystems::Animate).run_if(in_state(Mode::Field)))
        .add_systems(FixedUpdate, run_menu.before(run_script).before(EngineSystems::Animate).run_if(in_state(Mode::Field)))
        .add_systems(FixedUpdate, run_pause.before(ask_saves).before(run_menu).before(EngineSystems::Animate).run_if(in_state(Mode::Field)))
        .add_systems(FixedUpdate, run_title.after(leave_field).before(change_map).run_if(in_state(Mode::Field)))
        .add_systems(FixedUpdate, (ask_saves, after_saves).chain().before(leave_field).before(run_title).before(run_script).run_if(in_state(Mode::Field)))
        .add_systems(FixedUpdate, (count_time, mark_cleared))
        .add_systems(Update, (follow_camera, fog_of_the_map, draw_fade).run_if(in_state(Mode::Field)))
        .add_systems(Update, (show_glare, open_gallery))
        .add_systems(Update, shadows::show.run_if(in_state(Mode::Field)))
        .add_systems(Update, end_the_map.run_if(in_state(Mode::Field)))
        .add_systems(FixedUpdate, end_battle.run_if(in_state(Mode::Battle)))
        .add_systems(OnEnter(Mode::Battle), leave_for_battle)
        .add_systems(OnExit(Mode::Battle), come_back)
        .add_systems(OnEnter(Mode::Ride), leave_for_battle)
        .add_systems(OnExit(Mode::Ride), come_back_from_the_ride)
        .add_systems(FixedUpdate, talk_of_the_ride.run_if(in_state(Mode::Ride).and_then(|paused: Res<Paused>| !paused.up())));
}

fn setup(
    mut commands: Commands,
    data: Res<GameData>,
    mut source: ResMut<FieldSource>,
    mut world: ResMut<CollisionWorld>,
    mut stats: ResMut<PlayerStats>,
    items: Res<Items>,
    mut inventory: ResMut<Inventory>,
    pad: Res<Pad>,
    assets: Res<CharacterAssets>,
    (records, options, title_assets): (Res<Records>, Res<crate::options::Options>, Res<TitleAssets>),
    mut stores: AssetStores,
) {
    let root = commands.spawn((FieldRoot, Transform::IDENTITY, Visibility::Inherited)).id();
    let Some(mut map) = source.first.take() else { return };
    map.build(&mut commands, root, &mut world, &mut stores);

    // The field makes the player; the map's script then puts her where she belongs
    // and sets up everything else.
    let defaults = host::defaults(&source.tables).unwrap_or_else(|error| {
        error!("the field's tunables could not be read: {error:#}");
        scene::Defaults { touch_radius: 120.0, territory: 2000.0, pace: 95, notice_range: 1800.0, wander_radius: 300.0 }
    });
    let mut scene = Scene::new(map.number, 0, defaults);
    scene.light_as(map.scenery.lights);
    scene.map_bank = host::map_bank(&data, &source.tables, map.number);
    let start = start_position(&world, source.start, &source.body);
    scene.characters.insert(PLAYER, Character::new(0, start, 0.0, &defaults));
    // A new game: the game's own start, the system script's part of it, and then
    // what choosing "new game" sets; after which the map is entered.
    let mut progress = Progress::start();
    // The records that are open from the start are opened (0x14B63C).
    let stage = source.tables.maps.get(map.number as usize).map_or(0, |entry| i32::from(entry.stage));
    if let Err(error) = records.initialize(&mut records::World { progress: &mut progress, stats: &mut stats, stage }) {
        error!("the records' script failed: {error:#}");
    }
    let mut system = source.system.take();
    if let Some(system) = &mut system {
        let mut world = system::World {
            progress: &mut progress,
            scene: &mut scene,
            stats: &mut stats,
            tables: &source.tables,
            ground: &world,
        };
        if let Err(error) = system.call("init_player", &[], &mut world) {
            error!("the system script failed: {error:#}");
        }
        // The save's backup is made for the first time (0x97C).
        records.back_up(world.progress);
        world.progress.begin_first_mission();
        // Another mission is begun the way choosing it from the list begins it; the
        // first has the story's opening just behind it, unless this is its map.
        match source.options.mission {
            Some(mission) => {
                // Whoever has the list of missions to choose from has cleared the
                // first, which cannot be done in fewer battles than the first of
                // the records asks for: she can dodge.
                records.take_as_cleared(records::FIRST, world.progress);
                match system.call("mission_select", &[mission as u32], &mut world) {
                    Ok(_) => world.scene.entrance = world.scene.values.get(&value::ENTRANCE).copied().unwrap_or(0),
                    Err(error) => error!("the system script failed: {error:#}"),
                }
            }
            None if map.number != OPENING_MAP => world.progress.after_the_opening(),
            // On the opening's own map she is as the game's own start has her,
            // whose script makes her a new game's when it is over.
            None => {
                if let Some(start) = source.tables.starts.get(Starts::OPENING) {
                    *world.stats = PlayerStats::NONE;
                    world.stats.start(start, &items);
                    *inventory = Inventory::default();
                }
            }
        }
        if let Err(error) = system.call("sys_fld_enter_map", &[], &mut world) {
            error!("the system script failed: {error:#}");
        }
    } else {
        progress.begin_first_mission();
    }
    for &skill in &source.has {
        progress.set_flag(battle::skills::HAS + skill, true);
    }
    for &flag in &source.flags {
        progress.set_flag(flag, true);
    }
    for &(variable, value) in &source.variables {
        progress.set_variable(variable, value);
    }
    // A run that begins on the title has no game yet: the map's script waits
    // for the one NEW GAME begins.
    let mut script = map.script.take().filter(|_| !source.options.title);
    let skip_events = !source.options.play_events;
    match &mut script {
        Some(script) => {
            // Until the script fades it in, the picture is dark.
            scene.fade = scene::Fade::DARK;
            script.skip_events = skip_events;
            script.skip_windows = source.options.skip_messages;
            script.auto_skip = options.auto_skip;
            let mut field = Field {
                progress: &mut progress,
                stats: &mut stats,
                scene: &mut scene,
                tables: &source.tables,
                world: &world,
                resources: &*assets,
                items: &items,
                inventory: &mut inventory,
                pad: &pad,
                records: &records,
            };
            if let Err(error) = script.open(&mut field) {
                error!("the map's script failed: {error:#}");
                scene.fade = scene::Fade::CLEAR;
            }
        }
        None if source.options.title => {}
        None => {
            info!("the map has no script: nothing is set up on it");
            // Nor its camera, which a script or a trap would make one that follows
            // her: the port does, rather than leave it where a map's camera starts.
            scene.camera.set_kind(kind::FOLLOW);
        }
    }

    commands.spawn((
        Camera3d::default(),
        brs_engine::texture::camera_of_values(),
        IsDefaultUiCamera,
        Projection::Perspective(PerspectiveProjection {
            fov: view_height(scene.camera.figure(figure::FIELD_OF_VIEW)),
            near: CAMERA_NEAR,
            far: CAMERA_FAR,
            ..default()
        }),
        FollowCamera {
            tilt: 0.0,
            zoom: 1.0,
            spin: 0.0,
            turn: source.camera_turn,
            lag: Lag::default(),
            behind: [Vec3::ZERO; 2],
            cuts: 0,
            renewed: 0,
        },
    ));

    if source.options.stage_select {
        progress.set_variable(variable::MISSION_PROGRESS, MISSION_CLEARED);
        scene.leaving = Some(scene::LEAVING_CLEARED);
    }
    // While the title is up the field counts as left, and nothing of it is
    // shown or moved.
    let title = source.options.title.then(|| Title::new(&title_assets, &source.tables, source.options.cleared, title_seed()));
    if title.is_some() {
        scene.leaving = Some(scene::LEAVING_CLEARED);
        scene.fade = scene::Fade::DARK;
    }
    commands.insert_resource(source.tables.clone());
    commands.insert_resource(progress);
    if let Some(brains) = source.brains.take() {
        commands.insert_resource(brains);
    }
    if let Some(system) = system {
        commands.insert_resource(system);
    }
    commands.insert_resource(FieldState {
        scene,
        script,
        touching: BTreeSet::new(),
        in_touch: Vec::new(),
        asked: Vec::new(),
        event_of: None,
        body: source.body,
        skip_battles: source.options.skip_battles,
        lose_battle: source.options.lose_battle,
        fight: source.options.fight,
        skip_messages: source.options.skip_messages,
        auto_skip: options.auto_skip,
        skip_events,
        opening: true,
        // A place given on the command line overrides the script's.
        start: source.start.map(|_| start),
        start_view: None,
        camera_set: 0,
        event_over: true,
        last_position: start,
        hub: None,
        title,
        saves_for: None,
        cleared: source.options.cleared,
        fresh: None,
        riding: None,
    });
}

/// What the title's random numbers start from: the clock, as the original's do.
fn title_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |since| since.as_nanos() as u64)
}

/// Her steps are heard: a foot comes down as her animation passes its frame, with
/// the sound of the ground it comes down on (0xFC954). It goes by where her
/// animation is before this frame moves it on.
fn sound_steps(mut state: ResMut<FieldState>, tables: Res<FieldTables>, actors: Query<(&Actor, &Animator)>) {
    let Some((_, animator)) = actors.iter().find(|(actor, _)| actor.id == PLAYER) else { return };
    let scene = &mut state.scene;
    let Some(player) = scene.player().filter(|player| !player.hidden() && !player.flags.has(flag::NO_STEPS)) else { return };
    if animator.paused {
        return;
    }
    let before = animator.body().time();
    let after = before + STEP_ANIMATION_FRAMES * animator.speed;
    let feet: Vec<usize> = steps::feet(player.status, before, after).collect();
    if feet.is_empty() {
        return;
    }
    // The kind of ground the ray down through her last found (0xFD548).
    let kind = player.lit.ground;
    let sounds = feet.into_iter().filter_map(|foot| tables.step_sound(kind, foot));
    scene.sounds.extend(sounds.map(Request::common));
}

/// How far over a character and how far under it the ray goes that finds what it
/// stands on and which light falls there (0xFD4B8, 0xFD4DC).
const MARKS_ABOVE: f32 = 300.0;
const MARKS_BELOW: f32 = 200.0;

/// A frame of how the characters are lit, by what the map's collision says of the
/// ground under each.
fn light_characters(mut state: ResMut<FieldState>, world: Res<CollisionWorld>) {
    state.scene.light_characters(|character, frames| {
        let marks = frames.lie(&world, character.group()).marks(character.position, MARKS_ABOVE, MARKS_BELOW);
        (marks.kind, marks.light)
    });
}

/// A frame of the glare over the scene's picture.
fn step_glare(mut state: ResMut<FieldState>) {
    state.scene.work.glare.step();
}

/// The glare is over the field's scene, and over nothing else: not a battle's,
/// nor her model in the menu, nor the screens the field is gone for.
fn show_glare(state: Option<Res<FieldState>>, mode: Res<State<Mode>>, menu: Res<Menu>, mut glare: ResMut<brs_engine::canvas::Glare>) {
    let of_the_field = state.filter(|state| *mode.get() == Mode::Field && !menu.is_open() && state.hub.is_none() && state.title.is_none());
    let level = of_the_field.map_or(0.0, |state| state.scene.work.glare.shown());
    if glare.0 != level {
        glare.0 = level;
    }
}

/// The light a character's model is drawn in (0x10F3B4): the light all about the
/// scene and its light's own share of that, its light's colour and its shine, all
/// as the character has them, and the way to its light; and how much of the
/// character there is, which goes with the first of those (0x10F608).
fn light_of(scene: &Scene, character: &Character) -> Lit {
    let by = &scene.lights.lights[usize::from(character.lit.light).min(scenery::LIGHTS - 1)];
    let own = &character.lit;
    Lit { on: true, ambient: own.colour(0) + own.colour(1), diffuse: own.colour(2), specular: own.colour(3), towards: by.towards(character.position), alpha: character.alpha, reach: Vec2::new(scene.camera.figure(figure::FAR), scene.camera.figure(figure::NEAR)) }
}

/// The sounds about the map are as loud as she is near them, and to the side they
/// are on as the camera looks (0xD0FC4): worked out every other frame.
fn sound_ambience(mut state: ResMut<FieldState>, camera: Single<&Transform, With<FollowCamera>>, mut frames: Local<u32>) {
    let scene = &mut state.scene;
    scene.ambience_level.step();
    *frames = frames.wrapping_add(1);
    let Some(listener) = scene.player().map(|player| player.position).filter(|_| *frames % 2 == 0) else { return };
    // The angle the camera stands at about what it looks at.
    let back = camera.back();
    scene.sound_ambience(listener, camera.translation, back.x.atan2(back.z));
}

/// What the scripts have asked of the sound in this frame goes to the game's sound.
fn hand_sounds_on(mut state: ResMut<FieldState>, mut board: ResMut<Board>) {
    let scene = &mut state.scene;
    if !(scene.sounds.is_empty() && scene.comm.sounds.is_empty()) {
        board.ask_all(scene.sounds.drain(..).chain(scene.comm.sounds.drain(..)));
    }
}

/// Where the player stands before the map's script has said: the requested spot if
/// there is ground under it, otherwise the most open piece of ground on the map.
fn start_position(world: &CollisionWorld, requested: Option<Vec2>, body: &Body) -> Vec3 {
    let on_ground = |x: f32, z: f32| {
        let probe = Vec3::new(x, 0.0, z);
        world.ground(probe, START_SEARCH, START_SEARCH).map(|g| Vec3::new(x, g.height, z))
    };
    if let Some(found) = requested.and_then(|p| on_ground(p.x, p.y)) {
        return found;
    }

    let corners = world.triangles().iter().flat_map(|t| t.corners);
    let (min, max) = corners.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let mut best = (f32::MIN, Vec3::ZERO);
    for i in 0..START_GRID {
        for j in 0..START_GRID {
            let fraction = |n: usize| (n as f32 + 0.5) / START_GRID as f32;
            let (x, z) = (min.x + (max.x - min.x) * fraction(i), min.z + (max.z - min.z) * fraction(j));
            if let Some(feet) = on_ground(x, z) {
                let clearance = world.wall_distance(feet + Vec3::Y * body.step_height);
                if clearance > best.0 {
                    best = (clearance, feet);
                }
            }
        }
    }
    best.1
}

/// A frame of the map's script, of what it has asked of the system script, and of
/// the picture's fade.
#[allow(clippy::too_many_arguments)]
fn run_script(
    pad: Res<Pad>,
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    assets: Res<CharacterAssets>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    items: Res<Items>,
    mut inventory: ResMut<Inventory>,
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    (records, options, pause_assets): (Res<Records>, Res<crate::options::Options>, Res<PauseAssets>),
    mut paused: ResMut<Paused>,
) {
    let state = &mut *state;
    // Once the field has been left for the screen between missions nothing
    // of it goes on: the original has done with it by then. Nor while the
    // title is up.
    if state.hub.is_some() || state.title.is_some() {
        return;
    }
    // Start over a scene that can be skipped brings the pause up, which offers
    // to (0xE8830, 0x10CDEC): the event has no frame then, nor until the pause
    // has gone.
    if pad.pressed & button::START != 0 && state.script.as_ref().is_some_and(MapScript::can_skip) {
        paused.0 = Some(Pause::new(&pause_assets, pause::Kind::Skippable, pause::entries(&records, &progress), false));
        return;
    }
    // What the buttons do to the window of a mission cleared comes before its
    // frame, which the system script's has (0x4A6C0); a run with no one at the
    // buttons takes it as read.
    if let Some(cleared) = &mut state.scene.cleared {
        cleared.take_buttons(&pad);
        if state.skip_messages {
            cleared.read();
        }
    }
    state.auto_skip = options.auto_skip;
    state.scene.fade.advance();
    let busy = state.script.as_ref().is_some_and(MapScript::busy);
    if !busy {
        state.event_of = None;
    }
    // Not while the field is still to be started anew: a game loaded from the
    // title or from another map has its place on the map it was saved on.
    if state.opening && !busy && state.fresh.is_none() {
        state.opening = false;
        if let Some(player) = state.scene.characters.get_mut(&PLAYER) {
            let (faced, view) = state.start_view.take().unwrap_or((None, None));
            if let Some(start) = state.start.take() {
                player.place(start, faced.unwrap_or(player.facing()));
            }
            info!("the player starts at {}", player.position);
            if let Some(view) = view {
                state.scene.camera = state.scene.camera.restored(&view);
            }
        }
    }
    if let Some(script) = &mut state.script {
        script.skip_events = state.skip_events;
        script.skip_windows = state.skip_messages;
        script.auto_skip = state.auto_skip;
        let mut field = Field {
            progress: &mut progress,
            stats: &mut stats,
            scene: &mut state.scene,
            tables: &tables,
            world: &world,
            resources: &*assets,
            items: &items,
            inventory: &mut inventory,
            pad: &pad,
            records: &records,
        };
        if let Err(error) = script.update(&mut field) {
            error!("the map's script failed: {error:#}");
            state.script = None;
        }
    }
    let mut world = system::World {
        progress: &mut progress,
        scene: &mut state.scene,
        stats: &mut stats,
        tables: &tables,
        ground: &world,
    };
    if let Err(error) = system.update(&mut world) {
        error!("the system script failed: {error:#}");
    }
    // The window of a mission cleared, just opened, is given its list (0x49FC8).
    if let Some(cleared) = &mut state.scene.cleared {
        cleared.list(|| cleared::entries(&records, &progress));
    }
    // The window of a mission failed has its frame after the scripts have had
    // theirs (0x115738; its task's number is above the field's).
    state.scene.step_failed(&pad);
}

/// The windows of the records that a map's script has asked for (`s276`), each
/// shown in its turn (0xEF790) and all of them then forgotten as news (0xEF8A0);
/// and whether a panel that tells of a record is up, which a script can wait for
/// (`s275`). With no one to read them the windows are taken as read.
#[allow(clippy::too_many_arguments)]
fn show_records(
    pad: Res<Pad>,
    tables: Res<FieldTables>,
    records: Res<Records>,
    news: Res<RecordNews>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    items: Res<Items>,
    mut inventory: ResMut<Inventory>,
    (fonts, colours): (Res<Fonts>, Res<TextColours>),
    mut window: ResMut<RecordWindow>,
    mut board: ResMut<Board>,
    mut state: ResMut<FieldState>,
    mut shown: Local<Option<records::Shown>>,
) {
    let state = &mut *state;
    state.scene.record_news = news.busy();
    if !state.scene.record_windows {
        return;
    }
    let stage = usize::try_from(state.scene.map).ok().and_then(|map| tables.maps.get(map)).map_or(0, |map| i32::from(map.stage));
    let showing = shown.get_or_insert_with(|| records::Shown::begin(records::OF_MISSIONS, None));
    let mut world = records::World { progress: &mut progress, stats: &mut stats, stage };
    let mut screen = records::Screen { window: &mut window, inventory: &mut inventory, items: &items, fonts: &fonts, colours: &colours, board: &mut board };
    let more = showing.step(&records, &mut world, &mut screen);
    let read = Pad { pressed: button::CROSS, held: button::CROSS, ..Pad::default() };
    window.step(if state.skip_messages { &read } else { &pad }, &mut board);
    if !more {
        records.shown(records::OF_MISSIONS, &mut progress);
        window.shut();
        *shown = None;
        state.scene.record_windows = false;
    }
}

/// Has the picture of the screen taken that a script has asked for (the field's
/// flag 0x17: 0x10F358 takes one of the scene before the field's own interface is
/// drawn, 0x10EA18 one of all the field has on the screen), and notes when it has
/// come: the graphics card hands it over a frame or two later, and the script
/// waits for that.
fn take_pictures(
    mut commands: Commands,
    mut state: ResMut<FieldState>,
    mut captured: ResMut<Captured>,
    screen: Res<Screen>,
    offscreen: Option<Res<Offscreen>>,
) {
    match state.scene.capture {
        scene::Capture::Wanted { whole } => {
            let of = if whole { all_of_the_screen(offscreen.as_deref()) } else { screen.alone() };
            captured.ask(&mut commands, of);
            state.scene.capture = scene::Capture::Pending;
        }
        scene::Capture::Pending if !captured.pending() => state.scene.capture = scene::Capture::None,
        _ => {}
    }
}

/// What the system script does every frame the field is in its normal state: the
/// enemies that pop up, and the mission's clocks (0x10DBA4).
fn update_player(
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    mut steps: Local<u32>,
) {
    let state = &mut *state;
    let Some(at) = state.scene.player().map(|player| player.position) else { return };
    let moved = at.distance(state.last_position);
    state.last_position = at;
    *steps += 1;
    if steps.is_multiple_of(STEPS_PER_SECOND as u32) {
        let doing = state.scene.player().map(|player| (player.action, player.status, player.hidden()));
        debug!("she is at {at:.0} on map {}; what she does, her status and whether she is hidden: {doing:?}", state.scene.map);
    }
    let normal = !state.script.as_ref().is_some_and(MapScript::busy)
        && state.scene.battle.is_none()
        && state.scene.leaving.is_none();
    if !normal {
        return;
    }
    let mut world = system::World {
        progress: &mut progress,
        scene: &mut state.scene,
        stats: &mut stats,
        tables: &tables,
        ground: &world,
    };
    if let Err(error) = system.frame(Frame::of(moved), &mut world) {
        error!("the system script failed: {error:#}");
    }
}

/// What follows the field being left (`s173`, 0xE4B40): the system script is told
/// how it was left; a mission that is tried again starts over; otherwise the
/// screen between missions comes (`hub`), and has its frames here until a
/// mission is chosen on it. Either way the field then starts anew on the map
/// the system script names.
#[allow(clippy::too_many_arguments)]
fn leave_field(
    pad: Res<Pad>,
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    (assets, title_assets, fonts, colours): (Res<HubAssets>, Res<TitleAssets>, Res<Fonts>, Res<TextColours>),
    (records, items, mut inventory): (Res<Records>, Res<Items>, ResMut<Inventory>),
    (mut record_window, mut board, mut menu): (ResMut<RecordWindow>, ResMut<Board>, ResMut<Menu>),
    (saves, mut desk, played): (Res<Saves>, ResMut<SaveDesk>, Res<Played>),
    (data, mut commands, mut next): (Res<GameData>, Commands, ResMut<NextState<Mode>>),
) {
    let state = &mut *state;
    let Some(how) = state.scene.leaving else { return };
    if state.fresh.is_some() || state.title.is_some() || state.riding.is_some() {
        return;
    }
    let mut field = system::World {
        progress: &mut progress,
        scene: &mut state.scene,
        stats: &mut stats,
        tables: &tables,
        ground: &world,
    };
    let Some(hub) = &mut state.hub else {
        info!("the field is left: {how}");
        match hub::left(how, &mut system, &mut field) {
            // The ride on the bike comes first (0xE6F8C), and the field starts
            // over when it has been ridden.
            Ok(Some(fresh)) if how == scene::LEAVING_ELSEWHERE => match crate::bike::course(&data, field.scene.work.ride) {
                Ok(course) => {
                    info!("the field is left for the ride on the bike, course {course}");
                    field.scene.hear_ambience(false);
                    board.ask_all(field.scene.sounds.drain(..));
                    commands.insert_resource(crate::bike::show::Course(course));
                    commands.insert_resource(crate::bike::show::Ridden(None));
                    // A run that passes battles over has the ride ridden for it.
                    if state.skip_battles {
                        commands.insert_resource(crate::bike::show::Unattended);
                    }
                    next.set(Mode::Ride);
                    state.riding = Some(fresh);
                }
                Err(error) => {
                    error!("the ride on the bike cannot be ridden, and is taken as ridden through: {error:#}");
                    state.fresh = Some(fresh);
                }
            },
            Ok(Some(fresh)) => state.fresh = Some(fresh),
            Ok(None) => {
                // The sounds about the map are not the screen's.
                field.scene.hear_ambience(false);
                board.ask_all(field.scene.sounds.drain(..));
                state.hub = Some(Hub::new(&tables));
            }
            Err(error) => {
                error!("the system script failed: {error:#}");
                // Rather than stay nowhere, the mission is begun again where it is.
                state.fresh = Some((field.scene.map, field.scene.entrance));
            }
        }
        return;
    };
    let mut around = hub::Around {
        assets: &assets,
        pad: &pad,
        fonts: &fonts,
        colours: &colours,
        system: &mut system,
        world: field,
        records: &records,
        record_window: &mut record_window,
        board: &mut board,
        items: &items,
        inventory: &mut inventory,
        menu: &mut menu,
        skip_messages: state.skip_messages,
    };
    let outcome = hub.step(&mut around);
    // Where the system script has said the field is to start anew, if a
    // mission was chosen.
    let (map, entrance) = hub::named(around.world.scene, value::MAP, value::ENTRANCE);
    // SAVE and LOAD put the list of saves up (0x5E548, 0x5E5E0), which
    // `after_saves` sees the end of. What is saved here is saved as of the
    // screen between missions (variable 300, 0x23E0).
    match hub.take_wanted() {
        Some(hub::Wanted::Save) => {
            let mut kept = around.world.progress.clone();
            kept.set_variable(variable::SAVED_AT, 1);
            let game = Saved { played: *played, progress: kept, stats: *around.world.stats, inventory: around.inventory.clone(), place: Place::StageSelect, view: None };
            desk.open(SaveScreen::save(&saves, game));
        }
        Some(hub::Wanted::Load) => desk.open(SaveScreen::load(&saves)),
        None => {}
    }
    board.ask_all(std::mem::take(&mut hub.sounds));
    match outcome {
        Some(hub::Outcome::Mission(mission)) => {
            info!("mission {mission} is begun on map {map}, by entrance {entrance}");
            state.fresh = Some((map, entrance));
            state.hub = None;
        }
        // The game is left for the title (0x5E118).
        Some(hub::Outcome::Title) => {
            state.hub = None;
            state.title = Some(Title::new(&title_assets, &tables, state.cleared, title_seed()));
        }
        None => {}
    }
}

/// What of the field has the list of saves up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SavesFor {
    Script,
    Failed,
    Pause,
}

/// Puts the list of saves up for the field (0x111420, 0x4F40 and 0x4FF8): when
/// a script asks for it, as a terminal's does (`s178`), and when "LOAD" is chosen
/// in the window of a mission failed. What is saved here is saved as of the
/// field (variable 300 is 0): the map, the entrance she came by and where she
/// stands.
fn ask_saves(
    saves: Res<Saves>,
    progress: Res<Progress>,
    stats: Res<PlayerStats>,
    inventory: Res<Inventory>,
    played: Res<Played>,
    mut desk: ResMut<SaveDesk>,
    mut state: ResMut<FieldState>,
) {
    let state = &mut *state;
    if state.saves_for.is_some() || state.hub.is_some() || state.title.is_some() {
        return;
    }
    let scene = &mut state.scene;
    if scene.failed.as_mut().is_some_and(failed::Failed::take_load) {
        desk.open(SaveScreen::load(&saves));
        state.saves_for = Some(SavesFor::Failed);
        return;
    }
    match scene.saves {
        Some(scene::SavesAsked::Save) => {
            let mut kept = progress.clone();
            kept.set_variable(variable::SAVED_AT, 0);
            let at = scene.player().map(|player| (player.position, player.facing()));
            let place = Place::Field { map: scene.map, entrance: scene.entrance, at };
            let view = Some(scene.camera.record());
            desk.open(SaveScreen::save(&saves, Saved { played: *played, progress: kept, stats: *stats, inventory: inventory.clone(), place, view }));
        }
        Some(scene::SavesAsked::Load) => desk.open(SaveScreen::load(&saves)),
        None => return,
    }
    state.saves_for = Some(SavesFor::Script);
}

/// What follows the list of saves when it has gone (0x5E5A0 and 0x5E638 on
/// the screen between missions, 0x11A310 on the title). With no game loaded
/// whoever put it up has the screen again. A game loaded takes the place of the
/// one there was, and by where it was saved (variable 300, 0x2400) the screen
/// between missions is made anew (0x5CEFC) or the field starts on the save's map
/// (0xE6CB0).
#[allow(clippy::too_many_arguments)]
fn after_saves(
    tables: Res<FieldTables>,
    title_assets: Res<TitleAssets>,
    mut desk: ResMut<SaveDesk>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    mut inventory: ResMut<Inventory>,
    mut played: ResMut<Played>,
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    mut board: ResMut<Board>,
    mut paused: ResMut<Paused>,
    (saves, options, opened): (Res<Saves>, Res<crate::options::Options>, Res<Opened>),
) {
    let Some(outcome) = desk.take() else { return };
    let state = &mut *state;
    // The system data is written after every save (0x51C0).
    if outcome == save_screen::Outcome::Saved {
        let cleared = played.game_cleared(state.cleared, &progress);
        if let Err(error) = saves.write_system(&crate::save::System { options: *options, cleared, gallery: *opened }) {
            error!("the system data was not kept: {error:#}");
        }
    }
    let save_screen::Outcome::Loaded(saved) = outcome else {
        match state.saves_for.take() {
            // The script that asked goes on (0x111464).
            Some(SavesFor::Script) => state.scene.saves = None,
            Some(SavesFor::Failed) => {
                if let Some(failed) = &mut state.scene.failed {
                    failed.load_closed();
                }
            }
            Some(SavesFor::Pause) => {
                if let Some(pause) = &mut paused.0 {
                    pause.load_closed();
                }
            }
            None => {
                if let Some(hub) = &mut state.hub {
                    hub.saves_closed();
                } else if let Some(title) = &mut state.title {
                    title.screen_done(&title_assets, false);
                }
            }
        }
        return;
    };
    state.saves_for = None;
    info!("a game is loaded: {:?}", saved.about());
    // Loaded from the pause (0x12DC88), the pause is done with, and what it held
    // still is let go as the field is left (0x12D474).
    if paused.0.take().is_some() {
        board.ask(Request::LetGo);
    }
    if let Some(title) = &mut state.title {
        title.loaded();
        board.ask_all(std::mem::take(&mut title.sounds));
    }
    let Saved { played: time, progress: kept, stats: hers, inventory: carried, place, view } = *saved;
    (*played, *progress, *stats, *inventory) = (time, kept, hers, carried);
    system.abandon();
    (state.title, state.hub) = (None, None);
    match place {
        Place::StageSelect => {
            // The field stays left, and the screen between missions comes.
            state.scene.leaving = Some(scene::LEAVING_CLEARED);
            state.scene.hear_ambience(false);
            board.ask_all(state.scene.sounds.drain(..));
            state.hub = Some(Hub::new(&tables));
        }
        Place::Field { map, entrance, at } => {
            state.fresh = Some((map, entrance));
            // She stands where she stood, once the map's script has set the map
            // up (0x1CFC).
            (state.start, state.opening) = (at.map(|(stood, _)| stood), true);
            // And faces as she faced, with the camera its record gives (0x1DC0).
            state.start_view = Some((at.map(|(_, faced)| faced), view));
        }
    }
}

/// The scene proper is drawn only as far as the camera's lens sees (its figure
/// 3, twenty thousand until a script says: the maps' scripts say 236 times, from
/// ten thousand to a hundred and fifty thousand). What is far off about the
/// scene has its own lens and is drawn whole.
fn end_the_map(state: Res<FieldState>, mut materials: ResMut<Assets<brs_engine::map::MapMaterial>>, mut last: Local<(f32, f32, usize)>) {
    let now = (state.scene.camera.figure(figure::NEAR), state.scene.camera.figure(figure::FAR), materials.len());
    if *last == now {
        return;
    }
    *last = now;
    // Only what changes is touched, so that nothing else is made anew for it.
    let ids: Vec<_> = materials.ids().collect();
    for id in ids {
        if materials.get(id).is_some_and(|material| material.clone().seen_between(now.0, now.1)) {
            if let Some(mut material) = materials.get_mut(id) {
                material.seen_between(now.0, now.1);
            }
        }
    }
}

/// The sixtieths of a second a frame of the game's logic is.
const SIXTIETHS_A_STEP: u32 = 2;

/// What the game's own flags have open of the gallery is open in the system data
/// too, and that is kept (0x19AC sets both as a record's reward is given; 0x1A34
/// has the system data follow a game that is loaded).
fn open_gallery(
    progress: Res<Progress>,
    gallery: Res<Gallery>,
    mut opened: ResMut<Opened>,
    (saves, options, played): (Res<Saves>, Res<crate::options::Options>, Res<Played>),
    state: Option<Res<FieldState>>,
) {
    if !progress.is_changed() || !opened.bypass_change_detection().take_from(&gallery, &progress) {
        return;
    }
    opened.set_changed();
    let cleared = played.game_cleared(state.is_some_and(|state| state.cleared), &progress);
    if let Err(error) = saves.write_system(&crate::save::System { options: *options, cleared, gallery: *opened }) {
        error!("what of the gallery is open was not kept: {error:#}");
    }
}

/// The time played goes on while a game is on (0x2A80).
fn count_time(state: Option<Res<FieldState>>, mut played: ResMut<Played>) {
    if state.is_some_and(|state| state.title.is_none()) {
        played.tick(SIXTIETHS_A_STEP);
    }
}

/// A script has said the game is cleared (`s205`, 0x340): the game there is has
/// been so many times, and the system data's bit is that number's lowest.
fn mark_cleared(state: Option<ResMut<FieldState>>, mut played: ResMut<Played>) {
    let Some(mut state) = state.filter(|state| state.scene.game_cleared.is_some()) else { return };
    let times = state.scene.game_cleared.take().unwrap_or(0);
    info!("the game is cleared: {times}");
    played.cleared = times as u8;
    state.cleared = times & 1 != 0;
}

/// The title's frames while it is up (`title`), and what follows NEW GAME
/// (0x11A1A8): the game's own start has cleared the save (0x13C, as the title is
/// launched), the first mission is set as begun, and the field starts on its
/// map.
#[allow(clippy::too_many_arguments)]
fn run_title(
    pad: Res<Pad>,
    data: Res<GameData>,
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    assets: Res<TitleAssets>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    (items, mut inventory, records): (Res<Items>, ResMut<Inventory>, Res<Records>),
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    (mut board, mut menu, menus): (ResMut<Board>, ResMut<Menu>, Res<MenuAssets>),
    (saves, mut desk, mut played): (Res<Saves>, ResMut<SaveDesk>, ResMut<Played>),
    (gallery, opened, mut images): (Res<Gallery>, Res<Opened>, ResMut<Assets<Image>>),
) {
    let state = &mut *state;
    let Some(title) = &mut state.title else { return };
    // The film the title plays when it is left alone is played as a script's
    // is: what plays films is told of it through the scene, and what it says
    // there of how far it has got is handed on to the title.
    if let (Some(showing), Some(played)) = (&mut title.showing, &state.scene.film) {
        (showing.ready, showing.over) = (played.ready, played.over);
    }
    let outcome = title.step(&assets, &pad);
    // A film of the gallery's is played the same way; the gallery has its
    // frames from `run_menu`.
    let of_the_gallery = menu.showing().map(|showing| {
        if let Some(played) = &state.scene.film {
            (showing.ready, showing.over) = (played.ready, played.over);
        }
        showing.clone()
    });
    let showing = title.showing.clone().or(of_the_gallery);
    if showing.is_some() || state.scene.film.is_some() {
        state.scene.film = showing;
    }
    board.ask_all(std::mem::take(&mut title.sounds));
    // OPTIONS is the menus' screen, up alone (0x119F5C): `run_menu` has its
    // frames, and tells the title when it is done with. LOAD GAME is the list
    // of saves (0x119F40), which `after_saves` sees the end of.
    match title.take_wanted() {
        Some(title::interface::Entry::Options) => menu.open_options(&menus),
        Some(title::interface::Entry::Gallery) => menu.open_gallery(&menus, &gallery, &opened),
        Some(title::interface::Entry::Load) => desk.open(SaveScreen::load(&saves)),
        _ => {}
    }
    // The picture the gallery has asked for is read for it.
    menu.read_picture(&data, &mut images, &gallery);
    let Some(title::Outcome::NewGame) = outcome else { return };
    state.title = None;
    *played = Played::default();

    // A new game starts on the map of the story's opening, by its entrance 0
    // (0x95C, 0x964): that map's script sees a new game and plays the opening,
    // which sets what it leaves set itself and goes on to the first mission.
    let map = OPENING_MAP;
    let Some(first) = tables.map_number(&format!("d_{}", FIRST_MAP.to_ascii_lowercase())) else {
        error!("the game's list has no map {FIRST_MAP}");
        return;
    };
    info!("a new game is begun on map {map}");
    *progress = Progress::start();
    // The game's own start gives her the start table's first record, with
    // every figure at its most (0x908 with 0xE10), and then empties what she
    // carries (0x1B78): the opening shows her so, and its script gives her
    // the figures and the things of a new game when its film is over (`s262`).
    *stats = PlayerStats::NONE;
    match tables.starts.get(Starts::OPENING) {
        Some(start) => stats.start(start, &items),
        None => error!("the start table has no record for the opening"),
    }
    *inventory = Inventory::default();
    let stage = tables.maps.get(first).map_or(0, |entry| i32::from(entry.stage));
    if let Err(error) = records.initialize(&mut records::World { progress: &mut progress, stats: &mut stats, stage }) {
        error!("the records' script failed: {error:#}");
    }
    system.abandon();
    let mut field = system::World { progress: &mut progress, scene: &mut state.scene, stats: &mut stats, tables: &tables, ground: &world };
    if let Err(error) = system.call("init_player", &[], &mut field) {
        error!("the system script failed: {error:#}");
    }
    records.back_up(&mut progress);
    progress.begin_first_mission();
    state.fresh = Some((map, 0));
}

/// Goes on to the map a script has named, once the event that named it is over
/// (0xE8C48, 0x115E88): the map and everything its script had set up go, the next
/// map is loaded in their place, and its script sets it up and brings her in by the
/// entrance she is coming through. The field itself goes on as it was: its figures,
/// the mission's display, the player.
#[allow(clippy::too_many_arguments)]
fn change_map(
    mut commands: Commands,
    data: Res<GameData>,
    tables: Res<FieldTables>,
    assets: Res<CharacterAssets>,
    mut world: ResMut<CollisionWorld>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    (items, records): (Res<Items>, Res<Records>),
    mut inventory: ResMut<Inventory>,
    pad: Res<Pad>,
    mut system: ResMut<SystemScript>,
    mut brains: ResMut<Brains>,
    mut state: ResMut<FieldState>,
    root: Single<Entity, With<FieldRoot>>,
    gone: Query<Entity, Or<(With<MapPiece>, With<Actor>, With<effects::OfField>, With<effects::OfCharacter>)>>,
    mut stores: AssetStores,
) {
    let state = &mut *state;
    // The field started anew does not wait for the script that left it.
    let fresh = state.fresh.take();
    let Some((number, entrance)) = fresh.or(state.scene.map_change) else { return };
    if fresh.is_none() && state.script.as_ref().is_some_and(MapScript::busy) {
        return;
    }
    // The screen as it is stays up while the next map is made ready, and the map
    // is not left until the picture of it has been taken (0x116048, 0x116334).
    if fresh.is_none() && state.scene.keep_screen() {
        return;
    }
    state.scene.map_change = None;
    let mut map = match MapData::load(&data, &tables, number) {
        Ok(map) => map,
        Err(error) => {
            error!("map {number} cannot be gone to: {error:#}");
            return;
        }
    };
    for entity in &gone {
        commands.entity(entity).despawn();
    }
    map.build(&mut commands, *root, &mut world, &mut stores);

    // Going on to another stage clears the flags of the enemies beaten on this one.
    let stage = |number: i32| usize::try_from(number).ok().and_then(|number| tables.maps.get(number)).map(|map| map.stage);
    if fresh.is_none() && stage(state.scene.map) != stage(number) {
        for flag in STAGE_FLAGS {
            progress.set_flag(flag, false);
        }
    }

    let mut scene = match fresh {
        Some(_) => {
            system.abandon();
            state.scene.afresh(number, entrance)
        }
        None => state.scene.onward(number, entrance),
    };
    scene.light_as(map.scenery.lights);
    scene.map_bank = host::map_bank(&data, &tables, number);
    let start = start_position(&world, None, &state.body);
    if let Some(player) = scene.characters.get_mut(&PLAYER) {
        player.place(start, player.facing());
    }
    brains.clear();
    let mut field = system::World {
        progress: &mut progress,
        scene: &mut scene,
        stats: &mut stats,
        tables: &tables,
        ground: &world,
    };
    if let Err(error) = system.call("sys_fld_enter_map", &[], &mut field) {
        error!("the system script failed: {error:#}");
    }
    let mut script = map.script.take();
    if let Some(script) = &mut script {
        script.skip_events = state.skip_events;
        script.skip_windows = state.skip_messages;
        script.auto_skip = state.auto_skip;
        let mut field = Field {
            progress: &mut progress,
            stats: &mut stats,
            scene: &mut scene,
            tables: &tables,
            world: &world,
            resources: &*assets,
            items: &items,
            inventory: &mut inventory,
            pad: &pad,
            records: &records,
        };
        if let Err(error) = script.open(&mut field) {
            error!("the map's script failed: {error:#}");
            scene.fade = scene::Fade::CLEAR;
        }
    }
    if script.is_none() {
        scene.camera.set_kind(kind::FOLLOW);
    }
    state.last_position = scene.player().map_or(start, |player| player.position);
    state.scene = scene;
    state.script = script;
    state.touching.clear();
    state.in_touch.clear();
    state.asked.clear();
    state.event_of = None;
    state.camera_set = 0;
    state.event_over = true;
}

/// A frame of the game's logic with the menu up (0xF1BC4): the field has none of
/// its own, and what moves of it stands still. When the menu is done with, the
/// screen comes back from the dark it left it in (0xF1E10). The menu's screens
/// change her flags and her figures, which are only said to have changed when
/// one has.
#[allow(clippy::too_many_arguments)]
fn run_menu(
    pad: Res<Pad>,
    assets: Res<MenuAssets>,
    skills: Res<Skills>,
    mut menu: ResMut<Menu>,
    mut state: ResMut<FieldState>,
    mut board: ResMut<Board>,
    (mut progress, mut stats): (ResMut<Progress>, ResMut<PlayerStats>),
    (items, mut inventory): (Res<Items>, ResMut<Inventory>),
    (records, tables, mut options, title_assets, saves): (Res<Records>, Res<FieldTables>, ResMut<crate::options::Options>, Res<TitleAssets>, Res<Saves>),
    (gallery, opened, played): (Res<Gallery>, Res<Opened>, Res<Played>),
    mut animators: Query<&mut Animator>,
    mut held: Local<bool>,
) {
    let open = menu.is_open();
    if open != *held {
        // What the field animates is not stepped while the menu has the screen.
        for mut animator in &mut animators {
            animator.paused = open;
        }
        *held = open;
    }
    if open {
        let stage = usize::try_from(state.scene.map).ok().and_then(|map| tables.maps.get(map)).map_or(0, |map| i32::from(map.stage));
        let mut hers = menu::Hers {
            skills: &skills,
            items: &items,
            records: &records,
            stage,
            progress: progress.bypass_change_detection(),
            stats: stats.bypass_change_detection(),
            inventory: inventory.bypass_change_detection(),
            options: &mut options,
            gallery: &gallery,
            opened: &opened,
            changed: false,
        };
        if menu.step(&assets, &pad, &mut hers) {
            // The options are kept as the menus are left (0x5CEB4, 0x51C0).
            let cleared = played.game_cleared(state.cleared, hers.progress);
            if let Err(error) = saves.write_system(&crate::save::System { options: *hers.options, cleared, gallery: *opened }) {
                error!("the options were not kept: {error:#}");
            }
            // The screen between missions has its own way of coming back, and
            // so has the title.
            let state = &mut *state;
            if let Some(hub) = &mut state.hub {
                hub.menu_closed();
            } else if let Some(title) = &mut state.title {
                title.screen_done(&title_assets, true);
                if std::mem::take(&mut menu.tune_again) {
                    title.tune_again();
                }
            } else {
                state.scene.fade.begin(scene::fade_kind::IN, menu::BACK_FRAMES);
            }
        }
        if hers.changed {
            progress.set_changed();
            stats.set_changed();
            inventory.set_changed();
        }
    }
    board.ask_all(std::mem::take(&mut menu.sounds));
}

/// The pause's frames while it is up (0x10CB0C), and what follows it. While it
/// is up nothing of the field has a frame, and what the field animates stands
/// still: the original shows a picture of the screen as it was under it
/// (0x10CE64). When it is over what it held still goes on (0x12D144), and by
/// how it ended the game goes on as it was (0x10C998), the scene that was
/// playing is skipped, or the field is left as a mission given up (0x10CBF8).
/// A game loaded from it has taken the place of this one by then
/// (`after_saves`).
#[allow(clippy::too_many_arguments)]
fn run_pause(
    pad: Res<Pad>,
    assets: Res<PauseAssets>,
    saves: Res<Saves>,
    mut paused: ResMut<Paused>,
    mut desk: ResMut<SaveDesk>,
    mut state: ResMut<FieldState>,
    mut board: ResMut<Board>,
    mut animators: Query<&mut Animator>,
    mut held: Local<bool>,
) {
    let mut hold = |up: bool| {
        if up != *held {
            for mut animator in &mut animators {
                animator.paused = up;
            }
            *held = up;
        }
    };
    hold(paused.up());
    let Some(shown) = &mut paused.0 else { return };
    let outcome = shown.step(&assets, &pad);
    board.ask_all(shown.sounds.drain(..));
    // Square puts the list of saves up (0x12DBC4), which `after_saves` sees the
    // end of.
    if shown.take_load() {
        desk.open(SaveScreen::load(&saves));
        state.saves_for = Some(SavesFor::Pause);
    }
    let Some(outcome) = outcome else { return };
    let blacken = shown.blacken;
    paused.0 = None;
    hold(false);
    board.ask(Request::LetGo);
    let state = &mut *state;
    if blacken {
        state.scene.fade = scene::Fade::DARK;
    }
    match outcome {
        pause::Outcome::GoOn => {}
        pause::Outcome::Skip => {
            if let Some(script) = &mut state.script {
                script.skip_event(&mut state.scene);
            }
        }
        pause::Outcome::GiveUp => {
            info!("the mission is given up");
            state.scene.leaving = Some(scene::LEAVING_GIVEN_UP);
        }
    }
}

/// The map's frames while it is up (0xF31C0). Nothing of the field has a frame
/// meanwhile, and what the field animates stands still: the original shows a
/// picture of the screen as it was under it (0xF3388). When it has gone the
/// field goes on (0xF30A4).
fn run_map(pad: Res<Pad>, assets: Res<MapAssets>, mut map: ResMut<MapUp>, mut board: ResMut<Board>, mut animators: Query<&mut Animator>, mut held: Local<bool>) {
    let mut hold = |up: bool| {
        if up != *held {
            for mut animator in &mut animators {
                animator.paused = up;
            }
            *held = up;
        }
    };
    hold(map.up());
    let Some(shown) = &mut map.0 else { return };
    let gone = shown.step(&assets, &pad);
    board.ask_all(shown.sounds.drain(..));
    if gone {
        map.0 = None;
        hold(false);
    }
}

/// The stick moves the player: on the field she runs whenever it is pushed, the way
/// it is pushed as the camera sees it (0x105480). While the map's script is running
/// an event she is not the player's to move.
#[allow(clippy::too_many_arguments)]
fn control_player(
    pad: Res<Pad>,
    progress: Res<Progress>,
    (records, pause_assets): (Res<Records>, Res<PauseAssets>),
    (tables, map_assets): (Res<FieldTables>, Res<MapAssets>),
    mut menu: ResMut<Menu>,
    mut paused: ResMut<Paused>,
    mut map: ResMut<MapUp>,
    mut state: ResMut<FieldState>,
) {
    let busy = state.busy() || state.scene.battle.is_some() || state.scene.leaving.is_some() || state.scene.map_change.is_some();
    let steered = pad.seek.map(|wanted| sought(&state.scene, wanted));
    // The view the stick is taken by, which is the camera's but for a while
    // after a cut (0x101700).
    let yaw = state.scene.camera.stick_yaw().to_radians();
    state.scene.work.heavy = progress.flag(scene::HEAVY_GOING);
    let heavy = state.scene.work.heavy;
    let Some(player) = state.scene.player() else { return };
    // What she is doing is hers to finish, but that the stick takes her out of
    // a pull-up once she has been at it a while (0xF4864).
    let leaving = player.acting() && player.flags.has(flag::FREE_TO_GO);
    if busy || (player.acting() && !leaving) {
        return;
    }
    if leaving {
        let pushed = if pad.stick.length() > STICK_DEAD_ZONE { pad.stick } else { pad.dpad };
        let wanted = steered.map_or(pushed.length() > STICK_DEAD_ZONE, |way| way.is_some());
        let Some(player) = state.scene.characters.get_mut(&PLAYER).filter(|_| wanted) else { return };
        // It ends (0xF463C), and she sets off below as one that was doing nothing.
        player.end_action();
    }
    // Start brings the pause up (0x10D968), and that is the end of her frame;
    // in mission 44 it only says no, as triangle does there.
    if pad.pressed & button::START != 0 {
        if progress.variable(variable::SCENARIO) == menu::NO_MENU_SCENARIO {
            state.scene.sounds.push(Request::common(SOUND_REFUSED));
        } else {
            let no_stage_select = progress.flag(failed::NO_STAGE_SELECT);
            paused.0 = Some(Pause::new(&pause_assets, pause::Kind::Field, pause::entries(&records, &progress), no_stage_select));
            return;
        }
    }
    // Triangle brings the menu up (0x10D9FC), and that is the end of her frame.
    if pad.pressed & button::TRIANGLE != 0 {
        menu.ask(progress.variable(variable::SCENARIO));
        if menu.is_open() {
            return;
        }
    }
    // Square brings up the map of the stage the map is of (0x10DA90, 0xF3400),
    // and that is the end of her frame; a stage that has none has nothing come.
    if pad.pressed & button::SQUARE != 0 {
        let stage = usize::try_from(state.scene.map).ok().and_then(|map| tables.maps.get(map)).map_or(-1, |map| i32::from(map.stage));
        map.0 = MapScreen::open(&map_assets, &progress, stage);
        if map.up() {
            return;
        }
    }
    let seen_from = state.scene.camera.figures(figure::PLACE);
    let Some(player) = state.scene.characters.get_mut(&PLAYER) else { return };
    let pushed = if pad.stick.length() > STICK_DEAD_ZONE { pad.stick } else { pad.dpad };
    // Pushing the stick up runs away from the camera.
    let mut forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let mut right = Vec3::new(-forward.z, 0.0, forward.x);
    // In one of the field's frames that is turned, it is away from where the
    // camera is, over the ground as her frame has it. (The original works the
    // way out by her upright from the view's own turns, 0x101898; this stands
    // in for that where the upright is not the world's.)
    if player.lay != Quat::IDENTITY {
        let up = player.up();
        let away = player.position - seen_from;
        if let Some(over_ground) = (away - up * away.dot(up)).try_normalize() {
            (forward, right) = (over_ground, over_ground.cross(up));
        }
    }
    let direction = match steered {
        Some(way) => way,
        None => (pushed.length() > STICK_DEAD_ZONE).then(|| (right * pushed.x - forward * pushed.y).normalize()),
    };
    let Some(direction) = direction else {
        // Let go, she stands; or pulls up, if she has run a while (0x10560C).
        if player.status == player.move_status && player.move_status == status::RUN && player.ran > scene::BRAKE_AFTER {
            player.brake();
        } else {
            player.status = status::STAND;
        }
        return;
    };
    // She sets off in what she moves in, which is a trudge while the going is
    // heavy (0x1054D8).
    if player.status != player.move_status {
        player.move_status = moving_in(heavy);
        player.status = player.move_status;
        player.ran = 0;
    }
    // Sent back the way she came after a while of running, she pulls up first
    // (0x105580: the way wanted is behind the way she is going).
    if player.mover.direction.dot(direction) < 0.0 && player.ran > scene::BRAKE_AFTER && player.move_status == status::RUN {
        player.brake();
        return;
    }
    player.mover.direction = direction;
}

/// The status she moves in when the stick is pushed (0x1054D8).
fn moving_in(heavy: bool) -> i32 {
    if heavy { status::TRUDGE } else { status::RUN }
}

/// The way a scripted run that is steered has her run: to a character of the scene
/// wherever it has got to, to the nearest enemy that is about, or to a place; no way
/// at all once she is there, or if there is no such thing. Nothing of the original's.
fn sought(scene: &Scene, wanted: Seek) -> Option<Vec3> {
    let at = scene.player()?.position;
    let about = |id: i32| scene.characters.get(&id).filter(|character| !character.hidden()).map(|character| character.position);
    let target = match wanted {
        Seek::Enemy(id) => about(id as i32)?,
        Seek::Nearest => {
            let enemies = SYMBOLS.filter_map(about);
            enemies.min_by(|a, b| a.distance_squared(at).total_cmp(&b.distance_squared(at)))?
        }
        Seek::Point(x, z) => Vec3::new(x as f32, at.y, z as f32),
    };
    let way = (target - at).with_y(0.0);
    (way.length() > SEEK_REACH).then(|| way.normalize())
}

/// Which characters are out of her reach or of the camera's this frame.
fn reach_characters(mut state: ResMut<FieldState>, tables: Res<FieldTables>, camera: Single<&Transform, With<FollowCamera>>) {
    let figure = |at: usize| tables.tunables.u16(at).ok().map(f32::from);
    let (Some(frames_within), Some(drawn_within), Some(too_close)) = (figure(PRM_FRAMES_WITHIN), figure(PRM_DRAWN_WITHIN), figure(PRM_TOO_CLOSE)) else { return };
    state.scene.reach_characters(camera.translation, frames_within, drawn_within, too_close);
}

/// A frame of every character: those a behaviour script runs think and are moved by
/// what they think, the rest go on with what they were doing.
fn step_characters(
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    assets: Res<CharacterAssets>,
    mut brains: ResMut<Brains>,
    mut state: ResMut<FieldState>,
) {
    let state = &mut *state;
    // Under the window of a mission failed nothing of the field moves.
    if state.scene.held_by_failed() {
        return;
    }
    let normal = !state.script.as_ref().is_some_and(MapScript::busy)
        && state.scene.battle.is_none()
        && state.scene.leaving.is_none();
    state.scene.lay_characters();
    // What an event's script has told to come to where it stands comes.
    for (id, way) in std::mem::take(&mut state.scene.coming) {
        if let Some(character) = state.scene.characters.get_mut(&id) {
            let depth = motion::Resources::pop_height(&*assets, character.chr);
            character.come_by(way, depth);
        }
    }
    let frames = state.scene.frames.clone();
    let walls: Vec<Wall> = state.scene.walls.values().copied().collect();
    let solids: Vec<scene::Solid> = state.scene.characters.values().filter_map(|character| character.solid(state.body.radius)).collect();
    let ground = Ground { world: &world, body: &state.body, walls: &walls, solids: &solids, frames: Some(&frames) };
    let mut context = symbol::Context {
        scene: &mut state.scene,
        tunables: &tables.tunables,
        ground: &ground,
        resources: &*assets,
        normal,
        in_event: state.event_of,
        events: Vec::new(),
    };
    let thought = brains.update(&mut context);
    state.asked = context.events;
    for (id, character) in &mut state.scene.characters {
        if !thought.contains(id) {
            motion::step(character, &*assets, &ground);
        }
    }
    // One that stands with nothing to do idles after a while (0xF5F78, 0xF8FE0),
    // unless a script has nobody do so, or not this one (its flag 0x62,
    // 0xF9030): the waits are the tunables'.
    let nobody = state.scene.flag(scene::field_flag::NO_IDLING);
    let wait = |from: usize| Some((tables.tunables.u16(from).ok()? as i32, tables.tunables.u16(from + 2).ok()? as i32));
    if let (Some(stands), Some(goes_round)) = (wait(PRM_IDLE_AFTER), wait(PRM_IDLE_ROUND)) {
        for (id, character) in state.scene.characters.iter_mut().filter(|(_, character)| !character.asleep) {
            let (chr, was) = (character.chr, character.idle.playing());
            let quiet = nobody || character.flags.has(flag::NO_IDLING);
            character.idle_on(quiet, |motion| motion::Resources::motion_frames(&*assets, chr, motion), [stands, goes_round]);
            if let (None, Some((motion, _))) = (was, character.idle.playing()) {
                debug!("character {id} has stood a while and idles: animation {motion}");
            }
        }
    }
}

/// Looks for what the player has walked into, and starts the script's event for the
/// first thing found: a trap she has entered, a character she has come up to, or
/// one she is looking at (cross, standing by something there is to look at).
fn notice_touches(
    pad: Res<Pad>,
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    assets: Res<CharacterAssets>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    items: Res<Items>,
    mut inventory: ResMut<Inventory>,
    mut state: ResMut<FieldState>,
    records: Res<Records>,
) {
    let state = &mut *state;
    let Some(at) = state.scene.player().map(|player| player.position) else { return };
    let idle = !state.script.as_ref().is_some_and(MapScript::busy)
        && state.scene.battle.is_none()
        && state.scene.leaving.is_none();

    // The camera is told of the traps she has come into and left while she is the
    // player's to move (0x10D6EC), and of those she is in when an event is over
    // (0xE8C6C), when it takes what they have for it at once.
    if idle {
        let at_once = std::mem::take(&mut state.event_over);
        state.scene.walk_traps(at, at_once);
    } else {
        state.event_over = true;
    }

    // The system script has the field left (its value 56, the field's flag 0x58:
    // a mission's time has run out, or what she guards is lost). The field then
    // starts the map script's callback for a character with the number the
    // system script gave to say how (its value 57), and the map's script sees to
    // the rest (0x10D6BC to 0x10D6E8, 0x117E14 with 0x39, 0xE84A0). It comes
    // before whatever she has walked into.
    let leaves = idle && state.scene.flag(scene::field_flag::LEAVE);
    if leaves {
        state.scene.set_flag(scene::field_flag::LEAVE, false);
    }
    let left_by = leaves.then(|| Callback::Character(state.scene.values.get(&value::EXIT).copied().unwrap_or(0)));

    // An enemy that has caught her comes first.
    let mut found = std::mem::take(&mut state.asked).first().map(|&id| Callback::Character(id)).filter(|_| idle);
    let (facing, lay) = state.scene.player().map_or((0.0, Quat::IDENTITY), |player| (player.facing(), player.lay));
    let frames = state.scene.frames.clone();
    // What she is in touch with now (0x10DBEC): the traps she is in, turned as
    // they ask, and the characters she is by.
    let mut now: Vec<By> = Vec::new();
    for (&id, trap) in &mut state.scene.traps {
        let inside = trap.contains(at, &frames);
        // Walking into it calls the script, unless that is left to a button, or
        // the trap's own figure has it passed over (0x118484).
        if inside && !trap.inside && !trap.off && trap.on_touch && !trap.not_walked_into && idle && found.is_none() {
            debug!("the player has walked into trap {id}: {:?}", trap.kind);
            found = Some(Callback::Trap(id));
        }
        // A trap she is standing in when it is switched on fires when she next enters.
        trap.inside = inside;
        // Its sign, if it has one, has a frame (0x115704).
        if trap.sign.is_some() {
            trap.sign_frames = trap.sign_frames.wrapping_add(1);
        }
        if trap.touched(at, facing, &frames) {
            now.push(By::Trap(id));
        }
    }
    for (&id, character) in &state.scene.characters {
        // How far off it is over the ground and above or below, as her frame lies.
        let away = if lay == Quat::IDENTITY { character.position - at } else { lay.inverse() * (character.position - at) };
        let near = id != PLAYER
            && !character.hidden()
            && Vec2::new(away.x, away.z).length() < character.touch_radius
            && away.y.abs() < TOUCH_HEIGHT;
        if !near {
            state.touching.remove(&id);
            continue;
        }
        // Those a behaviour script runs ask for their event themselves, and she
        // is never in touch with them this way (0x10D1A4).
        if character.behaviour.is_none() {
            now.push(By::Character(id));
        }
        if !idle || found.is_some() {
            continue;
        }
        let touchable = character.flags.has(flag::TOUCHABLE) && character.behaviour.is_none();
        if touchable && state.touching.insert(id) {
            found = Some(Callback::Character(id));
        }
    }
    // The list keeps the order she came to them in.
    state.in_touch.retain(|by| now.contains(by));
    for by in now {
        if !state.in_touch.contains(&by) {
            state.in_touch.push(by);
        }
    }

    // The sign of what a button would do, and the button (0x10D700 to 0x10DB00).
    let hers = idle && found.is_none();
    if let Some(by) = action::frame(&mut state.scene, &state.in_touch, &pad, hers) {
        found = Some(match by {
            By::Character(id) => Callback::Character(id),
            By::Trap(id) => Callback::Trap(id),
        });
    }

    let (Some(callback), Some(script)) = (left_by.or(found), &mut state.script) else { return };
    debug!("the player has walked into {callback:?}{}", if left_by.is_some() { " (the system script has the field left)" } else { "" });
    if let Callback::Character(id) = callback {
        state.event_of = Some(id);
    }
    // An event takes the player out of the player's hands (0x1078A4).
    if let Some(player) = state.scene.characters.get_mut(&PLAYER) {
        player.stop();
    }
    let mut field = Field {
        progress: &mut progress,
        stats: &mut stats,
        scene: &mut state.scene,
        tables: &tables,
        world: &world,
        resources: &*assets,
        items: &items,
        inventory: &mut inventory,
        pad: &pad,
        records: &records,
    };
    if let Err(error) = script.call(callback, &mut field) {
        error!("the map's script failed: {error:#}");
        state.script = None;
    }
}

/// A frame of the characters' faces, which go by what the scene has spoken: that
/// it has begun to be heard is kept from when the sound begins it until a script
/// hushes it.
fn step_faces(mixer: Res<crate::sound::Mixer>, tables: Res<FieldTables>, mut state: ResMut<FieldState>) {
    let scene = &mut state.scene;
    // And whether the tune a script asked for is playing yet (`s177`).
    if !scene.tune_heard {
        scene.tune_heard = tables.tune(scene.tune).is_none_or(|tune| mixer.tune_begun(&crate::sound::tune_file(tune)));
    }
    let said = mixer.saying(host::STREAM_LINES);
    if said == Some(true) {
        scene.voice_begun = true;
    }
    let (begun, heard) = (scene.voice_begun, said.is_some());
    for character in scene.characters.values_mut() {
        if character.face.used {
            character.step_face(begun, heard);
        }
        // And of their necks, by the time each has of the frame (0xFEA30).
        let time = character.time();
        character.neck.step(time);
    }
}

/// The nodes of a model its face is moved by: the head, whose lie the two are
/// turned by, the lids and the jaw (0xFFEF4, 0x1001B8).
const HEAD_NODE: u8 = 4;
const LIDS_NODE: u8 = 5;
const JAW_NODE: u8 = 6;

/// A neck's turn as a model has it: how the head is last to be turned and how it
/// was to be turned when the neck was last started, both as the world has them
/// (the neck's + 0x140 and + 0x160), and which start of the neck's that was.
#[derive(Clone, Copy)]
struct NeckPose {
    to: Quat,
    from: Quat,
    starts: u32,
    /// The side a head that looks at something far round was last turned to
    /// (the character's flag 0x6D), none while it is to be chosen (flag 0x6C),
    /// and which saying of how the neck turns the head that goes with.
    side: Option<bool>,
    ways: u32,
}

impl Default for NeckPose {
    fn default() -> NeckPose {
        NeckPose { to: Quat::IDENTITY, from: Quat::IDENTITY, starts: 0, side: None, ways: 0 }
    }
}

/// How a model's head is turned in the model's own space with none of its
/// animation: the turns its file gives the head and what it hangs off (0x1E534,
/// 0xFECEC).
fn rest_turn(skeleton: &Skeleton, node: usize) -> Quat {
    let mut turn = Quat::IDENTITY;
    let mut at = Some(node);
    while let Some(index) = at {
        turn = skeleton.rest[index].rotation * turn;
        at = skeleton.parents[index];
    }
    turn.normalize()
}

/// Has a model's head be turned as its character's neck has it, and its lids and
/// jaw as the face has them (0x100164, 0xFFE9C and 0x1004F0, in that order after
/// the model's pose).
///
/// The lids and the jaw are each turned, where they are, about the line over the
/// ground along which the head's third axis lies. The neck then turns the head
/// and all that hangs off it, where the head is: by its angles about the head's
/// own axes as the animation has them (about the upright one first, then the one
/// across, then the one ahead), or to its angles from the way the body faces; on
/// its way from the turn it last had to that for as long as it is moving. One
/// that looks at something has the head's frame turned to `looked`, which is
/// where that is, as far as `limits` let it (see [`look`]).
fn pose_head(animator: &mut Animator, skeleton: &Skeleton, character: &Character, pose: &mut NeckPose, looked: Option<Vec3>, limits: [f32; 3]) {
    let Some(head) = skeleton.index_of(HEAD_NODE) else { return };
    let root = character.frame().rotation;
    // The head as its animation has it, without what the neck did last.
    animator.set_twist(head, Quat::IDENTITY);
    let placed = animator.model_space(skeleton, head);
    let posed = placed.rotation;

    let neck = &character.neck;
    if neck.starts != pose.starts {
        (pose.from, pose.starts) = (pose.to, neck.starts);
    }
    let mut turned = Quat::IDENTITY;
    if neck.on {
        // The head's frame with the turns of its file taken out of it: the
        // body's own axes as the head has them now.
        let frame = root * posed * rest_turn(skeleton, head).inverse();
        let by = |axis: Vec3, angle: f32| Quat::from_axis_angle(axis, angle);
        let about = |x: Vec3, y: Vec3, z: Vec3| by(z, neck.angles.z) * by(x, neck.angles.x) * by(y, neck.angles.y);
        pose.to = match neck.way {
            NeckWay::Own => about(frame * Vec3::X, frame * Vec3::Y, frame * Vec3::Z),
            NeckWay::FromBody => about(Vec3::X, Vec3::Y, Vec3::Z) * root * frame.inverse(),
            // At something: the frame of what the head hangs off, with the turns
            // of its file taken out likewise, is what the limits are from. With
            // nothing there to look at the head stays as it was to be.
            NeckWay::Point | NeckWay::Object => match looked {
                Some(point) => {
                    if neck.ways != pose.ways {
                        (pose.side, pose.ways) = (None, neck.ways);
                    }
                    let body = match skeleton.parents[head] {
                        Some(parent) => root * animator.model_space(skeleton, parent).rotation * rest_turn(skeleton, parent).inverse(),
                        None => root,
                    };
                    let at = character.frame().transform_point(placed.translation);
                    look(frame, at, body, point, limits, &mut pose.side) * frame.inverse()
                }
                None => pose.to,
            },
        };
        let now = if neck.moving { pose.from.slerp(pose.to, neck.part()) } else { pose.to };
        turned = (root.inverse() * now * root).normalize();
    }
    animator.set_twist(head, turned);

    let turns = [(LIDS_NODE, character.face.lids()), (JAW_NODE, character.face.jaw())];
    let axis = turns.iter().any(|&(_, turn)| turn != 0.0).then(|| {
        let lie = root * (posed * Vec3::Z);
        let way = lie.x.atan2(lie.z);
        // What the neck then does to the head it does to them.
        turned * (root.inverse() * Vec3::new(way.sin(), 0.0, way.cos()))
    });
    for (node, turn) in turns {
        let Some(index) = skeleton.index_of(node) else { continue };
        let about = axis.filter(|_| turn != 0.0).map_or(Quat::IDENTITY, |axis| Quat::from_axis_angle(axis, turn));
        animator.set_twist(index, about);
    }
}

/// How far above the node of a head what looks out of it is (0xFE8B8), the turn
/// past which something is taken to be behind (0x100970), and the point that a
/// neck told to look at a point looks at, which is written into the game
/// (0x1005C0).
const EYES_UP: f32 = 5.0;
const BEHIND: f32 = 2.967_06;
const NECK_POINT: Vec3 = Vec3::new(-2136.0, 1053.0, 3051.0);
/// Where in the field's tunables the three limits of a neck that looks at
/// something are, in whole degrees (0x10091C): how far round to either side,
/// and how far the one way and the other up and down.
const PRM_NECK_LIMITS: [usize; 3] = [0x64, 0x65, 0x66];

/// How the frame of a head is to be turned for it to look at `point` (0x1007F8).
/// `frame` is the head's frame and `at` where the head is; `body` is the frame
/// of what it hangs off, by whose axes it is held back. The way to the point is
/// brought within the first limit of the way ahead, round the upright axis, and
/// then within the second or the third up or down; the frame is stood up on
/// that way, by the head's own upright axis; and it is tipped by what it takes
/// for what is a little above the head's node to be what looks at the point.
/// `side` is the side a head that had to turn far round was last turned to,
/// which it keeps to while what it looks at goes round behind it.
fn look(frame: Quat, at: Vec3, body: Quat, point: Vec3, limits: [f32; 3], side: &mut Option<bool>) -> Quat {
    const SAME: f32 = 0.999_99;
    let unlike = |cosine: f32| cosine < SAME && cosine > -SAME;
    let (across, up, ahead) = (body * Vec3::X, body * Vec3::Y, body * Vec3::Z);
    let mut way = (point - at).normalize_or_zero();

    let level = way - up * up.dot(way);
    if level.length() > 1e-5 {
        let level = level.normalize();
        let cosine = ahead.dot(level);
        let mut axis = if unlike(cosine) { ahead.cross(level).normalize_or_zero() } else { up };
        let turn = cosine.clamp(-1.0, 1.0).acos();
        if turn > limits[0] {
            // Which way round it is: by the upright axis, or against it.
            let round = axis.dot(up);
            if turn > BEHIND {
                axis = if round < 0.0 { -up } else { up };
            }
            match *side {
                None => *side = Some(round < 0.0),
                Some(true) if round > 0.0 => axis = -axis,
                Some(false) if round < 0.0 => axis = -axis,
                Some(_) => {}
            }
            way = Quat::from_axis_angle(axis, limits[0] - turn) * way;
        }
    }

    let upright = way - across * across.dot(way);
    if upright.length() > 1e-5 {
        let upright = upright.normalize();
        let cosine = ahead.dot(upright);
        if unlike(cosine) {
            let axis = ahead.cross(upright).normalize_or_zero();
            let turn = cosine.acos();
            let limit = if (body.inverse() * axis).x < 0.0 { limits[1] } else { limits[2] };
            if turn > limit {
                way = Quat::from_axis_angle(axis, limit - turn) * way;
            }
        }
    }

    let own_up = if unlike((frame * Vec3::Y).dot(way)) { frame * Vec3::Y } else { -(frame * Vec3::Z) };
    let sideways = own_up.cross(way).normalize_or_zero();
    let mut turned = Quat::from_mat3(&Mat3::from_cols(sideways, way.cross(sideways).normalize_or_zero(), way));

    let to_eyes = point + Vec3::Y * EYES_UP - at;
    let far = to_eyes.length();
    let sine = if far > 0.0 { -EYES_UP / far } else { 1.0 };
    if sine > -1.0 && sine < 1.0 && unlike(way.dot(to_eyes / far)) {
        turned = Quat::from_axis_angle(way.cross(to_eyes / far).normalize_or_zero(), sine.asin()) * turned;
    }
    turned.normalize()
}

/// Loads the resource of every character of the scene that has not been loaded yet.
/// The player's is the bundle of the suit she has on.
fn load_characters(
    data: Res<GameData>,
    tables: Res<FieldTables>,
    state: Res<FieldState>,
    stats: Res<PlayerStats>,
    mut assets: ResMut<CharacterAssets>,
    mut stores: AssetStores,
) {
    let suit = player::suit_character(stats.suit);
    if assets.worn() != suit {
        assets.wear(suit);
    }
    if assets.map() != Some(state.scene.map) {
        assets.on_map(state.scene.map);
    }
    for character in state.scene.characters.values().filter(|character| character.effect.is_none()) {
        if assets.get(character.chr).is_none() {
            assets.ensure(character.chr, &data, &tables, &mut stores);
        }
    }
}

/// Makes the models follow the scene: one for each character that can be shown, where
/// the character is, doing what it does. The player's is made anew when she has
/// another suit or another weapon on than it was made with, and carries the
/// weapon, a model of its own, on the node it hangs on (as in a battle), for as
/// long as no script has put it away (setting 76).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_characters(
    mut commands: Commands,
    state: Res<FieldState>,
    tables: Res<FieldTables>,
    assets: Res<CharacterAssets>,
    (data, stats): (Res<GameData>, Res<PlayerStats>),
    mut stores: AssetStores,
    mut weapons: Local<BTreeMap<i32, Option<ModelAsset>>>,
    mut props: Local<BTreeMap<String, Option<ModelAsset>>>,
    mut carried: Query<&mut Visibility, (With<FieldWeapon>, Without<Actor>)>,
    root: Single<Entity, With<FieldRoot>>,
    mut actors: Query<(
        Entity,
        &mut Actor,
        &mut Stepped,
        &mut Animator,
        &mut Visibility,
        Option<&Parts>,
        Option<&player::ShownParts>,
        Option<&Skeleton>,
        &mut Lit,
    )>,
) {
    let worn = (assets.worn(), stats.weapon);
    // The model of the weapon she has on, read the first time it is wanted.
    let weapon = weapons.entry(stats.weapon).or_insert_with(|| {
        let file = player::weapon_model(stats.weapon)?;
        let (model, bytes) = data.model(&file).inspect_err(|error| warn!("{file} is not shown on the field: {error:#}")).ok()?;
        Some(ModelAsset::build(&model, &bytes, &mut stores))
    });
    let armed = state.scene.characters.get(&PLAYER).is_some_and(|player| !player.flags.has(flag::NO_WEAPON));
    for mut visibility in &mut carried {
        let wanted = if armed { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    // Where everybody's head is, for those whose necks look at somebody
    // (0x1089C0: the head's node, or where one with no such node stands), and
    // how far such a neck may turn.
    let heads: BTreeMap<i32, Vec3> = actors
        .iter()
        .filter_map(|(_, actor, _, animator, _, _, _, skeleton, _)| {
            let character = state.scene.characters.get(&actor.id)?;
            let skeleton = skeleton?;
            let head = animator.model_space(skeleton, skeleton.index_of(HEAD_NODE)?);
            Some((actor.id, character.frame().transform_point(head.translation)))
        })
        .collect();
    let limits = PRM_NECK_LIMITS.map(|at| f32::from(tables.tunables.u8(at).unwrap_or(0)).to_radians());
    let mut shown = BTreeSet::new();
    for (entity, mut actor, mut stepped, mut animator, mut visibility, parts, shown_parts, skeleton, mut lit) in &mut actors {
        let made_as_it_is = |character: &&Character| character.chr == actor.chr && (actor.id != PLAYER || actor.worn == worn);
        let character = state.scene.characters.get(&actor.id).filter(made_as_it_is);
        let (Some(character), Some(asset)) = (character, assets.get(actor.chr)) else {
            commands.entity(entity).despawn();
            continue;
        };
        shown.insert(actor.id);
        stepped.current = placement(character);
        lit.set_if_neq(light_of(&state.scene, character));
        let wanted = if character.drawn() { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        let anew = actor.motions != character.motions;
        if anew {
            actor.motions = character.motions;
        }
        animate(&mut animator, character, asset, state.scene.staging.as_deref(), anew, state.scene.work.heavy);
        // Its animations go on by the time it has of a frame (0xFCE90).
        let speed = character.pace_fraction();
        if animator.speed != speed {
            animator.speed = speed;
        }
        // What is layered on its body's animation is off while a script says so.
        let off = character.flags.has(flag::NO_LAYERS);
        if animator.layers_off != off {
            animator.layers_off = off;
        }
        // The face a script has given it since: its face's layer is held at
        // that face of the faces' animation, five frames of the game a face,
        // and comes over to it from how it is (0x10B458: the layer's clip, its
        // time, the frames, and that it stands).
        if actor.expressions != character.expressions {
            actor.expressions = character.expressions;
            if let (Some(layer), Some(faces), Some((face, frames))) = (asset.facial, asset.clip(animation::FACES), character.expression) {
                let at = (face.max(0) as f32 * FRAMES_A_FACE * STEP_ANIMATION_FRAMES).min(faces.frames());
                animator.set_slot_blended(1 + layer, faces, frames as f32 * STEP_ANIMATION_FRAMES);
                animator.slot_mut(1 + layer).set_time(at);
                animator.slot_mut(1 + layer).loop_between(at, at);
            }
        }
        if let Some(skeleton) = skeleton.filter(|_| character.face.used || character.neck.on || actor.headed) {
            // The head is seen to once more after the neck has let go of it.
            actor.headed = character.face.used || character.neck.on;
            let mut neck = actor.neck;
            let looked = match character.neck.way {
                NeckWay::Point => Some(NECK_POINT),
                NeckWay::Object => {
                    let whom = character.neck.object;
                    heads.get(&whom).copied().or_else(|| state.scene.characters.get(&whom).map(|whom| whom.position))
                }
                _ => None,
            };
            pose_head(&mut animator, skeleton, character, &mut neck, looked, limits);
            actor.neck = neck;
        }
        // What an event has given it to hold (0xFD2E4): each thing goes with the
        // node of its place, and is not seen while it is put away.
        for place in 0..scene::PROPS {
            let wanted = character.props[place].as_deref();
            if actor.props[place].as_ref().is_some_and(|(file, ..)| Some(file.as_str()) != wanted) {
                if let Some((_, model, _)) = actor.props[place].take() {
                    commands.entity(model).despawn();
                }
            }
            let node = skeleton.and_then(|skeleton| skeleton.node(scene::PROP_NODES[place]));
            if let (Some(file), None, Some(node)) = (wanted, &actor.props[place], node) {
                let asset = props.entry(file.to_owned()).or_insert_with(|| {
                    let (model, bytes) = data.model(file).inspect_err(|error| warn!("{file} is not shown: {error:#}")).ok()?;
                    Some(ModelAsset::build(&model, &bytes, &mut stores))
                });
                if let Some(asset) = asset {
                    let model = asset.spawn(&mut commands, Transform::IDENTITY);
                    commands.entity(model).insert(ChildOf(node));
                    actor.props[place] = Some((file.to_owned(), model, false));
                }
            }
            if let Some((_, model, away)) = &mut actor.props[place] {
                if *away != character.props_away[place] {
                    *away = character.props_away[place];
                    commands.entity(*model).insert(if *away { Visibility::Hidden } else { Visibility::Inherited });
                }
            }
        }
        // Under the window of a mission failed it is a picture of itself.
        let held = state.scene.held_by_failed();
        if animator.paused != held {
            animator.paused = held;
        }
        // The parts of its model that its flags have shown.
        if let Some(parts) = parts {
            let wanted = shown_parts_of(&tables, parts, actor.id, character, weapon.is_some());
            if shown_parts.is_none_or(|shown| shown.0 != wanted) {
                commands.entity(entity).insert(player::ShownParts(wanted));
            }
        }
    }

    for (&id, character) in &state.scene.characters {
        let Some(asset) = assets.get(character.chr).filter(|_| !shown.contains(&id)) else { continue };
        let at = placement(character);
        let entity = match (id, weapon.as_ref()) {
            (PLAYER, Some(weapon)) => {
                let (entity, [hand, root]) = asset.model.spawn_with_nodes(&mut commands, at, [WEAPON_NODE, ROOT_NODE]);
                if let Some(hand) = hand {
                    let model = weapon.spawn(&mut commands, Transform::IDENTITY);
                    let visibility = if armed { Visibility::Inherited } else { Visibility::Hidden };
                    commands.entity(model).insert((FieldWeapon, visibility, ChildOf(hand)));
                }
                commands.entity(entity).insert(RootNode(root));
                entity
            }
            _ => {
                let (entity, [mark, root]) = asset.model.spawn_with_nodes(&mut commands, at, [MARK_NODE, ROOT_NODE]);
                commands.entity(entity).insert((MarkNode(mark), RootNode(root)));
                entity
            }
        };
        let mut animator = Animator::with_slots(1 + asset.layers.len());
        if let Some(stand) = asset.clip(animation::STAND) {
            animator.set(stand);
            animator.body_mut().loop_clip();
        }
        for (index, layer) in asset.layers.iter().enumerate() {
            animator.set_slot(1 + index, layer);
            animator.slot_mut(1 + index).loop_clip();
        }
        let visibility = if character.drawn() { Visibility::Inherited } else { Visibility::Hidden };
        let mut entity = commands.entity(entity);
        // A face it was given before its model was made is put on it at once.
        let expressions = character.expressions.wrapping_sub(u32::from(character.expression.is_some()));
        let actor = Actor { id, chr: character.chr, worn, motions: character.motions, expressions, neck: NeckPose::default(), headed: false, props: Default::default() };
        entity.insert((actor, animator, Stepped::new(at), visibility, light_of(&state.scene, character), ChildOf(*root)));
    }
}

/// The switchable parts of a character's model that are shown: those its flags have
/// shown (`s224`). The weapon the player carries is a model of its own, shown
/// unless a script has put it away (setting 76); if there is none for what she
/// has on (`carries`), the cannon that is part of her model stands in for it.
fn shown_parts_of(tables: &FieldTables, parts: &Parts, id: i32, character: &Character, carries: bool) -> Vec<u16> {
    let mut shown = tables.shown_parts(parts.ids(), |part| character.flags.has(flag::PART + part));
    let cannon = player::part::CANNON;
    if id == PLAYER && !carries && !character.flags.has(flag::NO_WEAPON) && !shown.contains(&cannon) && parts.ids().any(|part| part == cannon) {
        shown.push(cannon);
    }
    shown
}

fn placement(character: &Character) -> Transform {
    character.frame()
}

/// Gives a character the animation that goes with what it is doing. `anew` is
/// that a script has set an animation playing since it was last seen to, which
/// begins even the one that is playing from its start (0xF7CF4).
fn animate(animator: &mut Animator, character: &Character, asset: &CharacterAsset, staging: Option<&Staging>, anew: bool, heavy: bool) {
    let clip = |number: usize| asset.clip(number);
    // How long it takes to go over to another animation, in the animation's frames
    // (setting 33, 0x1084E8).
    let blend = character.blend_frames() as f32 * STEP_ANIMATION_FRAMES;
    // One a trap is taking somewhere has the animation of the way it is taken,
    // each begun as that part of it begins.
    if let Some((number, loops, over)) = taken_motion(&character.action) {
        if let Some(wanted) = usize::try_from(number).ok().and_then(clip) {
            if anew || !animator.body().is(wanted) {
                animator.set_blended(wanted, over * STEP_ANIMATION_FRAMES);
                if loops {
                    animator.body_mut().loop_clip();
                } else {
                    animator.body_mut().play_out();
                }
            }
        }
        return;
    }
    match character.action {
        Action::Destroy { .. } => {
            if let Some(end) = clip(animation::DESTROYED) {
                if !animator.body().is(end) {
                    animator.set_blended(end, blend);
                    animator.body_mut().play_out();
                }
            }
            return;
        }
        Action::Turn { rate } => {
            // A turn has an animation to either side, for those that have them: it
            // plays once, and is come to over eight frames whatever the
            // character's own are (0xF4C4C on: 0x4000 of its time).
            let turn = clip(animation::TURN[1]).and(clip(animation::TURN[(rate < 0.0) as usize]));
            if let Some(turn) = turn {
                if !animator.body().is(turn) {
                    animator.set_blended(turn, TURN_BLEND_FRAMES * STEP_ANIMATION_FRAMES);
                    animator.body_mut().play_out();
                }
                return;
            }
        }
        _ => {}
    }
    // What it stands, runs and walks in is its own to say, once a script has
    // said (0xFD960).
    let own = |what: i32, usual: usize| clip(character.motion_of(what).unwrap_or(usual));
    let by_status = match character.status {
        status::WALK => own(scene::setting::WALK_MOTION, animation::WALK).or(own(scene::setting::RUN_MOTION, animation::RUN)),
        status::RUN => own(scene::setting::RUN_MOTION, animation::RUN).or(own(scene::setting::WALK_MOTION, animation::WALK)),
        status::TRUDGE => clip(animation::TRUDGE).or(own(scene::setting::WALK_MOTION, animation::WALK)),
        _ => None,
    };
    // An animation the script chose plays while the character has nothing else to do:
    // one of its own, or one an event has brought for a scene.
    let chosen = character.motion.and_then(|(motion, loops)| {
        let clip = match motion {
            Motion::Own(number) => clip(usize::try_from(number).ok()?)?,
            Motion::Staged(slot, number) => staging?.motion(slot, number)?,
        };
        Some((clip, loops))
    });
    match (by_status, chosen) {
        (Some(moving), _) => loop_blended(animator, moving, blend),
        (None, Some((chosen, true))) => {
            if anew {
                animator.set_blended(chosen, blend);
                animator.body_mut().loop_clip();
            } else {
                loop_blended(animator, chosen, blend);
            }
        }
        (None, Some((chosen, false))) => {
            if anew || !animator.body().is(chosen) {
                animator.set_blended(chosen, blend);
                animator.body_mut().play_out();
            }
        }
        (None, None) => {
            // One that has stood a while plays what it has for idling, which
            // comes at once (0xF9180 on), as the stand does after it.
            let idling = character.idle.playing().and_then(|(number, round)| Some((clip(usize::try_from(number).ok()?)?, round)));
            match idling {
                Some((idling, true)) => loop_blended(animator, idling, 0.0),
                Some((idling, false)) => {
                    if !animator.body().is(idling) {
                        animator.set_blended(idling, 0.0);
                        animator.body_mut().play_out();
                    }
                }
                None => {
                    // While the going is heavy, the stand of that (0xFD9C8).
                    let stand = heavy.then(|| clip(animation::STAND_HEAVY)).flatten().or(own(scene::setting::STAND_MOTION, animation::STAND));
                    if let Some(stand) = stand {
                        loop_blended(animator, stand, if character.idle.at_once { 0.0 } else { blend });
                    }
                }
            }
        }
    }
    // A script holds it at a frame of the game (0xE3400): whatever it plays is
    // there and stays.
    if let Some(frame) = character.held {
        let at = (frame as f32 * STEP_ANIMATION_FRAMES).min(animator.body().length());
        if animator.body().range() != (at, at) || animator.body().time() != at {
            animator.body_mut().set_time(at);
            animator.body_mut().loop_between(at, at);
        }
    }
}

/// The animation of one a trap is taking somewhere: its number, whether it goes
/// round, and the frames of the game it is come over to in. A leap's: the one it
/// gathers itself and springs with, which goes round within itself once it is in
/// the air (0xF8388, over four frames); the one it comes down in, come over to
/// for as long as it still has in the air (0xF8778); and the one it lands with,
/// at once (0xF897C). A rush's (0xF65DC, over one), the one it stops with
/// (0xF6840, over four), and a flight's (0xF6A14, over three). (One that leaps
/// from where it flew to is first put at the end of the one it springs with,
/// 0xF8364, which the port leaves out: it comes over from how it flew.)
fn taken_motion(action: &Action) -> Option<(i32, bool, f32)> {
    Some(match action {
        Action::Leap(leap) => match leap.part {
            LeapPart::Gather | LeapPart::Spring => (scene::MOTION_SPRING, true, 4.0),
            LeapPart::ComeDown => (scene::MOTION_COME_DOWN, true, (leap.length - leap.spring) as f32 / FULL_PACE as f32),
            LeapPart::Land => (scene::MOTION_LAND, false, 0.0),
        },
        Action::Rush => (scene::MOTION_RUSH, false, 1.0),
        // The same one for a pull-up after a run, over four too (0xFDADC).
        Action::RushStop { .. } | Action::Brake { .. } => (scene::MOTION_RUSH_STOP, false, 4.0),
        Action::Flight => (scene::MOTION_FLY, true, 3.0),
        _ => return None,
    })
}

/// Blends into `clip` over `blend` of its frames and loops it, unless it is already
/// playing.
fn loop_blended(animator: &mut Animator, clip: &Arc<Clip>, blend: f32) {
    if !animator.body().is(clip) {
        animator.set_blended(clip, blend);
        animator.body_mut().loop_clip();
    }
}

/// What the field's camera sees by: where it is and how it is turned.
#[derive(Clone, Copy)]
struct Sight {
    place: Vec3,
    turn: Quat,
    /// What a camera that follows her looks at.
    target: Option<Vec3>,
    /// The walls kept it from where its figures would have it.
    stopped: bool,
}

/// Where the camera's figures have it with her feet at `feet`, by its kind
/// (0xDD924); none if it is of a kind that has no say (a cut's) or that the port
/// does not have.
///
/// One that follows her (0xDB29C, 0xDB528) is off from the point it looks at by its
/// offset, turned by its pitch and then its yaw, as far as the walls that keep it in
/// let it; `own` is what the port's own controls add, a tilt and so many times the
/// way off. One that stays put (0xDDC00) is at its place, turned by its yaw, then
/// its pitch, then its roll (0x24B8C), a pitch below nought looking down. One that
/// looks from a point (0xDDE44) is at that point, or on the way from it to her as
/// far off her as its offset is long, and turned to the point it looks at. One that
/// goes along a path (0xDAA48) is where the path's move has it for where she is.
///
/// `lay` is how her frame lies. What follows her is off from her by its figures as
/// her frame has them (0xDB528 and 0xDB29C turn them by her frame's turn,
/// 0x109128), and a path's grid is asked where she is as her frame has that
/// (0xDAAB8), the path's own camera being as the world has it.
fn sight(camera: &camera::Camera, feet: Vec3, lay: Quat, own: (f32, f32), world: &CollisionWorld, paths: &CameraPaths) -> Option<Sight> {
    let angle = |what: i32| camera.figure(what).to_radians();
    let turned = |yaw: f32, pitch: f32, roll: f32| Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    match camera.kind {
        kind::FIXED => Some(Sight {
            place: camera.figures(figure::PLACE),
            turn: lay * turned(angle(figure::YAW), angle(figure::PITCH), angle(figure::ROLL)),
            target: None,
            stopped: false,
        }),
        kind::FOLLOW | kind::AROUND => {
            // Its pitch is the record's, leant as the scene has it (0xDB60C).
            let (target, place, turn, stopped) = following(camera, feet, lay, own, camera.leaning().to_radians(), world);
            Some(Sight { place, turn, target: Some(target), stopped })
        }
        kind::POINT | kind::POINT_AT_A_DISTANCE => {
            let target = feet + lay * camera.figures(figure::LOOK_AT);
            let point = camera.figures(figure::POINT);
            // The way from her to the point, as her frame has it (0xDDEDC).
            let from = lay.inverse() * (point - target);
            let (yaw, pitch) = (from.x.atan2(from.z), -from.y.atan2(from.xz().length()));
            let place = match camera.kind {
                kind::POINT => point,
                _ => target + lay * from.normalize_or_zero() * camera.figure(figure::OFFSET[2]),
            };
            Some(Sight { place, turn: lay * turned(yaw, pitch, angle(figure::ROLL)), target: None, stopped: false })
        }
        kind::PATH => {
            let path = usize::try_from(camera.path).ok().and_then(|path| paths.0.get(path)).and_then(Option::as_ref)?;
            let hers = if lay == Quat::IDENTITY { feet } else { lay.inverse() * feet };
            let pose = path.pose(hers.x, hers.z);
            Some(Sight { place: pose.transform.translation, turn: pose.transform.rotation, target: None, stopped: false })
        }
        _ => None,
    }
}

/// Where a camera that follows her is with `lean` added to its pitch, in radians:
/// what it looks at, its place as far as the walls let it, how it is turned, and
/// whether the walls stopped it.
fn following(camera: &camera::Camera, feet: Vec3, lay: Quat, own: (f32, f32), lean: f32, world: &CollisionWorld) -> (Vec3, Vec3, Quat, bool) {
    let angle = |what: i32| camera.figure(what).to_radians();
    let target = feet + lay * camera.figures(figure::LOOK_AT);
    let turn = lay * Quat::from_euler(EulerRot::YXZ, angle(figure::YAW), angle(figure::PITCH) + lean - own.0, 0.0);
    let off = turn * (camera.figures(figure::OFFSET) * own.1);
    let swept = world.sweep(target, off, CAMERA_RADIUS, Layer::Boundary);
    (target, target + off * swept.unwrap_or(1.0), turn, swept.is_some())
}

/// The pitch of the way from what a camera that follows her looks at to where its
/// own pitch alone has it, in degrees, taken as if the ground under that place
/// were as high as her feet (0xDE9B8 to 0xDEC40): the ground is looked for from
/// 100 over the place to 1000 under it, and where there is none it counts as
/// being as high as her feet. None where her frame is not the world's.
fn slope_under(camera: &camera::Camera, feet: Vec3, lay: Quat, own: (f32, f32), world: &CollisionWorld) -> Option<f32> {
    const OVER: f32 = 100.0;
    const UNDER: f32 = 1000.0;
    if lay != Quat::IDENTITY {
        return None;
    }
    let (target, place, ..) = following(camera, feet, lay, own, 0.0, world);
    let rise = world.ground(place, OVER, UNDER).map_or(0.0, |ground| ground.height - feet.y);
    let across = (place - target).with_y(0.0).length();
    Some((target.y - (place.y + rise)).atan2(across).to_degrees())
}

/// A frame of the camera (0xDC880): the figures that scripts and traps have on their
/// way somewhere move on, the L and R buttons turn it, gathering speed while held
/// (0xDECBC), and it keeps what it has worked out for itself with its figures: where
/// it has got to, and how it is turned if that is not for a script to say. A camera
/// that follows her gets to its place with a lag (0xDF28C). Which button turns which
/// way is not read out of the original yet: here R turns the view to the right. When
/// a script sets the camera up, what the port's own controls had done to it is
/// undone.
fn spin_camera(
    pad: Res<Pad>,
    world: Res<CollisionWorld>,
    paths: Res<CameraPaths>,
    scenery: Option<Res<scenery::Scenery>>,
    mut state: ResMut<FieldState>,
    mut follow: Single<&mut FollowCamera>,
) {
    let state = &mut *state;
    state.scene.camera.step();
    if state.camera_set != state.scene.camera.set {
        state.camera_set = state.scene.camera.set;
        (follow.tilt, follow.zoom, follow.spin) = (0.0, 1.0, 0.0);
    }
    let (left, right) = (pad.held & button::L != 0, pad.held & button::R != 0);
    let turn = follow.turn;
    let mut spin = follow.spin;
    if left != right {
        // Speeds up in the held direction; a turn the other way is cancelled first.
        let sign = if left { 1.0 } else { -1.0 };
        spin = if spin * sign >= 0.0 { spin + sign * turn.gain } else { (spin * sign + turn.reverse).min(0.0) * sign };
    } else if spin != 0.0 {
        spin = spin.signum() * (spin.abs() - turn.decay).max(0.0);
    }
    follow.spin = spin.clamp(-CAMERA_MAX_TURN, CAMERA_MAX_TURN);

    let scene = &mut state.scene;
    if follow.cuts != scene.camera.cuts {
        follow.cuts = scene.camera.cuts;
        follow.lag.cut();
        follow.behind = [Vec3::ZERO; 2];
    }
    if follow.renewed != scene.camera.renewed {
        follow.renewed = scene.camera.renewed;
        follow.lag.renew();
    }
    let made = |camera: &mut camera::Camera, what: [i32; 3], value: Vec3| {
        for (what, value) in what.into_iter().zip(value.to_array()) {
            camera.made(what, value);
        }
    };
    // The cut's camera leaves where it is, how it is turned and how wide it sees
    // with the figures (0xDCA7C).
    if let Some(cut) = &scene.cut {
        if scene.camera.kind == kind::CUT {
            let pose = cut.pose(0.0);
            let (yaw, pitch, roll) = pose.transform.rotation.to_euler(EulerRot::YXZ);
            made(&mut scene.camera, figure::PLACE, pose.transform.translation);
            made(&mut scene.camera, [figure::PITCH, figure::YAW, figure::ROLL], Vec3::new(pitch, yaw, roll).map(f32::to_degrees));
            let width = vertical_fov(pose.focal_length, pose.film_width, pose.zoom, 1.0);
            scene.camera.made(figure::FIELD_OF_VIEW, width.to_degrees());
        }
        return;
    }
    let Some((feet, lay)) = scene.player().map(|player| (player.position, player.lay)) else { return };
    // One that follows her leans its pitch first (0xDC9E0).
    if kind::follows(scene.camera.kind) {
        let slope = slope_under(&scene.camera, feet, lay, (follow.tilt, follow.zoom), &world);
        let (rates, level) = scenery.as_ref().map_or(([0.0; 2], 0.0), |scenery| (scenery.lean, scenery.level));
        scene.camera.lean(slope, rates, level);
    }
    let Some(sight) = sight(&scene.camera, feet, lay, (follow.tilt, follow.zoom), &world, &paths) else { return };
    let mut place = sight.place;
    match scene.camera.kind {
        kind::FOLLOW | kind::AROUND => {
            let target = sight.target.unwrap_or(place);
            place = follow.lag.step(sight.place, target, lay * Vec3::Y, sight.stopped);
            follow.behind = [follow.behind[1], place - sight.place];
            // Where it has got to is kept twice over (0xDDA28).
            made(&mut scene.camera, figure::POINT, place);
        }
        kind::POINT | kind::POINT_AT_A_DISTANCE | kind::PATH => {
            // Its angles are kept as her frame has them (0xDDF24, 0xDDF64).
            let own = if scene.camera.kind == kind::PATH { sight.turn } else { lay.inverse() * sight.turn };
            let (yaw, pitch, _) = own.to_euler(EulerRot::YXZ);
            // A camera on a path is taken to look level (0xDE2FC).
            let pitch = if scene.camera.kind == kind::PATH { 0.0 } else { pitch };
            scene.camera.made(figure::PITCH, pitch.to_degrees());
            scene.camera.made(figure::YAW, yaw.to_degrees());
            if scene.camera.kind == kind::PATH {
                scene.camera.made(figure::ROLL, 0.0);
            }
            if scene.camera.kind == kind::POINT {
                // How far off her the point is (0xDDFD8).
                let target = feet + scene.camera.figures(figure::LOOK_AT);
                scene.camera.made(figure::OFFSET[2], place.distance(target));
            }
        }
        _ => {}
    }
    if scene.camera.kind != kind::FIXED {
        made(&mut scene.camera, figure::PLACE, place);
    }
}

/// Puts the camera where it belongs: where a scene's cut has it, while one is up
/// (the original takes the place and the turn of the cut's camera node and the angle
/// of its lens, 0xDCA7C), and otherwise where its figures have it, by its kind
/// ([`sight`]). A camera that follows her looks at its point, or from very close the
/// way its angles have it (0xDDA4C). The picture is then moved over by the camera's
/// shift (0xDE310), and while the camera is going over from one view to the next it
/// is between the two (0xDCEDC).
fn follow_camera(
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    (look, shake): (Res<Look>, Res<effects::Shake>),
    world: Res<CollisionWorld>,
    paths: Res<CameraPaths>,
    mut state: ResMut<FieldState>,
    actors: Query<(&Actor, &Transform), Without<FollowCamera>>,
    camera: Single<(&mut Transform, &mut FollowCamera, &mut Projection)>,
) {
    let (mut transform, mut follow, mut projection) = camera.into_inner();
    // How near the scene is seen from is the record's, whatever the camera's
    // kind (0xD9DF8, and 0xD9E84 for a scene's cut): what draws the scene and
    // its characters has that; the lens itself sees from where effects do.
    if let Projection::Perspective(perspective) = &mut *projection {
        if perspective.near != CAMERA_NEAR {
            perspective.near = CAMERA_NEAR;
        }
    }
    let mut look_through = |angle: f32| {
        if let Projection::Perspective(perspective) = &mut *projection {
            if perspective.fov != angle {
                perspective.fov = angle;
            }
        }
    };
    if let Some(cut) = &state.scene.cut {
        // Between two steps of the game the cut is carried on by the part of a step
        // that has passed, like everything else that moves.
        let pose = cut.pose(fixed.overstep_fraction());
        *transform = pose.transform;
        look_through(pose.vertical_fov(SCREEN_ASPECT));
        return;
    }
    let scene = &mut state.scene;
    if kind::follows(scene.camera.kind) {
        // The buttons' turn is a speed per logic step; spread it over the frames
        // between. It turns the figure itself, as the original's does (0xDEE10).
        let turned = look.turn.x + follow.spin * time.delta_secs() * STEPS_PER_SECOND as f32;
        if turned != 0.0 {
            let yaw = scene.camera.figure(figure::YAW) + turned.to_degrees();
            scene.camera.turned(yaw);
        }
        follow.tilt = (follow.tilt + look.turn.y).clamp(-0.2, CAMERA_MAX_TILT);
        follow.zoom = (follow.zoom * look.zoom).clamp(*CAMERA_ZOOM.start(), *CAMERA_ZOOM.end());
    }
    // She is where her model is, which is between the game's steps where she is.
    let shown = actors.iter().find(|(actor, _)| actor.id == PLAYER).map(|(_, player)| player.translation);
    let Some(feet) = shown.or(scene.player().map(|player| player.position)) else { return };
    let lay = scene.player().map_or(Quat::IDENTITY, |player| player.lay);
    let Some(sight) = sight(&scene.camera, feet, lay, (follow.tilt, follow.zoom), &world, &paths) else { return };
    look_through(view_height(scene.camera.figure(figure::FIELD_OF_VIEW)));
    let into = fixed.overstep_fraction();
    let (mut place, mut turn) = (sight.place, sight.turn);
    if let Some(target) = sight.target {
        place += follow.behind[0].lerp(follow.behind[1], into);
        if place.distance(target) >= CAMERA_LOOK_FROM {
            turn = Transform::from_translation(place).looking_at(target, lay * Vec3::Y).rotation;
        }
    }
    place += turn * scene.camera.figures(figure::SHIFT);
    if let Some(blend) = &mut scene.camera.blend {
        let from = *blend.from.get_or_insert((transform.translation, transform.rotation));
        let part = blend.part(into);
        (place, turn) = (from.0.lerp(place, part), from.1.slerp(turn, part));
    }
    // What it is shaken by turns it in its own frame, last of all (0xDD848).
    (transform.translation, transform.rotation) = (place, turn * shake.turned());
}

/// Draws the screen's fade (the original's "Trans" task): black or white over the
/// scene and over all of the interface but a scene's captions; and the picture of
/// the screen it keeps, over the scene and under the interface (0x10F324).
fn draw_fade(state: Res<FieldState>, captured: Res<Captured>, seen: Res<Seen>, mut canvas: ResMut<Canvas>) {
    // While the next mission is chosen the field is gone: the game is on a
    // screen of its own then, with a fade of its own. So it is on the title.
    let chosen = state.hub.is_some() || state.title.is_some();
    let fade = &state.scene.fade;
    // A picture that has been asked for and has not come is not the one kept.
    if let Some(colour) = fade.picture().filter(|_| !chosen && state.scene.capture == scene::Capture::None) {
        captured.draw(&mut canvas, &seen, layer::SCREEN_PICTURE, colour);
    }
    let alpha = (fade.level.clamp(0.0, 1.0) * 255.0) as u8;
    if !chosen && alpha != 0 {
        let shade = if fade.white { 255 } else { 0 };
        canvas.fill(layer::FADE, hud::SCREEN, [shade, shade, shade, alpha]);
    }
}

/// Starts the battle the map's script has asked for: on the battle map that goes
/// with this map, against the group it named (0xD8364). First a picture is taken
/// of the field with all it has on the screen (0xD876C), which the battle comes
/// up under (`battle/encounter.rs`).
#[allow(clippy::too_many_arguments)]
fn start_battle(
    mut commands: Commands,
    data: Res<GameData>,
    tables: Res<FieldTables>,
    progress: Res<Progress>,
    skills: Res<battle::skills::Skills>,
    mut stats: ResMut<PlayerStats>,
    mut state: ResMut<FieldState>,
    (mut started, mut pictured): (Local<bool>, Local<bool>),
    mut next: ResMut<NextState<Mode>>,
) {
    let Some(request) = state.scene.battle else {
        (*started, *pictured) = (false, false);
        return;
    };
    if *started {
        return;
    }
    let lost = state.lose_battle == Some(request.group);
    if lost || state.skip_battles && state.fight != Some(request.group) {
        info!("the battle against group {} is passed over, as {}", request.group, if lost { "lost" } else { "won" });
        // A battle lost leaves her with no HP (0x94B34).
        if lost {
            stats.hp = 0.0;
        }
        state.scene.report = if lost { BattleReport::LOST } else { BattleReport::WON };
        state.scene.battle = None;
        state.scene.fade = scene::Fade::DARK;
        state.back_from_battle();
        return;
    }
    match state.scene.capture {
        scene::Capture::None if !*pictured => {
            state.scene.capture = scene::Capture::Wanted { whole: true };
            *pictured = true;
            return;
        }
        scene::Capture::None => {}
        _ => return,
    }
    *started = true;
    let stage = usize::try_from(state.scene.map).ok().and_then(|map| tables.maps.get(map)).map(|map| map.battle_stage());
    let source = stage
        .context("the map is not in the game's list")
        .and_then(|stage| {
            let group = u16::try_from(request.group).context("the group's number is out of range")?;
            let dodging = if progress.flag(crate::battle::actor::DODGE_FLAG) { "" } else { ", which she cannot dodge in yet" };
            info!("battle against group {group} on {stage}{dodging}");
            let terms = Terms::of_scenario(progress.variable(variable::SCENARIO));
            BattleSource::load(&data, &stage, group, *stats, terms, &skills)
        });
    match source {
        Ok(mut source) => {
            // The stage the map is of, which some of the records go by.
            source.map_stage = usize::try_from(state.scene.map).ok().and_then(|map| tables.maps.get(map)).map_or(0, |map| i32::from(map.stage));
            // The original plays it as the screen's wipe begins (0xD8890).
            let sound = if source.victory_pose() { SOUND_BATTLE_OF_NOTE } else { SOUND_BATTLE };
            state.scene.sounds.push(Request::common(sound));
            commands.insert_resource(battle::encounter::Encounter::with(source.victory_pose()));
            commands.insert_resource(source);
            next.set(Mode::Battle);
        }
        Err(error) => {
            // The port cannot stage every battle yet. Rather than stop the game the
            // battle is passed over, as if won.
            error!("the battle against group {} cannot be staged and is counted as won: {error:#}", request.group);
            state.scene.report = BattleReport::WON;
            state.scene.battle = None;
            state.scene.fade = scene::Fade::DARK;
            state.back_from_battle();
        }
    }
}

/// Takes the battle's outcome back to the script that asked for it, and its figures
/// into the counts the missions go by (0xD893C).
#[allow(clippy::too_many_arguments)]
fn end_battle(
    conclusion: Res<battle::Conclusion>,
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    mut next: ResMut<NextState<Mode>>,
) {
    let Some(outcome) = conclusion.outcome else { return };
    let report = match outcome {
        battle::flow::Outcome::Won => BattleReport::WON,
        battle::flow::Outcome::Lost => BattleReport::LOST,
    };
    info!("the battle is over: outcome {}", report.outcome);
    progress.count_battle(&conclusion.tally);
    // The mission's clock goes on by the time the battle took.
    let time = conclusion.tally.seconds.clamp(0, BATTLE_SECONDS_MOST) as u32 * TIME_PER_SECOND;
    let mut field = system::World {
        progress: &mut progress,
        scene: &mut state.scene,
        stats: &mut stats,
        tables: &tables,
        ground: &world,
    };
    if let Err(error) = system.call("update_mis_time", &[time], &mut field) {
        error!("the system script failed: {error:#}");
    }
    // She does not come out of a battle won with no HP at all.
    if report == BattleReport::WON && stats.hp <= 0.0 {
        stats.hp = 1.0;
    }
    state.scene.report = report;
    state.scene.battle = None;
    // The field comes back under the battle's last picture, which the script then
    // has go (0xE6B28); or dark, after a battle that ended by going dark, for the
    // script to bring back (0x9D78C).
    if conclusion.picture {
        state.scene.fade.hold_picture();
    } else {
        state.scene.fade = scene::Fade::DARK;
    }
    next.set(Mode::Field);
}

/// Puts the field out of sight while a battle is fought, and the sounds about its
/// map out of hearing (0xD8890).
fn leave_for_battle(
    mut commands: Commands,
    mut root: Single<&mut Visibility, With<FieldRoot>>,
    camera: Single<(Entity, &mut Camera), With<FollowCamera>>,
    mut state: ResMut<FieldState>,
    mut board: ResMut<Board>,
) {
    state.scene.hear_ambience(false);
    board.ask_all(state.scene.sounds.drain(..));
    **root = Visibility::Hidden;
    let (entity, mut camera) = camera.into_inner();
    camera.is_active = false;
    commands.entity(entity).remove::<IsDefaultUiCamera>();
}

fn come_back(
    mut commands: Commands,
    mut root: Single<&mut Visibility, With<FieldRoot>>,
    camera: Single<(Entity, &mut Camera), With<FollowCamera>>,
    mut state: ResMut<FieldState>,
    scenery: Res<Scenery>,
) {
    state.scene.hear_ambience(true);
    state.back_from_battle();
    **root = Visibility::Inherited;
    let (entity, mut camera) = camera.into_inner();
    camera.is_active = true;
    commands.entity(entity).insert(IsDefaultUiCamera);
    commands.insert_resource(backdrop_of(&scenery));
}

/// The talk of the ride on the bike (0x149CBC, 0x130778): talk `n` is the
/// event of the ride's stage, begun with `n`, which is one of the field's own
/// events and is played as those are, through the window people speak to her
/// through. The ride is told when it has been said.
#[allow(clippy::too_many_arguments)]
fn talk_of_the_ride(
    riding: Option<ResMut<crate::bike::show::Riding>>,
    course: Option<Res<crate::bike::show::Course>>,
    pad: Res<Pad>,
    tables: Res<FieldTables>,
    world: Res<CollisionWorld>,
    mut progress: ResMut<Progress>,
    mut stats: ResMut<PlayerStats>,
    mut system: ResMut<SystemScript>,
    mut state: ResMut<FieldState>,
    mut board: ResMut<Board>,
    mut playing: Local<Option<i32>>,
) {
    let (Some(mut riding), Some(course)) = (riding, course) else { return };
    let ride = &mut riding.ride;
    let state = &mut *state;
    match (ride.talk.going, *playing) {
        (Some(talk), None) => {
            let event = ride.tables.stage(&course.0).and_then(|stage| tables.event_number(&format!("FEVT_{}", stage.events)));
            match event {
                Some(event) => {
                    state.scene.events.push_back((event, talk));
                    *playing = Some(talk);
                }
                // A stage with no events has nothing to say.
                None => ride.talk_over(),
            }
        }
        (None, Some(_)) => *playing = None,
        _ => {}
    }
    if playing.is_none() {
        return;
    }
    // Each line stays as long as it is spoken, whatever a run takes as read.
    state.scene.comm.step(&pad, false);
    let mut field = system::World { progress: &mut progress, scene: &mut state.scene, stats: &mut stats, tables: &tables, ground: &world };
    if let Err(error) = system.play_events(&mut field) {
        error!("the ride's talk failed: {error:#}");
    }
    let scene = &mut state.scene;
    board.ask_all(scene.sounds.drain(..).chain(scene.comm.sounds.drain(..)));
    if !scene.event_running && scene.events.is_empty() && !scene.comm.busy() {
        ride.talk_over();
        *playing = None;
    }
}

/// The ride on the bike is over: the field is in sight again, and starts anew
/// where its scripts said, with how the ride came out for them to ask after
/// (`s260`).
fn come_back_from_the_ride(
    mut commands: Commands,
    mut root: Single<&mut Visibility, With<FieldRoot>>,
    camera: Single<(Entity, &mut Camera), With<FollowCamera>>,
    mut state: ResMut<FieldState>,
    scenery: Res<Scenery>,
    ridden: Option<Res<crate::bike::show::Ridden>>,
) {
    if let Some(outcome) = ridden.and_then(|ridden| ridden.0) {
        state.scene.work.ride_outcome = outcome as i32;
    }
    info!("the ride on the bike is over: {}", state.scene.work.ride_outcome);
    state.fresh = state.riding.take();
    // The field comes up from black, as it does when it is begun.
    state.scene.fade = scene::Fade::DARK;
    **root = Visibility::Inherited;
    let (entity, mut camera) = camera.into_inner();
    camera.is_active = true;
    commands.entity(entity).insert(IsDefaultUiCamera);
    commands.insert_resource(backdrop_of(&scenery));
    commands.remove_resource::<crate::bike::show::Ridden>();
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn a_neck_that_looks_at_something_turns_the_head_to_it_as_far_as_it_may() {
        let limits = [60f32.to_radians(), 30f32.to_radians(), 20f32.to_radians()];
        let ahead = |turned: Quat| turned * Vec3::Z;
        let bearing = |way: Vec3| way.x.atan2(way.z).to_degrees();
        // Straight ahead and level with what looks out of the head, five above
        // its node: the head stays as it is.
        let turned = look(Quat::IDENTITY, Vec3::ZERO, Quat::IDENTITY, Vec3::new(0.0, 5.0, 100.0), limits, &mut None);
        assert!(ahead(turned).distance(Vec3::Z) < 1e-3 && (turned * Vec3::Y).distance(Vec3::Y) < 1e-3, "{turned:?}");
        // Level with the node itself, it is tipped down by what the five come to.
        let turned = look(Quat::IDENTITY, Vec3::ZERO, Quat::IDENTITY, Vec3::new(0.0, 0.0, 100.0), limits, &mut None);
        assert!((ahead(turned).y + (5.0f32 / 100.125).sin()).abs() < 1e-3, "{}", ahead(turned));
        // Forty degrees to one side it looks there; a quarter turn to the side
        // is past what it may, and it looks sixty degrees round.
        let at = |degrees: f32, side: &mut Option<bool>| {
            let way = Quat::from_rotation_y(degrees.to_radians()) * Vec3::Z;
            look(Quat::IDENTITY, Vec3::ZERO, Quat::IDENTITY, way * 1000.0 + Vec3::Y * 5.0, limits, side)
        };
        assert!((bearing(ahead(at(40.0, &mut None))) - 40.0).abs() < 0.1);
        let mut side = None;
        assert!((bearing(ahead(at(90.0, &mut side))) - 60.0).abs() < 0.5 && side == Some(false));
        assert!((bearing(ahead(at(-90.0, &mut None))) + 60.0).abs() < 0.5);
        // What goes round behind it, past where it is taken to be behind, does
        // not have the head swing over to the other side: it keeps to the side
        // it was turned to, near as far round as it may go.
        assert!((bearing(ahead(at(175.0, &mut side))) - 60.0).abs() < 0.5);
        let kept = bearing(ahead(at(-175.0, &mut side)));
        assert!(kept > 55.0 && kept < 75.0 && side == Some(false), "{kept}");
        // With no side kept it would be turned to the other.
        assert!(bearing(ahead(at(-175.0, &mut None))) < -55.0);
        // Up and down it may turn less, and not as far the one way as the other.
        let high = look(Quat::IDENTITY, Vec3::ZERO, Quat::IDENTITY, Vec3::new(0.0, 1000.0, 100.0), limits, &mut None);
        let low = look(Quat::IDENTITY, Vec3::ZERO, Quat::IDENTITY, Vec3::new(0.0, -1000.0, 100.0), limits, &mut None);
        let (up, down) = (ahead(high).y.asin().to_degrees(), ahead(low).y.asin().to_degrees());
        assert!(up > 15.0 && up < 35.0 && down < -15.0 && down > -35.0 && (up + down).abs() > 5.0, "{up} {down}");
        // A body that is turned has the limits turned with it.
        let body = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let turned = look(body, Vec3::ZERO, body, Vec3::new(0.0, 5.0, 1000.0), limits, &mut None);
        assert!((bearing(ahead(turned)) - 30.0).abs() < 0.5, "{}", bearing(ahead(turned)));
    }

    #[test]
    fn the_fields_tunables_say_how_far_a_neck_may_turn() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        let limits = PRM_NECK_LIMITS.map(|at| tables.tunables.u8(at).unwrap());
        eprintln!("a neck's limits: {limits:?} degrees");
        assert!(limits.iter().all(|&limit| limit > 0 && limit <= 180));
    }

    /// What stands far off is drawn through a lens of its own in the original
    /// (0x10ECC4: from 50 out to 200000). The port has one lens for the whole
    /// map, which has to reach whatever of a map's second piece can be seen from
    /// anywhere on its first.
    #[test]
    fn what_stands_far_off_is_within_how_far_the_camera_sees() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        let bounds = |map: &brs_formats::stcm::Map| {
            let places = map.batches.iter().flat_map(|batch| &batch.vertices).map(|vertex| Vec3::from(vertex.position));
            places.fold(None, |bounds: Option<(Vec3, Vec3)>, place| Some(bounds.map_or((place, place), |(low, high)| (low.min(place), high.max(place)))))
        };
        let mut furthest = (0.0f32, String::new());
        for entry in &tables.maps {
            let bundle = format!("FLD/MAP/MAP_{}.BIN/11.bin/01.bin", entry.name.to_ascii_uppercase());
            let (Ok((proper, _)), Ok((far, _))) = (data.map(format!("{bundle}/00.stcm")), data.map(format!("{bundle}/01.stcm"))) else { continue };
            let (Some((low, high)), Some((far_low, far_high))) = (bounds(&proper), bounds(&far)) else { continue };
            // From the corner of the first piece that is farthest from the
            // corner of the second that is farthest from it.
            let reach = (far_high - low).max(high - far_low).max(Vec3::ZERO).length();
            if reach > furthest.0 {
                furthest = (reach, entry.name.clone());
            }
        }
        eprintln!("the farthest anything far off can be: {:.0} on {}", furthest.0, furthest.1);
        assert!(furthest.0 > 0.0 && furthest.0 < CAMERA_FAR, "{furthest:?}");
    }
}
