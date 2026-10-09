//! The window a record is shown in (`ifRecInfoTask`, 0x6F498 to 0x721D8): the
//! screen the game heads CHALLENGES. See "The records (challenges)" in
//! `docs/FIELD_NOTES.md`.
//!
//! It shows the sixteen records of a stage, two rows of eight squares numbered
//! down the columns, with the cursor on the one it was opened for; over them the
//! stage's name and that record's name, and under them what is to be done for it
//! and what it is worth. A record that is not open yet is a square that says
//! UNKNOWN with its number, and all that is told of it is what kind of thing
//! would open it; one that is open has its picture, and how far its count is as a
//! bar and in hundredths; one that is cleared has another picture and a mark that
//! says so.
//!
//! The records' script works it (`rec_clear_reward`, with engine functions 0x6C
//! to 0x71): it is shown for a record (0x6F674), comes, and then waits for cross;
//! the script goes on from there, and has it go when it is done (0x6F7A0). While
//! it is up the music is turned down to 1.
//!
//! What has just been opened or cleared is held back until its own turn to be
//! shown (0x1AD8): a record that is still held is drawn as it was before.
//!
//! Its file is `IF/MENU/IF_REC_INFO.GZ`: a table of texts, the layout, eight
//! textures, and twenty pictures of forty-five square, one for each of ten kinds
//! of record cleared and one for each open.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_formats::xtc;

use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::Layouts;
use crate::field::window::Window;
use crate::interface::{self, layer};
use crate::progress::Progress;
use crate::records::{flag, Records, COUNT};
use crate::sound::{Board, Keeper, Request, COMMON_BANK, FULL_VOLUME};
use crate::text::{self, Span, TextColours};
use crate::writing::{Align, Block, Fonts, Placing, Style};

const FILE: &str = "IF/MENU/IF_REC_INFO.GZ";
// The file's parts (0x70004): its texts, its layout, the layout's own eight
// textures, and the pictures of the kinds of record, cleared and open.
const TEXTS: usize = 0;
const LAYOUT: usize = 1;
const OWN_TEXTURES: std::ops::RangeInclusive<usize> = 4..=11;
const CLEARED_PICTURES: usize = 12;
const OPEN_PICTURES: usize = 22;
const KINDS: usize = 10;
/// Which of its texts says how it is left (0x70468).
const TEXT_CLOSE: usize = 8;

/// The records of a stage, which is what it shows at a time (0x6FC94).
pub const PAGE: i32 = 16;
/// The frames of the interface in a frame of the game.
const FRAMES_A_STEP: f32 = 2.0;

// What plays its layout has six parts, each playing a set of its own (0x702F8):
// all that is about the squares; the square the cursor is on; any other square;
// what is told of a record that is not known; what is told of one that is; and
// the cursor.
const FRAME: usize = 0;
const CHOSEN: usize = 1;
const SQUARE: usize = 2;
const UNKNOWN: usize = 3;
const KNOWN: usize = 4;
const CURSOR: usize = 5;
/// The sets the parts play as it comes (0x6F6D0), and the first five as it goes
/// (0x6F7C0); the cursor's goes round.
const COMING: [usize; 6] = [0, 3, 5, 7, 9, 2];
const GOING: [usize; 5] = [1, 4, 6, 8, 10];

// The groups of the frame that are more than something to draw (0x707A4): the
// number of the record the cursor is on, in figures; what stands before it, which
// makes room for them; the six stages' names, of which the one shown is drawn; and
// where its one text is written.
const NUMBER: usize = 28;
const NUMBER_LABEL: usize = 29;
const STAGES: usize = 60;
const CLOSE: usize = 75;
// Those of what is told of a record that is known (0x710BC): where what is to be
// done, what it is worth and its name are written; the mark of one cleared; its
// small picture; and for one not cleared the bar of how far its count is, the
// hundredths of that in two figures, and what goes with them.
const TO_CLEAR: usize = 7;
const WORTH: usize = 8;
const CLEARED_MARK: usize = 12;
const SMALL_PICTURE: usize = 13;
const NAME: usize = 16;
const BAR: usize = 17;
const WHILE_COUNTING: [usize; 2] = [18, 20];
const HUNDREDTHS: usize = 19;
// Those of what is told of one that is not (0x71838): two drawn only for a record
// there is, and where the kind of thing that opens it is written.
const ONLY_OF_ONE: [usize; 2] = [3, 6];
const HINT: usize = 9;
// Those of a square (0x71C04): its picture; what is over it until it is cleared;
// what a square with a record has; and for one not known the word UNKNOWN and
// its number.
const PICTURE: usize = 41;
const UNTIL_CLEARED: usize = 42;
const OF_A_RECORD: usize = 44;
const UNKNOWN_WORD: usize = 45;
const SQUARE_NUMBER: usize = 46;

