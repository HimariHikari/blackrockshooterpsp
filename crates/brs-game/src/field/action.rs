//! The sign of what a button would do with what she is by: a ring with the
//! cross in it and a word, ACTION, CHECK or TALK, which comes up over her when
//! she is by something there is something to do with, and goes when she is no
//! longer by it. See `docs/FIELD_NOTES.md`.
//!
//! The sign itself is an object of the signs' layout (0x464FC to 0x468D0), which
//! has a place for each of the three words and plays one set as it comes and
//! another as it goes; the field keeps it (its object at + 0x1D64, 0xEDCE0 to
//! 0xEE3B0) the way it keeps the sign that names a place: one that is asked for
//! while another is up waits for that to have gone.

use bevy::prelude::*;

use super::scene::Scene;
use crate::controls::{button, Pad};
use crate::sound::Request;

/// The frames of the interface that go by in a frame of the game (0x850).
pub const FRAMES_A_STEP: f32 = 2.0;
/// The sets of the signs' layout a sign plays as it comes and as it goes, by its
/// word (0x46774). The set between the two, which has the button shrink away,
/// is kept by the sign and never played.
pub const SETS: [[usize; 2]; 3] = [[6, 8], [9, 11], [12, 14]];
/// The frames either set has.
pub const SET_FRAMES: f32 = 20.0;
/// How far above where she stands the sign is put (0xEE190).
pub const ABOVE: f32 = 50.0;

/// What she is by that a button does something with (the field's + 0xE70).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    Character(i32),
    Trap(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Coming = 0,
    Standing = 1,
    Going = 2,
}

/// What of the sign there is to draw: which word's, the set it is playing and
/// how far into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Showing {
    pub kind: usize,
    pub set: usize,
    pub frames: f32,
    /// It is still playing: the frame drawn may be past the game's last one.
    pub playing: bool,
}

/// Where the sign is: so far above where she stands, by her own upright as her
/// frame lies (0xEE178 to 0xEE290, with 0x1090E0 at 0xEE1C0; there it is given
/// the place on the screen the camera has that at, and behind the camera it is
/// not drawn).
pub fn place(she: Vec3, up: Vec3) -> Vec3 {
    she + up * ABOVE
}

/// The sound of taking up what there is to do, of the bank that is always there
/// (0x10D844).
const SOUND_TAKEN: u32 = 3;

/// What cross and square do with something, as the words of its sign.
fn words(scene: &Scene, by: By) -> [Option<i32>; 2] {
    match by {
        By::Character(id) => scene.characters.get(&id).map_or([None; 2], |character| [character.prompt(0), character.prompt(1)]),
        By::Trap(id) => scene.traps.get(&id).map_or([None; 2], |trap| [trap.prompt(0), trap.prompt(1)]),
    }
}

/// What a frame of the field makes of the sign and of the buttons that go with
/// it (0x10D700 to 0x10DB00). `in_touch` is what she is in touch with, in the
/// order she came to it; `hers` is whether the field is hers to move in and
/// nothing has started of itself this frame. It answers with what a button has
/// just done something with, whose event the field then starts.
///
/// The sign is for the first thing she is in touch with that a button does
/// anything with (0x11743C), and goes when she is no longer by that. Once it
/// stands, cross does the first thing there is to do with it and square the
/// second (0x10D7B8): with the sound of taking something up, and the script can
/// read which button it was and what word the sign had.
pub fn frame(scene: &mut Scene, in_touch: &[By], pad: &Pad, hers: bool) -> Option<By> {
    if !hers {
        // 0x115DFC: it goes by its set, and what it was for is forgotten.
        if scene.action_sign.asked() {
            scene.action_sign.dismiss(false);
        }
        scene.action_sign.by = None;
        return None;
    }
    let mut done = None;
    if scene.action_sign.asked() {
        match scene.action_sign.by.filter(|by| in_touch.contains(by)) {
            None => {
                scene.action_sign.dismiss(false);
                scene.action_sign.by = None;
            }
            Some(by) if scene.action_sign.standing() => {
                let its = words(scene, by);
                let pressed = |button: u32, slot: usize| (pad.pressed & button != 0).then_some(slot).filter(|&slot| its[slot].is_some());
                if let Some(slot) = pressed(button::CROSS, 0).or_else(|| pressed(button::SQUARE, 1)) {
                    scene.sounds.push(Request::common(SOUND_TAKEN));
                    scene.chosen = (slot as i32, its[slot].unwrap_or(0));
                    scene.action_sign.dismiss(false);
                    scene.action_sign.by = None;
                    done = Some(by);
                }
            }
            Some(_) => {}
        }
    }
    if done.is_none() && !scene.action_sign.asked() {
        let by = in_touch.iter().copied().find(|&by| match by {
            By::Character(id) => scene.characters.get(&id).is_some_and(|character| character.by_action && !character.hidden()),
            By::Trap(id) => scene.traps.get(&id).is_some_and(|trap| trap.by_action && !trap.off),
        });
        if let Some(by) = by {
            let word = words(scene, by)[0];
            scene.action_sign.ask(word);
            scene.action_sign.by = Some(by);
        }
    }
    done
}

