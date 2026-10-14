//! The pause ("Pause", 0x12CFF0 to 0x12E128, and its interface, 0x69810 to
//! 0x6B150): what START brings up. See "The start window" in
//! `docs/MENU_NOTES.md`.
//!
//! It is one thing wherever it is asked for, made with a kind that says what
//! can be done from it. On the field the mission can be given up for the screen
//! between missions (triangle, which asks first) and a game loaded (square); in
//! a scene the scene can be skipped (cross); in a battle there is nothing but to
//! go on. START goes on in all of them.
//!
//! While it is up the game is drawn sixty times a second (0x808) and nothing
//! else of it has a frame: so a frame of the port's logic is two of the pause's.
//! What was playing stands still, the tune, what was being said and the sounds,
//! and the pause's own sounds are heard over that.
//!
//! Its layout is a white sheet over the screen as it stood, with the word PAUSE,
//! and on the right the records whose counts have gone up (the records' list 2),
//! four rows of them at a time: each with its small picture, its name, how far
//! it has come in hundredths or the word CLEAR, and a bar of that. At the foot
//! are the buttons there are. In a scene and in a battle the list is left out.

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Seen};
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_formats::xtc;

use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::{draw_window_on, RESIDENT_PACK_WINDOW};
use crate::field::window::Window;
use crate::interface::{self, layer};
use crate::progress::Progress;
use crate::record_window::draw_small_picture;
use crate::records::{Records, COUNT};
use crate::sound::Request;
use crate::text::{self, TextColours};
use crate::writing::{Align, Block, Fonts, Placing, Style};

/// The pack its layout is in (0x2CCC4) and the file of it the layout starts at
/// (0x6A34C); and the system's texts, which its words are of (0x97DC with 4).
const PACK: &str = "SYSTEM.VOL/if_fldsys.lpk";
const LAYOUT: usize = 26;
const TEXTS: &str = "SYSTEM.VOL/msg_system.bin";

// The system's texts it has: the two answers, the question, and the words at
// the foot for each way it can be up in (0x6AF94).
const TEXT_YES: usize = 0;
const TEXT_NO: usize = 1;
const TEXT_GIVE_UP: usize = 59;
const HINTS: [Option<usize>; 6] = [None, Some(57), Some(58), Some(61), None, Some(62)];
/// The ways the list is neither drawn nor the pad's in (its flag 2).
const WAYS_WITHOUT_LIST: [usize; 3] = [1, 3, 4];

/// The frames of the pause in a frame of the port's logic.
const FRAMES_A_STEP: usize = 2;
/// The frames it comes for before it takes a button (0x69E04).
const COMING_FRAMES: i32 = 20;
/// The rows of the list, how far apart they are (0x6A8B8), and how many records
/// there have to be for the list to have its knob (0x69EE0).
const ROWS: usize = 4;
const ROW_PITCH: f32 = 20.0;
const MANY: usize = 5;
/// The frames the screen takes to go dark once the mission is given up
/// (0x12DC48).
const DARK_FRAMES: i32 = 60;
/// The answer that gives the mission up, and the one the cursor starts on and
/// circle goes to (0x6BC84, 0x6BC94).
const YES: usize = 0;
const NO: i32 = 1;

/// The sets of its layout (0x69E8C): all of it coming and going; the list's
/// knob and what it runs in, for a list of more rows than are shown; and the
/// sheet going black, for a scene that is skipped.
mod set {
    pub const COMES: usize = 0;
    pub const GOES: usize = 1;
    pub const KNOB_COMES: usize = 2;
    pub const KNOB_GOES: usize = 3;
    pub const DARKENS: usize = 4;
}

/// The groups of its layout that are more than something to draw (0x6A5D0).
mod group {
    /// The sheet over the screen.
    pub const SHEET: usize = 1;
    /// The list's: from what heads it to the rows without a record, and the bar
    /// of a record that is not cleared (0x167FD8).
    pub const OF_THE_LIST: std::ops::RangeInclusive<usize> = 15..=30;
    // A row's small picture, its name and the mark before its figures: the first
    // track each has is for a record that is cleared, the second for one that
    // is not.
    pub const SMALL_PICTURE: usize = 16;
    pub const NAME: usize = 17;
    pub const MARK: usize = 18;
    // The hundredths and their sign, of a record that is not cleared; the word
    // CLEAR of one that is.
    pub const HUNDREDTHS: usize = 19;
    pub const PER_CENT: usize = 20;
    pub const CLEAR: usize = 21;
    // The bar: its ground, all of it for a record cleared, what shines on it,
    // and as much of it as the count has come to for one that is not.
    pub const BAR_GROUND: usize = 22;
    pub const BAR_CLEARED: usize = 23;
    pub const BAR_SHINE: [usize; 2] = [24, 25];
    pub const BAR: usize = 40;
    /// The knob of the list.
    pub const KNOB: usize = 27;
    /// What a row without a record has.
    pub const NO_RECORD: std::ops::RangeInclusive<usize> = 28..=30;
    /// The buttons' pictures, which are never drawn: the words have them.
    pub const BUTTONS: std::ops::RangeInclusive<usize> = 31..=33;
    /// Where the words at the foot are written.
    pub const HINT: usize = 34;
}

// Its sounds, of the bank that is always there: as it comes (0x69F14), as it
// goes (0x69908) or goes with a scene skipped (0x6993C), and as the list moves
// (0x6A198).
const SOUND_COMES: u32 = 0;
const SOUND_GOES: u32 = 1;
const SOUND_LIST: u32 = 2;
const SOUND_SKIPPED: u32 = 3;

