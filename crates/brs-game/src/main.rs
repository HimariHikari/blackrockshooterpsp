//! The port: a native reimplementation that plays from the original game's data.
//!
//! Usage: `brs-game [--data <unpacked folder>] [--title] [--cleared] [--map <stage>]
//!         [--mission <number>]
//!         [--start <x>,<z>] [--play-events] [--skip-battles] [--skip-messages]
//!         [--lose-battle <group>] [--fight <group>] [--stage-select]
//!         [--battle <stage>] [--group <number>] [--level <1-50>] [--afflict <effects>]
//!         [--ride <course>]
//!         [--skills <triangle>,<circle>,<cross>,<square>] [--auto <skills>]
//!         [--has <skills>] [--carry <items>] [--wear <items>]
//!         [--flags <flags>] [--variables <variable>:<value>,...]
//!         [--saves <folder>] [--script <steps>]
//!         [--screenshot <png>] [--screenshot-at <seconds>] [--background] [--window]
//!         [--mute]
//!         [--slow <seconds>:<times>] [--verbose]`
//!
//! By default the game begins as the original does, on its title: the maker's
//! mark, PRESS START, and the menu on which NEW GAME begins a game (`--title` says
//! so outright; `--cleared` has the title be that of a game that has been
//! cleared). Any of `--map`, `--mission`, `--start` and `--stage-select` begins
//! past it instead: a new game starts on the field map `--map` names (`STG01_01` if left
//! out): the map's script sets it up, and walking into an enemy starts a battle, which
//! comes back to the field when it is over. The events the script plays cannot be
//! shown yet, so each is skipped as soon as the game lets it be; `--play-events` has it take
//! the time it takes in the original (a game begun from the title plays them
//! without being told). `--start` puts the player somewhere else, and
//! `--skip-battles` counts every battle as won without fighting it (and has the
//! ride on the bike ridden by a rule of thumb, with her taking no harm), and
//! `--skip-messages` takes every window a script puts up as read (a question as
//! answered with what the cursor starts on, and a scene's caption that waits
//! for a button as passed), for checking the field's own flow unattended. `--lose-battle 100` counts a battle against that enemy group as lost
//! without fighting it, to see what the field does about a mission failed.
//! `--fight 103` has the battle against that group fought in a run that passes
//! battles over, to come to one battle by way of the field.
//! `--mission` starts on another mission than the first (they are
//! numbered as `tools_re/missions.py` lists them), on the map it begins on, as if it
//! had been chosen from the list of missions. `--stage-select` begins the run on
//! the screen between missions, as it is come to when that mission has been
//! cleared: left and right change the stage, cross on it goes to its missions,
//! and cross on one of those asks whether to begin it.
//!
//! `--battle STG01_01` starts a battle on that battle map and nothing else, against
//! the enemy group `--group` names (2, the game's first battle, if left out).
//! `--ride normal` rides that course of the ride on the bike and nothing else
//! (`easy`, `normal`, `hard`, `ex00`, `ex01`, `ex_easy`, `ex_normal`, `ex_hard`).
//! `--level` gives her the figures of that level instead of a new game's, on the
//! field as well. `--afflict 602,900` gives her and every enemy those status effects
//! (numbered as `SYSTEM.VOL/btl_eff_tbl.bin` has them) as the first fight begins, for
//! a look at their marks by the gauges and at what the battle's log says of them.
//! `--skills 25,24,0,0` puts skills in her four slots, on triangle, circle, cross
//! and square (numbered as `SYSTEM.VOL/btl_skl_tbl.bin` has them, nought for
//! none), from the start: in a game the menu's CUSTOMIZE does it. In a
//! battle R held and a slot's button uses its skill: all thirty of them.
//! `--auto 33,36,37` has her have skills that act of themselves in a battle (30 to
//! 38), which she otherwise has by the save's flags: the game gives them out
//! for its records. `--has 1,2,23,30,42` has her have those skills from the start
//! of a run on the field, for CUSTOMIZE to list, and `--carry 1:3,6,132` has her
//! carry those items (three of the first; numbered as the three tables of
//! `SYSTEM.VOL/db_resident.lpk` have them, one after the other), for ITEMS.
//! `--wear 132,136` has her carry and have on those weapons and suits (the
//! Hyperion and the Schoolgirl suit) from the start, on the field or in a battle
//! run alone. `--flags 6157,6158,6600,6802` sets flags of the save and
//! `--variables 20000:3` gives variables of it from the start of a run on the
//! field: those are the first two stages seen, the first record open with three
//! counted and the third cleared, for CHALLENGES to show.
//!
//! Games are saved in the folder `save` beside where the game is run from, as
//! `DATA00.sav` to `DATA19.sav`; `--saves <folder>` names another, which a run
//! that is only a check should do.
//!
//! The controls are the PSP's, on the keyboard or a gamepad: see `controls.rs` for
//! the keys. On the field the stick moves and L/R turn the camera; dragging with the
//! right mouse button or the right stick turns it too, and the scroll wheel zooms.
//! Triangle brings up the menu (up and down move its cursor, circle leaves it,
//! cross on CUSTOMIZE goes to the screen her skills are set with: L and R change
//! its list, cross sets, takes out or switches a skill, circle goes back; and
//! cross on ITEMS to what she carries: L and R change the list, cross uses a
//! consumable or changes to a weapon or a suit; cross on CHALLENGES to the
//! records: L and R change the stage, the D-pad moves over its sixteen; and
//! cross on OPTIONS to the volumes and to whether a scene's captions wait for
//! a button: up and down choose, left and right change, square asks about the
//! defaults).
//! Cross looks at what she stands by, turns the page of a window and makes a choice
//! (up and down move the cursor, circle puts it on the choice that changes nothing),
//! L and R answer a question asked over the radio, and start skips a story scene.
//! In battle the stick or D-pad
//! aims, square shoots, circle dodges and cross guards.
//!
//! `--script` replaces the controls with scripted input, e.g. `forward:2,left:1`
//! (see `controls::Script::parse`), and `--screenshot` saves one frame when the script
//! ends (or `--screenshot-at` seconds in) and exits. Together they let a change be
//! checked without anyone at the keyboard. `--screenshot-at` takes several times as
//! well, as in `20,45.5,60`: a frame is saved at each, the name numbered from 1
//! (`shot_1.png`, `shot_2.png`), and the game exits after the last. Such a run opens
//! no window: it draws to a picture in memory, so that it can go on while the machine
//! is used for something else, and puts no sound out. `--background` does the same
//! for a run that saves nothing. `--window` has such a run open its window after
//! all, for checking what only a window shows (what a change of its size does).
//! `--slow 42:8` has time go eight times slower from 42 seconds in, so that every
//! frame of the game's logic after that is drawn at least once whatever the machine
//! is busy with: frames saved a thirtieth of a second apart are then one after
//! another of them. The times `--screenshot-at` and `--script` take are the game's
//! own, which go slower with it.
//! `--mute` has the sound played as silence. With a run that opens no window it has
//! the sound played after all, as silence: for checking the way it takes.
//! `--verbose` logs what the game's own logic does, such as each change in what an
//! enemy's script is up to.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Capturing};
use bevy::window::WindowResolution;
use brs_engine::offscreen::{screenshot, Offscreen};
use brs_engine::EnginePlugin;

