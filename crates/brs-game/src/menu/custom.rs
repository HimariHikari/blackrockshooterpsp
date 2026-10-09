//! CUSTOMIZE, the first of the screens the MENU's entries lead to
//! (`menuCustomTask`, 0x4AFE0): her skills are put in her four slots with it, and
//! those that are always in force switched on and off. See `docs/MENU_NOTES.md`.
//!
//! It has two lists, which L and R go between: the skills that go in a slot, and
//! those always in force. Cross on one of the first asks which slot it is to go
//! in, or, if it is in one, whether it is to be taken out; cross on one of the
//! second puts it aside or takes it up again.

use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::Clip;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};
use brs_formats::xtc;

use super::{step_cursor, text_place, write_text, Hers, Outcome, View, Wipe, FRAMES_A_STEP, LEAVING_FRAMES, WIPE_BETWEEN, WIPE_GOES};
use crate::battle::skills;
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::draw_window_on;
use crate::field::scene::{fade_kind, Fade};
use crate::field::window::Window;
use crate::interface::{self, layer};
use crate::record_window::draw_small;
use crate::sound::Request;
use crate::text::{self, TextColours};
use crate::writing::{Fonts, Style};

/// The archive the screen is of (the first of the four named at 0x1711E8).
const PACK: &str = "IF/MENU/MENU_CUSTOM.GZ";
// Its files (0x4BC24, 0x4BD1C, 0x4BE04): the pose her model has, its texts, and
// its layout.
const POSE: usize = 0;
const TEXTS: usize = 1;
const LAYOUT: usize = 2;

// Its texts: what the window asks, with the skill's name for its "%s" (0x4DEF8);
// the two answers; and what the strip says of each list (0x4E470).
const TEXT_QUESTION: usize = 0;
const TEXT_YES: usize = 6;
const TEXT_NO: usize = 7;
const TEXT_OF_LIST: [usize; LISTS] = [8, 9];
const TEXT_COUNT: usize = 10;
/// What is written for a row or a slot with no skill (0x166E60): seven dashes.
const NO_SKILL: &str = "\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}";
// The window's cursor begins on the second of its answers, and circle puts it
// there (0x6BC94 and 0x6BC84 with 1); the first is the one that takes the skill
// out (0x4E1C8).
const ANSWER_YES: usize = 0;
const ANSWER_NO: i32 = 1;

// Its sounds, of the bank that is always there: as it has come (0x4B3B0), as a
// cursor moves or the list is changed (0x4B5C8, 0x4DDA0, 0x4E114), as the menus
// are left (0x4DCD4), as it is left or a slot is not chosen (0x4DDE0, 0x4E0D0),
// and as cross does something (0x4DE94, 0x4E020, 0x4E08C).
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_OUT: u32 = 1;
const SOUND_BACK: u32 = 5;
const SOUND_DONE: u32 = 3;
/// The frames it can be used in before its sound is played (0x4B374).
const SOUND_AFTER: i32 = 20;

const LISTS: usize = 2;
const ROWS: usize = 6;
const SLOTS: usize = 4;
/// What the names' table sorts the skills of each list under (0x4B784, 0x4B7A8).
const CATEGORIES: [u8; LISTS] = [skills::OF_A_SLOT, skills::ALWAYS];
/// How far one row is under another, and one slot (0x4D80C, 0x4DBD0).
const ROW_PITCH: f32 = 20.0;
const SLOT_PITCH: f32 = 18.0;
/// The slots are, from the top, triangle, square, circle and cross (0x4E590 on:
/// her stats 0xB, 0xE, 0xC and 0xD): which of the four her figures keep each is,
/// which are in the order triangle, circle, cross, square.
const SLOT_STATS: [usize; SLOTS] = [0, 3, 1, 2];
/// What a row keeps of the slot its skill is in (0x4E394 on), by her figures'
/// order: 3 for triangle, 1 for circle, 2 for cross, 4 for square. The rows are
/// asked in the slots' order, and a later answer takes the place of an earlier.
const SLOT_MARKS: [(usize, u8); SLOTS] = [(0, 3), (3, 4), (1, 1), (2, 2)];

/// The places of the layout's player, each of which plays a set of its own.
const PLACES: usize = 16;
mod place {
    /// The dark frame down the right.
    pub const FRAME: usize = 0;
    /// The screen itself: the list's panel, its title, what a skill does.
    pub const SCREEN: usize = 1;
    pub const SLOTS_GROUND: usize = 2;
    /// The bar behind each slot: this and the three after it.
    pub const SLOT_BARS: usize = 3;
    pub const SLOTS_SHADE: usize = 7;
    /// The slots' texts and pictures: for the slot the cursor is on, and the others.
    pub const SLOT_CHOSEN: usize = 8;
    pub const SLOT_OTHER: usize = 9;
    /// A row of the first list: the cursor's, another, and one with no skill.
    pub const ROW_CHOSEN: usize = 0xa;
    pub const ROW_OTHER: usize = 0xb;
    pub const ROW_EMPTY: usize = 0xc;
    /// The same of the second list.
    pub const ALWAYS_CHOSEN: usize = 0xd;
    pub const ALWAYS_OTHER: usize = 0xe;
    pub const ALWAYS_EMPTY: usize = 0xf;
}
/// The sets the places play at first (0x4BF24 on).
const SETS_AT_FIRST: [usize; PLACES] = [0, 2, 0xb, 0x15, 0x15, 0x15, 0x15, 0x17, 0xd, 0xd, 3, 4, 5, 6, 7, 8];
/// The sets places 2 to 9 play for each list (0x4B5EC).
const SETS_OF_LIST: [[usize; 8]; LISTS] = [[0xb, 0x15, 0x15, 0x15, 0x15, 0x17, 0xd, 0xd], [0xc, 0x16, 0x16, 0x16, 0x16, 0x18, 0xe, 0xe]];
// The sets that nudge the arrows by L and by R (0x4DD40, 0x4DD70).
const SET_L: usize = 9;
const SET_R: usize = 0xa;
// The sets of choosing a slot (0x4DF9C on) and of having done with it (0x4E12C
// on): of the slots' texts, of the cursor's row and of the other rows; and the
// set the chosen slot's bar flashes with.
const SET_SLOTS_CHOOSING: usize = 0xf;
const SET_SLOTS_CHOSEN: usize = 0x10;
const SET_ROW_CHOOSING: usize = 0x11;
const SET_OTHERS_CHOOSING: usize = 0x12;
const SET_ROW_CHOSEN: usize = 0x13;
const SET_OTHERS_CHOSEN: usize = 0x14;
const SET_BAR_FLASH: usize = 0x19;