// How its texts are written (0x6A41C to 0x6A528): the words at the foot in the
// capitals at ten, eighty to a line and two apart, to the right of their place;
// a record's name in the text's own font, forty to a line; its hundredths in
// the capitals at five, to the right of theirs.
const HINT_STYLE: Style = Style::of_kind(2, 0x50, 1, 2.0, 0.0);
const NAME_STYLE: Style = Style::of_kind(0, 0x28, 1, 0.0, 0.0);
const HUNDREDTHS_STYLE: Style = Style::of_kind(1, 5, 1, 1.0, 0.0);

/// What the pause is drawn from, and its words.
#[derive(Resource)]
pub struct PauseAssets {
    layout: Layout,
    /// The window its question is asked in.
    window: Layout,
    /// The sheet of the records' small pictures.
    small: (Handle<Image>, Vec2),
    yes: String,
    no: String,
    question: String,
    /// The words at the foot, by the way it is up in.
    hints: [Option<String>; 6],
}

impl PauseAssets {
    /// The layout of the window a script puts up, which the pause asks its
    /// question in: for whatever else has such a window and no layout of its own.
    pub fn window(&self) -> &Layout {
        &self.window
    }

    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<PauseAssets> {
        // The layout's eleven textures (the table at 0x167F80): two of the pack
        // that is always there, four places no object is drawn from, one of the
        // field's pack, the figures, the field pack's sheet of its words, the
        // small pictures and the chequer.
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let unused = shared(11);
        let textures = [shared(11), shared(10), unused.clone(), unused.clone(), unused.clone(), unused, own(16), shared(9), own(17), shared(20), shared(7)];
        let layout = interface::layout_with(data, images, PACK, LAYOUT, &textures)?;
        let small = interface::load_texture(data, images, &shared(20))?;
        let window = interface::layout(data, images, RESIDENT_PACK_WINDOW, 0)?;

        let texts = xtc::Table::parse(&data.read(TEXTS)?).context("the system's texts")?;
        let text = |number: usize| -> Result<String> {
            let at = texts.record(number).and_then(|record| record.get(..4)).with_context(|| format!("the system has no text {number}"))?;
            Ok(texts.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
        };
        let mut hints: [Option<String>; 6] = Default::default();
        for (way, number) in HINTS.into_iter().enumerate() {
            hints[way] = number.map(&text).transpose()?;
        }
        Ok(PauseAssets { layout, window, small, yes: text(TEXT_YES)?, no: text(TEXT_NO)?, question: text(TEXT_GIVE_UP)?, hints })
    }
}

/// What the pause is made for (0x12D66C), by the numbers the game has for it.
/// The ride asks for its own (`bike/show.rs`); the first is a scene whose script
/// has said it is not to be skipped (the field's flag 0x48).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A scene that is not to be skipped: start goes on.
    Scene = 0,
    /// A scene: cross skips it, start goes on.
    Skippable = 1,
    /// The field: triangle gives the mission up, square loads a game, start
    /// goes on.
    Field = 2,
    /// The ride, the game within the game: as the field without loading.
    Ride = 3,
    /// A battle: start goes on.
    Battle = 4,
}

/// How it ended (its + 0x14). The original has a fourth, a game loaded (3):
/// here the game that is loaded takes this one's place there and then, and the
/// pause with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The game goes on.
    GoOn = 0,
    /// The scene is skipped.
    Skip = 1,
    /// The mission is given up for the screen between missions.
    GiveUp = 2,
}

/// The states of its task (its + 0x20), by the numbers the game has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Coming = 0,
    There = 1,
    /// The question whether to give the mission up is up.
    Asking = 2,
    /// The list of saves is up.
    Loading = 3,
    Going = 5,
    /// The mission was given up: the screen goes dark.
    Darkening = 6,
    Over = 7,
}

/// A record of its list (0x69C54): its name, the kind its small picture is of,
/// and how much of its goal its count has come to, in hundredths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub icon: u8,
    pub percent: u8,
}

/// The records the pause lists (0x12D36C): those of the records' list 2, whose
/// counts have gone up.
pub fn entries(records: &Records, progress: &Progress) -> Vec<Entry> {
    let listed = records.list(2, progress).into_iter();
    listed
        .filter_map(|number| {
            let record = records.record(number)?;
            let share = progress.variable(COUNT + number) as f32 / record.goal as f32;
            Some(Entry { name: record.name, icon: record.icon, percent: ((share * 100.0) as i32).clamp(0, 100) as u8 })
        })
        .collect()
}

/// How much of its interface is there (its + 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    Gone = 0,
    Coming = 1,
    There = 2,
    Going = 3,
}

/// Its interface (0x6A274): the layout as it is played, and the list.
#[derive(Clone, Debug, PartialEq)]
struct Interface {
    player: Player,
    shown: Shown,
    /// The frames it has been coming for (+ 0x88).
    coming: i32,
    entries: Vec<Entry>,
    /// The first row shown (+ 0x54).
    top: usize,
    /// Which words it has at its foot (+ 0x5C).
    way: usize,
    /// The list is not drawn, nor the pad's (flag 2).
    list_hidden: bool,
    /// As it goes its sheet goes black (flag 8).
    darkens: bool,
}

impl Interface {
    /// Whether the list has more rows than are shown at a time, near enough: it
    /// has its knob then.
    fn many(&self) -> bool {
        self.entries.len() >= MANY
    }

    /// It comes (0x69E8C with 0).
    fn come(&mut self, layout: &Layout, sounds: &mut Vec<Request>) {
        self.player.play(layout, set::COMES);
        if self.many() {
            self.player.play(layout, set::KNOB_COMES);
        }
        sounds.push(Request::common(SOUND_COMES));
        (self.coming, self.shown) = (0, Shown::Coming);
    }