/// The sign and what the field keeps of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionSign {
    /// One is up (the field's flag 8).
    up: bool,
    /// Which word it has.
    kind: i32,
    state: State,
    // The sign's own (its flags 1, 2 and 4): it is shown, it is playing the set
    // it goes by, and the set it is playing is through.
    shown: bool,
    going: bool,
    through: bool,
    /// The frames of its layout it has played of that set.
    frames: f32,
    /// It is to go as soon as it has come (the field's flag 0x10), and at once
    /// rather than by its set (0x40).
    go_when_come: bool,
    at_once: bool,
    /// It has been asked for and not sent away since (0x20): what the field goes
    /// by when it decides whether to ask.
    asked: bool,
    /// The word of the one that is to come when this has gone (2).
    waiting: Option<i32>,
    /// What it is for.
    pub by: Option<By>,
}

impl Default for ActionSign {
    fn default() -> ActionSign {
        ActionSign {
            up: false,
            kind: 0,
            state: State::Going,
            shown: false,
            going: false,
            through: false,
            frames: 0.0,
            go_when_come: false,
            at_once: false,
            asked: false,
            waiting: None,
            by: None,
        }
    }
}

impl ActionSign {
    /// Whether it has been asked for (0xEE11C).
    pub fn asked(&self) -> bool {
        self.asked
    }

    /// Whether it has come and stands (0xEE0F0), which is when a button is taken.
    pub fn standing(&self) -> bool {
        self.state == State::Standing
    }

    /// A sign is put up with a word (0xEDD64, 0x467F0): its place plays the set
    /// it comes by from the start. With no word it keeps the one it had.
    fn set(&mut self, kind: Option<i32>) {
        self.kind = kind.unwrap_or(self.kind);
        (self.shown, self.going, self.through, self.frames) = (true, false, false, 0.0);
        (self.up, self.state) = (true, State::Coming);
    }

    /// It plays the set it goes by (0x46844).
    fn go(&mut self) {
        (self.shown, self.going, self.through, self.frames) = (true, true, false, 0.0);
    }

    /// 0xEDE84: it goes, by its set or at once (0x46894).
    fn leave(&mut self) {
        if self.at_once {
            (self.shown, self.going, self.through) = (false, true, true);
        } else {
            self.go();
        }
        self.go_when_come = false;
        self.state = State::Going;
    }

    /// The sign is asked for (0xEDFA4), with the word of what the first button
    /// does: with none up it comes; with one up, that goes first.
    pub fn ask(&mut self, kind: Option<i32>) {
        (self.go_when_come, self.at_once) = (false, false);
        if !self.up {
            self.set(kind);
        } else {
            if self.state == State::Standing {
                self.go();
                self.state = State::Going;
            }
            self.waiting = Some(kind.unwrap_or(self.kind));
        }
        self.asked = true;
    }

    /// It is sent away (0xEE050): by its set, or at once. One that waited to come
    /// after it is forgotten.
    pub fn dismiss(&mut self, at_once: bool) {
        self.at_once = at_once;
        self.waiting = None;
        if self.up {
            match self.state {
                State::Coming => self.go_when_come = true,
                State::Standing => self.leave(),
                State::Going => {}
            }
        }
        self.asked = false;
    }

    /// The sign's own frame (0x46688): its set goes on, until it is through.
    fn sign_frame(&mut self) {
        if !self.shown {
            return;
        }
        if self.frames >= SET_FRAMES {
            self.through = true;
        } else {
            self.frames += FRAMES_A_STEP;
        }
    }

