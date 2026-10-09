//! Models that take a light, as the PSP gave them one.
//!
//! The field lights each character as it draws it (0x10F3B4): the PSP's own
//! lighting, with one light that shines from a way and the light that is all
//! about the scene. What a corner's colour comes to is in `lit.wgsl`; what a
//! material makes of the light is in its own display list ([`Look`]), which
//! also says how it is put on what is behind it ([`Blend`]).
//!
//! A model is drawn without light until its root is given a [`Lit`]: then every
//! piece under it, whenever it comes there (a weapon in a hand, a thing an event
//! gives it to hold), gets a material of that root's own in place of the one it
//! was built with, and the root's light goes to all of them. A piece under no
//! such root whose picture is added to its colour, or taken twice over, gets a
//! material that does so with no light, one for all that were built with the
//! same.

use std::collections::HashMap;

use bevy::asset::uuid_handle;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, MaterialPlugin, MeshPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;

const SHADER: Handle<Shader> = uuid_handle!("7c1e5a2b-93d4-4f0e-8a6b-2d5f1c9e4b70");

// The commands of a material's display list that say how it takes light and how
// it is put on the screen.
const GE_TEXTURE_ON: u8 = 0x1e;
const GE_COLOUR_MATERIAL: u8 = 0x53;
const GE_SPECULAR_POWER: u8 = 0x5b;
const GE_TEXTURE_FUNCTION: u8 = 0xc9;
const GE_BLEND: u8 = 0xdf;
const GE_DEPTH_WRITE_OFF: u8 = 0xe7;
/// The texture function that adds the picture to what the light gives.
const FUNCTION_ADD: u8 = 4;
// What command 0xDF takes something by: how much of what is drawn there is, the
// rest of that, and a fixed colour.
const BY_ALPHA: u32 = 2;
const BY_THE_REST: u32 = 3;
const BY_FIXED: u32 = 10;

pub type LitMaterial = ExtendedMaterial<StandardMaterial, LitExtension>;

/// How a material is put on what is behind it. The game has the PSP mix every
/// model it draws (0x10F3E8 for the field's characters, 0xAB914 for a scene),
/// and a material's list says by what (command 0xDF, with the fixed colours of
/// 0xE0 and 0xE1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    /// All of it and nothing of what is behind (0xAA: by the fixed colours,
    /// which are white and black): 413 of the 1027 materials of the game's
    /// models, which also leave out what is under half there (command 0xDB).
    Solid,
    /// By how much of it there is (0x32): 597, which leave out only what is
    /// not there at all. It is what the field has before a model's own
    /// materials say (0x10F3F0).
    #[default]
    Through,
    /// Added to what is behind, by how much of it there is (0xA2, the second
    /// fixed colour white): 17, lights and glows, none of which writes how far
    /// it is.
    Added,
}

/// What a material makes of the light, as its display list sets the PSP.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The power its shine is raised to (command 0x5B): 50 for most of the field's
    /// materials, 10 for the rest.
    pub shine: f32,
    /// Which of its colours are its corners' own (command 0x53): a bit each for
    /// what it gives back of the light all about, of the light's own colour and of
    /// its shine. The others are white.
    pub corners: u8,
    /// How its picture is put on what the light gives (command 0xC9): 0 by it, 4
    /// added to it; and whether that is taken twice over. One whose picture is
    /// off (command 0x1E: 39 materials, the arrows that show the way among them)
    /// has neither.
    pub function: u8,
    pub doubled: bool,
    /// Whether it has a picture at all.
    pub pictured: bool,
    /// How it is put on what is behind it.
    pub blend: Blend,
    /// Whether it writes how far it is (command 0xE7 says when not: 23).
    pub depth_write: bool,
}

impl Default for Look {
    /// What a material that says nothing is drawn with: the frame's power (0xAB7F8)
    /// and what a model's draw sets before its materials (0xA60F8).
    fn default() -> Look {
        Look { shine: 10.0, corners: 1, function: 0, doubled: false, pictured: true, blend: Blend::Through, depth_write: true }
    }
}

