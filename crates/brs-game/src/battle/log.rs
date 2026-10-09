//! The battle's log (`BtLogMng`, 0x96368): the line of information at the top of the
//! screen that says what has just happened, one thing at a time.
//!
//! What has a line for it names the line by its number in the table of them
//! (`btl_msg_tbl.bin`) and gives three names, an actor's, a user's and a target's,
//! which the line's words have places for (0x96A14). The lines wait their turn: the
//! band comes, the first line's words come onto it and stay as long as the table
//! says, or less when another waits; then the words go and the next come, and after
//! the last the band goes (0x96A9C). The band is an entry of its own (kind 18) of a
//! record of the battle's layout, playing its first four sets (0x41BB0).
//!
//! In the port the enemies' scripts write to it (engine function 80, 0xBE418), their
//! parts that break and mend (0x85CBC), and the status effects as they are given
//! and as their time runs out (0x8DE7C, 0x8DEF8; `status.rs` keeps those lines for
//! it), her skills as their scenes have them say their lines, and a snipe that
//! finds nothing. Nothing else writes to it.

use std::collections::{HashMap, VecDeque};

use anyhow::{ensure, Context, Result};
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_formats::bxcb;

use crate::data::GameData;
use crate::interface::layer;
use crate::text::{self, TextColours};
use crate::writing::{Block, Fonts, Placing, Style};

const MESSAGES: &str = "SYSTEM.VOL/btl_msg_tbl.bin";

// The sets of the battle's layout the line plays (0x3EC18, 0x3EC8C, 0x3ED14,
// 0x3EDA0): the band comes, the band goes, words come onto it, and words go.
const SET_COMING: usize = 0;
const SET_GOING: usize = 1;
const SET_WORDS_COMING: usize = 2;
const SET_WORDS_GOING: usize = 3;
/// The group whose object the words are written at, in its colour (0x41BE8).
const WORDS_GROUP: usize = 2;
/// How the words are written (0x3EBB8): eighty glyphs on one line, with nothing
/// between one glyph and the next.
const WORDS_STYLE: Style = Style::of_kind(0, 0x50, 1, 0.0, 4.0);
/// The most a line's words can be, in bytes (0x96A4C, 0x96348).
const MOST_WORDS: usize = 0x7f;
/// The sound of words coming onto the line (0x96B90), of the bank that is always
/// there.
pub const SOUND_WORDS: u32 = 0x11;

// The lines the engine itself writes about a part of an enemy (0x86170, 0x85EEC).
pub const PART_BROKEN: i32 = 6;
pub const PART_MENDED: i32 = 7;

/// The names a line's words have places for (0x96978, 0x969AC, 0x969E0).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    pub actor: String,
    pub user: String,
    pub target: String,
}

impl Names {
    /// One name for all three, as an enemy's script gives its own (0xBE434).
    pub fn of(name: &str) -> Names {
        Names { actor: name.to_owned(), user: name.to_owned(), target: name.to_owned() }
    }
}

/// A line somebody has for the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub message: i32,
    pub names: Names,
}

/// The table of the log's lines: by its number, a line's words with their places
/// for names, and how long it stays up, in sixtieths of a second.
#[derive(Clone, Debug, Default)]
pub struct Messages(HashMap<i32, (String, f32)>);

impl Messages {
    pub fn load(data: &GameData) -> Result<Messages> {
        let table = bxcb::Table::parse(&data.read(MESSAGES)?).with_context(|| format!("parsing {MESSAGES}"))?;
        ensure!(table.fields == 3, "{MESSAGES} has {} fields per record", table.fields);
        let lines = table.records().map(|record| Ok((record[0] as i32, (table.string(record[1])?.to_owned(), record[2] as i32 as f32))));
        Ok(Messages(lines.collect::<Result<_>>()?))
    }

    /// A line's words with the names in them, and how long it stays (0x96A14);
    /// nothing for a line the table does not have.
    fn words(&self, line: &Line) -> Option<(String, f32)> {
        let (words, time) = self.0.get(&line.message)?;
        Some((fill(words, &line.names), *time))
    }
}

/// Puts the names in a line's words (0x96698): `<actor>`, `<user>` and `<target>`
/// give way to them, and anything else in such brackets to nothing. What does not
/// fit is left off.
fn fill(words: &str, names: &Names) -> String {
    let mut filled = String::new();
    let mut rest = words;
    while let Some(open) = rest.find('<') {
        filled += &rest[..open];
        let inside = &rest[open + 1..];
        let (tag, after) = inside.split_once('>').unwrap_or((inside, ""));
        filled += match tag {
            "actor" => &names.actor,
            "user" => &names.user,
            "target" => &names.target,
            _ => "",
        };
        rest = after;
    }
    filled += rest;
    let mut end = filled.len().min(MOST_WORDS);
    while !filled.is_char_boundary(end) {
        end -= 1;
    }
    filled.truncate(end);
    filled
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// There is no band.
    Idle,
    /// The band or the words are coming.
    Coming,
    /// The words are there, for their time.
    Up,
    /// The words are going, for the next to come.
    WordsGoing,
    /// The band is going.
    Going,
}

