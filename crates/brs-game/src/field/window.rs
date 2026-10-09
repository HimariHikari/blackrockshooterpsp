//! What a map's script puts up over the field for the player to read and to answer:
//! the window with a text in it, which can ask a question (`s252`, and `s95` with
//! `s75`), and the pictures that fill the screen behind it (`s248` to `s251`). See
//! `docs/FIELD_NOTES.md`.
//!
//! What is here is what the window does (0x6B814, 0x6BEE8); `field/hud.rs` draws it
//! from its layout, by the frames kept here.

use std::collections::VecDeque;

use brs_engine::layout::Layout;

use crate::controls::{button, Pad};
use crate::text::Span;

/// The frames of the window's layout that go by in a frame of the game (0x850).
pub const FRAMES_A_STEP: f32 = 2.0;

/// The sets of the window's layout (`if_rsdsys.lpk/000.lpk`), as the window plays
/// them (0x6BCB0, 0x6BD54): a window that only tells, and one that asks, whose
/// choices come and go by sets of their own.
pub mod set {
    pub const OPEN: usize = 0;
    pub const CLOSE: usize = 1;
    pub const OPEN_ASKING: usize = 2;
    pub const CLOSE_ASKING: usize = 3;
    pub const CHOICES_IN: usize = 4;
    pub const CHOICES_OUT: usize = 5;
}

/// The track of the opening set whose end is when a window that only tells takes a
/// button (0x6B888): the one that brings its text up.
pub const TEXT_TRACK: usize = 6;
/// The group of the choices' set whose track's end is when the choices sound
/// (0x6B900): the bar under the cursor.
pub const CURSOR_GROUP: usize = 12;

// The sounds of the window, of the bank that is always there: as it opens
// (0x6BE38), as the cursor moves (0x6C054), as the choices come up (0x6B940) and
// as it is closed with cross, by the choice made (0x6CE88: the first, or none
// to make, 0x6CEB0; the second, 0x6CEE0; any other closes it without a sound).
// The window has a second way of opening, with sound 7 (0x6BE90), which only the
// task of the title's and the saves' own messages uses (0xCFB9C, for one made
// without its + 0x20), and a sound 5 as a window whose flags 0x80 and 0x100 are
// both set is left the way 0x6BCA8 answers one for (0x6CFC8): no window a
// map's script puts up is either.
const SOUND_OPEN: u32 = 0;
const SOUND_CURSOR: u32 = 2;
const SOUND_CHOICES: u32 = 0x11;
const SOUND_CLOSE: u32 = 3;
const SOUND_CLOSE_SECOND: u32 = 4;

/// The frames a choice the cursor has left takes to dim (0x6BA18).
const LEFT_FRAMES: f32 = 30.0;

/// A choice that is not there: scripts fill the ones they do not use with a space.
const NO_CHOICE: &str = " ";

/// When the window does what, in the frames of its layout: these are the lengths
/// of the layout's own clips.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// A window that only tells takes a button from here on.
    pub ready: f32,
    /// A window that asks is open by here.
    pub open: f32,
    /// The choices are all there by here, and sound at this frame of their coming.
    pub choices: f32,
    pub choices_sound: f32,
    /// The window is gone so long after it is closed: one that only tells, and one
    /// that asks.
    pub close: f32,
    pub close_asking: f32,
}

impl Timing {
    /// What the game's own layout comes to.
    pub const ORIGINAL: Timing = Timing { ready: 22.0, open: 22.0, choices: 42.0, choices_sound: 24.0, close: 20.0, close_asking: 20.0 };

    /// The timing a layout of the window gives.
    pub fn of(layout: &Layout) -> Timing {
        let cursor = layout.track_of(set::CHOICES_IN, CURSOR_GROUP).unwrap_or(0);
        Timing {
            ready: layout.track_length(set::OPEN, TEXT_TRACK),
            open: layout.length(set::OPEN_ASKING),
            choices: layout.length(set::CHOICES_IN),
            choices_sound: layout.track_length(set::CHOICES_IN, cursor),
            close: layout.length(set::CLOSE),
            close_asking: layout.length(set::CLOSE_ASKING),
        }
    }
}

