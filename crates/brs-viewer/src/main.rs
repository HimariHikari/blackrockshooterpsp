//! Shows one model, map or layout of the interface from the game data.
//!
//! Usage: `brs-viewer <file> [--anim <animation file>]... [--camera <camera file>]
//!         [--time <seconds>] [--collision <file>]... [--screenshot <png>]
//!         [--yaw <degrees>] [--pitch <degrees>] [--distance <factor>] [--background]
//!         [--set <number,number..>] [--frame <frames>] [--texture <file>]...
//!         [--effect <number>]`
//!
//! A layout is named by its objects' file (`.sdoa`): its motions, its groups and its
//! textures are the files after it. `--set` names the sets of it to play, each over
//! the ones before (the first if left out), and `--frame` stops them so many frames
//! in (sixtieths of a second) instead of letting them play. A layout whose textures
//! are not the files after it has them named one by one with `--texture`, in the
//! order the game's code lists them.
//!
//! Models: drag with the left mouse button to orbit, scroll to zoom. Maps: hold the
//! right mouse button to look, WASD to move, Q/E for down/up, Shift to go faster.
//! An effect file (`.efc`, `.efp`) plays one of its effects at the middle of the
//! world, over and over: the one `--effect` numbers, or its first. `--frame` holds
//! it so many of its frames in. The log lists the numbers a file has.
//!
//! The first `--anim` is the body's animation; further ones are layered on top, each
//! driving only its own nodes (a hair or face animation). `--camera` looks through a
//! camera move (`.cam`) made for the model instead, in step with the body's
//! animation. `--time` freezes the animations at that point. `--parts 60,71` shows
//! only those of the model's parts (`--parts none` hides them all); the log lists the
//! ids a model has. `--collision` overlays a collision file on a map. `--distance`
//! moves the orbiting camera that many times as far off as it would stand. With
//! `--screenshot` the viewer saves one frame and exits. It then opens no window, and
//! draws to a picture in memory instead, so that it can run while the machine is used
//! for something else; `--background` does the same for a run that saves nothing.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use bevy::{
    asset::RenderAssetUsages,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::{
        render_resource::PrimitiveTopology,
        view::screenshot::{save_to_disk, Capturing},
    },
    window::WindowResolution,
};
use brs_engine::animation::{Animator, Clip, FRAMES_PER_SECOND};
use brs_engine::camera::CameraClip;
use brs_engine::canvas::Canvas;
use brs_engine::effects::{EffectCamera, EffectFile, EffectPool, EffectSystems, Playing, ScreenCamera};
use brs_engine::layout::{Layout, Look, Player};
use brs_engine::model::{ModelAsset, Parts, Skeleton};
use brs_engine::offscreen::{screenshot, Offscreen};
use brs_engine::{AssetStores, EnginePlugin};
use brs_formats::{cam, clim, container, efc, insa, insm, layout, ptmd, stcm};

/// Seconds to let the scene settle (asset upload, shader compilation) before a screenshot.
const SCREENSHOT_DELAY: f32 = 3.0;
/// Seconds after the screenshot request before the viewer may exit.
const EXIT_DELAY: f32 = 1.5;

const ORBIT_SPEED: f32 = 0.005;
const ZOOM_SPEED: f32 = 0.1;
const MAX_PITCH: f32 = 1.5;

/// Free-camera speeds on maps, as a fraction of the map's radius per second.
const MAP_WALK_SPEED: f32 = 0.15;
const MAP_RUN_SPEED: f32 = 0.6;

/// Opacity of collision overlays on maps.
const COLLISION_ALPHA: f32 = 0.55;

