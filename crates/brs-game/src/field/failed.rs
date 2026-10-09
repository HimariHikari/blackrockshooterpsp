//! The window of a mission failed (window 13): "MISSION FAILED" over the field,
//! and three things to choose from: to try the mission again, to go and choose
//! another, or to load a game. See `docs/FIELD_NOTES.md`.
//!
//! The field's own part of it is small (0xEE760 opens it, 0xEE8B0 is its frame):
//! it lets the field come to a stop, makes the window's task ("ifFldFailedTask",
//! 0x42A6C), and when that is done leaves the field the way the choice says, as
//! `s173` would. The task loads the window's own file, plays its jingle and runs
//! its menu (0x6E59C to 0x6F314), which is what is here; `field/hud/failed.rs`
//! draws it from its layout, by what is kept here.
//!
//! The script that opened the window never goes on: the system script's wait for
//! it (0xD5CA0) lasts for as long as the field does.

use crate::controls::{button, Pad};
use crate::sound::{Keeper, Request};

use super::scene::{value, Fade, Scene};

/// The frames of the window's layout that go by in a frame of the game (0x850).
pub const FRAMES_A_STEP: f32 = 2.0;

/// The sets of the window's layout (`IF/MENU/IF_MISSION_FAILED.GZ`) as the menu
/// plays them (0x6EF70), each over what it is playing: one that brings all of it,
/// and for each choice one that has the cursor come onto it, one that has it
/// leave, one for its going once it has been made, and one that brings it with
/// the cursor on it already. (Set 1, which takes all of it away, is never played:
/// the screen goes dark over it instead.)
pub mod set {
    pub const COMING: usize = 0;
    pub const ONTO: [usize; 3] = [2, 4, 6];
    pub const OFF: [usize; 3] = [3, 5, 7];
    pub const MADE: [usize; 3] = [8, 9, 10];
    pub const COMING_CHOSEN: [usize; 3] = [11, 12, 13];
}

/// The choices, top to bottom: "RETRY MISSION", "STAGE SELECT", "LOAD".
pub const RETRY: usize = 0;
pub const STAGE_SELECT: usize = 1;
pub const LOAD: usize = 2;

/// How the field is left for each choice (0xEE9A0): what `s173` would be given.
const LEAVING: [i32; 3] = [5, 3, 4];

/// The save's flag that has "STAGE SELECT" out of reach (0x6E838): the story's
/// opening sets it, when there is no other mission to choose yet.
pub const NO_STAGE_SELECT: i32 = 6169;

/// When the menu does what, in the frames of its layout: all of it has come by
/// here (the choice the cursor is on blinks last), and a choice made has gone by
/// here. They are what the layout's own clips come to (`field/hud/failed.rs`
/// checks them against it).
pub const COMING: f32 = 80.0;
pub const GOING: f32 = 50.0;
/// Its sound as it comes is played from this frame on (0x6E75C).
const SOUND_AT: f32 = 61.0;
/// The frames the screen takes to go dark once a choice is made (0x6F078), of the
/// game's: sixty of the interface's.
const DARK_FRAMES: i32 = 30;

// Its sounds, of the bank that is always there: as it has come (0x6E784), as the
// cursor moves (0x6EA14), as a choice is made (0x6E8CC), and when the choice
// cannot be made (0x6E93C).
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_MADE: u32 = 3;
const SOUND_REFUSED: u32 = 7;

/// Its jingle (0x42B00): the fifth of `jin_pack.bin`, on the stream the first
/// voice of a scene is heard on, with the tune turned down to next to nothing.
pub const JINGLE: &str = "SYSTEM.VOL/jin_pack.bin/004.riff";
const JINGLE_STREAM: usize = 0;
const TUNE_UNDER_JINGLE: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// All of it is coming; no button is taken.
    Coming,
    Asking,
    /// "LOAD" was chosen (3): the list of saves is up, and the window waits.
    Loading,
    /// A choice is made: it goes, and the screen goes dark.
    Going,
    /// It is gone, and the field is being left.
    Gone,
}