/// How far apart the squares are, across and down (0x71B5C).
const PITCH: Vec2 = Vec2::new(51.0, 48.0);
/// The cells of the sheet the figures are drawn from (the first of the table at
/// 0x171078), and of the sheet of small pictures: fifteen square in cells of
/// sixteen, four to a row, a pixel in (0x2D5D0).
pub const FIGURE_CELL: f32 = 20.0;
const SMALL: f32 = 15.0;
const SMALL_CELL: f32 = 16.0;
/// The small picture of each kind of record (0x2D540), of that sheet.
const SMALL_PICTURES: [usize; KINDS] = [0x15, 0x16, 0x0a, 0x17, 0x18, 0x19, 0x0b, 7, 0x12, 0x14];

/// Draws the small picture of a kind of record (0x2D4B0), of the sheet of them
/// (file 20 of the pack that is always there).
pub fn draw_small_picture(canvas: &mut Canvas, layer: i32, sheet: &(Handle<Image>, Vec2), kind: u8, at: Vec2, tint: [u8; 4]) {
    draw_small(canvas, layer, sheet, SMALL_PICTURES.get(kind as usize).copied().unwrap_or(0), at, tint);
}

/// Draws one of the sheet's small pictures by its number (0x2D5D0, 0x2D3D8).
pub fn draw_small(canvas: &mut Canvas, layer: i32, sheet: &(Handle<Image>, Vec2), picture: usize, at: Vec2, tint: [u8; 4]) {
    let from = Vec2::new((picture % 4) as f32, (picture / 4) as f32) * SMALL_CELL + Vec2::ONE;
    let (sheet, size) = sheet;
    canvas.sprite(layer, Paint::textured(sheet), Vertex::new(at, from / *size, tint), Vertex::new(at + Vec2::splat(SMALL), (from + Vec2::splat(SMALL)) / *size, tint));
}

// How its texts are written (0x6F81C): the one about leaving in the small
// capitals, to the right of its place; the rest in the text's own font, a line of
// ninety-six glyphs.
const CLOSE_STYLE: Style = Style::of_kind(2, 0x30, 6, 1.0, 4.0);
const WORDS_STYLE: Style = Style::of_kind(0, 0x60, 1, 1.0, 4.0);

// What is heard (0x6F730, 0x705AC), of the bank that is always there: as it comes
// for a record that is cleared, or for one that is not; and cross.
const SOUND_CLEARED: u32 = 0x66;
const SOUND_OPENED: u32 = 0x0f;
const SOUND_READ: u32 = 3;
/// How loud the music is under it, of 127 (0x6F6B4).
const MUSIC_UNDER_IT: i32 = 1;

/// How many figures a number is written with (0x22ACC).
fn figures(number: i32) -> i32 {
    let (mut left, mut count) = (number.unsigned_abs(), 1);
    while left >= 10 {
        left /= 10;
        count += 1;
    }
    count
}

/// The figure in a place of a number of `count` figures, the first place the
/// highest (0x22AFC).
pub fn figure(number: i32, count: i32, place: i32) -> i32 {
    number.max(0) / 10_i32.pow((count - 1 - place) as u32) % 10
}

/// What a square shows (0x71AC8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Square {
    /// There is no record for it.
    Empty,
    Unknown,
    Open,
    Cleared,
}

/// What is told of the record the cursor is on, as the window is shown for it
/// (0x6FAB0).
#[derive(Clone, Debug, Default)]
struct Told {
    square: Option<Square>,
    /// How much of its goal its count has come to.
    share: f32,
    name: Option<Block>,
    to_clear: Option<Block>,
    worth: Option<Block>,
    hint: Option<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Not up (its states 2 and 8).
    Shut,
    /// Coming (3).
    Coming,
    /// Up, until cross is pressed (4).
    Asking,
    /// Read (5): the script that works it goes on, and nothing of it moves.
    Read,
    /// Going (6).
    Going,
    /// Gone (7), for a frame.
    Gone,
}

/// The window, and what it is drawn from.
#[derive(Resource)]
pub struct RecordWindow {
    layout: Layout,
    /// The width of the sheet of figures, and the sheet of small pictures.
    figures: f32,
    small: (Handle<Image>, Vec2),
    /// The pictures of the kinds of record, open and cleared.
    open: Vec<(Handle<Image>, Vec2)>,
    cleared: Vec<(Handle<Image>, Vec2)>,
    close: String,
    phase: Phase,
    parts: [Player; 6],
    /// The stage whose records are shown, the square the cursor is on, and what
    /// is told of its record.
    stage: i32,
    cursor: usize,
    told: Told,
    close_words: Option<Block>,
    /// The records held back: opened or cleared, and not shown so yet (0x1AD8).
    held: BTreeSet<i32>,
    /// The window that says what a record cleared is worth, while it is up: the
    /// one a script puts up, over this one.
    message: Option<Window>,
}

