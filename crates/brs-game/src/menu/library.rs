//! CHALLENGES, the third of the screens the MENU's entries lead to
//! (`menuLibraryTask`, 0x56B50): the records of a stage at a time, sixteen
//! squares in two rows of eight, with what is told of the one the cursor is on.
//! See `docs/MENU_NOTES.md`.
//!
//! It is the screen the record's window is a cut of (`record_window.rs`): its
//! archive is that window's byte for byte but for the motions, and a square and
//! what is told of a record are drawn as there. What it has more are the stages
//! along its top, which L and R go through, how many of the sixteen are cleared,
//! and a cursor that goes over the squares. It changes nothing: cross does
//! nothing in it.

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{Layout, Look, Player};
use brs_formats::xtc;

use super::{figures, step_cursor, text_place, write_text, Hers, Outcome, Wipe, FRAMES_A_STEP, LEAVING_FRAMES, WIPE_BETWEEN, WIPE_GOES};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::scene::{fade_kind, Fade};
use crate::interface::{self, layer};
use crate::record_window::{draw_small_picture, figure, FIGURE_CELL, PAGE};
use crate::records::{flag, COUNT};
use crate::sound::Request;
use crate::text::TextColours;
use crate::writing::{Fonts, Placing, Style};

/// The archive the screen is of (the third of the four named at 0x1711E8).
const PACK: &str = "IF/MENU/MENU_LIBRARY.GZ";
// Its files (0x57700), which are the record window's: its texts, its layout, the
// layout's own eight textures, and the pictures of the ten kinds of record,
// cleared and open.
const TEXTS: usize = 0;
const LAYOUT: usize = 1;
const OWN_TEXTURES: std::ops::RangeInclusive<usize> = 4..=11;
const CLEARED_PICTURES: usize = 12;
const OPEN_PICTURES: usize = 22;
const KINDS: usize = 10;
/// Which of its texts the strip says (0x57A1C).
const TEXT_STRIP: usize = 0;

// Its sounds, of the bank that is always there: as it has come (0x57D40), as
// the cursor moves or the stage is another (0x57C70, 0x57CB0), as the menus are
// left (0x57BB0) and as it is left for the MENU (0x57B70).
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_OUT: u32 = 1;
const SOUND_BACK: u32 = 5;
/// The frames it is open for before its sound is played (0x57D1C).
const SOUND_AFTER: i32 = 26;

/// The flags of the stages the stage select has shown (0x5EE64 sets a stage's the
/// first time its missions are shown): the screen has as many stages as are set
/// one after another from the first (0x56E84).
pub const STAGE_SEEN: i32 = 0x180d;
const STAGES: usize = 6;

/// The squares of a page, which are down the columns: two rows of them.
const SQUARES: usize = PAGE as usize;
const ROWS: usize = 2;
/// How far apart the squares are, across and down (0x59118).
const PITCH: Vec2 = Vec2::new(51.0, 48.0);

/// The places of the layout's player, each of which plays a set of its own
/// (0x57A58): all that is about the squares, what is told of a record that is
/// known and of one that is not, and the sixteen squares.
mod place {
    pub const FRAME: usize = 0;
    pub const KNOWN: usize = 1;
    pub const UNKNOWN: usize = 2;
    pub const SQUARES: usize = 3;
}
// The sets: what the first three places play, the square the cursor is on at
// first and any other, a square the cursor has left and one it has come to
// (0x59A7C), and the arrows nudged by L and by R (0x57C14, 0x57C44).
const SET_FRAME: usize = 0;
const SET_CHOSEN_AT_FIRST: usize = 1;
const SET_SQUARE: usize = 2;
const SET_LEFT: usize = 3;
const SET_COME_TO: usize = 4;
const SET_L: usize = 5;
const SET_R: usize = 6;
const SET_UNKNOWN: usize = 7;
const SET_KNOWN: usize = 8;

/// What the layout's groups are to the code that draws it (0x57E34; the tables
/// at 0x167278, 0x167328 and 0x167360).
mod group {
    // Of all that is about the squares: where the count of those cleared and
    // the total are written, and the stroke between them; the number of the
    // cursor's square in figures, and what stands before it; L's arrow and R's;
    // the six stages' tabs, dark and lit; and the six stages' names.
    pub const COUNT: usize = 23;
    pub const TOTAL: usize = 24;
    pub const STROKE: usize = 25;
    pub const NUMBER: usize = 28;
    pub const NUMBER_LABEL: usize = 29;
    pub const L: usize = 30;
    pub const R: usize = 31;
    pub const TABS_DARK: usize = 48;
    pub const TABS_LIT: usize = 54;
    pub const STAGE_NAMES: usize = 60;
    // Of what is told of a record that is known: where what is to be done, what
    // it is worth and its name are written; the mark of one cleared; its small
    // picture; and for one not cleared the bar of how far its count is, the
    // hundredths of that, and what goes with them.
    pub const TO_CLEAR: usize = 7;
    pub const WORTH: usize = 8;
    pub const CLEARED_MARK: usize = 12;
    pub const SMALL_PICTURE: usize = 13;
    pub const NAME: usize = 16;
    pub const BAR: usize = 17;
    pub const WHILE_COUNTING: [usize; 2] = [18, 20];
    pub const HUNDREDTHS: usize = 19;
    // Of what is told of one that is not: two drawn only for a record there is,
    // and where the kind of thing that opens it is written.
    pub const ONLY_OF_ONE: [usize; 2] = [3, 6];
    pub const HINT: usize = 9;
    // Of a square: its picture; what is over it until it is cleared; what a
    // square with a record has; for one not known the word UNKNOWN and its
    // number; and the frame of one cleared.
    pub const PICTURE: usize = 41;
    pub const UNTIL_CLEARED: usize = 42;
    pub const OF_A_RECORD: usize = 44;
    pub const UNKNOWN_WORD: usize = 45;
    pub const SQUARE_NUMBER: usize = 46;
    pub const CLEARED_FRAME: usize = 47;
}

