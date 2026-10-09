//! The game's fonts and what goes with them (`SYSTEM.VOL/font.lpk`), read as the
//! font library does (0x1D124 on).
//!
//! A font (`000.bin` the text's, `001.bin` the one of big capitals and figures) is a
//! header, a palette, the glyphs, and three tables:
//!
//! | off  | size | meaning                                                       |
//! |------|------|---------------------------------------------------------------|
//! | 0x00 | 4    | offset of the code table: header, palette and glyphs before it |
//! | 0x04 | 4    | bytes of palette                                              |
//! | 0x08 | 4    | bytes of glyphs                                               |
//! | 0x0C | 4    | bytes of the code table                                       |
//! | 0x10 | 4    | bytes of the metric table                                     |
//! | 0x14 | 4    | flags: bit 31 there is a palette; low two bits how glyphs are found (3) |
//! | 0x18 | 2    | number of glyphs                                              |
//! | 0x1A | 1, 1 | width and height of a glyph, in pixels (15 by 15)             |
//! | 0x1C | 1    | bits a pixel (4)                                              |
//!
//! The palette (from 0x20) is four bytes a colour: every one white, with the alpha
//! that makes the glyph. A glyph is its pixels row by row with no padding between
//! rows, the first pixel of a byte in its high half, in a whole number of bytes (113
//! for 15 by 15). The code table has the glyphs' characters in rising order, two bytes
//! each, as Shift-JIS; the metric table has eight bytes for each character that is
//! not as wide as the font (three words: how far right and how far up it is drawn,
//! and how far the next character is), and after it come those characters' codes in
//! rising order.
//!
//! `002.sbs` is a sprite sheet ([`crate::chunks`]) of the icons a text can have in it
//! (the buttons, mostly); `ucs2jis.bin` beside the pack turns a character into the
//! code the tables know it by.

use anyhow::{ensure, Context, Result};

use crate::chunks;
use crate::reader::Reader;
use crate::sbs::{self, Sprite};

const HEADER_LEN: usize = 0x20;
const FLAG_PALETTE: u32 = 0x8000_0000;
const FIND_MASK: u32 = 3;
/// The ways of finding a glyph that go by the code table (0x1D488).
const FIND_BY_CODE: std::ops::Range<u32> = 2..4;
const METRIC_LEN: usize = 8;

/// Where a character that is not as wide as the font is drawn and how far the next
/// one is, in the font's pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metric {
    /// How far right of the pen it is drawn.
    pub x: i16,
    /// How far above its line it is drawn: negative for one that hangs below.
    pub y: i16,
    pub advance: i16,
}

#[derive(Clone, Debug)]
pub struct Font {
    pub width: usize,
    pub height: usize,
    /// Red, green, blue and alpha of each of a pixel's values.
    palette: Vec<[u8; 4]>,
    glyph_len: usize,
    pixels: Vec<u8>,
    /// The characters there are glyphs for, in rising order: a glyph's place in the
    /// file is its character's place here.
    codes: Vec<u16>,
    metrics: Vec<(u16, Metric)>,
}

