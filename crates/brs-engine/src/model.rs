//! Models: built once into meshes and materials, then spawned as often as needed.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use brs_formats::insm;

use crate::lit::Look;
use crate::texture;
use crate::AssetStores;

/// Bevy skins each vertex with at most this many joints.
const JOINTS_PER_VERTEX: usize = 4;

/// A model ready to spawn. Cheap to clone (handles and small tables).
#[derive(Clone)]
pub struct ModelAsset {
    nodes: Vec<NodeAsset>,
    pieces: Vec<Piece>,
    /// Bounds of the model in its bind pose, in model space.
    pub min: Vec3,
    pub max: Vec3,
    /// Average position of the skeleton's nodes in the rest pose, in model space.
    pub node_centre: Vec3,
}

#[derive(Clone)]
struct NodeAsset {
    /// The file's identifier for the node; [`NO_ID`] on most.
    id: u8,
    parent: Option<usize>,
    /// Relative to its parent.
    rest: Transform,
    /// The same in the model's space.
    placed: Transform,
    /// See [`Skeleton::fitted`].
    fitted: bool,
    has_child: bool,
    /// The first node after this one and all that is under it.
    after: Option<usize>,
}

/// One submesh, ready to draw.
#[derive(Clone)]
struct Piece {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    binding: Binding,
    /// The id of the part this belongs to, if the model groups it into one.
    part: Option<u16>,
}

/// The node id of nodes the game never looks up.
pub const NO_ID: u8 = 0xff;

#[derive(Clone)]
enum Binding {
    /// Hangs off one node.
    Rigid(usize),
    /// Follows several nodes through per-vertex weights.
    Skinned { joints: Vec<usize>, inverse_bindposes: Handle<SkinnedMeshInverseBindposes> },
}

/// On a spawned model's root entity: its node entities in model order, and the
/// transforms the model file gives them.
///
/// The node entities all hang off the root, each placed in the model's space: the
/// animation works out where every node is (see [`crate::animation`]), and a model
/// nothing animates stays as its file has it.
#[derive(Component)]
pub struct Skeleton {
    pub nodes: Vec<Entity>,
    /// Each node relative to its parent.
    pub rest: Vec<Transform>,
    /// Each node's parent, which always comes before it; `None` is the model itself.
    pub parents: Vec<Option<usize>>,
    /// Whether an animation's translation of the node is fitted to this model's
    /// proportions (see [`crate::animation::fit`]): true of a skeleton node whose
    /// parent is one too, as the original has it (0x21530).
    pub fitted: Vec<bool>,
    /// Whether each node has nodes under it, and the first node after it and all that
    /// is under it: what an animation made for another skeleton finds its nodes by
    /// (see [`crate::animation::match_nodes`]).
    pub has_child: Vec<bool>,
    pub after: Vec<Option<usize>>,
    /// The file's identifier for each node, which is how the game refers to the ones
    /// it needs (a hand to hold something, a muzzle); [`NO_ID`] on the rest.
    pub ids: Vec<u8>,
}

impl Skeleton {
    /// Index of the node with the given identifier.
    pub fn index_of(&self, id: u8) -> Option<usize> {
        self.ids.iter().position(|&node| node == id)
    }

    /// The entity of the node with the given identifier.
    pub fn node(&self, id: u8) -> Option<Entity> {
        self.index_of(id).map(|index| self.nodes[index])
    }
}

/// On a spawned model's root entity: the mesh entities of each of the model's parts,
/// for showing and hiding them by id.
#[derive(Component, Default)]
pub struct Parts(Vec<(u16, Vec<Entity>)>);

impl Parts {
    pub fn ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.0.iter().map(|(id, _)| *id)
    }

    /// The mesh entities of part `id`; empty if the model has no such part.
    pub fn entities(&self, id: u16) -> &[Entity] {
        self.0.iter().find(|(part, _)| *part == id).map_or(&[], |(_, entities)| entities)
    }

    /// Shows the parts whose ids are in `shown` and hides the others.
    pub fn show_only(&self, shown: &[u16], visibility: &mut Query<&mut Visibility>) {
        for (id, entities) in &self.0 {
            let state = if shown.contains(id) { Visibility::Inherited } else { Visibility::Hidden };
            for &entity in entities {
                if let Ok(mut visibility) = visibility.get_mut(entity) {
                    *visibility = state;
                }
            }
        }
    }
}