/// A window with a text, and perhaps choices beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// The pages still to be read, the one shown first.
    pub pages: VecDeque<Vec<Span>>,
    pub choices: Vec<String>,
    /// The choice the cursor is on.
    pub selected: usize,
    /// The choice circle puts the cursor on.
    pub cancel: usize,
    /// The lines it is made tall enough for (0x6BB88, 0x6BC5C): those of its whole
    /// text, or its choices if they are more.
    pub lines: usize,
    /// Frames of its layout since it was opened.
    pub frames: f32,
    /// Frames since its choices began to come up: they do with its last page.
    pub choice_frames: f32,
    /// For each choice, the frames since the cursor left it, if it has.
    pub left: Vec<Option<f32>>,
    /// Frames since it began to close.
    pub closing: Option<f32>,
    /// The sounds it has made since they were last taken up.
    pub sounds: Vec<u32>,
    /// It takes buttons, and its choices can be made.
    ready: bool,
    choices_ready: bool,
    choices_sounded: bool,
    timing: Timing,
}

impl Window {
    /// A window with `pages` to read and then `choices` to pick from, of which those
    /// from the first that is not there on are left out. The cursor starts on
    /// `selected`.
    pub fn new(pages: Vec<Vec<Span>>, choices: &[&str], selected: i32, cancel: i32) -> Window {
        let choices: Vec<String> = choices.iter().take_while(|choice| !choice.is_empty() && **choice != NO_CHOICE).map(|choice| choice.to_string()).collect();
        let within = |index: i32| usize::try_from(index).ok().filter(|&index| index < choices.len()).unwrap_or(0);
        let (selected, cancel) = (within(selected), within(cancel));
        // The lines of the text are counted over all its pages (0x22BC4).
        let breaks = pages.iter().flatten().flat_map(|span| span.text.chars()).filter(|character| *character == '\n').count();
        let lines = (breaks + 1).max(choices.len());
        let left = vec![None; choices.len()];
        Window {
            pages: pages.into(),
            choices,
            selected,
            cancel,
            lines,
            frames: 0.0,
            choice_frames: 0.0,
            left,
            closing: None,
            sounds: vec![SOUND_OPEN],
            ready: false,
            choices_ready: false,
            choices_sounded: false,
            timing: Timing::ORIGINAL,
        }
    }

    /// The page that is shown.
    pub fn page(&self) -> &[Span] {
        self.pages.front().map_or(&[], Vec::as_slice)
    }

    /// Whether it is one that asks.
    pub fn asking(&self) -> bool {
        !self.choices.is_empty()
    }

    /// Whether the choices are up: the last page is, and there are any.
    pub fn choosing(&self) -> bool {
        self.pages.len() <= 1 && self.asking()
    }

    /// A frame of it: what the buttons do (0x6BEE8), then its motions (0x6B814).
    /// Cross turns the page; on the last it closes the window, or makes the choice
    /// the cursor is on. Up and down move the cursor, round the ends, and circle
    /// puts it on the choice that changes nothing. Answers the choice made once
    /// the window has gone.
    pub fn step(&mut self, pad: &Pad) -> Option<usize> {
        let asking = self.asking();
        if let Some(closing) = &mut self.closing {
            *closing += FRAMES_A_STEP;
            let length = if asking { self.timing.close_asking } else { self.timing.close };
            return (*closing >= length).then_some(self.selected);
        }

        let pressed = |buttons: u32| pad.pressed & buttons != 0;
        if self.ready {
            if self.pages.len() > 1 {
                if pressed(button::CROSS) {
                    self.pages.pop_front();
                }
            } else if !asking {
                if pressed(button::CROSS) {
                    self.close();
                }
            } else if self.choices_ready {
                let before = self.selected;
                let count = self.choices.len() as i32;
                let moved = (before as i32 + pressed(button::DOWN) as i32 - pressed(button::UP) as i32).rem_euclid(count) as usize;
                if moved != before {
                    self.move_cursor(before, moved);
                }
                if pressed(button::CIRCLE) {
                    self.move_cursor(before, self.cancel);
                } else if pressed(button::CROSS) {
                    self.close();
                }
            }
        }

        self.frames += FRAMES_A_STEP;
        self.ready |= self.frames >= if asking { self.timing.open } else { self.timing.ready };
        for left in self.left.iter_mut().flatten() {
            if *left < LEFT_FRAMES {
                *left += FRAMES_A_STEP;
            }
        }
        if self.choosing() && self.closing.is_none() {
            self.choice_frames += FRAMES_A_STEP;
            if !self.choices_sounded && self.choice_frames >= self.timing.choices_sound {
                self.choices_sounded = true;
                self.sounds.push(SOUND_CHOICES);
            }
            self.choices_ready |= self.choice_frames >= self.timing.choices;
        }
        None
    }