#[derive(Resource)]
struct Args {
    file: PathBuf,
    /// The body's animation first, then any layers.
    animations: Vec<PathBuf>,
    /// A camera move to look through instead of orbiting.
    camera: Option<PathBuf>,
    /// Freeze the animation at this many seconds instead of playing it.
    time: Option<f32>,
    /// Show only these of the model's parts; `None` shows them all.
    parts: Option<Vec<u16>>,
    screenshot: Option<PathBuf>,
    /// Collision files to overlay on a map.
    collision: Vec<PathBuf>,
    yaw: f32,
    pitch: f32,
    /// How many times as far off as usual the orbiting camera stands.
    distance: f32,
    /// Run without a window.
    background: bool,
    /// The sets of a layout to play, and the frame to stop them at.
    sets: Vec<usize>,
    frame: Option<f32>,
    /// A layout's textures, where they are not the files after it.
    textures: Vec<PathBuf>,
    /// The number of the effect of an effect file to play.
    effect: Option<u32>,
}

/// A layout and what plays its sets.
#[derive(Resource)]
struct LayoutSource {
    layout: Layout,
    players: Vec<Player>,
    playing: bool,
}

/// An effect file's bytes, until it is made ready, and then the file and which of
/// its effects is played.
#[derive(Resource)]
struct EffectSource {
    data: Vec<u8>,
    file: Option<(Arc<EffectFile>, u32)>,
}

/// How far off the camera stands from an effect, and how high it looks: effects
/// are made to be seen about a fighter, who is some 150 high.
const EFFECT_DISTANCE: f32 = 700.0;
const EFFECT_HEIGHT: f32 = 80.0;

/// The frames of a layout in a second.
const LAYOUT_FRAMES_A_SECOND: f32 = 60.0;

/// A parsed model, the file bytes its textures are decoded from, and clips to play.
#[derive(Resource)]
struct ModelSource {
    model: insm::Model,
    data: Vec<u8>,
    clips: Vec<Arc<Clip>>,
    camera: Option<Arc<CameraClip>>,
}

/// A parsed map, the file bytes its textures are decoded from, and collision overlays.
#[derive(Resource)]
struct MapSource {
    map: stcm::Map,
    data: Vec<u8>,
    collisions: Vec<clim::Collision>,
}

#[derive(Resource)]
struct Orbit {
    target: Vec3,
    /// Where `target` sits while the skeleton is in its rest pose.
    rest_target: Vec3,
    /// Average node position in the rest pose; the camera follows how far this moves.
    rest_centre: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let data = read_unwrapped(&args.file)?;
    let parsing = |path: &PathBuf| format!("parsing {}", path.display());

