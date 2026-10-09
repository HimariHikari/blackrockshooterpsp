//! GALLERY (`menuGalleryTask`, 0x4E734 to 0x52DE0), which the title has up alone:
//! three lists of what has been opened, the pictures to look at, the music to
//! listen to and the films to watch again. See `docs/MENU_NOTES.md`, "GALLERY".
//!
//! Across the screen stand twelve cards of the list, each with the small picture
//! of its thing; left and right move the cursor over them and the list under it, L
//! and R change the list, and cross has the thing under the cursor seen or heard.
//! A picture is looked at whole and can be moved about and made larger; a tune
//! plays while the list goes on being the list; a film is played over the screen.

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex, HEIGHT, WIDTH};
use brs_engine::layout::{Layout, Look, Player};
use brs_formats::xtc;

use super::{figures, step_cursor, text_place, write_text, Outcome, FRAMES_A_STEP, LEAVING_FRAMES};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::scene::{fade_kind, Fade, FilmAsked};
use crate::gallery::{list, Gallery, Opened, Thing, LISTS};
use crate::interface::{self, layer};
use crate::record_window::{figure, FIGURE_CELL};
use crate::sound::{self, Request};
use crate::text::TextColours;
use crate::writing::{Align, Fonts, Placing, Style};

const PACK: &str = "IF/MENU/MENU_GALLERY.GZ";
// The archive's files: what the strip says of each list and of a picture that is
// up, the layout, its own textures, and the small pictures.
const TEXTS: usize = 0;
const LAYOUT: usize = 4;
const OWN_TEXTURES: std::ops::RangeInclusive<usize> = 7..=10;
const SMALL_PICTURES: usize = 11;
/// The text the strip has while a picture is looked at (0x50708).
const TEXT_OF_A_PICTURE: usize = 3;

// Its sounds, of the bank that is always there: once it has come (0x4F31C), as
// the cursor or the list moves, as a thing is seen or heard, as it is left or a
// thing is, and for a thing that is not open (0x504CC).
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_CHOSEN: u32 = 3;
const SOUND_BACK: u32 = 5;
const SOUND_REFUSED: u32 = 7;
/// The frames it waits before its first sound (0x4F2E0).
const SOUND_AFTER: i32 = 0x16;

/// The cards across the screen, and how far each stands to the right of the one
/// before (0x51858).
const CARDS: usize = 12;
const PITCH: f32 = 32.0;

/// The layout's places: the frame, and then the cards.
mod place {
    pub const FRAME: usize = 0;
    pub const CARDS: usize = 1;
}

// The layout's sets. A card comes with one of the first four, by whether the
// cursor is on it and whether its thing is open (0x501C8), and is in one of the
// next four from then on (0x4FB74); the frame plays the rest: as L or R changes
// the list, and as the cursor goes round from one end of the list to the other.
const SET_FRAME: usize = 0;
const SET_COMES_CHOSEN_SHUT: usize = 1;
const SET_COMES_SHUT: usize = 2;
const SET_COMES_CHOSEN: usize = 3;
const SET_COMES: usize = 4;
const SET_SHUT: usize = 5;
const SET_CHOSEN_SHUT: usize = 6;
const SET_OPEN: usize = 7;
const SET_CHOSEN: usize = 8;
const SET_L: usize = 9;
const SET_R: usize = 10;
const SET_ROUND_BACK: usize = 11;
const SET_ROUND_ON: usize = 12;

/// What the groups of the layout are to the code that draws it (0x50BB0).
mod group {
    // Of the frame: what the thing is; the total, the stroke and the count of
    // those open; what only one list has, the first of three; what stands
    // before the name; the cursor's number; what a thing that is not open has
    // for a name; the name; and the three that go by the list another way.
    pub const ABOUT: usize = 6;
    pub const TOTAL: usize = 19;
    pub const STROKE: usize = 20;
    pub const COUNT: usize = 21;
    pub const OF_A_LIST: usize = 22;
    pub const BEFORE_NAME: usize = 25;
    pub const NUMBER: usize = 26;
    pub const NOT_OPEN: usize = 27;
    pub const NAME: usize = 28;
    pub const NOT_OF_THE_FIRST: usize = 31;
    pub const NOT_OF_THE_SECOND: usize = 32;
    pub const NOT_OF_THE_THIRD: usize = 33;
    // Of a card: its small picture; what one that is open has, and one of the
    // music besides, and one that is not open; and its number.
    pub const SMALL: usize = 8;
    pub const OPEN: usize = 10;
    pub const OF_MUSIC: usize = 11;
    pub const SHUT: usize = 12;
    pub const CARD_NUMBER: usize = 18;
}

// How its texts are written (0x4EEE8): the name and what the thing is in the
// text's own font, forty-eight glyphs to a line, the name set to the right of its
// box (0x271D4); the total in the capitals at ten and the count in the capitals
// at fifteen, as CHALLENGES has them.
const WORDS_STYLE: Style = Style::of_kind(0, 0x30, 6, 0.0, 4.0);
const TOTAL_STYLE: Style = Style::of_kind(2, 4, 1, 1.0, 4.0);
const COUNT_STYLE: Style = Style::of_kind(3, 4, 1, 1.0, 4.0);
const TOTAL_ROOM: f32 = 10.0;
const COUNT_ROOM: f32 = 15.0;
/// How far to the left the third list has one of the frame's groups (0x50C50).
const THIRD_LIST_MOVES: f32 = -13.0;

// The picture that is looked at (0x4FC8C, 0x504F4): how large it may be made, by
// how much a frame, and how far a frame moves it, in its own pixels.
const LARGEST: f32 = 2.0;
const GROWS_BY: f32 = 0.01;
const MOVES_BY: f32 = 8.0;
const MIDDLE: Vec2 = Vec2::new(WIDTH / 2.0, HEIGHT / 2.0);
/// A stick pushed less than this is not pushed.
const STICK_DEAD_ZONE: f32 = 0.2;

