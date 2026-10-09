//! Effects: `EFC` files, each an effect with what it is made of, and `EFP` files,
//! many effects with what they share (`docs/FORMAT_NOTES.md`, "Effects").
//!
//! Either is a head and a chain of parts: models (`INSM`) with the animations
//! (`INSA`) that move them, textures (`PTMD`), and the scripts that say what an
//! effect lets go of them, when and how. An effect is asked for by number, and its
//! number is in its script's name. What a script says is not read here yet.

use std::ops::Range;

use anyhow::{bail, ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"EFC\0";
pub const MAGIC_OF_MANY: &[u8; 4] = b"EFP\0";

/// The versions the game takes (0x126B08).
const VERSIONS: [u16; 2] = [100, 101];
const VERSION_AT: usize = 0x04;
/// How many parts there are of each kind, a byte each: scripts, models,
/// animations, textures.
const COUNTS_AT: usize = 0x0c;
const HEAD_LEN: usize = 0x10;

/// A part's head: how far it is to the next part's head, the part's kind, and its
/// name; its data follows.
const PART_HEAD_LEN: usize = 0x20;
const PART_KIND_AT: usize = 0x04;
const PART_NAME_AT: usize = 0x08;

/// What a part is (0x127148). A part of kind nought ends the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An effect's script (`.esb`).
    Script = 1,
    Model = 2,
    /// Of the model before it: its nodes' (`.anm`) or its textures' (`.imta`).
    Animation = 3,
    Texture = 4,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub kind: Kind,
    pub name: String,
    /// Where in the file its data is.
    pub data: Range<usize>,
}

impl Part {
    /// The number an effect is asked for by (0xA5A6C): the six figures after the
    /// two letters its script's name begins with.
    pub fn number(&self) -> Option<u32> {
        let figures = self.name.get(2..8).filter(|figures| figures.bytes().all(|byte| byte.is_ascii_digit()))?;
        (self.kind == Kind::Script).then(|| figures.parse().ok()).flatten()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub version: u16,
    pub parts: Vec<Part>,
}

impl File {
    pub fn parse(d: &[u8]) -> Result<File> {
        ensure!(d.starts_with(MAGIC) || d.starts_with(MAGIC_OF_MANY), "not an effect file");
        ensure!(d.len() >= HEAD_LEN, "file ends inside its head");
        let r = Reader(d);
        let version = r.u16(VERSION_AT)?;
        ensure!(VERSIONS.contains(&version), "version {version} is not one the game takes");

        // The chain (0x126E80): each head says how far the next is.
        let mut parts = Vec::new();
        let mut at = HEAD_LEN;
        loop {
            ensure!(at + PART_HEAD_LEN <= d.len(), "the chain of parts runs off the file at {at:#x}");
            let kind = match r.u8(at + PART_KIND_AT)? {
                0 => break,
                1 => Kind::Script,
                2 => Kind::Model,
                3 => Kind::Animation,
                4 => Kind::Texture,
                other => bail!("a part of kind {other} at {at:#x}"),
            };
            let to_next = r.u32(at)? as usize;
            ensure!(to_next >= PART_HEAD_LEN && at + to_next <= d.len(), "the part at {at:#x} says the next is {to_next:#x} on");
            let name = &d[at + PART_NAME_AT..at + PART_HEAD_LEN];
            let name = &name[..name.iter().position(|&byte| byte == 0).unwrap_or(name.len())];
            let name = std::str::from_utf8(name).with_context(|| format!("the name of the part at {at:#x}"))?.to_owned();
            parts.push(Part { kind, name, data: at + PART_HEAD_LEN..at + to_next });
            at += to_next;
        }

        // The head counts them by kind.
        let counts: [u8; 4] = r.bytes(COUNTS_AT)?;
        for (kind, count) in [Kind::Script, Kind::Model, Kind::Animation, Kind::Texture].into_iter().zip(counts) {
            let found = parts.iter().filter(|part| part.kind == kind).count();
            ensure!(found == usize::from(count), "the head counts {count} parts that are a {kind:?}, the chain has {found}");
        }
        Ok(File { version, parts })
    }

