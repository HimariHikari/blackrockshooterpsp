//! Effects in the world: an effect file's pictures, models and scripts made ready,
//! the effects that play stepped with the game's frames, and what there is to see
//! of them drawn. How an effect goes is [`crate::effect`]'s.
//!
//! Every particle is a mesh of its own entity: a flat one the one square all of
//! them share, a model one the pieces of its model. Its colour goes with it as the
//! mesh's tag, so that all the particles of a picture are drawn with one material.
//!
//! What is fixed to the screen is drawn as the rest is, by a camera that sees the
//! screen flat in its own pixels (0x128C78): one that this keeps, which draws after
//! the cameras of a scene, and for what is of a scene's first draw list one that
//! whoever has that scene's cameras puts before them ([`screen_camera`]).
//!
//! The original draws the emitters of an effect in an order of the script's, each
//! over what was drawn before with the depth read and not written (0x1288B4,
//! 0x127D90). So that the same is over the same here, an emitter's turn in that
//! order counts for more than how far its particles are when what is seen through
//! is put in order.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::asset::{uuid_handle, RenderAssetUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::ScalingMode;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::Affine3A;
use bevy::mesh::{Indices, MeshTag, MeshVertexBufferLayoutRef};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction, PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use brs_formats::esb::{self, Blend};
use brs_formats::{efc, insa, insm, ptmd};

use crate::animation::{match_nodes, transform_of, Clip, Joint, Posed};
use crate::canvas;
use crate::effect::{Body, Camera, Effect, Pool};
use crate::model::{build_mesh, Skeleton};
use crate::{texture, AssetStores};

const SHADER: Handle<Shader> = uuid_handle!("4b7a2f90-1d3e-4c86-b5a1-9e0f6d2c7a13");

/// How much an emitter's turn in the order of its effect counts for against how
/// far a particle is, and how much being of an effect at all does: more than the
/// world is wide.
const TURN_DEPTH: f32 = 100_000.0;
/// What being of the scene's last draw list counts for: more than any turn.
const ON_TOP_DEPTH: f32 = 20_000_000.0;
/// What a place in an order that whoever plays the effects has given them counts
/// for: more than the turns of an effect's emitters, of which there are few.
const ORDER_DEPTH: f32 = 2_000_000.0;

/// The layer the particles that are fixed to the screen are drawn in when their
/// effect is of a scene's first draw list: for a camera of that scene's, which
/// draws before its others.
pub const SCREEN_BEHIND_LAYER: usize = 29;
/// The layer the rest of them are drawn in, for the camera this keeps.
const SCREEN_LAYER: usize = 30;
/// When that camera draws: after the cameras of a scene, and before whatever
/// has the whole picture for itself.
const SCREEN_ORDER: isize = 3;
/// How far before and behind the screen such a camera sees (0x128C88).
const SCREEN_DEPTH: f32 = 500.0;

