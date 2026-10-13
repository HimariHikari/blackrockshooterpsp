//! The game's screen of 480 by 272, drawn over whatever the cameras show.
//!
//! The original draws its interface as lists of textured, coloured triangles, in the
//! order it wants them over each other, and so does this: what is to be seen is asked
//! for anew every frame ([`Canvas`]) in the screen's own pixels, and a camera of its
//! own draws it, as few meshes as the order allows, scaled to the window with the
//! screen's shape kept.
//!
//! The PSP mixes what it draws with what is there on the values themselves: a
//! panel that is seven parts in ten black leaves three parts in ten of the number
//! each pixel had. A graphics card of today mixes light instead, which leaves such
//! a panel a good deal easier to see through. So the canvas's camera mixes as the
//! PSP does ([`CompositingSpace::Srgb`]), and so that what it draws over is mixed
//! with the same way, every other camera draws to a picture ([`Screen`]) which the
//! canvas's camera lays down first.

use std::collections::HashMap;

use bevy::asset::{uuid_handle, RenderAssetUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{CompositingSpace, RenderTarget, ScalingMode, Viewport};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d, PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin};
use bevy::window::PrimaryWindow;

use crate::offscreen::Offscreen;

/// The size of the original's screen, in its pixels.
pub const WIDTH: f32 = 480.0;
pub const HEIGHT: f32 = 272.0;

/// The canvas's camera draws after those of the scenes.
const CAMERA_ORDER: isize = 10;
/// The layer of what is black beside the game's screen in a window of another
/// shape: over everything.
const BESIDE_LAYER: i32 = i32::MAX;
/// The layer the canvas's meshes and its camera are on, alone: no other camera
/// draws the canvas, and its own draws nothing else.
const RENDER_LAYER: usize = 31;
/// How far apart in depth one batch is drawn from the next, which is what keeps
/// them in their order.
const BATCH_DEPTH: f32 = 0.01;

/// How far behind the canvas the picture of the scene is laid down.
const SCENE_DEPTH: f32 = -1.0;
/// The size the scene's picture is made at, until there is a window to go by.
const FIRST_SIZE: UVec2 = UVec2::new(1280, 720);

const SHADER: Handle<Shader> = uuid_handle!("6f0c1b56-6c0e-4a0f-9a55-3f2a3b6c8e21");
const SCENE_SHADER: Handle<Shader> = uuid_handle!("0d1c8f8e-4f3a-4c59-8a0e-7d5f1c2b9a44");

/// A corner of something drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// In the screen's pixels, from its top left corner.
    pub position: Vec2,
    /// In the texture, 0 to 1 across it and down it.
    pub uv: Vec2,
    /// Red, green, blue and alpha as the game has them: what the texture's colour is
    /// multiplied by, or the colour itself of what has no texture.
    pub colour: [u8; 4],
}

impl Vertex {
    pub fn new(position: Vec2, uv: Vec2, colour: [u8; 4]) -> Vertex {
        Vertex { position, uv, colour }
    }
}

/// What something is drawn with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Paint<'a> {
    pub texture: Option<&'a Handle<Image>>,
    /// It is added to what is behind it rather than laid over it.
    pub additive: bool,
    /// Its texture is as fine as the window, not one of the game's, which are
    /// made for the original's screen: it is drawn from as it is, however many
    /// of its texels go to one of that screen's pixels.
    pub fine: bool,
}

impl<'a> Paint<'a> {
    /// No texture: the corners' colours alone.
    pub const PLAIN: Paint<'static> = Paint { texture: None, additive: false, fine: false };

    pub fn textured(texture: &'a Handle<Image>) -> Paint<'a> {
        Paint { texture: Some(texture), additive: false, fine: false }
    }

    pub fn added(self) -> Paint<'a> {
        Paint { additive: true, ..self }
    }

    pub fn fine(self) -> Paint<'a> {
        Paint { fine: true, ..self }
    }
}

/// What is drawn over the scene this frame. Things are drawn in the order of their
/// layers, and within a layer in the order they were asked for.
#[derive(Resource, Default)]
pub struct Canvas {
    vertices: Vec<Vertex>,
    draws: Vec<Draw>,
    /// What is asked for is cut to this part of the screen.
    cut: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
struct Draw {
    layer: i32,
    texture: Option<Handle<Image>>,
    additive: bool,
    fine: bool,
    /// Its corners among the canvas's, three to a triangle.
    first: usize,
    count: usize,
    /// It is not something of the canvas's but the place of the scene's picture
    /// in the order.
    scene: bool,
}

impl Canvas {
    /// Triangles, three corners each.
    pub fn triangles(&mut self, layer: i32, paint: Paint, corners: &[Vertex]) {
        let whole = corners.len() - corners.len() % 3;
        self.push(layer, paint, corners[..whole].iter().copied());
    }