impl Look {
    /// Reads it from a material's display list, as `(command, argument)` pairs.
    pub fn of(state: &[(u8, u32)]) -> Look {
        let mut look = Look::default();
        for &(command, argument) in state {
            match command {
                GE_COLOUR_MATERIAL => look.corners = (argument & 7) as u8,
                // The top three bytes of a float.
                GE_SPECULAR_POWER => look.shine = f32::from_bits(argument << 8),
                GE_TEXTURE_FUNCTION => (look.function, look.doubled) = ((argument & 7) as u8, argument & 0x1_0000 != 0),
                GE_TEXTURE_ON => look.pictured = argument & 1 != 0,
                GE_BLEND => {
                    look.blend = match (argument & 0xf, argument >> 4 & 0xf) {
                        (BY_ALPHA, BY_THE_REST) => Blend::Through,
                        (BY_ALPHA, BY_FIXED) => Blend::Added,
                        _ => Blend::Solid,
                    }
                }
                GE_DEPTH_WRITE_OFF => look.depth_write = argument & 1 == 0,
                _ => {}
            }
        }
        // The PSP puts no picture on what has none, by any function.
        if !look.pictured {
            (look.function, look.doubled) = (0, false);
        }
        look
    }

    /// How the port has it drawn. `partly` is whether any of it is partly there:
    /// a corner of its pieces or a pixel of its picture. One that is mixed and
    /// has nothing to mix is drawn as a solid one is, cut at a half, and so
    /// stays among what is drawn first; what is mixed is drawn after all that,
    /// by how far it is, where the PSP draws a model's pieces in their order.
    pub fn alpha_mode(&self, partly: bool) -> AlphaMode {
        match self.blend {
            Blend::Added => AlphaMode::Add,
            Blend::Through if partly || !self.depth_write => AlphaMode::Blend,
            _ => AlphaMode::Mask(0.5),
        }
    }

    /// Whether a plain material would not do for one built `drawn` so: its
    /// picture is added or taken twice over, or it is mixed and yet writes how
    /// far it is.
    fn wants_its_own(&self, drawn: AlphaMode) -> bool {
        self.function == FUNCTION_ADD || self.doubled || self.depth_write && drawn == AlphaMode::Blend
    }
}

/// The looks of the materials models were built with, for when one is to be lit;
/// and with each the material that puts its picture on its own way with no light,
/// once a piece has wanted it.
#[derive(Resource, Default)]
pub struct Looks(HashMap<AssetId<StandardMaterial>, (Look, Option<Handle<LitMaterial>>)>);

impl Looks {
    pub fn set(&mut self, material: &Handle<StandardMaterial>, look: Look) {
        self.0.insert(material.id(), (look, None));
    }
}

/// The light on a model, on its root: the colours are the game's own numbers.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(Dressed)]
pub struct Lit {
    /// Whether it is lit at all: one that is not is drawn by its corners' colours.
    pub on: bool,
    /// The light all about the scene and the light's own share of that, summed.
    pub ambient: Vec3,
    /// The light's own colour, and the colour of its shine.
    pub diffuse: Vec3,
    pub specular: Vec3,
    /// The way to the light, in the world.
    pub towards: Vec3,
    /// How much of it there is: the alpha the PSP is given with the light all
    /// about (0x10F608, the character's own, one unless it is fading). It
    /// counts only for one that is lit.
    pub alpha: f32,
    /// How far before the camera the lens it is drawn by sees, and from how
    /// near: nothing of it is drawn beyond the first (nought is no end) or
    /// nearer than the second.
    pub reach: Vec2,
}

impl Default for Lit {
    fn default() -> Lit {
        Lit { on: false, ambient: Vec3::ONE, diffuse: Vec3::ZERO, specular: Vec3::ZERO, towards: Vec3::Z, alpha: 1.0, reach: Vec2::ZERO }
    }
}

