//! Map geometry: one mesh for each run of the map's list that is drawn the same
//! way, lit by its baked vertex colours, and drawn in the list's own order.
//!
//! The PSP draws a map as one list, strip after strip, each tested against the
//! depth of what was drawn before it and, where its material says so, mixed with
//! what is there. So what a see-through strip shows through it is what came
//! before it in the list, and it hides what comes after it and lies behind it: a
//! polished floor is the first thing drawn of its hall, mixed with the colour the
//! screen was cleared to, and the hall hung upside down under it is seen only
//! where the floor is not drawn at all. Here every batch of a map is therefore
//! drawn in one pass, in the list's order, each writing its depth or not as its
//! material says; whatever else is seen through (effects, a battle's shots) is
//! drawn after all of a map.
//!
//! A field map has two pieces, and the game draws the small one first (0x10EC28):
//! the sky and whatever stands far off, with a lens of its own range, with no
//! test against the depth and none of it written, before the fog is set. Then
//! the map proper, tested and written. Here the far piece is tested all the same,
//! since the characters are drawn before any of a map and it must not be drawn
//! over them; it writes no depth, so nothing of it hides anything of the map.

use bevy::asset::{uuid_handle, RenderAssetUsages};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Face, PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use brs_formats::stcm;

use crate::texture;
use crate::AssetStores;

const SHADER: Handle<Shader> = uuid_handle!("2f6b8d14-c0a7-4e39-9b52-71d3e8a4f6c5");

/// How much a batch's place in the order of a map's drawing counts for against how
/// far it is, where what is seen through is put in order: more than a map is wide.
/// A map's batches come before everything that has nothing taken off how far it
/// is, the first of them the furthest before.
const TURN_DEPTH: f32 = 1_000_000.0;
/// More batches than the maps of one scene have between them.
const TURNS: f32 = 4096.0;

// The bits of what the shader is told of a material.
const PICTURE_ALPHA_COUNTS: u32 = 1;
const PICTURE_ADDED: u32 = 2;
const ALL_THERE: u32 = 4;

/// What [`spawn`] made.
pub struct Spawned {
    /// One entity per batch of the map.
    pub entities: Vec<Entity>,
    pub min: Vec3,
    pub max: Vec3,
}

/// What a batch of a map is drawn with.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(Drawn)]
pub struct MapMaterial {
    #[texture(0)]
    #[sampler(1)]
    picture: Option<Handle<Image>>,
    /// The least alpha that is drawn, in parts of one; the bits above; what an
    /// added picture is taken by; and whether the fog is on it.
    #[uniform(2)]
    look: Vec4,
    /// How far before the camera its lens sees, and from how near: nothing of
    /// it is drawn beyond the first (nought is no end) or nearer than the second.
    #[uniform(3)]
    reach: Vec4,
    piece: Piece,
    blend: stcm::Blend,
    depth_write: bool,
    one_side: bool,
    /// Its place in the order of the scene's maps' drawing.
    turn: usize,
}

/// The part of a material that chooses how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Drawn {
    blend: stcm::Blend,
    depth_write: bool,
    one_side: bool,
}

impl From<&MapMaterial> for Drawn {
    fn from(material: &MapMaterial) -> Drawn {
        Drawn { blend: material.blend, depth_write: material.depth_write, one_side: material.one_side }
    }
}

impl MapMaterial {
    /// Has the scene proper seen from so near to so far before the camera: the
    /// two sides of the lens the PSP draws it with, which a scene's script moves
    /// (0xD9D54, its + 0xD0). What is far off about a scene has a lens of its
    /// own (see [`FAR_OFF_FROM`]) and is left as it is. Says whether that
    /// changed anything.
    pub fn seen_between(&mut self, near: f32, far: f32) -> bool {
        let changes = self.piece == Piece::Scene && (self.reach.x != far || self.reach.y != near);
        if changes {
            (self.reach.x, self.reach.y) = (far, near);
        }
        changes
    }
}

impl Material for MapMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// Every batch goes with what is seen through, which is drawn in order.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// The order is the list's: see [`TURN_DEPTH`].
    fn depth_bias(&self) -> f32 {
        (self.turn as f32 - TURNS) * TURN_DEPTH
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let corners = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![corners];
        let Drawn { blend, depth_write, one_side } = key.bind_group_data;
        // A batch drawn with culling on shows only the side of its faces that its
        // corners go round anticlockwise on: the game has that for the front
        // wherever it draws a scene (0x15AB34 with 1, as at 0x2F2C). A wall of the
        // edge of a map is such a batch, and a camera put behind it sees through.
        descriptor.primitive.cull_mode = one_side.then_some(Face::Back);
        // What is there keeps how much of it there is.
        let kept = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
        let by_alpha = |dst_factor: BlendFactor, operation: BlendOperation| BlendState { color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor, operation }, alpha: kept };
        let mixed = match blend {
            // What replaces what is there is all there (the shader's doing), and
            // laid over it comes to the same.
            stcm::Blend::Opaque | stcm::Blend::Alpha => by_alpha(BlendFactor::OneMinusSrcAlpha, BlendOperation::Add),
            stcm::Blend::Add => by_alpha(BlendFactor::One, BlendOperation::Add),
            // What is there, less what it lets through, less the colour by its alpha.
            stcm::Blend::Subtract => by_alpha(BlendFactor::OneMinusSrcAlpha, BlendOperation::ReverseSubtract),
        };
        for target in descriptor.fragment.iter_mut().flat_map(|fragment| fragment.targets.iter_mut()).flatten() {
            target.blend = Some(mixed);
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_write_enabled = Some(depth_write);
        }
        Ok(())
    }
}

