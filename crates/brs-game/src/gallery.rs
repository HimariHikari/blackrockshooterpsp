//! What the gallery has to show: the three tables of the pack that is always there
//! (`SYSTEM.VOL/db_resident.lpk` files 6 to 8, read through the functions at
//! 0x12B768 to 0x12BED4), and which of their things are open. See
//! `docs/MENU_NOTES.md`, "GALLERY".
//!
//! A thing's number counts through all three tables from one. It is open while the
//! system data's flag `11000 + number` is set (0x4F49C); a record's reward opens
//! every thing of a group (0x19AC), setting that and the game's own flag `8100 +
//! number`, and a game that is loaded opens what its own flags say (0x1A34).

use anyhow::{ensure, Context, Result};
use bevy::prelude::*;
use brs_formats::xtc;

use crate::data::GameData;
use crate::progress::Progress;

const DATABASE: &str = "SYSTEM.VOL/db_resident.lpk";
/// The tables' files, a list each: the pictures, the music, the films.
const TABLES: [&str; LISTS] = ["006.xtc", "007.xtc", "008.xtc"];
pub const LISTS: usize = 3;

/// The lists, by the numbers the screen has for them (its + 0x80).
pub mod list {
    pub const PICTURES: usize = 0;
    pub const MUSIC: usize = 1;
    pub const FILMS: usize = 2;
}

/// Byte 12 of a record: the bit of one that is real (0x12BB74), and the bits of
/// the group it is opened with (0x12BAE0).
const REAL: u8 = 0x20;
const GROUP_BITS: u8 = 0x1f;

/// The game's own flag of a thing that has been opened, less the thing's number
/// (0x19FC). The system data's are from 0x2AF8 on (0x19F0), which [Opened]
/// keeps as its bits.
pub const GAME_FLAG: i32 = 0x1fa4;

/// How many things the three tables have room for, which is as many flags.
pub const NUMBERS: usize = 208;

/// One of the things there are to see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thing {
    /// Its number, through all three tables from one.
    pub number: u32,
    pub list: usize,
    pub name: String,
    pub about: String,
    /// What its file is named by: the three figures of a picture's archive, the
    /// name of a tune, the name of a film.
    pub file: String,
    pub group: u8,
    /// Where it comes in its list (0x12BA58).
    pub order: u8,
    /// Which of the gallery's small pictures is its own, from one (0x12BC14).
    pub small: u8,
    /// Of a picture: how many pieces across it is (0x12BC9C), and how wide and
    /// how high (0x12BD4C, 0x12BCF4).
    pub across: u8,
    pub size: UVec2,
}

impl Thing {
    /// The file it is seen or heard from, as the screen names it (0x4F3B0).
    pub fn path(&self) -> String {
        match self.list {
            list::PICTURES => format!("GALLERY/GLR_ART_{}.BIN", self.file.to_ascii_uppercase()),
            list::MUSIC => format!("SOUND/BGM_{}.AT3", self.file.to_ascii_uppercase()),
            _ => self.file.clone(),
        }
    }
}

/// Every real thing of the three tables.
#[derive(Resource, Clone, Debug, Default)]
pub struct Gallery(Vec<Thing>);

impl Gallery {
    pub fn load(data: &GameData) -> Result<Gallery> {
        let (mut things, mut number) = (Vec::new(), 0);
        for (list, file) in TABLES.into_iter().enumerate() {
            let path = format!("{DATABASE}/{file}");
            let table = xtc::Table::parse(&data.read(&path)?).with_context(|| format!("parsing {path}"))?;
            for index in 0..table.len() {
                number += 1;
                let record = table.record(index).context("a record within the table's count")?;
                ensure!(record.len() >= if list == list::PICTURES { 20 } else { 16 }, "{path}: records are {} bytes", record.len());
                if record[12] & REAL == 0 {
                    continue;
                }
                let word = |at: usize| u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]);
                let string = |at: usize| table.string(word(at)).unwrap_or_default().to_owned();
                let short = |at: usize| u32::from(u16::from_le_bytes([record[at], record[at + 1]]));
                let picture = list == list::PICTURES;
                things.push(Thing {
                    number,
                    list,
                    name: string(0),
                    about: string(4),
                    file: string(8),
                    group: record[12] & GROUP_BITS,
                    order: record[13],
                    small: if picture { record[15] } else { record[14] },
                    across: if picture { record[14] } else { 0 },
                    size: if picture { UVec2::new(short(18), short(16)) } else { UVec2::ZERO },
                });
            }
        }
        ensure!(number as usize <= NUMBERS, "the gallery's tables have {number} records");
        Ok(Gallery(things))
    }

    /// The things of a list in its order (0x4F978: the real ones, sorted by where
    /// each says it comes).
    pub fn list(&self, list: usize) -> Vec<&Thing> {
        let mut things: Vec<&Thing> = self.0.iter().filter(|thing| thing.list == list).collect();
        things.sort_by_key(|thing| thing.order);
        things
    }

    pub fn all(&self) -> impl Iterator<Item = &Thing> {
        self.0.iter()
    }

    /// A record's reward (0x19AC): every thing of a group is opened, or shut, in
    /// the game's own flags. [`Opened::take_from`] has the system data follow.
    pub fn open_group(&self, group: i32, on: bool, progress: &mut Progress) {
        for thing in self.0.iter().filter(|thing| i32::from(thing.group) == group) {
            progress.set_flag(GAME_FLAG + thing.number as i32, on);
        }
    }
}