/// What the screen is drawn from.
pub struct Assets {
    layout: Layout,
    /// What the strip says: of each list, and of a picture that is up.
    strips: Vec<String>,
    /// The width of the sheet of figures (0x2CD24).
    figures: f32,
    /// The small pictures, numbered from one.
    small: Vec<(Handle<Image>, Vec2)>,
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        // The layout's eleven textures (0x50050 on): the archive's own four, three
        // places left empty, in which the small pictures are drawn, and four of
        // the pack that is always there, the second of them the figures.
        let mut textures: Vec<String> = OWN_TEXTURES.map(own).collect();
        textures.extend([own(7), own(7), own(7), shared(7), shared(9), shared(20), shared(12)]);
        let table = xtc::Table::parse(&data.read(interface::file(PACK, TEXTS, "xtc"))?).context("the texts of the GALLERY")?;
        let strip = |index: usize| -> Result<String> {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("the GALLERY has no text {index}"))?;
            Ok(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
        };
        let pack = interface::file(PACK, SMALL_PICTURES, "lpk");
        let mut small = Vec::new();
        while data.exists(&interface::file(&pack, small.len(), "ptm")) {
            small.push(interface::load_texture(data, images, &interface::file(&pack, small.len(), "ptm"))?);
        }
        Ok(Assets {
            layout: interface::layout_with(data, images, PACK, LAYOUT, &textures)?,
            strips: (0..=TEXT_OF_A_PICTURE).map(strip).collect::<Result<_>>()?,
            figures: interface::load_texture(data, images, &shared(9))?.1.x,
            small,
        })
    }
}

/// The states its task goes through (0x4EDCC), by the numbers it has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// It is made, and the screen's fade is still coming off.
    Coming = 0,
    Open = 1,
    /// A picture or a film has the screen.
    Seeing = 3,
    /// It is left: until the screen has gone dark.
    Leaving = 4,
    Done = 5,
}

/// One of a list's things, as the screen has it.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    number: u32,
    open: bool,
    small: u8,
    name: String,
    about: String,
}

/// A picture that is up: its pieces, which lie in rows of so many, how large it
/// is, and the view of it (the screen's + 0x88 on).
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pieces: Vec<(Handle<Image>, Vec2)>,
    across: usize,
    size: Vec2,
    /// The pixel of it that is in the middle of the screen, and how large it is
    /// shown; and the same as it began (+ 0x90, + 0x9C), which square puts back.
    middle: Vec2,
    scale: f32,
    fits: f32,
}

impl Picture {
    /// The picture as it is first seen (0x4FC8C): whole, in the middle.
    pub fn new(pieces: Vec<(Handle<Image>, Vec2)>, across: usize, size: Vec2) -> Picture {
        let fits = (HEIGHT / size.y).min(WIDTH / size.x);
        Picture { pieces, across: across.max(1), size, middle: (size * 0.5).trunc(), scale: fits, fits }
    }

    /// Where on the screen a pixel of it is (0x52718).
    fn on_screen(&self, pixel: Vec2) -> Vec2 {
        (pixel - self.middle) * self.scale + MIDDLE
    }

    /// A frame of looking at it (0x50750), which says whether it is done with.
    fn frame(&mut self, pad: &Pad) -> bool {
        if pad.pressed & button::SQUARE != 0 {
            (self.middle, self.scale) = ((self.size * 0.5).trunc(), self.fits);
        } else if pad.pressed & button::CIRCLE != 0 {
            return true;
        } else if pad.pressed & button::CROSS != 0 {
            // The strip is the screen's to hide.
        } else if pad.held & button::TRIANGLE != 0 {
            // Smaller with down, larger with up, the stick doing as the D-pad
            // (0x508F4: down is the stick's way forward).
            let stick = if pad.stick.length() > STICK_DEAD_ZONE { pad.stick.y } else { 0.0 };
            if pad.held & button::DOWN != 0 || stick > 0.0 {
                self.scale = (self.scale - GROWS_BY).clamp(self.fits, LARGEST);
            }
            if pad.held & button::UP != 0 || stick < 0.0 {
                self.scale = (self.scale + GROWS_BY).clamp(self.fits, LARGEST);
            }
        } else {
            // The D-pad, where it is held, in place of the stick.
            let mut by = if pad.stick.length() > STICK_DEAD_ZONE { pad.stick } else { Vec2::ZERO };
            if pad.held & button::LEFT != 0 {
                by.x = -1.0;
            }
            if pad.held & button::RIGHT != 0 {
                by.x = 1.0;
            }
            if pad.held & button::UP != 0 {
                by.y = -1.0;
            }
            if pad.held & button::DOWN != 0 {
                by.y = 1.0;
            }
            self.middle = (self.middle + by * MOVES_BY).trunc().clamp(Vec2::ZERO, self.size);
        }
        false
    }

    /// Draws its pieces (0x4F5D4): each row as high as its pieces, each piece
    /// to the right of the one before.
    fn draw(&self, canvas: &mut Canvas) {
        let mut down = 0.0;
        for row in self.pieces.chunks(self.across) {
            let mut along = 0.0;
            for (piece, size) in row {
                let (from, to) = (self.on_screen(Vec2::new(along, down)), self.on_screen(Vec2::new(along, down) + *size));
                // From the middle of its first pixel to the middle of its last, so
                // that no edge is smoothed with the edge across from it.
                let inset = Vec2::splat(0.5) / *size;
                canvas.sprite(layer::MENU_SCREEN, Paint::textured(piece), Vertex::new(from, inset, [255; 4]), Vertex::new(to, Vec2::ONE - inset, [255; 4]));
                along += size.x;
            }
            down += row.last().map_or(0.0, |(_, size)| size.y);
        }
    }
}

/// What has the screen while the state is [`State::Seeing`] (its + 0x84).
#[derive(Clone, Debug, PartialEq)]
enum Seen {
    /// A picture whose file is being read.
    Loading,
    Picture(Picture),
    Film,
}