    let title = format!("brs-viewer - {}", args.file.file_name().unwrap_or_default().to_string_lossy());
    let window = Window { title, resolution: WindowResolution::new(1280, 720), ..default() };
    // A run that saves its picture and exits has nobody watching it.
    let offscreen = args.background || args.screenshot.is_some();
    let mut app = App::new();
    // The viewer has nothing to be heard.
    app.add_plugins(brs_engine::offscreen::plugins(window, offscreen, false))
    .add_plugins((FreeCameraPlugin, EnginePlugin::default()))
    .insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.15)))
    .add_systems(Update, (orbit_camera, staged_camera, capture, show_parts));

    if data.starts_with(efc::MAGIC) || data.starts_with(efc::MAGIC_OF_MANY) {
        app.add_plugins(brs_engine::canvas::plugin)
            .insert_resource(EffectSource { data, file: None })
            .add_systems(Startup, setup_effect)
            .add_systems(Update, play_effect.after(orbit_camera).before(EffectSystems::Step));
        app.insert_resource(args).run();
        return Ok(());
    }

    if data.starts_with(layout::OBJECTS_MAGIC) {
        let layout = read_layout(&args.file, &data, &args.textures, &mut app.world_mut().resource_mut::<Assets<Image>>())?;
        info!("layout: {} objects, {} sets, {} clips", layout.objects.objects.len(), layout.motions.sets.len(), layout.motions.clips.len());
        let sets = if args.sets.is_empty() { vec![0] } else { args.sets.clone() };
        let mut players: Vec<Player> = sets.iter().map(|&set| layout.player(set)).collect();
        if let Some(frame) = args.frame {
            players.iter_mut().for_each(|player| {
                player.advance(&layout, frame);
            });
        }
        app.add_plugins(brs_engine::canvas::plugin)
            .insert_resource(LayoutSource { layout, players, playing: args.frame.is_none() })
            .add_systems(Startup, setup_layout)
            .add_systems(Update, draw_layout);
    } else if data.starts_with(stcm::MAGIC) {
        let map = stcm::Map::parse(&data).with_context(|| parsing(&args.file))?;
        let collisions = args
            .collision
            .iter()
            .map(|path| clim::Collision::parse(&read_unwrapped(path)?).with_context(|| parsing(path)))
            .collect::<Result<Vec<_>>>()?;
        app.insert_resource(MapSource { map, data, collisions }).add_systems(Startup, setup_map);
    } else {
        let model = insm::Model::parse(&data).with_context(|| parsing(&args.file))?;
        let clips = args
            .animations
            .iter()
            .map(|path| Ok(Clip::new(insa::Animation::parse(&read_unwrapped(path)?).with_context(|| parsing(path))?)))
            .collect::<Result<Vec<_>>>()?;
        let camera = match &args.camera {
            Some(path) => {
                let camera = cam::Camera::parse(&read_unwrapped(path)?).with_context(|| parsing(path))?;
                Some(CameraClip::new(camera))
            }
            None => None,
        };
        app.insert_resource(ModelSource { model, data, clips, camera }).add_systems(Startup, setup_model);
    }
    app.insert_resource(args).run();
    Ok(())
}

/// Reads the layout whose objects are in `file`: its motions and groups are the two
/// files after it by number, and its textures those after them unless they are
/// named.
fn read_layout(file: &PathBuf, objects: &[u8], named: &[PathBuf], images: &mut Assets<Image>) -> Result<Layout> {
    let number: usize = file.file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse().ok()).context("a layout's file is named by its number")?;
    let sibling_name = |index: usize, extension: &str| file.with_file_name(format!("{index:03}.{extension}"));
    let sibling = |index: usize, extension: &str| read_unwrapped(&sibling_name(index, extension));
    let count = layout::Objects::parse(objects)?.textures;
    let files: Vec<PathBuf> = if named.is_empty() { (0..count).map(|index| sibling_name(number + 3 + index, "ptm")).collect() } else { named.to_vec() };
    let textures = files
        .iter()
        .map(|file| {
            let bytes = read_unwrapped(file)?;
            let header = ptmd::parse_header(&bytes).context("a layout's texture is not one")?;
            Ok(images.add(brs_engine::texture::image(&bytes, &header)))
        })
        .collect::<Result<Vec<Handle<Image>>>>()?;
    Layout::parse(objects, &sibling(number + 1, "xbn")?, &sibling(number + 2, "bin")?, textures)
}

/// A layout is drawn over nothing: a camera that only clears the screen.
fn setup_layout(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn draw_layout(mut source: ResMut<LayoutSource>, mut canvas: ResMut<Canvas>, time: Res<Time>) {
    let LayoutSource { layout, players, playing } = &mut *source;
    for player in players {
        if *playing {
            player.advance(layout, time.delta_secs() * LAYOUT_FRAMES_A_SECOND);
        }
        layout.draw(&mut canvas, 0, player, &Look::default());
    }
}

/// Reads a file, removing its compression wrapper if it comes straight from the disc.
fn read_unwrapped(path: &PathBuf) -> Result<Vec<u8>> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(container::unwrap(&data)?.unwrap_or(data))
}