    /// A frame of it (0xEE294 on).
    pub fn step(&mut self) {
        if !self.up {
            return;
        }
        self.sign_frame();
        match self.state {
            State::Coming if self.through => {
                if self.go_when_come {
                    self.leave();
                } else if self.waiting.is_some() {
                    self.go();
                    self.state = State::Going;
                } else {
                    self.state = State::Standing;
                }
            }
            State::Going if self.through => match self.waiting.take() {
                Some(kind) => self.set(Some(kind)),
                None => (self.up, self.shown) = (false, false),
            },
            _ => {}
        }
    }

    /// What there is of it to draw (0xEDF20, 0x466F8).
    pub fn showing(&self) -> Option<Showing> {
        let kind = usize::try_from(self.kind).ok().filter(|&kind| kind < SETS.len())?;
        let seen = self.up && self.shown;
        seen.then(|| Showing { kind, set: SETS[kind][self.going as usize], frames: self.frames, playing: !self.through })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::scene::{prompt, setting, test_defaults, Character, TrapKind};

    #[test]
    fn the_sign_comes_for_what_she_is_by_and_cross_does_what_it_says() {
        let mut scene = Scene::new(0, 0, test_defaults());
        // A home point, which cross checks; a soldier, whom cross talks to; an
        // arrow, which no button does anything with; and a trap made to be done
        // by a button.
        for id in [200, 600, 700] {
            scene.characters.insert(id, Character::new(17, Vec3::ZERO, 0.0, &scene.defaults));
        }
        scene.characters.get_mut(&200).unwrap().set(setting::CONTACT, prompt::CHECK);
        scene.characters.get_mut(&700).unwrap().set(setting::BY_ACTION, 0);
        scene.make_trap(502, TrapKind::Event);
        let (nothing, cross, square) = (Pad::default(), Pad { pressed: button::CROSS, ..Pad::default() }, Pad { pressed: button::SQUARE, ..Pad::default() });
        let frame = |scene: &mut Scene, in_touch: &[By], pad: &Pad, hers: bool| {
            let done = frame(scene, in_touch, pad, hers);
            scene.action_sign.step();
            done
        };

        // By nothing, or by the arrow alone, there is no sign.
        assert_eq!(frame(&mut scene, &[], &cross, true), None);
        assert_eq!(frame(&mut scene, &[By::Character(700)], &cross, true), None);
        assert!(scene.action_sign.showing().is_none() && scene.sounds.is_empty());

        // By the arrow and then the home point: CHECK comes, and cross does
        // nothing until it stands.
        let by = [By::Character(700), By::Character(200)];
        assert_eq!(frame(&mut scene, &by, &nothing, true), None);
        assert_eq!((scene.action_sign.showing().map(|showing| showing.set), scene.action_sign.by), (Some(9), Some(By::Character(200))));
        for _ in 0..9 {
            assert_eq!(frame(&mut scene, &by, &cross, true), None);
        }
        assert!(!scene.action_sign.standing());
        assert_eq!(frame(&mut scene, &by, &nothing, true), None);
        assert!(scene.action_sign.standing());
        // Square has nothing to do with it; cross checks it, with its sound, and
        // the sign goes.
        assert_eq!(frame(&mut scene, &by, &square, true), None);
        assert_eq!(frame(&mut scene, &by, &cross, true), Some(By::Character(200)));
        assert_eq!((scene.sounds.as_slice(), scene.chosen), ([Request::common(3)].as_slice(), (0, prompt::CHECK)));
        assert_eq!((scene.action_sign.showing().map(|showing| showing.set), scene.action_sign.asked()), (Some(11), false));
        // While its event runs the field is not hers, and no sign is asked for.
        for _ in 0..12 {
            assert_eq!(frame(&mut scene, &by, &cross, false), None);
        }
        assert!(scene.action_sign.showing().is_none());

        // By the soldier TALK comes; she walks on into the trap, still by him:
        // the sign is his until she leaves him. Then it is the trap's: TALK
        // goes, and ACTION comes when it has.
        let by = [By::Character(600)];
        frame(&mut scene, &by, &nothing, true);
        assert_eq!(scene.action_sign.showing().map(|showing| showing.set), Some(12));
        let by = [By::Character(600), By::Trap(502)];
        for _ in 0..15 {
            frame(&mut scene, &by, &nothing, true);
        }
        assert_eq!((scene.action_sign.by, scene.action_sign.standing()), (Some(By::Character(600)), true));
        let by = [By::Trap(502)];
        frame(&mut scene, &by, &nothing, true);
        assert_eq!((scene.action_sign.showing().map(|showing| showing.set), scene.action_sign.by, scene.action_sign.asked()), (Some(14), Some(By::Trap(502)), true));
        let mut frames = 1;
        while !scene.action_sign.standing() {
            assert_eq!(frame(&mut scene, &by, &cross, true), None);
            frames += 1;
            assert!(frames < 100);
        }
        // Eleven frames to go and eleven to come.
        assert_eq!((frames, scene.action_sign.showing().map(|showing| showing.set)), (22, Some(6)));
        assert_eq!(frame(&mut scene, &by, &cross, true), Some(By::Trap(502)));
        assert_eq!(scene.chosen, (0, prompt::ACTION));

        // A trap that has been switched off has no sign.
        for _ in 0..12 {
            frame(&mut scene, &[], &nothing, true);
        }
        scene.traps.get_mut(&502).unwrap().off = true;
        assert_eq!(frame(&mut scene, &by, &cross, true), None);
        assert!(scene.action_sign.showing().is_none());
    }

    /// Frames until it stands.
    fn come(sign: &mut ActionSign) -> i32 {
        let mut frames = 0;
        while !sign.standing() {
            sign.step();
            frames += 1;
            assert!(frames < 100);
        }
        frames
    }

    #[test]
    fn it_comes_by_its_set_stands_and_goes_by_the_other() {
        let mut sign = ActionSign::default();
        assert!(sign.showing().is_none() && !sign.asked());
        sign.step();
        assert!(sign.showing().is_none());

        // Asked for with CHECK: its place plays set 9, twenty frames of it at two
        // a frame, and the frame after it stands.
        sign.ask(Some(1));
        assert!(sign.asked() && !sign.standing());
        assert_eq!(sign.showing(), Some(Showing { kind: 1, set: 9, frames: 0.0, playing: true }));
        assert_eq!(come(&mut sign), 11);
        assert_eq!(sign.showing().map(|showing| (showing.set, showing.frames, showing.playing)), Some((9, 20.0, false)));
        assert_eq!(place(Vec3::new(10.0, 0.0, -20.0), Vec3::Y), Vec3::new(10.0, 50.0, -20.0));
        // By her own upright, where her frame is turned.
        assert_eq!(place(Vec3::new(10.0, 0.0, -20.0), Vec3::NEG_X), Vec3::new(-40.0, 0.0, -20.0));

        // Sent away it plays set 11 and is done with when that is through.
        sign.dismiss(false);
        assert!(!sign.asked() && !sign.standing());
        assert_eq!(sign.showing().map(|showing| (showing.set, showing.frames)), Some((11, 0.0)));
        let mut frames = 0;
        while sign.showing().is_some() {
            sign.step();
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(frames, 11);
    }

    #[test]
    fn one_asked_for_while_another_is_up_waits_for_that_to_have_gone() {
        let mut sign = ActionSign::default();
        sign.ask(Some(2));
        come(&mut sign);
        // TALK stands; ACTION is asked for: TALK goes, and ACTION comes after it.
        sign.ask(Some(0));
        assert_eq!(sign.showing().map(|showing| showing.set), Some(14));
        for _ in 0..10 {
            sign.step();
            assert_eq!(sign.showing().map(|showing| showing.kind), Some(2));
        }
        sign.step();
        assert_eq!(sign.showing().map(|showing| (showing.kind, showing.set, showing.frames)), Some((0, 6, 0.0)));
        assert_eq!(come(&mut sign), 11);

        // Sent away as it comes, it goes as soon as it has come; at once, it is
        // gone the frame after.
        let mut sign = ActionSign::default();
        sign.ask(Some(0));
        sign.step();
        sign.dismiss(false);
        for _ in 0..9 {
            sign.step();
            assert_eq!(sign.showing().map(|showing| showing.set), Some(6));
        }
        sign.step();
        assert_eq!(sign.showing().map(|showing| (showing.set, showing.frames)), Some((8, 0.0)));

        let mut sign = ActionSign::default();
        sign.ask(Some(0));
        come(&mut sign);
        sign.dismiss(true);
        assert!(sign.showing().is_none());
        sign.step();
        sign.ask(Some(1));
        assert_eq!(sign.showing().map(|showing| (showing.kind, showing.set)), Some((1, 9)));
    }
}