    /// A strip of triangles: each corner after the second makes one with the two
    /// before it.
    pub fn strip(&mut self, layer: i32, paint: Paint, corners: &[Vertex]) {
        self.push(layer, paint, corners.windows(3).flatten().copied());
    }

    /// A rectangle, from its top left corner to its bottom right, as the PSP draws
    /// a sprite: all of it in the colour of the second corner.
    pub fn sprite(&mut self, layer: i32, paint: Paint, from: Vertex, to: Vertex) {
        let corner = |x: bool, y: bool| Vertex {
            position: Vec2::new(if x { to.position.x } else { from.position.x }, if y { to.position.y } else { from.position.y }),
            uv: Vec2::new(if x { to.uv.x } else { from.uv.x }, if y { to.uv.y } else { from.uv.y }),
            colour: to.colour,
        };
        self.strip(layer, paint, &[corner(false, false), corner(true, false), corner(false, true), corner(true, true)]);
    }

    /// A rectangle of one colour and no texture.
    pub fn fill(&mut self, layer: i32, area: Rect, colour: [u8; 4]) {
        self.sprite(layer, Paint::PLAIN, Vertex::new(area.min, Vec2::ZERO, colour), Vertex::new(area.max, Vec2::ZERO, colour));
    }

    /// Has everything asked for from now on cut to a part of the screen, as the PSP
    /// has what it draws cut to a rectangle of its pixels (0x15B098); or to none.
    pub fn cut_to(&mut self, area: Option<Rect>) {
        self.cut = area;
    }

    /// Has the picture of the scene drawn here in the order, over what was asked
    /// for before it, instead of under everything: what the scenes' cameras left
    /// clear of it lets what is under it through. It is for something of a scene
    /// that belongs among the interface's own things, as a model shown in a menu.
    pub fn scene_here(&mut self, layer: i32) {
        self.draws.push(Draw { layer, texture: None, additive: false, fine: false, first: self.vertices.len(), count: 0, scene: true });
    }

    /// Every corner asked for so far, three to a triangle, in the order they came.
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// The corners asked for so far of what is drawn from a texture, for whatever
    /// looks into what was drawn.
    pub fn drawn_from(&self, texture: &Handle<Image>) -> Vec<Vertex> {
        let of_it = self.draws.iter().filter(|draw| draw.texture.as_ref() == Some(texture));
        of_it.flat_map(|draw| self.vertices[draw.first..draw.first + draw.count].iter().copied()).collect()
    }

    fn push(&mut self, layer: i32, paint: Paint, corners: impl Iterator<Item = Vertex>) {
        let first = self.vertices.len();
        match self.cut {
            None => self.vertices.extend(corners),
            Some(area) => {
                let corners: Vec<Vertex> = corners.collect();
                for triangle in corners.chunks_exact(3) {
                    // What is left of a triangle has more corners than three: it is
                    // drawn as a fan about its first.
                    let left = cut(triangle, area);
                    for pair in left.windows(2).skip(1) {
                        self.vertices.extend([left[0], pair[0], pair[1]]);
                    }
                }
            }
        }
        let count = self.vertices.len() - first;
        if count == 0 {
            return;
        }
        // What follows straight on from the last thing asked for joins it.
        match self.draws.last_mut() {
            Some(last) if !last.scene && last.layer == layer && last.texture.as_ref() == paint.texture && last.additive == paint.additive && last.fine == paint.fine => {
                last.count += count;
            }
            _ => self.draws.push(Draw { layer, texture: paint.texture.cloned(), additive: paint.additive, fine: paint.fine, first, count, scene: false }),
        }
    }

