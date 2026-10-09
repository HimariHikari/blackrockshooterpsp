//! The scene of the screen between missions: what is drawn for the one group
//! of its layout that is not drawn from the layout (0x627B4). Her model
//! (0x66098 to 0x66C88), in the suit and with the weapon she has on, stands
//! in it and is seen by a camera of the screen's own (0x643AC to 0x647E8); each
//! is in one of two animations of the screen's archive, as its script has them
//! played (`interface.rs` keeps how far each is). See
//! `docs/STAGE_SELECT_NOTES.md`.
//!
//! The camera draws to the scenes' picture, as the menus' does: the field's
//! cameras are put out for as long as it does, and the picture is put among
//! what the screen draws where the group has its place.

use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::canvas::{Canvas, Seen, HEIGHT, WIDTH};
use brs_engine::model::ModelAsset;
use brs_engine::AssetStores;

use super::{Hub, HubAssets, FRAMES_A_STEP};
use crate::battle::actor::WEAPON_NODE;
use crate::battle::tables::PlayerStats;
use crate::data::GameData;
use crate::field::{FieldSource, FieldState};
use crate::menu::Menu;
use crate::player::{self, part, ShownParts};
use crate::text::TextColours;
use crate::writing::Fonts;

/// What of her model's switchable parts is shown: the screen hides the cannon
/// and the blade that are of the model itself and two of the shapes of her
/// right hand (0x66B7C), and leaves the rest as the model has them.
const HER_PARTS: [u16; 3] = [part::RIGHT_HAND_CANNON, part::LEFT_HAND_BLADE, part::LEFT_HAND_EMPTY];
/// Where the port has her stand for it: nowhere near anything of a map, nor
/// where the menus have theirs.
const PLACE: Vec3 = Vec3::new(0.0, 200_000.0, 0.0);
/// How near and how far the scene's camera sees (0x643DC).
const NEAR: f32 = 10.0;
const FAR: f32 = 500.0;
/// The scene's camera draws after the field's and before the canvas's.
const CAMERA_ORDER: isize = 5;

/// Her model in the scene: what it was made with on, by the items.
#[derive(Component)]
struct Her {
    wears: (i32, i32),
}

#[derive(Component)]
struct SceneCamera;

/// Whether the scene's camera is drawing this frame.
#[derive(Resource, Default)]
struct Shown(bool);