    /// It goes (0x69E8C with 1).
    fn go(&mut self, layout: &Layout) {
        self.player.play(layout, set::GOES);
        if self.many() {
            self.player.play(layout, set::KNOB_GOES);
        }
        if self.darkens {
            self.player.play(layout, set::DARKENS);
        }
        self.shown = Shown::Going;
    }

    /// Whether it is neither coming nor going (0x699A0).
    fn has_come(&self) -> bool {
        !matches!(self.shown, Shown::Coming | Shown::Going)
    }

    /// Whether it has gone (0x69A20).
    fn has_gone(&self) -> bool {
        self.shown == Shown::Gone
    }

    /// The list is moved to a row (0x69FE4), no further than its last four; says
    /// whether that moved it.
    fn scroll(&mut self, to: i32) -> bool {
        let last = (self.entries.len() as i32 - ROWS as i32).max(0);
        let to = to.clamp(0, last) as usize;
        std::mem::replace(&mut self.top, to) != to
    }

    /// A frame of it (0x6A09C, 0x69CD0). While it is there, in a way that has
    /// the list, up and down move the list a row, again and again while they
    /// are held, though round its ends only when they are pressed anew.
    fn frame(&mut self, layout: &Layout, pad: &Pad, sounds: &mut Vec<Request>) {
        if self.shown == Shown::There && self.way != 0 && !self.list_hidden {
            let (top, last) = (self.top as i32, self.entries.len() as i32 - ROWS as i32);
            let mut moved = false;
            // Up comes before down when both are held.
            if pad.held & button::UP != 0 {
                if pad.repeat & button::UP != 0 {
                    if top != 0 {
                        moved = self.scroll(top - 1);
                    } else if pad.pressed & button::UP != 0 {
                        moved = self.scroll(last);
                    }
                }
            } else if pad.held & button::DOWN != 0 && pad.repeat & button::DOWN != 0 {
                if top != last {
                    moved = self.scroll(top + 1);
                } else if pad.pressed & button::DOWN != 0 {
                    moved = self.scroll(0);
                }
            }
            if moved {
                sounds.push(Request::common(SOUND_LIST));
            }
        }
        match self.shown {
            Shown::Coming if self.coming < COMING_FRAMES => self.coming += 1,
            Shown::Coming => self.shown = Shown::There,
            Shown::Going if self.player.finished(layout) => self.shown = Shown::Gone,
            Shown::Going | Shown::There | Shown::Gone => {}
        }
        self.player.advance(layout, 1.0);
    }
}

/// The sheet that darkens the screen over the pause ("sysFadeTask", 0x12F1B8):
/// told to (0x12F204 with 2), it goes black over so many frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Dark {
    on: bool,
    count: i32,
    frames: i32,
    alpha: u8,
}

impl Dark {
    fn begin(&mut self, frames: i32) {
        *self = Dark { on: true, count: 0, frames, alpha: 0 };
    }

    /// A frame of it (0x12F8F8).
    fn frame(&mut self) {
        if self.on && self.count <= self.frames {
            self.alpha = (255.0 * (self.count as f32 / self.frames as f32)) as u8;
            self.count += 1;
        }
    }

    /// Whether it has got there (0x12FB0C).
    fn done(&self) -> bool {
        self.frames < self.count
    }
}

