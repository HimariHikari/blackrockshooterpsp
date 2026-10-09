//! The ride on the bike as it is seen: the road's pieces, the models of what is
//! on it, and the camera. What is shown is what the ride has (`ride.rs`); nothing
//! here decides anything of it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use brs_engine::animation::{Animator, Clip, FRAMES_PER_SECOND};
use brs_engine::camera::CameraClip;
use brs_engine::canvas::{Canvas, Seen, HEIGHT, WIDTH};
use brs_engine::effect::Camera as EffectView;
use brs_engine::effects::{EffectCamera, EffectFile, EffectSystems, Playing};
use brs_engine::lit::Lit;
use brs_engine::map::{self, Built};
use brs_engine::model::{ModelAsset, Skeleton};
use brs_engine::step::SIXTIETHS_PER_STEP;
use brs_engine::{AssetStores, EngineSystems};
use brs_formats::{cam, insa, insm, stcm};

use super::display::Gauges;
use super::ride::{pressed, turn, Ride, Scenery, SoundAsked, FRAME, SOUND_SLOTS};
use super::script::EffectLook;
use super::road::light_at;
use super::rules::Asked;
use super::tables::{course_pack, Pack, Resource, View, ANIMATIONS, SYSTEM_PACK, VIEW_FAR, VIEW_ROAD};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::draw_window_on;
use crate::field::scene::Fade;
use crate::field::tables::FieldTables;
use crate::field::window::Window;
use crate::interface::layer;
use crate::pause::{self, Pause, PauseAssets, Paused};
use crate::player::{self, ShownParts};
use crate::progress::Progress;
use crate::records::Records;
use crate::shadow::{Of, Shadows, LIFT};
use crate::sound::{self, Board, Request};
use crate::text::{self, TextColours};
use crate::writing::Fonts;
use crate::Mode;

/// The course a ride is to be of.
#[derive(Resource, Clone)]
pub struct Course(pub String);

/// How the ride came out, for what the game goes back to when it is over;
/// there is none of it when the ride is all there is.
#[derive(Resource)]
pub struct Ridden(pub Option<super::ride::Outcome>);

/// The ride is one nobody plays (a run that passes battles over): it is ridden
/// by a rule of thumb, and she takes no harm.
#[derive(Resource)]
pub struct Unattended;

/// The ride that goes on.
#[derive(Resource)]
pub struct Riding {
    pub ride: Ride,
    /// The window of what the display says (`n28`), while it is up.
    window: Option<Window>,
    /// How long the talk that goes on has gone on, until the port plays it.
    talked: i32,
}

/// A model of the ride's and its animations, made ready.
struct Looked {
    model: ModelAsset,
    clips: [Option<Arc<Clip>>; ANIMATIONS],
}

/// What the ride is drawn from.
#[derive(Resource)]
struct RideAssets {
    models: Vec<Option<Looked>>,
    /// The moves of a model that is a camera's rig, by its animations.
    moves: Vec<[Option<Arc<CameraClip>>; ANIMATIONS]>,
    pieces: Vec<Option<Built>>,
}

/// Everything that is made for the ride, to be unmade with it.
#[derive(Component)]
struct OfTheRide;

/// One of a character's models as it is shown.
#[derive(Component)]
struct ShownModel {
    number: i32,
    slot: usize,
    resource: usize,
    begun: u32,
}

/// A thing of scenery as it is shown: which of the ride's.
#[derive(Component)]
struct ShownScenery(usize);

#[derive(Component)]
struct RideCamera;

/// The shadow of a character as it is shown: whose.
#[derive(Component)]
struct ShownShadow(i32);

/// One of a character's effects as it is shown: whose, in which slot, and
/// which of the slot's it is; and whether it has been put anywhere yet.
#[derive(Component)]
struct ShownEffect {
    number: i32,
    slot: usize,
    made: u32,
    placed: bool,
}

/// The ride's effect files, by their numbers in the system's table, each read
/// the first time it is wanted; one that cannot be read is kept as that.
#[derive(Resource, Default)]
struct RideEffects(HashMap<usize, Option<Arc<EffectFile>>>);

