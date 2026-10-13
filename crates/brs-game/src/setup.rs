//! The window the game opens when its data is not there yet.
//!
//! The port has none of the game in it: it plays from what the disc image of
//! the player's own copy is unpacked into. Started where that has not been
//! made, the game opens this window instead of its own: the disc image is
//! dropped on it (or chosen, with a click), the game is made ready in the
//! folder (`brs_extract::setup`, on a thread of its own, so that the window
//! goes on answering), and the game is started. It is the same program again
//! that is started, since a program has one window's worth of the system's
//! events to give out and this window has had it.
//!
//! A setup that was closed before it was through is gone on with as the game
//! is next started, from the disc image it was begun from.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Capturing};
use bevy::window::WindowResolution;
use brs_engine::offscreen::{screenshot, Offscreen};
use brs_extract::setup::{self, Asked, Doing, Sound};

pub use brs_extract::setup::BEGUN;

/// The window's size: the PSP's screen twice over.
const SIZE: (u32, u32) = (960, 544);
const BACKDROP: Color = Color::srgb(0.03, 0.04, 0.07);
const PANEL: Color = Color::srgb(0.07, 0.09, 0.14);
const PANEL_OVER: Color = Color::srgb(0.10, 0.16, 0.26);
const EDGE: Color = Color::srgb(0.25, 0.45, 0.85);
const EDGE_OVER: Color = Color::srgb(0.55, 0.80, 1.0);
const BRIGHT: Color = Color::srgb(0.92, 0.95, 1.0);
const DIM: Color = Color::srgb(0.55, 0.62, 0.74);
const WRONG: Color = Color::srgb(1.0, 0.55, 0.45);
/// Seconds the last word stays up before the game is started.
const BEFORE_THE_GAME: f32 = 1.0;

/// Where the game is made when it is nowhere yet: in the program's own folder;
/// or, where the program lies as a build of its source leaves it
/// (`target/<profile>` under the source), in the source's folder, since what a
/// build makes is cleared away.
pub fn home() -> PathBuf {
    let Ok(program) = std::env::current_exe() else { return PathBuf::from(".") };
    home_of(&program)
}

fn home_of(program: &Path) -> PathBuf {
    let Some(folder) = program.parent() else { return PathBuf::from(".") };
    let built = folder.parent().filter(|target| target.file_name().is_some_and(|name| name == "target")).and_then(Path::parent);
    built.filter(|source| source.join("Cargo.toml").is_file()).unwrap_or(folder).to_owned()
}

/// Whether the game is yet to be made in the folder the program is in now:
/// there is no data, or a setup was begun and is not through.
pub fn wanted(data: &Path) -> bool {
    !data.join("FLD").is_dir() || Path::new(BEGUN).is_file()
}

/// A run nobody watches: where its pictures go and when they are taken.
pub struct Unseen {
    pub path: PathBuf,
    pub at: Vec<f32>,
}

/// What the thread that makes the game says of itself.
#[derive(Default)]
struct Told {
    doing: Option<Doing>,
    ended: Option<Result<Sound, String>>,
}

enum Stage {
    /// Waiting for the disc image: with what went wrong the last time, and the
    /// window of the system's own for choosing it, while that is open.
    Asking { wrong: Option<String>, choosing: Option<Mutex<Receiver<Option<PathBuf>>>> },
    /// The game is being made.
    Making,
    /// It is made, but without its sound: the player says when to go on.
    Silent,
    /// It is made: the game is started when the time is up.
    Made(Timer),
}

#[derive(Resource)]
struct Setup {
    home: PathBuf,
    stage: Stage,
    told: Arc<Mutex<Told>>,
    /// A file is held over the window.
    over: bool,
    /// What the game is started with: what this was, less the disc image.
    again: Vec<String>,
    /// Whether the game is started at all: not in a run nobody watches.
    starts: bool,
}