/// A material a lit root has had made for its pieces: the one they were built
/// with, its own, and what the first says of how it is drawn.
struct Own {
    built_with: AssetId<StandardMaterial>,
    own: Handle<LitMaterial>,
    look: Look,
    drawn: AlphaMode,
}

/// The materials a lit root has had made for its pieces.
#[derive(Component, Default)]
pub struct Dressed(Vec<Own>);

/// How a material built to be drawn `built` is drawn under `light`: one that is
/// mixed and had nothing to mix has, once there is less of its model than all.
fn drawn_under(light: &Lit, look: &Look, built: AlphaMode) -> AlphaMode {
    if light.on && light.alpha < 1.0 && look.blend == Blend::Through {
        AlphaMode::Blend
    } else {
        built
    }
}

/// The part of a material that chooses how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Drawn {
    depth_write: bool,
}

impl From<&LitExtension> for Drawn {
    fn from(extension: &LitExtension) -> Drawn {
        Drawn { depth_write: extension.depth_write }
    }
}

/// What the shader is given of a light and of a material's look.
#[derive(Asset, AsBindGroup, Reflect, Clone, Debug, Default)]
#[bind_group_data(Drawn)]
pub struct LitExtension {
    #[uniform(100)]
    ambient: Vec4,
    #[uniform(100)]
    diffuse: Vec4,
    #[uniform(100)]
    specular: Vec4,
    #[uniform(100)]
    towards: Vec4,
    #[uniform(100)]
    look: Vec4,
    #[uniform(100)]
    reach: Vec4,
    /// Whether it writes how far it is where it is mixed.
    depth_write: bool,
}

impl LitExtension {
    fn new(light: &Lit, look: Look) -> LitExtension {
        let mut extension = LitExtension {
            specular: Vec4::new(0.0, 0.0, 0.0, look.shine),
            look: Vec4::new(f32::from(look.corners), f32::from(look.function), if look.doubled { 1.0 } else { 0.0 }, 0.0),
            depth_write: look.depth_write,
            ..default()
        };
        extension.set(light);
        extension
    }

    fn set(&mut self, light: &Lit) {
        self.ambient = light.ambient.extend(light.alpha.clamp(0.0, 1.0));
        self.diffuse = light.diffuse.extend(1.0);
        self.specular = light.specular.extend(self.specular.w);
        self.towards = light.towards.normalize_or(Vec3::Z).extend(if light.on { 1.0 } else { 0.0 });
        self.reach = light.reach.extend(0.0).extend(0.0);
    }

    /// Whether its picture is added to what the light gives.
    pub fn adds(&self) -> bool {
        self.look.y == f32::from(FUNCTION_ADD)
    }
}