/// What a camera has to see the screen flat, in its own pixels up from its
/// bottom left corner (0x128C78): from one before it, looking at it, with as
/// much of it as there is fitted to what is drawn to.
pub fn screen_camera() -> impl Bundle {
    (
        texture::camera_of_values(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin { min_width: canvas::WIDTH, min_height: canvas::HEIGHT },
            near: -SCREEN_DEPTH,
            far: SCREEN_DEPTH,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(canvas::WIDTH / 2.0, canvas::HEIGHT / 2.0, 1.0),
    )
}

/// The camera this keeps for what is fixed to the screen: it draws while there
/// is any of it.
#[derive(Component)]
pub struct ScreenCamera;

fn setup(mut commands: Commands) {
    commands.spawn((
        ScreenCamera,
        Camera3d::default(),
        bevy::camera::Camera { order: SCREEN_ORDER, clear_color: ClearColorConfig::None, is_active: false, ..default() },
        screen_camera(),
        RenderLayers::layer(SCREEN_LAYER),
    ));
}

/// The command of a material's list that says how its texture goes on past its
/// edges, across and down: nought round again, one held at the edge.
const GE_TEXTURE_WRAP: u8 = 0xc7;

/// A model of an effect file, made ready.
struct Shape {
    nodes: Vec<ShapeNode>,
    pieces: Vec<Piece>,
    /// The animation that moves it, and which of its nodes moves each of the
    /// model's.
    clip: Option<(Arc<Clip>, Vec<Option<usize>>)>,
}

struct ShapeNode {
    parent: Option<usize>,
    rest: Transform,
    fitted: bool,
}

struct Piece {
    mesh: Handle<Mesh>,
    picture: Option<Handle<Image>>,
    /// The node it hangs from; one that goes by several is drawn as the model
    /// has it at rest.
    node: Option<usize>,
    /// Its corners have no colours of their own.
    plain: bool,
}

impl Shape {
    fn new(model: &insm::Model, data: &[u8], animation: Option<insa::Animation>, stores: &mut AssetStores) -> Shape {
        // A model's pictures go on past their edges as its materials say, and are
        // drawn smoothed (0x122780).
        let pictures: Vec<Handle<Image>> = model
            .textures
            .iter()
            .enumerate()
            .map(|(index, (offset, header))| {
                let wrap = model.materials.iter().filter(|material| material.texture == Some(index)).find_map(|material| material.state.iter().find(|(command, _)| *command == GE_TEXTURE_WRAP).map(|(_, wrap)| *wrap));
                let held = |held: bool| if held { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
                let wrap = wrap.unwrap_or(0);
                stores.images.add(picture(&data[*offset..], header, held(wrap & 0xff != 0), held(wrap >> 8 & 0xff != 0)))
            })
            .collect();
        let nodes: Vec<ShapeNode> = model
            .nodes
            .iter()
            .map(|node| {
                let rotation = Quat::from_array(node.rotation);
                let rotation = if rotation.length_squared() > 1e-6 { rotation.normalize() } else { Quat::IDENTITY };
                let fitted = node.in_skeleton() && node.parent.is_some_and(|parent| model.nodes[parent].in_skeleton());
                ShapeNode { parent: node.parent, rest: Transform { translation: Vec3::from(node.translation), rotation, scale: Vec3::ONE }, fitted }
            })
            .collect();
        let pieces = model
            .submeshes
            .iter()
            .filter(|submesh| !submesh.indices.is_empty())
            .map(|submesh| Piece {
                mesh: stores.meshes.add(build_mesh(submesh)),
                picture: model.materials[submesh.material].texture.map(|texture| pictures[texture].clone()),
                node: submesh.rigid.then(|| submesh.nodes[0]),
                plain: !submesh.coloured,
            })
            .collect();
        let clip = animation.map(|animation| {
            // Which of the animation's nodes moves which of the model's is found
            // as for any model.
            let skeleton = Skeleton {
                nodes: Vec::new(),
                rest: nodes.iter().map(|node| node.rest).collect(),
                parents: nodes.iter().map(|node| node.parent).collect(),
                fitted: nodes.iter().map(|node| node.fitted).collect(),
                has_child: model.nodes.iter().map(insm::Node::has_child).collect(),
                after: model.nodes.iter().map(|node| node.next).collect(),
                ids: model.nodes.iter().map(|node| node.id).collect(),
            };
            let matched = match_nodes(&skeleton, &animation.nodes);
            (Clip::new(animation), matched)
        });
        Shape { nodes, pieces, clip }
    }

    /// Where each of its nodes is in the model's own space, so many of its
    /// animation's frames in: it plays once and holds at its end (0x2359C).
    fn posed(&self, frames: f32) -> Vec<Affine3A> {
        let mut placed: Vec<Affine3A> = Vec::with_capacity(self.nodes.len());
        let mut scales: Vec<Vec3> = Vec::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let joint = Joint { rest: node.rest.translation, fitted: node.fitted };
            let posed = self
                .clip
                .as_ref()
                .and_then(|(clip, matched)| {
                    let animation = &clip.animation;
                    let frame = frames.min(animation.end.saturating_sub(animation.start) as f32 / insa::UNITS_PER_FRAME as f32);
                    clip.pose(matched[index]?, frame, &joint, node.parent.map(|parent| scales[parent]), (node.rest.rotation, node.rest.translation), None)
                })
                .unwrap_or_else(|| Posed::rest(node.rest));
            placed.push(node.parent.map_or(posed.local, |parent| placed[parent] * posed.local));
            scales.push(posed.scale);
        }
        placed
    }
}

/// A picture of an effect, drawn smoothed (0x126CFC).
fn picture(data: &[u8], header: &ptmd::Header, across: ImageAddressMode, down: ImageAddressMode) -> Image {
    let mut image = texture::image(data, header);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: across,
        address_mode_v: down,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// An effect file made ready: its effects by their numbers, and what they are
/// made of.
pub struct EffectFile {
    scripts: HashMap<u32, Arc<esb::Script>>,
    /// The file's own pictures, which flat particles are of, by their place
    /// among them.
    pictures: Vec<Handle<Image>>,
    shapes: Vec<Shape>,
}

impl EffectFile {
    pub fn load(data: &[u8], stores: &mut AssetStores) -> Result<Arc<EffectFile>> {
        let file = efc::File::parse(data)?;
        let (mut scripts, mut pictures, mut shapes) = (HashMap::new(), Vec::new(), Vec::new());
        for (index, part) in file.parts.iter().enumerate() {
            let bytes = &data[part.data.clone()];
            match part.kind {
                efc::Kind::Script => {
                    let script = esb::Script::parse(bytes, file.version).with_context(|| format!("the script {}", part.name))?;
                    if let Some(number) = part.number() {
                        scripts.insert(number, Arc::new(script));
                    }
                }
                efc::Kind::Model => {
                    let model = insm::Model::parse(bytes).with_context(|| format!("the model {}", part.name))?;
                    // The animation of its nodes is the last such after it; one
                    // of its pictures is not taken (0x1223B8).
                    let of_nodes = |name: &str| [".anm", ".imm"].iter().any(|ending| name.to_ascii_lowercase().ends_with(ending));
                    let animation = file.animations_of(index).iter().rev().find(|part| of_nodes(&part.name));
                    let animation = animation.map(|part| insa::Animation::parse(&data[part.data.clone()]).with_context(|| format!("the animation {}", part.name))).transpose()?;
                    shapes.push(Shape::new(&model, bytes, animation, stores));
                }
                efc::Kind::Animation => {}
                efc::Kind::Texture => {
                    let header = ptmd::parse_header(bytes).with_context(|| format!("the texture {}", part.name))?;
                    // A flat particle's picture is held at its edges (0x126CF0).
                    pictures.push(stores.images.add(picture(bytes, &header, ImageAddressMode::ClampToEdge, ImageAddressMode::ClampToEdge)));
                }
            }
        }
        Ok(Arc::new(EffectFile { scripts, pictures, shapes }))
    }

    /// The numbers of its effects, in order.
    pub fn numbers(&self) -> Vec<u32> {
        let mut numbers: Vec<u32> = self.scripts.keys().copied().collect();
        numbers.sort_unstable();
        numbers
    }

    /// One of its effects, to play.
    pub fn play(self: &Arc<EffectFile>, number: u32) -> Option<Playing> {
        let script = self.scripts.get(&number)?;
        Some(Playing::new(Effect::new(Arc::clone(script)), Arc::clone(self)))
    }
}

/// An effect that is playing, as the scenes' engine keeps one (0xA5D94). Its
/// entity is at nowhere in particular: the effect is put where it is to be by
/// [`Playing::place`]. It is taken away when the effect is over.
///
/// It has no time of its own. Whoever it belongs to gives it the time that goes
/// by ([`Playing::give`], 0xA5E4C), and it has that time when it is next drawn,
/// which is once a frame while it is to be seen (0xA5E90). The first time it is
/// drawn it has a whole frame besides, before any time it has been given: so an
/// effect is first seen a frame in, and one that was given time while it could
/// not be seen that much further.
#[derive(Component)]
#[require(Transform, Visibility)]
pub struct Playing {
    pub effect: Effect,
    file: Arc<EffectFile>,
    /// The entities its particles are drawn as, used again from frame to frame.
    shown: Vec<Entity>,
    /// The time it has been given and has not had, in sixtieths of a second (its
    /// + 0x54).
    waiting: f32,
    /// The time it has had (its + 0x58).
    had: f32,
    /// It has had its first frame (its flag 2 is off).
    begun: bool,
    /// It has been put somewhere since it was last drawn (its flag 1).
    put: bool,
    /// It is not to be seen (0xAC8A4), and so is not drawn and has no frames:
    /// the time it is given waits for it.
    pub hidden: bool,
    /// It has its frames and is not drawn: what the field does with one of a
    /// character that is hidden, which goes on all the same (0xF5E50, 0x106838).
    pub unseen: bool,
    /// It is given no time by the clock, where the clock is what gives it.
    pub still: bool,
    /// It is drawn after all else of the scene and whatever is nearer: it is of
    /// the scene's fifth draw list, which is drawn last and has the depth
    /// cleared for it (0xA64F4, 0xABB20).
    pub on_top: bool,
    /// It is of the scene's first draw list, the sky's (0xACDD0 with 0), which is
    /// drawn before all else and has the depth cleared after it (0xABAB8): its
    /// particles are drawn in these layers, for whichever camera draws that
    /// list, and write no depth. As it is when it is first drawn.
    pub behind: Option<RenderLayers>,
    /// It is drawn apart from the scene, by a camera of whoever made it: what it
    /// has in the world is drawn in these layers. (What it has on the screen is
    /// drawn there as ever.) As it is when it is first drawn.
    pub apart: Option<RenderLayers>,
    /// It is kept when its effect is over, for whoever made it to take away.
    pub stays: bool,
    /// Which lens what it has in the world is drawn through.
    pub lens: Lens,
    /// Its place among the effects whose order of drawing has been given: one of
    /// a higher number is drawn after one of a lower, however far each is, and
    /// all of them after those that have none (nought).
    pub order: u32,
}

/// Which lens an effect in the world is drawn through, where whoever plays it
/// has more than one. The field has (0xE7568): its own effects and those its
/// characters are go through the scene's (0xE7ACC from 0x10E898, with nought),
/// and those of a character that asks for it through one that sees from just
/// before the camera (0x103010, 0x103094).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lens {
    /// The camera's own, and all it sees.
    #[default]
    Whole,
    /// The scene's: from how near to how far [`EffectReach`] says.
    Scene,
    /// The one that sees from just before the camera, as far as the scene's.
    Close,
}

/// How near and how far the scene's lens sees, for the effects that are drawn
/// through it or through the one beside it ([`Lens`]); nought is no end.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct EffectReach {
    pub near: f32,
    pub far: f32,
}