/// How long a talk is taken to last until the port plays it: four seconds.
const TALK: i32 = 120;
/// The kind of scenery that is the sky.
const SKY: i32 = 2;

/// The colour a ride's numbers name, as the scene's colours are given.
fn colour([red, green, blue]: [f32; 3]) -> Color {
    brs_engine::texture::values(Color::srgb(red, green, blue))
}

/// The light on a thing at a place by the road (the end of 0x142574): none but
/// the light that is all about, of the colour the place has (0x13E33C gives
/// the PSP's light that colour for all about and black for its own and its
/// shine).
fn light_of(ride: &Ride, place: Vec3) -> Lit {
    Lit { on: true, ambient: Vec3::from(light_at(&ride.road_tables, place)), diffuse: Vec3::ZERO, specular: Vec3::ZERO, towards: Vec3::Y, alpha: 1.0, reach: Vec2::ZERO }
}

/// Whether a thing of scenery is drawn (0x13C7B8 with 0x131128): the sky always;
/// another when it is as near along the road as it is big, or in front of the
/// eye and no farther along the road than its view sees.
fn seen(scenery: &Scenery, view: &View, eye: Vec3, forward: Vec3) -> bool {
    if scenery.kind == SKY {
        return true;
    }
    let along = (eye.z - scenery.place.z).abs();
    along < scenery.sight || (along < view.sight + scenery.sight && forward.dot(scenery.place - eye) >= 0.0)
}

/// The interface's frames to one of the game's (0x850).
const FRAMES: f32 = 2.0;

/// The bank of the mini-game's own sounds (0x149624: slot 6 of the bundle that
/// is member 1 of the system's pack, where a bundle of the field's has its
/// bank too), and the numbers its sounds are known to the mixer by: a slot's,
/// from this on.
pub const BANK: &str = "MINI_GAME/BMG_SYS.BIN/001.bin/06.bin/00.pphd";
const HANDLES: u32 = 0x2000_0000;
/// The stream a voice of the mini-game's is played on when it is free (0x149B80).
const VOICE_STREAM: usize = 1;

/// What the sound is asked for a sound the ride has asked for (0x149624,
/// 0x14979C, 0x149704, 0x149B80, 0x149880, 0x1498B0).
fn request(ride: &Ride, asked: SoundAsked, tunes: Option<&FieldTables>) -> Option<Request> {
    Some(match asked {
        SoundAsked::Play(slot, loudness) => {
            Request::Effect { handle: HANDLES + slot as u32, bank: BANK.to_owned(), sound: slot as u32, volume: loudness & 0xff, note: sound::NOTE }
        }
        SoundAsked::Stop(slot) => Request::StopEffect(HANDLES + slot as u32),
        SoundAsked::Common(number) => Request::Play { bank: sound::COMMON_BANK.to_owned(), sound: u32::try_from(number).ok()?, volume: sound::FULL_VOLUME },
        SoundAsked::Voice(voice) => {
            let resource = (*ride.tables.voices.get(usize::try_from(voice).ok()?)?)?;
            Request::SayAt(VOICE_STREAM, format!("{SYSTEM_PACK}/{:03}.at3", resource.member), sound::FULL_VOLUME)
        }
        SoundAsked::Music(tune) => Request::Music(sound::tune_file(tunes?.tune(tune)?)),
        SoundAsked::MusicOut(blocks) => Request::StopMusic(blocks),
    })
}