/// The GALLERY screen.
#[derive(Clone, Debug, PartialEq)]
pub struct GalleryScreen {
    state: State,
    /// It is to be left (+ 0x18): by circle or by triangle.
    leaving: Option<Outcome>,
    /// The first card's first motion is over (flag 0x100): it takes the pad.
    ready: bool,
    /// The frames it has been waiting to sound in (+ 0x70), and whether it has
    /// sounded (flag 0x10000).
    waited: i32,
    sounded: bool,
    places: Vec<Player>,
    /// The list shown (+ 0x80), its things in order (+ 0xA4), the first of the
    /// twelve shown (+ 0x78) and which of the twelve the cursor is on (+ 0x74).
    list: usize,
    things: Vec<Shown>,
    first: usize,
    cursor: usize,
    seen: Option<Seen>,
    /// Cross has hidden the strip over a picture (flag 1).
    strip_hidden: bool,
    /// The picture whose file is to be read, the film that is to be played, and
    /// the tune that is playing (+ 0x2EC).
    wanted: Option<String>,
    pub showing: Option<FilmAsked>,
    playing: Option<u32>,
    /// Something was played (+ 0x2E8): the title has its tune begin again.
    pub played: bool,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl GalleryScreen {
    /// The screen as it is made (0x4FF74): the pictures' list, the cursor on its
    /// first thing.
    pub fn new(assets: &Assets, gallery: &Gallery, opened: &Opened) -> GalleryScreen {
        let layout = &assets.layout;
        let mut screen = GalleryScreen {
            state: State::Coming,
            leaving: None,
            ready: false,
            waited: 0,
            sounded: false,
            places: vec![layout.player(SET_FRAME)],
            list: list::PICTURES,
            things: Vec::new(),
            first: 0,
            cursor: 0,
            seen: None,
            strip_hidden: false,
            wanted: None,
            showing: None,
            playing: None,
            played: false,
            sounds: Vec::new(),
        };
        screen.build(gallery, opened);
        // Each card comes as what its thing is, the cursor's another way.
        for card in 0..CARDS {
            let open = screen.things.get(card).is_some_and(|thing| thing.open);
            let set = match (card == 0, open) {
                (true, true) => SET_COMES_CHOSEN,
                (true, false) => SET_COMES_CHOSEN_SHUT,
                (false, true) => SET_COMES,
                (false, false) => SET_COMES_SHUT,
            };
            screen.places.push(layout.player(set));
        }
        screen
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// What the strip along the foot says (0x4F7FC, 0x50708): of the list, or of
    /// a picture that is up.
    pub fn strip<'a>(&self, assets: &'a Assets) -> &'a str {
        let text = if matches!(self.seen, Some(Seen::Picture(_))) { TEXT_OF_A_PICTURE } else { self.list };
        assets.strips.get(text).map_or("", String::as_str)
    }

    /// Whether the strip is drawn: not while a film is up, nor over a picture
    /// when cross has hidden it.
    pub fn shows_strip(&self) -> bool {
        match self.seen {
            Some(Seen::Film) => false,
            Some(Seen::Picture(_)) => !self.strip_hidden,
            _ => true,
        }
    }

    /// Whether a film has the screen, when nothing of the menus is drawn (0x4F524).
    pub fn film_up(&self) -> bool {
        self.seen == Some(Seen::Film)
    }

    /// The archive of the picture that is to be read, once (0x3C028).
    pub fn take_wanted(&mut self) -> Option<String> {
        self.wanted.take()
    }

    /// The picture's file has been read (0x505B0): it is up.
    pub fn show(&mut self, picture: Picture) {
        if self.seen == Some(Seen::Loading) {
            self.seen = Some(Seen::Picture(picture));
        }
    }

    /// Makes the list (0x522D4, 0x4F978): its things in their order, with which
    /// of them are open; and the cursor and the first shown are kept within it.
    fn build(&mut self, gallery: &Gallery, opened: &Opened) {
        let shown = |thing: &Thing| Shown { number: thing.number, open: opened.has(thing.number), small: thing.small, name: thing.name.clone(), about: thing.about.clone() };
        self.things = gallery.list(self.list).into_iter().map(shown).collect();
        let count = self.things.len();
        self.first = if count <= CARDS { 0 } else { self.first.min(count - CARDS) };
        self.cursor = self.cursor.min(count.saturating_sub(1)).min(CARDS - 1);
    }

    /// The thing under the cursor.
    fn chosen(&self) -> Option<&Shown> {
        self.things.get(self.first + self.cursor)
    }

    /// The set a card is in (0x4FB74), by whether the cursor is on it and whether
    /// its thing is open.
    fn set_of(&self, card: usize) -> usize {
        let open = self.things.get(self.first + card).is_some_and(|thing| thing.open);
        match (card == self.cursor, open) {
            (true, true) => SET_CHOSEN,
            (true, false) => SET_CHOSEN_SHUT,
            (false, true) => SET_OPEN,
            (false, false) => SET_SHUT,
        }
    }

    /// A frame of its task (0x4EDCC), with the screen's fade. Answers how it
    /// ended, in the frame it does.
    pub fn frame(&mut self, assets: &Assets, gallery: &Gallery, opened: &Opened, fade: &mut Fade, pad: &Pad) -> Option<Outcome> {
        match self.state {
            State::Coming => {
                if !fade.running() {
                    self.state = State::Open;
                }
            }
            State::Open => {
                if self.leaving.is_some() {
                    fade.begin(fade_kind::OUT, LEAVING_FRAMES / FRAMES_A_STEP as i32);
                    self.state = State::Leaving;
                }
            }
            State::Leaving => {
                if !fade.running() {
                    self.state = State::Done;
                    return self.leaving;
                }
            }
            State::Seeing | State::Done => {}
        }
        if self.state == State::Seeing {
            self.see(pad);
            // The layout goes on while a picture's file is read and under a
            // film (0x5058C, 0x50B78), and stands still under a picture.
            if !matches!(self.seen, Some(Seen::Picture(_))) {
                self.advance(&assets.layout);
            }
        } else {
            self.at_the_list(assets, gallery, opened, pad);
        }
        None
    }

    /// A frame of the list (0x4F0DC).
    fn at_the_list(&mut self, assets: &Assets, gallery: &Gallery, opened: &Opened, pad: &Pad) {
        let layout = &assets.layout;
        if self.state == State::Open {
            if self.ready {
                self.take_pad(layout, gallery, opened, pad);
            } else {
                // It is ready when the first card has come (0x31C20), and sounds
                // once on the way.
                self.ready = self.places[place::CARDS].finished(layout);
                if !self.sounded {
                    if self.waited < SOUND_AFTER {
                        self.waited += FRAMES_A_STEP as i32;
                    } else {
                        self.sound(SOUND_COME);
                        (self.waited, self.sounded) = (0, true);
                    }
                }
            }
        }
        self.advance(layout);
    }