impl MaterialExtension for LitExtension {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// What is mixed by how much of it there is still hides what is behind it
    /// where its list has it write how far it is (all but 23 do): left to
    /// itself bevy writes no depth for anything it mixes.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let mixed = key.mesh_key.intersection(MeshPipelineKey::BLEND_RESERVED_BITS) == MeshPipelineKey::BLEND_ALPHA;
        if let Some(depth) = descriptor.depth_stencil.as_mut().filter(|_| mixed && key.bind_group_data.depth_write) {
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}

/// Every piece that has come under a lit root gets a material of that root's own.
fn dress(
    mut commands: Commands,
    mut looks: ResMut<Looks>,
    standard: Res<Assets<StandardMaterial>>,
    mut lit: ResMut<Assets<LitMaterial>>,
    fresh: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    parents: Query<&ChildOf>,
    mut roots: Query<(&Lit, &mut Dressed)>,
) {
    for (entity, material) in &fresh {
        let mut at = entity;
        let root = loop {
            if roots.contains(at) {
                break Some(at);
            }
            match parents.get(at) {
                Ok(parent) => at = parent.parent(),
                Err(_) => break None,
            }
        };
        let built_with = material.id();
        let Some((light, mut dressed)) = root.and_then(|root| roots.get_mut(root).ok()) else {
            // Under no light: only what a plain material has not is to be had.
            let Some((look, plain)) = looks.0.get_mut(&built_with) else { continue };
            if plain.is_none() {
                let Some(base) = standard.get(built_with).filter(|base| look.wants_its_own(base.alpha_mode)) else { continue };
                *plain = Some(lit.add(LitMaterial { base: base.clone(), extension: LitExtension::new(&Lit::default(), *look) }));
            }
            let Some(plain) = plain.clone() else { continue };
            commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(plain));
            continue;
        };
        let own = match dressed.0.iter().find(|own| own.built_with == built_with) {
            Some(own) => own.own.clone(),
            None => {
                let Some(base) = standard.get(built_with) else { continue };
                let look = looks.0.get(&built_with).map(|(look, _)| *look).unwrap_or_default();
                let drawn = base.alpha_mode;
                let base = StandardMaterial { alpha_mode: drawn_under(light, &look, drawn), ..base.clone() };
                let own = lit.add(LitMaterial { base, extension: LitExtension::new(light, look) });
                dressed.0.push(Own { built_with, own: own.clone(), look, drawn });
                own
            }
        };
        commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(own));
    }
}

/// A root's light goes to its materials when it has changed.
fn light(mut lit: ResMut<Assets<LitMaterial>>, roots: Query<(&Lit, &Dressed), Changed<Lit>>) {
    for (light, dressed) in &roots {
        for own in &dressed.0 {
            if let Some(mut material) = lit.get_mut(&own.own) {
                material.extension.set(light);
                material.base.alpha_mode = drawn_under(light, &own.look, own.drawn);
            }
        }
    }
}

/// The looks of materials that are gone are forgotten.
fn forget(mut looks: ResMut<Looks>, mut events: MessageReader<AssetEvent<StandardMaterial>>) {
    for event in events.read() {
        if let AssetEvent::Removed { id } = event {
            looks.0.remove(id);
        }
    }
}