/// Begins the ride of the course asked for, and reads what it is drawn from.
fn begin(mut commands: Commands, course: Res<Course>, data: Res<GameData>, mut stores: AssetStores) {
    let ride = match Ride::new(&data, &course.0) {
        Ok(ride) => ride,
        Err(error) => {
            error!("the ride on the bike cannot begin: {error:#}");
            return;
        }
    };
    let pack = course_pack(&course.0);
    let of = |resource: Resource| -> Option<Vec<u8>> {
        let pack = match resource.pack {
            Pack::System => SYSTEM_PACK,
            Pack::Course => &pack,
            Pack::Common => return None,
        };
        data.member(pack, resource.member).ok().flatten()
    };
    let moves = ride
        .tables
        .models
        .iter()
        .map(|resource| {
            let rig = resource.model.and_then(of).filter(|bytes| cam::is_camera(bytes));
            resource.animations.map(|animation| {
                let (rig, bytes) = (rig.as_ref()?, animation.and_then(of)?);
                cam::Camera::from_parts(rig, &bytes).ok().map(CameraClip::new)
            })
        })
        .collect();
    let models = ride
        .tables
        .models
        .iter()
        .map(|resource| {
            let (model, bytes) = match resource.model? {
                // She herself, as the common game has her.
                Resource { pack: Pack::Common, .. } => data.model(player::model()).ok()?,
                resource => {
                    let bytes = of(resource)?;
                    (insm::Model::parse(&bytes).ok()?, bytes)
                }
            };
            let clips = resource.animations.map(|animation| animation.and_then(of).and_then(|bytes| insa::Animation::parse(&bytes).ok()).map(Clip::new));
            Some(Looked { model: ModelAsset::build(&model, &bytes, &mut stores), clips })
        })
        .collect();
    // The sky is drawn through a view of its own, with the eye in its middle
    // (0x132788): it is behind everything and hides nothing.
    let sky = usize::try_from(ride.road_tables.sky).ok();
    let mut turn = 0;
    let mut built = |index: usize, resource: &Option<Resource>, stores: &mut AssetStores| {
        let bytes = of((*resource)?)?;
        let piece = stcm::Map::parse(&bytes).ok()?;
        let kind = if sky == Some(index) { map::Piece::FarOff } else { map::Piece::Scene };
        Some(Built::build(&piece, &bytes, kind, &mut turn, stores))
    };
    let mut pieces: Vec<Option<Built>> = ride.tables.pieces.iter().map(|_| None).collect();
    let first = sky.filter(|sky| *sky < pieces.len());
    for index in first.into_iter().chain((0..pieces.len()).filter(|index| Some(*index) != first)) {
        pieces[index] = built(index, &ride.tables.pieces[index], &mut stores);
    }
    // The road's view is the one all that is on the road is seen through.
    let view = ride.road_tables.views[VIEW_ROAD];
    let far = ride.road_tables.views.iter().map(|view| view.far).fold(view.far, f32::max);
    commands.insert_resource(ClearColor(colour(ride.road_tables.backdrop)));
    match Gauges::load(&data, &mut stores.images) {
        Ok(gauges) => commands.insert_resource(gauges),
        Err(error) => error!("the ride's display cannot be read: {error:#}"),
    }
    commands.insert_resource(RideAssets { models, moves, pieces });
    commands.insert_resource(Riding { ride, window: None, talked: 0 });
    commands.spawn((
        OfTheRide,
        RideCamera,
        Camera3d::default(),
        brs_engine::texture::camera_of_values(),
        IsDefaultUiCamera,
        Projection::Perspective(PerspectiveProjection { fov: 0.8, near: view.near, far, ..default() }),
        DistanceFog { color: colour(view.fog), falloff: FogFalloff::Linear { start: view.fog_near, end: view.fog_far }, ..default() },
        Transform::from_xyz(0.0, 300.0, -800.0).looking_at(Vec3::new(0.0, 100.0, 0.0), Vec3::Y),
    ));
}

/// The pause (0x147BA8, and the task `_pause_task`, 0x14829C): START brings the
/// common game's up, of the ride's kind, while the ride is not ending; all of
/// the ride stands still under it, and given up from it the ride is over at
/// once (0x149FCC with 5).
#[allow(clippy::too_many_arguments)]
fn pause_ride(
    riding: Option<ResMut<Riding>>,
    pad: Res<Pad>,
    assets: Res<PauseAssets>,
    records: Res<Records>,
    progress: Option<Res<Progress>>,
    mut paused: ResMut<Paused>,
    mut board: ResMut<Board>,
    mut animators: Query<&mut Animator, With<ShownModel>>,
) {
    let Some(mut riding) = riding else { return };
    let mut hold = |held: bool| {
        for mut animator in &mut animators {
            animator.paused = held;
        }
    };
    if let Some(shown) = &mut paused.0 {
        let outcome = shown.step(&assets, &pad);
        board.ask_all(shown.sounds.drain(..));
        if let Some(outcome) = outcome {
            paused.0 = None;
            hold(false);
            board.ask(Request::LetGo);
            if outcome == pause::Outcome::GiveUp {
                riding.ride.give_up();
            }
        }
    } else if pad.pressed & button::START != 0 && riding.ride.outcome.is_none() {
        let listed = progress.map(|progress| pause::entries(&records, &progress)).unwrap_or_default();
        paused.0 = Some(Pause::new(&assets, pause::Kind::Ride, listed, false));
        hold(true);
    }
}