/// What the layout's groups are to the code that draws it (0x4C1C8; the tables
/// at 0x166E98 and 0x166F08).
mod group {
    pub const SCROLL_BAR: usize = 18;
    pub const SCROLL_KNOB: usize = 19;
    /// Where what the cursor's skill does is written.
    pub const WHAT_IT_DOES: usize = 21;
    /// Where a row's text is written.
    pub const TEXT: usize = 24;
    /// The picture of a skill that is always in force.
    pub const ALWAYS_PICTURE: usize = 25;
    /// The words OFF and ON of such a skill: each group has two tracks, the
    /// first with its word lit and the second with it dim.
    pub const OFF: usize = 26;
    pub const ON: usize = 27;
    /// Drawn only if the row's skill is in a slot; and the pictures of the
    /// buttons, by what the row keeps of its slot less one.
    pub const SET: usize = 32;
    pub const BUTTONS: [usize; 4] = [33, 34, 35, 36];
    /// The picture of a skill of a slot.
    pub const PICTURE: usize = 37;
    /// Each list's title and its tab lit, drawn while it is the one shown; and
    /// its tab dark, drawn while it is not.
    pub const TITLE: [usize; 2] = [38, 29];
    pub const TAB_LIT: [usize; 2] = [42, 44];
    pub const TAB_DARK: [usize; 2] = [41, 43];
    /// Where the slots' texts are written, and the pictures of their buttons.
    pub const SLOT_TEXT: usize = 58;
    pub const SLOT_BUTTONS: [usize; 4] = [59, 60, 61, 62];
}

// How its texts are written (0x4C034, 0x4C068, 0x4B184): a row's and a slot's on
// one line of twenty-four glyphs, what a skill does on two of thirty-six, with
// nothing between glyphs or lines.
const ROW_STYLE: Style = Style::of_kind(0, 0x18, 1, 0.0, 0.0);
const WHAT_IT_DOES_STYLE: Style = Style::of_kind(0, 0x24, 2, 0.0, 0.0);

/// The picture of the sheet of small ones that a skill has beside its name, by
/// the kind the names' table gives it, from 1 (0x2D3A4 on); the sheet's first
/// for any other.
const PICTURES: [usize; 7] = [0x0a, 0x17, 0x0b, 0x1d, 0x3a, 0x0b, 0x0b];

/// How the menus' viewer has her model seen in it (0x4BDBC to 0x4BEF0).
pub const VIEW: View = View {
    turns: Vec3::new(14.0, 10.0, 8.0),
    eye: Vec3::new(-37.0, 124.0, 119.0),
    looked_at: Vec3::new(-19.0, 91.0, 3.0),
    lens: 65.0,
    near: 10.0,
    far: 500.0,
    picture: Vec2::new(129.0, -21.0),
};

/// What the screen is drawn from.
pub struct Assets {
    layout: Layout,
    texts: Vec<String>,
    /// The sheet of small pictures (0x2CE5C), which the skills' are of.
    small: (Handle<Image>, Vec2),
    /// The pose her model has in it.
    pub pose: Arc<Clip>,
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        // Its textures (0x4BC38 on): four of the archive, the small pictures
        // (0x2CE5C), two more of the archive, two places that are left empty, and
        // one of the pack that is always there (0x2CE28).
        let textures = [own(5), own(6), own(7), own(8), shared(20), own(9), own(10), own(5), own(5), shared(12)];
        let table = xtc::Table::parse(&data.read(interface::file(PACK, TEXTS, "xtc"))?).context("the texts of CUSTOMIZE")?;
        let text = |index: usize| -> Result<String> {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("CUSTOMIZE has no text {index}"))?;
            Ok(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
        };
        Ok(Assets {
            layout: interface::layout_with(data, images, PACK, LAYOUT, &textures)?,
            texts: (0..TEXT_COUNT).map(text).collect::<Result<_>>()?,
            small: interface::load_texture(data, images, &shared(20))?,
            pose: data.clip(interface::file(PACK, POSE, "insa"))?,
        })
    }
}

/// The states its task goes through (0x4B978), by the numbers it has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// It is made, and the wipe is still uncovering it.
    Coming = 0,
    Open = 3,
    /// It is left: until the wipe has covered it, or the screen has gone dark.
    Leaving = 6,
    Done = 7,
}

/// What it is at (its + 0x18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    List = 0,
    /// A slot is being chosen for the cursor's skill.
    Slot = 1,
    /// The window asks whether the cursor's skill is to be taken out of its slot.
    Asking = 2,
    /// It is being left for the MENU.
    Back = 3,
    /// All the menus are being left.
    Out = 5,
}

/// One of the six rows of the list.
#[derive(Clone, Debug, Default, PartialEq)]
struct Row {
    /// Its skill's name, or the dashes.
    text: String,
    /// Which slot its skill is in, by [`SLOT_MARKS`]; nought for none.
    slot: u8,
    /// Its skill is not put aside.
    in_force: bool,
    /// Which of the small pictures its skill has.
    picture: usize,
}

