//! Player input, from the keyboard, a gamepad, or a script given on the command line.
//!
//! Game logic sees a [`Pad`] shaped like the original's: which buttons are held, which
//! went down or up since the last logic step, the analogue stick and the D-pad. The
//! button bits are the ones the original's code tests, so ported logic reads the same.
//!
//! | PSP | Keyboard | Gamepad |
//! | --- | --- | --- |
//! | analogue stick | W A S D (hold Ctrl to tilt gently) | left stick |
//! | D-pad | arrow keys | D-pad |
//! | square, cross, circle, triangle | J, K, L, I | west, south, east, north |
//! | L, R | Q, E | shoulder buttons |
//! | start, select | Enter, Tab | start, select |

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

/// Button bits, as the original's battle code sees them (its table at 0x16A3C4 maps
/// the hardware's bits to these).
pub mod button {
    pub const START: u32 = 0x001;
    pub const SELECT: u32 = 0x002;
    pub const UP: u32 = 0x004;
    pub const RIGHT: u32 = 0x008;
    pub const DOWN: u32 = 0x010;
    pub const LEFT: u32 = 0x020;
    pub const TRIANGLE: u32 = 0x040;
    pub const CIRCLE: u32 = 0x080;
    pub const CROSS: u32 = 0x100;
    pub const SQUARE: u32 = 0x200;
    pub const L: u32 = 0x400;
    pub const R: u32 = 0x800;
}

/// Seconds a scripted run waits before its first step, so the scene has settled.
const SCRIPT_WARM_UP: f32 = 1.0;

/// Stick deflection that a scripted "walk" step or a held Ctrl key produces.
const WALK_DEFLECTION: f32 = 0.4;

const MOUSE_LOOK_SPEED: f32 = 0.005;
const STICK_LOOK_SPEED: f32 = 2.5;
const ZOOM_SPEED: f32 = 0.1;

/// The controller as of the current logic step.
#[derive(Resource, Default)]
pub struct Pad {
    pub held: u32,
    /// Buttons that went down on this step.
    pub pressed: u32,
    /// Buttons that came up on this step.
    pub released: u32,
    /// Buttons that count as pressed on this step for what takes a held button
    /// again and again, as the menus' cursors do: those that went down, and the
    /// directions and L and R once they have been held long enough (0x10940).
    pub repeat: u32,
    /// X is right and Y is down, as on the PSP; length at most 1.
    pub stick: Vec2,
    /// The D-pad as a direction: X right, Y down, each -1, 0 or 1.
    pub dpad: Vec2,
    /// Only a scripted run sets this: the stick is to be worked for it, in a battle
    /// towards something the cursor can lock onto, on the field towards something
    /// she is to run to.
    pub seek: Option<Seek>,
}

/// How buttons held down come again (0x10940, a part of the pad's frame): the
/// directions and L and R do. Pressing one starts a wait of 24 sixtieths, after
/// which every one of them that is held counts as pressed, and again every 4
/// sixtieths from then on. Letting go of one of them ends it for all, as does a
/// frame in which neither what was held nor what is held is just them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Repeat {
    /// The buttons that are coming again.
    buttons: u32,
    /// The sixtieths until they next do.
    wait: i32,
}

impl Repeat {
    const BUTTONS: u32 = button::UP | button::RIGHT | button::DOWN | button::LEFT | button::L | button::R;
    const FIRST_WAIT: i32 = 24;
    const WAIT: i32 = 4;

    /// A frame of the pad that lasts so many sixtieths: answers the buttons that
    /// count as pressed in it.
    fn step(&mut self, was_held: u32, held: u32, sixtieths: i32) -> u32 {
        if held & Self::BUTTONS != self.buttons && was_held & Self::BUTTONS != self.buttons {
            self.buttons = 0;
        }
        let (pressed, released) = (held & !was_held, was_held & !held);
        if released & self.buttons != 0 {
            self.buttons = 0;
        }
        if pressed != 0 {
            if pressed & Self::BUTTONS & !self.buttons != 0 {
                self.wait = Self::FIRST_WAIT;
            }
            self.buttons = (self.buttons | pressed) & Self::BUTTONS;
            return pressed;
        }
        if self.buttons != 0 {
            self.wait -= sixtieths;
            if self.wait <= 0 {
                self.wait = Self::WAIT;
                return self.buttons;
            }
        }
        0
    }
}