/// Whether the ride has its frames: not while the pause is up.
fn not_paused(paused: Res<Paused>) -> bool {
    !paused.up()
}

/// A frame of the ride (the task's frame, 0x147C4C).
#[allow(clippy::too_many_arguments)]
fn step(
    riding: Option<ResMut<Riding>>,
    gauges: Option<ResMut<Gauges>>,
    pad: Res<Pad>,
    ridden: Option<ResMut<Ridden>>,
    mut next: ResMut<NextState<Mode>>,
    mut board: ResMut<Board>,
    tunes: Option<Res<FieldTables>>,
    unattended: Option<Res<Unattended>>,
) {
    let Some(mut riding) = riding else { return };
    let riding = &mut *riding;
    if riding.ride.over() {
        // The task's end (its states 1 and 2): the screen goes black, and the
        // game goes back to what it came from.
        if let Some(mut ridden) = ridden.filter(|ridden| ridden.0.is_none()) {
            ridden.0 = riding.ride.outcome;
            next.set(Mode::Field);
        }
        return;
    }
    // Left and right are the pad's or the stick's.
    let asked = Asked {
        left: pad.held & button::LEFT != 0 || pad.stick.x < -0.5,
        right: pad.held & button::RIGHT != 0 || pad.stick.x > 0.5,
        special: pad.pressed & button::SQUARE != 0,
    };
    let buttons = [(button::LEFT, pressed::LEFT), (button::RIGHT, pressed::RIGHT), (button::TRIANGLE, pressed::TRIANGLE), (button::SQUARE, pressed::SQUARE), (button::CROSS, pressed::CROSS)];
    let pressed = buttons.iter().filter(|(button, _)| pad.pressed & button != 0).fold(0, |bits, (_, bit)| bits | bit);
    let asked = if unattended.is_some() {
        riding.ride.unharmed();
        riding.ride.rule_of_thumb()
    } else {
        asked
    };
    riding.ride.frame(asked, pressed);
    if let Some(mut gauges) = gauges {
        gauges.advance(&mut riding.ride.display, FRAMES);
    }
    // The talk is the field's to play (`field::talk_of_the_ride`); a ride that
    // is all there is has no field, and takes it as said after a while.
    if riding.ride.talk.going.is_some() && ridden.is_none() {
        riding.talked += 1;
        if riding.talked >= TALK {
            riding.talked = 0;
            riding.ride.talk_over();
        }
    }
    // What the display says (0x132E9C) is put up in the system's window, and
    // is said when that has been read (0x132ECC); a ride nobody plays takes it
    // as read.
    if let (Some(says), None) = (&riding.ride.display.says, &riding.window) {
        if unattended.is_some() {
            riding.ride.display.says = None;
        } else {
            riding.window = Some(Window::new(text::pages(says, &text::Plain), &[], 0, 0));
        }
    }
    if let Some(window) = &mut riding.window {
        let read = window.step(&pad).is_some();
        board.ask_all(window.sounds.drain(..).map(Request::common));
        if read {
            (riding.window, riding.ride.display.says) = (None, None);
        }
    }
    let asked = std::mem::take(&mut riding.ride.sounds);
    board.ask_all(asked.into_iter().filter_map(|asked| request(&riding.ride, asked, tunes.as_deref())));
    if riding.ride.over() {
        info!("the ride on the bike is over: {:?}", riding.ride.outcome);
        riding.ride.fade = Fade::DARK;
    }
}