impl RecordWindow {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<RecordWindow> {
        // The layout's fourteen textures (0x70088): the file's own eight; the
        // sheet of small pictures; the first of the records' pictures, in whose
        // place each square's own is drawn; and four more of the pack that is
        // always there, the third of them the figures.
        let own = |file: usize| interface::file(FILE, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        let mut textures: Vec<String> = OWN_TEXTURES.map(own).collect();
        textures.extend([shared(20), own(CLEARED_PICTURES), shared(7), shared(9), shared(12), shared(10)]);
        let layout = interface::layout_with(data, images, FILE, LAYOUT, &textures)?;
        let (_, figures) = interface::load_texture(data, images, &shared(9))?;
        let small = interface::load_texture(data, images, &shared(20))?;
        let mut pictures = |first: usize| (first..first + KINDS).map(|file| interface::load_texture(data, images, &own(file))).collect::<Result<Vec<_>>>();
        let (cleared, open) = (pictures(CLEARED_PICTURES)?, pictures(OPEN_PICTURES)?);
        let texts = xtc::Table::parse(&data.read(interface::file(FILE, TEXTS, "xtc"))?).context("the record window's texts")?;
        let at = texts.record(TEXT_CLOSE).and_then(|record| record.get(..4)).context("the record window has no text about leaving")?;
        let close = texts.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned();
        let parts = COMING.map(|set| layout.player(set));
        Ok(RecordWindow {
            layout,
            figures: figures.x,
            small,
            open,
            cleared,
            close,
            phase: Phase::Shut,
            parts,
            stage: 0,
            cursor: 0,
            told: Told::default(),
            close_words: None,
            held: BTreeSet::new(),
            message: None,
        })
    }

    /// Says what a record cleared is worth (0x6BAFC, 0x6BCB0): the window is up
    /// until it has been read.
    pub fn say(&mut self, pages: Vec<Vec<Span>>) {
        self.message = Some(Window::new(pages, &[], 0, 0));
    }

    /// Whether that is still being said (0x6BDE0).
    pub fn saying(&self) -> bool {
        self.message.is_some()
    }

    /// The window that says it, to be drawn.
    pub fn message(&self) -> Option<&Window> {
        self.message.as_ref()
    }

    /// The records that are to be shown one after another are held back until
    /// each one's turn (0x380E0): nothing else is.
    pub fn hold(&mut self, records: &[i32]) {
        self.held = records.iter().copied().collect();
    }

    /// A record's turn has come (0x38024).
    pub fn reveal(&mut self, record: i32) {
        self.held.remove(&record);
    }

    /// Whether a record is cleared and may be seen to be (0x6FF50).
    fn seen_cleared(&self, record: i32, progress: &Progress) -> bool {
        progress.flag(flag::CLEARED + record) && !self.held.contains(&record)
    }

    /// Whether a record is known (0x6FFA0): it is open, and is not one whose
    /// opening is still held back.
    fn known(&self, record: i32, records: &Records, progress: &Progress) -> bool {
        records.is_open(record, progress) && !(self.held.contains(&record) && progress.flag(flag::OPENED_NOW + record))
    }

    /// The record of a square of the stage that is shown, if there is one.
    fn record_of(&self, square: usize, records: &Records) -> Option<i32> {
        let record = self.stage * PAGE + square as i32;
        (square < PAGE as usize && record < records.count()).then_some(record)
    }

    fn square(&self, square: usize, records: &Records, progress: &Progress) -> Square {
        match self.record_of(square, records) {
            None => Square::Empty,
            Some(record) if self.seen_cleared(record, progress) => Square::Cleared,
            Some(record) if self.known(record, records, progress) => Square::Open,
            Some(_) => Square::Unknown,
        }
    }