/// The CUSTOMIZE screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Custom {
    state: State,
    at: At,
    /// Its first motion is over (flag 0x100): it takes the pad.
    ready: bool,
    /// The frames it has been waiting to sound in (+ 0x7F4), and whether it has
    /// sounded (flag 0x10000).
    waited: i32,
    sounded: bool,
    places: Vec<Player>,
    /// Which list is shown (+ 0x7E0), and its skills (+ 0x3E0).
    list: usize,
    skills: Vec<i32>,
    /// The row the cursor is on, and how far the list is scrolled (+ 0x7E4,
    /// + 0x7E8).
    row: usize,
    scroll: usize,
    rows: [Row; ROWS],
    /// What the cursor's skill does.
    what_it_does: String,
    /// The slot the cursor is on (+ 0x7F0), the skills in the four, and what is
    /// written for each.
    slot: usize,
    slots: [i32; SLOTS],
    slot_texts: [String; SLOTS],
    /// The window that asks, while it is up, and what it answered once it has
    /// closed.
    window: Option<Window>,
    answer: Option<usize>,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl Custom {
    /// The screen as it is made (0x4BB98).
    pub fn new(assets: &Assets, hers: &mut Hers) -> Custom {
        let mut custom = Custom {
            state: State::Coming,
            at: At::List,
            ready: false,
            waited: 0,
            sounded: false,
            places: SETS_AT_FIRST.iter().map(|&set| assets.layout.player(set)).collect(),
            list: 0,
            skills: Vec::new(),
            row: 0,
            scroll: 0,
            rows: Default::default(),
            what_it_does: String::new(),
            slot: 0,
            slots: [0; SLOTS],
            slot_texts: Default::default(),
            window: None,
            answer: None,
            sounds: Vec::new(),
        };
        custom.build(hers);
        custom.fill_rows(hers);
        for (slot, stat) in SLOT_STATS.into_iter().enumerate() {
            custom.give(hers, slot, i32::from(hers.stats.skills[stat]));
        }
        custom
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// The skill the cursor is on; nought if the list has none there.
    fn chosen(&self) -> i32 {
        self.skills.get(self.scroll + self.row).copied().unwrap_or(0)
    }

    /// What the strip along the foot says (0x4E470).
    pub fn strip<'a>(&self, assets: &'a Assets) -> &'a str {
        &assets.texts[TEXT_OF_LIST[self.list]]
    }

    /// Whether her model stands still (0x4B400): the viewer is not stepped while
    /// the screen is being left.
    pub fn still(&self) -> bool {
        matches!(self.at, At::Back | At::Out)
    }

    /// Builds the list that is shown (0x4B740): the cursor's row is kept if the
    /// list has a skill for it, and is otherwise the last that has one; the list
    /// is shown from its top.
    fn build(&mut self, hers: &Hers) {
        self.skills = hers.skills.listed(CATEGORIES[self.list], hers.progress);
        self.row = self.row.min(self.skills.len().saturating_sub(1));
        self.scroll = 0;
    }

    /// Fills the rows in (0x4E248): each with its skill's name, which slot has it
    /// and whether it is in force; and what the cursor's skill does.
    fn fill_rows(&mut self, hers: &Hers) {
        let about = |skill: i32| hers.skills.about(skill);
        for (index, row) in self.rows.iter_mut().enumerate() {
            *row = Row { text: NO_SKILL.to_owned(), ..Row::default() };
            if index >= self.skills.len() {
                continue;
            }
            let skill = self.skills.get(self.scroll + index).copied().unwrap_or(0);
            row.text = about(skill).map_or(String::new(), |about| about.name.clone());
            row.in_force = !hers.progress.flag(skills::SET + skill);
            for (stat, mark) in SLOT_MARKS {
                if i32::from(hers.stats.skills[stat]) == skill {
                    row.slot = mark;
                }
            }
            let kind = about(skill).map_or(0, |about| about.picture);
            row.picture = PICTURES.get(usize::from(kind).wrapping_sub(1)).copied().unwrap_or(0);
        }
        let skill = self.chosen();
        self.what_it_does = about(skill).filter(|_| skill > 0).map_or(String::new(), |about| about.description.clone());
    }

    /// Gives a slot a skill, or none (0x4E4F0): its text is the skill's name, her
    /// figures have it, and any other slot that had the same skill has none.
    fn give(&mut self, hers: &mut Hers, slot: usize, skill: i32) {
        for other in 0..SLOTS {
            if other == slot {
                self.slots[other] = skill;
            } else if skill != 0 && self.slots[other] == skill {
                self.slots[other] = 0;
            } else {
                continue;
            }
            let now = self.slots[other];
            self.slot_texts[other] = match now {
                0 => NO_SKILL.to_owned(),
                _ => hers.skills.about(now).map_or(String::new(), |about| about.name.clone()),
            };
            hers.set_slot(SLOT_STATS[other], now);
        }
    }

    /// A frame of its task (0x4B978), with the wipe and the screen's fade, which
    /// are the menus'. Answers how it ended, in the frame it does.
    pub fn frame(&mut self, assets: &Assets, hers: &mut Hers, wipe: &mut Wipe, fade: &mut Fade, pad: &Pad, first: bool) -> Option<Outcome> {
        match self.state {
            State::Coming => {
                self.screen(assets, hers, pad, first);
                if !fade.running() && wipe.idle() {
                    self.state = State::Open;
                }
            }
            State::Open => {
                self.screen(assets, hers, pad, first);
                match self.at {
                    At::Back => {
                        wipe.play(WIPE_BETWEEN);
                        self.state = State::Leaving;
                    }
                    At::Out => {
                        wipe.play(WIPE_GOES);
                        fade.begin(fade_kind::OUT, LEAVING_FRAMES);
                        self.state = State::Leaving;
                    }
                    At::List | At::Slot | At::Asking => {}
                }
            }
            State::Leaving => {
                self.screen(assets, hers, pad, first);
                let back = self.at == At::Back;
                if if back { wipe.gone() } else { !fade.running() } {
                    // Her figures are worked out anew as it ends (0x14D008).
                    hers.figure();
                    self.state = State::Done;
                    return Some(if back { Outcome::Back } else { Outcome::Out });
                }
            }
            State::Done => {}
        }
        None
    }

    /// A frame of the screen itself (0x4B2EC). The pad's buttons that have newly
    /// gone down are for the `first` of the frames a frame of the game's logic
    /// is, and the window, whose frames are the game's, has its own then.
    fn screen(&mut self, assets: &Assets, hers: &mut Hers, pad: &Pad, first: bool) {
        if self.state == State::Open {
            if self.ready {
                let (pressed, repeat) = if first { (pad.pressed, pad.repeat) } else { (0, 0) };
                match self.at {
                    At::List => self.at_the_list(assets, hers, pressed, repeat),
                    At::Slot => self.at_the_slots(assets, hers, pressed, repeat),
                    At::Asking => self.asked(hers),
                    At::Back | At::Out => {}
                }
            } else {
                // It is ready when the cursor's row has come (0x31C20), and sounds
                // on the way if the list has anything.
                self.ready = self.places[place::ROW_CHOSEN].finished(&assets.layout);
                if !self.skills.is_empty() && !self.sounded {
                    if self.waited < SOUND_AFTER {
                        self.waited += 1;
                    } else {
                        self.sound(SOUND_COME);
                        (self.waited, self.sounded) = (0, true);
                    }
                }
            }
        }
        for place in &mut self.places {
            place.advance(&assets.layout, 1.0);
        }
        if first {
            if let Some(window) = &mut self.window {
                let answer = window.step(pad);
                self.sounds.extend(window.sounds.drain(..).map(Request::common));
                if answer.is_some() {
                    (self.answer, self.window) = (answer, None);
                }
            }
        }
    }

    /// What the pad does at the list (0x4DCA4).
    fn at_the_list(&mut self, assets: &Assets, hers: &mut Hers, pressed: u32, repeat: u32) {
        let layout = &assets.layout;
        if pressed & button::TRIANGLE != 0 {
            self.at = At::Out;
            self.sound(SOUND_OUT);
        } else if self.move_cursor(pressed, repeat) {
            self.fill_rows(hers);
        } else if step_cursor(button::L, button::R, &mut self.list, LISTS, pressed, repeat) {
            // The arrow by the button that did it is nudged, and the places have
            // the sets of the list that is shown now (0x4B5EC).
            if repeat & button::R != 0 {
                self.places[place::SCREEN].play(layout, SET_R);
            } else if repeat & button::L != 0 {
                self.places[place::SCREEN].play(layout, SET_L);
            }
            for (place, &set) in (place::SLOTS_GROUND..).zip(&SETS_OF_LIST[self.list]) {
                self.places[place].play(layout, set);
            }
            self.build(hers);
            self.fill_rows(hers);
            self.sound(SOUND_CURSOR);
        } else if pressed & button::CIRCLE != 0 {
            self.at = At::Back;
            self.sound(SOUND_BACK);
        } else if pressed & button::CROSS != 0 {
            let skill = self.chosen();
            if self.list != 0 {
                // One always in force is put aside, or taken up again. Nothing
                // asks whether the list has any: with none it is the flag of
                // skill nought that is turned over.
                let aside = !hers.progress.flag(skills::SET + skill);
                hers.put_aside(skill, aside);
                self.rows[self.row].in_force = !aside;
                self.sound(SOUND_DONE);
            } else if self.skills.is_empty() {
            } else if self.slots.contains(&skill) {
                // It is in a slot: the window asks whether it is to come out.
                let name = hers.skills.about(skill).map_or("", |about| about.name.as_str());
                let question = assets.texts[TEXT_QUESTION].replacen("%s", name, 1);
                let mut window = Window::new(text::pages(&question, &text::Plain), &[&assets.texts[TEXT_YES], &assets.texts[TEXT_NO]], ANSWER_NO, ANSWER_NO);
                self.sounds.extend(window.sounds.drain(..).map(Request::common));
                (self.window, self.answer) = (Some(window), None);
                self.at = At::Asking;
            } else {
                // It is not: a slot is chosen for it.
                self.places[place::SLOT_CHOSEN].play(layout, SET_SLOTS_CHOOSING);
                self.play_rows(layout, SET_ROW_CHOOSING, SET_OTHERS_CHOOSING);
                self.at = At::Slot;
                self.sound(SOUND_DONE);
            }
        }
    }

    /// What the pad does while a slot is being chosen (0x4E030).
    fn at_the_slots(&mut self, assets: &Assets, hers: &mut Hers, pressed: u32, repeat: u32) {
        let layout = &assets.layout;
        if pressed & button::CROSS != 0 {
            let skill = self.chosen();
            self.give(hers, self.slot, skill);
            self.fill_rows(hers);
            self.sound(SOUND_DONE);
        } else if pressed & button::CIRCLE != 0 {
            self.sound(SOUND_BACK);
        } else {
            if step_cursor(button::UP, button::DOWN, &mut self.slot, SLOTS, pressed, repeat) {
                self.sound(SOUND_CURSOR);
            }
            return;
        }
        // Either way it is back at the list, and the slot the cursor was on
        // flashes.
        self.at = At::List;
        self.places[place::SLOT_CHOSEN].play(layout, SET_SLOTS_CHOSEN);
        self.places[place::SLOT_BARS + self.slot].play(layout, SET_BAR_FLASH);
        self.play_rows(layout, SET_ROW_CHOSEN, SET_OTHERS_CHOSEN);
    }

    /// What follows the window's closing (0x4E1B0): on its first answer the slot
    /// that has the cursor's skill has none.
    fn asked(&mut self, hers: &mut Hers) {
        let Some(answer) = self.answer.take() else { return };
        if answer == ANSWER_YES {
            let skill = self.chosen();
            if let Some(slot) = self.slots.iter().position(|&of| of == skill) {
                self.give(hers, slot, 0);
                self.fill_rows(hers);
            }
        }
        self.at = At::List;
    }

    /// Has the rows' places play a set each (0x4DFA4, 0x4E148): the cursor's row
    /// one, and the others another if any other row has a skill.
    fn play_rows(&mut self, layout: &Layout, chosen: usize, others: usize) {
        let filled = self.skills.len().min(ROWS);
        if self.row < filled {
            self.places[place::ROW_CHOSEN].play(layout, chosen);
        }
        if (0..filled).any(|row| row != self.row) {
            self.places[place::ROW_OTHER].play(layout, others);
        }
    }

    /// Up and down move the list's cursor (0x4B454): through the six rows and
    /// then by scrolling the list, a step each time the button counts as pressed,
    /// and from either end to the other only when it has newly gone down. Answers
    /// whether it did anything; it is heard if the cursor is somewhere else.
    fn move_cursor(&mut self, pressed: u32, repeat: u32) -> bool {
        let count = self.skills.len();
        if count == 0 {
            return false;
        }
        let before = self.scroll + self.row;
        let moved = if repeat & button::UP != 0 {
            if before > 0 {
                match self.row {
                    0 => self.scroll -= 1,
                    _ => self.row -= 1,
                }
                true
            } else if pressed & button::UP != 0 {
                self.row = (count - 1).min(ROWS - 1);
                self.scroll = count - 1 - self.row;
                true
            } else {
                false
            }
        } else if repeat & button::DOWN != 0 {
            if before < count - 1 {
                match self.row {
                    row if row < ROWS - 1 => self.row += 1,
                    _ => self.scroll += 1,
                }
                true
            } else if pressed & button::DOWN != 0 {
                (self.row, self.scroll) = (0, 0);
                true
            } else {
                false
            }
        } else {
            false
        };
        if moved && self.scroll + self.row != before {
            self.sound(SOUND_CURSOR);
        }
        moved
    }
}

