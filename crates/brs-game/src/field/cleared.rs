//! The window of a mission cleared (window 12): MISSION CLEAR over the field, the
//! mission's title, and the challenges that were cleared in it. See
//! `docs/FIELD_NOTES.md`.
//!
//! The field's own part of it is small (0xEDC0C opens it for the mission the
//! save names): it makes the window's task ("ResultFldTask", 0x49F1C), gives it
//! the mission's title (0x4A1DC) and has it list the challenges and come
//! (0x49FC8). The task plays the jingle of a battle won with the tune turned
//! down under it, waits for cross once all of it has come, and goes; the system
//! script that opened it waits until it has gone (0x4A184). What is here is what
//! the task does (0x4A550) and what its interface keeps (0x44D24 to 0x45614);
//! `field/hud/cleared.rs` draws it from its layout, by what is kept here.

use crate::controls::{button, Pad};
use crate::progress::Progress;
use crate::records::{Records, COUNT};
use crate::sound::{Keeper, Request};

/// The frames of the window's layout that go by in a frame of the game (0x850).
pub const FRAMES_A_STEP: f32 = 2.0;

/// The sets of its layout (`if_fldsys.lpk`, files 12 to 14) that its three
/// places play as it comes (0x44EF0) and as it goes (0x44F74): all of it but the
/// list's rows; a row with a challenge; a row without one. And the sets the
/// first place plays as well for a list of no more rows than are shown: what the
/// list's knob runs in is fainter then, and has no knob.
pub mod set {
    pub const COMING: [usize; 3] = [0, 2, 4];
    pub const GOING: [usize; 3] = [1, 3, 5];
    pub const SHORT_LIST_COMING: usize = 6;
    pub const SHORT_LIST_GOING: usize = 7;
}

/// When it does what, in the frames of its layout: all of it has come by here
/// (the rings behind its words are the last to), and it has gone by here. They
/// are what the layout's own clips come to (`field/hud/cleared.rs` checks them
/// against it).
pub const COMING: f32 = 124.0;
pub const GOING: f32 = 32.0;
/// The frames it stands before a run with no one at the buttons takes it as read.
const READ_AFTER: f32 = 120.0;

/// The rows of its list, and how many challenges there have to be for the list
/// to move (0x4523C).
pub const ROWS: usize = 4;
pub const MANY: usize = 5;

/// Its jingle (0x4A66C): the first of `jin_pack.bin`, which is that of a battle
/// won, on the stream the first voice of a scene is heard on at 96 of 127, with
/// the tune turned down to next to nothing (0x4A59C).
pub const JINGLE: &str = "SYSTEM.VOL/jin_pack.bin/000.riff";
const JINGLE_STREAM: usize = 0;
const JINGLE_VOLUME: i32 = 0x60;
const TUNE_UNDER_JINGLE: i32 = 1;

// Its sounds, of the bank that is always there: as cross sends it away
// (0x4A6EC), and as the list moves (0x4A738).
const SOUND_NEXT: u32 = 3;
const SOUND_LIST: u32 = 2;

/// A challenge of its list (0x4518C): its name, and the kind its small picture
/// is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub icon: u8,
}

/// The challenges it lists (0x49FC8): those of the records' list 2, whose
/// counts have gone up, that have come to their goals.
pub fn entries(records: &Records, progress: &Progress) -> Vec<Entry> {
    let listed = records.list(2, progress).into_iter();
    listed
        .filter_map(|number| {
            let record = records.record(number)?;
            (progress.variable(COUNT + number) >= record.goal).then_some(Entry { name: record.name, icon: record.icon })
        })
        .collect()
}

/// The states of its task (0x4A550), by the numbers the game has for them. (Its
/// state 3, in which it would show the window of each challenge in turn, is
/// never set: a map's script asks for those windows itself, `s276`.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Just made: its jingle is to begin.
    Starting = 1,
    Coming = 2,
    /// It takes cross, and up and down for its list.
    Asking = 4,
    Going = 5,
    Gone = 6,
}

/// The window, from when the system script opens it.
#[derive(Clone, Debug, PartialEq)]
pub struct Cleared {
    pub phase: Phase,
    /// The mission's title.
    pub title: String,
    /// The challenges cleared in the mission.
    pub entries: Vec<Entry>,
    /// The first row shown.
    pub top: usize,
    /// The frames of its layout it has played since it came, up to when it was
    /// told to go; and those it has played since.
    pub shown: f32,
    pub going: f32,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
    /// Its list has been given it.
    listed: bool,
}