impl ModelAsset {
    /// Builds the meshes, textures and materials for `model`, whose file bytes are `data`.
    pub fn build(model: &insm::Model, data: &[u8], stores: &mut AssetStores) -> ModelAsset {
        // Each picture, and whether any pixel of it is partly there.
        let textures: Vec<(Handle<Image>, bool)> = model
            .textures
            .iter()
            .map(|(offset, header)| {
                let image = texture::image(&data[*offset..], header);
                let soft = texture::has_soft_alpha(&image);
                (stores.images.add(image), soft)
            })
            .collect();
        let materials: Vec<Handle<StandardMaterial>> = model
            .materials
            .iter()
            .enumerate()
            .map(|(index, m)| {
                let look = Look::of(&m.state);
                let picture = m.texture.filter(|_| look.pictured).map(|t| &textures[t]);
                let partly = picture.is_some_and(|(_, soft)| *soft) || corners_partly_there(model, index);
                let mut material = texture::flat_material(picture.map(|(handle, _)| handle.clone()));
                material.alpha_mode = look.alpha_mode(partly);
                let material = stores.materials.add(material);
                stores.looks.set(&material, look);
                material
            })
            .collect();

        let mut nodes = Vec::with_capacity(model.nodes.len());
        let mut world: Vec<Transform> = Vec::with_capacity(model.nodes.len());
        for node in &model.nodes {
            let rotation = Quat::from_array(node.rotation);
            let rest = Transform {
                translation: Vec3::from(node.translation),
                rotation: if rotation.length_squared() > 1e-6 { rotation.normalize() } else { Quat::IDENTITY },
                scale: Vec3::ONE,
            };
            let placed = node.parent.map_or(rest, |parent| world[parent] * rest);
            world.push(placed);
            let fitted = node.in_skeleton() && node.parent.is_some_and(|parent| model.nodes[parent].in_skeleton());
            nodes.push(NodeAsset {
                id: node.id,
                parent: node.parent,
                rest,
                placed,
                fitted,
                has_child: node.has_child(),
                after: node.next,
            });
        }

        let mut inverse_bind = vec![Mat4::IDENTITY; model.nodes.len()];
        for binding in &model.bindings {
            if let Some(matrix) = &binding.inverse_bind {
                inverse_bind[binding.node] = Mat4::from_cols_array(matrix);
            }
        }

        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut pieces = Vec::new();
        for (index, submesh) in model.submeshes.iter().enumerate().filter(|(_, s)| !s.indices.is_empty()) {
            // Rigid vertices are in their node's space, skinned ones in model space.
            let to_model = if submesh.rigid { world[submesh.nodes[0]] } else { Transform::IDENTITY };
            for vertex in &submesh.vertices {
                let p = to_model.transform_point(Vec3::from(vertex.position));
                min = min.min(p);
                max = max.max(p);
            }
            let binding = if submesh.rigid {
                Binding::Rigid(submesh.nodes[0])
            } else {
                let matrices: Vec<Mat4> = submesh.nodes.iter().map(|&n| inverse_bind[n]).collect();
                Binding::Skinned {
                    joints: submesh.nodes.clone(),
                    inverse_bindposes: stores.inverse_bindposes.add(matrices),
                }
            };
            pieces.push(Piece {
                mesh: stores.meshes.add(build_mesh(submesh)),
                material: materials[submesh.material].clone(),
                binding,
                part: model.parts.iter().find(|part| part.submeshes.contains(&index)).map(|part| part.id),
            });
        }
        let node_centre = world.iter().map(|t| t.translation).sum::<Vec3>() / world.len().max(1) as f32;
        ModelAsset { nodes, pieces, min, max, node_centre }
    }

    /// Spawns an instance and returns its root entity, which carries a [`Skeleton`].
    /// Move, rotate or scale the root to place the model.
    pub fn spawn(&self, commands: &mut Commands, transform: Transform) -> Entity {
        self.spawn_nodes(commands, transform).0
    }

    /// Like [`ModelAsset::spawn`], and also returns the nodes with the identifiers
    /// `ids`, for hanging something else off them (a weapon in a hand).
    pub fn spawn_with_nodes<const N: usize>(&self, commands: &mut Commands, transform: Transform, ids: [u8; N]) -> (Entity, [Option<Entity>; N]) {
        let (root, nodes) = self.spawn_nodes(commands, transform);
        (root, ids.map(|id| self.nodes.iter().position(|node| node.id == id).map(|index| nodes[index])))
    }

