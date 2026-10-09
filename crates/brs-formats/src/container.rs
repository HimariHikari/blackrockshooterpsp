//! Compression wrappers (gzip, `CZAA`) and archives (`RTDP`, `LPK`).

use std::io::Read;

use anyhow::{bail, ensure, Context, Result};
use flate2::read::{GzDecoder, ZlibDecoder};

use crate::u32_at;

/// Removes one compression layer, or returns `None` if `data` is not wrapped.
///
/// * gzip: used for most of `FLD/`, `IF/`, `GALLERY/` and `MINI_GAME/`.
/// * `CZAA`: 16-byte header (magic, uncompressed size, 8 zero bytes) + zlib.
pub fn unwrap(data: &[u8]) -> Result<Option<Vec<u8>>> {
    if data.starts_with(&[0x1f, 0x8b, 0x08]) {
        let mut out = Vec::new();
        GzDecoder::new(data).read_to_end(&mut out).context("gzip")?;
        return Ok(Some(out));
    }
    if data.starts_with(b"CZAA") && data.len() > 16 {
        let size = u32_at(data, 4).unwrap() as usize;
        let mut out = Vec::with_capacity(size);
        ZlibDecoder::new(&data[16..]).read_to_end(&mut out).context("CZAA zlib")?;
        ensure!(out.len() == size, "CZAA size mismatch: header {size}, got {}", out.len());
        return Ok(Some(out));
    }
    Ok(None)
}

pub struct RtdpEntry {
    pub name: String,
    pub data: Vec<u8>,
}

const RTDP_KEY: u8 = 0x55;

/// `RTDP` archive (`.VOL`).
///
/// Header: magic, data start, entry count, total file size, unknown u32, padding to 0x20.
/// Entries are 40 bytes: 32-byte NUL-padded name, size, offset relative to data start.
/// Member data is XORed with 0x55; the header and names are not.
pub fn parse_rtdp(d: &[u8]) -> Result<Vec<RtdpEntry>> {
    ensure!(d.starts_with(b"RTDP"), "not an RTDP archive");
    let data_start = u32_at(d, 4).context("truncated RTDP header")? as usize;
    let count = u32_at(d, 8).context("truncated RTDP header")? as usize;
    ensure!(0x20 + count * 40 <= data_start && data_start <= d.len(), "bad RTDP header");

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let e = &d[0x20 + i * 40..0x20 + (i + 1) * 40];
        let name_len = e[..32].iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&e[..name_len]).into_owned();
        let size = u32_at(e, 32).unwrap() as usize;
        let start = data_start + u32_at(e, 36).unwrap() as usize;
        let Some(raw) = d.get(start..start + size) else {
            bail!("RTDP entry {name:?} runs past end of archive");
        };
        entries.push(RtdpEntry { name, data: raw.iter().map(|b| b ^ RTDP_KEY).collect() });
    }
    Ok(entries)
}

/// Slots of an `SC` bundle start on multiples of this.
const SC_ALIGNMENT: usize = 16;