    /// What was asked for, put in the order it is drawn in and into batches of one
    /// texture each, and how many of them come before the picture of the scene if
    /// that has a place among them; the canvas is left empty.
    fn take(&mut self) -> (Vec<Batch>, Option<usize>) {
        self.cut = None;
        let mut draws = std::mem::take(&mut self.draws);
        let vertices = std::mem::take(&mut self.vertices);
        // The sort keeps those of a layer in the order they came.
        draws.sort_by_key(|draw| draw.layer);
        let mut batches: Vec<Batch> = Vec::new();
        let mut scene = None;
        // What comes after the scene's picture does not join what came before it.
        let mut apart = false;
        for draw in draws {
            if draw.scene {
                scene = Some(batches.len());
                apart = true;
                continue;
            }
            let corners = &vertices[draw.first..draw.first + draw.count];
            match batches.last_mut() {
                Some(batch) if !apart && batch.texture == draw.texture && batch.additive == draw.additive && batch.fine == draw.fine => {
                    batch.vertices.extend_from_slice(corners);
                }
                _ => batches.push(Batch { texture: draw.texture, additive: draw.additive, fine: draw.fine, vertices: corners.to_vec() }),
            }
            apart = false;
        }
        (batches, scene)
    }
}

/// What of a triangle is inside a rectangle: its corners in order, with new ones
/// where its sides cross the rectangle's, each of those between the two it lies
/// between in everything.
fn cut(triangle: &[Vertex], area: Rect) -> Vec<Vertex> {
    let between = |a: &Vertex, b: &Vertex, along: f32| Vertex {
        position: a.position.lerp(b.position, along),
        uv: a.uv.lerp(b.uv, along),
        colour: std::array::from_fn(|part| (a.colour[part] as f32 + (b.colour[part] as f32 - a.colour[part] as f32) * along).round() as u8),
    };
    // The four sides: which coordinate, the bound, and whether inside is above it.
    let sides = [(0, area.min.x, true), (0, area.max.x, false), (1, area.min.y, true), (1, area.max.y, false)];
    let mut corners = triangle.to_vec();
    for (axis, bound, above) in sides {
        let inside = |corner: &Vertex| if above { corner.position[axis] >= bound } else { corner.position[axis] <= bound };
        let mut kept = Vec::with_capacity(corners.len() + 1);
        for (index, from) in corners.iter().enumerate() {
            let to = &corners[(index + 1) % corners.len()];
            if inside(from) {
                kept.push(*from);
            }
            if inside(from) != inside(to) {
                let mut crossing = between(from, to, (bound - from.position[axis]) / (to.position[axis] - from.position[axis]));
                // It is on the side itself, whatever the arithmetic made of it.
                crossing.position[axis] = bound;
                kept.push(crossing);
            }
        }
        corners = kept;
        if corners.is_empty() {
            break;
        }
    }
    corners
}

/// What is drawn in one go: triangles of one texture, blended one way.
#[derive(Clone, Debug, PartialEq)]
struct Batch {
    texture: Option<Handle<Image>>,
    additive: bool,
    fine: bool,
    vertices: Vec<Vertex>,
}

/// What a batch is drawn with.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(Blending)]
pub struct CanvasMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Option<Handle<Image>>,
    additive: bool,
    fine: bool,
}

/// The part of a material that chooses how the PSP's blend is set, and how its
/// texture is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Blending {
    additive: bool,
    fine: bool,
}

impl From<&CanvasMaterial> for Blending {
    fn from(material: &CanvasMaterial) -> Blending {
        Blending { additive: material.additive, fine: material.fine }
    }
}

impl Material2d for CanvasMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.additive {
            // The colour by its alpha, added to what is there, which is left whole.
            let blend = BlendState {
                color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
                alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
            };
            for target in descriptor.fragment.iter_mut().flat_map(|fragment| fragment.targets.iter_mut()).flatten() {
                target.blend = Some(blend);
            }
        }
        if key.bind_group_data.fine {
            if let Some(fragment) = &mut descriptor.fragment {
                fragment.shader_defs.push("FINE".into());
            }
        }
        Ok(())
    }
}

/// How strong the glare over the scene's picture is, in parts of one: the game
/// draws the screen a quarter of its size by this and adds that, drawn back large
/// and so made soft, to the screen (its task `Glare`, 0x1A6B4), before anything
/// of the interface is drawn. Whoever draws a scene says; nothing is none.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Glare(pub f32);

/// What the picture of the scene is laid down with.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(Laying)]
pub struct SceneMaterial {
    #[texture(0)]
    #[sampler(1)]
    scene: Handle<Image>,
    /// How strong the glare is, and how large a pixel of the game's screen is in
    /// the picture, across and down, as parts of the picture.
    #[uniform(2)]
    glare: Vec4,
    /// Where the game's screen begins in the picture, as parts of it.
    #[uniform(3)]
    corner: Vec4,
    /// It is laid over what is there, by how much of it the scenes' cameras drew.
    over: bool,
}

/// The part of the scene's material that chooses how it is mixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Laying {
    over: bool,
}

impl From<&SceneMaterial> for Laying {
    fn from(material: &SceneMaterial) -> Laying {
        Laying { over: material.over }
    }
}

impl Material2d for SceneMaterial {
    fn fragment_shader() -> ShaderRef {
        SCENE_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        if self.over { AlphaMode2d::Blend } else { AlphaMode2d::Opaque }
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.over {
            // What a camera drew over nothing is its colour by how much of it is
            // there already.
            for target in descriptor.fragment.iter_mut().flat_map(|fragment| fragment.targets.iter_mut()).flatten() {
                target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            }
        }
        Ok(())
    }
}

/// The two ways the scene's picture is laid down: under everything, and over what
/// was asked for before its place.
#[derive(Resource)]
struct SceneMaterials {
    under: Handle<SceneMaterial>,
    over: Handle<SceneMaterial>,
}