fn parse_args() -> Result<Args> {
    let mut args = Args {
        file: PathBuf::new(),
        animations: Vec::new(),
        camera: None,
        time: None,
        parts: None,
        screenshot: None,
        collision: Vec::new(),
        yaw: 25.0,
        pitch: 10.0,
        distance: 1.0,
        background: false,
        sets: Vec::new(),
        frame: None,
        textures: Vec::new(),
        effect: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().with_context(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--anim" => args.animations.push(value()?.into()),
            "--camera" => args.camera = Some(value()?.into()),
            "--time" => args.time = Some(value()?.parse()?),
            "--parts" => {
                let list = value()?;
                let ids = list.split(',').filter(|id| *id != "none").map(str::parse).collect::<Result<_, _>>();
                args.parts = Some(ids.with_context(|| format!("--parts takes ids like 60,71 or none, not {list}"))?);
            }
            "--screenshot" => args.screenshot = Some(value()?.into()),
            "--collision" => args.collision.push(value()?.into()),
            "--yaw" => args.yaw = value()?.parse()?,
            "--pitch" => args.pitch = value()?.parse()?,
            "--distance" => args.distance = value()?.parse()?,
            "--background" => args.background = true,
            "--set" => {
                let list = value()?;
                args.sets = list.split(',').map(str::parse).collect::<Result<_, _>>().with_context(|| format!("--set takes numbers like 0,4, not {list}"))?;
            }
            "--frame" => args.frame = Some(value()?.parse()?),
            "--texture" => args.textures.push(value()?.into()),
            "--effect" => args.effect = Some(value()?.parse()?),
            _ if arg.starts_with("--") => bail!("unknown option {arg}"),
            _ => args.file = arg.into(),
        }
    }
    if args.file.as_os_str().is_empty() {
        bail!("usage: brs-viewer <file> [--anim <file>]... [--camera <file>] [--time <seconds>] [--parts <id,id..|none>] [--collision <file>]... [--screenshot <png>] [--yaw <degrees>] [--pitch <degrees>] [--distance <factor>] [--background]");
    }
    Ok(args)
}

/// Lists a newly spawned model's parts and hides the ones `--parts` leaves out.
fn show_parts(args: Res<Args>, models: Query<&Parts, Added<Parts>>, mut visibility: Query<&mut Visibility>) {
    for parts in &models {
        info!("parts: {:?}", parts.ids().collect::<Vec<_>>());
        if let Some(shown) = &args.parts {
            parts.show_only(shown, &mut visibility);
        }
    }
}

fn setup_model(mut commands: Commands, source: Res<ModelSource>, args: Res<Args>, mut stores: AssetStores) {
    let asset = ModelAsset::build(&source.model, &source.data, &mut stores);
    let root = asset.spawn(&mut commands, Transform::IDENTITY);

    if !source.clips.is_empty() {
        let mut animator = Animator::with_slots(source.clips.len());
        for (index, clip) in source.clips.iter().enumerate() {
            animator.set_slot(index, clip);
            let slot = animator.slot_mut(index);
            match args.time {
                Some(seconds) => slot.set_time(seconds * FRAMES_PER_SECOND),
                // Repeat the whole clip rather than its loop range, so all of it can be seen.
                None => slot.loop_between(0.0, clip.frames()),
            }
            info!(
                "animation {index}: {} frames ({:.2} s), loop range {:?}",
                clip.frames(),
                clip.seconds(),
                clip.loop_range()
            );
        }
        animator.paused = args.time.is_some();
        commands.entity(root).insert(animator);
    }

    let centre = (asset.min + asset.max) / 2.0;
    let radius = ((asset.max - asset.min).length() / 2.0).max(0.01);
    commands.insert_resource(Orbit {
        target: centre,
        rest_target: centre,
        rest_centre: asset.node_centre,
        distance: radius * 2.6 * args.distance,
        yaw: args.yaw.to_radians(),
        pitch: args.pitch.to_radians(),
    });
    commands.spawn((
        Camera3d::default(),
        brs_engine::texture::camera_of_values(),
        Projection::Perspective(PerspectiveProjection { near: radius * 0.01, far: radius * 100.0, ..default() }),
    ));
}