impl Setup {
    /// Begins making the game from a disc image.
    fn begin(&mut self, image: PathBuf) {
        *self.told.lock().unwrap() = Told::default();
        self.stage = Stage::Making;
        let (told, folder) = (self.told.clone(), self.home.clone());
        std::thread::spawn(move || {
            let asked = Asked { image, folder, sound: true, jobs: setup::JOBS };
            let ended = setup::make(&asked, &|doing| told.lock().unwrap().doing = Some(doing));
            told.lock().unwrap().ended = Some(ended.map_err(|error| format!("{error:#}")));
        });
    }
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Headline;
#[derive(Component)]
struct Detail;
#[derive(Component)]
struct Track;
#[derive(Component)]
struct Bar;
#[derive(Component)]
struct Foot;

/// Opens the window and makes the game in `home`, from `image` if one was
/// given (dropped on the program, say) or a setup was begun there; then starts
/// the game with `again`.
pub fn open(home: PathBuf, image: Option<PathBuf>, again: Vec<String>, unseen: Option<Unseen>) -> Result<()> {
    let image = image.or_else(|| setup::begun_from(&home));
    let mut setup = Setup { home, stage: Stage::Asking { wrong: None, choosing: None }, told: default(), over: false, again, starts: unseen.is_none() };
    if let Some(image) = image {
        setup.begin(image);
    }
    let window = Window { title: "Black Rock Shooter: The Game - setting up".into(), resolution: WindowResolution::new(SIZE.0, SIZE.1), ..default() };
    let mut app = App::new();
    app.add_plugins(brs_engine::offscreen::plugins(window, unseen.is_some(), false))
        .insert_resource(ClearColor(BACKDROP))
        .insert_resource(setup)
        .add_systems(Startup, lay_out)
        .add_systems(Update, (take, follow, show).chain());
    if let Some(unseen) = unseen {
        app.insert_resource(Pictures { path: unseen.path, at: unseen.at, taken: 0, asked: -1.0, last: None }).add_systems(Update, picture.after(show));
    }
    app.run();
    Ok(())
}

fn text(words: &str, size: f32, colour: Color) -> impl Bundle {
    (Text::new(words), TextFont { font_size: bevy::text::FontSize::Px(size), ..default() }, TextColor(colour), TextLayout::justify(Justify::Center))
}

fn lay_out(mut commands: Commands, setup: Res<Setup>) {
    // The camera is named to what is laid out: left to find one, it looks for
    // the window's, and a run nobody watches has no window.
    let camera = commands.spawn(Camera2d).id();
    let foot = format!("The game is made in  {}\nIt takes about 1.4 GB. Your disc image is only read, never changed.", setup.home.display());
    commands.spawn((
        UiTargetCamera(camera),
        Node { width: percent(100), height: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: px(22), padding: UiRect::all(px(28)), ..default() },
        children![
            text("BLACK ROCK SHOOTER: THE GAME", 34.0, BRIGHT),
            text("the PC port", 18.0, DIM),
            (
                Panel,
                Node { width: percent(88), height: px(250), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: px(16), padding: UiRect::all(px(20)), border: UiRect::all(px(3)), border_radius: BorderRadius::all(px(14)), ..default() },
                BorderColor::all(EDGE),
                BackgroundColor(PANEL),
                children![
                    (Headline, text("", 26.0, BRIGHT)),
                    (Detail, text("", 17.0, DIM)),
                    (
                        Track,
                        Node { width: percent(86), height: px(14), border_radius: BorderRadius::all(px(7)), ..default() },
                        BackgroundColor(BACKDROP),
                        children![(Bar, Node { width: percent(0), height: percent(100), border_radius: BorderRadius::all(px(7)), ..default() }, BackgroundColor(EDGE))],
                    ),
                ],
            ),
            (Foot, text(&foot, 13.0, DIM)),
        ],
    ));
}

/// Takes the disc image: one let go over the window, or one chosen in the
/// system's window for it, which a click or Enter opens.
fn take(mut setup: ResMut<Setup>, mut dropped: MessageReader<FileDragAndDrop>, mouse: Res<ButtonInput<MouseButton>>, keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    let mut image = None;
    for drop in dropped.read() {
        match drop {
            FileDragAndDrop::HoveredFile { .. } => setup.over = true,
            FileDragAndDrop::HoveredFileCanceled { .. } => setup.over = false,
            FileDragAndDrop::DroppedFile { path_buf, .. } => {
                setup.over = false;
                image = Some(path_buf.clone());
            }
        }
    }
    let go = mouse.just_pressed(MouseButton::Left) || keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]);
    match &mut setup.stage {
        Stage::Asking { choosing, .. } => {
            if let Some(chosen) = choosing.as_ref().and_then(|chosen| chosen.lock().unwrap().try_recv().ok()) {
                *choosing = None;
                image = image.or(chosen);
            } else if go && choosing.is_none() && image.is_none() {
                let (answer, chosen) = mpsc::channel();
                std::thread::spawn(move || answer.send(setup::choose_the_image()));
                *choosing = Some(Mutex::new(chosen));
            }
            if let Some(image) = image {
                setup.begin(image);
            }
        }
        Stage::Silent if go => start(&setup, &mut exit),
        _ => {}
    }
}