/// `SC` bundle: a fixed number of slots, some of them empty. Used throughout `FLD/`,
/// and slots often hold further `SC` bundles.
///
/// Header: magic `SC`, u16 slot count, then a word for each slot. Slots are aligned to
/// 16 bytes, and a word's low four bits are not part of the offset: they count the
/// padding bytes at the end of its slot. Slot `i` starts at `word[i]` rounded down to
/// 16 and ends that many bytes short of where the next one starts (the original works
/// a slot's size out the same way, at 0xE7064).
///
/// Nearly every bundle has one more word after those, the size of the bundle. Two in
/// `fld_resident.bin` do not (one has no room for it before its first slot, the other
/// has a zero there); their last slot runs to the end of the data.
///
/// The two-letter magic is weak, so this returns `None` unless the whole table is
/// consistent with the data's length.
pub fn parse_sc(d: &[u8]) -> Option<Vec<&[u8]>> {
    if !d.starts_with(b"SC") {
        return None;
    }
    let count = u16::from_le_bytes([*d.get(2)?, *d.get(3)?]) as usize;
    let mut words: Vec<usize> = (0..count).map(|i| u32_at(d, 4 + i * 4).map(|o| o as usize)).collect::<Option<_>>()?;
    let after = u32_at(d, 4 + count * 4)? as usize;
    let sized = after == d.len();
    words.push(d.len());
    let starts: Vec<usize> = words.iter().map(|word| word - word % SC_ALIGNMENT).collect();
    // Without its size to go by, a bundle is only taken for one if its first slot
    // begins straight after the table, with nothing but zeros between them.
    let table_end = 4 + (count + sized as usize) * 4;
    let begins = if sized {
        starts[0] >= table_end
    } else {
        starts[0] == table_end.next_multiple_of(SC_ALIGNMENT) && (starts[0] == table_end || after == 0)
    };
    let consistent = count > 0 && begins && starts.windows(2).all(|pair| pair[0] <= pair[1]);
    if !consistent {
        return None;
    }
    (0..count)
        .map(|i| {
            let end = starts[i + 1].checked_sub(words[i] % SC_ALIGNMENT)?;
            d.get(starts[i]..end)
        })
        .collect()
}

/// One slot of an `SC` bundle, found by its own word and the next one alone, as the
/// original finds it (0x102554): `None` when the slot is empty. Unlike [`parse_sc`]
/// this does not need the rest of the table to make sense, which matters for the
/// bundles that leave the words of the slots they do not use, and the size after
/// them, at zero (an event's camera cuts): such a slot runs to the end of the data.
pub fn sc_slot(d: &[u8], slot: usize) -> Option<&[u8]> {
    if !d.starts_with(b"SC") {
        return None;
    }
    let word = u32_at(d, 4 + slot * 4)? as usize;
    let next = u32_at(d, 4 + (slot + 1) * 4)? as usize;
    let (start, padding) = (word - word % SC_ALIGNMENT, word % SC_ALIGNMENT);
    let after = next - next % SC_ALIGNMENT;
    if after == start + padding {
        return None;
    }
    let end = if after > start { after - padding } else { d.len() };
    d.get(start..end).filter(|data| !data.is_empty())
}

/// `LPK` pack: an unnamed list of blobs.
///
/// Header: magic `LPK\0`, entry count, unknown u32 (0 in every file seen),
/// then `count` offsets followed by `count` sizes.
pub fn parse_lpk(d: &[u8]) -> Result<Vec<&[u8]>> {
    ensure!(d.starts_with(b"LPK\0"), "not an LPK pack");
    let count = u32_at(d, 4).context("truncated LPK header")? as usize;
    ensure!(12 + count * 8 <= d.len(), "bad LPK entry count {count}");

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let off = u32_at(d, 12 + i * 4).unwrap() as usize;
        let size = u32_at(d, 12 + (count + i) * 4).unwrap() as usize;
        entries.push(d.get(off..off + size).with_context(|| format!("LPK entry {i} out of range"))?);
    }
    Ok(entries)
}

/// `PDK` pack: a fixed number of numbered members, some of them empty. The files of
/// the ride on the bike are these (`MINI_GAME/BMG_*.BIN`, read by 0x147D1C).
///
/// Header: magic `PDK\x01`, u16 member count, u16 where the members begin, then
/// for each member where it is, counted from there, and how long: an empty one has
/// nought for both.
pub fn parse_pdk(d: &[u8]) -> Result<Vec<&[u8]>> {
    ensure!(d.starts_with(PDK_MAGIC), "not a PDK pack");
    let short = |at: usize| d.get(at..at + 2).map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as usize);
    let (count, begin) = (short(4).context("truncated PDK header")?, short(6).context("truncated PDK header")?);
    ensure!(8 + count * 8 <= begin && begin <= d.len(), "bad PDK header: {count} members from {begin:#x}");
    let mut members = Vec::with_capacity(count);
    for i in 0..count {
        let off = u32_at(d, 8 + i * 8).unwrap() as usize;
        let size = u32_at(d, 12 + i * 8).unwrap() as usize;
        members.push(d.get(begin + off..begin + off + size).with_context(|| format!("PDK member {i} out of range"))?);
    }
    Ok(members)
}

