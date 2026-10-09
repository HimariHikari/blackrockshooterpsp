//! ITEMS, the second of the screens the MENU's entries lead to (`menuItemTask`,
//! 0x52DE0): what she carries. See `docs/MENU_NOTES.md`.
//!
//! It has three lists, which L and R go round: what is used up, what she wears
//! and what is kept for the story's sake. Cross on one of the first uses it, if
//! it is one that gives HP back and not all her HP is there; cross on one of
//! the second changes to it; each after the window has asked. On the list of
//! what she wears her model has on what the cursor is on.

use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::Clip;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};
use brs_formats::xtc;

use super::{draw_number, step_cursor, text_place, write_text, Hers, Outcome, View, Wipe, FRAMES_A_STEP, GAUGE_PLACE, LEAVING_FRAMES, WIPE_BETWEEN, WIPE_GOES};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::draw_window_on;
use crate::field::hud::gauge::Gauge;
use crate::field::scene::{fade_kind, Fade};
use crate::field::window::Window;
use crate::interface::{self, layer};
use crate::items::{kind, Class, USE_IN_BATTLES};
use crate::progress::variable;
use crate::record_window::draw_small;
use crate::sound::Request;
use crate::text::{self, TextColours};
use crate::writing::{Fonts, Style};

/// The archive the screen is of (the second of the four named at 0x1711E8).
const PACK: &str = "IF/MENU/MENU_ITEM.GZ";
// Its files (0x541C0, 0x54270, 0x5439C): the pose her model has, its texts, and
// its layout.
const POSE: usize = 0;
const TEXTS: usize = 1;
const LAYOUT: usize = 2;

// Its texts: what the window asks before one is used, and says of one that
// would do nothing; what it asks before a weapon or a suit is changed to; what
// it says of the HP she got back, or of having it all; what it says of the
// weapon or the suit she has changed to; the two answers; and what the strip
// says of each list.
const TEXT_USE: usize = 0;
const TEXT_NO_EFFECT: usize = 1;
const TEXT_CHANGE: [usize; 2] = [2, 3];
const TEXT_HEALED: usize = 4;
const TEXT_HEALED_ALL: usize = 5;
const TEXT_CHANGED: [usize; 2] = [8, 9];
const TEXT_YES: usize = 11;
const TEXT_NO: usize = 12;
const TEXT_OF_LIST: [usize; LISTS] = [13, 14, 15];
const TEXT_COUNT: usize = 16;
/// What is written for a row with no item (0x1670E8): seven dashes.
const NO_ITEM: &str = "\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}";
// The window's cursor begins on the second of its answers, and circle puts it
// there (0x54978 on); the first is the one that has the thing done (0x54A2C).
const ANSWER_YES: usize = 0;
const ANSWER_NO: i32 = 1;

/// The mission in which nothing can be used (0x545C0: the screen's flag 0x800).
const NO_USE_SCENARIO: i32 = 0x42;

// Its sounds, of the bank that is always there: as it has come (0x550DC), as a
// cursor moves or the list is changed (0x56554, 0x547D0), "no" (0x549D8, and
// the window that tells of an item that would do nothing, 0x6BE90), as it is
// left for the MENU or the menus are left (0x53FC4, 0x53FF8), as HP is given
// back (0x54DCC) and as she changes to something (0x55028).
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_REFUSED: u32 = 7;
const SOUND_BACK: u32 = 5;
const SOUND_OUT: u32 = 1;
const SOUND_HEALED: u32 = 0x10;
const SOUND_CHANGED: u32 = 3;
/// The frames it can be used in before its sound is played (0x550B8).
const SOUND_AFTER: i32 = 4;

const LISTS: usize = 3;
const ROWS: usize = 6;
/// The table of items each list is of (0x532DC on).
const CLASSES: [Class; LISTS] = [Class::Consumable, Class::Equipment, Class::Valuable];
/// The list of what she wears.
const WORN: usize = 1;
/// How far one row is under another (0x55F9C).
const ROW_PITCH: f32 = 20.0;
/// What a row of the third list keeps for its item's kind (0x56750).
const KIND_OF_THE_THIRD: u8 = 0xb;

/// The places of the layout's player and the sets they play (0x544E8 on).
mod place {
    /// The screen itself: the list's panel, its title, what an item is.
    pub const SCREEN: usize = 0;
    /// The panels down the right.
    pub const PANELS: usize = 1;
    /// A row: the cursor's, another, and one with no item.
    pub const ROW_CHOSEN: usize = 2;
    pub const ROW_OTHER: usize = 3;
    pub const ROW_EMPTY: usize = 4;
    /// The panel of her attack, for a weapon.
    pub const ATTACK: usize = 5;
}
const SETS_AT_FIRST: [usize; 6] = [1, 0, 2, 3, 4, 5];
// The sets that nudge the arrows by L and by R (0x547A4, 0x54774).
const SET_L: usize = 6;
const SET_R: usize = 7;