/// What a scripted run has the cursor steered to in a battle, or her on the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seek {
    /// Whatever is nearest the cursor; on the field, the nearest enemy about.
    Nearest,
    /// The enemy with this number, counting from 0 in the order they joined the
    /// battle; the nearest thing if that one cannot be locked onto. On the field,
    /// the character the map's script knows by this number.
    Enemy(usize),
    /// On the field, this place on the ground (x and z).
    Point(i32, i32),
}

/// The controller as of the current displayed frame, before it is cut into steps.
#[derive(Resource, Default)]
pub struct RawPad {
    held: u32,
    /// Buttons that have been down at any moment since the last logic step, so that a
    /// tap shorter than a step still counts.
    seen: u32,
    stick: Vec2,
    seek: Option<Seek>,
    /// Which of the buttons held are coming again, and when.
    repeating: Repeat,
}

/// Camera input that is not part of the original's controls and applies per displayed
/// frame: dragging with the right mouse button, the right stick, and the scroll wheel.
#[derive(Resource)]
pub struct Look {
    /// Camera turn in radians this frame: X is yaw, Y is pitch.
    pub turn: Vec2,
    /// Multiplier for the camera distance this frame; 1 leaves it unchanged.
    pub zoom: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look { turn: Vec2::ZERO, zoom: 1.0 }
    }
}

/// Input to feed in place of the real thing: steps played in order.
#[derive(Resource, Default)]
pub struct Script {
    steps: Vec<Step>,
}

struct Step {
    stick: Vec2,
    buttons: u32,
    seek: Option<Seek>,
    seconds: f32,
}

