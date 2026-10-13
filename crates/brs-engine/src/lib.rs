//! Bevy-side building blocks shared by the viewer and the game: turning the game's
//! models, animations, maps and collision data into entities and queries.

use bevy::ecs::system::SystemParam;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;

pub mod animation;
pub mod audio;
pub mod camera;
pub mod canvas;
pub mod chain;
pub mod collision;
pub mod effect;
pub mod effects;
pub mod film;
pub mod layout;
pub mod lit;
pub mod map;
pub mod model;
pub mod offscreen;
pub mod step;
pub mod texture;

/// Registers the engine's systems.
///
/// By default animations advance with every displayed frame, which suits a viewer.
/// With `stepped` set they advance with the game's fixed logic steps instead (see
/// [`step`]): logic goes in `FixedUpdate`, before [`EngineSystems::Animate`] if it
/// changes an [`animation::Animator`], and moves things through [`step::Stepped`].
#[derive(Default)]
pub struct EnginePlugin {
    pub stepped: bool,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineSystems {
    Animate,
}

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        // A run that plays no sound still loads and asks for it.
        audio::plugin(app, !app.is_plugin_added::<bevy::audio::AudioPlugin>());
        effects::plugin(app, self.stepped);
        lit::plugin(app);
        map::plugin(app);
        if self.stepped {
            app.insert_resource(Time::<Fixed>::from_hz(step::STEPS_PER_SECOND))
                .add_systems(Startup, step::no_catching_up)
                .add_systems(FixedFirst, step::remember)
                .add_systems(FixedUpdate, (animation::advance, chain::swing).chain().in_set(EngineSystems::Animate))
                .add_systems(
                    RunFixedMainLoop,
                    (step::smooth, animation::pose_between_steps)
                        .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
                );
        } else {
            app.add_systems(Update, (animation::advance, chain::swing, animation::pose).chain().in_set(EngineSystems::Animate));
        }
    }
}

/// The asset collections needed to build models and maps.
#[derive(SystemParam)]
pub struct AssetStores<'w> {
    pub images: ResMut<'w, Assets<Image>>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    /// What the batches of maps are drawn with (see [`map`]).
    pub maps: ResMut<'w, Assets<map::MapMaterial>>,
    /// What each material built for a model makes of a light (see [`lit`]).
    pub looks: ResMut<'w, lit::Looks>,
    pub inverse_bindposes: ResMut<'w, Assets<SkinnedMeshInverseBindposes>>,
}