impl Font {
    pub fn parse(d: &[u8]) -> Result<Font> {
        ensure!(d.len() >= HEADER_LEN, "file ends inside its header");
        let r = Reader(d);
        let (codes_at, palette_len, pixels_len) = (r.u32(0x00)? as usize, r.u32(0x04)? as usize, r.u32(0x08)? as usize);
        let (codes_len, metrics_len) = (r.u32(0x0c)? as usize, r.u32(0x10)? as usize);
        let flags = r.u32(0x14)?;
        let glyphs = r.u16(0x18)? as usize;
        let (width, height, bits) = (r.u8(0x1a)? as usize, r.u8(0x1b)? as usize, r.u8(0x1c)? as usize);

        ensure!(flags & FLAG_PALETTE != 0, "a font with no palette of its own");
        ensure!(FIND_BY_CODE.contains(&(flags & FIND_MASK)), "glyphs are not found by their codes (flags {flags:#x})");
        ensure!(bits == 4, "{bits} bits a pixel");
        ensure!(width > 0 && height > 0, "glyphs of no size");
        ensure!(palette_len == (1 << bits) * 4, "a palette of {palette_len} bytes");
        ensure!(codes_at == HEADER_LEN + palette_len + pixels_len, "the tables are not where the header's lengths put them");
        let glyph_len = (width * height * bits).div_ceil(8);
        ensure!(pixels_len == glyphs * glyph_len, "{glyphs} glyphs of {glyph_len} bytes are not {pixels_len} bytes");
        ensure!(codes_len == glyphs * 2, "{glyphs} glyphs but {codes_len} bytes of codes");
        ensure!(metrics_len % METRIC_LEN == 0, "a metric table of {metrics_len} bytes");
        let metric_count = metrics_len / METRIC_LEN;
        let metrics_at = codes_at + codes_len;
        let metric_codes_at = metrics_at + metrics_len;
        ensure!(metric_codes_at + metric_count * 2 <= d.len(), "file ends inside its tables");

        let palette = d[HEADER_LEN..HEADER_LEN + palette_len].chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        let codes = (0..glyphs).map(|index| r.u16(codes_at + index * 2)).collect::<Result<Vec<u16>>>()?;
        ensure!(codes.windows(2).all(|pair| pair[0] < pair[1]), "the codes are not in rising order");
        let metrics = (0..metric_count)
            .map(|index| {
                let at = metrics_at + index * METRIC_LEN;
                let metric = Metric { x: r.i16(at)?, y: r.i16(at + 2)?, advance: r.i16(at + 4)? };
                Ok((r.u16(metric_codes_at + index * 2)?, metric))
            })
            .collect::<Result<Vec<(u16, Metric)>>>()?;
        ensure!(metrics.windows(2).all(|pair| pair[0].0 < pair[1].0), "the metrics' codes are not in rising order");

        Ok(Font { width, height, palette, glyph_len, pixels: d[HEADER_LEN + palette_len..codes_at].to_vec(), codes, metrics })
    }

    /// Number of glyphs.
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    /// The characters there are glyphs for, as Shift-JIS, in the glyphs' order.
    pub fn codes(&self) -> &[u16] {
        &self.codes
    }

    /// Which glyph a character has (0x1D41C).
    pub fn glyph(&self, code: u16) -> Option<usize> {
        self.codes.binary_search(&code).ok()
    }

    /// The metric of a character that has one (0x1D308): one that has none is drawn
    /// at the pen and is as wide as the font.
    pub fn metric(&self, code: u16) -> Option<Metric> {
        self.metrics.binary_search_by_key(&code, |(code, _)| *code).ok().map(|index| self.metrics[index].1)
    }

    /// The colours of a glyph's pixels, row by row.
    pub fn pixels(&self, glyph: usize) -> impl Iterator<Item = [u8; 4]> + '_ {
        let data = &self.pixels[glyph * self.glyph_len..(glyph + 1) * self.glyph_len];
        (0..self.width * self.height).map(move |pixel| {
            let byte = data[pixel / 2];
            let value = if pixel % 2 == 0 { byte >> 4 } else { byte & 0x0f };
            self.palette[value as usize]
        })
    }
}

/// The table that turns a character into the code the fonts know it by
/// (`SYSTEM.VOL/ucs2jis.bin`): two bytes for each of the 65,536 characters it can,
/// the character's JIS code, or zero for none. The fonts' tables are in Shift-JIS,
/// which a JIS code of two bytes is turned into by rule.
#[derive(Clone)]
pub struct Codes(Vec<u16>);

impl Codes {
    const CHARACTERS: usize = 0x1_0000;

    pub fn parse(d: &[u8]) -> Result<Codes> {
        ensure!(d.len() == Self::CHARACTERS * 2, "{} bytes is not a code for each of {} characters", d.len(), Self::CHARACTERS);
        Ok(Codes(d.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect()))
    }

    /// The Shift-JIS code of a character: of one byte or of two.
    pub fn shift_jis(&self, character: char) -> Option<u16> {
        let jis = *self.0.get(character as usize)?;
        match jis {
            0 => None,
            1..=0xff => Some(jis),
            _ => {
                let (row, cell) = ((jis >> 8) as u8, jis as u8);
                let mut first = ((row - 0x21) >> 1) + 0x81;
                if first > 0x9f {
                    first += 0x40;
                }
                let second = match row % 2 {
                    1 => cell + 0x1f + u8::from(cell >= 0x60),
                    _ => cell + 0x7e,
                };
                Some(u16::from_be_bytes([first, second]))
            }
        }
    }
}