// How its texts are written (0x56CB4): what is told of a record in the text's
// own font, a line of ninety-six glyphs; the total in the capitals at ten and
// the count of those cleared in the capitals at fifteen.
const WORDS_STYLE: Style = Style::of_kind(0, 0x60, 1, 1.0, 4.0);
const TOTAL_STYLE: Style = Style::of_kind(2, 4, 1, 1.0, 4.0);
const COUNT_STYLE: Style = Style::of_kind(3, 4, 1, 1.0, 4.0);
// What the total and the count are set to the right in (0x572C4, 0x57328): each
// is written so much less its own width to the right of where its group is.
const TOTAL_ROOM: f32 = 10.0;
const COUNT_ROOM: f32 = 15.0;

/// How far a stage's tab, and the arrows either side of them, are moved for each
/// stage there is short of six (0x59B28, 0x58534, 0x585B0), and how far to the
/// left the dark tabs between the first and the lit one stand (0x59D3C).
const TAB_HALF: f32 = 8.0;
const TAB_CLOSED_UP: f32 = -13.0;

/// What the screen is drawn from.
pub struct Assets {
    layout: Layout,
    /// What the strip along the foot says.
    strip: String,
    /// The width of the sheet of figures (0x2CD24), and the sheet of small
    /// pictures (0x2CE5C).
    figures: f32,
    small: (Handle<Image>, Vec2),
    /// The pictures of the kinds of record, open and cleared.
    open: Vec<(Handle<Image>, Vec2)>,
    cleared: Vec<(Handle<Image>, Vec2)>,
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        // The layout's fourteen textures (0x577C8 on): the archive's own eight;
        // the sheet of small pictures; the first of the records' pictures, in
        // whose place each square's own is drawn; and four more of the pack that
        // is always there, the second of them the figures.
        let mut textures: Vec<String> = OWN_TEXTURES.map(own).collect();
        textures.extend([shared(20), own(CLEARED_PICTURES), shared(7), shared(9), shared(12), shared(10)]);
        let table = xtc::Table::parse(&data.read(interface::file(PACK, TEXTS, "xtc"))?).context("the texts of CHALLENGES")?;
        let at = table.record(TEXT_STRIP).and_then(|record| record.get(..4)).context("CHALLENGES has no text for the strip")?;
        let mut pictures = |first: usize| (first..first + KINDS).map(|file| interface::load_texture(data, images, &own(file))).collect::<Result<Vec<_>>>();
        let (cleared, open) = (pictures(CLEARED_PICTURES)?, pictures(OPEN_PICTURES)?);
        Ok(Assets {
            layout: interface::layout_with(data, images, PACK, LAYOUT, &textures)?,
            strip: table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned(),
            figures: interface::load_texture(data, images, &shared(9))?.1.x,
            small: interface::load_texture(data, images, &shared(20))?,
            open,
            cleared,
        })
    }
}

/// The states its task goes through (0x57500), by the numbers it has for them.
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
    Squares = 0,
    /// It is being left for the MENU.
    Back = 3,
    /// All the menus are being left.
    Out = 5,
}

/// What a square shows (0x590C0), and what is told of the cursor's (its + 0x114).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Square {
    /// There is no record for it.
    #[default]
    Empty = 0,
    Unknown = 1,
    Open = 2,
    Cleared = 3,
}

/// What is told of the record the cursor is on (0x56FEC).
#[derive(Clone, Debug, Default, PartialEq)]
struct Told {
    of: Square,
    /// How much of its goal its count has come to (+ 0x110).
    share: f32,
    /// Which of the ten kinds of picture it has.
    icon: u8,
    name: String,
    to_clear: String,
    worth: String,
    hint: String,
}