    fn advance(&mut self, layout: &Layout) {
        for place in &mut self.places {
            place.advance(layout, FRAMES_A_STEP as f32);
        }
    }

    /// What the pad does (0x4F10C).
    fn take_pad(&mut self, layout: &Layout, gallery: &Gallery, opened: &Opened, pad: &Pad) {
        let (pressed, repeat) = (pad.pressed, pad.repeat);
        if pressed & button::CROSS != 0 {
            self.choose(gallery);
        } else if pressed & button::CIRCLE != 0 {
            self.leaving = Some(Outcome::Back);
            self.sound(SOUND_BACK);
        } else if pressed & button::TRIANGLE != 0 {
            self.leaving = Some(Outcome::Out);
            self.sound(SOUND_BACK);
        } else if step_cursor(button::L, button::R, &mut self.list, LISTS, pressed, repeat) {
            // The arrow by the button that did it is nudged, and the list is the
            // other: every card is what it is in that, from its start.
            if repeat & button::R != 0 {
                self.places[place::FRAME].play(layout, SET_R);
            } else if repeat & button::L != 0 {
                self.places[place::FRAME].play(layout, SET_L);
            }
            self.build(gallery, opened);
            for card in 0..CARDS {
                self.places[place::CARDS + card] = layout.player(self.set_of(card));
            }
            self.sound(SOUND_CURSOR);
        } else if self.move_cursor(layout, pressed, repeat) {
            self.sound(SOUND_CURSOR);
        }
    }

    /// Left and right move the cursor (0x51DB0): over the twelve cards, and at an
    /// end of them the list under it by one; from the first thing back to the
    /// last, and from the last on to the first, only when the button has newly
    /// gone down. Answers whether it moved.
    fn move_cursor(&mut self, layout: &Layout, pressed: u32, repeat: u32) -> bool {
        let count = self.things.len();
        if count == 0 {
            return false;
        }
        let at = self.first + self.cursor;
        let before = self.cursor;
        if repeat & button::LEFT != 0 {
            if at == 0 {
                if pressed & button::LEFT == 0 {
                    return false;
                }
                // Round to the last (0x51E6C): the cursor on the last card there
                // is, and the list as far on as it goes.
                (self.cursor, self.first) = ((count - 1).min(CARDS - 1), count.saturating_sub(CARDS));
                self.begin_all(layout);
                self.places[place::FRAME].play(layout, SET_ROUND_BACK);
            } else if self.cursor > 0 {
                self.cursor -= 1;
                self.play_card(layout, before);
                self.play_card(layout, self.cursor);
            } else {
                self.first -= 1;
                self.hand_on(layout, false);
                self.play_card(layout, self.cursor);
                self.places[place::FRAME].play(layout, SET_ROUND_BACK);
            }
        } else if repeat & button::RIGHT != 0 {
            if at + 1 >= count {
                if pressed & button::RIGHT == 0 {
                    return false;
                }
                (self.cursor, self.first) = (0, 0);
                self.begin_all(layout);
                self.places[place::FRAME].play(layout, SET_ROUND_ON);
            } else if self.cursor + 1 < CARDS {
                self.cursor += 1;
                self.play_card(layout, before);
                self.play_card(layout, self.cursor);
            } else {
                self.first += 1;
                self.hand_on(layout, true);
                self.play_card(layout, self.cursor);
                self.places[place::FRAME].play(layout, SET_ROUND_ON);
            }
        } else {
            return false;
        }
        true
    }

    /// A card plays the set it is in now (0x4FB74).
    fn play_card(&mut self, layout: &Layout, card: usize) {
        let set = self.set_of(card);
        self.places[place::CARDS + card].play(layout, set);
    }

    /// Every card is what it is now, from the start of its set (0x4FC38).
    fn begin_all(&mut self, layout: &Layout) {
        for card in 0..CARDS {
            self.places[place::CARDS + card] = layout.player(self.set_of(card));
        }
    }

    /// The list has moved by one under the cursor (0x4FAA4): each card but the
    /// one a thing has newly come to is given the set it is in now, as far into
    /// it as the card its thing was on had got, so that what plays goes on with
    /// the thing. The cursor's card is begun anew by who calls this.
    fn hand_on(&mut self, layout: &Layout, on: bool) {
        let frames: Vec<f32> = (0..CARDS).map(|card| self.places[place::CARDS + card].frame()).collect();
        let cards = if on { 0..CARDS - 1 } else { 1..CARDS };
        for card in cards {
            let from = if on { card + 1 } else { card - 1 };
            self.places[place::CARDS + card] = Player::at(layout, self.set_of(card), frames[from]);
        }
    }

    /// Cross on a thing (0x50294).
    fn choose(&mut self, gallery: &Gallery) {
        let Some((number, open)) = self.chosen().map(|thing| (thing.number, thing.open)) else { return };
        let Some(thing) = gallery.all().find(|thing| thing.number == number).filter(|_| open) else {
            self.sound(SOUND_REFUSED);
            return;
        };
        match self.list {
            list::PICTURES => {
                self.wanted = Some(thing.path());
                (self.seen, self.strip_hidden) = (Some(Seen::Loading), false);
                self.state = State::Seeing;
            }
            list::MUSIC => {
                // The one that is playing is stopped; any other is played, at
                // full volume (0xA204, 0x9E84).
                if self.playing == Some(number) {
                    self.sounds.push(Request::StopMusic(0));
                    self.playing = None;
                } else {
                    self.sounds.extend([Request::MusicVolume(0x7f, 0.0), Request::Music(sound::tune_file(&thing.file))]);
                    (self.playing, self.played) = (Some(number), true);
                }
            }
            _ => {
                // What plays stops for the film, which is started as soon as it
                // can be (0x11863C, 0x1188E4).
                self.sounds.push(Request::StopMusic(0));
                self.showing = Some(FilmAsked::new(format!("movie/{}.pmf", thing.file)));
                (self.playing, self.played, self.seen) = (None, true, Some(Seen::Film));
                self.state = State::Seeing;
            }
        }
        self.sound(SOUND_CHOSEN);
    }

