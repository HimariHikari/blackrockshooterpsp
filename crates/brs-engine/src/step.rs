//! Fixed logic steps, and smoothing between them.
//!
//! The original advances its field and battle logic 30 times a second (its menus run
//! at 60), with speeds, timers and animation lengths all counted against that. The
//! port keeps the logic on the same fixed steps, so it behaves the same at any display
//! rate, and blends what the logic moves between one step and the next for display.

use bevy::prelude::*;

/// Logic steps per second.
pub const STEPS_PER_SECOND: f64 = 30.0;

/// The original measures time in sixtieths of a second, so one logic step is two.
pub const SIXTIETHS_PER_STEP: f32 = (60.0 / STEPS_PER_SECOND) as f32;

/// Where game logic has put an entity. Logic moves `current` once per step; the
/// entity's [`Transform`] is blended from the previous step's value towards it.
#[derive(Component, Clone, Copy)]
pub struct Stepped {
    previous: Transform,
    pub current: Transform,
}

impl Stepped {
    pub fn new(at: Transform) -> Self {
        Stepped { previous: at, current: at }
    }

    /// Moves at once, with no blend from the old place: a cut, not a move.
    pub fn snap(&mut self, to: Transform) {
        *self = Stepped::new(to);
    }
}

/// At the start of each step, notes where everything was.
pub fn remember(mut stepped: Query<&mut Stepped>) {
    for mut stepped in &mut stepped {
        stepped.previous = stepped.current;
    }
}

/// Before drawing, places each stepped entity part-way between its last two steps.
pub fn smooth(fixed: Res<Time<Fixed>>, mut stepped: Query<(&Stepped, &mut Transform)>) {
    let alpha = fixed.overstep_fraction();
    for (stepped, mut transform) in &mut stepped {
        *transform = Transform {
            translation: stepped.previous.translation.lerp(stepped.current.translation, alpha),
            rotation: stepped.previous.rotation.slerp(stepped.current.rotation, alpha),
            scale: stepped.previous.scale.lerp(stepped.current.scale, alpha),
        };
    }
}