/// What of the gallery is open: the system data's flags from 11001 on, a bit for
/// each number.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opened(pub [u8; NUMBERS / 8]);

impl Opened {
    pub fn has(&self, number: u32) -> bool {
        let at = number.wrapping_sub(1) as usize;
        self.0.get(at / 8).is_some_and(|bits| bits & (1 << (at % 8)) != 0)
    }

    pub fn open(&mut self, number: u32) {
        let at = number.wrapping_sub(1) as usize;
        if let Some(bits) = self.0.get_mut(at / 8) {
            *bits |= 1 << (at % 8);
        }
    }

    /// Whatever the game's own flags have open is open (0x1A34, and what 0x19AC
    /// sets with them). Says whether anything was opened by it.
    pub fn take_from(&mut self, gallery: &Gallery, progress: &Progress) -> bool {
        let before = *self;
        for thing in gallery.all().filter(|thing| progress.flag(GAME_FLAG + thing.number as i32)) {
            self.open(thing.number);
        }
        *self != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gallery_has_three_lists_numbered_through() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let gallery = Gallery::load(&data).expect("the gallery's tables load");
        let (pictures, music, films) = (gallery.list(list::PICTURES), gallery.list(list::MUSIC), gallery.list(list::FILMS));
        assert_eq!((pictures.len(), music.len(), films.len()), (83, 30, 33));
        // The pictures are numbered from one, the music after the pictures' 128
        // records and the films after the music's 40.
        let first = pictures[0];
        assert_eq!((first.number, first.name.as_str(), first.about.as_str(), first.file.as_str()), (1, "Key Art", "BRS THE GAME Key Art", "001"));
        assert_eq!((first.group, first.small, first.across, first.size), (7, 1, 4, UVec2::new(2048, 884)));
        assert_eq!(first.path(), "GALLERY/GLR_ART_001.BIN");
        assert_eq!((music[0].number, music[0].name.as_str(), music[0].path().as_str(), music[0].small), (129, "Wake Up Call", "SOUND/BGM_TITLE.AT3", 0x3f));
        assert_eq!((films[0].number, films[0].name.as_str(), films[0].file.as_str(), films[0].small), (169, "Opening Movie", "BRS_OP_final", 0x71));
        // Every picture's file is there, and no small picture is past the 146.
        for picture in &pictures {
            assert!(data.exists(&format!("{}/000.ptm", picture.path())), "{}", picture.path());
        }
        assert!(gallery.all().all(|thing| (1..=146).contains(&thing.small)));

        // A record's reward opens a group in the game's flags, and the system
        // data's follow.
        let (mut progress, mut opened) = (Progress::new_game(), Opened::default());
        assert!(!opened.take_from(&gallery, &progress));
        gallery.open_group(7, true, &mut progress);
        assert!(progress.flag(GAME_FLAG + 1) && !progress.flag(GAME_FLAG + 3));
        assert!(opened.take_from(&gallery, &progress) && opened.has(1) && !opened.has(3) && !opened.has(0));
        let of_seven = gallery.all().filter(|thing| thing.group == 7).count();
        assert_eq!(gallery.all().filter(|thing| opened.has(thing.number)).count(), of_seven);
        // What is open stays open when a game that has not opened it is loaded.
        assert!(!opened.take_from(&gallery, &Progress::new_game()) && opened.has(1));
    }
}
