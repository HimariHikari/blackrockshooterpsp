//! The sign that names a place (0x46DB8 to 0x4804C): white letters of a sheet of
//! its own, in up to three lines, at a corner of the screen. See
//! `docs/FIELD_NOTES.md`.
//!
//! Two things put one up. A story scene has a title of three lines typed out at
//! the top left, letter by letter behind a star that blinks, with a sound for
//! each ("25 SEPTEMBER:2051", "37.77-122.41", "SAN FRANCISCO"): a map's script
//! makes it (`s36`), starts it (`s39`) and waits for it (`s25`), and it is a task
//! of the script's ("Location", 0xCFEC8), which is [`Title`] here. And the field
//! shows the name of the place she has come into, all at once at the bottom
//! left, for a while (window 16; the field's object at + 0x1D88), which is
//! [`PlaceName`]. Either goes as it came apart: its letters fade in six lots,
//! which are made up at random as it is laid out.
//!
//! Its data is a pack of the interface that is always there
//! (`if_rsdsys.lpk/022.lpk`): the sheet, the lines of each place, and how far
//! each letter moves the pen.

use anyhow::{Context, Result};
use brs_formats::xtc;

use crate::battle::damage::Random;
use crate::data::GameData;
use crate::sound::{Request, COMMON_BANK};

/// The sheet of its letters, and its two tables.
pub const SHEET: &str = "SYSTEM.VOL/if_rsdsys.lpk/022.lpk/000.ptm";
const PLACES: &str = "SYSTEM.VOL/if_rsdsys.lpk/022.lpk/001.xtc";
const GLYPHS: &str = "SYSTEM.VOL/if_rsdsys.lpk/022.lpk/002.xtc";

/// A letter's cell of the sheet, and the size it is drawn at.
pub const GLYPH: i32 = 11;
/// The lines a sign has, and how far apart they are (0x47D88).
pub const LINES: usize = 3;
const LINE_PITCH: i32 = 13;
/// Where its first letter is put, by where it is told to be (0x47A78): the top
/// left, or the bottom left. Told anything else it is at the corner itself.
const ORIGINS: [[i32; 2]; 2] = [[6, 6], [6, 0xd9]];
/// Where the star that is its cursor is in the sheet (0x47DC0), which is also
/// what is drawn for a character the sheet has no letter of.
pub const STAR: [i32; 2] = [8 * GLYPH, 2 * GLYPH];
/// The character a sign's text is taken to have where it has one the table of
/// letters does not (0x47B7C).
const NO_LETTER: char = '\u{25a0}';

/// The frames of the interface that go by in a frame of the game (0x850).
const FRAMES: i32 = 2;
/// The lots its letters go in, and the lot of those the draw left over (0x47DA0).
const LOTS: i32 = 6;
const LEFT_OVER: i32 = 5;
/// A letter that is next to one just drawn for a lot, while that lot is drawn.
const KEPT_BACK: i32 = -1;
// The frames it waits: after a space, after a line, and before it begins to go
// (0x47500, 0x47518, 0x471BC).
const WAIT_AFTER_SPACE: i32 = 2;
const WAIT_AFTER_LINE: i32 = 0x14;
const WAIT_BEFORE_GOING: i32 = 6;
/// The sound of a letter typed, of the bank that is always there, and how loud
/// (0x47590).
const SOUND_TYPED: u32 = 9;
const TYPED_VOLUME: i32 = 0x2a;

/// How far a letter moves the pen, and where it is put about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Glyph {
    code: char,
    dx: i32,
    dy: i32,
    advance: i32,
}

/// The sign's tables: the lines of each place, and the letters.
#[derive(Clone, Debug)]
pub struct Places {
    places: Vec<[String; LINES]>,
    glyphs: Vec<Glyph>,
}