    /// Shows it for a record (0x6F3C8, 0x6F674): its stage's records, with the
    /// cursor on it.
    pub fn show(&mut self, record: i32, records: &Records, progress: &Progress, fonts: &Fonts, colours: &TextColours, board: &mut Board) {
        (self.stage, self.cursor) = (record.max(0) / PAGE, (record.max(0) % PAGE) as usize);
        self.phase = Phase::Coming;
        board.ask_all([Request::KeepMusicVolume(Keeper::Record), Request::MusicVolume(MUSIC_UNDER_IT, 0.0)]);
        // The cursor is kept on a square that has a record (0x6FC54).
        let there = (0..PAGE as usize).filter(|&square| self.record_of(square, records).is_some()).count();
        if self.cursor >= there {
            self.cursor = there.saturating_sub(1);
        }
        let write = |words: &str| fonts.write(&WORDS_STYLE, &text::pages(words, &text::Plain).swap_remove(0), colours);
        let cursor_on = self.record_of(self.cursor, records).and_then(|number| Some((number, records.record(number)?)));
        self.told = match cursor_on {
            None => Told::default(),
            Some((number, record)) if self.known(number, records, progress) => {
                let cleared = self.seen_cleared(number, progress);
                Told {
                    square: Some(if cleared { Square::Cleared } else { Square::Open }),
                    share: if cleared { 1.0 } else { progress.variable(COUNT + number) as f32 / record.goal as f32 },
                    name: Some(write(&record.name)),
                    to_clear: Some(write(&record.words[0])),
                    worth: Some(write(&record.words[2])).filter(|_| !record.words[2].is_empty()),
                    hint: None,
                }
            }
            Some((_, record)) => Told { square: Some(Square::Unknown), hint: Some(write(&record.words[1])).filter(|_| !record.words[1].is_empty()), ..Told::default() },
        };
        self.close_words = Some(fonts.write(&CLOSE_STYLE, &text::pages(&self.close, &text::Plain).swap_remove(0), colours));
        self.parts = COMING.map(|set| self.layout.player(set));
        let sound = if progress.flag(flag::CLEARED + record) { SOUND_CLEARED } else { SOUND_OPENED };
        board.play(COMMON_BANK, sound, FULL_VOLUME);
    }

    /// Has it go (0x6F420, 0x6F7A0).
    pub fn leave(&mut self) {
        self.phase = Phase::Going;
        for (part, set) in self.parts.iter_mut().zip(GOING) {
            part.play(&self.layout, set);
        }
    }

    /// It is done with, whatever it was doing (0x6F95C).
    pub fn shut(&mut self) {
        self.phase = Phase::Shut;
        self.held.clear();
        self.message = None;
    }