impl Cleared {
    /// The window as its task makes it (0x4A21C): the volume the tune is to have
    /// is put by.
    pub fn new(title: String) -> Cleared {
        let sounds = vec![Request::KeepMusicVolume(Keeper::Cleared)];
        Cleared { phase: Phase::Starting, title, entries: Vec::new(), top: 0, shown: 0.0, going: 0.0, sounds, listed: false }
    }

    /// Gives it its list (0x49FC8), once: the challenges whose counts have gone
    /// up in the mission and have come to their goals.
    pub fn list(&mut self, entries: impl FnOnce() -> Vec<Entry>) {
        if !std::mem::replace(&mut self.listed, true) {
            self.entries = entries();
        }
    }

    /// Whether its list has more rows than are shown at a time, near enough: only
    /// then does it move, and have its knob.
    pub fn many(&self) -> bool {
        self.entries.len() >= MANY
    }

    /// It is told to go (0x4A140).
    fn leave(&mut self) {
        self.sounds.push(Request::common(SOUND_NEXT));
        (self.phase, self.going) = (Phase::Going, 0.0);
    }

    /// It is taken as read, by a run with no one at the buttons: as cross, once
    /// it has stood for a while. (That is the port's own; a picture taken of such
    /// a run shows it as it stands.)
    pub fn read(&mut self) {
        if self.phase == Phase::Asking && self.shown >= COMING + READ_AFTER {
            self.leave();
        }
    }

    /// What the buttons do while it asks (0x4A6C0). Cross sends it away. Up and
    /// down move a list of five or more a row (0x4520C), again and again while
    /// they are held, though round its ends only when they are pressed anew.
    pub fn take_buttons(&mut self, pad: &Pad) {
        if self.phase != Phase::Asking {
            return;
        }
        if pad.pressed & button::CROSS != 0 {
            self.leave();
            return;
        }
        if !self.many() {
            return;
        }
        let last = self.entries.len() - ROWS;
        let before = self.top;
        // Up comes before down.
        if pad.repeat & button::UP != 0 {
            if self.top > 0 {
                self.top -= 1;
            } else if pad.pressed & button::UP != 0 {
                self.top = last;
            }
        } else if pad.repeat & button::DOWN != 0 {
            if self.top < last {
                self.top += 1;
            } else if pad.pressed & button::DOWN != 0 {
                self.top = 0;
            }
        }
        if self.top != before {
            self.sounds.push(Request::common(SOUND_LIST));
        }
    }