/// Has her model stand in the scene for as long as the screen has one, in the
/// suit and with the weapon she has on (0x66A0C), moved by the animation the
/// script has her in; and the scene's camera see her, where its own animation
/// has it (0x6446C).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show(
    mut commands: Commands,
    state: Res<FieldState>,
    menu: Res<Menu>,
    assets: Res<HubAssets>,
    (data, source, stats): (Res<GameData>, Res<FieldSource>, Res<PlayerStats>),
    steps: Res<Time<Fixed>>,
    mut stores: AssetStores,
    mut shown: ResMut<Shown>,
    made: Query<Entity, Or<(With<Her>, With<SceneCamera>)>>,
    mut her: Query<(&Her, &mut Animator), Without<SceneCamera>>,
    mut camera: Query<(&mut Camera, &mut Transform, &mut Projection), With<SceneCamera>>,
    mut others: Query<(Entity, &mut Camera), (With<Camera3d>, Without<SceneCamera>)>,
    mut put_out: Local<Vec<Entity>>,
) {
    // The MENU, once it is made, has the screen to itself.
    let scene = state.hub().filter(|_| !menu.made()).and_then(Hub::scene);
    let wears = (stats.suit, stats.weapon);
    // With something else on than she was made with, she is made anew.
    let stale = her.iter().any(|(her, _)| her.wears != wears);
    let drawn = scene.is_some_and(|scene| scene.drawn) && !made.is_empty() && !stale;
    // The scenes' other cameras are put out while this one draws.
    if drawn && put_out.is_empty() {
        for (entity, mut other) in &mut others {
            if other.is_active {
                other.is_active = false;
                put_out.push(entity);
            }
        }
    } else if !drawn {
        for entity in put_out.drain(..) {
            if let Ok((_, mut other)) = others.get_mut(entity) {
                other.is_active = true;
            }
        }
    }
    if shown.0 != drawn {
        shown.0 = drawn;
    }
    let Some(scene) = scene.filter(|_| !stale) else {
        for entity in &made {
            commands.entity(entity).despawn();
        }
        return;
    };
    if made.is_empty() {
        // The suit is a character of the field (0x14BE78), whose bundle has her
        // model in it.
        let read = |file: String| data.model(&file).inspect_err(|error| warn!("{file} is not shown between missions: {error:#}")).ok();
        let suit = source.character_bundle(player::suit_character(wears.0)).map(|bundle| player::model_of(&bundle));
        let Some((model, bytes)) = suit.and_then(read).or_else(|| read(player::model())) else { return };
        let built = ModelAsset::build(&model, &bytes, &mut stores);
        let (root, [hand]) = built.spawn_with_nodes(&mut commands, Transform::from_translation(PLACE), [WEAPON_NODE]);
        commands.entity(root).insert((Her { wears }, Animator::new(), ShownParts(HER_PARTS.to_vec())));
        // Her weapon is a model of its own, on the node it hangs on (0x66288).
        let weapon = player::weapon_model(wears.1).and_then(read).or_else(|| read(crate::battle::WEAPON.to_owned()));
        if let (Some(hand), Some((model, bytes))) = (hand, weapon) {
            let cannon = ModelAsset::build(&model, &bytes, &mut stores).spawn(&mut commands, Transform::IDENTITY);
            commands.entity(cannon).insert(ChildOf(hand));
        }
        commands.spawn((
            SceneCamera,
            Camera3d::default(),
            Camera { order: CAMERA_ORDER, clear_color: ClearColorConfig::Custom(Color::NONE), is_active: false, ..default() },
            brs_engine::texture::camera_of_values(),
            Projection::Perspective(PerspectiveProjection { near: NEAR, far: FAR, ..default() }),
            Transform::from_translation(PLACE),
        ));
        return;
    }
    // Between two frames of the game's logic both are carried on by the part
    // of a frame that has passed.
    let ahead = steps.overstep_fraction() * FRAMES_A_STEP as f32;
    for (_, mut animator) in &mut her {
        let clip = assets.interface.her_clip(scene.late, scene.her.which);
        if !animator.body().is(clip) {
            animator.set(clip);
        }
        // How far it is is the screen's to say.
        if !animator.paused {
            animator.paused = true;
        }
        animator.body_mut().set_time(scene.her.at(ahead, clip.frames(), clip.loop_range()));
    }
    for (mut camera, mut transform, mut projection) in &mut camera {
        let clip = assets.interface.camera_clip(scene.late, scene.camera.which);
        let pose = clip.pose(scene.camera.at(ahead, clip.frames(), (0.0, clip.frames())));
        *transform = Transform { translation: PLACE + pose.transform.translation, ..pose.transform };
        let angle = pose.vertical_fov(WIDTH / HEIGHT);
        if let Projection::Perspective(lens) = &mut *projection {
            if lens.fov != angle {
                lens.fov = angle;
            }
        }
        if camera.is_active != drawn {
            camera.is_active = drawn;
        }
    }
}

/// Draws the screen between missions while it is up, unless the MENU it made
/// has the screen.
#[allow(clippy::too_many_arguments)]
fn draw(
    state: Res<FieldState>,
    menu: Res<Menu>,
    assets: Res<HubAssets>,
    (fonts, colours): (Res<Fonts>, Res<TextColours>),
    seen: Res<Seen>,
    shown: Res<Shown>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    if let Some(hub) = state.hub().filter(|_| !menu.made()) {
        hub.draw(&mut canvas, &assets, &fonts, &colours, &seen, shown.0, steps.overstep_fraction());
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shown>().add_systems(Update, (show, draw).chain().run_if(in_state(crate::Mode::Field)));
}