impl Places {
    pub fn load(data: &GameData) -> Result<Places> {
        let places = xtc::Table::parse(&data.read(PLACES)?).with_context(|| format!("parsing {PLACES}"))?;
        let glyphs = xtc::Table::parse(&data.read(GLYPHS)?).with_context(|| format!("parsing {GLYPHS}"))?;
        let word = |record: &[u8], at: usize| record.get(at..at + 4).map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()));
        let mut lines = Vec::with_capacity(places.len());
        for index in 0..places.len() {
            let record = places.record(index).context("a place's record is missing")?;
            let mut place: [String; LINES] = Default::default();
            for (line, words) in place.iter_mut().enumerate() {
                let at = word(record, line * 4).context("a place's record is too short")?;
                *words = places.string(at)?.to_owned();
            }
            lines.push(place);
        }
        let mut letters = Vec::with_capacity(glyphs.len());
        for index in 0..glyphs.len() {
            let record = glyphs.record(index).filter(|record| record.len() >= 7).context("a letter's record is too short")?;
            let code = glyphs.string(word(record, 0).unwrap_or(0))?.chars().next().context("a letter's record names no letter")?;
            let byte = |at: usize| i32::from(record[at] as i8);
            letters.push(Glyph { code, dx: byte(4), dy: byte(5), advance: byte(6) });
        }
        Ok(Places { places: lines, glyphs: letters })
    }

    /// The lines of a place (0x2C54C), if the table has it.
    pub fn lines(&self, place: i32) -> Option<&[String; LINES]> {
        usize::try_from(place).ok().and_then(|place| self.places.get(place))
    }

    /// 0x472B8.
    fn glyph(&self, code: char) -> Option<Glyph> {
        self.glyphs.iter().copied().find(|glyph| glyph.code == code)
    }
}

/// Where in the sheet a character's letter is (0x47B90 to 0x47CCC): the capitals
/// eleven to a row, then W to Z with a blank, a colon, a dash, a full stop and
/// the star, then the figures.
fn source(code: char) -> [i32; 2] {
    let code = code as i32;
    let cell = |column: i32, row: i32| [column * GLYPH, row * GLYPH];
    match code {
        0x41..=0x5a => cell((code - 0x41) % 11, (code - 0x41) / 11),
        0x30..=0x39 => cell(code - 0x30, 3),
        0x20 | 0x3000 => cell(4, 2),
        0x3a | 0xff1a => cell(5, 2),
        0x2d => cell(6, 2),
        0x2e => cell(7, 2),
        _ => cell(8, 2),
    }
}

/// A letter of the sign as it is drawn: where, from where in the sheet, and how
/// much of it is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub at: [i32; 2],
    pub source: [i32; 2],
    pub alpha: u8,
    /// It is a space: it is typed without a sound and waited after.
    space: bool,
    /// The lot it goes with.
    lot: i32,
}

/// Where on the screen a sign is (its + 0x30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft = 0,
    BottomLeft = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Idle = 0,
    Typing = 1,
    Going = 2,
}