/// The picture every camera but the canvas's draws to.
#[derive(Resource, Clone)]
pub struct Screen {
    pub scene: Handle<Image>,
}

impl Screen {
    /// What to take a picture of for one of the scene alone.
    pub fn alone(&self) -> Screenshot {
        Screenshot::image(self.scene.clone())
    }
}

/// What to take a picture of for one of all that is on the screen.
pub fn all_of_the_screen(offscreen: Option<&Offscreen>) -> Screenshot {
    crate::offscreen::screenshot(offscreen)
}

/// All that the canvas's camera sees, in the screen's pixels: the screen, and what
/// is beside it or above and below it in a window of another shape.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Seen(pub Rect);

impl Default for Seen {
    fn default() -> Seen {
        Seen(Rect::new(0.0, 0.0, WIDTH, HEIGHT))
    }
}

/// A picture of the screen as it was drawn, scene and canvas together, kept to be
/// drawn again: what the original has by copying its screen to a buffer of its own
/// (0x29864), for a frame that stands while something else goes on, or that what
/// comes next shows through as it goes. The graphics card hands a picture over a
/// frame or two after it is asked for, so one that has been asked for is waited
/// for ([`Captured::pending`]).
#[derive(Resource)]
pub struct Captured {
    picture: Handle<Image>,
    /// The frames one has been asked for and has not come.
    waited: Option<u32>,
}

/// The frames a picture is waited for before it is given up: it does not come if
/// another was being taken of the same frame.
const CAPTURE_PATIENCE: u32 = 20;

impl Captured {
    /// Whether one has been asked for and has yet to come.
    pub fn pending(&self) -> bool {
        self.waited.is_some()
    }

    /// Asks for a picture as the screen is next drawn, in place of the one kept:
    /// of all of it ([`all_of_the_screen`]), or of the scene alone, without what
    /// the canvas draws over it ([`Screen::alone`]).
    pub fn ask(&mut self, commands: &mut Commands, of: Screenshot) {
        if self.waited.is_none() {
            self.waited = Some(0);
            commands.spawn(of).observe(keep);
        }
    }

    /// Draws the picture kept over all the canvas's camera sees, by a colour: its
    /// alpha is how much of the picture there is, the rest how bright.
    pub fn draw(&self, canvas: &mut Canvas, seen: &Seen, layer: i32, colour: [u8; 4]) {
        let (from, to) = (Vertex::new(seen.0.min, Vec2::ZERO, colour), Vertex::new(seen.0.max, Vec2::ONE, colour));
        canvas.sprite(layer, Paint::textured(&self.picture).fine(), from, to);
    }
}

/// The picture has come: its numbers are kept as they were put on the screen, to
/// be drawn again as numbers, all of them there.
fn keep(taken: On<ScreenshotCaptured>, mut images: ResMut<Assets<Image>>, mut captured: ResMut<Captured>) {
    captured.waited = None;
    let image = &taken.image;
    let blue_first = match image.texture_descriptor.format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => false,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => true,
        other => {
            warn!("a picture of the screen came as {other:?}, which is not kept");
            return;
        }
    };
    let Some(data) = &image.data else { return };
    let mut pixels = data.clone();
    for pixel in pixels.chunks_exact_mut(4) {
        if blue_first {
            pixel.swap(0, 2);
        }
        pixel[3] = 255;
    }
    let kept = Image::new(image.texture_descriptor.size, TextureDimension::D2, pixels, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    if let Err(error) = images.insert(&captured.picture, kept) {
        warn!("a picture of the screen could not be kept: {error}");
    }
}

/// A picture that was asked for and has not come is given up after a while.
fn give_up(mut captured: ResMut<Captured>) {
    if let Some(waited) = &mut captured.waited {
        *waited += 1;
        if *waited > CAPTURE_PATIENCE {
            warn!("a picture of the screen did not come");
            captured.waited = None;
        }
    }
}

/// The camera that draws the canvas.
#[derive(Component)]
pub struct CanvasCamera;

/// The picture of the scene, as the canvas's camera draws it.
#[derive(Component)]
struct SceneShown;

/// One of the meshes the canvas is drawn as.
#[derive(Component)]
struct Drawn;

/// The materials the batches are drawn with, one for each texture, blend and way
/// of drawing from the texture.
#[derive(Resource, Default)]
struct Materials(HashMap<(Option<AssetId<Image>>, bool, bool), Handle<CanvasMaterial>>);