impl Custom {
    /// Draws it as it is `ahead` of a frame of the game's logic past its last one
    /// (0x4C1C8), with her model as the menus' camera has drawn it if it has
    /// (`her`). The strip along the foot, which is drawn last, is the menus'.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, window: &Layout, fonts: &Fonts, colours: &TextColours, her: bool, ahead: f32) {
        let layout = &assets.layout;
        let layer = layer::MENU_SCREEN;
        let on = |place: &Player| {
            let mut place = place.clone();
            place.advance(layout, ahead * FRAMES_A_STEP as f32);
            place
        };
        let places: Vec<Player> = self.places.iter().map(on).collect();
        let plain = Look::default();
        let count = self.skills.len();
        let height = |group: usize| layout.first_of(group).and_then(|object| layout.objects.objects.get(object)).map_or(1.0, |object| f32::from(object.size[1]));

        layout.draw(canvas, layer, &places[place::FRAME], &plain);
        if her {
            canvas.scene_here(layer);
        }

        // The screen itself, by what each track's group is.
        let screen = &places[place::SCREEN];
        for track in 0..screen.len() {
            let Some(group) = screen.group(track) else { continue };
            let of_a_list = |groups: [usize; LISTS]| groups.iter().position(|&of| of == group);
            if let Some(list) = of_a_list(group::TAB_DARK) {
                if self.list != list {
                    layout.draw_track(canvas, layer, screen, track, &plain);
                }
            } else if let Some(list) = of_a_list(group::TAB_LIT).or(of_a_list(group::TITLE)) {
                if self.list == list {
                    layout.draw_track(canvas, layer, screen, track, &plain);
                }
            } else if group == group::WHAT_IT_DOES {
                // Written in black (0x4B1BC), at so much of the track's alpha as
                // its object has.
                if let Some((at, colour)) = text_place(layout, screen, track, 0.0) {
                    write_text(canvas, layer, fonts, colours, &WHAT_IT_DOES_STYLE, &self.what_it_does, at, [0, 0, 0, colour[3]]);
                }
            } else if group == group::SCROLL_BAR || group == group::SCROLL_KNOB {
                // The bar is there only for a list of more than its six rows; its
                // knob is as much of it as six are of the list, and as far down
                // as the list is scrolled.
                if count <= ROWS {
                    continue;
                }
                let mut look = plain;
                if group == group::SCROLL_KNOB {
                    let step = (height(group::SCROLL_BAR) - 2.0) / count as f32;
                    let tall = step * ROWS as f32 / height(group::SCROLL_KNOB);
                    look.inner = Affine2::from_translation(Vec2::new(0.0, step * self.scroll as f32)) * Affine2::from_scale(Vec2::new(1.0, tall));
                }
                layout.draw_track(canvas, layer, screen, track, &look);
            } else {
                layout.draw_track(canvas, layer, screen, track, &plain);
            }
        }