/// Goes by what the thread that makes the game has come to.
fn follow(mut setup: ResMut<Setup>, time: Res<Time>, mut exit: MessageWriter<AppExit>) {
    match &mut setup.stage {
        Stage::Making => {
            let Some(ended) = setup.told.lock().unwrap().ended.take() else { return };
            setup.stage = match ended {
                Ok(Sound::NoFfmpeg) => Stage::Silent,
                Ok(_) => Stage::Made(Timer::from_seconds(BEFORE_THE_GAME, TimerMode::Once)),
                Err(wrong) => Stage::Asking { wrong: Some(wrong), choosing: None },
            };
        }
        Stage::Made(wait) => {
            if wait.tick(time.delta()).just_finished() {
                start(&setup, &mut exit);
            }
        }
        _ => {}
    }
}

/// Starts the game where it was made, and ends this.
fn start(setup: &Setup, exit: &mut MessageWriter<AppExit>) {
    if !setup.starts {
        return;
    }
    let started = std::env::current_exe().and_then(|program| std::process::Command::new(program).args(&setup.again).current_dir(&setup.home).spawn());
    if let Err(error) = started {
        error!("the game could not be started: {error}");
    }
    exit.write(AppExit::Success);
}

/// A share of a whole as the bar shows it.
fn share(done: u64, of: u64) -> f32 {
    if of == 0 {
        return 0.0;
    }
    (done as f32 / of as f32).clamp(0.0, 1.0)
}

/// What the window says, and how full its bar is, if it shows one.
fn words(setup: &Setup) -> (String, String, Option<f32>, bool) {
    match &setup.stage {
        Stage::Asking { wrong: Some(wrong), .. } => ("It did not work".into(), format!("{wrong}\n\nDrop the disc image (.iso) here to try again, or click to choose it."), None, true),
        Stage::Asking { choosing: Some(_), .. } => ("Choose the disc image".into(), "A window has opened for it.".into(), None, false),
        Stage::Asking { .. } if setup.over => ("Let go to begin".into(), String::new(), None, false),
        Stage::Asking { .. } => ("Drop the game's disc image (.iso) here".into(), "the .iso of your own copy of the game\n\nor click to choose it".into(), None, false),
        Stage::Making => match setup.told.lock().unwrap().doing {
            None => ("Reading the disc image".into(), String::new(), Some(0.0), false),
            Some(Doing::Films) => ("1 of 3   Copying the films".into(), String::new(), Some(0.0), false),
            Some(Doing::Data { done, of }) => ("2 of 3   Unpacking the game".into(), format!("{done} of {of} files of the disc"), Some(share(done as u64, of as u64)), false),
            Some(Doing::Ffmpeg { fetched }) => (
                "3 of 3   Fetching ffmpeg for the sound".into(),
                format!("{} MB of about {}, from gyan.dev, this once: it decodes the game's music and voices", fetched >> 20, setup::FFMPEG_BYTES >> 20),
                Some(share(fetched, setup::FFMPEG_BYTES)),
                false,
            ),
            Some(Doing::Sound { done, of }) => (
                "3 of 3   Making the sound playable".into(),
                format!("{done} of {of} recordings of music and voices. This is the longest part, a few minutes.\nIf the window is closed, it goes on from here the next time."),
                Some(share(done as u64, of as u64)),
                false,
            ),
        },
        Stage::Silent => (
            "The game is ready, but without its sound".into(),
            "ffmpeg, which decodes the sound, could not be fetched (is the internet there?).\nClick or press Enter to play without sound.\nFor the sound, drop the disc image on the program again another time.".into(),
            None,
            true,
        ),
        Stage::Made(_) => ("The game is ready".into(), "Starting it...".into(), Some(1.0), false),
    }
}