/// An icon a text can have in it (a record of the sheet's `DAT` chunk, 0x36A8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icon {
    /// The number texts know it by.
    pub number: i16,
    /// Where it is in the sheet's texture.
    pub sprite: Sprite,
    /// How far right and down of the pen it is drawn.
    pub x: i16,
    pub y: i16,
    /// It is drawn to the text's size, `size` being the size it is drawn as it is
    /// at; one that is not is drawn as it is, in the middle of its line's height.
    pub scaled: bool,
    pub size: i16,
}

/// The sheet of icons (`002.sbs`).
#[derive(Clone, Debug)]
pub struct Icons {
    icons: Vec<Icon>,
    /// The sheet's texture, a `PTMD`.
    pub texture: Vec<u8>,
}

impl Icons {
    const ICON_LEN: usize = 12;

    pub fn parse(d: &[u8]) -> Result<Icons> {
        let sprites = sbs::parse(chunks::find(d, b"SBS").context("the sheet has no sprite table")?)?;
        let texture = chunks::find(d, b"PTM").context("the sheet has no texture")?.to_vec();
        let list = chunks::find(d, b"DAT").context("the sheet has no list of icons")?;
        let r = Reader(list);
        let count = r.u32(0)? as usize;
        ensure!(4 + count * Self::ICON_LEN <= list.len(), "the list ends inside its {count} icons");
        let icons = (0..count)
            .map(|index| {
                let at = 4 + index * Self::ICON_LEN;
                let sprite = r.i16(at + 2)?;
                let sprite = *usize::try_from(sprite).ok().and_then(|sprite| sprites.get(sprite)).with_context(|| format!("icon {index} is sprite {sprite}, which the sheet has not"))?;
                Ok(Icon { number: r.i16(at)?, sprite, x: r.i16(at + 4)?, y: r.i16(at + 6)?, scaled: r.i16(at + 8)? != 0, size: r.i16(at + 10)? })
            })
            .collect::<Result<Vec<Icon>>>()?;
        Ok(Icons { icons, texture })
    }

    /// The icon texts know by that number: the first of the list that has it.
    pub fn get(&self, number: i32) -> Option<&Icon> {
        self.icons.iter().find(|icon| icon.number as i32 == number)
    }