/// The window, from when the system script opens it.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    pub phase: Phase,
    /// The choice the cursor is on.
    pub cursor: usize,
    /// "STAGE SELECT" cannot be chosen.
    pub no_stage_select: bool,
    /// The frames of its layout since it came up.
    pub clock: f32,
    /// The frames it has been coming, or going, for (the menu's + 0x60).
    pub frames: f32,
    /// The sets it has played and the frame of its clock each was played at: the
    /// one that brings all of it, and the last one played for each choice (a set
    /// played for a choice takes the place of what that choice was playing).
    pub plays: Vec<(usize, f32)>,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
    sounded: bool,
    /// "LOAD" was chosen, until whoever puts the list of saves up has taken that.
    load_asked: bool,
}

impl Failed {
    /// The window as its task makes it (0x42AB4, 0x6EF70 with 0): all of it comes,
    /// with the cursor on the first choice; the volume the tune is to have is put
    /// by, the tune is turned down and the jingle played.
    pub fn new(no_stage_select: bool) -> Failed {
        let sounds = vec![Request::KeepMusicVolume(Keeper::Failed), Request::MusicVolume(TUNE_UNDER_JINGLE, 0.0), Request::Say(JINGLE_STREAM, JINGLE.into())];
        let plays = vec![(set::COMING, 0.0), (set::COMING_CHOSEN[RETRY], 0.0)];
        Failed { phase: Phase::Coming, cursor: RETRY, no_stage_select, clock: 0.0, frames: 0.0, plays, sounds, sounded: false, load_asked: false }
    }

    /// Plays a set for a choice, in place of what the choice was playing.
    fn play(&mut self, sets: [usize; 3], choice: usize) {
        let of_choice = |set: usize| [set::ONTO, set::OFF, set::MADE, set::COMING_CHOSEN].iter().any(|sets| sets[choice] == set);
        self.plays.retain(|&(set, _)| !of_choice(set));
        self.plays.push((sets[choice], self.clock));
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// The cursor goes up or down a choice, round the ends, and past "STAGE
    /// SELECT" when that cannot be chosen (0x6F100, 0x6F20C).
    fn move_cursor(&mut self, down: bool) {
        let from = self.cursor;
        let step = if down { 1 } else { 2 };
        let mut to = (from + step) % 3;
        if self.no_stage_select && to == STAGE_SELECT {
            to = (to + step) % 3;
        }
        self.cursor = to;
        self.play(set::OFF, from);
        self.play(set::ONTO, to);
    }

    /// The choice the cursor is on is made (0x6EF70 with 1): it goes, and the
    /// screen goes dark over sixty frames.
    fn make(&mut self, fade: &mut Fade) {
        fade.restart(true, false, DARK_FRAMES);
        self.play(set::MADE, self.cursor);
        self.frames = 0.0;
        self.phase = Phase::Going;
        self.sound(SOUND_MADE);
    }

    /// Whether "LOAD" has just been chosen: the list of saves is to be put up
    /// (0x4FF8), once.
    pub fn take_load(&mut self) -> bool {
        std::mem::take(&mut self.load_asked)
    }

    /// The list of saves has gone with no game loaded: the window asks again.
    pub fn load_closed(&mut self) {
        if self.phase == Phase::Loading {
            self.phase = Phase::Asking;
        }
    }

    /// What the buttons do while it asks (0x6E854). Cross makes the choice the
    /// cursor is on: but "STAGE SELECT" only if it is to be had, and "LOAD" brings
    /// up the list of saves a game is loaded from (0x4FF8) and waits for it. Up
    /// and down move the cursor, again and again while they are held, though
    /// round the ends only when they are pressed anew.
    fn take_buttons(&mut self, pad: &Pad, fade: &mut Fade) {
        if pad.pressed & button::CROSS != 0 {
            match self.cursor {
                STAGE_SELECT if self.no_stage_select => self.sound(SOUND_REFUSED),
                LOAD => {
                    self.sound(SOUND_MADE);
                    (self.phase, self.load_asked) = (Phase::Loading, true);
                }
                _ => self.make(fade),
            }
            return;
        }
        for (held, down, end) in [(button::UP, false, RETRY), (button::DOWN, true, LOAD)] {
            if pad.held & held == 0 {
                continue;
            }
            if pad.repeat & held != 0 && (self.cursor != end || pad.pressed & held != 0) {
                self.move_cursor(down);
                self.sound(SOUND_CURSOR);
            }
            // Up comes before down when both are held.
            return;
        }
    }

    /// A frame of it: what the buttons do (0x6E854), then its motion (0x6E718).
    /// Answers the choice made, once, when the window has gone.
    pub fn step(&mut self, pad: &Pad, fade: &mut Fade) -> Option<usize> {
        if self.phase == Phase::Asking {
            self.take_buttons(pad, fade);
        }
        let mut gone = None;
        match self.phase {
            Phase::Coming => {
                if !self.sounded && self.frames >= SOUND_AT {
                    self.sounded = true;
                    self.sound(SOUND_COME);
                }
                self.frames += FRAMES_A_STEP;
                if self.clock >= COMING {
                    self.phase = Phase::Asking;
                }
            }
            Phase::Going => {
                // How long the choice has been going is what its set has played.
                let played = self.frames;
                self.frames += FRAMES_A_STEP;
                if played >= GOING && !fade.running() {
                    self.phase = Phase::Gone;
                    // The task's end (0x42CA4): the jingle stops if it is still
                    // heard, and the tune is as loud again as it was to be.
                    self.sounds.extend([Request::Hush(JINGLE_STREAM, 0), Request::MusicVolumeBack(Keeper::Failed)]);
                    gone = Some(self.cursor);
                }
            }
            Phase::Asking | Phase::Loading | Phase::Gone => {}
        }
        if self.phase != Phase::Gone {
            self.clock += FRAMES_A_STEP;
        }
        gone
    }

    /// How much is left of what fades by the menu's own doing as a choice goes
    /// (0x6EE64, 0x6EEB0), so many frames of the layout past its last frame: what
    /// takes sixty frames over it, and what takes fifty.
    pub fn going_shares(&self, ahead: f32) -> (f32, f32) {
        let left = |over: f32| (1.0 - (self.frames + ahead) / over).clamp(0.0, 1.0);
        (left(60.0), left(GOING))
    }
}

impl Scene {
    /// The system script opens the window (0xEE760). The original first lets the
    /// field come to a stop and loads the window's file, which takes it a few
    /// frames; here it is up at once. The screen's fade is cleared (0x6F064).
    pub fn open_failed(&mut self, no_stage_select: bool) {
        self.fade = Fade::CLEAR;
        self.failed = Some(Failed::new(no_stage_select));
    }