mod battle;
mod bike;
mod shadow;
mod controls;
mod data;
mod field;
mod film;
mod gallery;
mod hub;
mod interface;
mod items;
mod menu;
mod options;
mod pause;
mod player;
mod progress;
mod record_news;
mod record_window;
mod clock;
mod records;
mod save;
mod save_screen;
mod sound;
mod text;
mod title;
mod writing;

use battle::tables::{Levels, PlayerStats};
use controls::Script;
use data::GameData;
use items::{Inventory, Items};

const DEFAULT_DATA: &str = "game/unpacked";
const DEFAULT_MAP: &str = "STG01_01";
/// The enemy group of the game's first battle: two Eaters and two of the flying kind.
const DEFAULT_GROUP: u16 = 2;

/// What the log leaves out by default, as Bevy has it.
const DEFAULT_LOG_FILTER: &str = bevy::log::DEFAULT_FILTER;

/// What the game is showing: the field, or a battle that has been walked into on it.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Field,
    Battle,
    /// The ride on the bike, the game within the game.
    Ride,
}

/// Seconds to let the scene settle (asset upload, shader compilation) before a screenshot.
const SCREENSHOT_DELAY: f32 = 3.0;
/// Seconds after the screenshot request before the game may exit.
const EXIT_DELAY: f32 = 1.5;