    pub fn len(&self) -> usize {
        self.icons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks::{chunk, end};

    /// A font of two glyphs, two pixels by two: the first all of the palette's second
    /// colour, the second its four first.
    fn file() -> Vec<u8> {
        let glyphs: [&[u8]; 2] = [&[0x11, 0x11], &[0x01, 0x23]];
        let codes = [0x8260u16, 0x8281];
        let mut d = Vec::new();
        d.extend((0x20u32 + 0x40 + 4).to_le_bytes());
        d.extend(0x40u32.to_le_bytes());
        d.extend(4u32.to_le_bytes());
        d.extend(4u32.to_le_bytes());
        d.extend(8u32.to_le_bytes());
        d.extend(0x8000_0003u32.to_le_bytes());
        d.extend(2u16.to_le_bytes());
        d.extend([2, 2, 4, 0, 0, 0]);
        for value in 0..16u8 {
            d.extend([255, 255, 255, value * 17]);
        }
        d.extend(glyphs.concat());
        d.extend(codes.iter().flat_map(|code| code.to_le_bytes()));
        // The second character is drawn one to the right, one below, and is one wide.
        d.extend([1i16, -1, 1, 0].iter().flat_map(|value| value.to_le_bytes()));
        d.extend(0x8281u16.to_le_bytes());
        d
    }

    #[test]
    fn glyphs_are_found_by_their_codes_and_read_high_half_first() {
        let font = Font::parse(&file()).unwrap();
        assert_eq!((font.len(), font.width, font.height), (2, 2, 2));
        assert_eq!(font.glyph(0x8260), Some(0));
        assert_eq!(font.glyph(0x8281), Some(1));
        assert_eq!(font.glyph(0x8261), None);
        let alpha = |glyph| font.pixels(glyph).map(|colour| colour[3]).collect::<Vec<u8>>();
        assert_eq!(alpha(0), [17, 17, 17, 17]);
        assert_eq!(alpha(1), [0, 17, 34, 51]);
        assert_eq!(font.metric(0x8281), Some(Metric { x: 1, y: -1, advance: 1 }));
        assert_eq!(font.metric(0x8260), None);
    }

    #[test]
    fn a_font_whose_lengths_disagree_is_refused() {
        let mut short = file();
        short.pop();
        assert!(Font::parse(&short).is_err());
        let mut unsorted = file();
        unsorted[0x64..0x68].copy_from_slice(&[0x81, 0x82, 0x60, 0x82]);
        assert!(Font::parse(&unsorted).is_err());
        let mut bits = file();
        bits[0x1c] = 8;
        assert!(Font::parse(&bits).is_err());
    }

    #[test]
    fn a_character_is_known_by_its_shift_jis_code() {
        let mut table = vec![0u8; Codes::CHARACTERS * 2];
        let mut set = |character: char, jis: u16| table[character as usize * 2..][..2].copy_from_slice(&jis.to_le_bytes());
        set('A', 0x41);
        set('\u{3042}', 0x2422);
        set('\u{3000}', 0x2121);
        set('\u{ff21}', 0x2341);
        set('\u{4e9c}', 0x3021);
        set('\u{30df}', 0x255f);
        set('\u{30e0}', 0x2560);
        set('\u{7199}', 0x7426);
        let codes = Codes::parse(&table).unwrap();
        assert_eq!(codes.shift_jis('A'), Some(0x41));
        assert_eq!(codes.shift_jis('\u{3042}'), Some(0x82a0));
        assert_eq!(codes.shift_jis('\u{3000}'), Some(0x8140));
        assert_eq!(codes.shift_jis('\u{ff21}'), Some(0x8260));
        assert_eq!(codes.shift_jis('\u{4e9c}'), Some(0x889f));
        // The second byte steps over 0x7F.
        assert_eq!(codes.shift_jis('\u{30df}'), Some(0x837e));
        assert_eq!(codes.shift_jis('\u{30e0}'), Some(0x8380));
        // The first byte steps over the half-width range.
        assert_eq!(codes.shift_jis('\u{7199}'), Some(0xeaa4));
        assert_eq!(codes.shift_jis('B'), None);
        assert_eq!(codes.shift_jis('\u{1f600}'), None);
        assert!(Codes::parse(&table[1..]).is_err());
    }

    #[test]
    fn an_icon_is_a_sprite_of_the_sheet_by_number() {
        let sprites = [0u8, 0, 2, 1, 1, 15, 15, 17, 129, 23, 15];
        let mut list = 2u32.to_le_bytes().to_vec();
        list.extend([7i16, 1, 0, 0, 0, 15].iter().flat_map(|value| value.to_le_bytes()));
        list.extend([9i16, 0, 2, -1, 1, 15].iter().flat_map(|value| value.to_le_bytes()));
        let mut d = chunk(b"SBS", true, &sprites);
        d.extend(chunk(b"PTM", true, b"PTMD"));
        d.extend(chunk(b"DAT", true, &list));
        d.extend(end());
        let icons = Icons::parse(&d).unwrap();
        assert_eq!(icons.len(), 2);
        assert_eq!(icons.texture, b"PTMD");
        let wide = icons.get(7).unwrap();
        assert_eq!((wide.sprite, wide.scaled), (Sprite { x: 17, y: 129, width: 23, height: 15 }, false));
        let other = icons.get(9).unwrap();
        assert_eq!((other.sprite.x, other.x, other.y, other.scaled, other.size), (1, 2, -1, true, 15));
        assert!(icons.get(8).is_none());

        // An icon of a sprite the sheet has not.
        let mut bad = 1u32.to_le_bytes().to_vec();
        bad.extend([0i16, 5, 0, 0, 0, 15].iter().flat_map(|value| value.to_le_bytes()));
        let mut d = chunk(b"SBS", true, &sprites);
        d.extend(chunk(b"PTM", true, b"PTMD"));
        d.extend(chunk(b"DAT", true, &bad));
        d.extend(end());
        assert!(Icons::parse(&d).is_err());
    }
}