    /// A frame of the window of a mission failed, if it is up (0xEE8B0): when it
    /// has gone the choice made is kept where scripts read it and the field is
    /// left for it, as by `s173` (0xEE9E0 to 0xEFA38). It is not a window that a
    /// run which takes windows as read passes over: it waits for its choice.
    pub fn step_failed(&mut self, pad: &Pad) {
        let Some(failed) = &mut self.failed else { return };
        let made = failed.step(pad, &mut self.fade);
        self.sounds.append(&mut failed.sounds);
        if let Some(choice) = made {
            self.values.insert(value::FAILED_CHOICE, choice as i32);
            self.leaving = Some(LEAVING[choice]);
        }
    }

    /// Whether the field stands still under the window of a mission failed: the
    /// original stops the field for it and shows a picture of the screen as it
    /// was (0xF055C, 0xEE73C).
    pub fn held_by_failed(&self) -> bool {
        self.failed.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    fn hold(buttons: u32, repeat: u32) -> Pad {
        Pad { held: buttons, repeat, ..Pad::default() }
    }

    /// Steps the window with nothing pressed until it asks; says how many frames
    /// of the game that took.
    fn come(failed: &mut Failed, fade: &mut Fade) -> usize {
        let mut frames = 0;
        while failed.phase == Phase::Coming {
            assert_eq!(failed.step(&Pad::default(), fade), None);
            frames += 1;
            assert!(frames < 1000);
        }
        frames
    }

    /// A scene with nothing in it.
    fn empty() -> Scene {
        let defaults = super::super::scene::Defaults { touch_radius: 120.0, territory: 2000.0, pace: 95, notice_range: 1800.0, wander_radius: 300.0 };
        Scene::new(0, 0, defaults)
    }

    fn common(sounds: &[Request]) -> Vec<u32> {
        let sound = |request: &Request| match request {
            Request::Play { sound, .. } => Some(*sound),
            _ => None,
        };
        sounds.iter().filter_map(sound).collect()
    }

    #[test]
    fn it_comes_with_its_jingle_and_takes_no_button_until_it_is_up() {
        let mut failed = Failed::new(false);
        let mut fade = Fade::CLEAR;
        assert_eq!(
            std::mem::take(&mut failed.sounds),
            [Request::KeepMusicVolume(Keeper::Failed), Request::MusicVolume(1, 0.0), Request::Say(0, JINGLE.into())]
        );
        assert_eq!(failed.plays, [(set::COMING, 0.0), (set::COMING_CHOSEN[RETRY], 0.0)]);
        // Cross does nothing while it comes: eighty frames of its layout, which the
        // menu sees to have gone by in its forty-first frame.
        for _ in 0..40 {
            assert_eq!(failed.step(&press(button::CROSS | button::DOWN), &mut fade), None);
            assert_eq!((failed.phase, failed.cursor), (Phase::Coming, RETRY));
        }
        assert_eq!(failed.step(&press(button::CROSS), &mut fade), None);
        assert_eq!(failed.phase, Phase::Asking);
        // Its sound came on the way, once.
        assert_eq!(common(&failed.sounds), [SOUND_COME]);
        assert_eq!(come(&mut Failed::new(false), &mut fade), 41);
    }

    #[test]
    fn its_sound_is_heard_once_sixty_one_frames_have_gone_by() {
        let mut failed = Failed::new(false);
        let mut fade = Fade::CLEAR;
        failed.sounds.clear();
        // The count is two a frame of the game, looked at before it is added to.
        for frame in 0..31 {
            failed.step(&Pad::default(), &mut fade);
            assert!(failed.sounds.is_empty(), "nothing yet in frame {frame}");
        }
        failed.step(&Pad::default(), &mut fade);
        assert_eq!(common(&failed.sounds), [SOUND_COME]);
    }

    #[test]
    fn the_cursor_goes_round_only_on_a_new_press() {
        let mut failed = Failed::new(false);
        let mut fade = Fade::CLEAR;
        come(&mut failed, &mut fade);
        failed.sounds.clear();
        failed.step(&press(button::DOWN), &mut fade);
        assert_eq!(failed.cursor, STAGE_SELECT);
        // Held, it comes again when the pad says so, and stops at the last choice.
        failed.step(&hold(button::DOWN, 0), &mut fade);
        assert_eq!(failed.cursor, STAGE_SELECT);
        failed.step(&hold(button::DOWN, button::DOWN), &mut fade);
        assert_eq!(failed.cursor, LOAD);
        failed.step(&hold(button::DOWN, button::DOWN), &mut fade);
        assert_eq!(failed.cursor, LOAD);
        assert_eq!(common(&std::mem::take(&mut failed.sounds)), [SOUND_CURSOR; 2]);
        // Pressed anew it goes round to the first, and up from there to the last.
        failed.step(&press(button::DOWN), &mut fade);
        assert_eq!(failed.cursor, RETRY);
        failed.step(&hold(button::UP, button::UP), &mut fade);
        assert_eq!(failed.cursor, RETRY);
        failed.step(&press(button::UP), &mut fade);
        assert_eq!(failed.cursor, LOAD);
        // Each move has the choice left play its set and the one come onto its.
        let last: Vec<usize> = failed.plays.iter().map(|&(set, _)| set).collect();
        assert_eq!(last, [set::COMING, set::OFF[STAGE_SELECT], set::OFF[RETRY], set::ONTO[LOAD]]);
    }

    #[test]
    fn stage_select_is_passed_over_and_refused_when_it_is_not_to_be_had() {
        let mut failed = Failed::new(true);
        let mut fade = Fade::CLEAR;
        come(&mut failed, &mut fade);
        failed.sounds.clear();
        failed.step(&press(button::DOWN), &mut fade);
        assert_eq!(failed.cursor, LOAD);
        failed.step(&press(button::UP), &mut fade);
        assert_eq!(failed.cursor, RETRY);
        // Were the cursor on it all the same, cross would only sound.
        failed.cursor = STAGE_SELECT;
        failed.sounds.clear();
        assert_eq!(failed.step(&press(button::CROSS), &mut fade), None);
        assert_eq!((failed.phase, common(&failed.sounds)), (Phase::Asking, vec![SOUND_REFUSED]));
    }

    #[test]
    fn load_asks_for_the_list_of_saves_and_asks_again_when_that_has_gone() {
        let mut failed = Failed::new(false);
        let mut fade = Fade::CLEAR;
        come(&mut failed, &mut fade);
        failed.step(&press(button::UP), &mut fade);
        failed.sounds.clear();
        assert!(!failed.take_load());
        assert_eq!(failed.step(&press(button::CROSS), &mut fade), None);
        assert_eq!((failed.phase, failed.cursor, common(&failed.sounds)), (Phase::Loading, LOAD, vec![SOUND_MADE]));
        assert!(!fade.running());
        // It is asked for once, and the window takes nothing while it is up.
        assert!(failed.take_load() && !failed.take_load());
        failed.step(&press(button::UP), &mut fade);
        failed.step(&press(button::CROSS), &mut fade);
        assert_eq!((failed.phase, failed.cursor), (Phase::Loading, LOAD));
        failed.load_closed();
        assert_eq!(failed.phase, Phase::Asking);
        failed.step(&press(button::UP), &mut fade);
        assert_eq!(failed.cursor, STAGE_SELECT);
    }

    #[test]
    fn a_choice_made_goes_as_the_screen_goes_dark_and_the_field_is_left() {
        let mut scene = empty();
        scene.fade = Fade::DARK;
        scene.open_failed(false);
        assert_eq!(scene.fade, Fade::CLEAR);
        while scene.failed.as_ref().unwrap().phase == Phase::Coming {
            scene.step_failed(&Pad::default());
        }
        scene.sounds.clear();
        scene.step_failed(&press(button::DOWN));
        scene.step_failed(&press(button::CROSS));
        let failed = scene.failed.as_ref().unwrap();
        assert_eq!((failed.phase, failed.cursor), (Phase::Going, STAGE_SELECT));
        assert!(scene.fade.running());
        assert_eq!(failed.plays.last().map(|play| play.0), Some(set::MADE[STAGE_SELECT]));
        // The screen takes thirty frames of the game to go dark; then it is gone.
        let mut frames = 1;
        while scene.leaving.is_none() {
            scene.fade.advance();
            scene.step_failed(&press(button::CROSS));
            frames += 1;
            assert!(frames < 100);
        }
        assert!((30..=32).contains(&frames), "{frames}");
        assert_eq!(scene.fade.level, 1.0);
        assert_eq!((scene.leaving, scene.values.get(&value::FAILED_CHOICE)), (Some(3), Some(&1)));
        // The tune is to be as loud again as it was, and the window stays, gone:
        // the script that opened it waits for it for good.
        assert!(scene.sounds.ends_with(&[Request::Hush(0, 0), Request::MusicVolumeBack(Keeper::Failed)]));
        assert_eq!(scene.failed.as_ref().map(|failed| failed.phase), Some(Phase::Gone));
        scene.step_failed(&press(button::CROSS));
        assert_eq!(scene.leaving, Some(3));
    }

    #[test]
    fn it_waits_for_its_choice_and_trying_again_leaves_the_field_for_that() {
        let mut scene = empty();
        scene.open_failed(false);
        for _ in 0..200 {
            scene.fade.advance();
            scene.step_failed(&Pad::default());
        }
        assert_eq!((scene.failed.as_ref().map(|failed| failed.phase), scene.leaving), (Some(Phase::Asking), None));
        scene.step_failed(&press(button::CROSS));
        for _ in 0..40 {
            scene.fade.advance();
            scene.step_failed(&Pad::default());
        }
        assert_eq!((scene.leaving, scene.values.get(&value::FAILED_CHOICE)), (Some(5), Some(&0)));
    }
}