/// What the layout's groups are to the code that draws it (0x551A8; the tables
/// at 0x167128, 0x167188 and 0x167220).
mod group {
    pub const SCROLL_BAR: usize = 7;
    pub const SCROLL_KNOB: usize = 8;
    /// Where what the cursor's item is, is written.
    pub const WHAT_IT_IS: usize = 10;
    /// A row: its text, its item's picture, how many she has (each figure over
    /// the group before it), and the mark of what she has on.
    pub const TEXT: usize = 12;
    pub const PICTURE: usize = 13;
    pub const UNDER_COUNT: usize = 14;
    pub const COUNT: usize = 15;
    pub const WORN: usize = 47;
    /// Each list's title and its tab lit, drawn while it is the one shown; and
    /// its tab dark, drawn while it is not.
    pub const TITLE: [usize; 3] = [19, 21, 20];
    pub const TAB_LIT: [usize; 3] = [27, 28, 29];
    pub const TAB_DARK: [usize; 3] = [24, 25, 26];
    /// The panel of her attack: what it is and what it would be, each figure
    /// over a group of its own; the signs of its being more and less; and the
    /// label, by the kind of the cursor's item.
    pub const UNDER_NOW: usize = 34;
    pub const UNDER_THEN: usize = 35;
    pub const NOW: usize = 36;
    pub const THEN: usize = 37;
    pub const MORE: usize = 38;
    pub const LESS: usize = 39;
    pub const LABELS: [(usize, u8); 3] = [(40, 7), (41, 9), (42, 8)];
}
/// How far left the second list's dark tab stands while the third is the one
/// shown (0x555A8).
const SECOND_TAB_MOVES: f32 = -13.0;

// How its texts are written (0x545A4, 0x52F88): as CUSTOMIZE's are.
const ROW_STYLE: Style = Style::of_kind(0, 0x18, 1, 0.0, 0.0);
const WHAT_IT_IS_STYLE: Style = Style::of_kind(0, 0x24, 2, 0.0, 0.0);

/// The picture of the sheet of small ones that an item has beside its name, by
/// its kind, from 1 (0x2D21C on); the sheet's first for any other.
const PICTURES: [usize; 11] = [4, 5, 4, 9, 9, 9, 6, 6, 6, 7, 8];

/// How the screen's own viewer has her model seen (0x54310 to 0x544B8).
pub const VIEW: View = View {
    turns: Vec3::new(13.0, 27.0, -2.0),
    eye: Vec3::new(135.0, 243.0, 363.0),
    looked_at: Vec3::new(-42.0, 77.0, 23.0),
    lens: 20.0,
    near: 100.0,
    far: 3000.0,
    picture: Vec2::new(86.0, -32.0),
};

/// What the screen is drawn from.
pub struct Assets {
    layout: Layout,
    texts: Vec<String>,
    /// The sheet of small pictures (0x2CE5C), which the items' are of, and how
    /// wide the sheet of figures is (0x2CD24).
    small: (Handle<Image>, Vec2),
    figures: f32,
    /// The pose her model has in it.
    pub pose: Arc<Clip>,
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        // Its textures (0x541E4 on): three of the archive, a place left empty,
        // one more of the archive, another empty place, the small pictures
        // (0x2CE5C) and the figures (0x2CD24).
        let textures = [own(5), own(6), own(7), own(5), own(8), own(5), shared(20), shared(9)];
        let table = xtc::Table::parse(&data.read(interface::file(PACK, TEXTS, "xtc"))?).context("the texts of ITEMS")?;
        let text = |index: usize| -> Result<String> {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("ITEMS has no text {index}"))?;
            Ok(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
        };
        Ok(Assets {
            layout: interface::layout_with(data, images, PACK, LAYOUT, &textures)?,
            texts: (0..TEXT_COUNT).map(text).collect::<Result<_>>()?,
            small: interface::load_texture(data, images, &shared(20))?,
            figures: interface::load_texture(data, images, &shared(9))?.1.x,
            pose: data.clip(interface::file(PACK, POSE, "insa"))?,
        })
    }
}

/// The states its task goes through (0x53DD4), by the numbers it has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// It is made, and the wipe is still uncovering it.
    Coming = 0,
    Open = 3,
    /// It is left: for a frame, in which the original sees to its files.
    Left = 6,
    /// Until the wipe has covered it, or the screen has gone dark.
    Leaving = 7,
    Done = 8,
}

/// What it is at (its + 0x18). The original has two more, for a kind of item
/// its tables have none of: one that would end the mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    List = 0,
    /// The window asks whether the cursor's item is to be used, or changed to.
    Asking = 1,
    /// It is.
    Doing = 3,
    /// The window tells what came of it.
    Telling = 4,
    /// The screen is being left for the MENU.
    Back = 6,
    /// All the menus are being left.
    Out = 8,
}

/// One of the six rows of the list.
#[derive(Clone, Debug, Default, PartialEq)]
struct Row {
    /// Its item's name, as it is written; or the dashes.
    text: String,
    item: i32,
    /// How many of it she has.
    count: i32,
    /// Its kind, on the list of what she wears.
    kind: u8,
    /// Which of the small pictures it has.
    picture: usize,
    /// It is the weapon or the suit she has on.
    worn: bool,
}