impl Script {
    /// Parses e.g. `forward:2,left:0.5,wait:1,square:0.1,right+circle:0.2`. A step is
    /// one or more names joined by `+`, held for the given seconds (1 if left out).
    /// Names: `forward`, `back`, `left`, `right` (the stick), `walk` (the stick gently
    /// forward), `wait`, the buttons `square`, `cross`, `circle`, `triangle`, `l`,
    /// `r`, `start`, `select`, `up`, `down`, `dleft`, `dright`, and `seek`, which in
    /// a battle works the stick to bring the cursor to the nearest lock point, or
    /// with a number after it (`seek2`) to that enemy's. On the field `seek` runs
    /// her to the nearest enemy, `seek502` to the character of that number, and
    /// `to-400/2200` to that place (x and z), each straight there.
    pub fn parse(text: &str) -> anyhow::Result<Script> {
        let mut steps = Vec::new();
        for part in text.split(',').filter(|p| !p.is_empty()) {
            let (names, seconds) = part.split_once(':').unwrap_or((part, "1"));
            let mut step = Step { stick: Vec2::ZERO, buttons: 0, seek: None, seconds: seconds.parse()? };
            for name in names.split('+') {
                match name {
                    "walk" => step.stick += Vec2::NEG_Y * WALK_DEFLECTION,
                    "forward" => step.stick += Vec2::NEG_Y,
                    "back" => step.stick += Vec2::Y,
                    "left" => step.stick += Vec2::NEG_X,
                    "right" => step.stick += Vec2::X,
                    "wait" => {}
                    "seek" => step.seek = Some(Seek::Nearest),
                    enemy if enemy.starts_with("seek") => step.seek = Some(Seek::Enemy(enemy["seek".len()..].parse()?)),
                    place if place.starts_with("to") && place.contains('/') => {
                        let (x, z) = place["to".len()..].split_once('/').unwrap_or_default();
                        step.seek = Some(Seek::Point(x.parse()?, z.parse()?));
                    }
                    "square" => step.buttons |= button::SQUARE,
                    "cross" => step.buttons |= button::CROSS,
                    "circle" => step.buttons |= button::CIRCLE,
                    "triangle" => step.buttons |= button::TRIANGLE,
                    "l" => step.buttons |= button::L,
                    "r" => step.buttons |= button::R,
                    "start" => step.buttons |= button::START,
                    "select" => step.buttons |= button::SELECT,
                    "up" => step.buttons |= button::UP,
                    "down" => step.buttons |= button::DOWN,
                    "dleft" => step.buttons |= button::LEFT,
                    "dright" => step.buttons |= button::RIGHT,
                    other => anyhow::bail!("unknown script step {other:?}; see Script::parse for the names"),
                }
            }
            steps.push(step);
        }
        Ok(Script { steps })
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Total running time including the warm-up.
    pub fn seconds(&self) -> f32 {
        if self.steps.is_empty() {
            0.0
        } else {
            SCRIPT_WARM_UP + self.steps.iter().map(|step| step.seconds).sum::<f32>()
        }
    }

    /// The stick, the buttons and whether the cursor is to be steered, `elapsed`
    /// seconds into the run.
    fn at(&self, elapsed: f32) -> (Vec2, u32, Option<Seek>) {
        let mut at = elapsed - SCRIPT_WARM_UP;
        if at < 0.0 {
            return (Vec2::ZERO, 0, None);
        }
        for step in &self.steps {
            if at < step.seconds {
                return (step.stick, step.buttons, step.seek);
            }
            at -= step.seconds;
        }
        (Vec2::ZERO, 0, None)
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Pad>()
        .init_resource::<RawPad>()
        .init_resource::<Look>()
        .add_systems(PreUpdate, read_devices.after(bevy::input::InputSystems))
        .add_systems(FixedPreUpdate, step_pad);
}

/// Reads the keyboard, mouse and gamepads once per displayed frame. A non-empty
/// [`Script`] replaces them entirely, so a verification run cannot be disturbed by
/// whoever is at the keyboard.
fn read_devices(
    mut raw: ResMut<RawPad>,
    mut look: ResMut<Look>,
    script: Res<Script>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    gamepads: Query<&Gamepad>,
) {
    *look = Look::default();
    if !script.is_empty() {
        let (stick, buttons, seek) = script.at(time.elapsed_secs());
        raw.stick = stick;
        raw.held = buttons;
        raw.seen |= buttons;
        raw.seek = seek;
        return;
    }

    const KEYS: [(KeyCode, u32); 12] = [
        (KeyCode::Enter, button::START),
        (KeyCode::Tab, button::SELECT),
        (KeyCode::ArrowUp, button::UP),
        (KeyCode::ArrowRight, button::RIGHT),
        (KeyCode::ArrowDown, button::DOWN),
        (KeyCode::ArrowLeft, button::LEFT),
        (KeyCode::KeyI, button::TRIANGLE),
        (KeyCode::KeyL, button::CIRCLE),
        (KeyCode::KeyK, button::CROSS),
        (KeyCode::KeyJ, button::SQUARE),
        (KeyCode::KeyQ, button::L),
        (KeyCode::KeyE, button::R),
    ];
    const PAD_BUTTONS: [(GamepadButton, u32); 12] = [
        (GamepadButton::Start, button::START),
        (GamepadButton::Select, button::SELECT),
        (GamepadButton::DPadUp, button::UP),
        (GamepadButton::DPadRight, button::RIGHT),
        (GamepadButton::DPadDown, button::DOWN),
        (GamepadButton::DPadLeft, button::LEFT),
        (GamepadButton::North, button::TRIANGLE),
        (GamepadButton::East, button::CIRCLE),
        (GamepadButton::South, button::CROSS),
        (GamepadButton::West, button::SQUARE),
        (GamepadButton::LeftTrigger, button::L),
        (GamepadButton::RightTrigger, button::R),
    ];

    let mut held = 0;
    for (key, bit) in KEYS {
        if keys.pressed(key) {
            held |= bit;
        }
    }
    let axis = |negative: KeyCode, positive: KeyCode| {
        keys.pressed(positive) as i32 as f32 - keys.pressed(negative) as i32 as f32
    };
    let mut stick = Vec2::new(axis(KeyCode::KeyA, KeyCode::KeyD), axis(KeyCode::KeyW, KeyCode::KeyS));
    // A keyboard has no half-way: holding Ctrl stands in for tilting the stick gently.
    if keys.pressed(KeyCode::ControlLeft) {
        stick *= WALK_DEFLECTION;
    }

    let step = time.delta_secs();
    if mouse.pressed(MouseButton::Right) {
        look.turn += Vec2::new(-motion.delta.x, motion.delta.y) * MOUSE_LOOK_SPEED;
    }
    for gamepad in &gamepads {
        for (pad_button, bit) in PAD_BUTTONS {
            if gamepad.pressed(pad_button) {
                held |= bit;
            }
        }
        // Bevy's sticks have Y up; the PSP's has Y down.
        stick += gamepad.left_stick() * Vec2::new(1.0, -1.0);
        let right = gamepad.right_stick();
        look.turn += Vec2::new(-right.x, right.y) * STICK_LOOK_SPEED * step;
    }
    look.zoom = (1.0 - scroll.delta.y * ZOOM_SPEED).clamp(0.5, 2.0);

    raw.stick = stick.clamp_length_max(1.0);
    raw.held = held;
    raw.seen |= held;
}

/// Cuts the input into logic steps: what is held now, and what changed since the
/// last step.
fn step_pad(mut pad: ResMut<Pad>, mut raw: ResMut<RawPad>) {
    let held = raw.held | std::mem::take(&mut raw.seen);
    let was_held = pad.held;
    pad.pressed = held & !was_held;
    pad.released = was_held & !held;
    pad.repeat = raw.repeating.step(was_held, held, brs_engine::step::SIXTIETHS_PER_STEP as i32);
    pad.held = held;
    pad.stick = raw.stick;
    pad.seek = raw.seek;
    let direction = |negative: u32, positive: u32| (held & positive != 0) as i32 as f32 - (held & negative != 0) as i32 as f32;
    pad.dpad = Vec2::new(direction(button::LEFT, button::RIGHT), direction(button::UP, button::DOWN));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frames of the game, counted from the press, in which a button held for
    /// `frames` of them counts as pressed.
    fn comes(button: u32, frames: usize) -> Vec<usize> {
        let mut repeat = Repeat::default();
        let mut was_held = 0;
        let mut came = Vec::new();
        for frame in 0..frames {
            if repeat.step(was_held, button, 2) & button != 0 {
                came.push(frame);
            }
            was_held = button;
        }
        came
    }

    #[test]
    fn a_direction_held_comes_again_after_a_wait_and_then_quickly() {
        // Twenty-four sixtieths after the press, then every four: at thirty frames
        // a second, the twelfth frame after it and every other one from there.
        assert_eq!(comes(button::DOWN, 19), [0, 12, 14, 16, 18]);
        assert_eq!(comes(button::L, 13), [0, 12]);
        // The other buttons count once, when they go down.
        assert_eq!(comes(button::CROSS, 40), [0]);
    }

    #[test]
    fn letting_go_ends_it_and_another_direction_starts_the_wait_anew() {
        let mut repeat = Repeat::default();
        assert_eq!(repeat.step(0, button::DOWN, 2), button::DOWN);
        for _ in 0..11 {
            assert_eq!(repeat.step(button::DOWN, button::DOWN, 2), 0);
        }
        // Up goes down as well just before down would have come again: the wait is
        // the full one once more, and then both come.
        let both = button::DOWN | button::UP;
        assert_eq!(repeat.step(button::DOWN, both, 2), button::UP);
        for _ in 0..11 {
            assert_eq!(repeat.step(both, both, 2), 0);
        }
        assert_eq!(repeat.step(both, both, 2), both);
        // Letting one of them go ends it for the other too, for good.
        assert_eq!(repeat.step(both, button::DOWN, 2), 0);
        for _ in 0..40 {
            assert_eq!(repeat.step(button::DOWN, button::DOWN, 2), 0);
        }
    }
}