/// Makes an effect file ready and sets the camera to look at where its effects
/// play.
fn setup_effect(mut commands: Commands, mut source: ResMut<EffectSource>, args: Res<Args>, mut stores: AssetStores) {
    let file = match EffectFile::load(&source.data, &mut stores) {
        Ok(file) => file,
        Err(error) => {
            error!("the effect file could not be read: {error:#}");
            return;
        }
    };
    let numbers = file.numbers();
    info!("effects: {numbers:?}");
    let Some(number) = args.effect.or(numbers.first().copied()) else { return };
    source.file = Some((file, number));
    let target = Vec3::new(0.0, EFFECT_HEIGHT, 0.0);
    commands.insert_resource(Orbit { target, rest_target: target, rest_centre: Vec3::ZERO, distance: EFFECT_DISTANCE * args.distance, yaw: args.yaw.to_radians(), pitch: args.pitch.to_radians() });
    commands.spawn((
        Camera3d::default(),
        brs_engine::texture::camera_of_values(),
        Projection::Perspective(PerspectiveProjection { near: 1.0, far: 100_000.0, ..default() }),
    ));
}

/// Keeps the effects' camera as the viewer's is, and an effect playing: again
/// when it is over, or, with `--frame`, held so many frames in.
fn play_effect(
    mut commands: Commands,
    source: Res<EffectSource>,
    args: Res<Args>,
    camera: Query<&Transform, (With<Camera3d>, Without<ScreenCamera>)>,
    mut seen: ResMut<EffectCamera>,
    mut pool: ResMut<EffectPool>,
    playing: Query<(), With<Playing>>,
) {
    let Ok(camera) = camera.single() else { return };
    seen.0 = brs_engine::effect::Camera::of(camera.to_matrix());
    let Some((file, number)) = &source.file else { return };
    if !playing.is_empty() {
        return;
    }
    let Some(mut effect) = file.play(*number) else {
        error!("the file has no effect {number}");
        return;
    };
    if let Some(frames) = args.frame {
        effect.hold(frames.max(1.0) as u32, &mut pool.0, &seen.0);
        info!("effect {number} held {frames} frames in: {} particles, {} to draw", effect.effect.particles(), effect.effect.drawn().len());
    }
    commands.spawn(effect);
}

fn setup_map(mut commands: Commands, source: Res<MapSource>, args: Res<Args>, mut stores: AssetStores) {
    let spawned = brs_engine::map::spawn(&mut commands, &source.map, &source.data, Transform::IDENTITY, brs_engine::map::Piece::Scene, &mut 0, &mut stores);
    let (min, max) = (spawned.min, spawned.max);

    // Collision overlays: translucent, one hue per file, shaded by attribute.
    for (file, collision) in source.collisions.iter().enumerate() {
        let mut positions = Vec::with_capacity(collision.triangles.len() * 3);
        let mut colors = Vec::with_capacity(collision.triangles.len() * 3);
        for triangle in &collision.triangles {
            let hue = (file as f32 * 200.0 + triangle.attribute as f32 * 60.0) % 360.0;
            let color = Color::hsla(hue, 1.0, 0.5, COLLISION_ALPHA).to_srgba().to_f32_array();
            for corner in collision.corners(triangle) {
                positions.push(corner);
                colors.push(color);
            }
        }
        if positions.is_empty() {
            continue;
        }
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        commands.spawn((
            Mesh3d(stores.meshes.add(mesh)),
            MeshMaterial3d(stores.materials.add(StandardMaterial {
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                double_sided: true,
                ..default()
            })),
        ));
    }

    // Maps get a free-flying camera, starting outside the map and looking at its centre.
    let radius = ((max - min).length() / 2.0).max(0.01);
    let centre = (min + max) / 2.0;
    let direction = Quat::from_euler(EulerRot::YXZ, args.yaw.to_radians(), -args.pitch.to_radians(), 0.0);
    commands.spawn((
        Camera3d::default(),
        brs_engine::texture::camera_of_values(),
        Projection::Perspective(PerspectiveProjection { near: radius * 0.001, far: radius * 100.0, ..default() }),
        Transform::from_translation(centre + direction * Vec3::new(0.0, 0.0, radius * 1.8)).looking_at(centre, Vec3::Y),
        FreeCamera { walk_speed: radius * MAP_WALK_SPEED, run_speed: radius * MAP_RUN_SPEED, ..default() },
    ));
}