/// The sixtieths of a second a frame of an effect is (0xA5F5C).
const SIXTIETHS_A_FRAME: f32 = 2.0;

impl Playing {
    /// One that is yet to be made: it is put where everything begins, and that
    /// is told to its effect the first time it is drawn (0xA5CA0).
    fn new(mut effect: Effect, file: Arc<EffectFile>) -> Playing {
        effect.place(Mat4::IDENTITY);
        Playing { effect, file, shown: Vec::new(), waiting: 0.0, had: 0.0, begun: false, put: true, hidden: false, unseen: false, still: false, on_top: false, behind: None, apart: None, stays: false, lens: Lens::Whole, order: 0 }
    }

    /// Puts it by a matrix (0xA5E20).
    pub fn place(&mut self, matrix: Mat4) {
        self.effect.place(matrix);
        self.put = true;
    }

    /// It is playing: yet to be made, or made and not over (0xA5FF8).
    pub fn playing(&self) -> bool {
        !self.begun || !self.effect.over()
    }

    /// Gives it time, in sixtieths of a second, to have when it is next drawn
    /// (0xA5E4C). One that is over takes none.
    pub fn give(&mut self, sixtieths: f32) {
        if self.playing() {
            self.waiting += sixtieths;
        }
    }

    /// Has so many whole frames at once and is then kept as it is, whatever the
    /// clock does: for looking at one.
    pub fn hold(&mut self, frames: u32, pool: &mut Pool, camera: &Camera) {
        for _ in 0..frames {
            self.effect.step(pool, camera, 1.0);
        }
        self.had += SIXTIETHS_A_FRAME * frames as f32;
        (self.begun, self.put, self.still, self.stays) = (true, false, true, true);
    }

