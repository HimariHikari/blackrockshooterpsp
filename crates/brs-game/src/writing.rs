//! Writing on the screen with the game's own fonts, as its text objects do (0x280E8
//! to 0x28B00; `docs/FORMAT_NOTES.md` has the fonts, `docs/FIELD_NOTES.md` the rules).
//!
//! A text is laid out glyph by glyph into lines ([`Fonts::write`]): there is no
//! wrapping by width, only new lines where the text has them and when a line has
//! as many glyphs as it can hold. Every glyph is the font's whole cell of 15 by 15
//! drawn at the style's size; what differs from one character to the next is how far
//! the pen moves on, which the font's metrics say for the characters that are not as
//! wide as the cell. A block is then drawn at a place ([`Fonts::draw`]), each line
//! from the left, centred or from the right of the width it is given.

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::modulate;
use brs_engine::texture;
use brs_formats::font::{Codes, Font, Icons};
use brs_formats::ptmd;

use crate::data::GameData;
use crate::text::{self, Span, TextColours};

const TEXT_FONT: &str = "SYSTEM.VOL/font.lpk/000.bin";
const CAPITALS_FONT: &str = "SYSTEM.VOL/font.lpk/001.bin";
const ICON_SHEET: &str = "SYSTEM.VOL/font.lpk/002.sbs";
const CODES: &str = "SYSTEM.VOL/ucs2jis.bin";

/// The glyphs are kept in one picture, a cell each with a pixel to spare, so that
/// smoothing one does not take from its neighbour.
const ATLAS_CELL: usize = 16;
const ATLAS_ACROSS: usize = 64;
/// What is this clear or clearer is not drawn (0xD1944: the alpha test the texts
/// are drawn under).
const ALPHA_FLOOR: u8 = 8;

/// The characters below this are of the narrow font (0x278E0), which the game never
/// loads: they draw nothing and move nothing.
const FIRST_WIDE: u32 = 0x81;
/// What ASCII letters and figures are moved by to their full-width forms (0x27848:
/// the figure nought to U+FF10).
const TO_FULL_WIDTH: u32 = 0xfee0;
/// The other characters that are written as full-width ones (0x170F98, 0x170FAC).
const FULL_WIDTH: [(char, char); 20] = [
    (' ', '\u{3000}'),
    ('"', '\u{201d}'),
    ('\'', '\u{2019}'),
    ('.', '\u{ff0e}'),
    (',', '\u{ff0c}'),
    ('<', '\u{ff1c}'),
    ('>', '\u{ff1e}'),
    ('!', '\u{ff01}'),
    ('?', '\u{ff1f}'),
    ('-', '\u{ff0d}'),
    ('+', '\u{ff0b}'),
    ('(', '\u{ff08}'),
    (')', '\u{ff09}'),
    ('/', '\u{ff0f}'),
    ('*', '\u{ff0a}'),
    ('&', '\u{ff06}'),
    ('%', '\u{ff05}'),
    (';', '\u{ff1b}'),
    (':', '\u{ff1a}'),
    ('#', '\u{ff03}'),
];
/// What is drawn for a character the font has no glyph for (0x16619C).
const NO_GLYPH: char = '\u{ff20}';
/// The character a dash is drawn from (0x1661A0).
const DASH: char = '\u{2015}';
// A dash is that glyph's left quarter, half a glyph's width of its middle column
// for each piece after the first, and its second quarter (0x26FC8).
const DASH_END: f32 = 0.25;
const DASH_MIDDLE: f32 = 0.5;

/// The colour a text's shadow is drawn in, by the text's own (0x2DA48), and how far
/// right and down of the text it lies.
const SHADOW: [u8; 3] = [0x22, 0x18, 0x15];
const SHADOW_OFFSET: Vec2 = Vec2::new(1.0, 1.0);

/// Which font a text is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// The text's: 3,354 glyphs.
    Text = 0,
    /// Big capitals and figures: 40 glyphs.
    Capitals = 1,
}

/// How a text is written: what its text object is set up with (0x2D788).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub face: Face,
    /// The size a glyph's cell is drawn at; a line is as high.
    pub size: f32,
    /// What is left between one glyph and the next, and between lines.
    pub letter_gap: f32,
    pub line_gap: f32,
    /// The glyphs a line holds before another is begun, and the lines there can be
    /// after the first.
    pub per_line: usize,
    pub lines: usize,
    /// The frames each glyph takes to come up; none for a text that is there at once.
    pub reveal: f32,
}