struct Args {
    data: PathBuf,
    map: String,
    start: Option<Vec2>,
    /// Whether the game begins on its title: as asked for, or else unless a
    /// place to begin was named.
    title: Option<bool>,
    field: field::Options,
    /// The battle map to fight on, instead of walking the field.
    battle: Option<String>,
    /// The course of the ride on the bike to ride, and nothing else.
    ride: Option<String>,
    /// The enemy group to fight there, and the player's level.
    group: u16,
    level: u32,
    /// Status effects to begin the first fight with.
    afflict: Vec<i32>,
    /// The skills to put in her slots, if any were named.
    skills: Option<[u8; 4]>,
    /// Skills that act of themselves for her to have in a battle.
    auto: Vec<i32>,
    /// Skills she has from the start of a run on the field.
    has: Vec<i32>,
    /// Items she carries from the start, and how many of each.
    carry: Vec<(i32, i32)>,
    /// Weapons and suits she has on from the start.
    wear: Vec<i32>,
    /// Flags of the save set from the start of a run on the field, and
    /// variables of it with what each is.
    flags: Vec<i32>,
    variables: Vec<(i32, i32)>,
    /// The folder games are saved in.
    saves: PathBuf,
    script: Script,
    screenshot: Option<PathBuf>,
    /// When to take the screenshots, in seconds, if not one as the script ends.
    screenshot_at: Vec<f32>,
    /// From so many seconds in, time goes so many times slower.
    slow: Option<(f32, f32)>,
    /// Run without a window.
    background: bool,
    /// Show the window even in a run that saves its pictures: for what only a
    /// window shows.
    window: bool,
    /// The sound is played as silence.
    mute: bool,
    verbose: bool,
}

/// In a verification run: from when time goes slower, and how many times.
#[derive(Resource)]
struct Slow {
    after: f32,
    times: f32,
}

/// Has time go slower once it is time for that.
fn slow(slow: Res<Slow>, mut time: ResMut<Time<Virtual>>) {
    let speed = slow.times.recip();
    if time.elapsed_secs() >= slow.after && time.relative_speed() != speed {
        time.set_relative_speed(speed);
    }
}

/// In a verification run: where to save the frames, and when.
#[derive(Resource)]
struct Capture {
    path: PathBuf,
    at: Vec<f32>,
}

impl Capture {
    /// Where frame `index` goes: the path as given if there is only the one,
    /// otherwise with its number after the name.
    fn path(&self, index: usize) -> PathBuf {
        if self.at.len() == 1 {
            return self.path.clone();
        }
        let stem = self.path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = self.path.extension().unwrap_or_default().to_string_lossy();
        self.path.with_file_name(format!("{stem}_{}.{extension}", index + 1))
    }
}

/// Started somewhere the game's data is not (by a double click on the program,
/// say, which starts it in its own folder), the game goes to the folder above
/// the program that has it, so that the data, the saves and the rest are where
/// they are when it is started from the project's folder.
fn go_to_the_data(data: &Path) {
    if data != Path::new(DEFAULT_DATA) || data.join("FLD").is_dir() {
        return;
    }
    let Ok(program) = std::env::current_exe() else { return };
    if let Some(home) = program.ancestors().skip(1).find(|folder| folder.join(DEFAULT_DATA).join("FLD").is_dir()) {
        if let Err(error) = std::env::set_current_dir(home) {
            eprintln!("could not go to {}: {error}", home.display());
        }
    }
}