    /// A frame with a picture or a film up (0x504F4).
    fn see(&mut self, pad: &Pad) {
        match &mut self.seen {
            Some(Seen::Picture(picture)) => {
                if pad.pressed & (button::SQUARE | button::CIRCLE) == 0 && pad.pressed & button::CROSS != 0 {
                    self.strip_hidden = !self.strip_hidden;
                }
                if picture.frame(pad) {
                    // The file is let go, and the list is back (0x507A8).
                    self.sound(SOUND_BACK);
                    (self.seen, self.state) = (None, State::Open);
                }
            }
            Some(Seen::Film) => {
                // It is started when it can be; cross or circle ends it; and
                // when it is over the list is back (0x50AD8).
                let Some(film) = &mut self.showing else {
                    (self.seen, self.state) = (None, State::Open);
                    return;
                };
                film.started |= film.ready;
                let ended = pad.pressed & (button::CROSS | button::CIRCLE) != 0;
                if ended || film.over {
                    self.showing = None;
                    (self.seen, self.state) = (None, State::Open);
                    if ended {
                        self.sound(SOUND_BACK);
                    }
                }
            }
            // The picture's file is waited for; the layout goes on meanwhile.
            Some(Seen::Loading) | None => {}
        }
    }
}

impl GalleryScreen {
    /// Draws a number with a track, each figure from its cell of the figures'
    /// sheet: the last where the track has its group, moved by `from`, and each
    /// one before it a figure's width and one to its left (0x50EC8, 0x518D4).
    #[allow(clippy::too_many_arguments)]
    fn number(canvas: &mut Canvas, assets: &Assets, player: &Player, track: usize, number: i32, from: f32, outer: Affine2) {
        let layout = &assets.layout;
        let Some(width) = player.group(track).and_then(|group| layout.first_of(group)).map(|object| layout.objects.objects[object].size[0] as f32) else { return };
        let count = figures(number);
        for place in 0..count {
            let inner = Affine2::from_translation(Vec2::new(from + (place - count + 1) as f32 * (width + 1.0), 0.0));
            let shift = Vec2::new(FIGURE_CELL * figure(number, count, place) as f32 / assets.figures, 0.0);
            layout.draw_track(canvas, layer::MENU_SCREEN, player, track, &Look { inner, outer, shift, ..default() });
        }
    }

    /// Draws it as it is `ahead` of a frame of the game's logic past its last one
    /// (0x4F4E4). The strip along the foot, which is drawn last, is the menus'.
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, fonts: &Fonts, colours: &TextColours, ahead: f32) {
        match &self.seen {
            Some(Seen::Picture(picture)) => picture.draw(canvas),
            Some(Seen::Film) => {}
            _ => self.draw_list(canvas, assets, fonts, colours, ahead),
        }
    }

    /// Draws the list (0x50BB0): the frame, and the twelve cards.
    fn draw_list(&self, canvas: &mut Canvas, assets: &Assets, fonts: &Fonts, colours: &TextColours, ahead: f32) {
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

        // The total and the count of those open, as they are written (0x522D4):
        // each is set to the right in the room it has.
        let total = fonts.write_plain(&TOTAL_STYLE, &self.things.len().to_string(), colours);
        let count = fonts.write_plain(&COUNT_STYLE, &self.things.iter().filter(|thing| thing.open).count().to_string(), colours);
        let total_from = (TOTAL_ROOM - total.size().x).trunc();
        let count_from = (COUNT_ROOM - count.size().x).trunc();

        // The name is set to the right of where its group is, and what stands
        // before it stands as far to the left as it is wide: or as the words a
        // thing that is not open has in its place are (0x50DEC).
        let chosen = self.chosen();
        let open = chosen.is_some_and(|thing| thing.open);
        let name = chosen.filter(|_| open).map(|thing| fonts.write(&WORDS_STYLE, &crate::text::pages(&thing.name, &crate::text::Plain).swap_remove(0), colours));
        let not_open = layout.first_of(group::NOT_OPEN).map_or(0.0, |object| layout.objects.objects[object].size[0] as f32);
        let name_width = name.as_ref().map_or(not_open, |name| name.size().x.trunc());

        let frame = &places[place::FRAME];
        let number = (self.first + self.cursor) as i32 + 1;
        for track in 0..frame.len() {
            let Some(group) = frame.group(track) else { continue };
            match group {
                group::ABOUT => {
                    if let (Some((at, tint)), Some(thing)) = (text_place(layout, frame, track, 0.0), chosen.filter(|_| open)) {
                        write_text(canvas, layer, fonts, colours, &WORDS_STYLE, &thing.about, at, tint);
                    }
                }
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
                group if (group::OF_A_LIST..group::OF_A_LIST + LISTS).contains(&group) => {
                    if group == group::OF_A_LIST + self.list {
                        layout.draw_track(canvas, layer, frame, track, &plain);
                    }
                }
                group::BEFORE_NAME => layout.draw_track(canvas, layer, frame, track, &across(-name_width)),
                group::NUMBER => GalleryScreen::number(canvas, assets, frame, track, number, -name_width, Affine2::IDENTITY),
                group::NOT_OPEN => {
                    if !open {
                        layout.draw_track(canvas, layer, frame, track, &plain);
                    }
                }
                group::NAME => {
                    let Some(object) = layout.first_of(group).map(|object| &layout.objects.objects[object]) else { continue };
                    if let (Some((at, tint)), Some(name)) = (text_place(layout, frame, track, 0.0), &name) {
                        fonts.draw(canvas, layer, name, &Placing { tint, width: object.size[0] as f32, align: Align::Right, ..Placing::at(at) });
                    }
                }
                group::NOT_OF_THE_FIRST => {
                    if self.list != list::PICTURES {
                        layout.draw_track(canvas, layer, frame, track, &plain);
                    }
                }
                group::NOT_OF_THE_SECOND => {
                    if self.list != list::MUSIC {
                        layout.draw_track(canvas, layer, frame, track, &across(if self.list == list::FILMS { THIRD_LIST_MOVES } else { 0.0 }));
                    }
                }
                group::NOT_OF_THE_THIRD => {
                    if self.list != list::FILMS {
                        layout.draw_track(canvas, layer, frame, track, &plain);
                    }
                }
                _ => layout.draw_track(canvas, layer, frame, track, &plain),
            }
        }

        // The cards (0x51834): each by its own place, moved to where it is.
        for card in 0..CARDS {
            let thing = self.things.get(self.first + card);
            let open = thing.is_some_and(|thing| thing.open);
            let by = Vec2::new(card as f32 * PITCH, 0.0);
            let outer = Affine2::from_translation(by);
            let player = &places[place::CARDS + card];
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let drawn = match group {
                    group::SMALL => {
                        if let (true, Some(thing)) = (open, thing) {
                            self.draw_small(canvas, assets, player, track, thing.small, by);
                        }
                        false
                    }
                    group::OPEN => open,
                    group::OF_MUSIC => open && self.list == list::MUSIC,
                    group::SHUT => !open,
                    group::CARD_NUMBER => {
                        GalleryScreen::number(canvas, assets, player, track, (self.first + card) as i32 + 1, 0.0, outer);
                        false
                    }
                    _ => true,
                };
                if drawn {
                    layout.draw_track(canvas, layer, player, track, &Look { outer, ..plain });
                }
            }
        }
    }

    /// Draws a thing's small picture in the rectangle its group has (0x4F678).
    fn draw_small(&self, canvas: &mut Canvas, assets: &Assets, player: &Player, track: usize, small: u8, by: Vec2) {
        let layout = &assets.layout;
        let Some((picture, _)) = usize::from(small).checked_sub(1).and_then(|index| assets.small.get(index)) else { return };
        let Some((at, tint)) = text_place(layout, player, track, 0.0) else { return };
        let Some(object) = layout.first_of(group::SMALL).map(|object| &layout.objects.objects[object]) else { return };
        let (at, size) = (at + by, Vec2::new(object.size[0] as f32, object.size[1] as f32));
        canvas.sprite(layer::MENU_SCREEN, Paint::textured(picture), Vertex::new(at, Vec2::ZERO, tint), Vertex::new(at + size, Vec2::ONE, tint));
    }
}