    /// What becomes of it as it is drawn (0xA5E90). The first time, it has a
    /// whole frame (0xA5D48). Then it has the time that waits for it; with none
    /// waiting, one that has been put somewhere since it was last drawn has a
    /// frame of no time, which puts its particles where they now belong.
    fn frame(&mut self, pool: &mut Pool, camera: &Camera) {
        if !self.playing() {
            return;
        }
        let put = std::mem::take(&mut self.put);
        if !self.begun {
            self.begun = true;
            self.effect.step(pool, camera, 1.0);
            self.had += SIXTIETHS_A_FRAME;
        }
        if self.waiting > 0.0 {
            let waited = std::mem::take(&mut self.waiting);
            self.effect.step(pool, camera, waited / SIXTIETHS_A_FRAME);
            self.had += waited;
        } else if put && self.had > 0.0 {
            self.effect.step(pool, camera, 0.0);
        }
    }
}

/// One of the entities a particle is drawn as.
#[derive(Component)]
struct Shown;

/// What all the effects that play share: their random numbers, and how many
/// particles they may have between them.
#[derive(Resource, Default)]
pub struct EffectPool(pub Pool);

/// The camera the effects go by: whoever has the camera of the scene keeps this
/// as it is.
#[derive(Resource, Default)]
pub struct EffectCamera(pub Camera);

