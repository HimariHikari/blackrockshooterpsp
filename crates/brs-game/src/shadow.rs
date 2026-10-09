//! The round shadows on the ground under what stands on it.
//!
//! The field draws one under each of its characters (0x10791C, from the
//! character's draw at 0x1067F8) and the ride on the bike one under each of its
//! (0x146B1C): the same small picture (0x1100BC(0): the first of the field's
//! resident textures), black, on a flat square laid on the ground a little above
//! where the thing stands (3 and 3 again), mixed over what is there by the
//! picture's alpha and a strength: 96 of 255 on the field, 150 on the ride.

use anyhow::Result;
use bevy::prelude::*;

use crate::data::GameData;
use crate::interface;

/// The picture.
const PICTURE: &str = "SYSTEM.VOL/fld_resident.bin/01.bin/00.ptm";
/// How far above where the thing stands the square lies.
pub const LIFT: f32 = 6.0;
/// How strong a shadow is on the field and on the ride, of 255.
const FIELD_ALPHA: f32 = 96.0;
const RIDE_ALPHA: f32 = 150.0;

/// Reads what the shadows are drawn with as the game starts.
fn load(mut commands: Commands, data: Res<GameData>, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    match Shadows::load(&data, &mut images, &mut meshes, &mut materials) {
        Ok(shadows) => commands.insert_resource(shadows),
        Err(error) => warn!("the shadows cannot be drawn: {error:#}"),
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load);
}

/// What the shadows are drawn with: a square from -1 to 1 across and along, and
/// the picture at each of the two strengths.
#[derive(Resource, Clone)]
pub struct Shadows {
    mesh: Handle<Mesh>,
    field: Handle<StandardMaterial>,
    ride: Handle<StandardMaterial>,
}

/// Which strength a shadow has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Of {
    Field,
    Ride,
}

impl Shadows {
    pub fn load(data: &GameData, images: &mut Assets<Image>, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Result<Shadows> {
        let (picture, _) = interface::load_texture(data, images, PICTURE)?;
        let mut material = |alpha: f32| {
            materials.add(StandardMaterial {
                base_color: Color::linear_rgba(0.0, 0.0, 0.0, alpha / 255.0),
                base_color_texture: Some(picture.clone()),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            })
        };
        let (field, ride) = (material(FIELD_ALPHA), material(RIDE_ALPHA));
        Ok(Shadows { mesh: meshes.add(Plane3d::default().mesh().size(2.0, 2.0)), field, ride })
    }

    /// What a shadow is made of, to be put somewhere and given its size.
    pub fn bundle(&self, of: Of, transform: Transform) -> impl Bundle {
        let material = match of {
            Of::Field => self.field.clone(),
            Of::Ride => self.ride.clone(),
        };
        (Mesh3d(self.mesh.clone()), MeshMaterial3d(material), transform, Visibility::Inherited)
    }
}