    /// A frame of it (0x4A550), after what the buttons did: says whether it is
    /// still there.
    pub fn step(&mut self) -> bool {
        if self.phase == Phase::Starting {
            self.sounds.extend([Request::MusicVolume(TUNE_UNDER_JINGLE, 0.0), Request::SayAt(JINGLE_STREAM, JINGLE.into(), JINGLE_VOLUME)]);
            self.phase = Phase::Coming;
        }
        match self.phase {
            Phase::Coming if self.shown >= COMING => self.phase = Phase::Asking,
            // Gone, its jingle stops if it is still heard and the tune is as loud
            // again as it was to be (0x4A760).
            Phase::Going if self.going >= GOING => {
                self.phase = Phase::Gone;
                self.sounds.extend([Request::Hush(JINGLE_STREAM, 0), Request::MusicVolumeBack(Keeper::Cleared)]);
            }
            _ => {}
        }
        // Its layout goes on (0x44EA8).
        match self.phase {
            Phase::Going => self.going += FRAMES_A_STEP,
            Phase::Gone => return false,
            _ => self.shown += FRAMES_A_STEP,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    fn hold(buttons: u32) -> Pad {
        Pad { held: buttons, repeat: buttons, ..Pad::default() }
    }

    fn entries(count: usize) -> Vec<Entry> {
        (0..count).map(|number| Entry { name: format!("challenge {number}"), icon: 0 }).collect()
    }

    fn common(sounds: &[Request]) -> Vec<u32> {
        let sound = |request: &Request| match request {
            Request::Play { sound, .. } => Some(*sound),
            _ => None,
        };
        sounds.iter().filter_map(sound).collect()
    }

    /// Steps it with nothing pressed until it asks; says how many frames of the
    /// game that took.
    fn come(cleared: &mut Cleared) -> usize {
        let mut frames = 0;
        while cleared.phase != Phase::Asking {
            cleared.take_buttons(&Pad::default());
            assert!(cleared.step());
            frames += 1;
            assert!(frames < 1000);
        }
        cleared.sounds.clear();
        frames
    }

    #[test]
    fn it_comes_with_its_jingle_and_takes_no_button_until_it_has() {
        let mut cleared = Cleared::new("The Final Weapon Awakens".into());
        assert_eq!(std::mem::take(&mut cleared.sounds), [Request::KeepMusicVolume(Keeper::Cleared)]);
        cleared.take_buttons(&press(button::CROSS));
        assert!(cleared.step());
        assert_eq!(std::mem::take(&mut cleared.sounds), [Request::MusicVolume(1, 0.0), Request::SayAt(0, JINGLE.into(), 0x60)]);
        assert_eq!((cleared.phase, cleared.shown), (Phase::Coming, 2.0));
        // Cross does nothing while it comes: a hundred and twenty-four frames of
        // its layout, which it sees to have gone by in its sixty-third frame.
        for _ in 1..62 {
            cleared.take_buttons(&press(button::CROSS));
            assert!(cleared.step());
            assert_eq!(cleared.phase, Phase::Coming);
        }
        cleared.take_buttons(&press(button::CROSS));
        assert!(cleared.step());
        assert_eq!(cleared.phase, Phase::Asking);
        assert!(cleared.sounds.is_empty());
        assert_eq!(come(&mut Cleared::new(String::new())), 63);
    }

    #[test]
    fn cross_sends_it_away_and_the_tune_is_back_when_it_has_gone() {
        let mut cleared = Cleared::new(String::new());
        come(&mut cleared);
        // It waits for as long as it is left to.
        for _ in 0..200 {
            cleared.take_buttons(&Pad::default());
            assert!(cleared.step());
        }
        assert_eq!(cleared.phase, Phase::Asking);
        let shown = cleared.shown;
        cleared.take_buttons(&press(button::CROSS));
        assert_eq!((cleared.phase, common(&cleared.sounds)), (Phase::Going, vec![SOUND_NEXT]));
        // Its going takes thirty-two frames of its layout, sixteen of the game;
        // what it had played of its coming stays as it was.
        let mut frames = 0;
        while cleared.step() {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!((frames, cleared.phase, cleared.shown), (16, Phase::Gone, shown));
        assert!(cleared.sounds.ends_with(&[Request::Hush(0, 0), Request::MusicVolumeBack(Keeper::Cleared)]));
        // A run with no one at the buttons takes it as read two seconds after it
        // has come.
        let mut cleared = Cleared::new(String::new());
        cleared.read();
        assert_eq!(cleared.phase, Phase::Starting);
        come(&mut cleared);
        for _ in 0..59 {
            cleared.read();
            assert!(cleared.step());
            assert_eq!(cleared.phase, Phase::Asking);
        }
        cleared.read();
        assert_eq!(cleared.phase, Phase::Going);
    }

    #[test]
    fn its_list_is_given_once_and_moves_only_when_it_is_longer_than_what_is_shown() {
        let mut cleared = Cleared::new(String::new());
        cleared.list(|| entries(4));
        cleared.list(|| entries(9));
        assert_eq!(cleared.entries.len(), 4);
        assert!(!cleared.many());
        come(&mut cleared);
        cleared.take_buttons(&press(button::DOWN));
        cleared.take_buttons(&press(button::UP));
        assert_eq!((cleared.top, cleared.sounds.len()), (0, 0));

        let mut cleared = Cleared::new(String::new());
        cleared.list(|| entries(6));
        // Not while it comes.
        cleared.take_buttons(&press(button::DOWN));
        assert_eq!(cleared.top, 0);
        come(&mut cleared);
        cleared.take_buttons(&press(button::DOWN));
        assert_eq!(cleared.top, 1);
        cleared.take_buttons(&hold(button::DOWN));
        assert_eq!(cleared.top, 2);
        // Held, it stops at the last four; pressed anew it goes round.
        cleared.take_buttons(&hold(button::DOWN));
        assert_eq!(cleared.top, 2);
        cleared.take_buttons(&press(button::DOWN));
        assert_eq!(cleared.top, 0);
        cleared.take_buttons(&hold(button::UP));
        assert_eq!(cleared.top, 0);
        cleared.take_buttons(&press(button::UP));
        assert_eq!(cleared.top, 2);
        assert_eq!(common(&cleared.sounds), [SOUND_LIST; 4]);
    }
}