/// The pause, from when it is made.
pub struct Pause {
    kind: Kind,
    interface: Interface,
    state: State,
    outcome: Outcome,
    /// A scene is to be skipped (its + 8).
    skip: bool,
    /// The screen between missions is not to be had yet (0x12D458).
    no_stage_select: bool,
    window: Option<Window>,
    dark: Dark,
    /// Square was pressed, until whoever puts the list of saves up has taken
    /// that.
    load_asked: bool,
    /// The screen is to be black from now on (0x297E4 with 2 and no frames).
    pub blacken: bool,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl Pause {
    /// The pause as its task makes it (0x12D5EC): what plays stands still, its
    /// interface is told which words it has and comes, and has a first frame.
    /// `no_stage_select` is the save's flag 6169.
    pub fn new(assets: &PauseAssets, kind: Kind, entries: Vec<Entry>, no_stage_select: bool) -> Pause {
        let way = match kind {
            Kind::Scene => 0,
            Kind::Skippable => 1,
            Kind::Field if no_stage_select => 5,
            Kind::Field => 2,
            Kind::Ride => 3,
            Kind::Battle => 4,
        };
        let player = assets.layout.player(usize::MAX);
        let mut interface = Interface { player, shown: Shown::Gone, coming: 0, entries, top: 0, way, list_hidden: WAYS_WITHOUT_LIST.contains(&way), darkens: false };
        let mut sounds = vec![Request::Hold];
        interface.come(&assets.layout, &mut sounds);
        interface.frame(&assets.layout, &Pad::default(), &mut sounds);
        Pause {
            kind,
            interface,
            state: State::Coming,
            outcome: Outcome::GoOn,
            skip: false,
            no_stage_select,
            window: None,
            dark: Dark::default(),
            load_asked: false,
            blacken: false,
            sounds,
        }
    }

    /// Whether square has just been pressed: the list of saves is to be put up
    /// (0x4FF8), once.
    pub fn take_load(&mut self) -> bool {
        std::mem::take(&mut self.load_asked)
    }

    /// The list of saves has gone with no game loaded (0x12DCC0): the pause is
    /// as it was.
    pub fn load_closed(&mut self) {
        if self.state == State::Loading {
            self.state = State::There;
        }
    }

    /// It is told to go (0x12D080, 0x12E068): with the sound of how it ended,
    /// and its sheet going black if a scene is skipped.
    fn leave(&mut self, layout: &Layout) {
        self.interface.darkens = self.skip;
        let sound = if self.outcome == Outcome::Skip { SOUND_SKIPPED } else { SOUND_GOES };
        self.sounds.push(Request::common(sound));
        self.interface.go(layout);
        self.state = State::Going;
    }

    /// Whether its interface has no frames: on the field, while it asks or the
    /// list of saves is up (0x12DD64).
    fn still(&self) -> bool {
        self.kind == Kind::Field && matches!(self.state, State::Asking | State::Loading)
    }

    /// A frame of the port's logic: two of its own, of which the second has what
    /// the buttons did. Answers how it ended, once, when it is over.
    pub fn step(&mut self, assets: &PauseAssets, pad: &Pad) -> Option<Outcome> {
        if self.state == State::Over {
            return None;
        }
        let held = Pad { held: pad.held, ..Pad::default() };
        for frame in 0..FRAMES_A_STEP {
            let last = frame + 1 == FRAMES_A_STEP;
            self.frame(assets, if last { pad } else { &held }, last);
            if self.state == State::Over {
                return Some(self.outcome);
            }
        }
        None
    }

    /// A frame of its task (0x12D8A4), and of what darkens the screen over it.
    fn frame(&mut self, assets: &PauseAssets, pad: &Pad, with_buttons: bool) {
        let layout = &assets.layout;
        let pressed = |buttons: u32| pad.pressed & buttons != 0;
        match self.state {
            State::Coming => {
                if self.interface.has_come() {
                    self.state = State::There;
                }
            }
            State::There => match self.kind {
                Kind::Scene | Kind::Battle => {
                    if pressed(button::START) {
                        self.leave(layout);
                    }
                }
                Kind::Skippable => {
                    if pressed(button::CROSS) {
                        (self.skip, self.outcome) = (true, Outcome::Skip);
                        self.leave(layout);
                    } else if pressed(button::START) {
                        self.leave(layout);
                    }
                }
                Kind::Field | Kind::Ride => {
                    let field = self.kind == Kind::Field;
                    if pressed(button::TRIANGLE) {
                        // The cursor starts on "No", which is circle's answer too.
                        if !(field && self.no_stage_select) {
                            self.window = Some(Window::new(text::pages(&assets.question, &text::Plain), &[&assets.yes, &assets.no], NO, NO));
                            self.state = State::Asking;
                        }
                    } else if field && pressed(button::SQUARE) {
                        (self.state, self.load_asked) = (State::Loading, true);
                    } else if pressed(button::START) {
                        self.leave(layout);
                    }
                }
            },
            // The window's own frame is two of the pause's.
            State::Asking if with_buttons => {
                let answer = self.window.as_mut().and_then(|window| {
                    let answer = window.step(pad);
                    self.sounds.extend(window.sounds.drain(..).map(Request::common));
                    answer
                });
                match answer {
                    Some(YES) => {
                        self.window = None;
                        (self.outcome, self.state) = (Outcome::GiveUp, State::Darkening);
                        self.dark.begin(DARK_FRAMES);
                    }
                    Some(_) => {
                        self.window = None;
                        self.state = State::There;
                    }
                    None => {}
                }
            }
            State::Asking | State::Loading => {}
            State::Going => {
                if self.interface.has_gone() {
                    // A scene skipped is not to be seen again (0x12DA44).
                    self.blacken = self.skip;
                    self.state = State::Over;
                }
            }
            State::Darkening => {
                if self.dark.done() {
                    self.blacken = true;
                    self.state = State::Over;
                }
            }
            State::Over => return,
        }
        if !self.still() {
            self.interface.frame(layout, pad, &mut self.sounds);
        }
        self.dark.frame();
    }

    /// Draws it (0x12E030, 0x6A5D0), as far on from the game's last frame as the
    /// frame drawn is: its layout track by track, the list's groups once for
    /// each row; then its question; then what darkens the screen over it.
    pub fn draw(&self, canvas: &mut Canvas, assets: &PauseAssets, fonts: &Fonts, colours: &TextColours, seen: &Seen, ahead: f32) {
        if self.state == State::Over {
            return;
        }
        let (layout, list) = (&assets.layout, &self.interface);
        let mut player = list.player.clone();
        if !self.still() {
            player.advance(layout, ahead * FRAMES_A_STEP as f32);
        }
        let written = |words: &str, style: &Style| fonts.write(style, &text::pages(words, &text::Plain).into_iter().next().unwrap_or_default(), colours);
        let down = |row: usize| row as f32 * ROW_PITCH;
        let lower = |row: usize| Look::inner(Affine2::from_translation(Vec2::new(0.0, down(row))));
        let rows = || (0..ROWS).map(|row| (row, list.entries.get(list.top + row)));
        let records = || rows().filter_map(|(row, entry)| Some((row, entry?, entry?.percent >= 100)));

        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            if list.list_hidden && (group::OF_THE_LIST.contains(&group) || group == group::BAR) {
                continue;
            }
            match group {
                // The sheet covers all that is seen, whatever the window's shape.
                group::SHEET => {
                    if let Some((_, colour)) = layout.placed(&player, track) {
                        canvas.fill(layer::PAUSE, seen.0, colour);
                    }
                }
                group::SMALL_PICTURE | group::NAME | group::MARK => {
                    let number = player.number_in_group(track);
                    for (row, entry, cleared) in records() {
                        if matches!((number, cleared), (Some(0), false) | (Some(1), true)) {
                            continue;
                        }
                        match group {
                            group::SMALL_PICTURE => {
                                let place = layout.first_of(group).and_then(|object| layout.place(object));
                                let (Some((moved, tint)), Some(place)) = (layout.placed(&player, track), place) else { continue };
                                let at = place + Vec2::new(0.0, down(row)) + moved.translation.trunc();
                                draw_small_picture(canvas, layer::PAUSE, &assets.small, entry.icon, at, tint);
                            }
                            group::NAME => write(canvas, fonts, layout, &player, track, &written(&entry.name, &NAME_STYLE), down(row), false),
                            _ => layout.draw_track(canvas, layer::PAUSE, &player, track, &lower(row)),
                        }
                    }
                }
                group::HUNDREDTHS | group::PER_CENT => {
                    for (row, entry, _) in records().filter(|&(_, _, cleared)| !cleared) {
                        if group == group::HUNDREDTHS {
                            let figures = written(&format!("{:02}", entry.percent), &HUNDREDTHS_STYLE);
                            write(canvas, fonts, layout, &player, track, &figures, down(row), true);
                        } else {
                            layout.draw_track(canvas, layer::PAUSE, &player, track, &lower(row));
                        }
                    }
                }
                group::CLEAR | group::BAR_CLEARED => {
                    for (row, ..) in records().filter(|&(_, _, cleared)| cleared) {
                        layout.draw_track(canvas, layer::PAUSE, &player, track, &lower(row));
                    }
                }
                group::BAR_GROUND => {
                    for (row, ..) in records() {
                        layout.draw_track(canvas, layer::PAUSE, &player, track, &lower(row));
                    }
                }
                group if group::BAR_SHINE.contains(&group) => {
                    for (row, ..) in records() {
                        layout.draw_track(canvas, layer::PAUSE, &player, track, &lower(row));
                    }
                }
                // As much of the bar as the count has come to, from its left.
                group::BAR => {
                    for (row, entry, _) in records().filter(|&(_, _, cleared)| !cleared) {
                        let share = (f32::from(entry.percent) / 100.0).clamp(0.0, 1.0);
                        let inner = Affine2::from_scale_angle_translation(Vec2::new(share, 1.0), 0.0, Vec2::new(0.0, down(row)));
                        layout.draw_track(canvas, layer::PAUSE, &player, track, &Look::inner(inner));
                    }
                }
                group if group::NO_RECORD.contains(&group) => {
                    for (row, _) in rows().filter(|(_, entry)| entry.is_none()) {
                        layout.draw_track(canvas, layer::PAUSE, &player, track, &lower(row));
                    }
                }
                group if group::BUTTONS.contains(&group) => {}
                group::HINT => {
                    if let Some(words) = assets.hints.get(list.way).and_then(Option::as_ref) {
                        write(canvas, fonts, layout, &player, track, &written(words, &HINT_STYLE), 0.0, true);
                    }
                }
                // The knob is as much of what it runs in as the rows shown are of
                // the list, as far down it as the first of them is (0x6AB88).
                group::KNOB => {
                    let height = layout.first_of(group).map_or(0.0, |object| f32::from(layout.objects.objects[object].size[1]));
                    let count = list.entries.len() as f32;
                    let (scale, by) = if count > 0.0 { ((ROWS as f32 / count).clamp(0.0, 1.0), list.top as f32 * height / count) } else { (1.0, 0.0) };
                    let inner = Affine2::from_scale_angle_translation(Vec2::new(1.0, scale), 0.0, Vec2::new(0.0, by));
                    layout.draw_track(canvas, layer::PAUSE, &player, track, &Look::inner(inner));
                }
                _ => layout.draw_track(canvas, layer::PAUSE, &player, track, &Look::default()),
            }
        }

        if let Some(window) = &self.window {
            draw_window_on(canvas, layer::PAUSE, &assets.window, fonts, colours, window, ahead);
        }
        if self.dark.alpha != 0 {
            canvas.fill(layer::PAUSE_FADE, seen.0, [0, 0, 0, self.dark.alpha]);
        }
    }
}