pub(crate) fn plugin(app: &mut App) {
    let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
    shaders.insert(&SHADER, Shader::from_wgsl(include_str!("lit.wgsl"), file!())).expect("the lit models' shader has a handle of its own");
    app.add_plugins(MaterialPlugin::<LitMaterial>::default()).init_resource::<Looks>().add_systems(PostUpdate, (forget, light, dress).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_look_is_read_from_a_material_s_list() {
        // The field's commonest: its corners' colour for what is all about and for
        // the light's own, a power of fifty, its picture by the light.
        let look = Look::of(&[(0x1e, 1), (0x53, 3), (0x5b, 0x424800), (0xc7, 0), (0xc9, 0x100), (0xdb, 0xff0006)]);
        assert_eq!(look, Look { shine: 50.0, corners: 3, ..Look::default() });
        // One of plain colours, a power of ten, its picture twice over; and one
        // whose picture is added.
        assert_eq!(Look::of(&[(0x53, 0), (0x5b, 0x412000), (0xc9, 0x10100)]), Look { shine: 10.0, corners: 0, doubled: true, ..Look::default() });
        assert_eq!(Look::of(&[(0xc9, 0x104)]), Look { function: 4, ..Look::default() });
        assert_eq!(Look::of(&[]), Look::default());
    }

    #[test]
    fn a_look_says_how_a_material_is_put_on_what_is_behind_it() {
        // The arrow that shows the way: no picture, so nothing is added to its
        // corners' colours whatever its function; mixed by how much of it there is.
        let arrow = Look::of(&[(0x1e, 0), (0xc9, 0x104), (0xdf, 0x32), (0xdb, 0xff0006), (0xe7, 0), (0x53, 3), (0x5b, 0x424800)]);
        assert_eq!(arrow, Look { shine: 50.0, corners: 3, pictured: false, ..Look::default() });
        // Drawn solid a plain material does for it; mixed, it needs its own to
        // go on writing how far it is.
        assert!(!arrow.wants_its_own(AlphaMode::Mask(0.5)) && arrow.wants_its_own(AlphaMode::Blend));
        // With nothing of it partly there it is drawn as a solid one; with a
        // corner or a pixel that is, it is mixed.
        assert_eq!(arrow.alpha_mode(false), AlphaMode::Mask(0.5));
        assert_eq!(arrow.alpha_mode(true), AlphaMode::Blend);
        // A solid one, by the fixed colours; a glow, added and writing no depth;
        // and one that is mixed and writes none.
        let solid = Look::of(&[(0x1e, 1), (0xc9, 0x100), (0xdf, 0xaa), (0xe0, 0xffffff), (0xe1, 0), (0xdb, 0xff8007), (0xe7, 0)]);
        assert_eq!((solid.blend, solid.alpha_mode(true)), (Blend::Solid, AlphaMode::Mask(0.5)));
        let glow = Look::of(&[(0x1e, 1), (0xc9, 0x10100), (0xdf, 0xa2), (0xe1, 0xffffff), (0xe7, 1)]);
        assert_eq!((glow.blend, glow.depth_write, glow.alpha_mode(false)), (Blend::Added, false, AlphaMode::Add));
        assert!(glow.wants_its_own(AlphaMode::Add));
        let veil = Look::of(&[(0x1e, 1), (0xdf, 0x32), (0xe7, 1)]);
        assert_eq!(veil.alpha_mode(false), AlphaMode::Blend);
        assert!(!veil.wants_its_own(AlphaMode::Blend));
    }

    #[test]
    fn what_the_shader_is_given_follows_the_light() {
        let mut extension = LitExtension::new(&Lit::default(), Look { shine: 50.0, corners: 3, function: 4, doubled: true, ..Look::default() });
        assert_eq!((extension.towards.w, extension.specular.w, extension.look), (0.0, 50.0, Vec4::new(3.0, 4.0, 1.0, 0.0)));
        assert!(extension.adds());
        extension.set(&Lit { on: true, ambient: Vec3::new(0.95, 0.85, 1.2), diffuse: Vec3::new(0.25, 0.35, 0.0), specular: Vec3::ONE, towards: Vec3::new(0.0, 3.0, 4.0), alpha: 1.0, reach: Vec2::new(20_000.0, 50.0) });
        assert_eq!(extension.reach, Vec4::new(20_000.0, 50.0, 0.0, 0.0));
        assert_eq!(extension.ambient, Vec4::new(0.95, 0.85, 1.2, 1.0));
        assert_eq!(extension.specular, Vec4::new(1.0, 1.0, 1.0, 50.0));
        assert!((extension.towards - Vec4::new(0.0, 0.6, 0.8, 1.0)).length() < 1e-6);
    }

    #[test]
    fn a_model_that_is_fading_has_what_is_mixed_drawn_so() {
        let mixed = Look::default();
        let solid = Look { blend: Blend::Solid, ..Look::default() };
        let whole = Lit { on: true, ..Lit::default() };
        let fading = Lit { on: true, alpha: 0.5, ..Lit::default() };
        let cut = AlphaMode::Mask(0.5);
        assert_eq!(drawn_under(&whole, &mixed, cut), cut);
        assert_eq!(drawn_under(&fading, &mixed, cut), AlphaMode::Blend);
        // A solid one is cut still: it is gone when under half of it is there.
        assert_eq!(drawn_under(&fading, &solid, cut), cut);
        // One that takes no light has only its corners' own.
        assert_eq!(drawn_under(&Lit { alpha: 0.5, ..Lit::default() }, &mixed, cut), cut);
        let mut extension = LitExtension::new(&fading, mixed);
        assert_eq!(extension.ambient.w, 0.5);
        extension.set(&whole);
        assert_eq!(extension.ambient.w, 1.0);
    }
}