    /// The effects of the file: each one's number and its script.
    pub fn effects(&self) -> impl Iterator<Item = (u32, &Part)> {
        self.parts.iter().filter_map(|part| Some((part.number()?, part)))
    }

    /// A model's animations: the parts of that kind that follow it (0x127260).
    pub fn animations_of(&self, model: usize) -> &[Part] {
        let after = self.parts.get(model + 1..).unwrap_or(&[]);
        &after[..after.iter().position(|part| part.kind != Kind::Animation).unwrap_or(after.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(kind: u8, name: &str, data: &[u8]) -> Vec<u8> {
        let mut part = Vec::new();
        part.extend(((PART_HEAD_LEN + data.len()) as u32).to_le_bytes());
        part.extend([kind, 0, 0, 0]);
        let mut field = [0u8; 24];
        field[..name.len()].copy_from_slice(name.as_bytes());
        part.extend(field);
        part.extend(data);
        part
    }

    fn file(counts: [u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend(MAGIC_OF_MANY);
        d.extend(101u32.to_le_bytes());
        d.extend(0u32.to_le_bytes());
        d.extend(counts);
        for part in parts {
            d.extend(part);
        }
        // The part that ends the chain has the file's own name.
        d.extend([0u8; 8]);
        let mut field = [0u8; 24];
        field[..9].copy_from_slice(b"eb205.efp");
        d.extend(field);
        d
    }

    #[test]
    fn the_parts_are_a_chain_and_an_effects_number_is_in_its_scripts_name() {
        let parts = [
            part(2, "bullet_fire01_m.mdl", b"INSM...."),
            part(3, "bullet_fire01_m.anm", b"INSA"),
            part(3, "bullet_fire01_m.imta", b"INSA"),
            part(4, "ef_a_kml.ptm", b"PTMD"),
            part(1, "eb205001.esb", &[1, 2, 3, 4, 5, 6, 7, 8]),
            part(1, "eb205016.esb", &[9; 16]),
        ];
        let d = file([2, 1, 2, 1], &parts);
        let read = File::parse(&d).unwrap();
        assert_eq!(read.version, 101);
        let kinds: Vec<Kind> = read.parts.iter().map(|part| part.kind).collect();
        assert_eq!(kinds, [Kind::Model, Kind::Animation, Kind::Animation, Kind::Texture, Kind::Script, Kind::Script]);
        assert_eq!(read.parts[0].name, "bullet_fire01_m.mdl");
        assert_eq!(&d[read.parts[0].data.clone()], b"INSM....");
        assert_eq!(&d[read.parts[4].data.clone()], &[1, 2, 3, 4, 5, 6, 7, 8]);
        // Only a script has a number.
        let numbers: Vec<u32> = read.effects().map(|(number, _)| number).collect();
        assert_eq!(numbers, [205001, 205016]);
        assert_eq!(read.parts[0].number(), None);
        // A model's animations are the parts after it that are animations.
        assert_eq!(read.animations_of(0).len(), 2);
        assert!(read.animations_of(3).is_empty() && read.animations_of(5).is_empty());
    }

    #[test]
    fn a_file_that_is_not_as_it_says_is_refused() {
        let good = file([1, 0, 0, 0], &[part(1, "ev000123.esb", &[0; 4])]);
        assert_eq!(File::parse(&good).unwrap().effects().next().map(|(number, _)| number), Some(123));
        // Counts that are not the chain's, a version the game does not take, a
        // kind it does not know, and a chain that runs off the end.
        assert!(File::parse(&file([2, 0, 0, 0], &[part(1, "ev000123.esb", &[0; 4])])).is_err());
        let mut old = good.clone();
        old[VERSION_AT] = 99;
        assert!(File::parse(&old).is_err());
        assert!(File::parse(&file([0, 0, 0, 0], &[part(7, "what", &[])])).is_err());
        assert!(File::parse(&good[..good.len() - PART_HEAD_LEN]).is_err());
        assert!(File::parse(b"EFX\0").is_err());
    }
}