/// The CHALLENGES screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Library {
    state: State,
    at: At,
    /// The first square's first motion is over (flag 0x100): it takes the pad.
    ready: bool,
    /// The frames it has been waiting to sound in (+ 0xB8), and whether it has
    /// sounded (flag 0x10000).
    waited: i32,
    sounded: bool,
    places: Vec<Player>,
    /// How many stages there are to show (+ 0x108), and the one shown (+ 0x10C).
    stages: usize,
    stage: usize,
    /// The square the cursor is on (+ 0xFC).
    cursor: usize,
    /// The records of the stage shown (+ 0x11C), what each square shows with the
    /// kind of picture it has, and how many of them are cleared (+ 0x118).
    records: Vec<i32>,
    squares: [(Square, u8); SQUARES],
    cleared: usize,
    told: Told,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl Library {
    /// The screen as it is made (0x57700).
    pub fn new(assets: &Assets, hers: &Hers) -> Library {
        let layout = &assets.layout;
        let mut places = vec![layout.player(SET_FRAME), layout.player(SET_KNOWN), layout.player(SET_UNKNOWN), layout.player(SET_CHOSEN_AT_FIRST)];
        places.extend((1..SQUARES).map(|_| layout.player(SET_SQUARE)));
        // The stages (0x56E84): as many as have been seen one after another from
        // the first, and at least one; and the one shown is that of the map she
        // is on, the menus having been come to from the field.
        let stages = (0..STAGES).take_while(|&stage| hers.progress.flag(STAGE_SEEN + stage as i32)).count().max(1);
        let of_the_map = usize::try_from(hers.stage).ok().filter(|&stage| stage < STAGES).unwrap_or(0);
        let mut library = Library {
            state: State::Coming,
            at: At::Squares,
            ready: false,
            waited: 0,
            sounded: false,
            places,
            stages,
            stage: of_the_map.min(stages - 1),
            cursor: 0,
            records: Vec::new(),
            squares: [(Square::Empty, 0); SQUARES],
            cleared: 0,
            told: Told::default(),
            sounds: Vec::new(),
        };
        library.build(hers);
        library.tell(hers);
        library
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// What the strip along the foot says (0x57A2C).
    pub fn strip<'a>(&self, assets: &'a Assets) -> &'a str {
        &assets.strip
    }

    /// Builds the page (0x5714C, 0x571D0): the sixteen records of the stage
    /// shown, or as many as there are; how many of them are cleared; and the
    /// cursor kept, or put on the last of them if there are fewer.
    fn build(&mut self, hers: &Hers) {
        let first = self.stage as i32 * PAGE;
        self.records = (first..(first + PAGE).min(hers.records.count())).collect();
        let cleared = |record: i32| hers.progress.flag(flag::CLEARED + record);
        self.cleared = self.records.iter().filter(|&&record| cleared(record)).count();
        self.squares = [(Square::Empty, 0); SQUARES];
        for (square, &record) in self.squares.iter_mut().zip(&self.records) {
            let shows = match (hers.records.is_open(record, hers.progress), cleared(record)) {
                (false, _) => Square::Unknown,
                (true, false) => Square::Open,
                (true, true) => Square::Cleared,
            };
            *square = (shows, hers.records.record(record).map_or(0, |of| of.icon));
        }
        self.cursor = self.cursor.min(self.records.len().saturating_sub(1));
    }

    /// What is told of the cursor's record (0x56FEC). Of one that is open: what
    /// is to be done, its name and what it is worth, and how much of its goal
    /// its count has come to, which is all of it once it is cleared. Of one that
    /// is not: the kind of thing that opens it. Of a square with none, nothing.
    fn tell(&mut self, hers: &Hers) {
        self.told = Told::default();
        let Some(&number) = self.records.get(self.cursor) else { return };
        let Some(record) = hers.records.record(number) else { return };
        let [to_clear, hint, worth] = record.words;
        self.told = match self.squares[self.cursor].0 {
            Square::Cleared => Told { of: Square::Cleared, share: 1.0, icon: record.icon, name: record.name, to_clear, worth, hint: String::new() },
            Square::Open => {
                let share = hers.progress.variable(COUNT + number) as f32 / record.goal as f32;
                Told { of: Square::Open, share, icon: record.icon, name: record.name, to_clear, worth, hint: String::new() }
            }
            _ => Told { of: Square::Unknown, icon: record.icon, hint, ..Told::default() },
        };
    }

    /// A frame of its task (0x57500), with the wipe and the screen's fade, which
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
                    At::Squares => {}
                }
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

    /// A frame of the screen itself (0x57B28). The pad's buttons that have newly
    /// gone down are for the `first` of the frames a frame of the game's logic
    /// is.
    fn screen(&mut self, assets: &Assets, hers: &Hers, pad: &Pad, first: bool) {
        if self.state == State::Open {
            if self.ready {
                let (pressed, repeat) = if first { (pad.pressed, pad.repeat) } else { (0, 0) };
                self.at_the_squares(assets, hers, pressed, repeat);
            } else {
                // It is ready when the first square has come (0x31C20), and
                // sounds on the way if the page has any record.
                self.ready = self.places[place::SQUARES].finished(&assets.layout);
                if !self.records.is_empty() && !self.sounded {
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
    }

    /// What the pad does (0x57B58).
    fn at_the_squares(&mut self, assets: &Assets, hers: &Hers, pressed: u32, repeat: u32) {
        let layout = &assets.layout;
        if pressed & button::CIRCLE != 0 {
            self.at = At::Back;
            self.sound(SOUND_BACK);
            return;
        }
        if pressed & button::TRIANGLE != 0 {
            self.at = At::Out;
            self.sound(SOUND_OUT);
            return;
        }
        let shown = self.stage;
        if step_cursor(button::L, button::R, &mut self.stage, self.stages, pressed, repeat) {
            // The arrow by the button that did it is nudged, and the page is
            // that of the stage shown now: which is heard if it is another.
            if repeat & button::R != 0 {
                self.places[place::FRAME].play(layout, SET_R);
            } else if repeat & button::L != 0 {
                self.places[place::FRAME].play(layout, SET_L);
            }
            self.build(hers);
            self.tell(hers);
            if self.stage != shown {
                self.sound(SOUND_CURSOR);
            }
        } else if self.move_cursor(layout, pressed, repeat) {
            self.tell(hers);
            self.sound(SOUND_CURSOR);
        }
    }

    /// The pad moves the cursor over the squares (0x5971C), which are numbered
    /// down the columns: left and right by a column each time the button counts
    /// as pressed, and from an end column to the same row of the other only
    /// when it has newly gone down; up and down between the two rows, towards
    /// the row the button names each time it counts as pressed and away from it
    /// only on a new press. Left or right and up or down are both taken in a
    /// frame. The square it has left and the one it has come to play a set
    /// each. Answers whether anything was done, which it can be with the cursor
    /// where it was.
    fn move_cursor(&mut self, layout: &Layout, pressed: u32, repeat: u32) -> bool {
        let count = self.records.len() as i32;
        if count <= 0 {
            return false;
        }
        let rows = ROWS as i32;
        // The count made even: a column that has one record has both squares.
        let even = count + count % rows;
        let before = self.cursor as i32;
        let (mut cursor, mut moved) = (before, false);
        if repeat & button::LEFT != 0 {
            if cursor - rows >= 0 {
                (cursor, moved) = (cursor - rows, true);
            } else if pressed & button::LEFT != 0 {
                let back = rows - cursor % rows;
                (cursor, moved) = ((even - back).max(0).min(PAGE - back), true);
            }
        } else if repeat & button::RIGHT != 0 {
            if cursor + rows < even {
                (cursor, moved) = (cursor + rows, true);
            } else if pressed & button::RIGHT != 0 {
                (cursor, moved) = (cursor % rows, true);
            }
        }
        let lower = cursor % rows != 0;
        if repeat & button::UP != 0 {
            if lower {
                (cursor, moved) = (cursor - 1, true);
            } else if pressed & button::UP != 0 && cursor + 1 < even {
                (cursor, moved) = (cursor + 1, true);
            }
        } else if repeat & button::DOWN != 0 && cursor < even {
            if !lower {
                (cursor, moved) = (cursor + 1, true);
            } else if pressed & button::DOWN != 0 {
                (cursor, moved) = (cursor - 1, true);
            }
        }
        if moved && cursor != before {
            self.cursor = cursor as usize;
            self.places[place::SQUARES + before as usize].play(layout, SET_LEFT);
            self.places[place::SQUARES + self.cursor].play(layout, SET_COME_TO);
        }
        moved
    }
}

impl Library {
    /// Draws a number with a track, each figure from its cell of the figures'
    /// sheet: the last where the track has its group, moved by `from`, and each
    /// one before it a figure's width and one to its left (0x57FDC, 0x58B1C,
    /// 0x59574).
    #[allow(clippy::too_many_arguments)]
    fn number(canvas: &mut Canvas, assets: &Assets, player: &Player, track: usize, number: i32, count: i32, from: f32, outer: Affine2) {
        let layout = &assets.layout;
        let Some(width) = player.group(track).and_then(|group| layout.first_of(group)).map(|object| layout.objects.objects[object].size[0] as f32) else { return };
        for place in 0..count {
            let inner = Affine2::from_translation(Vec2::new(from + (place - count + 1) as f32 * (width + 1.0), 0.0));
            let shift = Vec2::new(FIGURE_CELL * figure(number, count, place) as f32 / assets.figures, 0.0);
            layout.draw_track(canvas, layer::MENU_SCREEN, player, track, &Look { inner, outer, shift, ..default() });
        }
    }

    /// Draws the track of a stage's tab (0x59AC0). The tabs stand so far to the
    /// right for each stage there is short of six; a stage has one if there are
    /// two stages at least and it is one of them: lit while it is the one shown
    /// and dark while it is not, and the dark ones between the first and the
    /// one shown, when that is the third or a later, stand closed up to the left.
    fn draw_tab(&self, canvas: &mut Canvas, layout: &Layout, frame: &Player, track: usize, group: usize) {
        let lit = group >= group::TABS_LIT;
        let stage = group - if lit { group::TABS_LIT } else { group::TABS_DARK };
        if self.stages < 2 || stage >= self.stages || lit != (stage == self.stage) {
            return;
        }
        let outer = Affine2::from_translation(Vec2::new(TAB_HALF * (STAGES - self.stages) as f32, 0.0));
        let closed_up = !lit && stage != 0 && self.stage >= 2 && stage < self.stage;
        let inner = Affine2::from_translation(Vec2::new(if closed_up { TAB_CLOSED_UP } else { 0.0 }, 0.0));
        layout.draw_track(canvas, layer::MENU_SCREEN, frame, track, &Look { inner, outer, ..default() });
    }

    /// Draws a square's picture (0x573D4): that of its record's kind, the one of
    /// a record cleared if it is, a pixel up and to the left of where the layout
    /// has it.
    #[allow(clippy::too_many_arguments)]
    fn draw_picture(&self, canvas: &mut Canvas, assets: &Assets, player: &Player, track: usize, square: usize, by: Vec2) {
        let layout = &assets.layout;
        let (shows, icon) = self.squares[square];
        let pictures = if shows == Square::Cleared { &assets.cleared } else { &assets.open };
        let (Some((at, tint)), Some((picture, whole))) = (text_place(layout, player, track, 0.0), pictures.get(icon as usize)) else { return };
        let Some(object) = layout.first_of(group::PICTURE).map(|object| &layout.objects.objects[object]) else { return };
        let (at, size) = (at + by - Vec2::ONE, Vec2::new(object.size[0] as f32, object.size[1] as f32));
        canvas.sprite(layer::MENU_SCREEN, Paint::textured(picture), Vertex::new(at, Vec2::ZERO, tint), Vertex::new(at + size, size / *whole, tint));
    }

    /// Draws it as it is `ahead` of a frame of the game's logic past its last one
    /// (0x57E34). The strip along the foot, which is drawn last, is the menus'.
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, fonts: &Fonts, colours: &TextColours, ahead: f32) {
        let layout = &assets.layout;
        let layer = layer::MENU_SCREEN;
        let on = |place: &Player| {
            let mut place = place.clone();
            place.advance(layout, ahead * FRAMES_A_STEP as f32);
            place
        };
        let places: Vec<Player> = self.places.iter().map(on).collect();
        let plain = Look::default();
        let across = |by: f32| Look::inner(Affine2::from_translation(Vec2::new(by, 0.0)));

        // The total and the count of those cleared, as they are written
        // (0x571D0): each is set to the right in the room it has.
        let total = fonts.write_plain(&TOTAL_STYLE, &PAGE.to_string(), colours);
        let count = fonts.write_plain(&COUNT_STYLE, &self.cleared.to_string(), colours);
        let total_from = (TOTAL_ROOM - total.size().x).trunc();
        let count_from = (COUNT_ROOM - count.size().x).trunc();

        // All that is about the squares, by what each track's group is.
        let frame = &places[place::FRAME];
        let number = self.cursor as i32 + 1;
        let short = (STAGES - self.stages) as f32;
        for track in 0..frame.len() {
            let Some(group) = frame.group(track) else { continue };
            match group {
                group if (group::TABS_DARK..group::TABS_LIT + STAGES).contains(&group) => self.draw_tab(canvas, layout, frame, track, group),
                group if (group::STAGE_NAMES..group::STAGE_NAMES + STAGES).contains(&group) => {
                    if group == group::STAGE_NAMES + self.stage {
                        layout.draw_track(canvas, layer, frame, track, &plain);
                    }
                }
                group::NUMBER => Library::number(canvas, assets, frame, track, number, figures(number), 0.0, Affine2::IDENTITY),
                // Six to the left for each figure past the first (0x580AC).
                group::NUMBER_LABEL => layout.draw_track(canvas, layer, frame, track, &across(6.0 - 6.0 * figures(number) as f32)),
                group::COUNT => {
                    if let Some((at, tint)) = text_place(layout, frame, track, 0.0) {
                        fonts.draw(canvas, layer, &count, &Placing { tint, ..Placing::at(at + Vec2::new(count_from + total_from, 0.0)) });
                    }
                }
                group::TOTAL => {
                    if let Some((at, tint)) = text_place(layout, frame, track, 0.0) {
                        fonts.draw(canvas, layer, &total, &Placing { tint, ..Placing::at(at + Vec2::new(total_from, 0.0)) });
                    }
                }
                group::STROKE => layout.draw_track(canvas, layer, frame, track, &across(total_from)),
                // The arrows are there for two stages or more, and keep to the
                // ends of the row of tabs.
                group::R | group::L => {
                    if self.stages >= 2 {
                        let by = if group == group::R { -TAB_HALF } else { TAB_HALF } * short;
                        layout.draw_track(canvas, layer, frame, track, &across(by));
                    }
                }
                _ => layout.draw_track(canvas, layer, frame, track, &plain),
            }
        }

        // What is told of the record the cursor is on.
        let told = &self.told;
        let write = |canvas: &mut Canvas, player: &Player, track: usize, words: &str| {
            if let Some((at, tint)) = text_place(layout, player, track, 0.0) {
                write_text(canvas, layer, fonts, colours, &WORDS_STYLE, words, at, tint);
            }
        };
        if matches!(told.of, Square::Open | Square::Cleared) {
            let cleared = told.of == Square::Cleared;
            let known = &places[place::KNOWN];
            for track in 0..known.len() {
                let Some(group) = known.group(track) else { continue };
                match group {
                    group::TO_CLEAR => write(canvas, known, track, &told.to_clear),
                    group::WORTH => write(canvas, known, track, &told.worth),
                    group::NAME => write(canvas, known, track, &told.name),
                    group::CLEARED_MARK => {
                        if cleared {
                            layout.draw_track(canvas, layer, known, track, &plain);
                        }
                    }
                    group::SMALL_PICTURE => {
                        if let Some((at, tint)) = text_place(layout, known, track, 0.0) {
                            draw_small_picture(canvas, layer, &assets.small, told.icon, at, tint);
                        }
                    }
                    // The bar, cut to how much of its goal the count has come
                    // to (0x58758).
                    group::BAR => {
                        let Some(bar) = layout.first_of(group).map(|object| &layout.objects.objects[object]) else { continue };
                        let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32);
                        let width = (bar.size[0] as f32 * told.share).trunc();
                        if !cleared && width > 0.0 {
                            canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(width, bar.size[1] as f32))));
                            layout.draw_track(canvas, layer, known, track, &plain);
                            canvas.cut_to(None);
                        }
                    }
                    group::HUNDREDTHS => {
                        if !cleared {
                            Library::number(canvas, assets, known, track, (told.share * 100.0) as i32, 2, 0.0, Affine2::IDENTITY);
                        }
                    }
                    group if group::WHILE_COUNTING.contains(&group) => {
                        if !cleared {
                            layout.draw_track(canvas, layer, known, track, &plain);
                        }
                    }
                    _ => layout.draw_track(canvas, layer, known, track, &plain),
                }
            }
        } else {
            let of_one = told.of == Square::Unknown;
            let unknown = &places[place::UNKNOWN];
            for track in 0..unknown.len() {
                let Some(group) = unknown.group(track) else { continue };
                match group {
                    group::HINT => {
                        if of_one {
                            write(canvas, unknown, track, &told.hint);
                        }
                    }
                    group if group::ONLY_OF_ONE.contains(&group) => {
                        if of_one {
                            layout.draw_track(canvas, layer, unknown, track, &plain);
                        }
                    }
                    _ => layout.draw_track(canvas, layer, unknown, track, &plain),
                }
            }
        }

        // The squares, down the columns (0x590C0): each by its own place, moved
        // to where it is.
        for (square, &(shows, _)) in self.squares.iter().enumerate() {
            let by = Vec2::new((square / ROWS) as f32, (square % ROWS) as f32) * PITCH;
            let outer = Affine2::from_translation(by);
            let player = &places[place::SQUARES + square];
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let drawn = match group {
                    group::PICTURE => {
                        if matches!(shows, Square::Open | Square::Cleared) {
                            self.draw_picture(canvas, assets, player, track, square, by);
                        }
                        false
                    }
                    group::UNTIL_CLEARED => matches!(shows, Square::Unknown | Square::Open),
                    group::OF_A_RECORD => shows != Square::Empty,
                    group::UNKNOWN_WORD => shows == Square::Unknown,
                    // Its number, about the middle of where its figure is
                    // (0x59500).
                    group::SQUARE_NUMBER => {
                        if shows == Square::Unknown {
                            let (number, width) = (square as i32 + 1, layout.first_of(group).map_or(0, |object| layout.objects.objects[object].size[0] as i32));
                            let count = figures(number);
                            Library::number(canvas, assets, player, track, number, count, ((count - 1) * width / 2) as f32, outer);
                        }
                        false
                    }
                    group::CLEARED_FRAME => shows == Square::Cleared,
                    _ => true,
                };
                if drawn {
                    layout.draw_track(canvas, layer, player, track, &Look { outer, ..plain });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{common, game, press, Game};
    use super::super::{Menu, Screen, State as MenuState};
    use super::*;

    fn library(menu: &Menu) -> Option<&Library> {
        match menu.main.as_ref()?.screen.as_ref()? {
            Screen::Library(library) => Some(library),
            _ => None,
        }
    }

    /// Brings the MENU up and has its cursor on its third entry.
    fn at_the_entry(game: &mut Game) -> Menu {
        let mut menu = Menu::default();
        menu.ask(1);
        while !menu.main.as_ref().unwrap().ready {
            game.step(&mut menu, &Pad::default());
        }
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::DOWN));
        menu.sounds.clear();
        menu
    }

    /// Brings the MENU up and chooses its third entry: the menus as CHALLENGES
    /// takes the pad.
    fn come(game: &mut Game) -> Menu {
        let mut menu = at_the_entry(game);
        game.step(&mut menu, &press(button::CROSS));
        let mut steps = 0;
        while !library(&menu).is_some_and(|library| library.ready) {
            assert!(!game.step(&mut menu, &Pad::default()));
            steps += 1;
            assert!(steps < 200);
        }
        menu.sounds.clear();
        menu
    }

    fn held(buttons: u32) -> Pad {
        Pad { held: buttons, repeat: buttons, ..Pad::default() }
    }

    /// Where the things drawn from a texture have their top left corners, and
    /// how far across the texture each is drawn from.
    fn corners(canvas: &Canvas, texture: &Handle<Image>) -> Vec<(Vec2, f32)> {
        let things = canvas.drawn_from(texture);
        things.chunks(6).map(|thing| thing.iter().fold((thing[0].position, thing[0].uv.x), |(at, across), vertex| (at.min(vertex.position), across.min(vertex.uv.x)))).collect()
    }

    fn places(canvas: &Canvas, texture: &Handle<Image>) -> Vec<Vec2> {
        corners(canvas, texture).into_iter().map(|(at, _)| at).collect()
    }

    fn drawn(menu: &Menu, game: &Game) -> Canvas {
        let mut canvas = Canvas::default();
        library(menu).unwrap().draw(&mut canvas, &game.assets.library, &game.fonts, &game.colours, 0.0);
        canvas
    }

    #[test]
    fn cross_on_the_menus_third_entry_brings_it_and_it_takes_the_pad_once_its_first_square_has_come() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets.library;
        assert_eq!(assets.strip, "Check achievable challenges for the stage.");
        assert_eq!((assets.layout.textures.len(), assets.layout.motions.sets.len(), assets.open.len(), assets.cleared.len()), (14, 9, 10, 10));
        // The square the cursor is on at first has come after sixty frames.
        let tracks = assets.layout.motions.sets[SET_CHOSEN_AT_FIRST].len();
        let longest = (0..tracks).map(|track| assets.layout.track_length(SET_CHOSEN_AT_FIRST, track)).fold(0.0, f32::max);
        assert_eq!(longest, 60.0);
        // The game has ninety-six records, sixteen to each of its six stages.
        assert_eq!(game.records.count(), 96);

        let mut menu = at_the_entry(&mut game);
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [3]);
        // Fourteen frames of the wipe on, the screen is covered and it is made.
        for _ in 0..6 {
            game.step(&mut menu, &Pad::default());
            assert!(menu.main.as_ref().unwrap().state == MenuState::Chosen && library(&menu).is_none());
        }
        game.step(&mut menu, &Pad::default());
        let made = library(&menu).unwrap().clone();
        assert_eq!((made.state, made.at, made.stages, made.stage, made.cursor), (State::Coming, At::Squares, 1, 0, 0));
        assert_eq!((made.records, made.cleared, made.places.len()), ((0..16).collect::<Vec<i32>>(), 0, place::SQUARES + SQUARES));
        assert_eq!((made.places[place::SQUARES].set(), made.places[place::SQUARES + 1].set(), made.places[place::SQUARES + 15].set()), (SET_CHOSEN_AT_FIRST, SET_SQUARE, SET_SQUARE));
        // It takes the pad when its first square has come, sixty of its frames
        // after it was made; and its sound is played twenty-six frames after
        // the wipe is through, which is before that.
        let (mut steps, mut sounded_at) = (7, None);
        while !library(&menu).unwrap().ready {
            game.step(&mut menu, &press(button::CIRCLE | button::RIGHT));
            steps += 1;
            if !menu.sounds.is_empty() {
                assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_COME]);
                assert_eq!(sounded_at.replace(steps), None);
            }
        }
        assert_eq!((steps, sounded_at), (37, Some(31)));
        let came = library(&menu).unwrap();
        assert_eq!((came.state, came.at, came.cursor, menu.main.as_ref().unwrap().wipe.playing), (State::Open, At::Squares, 0, false));
        // Cross does nothing in it.
        game.step(&mut menu, &press(button::CROSS));
        assert!(menu.sounds.is_empty() && library(&menu).unwrap().at == At::Squares);
    }

    #[test]
    fn it_shows_the_stage_of_the_map_she_is_on_and_l_and_r_go_through_the_stages_seen() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // Three stages have been seen one after another; the fifth's flag
        // counts for nothing without the fourth's.
        for stage in [0, 1, 2, 4] {
            game.progress.set_flag(STAGE_SEEN + stage, true);
        }
        game.stage = 1;
        let mut menu = come(&mut game);
        let now = |menu: &Menu| library(menu).unwrap().clone();
        assert_eq!((now(&menu).stages, now(&menu).stage, now(&menu).records), (3, 1, (16..32).collect::<Vec<i32>>()));
        // R shows the next stage, with its arrow nudged, and is heard.
        game.step(&mut menu, &press(button::R));
        assert_eq!((now(&menu).stage, now(&menu).records[0], now(&menu).places[place::FRAME].set()), (2, 32, SET_R));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR]);
        // Held, it does not go round from the last stage; a new press does.
        game.step(&mut menu, &held(button::R));
        assert!(now(&menu).stage == 2 && menu.sounds.is_empty());
        game.step(&mut menu, &press(button::R));
        assert_eq!((now(&menu).stage, now(&menu).records[0]), (0, 0));
        game.step(&mut menu, &press(button::L));
        assert_eq!((now(&menu).stage, now(&menu).places[place::FRAME].set()), (2, SET_L));
        game.step(&mut menu, &held(button::L));
        assert_eq!(now(&menu).stage, 1);
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 3]);
        // The cursor stays on its square from one stage to the next.
        game.step(&mut menu, &press(button::RIGHT | button::DOWN));
        game.step(&mut menu, &press(button::L));
        assert_eq!((now(&menu).stage, now(&menu).cursor), (0, 3));

        // On a map of a stage that has not been seen it is the last that has.
        game.stage = 5;
        let menu = come(&mut game);
        assert_eq!((now(&menu).stages, now(&menu).stage), (3, 2));

        // With one stage there is nothing to go through: R makes the page anew
        // all the same, and is not heard.
        for stage in 0..6 {
            game.progress.set_flag(STAGE_SEEN + stage, false);
        }
        let mut menu = come(&mut game);
        assert_eq!((now(&menu).stages, now(&menu).stage), (1, 0));
        game.step(&mut menu, &press(button::R));
        assert_eq!((now(&menu).stage, now(&menu).places[place::FRAME].set()), (0, SET_R));
        assert!(menu.sounds.is_empty());
    }

    #[test]
    fn the_cursor_goes_over_two_rows_of_eight_and_round_only_on_a_new_press() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        let set_of = |menu: &Menu, square: usize| library(menu).unwrap().places[place::SQUARES + square].set();
        let go = |game: &mut Game, menu: &mut Menu, pad: Pad, to: usize| {
            game.step(menu, &pad);
            assert_eq!(library(menu).unwrap().cursor, to);
        };
        // Down goes to the lower row each time it counts as pressed, and from
        // it back up only when it has newly gone down; up likewise.
        go(&mut game, &mut menu, held(button::DOWN), 1);
        assert_eq!((set_of(&menu, 0), set_of(&menu, 1), set_of(&menu, 2)), (SET_LEFT, SET_COME_TO, SET_SQUARE));
        go(&mut game, &mut menu, held(button::DOWN), 1);
        go(&mut game, &mut menu, press(button::DOWN), 0);
        go(&mut game, &mut menu, held(button::UP), 0);
        go(&mut game, &mut menu, press(button::UP), 1);
        go(&mut game, &mut menu, held(button::UP), 0);
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 4]);
        // Right goes a column on, which is two squares, and from the last
        // column to the first only on a new press; left likewise.
        for to in [2, 4, 6, 8, 10, 12, 14, 14] {
            go(&mut game, &mut menu, held(button::RIGHT), to);
        }
        go(&mut game, &mut menu, press(button::RIGHT), 0);
        go(&mut game, &mut menu, held(button::LEFT), 0);
        go(&mut game, &mut menu, press(button::LEFT), 14);
        go(&mut game, &mut menu, held(button::LEFT), 12);
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 10]);
        // Both ways are taken in one frame, and the lower row goes round as
        // the upper does.
        go(&mut game, &mut menu, press(button::RIGHT | button::DOWN), 15);
        go(&mut game, &mut menu, press(button::RIGHT), 1);
        go(&mut game, &mut menu, press(button::LEFT), 15);
        assert_eq!((set_of(&menu, 1), set_of(&menu, 15), set_of(&menu, 14)), (SET_LEFT, SET_COME_TO, SET_LEFT));
        // Up is asked first: down does nothing while up counts as pressed.
        go(&mut game, &mut menu, held(button::UP | button::DOWN), 14);
        go(&mut game, &mut menu, held(button::UP | button::DOWN), 14);
    }

    #[test]
    fn what_is_told_is_of_the_record_the_cursor_is_on() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The first record is open, a quarter of the way to its four battles;
        // the third is cleared; the others of the stage are not known.
        let first = game.records.record(0).unwrap();
        assert_eq!((first.name.as_str(), first.goal), ("First Challenge", 4));
        game.progress.set_flag(flag::OPEN, true);
        game.progress.set_variable(COUNT, 1);
        game.progress.set_flag(flag::OPEN + 2, true);
        game.progress.set_flag(flag::CLEARED + 2, true);
        // One that is cleared and was never opened is not known for that.
        game.progress.set_flag(flag::CLEARED + 5, true);
        let mut menu = come(&mut game);
        let now = |menu: &Menu| library(menu).unwrap().clone();
        let shows: Vec<Square> = now(&menu).squares.iter().map(|square| square.0).collect();
        assert_eq!(shows[..4], [Square::Open, Square::Unknown, Square::Cleared, Square::Unknown]);
        assert!(shows[4..].iter().all(|shows| *shows == Square::Unknown));
        assert_eq!(now(&menu).cleared, 2);
        let told = now(&menu).told;
        assert_eq!((told.of, told.share, told.name.as_str(), told.icon), (Square::Open, 0.25, "First Challenge", first.icon));
        assert!(!told.to_clear.is_empty());
        assert_eq!((told.to_clear, told.worth, told.hint), (first.words[0].clone(), first.words[2].clone(), String::new()));
        // Of one not known only the kind of thing that opens it is told.
        game.step(&mut menu, &press(button::DOWN));
        let second = game.records.record(1).unwrap();
        let told = now(&menu).told;
        assert_eq!((told.of, told.share, told.name.as_str(), told.to_clear.as_str(), told.worth.as_str()), (Square::Unknown, 0.0, "", "", ""));
        assert_eq!(told.hint, second.words[1]);
        // One cleared is told as the whole of its goal, whatever its count.
        game.step(&mut menu, &press(button::UP));
        game.step(&mut menu, &press(button::RIGHT));
        let third = game.records.record(2).unwrap();
        let told = now(&menu).told;
        assert_eq!((told.of, told.share, told.name, told.to_clear), (Square::Cleared, 1.0, third.name, third.words[0].clone()));
    }

    #[test]
    fn it_draws_the_stages_tabs_the_squares_and_what_is_told() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        for stage in 0..3 {
            game.progress.set_flag(STAGE_SEEN + stage, true);
        }
        game.stage = 2;
        // Of the third stage's records the first is open, one of its goal
        // counted, and the third is cleared.
        let (open, cleared) = (game.records.record(32).unwrap(), game.records.record(34).unwrap());
        game.progress.set_flag(flag::OPEN + 32, true);
        game.progress.set_variable(COUNT + 32, 1);
        game.progress.set_flag(flag::OPEN + 34, true);
        game.progress.set_flag(flag::CLEARED + 34, true);
        let mut menu = come(&mut game);
        let canvas = drawn(&menu, &game);
        let assets = &game.assets.library;
        let textures = &assets.layout.textures;

        // The tabs stand eight to the right for each of the three stages there
        // are not: the first two dark, the second of them closed up to the
        // first, and the third lit; and the arrows keep to the row's ends.
        let small = places(&canvas, &textures[8]);
        assert!(small.contains(&Vec2::new(210.0, 5.0)) && small.contains(&Vec2::new(226.0, 5.0)), "{small:?}");
        assert!(!small.iter().any(|at| at.y == 5.0 && at.x > 226.0));
        let of_the_archive = places(&canvas, &textures[1]);
        assert!(of_the_archive.contains(&Vec2::new(242.0, 5.0)), "{of_the_archive:?}");
        assert!(!of_the_archive.contains(&Vec2::new(210.0, 5.0)) && !of_the_archive.contains(&Vec2::new(226.0, 5.0)));
        for arrow in [Vec2::new(178.0, 6.0), Vec2::new(196.0, 6.0), Vec2::new(287.0, 6.0), Vec2::new(272.0, 6.0)] {
            assert!(of_the_archive.contains(&arrow), "{arrow}: {of_the_archive:?}");
        }

        // The squares' pictures: the open one of the first record's kind, and
        // the cleared one of the third's a column on, forty-five square, a
        // pixel up and to the left of where the layout has them.
        assert_eq!(places(&canvas, &assets.open[open.icon as usize].0), [Vec2::new(38.0, 103.0)]);
        assert_eq!(places(&canvas, &assets.cleared[cleared.icon as usize].0), [Vec2::new(38.0 + 51.0, 103.0)]);
        let whole = canvas.drawn_from(&assets.open[open.icon as usize].0);
        assert!(whole.iter().any(|vertex| vertex.position == Vec2::new(83.0, 148.0)));

        // The figures: the number of the cursor's square, 1; the hundredths of
        // the first record's share, in two; and the numbers of the squares not
        // known, each about the middle of its place, the tenth's in two figures.
        let sheet = assets.figures;
        let cell = move |figure: f32| figure * FIGURE_CELL / sheet;
        let share = 1.0 / open.goal as f32;
        let hundredths = (share * 100.0) as i32;
        let figures = corners(&canvas, &textures[11]);
        assert!(figures.contains(&(Vec2::new(463.0, 63.0), cell(1.0))), "{figures:?}");
        assert!(figures.contains(&(Vec2::new(452.0, 78.0), cell((hundredths / 10) as f32))) && figures.contains(&(Vec2::new(458.0, 78.0), cell((hundredths % 10) as f32))));
        assert!(figures.contains(&(Vec2::new(59.0, 141.0 + 48.0), cell(2.0))));
        assert!(figures.contains(&(Vec2::new(55.0 + 4.0 * 51.0, 141.0 + 48.0), cell(1.0))) && figures.contains(&(Vec2::new(61.0 + 4.0 * 51.0, 141.0 + 48.0), cell(0.0))));
        // Neither the open square nor the cleared one has a number.
        assert!(!figures.iter().any(|(at, _)| at.y == 141.0 && at.x < 59.0 + 2.0 * 51.0));

        // The bar is cut to as much of its 193 as the count is of the goal.
        let bar = (193.0 * share).trunc();
        assert!(bar > 0.0 && bar != 82.0 && bar < 164.0, "{bar}");
        let ends = move |canvas: &Canvas| canvas.vertices().iter().any(|vertex| (90.0..=92.0).contains(&vertex.position.y) && vertex.position.x == 276.0 + bar);
        assert!(ends(&canvas));

        // The total is set to the right in its ten and the count of those
        // cleared in its fifteen, which the total's width moves with it.
        let total = game.fonts.write_plain(&TOTAL_STYLE, "16", &game.colours).size().x;
        let count = game.fonts.write_plain(&COUNT_STYLE, "1", &game.colours).size().x;
        let (total_from, count_from) = ((10.0 - total).trunc(), (15.0 - count).trunc());
        let written = |from: f32, to: f32| -> f32 {
            let of_it = canvas.vertices().iter().filter(|vertex| vertex.uv != Vec2::ZERO && (5.0..=20.0).contains(&vertex.position.y) && (from..to).contains(&vertex.position.x));
            of_it.map(|vertex| vertex.position.x).fold(f32::MAX, f32::min)
        };
        assert_eq!(library(&menu).unwrap().cleared, 1);
        assert_eq!(written(464.0 + total_from, 480.0), 464.0 + total_from);
        assert_eq!(written(436.0 + count_from + total_from, 457.0 + total_from), 436.0 + count_from + total_from);

        // On the cleared square what is told has no bar and no hundredths.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!(library(&menu).unwrap().told.of, Square::Cleared);
        let canvas = drawn(&menu, &game);
        let figures = corners(&canvas, &game.assets.library.layout.textures[11]);
        assert!(!ends(&canvas));
        assert!(figures.contains(&(Vec2::new(463.0, 63.0), cell(3.0))) && !figures.iter().any(|(at, _)| at.y == 78.0));
    }

    #[test]
    fn circle_goes_back_to_the_menu_and_triangle_out_of_the_menus() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        // Circle: the wipe covers it again, and fourteen frames on the MENU is
        // back with its cursor where it was.
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_BACK]);
        let main = |menu: &Menu| menu.main.clone().unwrap();
        assert_eq!((library(&menu).unwrap().state, main(&menu).wipe.set, main(&menu).wipe.playing), (State::Leaving, WIPE_BETWEEN, true));
        let mut steps = 1;
        while library(&menu).is_some() {
            assert!(!game.step(&mut menu, &press(button::RIGHT)));
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 8);
        assert_eq!((main(&menu).state, main(&menu).entry), (MenuState::Open, 2));
        assert!(menu.is_open());

        // Triangle: the screen goes dark as it does when the MENU is left, and
        // the menus are done with.
        let mut menu = come(&mut game);
        assert!(!game.step(&mut menu, &press(button::TRIANGLE)));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_OUT]);
        assert_eq!((library(&menu).unwrap().state, main(&menu).wipe.set), (State::Leaving, WIPE_GOES));
        let mut steps = 1;
        while !game.step(&mut menu, &Pad::default()) {
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 7);
        assert!(!menu.is_open());
    }
}
