//! Files made of chunks: the sprite sheets of the interface (`.sbs`) are these.
//!
//! Read as the game walks them (0xD2F18). A chunk starts with eight bytes:
//!
//! | off | size | meaning                                                        |
//! |-----|------|----------------------------------------------------------------|
//! | 0   | 4    | tag: three letters and a zero (`SBS`, `PTM`, `DAT`), or `EOFC`  |
//! | 4   | 1    | flags: 0x80 on every chunk; 0x08 when it is aligned to 16 bytes |
//! | 5   | 3    | length of the contents, most significant byte first            |
//!
//! The contents follow the header, which is padded to the chunk's alignment (4 or
//! 16 bytes), and are padded to it themselves. A chunk tagged `EOFC` ends the file.

/// The tag of the chunk that ends a file.
pub const END: &[u8; 4] = b"EOFC";

const HEADER_LEN: usize = 8;
const FLAG_VALID: u8 = 0x80;
const FLAG_ALIGN_16: u8 = 0x08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub tag: [u8; 4],
    pub contents: &'a [u8],
}

/// The chunks of `d` in the file's order, up to the one that ends it or the first
/// that is not sound.
pub fn chunks(d: &[u8]) -> impl Iterator<Item = Chunk<'_>> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let header = d.get(at..at + HEADER_LEN)?;
        let tag: [u8; 4] = header[..4].try_into().unwrap();
        let flags = header[4];
        if flags & FLAG_VALID == 0 || &tag == END {
            return None;
        }
        let align = if flags & FLAG_ALIGN_16 != 0 { 16 } else { 4 };
        let len = u32::from_be_bytes([0, header[5], header[6], header[7]]) as usize;
        let start = at + HEADER_LEN.next_multiple_of(align);
        let contents = d.get(start..start + len)?;
        at = start + len.next_multiple_of(align);
        Some(Chunk { tag, contents })
    })
}

/// The contents of the first chunk with that tag (its three letters).
pub fn find<'a>(d: &'a [u8], tag: &[u8; 3]) -> Option<&'a [u8]> {
    chunks(d).find(|chunk| chunk.tag[..3] == tag[..] && chunk.tag[3] == 0).map(|chunk| chunk.contents)
}

#[cfg(test)]
pub(crate) fn chunk(tag: &[u8; 3], align_16: bool, contents: &[u8]) -> Vec<u8> {
    let align = if align_16 { 16 } else { 4 };
    let mut d = tag.to_vec();
    d.push(0);
    d.push(FLAG_VALID | if align_16 { FLAG_ALIGN_16 } else { 0 });
    d.extend(&(contents.len() as u32).to_be_bytes()[1..]);
    d.resize(HEADER_LEN.next_multiple_of(align), 0);
    d.extend(contents);
    d.resize(d.len().next_multiple_of(align), 0);
    d
}

#[cfg(test)]
pub(crate) fn end() -> Vec<u8> {
    let mut d = END.to_vec();
    d.extend([FLAG_VALID | FLAG_ALIGN_16, 0, 0, 0]);
    d.resize(16, 0);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_are_walked_by_their_alignment_up_to_the_end() {
        let mut d = chunk(b"SBS", true, &[1, 2, 3]);
        d.extend(chunk(b"DAT", false, &[4, 5, 6, 7, 8]));
        d.extend(end());
        d.extend(chunk(b"PTM", false, &[9]));
        let found: Vec<Chunk> = chunks(&d).collect();
        assert_eq!(found.len(), 2);
        assert_eq!((&found[0].tag, found[0].contents), (b"SBS\0", &[1u8, 2, 3][..]));
        assert_eq!((&found[1].tag, found[1].contents), (b"DAT\0", &[4u8, 5, 6, 7, 8][..]));
        assert_eq!(find(&d, b"DAT"), Some(&[4u8, 5, 6, 7, 8][..]));
        // What comes after the end is not looked at.
        assert_eq!(find(&d, b"PTM"), None);
        // A chunk that runs past the file ends the walk.
        assert_eq!(chunks(&d[..18]).count(), 0);
    }
}