fn main() -> Result<()> {
    let args = parse_args()?;
    go_to_the_data(&args.data);
    let data = GameData::new(&args.data)?;

    let mut app = App::new();
    let window = Window {
        title: "Black Rock Shooter: The Game".into(),
        resolution: WindowResolution::new(1280, 720),
        ..default()
    };
    // A run that saves its pictures and exits has nobody watching it.
    let offscreen = (args.background || args.screenshot.is_some()) && !args.window;
    let log = if args.verbose { LogPlugin { filter: format!("{DEFAULT_LOG_FILTER},brs_game=debug"), ..default() } } else { LogPlugin::default() };
    // Nor anybody listening, unless it is to silence.
    let out = match (args.mute, offscreen) {
        (true, _) => sound::Out::Muted,
        (false, true) => sound::Out::Off,
        (false, false) => sound::Out::On,
    };
    app.add_plugins(brs_engine::offscreen::plugins(window, offscreen, out != sound::Out::Off).set(log))
        .add_plugins((EnginePlugin { stepped: true }, brs_engine::canvas::plugin, controls::plugin, player::plugin));
    sound::plugin(&mut app, &data, out)?;

    // What the interface is written and drawn with.
    let mut images = app.world_mut().resource_mut::<Assets<Image>>();
    let fonts = writing::Fonts::load(&data, &mut images).context("reading the fonts")?;
    let layouts = field::hud::Layouts::load(&data, &mut images).context("reading the field's layouts")?;
    let battle_display = battle::hud::Hud::load(&data, &mut images).context("reading the battle's layouts")?;
    let record_window = record_window::RecordWindow::load(&data, &mut images).context("reading the window of the records")?;
    let record_news = record_news::RecordNews::load(&data, &mut images).context("reading the panels of the records")?;
    let menus = menu::MenuAssets::load(&data, &mut images).context("reading the menus' layouts")?;
    let stage_select = hub::HubAssets::load(&data, &mut images).context("reading the stage select's layout")?;
    let title = title::TitleAssets::load(&data, &mut images).context("reading the title's layout")?;
    let pause = pause::PauseAssets::load(&data, &mut images).context("reading the pause's layout")?;
    let maps = field::map_screen::MapAssets::load(&data, &mut images).context("reading the stages' maps")?;
    // The picture every save is shown with is the game's own, which is not of the
    // game's data but beside it on the disc: without it a plain panel is drawn.
    let icon = args.data.parent().map(|beside| beside.join("extracted/PSP_GAME/ICON0.PNG")).and_then(|path| std::fs::read(path).ok());
    let icon = icon.and_then(|bytes| {
        let read = Image::from_buffer(&bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, false, ImageSampler::linear(), RenderAssetUsages::RENDER_WORLD);
        read.inspect_err(|error| warn!("the saves' picture cannot be read: {error}")).ok()
    });
    let icon = save_screen::Icon(icon.map(|icon| images.add(icon)));
    app.insert_resource(menus).insert_resource(stage_select).insert_resource(title).insert_resource(pause).insert_resource(maps).insert_resource(icon);
    // The system data is read as the game starts (0x5278): the options as they
    // were left, and whether the game has been cleared.
    let saves = save::Saves::new(&args.saves);
    let mut args = args;
    app.init_resource::<gallery::Opened>();
    if let Some(system) = saves.read_system() {
        app.insert_resource(system.options).insert_resource(system.gallery);
        args.field.cleared |= system.cleared;
    }
    app.insert_resource(gallery::Gallery::load(&data).context("reading the gallery's tables")?);
    app.insert_resource(saves).init_resource::<save::Played>();
    app.insert_resource(fonts).insert_resource(layouts).insert_resource(battle_display).insert_resource(record_window).insert_resource(record_news);

    // What she is and has: a new game's, or what the level asked for comes to.
    let levels = Levels::load(&data).context("reading the level table")?;
    let stats = PlayerStats::new_game(&data).context("reading the player's starting figures")?;
    let mut stats = stats.at_level(&levels, args.level)?;
    if let Some(slots) = args.skills {
        stats.skills = slots;
    }
    let items = Items::load(&data).context("reading the item tables")?;
    let mut inventory = Inventory::new_game(&items);
    for &(item, count) in &args.carry {
        inventory.add(&items, item, count);
    }
    for &item in &args.wear {
        inventory.add(&items, item, 1);
        stats.equip(&items, item);
    }
    let colours = text::TextColours::load(&data).context("reading the text colours")?;
    let records = records::Records::load(&data).context("reading the records' script")?;
    let skills = battle::skills::Skills::load(&data).context("reading the skill table")?;
    battle::skills::plugin(&mut app);
    shadow::plugin(&mut app);
    app.insert_resource(stats).insert_resource(levels).insert_resource(items).insert_resource(inventory).insert_resource(colours);

    match &args.battle {
        // A ride on the bike and nothing else.
        _ if args.ride.is_some() => {
            let course = bike::show::Course(args.ride.clone().unwrap());
            app.insert_state(Mode::Ride).insert_resource(ClearColor(field::BACKDROP)).insert_resource(course).insert_resource(progress::Progress::start()).add_plugins((bike::plugin, pause::plugin));
        }
        // A battle and nothing else: it stays on its end, with no field to go back to.
        // It is fought as in a game just begun, in the mission asked for if one was.
        Some(stage) => {
            let battle = battle::BattleSource::load(&data, stage, args.group, stats, battle::Terms::default(), &skills)?;
            let mut progress = progress::Progress::start();
            progress.begin_first_mission();
            if let Some(mission) = args.field.mission {
                progress.set_variable(progress::variable::SCENARIO, mission);
            }
            // With no story behind it, she can dodge from the start.
            progress.set_flag(battle::actor::DODGE_FLAG, true);
            let mut figures = stats;
            records.initialize(&mut records::World { progress: &mut progress, stats: &mut figures, stage: 0 })?;
            app.insert_state(Mode::Battle).insert_resource(battle).insert_resource(progress).add_plugins((battle::plugin, pause::plugin));
        }
        None => {
            // A game begun from its title is one somebody plays: its story is
            // shown, as `--play-events` has it for a run that begins past it.
            let title = args.title.unwrap_or(true);
            let options = field::Options { title, play_events: args.field.play_events || title, ..args.field };
            let field = field::FieldSource::load(&data, &args.map, args.start, options)?.having(args.has.clone()).with_save(args.flags.clone(), args.variables.clone());
            app.init_state::<Mode>()
                .insert_resource(ClearColor(field::BACKDROP))
                .insert_resource(field)
                .add_plugins((field::plugin, battle::plugin, menu::plugin, hub::plugin, title::plugin, save_screen::plugin, pause::plugin, film::plugin, bike::plugin));
        }
    }
    app.insert_resource(data).insert_resource(records).insert_resource(skills).insert_resource(battle::Afflict(args.afflict)).insert_resource(battle::Automatic(args.auto)).add_plugins((record_news::plugin, record_window::plugin));

    if let Some((after, times)) = args.slow {
        app.insert_resource(Slow { after, times }).add_systems(First, slow);
    }
    if let Some(path) = args.screenshot {
        let mut at = args.screenshot_at;
        if at.is_empty() {
            at.push(args.script.seconds().max(SCREENSHOT_DELAY));
        }
        app.insert_resource(Capture { path, at }).add_systems(Update, capture);
    }
    app.insert_resource(args.script).run();
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = Args {
        data: DEFAULT_DATA.into(),
        map: DEFAULT_MAP.into(),
        start: None,
        title: None,
        field: field::Options::default(),
        battle: None,
        ride: None,
        group: DEFAULT_GROUP,
        level: 1,
        afflict: Vec::new(),
        skills: None,
        auto: Vec::new(),
        has: Vec::new(),
        carry: Vec::new(),
        wear: Vec::new(),
        flags: Vec::new(),
        variables: Vec::new(),
        saves: save::DEFAULT_FOLDER.into(),
        script: Script::default(),
        screenshot: None,
        screenshot_at: Vec::new(),
        slow: None,
        background: false,
        window: false,
        mute: false,
        verbose: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().with_context(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--data" => args.data = value()?.into(),
            "--title" => args.title = Some(true),
            "--cleared" => args.field.cleared = true,
            "--map" => {
                args.map = value()?;
                args.title.get_or_insert(false);
            }
            "--start" => {
                let text = value()?;
                let (x, z) = text.split_once(',').context("--start takes <x>,<z>")?;
                args.start = Some(Vec2::new(x.parse()?, z.parse()?));
                args.title.get_or_insert(false);
            }
            "--play-events" => args.field.play_events = true,
            "--skip-battles" => args.field.skip_battles = true,
            "--lose-battle" => args.field.lose_battle = Some(value()?.parse().context("--lose-battle takes an enemy group's number")?),
            "--fight" => args.field.fight = Some(value()?.parse().context("--fight takes an enemy group's number")?),
            "--skip-messages" => args.field.skip_messages = true,
            "--stage-select" => {
                args.field.stage_select = true;
                args.title.get_or_insert(false);
            }
            "--mission" => {
                args.field.mission = Some(value()?.parse().context("--mission takes a number")?);
                args.title.get_or_insert(false);
            }
            "--battle" => args.battle = Some(value()?),
            "--ride" => args.ride = Some(value()?),
            "--group" => args.group = value()?.parse().context("--group takes a number")?,
            "--level" => args.level = value()?.parse().context("--level takes a number")?,
            "--afflict" => {
                let effects: Result<Vec<i32>, _> = value()?.split(',').map(str::parse).collect();
                args.afflict = effects.context("--afflict takes status effects' numbers with commas between")?;
            }
            "--auto" => {
                let skills: Result<Vec<i32>, _> = value()?.split(',').map(str::parse).collect();
                args.auto = skills.context("--auto takes skills' numbers with commas between")?;
            }
            "--has" => {
                let skills: Result<Vec<i32>, _> = value()?.split(',').map(str::parse).collect();
                args.has = skills.context("--has takes skills' numbers with commas between")?;
            }
            "--carry" => {
                let one = |named: &str| -> Result<(i32, i32)> {
                    let (item, count) = named.split_once(':').unwrap_or((named, "1"));
                    Ok((item.parse()?, count.parse()?))
                };
                let items: Result<Vec<(i32, i32)>> = value()?.split(',').map(one).collect();
                args.carry = items.context("--carry takes items' numbers with commas between, each with a colon and how many if not one")?;
            }
            "--wear" => {
                let items: Result<Vec<i32>, _> = value()?.split(',').map(str::parse).collect();
                args.wear = items.context("--wear takes items' numbers with commas between")?;
            }
            "--flags" => {
                let flags: Result<Vec<i32>, _> = value()?.split(',').map(str::parse).collect();
                args.flags = flags.context("--flags takes flags' numbers with commas between")?;
            }
            "--variables" => {
                let one = |named: &str| -> Result<(i32, i32)> {
                    let (variable, value) = named.split_once(':').context("a variable is given as <number>:<value>")?;
                    Ok((variable.parse()?, value.parse()?))
                };
                let variables: Result<Vec<(i32, i32)>> = value()?.split(',').map(one).collect();
                args.variables = variables.context("--variables takes variables' numbers with commas between, each with a colon and its value")?;
            }
            "--skills" => {
                let named: Result<Vec<u8>, _> = value()?.split(',').map(str::parse).collect();
                let named = named.context("--skills takes skills' numbers with commas between")?;
                anyhow::ensure!(named.len() <= 4, "she has four slots for skills");
                let mut slots = [0; 4];
                slots[..named.len()].copy_from_slice(&named);
                args.skills = Some(slots);
            }
            "--saves" => args.saves = value()?.into(),
            "--script" => args.script = Script::parse(&value()?)?,
            "--screenshot" => args.screenshot = Some(value()?.into()),
            "--screenshot-at" => {
                let times = value()?;
                let times: Result<Vec<f32>, _> = times.split(',').map(str::parse).collect();
                args.screenshot_at = times.context("--screenshot-at takes seconds, or several with commas between")?;
                args.screenshot_at.sort_by(f32::total_cmp);
            }
            "--slow" => {
                let given = value()?;
                let (after, times) = given.split_once(':').context("--slow takes seconds and how many times slower, with a colon between")?;
                let times: f32 = times.parse().context("--slow takes how many times slower as a number")?;
                anyhow::ensure!(times > 0.0, "--slow takes how many times slower as more than nought");
                args.slow = Some((after.parse().context("--slow takes seconds as a number")?, times));
            }
            "--background" => args.background = true,
            "--window" => args.window = true,
            "--mute" => args.mute = true,
            "--verbose" => args.verbose = true,
            _ => bail!("unknown option {arg}; see the top of crates/brs-game/src/main.rs for usage"),
        }
    }
    Ok(args)
}

/// Saves a frame at each appointed time, then exits.
fn capture(
    mut commands: Commands,
    capture: Res<Capture>,
    offscreen: Option<Res<Offscreen>>,
    time: Res<Time>,
    mut taken: Local<usize>,
    capturing: Query<(), With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match capture.at.get(*taken) {
        Some(&at) if elapsed >= at => {
            commands.spawn(screenshot(offscreen.as_deref())).observe(save_to_disk(capture.path(*taken)));
            *taken += 1;
        }
        Some(_) => {}
        None => {
            let last = capture.at.last().copied().unwrap_or(0.0);
            if elapsed >= last + EXIT_DELAY && capturing.is_empty() {
                exit.write(AppExit::Success);
            }
        }
    }
}
