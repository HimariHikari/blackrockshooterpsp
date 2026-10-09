//! The game's text, as scripts and tables give it: plain text with codes in it that
//! colour it, put a button's icon in it, the name of an item or a number, or end a
//! page. The original lays a text out glyph by glyph (0x2837C); here it is cut into
//! runs of one colour, to be drawn by whatever draws text. See `docs/FIELD_NOTES.md`.

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_formats::xtc;

use crate::data::GameData;

const COLOURS: &str = "SYSTEM.VOL/font.lpk/003.xtc";

// The codes (the jump table at 0x1661B8). The others below 0x1E are drawn as glyphs.
const NEW_LINE: char = '\n';
const NEW_PAGE: char = '\x0c';
const COLOUR: char = '\x13';
const ITEM_NAME: char = '\x14';
const ICON: char = '\x15';
/// Reads a number and does nothing with it.
const NOTHING: char = '\x16';
const DASH: char = '\x17';
// Two that put in a text of the game's own: the function the first asks has none to
// give (0x12CF74), and the second names one of the work area's that the port does
// not keep (0x12FBE8).
const NO_TEXT: char = '\x19';
const WORK_TEXT: char = '\x1a';
const NUMBER: char = '\x1d';
/// The same things can be asked for in writing: a backslash, a letter, and a number
/// in brackets (the jump table at 0x166208).
const ESCAPE: char = '\\';

/// The colour the name of an item is put in (0x28508).
const NAME_COLOUR: u8 = 1;
/// One length of the dash a text can draw across (the string at 0x166260).
#[cfg(test)]
const DASH_PIECE: char = '\u{2015}';

// An icon and a dash are kept in a run's text as characters of their own, from the
// range that is nobody's: the game's texts have none of it.
const ICON_BASE: u32 = 0xe000;
const DASH_BASE: u32 = 0xe100;
const MARKS: u32 = 0x100;

/// The icon a character of a run stands for, by its number.
pub fn as_icon(character: char) -> Option<i32> {
    (ICON_BASE..ICON_BASE + MARKS).contains(&(character as u32)).then(|| (character as u32 - ICON_BASE) as i32)
}

/// The dash a character of a run stands for: how many pieces long it is.
pub fn as_dash(character: char) -> Option<i32> {
    (DASH_BASE..DASH_BASE + MARKS).contains(&(character as u32)).then(|| (character as u32 - DASH_BASE) as i32)
}

/// A run of text in one colour. A new line is a `'\n'` in it; an icon and a dash
/// are each one character ([`as_icon`], [`as_dash`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    /// One of the game's text colours, by number; 0 is the one text starts in.
    pub colour: u8,
}

/// What a text can ask to have put in.
pub trait Lookup {
    /// A variable of the script that is showing the text: one of the locals of the
    /// function that called, or one of the script's globals.
    fn local(&self, index: usize) -> i32;
    fn global(&self, index: usize) -> i32;
    /// The name of an item.
    fn item(&self, number: i32) -> String;
}

/// A lookup for texts that ask for nothing.
pub struct Plain;

impl Lookup for Plain {
    fn local(&self, _: usize) -> i32 {
        0
    }

    fn global(&self, _: usize) -> i32 {
        0
    }

    fn item(&self, _: i32) -> String {
        String::new()
    }
}

/// A button's icon in words, by its number, for what is written out rather than
/// drawn. The names are of what the sheet of icons shows (`font.lpk/002.sbs`).
pub fn icon(number: i32) -> &'static str {
    match number {
        0x00 => "Circle",
        0x01 => "Cross",
        0x02 => "Triangle",
        0x03 => "Square",
        0x29 => "L",
        0x2a => "R",
        0x2b => "SELECT",
        0x2c => "START",
        0x2d => "Up",
        0x2e => "Down",
        0x2f => "Left",
        0x30 => "Right",
        _ => "?",
    }
}