/// The ITEMS screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemScreen {
    state: State,
    at: At,
    /// Its first motion is over (flag 0x100): it takes the pad.
    ready: bool,
    /// The frames it has been waiting to sound in (+ 0x800), and whether it has
    /// sounded (flag 0x10000).
    waited: i32,
    sounded: bool,
    /// Nothing can be used (flag 0x800).
    no_use: bool,
    places: Vec<Player>,
    /// The frames her gauge has been playing its set for.
    frames: f32,
    /// Which list is shown (+ 0x7E8), and its items (+ 0x3E8).
    list: usize,
    items: Vec<i32>,
    /// The row the cursor is on, and how far the list is scrolled (+ 0x7EC,
    /// + 0x7F0).
    row: usize,
    scroll: usize,
    rows: [Row; ROWS],
    /// What the cursor's item is.
    what_it_is: String,
    /// Her attack, and what it would be with the cursor's item (+ 0x7F8,
    /// + 0x7FC).
    attack: i32,
    attack_then: i32,
    /// What her model has on: the suit and the weapon, by their items.
    tried_on: (i32, i32),
    /// The window, while it is up, and what it answered once it has closed.
    window: Option<Window>,
    answer: Option<usize>,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl ItemScreen {
    /// The screen as it is made (0x5411C).
    pub fn new(assets: &Assets, hers: &mut Hers) -> ItemScreen {
        let mut screen = ItemScreen {
            state: State::Coming,
            at: At::List,
            ready: false,
            waited: 0,
            sounded: false,
            no_use: hers.progress.variable(variable::SCENARIO) == NO_USE_SCENARIO,
            places: SETS_AT_FIRST.iter().map(|&set| assets.layout.player(set)).collect(),
            frames: 0.0,
            list: 0,
            items: Vec::new(),
            row: 0,
            scroll: 0,
            rows: Default::default(),
            what_it_is: String::new(),
            attack: hers.stats.attack,
            attack_then: 0,
            tried_on: (hers.stats.suit, hers.stats.weapon),
            window: None,
            answer: None,
            sounds: Vec::new(),
        };
        screen.build(hers);
        screen.fill_rows(hers);
        screen
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// The item the cursor is on; nought if the list has none there.
    fn chosen(&self) -> i32 {
        self.items.get(self.scroll + self.row).copied().unwrap_or(0)
    }

    /// What the strip along the foot says (0x567C4).
    pub fn strip<'a>(&self, assets: &'a Assets) -> &'a str {
        &assets.texts[TEXT_OF_LIST[self.list]]
    }

    /// Whether her model stands still (0x55164): while the screen is being left.
    pub fn still(&self) -> bool {
        matches!(self.at, At::Back | At::Out)
    }

    /// The suit and the weapon her model has on, by their items (0x534E8): on
    /// the list of what she wears the cursor's item in place of the one of hers
    /// it would take the place of, and otherwise what she has on.
    pub fn tried_on(&self) -> (i32, i32) {
        self.tried_on
    }

    /// Builds the list that is shown (0x53298): one of seven or more is not
    /// scrolled past its last six, and the cursor's row is kept if the list has
    /// an item for it, and is otherwise its last.
    fn build(&mut self, hers: &Hers) {
        self.items = hers.items.carried(CLASSES[self.list], hers.inventory);
        let count = self.items.len();
        if count <= ROWS {
            self.scroll = 0;
        } else if count < self.scroll + ROWS {
            self.scroll = count - ROWS;
        }
        self.row = self.row.min(count.saturating_sub(1));
    }

    /// Fills the rows in (0x5657C): each with its item's name and how many she
    /// has; what the cursor's item is; and, on the list of what she wears, what
    /// her attack would be with it.
    fn fill_rows(&mut self, hers: &Hers) {
        let number = self.chosen();
        if self.list == WORN && hers.stats.weapon != 0 {
            let mut with_it = *hers.stats;
            with_it.equip(hers.items, number);
            self.attack_then = with_it.attack;
        }
        for (index, row) in self.rows.iter_mut().enumerate() {
            *row = Row { text: NO_ITEM.to_owned(), ..Row::default() };
            if index >= self.items.len() {
                continue;
            }
            let number = self.items.get(self.scroll + index).copied().unwrap_or(0);
            let Some(item) = hers.items.get(number) else { continue };
            // One that cannot be used here is written in another colour.
            let only_in_battles = item.class == Class::Consumable && (item.usage == USE_IN_BATTLES || self.no_use);
            row.text = if only_in_battles { format!("\\c(02){}\\c(00)", item.name) } else { item.name.clone() };
            row.item = number;
            row.count = i32::from(hers.inventory.held(number));
            row.kind = match self.list {
                0 => 0,
                WORN => item.kind,
                _ => KIND_OF_THE_THIRD,
            };
            row.picture = PICTURES.get(usize::from(item.kind).wrapping_sub(1)).copied().unwrap_or(0);
            row.worn = number == hers.stats.weapon || number == hers.stats.suit;
        }
        self.what_it_is = hers.items.get(number).filter(|_| number > 0).map_or(String::new(), |item| item.description.clone());
        self.try_on(hers);
    }

    /// Has her model have on what the cursor is on (0x534E8).
    fn try_on(&mut self, hers: &Hers) {
        let worn = (hers.stats.suit, hers.stats.weapon);
        let number = self.chosen();
        self.tried_on = match hers.items.get(number).filter(|_| self.list == WORN).map(|item| item.kind) {
            Some(kind::SUIT) => (number, worn.1),
            Some(kind::WEAPON) => (worn.0, number),
            _ => worn,
        };
    }

    /// A frame of its task (0x53DD4), with the wipe and the screen's fade, which
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
                if self.still() {
                    self.state = State::Left;
                    if self.at == At::Out {
                        fade.begin(fade_kind::OUT, LEAVING_FRAMES);
                    }
                }
            }
            State::Left => {
                self.screen(assets, hers, pad, first);
                if self.at == At::Back {
                    wipe.play(WIPE_BETWEEN);
                    self.sound(SOUND_BACK);
                } else {
                    self.sound(SOUND_OUT);
                    wipe.play(WIPE_GOES);
                }
                self.state = State::Leaving;
            }
            State::Leaving => {
                self.screen(assets, hers, pad, first);
                let back = self.at == At::Back;
                if if back { wipe.gone() } else { !fade.running() } {
                    self.state = State::Done;
                    return Some(if back { Outcome::Back } else { Outcome::Out });
                }
            }
            State::Done => {}
        }
        None
    }

    /// A frame of the screen itself (0x5463C). The pad's buttons that have newly
    /// gone down are for the `first` of the frames a frame of the game's logic
    /// is, and the window has its own then.
    fn screen(&mut self, assets: &Assets, hers: &mut Hers, pad: &Pad, first: bool) {
        if self.state == State::Open {
            if self.ready {
                let (pressed, repeat) = if first { (pad.pressed, pad.repeat) } else { (0, 0) };
                match self.at {
                    At::List => self.at_the_list(assets, hers, pressed, repeat),
                    At::Asking => self.asked(),
                    At::Doing => self.do_it(assets, hers),
                    // Once the window that tells has closed, back to the list.
                    At::Telling => {
                        if self.answer.take().is_some() {
                            self.at = At::List;
                        }
                    }
                    At::Back | At::Out => {}
                }
            } else {
                // It is ready when the cursor's row has come (0x31C20), and sounds
                // on the way if the list has anything.
                self.ready = self.places[place::ROW_CHOSEN].finished(&assets.layout);
                if !self.items.is_empty() && !self.sounded {
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
        self.frames += 1.0;
        if first {
            if let Some(window) = &mut self.window {
                let answer = window.step(pad);
                self.sounds.extend(window.sounds.drain(..).map(Request::common));
                if answer.is_some() {
                    (self.answer, self.window) = (answer, None);
                }
            }
        }
        self.try_on(hers);
    }

    /// What the pad does at the list (0x546D4).
    fn at_the_list(&mut self, assets: &Assets, hers: &mut Hers, pressed: u32, repeat: u32) {
        let layout = &assets.layout;
        if pressed & button::TRIANGLE != 0 {
            self.at = At::Out;
        } else if pressed & button::CIRCLE != 0 {
            self.at = At::Back;
        } else if self.move_cursor(pressed, repeat) {
            self.fill_rows(hers);
        } else if step_cursor(button::L, button::R, &mut self.list, LISTS, pressed, repeat) {
            if repeat & button::R != 0 {
                self.places[place::SCREEN].play(layout, SET_R);
            } else if repeat & button::L != 0 {
                self.places[place::SCREEN].play(layout, SET_L);
            }
            self.build(hers);
            self.fill_rows(hers);
            self.sound(SOUND_CURSOR);
        } else if pressed & button::CROSS != 0 {
            let number = self.chosen();
            let Some(item) = hers.items.get(number).filter(|_| !self.items.is_empty()) else {
                self.sound(SOUND_REFUSED);
                return;
            };
            let named = |text: usize| assets.texts[text].replacen("%s", &self.rows[self.row].text, 1);
            match self.list {
                // One that can be used here: the window asks, or tells that it
                // would do nothing.
                0 if item.used_from_the_menu() && !self.no_use => {
                    if hers.stats.helped_by(item) {
                        let question = named(TEXT_USE);
                        self.ask(assets, &question);
                    } else {
                        let words = named(TEXT_NO_EFFECT);
                        self.tell(&words, SOUND_REFUSED);
                    }
                }
                WORN => {
                    let question = named(match item.kind {
                        kind::WEAPON => TEXT_CHANGE[0],
                        kind::SUIT => TEXT_CHANGE[1],
                        _ => TEXT_USE,
                    });
                    self.ask(assets, &question);
                }
                _ => self.sound(SOUND_REFUSED),
            }
        }
    }

    /// Has the window ask (0x54930 on): with "Yes" and "No", the cursor on "No".
    fn ask(&mut self, assets: &Assets, question: &str) {
        let mut window = Window::new(text::pages(question, &text::Plain), &[&assets.texts[TEXT_YES], &assets.texts[TEXT_NO]], ANSWER_NO, ANSWER_NO);
        self.sounds.extend(window.sounds.drain(..).map(Request::common));
        (self.window, self.answer) = (Some(window), None);
        self.at = At::Asking;
    }

    /// Has the window tell something (0x6BB88), with the sound that goes with it
    /// in place of the window's own (0x6BCB0, 0x6BE90).
    fn tell(&mut self, words: &str, sound: u32) {
        let mut window = Window::new(text::pages(words, &text::Plain), &[], 0, 0);
        window.sounds.clear();
        (self.window, self.answer) = (Some(window), None);
        self.sound(sound);
        self.at = At::Telling;
    }

    /// What follows the window's closing after it has asked (0x54A14).
    fn asked(&mut self) {
        let Some(answer) = self.answer.take() else { return };
        self.at = if answer == ANSWER_YES { At::Doing } else { At::List };
    }

    /// The thing is done (0x54C7C): the cursor's item is used, or changed to,
    /// and the window tells what came of it.
    fn do_it(&mut self, assets: &Assets, hers: &mut Hers) {
        let number = self.chosen();
        let Some(item) = hers.items.get(number).cloned() else {
            self.at = At::List;
            return;
        };
        match self.list {
            0 => {
                let gave = hers.take(number);
                self.build(hers);
                self.fill_rows(hers);
                let words = if hers.stats.hp < hers.stats.max_hp { assets.texts[TEXT_HEALED].replacen("%d", &gave.to_string(), 1) } else { assets.texts[TEXT_HEALED_ALL].clone() };
                self.tell(&words, SOUND_HEALED);
            }
            WORN => {
                hers.equip(number);
                self.build(hers);
                self.attack = hers.stats.attack;
                self.fill_rows(hers);
                let told = match item.kind {
                    kind::WEAPON => TEXT_CHANGED[0],
                    kind::SUIT => TEXT_CHANGED[1],
                    _ => TEXT_USE,
                };
                let words = assets.texts[told].replacen("%s", &self.rows[self.row].text, 1);
                self.tell(&words, SOUND_CHANGED);
            }
            _ => self.at = At::List,
        }
    }

    /// Up and down move the list's cursor (0x563CC), as CUSTOMIZE's.
    fn move_cursor(&mut self, pressed: u32, repeat: u32) -> bool {
        let count = self.items.len();
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

    /// Draws it as it is `ahead` of a frame of the game's logic past its last one
    /// (0x551A8), with her model as the menus' camera has drawn it if it has
    /// (`her`), and her HP and all of it for her gauge. The strip along the
    /// foot, which is drawn last, is the menus'.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, gauge: &Gauge, window: &Layout, fonts: &Fonts, colours: &TextColours, hp: (i32, i32), her: bool, ahead: f32) {
        let layout = &assets.layout;
        let layer = layer::MENU_SCREEN;
        let frames = ahead * FRAMES_A_STEP as f32;
        let on = |place: &Player| {
            let mut place = place.clone();
            place.advance(layout, frames);
            place
        };
        let places: Vec<Player> = self.places.iter().map(on).collect();
        let plain = Look::default();
        let count = self.items.len();
        let height = |group: usize| layout.first_of(group).and_then(|object| layout.objects.objects.get(object)).map_or(1.0, |object| f32::from(object.size[1]));

        layout.draw(canvas, layer, &places[place::PANELS], &plain);
        if her {
            canvas.scene_here(layer);
        }

        // The screen itself, by what each track's group is.
        let screen = &places[place::SCREEN];
        for track in 0..screen.len() {
            let Some(group) = screen.group(track) else { continue };
            let of_a_list = |groups: [usize; LISTS]| groups.iter().position(|&of| of == group);
            if let Some(list) = of_a_list(group::TAB_DARK) {
                // The second list's stands aside for the third's lit one.
                let mut look = plain;
                if list == 1 && self.list == 2 {
                    look.inner = Affine2::from_translation(Vec2::new(SECOND_TAB_MOVES, 0.0));
                }
                if self.list != list {
                    layout.draw_track(canvas, layer, screen, track, &look);
                }
            } else if let Some(list) = of_a_list(group::TAB_LIT).or(of_a_list(group::TITLE)) {
                if self.list == list {
                    layout.draw_track(canvas, layer, screen, track, &plain);
                }
            } else if group == group::WHAT_IT_IS {
                // Written in black (0x52FB4), at so much of the track's alpha as
                // its object has.
                if let Some((at, colour)) = text_place(layout, screen, track, 0.0) {
                    write_text(canvas, layer, fonts, colours, &WHAT_IT_IS_STYLE, &self.what_it_is, at, [0, 0, 0, colour[3]]);
                }
            } else if group == group::SCROLL_BAR || group == group::SCROLL_KNOB {
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
            let player = &places[match filled {
                false => place::ROW_EMPTY,
                true if index == self.row => place::ROW_CHOSEN,
                true => place::ROW_OTHER,
            }];
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let shown = match group {
                    group::TEXT => {
                        if let Some((at, colour)) = text_place(layout, player, track, down) {
                            write_text(canvas, layer, fonts, colours, &ROW_STYLE, &row.text, at, colour);
                        }
                        false
                    }
                    _ if !filled => true,
                    // The item's picture, in its own colours.
                    group::PICTURE => {
                        if let Some((at, _)) = text_place(layout, player, track, down) {
                            draw_small(canvas, layer, &assets.small, row.picture, at, [255; 4]);
                        }
                        false
                    }
                    // How many she has, in two figures at the least.
                    group::COUNT => {
                        draw_number(canvas, layer, layout, assets.figures, player, track, group::UNDER_COUNT, row.count, 2, true, &look);
                        false
                    }
                    group::UNDER_COUNT => false,
                    group::WORN => self.list == WORN && row.worn,
                    _ => true,
                };
                if shown {
                    layout.draw_track(canvas, layer, player, track, &look);
                }
            }
        }

        match self.list {
            // Under the first list, her HP (0x55FC4).
            0 => gauge.draw_on(canvas, layer, &gauge.player(self.frames + frames), hp.0, hp.1, GAUGE_PLACE, 255),
            // Beside a weapon, her attack and what it would be with it.
            WORN if count > 0 && self.rows[self.row].kind == kind::WEAPON => {
                let player = &places[place::ATTACK];
                let (now, then) = (self.attack, self.attack_then);
                let mut thens = 0;
                for track in 0..player.len() {
                    let Some(group) = player.group(track) else { continue };
                    let shown = match group {
                        group::UNDER_NOW | group::UNDER_THEN => false,
                        group::NOW => {
                            draw_number(canvas, layer, layout, assets.figures, player, track, group::UNDER_NOW, now, 1, false, &plain);
                            false
                        }
                        // Of its two tracks the first is for more and the
                        // second for less: for the same there is neither.
                        group::THEN => {
                            thens += 1;
                            if if thens == 1 { now < then } else { then < now } {
                                draw_number(canvas, layer, layout, assets.figures, player, track, group::UNDER_THEN, then, 1, false, &plain);
                            }
                            false
                        }
                        group::MORE => now < then,
                        group::LESS => then < now,
                        _ => group::LABELS.iter().find(|(of, _)| *of == group).is_none_or(|(_, kind)| *kind == self.rows[self.row].kind),
                    };
                    if shown {
                        layout.draw_track(canvas, layer, player, track, &plain);
                    }
                }
            }
            _ => {}
        }

        if let Some(shown) = &self.window {
            draw_window_on(canvas, layer, window, fonts, colours, shown, ahead);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{common, game, press, Game};
    use super::super::{Menu, Screen, State as MenuState};
    use super::*;
    use brs_engine::canvas::Vertex;

    /// Brings the MENU up and chooses its second entry; has ITEMS take the pad.
    fn come(game: &mut Game) -> Menu {
        let mut menu = Menu::default();
        menu.ask(1);
        while !menu.main.as_ref().unwrap().ready {
            game.step(&mut menu, &Pad::default());
        }
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::CROSS));
        let mut steps = 1;
        while !screen(&menu).is_some_and(|screen| screen.ready) {
            assert!(!game.step(&mut menu, &Pad::default()));
            steps += 1;
            assert!(steps < 200);
        }
        menu
    }

    fn screen(menu: &Menu) -> Option<&ItemScreen> {
        match menu.main.as_ref()?.screen.as_ref()? {
            Screen::Items(screen) => Some(screen),
            _ => None,
        }
    }

    /// She is given so many of these items.
    fn carries(game: &mut Game, items: &[(i32, i32)]) {
        for &(item, count) in items {
            game.inventory.add(&game.items, item, count);
        }
    }

    /// Lets the window come, presses the buttons, and has it go.
    fn answer(game: &mut Game, menu: &mut Menu, buttons: &[u32]) {
        for _ in 0..30 {
            game.step(menu, &Pad::default());
        }
        for &button in buttons {
            game.step(menu, &press(button));
        }
        let mut steps = 0;
        while screen(menu).unwrap().window.is_some() {
            game.step(menu, &Pad::default());
            steps += 1;
            assert!(steps < 60);
        }
        game.step(menu, &Pad::default());
    }

    #[test]
    fn its_three_lists_are_what_she_carries_of_each_kind() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        carries(&mut game, &[(1, 3), (14, 1), (6, 2), (130, 1), (257, 1)]);
        let assets = &game.assets.item;
        assert_eq!((assets.texts[TEXT_USE].as_str(), assets.texts[TEXT_NO_EFFECT].as_str()), ("Use \u{201c}\\c(01)%s\\c(00)\u{201d}?", "This item will have no effect\nso you cannot use it."));
        assert_eq!(TEXT_OF_LIST.map(|text| assets.texts[text].as_str()), ["Check and use consumables. \\b(01)Use", "Check and change equipment. \\b(01)Change", "Check rare items."]);
        assert_eq!(assets.layout.length(SETS_AT_FIRST[place::ROW_CHOSEN]), 38.0);

        let mut menu = come(&mut game);
        assert!(common(&std::mem::take(&mut menu.sounds)).ends_with(&[3, SOUND_COME]));
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        // What is used up: the Repairers before the Nano Repair, and the Body
        // Cooler, which is for battles, in another colour.
        assert_eq!((now(&menu).list, now(&menu).items.as_slice()), (0, &[1, 6, 14][..]));
        let rows = now(&menu).rows;
        assert_eq!([rows[0].text.as_str(), rows[1].text.as_str(), rows[2].text.as_str(), rows[3].text.as_str()], ["Repairer 200", "Nano Repair 50%", "\\c(02)Body Cooler\\c(00)", NO_ITEM]);
        assert_eq!((rows[0].count, rows[1].count, rows[0].picture, rows[2].picture), (3, 2, 4, 5));
        assert_eq!(now(&menu).what_it_is, "Heals 200 HP.\nD'dario Next quality.");
        // R goes on to what she wears, with the marks of what she has on, and to
        // what is kept; and round to the first again on a new press.
        game.step(&mut menu, &press(button::R));
        let worn = now(&menu);
        assert_eq!((worn.list, worn.items.as_slice(), worn.strip(&game.assets.item)), (1, &[129, 130, 133][..], "Check and change equipment. \\b(01)Change"));
        assert_eq!((worn.rows[0].worn, worn.rows[1].worn, worn.rows[2].worn, worn.rows[0].kind, worn.rows[2].kind), (true, false, true, kind::WEAPON, kind::SUIT));
        game.step(&mut menu, &press(button::R));
        assert_eq!((now(&menu).list, now(&menu).items.as_slice(), now(&menu).rows[0].kind), (2, &[257][..], KIND_OF_THE_THIRD));
        game.step(&mut menu, &Pad { held: button::R, repeat: button::R, ..Pad::default() });
        assert_eq!(now(&menu).list, 2);
        game.step(&mut menu, &press(button::R));
        assert_eq!(now(&menu).list, 0);
        game.step(&mut menu, &press(button::L));
        assert_eq!(now(&menu).list, 2);
        // Nothing comes of cross on what is kept but "no".
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((common(&std::mem::take(&mut menu.sounds)), now(&menu).at), (vec![SOUND_REFUSED], At::List));
    }

    #[test]
    fn a_repairer_is_used_once_the_window_has_asked() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        carries(&mut game, &[(1, 2), (14, 1)]);
        game.stats.hp = 300.0;
        let mut menu = come(&mut game);
        menu.sounds.clear();
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        // The window asks, with its cursor on "No": which leaves things as they
        // are.
        game.step(&mut menu, &press(button::CROSS));
        let asking = now(&menu).window.unwrap();
        assert_eq!((now(&menu).at, text::plain(asking.page()), asking.selected), (At::Asking, "Use \u{201c}Repairer 200\u{201d}?".to_owned(), 1));
        answer(&mut game, &mut menu, &[button::CROSS]);
        assert_eq!((now(&menu).at, game.stats.hp, game.inventory.held(1)), (At::List, 300.0, 2));
        menu.sounds.clear();
        // "Yes": she has 196 more HP, which is all of it, and one Repairer fewer.
        game.step(&mut menu, &press(button::CROSS));
        for _ in 0..30 {
            game.step(&mut menu, &Pad::default());
        }
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::CROSS));
        while now(&menu).at == At::Asking {
            game.step(&mut menu, &Pad::default());
        }
        game.step(&mut menu, &Pad::default());
        assert_eq!((now(&menu).at, game.stats.hp, game.inventory.held(1), now(&menu).rows[0].count), (At::Telling, 496.0, 1, 1));
        assert_eq!(text::plain(now(&menu).window.unwrap().page()), "BRS's HP healed to maximum.");
        assert!(common(&menu.sounds).ends_with(&[SOUND_HEALED]));
        answer(&mut game, &mut menu, &[button::CROSS]);
        assert_eq!(now(&menu).at, At::List);
        // With all her HP there the last one would do nothing, and the window
        // says so; and what is for battles cannot be used here at all.
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((now(&menu).at, text::plain(now(&menu).window.unwrap().page())), (At::Telling, "This item will have no effect\nso you cannot use it.".to_owned()));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_REFUSED]);
        answer(&mut game, &mut menu, &[button::CROSS]);
        game.step(&mut menu, &press(button::DOWN));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((now(&menu).at, common(&std::mem::take(&mut menu.sounds)), game.inventory.held(1)), (At::List, vec![SOUND_REFUSED], 1));
        // A Repairer that does not make her whole says how much it gave.
        game.stats.hp = 100.0;
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::CROSS));
        answer(&mut game, &mut menu, &[button::UP, button::CROSS]);
        assert_eq!((game.stats.hp, game.inventory.held(1), now(&menu).items.as_slice()), (300.0, 0, &[14][..]));
        assert_eq!(text::plain(now(&menu).window.unwrap().page()), "BRS's HP healed by 200.");
    }

    #[test]
    fn a_weapon_is_changed_to_and_her_attack_shown_before_it_is() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        carries(&mut game, &[(132, 1), (134, 1)]);
        let attack = game.stats.attack;
        let mut menu = come(&mut game);
        game.step(&mut menu, &press(button::R));
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        // On the Rock Cannon she has, her attack would be the same.
        assert_eq!((now(&menu).items.as_slice(), now(&menu).attack, now(&menu).attack_then, now(&menu).tried_on), (&[129, 132, 133, 134][..], attack, attack, (133, 129)));
        // On the Hyperion it would be forty-five more, and her model has it.
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!((now(&menu).attack, now(&menu).attack_then, now(&menu).tried_on), (attack, attack + 45, (133, 132)));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(text::plain(now(&menu).window.unwrap().page()), "Change weapon to \u{201c}Hyperion\u{201d}?");
        answer(&mut game, &mut menu, &[button::UP, button::CROSS]);
        assert_eq!((game.stats.weapon, game.stats.attack, game.stats.burst_bullets, now(&menu).attack, now(&menu).attack_then), (132, attack + 45, 8, attack + 45, attack + 45));
        assert_eq!(text::plain(now(&menu).window.unwrap().page()), "Weapon has been changed to \u{201c}Hyperion\u{201d}.");
        assert!(common(&menu.sounds).ends_with(&[SOUND_CHANGED]));
        assert_eq!(now(&menu).rows.clone().map(|row| row.worn)[..4], [false, true, true, false]);
        answer(&mut game, &mut menu, &[button::CROSS]);
        // The Rock Cannon would be forty-five less now.
        game.step(&mut menu, &press(button::UP));
        assert_eq!((now(&menu).attack_then, now(&menu).tried_on), (attack, (133, 129)));
        // A suit is tried on and changed to likewise.
        game.step(&mut menu, &press(button::UP));
        assert_eq!((now(&menu).row, now(&menu).tried_on), (3, (134, 132)));
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(text::plain(now(&menu).window.unwrap().page()), "Change costume to \u{201c}Lost All\u{201d}?");
        answer(&mut game, &mut menu, &[button::UP, button::CROSS]);
        assert_eq!((game.stats.suit, game.stats.dodge_heat), (134, 10));
        assert_eq!(text::plain(now(&menu).window.unwrap().page()), "Costume has been changed to \u{201c}Lost All\u{201d}.");
    }

    #[test]
    fn circle_goes_back_to_the_menu_and_triangle_out_of_the_menus() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        menu.sounds.clear();
        // Circle: a frame on, the wipe covers it and its sound is heard; then
        // the MENU is back, with the cursor on the entry it was left by.
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_BACK]);
        let main = |menu: &Menu| menu.main.clone().unwrap();
        assert_eq!((screen(&menu).unwrap().state, main(&menu).wipe.set, main(&menu).wipe.frames), (State::Leaving, WIPE_BETWEEN, 1.0));
        let mut steps = 1;
        while screen(&menu).is_some() {
            assert!(!game.step(&mut menu, &press(button::DOWN)));
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!((steps, main(&menu).state, main(&menu).entry), (8, MenuState::Back, 1));

        let mut menu = come(&mut game);
        menu.sounds.clear();
        assert!(!game.step(&mut menu, &press(button::TRIANGLE)));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_OUT]);
        assert_eq!((screen(&menu).unwrap().state, main(&menu).wipe.set), (State::Leaving, WIPE_GOES));
        let mut steps = 1;
        while !game.step(&mut menu, &Pad::default()) {
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 7);
        assert!(!menu.is_open());
    }

    #[test]
    fn it_draws_how_many_she_has_and_what_her_attack_would_be() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        carries(&mut game, &[(1, 3), (2, 5), (132, 1)]);
        let mut menu = come(&mut game);
        for _ in 0..30 {
            game.step(&mut menu, &Pad::default());
        }
        let drawn = |menu: &Menu, game: &Game| -> Canvas {
            let mut canvas = Canvas::default();
            screen(menu).unwrap().draw(&mut canvas, &game.assets.item, &game.assets.gauge, &game.assets.window, &game.fonts, &game.colours, (300, 496), false, 0.0);
            canvas
        };
        // Where the things drawn from a texture are, and the left edge of what
        // each is drawn from.
        let things = |canvas: &Canvas, texture: &Handle<Image>| -> Vec<(Vec2, f32)> {
            let corners = canvas.drawn_from(texture);
            corners.chunks(6).map(|thing: &[Vertex]| thing.iter().fold((thing[0].position, thing[0].uv.x), |(at, across), vertex| (at.min(vertex.position), across.min(vertex.uv.x)))).collect()
        };
        let cell = |digit: f32| digit * 20.0 / 256.0;
        // The first row's three as "03" and the second's five as "05", set to
        // the right, eleven apart; the cursor's row stands fifteen to the right
        // of the others, as in CUSTOMIZE.
        let canvas = drawn(&menu, &game);
        let figures = things(&canvas, &game.assets.item.layout.textures[7]);
        let row = |y: f32| -> Vec<(f32, f32)> { figures.iter().filter(|(at, _)| at.y == y).map(|(at, across)| (at.x, *across)).collect() };
        assert_eq!(row(83.0), [(227.0 + 15.0, cell(0.0)), (238.0 + 15.0, cell(3.0))]);
        assert_eq!(row(103.0), [(227.0, cell(0.0)), (238.0, cell(5.0))]);
        // The items' pictures.
        let pictures: Vec<Vec2> = things(&canvas, &game.assets.item.small.0).iter().map(|(at, _)| *at).collect();
        assert_eq!(pictures, [Vec2::new(37.0 + 15.0, 80.0), Vec2::new(37.0, 100.0)]);

        // On the Hyperion, her attack of 52 and the 97 it would be, from the
        // left of the panel beside the list.
        game.step(&mut menu, &press(button::R));
        game.step(&mut menu, &press(button::DOWN));
        for _ in 0..10 {
            game.step(&mut menu, &Pad::default());
        }
        let canvas = drawn(&menu, &game);
        let figures = things(&canvas, &game.assets.item.layout.textures[7]);
        let row = |y: f32| -> Vec<(f32, f32)> { figures.iter().filter(|(at, _)| at.y == y).map(|(at, across)| (at.x, *across)).collect() };
        assert_eq!(row(202.0), [(390.0, cell(5.0)), (401.0, cell(2.0))]);
        assert_eq!(row(226.0), [(390.0, cell(9.0)), (401.0, cell(7.0))]);
        // On the Rock Cannon she has, there is no second figure.
        game.step(&mut menu, &press(button::UP));
        let canvas = drawn(&menu, &game);
        let figures = things(&canvas, &game.assets.item.layout.textures[7]);
        assert!(figures.iter().any(|(at, _)| at.y == 202.0) && !figures.iter().any(|(at, _)| at.y == 226.0));
    }
}