/// Has the models of what is on the road be there, where each is.
#[allow(clippy::type_complexity)]
fn show_models(
    mut commands: Commands,
    riding: Option<Res<Riding>>,
    assets: Option<Res<RideAssets>>,
    mut shown: Query<(Entity, &mut ShownModel, &mut Transform, &mut Animator, &mut Lit, Option<&Skeleton>, Option<&mut ShownParts>)>,
) {
    let (Some(riding), Some(assets)) = (riding, assets) else { return };
    let ride = &riding.ride;
    let look = |number: i32, slot: usize| {
        let index = ride.characters.iter().position(|character| character.number == number)?;
        let model = &ride.riders[index].looks.models[slot];
        Some((&ride.characters[index], &ride.riders[index].looks, model, model.resource?))
    };
    for (entity, mut shown, mut transform, mut animator, mut lit, skeleton, parts) in &mut shown {
        let Some((character, looks, model, resource)) = look(shown.number, shown.slot).filter(|(.., resource)| *resource == shown.resource) else {
            commands.entity(entity).despawn();
            continue;
        };
        *transform = Transform::from_matrix(character.matrix()).with_scale(Vec3::splat(model.scale));
        let light = light_of(ride, transform.translation);
        if *lit != light {
            *lit = light;
        }
        let Some(clips) = assets.models.get(resource).and_then(Option::as_ref).map(|looked| &looked.clips) else { continue };
        if shown.begun != model.begun {
            shown.begun = model.begun;
            if let Some(clip) = usize::try_from(model.animation).ok().and_then(|animation| clips.get(animation)).and_then(Option::as_ref) {
                animator.set_blended(clip, looks.blend * FRAMES_PER_SECOND);
                if model.looped {
                    animator.body_mut().loop_clip();
                }
            }
        }
        let speed = model.speed * ride.time / FRAME;
        if animator.speed != speed {
            animator.speed = speed;
        }
        // The bones a script turns (`n24`, 0x1443A8): each in its own frame, on
        // top of its animation, about its X or its Z; the number is taken for
        // the angle as it is, which makes radians of it. The bike's wheels
        // are turned so, by how far she has come.
        if let Some(skeleton) = skeleton {
            for &(bone, way, angle) in &model.turned {
                let Some(index) = u8::try_from(bone).ok().and_then(|bone| skeleton.index_of(bone)) else { continue };
                animator.set_own_turn(index, if way == 0 { Quat::from_rotation_x(angle) } else { Quat::from_rotation_z(angle) });
            }
        }
        if let Some(mut parts) = parts {
            // A part the script gave nought is one that is shown.
            let wanted: Vec<u16> = model.parts.iter().filter(|(_, hidden)| *hidden == 0).map(|(part, _)| *part as u16).collect();
            if parts.0 != wanted {
                parts.0 = wanted;
            }
        }
    }
    for (index, character) in ride.characters.iter().enumerate() {
        for (slot, model) in ride.riders[index].looks.models.iter().enumerate() {
            let Some(resource) = model.resource else { continue };
            if shown.iter().any(|(_, shown, ..)| shown.number == character.number && shown.slot == slot && shown.resource == resource) {
                continue;
            }
            let Some(looked) = assets.models.get(resource).and_then(Option::as_ref) else { continue };
            let placed = Transform::from_matrix(character.matrix());
            let root = looked.model.spawn(&mut commands, placed);
            // Her animation is begun on the frame after she is made.
            let begun = model.begun.wrapping_sub(1);
            commands.entity(root).insert((
                OfTheRide,
                ShownModel { number: character.number, slot, resource, begun },
                Animator::new(),
                light_of(ride, placed.translation),
                ShownParts(Vec::new()),
            ));
        }
    }
}