pub fn plugin(app: &mut App) {
    let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
    shaders.insert(&SHADER, Shader::from_wgsl(include_str!("canvas.wgsl"), file!())).expect("the canvas's shader has a handle of its own");
    shaders.insert(&SCENE_SHADER, Shader::from_wgsl(include_str!("scene.wgsl"), file!())).expect("the scene's shader has a handle of its own");

    // The scene's picture is as big as what is drawn to in the end: the picture of a
    // run with no window, or the window, which is not there yet.
    let world = app.world_mut();
    let picture = world.get_resource::<Offscreen>().map(|offscreen| offscreen.0.clone());
    let mut images = world.resource_mut::<Assets<Image>>();
    let size = picture.and_then(|picture| images.get(&picture).map(Image::size)).unwrap_or(FIRST_SIZE);
    let scene = images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None));
    // Until a picture of the screen has been taken, the one kept is of nothing.
    let nothing = Extent3d { width: 1, height: 1, depth_or_array_layers: 1 };
    let picture = images.add(Image::new(nothing, TextureDimension::D2, vec![0, 0, 0, 255], TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD));

    app.add_plugins((Material2dPlugin::<CanvasMaterial>::default(), Material2dPlugin::<SceneMaterial>::default()))
        .insert_resource(Screen { scene })
        .insert_resource(Captured { picture, waited: None })
        .init_resource::<Seen>()
        .init_resource::<Canvas>()
        .init_resource::<Materials>()
        .add_observer(aim)
        .add_systems(Startup, setup)
        .init_resource::<Glare>()
        .add_systems(First, (fit, give_up))
        .add_systems(PostUpdate, glare.before(draw))
        .add_systems(PostUpdate, draw);
}

fn setup(mut commands: Commands, screen: Res<Screen>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<SceneMaterial>>) {
    commands.spawn((
        CanvasCamera,
        Camera2d,
        CompositingSpace::Srgb,
        RenderLayers::layer(RENDER_LAYER),
        Camera { order: CAMERA_ORDER, ..default() },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin { min_width: WIDTH, min_height: HEIGHT },
            ..OrthographicProjection::default_2d()
        }),
    ));
    let scene = SceneMaterials {
        under: materials.add(SceneMaterial { scene: screen.scene.clone(), glare: Vec4::ZERO, corner: Vec4::ZERO, over: false }),
        over: materials.add(SceneMaterial { scene: screen.scene.clone(), glare: Vec4::ZERO, corner: Vec4::ZERO, over: true }),
    };
    commands.spawn((
        SceneShown,
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial2d(scene.under.clone()),
        Transform::from_xyz(0.0, 0.0, SCENE_DEPTH).with_scale(Vec3::new(WIDTH, HEIGHT, 1.0)),
        NoFrustumCulling,
        RenderLayers::layer(RENDER_LAYER),
    ));
    commands.insert_resource(scene);
}

/// Every camera but the canvas's draws to the scene's picture; the canvas's draws to
/// the window, or to the picture a run with no window has in its stead.
fn aim(added: On<Add, Camera>, screen: Res<Screen>, offscreen: Option<Res<Offscreen>>, canvas: Query<(), With<CanvasCamera>>, mut commands: Commands) {
    let mut camera = commands.entity(added.entity);
    match (canvas.contains(added.entity), offscreen) {
        (false, _) => camera.insert(RenderTarget::from(screen.scene.clone())),
        (true, Some(offscreen)) => camera.insert(RenderTarget::from(offscreen.0.clone())),
        (true, None) => &mut camera,
    };
}

/// Keeps the scene's picture the size of the window, and what shows it the size of
/// all the canvas's camera sees: the screen, and what is beside it or above and
/// below it in a window of another shape. The scenes' cameras draw to the part of
/// the picture that is the game's screen, so that they see what the PSP's screen
/// showed whatever shape the window has.
#[allow(clippy::too_many_arguments)]
fn fit(
    windows: Query<&Window, With<PrimaryWindow>>,
    screen: Res<Screen>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Query<&mut Transform, With<SceneShown>>,
    mut all: ResMut<Seen>,
    scene_materials: Option<Res<SceneMaterials>>,
    mut materials: ResMut<Assets<SceneMaterial>>,
    mut cameras: Query<(&mut Camera, &RenderTarget), Without<CanvasCamera>>,
) {
    if let Ok(window) = windows.single() {
        let size = Extent3d { width: window.physical_width().max(1), height: window.physical_height().max(1), depth_or_array_layers: 1 };
        if images.get(&screen.scene).is_some_and(|scene| scene.texture_descriptor.size != size) {
            if let Some(mut scene) = images.get_mut(&screen.scene) {
                scene.resize(size);
            }
            // A picture of another size is another picture to the graphics card, and
            // what shows it would go on showing the one it was made with (all black,
            // if the window changed while nothing was being drawn to it, as on the
            // title): it is given the picture again, which has it made anew.
            if let Some(scene_materials) = &scene_materials {
                for shows in [&scene_materials.under, &scene_materials.over] {
                    if let Some(mut material) = materials.get_mut(shows) {
                        material.scene = screen.scene.clone();
                    }
                }
            }
        }
    }
    let Some(size) = images.get(&screen.scene).map(|scene| scene.size().as_vec2()) else { return };
    let seen = size / (size.x / WIDTH).min(size.y / HEIGHT);
    let around = Rect::from_center_size(Vec2::new(WIDTH, HEIGHT) / 2.0, seen);
    if all.0 != around {
        all.0 = around;
    }
    for mut transform in &mut shown {
        if transform.scale.truncate() != seen {
            transform.scale = seen.extend(1.0);
        }
    }
    // Every pixel the screen has any of: what is black beside the screen covers
    // the rest, and the part of an edge's pixel that is beside it.
    let whole = size.as_uvec2();
    let its = Vec2::new(WIDTH, HEIGHT) * (size.x / WIDTH).min(size.y / HEIGHT);
    let from = ((size - its) / 2.0).floor().max(Vec2::ZERO).as_uvec2().min(whole - UVec2::ONE);
    let part = (((size + its) / 2.0).ceil().as_uvec2().min(whole) - from).max(UVec2::ONE);
    for (mut camera, target) in &mut cameras {
        if target.as_image() != Some(&screen.scene) {
            continue;
        }
        if camera.viewport.as_ref().is_none_or(|viewport| viewport.physical_position != from || viewport.physical_size != part) {
            camera.viewport = Some(Viewport { physical_position: from, physical_size: part, ..default() });
        }
    }
}