impl Style {
    /// One of the kinds of text the game has (0x2D7FC): its own font at 15, or the
    /// capitals at 5, 10 or 15.
    pub const fn of_kind(kind: i32, per_line: usize, lines: usize, letter_gap: f32, line_gap: f32) -> Style {
        let (face, size) = match kind {
            0 => (Face::Text, 15.0),
            1 => (Face::Capitals, 5.0),
            2 => (Face::Capitals, 10.0),
            _ => (Face::Capitals, 15.0),
        };
        Style { face, size, letter_gap, line_gap, per_line, lines, reveal: 0.0 }
    }
}

/// How a block's lines lie in the width they are given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Centre,
    Right,
}

/// Where and how a block is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placing {
    pub at: Vec2,
    /// The width its lines are centred in or lie to the right of.
    pub width: f32,
    pub align: Align,
    /// The colour all of it is drawn by, over its own.
    pub tint: [u8; 4],
    pub shadow: bool,
    /// The frames it has had to come up in.
    pub shown: f32,
}

impl Placing {
    pub fn at(at: Vec2) -> Placing {
        Placing { at, width: brs_engine::canvas::WIDTH, align: Align::Left, tint: [255; 4], shadow: false, shown: f32::INFINITY }
    }
}

/// What a glyph of a block is drawn from.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    /// A cell of the fonts' picture, from so far across it to so far.
    Glyph { cell: usize, across: [f32; 2] },
    /// A part of the icons' picture, in its pixels.
    Icon(Rect),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Piece {
    /// From the start of its line.
    area: Rect,
    source: Source,
    colour: [u8; 4],
    /// When it comes up, in its line's frames.
    time: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Line {
    top: f32,
    width: f32,
    /// The frames it takes to come up.
    time: f32,
    /// What has been put on it, drawn or not.
    count: usize,
    pieces: Vec<Piece>,
}

/// A text laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    lines: Vec<Line>,
    line_height: f32,
    line_gap: f32,
}

impl Block {
    /// The width of its widest line and the height of its lines together (0x273AC).
    pub fn size(&self) -> Vec2 {
        let width = self.lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let gaps = self.lines.len().saturating_sub(1) as f32 * self.line_gap;
        Vec2::new(width, self.lines.len() as f32 * self.line_height + gaps)
    }

    /// The frames all of it takes to come up.
    pub fn time(&self) -> f32 {
        self.lines.iter().map(|line| line.time).sum()
    }
}

/// The game's fonts and icons, as pictures to draw from.
#[derive(Resource)]
pub struct Fonts {
    faces: [(Font, usize); 2],
    codes: Codes,
    icons: Icons,
    atlas: Handle<Image>,
    atlas_size: Vec2,
    icon_sheet: Handle<Image>,
    icon_sheet_size: Vec2,
}

impl Fonts {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<Fonts> {
        let font = |path: &str| Font::parse(&data.read(path)?).with_context(|| format!("parsing {path}"));
        let (text, capitals) = (font(TEXT_FONT)?, font(CAPITALS_FONT)?);
        let codes = Codes::parse(&data.read(CODES)?).with_context(|| format!("parsing {CODES}"))?;
        let icons = Icons::parse(&data.read(ICON_SHEET)?).with_context(|| format!("parsing {ICON_SHEET}"))?;

        // Both fonts' glyphs, the text's first.
        let cells = text.len() + capitals.len();
        let (width, height) = (ATLAS_ACROSS * ATLAS_CELL, cells.div_ceil(ATLAS_ACROSS) * ATLAS_CELL);
        let mut pixels = vec![0u8; width * height * 4];
        let fonts = [(&text, 0), (&capitals, text.len())];
        for (font, first) in fonts {
            for glyph in 0..font.len() {
                let (left, top) = cell_corner(first + glyph);
                for (pixel, colour) in font.pixels(glyph).enumerate() {
                    let (x, y) = (left + pixel % font.width, top + pixel / font.width);
                    let alpha = if colour[3] < ALPHA_FLOOR { 0 } else { colour[3] };
                    pixels[(y * width + x) * 4..][..4].copy_from_slice(&[colour[0], colour[1], colour[2], alpha]);
                }
            }
        }
        let size = Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 };
        // As the game's textures are (`brs_engine::texture`): plain numbers.
        let atlas = Image::new(size, TextureDimension::D2, pixels, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);