    /// Whether whoever works it is to wait (0x6FA6C): while it comes, until cross
    /// has been pressed, and while it goes.
    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Coming | Phase::Asking | Phase::Going | Phase::Gone)
    }

    /// Whether anything of it is drawn.
    pub fn up(&self) -> bool {
        self.phase != Phase::Shut
    }

    /// A frame of it (0x704A4), and of what is said over it.
    pub fn step(&mut self, pad: &Pad, board: &mut Board) {
        if let Some(message) = &mut self.message {
            let read = message.step(pad);
            board.ask_all(message.sounds.drain(..).map(Request::common));
            if read.is_some() {
                self.message = None;
            }
        }
        let moving = match self.phase {
            Phase::Shut | Phase::Read => false,
            Phase::Coming => {
                if self.parts[CHOSEN].finished(&self.layout) {
                    self.phase = Phase::Asking;
                }
                true
            }
            Phase::Asking => {
                if pad.pressed & button::CROSS != 0 {
                    self.phase = Phase::Read;
                    board.play(COMMON_BANK, SOUND_READ, FULL_VOLUME);
                }
                true
            }
            // Gone, the music is as loud again as it was (0x705F4).
            Phase::Going => {
                if self.parts[FRAME].finished(&self.layout) {
                    self.phase = Phase::Gone;
                    board.ask(Request::MusicVolumeBack(Keeper::Record));
                }
                true
            }
            Phase::Gone => {
                self.phase = Phase::Shut;
                false
            }
        };
        if moving {
            for part in &mut self.parts {
                part.advance(&self.layout, FRAMES_A_STEP);
            }
        }
    }

    /// Writes a text where a group's object is, moved as the group's track is and
    /// in its colour (0x2D900).
    fn write(&self, canvas: &mut Canvas, fonts: &Fonts, part: usize, track: usize, words: Option<&Block>, to_the_right: bool) {
        let (layout, player) = (&self.layout, &self.parts[part]);
        let (Some(words), Some((moved, colour))) = (words, layout.placed(player, track)) else { return };
        let Some(object) = player.group(track).and_then(|group| layout.first_of(group)) else { return };
        let (Some(place), size) = (layout.place(object), layout.objects.objects[object].size) else { return };
        let tint = modulate(colour, layout.colour(object));
        let placing = Placing { tint, ..Placing::at(place + moved.translation.trunc()) };
        let placing = if to_the_right { Placing { width: size[0] as f32, align: Align::Right, ..placing } } else { placing };
        fonts.draw(canvas, layer::RECORD, words, &placing);
    }

    /// Draws a number with a track, each figure from its cell of the figures'
    /// sheet: the last where the track has its group, moved by `from`, and each
    /// one before it a figure's width and one to its left (0x70AC4).
    #[allow(clippy::too_many_arguments)]
    fn number(&self, canvas: &mut Canvas, part: usize, track: usize, number: i32, count: i32, from: f32, outer: Affine2) {
        let (layout, player) = (&self.layout, &self.parts[part]);
        let Some(width) = player.group(track).and_then(|group| layout.first_of(group)).map(|object| layout.objects.objects[object].size[0] as f32) else { return };
        for place in 0..count {
            let inner = Affine2::from_translation(Vec2::new(from + (place - count + 1) as f32 * (width + 1.0), 0.0));
            let shift = Vec2::new(FIGURE_CELL * figure(number, count, place) as f32 / self.figures, 0.0);
            layout.draw_track(canvas, layer::RECORD, player, track, &Look { inner, outer, shift, ..default() });
        }
    }

    /// Where a track has its group's object, and the colour it is drawn by.
    fn placed(&self, part: usize, track: usize) -> Option<(Vec2, [i16; 2], [u8; 4])> {
        let (layout, player) = (&self.layout, &self.parts[part]);
        let (moved, colour) = layout.placed(player, track)?;
        let object = layout.first_of(player.group(track)?)?;
        Some((layout.place(object)? + moved.translation.trunc(), layout.objects.objects[object].size, modulate(colour, layout.colour(object))))
    }

    /// Draws it (0x70694).
    pub fn draw(&self, canvas: &mut Canvas, fonts: &Fonts, records: &Records, progress: &Progress) {
        if self.phase == Phase::Shut {
            return;
        }
        let layout = &self.layout;
        let plain = Look::default();

        // All that is about the squares.
        let frame = &self.parts[FRAME];
        let number = self.cursor as i32 + 1;
        for track in 0..frame.len() {
            let Some(group) = frame.group(track) else { continue };
            match group {
                NUMBER => self.number(canvas, FRAME, track, number, figures(number), 0.0, Affine2::IDENTITY),
                // Six to the left for each figure past the first (0x70B84).
                NUMBER_LABEL => {
                    let inner = Affine2::from_translation(Vec2::new(6.0 - 6.0 * figures(number) as f32, 0.0));
                    layout.draw_track(canvas, layer::RECORD, frame, track, &Look { inner, ..plain });
                }
                group if (STAGES..STAGES + 6).contains(&group) => {
                    if group == STAGES + self.stage as usize {
                        layout.draw_track(canvas, layer::RECORD, frame, track, &plain);
                    }
                }
                CLOSE => self.write(canvas, fonts, FRAME, track, self.close_words.as_ref(), true),
                _ => layout.draw_track(canvas, layer::RECORD, frame, track, &plain),
            }
        }

        // What is told of the record the cursor is on.
        let told = &self.told;
        if matches!(told.square, Some(Square::Open | Square::Cleared)) {
            let cleared = told.square == Some(Square::Cleared);
            let known = &self.parts[KNOWN];
            for track in 0..known.len() {
                let Some(group) = known.group(track) else { continue };
                match group {
                    TO_CLEAR => self.write(canvas, fonts, KNOWN, track, told.to_clear.as_ref(), false),
                    WORTH => self.write(canvas, fonts, KNOWN, track, told.worth.as_ref(), false),
                    NAME => self.write(canvas, fonts, KNOWN, track, told.name.as_ref(), false),
                    CLEARED_MARK => {
                        if cleared {
                            layout.draw_track(canvas, layer::RECORD, known, track, &plain);
                        }
                    }
                    SMALL_PICTURE => {
                        let record = self.record_of(self.cursor, records).and_then(|number| records.record(number));
                        let (Some(record), Some((at, _, tint))) = (record, self.placed(KNOWN, track)) else { continue };
                        draw_small_picture(canvas, layer::RECORD, &self.small, record.icon, at, tint);
                    }
                    // The bar, cut to how much of its goal the count has come to
                    // (0x71158).
                    BAR => {
                        if cleared {
                            continue;
                        }
                        let Some(bar) = layout.first_of(group).map(|object| &layout.objects.objects[object]) else { continue };
                        let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32);
                        let across = (bar.size[0] as f32 * told.share).trunc();
                        if across > 0.0 {
                            canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(across, bar.size[1] as f32))));
                            layout.draw_track(canvas, layer::RECORD, known, track, &plain);
                            canvas.cut_to(None);
                        }
                    }
                    HUNDREDTHS => {
                        if !cleared {
                            self.number(canvas, KNOWN, track, (told.share * 100.0) as i32, 2, 0.0, Affine2::IDENTITY);
                        }
                    }
                    group if WHILE_COUNTING.contains(&group) => {
                        if !cleared {
                            layout.draw_track(canvas, layer::RECORD, known, track, &plain);
                        }
                    }
                    _ => layout.draw_track(canvas, layer::RECORD, known, track, &plain),
                }
            }
        } else {
            let of_one = told.square == Some(Square::Unknown);
            let unknown = &self.parts[UNKNOWN];
            for track in 0..unknown.len() {
                let Some(group) = unknown.group(track) else { continue };
                match group {
                    HINT => {
                        if of_one {
                            self.write(canvas, fonts, UNKNOWN, track, told.hint.as_ref(), false);
                        }
                    }
                    group if ONLY_OF_ONE.contains(&group) => {
                        if of_one {
                            layout.draw_track(canvas, layer::RECORD, unknown, track, &plain);
                        }
                    }
                    _ => layout.draw_track(canvas, layer::RECORD, unknown, track, &plain),
                }
            }
        }

        // The squares, down the columns (0x71AC0): each drawn by the part of the
        // one the cursor is on or by that of the others, moved to where it is.
        let moved_to = |square: usize| Vec2::new((square / 2) as f32, (square % 2) as f32) * PITCH;
        for square in 0..PAGE as usize {
            let state = self.square(square, records, progress);
            let chosen = square == self.cursor && self.record_of(0, records).is_some();
            let part = if chosen { CHOSEN } else { SQUARE };
            let (player, by) = (&self.parts[part], moved_to(square));
            let outer = Affine2::from_translation(by);
            let look = Look { outer, ..plain };
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let drawn = match group {
                    PICTURE => {
                        if matches!(state, Square::Open | Square::Cleared) {
                            self.draw_picture(canvas, part, track, square, by, records, progress);
                        }
                        false
                    }
                    UNTIL_CLEARED => matches!(state, Square::Unknown | Square::Open),
                    OF_A_RECORD => state != Square::Empty,
                    UNKNOWN_WORD => state == Square::Unknown,
                    // Its number, about the middle of where its figure is (0x71F34).
                    SQUARE_NUMBER => {
                        if state == Square::Unknown {
                            let (number, width) = (square as i32 + 1, layout.first_of(group).map_or(0, |object| layout.objects.objects[object].size[0] as i32));
                            let count = figures(number);
                            self.number(canvas, part, track, number, count, ((count - 1) * width / 2) as f32, outer);
                        }
                        false
                    }
                    _ => true,
                };
                if drawn {
                    layout.draw_track(canvas, layer::RECORD, player, track, &look);
                }
            }
        }
        // And the cursor, on its square (0x720D0).
        let outer = Affine2::from_translation(moved_to(self.cursor));
        layout.draw(canvas, layer::RECORD, &self.parts[CURSOR], &Look { outer, ..plain });
    }

    /// Draws a square's picture (0x6FDBC): that of its record's kind, the one of
    /// a record cleared once it may be seen to be, a pixel up and to the left of
    /// where the layout has it.
    #[allow(clippy::too_many_arguments)]
    fn draw_picture(&self, canvas: &mut Canvas, part: usize, track: usize, square: usize, by: Vec2, records: &Records, progress: &Progress) {
        let Some(number) = self.record_of(square, records) else { return };
        let (Some(record), Some((at, size, tint))) = (records.record(number), self.placed(part, track)) else { return };
        if !records.is_open(number, progress) {
            return;
        }
        let held = self.held.contains(&number);
        let pictures = match (progress.flag(flag::CLEARED + number), held) {
            (true, false) => &self.cleared,
            (true, true) | (false, false) => &self.open,
            (false, true) => return,
        };
        let Some((picture, whole)) = pictures.get(record.icon as usize) else { return };
        let (at, size) = (at + by - Vec2::ONE, Vec2::new(size[0] as f32, size[1] as f32));
        canvas.sprite(layer::RECORD, Paint::textured(picture), Vertex::new(at, Vec2::ZERO, tint), Vertex::new(at + size, size / *whole, tint));
    }
}