/// Cuts a text into its pages, and each page into runs of one colour.
pub fn pages(text: &str, lookup: &dyn Lookup) -> Vec<Vec<Span>> {
    let mut layout = Layout { pages: vec![Vec::new()], colour: 0 };
    let mut rest = text.chars();
    while let Some(character) = rest.next() {
        match character {
            NEW_PAGE => layout.pages.push(Vec::new()),
            COLOUR => layout.colour = short(&mut rest, lookup) as u8,
            ITEM_NAME => layout.name(long(&mut rest, lookup), lookup),
            ICON => layout.icon(short(&mut rest, lookup)),
            NOTHING => {
                long(&mut rest, lookup);
            }
            DASH => layout.dash(short(&mut rest, lookup)),
            NO_TEXT => {
                short(&mut rest, lookup);
            }
            WORK_TEXT => {
                hex(&mut rest, 2);
            }
            NUMBER => layout.push(&long(&mut rest, lookup).to_string()),
            ESCAPE => {
                let Some(letter) = rest.next() else { break };
                let number = bracketed(&mut rest);
                match letter {
                    'b' => layout.icon(number),
                    'c' => layout.colour = number as u8,
                    'f' => layout.pages.push(Vec::new()),
                    'z' => layout.dash(number),
                    // The other letters do nothing.
                    _ => {}
                }
            }
            NEW_LINE => layout.push("\n"),
            other => layout.push(other.encode_utf8(&mut [0; 4])),
        }
    }
    layout.pages
}

/// The text of a page with nothing but its characters: an icon named in brackets,
/// a dash as so many of its pieces.
#[cfg(test)]
pub fn plain(page: &[Span]) -> String {
    let mut words = String::new();
    for character in page.iter().flat_map(|span| span.text.chars()) {
        if let Some(number) = as_icon(character) {
            words += &format!("[{}]", icon(number));
        } else if let Some(pieces) = as_dash(character) {
            words.extend(std::iter::repeat_n(DASH_PIECE, pieces as usize));
        } else {
            words.push(character);
        }
    }
    words
}

struct Layout {
    pages: Vec<Vec<Span>>,
    colour: u8,
}

impl Layout {
    fn push(&mut self, text: &str) {
        let page = self.pages.last_mut().expect("there is always a page");
        match page.last_mut() {
            Some(span) if span.colour == self.colour => span.text += text,
            _ => page.push(Span { text: text.to_owned(), colour: self.colour }),
        }
    }

    fn name(&mut self, item: i32, lookup: &dyn Lookup) {
        let colour = std::mem::replace(&mut self.colour, NAME_COLOUR);
        self.push(&lookup.item(item));
        self.colour = colour;
    }

    fn icon(&mut self, number: i32) {
        self.mark(ICON_BASE, number);
    }

    fn dash(&mut self, pieces: i32) {
        if pieces > 0 {
            self.mark(DASH_BASE, pieces);
        }
    }

    /// An icon or a dash, as the character that stands for it; one whose number
    /// is out of all range is left out.
    fn mark(&mut self, base: u32, number: i32) {
        let character = u32::try_from(number).ok().filter(|number| *number < MARKS).and_then(|number| char::from_u32(base + number));
        if let Some(character) = character {
            self.push(character.encode_utf8(&mut [0; 4]));
        }
    }
}

/// A number after a code, in two digits (0x25CF4) or in four (0x27438), all in
/// sixteens; or a variable of the script, named by `L` or `G` and eight digits.
fn short(rest: &mut std::str::Chars, lookup: &dyn Lookup) -> i32 {
    number(rest, lookup, 2)
}

fn long(rest: &mut std::str::Chars, lookup: &dyn Lookup) -> i32 {
    number(rest, lookup, 4)
}

fn number(rest: &mut std::str::Chars, lookup: &dyn Lookup, digits: usize) -> i32 {
    let mut ahead = rest.clone();
    match ahead.next().map(|first| first.to_ascii_uppercase()) {
        Some(kind @ ('L' | 'G')) => {
            *rest = ahead;
            let index = hex(rest, 8) as usize;
            if kind == 'L' { lookup.local(index) } else { lookup.global(index) }
        }
        _ => hex(rest, digits) as i32,
    }
}

/// The next `digits` characters as a number in sixteens. One that is no digit counts
/// as nothing, as the original's conversion has it.
fn hex(rest: &mut std::str::Chars, digits: usize) -> u32 {
    let mut value = 0u32;
    for _ in 0..digits {
        let digit = rest.next().and_then(|character| character.to_digit(16)).unwrap_or(0);
        value = (value << 4) | digit;
    }
    value
}