    /// The cursor goes from one choice to another (0x6C028): the one it leaves
    /// starts to dim.
    fn move_cursor(&mut self, from: usize, to: usize) {
        self.left[from] = Some(0.0);
        self.selected = to;
        self.sounds.push(SOUND_CURSOR);
    }

    /// It begins to close (0x6CE54), with the sound of the choice made.
    fn close(&mut self) {
        self.closing = Some(0.0);
        match self.selected {
            0 => self.sounds.push(SOUND_CLOSE),
            1 => self.sounds.push(SOUND_CLOSE_SECOND),
            _ => {}
        }
    }
}

/// How a picture comes or goes (`s250`'s third number).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fade {
    /// It is there at once, and stays whole until it is gone: the screen is faded
    /// over it meanwhile (0x297E4).
    Whole,
    /// It fades in or out itself.
    Own,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Hidden,
    Coming { fade: Fade, frames: i32, elapsed: i32 },
    Shown,
    Going { fade: Fade, frames: i32, elapsed: i32 },
}

/// A picture over the whole screen (the original's object at 0x6B154).
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    /// The file it is, as the script names it (`tut_pit001`).
    pub name: String,
    state: State,
}

impl Picture {
    /// A picture that has been loaded and is not shown yet.
    pub fn new(name: &str) -> Picture {
        Picture { name: name.to_owned(), state: State::Hidden }
    }

    /// `s250(slot, way, kind, frames)`: brings the picture in (way 0) or takes it out
    /// (way 1) over so many frames of the interface (0xD0778). Kind 2 fades the
    /// picture itself; the others fade the screen.
    pub fn fade(&mut self, way: i32, kind: i32, frames: i32) {
        let fade = if kind == 2 { Fade::Own } else { Fade::Whole };
        let frames = frames.max(0);
        match way {
            0 => self.state = State::Coming { fade, frames, elapsed: 0 },
            1 => self.state = State::Going { fade, frames, elapsed: 0 },
            _ => {}
        }
    }

    /// A frame of it (0x6B208): the interface's frames that a frame of the game is
    /// go by.
    pub fn step(&mut self) {
        match &mut self.state {
            State::Coming { frames, elapsed, .. } => {
                *elapsed += FRAMES_A_STEP as i32;
                if *elapsed >= *frames {
                    self.state = State::Shown;
                }
            }
            State::Going { frames, elapsed, .. } => {
                *elapsed += FRAMES_A_STEP as i32;
                if *elapsed >= *frames {
                    self.state = State::Hidden;
                }
            }
            State::Hidden | State::Shown => {}
        }
    }

    /// How much of it shows, from 0 to 1 (0x6B484).
    pub fn alpha(&self) -> f32 {
        let share = |elapsed: i32, frames: i32| if frames > 0 { (elapsed as f32 / frames as f32).clamp(0.0, 1.0) } else { 1.0 };
        match self.state {
            State::Hidden => 0.0,
            State::Shown => 1.0,
            State::Coming { fade: Fade::Whole, .. } | State::Going { fade: Fade::Whole, .. } => 1.0,
            State::Coming { fade: Fade::Own, frames, elapsed } => share(elapsed, frames),
            State::Going { fade: Fade::Own, frames, elapsed } => 1.0 - share(elapsed, frames),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text;

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, ..Pad::default() }
    }