/// What shows the scene's picture is told how strong the glare is, and where the
/// game's screen lies in the picture, when either has changed.
fn glare(glare: Res<Glare>, all: Res<Seen>, scene_materials: Res<SceneMaterials>, mut materials: ResMut<Assets<SceneMaterial>>) {
    let seen = all.0;
    if seen.width() <= 0.0 || seen.height() <= 0.0 {
        return;
    }
    let pixel = Vec2::ONE / seen.size();
    let told = Vec4::new(glare.0.clamp(0.0, 1.0), pixel.x, pixel.y, 0.0);
    let corner = (-seen.min * pixel).extend(0.0).extend(0.0);
    for shows in [&scene_materials.under, &scene_materials.over] {
        if materials.get(shows).is_some_and(|material| material.glare != told || material.corner != corner) {
            if let Some(mut material) = materials.get_mut(shows) {
                (material.glare, material.corner) = (told, corner);
            }
        }
    }
}

/// Makes the frame's batches into meshes, using again those of the frame before.
fn draw(
    mut commands: Commands,
    mut canvas: ResMut<Canvas>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut canvas_materials: ResMut<Assets<CanvasMaterial>>,
    mut materials: ResMut<Materials>,
    mut pool: Local<Vec<Entity>>,
    mut drawn: Query<(&Mesh2d, &mut MeshMaterial2d<CanvasMaterial>, &mut Visibility), With<Drawn>>,
    scene_materials: Res<SceneMaterials>,
    mut scene: Query<(&mut Transform, &mut MeshMaterial2d<SceneMaterial>), With<SceneShown>>,
    seen: Res<Seen>,
) {
    // Beside the game's screen, and above and below it, there is nothing: black
    // over whatever was drawn there.
    let (all, screen) = (seen.0, Rect::new(0.0, 0.0, WIDTH, HEIGHT));
    for beside in [
        Rect::new(all.min.x, all.min.y, screen.min.x, all.max.y),
        Rect::new(screen.max.x, all.min.y, all.max.x, all.max.y),
        Rect::new(screen.min.x, all.min.y, screen.max.x, screen.min.y),
        Rect::new(screen.min.x, screen.max.y, screen.max.x, all.max.y),
    ] {
        if beside.width() > 0.0 && beside.height() > 0.0 {
            canvas.fill(BESIDE_LAYER, beside, [0, 0, 0, 255]);
        }
    }
    let (batches, scene_place) = canvas.take();
    // The scene's picture: under everything, or between the batch before its place
    // and the one after.
    let (depth, material) = match scene_place {
        Some(before) => ((before as f32 - 0.5) * BATCH_DEPTH, &scene_materials.over),
        None => (SCENE_DEPTH, &scene_materials.under),
    };
    for (mut transform, mut shown) in &mut scene {
        if transform.translation.z != depth {
            transform.translation.z = depth;
        }
        if shown.0 != *material {
            shown.0 = material.clone();
        }
    }
    for (index, batch) in batches.iter().enumerate() {
        let key = (batch.texture.as_ref().map(Handle::id), batch.additive, batch.fine);
        let material = materials
            .0
            .entry(key)
            .or_insert_with(|| canvas_materials.add(CanvasMaterial { texture: batch.texture.clone(), additive: batch.additive, fine: batch.fine }))
            .clone();
        match pool.get(index).and_then(|&entity| drawn.get_mut(entity).ok()) {
            Some((mesh, mut shown, mut visibility)) => {
                if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
                    fill(&mut mesh, &batch.vertices);
                }
                if shown.0 != material {
                    shown.0 = material;
                }
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                fill(&mut mesh, &batch.vertices);
                let depth = Transform::from_xyz(0.0, 0.0, index as f32 * BATCH_DEPTH);
                let layer = RenderLayers::layer(RENDER_LAYER);
                let entity = commands.spawn((Drawn, Mesh2d(meshes.add(mesh)), MeshMaterial2d(material), depth, NoFrustumCulling, layer)).id();
                pool.push(entity);
            }
        }
    }
    for &entity in pool.iter().skip(batches.len()) {
        if let Ok((_, _, mut visibility)) = drawn.get_mut(entity) {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Puts a batch's corners into a mesh, the screen's top left corner at the top left
/// of what the canvas's camera sees.
fn fill(mesh: &mut Mesh, vertices: &[Vertex]) {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|vertex| [vertex.position.x - WIDTH / 2.0, HEIGHT / 2.0 - vertex.position.y, 0.0]).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|vertex| vertex.uv.to_array()).collect();
    let colours: Vec<[f32; 4]> = vertices.iter().map(|vertex| parts(vertex.colour)).collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
}