/// The sign itself (0x46DB8): the letters of a place's lines, and how far it is
/// in coming or going.
#[derive(Clone, Debug, PartialEq)]
pub struct Sign {
    place: i32,
    corner: i32,
    /// Its lines' letters.
    pub rows: [Vec<Cell>; LINES],
    /// The star that goes before the letters as they are typed.
    pub cursor: Cell,
    mode: Mode,
    /// It is coming or going, and has not done (its flag 8).
    busy: bool,
    /// It has gone (its flag 2), as one that has never come has.
    gone: bool,
    /// All its letters have been typed (its flag 0x20000000).
    all_typed: bool,
    /// It has been told to come at least once (its flag 0x40000000): until then
    /// it has no frames.
    started: bool,
    /// The frames it has been at what it is doing (+ 0x18), those it has yet to
    /// wait (+ 0xF68), and its two clocks (+ 0xF6C, + 0xF70).
    time: i32,
    wait: i32,
    beat: i32,
    blink: i32,
    /// The lots that have gone (+ 0xF74).
    lots_gone: i32,
    /// The letter that is typed next (+ 0x28, + 0x24).
    row: usize,
    column: usize,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl Sign {
    /// A sign for a place, not shown yet.
    pub fn new(place: i32) -> Sign {
        let cursor = Cell { at: [0, 0], source: STAR, alpha: 0, space: false, lot: 0 };
        Sign {
            place,
            corner: Corner::TopLeft as i32,
            rows: Default::default(),
            cursor,
            mode: Mode::Idle,
            busy: false,
            gone: true,
            all_typed: false,
            started: false,
            time: 0,
            wait: -1,
            beat: 0,
            blink: 0,
            lots_gone: 0,
            row: 0,
            column: 0,
            sounds: Vec::new(),
        }
    }

    /// It is told where on the screen it is to be (0x472A8), before it comes.
    pub fn put(&mut self, corner: i32) {
        self.corner = corner;
    }

    /// Whether it has done coming, or going (0x47280).
    pub fn done(&self) -> bool {
        !self.busy
    }

    /// Whether it has gone (0x47258).
    pub fn gone(&self) -> bool {
        self.gone
    }

    fn cells_mut(&mut self) -> impl Iterator<Item = &mut Cell> {
        self.rows.iter_mut().flatten()
    }

    /// It comes (0x47058): its letters are laid out, and typed one by one, or
    /// all there at once.
    pub fn show(&mut self, places: &Places, at_once: bool, random: &mut Random) {
        (self.mode, self.gone, self.started) = (Mode::Typing, false, true);
        (self.time, self.row, self.column, self.beat, self.blink) = (0, 0, 0, 0, 0);
        self.cursor.alpha = 0;
        self.lay_out(places, random);
        if at_once {
            self.mode = Mode::Idle;
            self.cells_mut().for_each(|cell| cell.alpha = 0xff);
            self.busy = false;
        } else {
            self.busy = true;
        }
    }

    /// It goes (0x4717C): its letters fade a lot at a time, or are gone at once.
    pub fn go(&mut self, at_once: bool) {
        self.time %= 0x24;
        (self.mode, self.gone) = (Mode::Going, false);
        (self.row, self.column, self.beat, self.blink) = (0, 0, 0, 0);
        self.wait = WAIT_BEFORE_GOING;
        if at_once {
            (self.gone, self.mode, self.busy) = (true, Mode::Idle, false);
            self.cells_mut().for_each(|cell| cell.alpha = 0);
            self.cursor.alpha = 0;
        } else {
            self.busy = true;
        }
    }

    /// Lays its letters out (0x47A30): each line from the left, a pixel between
    /// letters, the lines thirteen apart; then draws the lots they go in.
    fn lay_out(&mut self, places: &Places, random: &mut Random) {
        let origin = usize::try_from(self.corner).ok().and_then(|corner| ORIGINS.get(corner)).copied().unwrap_or([0, 0]);
        let lines = places.lines(self.place).cloned().unwrap_or_default();
        for (row, line) in lines.iter().enumerate() {
            let (y, mut pen) = (origin[1] + row as i32 * LINE_PITCH, 0);
            self.rows[row].clear();
            for character in line.chars() {
                // A character the table does not have is the star, a letter wide.
                let glyph = places.glyph(character).unwrap_or(Glyph { code: NO_LETTER, dx: 0, dy: 0, advance: GLYPH });
                let space = matches!(glyph.code, ' ' | '\u{3000}');
                self.rows[row].push(Cell { at: [origin[0] + pen + glyph.dx, y + glyph.dy], source: source(glyph.code), alpha: 0, space, lot: LEFT_OVER });
                pen += glyph.advance + 1;
            }
        }
        self.cursor.at = origin;
        self.draw_lots(random);
    }

    /// Draws the lots (0x47D9C): five of them get a sixth of the letters each,
    /// taken at random but never two that stand side by side; what is left is
    /// the sixth lot.
    fn draw_lots(&mut self, random: &mut Random) {
        let counts: [i32; LINES] = std::array::from_fn(|row| self.rows[row].len() as i32);
        let total: i32 = counts.iter().sum();
        let (mut left_in, mut left) = (counts, total);
        for lot in 0..LOTS - 1 {
            // Those of each line, and of all, that this lot may still take.
            let (mut may_in, mut may) = (left_in, left);
            for _ in 0..total / LOTS {
                // One of them at random: which line it is of, and which of that
                // line's. With none left it is the first line's first, if any.
                let mut pick = random.below(may);
                let (mut row, mut nth) = (0, 0);
                for (line, &free) in may_in.iter().enumerate() {
                    if pick < free {
                        (row, nth) = (line, pick);
                        break;
                    }
                    pick -= free;
                }
                let cells = &mut self.rows[row];
                let Some(index) = (0..cells.len()).filter(|&index| lot < cells[index].lot).nth(nth as usize) else { continue };
                cells[index].lot = lot;
                may_in[row] -= 1;
                left_in[row] -= 1;
                may -= 1;
                left -= 1;
                // Its neighbours are kept back for the rest of this lot.
                let neighbours = [index.checked_sub(1), Some(index + 1).filter(|&next| next < cells.len())];
                for neighbour in neighbours.into_iter().flatten() {
                    if lot < cells[neighbour].lot {
                        cells[neighbour].lot = KEPT_BACK;
                        may_in[row] -= 1;
                        may -= 1;
                    }
                }
            }
            for cell in self.cells_mut().filter(|cell| cell.lot == KEPT_BACK) {
                cell.lot = LEFT_OVER;
            }
        }
    }

    /// A frame of it (0x4737C). The original has none while the pause is up
    /// (0x12D0D8), which here stops all of the field.
    pub fn step(&mut self) {
        if !self.started {
            return;
        }
        if self.wait >= 0 {
            self.wait -= FRAMES;
            return;
        }
        match self.mode {
            Mode::Typing => self.type_on(),
            Mode::Going => self.fade(),
            Mode::Idle => {}
        }
        // Its clocks go on (0x47938).
        self.time += FRAMES;
        match self.mode {
            Mode::Typing => {
                self.beat += FRAMES;
                self.blink += FRAMES;
                if self.beat >= 4 {
                    self.beat = 0;
                }
                if !self.all_typed && self.blink >= 4 {
                    self.blink = 0;
                }
            }
            Mode::Going => {
                self.beat += FRAMES;
                self.blink += FRAMES;
                if self.beat >= 6 {
                    self.beat = 0;
                }
            }
            Mode::Idle => {}
        }
    }

    /// The letter that is typed next, if there is one.
    fn next(&self) -> Option<Cell> {
        self.rows.get(self.row).and_then(|row| row.get(self.column)).copied()
    }

    /// A frame of its coming (0x47418): on every other frame the next letter is
    /// there, and on the frames between the star shows where it will be; when all
    /// are, the star stands past the last and fades out and in, and the sign
    /// has come when it has faded out the fourth time.
    fn type_on(&mut self) {
        let next = self.next();
        if let (false, Some(cell)) = (self.all_typed, next) {
            self.cursor.at = cell.at;
        }
        if let (true, false, Some(cell)) = (self.beat >= 2 && self.time > 0, self.all_typed, next) {
            self.rows[self.row][self.column].alpha = 0xff;
            self.column += 1;
            if cell.space {
                self.wait = WAIT_AFTER_SPACE;
            } else {
                self.sounds.push(Request::Play { bank: COMMON_BANK.to_owned(), sound: SOUND_TYPED, volume: TYPED_VOLUME });
            }
            if self.column >= self.rows[self.row].len() {
                self.wait = WAIT_AFTER_LINE;
                if self.row + 1 < LINES && !self.rows[self.row + 1].is_empty() {
                    (self.row, self.column) = (self.row + 1, 0);
                } else {
                    self.all_typed = true;
                }
            }
            if self.all_typed {
                self.wait = 0;
                self.cursor.at[0] += GLYPH + 1;
            }
        }
        if !self.all_typed {
            // It is not seen over a space.
            let over_a_space = self.next().is_some_and(|cell| cell.space);
            self.cursor.alpha = if self.blink < 2 && !over_a_space { 0xff } else { 0 };
            return;
        }
        // Out and in over twenty frames each way (0x475BC).
        let blink = self.blink;
        let since = |from: i32| (blink - from) as f32 / 20.0;
        let share = match blink {
            ..3 => 0.0,
            3..0x16 => 1.0 - since(2),
            0x16..0x2a => since(0x16),
            0x2a..0x3e => 1.0 - since(0x2a),
            0x3e..0x52 => since(0x3e),
            0x52..0x66 => 1.0 - since(0x52),
            0x66..0x7a => since(0x66),
            0x7a..0x8e => 1.0 - since(0x7a),
            _ => {
                self.busy = false;
                0.0
            }
        };
        self.cursor.alpha = (share * 255.0) as u8;
    }

    /// A frame of its going (0x477D8): the lot whose turn it is fades over two
    /// frames and is gone on the third, the lot of those left over first.
    fn fade(&mut self) {
        if self.gone {
            return;
        }
        let lot = LEFT_OVER - self.lots_gone;
        if self.beat < 4 {
            let alpha = ((1.0 - self.beat as f32 * 0.25) * 255.0) as u8;
            self.cells_mut().filter(|cell| cell.lot == lot).for_each(|cell| cell.alpha = alpha);
        } else if self.beat < 6 {
            self.cells_mut().filter(|cell| cell.lot == lot).for_each(|cell| cell.alpha = 0);
            self.lots_gone += 1;
            if self.lots_gone >= LOTS {
                (self.mode, self.gone, self.busy) = (Mode::Idle, true, false);
            }
        }
        self.cursor.alpha = 0;
    }
}

/// What a sign's lots are drawn from. The original draws from the game's one
/// run of random numbers (0x93A0); here each sign has a run of its own, by its
/// place.
fn lots_of(place: i32) -> Random {
    Random::new(0x9e37_79b9_7f4a_7c15 ^ place as u64)
}

/// The title of a scene's place ("Location", 0xCFEC8), which a map's script
/// makes (`s36`), starts with how long it is to stand (`s39`), may tell to go
/// (`s129`) and waits to see the end of (`s25`).
#[derive(Clone, Debug, PartialEq)]
pub struct Title {
    pub sign: Sign,
    /// It has been started (its state 2): until then nothing of it is there.
    shown: bool,
    /// The frames of the interface it is to stand once it has come, those it
    /// has stood, and whether it has been told to go for that.
    stand: i32,
    stood: i32,
    going: bool,
}

impl Title {
    pub fn new(place: i32) -> Title {
        Title { sign: Sign::new(place), shown: false, stand: 0, stood: 0, going: false }
    }

