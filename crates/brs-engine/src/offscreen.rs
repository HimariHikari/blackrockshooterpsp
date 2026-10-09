//! Drawing without a window, for runs nobody is watching.
//!
//! A run that only saves pictures to check a change by has no business on the screen
//! of whoever is using the machine: a window, however it is opened, can end up in
//! front of what they are doing. Such a run opens none. Every camera draws to a
//! picture in memory instead, and the pictures saved are taken from that. It makes no
//! sound either, unless it is told to keep the sound.

use std::time::Duration;

use bevy::app::{PluginGroupBuilder, ScheduleRunnerPlugin};
use bevy::audio::AudioPlugin;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::Screenshot;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

/// Steps a second of a run without a window, which has no display to keep time by.
const STEPS_PER_SECOND: f64 = 60.0;

/// In a run without a window: the picture every camera draws to.
#[derive(Resource, Clone)]
pub struct Offscreen(pub Handle<Image>);

/// Bevy's plugins for a program that shows `window`; or, with `offscreen`, for one
/// that shows nothing and draws to a picture of the window's size. Without `sound`
/// the program has nothing to put sound out with.
pub fn plugins(window: Window, offscreen: bool, sound: bool) -> PluginGroupBuilder {
    let plugins = if sound { DefaultPlugins.build() } else { DefaultPlugins.build().disable::<AudioPlugin>() };
    if !offscreen {
        return plugins.set(WindowPlugin { primary_window: Some(window), ..default() });
    }
    let (width, height) = (window.resolution.physical_width(), window.resolution.physical_height());
    plugins
        .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, ..default() })
        .disable::<WinitPlugin>()
        .add(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / STEPS_PER_SECOND)))
        .add(OffscreenPlugin { width, height })
}

/// What to take a picture of: the window, or what is drawn to in its stead.
pub fn screenshot(offscreen: Option<&Offscreen>) -> Screenshot {
    match offscreen {
        Some(Offscreen(picture)) => Screenshot::image(picture.clone()),
        None => Screenshot::primary_window(),
    }
}

struct OffscreenPlugin {
    width: u32,
    height: u32,
}

impl Plugin for OffscreenPlugin {
    fn build(&self, app: &mut App) {
        let picture = Image::new_target_texture(self.width, self.height, TextureFormat::Rgba8UnormSrgb, None);
        let picture = app.world_mut().resource_mut::<Assets<Image>>().add(picture);
        app.insert_resource(Offscreen(picture)).add_observer(draw_to_picture);
    }
}

/// A new camera would draw to the window; there is none, so it draws to the picture.
/// Where the canvas is in use it is the canvas that says what each camera draws to:
/// only its own draws to the picture.
fn draw_to_picture(added: On<Add, Camera>, offscreen: Res<Offscreen>, screen: Option<Res<crate::canvas::Screen>>, mut commands: Commands) {
    if screen.is_none() {
        commands.entity(added.entity).insert(RenderTarget::from(offscreen.0.clone()));
    }
}