/// Writes a text where a track's group has its object (0x6AC98): moved as the
/// track is and so much further down, in the object's colour by the track's, and
/// to the right of the object's width if it is one that is.
#[allow(clippy::too_many_arguments)]
fn write(canvas: &mut Canvas, fonts: &Fonts, layout: &Layout, player: &Player, track: usize, words: &Block, down: f32, to_the_right: bool) {
    let Some((moved, colour)) = layout.placed(player, track) else { return };
    let Some(object) = player.group(track).and_then(|group| layout.first_of(group)) else { return };
    let (Some(place), size) = (layout.place(object), layout.objects.objects[object].size) else { return };
    let tint = modulate(layout.colour(object), colour);
    let placing = Placing { tint, ..Placing::at(place + moved.translation.trunc() + Vec2::new(0.0, down)) };
    let placing = if to_the_right { Placing { width: f32::from(size[0]), align: Align::Right, ..placing } } else { placing };
    fonts.draw(canvas, layer::PAUSE, words, &placing);
}

/// The pause, while it is up.
#[derive(Resource, Default)]
pub struct Paused(pub Option<Pause>);

impl Paused {
    pub fn up(&self) -> bool {
        self.0.is_some()
    }
}

/// Draws the pause over all the game has on the screen.
fn draw(paused: Res<Paused>, assets: Res<PauseAssets>, fonts: Res<Fonts>, colours: Res<TextColours>, seen: Res<Seen>, steps: Res<Time<Fixed>>, mut canvas: ResMut<Canvas>) {
    if let Some(pause) = &paused.0 {
        pause.draw(&mut canvas, &assets, &fonts, &colours, &seen, steps.overstep_fraction());
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Paused>().add_systems(Update, draw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::tables::PlayerStats;
    use crate::records::{Event, World};

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    struct Game {
        assets: PauseAssets,
        fonts: Fonts,
        colours: TextColours,
    }

    fn game() -> Option<Game> {
        let data = GameData::new(DATA).ok()?;
        let mut images = Assets::<Image>::default();
        let assets = PauseAssets::load(&data, &mut images).unwrap();
        Some(Game { assets, fonts: Fonts::load(&data, &mut images).unwrap(), colours: TextColours::load(&data).unwrap() })
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    fn hold(buttons: u32, repeat: u32) -> Pad {
        Pad { held: buttons, repeat, ..Pad::default() }
    }

    fn entry(name: &str, percent: u8) -> Entry {
        Entry { name: name.into(), icon: 0, percent }
    }

    fn common(sounds: &[Request]) -> Vec<u32> {
        let sound = |request: &Request| match request {
            Request::Play { sound, .. } => Some(*sound),
            _ => None,
        };
        sounds.iter().filter_map(sound).collect()
    }

    /// Lets it come, with nothing pressed, and forgets what it has sounded.
    fn come(pause: &mut Pause, assets: &PauseAssets) {
        while pause.state == State::Coming {
            assert_eq!(pause.step(assets, &Pad::default()), None);
        }
        pause.sounds.clear();
    }

    /// Steps it with nothing pressed until it is over; says how it ended, and
    /// after how many frames of the game.
    fn end(pause: &mut Pause, assets: &PauseAssets) -> (Outcome, usize) {
        (1..200).find_map(|frame| pause.step(assets, &Pad::default()).map(|outcome| (outcome, frame))).expect("the pause ends")
    }

    /// The colour its sheet is drawn in.
    fn sheet(pause: &Pause, assets: &PauseAssets) -> Option<[u8; 4]> {
        let player = &pause.interface.player;
        let track = player.track_of(group::SHEET)?;
        assets.layout.placed(player, track).map(|(_, colour)| colour)
    }

    #[test]
    fn its_layout_and_its_words_are_the_games() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let layout = &game.assets.layout;
        assert_eq!((layout.objects.objects.len(), layout.groups.groups.len(), layout.motions.sets.len(), layout.textures.len()), (65, 41, 5, 11));
        // The objects its texts, its small pictures and its knob go by.
        let first = |group: usize| layout.first_of(group);
        assert_eq!((first(group::HINT), first(group::NAME), first(group::HUNDREDTHS)), (Some(57), Some(40), Some(42)));
        assert_eq!((first(group::SMALL_PICTURE), first(group::KNOB)), (Some(39), Some(50)));
        let object = |number: usize| (layout.objects.objects[number].position, layout.objects.objects[number].size);
        assert_eq!(object(40), ([267, 161], [150, 15]));
        assert_eq!(object(42), ([433, 166], [5, 5]));
        assert_eq!(object(57), ([365, 253], [104, 10]));
        assert_eq!(object(39), ([251, 161], [15, 15]));
        assert_eq!(object(50).1, [2, 75]);
        // What comes goes: the sets of all of it move the same groups, but for
        // the one track that is only ever there, and so do the knob's.
        let groups = |set: usize| layout.motions.sets[set].iter().map(|track| track.group as usize).collect::<Vec<_>>();
        let (mut comes, mut goes) = (groups(set::COMES), groups(set::GOES));
        comes.sort_unstable();
        goes.sort_unstable();
        goes.insert(goes.iter().rposition(|&group| group == group::SHEET).unwrap(), group::SHEET);
        assert_eq!(comes, goes);
        assert_eq!(groups(set::KNOB_COMES), groups(set::KNOB_GOES));
        assert_eq!(groups(set::DARKENS), [group::SHEET]);
        assert_eq!((layout.length(set::COMES), layout.length(set::GOES)), (120.0, 32.0));

        assert_eq!(game.assets.question, "Finish mission and return to\nthe Stage Select screen?");
        assert_eq!((game.assets.yes.as_str(), game.assets.no.as_str()), ("Yes", "No"));
        let hints: Vec<Option<&str>> = game.assets.hints.iter().map(Option::as_deref).collect();
        assert_eq!(hints, [None, Some("\\b(01)SKIP"), Some("\\b(02)RETURN \\b(03)LOAD"), Some("\\b(02)RETURN"), None, Some("\\b(03)LOAD")]);
    }

    #[test]
    fn its_list_is_of_the_records_whose_counts_have_gone_up() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let records = Records::load(&data).unwrap();
        let mut progress = Progress::new_game();
        let mut stats = PlayerStats { level: 1, experience: 0, max_hp: 300.0, hp: 300.0, attack: 20, special: 20, defence: 20, burst_bullets: 3, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 };
        records.initialize(&mut World { progress: &mut progress, stats: &mut stats, stage: 0 }).unwrap();
        assert!(entries(&records, &progress).is_empty());
        // A battle won is one of the four of the first record.
        records.battle_finished(Event::battle(100, false, true), &mut World { progress: &mut progress, stats: &mut stats, stage: 0 }).unwrap();
        let listed = entries(&records, &progress);
        let first = listed.first().expect("a record has been counted");
        assert_eq!((first.name.as_str(), first.percent), (records.record(0).unwrap().name.as_str(), 25));
        assert!(listed.iter().all(|entry| entry.percent <= 100));
    }

    #[test]
    fn it_comes_with_what_plays_held_still_and_takes_no_button_until_it_is_there() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets;
        let mut pause = Pause::new(assets, Kind::Field, Vec::new(), false);
        assert_eq!(std::mem::take(&mut pause.sounds), [Request::Hold, Request::common(SOUND_COMES)]);
        assert_eq!((pause.interface.way, pause.interface.list_hidden), (2, false));
        // It comes for twenty of its frames and is seen to be there in the one
        // after: start does nothing in ten frames of the game, and in the
        // eleventh it does.
        for _ in 0..10 {
            assert_eq!(pause.step(assets, &press(button::START | button::TRIANGLE | button::SQUARE)), None);
            assert_eq!(pause.state, State::Coming);
        }
        assert_eq!(sheet(&pause, assets), Some([255, 255, 255, 128]));
        assert_eq!(pause.step(assets, &press(button::START)), None);
        assert_eq!((pause.state, common(&pause.sounds)), (State::Going, vec![SOUND_GOES]));
        // It is gone when its layout has played its going through, thirty-two
        // frames, and the game goes on; the screen is left as it is.
        assert_eq!(end(&mut pause, assets), (Outcome::GoOn, 17));
        assert!(!pause.blacken);
        assert_eq!(pause.step(assets, &press(button::START)), None);
        // Nothing of it is drawn once it is over.
        let mut canvas = Canvas::default();
        pause.draw(&mut canvas, assets, &game.fonts, &game.colours, &Seen::default(), 0.0);
        assert!(canvas.vertices().is_empty());
    }

    #[test]
    fn triangle_asks_before_the_mission_is_given_up_and_the_screen_goes_dark_over_it() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets;
        let mut pause = Pause::new(assets, Kind::Field, Vec::new(), false);
        come(&mut pause, assets);
        assert_eq!(pause.step(assets, &press(button::TRIANGLE)), None);
        assert_eq!(pause.state, State::Asking);
        let window = pause.window.as_ref().unwrap();
        assert_eq!((window.choices.as_slice(), window.selected, window.cancel), (["Yes".to_owned(), "No".to_owned()].as_slice(), 1, 1));
        // While it asks its interface has no frame, and start is the window's.
        let before = pause.interface.clone();
        for _ in 0..40 {
            assert_eq!(pause.step(assets, &press(button::START | button::SQUARE)), None);
        }
        assert!(pause.interface == before && pause.state == State::Asking);
        // "No" is where the cursor is: the pause is as it was.
        pause.step(assets, &press(button::CROSS));
        while pause.state == State::Asking {
            assert_eq!(pause.step(assets, &Pad::default()), None);
        }
        assert_eq!((pause.state, pause.outcome), (State::There, Outcome::GoOn));
        assert!(pause.window.is_none() && !pause.dark.on);

        // Asked again and answered "Yes", the screen goes dark over sixty frames,
        // and it is over in the one after: the mission is given up.
        pause.step(assets, &press(button::TRIANGLE));
        for _ in 0..40 {
            pause.step(assets, &Pad::default());
        }
        pause.step(assets, &press(button::UP));
        pause.step(assets, &press(button::CROSS));
        while pause.state == State::Asking {
            assert_eq!(pause.step(assets, &Pad::default()), None);
        }
        assert_eq!((pause.state, pause.outcome), (State::Darkening, Outcome::GiveUp));
        for _ in 0..15 {
            assert_eq!(pause.step(assets, &press(button::START)), None);
        }
        assert_eq!(pause.dark.alpha, 255 / 2);
        assert_eq!(end(&mut pause, assets), (Outcome::GiveUp, 16));
        assert!(pause.blacken && pause.dark.alpha == 255);
    }

    #[test]
    fn a_scene_is_skipped_with_cross_and_gone_on_with_by_start() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets;
        let mut pause = Pause::new(assets, Kind::Skippable, vec![entry("one", 10)], false);
        assert_eq!((pause.interface.way, pause.interface.list_hidden), (1, true));
        come(&mut pause, assets);
        // Triangle and square are the field's.
        assert_eq!(pause.step(assets, &press(button::TRIANGLE | button::SQUARE)), None);
        assert_eq!(pause.state, State::There);
        assert!(pause.window.is_none() && !pause.take_load());
        pause.step(assets, &press(button::CROSS));
        assert_eq!((pause.state, pause.outcome, pause.skip), (State::Going, Outcome::Skip, true));
        assert_eq!(common(&pause.sounds), [SOUND_SKIPPED]);
        // As it goes its sheet goes black, so that what is passed over is not
        // seen; and the screen is black when it has gone.
        assert_eq!(sheet(&pause, assets), Some([0, 0, 0, 12]));
        for _ in 0..10 {
            pause.step(assets, &Pad::default());
        }
        assert_eq!(sheet(&pause, assets), Some([0, 0, 0, 255]));
        assert_eq!(end(&mut pause, assets), (Outcome::Skip, 7));
        assert!(pause.blacken);

        // Start goes on with the scene.
        let mut pause = Pause::new(assets, Kind::Skippable, Vec::new(), false);
        come(&mut pause, assets);
        pause.step(assets, &press(button::START));
        assert_eq!((pause.state, common(&pause.sounds)), (State::Going, vec![SOUND_GOES]));
        for _ in 0..10 {
            pause.step(assets, &Pad::default());
        }
        assert_eq!(sheet(&pause, assets), None);
        assert_eq!(end(&mut pause, assets).0, Outcome::GoOn);
        assert!(!pause.blacken);
        // In a battle there is start and nothing else.
        let mut pause = Pause::new(assets, Kind::Battle, vec![entry("one", 10)], false);
        assert_eq!((pause.interface.way, pause.interface.list_hidden), (4, true));
        come(&mut pause, assets);
        pause.step(assets, &press(button::TRIANGLE | button::SQUARE | button::CROSS));
        assert_eq!(pause.state, State::There);
        pause.step(assets, &press(button::START));
        assert_eq!(end(&mut pause, assets).0, Outcome::GoOn);
    }

    #[test]
    fn the_list_moves_a_row_at_a_time_and_round_its_ends_on_a_new_press() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets;
        let records: Vec<Entry> = (0..6).map(|number| entry(&format!("record {number}"), number * 20)).collect();
        let mut pause = Pause::new(assets, Kind::Field, records, false);
        assert!(pause.interface.many());
        // It does not move while the pause comes.
        pause.step(assets, &press(button::DOWN));
        assert_eq!(pause.interface.top, 0);
        come(&mut pause, assets);
        pause.step(assets, &press(button::DOWN));
        assert_eq!(pause.interface.top, 1);
        // Held, it moves again when the pad says so, and stops at the last four.
        pause.step(assets, &hold(button::DOWN, 0));
        assert_eq!(pause.interface.top, 1);
        pause.step(assets, &hold(button::DOWN, button::DOWN));
        assert_eq!(pause.interface.top, 2);
        pause.step(assets, &hold(button::DOWN, button::DOWN));
        assert_eq!(pause.interface.top, 2);
        assert_eq!(common(&std::mem::take(&mut pause.sounds)), [SOUND_LIST; 2]);
        // Pressed anew it goes round to the first, and up from there to the last.
        pause.step(assets, &press(button::DOWN));
        assert_eq!(pause.interface.top, 0);
        pause.step(assets, &hold(button::UP, button::UP));
        assert_eq!(pause.interface.top, 0);
        pause.step(assets, &press(button::UP));
        assert_eq!(pause.interface.top, 2);
        // Up comes before down.
        pause.step(assets, &press(button::UP | button::DOWN));
        assert_eq!(pause.interface.top, 1);
        assert_eq!(common(&pause.sounds), [SOUND_LIST; 3]);

        // A list of no more rows than are shown does not move, nor sound.
        let mut pause = Pause::new(assets, Kind::Field, vec![entry("one", 10), entry("two", 20)], false);
        assert!(!pause.interface.many());
        come(&mut pause, assets);
        pause.step(assets, &press(button::DOWN));
        pause.step(assets, &press(button::UP));
        assert_eq!((pause.interface.top, pause.sounds.len()), (0, 0));
    }

    #[test]
    fn square_asks_for_the_list_of_saves_and_triangle_is_nothing_with_no_screen_to_go_to() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets;
        let mut pause = Pause::new(assets, Kind::Field, Vec::new(), true);
        assert_eq!((pause.interface.way, pause.interface.list_hidden), (5, false));
        come(&mut pause, assets);
        pause.step(assets, &press(button::TRIANGLE));
        assert!(pause.state == State::There && pause.window.is_none());
        pause.step(assets, &press(button::SQUARE));
        assert_eq!(pause.state, State::Loading);
        assert!(pause.take_load() && !pause.take_load());
        // While that list is up the pause takes nothing and nothing of it moves.
        let before = pause.interface.clone();
        for _ in 0..5 {
            assert_eq!(pause.step(assets, &press(button::START | button::SQUARE)), None);
        }
        assert!(pause.state == State::Loading && pause.interface == before && !pause.take_load());
        pause.load_closed();
        assert_eq!(pause.state, State::There);
        pause.step(assets, &press(button::START));
        assert_eq!(end(&mut pause, assets).0, Outcome::GoOn);
    }

    #[test]
    fn a_row_is_drawn_by_how_far_its_record_has_come() {
        let Some(game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets;
        let drawn = |pause: &Pause| {
            let mut canvas = Canvas::default();
            pause.draw(&mut canvas, assets, &game.fonts, &game.colours, &Seen::default(), 0.0);
            (canvas.vertices().to_vec(), canvas.drawn_from(&assets.small.0))
        };
        let mut pause = Pause::new(assets, Kind::Field, vec![entry("one", 25), entry("two", 100)], false);
        come(&mut pause, assets);
        let (all, pictures) = drawn(&pause);
        // The sheet is over all of the screen.
        let over = |at: Vec2| all.iter().any(|vertex| vertex.position == at && vertex.colour == [255, 255, 255, 128]);
        assert!(over(Vec2::ZERO) && over(Vec2::new(480.0, 272.0)));
        // A small picture for each of the two records, twenty apart, and none
        // for the rows without one.
        let picture = |at: Vec2| pictures.iter().any(|vertex| vertex.position == at);
        assert!(picture(Vec2::new(251.0, 161.0)) && picture(Vec2::new(266.0, 176.0)) && picture(Vec2::new(251.0, 181.0)));
        assert!(!picture(Vec2::new(251.0, 201.0)) && !picture(Vec2::new(251.0, 221.0)));
        // The first has a quarter of its bar; the second, cleared, all of the
        // other bar.
        let coloured = |at: Vec2, colour: [u8; 4]| all.iter().any(|vertex| vertex.position == at && vertex.colour == colour);
        let (counting, cleared) = ([0xe2, 0x6b, 0x1b, 0xff], [0x00, 0x3f, 0xa6, 0xff]);
        assert!(coloured(Vec2::new(250.0, 177.0), counting) && all.iter().any(|vertex| vertex.position == Vec2::new(298.5, 177.0)));
        assert!(!coloured(Vec2::new(250.0, 197.0), counting));
        assert!(coloured(Vec2::new(250.0, 197.0), cleared) && !coloured(Vec2::new(250.0, 177.0), cleared));
        // With no records it has the four rows without one.
        let mut bare = Pause::new(assets, Kind::Field, Vec::new(), false);
        come(&mut bare, assets);
        assert!(drawn(&bare).1.is_empty());

        // In a battle the list is left out, and the words at its foot with it.
        let mut pause = Pause::new(assets, Kind::Battle, vec![entry("one", 25), entry("two", 100)], false);
        come(&mut pause, assets);
        let (in_battle, pictures) = drawn(&pause);
        assert!(pictures.is_empty() && !in_battle.is_empty());
        assert!(!in_battle.iter().any(|vertex| vertex.colour == counting || vertex.colour == cleared));
        assert!(in_battle.len() < all.len());
    }
}