/// Reads the pieces of a picture's archive (0x505B8), for [`GalleryScreen::show`].
pub fn read_picture(data: &GameData, images: &mut bevy::prelude::Assets<Image>, gallery: &Gallery, archive: &str) -> Result<Picture> {
    let thing = gallery.all().find(|thing| thing.list == list::PICTURES && thing.path() == archive).with_context(|| format!("the gallery has no picture {archive}"))?;
    let mut pieces = Vec::new();
    while data.exists(&interface::file(archive, pieces.len(), "ptm")) {
        pieces.push(interface::load_texture(data, images, &interface::file(archive, pieces.len(), "ptm"))?);
    }
    Ok(Picture::new(pieces, usize::from(thing.across), thing.size.as_vec2()))
}

#[cfg(test)]
mod tests {
    use super::super::testing::{common, game, press, Game};
    use super::super::{Alone, Menu};
    use super::*;

    fn gallery(menu: &Menu) -> Option<&GalleryScreen> {
        match &menu.single.as_ref()?.screen {
            Alone::Gallery(screen) => Some(screen),
            Alone::Options(_) => None,
        }
    }

    /// The title makes it, and it is stepped until it takes the pad.
    fn come(game: &mut Game) -> Menu {
        let mut menu = Menu::default();
        menu.open_gallery(&game.assets, &game.gallery, &game.opened);
        let mut steps = 0;
        while !gallery(&menu).unwrap().ready {
            assert!(!game.step(&mut menu, &Pad::default()));
            steps += 1;
            assert!(steps < 400, "it never takes the pad");
        }
        menu.sounds.clear();
        menu
    }