/// How near the lens sees that the field draws what is far off through
/// (0xD9E1C: from 50 out to 200000), whatever the scene's own does.
const FAR_OFF_FROM: f32 = 50.0;

/// Which piece of a scene a map is: the scene proper, or what is far off about
/// it, which is drawn before it and hides nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Scene,
    FarOff,
}

/// Spawns `map` (whose file bytes are `data`) at `transform`. `turn` is how many
/// batches of the scene's maps have been spawned before it, in the order the game
/// draws them, and is moved on by this map's.
pub fn spawn(commands: &mut Commands, map: &stcm::Map, data: &[u8], transform: Transform, piece: Piece, turn: &mut usize, stores: &mut AssetStores) -> Spawned {
    let built = Built::build(map, data, piece, turn, stores);
    let mut spawned = Spawned { entities: built.spawn(commands, transform), min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) };
    if built.min.cmple(built.max).all() {
        for corner in 0..8 {
            let pick = |axis: usize| if corner >> axis & 1 == 0 { built.min[axis] } else { built.max[axis] };
            let at = transform.transform_point(Vec3::new(pick(0), pick(1), pick(2)));
            (spawned.min, spawned.max) = (spawned.min.min(at), spawned.max.max(at));
        }
    }
    spawned
}

/// A map made ready to be put somewhere, any number of times: what each of its
/// batches is drawn with is made once and shared by every copy. The road of the
/// ride on the bike is a hundred rows of the same few pieces.
pub struct Built {
    batches: Vec<(Handle<Mesh>, Handle<MapMaterial>)>,
    /// The corners of the box all of it is in, as the map has it.
    pub min: Vec3,
    pub max: Vec3,
}

impl Built {
    /// Makes `map` (whose file bytes are `data`) ready. `turn` is as for [`spawn`].
    pub fn build(map: &stcm::Map, data: &[u8], piece: Piece, turn: &mut usize, stores: &mut AssetStores) -> Built {
        let mut built = Built { batches: Vec::new(), min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) };
        for (mesh, drawn_with, low, high) in batches(map, data, piece, turn, stores) {
            built.batches.push((stores.meshes.add(mesh), stores.maps.add(drawn_with)));
            (built.min, built.max) = (built.min.min(low), built.max.max(high));
        }
        built
    }

    /// Puts a copy of it at `transform`: one entity per batch.
    pub fn spawn(&self, commands: &mut Commands, transform: Transform) -> Vec<Entity> {
        self.batches.iter().map(|(mesh, material)| commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform)).id()).collect()
    }
}

/// The mesh and the material of each batch of a map that draws anything, and the
/// corners of the box it is in.
fn batches(map: &stcm::Map, data: &[u8], piece: Piece, turn: &mut usize, stores: &mut AssetStores) -> Vec<(Mesh, MapMaterial, Vec3, Vec3)> {
    let textures: Vec<Handle<Image>> = map.textures.iter().map(|(offset, header)| stores.images.add(texture::image(&data[*offset..], header))).collect();

    let mut made = Vec::new();
    for batch in map.batches.iter().filter(|b| !b.indices.is_empty()) {
        let vertices = &batch.vertices;
        let material = &batch.material;
        // Where the material says so the game doubles texture x vertex colour, so that
        // mid-grey means "unchanged" and what is lighter than that lightens. The
        // colours are the game's own numbers (see `texture`), and what comes of
        // the doubling is cut off at full only once it is drawn.
        let scale = if material.double_color { 2.0 } else { 1.0 };
        let colors: Vec<[f32; 4]> = vertices
            .iter()
            .map(|v| {
                let channel = |c: u8| c as f32 / 255.0 * scale;
                [channel(v.color[0]), channel(v.color[1]), channel(v.color[2]), v.color[3] as f32 / 255.0]
            })
            .collect();
        let uvs: Vec<[f32; 2]> =
            vertices.iter().map(|v| [v.uv[0] + material.uv_offset[0], v.uv[1] + material.uv_offset[1]]).collect();
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices.iter().map(|v| v.position).collect::<Vec<_>>())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
            .with_inserted_indices(Indices::U32(batch.indices.clone()));

        let mut bits = 0;
        if material.texture_alpha {
            bits |= PICTURE_ALPHA_COUNTS;
        }
        if material.add_texture {
            bits |= PICTURE_ADDED;
        }
        if material.blend == stcm::Blend::Opaque {
            bits |= ALL_THERE;
        }
        let drawn_with = MapMaterial {
            picture: batch.texture.map(|texture| textures[texture].clone()),
            look: Vec4::new(material.alpha_cutoff as f32 / 255.0, bits as f32, scale, if material.fog { 1.0 } else { 0.0 }),
            reach: Vec4::new(0.0, if piece == Piece::FarOff { FAR_OFF_FROM } else { 0.0 }, 0.0, 0.0),
            piece,
            blend: material.blend,
            depth_write: material.depth_write && piece == Piece::Scene,
            one_side: batch.cull,
            turn: *turn,
        };
        *turn += 1;
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for vertex in vertices {
            let at = Vec3::from(vertex.position);
            (low, high) = (low.min(at), high.max(at));
        }
        made.push((mesh, drawn_with, low, high));
    }
    made
}

pub(crate) fn plugin(app: &mut App) {
    let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
    shaders.insert(&SHADER, Shader::from_wgsl(include_str!("map.wgsl"), file!())).expect("the maps' shader has a handle of its own");
    app.add_plugins(MaterialPlugin::<MapMaterial>::default());
}