/// Writes what the window says.
fn show(
    setup: Res<Setup>,
    mut headline: Query<&mut Text, (With<Headline>, Without<Detail>)>,
    mut detail: Query<(&mut Text, &mut TextColor), (With<Detail>, Without<Headline>)>,
    mut track: Query<&mut Visibility, With<Track>>,
    mut bar: Query<&mut Node, With<Bar>>,
    mut panel: Query<(&mut BackgroundColor, &mut BorderColor), With<Panel>>,
) {
    let (first, second, filled, wrong) = words(&setup);
    if let Ok(mut headline) = headline.single_mut() {
        if **headline != first {
            **headline = first;
        }
    }
    if let Ok((mut detail, mut colour)) = detail.single_mut() {
        if **detail != second {
            **detail = second;
        }
        colour.set_if_neq(TextColor(if wrong { WRONG } else { DIM }));
    }
    if let Ok(mut track) = track.single_mut() {
        track.set_if_neq(if filled.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }
    if let (Ok(mut bar), Some(filled)) = (bar.single_mut(), filled) {
        let width = percent(filled * 100.0);
        if bar.width != width {
            bar.width = width;
        }
    }
    if let Ok((mut ground, mut edge)) = panel.single_mut() {
        ground.set_if_neq(BackgroundColor(if setup.over { PANEL_OVER } else { PANEL }));
        edge.set_if_neq(BorderColor::all(if setup.over { EDGE_OVER } else { EDGE }));
    }
}

/// In a run nobody watches: the pictures it saves, and the one it ends on.
#[derive(Resource)]
struct Pictures {
    path: PathBuf,
    at: Vec<f32>,
    taken: usize,
    /// When a picture was last asked for.
    asked: f32,
    /// When the last picture of all was asked for, once it has been.
    last: Option<f32>,
}

/// Saves a picture at each appointed time and one as the setup ends (or as the
/// times run out with nothing being made), then ends the run.
fn picture(mut commands: Commands, mut pictures: ResMut<Pictures>, setup: Res<Setup>, offscreen: Option<Res<Offscreen>>, time: Res<Time<Real>>, capturing: Query<(), With<Capturing>>, mut exit: MessageWriter<AppExit>) {
    let now = time.elapsed_secs();
    let (path, stem) = (pictures.path.clone(), pictures.path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    let named = |name: String| path.with_file_name(format!("{stem}_{name}.png"));
    if let Some(last) = pictures.last {
        if now >= last + 1.5 && capturing.is_empty() {
            exit.write(AppExit::Success);
        }
        return;
    }
    // One picture at a time.
    if !capturing.is_empty() || now < pictures.asked + 0.5 {
        return;
    }
    if pictures.at.get(pictures.taken).is_some_and(|&at| now >= at) {
        commands.spawn(screenshot(offscreen.as_deref())).observe(save_to_disk(named((pictures.taken + 1).to_string())));
        pictures.taken += 1;
        pictures.asked = now;
        return;
    }
    let over = pictures.taken >= pictures.at.len();
    if !matches!(setup.stage, Stage::Making) && over {
        commands.spawn(screenshot(offscreen.as_deref())).observe(save_to_disk(named("end".into())));
        pictures.last = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_is_made_beside_the_program_or_in_the_source_it_was_built_from() {
        // A program somebody was given: its own folder, whatever lies above it.
        assert_eq!(home_of(Path::new("/games/brs/brs-game")), Path::new("/games/brs"));
        // A build of this source: the source's folder, not what the build makes.
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).unwrap();
        assert_eq!(home_of(&source.join("target").join("debug").join("brs-game")), source);
        // A folder that is only named as a build's is: the program's own.
        assert_eq!(home_of(Path::new("/nowhere/target/debug/brs-game")), Path::new("/nowhere/target/debug"));
    }

    #[test]
    fn the_bar_is_as_full_as_the_share_that_is_done() {
        assert_eq!(share(0, 0), 0.0);
        assert_eq!(share(1, 4), 0.25);
        assert_eq!(share(9, 4), 1.0);
    }

    #[test]
    fn the_window_says_what_is_being_done() {
        let mut setup = Setup { home: PathBuf::from("here"), stage: Stage::Asking { wrong: None, choosing: None }, told: default(), over: false, again: Vec::new(), starts: false };
        assert!(words(&setup).0.contains("Drop"));
        setup.over = true;
        assert_eq!(words(&setup).0, "Let go to begin");
        setup.stage = Stage::Making;
        setup.told.lock().unwrap().doing = Some(Doing::Data { done: 5, of: 10 });
        let (first, second, filled, wrong) = words(&setup);
        assert!(first.starts_with("2 of 3") && second.starts_with("5 of 10") && filled == Some(0.5) && !wrong);
        setup.stage = Stage::Asking { wrong: Some("this is not a disc image".into()), choosing: None };
        let (first, second, filled, wrong) = words(&setup);
        assert!(first == "It did not work" && second.starts_with("this is not a disc image") && filled.is_none() && wrong);
    }
}