/// What a particle is drawn with: its picture, mixed as its emitter says.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(Mixing)]
pub struct EffectMaterial {
    #[texture(0)]
    #[sampler(1)]
    picture: Option<Handle<Image>>,
    blend: Blend,
    /// A model's piece whose corners have no colours: see the shader.
    plain: bool,
    turn: usize,
    /// It is drawn after all else, whatever is nearer.
    on_top: bool,
    /// It is drawn before all else, and nothing is behind it.
    behind: bool,
    /// It is on the screen, where nothing is nearer than anything else.
    screen: bool,
    /// The lens it is drawn through, and how far and from how near that sees.
    lens: Lens,
    /// Its effect's place in an order of drawing that was given, or nought.
    order: u32,
    #[uniform(2)]
    reach: Vec4,
}

impl EffectMaterial {
    /// Has it seen as its lens sees with the scene's seeing so; says whether
    /// that changed anything.
    fn seen_by(&mut self, scene: EffectReach) -> bool {
        let reach = match self.lens {
            Lens::Whole => Vec4::ZERO,
            Lens::Scene => Vec4::new(scene.far, scene.near, 0.0, 0.0),
            Lens::Close => Vec4::new(scene.far, 0.0, 0.0, 0.0),
        };
        let changes = self.reach != reach;
        self.reach = reach;
        changes
    }
}

/// The part of a material that chooses how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mixing {
    blend: Blend,
    plain: bool,
    on_top: bool,
    behind: bool,
    screen: bool,
}

impl From<&EffectMaterial> for Mixing {
    fn from(material: &EffectMaterial) -> Mixing {
        Mixing { blend: material.blend, plain: material.plain, on_top: material.on_top, behind: material.behind, screen: material.screen }
    }
}