/// The log of the battle that is on.
#[derive(Resource)]
pub struct Log {
    /// The lines that wait, the one that is up first: its words and its time.
    lines: VecDeque<(String, f32)>,
    phase: Phase,
    /// How long the words that are up have been, in sixtieths.
    up_for: f32,
    band: Player,
    /// The band's motion has not been started (the entry's bit 30, 0x3CFD4).
    waiting: bool,
    words: Option<Block>,
}

impl Log {
    pub fn new(layout: &Layout) -> Log {
        Log { lines: VecDeque::new(), phase: Phase::Idle, up_for: 0.0, band: layout.player(SET_COMING), waiting: true, words: None }
    }

    /// Gives it a line (0x96A14), to come up in its turn.
    pub fn write(&mut self, messages: &Messages, line: &Line) {
        match messages.words(line) {
            Some(words) => self.lines.push_back(words),
            None => debug!("the log has no line {}", line.message),
        }
    }

    /// Whether it still has something to say (its flag 1, 0x96A84): what follows a
    /// battle waits for it.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle || !self.lines.is_empty()
    }

    /// The words of the first of its lines come onto the band (0x96B60); `write`
    /// lays them out.
    fn bring_words(&mut self, layout: &Layout, write: &mut dyn FnMut(&str) -> Block) {
        self.band.play(layout, SET_WORDS_COMING);
        self.words = self.lines.front().map(|(words, _)| write(words));
        self.up_for = 0.0;
        self.phase = Phase::Coming;
    }

    /// One frame of it, of `dt` sixtieths (0x96A9C). With another line waiting the
    /// one that is up stays only for `short`. Says how many times words came onto
    /// the band, each of which is heard.
    pub fn step(&mut self, layout: &Layout, dt: f32, short: f32, write: &mut dyn FnMut(&str) -> Block) -> u32 {
        if !self.waiting {
            self.band.advance(layout, dt);
        }
        let mut heard = 0;
        loop {
            match self.phase {
                Phase::Idle => {
                    if self.lines.is_empty() {
                        return heard;
                    }
                    self.waiting = false;
                    self.band.play(layout, SET_COMING);
                    self.bring_words(layout, write);
                    heard += 1;
                }
                // Once the motion is over the words are up, from the next frame.
                Phase::Coming => {
                    if self.band.finished(layout) {
                        self.phase = Phase::Up;
                    }
                    return heard;
                }
                Phase::Up => {
                    self.up_for += dt;
                    let stays = if self.lines.len() > 1 { short } else { self.lines.front().map_or(0.0, |line| line.1) };
                    if self.up_for < stays {
                        return heard;
                    }
                    self.lines.pop_front();
                    if self.lines.is_empty() {
                        self.band.play(layout, SET_GOING);
                        self.words = None;
                        self.phase = Phase::Going;
                    } else {
                        self.band.play(layout, SET_WORDS_GOING);
                        self.phase = Phase::WordsGoing;
                    }
                }
                Phase::WordsGoing => {
                    if !self.band.finished(layout) {
                        return heard;
                    }
                    self.bring_words(layout, write);
                    heard += 1;
                }
                // With the band gone, a line that has come meanwhile brings it back
                // at once.
                Phase::Going => {
                    if !self.band.finished(layout) {
                        return heard;
                    }
                    self.phase = Phase::Idle;
                }
            }
        }
    }

    /// Draws it (0x41BB0): the band's groups as they are, and the words where the
    /// object of theirs is, in its colour and with their shadow.
    pub fn draw(&self, canvas: &mut Canvas, layout: &Layout, fonts: &Fonts) {
        for track in 0..self.band.len() {
            if self.band.group(track) != Some(WORDS_GROUP) {
                layout.draw_track(canvas, layer::DISPLAY, &self.band, track, &Look::default());
                continue;
            }
            let (Some(words), Some((moved, colour))) = (&self.words, layout.placed(&self.band, track)) else { continue };
            let Some((object, place)) = layout.first_of(WORDS_GROUP).and_then(|object| Some((object, layout.place(object)?))) else { continue };
            let tint = modulate(colour, layout.colour(object));
            fonts.draw(canvas, layer::DISPLAY, words, &Placing { tint, shadow: true, ..Placing::at(place + moved.translation.trunc()) });
        }
    }
}