    #[test]
    fn the_layouts_sets_are_what_the_code_takes_them_for() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets.gallery;
        let layout = &assets.layout;
        assert_eq!((layout.textures.len(), layout.motions.sets.len(), assets.small.len(), assets.strips.len()), (11, 13, 146, 4));
        // What the strip says, each with the mark of its button before it.
        assert_eq!(assets.strips[..3], ["\\b(01)View", "\\b(01)Play/Stop", "\\b(01)Play/Stop"]);
        assert!(assets.strips[3].contains("Zoom in") && assets.strips[3].contains("Reset"));
        // The frame's set has all the groups the code draws by; a card's sets
        // all have the same ten; the four a card comes with are fifty frames,
        // and the four nudges of the frame have a track each.
        let groups = |set: usize| {
            let player = layout.player(set);
            (0..player.len()).filter_map(|track| player.group(track)).collect::<Vec<usize>>()
        };
        let longest = |set: usize| (0..layout.motions.sets[set].len()).map(|track| layout.track_length(set, track)).fold(0.0, f32::max);
        for group in [group::ABOUT, group::TOTAL, group::STROKE, group::COUNT, group::OF_A_LIST, group::OF_A_LIST + 2, group::BEFORE_NAME, group::NUMBER, group::NOT_OPEN, group::NAME, group::NOT_OF_THE_THIRD] {
            assert!(groups(SET_FRAME).contains(&group), "{group}");
        }
        for set in SET_COMES_CHOSEN_SHUT..=SET_CHOSEN {
            assert_eq!(groups(set), [group::SHUT, group::SMALL, group::OF_MUSIC, group::OPEN, 13, 14, 15, 16, 17, group::CARD_NUMBER], "{set}");
        }
        assert_eq!([SET_COMES_CHOSEN_SHUT, SET_COMES_SHUT, SET_COMES_CHOSEN, SET_COMES].map(longest), [50.0; 4]);
        assert_eq!([SET_L, SET_R, SET_ROUND_BACK, SET_ROUND_ON].map(|set| (groups(set).len(), longest(set))), [(1, 10.0); 4]);
    }

    /// What of the gallery a test has open: the first three pictures and the
    /// fourteenth, the first two tunes and the first film.
    fn some_open(game: &mut Game) {
        for number in [1, 2, 3, 14, 129, 130, 169] {
            game.opened.open(number);
        }
    }

    #[test]
    fn it_comes_under_the_screens_fade_and_takes_the_pad_once_its_first_card_has_come() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        some_open(&mut game);
        let mut menu = Menu::default();
        menu.open_gallery(&game.assets, &game.gallery, &game.opened);
        let made = gallery(&menu).unwrap().clone();
        assert_eq!((made.state, made.list, made.things.len(), made.first, made.cursor), (State::Coming, list::PICTURES, 83, 0, 0));
        // The cursor's card comes as one whose thing is open, the fourth as one
        // whose thing is not.
        assert_eq!((made.places.len(), made.places[place::CARDS].set(), made.places[place::CARDS + 1].set(), made.places[place::CARDS + 3].set()), (1 + CARDS, SET_COMES_CHOSEN, SET_COMES, SET_COMES_SHUT));
        // A frame of it is two of the interface's: it is open when the fade is
        // off, in its eighth; it sounds when twenty-two of the interface's have
        // gone by since; and it takes the pad in the frame after its first
        // card's fifty are over.
        let (mut steps, mut sounded_at, mut open_at) = (0, None, None);
        while !gallery(&menu).unwrap().ready {
            assert!(!game.step(&mut menu, &press(button::CROSS | button::RIGHT)));
            steps += 1;
            if gallery(&menu).unwrap().state == State::Open {
                open_at.get_or_insert(steps);
            }
            if !menu.sounds.is_empty() {
                assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_COME]);
                assert_eq!(sounded_at.replace(steps), None);
            }
            assert!(steps < 100);
        }
        assert_eq!((open_at, sounded_at, steps), (Some(8), Some(19), 26));
        assert_eq!((gallery(&menu).unwrap().cursor, gallery(&menu).unwrap().seen.is_none()), (0, true));
    }

    #[test]
    fn left_and_right_move_the_cursor_and_the_list_under_it_and_l_and_r_change_the_list() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        some_open(&mut game);
        let mut menu = come(&mut game);
        let at = |menu: &Menu| (gallery(menu).unwrap().first, gallery(menu).unwrap().cursor);
        let sets = |menu: &Menu, cards: [usize; 2]| cards.map(|card| gallery(menu).unwrap().places[place::CARDS + card].set());
        // Right: the next card, each of the two in the set it is in now.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((at(&menu), sets(&menu, [0, 1]), common(&std::mem::take(&mut menu.sounds))), ((0, 1), [SET_OPEN, SET_CHOSEN], vec![SOUND_CURSOR]));
        // On to the last of the twelve, and then the list moves under it.
        for _ in 0..10 {
            game.step(&mut menu, &press(button::RIGHT));
        }
        assert_eq!((at(&menu), sets(&menu, [10, 11])), ((0, 11), [SET_SHUT, SET_CHOSEN_SHUT]));
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((at(&menu), gallery(&menu).unwrap().places[place::FRAME].set()), ((1, 11), SET_ROUND_ON));
        // Once more, and the fourteenth picture, which is open, is under the
        // cursor, with the thirteenth, which is not, on the card before.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((at(&menu), sets(&menu, [10, 11])), ((2, 11), [SET_SHUT, SET_CHOSEN]));
        // Left from the first goes round to the last only on a new press.
        let mut menu = come(&mut game);
        game.step(&mut menu, &Pad { held: button::LEFT, repeat: button::LEFT, ..Pad::default() });
        assert_eq!((at(&menu), menu.sounds.len()), ((0, 0), 0));
        game.step(&mut menu, &press(button::LEFT));
        assert_eq!((at(&menu), gallery(&menu).unwrap().places[place::FRAME].set()), ((83 - CARDS, CARDS - 1), SET_ROUND_BACK));
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!(at(&menu), (0, 0));
        menu.sounds.clear();

        // R: the music, thirty of it, the cursor and the list where they were;
        // the strip says what cross does there.
        game.step(&mut menu, &press(button::RIGHT));
        game.step(&mut menu, &press(button::R));
        let music = gallery(&menu).unwrap();
        assert_eq!((music.list, music.things.len(), music.first, music.cursor, music.places[place::FRAME].set()), (list::MUSIC, 30, 0, 1, SET_R));
        assert_eq!((music.strip(&game.assets.gallery), common(&std::mem::take(&mut menu.sounds))), ("\\b(01)Play/Stop", vec![SOUND_CURSOR, SOUND_CURSOR]));
        assert_eq!(sets(&menu, [0, 1, ]), [SET_OPEN, SET_CHOSEN]);
        // L from the pictures goes round to the films.
        game.step(&mut menu, &press(button::L));
        game.step(&mut menu, &press(button::L));
        assert_eq!((gallery(&menu).unwrap().list, gallery(&menu).unwrap().things.len()), (list::FILMS, 33));
    }

    #[test]
    fn cross_has_a_thing_seen_or_heard_if_it_is_open() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        some_open(&mut game);
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        let mut images = bevy::prelude::Assets::<Image>::default();
        let mut menu = come(&mut game);
        // The fourth picture is not open: cross is refused.
        for _ in 0..3 {
            game.step(&mut menu, &press(button::RIGHT));
        }
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((common(&std::mem::take(&mut menu.sounds)), gallery(&menu).unwrap().state), (vec![SOUND_REFUSED], State::Open));

        // The first is: its file is asked for, and once that is read it is up,
        // whole and in the middle, and the strip says what can be done with it.
        for _ in 0..3 {
            game.step(&mut menu, &press(button::LEFT));
        }
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((common(&std::mem::take(&mut menu.sounds)), gallery(&menu).unwrap().state, gallery(&menu).unwrap().seen.clone()), (vec![SOUND_CHOSEN], State::Seeing, Some(Seen::Loading)));
        menu.read_picture(&data, &mut images, &game.gallery);
        let Some(Seen::Picture(picture)) = gallery(&menu).unwrap().seen.clone() else { panic!("the picture should be up") };
        assert_eq!((picture.pieces.len(), picture.across, picture.size, picture.middle), (8, 4, Vec2::new(2048.0, 884.0), Vec2::new(1024.0, 442.0)));
        assert!((picture.scale - 480.0 / 2048.0).abs() < 1e-6 && picture.on_screen(Vec2::ZERO).abs_diff_eq(Vec2::new(0.0, 136.0 - 442.0 * 480.0 / 2048.0), 1e-3));
        assert!(gallery(&menu).unwrap().strip(&game.assets.gallery).contains("Zoom in") && gallery(&menu).unwrap().shows_strip());
        // Its pieces are drawn in two rows of four, the second under the first.
        let mut canvas = Canvas::default();
        gallery(&menu).unwrap().draw(&mut canvas, &game.assets.gallery, &game.fonts, &game.colours, 0.0);
        let corner = |piece: usize| canvas.drawn_from(&picture.pieces[piece].0).iter().fold(Vec2::MAX, |at, vertex| at.min(vertex.position));
        let scale = 480.0 / 2048.0;
        assert!(corner(1).abs_diff_eq(picture.on_screen(Vec2::new(512.0, 0.0)), 1e-3) && corner(4).abs_diff_eq(picture.on_screen(Vec2::new(0.0, 512.0)), 1e-3));
        assert!((corner(4).y - corner(0).y - 512.0 * scale).abs() < 1e-3);

        // Triangle and up make it larger, by a hundredth a frame and no larger
        // than twice its own size; the D-pad moves it eight of its pixels a frame,
        // no further than its edge; square puts it back; cross hides the strip.
        let held = |buttons: u32| Pad { held: buttons, ..Pad::default() };
        game.step(&mut menu, &held(button::TRIANGLE | button::UP));
        let seen = |menu: &Menu| match gallery(menu).unwrap().seen.clone() {
            Some(Seen::Picture(picture)) => (picture.middle, picture.scale),
            _ => panic!("the picture should be up"),
        };
        assert!((seen(&menu).1 - scale - GROWS_BY).abs() < 1e-6);
        for _ in 0..300 {
            game.step(&mut menu, &held(button::TRIANGLE | button::UP));
        }
        assert_eq!(seen(&menu), (Vec2::new(1024.0, 442.0), LARGEST));
        game.step(&mut menu, &held(button::LEFT | button::DOWN));
        assert_eq!(seen(&menu).0, Vec2::new(1016.0, 450.0));
        for _ in 0..200 {
            game.step(&mut menu, &held(button::LEFT | button::DOWN));
        }
        assert_eq!(seen(&menu).0, Vec2::new(0.0, 884.0));
        game.step(&mut menu, &press(button::SQUARE));
        assert_eq!(seen(&menu), (Vec2::new(1024.0, 442.0), scale));
        game.step(&mut menu, &press(button::CROSS));
        assert!(!gallery(&menu).unwrap().shows_strip());
        // Circle: the list again, the cursor where it was, and nothing played.
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CIRCLE));
        let back = gallery(&menu).unwrap();
        assert_eq!((back.state, back.seen.is_none(), back.cursor, back.played, common(&menu.sounds)), (State::Open, true, 0, false, vec![SOUND_BACK]));

        // A tune plays while the list goes on; cross on it again stops it, and
        // on another plays that.
        game.step(&mut menu, &press(button::R));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        let tune = |name: &str| Request::Music(sound::tune_file(name));
        assert_eq!(std::mem::take(&mut menu.sounds), [Request::MusicVolume(0x7f, 0.0), tune("title"), Request::common(SOUND_CHOSEN)]);
        assert_eq!((gallery(&menu).unwrap().state, gallery(&menu).unwrap().playing, gallery(&menu).unwrap().played), (State::Open, Some(129), true));
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((std::mem::take(&mut menu.sounds), gallery(&menu).unwrap().playing), (vec![Request::StopMusic(0), Request::common(SOUND_CHOSEN)], None));
        game.step(&mut menu, &press(button::RIGHT));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!((std::mem::take(&mut menu.sounds)[1].clone(), gallery(&menu).unwrap().playing), (tune("system1"), Some(130)));

        // A film has the screen: what plays stops, it is started as soon as it
        // can be, and when it is over the list is back.
        game.step(&mut menu, &press(button::LEFT));
        game.step(&mut menu, &press(button::R));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(std::mem::take(&mut menu.sounds), [Request::StopMusic(0), Request::common(SOUND_CHOSEN)]);
        assert_eq!(menu.showing().cloned(), Some(FilmAsked::new("movie/BRS_OP_final.pmf".to_owned())));
        assert!(gallery(&menu).unwrap().film_up() && !gallery(&menu).unwrap().shows_strip());
        game.step(&mut menu, &Pad::default());
        assert!(!menu.showing().unwrap().started);
        menu.showing().unwrap().ready = true;
        game.step(&mut menu, &Pad::default());
        assert!(menu.showing().unwrap().started && gallery(&menu).unwrap().film_up());
        menu.showing().unwrap().over = true;
        game.step(&mut menu, &Pad::default());
        assert!(menu.showing().is_none() && !gallery(&menu).unwrap().film_up() && gallery(&menu).unwrap().state == State::Open && menu.sounds.is_empty());
        // Cross or circle ends one that is playing.
        game.step(&mut menu, &press(button::CROSS));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!((menu.showing().is_none(), common(&std::mem::take(&mut menu.sounds)), gallery(&menu).unwrap().state), (true, vec![SOUND_BACK], State::Open));

        // Circle leaves it: the screen's fade begins in the frame after, takes
        // seven of the game's, and then it is done with. It says that something
        // was played: the title's tune begins again.
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_BACK]);
        let mut steps = 1;
        while !game.step(&mut menu, &press(button::RIGHT)) {
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!((steps, menu.is_open(), menu.tune_again), (8, false, true));
    }

    #[test]
    fn it_draws_the_cards_with_their_small_pictures_and_what_is_told_of_the_cursors() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        some_open(&mut game);
        let menu = come(&mut game);
        let assets = &game.assets.gallery;
        let mut canvas = Canvas::default();
        gallery(&menu).unwrap().draw(&mut canvas, assets, &game.fonts, &game.colours, 0.0);
        // The small pictures of the three that are open, each in its card's
        // window, thirty-two apart; none of the fourth's.
        let corner = |small: usize| canvas.drawn_from(&assets.small[small].0).iter().fold(Vec2::MAX, |at, vertex| at.min(vertex.position));
        let size = |small: usize| canvas.drawn_from(&assets.small[small].0).iter().fold(Vec2::MIN, |at, vertex| at.max(vertex.position)) - corner(small);
        assert_eq!(((corner(1) - corner(0)).x, corner(2) - corner(1), size(1)), (PITCH, Vec2::new(PITCH, 0.0), Vec2::new(31.0, 86.0)));
        assert!(canvas.drawn_from(&assets.small[3].0).is_empty());
    }
}