impl Material for EffectMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// What is seen through is drawn the furthest first, and this is taken off how
    /// far a thing is: all the effects after everything else, and an emitter after
    /// those whose turn is before its own.
    fn depth_bias(&self) -> f32 {
        TURN_DEPTH * (1 + self.turn) as f32 + ORDER_DEPTH * self.order as f32 + if self.on_top { ON_TOP_DEPTH } else { 0.0 }
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
        descriptor.primitive.cull_mode = None;
        let Mixing { blend, plain, on_top, behind, screen } = key.bind_group_data;
        if plain {
            descriptor.vertex.shader_defs.push("OWN_COLOUR_TWICE".into());
        }
        // What is there keeps how much of it there is, but for what is laid over
        // it or put in its place (0x11C6E0 on).
        let kept = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
        let by_alpha = |operation: BlendOperation| BlendState { color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::One, operation }, alpha: kept };
        let mixed = match blend {
            Blend::Over | Blend::AsBefore => BlendState::ALPHA_BLENDING,
            Blend::Added => by_alpha(BlendOperation::Add),
            Blend::TakenAway => by_alpha(BlendOperation::ReverseSubtract),
            Blend::Solid => BlendState::REPLACE,
        };
        for target in descriptor.fragment.iter_mut().flat_map(|fragment| fragment.targets.iter_mut()).flatten() {
            target.blend = Some(mixed);
        }
        // Only what is all there is written to the depth (0x11C7A4).
        if let Some(depth) = &mut descriptor.depth_stencil {
            // What is of the first draw list has the depth cleared after it.
            depth.depth_write_enabled = Some(blend == Blend::Solid && !behind && !screen);
            // What is of the last draw list is drawn with the depth cleared, and
            // what is on the screen with none.
            if on_top || screen {
                depth.depth_compare = Some(CompareFunction::Always);
            }
        }
        Ok(())
    }
}

/// The materials the particles are drawn with: one for each picture, way of mixing,
/// turn and lens.
#[derive(Resource, Default)]
struct Materials(HashMap<(Option<AssetId<Image>>, Mixing, usize, Lens, u32), Handle<EffectMaterial>>);

/// Gives the materials of the lenses that do not see all how near and how far
/// those see, whenever that changes or there is another material.
fn reach(scene: Res<EffectReach>, mut materials: ResMut<Assets<EffectMaterial>>, mut last: Local<(EffectReach, usize)>) {
    let now = (*scene, materials.len());
    if *last == now {
        return;
    }
    *last = now;
    let ids: Vec<_> = materials.ids().collect();
    for id in ids {
        if materials.get(id).is_some_and(|material| material.clone().seen_by(now.0)) {
            if let Some(mut material) = materials.get_mut(id) {
                material.seen_by(now.0);
            }
        }
    }
}

/// The square every flat particle is: one by one about its middle, all of its
/// picture on it, the picture's top at its top (0x16E3A8).
#[derive(Resource)]
struct Square(Handle<Mesh>);

const SQUARE: [([f32; 3], [f32; 2]); 4] = [([-0.5, 0.5, 0.0], [0.0, 0.0]), ([-0.5, -0.5, 0.0], [0.0, 1.0]), ([0.5, 0.5, 0.0], [1.0, 0.0]), ([0.5, -0.5, 0.0], [1.0, 1.0])];

fn square() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, SQUARE.map(|(corner, _)| corner).to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, SQUARE.map(|(_, uv)| uv).to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; 4])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 2, 1, 3]))
}

/// The systems that step the effects, and those that show them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum EffectSystems {
    Step,
    Show,
}

/// The frames of the effects that go by in a second.
const FRAMES_PER_SECOND: f32 = 30.0;

pub(crate) fn plugin(app: &mut App, stepped: bool) {
    let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
    shaders.insert(&SHADER, Shader::from_wgsl(include_str!("effects.wgsl"), file!())).expect("the effects' shader has a handle of its own");
    let square = app.world_mut().resource_mut::<Assets<Mesh>>().add(square());
    app.add_plugins(MaterialPlugin::<EffectMaterial>::default())
        .insert_resource(Square(square))
        .init_resource::<Materials>()
        .init_resource::<EffectPool>()
        .init_resource::<EffectReach>()
        .init_resource::<EffectCamera>();
    if stepped {
        app.add_systems(FixedUpdate, (step_frame.in_set(EffectSystems::Step), show.in_set(EffectSystems::Show), reach).chain());
    } else {
        app.add_systems(Update, (step_time.in_set(EffectSystems::Step), show.in_set(EffectSystems::Show), reach).chain());
    }
    app.add_systems(Startup, setup);
}

/// Every effect that is to be seen is drawn, which is when it has its time; those
/// that are over are taken away.
fn frame(commands: &mut Commands, pool: &mut Pool, camera: &Camera, playing: &mut Query<(Entity, &mut Playing)>) {
    pool.particles = playing.iter().map(|(_, playing)| playing.effect.particles()).sum();
    for (entity, mut playing) in playing.iter_mut() {
        if playing.hidden {
            continue;
        }
        playing.frame(pool, camera);
        if !playing.playing() && !playing.stays {
            commands.entity(entity).despawn();
        }
    }
}