        let header = ptmd::parse_header(&icons.texture).context("the icons' sheet has no texture")?;
        let icon_sheet = texture::image(&icons.texture, &header);
        let icon_sheet_size = texture::size(&header);
        let first_capital = text.len();
        Ok(Fonts {
            faces: [(text, 0), (capitals, first_capital)],
            codes,
            icons,
            atlas: images.add(atlas),
            atlas_size: Vec2::new(width as f32, height as f32),
            icon_sheet: images.add(icon_sheet),
            icon_sheet_size,
        })
    }

    /// Lays a text out (0x2837C): its runs of one colour, with new lines in them.
    pub fn write(&self, style: &Style, spans: &[Span], colours: &TextColours) -> Block {
        let mut writer = Writer { fonts: self, style, lines: vec![Line::default()], pen: 0.0, time: 0.0 };
        for span in spans {
            let colour = colours.bytes(span.colour);
            for character in span.text.chars() {
                if character == '\n' {
                    writer.new_line();
                } else if let Some(number) = text::as_icon(character) {
                    writer.icon(number);
                } else if let Some(pieces) = text::as_dash(character) {
                    writer.dash(pieces, colour);
                } else {
                    writer.glyph(character, colour);
                }
            }
        }
        Block { lines: writer.lines, line_height: style.size, line_gap: style.line_gap }
    }

    /// A text of one colour, as the tables and the code have theirs.
    pub fn write_plain(&self, style: &Style, words: &str, colours: &TextColours) -> Block {
        self.write(style, &[Span { text: words.to_owned(), colour: 0 }], colours)
    }

    /// Draws a block (0x26C34, 0x2D900): its shadow first if it has one.
    pub fn draw(&self, canvas: &mut Canvas, layer: i32, block: &Block, placing: &Placing) {
        if placing.shadow {
            let [red, green, blue] = SHADOW;
            self.draw_once(canvas, layer, block, placing, SHADOW_OFFSET, [red, green, blue, placing.tint[3]]);
        }
        self.draw_once(canvas, layer, block, placing, Vec2::ZERO, placing.tint);
    }

    fn draw_once(&self, canvas: &mut Canvas, layer: i32, block: &Block, placing: &Placing, offset: Vec2, tint: [u8; 4]) {
        // The lines come up one after the other, each in its own frames.
        let mut left = placing.shown;
        for line in &block.lines {
            let shown = left.min(line.time);
            left -= shown;
            let spare = placing.width - line.width;
            let x = match placing.align {
                Align::Left => placing.at.x,
                Align::Centre => (placing.at.x + spare * 0.5).trunc(),
                Align::Right => (placing.at.x + spare).trunc(),
            };
            let origin = Vec2::new(x, placing.at.y + line.top) + offset;
            for piece in line.pieces.iter().take_while(|piece| piece.time <= shown) {
                let (texture, size, source) = match piece.source {
                    Source::Glyph { cell, across } => {
                        let (left, top) = cell_corner(cell);
                        let cell_size = (ATLAS_CELL - 1) as f32;
                        let from = Vec2::new(left as f32 + across[0] * cell_size, top as f32);
                        let to = Vec2::new(left as f32 + across[1] * cell_size, top as f32 + cell_size);
                        (&self.atlas, self.atlas_size, Rect::from_corners(from, to))
                    }
                    Source::Icon(area) => (&self.icon_sheet, self.icon_sheet_size, area),
                };
                let colour = modulate(piece.colour, tint);
                canvas.sprite(
                    layer,
                    Paint::textured(texture),
                    Vertex::new(origin + piece.area.min, source.min / size, colour),
                    Vertex::new(origin + piece.area.max, source.max / size, colour),
                );
            }
            // Nothing of the lines after one that is still coming up is drawn.
            if shown < line.time {
                break;
            }
        }
    }

    /// The character a text's character is written as (0x27800): ASCII letters and
    /// figures and some of its signs have full-width forms, which the fonts have.
    fn written(character: char) -> char {
        if character.is_ascii_alphanumeric() {
            return char::from_u32(character as u32 + TO_FULL_WIDTH).unwrap_or(character);
        }
        FULL_WIDTH.iter().find(|(narrow, _)| *narrow == character).map_or(character, |(_, wide)| *wide)
    }
}

