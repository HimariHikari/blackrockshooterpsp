//! Fixed logic steps, and what is drawn of them.
//!
//! The original advances its field and battle logic 30 times a second (its menus run
//! at 60), with speeds, timers and animation lengths all counted against that, and
//! draws a frame for each step. The port keeps the logic on the same fixed steps, so
//! it behaves the same at any display rate, and draws what the logic moves where the
//! last step left it: thirty pictures of the world a second, as on the PSP, however
//! often the window is drawn.
//!
//! It used to blend places between one step and the next, and run animations on by
//! the part of a step that had passed. The two did not go together (a place was
//! drawn behind its step and a pose ahead of it), which showed as feet slipping
//! over the ground and as a pose jumping back when a step changed the animation.

use bevy::prelude::*;

/// Logic steps per second.
pub const STEPS_PER_SECOND: f64 = 30.0;

/// The original measures time in sixtieths of a second, so one logic step is two.
pub const SIXTIETHS_PER_STEP: f32 = (60.0 / STEPS_PER_SECOND) as f32;

/// Has a frame that took long count for one step and no more. A frame takes
/// long when a scene is read from the disc, or when the graphics card makes
/// ready for something it draws for the first time; left to itself the game
/// would then run every step the long frame was worth at once (up to a quarter
/// of a second of them), and all that moves would be seen to skip ahead. As it
/// is the game goes on from where it was, the way the PSP slows when it has
/// much to do.
pub fn no_catching_up(mut time: ResMut<Time<Virtual>>) {
    time.set_max_delta(std::time::Duration::from_secs_f64(1.0 / STEPS_PER_SECOND));
}

/// Where game logic has put an entity. Logic moves `current` once per step, and
/// the entity's [`Transform`] is that; where it was a step before is kept for
/// whoever asks how it moved.
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

/// Before drawing, places each stepped entity where its last step left it.
pub fn smooth(mut stepped: Query<(&Stepped, &mut Transform)>) {
    for (stepped, mut transform) in &mut stepped {
        if *transform != stepped.current {
            *transform = stepped.current;
        }
    }
}