/// Where the game is stepped by its frames each of them draws the effects once,
/// and whoever an effect belongs to gives it its time.
fn step_frame(mut commands: Commands, mut pool: ResMut<EffectPool>, camera: Res<EffectCamera>, mut playing: Query<(Entity, &mut Playing)>) {
    frame(&mut commands, &mut pool.0, &camera.0, &mut playing);
}

/// Where nothing steps the game by its frames, the effects' frames go by with the
/// clock, which gives each its time; one that has just been made has its first
/// at once.
fn step_time(mut commands: Commands, time: Res<Time>, mut gone: Local<f32>, mut pool: ResMut<EffectPool>, camera: Res<EffectCamera>, mut playing: Query<(Entity, &mut Playing)>) {
    *gone += time.delta_secs() * FRAMES_PER_SECOND;
    let fresh = playing.iter().any(|(_, playing)| !playing.begun && !playing.hidden);
    let frames = gone.floor().min(4.0) as u32;
    *gone -= gone.floor();
    if frames == 0 && fresh {
        frame(&mut commands, &mut pool.0, &camera.0, &mut playing);
    }
    for _ in 0..frames {
        for (_, mut playing) in playing.iter_mut() {
            if playing.begun && !playing.still {
                playing.give(SIXTIETHS_A_FRAME);
            }
        }
        frame(&mut commands, &mut pool.0, &camera.0, &mut playing);
    }
}

/// What a particle is drawn as: a mesh with a picture, somewhere, in a colour.
struct Instance {
    mesh: Handle<Mesh>,
    picture: Option<Handle<Image>>,
    mixing: Mixing,
    turn: usize,
    matrix: Affine3A,
    colour: [u8; 4],
    /// It is on the screen and not in the world.
    screen: bool,
}

impl Playing {
    /// What there is to draw of it, in the world and on the screen.
    fn instances(&self, square: &Handle<Mesh>) -> Vec<Instance> {
        let mut instances = Vec::new();
        let behind = self.behind.is_some();
        for drawn in self.effect.drawn() {
            let screen = drawn.screen;
            let matrix = Affine3A::from_mat4(drawn.matrix);
            match drawn.body {
                Body::Flat { texture } => {
                    // A picture the file has not got shows nothing (0x126CB8).
                    let picture = match texture {
                        Some(index) => match self.file.pictures.get(index) {
                            Some(picture) => Some(picture.clone()),
                            None => continue,
                        },
                        None => None,
                    };
                    instances.push(Instance { mesh: square.clone(), picture, mixing: Mixing { blend: drawn.blend, plain: false, on_top: self.on_top, behind, screen }, turn: drawn.turn, matrix, colour: drawn.colour, screen });
                }
                Body::Model { model, frames } => {
                    let Some(shape) = self.file.shapes.get(model) else { continue };
                    let placed = shape.posed(frames);
                    for piece in &shape.pieces {
                        let matrix = piece.node.map_or(matrix, |node| matrix * placed[node]);
                        let mixing = Mixing { blend: drawn.blend, plain: piece.plain, on_top: self.on_top, behind, screen };
                        instances.push(Instance { mesh: piece.mesh.clone(), picture: piece.picture.clone(), mixing, turn: drawn.turn, matrix, colour: drawn.colour, screen });
                    }
                }
            }
        }
        instances
    }
}