/// Lays a line's words out as the log writes them.
pub fn written(fonts: &Fonts, colours: &TextColours, words: &str) -> Block {
    fonts.write(&WORDS_STYLE, &text::pages(words, &text::Plain).swap_remove(0), colours)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::hud::Hud;

    fn game() -> Option<(Layout, Messages, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let hud = Hud::load(&data, &mut images).unwrap();
        let (layout, messages) = hud.of_the_log();
        Some((layout.clone(), messages.clone(), Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    #[test]
    fn a_lines_words_have_the_names_put_in() {
        let names = Names { actor: "Eater".into(), user: "BRS".into(), target: "Mii".into() };
        assert_eq!(fill("BRS has defeated \\b(60)<actor>!", &names), "BRS has defeated \\b(60)Eater!");
        assert_eq!(fill("<user> to <target><nobody>.", &names), "BRS to Mii.");
        assert_eq!(fill("no names", &names), "no names");
        // What does not fit is left off, at a whole character.
        let long = Names::of(&"\u{ff22}".repeat(60));
        assert_eq!(fill("<actor>", &long).len(), 126);
    }

    #[test]
    fn the_table_has_the_lines_the_engine_writes() {
        let Some((_, messages, ..)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(messages.0.len(), 184);
        let line = |message: i32| messages.words(&Line { message, names: Names::of("Eater") });
        assert_eq!(line(1), Some(("BRS has defeated \\b(60)Eater!".to_owned(), 60.0)));
        assert_eq!(line(PART_BROKEN), Some(("Eater\\b(60)is stunned!".to_owned(), 60.0)));
        assert_eq!(line(PART_MENDED), Some(("Eater\\b(60)is no longer stunned!".to_owned(), 60.0)));
        assert_eq!(line(9), None);
    }

    #[test]
    fn lines_come_up_one_after_the_other_and_the_band_goes_after_the_last() {
        let Some((layout, messages, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut write = |words: &str| written(&fonts, &colours, words);
        let drawn = |log: &Log| {
            let mut canvas = Canvas::default();
            log.draw(&mut canvas, &layout, &fonts);
            canvas.vertices().len()
        };
        let mut log = Log::new(&layout);
        // With nothing to say there is nothing of it, whatever time goes by.
        assert_eq!(log.step(&layout, 2.0, 30.0, &mut write), 0);
        assert!(!log.busy() && drawn(&log) == 0);

        // A line: the band comes with its words, which is heard; they are up once
        // the motion is over, sixteen frames on, and stay for the line's sixty.
        log.write(&messages, &Line { message: 1, names: Names::of("Eater") });
        assert!(log.busy());
        assert_eq!(log.step(&layout, 2.0, 30.0, &mut write), 1);
        let mut frames = 0;
        while log.phase == Phase::Coming {
            assert_eq!(log.step(&layout, 2.0, 30.0, &mut write), 0);
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(frames, 8);
        // The band, the mark before the words, the word under them, and the words
        // with their shadow: an icon and twenty glyphs, twice.
        let glyphs = "BRShasdefeated>Eater!".len();
        assert_eq!(drawn(&log), (3 + glyphs * 2) * 6);
        let mut up = 0;
        while log.phase == Phase::Up {
            log.step(&layout, 2.0, 30.0, &mut write);
            up += 1;
            assert!(up < 100);
        }
        assert_eq!(up, 30);
        // Then the band goes, over eight frames, and the log has no more to say.
        assert_eq!(log.phase, Phase::Going);
        let mut going = 0;
        while log.busy() {
            log.step(&layout, 2.0, 30.0, &mut write);
            going += 1;
            assert!(going < 100);
        }
        assert_eq!(going, 4);
        assert_eq!(drawn(&log), 0);

        // With a second line waiting the first stays only the short while, then
        // its words go and the second's come, which is heard again.
        log.write(&messages, &Line { message: 1, names: Names::of("Eater") });
        log.write(&messages, &Line { message: PART_BROKEN, names: Names::of("Core") });
        let mut heard = log.step(&layout, 2.0, 30.0, &mut write);
        while log.phase == Phase::Coming {
            heard += log.step(&layout, 2.0, 30.0, &mut write);
        }
        let mut up = 0;
        while log.phase == Phase::Up {
            heard += log.step(&layout, 2.0, 30.0, &mut write);
            up += 1;
        }
        assert_eq!((heard, up, log.phase), (1, 15, Phase::WordsGoing));
        while log.phase == Phase::WordsGoing {
            heard += log.step(&layout, 2.0, 30.0, &mut write);
        }
        assert_eq!((heard, log.lines.len()), (2, 1));
    }
}
