//! The shadows under the field's characters (0x10791C).
//!
//! A character that is drawn has its shadow drawn first (0x1067F8), while its
//! flag 0x16 is set, which whatever makes a character sets (0xE1954, 0xD6AB8,
//! and 0x1116B0 for her), and its flag 0x61 is not, which a script sets with
//! setting 77 for what is to have none. The shadow is the round one
//! of [`crate::shadow`], as large as the character's figure 0x2B has it (its
//! + 0x174, which is its script's `shadow_radius`, 0x106AF0: sixty unless the
//! script says, so 120 across), as strong as its + 0x1F0, which every frame of
//! the character makes one (0xF5F68).
//!
//! It does not lie where the character is but on the ground under its model:
//! each frame of a character's model (0xFD3CC) looks for the ground along the
//! character's upright, from 300 over the model's first node (0x107254,
//! 0xFDDA4: where its animation has it; the character's own place for one that
//! is no plain model) to 200 under, and keeps where it finds any (its + 0x1E0,
//! with its flag 0x17). So the shadow goes with what a scene has its people do
//! on the spot, and the shadow of one that leaps or
//! hovers stays on the ground, and where there is none within reach it stays
//! where ground was last found. It is turned from lying flat to the lie of the
//! ground the character last stood on (its + 0x80, 0x106CB8), and lies 6 over
//! the point.
//!
//! One whose frame of the field is turned (the sixth stage's floors) has it
//! where it stands, lying as its frame does: its ground is not looked for.
//!
//! A script can have it put some way off that place (settings 80 to 82: the
//! character's + 0x440 up, + 0x444 along x and + 0x448 along z, added as the
//! point is made, 0x107B54 and 0x107B74).
//!
//! It is drawn through the scene's lens as its character is (0x106994), and the
//! port's one lens sees nearer and further than that, so a shadow whose middle
//! is not between the lens's two figures before the camera is left out, whole.

use std::collections::BTreeSet;

use bevy::prelude::*;
use brs_engine::collision::CollisionWorld;

use super::assets::CharacterAssets;
use super::camera::figure;
use super::scene::flag::NO_SHADOW;
use super::{Actor, FieldRoot, FieldState, FollowCamera, RootNode};
use crate::shadow::{Of, Shadows, LIFT};

/// How large a shadow is until a script says: the field's figure 4.
const SIZE: f32 = 60.0;
/// How far over a character and how far under it its ground is looked for
/// (0xFD4B8, 0xFD4DC).
const GROUND_ABOVE: f32 = 300.0;
const GROUND_BELOW: f32 = 200.0;

/// On the shadow of a character: the character's number, and where ground was
/// last found under it with which way is up there.
#[derive(Component)]
pub struct ShadowOf {
    id: i32,
    kept: Option<(Vec3, Vec3)>,
}

/// Where a shadow lies and how: on the ground under `at` if there is any within
/// reach, which is then kept; else where ground was last found; else, with none
/// found yet, where the character is.
fn lie(world: &CollisionWorld, at: Vec3, kept: &mut Option<(Vec3, Vec3)>) -> (Vec3, Quat) {
    if let Some(ground) = world.ground(at, GROUND_ABOVE, GROUND_BELOW) {
        *kept = Some((Vec3::new(at.x, ground.height, at.z), ground.normal));
    }
    let (point, up) = kept.unwrap_or((at, Vec3::Y));
    (point + Vec3::Y * LIFT, Quat::from_rotation_arc(Vec3::Y, up.normalize_or(Vec3::Y)))
}

/// Whether a point is before the camera where a lens sees that sees from so
/// near to so far (nought is no end).
fn seen(camera: &Transform, point: Vec3, near: f32, far: f32) -> bool {
    let before = (point - camera.translation).dot(*camera.forward());
    before >= near && (far <= 0.0 || before <= far)
}