/// Shows every effect's particles as they are now, using again the entities of
/// the frame before.
fn show(
    mut commands: Commands,
    mut playing: Query<(Entity, &mut Playing)>,
    mut shown: Query<(&mut Mesh3d, &mut MeshMaterial3d<EffectMaterial>, &mut Transform, &mut MeshTag, &mut Visibility, &mut RenderLayers), With<Shown>>,
    square: Res<Square>,
    mut materials: ResMut<Materials>,
    mut effect_materials: ResMut<Assets<EffectMaterial>>,
    mut screen: Query<&mut bevy::camera::Camera, With<ScreenCamera>>,
) {
    let mut on_screen = false;
    for (entity, mut playing) in &mut playing {
        let instances = if playing.hidden || playing.unseen { Vec::new() } else { playing.instances(&square.0) };
        for (index, instance) in instances.iter().enumerate() {
            // Which camera draws it: the scene's own, one of the screen's, or
            // the one that draws the scene's first draw list.
            let layers = match (instance.screen, &playing.behind) {
                (true, Some(_)) => RenderLayers::layer(SCREEN_BEHIND_LAYER),
                (true, None) => RenderLayers::layer(SCREEN_LAYER),
                (false, Some(layers)) => layers.clone(),
                (false, None) => playing.apart.clone().unwrap_or_default(),
            };
            on_screen |= instance.screen && playing.behind.is_none();
            // What is on the screen is before no lens.
            let lens = if instance.screen { Lens::Whole } else { playing.lens };
            let key = (instance.picture.as_ref().map(Handle::id), instance.mixing, instance.turn, lens, playing.order);
            let material = materials
                .0
                .entry(key)
                .or_insert_with(|| {
                    let Mixing { blend, plain, on_top, behind, screen } = instance.mixing;
                    effect_materials.add(EffectMaterial { picture: instance.picture.clone(), blend, plain, turn: instance.turn, on_top, behind, screen, lens, order: playing.order, reach: Vec4::ZERO })
                })
                .clone();
            let transform = transform_of(&instance.matrix);
            let tag = MeshTag(u32::from_le_bytes(instance.colour));
            match playing.shown.get(index).and_then(|&entity| shown.get_mut(entity).ok()) {
                Some((mut mesh, mut with, mut at, mut colour, mut visibility, mut drawn_in)) => {
                    if mesh.0 != instance.mesh {
                        mesh.0 = instance.mesh.clone();
                    }
                    if with.0 != material {
                        with.0 = material;
                    }
                    *at = transform;
                    if colour.0 != tag.0 {
                        *colour = tag;
                    }
                    visibility.set_if_neq(Visibility::Inherited);
                    if *drawn_in != layers {
                        *drawn_in = layers;
                    }
                }
                None => {
                    let made = commands.spawn((Shown, Mesh3d(instance.mesh.clone()), MeshMaterial3d(material), transform, tag, NoFrustumCulling, layers, ChildOf(entity))).id();
                    if index < playing.shown.len() {
                        playing.shown[index] = made;
                    } else {
                        playing.shown.push(made);
                    }
                }
            }
        }
        for &entity in playing.shown.iter().skip(instances.len()) {
            if let Ok((_, _, _, _, mut visibility, _)) = shown.get_mut(entity) {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for mut camera in &mut screen {
        if camera.is_active != on_screen {
            camera.is_active = on_screen;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material(lens: Lens) -> EffectMaterial {
        EffectMaterial { picture: None, blend: Blend::Over, plain: false, turn: 0, on_top: false, behind: false, screen: false, lens, order: 0, reach: Vec4::ZERO }
    }

    /// The field's lenses (0xD9D54): the scene's sees from its near figure to its
    /// far one, the one beside it from just before the camera to the same far
    /// figure; an effect of neither is seen wherever it is.
    #[test]
    fn a_material_is_seen_as_far_and_from_as_near_as_its_lens_sees() {
        let scene = EffectReach { near: 150.0, far: 10_000.0 };
        let mut whole = material(Lens::Whole);
        assert!(!whole.seen_by(scene));
        assert_eq!(whole.reach, Vec4::ZERO);
        let mut of_scene = material(Lens::Scene);
        assert!(of_scene.seen_by(scene));
        assert_eq!(of_scene.reach, Vec4::new(10_000.0, 150.0, 0.0, 0.0));
        assert!(!of_scene.seen_by(scene), "nothing changes the second time");
        let mut close = material(Lens::Close);
        assert!(close.seen_by(scene));
        assert_eq!(close.reach, Vec4::new(10_000.0, 0.0, 0.0, 0.0));
    }
}