/// Has the road's scenery be there, as far as each view sees it.
fn show_scenery(mut commands: Commands, riding: Option<Res<Riding>>, assets: Option<Res<RideAssets>>, shown: Query<(Entity, &ShownScenery)>) {
    let (Some(riding), Some(assets)) = (riding, assets) else { return };
    let ride = &riding.ride;
    let (eye, forward) = (ride.camera.eye, (ride.camera.mark - ride.camera.eye).normalize_or(Vec3::Z));
    let near = |index: usize| {
        let scenery = &ride.scenery[index];
        // A row's first pattern is the road's, its second what is far off.
        let view = &ride.road_tables.views[if scenery.kind == 0 { VIEW_ROAD } else { VIEW_FAR }];
        seen(scenery, view, eye, forward)
    };
    let mut there = vec![false; ride.scenery.len()];
    for (entity, shown) in &shown {
        if near(shown.0) {
            there[shown.0] = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (index, scenery) in ride.scenery.iter().enumerate().filter(|(index, _)| near(*index) && !there[*index]) {
        let root = commands.spawn((OfTheRide, ShownScenery(index), Transform::from_translation(scenery.place), Visibility::default())).id();
        for piece in &scenery.looks.pieces {
            let Some(built) = piece.resource.and_then(|resource| assets.pieces.get(resource)).and_then(Option::as_ref) else { continue };
            let turned = Transform::from_translation(piece.place).with_rotation(Quat::from_rotation_y(piece.heading.to_radians()));
            for entity in built.spawn(&mut commands, turned) {
                commands.entity(entity).insert(ChildOf(root));
            }
        }
    }
}

/// The camera, where the ride's has it (0x1375D8, 0); and where its own model's
/// motion has it, while it rides along on one (0x1365C8).
fn show_camera(
    riding: Option<ResMut<Riding>>,
    assets: Option<Res<RideAssets>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<RideCamera>>,
    mut sky: Query<(&ShownScenery, &mut Transform), Without<RideCamera>>,
) {
    let (Some(mut riding), Some(assets)) = (riding, assets) else { return };
    let ride = &mut riding.ride;
    let clip = ride.camera_motion.and_then(|motion| Some((motion, assets.moves.get(motion.model)?.get(usize::try_from(motion.animation).ok()?)?.as_ref()?)));
    if let Some((motion, clip)) = clip {
        let pose = clip.pose(motion.at * FRAMES_PER_SECOND);
        // The rig is where the one the camera goes with is, turned as it is.
        let turns = ride.character(ride.camera.follows).map_or(Vec3::ZERO, |followed| followed.turns);
        let turned = Quat::from_euler(EulerRot::ZYX, turns.z.to_radians(), turns.y.to_radians(), turns.x.to_radians());
        let rig = Transform::from_translation(ride.camera.anchor()).with_rotation(turned);
        let seen = rig * pose.transform;
        let lens = pose.vertical_fov(WIDTH / HEIGHT).to_degrees();
        ride.camera.ride_along(seen.translation, seen.translation + *seen.forward(), *seen.up(), lens);
    }
    let seen = &riding.ride.camera;
    if seen.eye.distance_squared(seen.mark) < 1e-6 || seen.lens() <= 0.0 {
        return;
    }
    for (mut transform, mut projection) in &mut camera {
        *transform = Transform::from_translation(seen.eye).looking_at(seen.mark, seen.up);
        if let Projection::Perspective(lens) = &mut *projection {
            let angle = seen.lens().to_radians();
            if lens.fov != angle {
                lens.fov = angle;
            }
        }
    }
    // The sky is seen from its own middle (the first of the three views has
    // the eye there).
    for (shown, mut transform) in &mut sky {
        if riding.ride.scenery[shown.0].kind == SKY {
            transform.translation = seen.eye;
        }
    }
}

/// The shadows of what is on the road (0x146B1C): under each character that
/// has a model and has not had its shadow left out (`n09` with 10), as wide and
/// as long as its script has it (11 and 12), turned as it is.
fn show_shadows(
    mut commands: Commands,
    riding: Option<Res<Riding>>,
    shadows: Option<Res<Shadows>>,
    mut shown: Query<(Entity, &ShownShadow, &mut Transform, &mut Visibility)>,
) {
    let (Some(riding), Some(shadows)) = (riding, shadows) else { return };
    let ride = &riding.ride;
    let placed = |number: i32| -> Option<Transform> {
        let index = ride.characters.iter().position(|character| character.number == number)?;
        let (character, looks) = (&ride.characters[index], &ride.riders[index].looks);
        let [wide, long, left_out] = looks.shadow;
        let drawn = left_out == 0 && looks.models.iter().any(|model| model.resource.is_some());
        drawn.then(|| Transform {
            translation: character.place + Vec3::Y * LIFT,
            rotation: Quat::from_rotation_y(character.turns.y.to_radians()),
            scale: Vec3::new(wide as f32, 1.0, long as f32),
        })
    };
    let mut there = HashSet::new();
    for (entity, of, mut transform, mut visibility) in &mut shown {
        if !ride.characters.iter().any(|character| character.number == of.0) {
            commands.entity(entity).despawn();
            continue;
        }
        there.insert(of.0);
        let wanted = match placed(of.0) {
            Some(placed) => {
                *transform = placed;
                Visibility::Inherited
            }
            None => Visibility::Hidden,
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    for character in ride.characters.iter().filter(|character| !there.contains(&character.number)) {
        if let Some(placed) = placed(character.number) {
            commands.spawn((OfTheRide, ShownShadow(character.number), shadows.bundle(Of::Ride, placed)));
        }
    }
}

/// The models of what is on the road, for where their bones are.
type Bones<'w, 's> = Query<'w, 's, (&'static ShownModel, Option<&'static Skeleton>, Option<&'static Animator>)>;

/// Where an effect of a character's is (0x13C9C0): its own place, turns and
/// size, from where the character is or from a bone of one of its models
/// (0x13C754).
fn effect_at(ride: &Ride, index: usize, look: &EffectLook, bones: &Bones) -> Mat4 {
    let character = &ride.characters[index];
    let own = Transform { translation: look.place, rotation: turn(look.turns), scale: Vec3::splat(look.scale) }.to_matrix();
    let slot = look.model.max(0) as usize;
    let bone = u8::try_from(look.bone).ok().and_then(|bone| {
        let (_, skeleton, animator) = bones.iter().find(|(shown, ..)| shown.number == character.number && shown.slot == slot)?;
        let (skeleton, animator) = (skeleton?, animator?);
        let scale = ride.riders[index].looks.models.get(slot).map_or(1.0, |model| model.scale);
        Some(character.matrix() * Mat4::from_scale(Vec3::splat(scale)) * animator.model_space(skeleton, skeleton.index_of(bone)?).to_matrix())
    });
    bone.unwrap_or_else(|| character.matrix()) * own
}

/// The effects of what is on the road (0x13C9C0, 0x13C7B8): one is made when
/// it is put in its slot, is where its character or a bone of it is (once, or
/// each frame if it is to follow), has the time the game's frame takes, and is
/// taken out of its slot when it has played out.
#[allow(clippy::too_many_arguments)]
fn show_effects(
    mut commands: Commands,
    riding: Option<ResMut<Riding>>,
    course: Res<Course>,
    data: Res<GameData>,
    mut stores: AssetStores,
    mut files: ResMut<RideEffects>,
    mut seen: ResMut<EffectCamera>,
    camera: Query<&Transform, With<RideCamera>>,
    bones: Bones,
    mut shown: Query<(Entity, &mut ShownEffect, &mut Playing)>,
) {
    let Some(mut riding) = riding else { return };
    let ride = &mut riding.ride;
    if let Ok(camera) = camera.single() {
        seen.0 = EffectView::of(camera.to_matrix());
    }
    let pace = ride.time / FRAME;
    let look_of = |ride: &Ride, made: &ShownEffect| {
        let index = ride.characters.iter().position(|character| character.number == made.number)?;
        let look = ride.riders[index].looks.effects[made.slot].filter(|look| look.made == made.made)?;
        Some((index, look))
    };
    let (mut there, mut over) = (HashSet::new(), Vec::new());
    for (entity, mut made, mut playing) in &mut shown {
        let Some((index, look)) = look_of(ride, &made) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !playing.playing() {
            over.push((made.number, made.slot, made.made));
            commands.entity(entity).despawn();
            continue;
        }
        there.insert((made.number, made.slot));
        playing.unseen = look.flag;
        if look.held {
            continue;
        }
        if look.follows || !made.placed {
            playing.place(effect_at(ride, index, &look, &bones));
            made.placed = true;
        }
        playing.give(SIXTIETHS_PER_STEP * if look.own_time { 1.0 } else { pace });
    }
    for (number, slot, made) in over {
        ride.effect_over(number, slot, made);
    }
    let pack = course_pack(&course.0);
    for (index, character) in ride.characters.iter().enumerate() {
        for (slot, look) in ride.riders[index].looks.effects.iter().enumerate() {
            let Some(look) = look.filter(|_| !there.contains(&(character.number, slot))) else { continue };
            let file = files.0.entry(look.resource).or_insert_with(|| {
                let resource = ride.tables.effects.get(look.resource).copied().flatten()?;
                let pack = match resource.pack {
                    Pack::System => SYSTEM_PACK,
                    Pack::Course => &pack,
                    Pack::Common => return None,
                };
                let bytes = data.member(pack, resource.member).ok().flatten()?;
                EffectFile::load(&bytes, &mut stores).inspect_err(|error| warn!("effect {} of the ride: {error:#}", look.resource)).ok()
            });
            let Some(mut playing) = file.as_ref().and_then(|file| file.play(*file.numbers().first()?)) else { continue };
            playing.place(effect_at(ride, index, &look, &bones));
            (playing.stays, playing.unseen) = (true, look.flag);
            commands.spawn((OfTheRide, ShownEffect { number: character.number, slot, made: look.made, placed: true }, playing));
        }
    }
}

/// Draws the display over the ride (0x132E04): the gauges, then its own fade.
#[allow(clippy::too_many_arguments)]
fn show_display(
    riding: Option<Res<Riding>>,
    gauges: Option<Res<Gauges>>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    seen: Res<Seen>,
    windows: Res<PauseAssets>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    let (Some(riding), Some(gauges)) = (riding, gauges) else { return };
    let display = &riding.ride.display;
    gauges.draw(&mut canvas, display, &fonts, &colours);
    if display.dark() != 0 {
        canvas.fill(layer::DISPLAY, seen.0, [0, 0, 0, display.dark()]);
    }
    // What it says, in the window (0x6C0F8), over its own fade.
    if let Some(window) = &riding.window {
        draw_window_on(&mut canvas, layer::WINDOW, windows.window(), &fonts, &colours, window, steps.overstep_fraction());
    }
    // The screen's fade, which the scripts ask for (`n0e`).
    let fade = &riding.ride.fade;
    let alpha = (fade.level.clamp(0.0, 1.0) * 255.0) as u8;
    if alpha != 0 {
        let shade = if fade.white { 255 } else { 0 };
        canvas.fill(layer::FADE, seen.0, [shade, shade, shade, alpha]);
    }
}

/// Unmakes everything of the ride, and has its sounds stop (0x1384FC).
fn end(mut commands: Commands, made: Query<Entity, With<OfTheRide>>, mut board: ResMut<Board>) {
    for entity in &made {
        commands.entity(entity).despawn();
    }
    board.ask_all((0..SOUND_SLOTS as u32).map(|slot| Request::StopEffect(HANDLES + slot)));
    board.ask(Request::DropBank(BANK.to_owned()));
    commands.remove_resource::<Riding>();
    commands.remove_resource::<RideAssets>();
    commands.insert_resource(RideEffects::default());
    commands.remove_resource::<Gauges>();
}

pub fn plugin(app: &mut App) {
    app.init_resource::<RideEffects>()
        .add_systems(FixedUpdate, pause_ride.before(step).run_if(in_state(Mode::Ride)))
        .add_systems(FixedUpdate, show_effects.after(step).before(EffectSystems::Step).run_if(in_state(Mode::Ride).and_then(not_paused)))
        .add_systems(OnEnter(Mode::Ride), begin)
        .add_systems(OnExit(Mode::Ride), end)
        .add_systems(FixedUpdate, step.before(EngineSystems::Animate).run_if(in_state(Mode::Ride).and_then(not_paused)))
        .add_systems(Update, (show_models, show_shadows, show_scenery, show_camera, show_display).chain().run_if(in_state(Mode::Ride)));
}