    fn pages(text: &str) -> Vec<Vec<Span>> {
        text::pages(text, &text::Plain)
    }

    /// Lets the window come up, with nothing pressed.
    fn settle(window: &mut Window) {
        for _ in 0..40 {
            assert_eq!(window.step(&Pad::default()), None);
        }
    }

    /// Lets the window close, and says what it answers and after how many frames.
    fn closed(window: &mut Window) -> (usize, usize) {
        (1..100).find_map(|frame| window.step(&Pad::default()).map(|answer| (answer, frame))).expect("the window closes")
    }

    #[test]
    fn a_window_takes_no_button_until_its_text_is_up_and_then_cross_closes_it() {
        let mut window = Window::new(pages("HP has been restored."), &[], 0, 0);
        assert_eq!(std::mem::take(&mut window.sounds), [SOUND_OPEN]);
        // The text is up after 22 frames of the layout, which are 11 of the game's.
        for _ in 0..11 {
            assert_eq!(window.step(&press(button::CROSS)), None);
            assert_eq!(window.closing, None);
        }
        // Holding the button is not pressing it.
        assert_eq!(window.step(&Pad { held: button::CROSS, ..Pad::default() }), None);
        assert_eq!(window.step(&press(button::CIRCLE)), None);
        assert_eq!(window.closing, None);
        assert_eq!(window.step(&press(button::CROSS)), None);
        assert_eq!((window.closing, std::mem::take(&mut window.sounds)), (Some(0.0), vec![SOUND_CLOSE]));
        // It is gone after the 20 frames its closing takes, and answers then.
        assert_eq!(closed(&mut window), (0, 10));
    }

    #[test]
    fn cross_turns_the_pages_before_it_closes_the_window() {
        let mut window = Window::new(pages("one\x0ctwo"), &[], 0, 0);
        settle(&mut window);
        assert_eq!(text::plain(window.page()), "one");
        assert_eq!(window.step(&press(button::CROSS)), None);
        assert_eq!(text::plain(window.page()), "two");
        assert_eq!(window.closing, None);
        assert_eq!(window.step(&press(button::CROSS)), None);
        assert_eq!(closed(&mut window).0, 0);
    }

    #[test]
    fn a_window_is_as_tall_as_the_lines_of_its_text_or_its_choices() {
        assert_eq!(Window::new(pages("one"), &[], 0, 0).lines, 1);
        assert_eq!(Window::new(pages("one\ntwo\nthree"), &[], 0, 0).lines, 3);
        // The lines are counted over the pages together.
        assert_eq!(Window::new(pages("one\ntwo\x0cthree\nfour"), &[], 0, 0).lines, 3);
        assert_eq!(Window::new(pages("one"), &["a", "b", "c", "d"], 0, 0).lines, 4);
    }

    #[test]
    fn a_question_is_answered_with_the_cursor() {
        // The terminal's: the unused choices are spaces, the cursor starts on the
        // first, and circle goes to the second.
        let choices = ["Yes", "No", " ", " ", " ", " ", " ", " ", " "];
        let mut window = Window::new(pages("Save Game Data?"), &choices, 0, 1);
        assert_eq!(window.choices, ["Yes", "No"]);
        assert!(window.choosing());
        window.sounds.clear();
        // Not before the choices are up: they take 42 frames to come, 21 of the
        // game's, and sound on the way.
        for _ in 0..21 {
            assert_eq!(window.step(&press(button::CROSS | button::DOWN)), None);
            assert_eq!((window.closing, window.selected), (None, 0));
        }
        assert_eq!(std::mem::take(&mut window.sounds), [SOUND_CHOICES]);
        assert_eq!(window.step(&press(button::UP)), None);
        assert_eq!(window.selected, 1, "up from the first goes round to the last");
        assert_eq!(window.left, [Some(FRAMES_A_STEP), None], "the choice the cursor left starts to dim");
        assert_eq!(window.step(&press(button::DOWN)), None);
        assert_eq!(window.selected, 0);
        assert_eq!(window.step(&press(button::CIRCLE)), None);
        assert_eq!(window.selected, 1, "circle goes to the choice that changes nothing, and does not make it");
        assert_eq!(std::mem::take(&mut window.sounds), [SOUND_CURSOR; 3]);
        // Circle comes before cross when both go down together.
        window.selected = 0;
        assert_eq!(window.step(&press(button::CIRCLE | button::CROSS)), None);
        assert_eq!(window.closing, None);
        window.sounds.clear();
        assert_eq!(window.step(&press(button::CROSS)), None);
        // The second choice has a sound of its own as the window closes.
        assert_eq!(std::mem::take(&mut window.sounds), [SOUND_CLOSE_SECOND]);
        assert_eq!(closed(&mut window).0, 1);
    }