    fn spawn_nodes(&self, commands: &mut Commands, transform: Transform) -> (Entity, Vec<Entity>) {
        let root = commands.spawn((transform, Visibility::default())).id();

        let entities: Vec<Entity> = self
            .nodes
            .iter()
            .map(|node| commands.spawn((node.placed, Visibility::default(), ChildOf(root))).id())
            .collect();

        let mut parts: Vec<(u16, Vec<Entity>)> = Vec::new();
        for piece in &self.pieces {
            let mut entity = commands.spawn((Mesh3d(piece.mesh.clone()), MeshMaterial3d(piece.material.clone())));
            if let Some(id) = piece.part {
                match parts.iter_mut().find(|(part, _)| *part == id) {
                    Some((_, entities)) => entities.push(entity.id()),
                    None => parts.push((id, vec![entity.id()])),
                }
            }
            match &piece.binding {
                Binding::Rigid(node) => {
                    entity.insert(ChildOf(entities[*node]));
                }
                Binding::Skinned { joints, inverse_bindposes } => {
                    entity.insert((
                        ChildOf(root),
                        SkinnedMesh {
                            inverse_bindposes: inverse_bindposes.clone(),
                            joints: joints.iter().map(|&n| entities[n]).collect(),
                        },
                        // The mesh moves with its joints, so its static bounds are meaningless.
                        NoFrustumCulling,
                    ));
                }
            }
        }

        let rest = self.nodes.iter().map(|n| n.rest).collect();
        let parents = self.nodes.iter().map(|n| n.parent).collect();
        let ids = self.nodes.iter().map(|n| n.id).collect();
        let skeleton = Skeleton {
            nodes: entities.clone(),
            rest,
            parents,
            fitted: self.nodes.iter().map(|n| n.fitted).collect(),
            has_child: self.nodes.iter().map(|n| n.has_child).collect(),
            after: self.nodes.iter().map(|n| n.after).collect(),
            ids,
        };
        commands.entity(root).insert((skeleton, Parts(parts)));
        (root, entities)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Whether a corner of any piece drawn with material `index` is partly there.
fn corners_partly_there(model: &insm::Model, index: usize) -> bool {
    model.submeshes.iter().any(|s| s.material == index && s.coloured && s.vertices.iter().any(|v| v.color[3] != 255))
}

pub(crate) fn build_mesh(submesh: &insm::Submesh) -> Mesh {
    let vertices = &submesh.vertices;
    let colors: Vec<[f32; 4]> = vertices
        .iter()
        // The game's own numbers (see `texture`).
        .map(|v| v.color.map(|channel| channel as f32 / 255.0))
        .collect();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices.iter().map(|v| v.position).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vertices.iter().map(|v| v.normal).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vertices.iter().map(|v| v.uv).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(submesh.indices.clone()));

    if !submesh.rigid {
        let (joints, weights): (Vec<_>, Vec<_>) =
            vertices.iter().map(|v| strongest_joints(&v.weights[..submesh.nodes.len()])).unzip();
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(joints));
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    }
    mesh
}

/// Picks a vertex's heaviest joints and rescales their weights to sum to one.
///
/// The game allows up to eight weights per vertex; only two strips in the whole game
/// use more than four.
fn strongest_joints(weights: &[f32]) -> ([u16; JOINTS_PER_VERTEX], [f32; JOINTS_PER_VERTEX]) {
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| weights[b].total_cmp(&weights[a]));

    let mut joints = [0u16; JOINTS_PER_VERTEX];
    let mut picked = [0.0f32; JOINTS_PER_VERTEX];
    for (slot, &joint) in order.iter().take(JOINTS_PER_VERTEX).enumerate() {
        joints[slot] = joint as u16;
        picked[slot] = weights[joint].max(0.0);
    }
    let total: f32 = picked.iter().sum();
    if total > 1e-6 {
        (joints, picked.map(|w| w / total))
    } else {
        (joints, [1.0, 0.0, 0.0, 0.0])
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::lit::Blend;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    fn models(folder: &Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                models(&path, found);
            } else if path.extension().is_some_and(|e| e == "insm" || e == "mdl") {
                found.push(path);
            }
        }
    }

    /// How the materials of the game's own models come to be drawn: most as
    /// solid ones, the glows added, and what is partly there mixed.
    #[test]
    fn the_games_models_are_drawn_as_their_materials_say() {
        let mut found = Vec::new();
        models(Path::new(DATA), &mut found);
        if found.is_empty() {
            return;
        }
        let (mut solid, mut mixed, mut added, mut by_corners, mut mixed_in) = (0, 0, 0, 0, Vec::new());
        for path in &found {
            let Ok(data) = std::fs::read(path) else { continue };
            let Ok(model) = insm::Model::parse(&data) else { continue };
            let soft: Vec<bool> = model.textures.iter().map(|(offset, header)| texture::has_soft_alpha(&texture::image(&data[*offset..], header))).collect();
            for (index, m) in model.materials.iter().enumerate() {
                let look = Look::of(&m.state);
                let corners = corners_partly_there(&model, index);
                let partly = m.texture.filter(|_| look.pictured).is_some_and(|t| soft[t]) || corners;
                match look.alpha_mode(partly) {
                    AlphaMode::Add => {
                        added += 1;
                        assert!((look.blend, look.depth_write) == (Blend::Added, false), "{}", path.display());
                    }
                    AlphaMode::Blend => {
                        mixed += 1;
                        by_corners += usize::from(corners);
                        let name = path.strip_prefix(DATA).unwrap_or(path).display().to_string();
                        if mixed_in.last() != Some(&name) {
                            mixed_in.push(name);
                        }
                    }
                    _ => solid += 1,
                }
            }
        }
        println!("{} models: {solid} solid, {mixed} mixed ({by_corners} by their corners) in {} models, {added} added", found.len(), mixed_in.len());
        for name in &mixed_in {
            println!("  {name}");
        }
        assert!(solid > mixed && added > 0);
    }
}
