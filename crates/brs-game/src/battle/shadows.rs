//! The round shadows of a battle.
//!
//! The scenes' engine can have a node of a model throw a round shadow on the
//! ground (0xACF64 gives a node one, of a radius and a strength; 0xAC878 draws
//! a node's after what the node itself draws): a fan of eight pieces about the
//! place on the ground, where up is nought, that is under the node, dark in
//! the middle ((30, 0, 30), all there) and nothing at its edge (the ten corners
//! at 0x16A4E8), as large as the radius whatever the size of the model, its
//! alpha by the strength.
//!
//! A battle gives them to her as it makes her (0x999E8): one of 75 under her
//! model's first node and one of 20 under each of nodes 20 and 24, her feet,
//! all of strength 96 of 255. An enemy's script gives its own (engine function
//! 51, 0xBDA28: the slot of a model, a node of it, the radius and the strength
//! of 255): every one of the 88 does as it loads its model, 104 calls between
//! them, from 50 across under the two smallest bosses to 480 under the largest
//! thing there is, at strengths from 100 to 240.
//!
//! A node's shadow is drawn while the node is to be seen (its flag 0x40); the
//! port has it by whether the model is.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::model::Skeleton;

use super::Battle;

/// The nodes of hers that throw one, and how large each is.
const HERS: [(u8, f32); 3] = [(0, 75.0), (20, 20.0), (24, 20.0)];
/// How strong hers are.
const HER_STRENGTH: f32 = 96.0 / 255.0;
/// The colour of the fan, of 255.
const COLOUR: [f32; 3] = [30.0 / 255.0, 0.0, 30.0 / 255.0];
/// How far over the ground the port has it, so that the two are told apart.
const LIFT: f32 = 1.0;

/// A shadow: the model it is of, the node of that it is under, how large it
/// is and how strong.
#[derive(Component)]
pub struct ShadowOf {
    model: Entity,
    node: u8,
    radius: f32,
    strength: f32,
}

/// The fan: its middle, then round its edge.
fn fan() -> Mesh {
    let mut places = vec![[0.0, 0.0, 0.0]];
    let mut colours = vec![[COLOUR[0], COLOUR[1], COLOUR[2], 1.0]];
    for corner in 0..8 {
        let (sine, cosine) = (corner as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
        // The corners between the four straight ones are at 0.7 each way.
        let out = if corner % 2 == 0 { 1.0 } else { 0.7 * std::f32::consts::SQRT_2 };
        places.push([-cosine * out, 0.0, sine * out]);
        colours.push([COLOUR[0], COLOUR[1], COLOUR[2], 0.0]);
    }
    let pieces: Vec<u32> = (0..8u32).flat_map(|piece| [0, 1 + piece, 1 + (piece + 1) % 8]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, places.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; places.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; places.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_indices(Indices::U32(pieces))
}

/// Has a node of `model` throw a shadow, and returns what was made for it.
pub fn give(commands: &mut Commands, model: Entity, node: u8, radius: f32, strength: f32) -> Entity {
    commands.spawn((Battle, ShadowOf { model, node, radius, strength: strength.clamp(0.0, 1.0) }, Transform::default(), Visibility::Hidden)).id()
}

/// Gives her her shadows, as the battle is set up.
pub fn give_hers(commands: &mut Commands, her: Entity) {
    for (node, radius) in HERS {
        give(commands, her, node, radius, HER_STRENGTH);
    }
}

/// Every shadow that has been given gets the fan, and a material as strong as
/// it is.
pub fn make(
    mut commands: Commands,
    fresh: Query<(Entity, &ShadowOf), Added<ShadowOf>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut the_fan: Local<Option<Handle<Mesh>>>,
) {
    for (entity, shadow) in &fresh {
        let mesh = the_fan.get_or_insert_with(|| meshes.add(fan())).clone();
        let material = materials.add(StandardMaterial {
            base_color: Color::linear_rgba(1.0, 1.0, 1.0, shadow.strength),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.entity(entity).insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

/// Has each be on the ground under its node, and go when its model has.
pub fn follow(
    mut commands: Commands,
    models: Query<(&Transform, &Visibility, &Skeleton, &Animator), Without<ShadowOf>>,
    mut shadows: Query<(Entity, &ShadowOf, &mut Transform, &mut Visibility)>,
) {
    for (entity, of, mut transform, mut visibility) in &mut shadows {
        let Ok((at, seen, skeleton, animator)) = models.get(of.model) else {
            commands.entity(entity).despawn();
            continue;
        };
        let under = skeleton.index_of(of.node).map(|index| at.to_matrix() * animator.model_space(skeleton, index).to_matrix());
        let wanted = match under {
            Some(under) if *seen != Visibility::Hidden => {
                let place = under.w_axis.truncate();
                *transform = Transform { translation: Vec3::new(place.x, LIFT, place.z), rotation: Quat::IDENTITY, scale: Vec3::new(of.radius, 1.0, of.radius) };
                Visibility::Inherited
            }
            _ => Visibility::Hidden,
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}