        // The rows, each under the one before.
        for (index, row) in self.rows.iter().enumerate() {
            let down = index as f32 * ROW_PITCH;
            let look = Look::inner(Affine2::from_translation(Vec2::new(0.0, down)));
            let filled = index < count;
            let chosen = index == self.row;
            let player = &places[match (filled, self.list == 0) {
                (false, true) => place::ROW_EMPTY,
                (false, false) => place::ALWAYS_EMPTY,
                (true, true) => if chosen { place::ROW_CHOSEN } else { place::ROW_OTHER },
                (true, false) => if chosen { place::ALWAYS_CHOSEN } else { place::ALWAYS_OTHER },
            }];
            // How many tracks of each of the two words' groups have gone by.
            let mut marks = [0; 2];
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let picture = |canvas: &mut Canvas| {
                    if let Some((at, colour)) = text_place(layout, player, track, down) {
                        draw_small(canvas, layer, &assets.small, row.picture, at, colour);
                    }
                };
                let shown = match group {
                    group::TEXT => {
                        if let Some((at, colour)) = text_place(layout, player, track, down) {
                            write_text(canvas, layer, fonts, colours, &ROW_STYLE, &row.text, at, colour);
                        }
                        false
                    }
                    _ if !filled => true,
                    // A skill of a slot: whether it is in one, and which.
                    group::SET if self.list == 0 => row.slot != 0,
                    group::PICTURE if self.list == 0 => {
                        picture(canvas);
                        false
                    }
                    _ if self.list == 0 => group::BUTTONS.iter().position(|&of| of == group).is_none_or(|button| usize::from(row.slot) == button + 1),
                    // One always in force: of its two words the one that is so
                    // is lit, which is the first of that word's tracks.
                    group::ALWAYS_PICTURE => {
                        picture(canvas);
                        false
                    }
                    group::OFF => {
                        marks[0] += 1;
                        (marks[0] == 1) != row.in_force
                    }
                    group::ON => {
                        marks[1] += 1;
                        (marks[1] == 1) == row.in_force
                    }
                    _ => true,
                };
                if shown {
                    layout.draw_track(canvas, layer, player, track, &look);
                }
            }
        }

        // The slots: the bar behind each, each under the one before; and of the
        // tracks of their texts and pictures, which are apart themselves, those
        // that are the slot's.
        layout.draw(canvas, layer, &places[place::SLOTS_GROUND], &plain);
        for slot in 0..SLOTS {
            let down = Affine2::from_translation(Vec2::new(0.0, slot as f32 * SLOT_PITCH));
            layout.draw(canvas, layer, &places[place::SLOT_BARS + slot], &Look::inner(down));
            let player = &places[if slot == self.slot { place::SLOT_CHOSEN } else { place::SLOT_OTHER }];
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                if group == group::SLOT_TEXT {
                    if player.number_in_group(track) == Some(slot) {
                        if let Some((at, colour)) = text_place(layout, player, track, 0.0) {
                            write_text(canvas, layer, fonts, colours, &ROW_STYLE, &self.slot_texts[slot], at, colour);
                        }
                    }
                } else if group::SLOT_BUTTONS.iter().position(|&of| of == group).is_none_or(|of| of == slot) {
                    layout.draw_track(canvas, layer, player, track, &plain);
                }
            }
        }
        layout.draw(canvas, layer, &places[place::SLOTS_SHADE], &plain);

        if let Some(asking) = &self.window {
            draw_window_on(canvas, layer, window, fonts, colours, asking, ahead);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{common, game, press, Game};
    use super::super::{Menu, Screen, State as MenuState};
    use super::*;
    use brs_engine::canvas::Vertex;

    /// Brings the MENU up and chooses its first entry; answers the frames of the
    /// game's logic it took from cross until CUSTOMIZE took the pad.
    fn come(game: &mut Game) -> (Menu, usize) {
        let mut menu = Menu::default();
        menu.ask(1);
        while !menu.main.as_ref().unwrap().ready {
            game.step(&mut menu, &Pad::default());
        }
        game.step(&mut menu, &press(button::CROSS));
        let mut steps = 1;
        while !custom(&menu).is_some_and(|custom| custom.ready) {
            assert!(!game.step(&mut menu, &Pad::default()));
            steps += 1;
            assert!(steps < 200);
        }
        (menu, steps)
    }

    fn custom(menu: &Menu) -> Option<&Custom> {
        match menu.main.as_ref()?.screen.as_ref()? {
            Screen::Custom(custom) => Some(custom),
            _ => None,
        }
    }

    /// She has these skills.
    fn has(game: &mut Game, skills: &[i32]) {
        for &skill in skills {
            game.progress.set_flag(skills::HAS + skill, true);
        }
    }

    fn drawn(menu: &Menu, game: &Game) -> Vec<Vertex> {
        let mut canvas = Canvas::default();
        if let Some(custom) = custom(menu) {
            custom.draw(&mut canvas, &game.assets.custom, &game.assets.window, &game.fonts, &game.colours, false, 0.0);
        }
        canvas.vertices().to_vec()
    }

    #[test]
    fn cross_on_the_menus_first_entry_brings_it_under_the_wipe() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        has(&mut game, &[1, 23]);
        game.stats.skills = [1, 0, 0, 0];
        let assets = &game.assets.custom;
        assert_eq!(assets.texts[TEXT_QUESTION], "Remove skill \u{201c}\\c(01)%s\\c(00)\u{201d}?");
        assert_eq!((assets.texts[TEXT_YES].as_str(), assets.texts[TEXT_NO].as_str()), ("Yes", "No"));
        assert_eq!(TEXT_OF_LIST.map(|text| assets.texts[text].as_str()), ["\\b(01)Set/Remove", "\\b(01)Switch on/off"]);
        assert_eq!((assets.layout.length(SETS_AT_FIRST[place::ROW_CHOSEN]), game.assets.wipe.length(WIPE_BETWEEN)), (54.0, 35.0));

        let mut menu = Menu::default();
        menu.ask(1);
        while !menu.main.as_ref().unwrap().ready {
            game.step(&mut menu, &Pad::default());
        }
        menu.sounds.clear();
        // Cross is heard and the wipe covers the MENU, which stands still for it.
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [3]);
        let main = |menu: &Menu| menu.main.clone().unwrap();
        assert_eq!((main(&menu).state, main(&menu).wipe.set, main(&menu).wipe.frames), (MenuState::Chosen, WIPE_BETWEEN, 2.0));
        let stood = main(&menu).panel_frames;
        // Fourteen frames of the wipe on, the screen is covered and CUSTOMIZE is
        // made; the MENU is no longer drawn.
        for _ in 0..6 {
            game.step(&mut menu, &press(button::CIRCLE | button::DOWN));
            assert!(main(&menu).state == MenuState::Chosen && custom(&menu).is_none());
        }
        game.step(&mut menu, &Pad::default());
        assert_eq!((main(&menu).state, main(&menu).panel_frames, main(&menu).entry), (MenuState::Screen, stood, 0));
        let made = custom(&menu).unwrap().clone();
        assert_eq!((made.state, made.at, made.skills.as_slice(), made.slots), (State::Coming, At::List, &[1, 23][..], [1, 0, 0, 0]));
        // It takes the pad once the wipe is through and its first row has come,
        // fifty-four of its frames after it was made: in the thirty-fourth frame
        // of the game's logic after cross's.
        let mut steps = 7;
        while !custom(&menu).unwrap().ready {
            game.step(&mut menu, &press(button::CIRCLE | button::DOWN));
            steps += 1;
        }
        assert_eq!(steps, 34);
        assert_eq!((custom(&menu).unwrap().state, custom(&menu).unwrap().row, main(&menu).wipe.playing), (State::Open, 0, false));
        // Its sound came twenty frames after the wipe was through.
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_COME]);
    }

    #[test]
    fn its_lists_are_the_skills_she_has_and_the_cursor_scrolls_the_longer_one() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        has(&mut game, &[1, 2, 3, 7, 10, 14, 18, 22, 20, 23, 66, 30, 33, 42]);
        let (mut menu, _) = come(&mut game);
        menu.sounds.clear();
        let at = |menu: &Menu| (custom(menu).unwrap().scroll, custom(menu).unwrap().row);
        let names = |menu: &Menu| custom(menu).unwrap().rows.clone().map(|row| row.text);
        // The first list, in the table's order for it: War Hammer before Steel
        // Cutter, and Aggressor + after Aggressor.
        assert_eq!(custom(&menu).unwrap().skills, [1, 2, 3, 7, 10, 14, 18, 22, 20, 23, 66]);
        assert_eq!(names(&menu), ["Charge Shot", "Burst Shot", "Photon Shower", "Stun Snipe", "Vulcannon", "Tracer Gun"]);
        assert_eq!(custom(&menu).unwrap().what_it_does, "Delivers a strong penetrating shot.\nCooldown: 20 seconds.");
        // Down goes through the six rows and then scrolls; held, it stops at the
        // end, and a new press goes round to the top.
        let held = |buttons: u32| Pad { held: buttons, repeat: buttons, ..Pad::default() };
        for expected in [(0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5), (5, 5)] {
            game.step(&mut menu, &held(button::DOWN));
            assert_eq!(at(&menu), expected);
        }
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 10]);
        assert_eq!(names(&menu)[2..], ["War Hammer", "Steel Cutter", "Aggressor", "Aggressor +"].map(str::to_owned));
        assert!(custom(&menu).unwrap().what_it_does.starts_with("Boosts ATK and DEF."));
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!(at(&menu), (0, 0));
        // Up from the top goes to the last, the list scrolled to its end, and up
        // from there scrolls back only when the cursor is on the first row.
        game.step(&mut menu, &held(button::UP));
        assert_eq!(at(&menu), (0, 0));
        game.step(&mut menu, &press(button::UP));
        assert_eq!(at(&menu), (5, 5));
        for expected in [(5, 4), (5, 3), (5, 2), (5, 1), (5, 0), (4, 0)] {
            game.step(&mut menu, &held(button::UP));
            assert_eq!(at(&menu), expected);
        }
        menu.sounds.clear();

        // R shows the skills that are always in force; the cursor's row is kept
        // if that list has one for it, and is otherwise its last.
        game.step(&mut menu, &press(button::R));
        let shown = custom(&menu).unwrap().clone();
        assert_eq!((shown.list, shown.skills.as_slice(), at(&menu)), (1, &[30, 33, 42][..], (0, 0)));
        assert_eq!(names(&menu)[..4], ["Aftercare", "Rocket Starter", "ATK Bonus", NO_SKILL].map(str::to_owned));
        assert_eq!((shown.places[place::SCREEN].set(), shown.places[place::SLOTS_GROUND].set(), shown.strip(&game.assets.custom)), (SET_R, 0xc, "\\b(01)Switch on/off"));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR]);
        // Held, R does not go round the two lists; a new press of either does.
        game.step(&mut menu, &held(button::R));
        assert_eq!(custom(&menu).unwrap().list, 1);
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::L));
        let shown = custom(&menu).unwrap().clone();
        assert_eq!((shown.list, at(&menu), shown.places[place::SCREEN].set(), shown.places[place::SLOTS_GROUND].set()), (0, (0, 2), SET_L, 0xb));
        game.step(&mut menu, &press(button::L));
        assert_eq!((custom(&menu).unwrap().list, at(&menu)), (1, (0, 2)));
    }

    #[test]
    fn cross_puts_a_skill_in_a_slot_and_asks_before_taking_one_out() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        has(&mut game, &[1, 3, 23]);
        game.stats.skills = [1, 0, 0, 0];
        let (mut menu, _) = come(&mut game);
        menu.sounds.clear();
        let now = |menu: &Menu| custom(menu).unwrap().clone();
        // Charge Shot is on triangle, the first of the slots.
        assert_eq!((now(&menu).slots, now(&menu).slot_texts[0].as_str(), now(&menu).slot_texts[1].as_str()), ([1, 0, 0, 0], "Charge Shot", NO_SKILL));
        assert_eq!(now(&menu).rows.map(|row| row.slot), [3, 0, 0, 0, 0, 0]);

        // Photon Shower is in none: cross has a slot chosen for it, and down
        // twice puts the slots' cursor on circle, the third.
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((now(&menu).at, now(&menu).places[place::SLOT_CHOSEN].set(), now(&menu).places[place::ROW_CHOSEN].set()), (At::Slot, SET_SLOTS_CHOOSING, SET_ROW_CHOOSING));
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!((now(&menu).slot, now(&menu).row), (2, 1));
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((now(&menu).at, now(&menu).slots, game.stats.skills), (At::List, [1, 0, 3, 0], [1, 3, 0, 0]));
        assert_eq!((now(&menu).rows[1].slot, now(&menu).slot_texts[2].as_str(), now(&menu).places[place::SLOT_BARS + 2].set()), (1, "Photon Shower", SET_BAR_FLASH));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR, SOUND_DONE, SOUND_CURSOR, SOUND_CURSOR, SOUND_DONE]);

        // Aggressor is given the slot Charge Shot is in, which it takes over;
        // then a slot is to be chosen for Charge Shot, and circle thinks better
        // of it.
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::CROSS));
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((now(&menu).slots, game.stats.skills), ([23, 0, 3, 0], [23, 3, 0, 0]));
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::CROSS));
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!((now(&menu).at, now(&menu).slots, now(&menu).slot), (At::List, [23, 0, 3, 0], 2));
        menu.sounds.clear();

        // Photon Shower is in a slot: cross asks, with the cursor on "No", which
        // leaves it there.
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::CROSS));
        let asking = now(&menu).window.unwrap();
        assert_eq!((now(&menu).at, text::plain(asking.page()), asking.choices.clone(), asking.selected), (At::Asking, "Remove skill \u{201c}Photon Shower\u{201d}?".to_owned(), vec!["Yes".to_owned(), "No".to_owned()], 1));
        let answer = |game: &mut Game, menu: &mut Menu, buttons: &[u32]| {
            for _ in 0..30 {
                game.step(menu, &Pad::default());
            }
            for &button in buttons {
                game.step(menu, &press(button));
            }
            let mut steps = 0;
            while custom(menu).unwrap().at == At::Asking {
                game.step(menu, &Pad::default());
                steps += 1;
                assert!(steps < 60);
            }
        };
        answer(&mut game, &mut menu, &[button::CROSS]);
        assert_eq!((now(&menu).slots, now(&menu).window), ([23, 0, 3, 0], None));
        // "Yes" takes it out.
        game.step(&mut menu, &press(button::CROSS));
        answer(&mut game, &mut menu, &[button::UP, button::CROSS]);
        assert_eq!((now(&menu).slots, game.stats.skills, now(&menu).rows[1].slot, now(&menu).slot_texts[2].as_str()), ([23, 0, 0, 0], [23, 0, 0, 0], 0, NO_SKILL));
    }

    #[test]
    fn cross_puts_a_skill_that_is_always_in_force_aside_and_takes_it_up_again() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        has(&mut game, &[1, 54, 42]);
        game.hers().figure();
        let (most_hp, attack) = (game.stats.max_hp, game.stats.attack);
        let (mut menu, _) = come(&mut game);
        game.step(&mut menu, &press(button::R));
        menu.sounds.clear();
        let now = |menu: &Menu| custom(menu).unwrap().clone();
        assert_eq!((now(&menu).skills.as_slice(), now(&menu).rows[0].in_force, now(&menu).rows[1].in_force), (&[42, 54][..], true, true));
        // The HP Bonus is put aside: she has seventy less HP to have at once.
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::CROSS));
        assert!(game.progress.flag(skills::SET + 54) && !now(&menu).rows[1].in_force && now(&menu).rows[0].in_force);
        assert_eq!((game.stats.max_hp, game.stats.attack), (most_hp - 70.0, attack));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR, SOUND_DONE]);
        game.step(&mut menu, &press(button::CROSS));
        assert!(!game.progress.flag(skills::SET + 54) && now(&menu).rows[1].in_force);
        assert_eq!(game.stats.max_hp, most_hp);
    }

    #[test]
    fn circle_goes_back_to_the_menu_and_triangle_out_of_the_menus() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        has(&mut game, &[1]);
        let (mut menu, _) = come(&mut game);
        menu.sounds.clear();
        // Circle: the wipe covers it again, and fourteen frames on the MENU is
        // back, which takes the pad the frame after.
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_BACK]);
        let main = |menu: &Menu| menu.main.clone().unwrap();
        assert_eq!((custom(&menu).unwrap().state, main(&menu).wipe.set, main(&menu).wipe.playing), (State::Leaving, WIPE_BETWEEN, true));
        let mut steps = 1;
        while custom(&menu).is_some() {
            assert!(!game.step(&mut menu, &press(button::DOWN)));
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 8);
        assert_eq!((main(&menu).state, main(&menu).entry, main(&menu).alpha, main(&menu).ready), (MenuState::Open, 0, 255, false));
        game.step(&mut menu, &Pad::default());
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!(main(&menu).entry, 1);
        assert!(menu.is_open() && main(&menu).wipe.playing);

        // Triangle: the screen goes dark as it does when the MENU is left, and
        // the menus are done with.
        let (mut menu, _) = come(&mut game);
        menu.sounds.clear();
        assert!(!game.step(&mut menu, &press(button::TRIANGLE)));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_OUT]);
        assert_eq!((custom(&menu).unwrap().state, main(&menu).wipe.set), (State::Leaving, WIPE_GOES));
        let mut steps = 1;
        while !game.step(&mut menu, &Pad::default()) {
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 7);
        assert!(!menu.is_open());
    }

    #[test]
    fn it_draws_its_rows_its_slots_and_what_the_cursors_skill_does() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        has(&mut game, &[1, 2, 3, 7, 10, 14, 18, 23, 30]);
        game.stats.skills = [1, 23, 0, 0];
        let (mut menu, _) = come(&mut game);
        for _ in 0..30 {
            game.step(&mut menu, &Pad::default());
        }
        let all = drawn(&menu, &game);
        // A skill's picture in each of the six rows, twenty apart; the cursor's
        // row stands fifteen to the right of the others.
        let mut canvas = Canvas::default();
        custom(&menu).unwrap().draw(&mut canvas, &game.assets.custom, &game.assets.window, &game.fonts, &game.colours, false, 0.0);
        let corners = |texture: &Handle<Image>| -> Vec<Vec2> {
            let things = canvas.drawn_from(texture);
            things.chunks(6).map(|thing| thing.iter().fold(thing[0].position, |at, vertex| at.min(vertex.position))).collect()
        };
        assert_eq!(corners(&game.assets.custom.small.0), (0..6).map(|row| Vec2::new(if row == 0 { 52.0 } else { 37.0 }, 80.0 + 20.0 * row as f32)).collect::<Vec<_>>());
        // Of the buttons' pictures, which the layout has of the same sheet: the
        // one of the slot the first row's skill is in beside it, and the four
        // slots' own, eighteen apart.
        let of_layout = corners(&game.assets.custom.layout.textures[4]);
        let beside: Vec<Vec2> = of_layout.iter().copied().filter(|at| (70.0..170.0).contains(&at.y)).collect();
        assert_eq!(beside, [Vec2::new(236.0 + 15.0, 80.0)]);
        let slots: Vec<Vec2> = of_layout.iter().copied().filter(|at| at.y >= 170.0).collect();
        assert_eq!(slots, (0..4).map(|slot| Vec2::new(316.0, 172.0 + 18.0 * slot as f32)).collect::<Vec<_>>());
        // The scroll bar is there for a list of more than six, and what the
        // skill does is written in black.
        assert!(all.iter().any(|vertex| vertex.colour[..3] == [0, 0, 0] && vertex.position.y >= 202.0 && vertex.position.y <= 232.0 && vertex.position.x >= 41.0 && vertex.position.x < 270.0));
        let knob = |menu: &Menu, game: &Game| -> (f32, f32) {
            let all = drawn(menu, game);
            let of_it: Vec<&Vertex> = all.iter().filter(|vertex| (276.0..=278.0).contains(&vertex.position.x) && vertex.position.y > 70.0).collect();
            of_it.iter().fold((f32::MAX, f32::MIN), |(top, foot), vertex| (top.min(vertex.position.y), foot.max(vertex.position.y)))
        };
        // Its knob is six eighths of the bar's 117, the list having eight, and
        // an eighth lower for each row the list is scrolled.
        assert_eq!(knob(&menu, &game), (79.0, 79.0 + 87.75));
        for _ in 0..7 {
            game.step(&mut menu, &press(button::DOWN));
        }
        assert_eq!(custom(&menu).unwrap().scroll, 2);
        assert_eq!(knob(&menu, &game), (79.0 + 29.25, 79.0 + 29.25 + 87.75));
    }
}