    #[test]
    fn a_question_asked_after_pages_waits_for_them_to_be_read() {
        let mut window = Window::new(pages("one\x0ctwo"), &["Yes", "No"], 1, 1);
        settle(&mut window);
        assert!(!window.choosing());
        assert_eq!(window.choice_frames, 0.0);
        // The cursor does not move while there is more to read.
        assert_eq!(window.step(&press(button::UP)), None);
        assert_eq!(window.selected, 1);
        assert_eq!(window.step(&press(button::CROSS)), None);
        // The choices come up with the last page, and take their time over it.
        assert!(window.choosing());
        assert_eq!(window.step(&press(button::CROSS)), None);
        assert_eq!(window.closing, None);
        settle(&mut window);
        assert_eq!(window.step(&press(button::CROSS)), None);
        assert_eq!(closed(&mut window).0, 1);
    }

    #[test]
    fn the_timing_is_that_of_the_games_own_layout() {
        use crate::data::GameData;
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = bevy::prelude::Assets::<bevy::prelude::Image>::default();
        let layout = crate::interface::layout(&data, &mut images, crate::field::hud::RESIDENT_PACK_WINDOW, 0).unwrap();
        assert_eq!(Timing::of(&layout), Timing::ORIGINAL);
    }

    #[test]
    fn a_picture_fades_in_and_out_over_the_frames_it_is_given() {
        let mut picture = Picture::new("tut_pit001");
        assert_eq!(picture.alpha(), 0.0);
        // Its frames are the interface's: two of them go by in a frame of the game.
        picture.fade(0, 2, 30);
        assert_eq!(picture.alpha(), 0.0);
        for _ in 0..5 {
            picture.step();
        }
        assert!((picture.alpha() - 1.0 / 3.0).abs() < 1e-6);
        for _ in 0..9 {
            picture.step();
        }
        assert!(picture.alpha() < 1.0);
        picture.step();
        assert_eq!(picture.alpha(), 1.0);
        // It stays until it is taken out.
        picture.step();
        assert_eq!(picture.alpha(), 1.0);
        picture.fade(1, 2, 30);
        for _ in 0..5 {
            picture.step();
        }
        assert!((picture.alpha() - 2.0 / 3.0).abs() < 1e-6);
        for _ in 0..10 {
            picture.step();
        }
        assert_eq!(picture.alpha(), 0.0);
        // A number of frames that is odd is over in the frame of the game it is
        // passed in.
        picture.fade(0, 2, 15);
        for _ in 0..7 {
            picture.step();
        }
        assert!((picture.alpha() - 14.0 / 15.0).abs() < 1e-6);
        picture.step();
        assert_eq!(picture.alpha(), 1.0);
    }

    #[test]
    fn a_picture_brought_in_over_no_frames_is_there_at_once() {
        let mut picture = Picture::new("tut_pit002");
        picture.fade(0, 2, 0);
        assert_eq!(picture.alpha(), 1.0);
        picture.step();
        assert_eq!(picture.alpha(), 1.0);
        // With the screen's fade instead of its own it is whole until it has gone.
        picture.fade(1, 0, 40);
        picture.step();
        assert_eq!(picture.alpha(), 1.0);
        for _ in 0..18 {
            picture.step();
        }
        assert_eq!(picture.alpha(), 1.0);
        picture.step();
        assert_eq!(picture.alpha(), 0.0);
    }
}