/// The number in brackets after an escape's letter, if there is one: every digit up
/// to the closing bracket counts and anything else there is passed over (0x28700).
fn bracketed(rest: &mut std::str::Chars) -> i32 {
    let mut ahead = rest.clone();
    if ahead.next() != Some('(') {
        return 0;
    }
    *rest = ahead;
    let mut value = 0i32;
    for character in rest.by_ref() {
        match character.to_digit(10) {
            _ if character == ')' => break,
            Some(digit) => value = value.wrapping_mul(10).wrapping_add(digit as i32),
            None => {}
        }
    }
    value
}

/// The colours a text can be put in, by number (the table 0x12CF7C reads from the
/// font's pack).
#[derive(Resource, Clone, Debug)]
pub struct TextColours(Vec<[u8; 4]>);

impl TextColours {
    pub fn load(data: &GameData) -> Result<TextColours> {
        let table = xtc::Table::parse(&data.read(COLOURS)?).with_context(|| format!("parsing {COLOURS}"))?;
        let colour = |index: usize| {
            let record = table.record(index).filter(|record| record.len() >= 4)?;
            Some([record[0], record[1], record[2], record[3]])
        };
        Ok(TextColours((0..table.len()).filter_map(colour).collect()))
    }

    /// Colour `number`, as red, green, blue and alpha; one the table does not have
    /// is the first, as text starts.
    pub fn bytes(&self, number: u8) -> [u8; 4] {
        self.0.get(number as usize).or(self.0.first()).copied().unwrap_or([255; 4])
    }

    pub fn get(&self, number: u8) -> Color {
        let [red, green, blue, alpha] = self.bytes(number);
        Color::srgba_u8(red, green, blue, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Script;

    impl Lookup for Script {
        fn local(&self, index: usize) -> i32 {
            [0, 0, 7, 3][index]
        }

        fn global(&self, index: usize) -> i32 {
            100 + index as i32
        }

        fn item(&self, number: i32) -> String {
            format!("item {number}")
        }
    }

    fn span(text: &str, colour: u8) -> Span {
        Span { text: text.into(), colour }
    }

    #[test]
    fn a_colour_code_colours_what_follows_it() {
        let text = "Tutorial:\n\u{201c}\x1301Battle Basics\x1300\u{201d}";
        let expected = vec![span("Tutorial:\n\u{201c}", 0), span("Battle Basics", 1), span("\u{201d}", 0)];
        assert_eq!(pages(text, &Plain), [expected]);
    }

    #[test]
    fn icons_are_named_and_a_dash_is_as_long_as_it_is_told() {
        let text = "\x152A+\x1503: Skill\x1702";
        assert_eq!(plain(&pages(text, &Plain)[0]), "[R]+[Square]: Skill\u{2015}\u{2015}");
    }

    #[test]
    fn a_text_can_name_the_scripts_variables() {
        // What finding something says: the item is the function's third local, how
        // many its fourth.
        let text = "You have obtained\n\x14L00000002 x\x1dL00000003.";
        let expected = vec![span("You have obtained\n", 0), span("item 7", 1), span(" x3.", 0)];
        assert_eq!(pages(text, &Script), [expected]);
        // Globals likewise, and numbers written out: four digits in sixteens.
        assert_eq!(plain(&pages("\x1dG00000002 \x1d00ff \x140005", &Script)[0]), "102 255 item 5");
        // A number that is read and dropped leaves nothing behind.
        assert_eq!(plain(&pages("a\x160001b\x1901c\x1a02d", &Script)[0]), "abcd");
    }

    #[test]
    fn a_page_ends_where_the_text_says() {
        let pages = pages("one\x0ctwo\\fthree", &Plain);
        let texts: Vec<String> = pages.iter().map(|page| plain(page)).collect();
        assert_eq!(texts, ["one", "two", "three"]);
    }

    #[test]
    fn the_same_can_be_asked_for_in_writing() {
        let text = "To Mama\\z(2) \\c(3)red\\c(0) \\b(1) \\q(9)x\\c";
        // The dash and the icon are each a character of their own in their runs.
        let expected = vec![
            span("To Mama\u{e102} ", 0),
            span("red", 3),
            // A letter that means nothing is passed over with its number, and one
            // with no number has none.
            span(" \u{e001} x", 0),
        ];
        let pages = pages(text, &Plain);
        assert_eq!(pages, [expected]);
        assert_eq!(plain(&pages[0]), "To Mama\u{2015}\u{2015} red [Cross] x");
        assert_eq!((as_dash('\u{e102}'), as_icon('\u{e001}'), as_icon('x'), as_dash('\u{e001}')), (Some(2), Some(1), None, None));
    }
}