/// Has every character that is drawn have its shadow under it.
#[allow(clippy::type_complexity)]
pub fn show(
    mut commands: Commands,
    shadows: Option<Res<Shadows>>,
    state: Res<FieldState>,
    (world, assets): (Res<CollisionWorld>, Res<CharacterAssets>),
    root: Single<Entity, With<FieldRoot>>,
    camera: Single<&Transform, (With<FollowCamera>, Without<ShadowOf>)>,
    actors: Query<(&Actor, &Transform, Option<&RootNode>), Without<ShadowOf>>,
    nodes: Query<&GlobalTransform>,
    mut shown: Query<(Entity, &mut ShadowOf, &mut Transform, &mut Visibility), (Without<Actor>, Without<FollowCamera>)>,
) {
    let Some(shadows) = shadows else { return };
    let scene = &state.scene;
    let (near, far) = (scene.camera.figure(figure::NEAR), scene.camera.figure(figure::FAR));
    // Where each character's shadow is, if it has one now.
    let placed = |id: i32, at: &Transform, under: Option<Vec3>, kept: &mut Option<(Vec3, Vec3)>| -> Option<Transform> {
        let character = scene.characters.get(&id)?;
        // One that has a model here is one that is ready to be drawn.
        if !character.drawn() || character.flags.has(NO_SHADOW) {
            return None;
        }
        let size = assets.get(character.chr).map_or(SIZE, |asset| asset.shadow_radius);
        let (translation, rotation) = if character.lay == Quat::IDENTITY {
            lie(&world, under.unwrap_or(at.translation), kept)
        } else {
            (at.translation + character.lay * Vec3::new(0.0, LIFT, 0.0), character.lay)
        };
        // And as far from there as a script has it put (0x107B54).
        let translation = translation + character.shadow_offset;
        seen(&camera, translation, near, far).then_some(Transform { translation, rotation, scale: Vec3::splat(size) })
    };
    let mut there = BTreeSet::new();
    for (entity, mut of, mut transform, mut visibility) in &mut shown {
        let Some((_, at, root)) = actors.iter().find(|(actor, ..)| actor.id == of.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        there.insert(of.id);
        let id = of.id;
        // Where the model's first node is, as it was last drawn.
        let under = root.and_then(|root| root.0).and_then(|node| nodes.get(node).ok()).map(GlobalTransform::translation);
        let wanted = match placed(id, at, under, &mut of.kept) {
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
    for (actor, at, _) in &actors {
        if there.contains(&actor.id) {
            continue;
        }
        // A model just made has not been drawn yet: its place is gone by.
        let mut kept = None;
        let Some(placed) = placed(actor.id, at, None, &mut kept) else { continue };
        commands.spawn((ShadowOf { id: actor.id, kept }, shadows.bundle(Of::Field, placed), ChildOf(*root)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brs_engine::collision::Layer;
    use brs_formats::clim::{Collision, Triangle};

    #[test]
    fn a_shadow_lies_on_the_ground_under_its_character_and_stays_where_ground_was() {
        // A floor that slopes up along x from 0 to 1000, a rise of 1 in 10 from 10.
        let collision = Collision {
            vertices: vec![[0.0, 10.0, -500.0], [0.0, 10.0, 500.0], [1000.0, 110.0, -500.0], [1000.0, 110.0, 500.0]],
            triangles: vec![Triangle { indices: [0, 1, 2], attribute: 0 }, Triangle { indices: [2, 1, 3], attribute: 0 }],
            attributes: Vec::new(),
        };
        let mut world = CollisionWorld::default();
        world.add(&collision, Layer::World);
        let mut kept = None;
        // Standing on it: under her feet, six over the ground, tilted as it is.
        let (at, turn) = lie(&world, Vec3::new(500.0, 60.0, 0.0), &mut kept);
        assert!(at.abs_diff_eq(Vec3::new(500.0, 66.0, 0.0), 1e-3), "{at}");
        let up = turn * Vec3::Y;
        assert!(up.x < -0.09 && up.y > 0.99, "{up}");
        // In the air over it, within reach: still on the ground.
        let (at, _) = lie(&world, Vec3::new(600.0, 250.0, 0.0), &mut kept);
        assert!(at.abs_diff_eq(Vec3::new(600.0, 76.0, 0.0), 1e-3), "{at}");
        // Higher than its reach, or off the ground's edge: where ground was last.
        let (at, _) = lie(&world, Vec3::new(700.0, 600.0, 0.0), &mut kept);
        assert!(at.abs_diff_eq(Vec3::new(600.0, 76.0, 0.0), 1e-3), "{at}");
        let (at, _) = lie(&world, Vec3::new(2000.0, 0.0, 0.0), &mut kept);
        assert!(at.abs_diff_eq(Vec3::new(600.0, 76.0, 0.0), 1e-3), "{at}");
        // With no ground ever found it is where the character is, lying flat.
        let (at, turn) = lie(&world, Vec3::new(2000.0, 0.0, 0.0), &mut None);
        assert_eq!((at, turn), (Vec3::new(2000.0, LIFT, 0.0), Quat::IDENTITY));
    }

    #[test]
    fn a_shadow_is_seen_only_where_the_scenes_lens_sees() {
        // A camera at 100 up that looks along x.
        let camera = Transform::from_xyz(0.0, 100.0, 0.0).looking_at(Vec3::new(1000.0, 100.0, 0.0), Vec3::Y);
        let at = |x: f32| Vec3::new(x, 6.0, 40.0);
        assert!(seen(&camera, at(500.0), 50.0, 20_000.0));
        // Nearer than the lens sees from, which a scene has be as much as 300.
        assert!(!seen(&camera, at(40.0), 50.0, 20_000.0));
        assert!(!seen(&camera, at(250.0), 300.0, 20_000.0));
        // Behind the camera, and beyond the far figure, which the woods have at 10000.
        assert!(!seen(&camera, at(-500.0), 50.0, 20_000.0));
        assert!(seen(&camera, at(15_000.0), 50.0, 20_000.0) && !seen(&camera, at(15_000.0), 50.0, 10_000.0));
        // With no far figure there is no end.
        assert!(seen(&camera, at(150_000.0), 50.0, 0.0));
    }
}