/// With `--camera`: looks through the camera move, at the frame the body's animation
/// has reached. The game plays such a move in the frame of the model it was made for,
/// which here stands at the origin.
fn staged_camera(
    source: Option<Res<ModelSource>>,
    args: Res<Args>,
    animators: Query<&Animator>,
    camera: Single<(&mut Transform, &mut Projection), (With<Camera3d>, Without<ScreenCamera>)>,
) {
    let Some(clip) = source.as_ref().and_then(|source| source.camera.as_ref()) else { return };
    let frame = match (args.time, animators.iter().next()) {
        (Some(seconds), _) => seconds * FRAMES_PER_SECOND,
        (None, Some(animator)) => animator.body().time(),
        (None, None) => 0.0,
    };
    let pose = clip.pose(frame);
    let (mut transform, mut projection) = camera.into_inner();
    *transform = pose.transform;
    if let Projection::Perspective(perspective) = &mut *projection {
        // The original works the angle out for the PSP's screen.
        perspective.fov = pose.vertical_fov(480.0 / 272.0);
    }
}

fn orbit_camera(
    orbit: Option<ResMut<Orbit>>,
    args: Res<Args>,
    skeletons: Query<&Skeleton>,
    nodes: Query<&GlobalTransform>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<ScreenCamera>)>,
) {
    // Maps use the free camera instead and have no orbit state; a camera move takes
    // the camera over altogether.
    let Some(mut orbit) = orbit else { return };
    if args.camera.is_some() {
        return;
    }

    // Animations can carry the model far from where it started; keep it in frame.
    let positions: Vec<Vec3> = skeletons
        .iter()
        .flat_map(|skeleton| nodes.iter_many(&skeleton.nodes).map(GlobalTransform::translation))
        .collect();
    if !positions.is_empty() {
        let centre = positions.iter().sum::<Vec3>() / positions.len() as f32;
        orbit.target = orbit.rest_target + (centre - orbit.rest_centre);
    }

    // A screenshot run must not be disturbed by whoever is using the mouse.
    if args.screenshot.is_none() {
        if buttons.pressed(MouseButton::Left) {
            orbit.yaw -= motion.delta.x * ORBIT_SPEED;
            orbit.pitch = (orbit.pitch + motion.delta.y * ORBIT_SPEED).clamp(-MAX_PITCH, MAX_PITCH);
        }
        orbit.distance *= (1.0 - scroll.delta.y * ZOOM_SPEED).clamp(0.5, 2.0);
    }

    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
    camera.translation = orbit.target + rotation * Vec3::new(0.0, 0.0, orbit.distance);
    camera.look_at(orbit.target, Vec3::Y);
}

/// In screenshot mode: saves one frame once the scene has settled, then exits.
fn capture(
    mut commands: Commands,
    args: Res<Args>,
    offscreen: Option<Res<Offscreen>>,
    time: Res<Time>,
    mut requested: Local<bool>,
    capturing: Query<(), With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &args.screenshot else { return };
    let elapsed = time.elapsed_secs();
    if !*requested && elapsed >= SCREENSHOT_DELAY {
        *requested = true;
        commands.spawn(screenshot(offscreen.as_deref())).observe(save_to_disk(path.clone()));
    } else if *requested && elapsed >= SCREENSHOT_DELAY + EXIT_DELAY && capturing.is_empty() {
        exit.write(AppExit::Success);
    }
}