/// The top left corner of a cell of the fonts' picture.
fn cell_corner(cell: usize) -> (usize, usize) {
    ((cell % ATLAS_ACROSS) * ATLAS_CELL, (cell / ATLAS_ACROSS) * ATLAS_CELL)
}

/// A text object while its text is put in.
struct Writer<'a> {
    fonts: &'a Fonts,
    style: &'a Style,
    lines: Vec<Line>,
    pen: f32,
    /// The frames the line's glyphs so far take to come up.
    time: f32,
}

impl Writer<'_> {
    /// The line being written, once there is room on it for one thing more
    /// (0x26F10): a full line is followed by a new one, and with no line left the
    /// thing is dropped.
    fn room(&mut self) -> Option<&mut Line> {
        if self.lines.last().is_some_and(|line| line.count >= self.style.per_line) && !self.new_line() {
            return None;
        }
        self.lines.last_mut()
    }

    /// Starts the next line under the last (0x26B00). There are as many lines as the
    /// style has, and one more.
    fn new_line(&mut self) -> bool {
        if self.lines.len() > self.style.lines {
            return false;
        }
        let top = self.lines.last().map_or(0.0, |line| line.top + self.style.size + self.style.line_gap);
        self.lines.push(Line { top, ..default() });
        self.pen = 0.0;
        self.time = 0.0;
        true
    }

    /// One more thing on the line: when it comes up, and how long the line then takes.
    fn put(&mut self, piece: Option<(Rect, Source, [u8; 4])>) {
        let (time, pen) = (self.time, self.pen);
        let Some(line) = self.lines.last_mut() else { return };
        line.count += 1;
        if let Some((area, source, colour)) = piece {
            line.pieces.push(Piece { area, source, colour, time });
        }
        line.width = line.width.max(pen);
    }

    fn timed(&mut self) {
        self.time += self.style.reveal;
        if let Some(line) = self.lines.last_mut() {
            line.time = self.time;
        }
    }

    /// A character (0x277A4).
    fn glyph(&mut self, character: char, colour: [u8; 4]) {
        let character = Fonts::written(character);
        if (character as u32) < FIRST_WIDE || self.room().is_none() {
            return;
        }
        let fonts = self.fonts;
        let (font, first) = &fonts.faces[self.style.face as usize];
        let size = self.style.size;
        let code = fonts.codes.shift_jis(character);
        let (mut at, mut advance) = (Vec2::new(self.pen, 0.0), size);
        let cell = match code.and_then(|code| font.metric(code)) {
            // A character with a metric of its own is drawn where that says, and the
            // pen goes on as far as it says.
            Some(metric) => {
                let scale = Vec2::new(size / font.width as f32, size / font.height as f32);
                at += Vec2::new(metric.x as f32 * scale.x, -(metric.y as f32) * scale.y);
                advance = metric.advance as f32 * scale.x;
                code.and_then(|code| font.glyph(code))
            }
            // One the font has no glyph for is drawn as an at sign.
            None => code.and_then(|code| font.glyph(code)).or_else(|| fonts.codes.shift_jis(NO_GLYPH).and_then(|code| font.glyph(code))),
        };
        let area = Rect::from_corners(at, at + Vec2::splat(size));
        self.pen += advance;
        self.put(cell.map(|cell| (area, Source::Glyph { cell: first + cell, across: [0.0, 1.0] }, colour)));
        self.timed();
        self.pen += self.style.letter_gap;
    }

    /// An icon (0x26640): as it is in the middle of the line's height, or to the
    /// text's size.
    fn icon(&mut self, number: i32) {
        let Some(icon) = self.fonts.icons.get(number).copied() else { return };
        if self.room().is_none() {
            return;
        }
        let size = self.style.size;
        let (mut width, mut height) = (icon.sprite.width as f32, icon.sprite.height as f32);
        let mut at = Vec2::new(self.pen, -(height - size) * 0.5);
        if icon.scaled && icon.size != 0 {
            let scale = size / icon.size as f32;
            at += Vec2::new(icon.x as f32, icon.y as f32) * scale;
            width *= scale;
            height *= scale;
        }
        let from = Vec2::new(icon.sprite.x as f32, icon.sprite.y as f32);
        let source = Rect::from_corners(from, from + Vec2::new(icon.sprite.width as f32, icon.sprite.height as f32));
        self.pen += width;
        // An icon keeps its own colours whatever the text's.
        self.put(Some((Rect::from_corners(at, at + Vec2::new(width, height)), Source::Icon(source), [255; 4])));
        self.timed();
        self.pen += self.style.letter_gap;
    }

    /// A dash of so many pieces (0x26FC8).
    fn dash(&mut self, pieces: i32, colour: [u8; 4]) {
        let fonts = self.fonts;
        let (font, first) = &fonts.faces[self.style.face as usize];
        let cell = fonts.codes.shift_jis(DASH).and_then(|code| font.glyph(code)).map(|cell| first + cell);
        for piece in 0..pieces {
            match piece {
                0 => self.dash_part(cell, DASH_END, [0.0, DASH_END], colour),
                _ => self.dash_part(cell, DASH_MIDDLE, [DASH_END, DASH_END], colour),
            }
            if piece == pieces - 1 {
                self.dash_part(cell, DASH_END, [DASH_END, DASH_MIDDLE], colour);
            }
            self.timed();
        }
        self.pen += self.style.letter_gap;
    }

    /// A part of the dash's glyph, drawn so much of a glyph's width wide (0x27AF8):
    /// to the nearest whole pixel.
    fn dash_part(&mut self, cell: Option<usize>, share: f32, across: [f32; 2], colour: [u8; 4]) {
        if self.room().is_none() {
            return;
        }
        let size = self.style.size;
        let width = (size * share + 0.5).floor();
        let at = Vec2::new(self.pen, 0.0);
        let area = Rect::from_corners(at, at + Vec2::new(width, size));
        self.pen += width;
        self.put(cell.map(|cell| (area, Source::Glyph { cell, across }, colour)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fonts, if the game's data is unpacked where the port looks for it.
    pub fn fonts() -> Option<(Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        Some((Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    const CAPTION: Style = Style::of_kind(0, 0x3c, 2, 1.0, 4.0);

    fn advances(block: &Block) -> Vec<f32> {
        block.lines[0].pieces.iter().map(|piece| piece.area.min.x).collect()
    }

    #[test]
    fn letters_are_as_wide_as_the_font_says_and_a_gap_apart() {
        let Some((fonts, colours)) = fonts() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // A capital is 7 wide, an i 3, an l 3, a space 4; one pixel between glyphs.
        let block = fonts.write_plain(&CAPTION, "Hill Ai", &colours);
        assert_eq!(advances(&block), [0.0, 8.0, 12.0, 16.0, 25.0, 33.0]);
        assert_eq!(block.size(), Vec2::new(36.0, 15.0));
        // Every glyph is the font's whole cell.
        assert!(block.lines[0].pieces.iter().all(|piece| piece.area.size() == Vec2::splat(15.0)));
        // A letter that hangs below the line is drawn a pixel lower.
        let block = fonts.write_plain(&CAPTION, "ag", &colours);
        let tops: Vec<f32> = block.lines[0].pieces.iter().map(|piece| piece.area.min.y).collect();
        assert_eq!(tops, [0.0, 1.0]);
        // The capitals are 15 wide at their full size and 5 at their smallest, the
        // gaps staying what they are told to be.
        let big = fonts.write_plain(&Style::of_kind(3, 0x30, 1, 0.0, 0.0), "AB1", &colours);
        assert_eq!(advances(&big), [0.0, 15.0, 30.0]);
        let small = fonts.write_plain(&Style::of_kind(1, 0x30, 1, 0.0, 0.0), "AB1", &colours);
        assert_eq!(advances(&small), [0.0, 5.0, 10.0]);
        assert_eq!(small.size(), Vec2::new(13.0, 5.0));
    }

    #[test]
    fn lines_follow_each_other_and_a_full_line_begins_the_next() {
        let Some((fonts, colours)) = fonts() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let block = fonts.write_plain(&CAPTION, "one\ntwo", &colours);
        let tops: Vec<f32> = block.lines.iter().map(|line| line.top).collect();
        assert_eq!(tops, [0.0, 19.0]);
        assert_eq!(block.size().y, 34.0);
        // Four to a line: the fifth letter starts the second line, and what there
        // is no line for is dropped.
        let narrow = Style { per_line: 4, lines: 1, ..CAPTION };
        let block = fonts.write_plain(&narrow, "abcdefghijkl", &colours);
        let counts: Vec<usize> = block.lines.iter().map(|line| line.pieces.len()).collect();
        assert_eq!(counts, [4, 4]);
        // A character the game writes with nothing draws nothing and moves nothing;
        // one the font lacks is an at sign, as wide as a cell.
        let block = fonts.write_plain(&CAPTION, "a@_b", &colours);
        assert_eq!(advances(&block), [0.0, 8.0]);
        let block = fonts.write_plain(&CAPTION, "a\u{1f600}b", &colours);
        assert_eq!(advances(&block), [0.0, 8.0, 24.0]);
    }

    #[test]
    fn an_icon_and_a_dash_have_their_own_widths() {
        let Some((fonts, colours)) = fonts() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The button's icon is 15 wide; start's is 23.
        let spans = text::pages("\x1501a\x152Cb", &text::Plain).remove(0);
        let block = fonts.write(&CAPTION, &spans, &colours);
        assert_eq!(advances(&block), [0.0, 16.0, 24.0, 48.0]);
        assert!(matches!(block.lines[0].pieces[0].source, Source::Icon(area) if area.min == Vec2::new(17.0, 1.0)));
        assert_eq!(block.lines[0].pieces[0].colour, [255; 4]);
        // A dash of two pieces: its start, its middle and its end, 16 in all.
        let spans = text::pages("To Mama\\z(2)!", &text::Plain).remove(0);
        let block = fonts.write(&CAPTION, &spans, &colours);
        let pieces = &block.lines[0].pieces;
        let dash: Vec<(f32, f32)> = pieces[6..9].iter().map(|piece| (piece.area.min.x, piece.area.width())).collect();
        let start = dash[0].0;
        assert_eq!(dash, [(start, 4.0), (start + 4.0, 8.0), (start + 12.0, 4.0)]);
        assert_eq!(pieces[9].area.min.x, start + 17.0);
    }

    #[test]
    fn a_text_comes_up_glyph_by_glyph_and_line_by_line() {
        let Some((fonts, colours)) = fonts() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let style = Style { reveal: 6.0, ..CAPTION };
        let block = fonts.write_plain(&style, "abc\nde", &colours);
        assert_eq!(block.time(), 30.0);
        let shown = |frames: f32| {
            let mut canvas = Canvas::default();
            fonts.draw(&mut canvas, 0, &block, &Placing { shown: frames, ..Placing::at(Vec2::ZERO) });
            canvas.vertices().len() / 6
        };
        assert_eq!([shown(0.0), shown(5.0), shown(6.0), shown(17.0)], [1, 1, 2, 3]);
        // The second line begins once the first is all there.
        assert_eq!([shown(18.0), shown(24.0), shown(1000.0)], [4, 5, 5]);
        // With a shadow everything is drawn twice.
        let mut canvas = Canvas::default();
        fonts.draw(&mut canvas, 0, &block, &Placing { shadow: true, ..Placing::at(Vec2::ZERO) });
        assert_eq!(canvas.vertices().len() / 6, 10);
    }

    #[test]
    fn lines_lie_left_centred_or_right_in_their_width() {
        let Some((fonts, colours)) = fonts() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let block = fonts.write_plain(&CAPTION, "Hill", &colours);
        assert_eq!(block.size().x, 19.0);
        let first = |align: Align| {
            let mut canvas = Canvas::default();
            fonts.draw(&mut canvas, 0, &block, &Placing { align, width: 480.0, ..Placing::at(Vec2::new(0.0, 229.0)) });
            canvas.vertices()[0].position
        };
        assert_eq!(first(Align::Left), Vec2::new(0.0, 229.0));
        // Half of what is spare, without its fraction.
        assert_eq!(first(Align::Centre), Vec2::new(230.0, 229.0));
        assert_eq!(first(Align::Right), Vec2::new(461.0, 229.0));
    }
}