pub const PDK_MAGIC: &[u8; 4] = b"PDK\x01";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pdk_pack_has_numbered_members_some_of_them_empty() {
        // Three members from 0x20 on: five bytes, none, and three more after
        // the padding that brings the second to a multiple of eight.
        let mut d = b"PDK\x01".to_vec();
        d.extend(3u16.to_le_bytes());
        d.extend(0x20u16.to_le_bytes());
        for (off, size) in [(0u32, 5u32), (0, 0), (8, 3)] {
            d.extend(off.to_le_bytes());
            d.extend(size.to_le_bytes());
        }
        d.extend(b"hello\0\0\0abc");
        assert_eq!(parse_pdk(&d).unwrap(), [&b"hello"[..], &b""[..], &b"abc"[..]]);
        // One cut short is refused, and so is anything that is not one.
        assert!(parse_pdk(&d[..d.len() - 1]).is_err() && parse_pdk(b"LPK\0").is_err());
    }

    /// A bundle of the given slots, laid out as the game's are: each on a multiple of
    /// 16, with the padding after it counted in the low bits of its word.
    fn bundle(slots: &[&[u8]]) -> Vec<u8> {
        let table_end = 4 + (slots.len() + 1) * 4;
        let mut body = Vec::new();
        let mut words = Vec::new();
        let first = table_end.next_multiple_of(SC_ALIGNMENT);
        for slot in slots {
            let padding = (SC_ALIGNMENT - slot.len() % SC_ALIGNMENT) % SC_ALIGNMENT;
            words.push((first + body.len() + padding) as u32);
            body.extend_from_slice(slot);
            body.extend(std::iter::repeat_n(0, padding));
        }
        words.push((first + body.len()) as u32);
        let mut d = b"SC".to_vec();
        d.extend((slots.len() as u16).to_le_bytes());
        for word in words {
            d.extend(word.to_le_bytes());
        }
        d.resize(first, 0);
        d.extend(body);
        d
    }

    #[test]
    fn a_slot_ends_where_its_padding_begins() {
        let slots: [&[u8]; 4] = [b"SSCR and twenty bytes", b"", b"exactly sixteen!", b"bscr"];
        let d = bundle(&slots);
        // The first slot is 21 bytes: its word carries 11 bytes of padding, and it
        // starts on the multiple of 16 below that word.
        assert_eq!(u32_at(&d, 4), Some(0x20 + 11));
        assert_eq!(parse_sc(&d).unwrap(), slots);
    }

    #[test]
    fn data_that_only_starts_like_a_bundle_is_not_one() {
        // `SCMP` files begin with the same two letters.
        assert!(parse_sc(b"SCMP\0\0\0\0\x30\0\0\0\x30\x29\x05\0").is_none());
        // A table whose last word is not the size of the data.
        let mut d = bundle(&[b"one", b"two"]);
        d.push(0);
        assert!(parse_sc(&d).is_none());
        // More padding than the slot is long.
        let mut d = bundle(&[b"one", b""]);
        d[8] |= 0xf;
        assert!(parse_sc(&d).is_none());
    }

    #[test]
    fn a_bundle_without_its_size_runs_to_the_end_of_the_data() {
        // The two such bundles in the game: one slot with zeros where the size would
        // be, and three slots with no room for it before the first.
        let mut one = b"SC\x01\0\x10\0\0\0\0\0\0\0\0\0\0\0".to_vec();
        one.extend(b"PTMD and the rest of a texture..");
        assert_eq!(parse_sc(&one).unwrap(), [&one[16..]]);

        let mut three = b"SC\x03\0\x10\0\0\0\x20\0\0\0\x30\0\0\0".to_vec();
        three.extend(b"first script....second script...third script....");
        let slots = parse_sc(&three).unwrap();
        assert_eq!(slots, [&three[16..32], &three[32..48], &three[48..]]);
    }
}
