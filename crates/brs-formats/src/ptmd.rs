//! `PTMD` textures.
//!
//! 16-byte header followed by pixel data and, for indexed formats, a palette:
//!
//! | off | size | meaning                                                   |
//! |-----|------|-----------------------------------------------------------|
//! | 0   | 4    | magic `PTMD`                                              |
//! | 4   | 1    | always 2 (version?)                                       |
//! | 5   | 1    | flags; bit 0 set on every indexed texture                 |
//! | 6   | 2    | low 12 bits = height, high 4 bits = log2(width)           |
//! | 8   | 1    | pixel format, PSP GE numbering (see [`Format`])           |
//! | 9   | 1    | high nibble = palette colour format, same numbering       |
//! | 10  | 2    | zero                                                      |
//! | 12  | 4    | header size + pixel data size (palette not included)      |
//!
//! Pixel data is stored swizzled, as the PSP GE expects. The palette has 16 or
//! 256 entries of 2 or 4 bytes depending on its colour format.

use crate::u32_at;

pub const MAGIC: &[u8; 4] = b"PTMD";
const HEADER_LEN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Rgb565,
    Rgba5551,
    Rgba4444,
    Rgba8888,
    Indexed4,
    Indexed8,
}

impl Format {
    fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Self::Rgb565,
            1 => Self::Rgba5551,
            2 => Self::Rgba4444,
            3 => Self::Rgba8888,
            4 => Self::Indexed4,
            5 => Self::Indexed8,
            _ => return None,
        })
    }

    pub fn is_indexed(self) -> bool {
        matches!(self, Self::Indexed4 | Self::Indexed8)
    }

    fn bits_per_pixel(self) -> usize {
        match self {
            Self::Indexed4 => 4,
            Self::Indexed8 => 8,
            Self::Rgba8888 => 32,
            _ => 16,
        }
    }

    fn palette_entries(self) -> usize {
        match self {
            Self::Indexed4 => 16,
            Self::Indexed8 => 256,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub width: usize,
    pub height: usize,
    pub format: Format,
    /// Colour format of the palette entries; only meaningful for indexed textures.
    pub palette_format: Format,
    pub flags: u8,
    /// Bytes of pixel data following the header.
    pub data_len: usize,
}

impl Header {
    fn palette_len(&self) -> usize {
        self.format.palette_entries() * self.palette_format.bits_per_pixel() / 8
    }

    /// Total length of the texture in its containing blob, palette included.
    pub fn total_len(&self) -> usize {
        HEADER_LEN + self.data_len + self.palette_len()
    }

    fn row_bytes(&self) -> usize {
        self.width * self.format.bits_per_pixel() / 8
    }
}

/// Parses and validates a header at the start of `d`.
///
/// Validation is strict (sizes must agree with the dimensions) so this is safe to
/// use when scanning opaque blobs for embedded textures.
pub fn parse_header(d: &[u8]) -> Option<Header> {
    if d.len() < HEADER_LEN || &d[..4] != MAGIC || d[4] != 2 {
        return None;
    }
    let dims = u16::from_le_bytes([d[6], d[7]]);
    let width = 1usize << (dims >> 12);
    let height = (dims & 0x0fff) as usize;
    let format = Format::from_byte(d[8])?;
    let palette_format = Format::from_byte(d[9] >> 4).filter(|f| !f.is_indexed())?;
    let data_len = (u32_at(d, 12)? as usize).checked_sub(HEADER_LEN)?;

    let expected = width * height * format.bits_per_pixel() / 8;
    // Tiny textures are padded up to a 16-byte row.
    if height == 0 || width > 1024 || (data_len != expected && data_len != expected.next_multiple_of(16)) {
        return None;
    }
    let header = Header { width, height, format, palette_format, flags: d[5], data_len };
    (header.total_len() <= d.len()).then_some(header)
}

/// Finds every texture embedded in `d`, returning `(offset, header)` pairs.
pub fn scan(d: &[u8]) -> Vec<(usize, Header)> {
    let mut found = Vec::new();
    let mut pos = 0;
    while pos + HEADER_LEN <= d.len() {
        let Some(rel) = d[pos..].windows(4).position(|w| w == MAGIC) else { break };
        pos += rel;
        match parse_header(&d[pos..]) {
            Some(h) => {
                found.push((pos, h));
                pos += h.total_len();
            }
            None => pos += 4,
        }
    }
    found
}

/// Decodes the texture at the start of `d` (as accepted by [`parse_header`]) to RGBA8.
pub fn decode(d: &[u8], h: &Header) -> Vec<u8> {
    let stored = &d[HEADER_LEN..HEADER_LEN + h.data_len];
    let pixels = unswizzle(stored, h.row_bytes(), h.height);
    let count = h.width * h.height;

    if !h.format.is_indexed() {
        return colours(&pixels, h.format, count).into_iter().flatten().collect();
    }

    let palette = colours(&d[HEADER_LEN + h.data_len..h.total_len()], h.palette_format, h.format.palette_entries());
    let mut out = Vec::with_capacity(count * 4);
    for p in 0..count {
        let index = match h.format {
            Format::Indexed8 => pixels[p],
            _ => (pixels[p / 2] >> (4 * (p & 1))) & 0x0f,
        };
        out.extend_from_slice(&palette[index as usize]);
    }
    out
}

/// Converts `count` direct-colour values (pixels or palette entries) to RGBA8.
fn colours(src: &[u8], format: Format, count: usize) -> Vec<[u8; 4]> {
    if format == Format::Rgba8888 {
        return src.chunks_exact(4).take(count).map(|c| [c[0], c[1], c[2], c[3]]).collect();
    }
    src.chunks_exact(2)
        .take(count)
        .map(|c| {
            let v = u16::from_le_bytes([c[0], c[1]]);
            match format {
                Format::Rgb565 => [expand(v, 0, 5), expand(v, 5, 6), expand(v, 11, 5), 255],
                Format::Rgba5551 => [expand(v, 0, 5), expand(v, 5, 5), expand(v, 10, 5), expand(v, 15, 1)],
                _ => [expand(v, 0, 4), expand(v, 4, 4), expand(v, 8, 4), expand(v, 12, 4)],
            }
        })
        .collect()
}

/// Extracts a `bits`-wide channel at `shift` and scales it to 0..=255.
fn expand(v: u16, shift: u32, bits: u32) -> u8 {
    let max = (1u32 << bits) - 1;
    ((v as u32 >> shift & max) * 255 / max) as u8
}

/// Reverses PSP GE swizzling: data is stored as 16-byte x 8-row blocks.
///
/// Textures narrower than one block or shorter than 8 rows are assumed linear.
fn unswizzle(src: &[u8], row_bytes: usize, height: usize) -> Vec<u8> {
    if row_bytes < 16 || height < 8 {
        return src.to_vec();
    }
    let mut out = vec![0u8; row_bytes * height];
    let mut blocks = src.chunks_exact(16);
    for by in (0..height).step_by(8) {
        for bx in (0..row_bytes).step_by(16) {
            for y in by..by + 8 {
                let Some(chunk) = blocks.next() else { return out };
                if y < height {
                    out[y * row_bytes + bx..][..16].copy_from_slice(chunk);
                }
            }
        }
    }
    out
}