    /// It is started (`0x101C`, 0xD0104): typed out, to stand so many frames of
    /// the game when it has been.
    pub fn start(&mut self, places: &Places, frames: i32) {
        self.sign.show(places, false, &mut lots_of(self.sign.place));
        (self.shown, self.stand) = (true, frames * FRAMES);
    }

    /// It is told to go (`0x101D`, 0xD0148).
    pub fn go(&mut self) {
        self.sign.go(false);
        self.shown = true;
    }

    /// Whether it is waiting to be started (`0x101A`, 0xD00E4).
    pub fn made(&self) -> bool {
        !self.shown
    }

    /// Whether anything of it is drawn (0xD00C0).
    pub fn shown(&self) -> bool {
        self.shown
    }

    /// A frame of it (0xCFFEC); says whether it is still there. Come, it stands
    /// its time and goes of itself; gone, its task ends.
    pub fn step(&mut self) -> bool {
        if !self.shown {
            return true;
        }
        self.sign.step();
        if self.going {
            return !self.sign.gone();
        }
        if self.sign.done() {
            if self.stood >= self.stand {
                self.sign.go(false);
                self.going = true;
            } else {
                self.stood += FRAMES;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Coming = 0,
    Standing = 1,
    Going = 2,
}

/// The name of the place she has come into, as the field shows it (window 16;
/// the field's object at + 0x1D88): all at once at the bottom left, for the
/// frames the field's tunables say, and then it goes. Another place named
/// meanwhile waits for this one to have gone.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceName {
    pub sign: Sign,
    state: State,
    /// The frames it is to stand, and those it has.
    frames: i32,
    stood: i32,
    /// It is to go as soon as it has come (its flag 0x10), and at once rather
    /// than a lot at a time (0x20).
    go_when_come: bool,
    at_once: bool,
    /// The place that is to be named when this one has gone, and for how long.
    waiting: Option<(i32, i32)>,
}

impl PlaceName {
    /// The name of a place, up (0xEEA2C, 0xEEAA0).
    fn new(places: &Places, place: i32, frames: i32) -> PlaceName {
        let mut sign = Sign::new(place);
        sign.put(Corner::BottomLeft as i32);
        sign.show(places, true, &mut lots_of(place));
        PlaceName { sign, state: State::Coming, frames, stood: 0, go_when_come: false, at_once: false, waiting: None }
    }

    /// It is told to go (0xEED78): at once, or a lot at a time. Whatever waited
    /// to be named after it is forgotten. The field tells it so, a lot at a
    /// time, when something begins that it is not to be seen over (0x115E44).
    pub fn dismiss(&mut self, at_once: bool) {
        self.at_once = at_once;
        self.waiting = None;
        match self.state {
            State::Coming => self.go_when_come = true,
            State::Standing => self.leave(),
            State::Going => {}
        }
    }

    /// 0xEEB5C.
    fn leave(&mut self) {
        self.sign.go(self.at_once);
        self.go_when_come = false;
        self.state = State::Going;
    }

    /// The field's window 16 (0xEDC8C): the name that is up, if one is, goes at
    /// once, and the place is named when it has; with none up it is named now
    /// (0xEEC70).
    pub fn name(shown: &mut Option<PlaceName>, places: &Places, place: i32, frames: i32) {
        match shown {
            None => *shown = Some(PlaceName::new(places, place, frames)),
            Some(name) => {
                name.dismiss(true);
                (name.go_when_come, name.at_once) = (false, false);
                if name.state == State::Standing {
                    name.sign.go(false);
                    name.state = State::Going;
                }
                name.waiting = Some((place, frames));
            }
        }
    }

    /// A frame of it (0xEEDF8); says whether a name is still up.
    pub fn step(&mut self, places: &Places) -> bool {
        match self.state {
            State::Coming => {
                self.sign.step();
                if self.sign.done() {
                    if self.go_when_come {
                        self.leave();
                    } else if self.waiting.is_some() {
                        self.sign.go(false);
                        self.state = State::Going;
                    } else {
                        self.state = State::Standing;
                    }
                }
            }
            State::Standing => {
                self.stood += 1;
                if self.stood >= self.frames {
                    self.dismiss(false);
                }
                self.sign.step();
            }
            State::Going => {
                self.sign.step();
                if self.sign.done() {
                    match self.waiting.take() {
                        Some((place, frames)) => *self = PlaceName::new(places, place, frames),
                        None => return false,
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    fn places() -> Option<Places> {
        Some(Places::load(&GameData::new(DATA).ok()?).unwrap())
    }

    /// Which of a sign's letters are there, line by line.
    fn there(sign: &Sign) -> String {
        let letter = |cell: &Cell| if cell.alpha != 0 { '#' } else { '.' };
        sign.rows.iter().map(|row| row.iter().map(letter).collect::<String>()).collect::<Vec<_>>().join("/")
    }

    fn typed(sign: &Sign) -> usize {
        there(sign).matches('#').count()
    }

    #[test]
    fn the_tables_have_the_places_lines_and_the_letters() {
        let Some(places) = places() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!((places.places.len(), places.glyphs.len()), (84, 41));
        assert_eq!(places.lines(0).unwrap(), &["25 SEPTEMBER\u{ff1a}2051".to_owned(), "37.77-122.41".to_owned(), "SAN FRANCISCO".to_owned()]);
        assert_eq!(places.lines(12).unwrap(), &["THE END OF ANKH ST.".to_owned(), String::new(), String::new()]);
        assert!(places.lines(84).is_none() && places.lines(-1).is_none());
        // A space and a full stop are narrow, and the full stop sits back.
        assert_eq!(places.glyph(' '), Some(Glyph { code: ' ', dx: 0, dy: 0, advance: 5 }));
        assert_eq!(places.glyph('.'), Some(Glyph { code: '.', dx: -4, dy: 0, advance: 5 }));
        assert_eq!(places.glyph('A').map(|glyph| glyph.advance), Some(11));
        assert_eq!(places.glyph('a'), None);
        // Every character of every place has a letter.
        for place in &places.places {
            for character in place.iter().flat_map(|line| line.chars()) {
                assert!(places.glyph(character).is_some(), "{character:?} of {place:?}");
            }
        }
        // Where the letters are in the sheet.
        assert_eq!((source('A'), source('K'), source('L'), source('Z')), ([0, 0], [110, 0], [0, 11], [33, 22]));
        assert_eq!((source('0'), source('9')), ([0, 33], [99, 33]));
        assert_eq!((source(' '), source('\u{ff1a}'), source('-'), source('.'), source('?')), ([44, 22], [55, 22], [66, 22], [77, 22], STAR));
    }

    #[test]
    fn every_map_names_a_place_the_sign_has_lines_for() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = crate::field::tables::FieldTables::load(&data).unwrap();
        for map in &tables.maps {
            let lines = tables.places.lines(i32::from(map.sign));
            assert!(lines.is_some_and(|lines| !lines[0].is_empty()), "{} names place {}", map.name, map.sign);
        }
        // The first map is the end of Ankh Street, which is what a save calls it
        // too; a map with no name of its own names that place as well.
        let (first, unnamed) = (&tables.maps[0], &tables.maps[tables.map_number("d_stg01_0g").unwrap()]);
        assert_eq!((first.sign, tables.places.lines(12).unwrap()[0].as_str(), first.place.as_str()), (12, "THE END OF ANKH ST.", "THE END OF ANKH ST."));
        assert_eq!((unnamed.sign, unnamed.place.as_str()), (12, "NONE"));
        // The name of a place stands two hundred frames (the field's tunables).
        assert_eq!(tables.tunables.u16(0x14).unwrap(), 200);
    }

    #[test]
    fn a_places_name_is_laid_out_from_the_bottom_left_and_goes_in_six_lots() {
        let Some(places) = places() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut shown = None;
        PlaceName::name(&mut shown, &places, 12, 90);
        let name = shown.as_mut().unwrap();
        // All of it is there at once: one line of nineteen letters, the first at
        // (6, 217), each twelve on but after a space, which is six.
        let row = &name.sign.rows[0];
        assert_eq!((row.len(), name.sign.rows[1].len(), name.sign.rows[2].len()), (19, 0, 0));
        assert!(row.iter().all(|cell| cell.alpha == 0xff) && name.sign.done() && !name.sign.gone());
        assert_eq!((row[0].at, row[1].at, row[3].at, row[4].at), ([6, 217], [18, 217], [42, 217], [48, 217]));
        // "ST." ends it: the full stop sits four back from where the pen is.
        assert_eq!((row[17].source, row[18].source, row[18].at[0] - row[17].at[0]), (source('T'), source('.'), 8));
        // Five lots of three letters, none of them side by side, and the rest.
        for lot in 0..5 {
            let of_lot: Vec<usize> = (0..row.len()).filter(|&index| row[index].lot == lot).collect();
            assert_eq!(of_lot.len(), 3, "lot {lot}: {of_lot:?}");
            assert!(of_lot.windows(2).all(|pair| pair[1] - pair[0] > 1), "lot {lot}: {of_lot:?}");
        }
        assert_eq!(row.iter().filter(|cell| cell.lot == LEFT_OVER).count(), 4);

        // It has come in its first frame, stands its ninety and is told to go.
        let mut frames = 0;
        while name.state != State::Going {
            assert!(name.step(&places));
            frames += 1;
            assert!(frames < 1000);
        }
        assert_eq!(frames, 91);
        // A wait of three frames more, and then each lot over three frames, the
        // lot of those left over first: all there, half there, gone.
        let alphas = |name: &PlaceName, lot: i32| name.sign.rows[0].iter().filter(|cell| cell.lot == lot).map(|cell| cell.alpha).collect::<Vec<_>>();
        for _ in 0..4 {
            assert!(name.step(&places));
            assert!(name.sign.rows[0].iter().all(|cell| cell.alpha == 0xff));
        }
        assert!(name.step(&places));
        assert_eq!((alphas(name, LEFT_OVER), alphas(name, 4)), (vec![127; 4], vec![0xff; 3]));
        assert!(name.step(&places));
        assert_eq!((alphas(name, LEFT_OVER), alphas(name, 4)), (vec![0; 4], vec![0xff; 3]));
        assert!(name.step(&places));
        assert!(name.step(&places));
        assert_eq!((alphas(name, 4), alphas(name, 3)), (vec![127; 3], vec![0xff; 3]));
        // Six lots are eighteen frames, and with the last the name is done with.
        let mut frames = 5;
        while name.step(&places) {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(frames + 1, 18);
        assert!(name.sign.gone() && name.sign.rows[0].iter().all(|cell| cell.alpha == 0));
    }

    #[test]
    fn a_place_named_while_another_is_up_waits_for_that_to_have_gone() {
        let Some(places) = places() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut shown = None;
        PlaceName::name(&mut shown, &places, 12, 90);
        for _ in 0..10 {
            assert!(shown.as_mut().unwrap().step(&places));
        }
        assert_eq!(shown.as_ref().unwrap().state, State::Standing);
        // The one that is up is gone at once, and the next comes the frame after.
        PlaceName::name(&mut shown, &places, 13, 90);
        let name = shown.as_mut().unwrap();
        assert!(name.state == State::Going && name.sign.gone() && name.waiting == Some((13, 90)));
        assert!(name.step(&places));
        assert_eq!((name.state, name.sign.rows[0].len(), name.stood), (State::Coming, "THE CENTER OF TOWN".len(), 0));
        assert!(name.sign.rows[0].iter().all(|cell| cell.alpha == 0xff));

        // Told to go a lot at a time, as when something begins over the field, it
        // goes so from where it stands, and is not named again.
        assert!(name.step(&places));
        name.dismiss(false);
        assert_eq!(name.state, State::Going);
        let mut frames = 0;
        while name.step(&places) {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(frames + 1, 4 + 18);
    }

    #[test]
    fn a_scenes_title_is_typed_letter_by_letter_behind_its_star() {
        let Some(places) = places() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut title = Title::new(0);
        // Made, nothing of it is there, and it waits to be started.
        for _ in 0..5 {
            assert!(title.step());
        }
        assert!(title.made() && !title.shown() && there(&title.sign) == "//");
        title.start(&places, 60);
        assert!(title.shown() && !title.made());
        assert_eq!(there(&title.sign), format!("{}/{}/{}", ".".repeat(17), ".".repeat(12), ".".repeat(13)));
        // Its lines begin at the top left, thirteen apart.
        let first = |row: usize| title.sign.rows[row][0].at;
        assert_eq!((first(0), first(1), first(2)), ([6, 6], [6, 19], [6, 32]));

        // A letter every other frame, each with its sound; the star shows on the
        // frames between, where the next letter will be.
        assert!(title.step());
        assert_eq!((title.sign.cursor.at, title.sign.cursor.alpha, typed(&title.sign)), ([6, 6], 0xff, 0));
        assert!(title.step());
        assert_eq!((title.sign.rows[0][0].alpha, title.sign.cursor.alpha, title.sign.sounds.len()), (0xff, 0, 1));
        assert_eq!(title.sign.sounds[0], Request::Play { bank: COMMON_BANK.to_owned(), sound: 9, volume: 42 });
        assert!(title.step());
        assert_eq!((title.sign.cursor.at, title.sign.cursor.alpha), ([18, 6], 0xff));
        assert!(title.step());
        assert_eq!(typed(&title.sign), 2);
        // The third is a space: the star is not seen over it, it has no sound, and
        // two frames are waited after it.
        assert!(title.step());
        assert_eq!(title.sign.cursor.alpha, 0);
        assert!(title.step());
        assert_eq!((typed(&title.sign), title.sign.sounds.len()), (3, 2));
        let mut frames = 0;
        while typed(&title.sign) < 4 {
            assert!(title.step());
            frames += 1;
            assert!(frames < 10);
        }
        assert_eq!(frames, 4);

        // All three lines are typed in the end, eleven frames waited after each of
        // the first two, with a sound for every letter that is not a space.
        let mut frames = 10;
        while !title.sign.all_typed {
            assert!(title.step());
            frames += 1;
            assert!(frames < 1000);
        }
        let (letters, spaces, lines) = (17 + 12 + 13, 2, 3);
        assert_eq!((typed(&title.sign), title.sign.sounds.len()), (letters, letters - spaces));
        assert_eq!(frames, 2 * letters + 2 * spaces + 11 * (lines - 1));
        // Then the star stands past the last letter and fades out and in, and the
        // title has come.
        let last = *title.sign.rows[2].last().unwrap();
        assert_eq!(title.sign.cursor.at, [last.at[0] + 12, last.at[1]]);
        let (mut frames, mut brightest) = (0, 0);
        while !title.sign.done() {
            assert!(title.step());
            frames += 1;
            brightest = brightest.max(title.sign.cursor.alpha);
            assert!(frames < 200);
        }
        assert_eq!((frames, brightest, title.sign.cursor.alpha), (71, 0xff, 0));
        // It stands sixty frames of the game, goes of itself, and is done with
        // when it has gone.
        let mut frames = 0;
        while !title.going {
            assert!(title.step());
            frames += 1;
            assert!(frames < 200);
        }
        assert_eq!(frames, 60);
        let mut frames = 0;
        while title.step() {
            frames += 1;
            assert!(frames < 100);
        }
        assert!(title.sign.gone() && typed(&title.sign) == 0);
        assert_eq!(frames + 1, 4 + 18);
    }
}