/// Draws the window, which is a task of its own (0x70694): over what the field
/// shows of the mission, or over a battle and its results.
#[allow(clippy::too_many_arguments)]
fn draw(
    window: Res<RecordWindow>,
    records: Res<Records>,
    progress: Option<Res<Progress>>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    layouts: Res<Layouts>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    if let Some(progress) = progress {
        window.draw(&mut canvas, &fonts, &records, &progress);
    }
    if let Some(message) = window.message() {
        layouts.draw_window(&mut canvas, &fonts, &colours, message, steps.overstep_fraction());
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, draw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::tables::PlayerStats;
    use crate::items::{Inventory, Items};
    use crate::records::{Event, Screen, Shown, World, OF_BATTLES};

    #[test]
    fn a_record_cleared_is_shown_and_what_it_is_worth_is_given_and_said() {
        let Some((mut window, records, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        let items = Items::load(&data).unwrap();
        let mut inventory = Inventory::new_game(&items);
        let mut stats = PlayerStats { level: 1, experience: 0, max_hp: 300.0, hp: 300.0, attack: 20, special: 20, defence: 20, burst_bullets: 3, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 };
        let mut progress = after_battles(&records, 4);
        let mut board = Board::default();
        // What the first record is worth is the flag that lets her dodge.
        const DODGE: i32 = 7500;
        assert!(!progress.flag(DODGE));
        window.hold(&records.list(OF_BATTLES, &progress));
        let mut shown = Shown::begin(OF_BATTLES, Some(1));
        let (mut said, mut frames, mut read_at) = (None, 0, Vec::new());
        loop {
            // Cross is pressed every fortieth frame.
            let pad = if frames % 40 == 39 { press(button::CROSS) } else { Pad::default() };
            let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
            let mut screen = Screen { window: &mut window, inventory: &mut inventory, items: &items, fonts: &fonts, colours: &colours, board: &mut board };
            let more = shown.step(&records, &mut world, &mut screen);
            let saying = window.saying();
            window.step(&pad, &mut board);
            if !more {
                break;
            }
            // She has it from when the window has been read, which is when it is
            // said; and what is said is read with cross too.
            assert_eq!(progress.flag(DODGE), saying || said.is_some(), "frame {frames}");
            if let Some(message) = window.message() {
                said.get_or_insert_with(|| text::plain(message.page()));
            }
            if saying && !window.saying() {
                read_at.push(frames);
            }
            frames += 1;
            assert!(frames < 1000);
        }
        assert_eq!(said.as_deref(), Some("Obtained basic skill \u{201c}Dodge\u{201d}\nYou can now Dodge using the [Circle]"));
        assert_eq!(read_at.len(), 1);
        assert!(progress.flag(DODGE) && !window.busy() && !window.up() && !window.saying());
        // It was shown as cleared, and that is not forgotten until the list is.
        assert_eq!(records.list(OF_BATTLES, &progress), [0]);
        records.shown(OF_BATTLES, &mut progress);
        assert!(records.list(OF_BATTLES, &progress).is_empty());
    }

    fn game() -> Option<(RecordWindow, Records, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let window = RecordWindow::load(&data, &mut images).unwrap();
        Some((window, Records::load(&data).unwrap(), Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    /// A game in which so many battles have been fought.
    fn after_battles(records: &Records, battles: usize) -> Progress {
        let mut progress = Progress::new_game();
        let mut stats = PlayerStats { level: 1, experience: 0, max_hp: 300.0, hp: 300.0, attack: 20, special: 20, defence: 20, burst_bullets: 3, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 };
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
        records.initialize(&mut world).unwrap();
        for _ in 0..battles {
            records.battle_finished(Event::battle(100, false, true), &mut world).unwrap();
        }
        progress
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, ..Pad::default() }
    }

    fn extent(vertices: &[Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    #[test]
    fn its_layout_has_the_parts_the_window_plays() {
        let Some((window, ..)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let layout = &window.layout;
        assert_eq!((layout.textures.len(), layout.motions.sets.len()), (14, 11));
        assert_eq!((window.open.len(), window.cleared.len()), (10, 10));
        assert_eq!(window.close, "\\b(01)CLOSE");
        // Each part's two sets move the same groups, and each group that is more
        // than something to draw is of the part it is looked for in.
        let groups = |set: usize| layout.motions.sets[set].iter().map(|track| track.group as usize).collect::<Vec<_>>();
        for (coming, going) in COMING.into_iter().zip(GOING) {
            assert_eq!(groups(coming), groups(going), "sets {coming} and {going}");
        }
        let has = |part: usize, wanted: &[usize]| wanted.iter().all(|group| groups(COMING[part]).contains(group));
        assert!(has(FRAME, &[NUMBER, NUMBER_LABEL, STAGES, STAGES + 5, CLOSE]));
        assert!(has(KNOWN, &[TO_CLEAR, WORTH, CLEARED_MARK, SMALL_PICTURE, NAME, BAR, HUNDREDTHS, 18, 20]));
        assert!(has(UNKNOWN, &[HINT, 3, 6]));
        for part in [CHOSEN, SQUARE] {
            assert!(has(part, &[PICTURE, UNTIL_CLEARED, OF_A_RECORD, UNKNOWN_WORD, SQUARE_NUMBER]));
        }
        // A square is fifty-five across with a picture of forty-five in it, and
        // eight of them, fifty-one apart, fill the screen's width.
        let object = |group: usize| &layout.objects.objects[layout.first_of(group).unwrap()];
        assert_eq!((object(40).position, object(40).size), ([34, 99], [55, 55]));
        assert_eq!((object(PICTURE).position, object(PICTURE).size), ([39, 104], [45, 45]));
    }

    #[test]
    fn it_comes_waits_for_cross_and_goes_when_it_is_told_to() {
        let Some((mut window, records, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let progress = after_battles(&records, 1);
        let mut board = Board::default();
        assert!(!window.busy() && !window.up());
        window.show(0, &records, &progress, &fonts, &colours, &mut board);
        assert!(window.busy() && window.up());
        // Cross does nothing until it has come: twelve frames for the square the
        // cursor is on to have played its set, and one more.
        let mut frames = 0;
        while window.phase == Phase::Coming {
            window.step(&press(button::CROSS), &mut board);
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!((frames, window.phase), (13, Phase::Asking));
        for _ in 0..30 {
            window.step(&Pad::default(), &mut board);
        }
        assert!(window.busy());
        // Cross lets whoever works it go on; it stays up, and nothing of it moves.
        window.step(&press(button::CROSS), &mut board);
        assert!(!window.busy() && window.up());
        let before = window.parts.clone();
        for _ in 0..10 {
            window.step(&Pad::default(), &mut board);
        }
        assert!(window.parts == before);
        // Told to go, it goes, and is no longer waited for once it has.
        window.leave();
        let mut frames = 0;
        while window.busy() {
            window.step(&Pad::default(), &mut board);
            frames += 1;
            assert!(frames < 100);
        }
        assert!(!window.up());
        let mut canvas = Canvas::default();
        window.draw(&mut canvas, &fonts, &records, &progress);
        assert!(canvas.vertices().is_empty());
    }

    #[test]
    fn it_shows_the_stages_sixteen_with_the_cursor_on_its_record() {
        let Some((mut window, records, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let progress = after_battles(&records, 1);
        let mut board = Board::default();
        window.show(0, &records, &progress, &fonts, &colours, &mut board);
        while window.phase == Phase::Coming {
            window.step(&Pad::default(), &mut board);
        }
        // The first record is open, a quarter of the way: the others of the stage
        // are not known.
        assert_eq!(window.told.square, Some(Square::Open));
        assert_eq!(window.told.share, 0.25);
        let states: Vec<Square> = (0..16).map(|square| window.square(square, &records, &progress)).collect();
        assert_eq!(states[0], Square::Open);
        assert!(states[1..].iter().all(|state| *state == Square::Unknown));
        let mut canvas = Canvas::default();
        window.draw(&mut canvas, &fonts, &records, &progress);
        let drawn = canvas.vertices().to_vec();
        // It covers the screen, and its first square's picture is where the
        // layout has it, a pixel up and to the left.
        let all = extent(&drawn);
        assert!(all.min.x <= 0.0 && all.min.y <= 0.0 && all.max.x >= 480.0 && all.max.y >= 272.0, "{all:?}");
        let corner = |at: Vec2| drawn.iter().any(|vertex| vertex.position == at);
        assert!(corner(Vec2::new(38.0, 103.0)) && corner(Vec2::new(83.0, 148.0)));
        // While it is still held back it is not known, and has no picture.
        window.hold(&[0]);
        assert_eq!(window.square(0, &records, &progress), Square::Unknown);
        let mut canvas = Canvas::default();
        window.draw(&mut canvas, &fonts, &records, &progress);
        assert!(!canvas.vertices().iter().any(|vertex| vertex.position == Vec2::new(38.0, 103.0)));
        window.reveal(0);
        assert_eq!(window.square(0, &records, &progress), Square::Open);

        // Cleared, it is told so: no bar and no hundredths, and what is heard as
        // it comes is another sound.
        let cleared = after_battles(&records, 4);
        let mut board = Board::default();
        window.show(0, &records, &cleared, &fonts, &colours, &mut board);
        assert_eq!((window.told.square, window.told.share), (Some(Square::Cleared), 1.0));
        assert_eq!(window.square(0, &records, &cleared), Square::Cleared);
        // A record of the second stage is on the second sixteen.
        window.show(19, &records, &progress, &fonts, &colours, &mut board);
        assert_eq!((window.stage, window.cursor), (1, 3));
        assert_eq!(window.told.square, Some(Square::Unknown));
        assert!(window.told.hint.is_some() && window.told.name.is_none());
    }
}