/// A colour of the game's as the meshes have theirs: its values as they are, each
/// as a part of one. They stay the game's values all the way to the screen: between
/// corners they are mixed as the PSP mixes them.
fn parts(colour: [u8; 4]) -> [f32; 4] {
    colour.map(|part| part as f32 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [u8; 4] = [255; 4];

    fn at(x: f32, y: f32) -> Vertex {
        Vertex::new(Vec2::new(x, y), Vec2::ZERO, WHITE)
    }

    #[test]
    fn things_are_drawn_by_layer_and_then_in_the_order_asked_for() {
        let texture = Handle::<Image>::default();
        let textured = Paint::textured(&texture);
        let mut canvas = Canvas::default();
        canvas.triangles(2, Paint::PLAIN, &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)]);
        canvas.triangles(1, textured, &[at(2.0, 0.0), at(3.0, 0.0), at(2.0, 1.0)]);
        canvas.triangles(2, Paint::PLAIN, &[at(4.0, 0.0), at(5.0, 0.0), at(4.0, 1.0)]);
        canvas.triangles(1, textured, &[at(6.0, 0.0), at(7.0, 0.0), at(6.0, 1.0)]);
        // What is added to what is behind it is a batch of its own.
        canvas.triangles(2, Paint::PLAIN.added(), &[at(8.0, 0.0), at(9.0, 0.0), at(8.0, 1.0)]);
        let (batches, scene) = canvas.take();
        assert_eq!(scene, None);
        // The textured ones of layer 1, then the plain ones of layer 2.
        let firsts = |batch: &Batch| batch.vertices.iter().step_by(3).map(|vertex| vertex.position.x).collect::<Vec<f32>>();
        assert_eq!(batches.len(), 3);
        assert_eq!((batches[0].texture.is_some(), firsts(&batches[0])), (true, vec![2.0, 6.0]));
        assert_eq!((batches[1].texture.is_some(), batches[1].additive, firsts(&batches[1])), (false, false, vec![0.0, 4.0]));
        assert_eq!((batches[2].additive, firsts(&batches[2])), (true, vec![8.0]));
        // The canvas starts each frame empty.
        assert!(canvas.take().0.is_empty());
    }

    #[test]
    fn the_scenes_picture_can_have_a_place_in_the_order() {
        let mut canvas = Canvas::default();
        let triangle = |x: f32| [at(x, 0.0), at(x + 1.0, 0.0), at(x, 1.0)];
        canvas.triangles(1, Paint::PLAIN, &triangle(0.0));
        canvas.triangles(3, Paint::PLAIN, &triangle(6.0));
        canvas.triangles(2, Paint::PLAIN, &triangle(2.0));
        canvas.scene_here(2);
        canvas.triangles(2, Paint::PLAIN, &triangle(4.0));
        // What was asked for before it, in its layer and in those under it, is one
        // batch; what comes after is another, though it is drawn the same way.
        let (batches, scene) = canvas.take();
        let firsts = |batch: &Batch| batch.vertices.iter().step_by(3).map(|vertex| vertex.position.x).collect::<Vec<f32>>();
        assert_eq!(scene, Some(1));
        assert_eq!(batches.iter().map(firsts).collect::<Vec<_>>(), [vec![0.0, 2.0], vec![4.0, 6.0]]);
        // With nothing asked for before it, it is under everything all the same.
        canvas.scene_here(0);
        canvas.triangles(0, Paint::PLAIN, &triangle(0.0));
        let (batches, scene) = canvas.take();
        assert_eq!((batches.len(), scene), (1, Some(0)));
        assert_eq!(canvas.take(), (Vec::new(), None));
    }

    #[test]
    fn a_strip_and_a_sprite_are_made_into_triangles() {
        let mut canvas = Canvas::default();
        canvas.strip(0, Paint::PLAIN, &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0), at(1.0, 1.0), at(0.0, 2.0)]);
        // Fewer than three corners make nothing, and an odd one over is left out.
        canvas.strip(0, Paint::PLAIN, &[at(0.0, 0.0), at(1.0, 0.0)]);
        canvas.triangles(0, Paint::PLAIN, &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0), at(9.0, 9.0)]);
        let red = [255, 0, 0, 128];
        let (from, to) = (Vertex::new(Vec2::new(10.0, 20.0), Vec2::new(0.25, 0.5), WHITE), Vertex::new(Vec2::new(30.0, 40.0), Vec2::new(0.75, 1.0), red));
        canvas.sprite(0, Paint::PLAIN, from, to);
        let (batches, _) = canvas.take();
        assert_eq!(batches.len(), 1);
        let vertices = &batches[0].vertices;
        assert_eq!(vertices.len(), 9 + 3 + 6);
        let sprite: Vec<(f32, f32, f32, f32)> = vertices[12..].iter().map(|v| (v.position.x, v.position.y, v.uv.x, v.uv.y)).collect();
        let (top_left, top_right, bottom_left, bottom_right) =
            ((10.0, 20.0, 0.25, 0.5), (30.0, 20.0, 0.75, 0.5), (10.0, 40.0, 0.25, 1.0), (30.0, 40.0, 0.75, 1.0));
        assert_eq!(sprite, [top_left, top_right, bottom_left, top_right, bottom_left, bottom_right]);
        // All of a sprite is the colour of its second corner.
        assert!(vertices[12..].iter().all(|vertex| vertex.colour == red));
    }

    #[test]
    fn what_is_drawn_can_be_cut_to_a_rectangle() {
        let mut canvas = Canvas::default();
        canvas.cut_to(Some(Rect::new(10.0, 10.0, 20.0, 30.0)));
        // A rectangle that is half in: its right half is left, with what it is
        // drawn from and its colour as they are half way.
        let (from, to) = (Vertex::new(Vec2::new(0.0, 10.0), Vec2::new(0.0, 0.0), WHITE), Vertex::new(Vec2::new(20.0, 30.0), Vec2::new(1.0, 1.0), WHITE));
        canvas.sprite(0, Paint::PLAIN, from, to);
        // One that is all out leaves nothing, and one that is all in is whole.
        canvas.fill(0, Rect::new(40.0, 0.0, 50.0, 5.0), WHITE);
        canvas.fill(0, Rect::new(12.0, 12.0, 14.0, 14.0), WHITE);
        canvas.cut_to(None);
        canvas.fill(0, Rect::new(40.0, 0.0, 50.0, 5.0), WHITE);
        let vertices = canvas.vertices();
        let cut = &vertices[..vertices.len() - 12];
        assert!(cut.len() >= 6 && cut.len() % 3 == 0);
        let (left, right) = (cut.iter().map(|v| v.position.x).fold(f32::MAX, f32::min), cut.iter().map(|v| v.position.x).fold(f32::MIN, f32::max));
        assert_eq!((left, right), (10.0, 20.0));
        assert!(cut.iter().all(|vertex| vertex.uv.x >= 0.5 && (vertex.position.x - 10.0) / 20.0 + 0.5 == vertex.uv.x));
        // Its area is that of the half.
        let area: f32 = cut.chunks_exact(3).map(|t| ((t[1].position - t[0].position).perp_dot(t[2].position - t[0].position) / 2.0).abs()).sum();
        assert_eq!(area, 200.0);
        let rest = &vertices[vertices.len() - 12..];
        assert_eq!((rest[0].position, rest[6].position), (Vec2::new(12.0, 12.0), Vec2::new(40.0, 0.0)));
    }

    #[test]
    fn the_screens_corners_are_the_corners_of_what_the_camera_sees() {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        fill(&mut mesh, &[at(0.0, 0.0), at(WIDTH, HEIGHT), Vertex::new(Vec2::new(240.0, 136.0), Vec2::ONE, [255, 255, 255, 0])]);
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|values| values.as_float3()).unwrap();
        assert_eq!(positions, [[-240.0, 136.0, 0.0], [240.0, -136.0, 0.0], [0.0, 0.0, 0.0]]);
        assert_eq!(parts([255, 255, 255, 0]), [1.0, 1.0, 1.0, 0.0]);
        assert_eq!(parts([51, 0, 0, 255])[0], 0.2);
    }
}
