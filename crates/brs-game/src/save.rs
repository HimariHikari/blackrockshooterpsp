//! Saved games: what the original keeps of a game in the one block it saves, and
//! the port's files for it. See `docs/SAVE_NOTES.md`.
//!
//! The original hands its block to the PSP's savedata dialog, which keeps it
//! (enciphered) in one of twenty directories, `DATA00` to `DATA19`. The port
//! keeps twenty files of its own under a folder of saves. A file is the parts of
//! the block one after the other, each with a name and its length: the time
//! played, the progress and what she carries as the block has them (so those two
//! are the original's byte for byte), her figures as the port keeps them, and
//! where she is.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{bail, ensure, Context, Result};
use bevy::prelude::*;

use crate::battle::tables::PlayerStats;
use crate::field::tables::FieldTables;
use crate::gallery::Opened;
use crate::items::Inventory;
use crate::options::{Options, LOUDEST};
use crate::progress::Progress;

/// How many saves there can be (0xF7FC): `DATA00` to `DATA19`.
pub const SLOTS: usize = 20;
/// The folder the saves are kept in unless another is named.
pub const DEFAULT_FOLDER: &str = "save";

/// The most the time played comes to (0x2AB0): 99:59:59, in sixtieths of a
/// second.
const TIME_CAP: u32 = 0x149_96c4;
const SIXTIETHS_A_SECOND: u32 = 60;

/// What a file begins with, and the number of its layout.
const MAGIC: &[u8; 8] = b"BRSSPORT";
const VERSION: u32 = 1;
// The parts of a file.
const PART_TIME: &[u8; 4] = b"TIME";
const PART_PROGRESS: &[u8; 4] = b"PROG";
const PART_HUB: &[u8; 4] = b"HUBS";
const PART_HERS: &[u8; 4] = b"HERS";
const PART_ITEMS: &[u8; 4] = b"ITEM";
const PART_PLACE: &[u8; 4] = b"PLAC";
/// The camera's record, of a game saved on the field: its kind and nine floats.
const PART_VIEW: &[u8; 4] = b"VIEW";

/// What the file of the system data begins with.
const SYSTEM_MAGIC: &[u8; 8] = b"BRSCPORT";
const SYSTEM_FILE: &str = "SYSTEM.sav";

/// The system data (the original's `BRSC.BIN`, of the word at the system's
/// + 0x21E74): the player's options, and whether the game has been cleared,
/// which the title goes by (0x394), and what of the gallery is open (its flags
/// from 11001 on). It is not of any one save: the original reads it as the game
/// starts and writes it when OPTIONS is left and after every save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct System {
    pub options: Options,
    pub cleared: bool,
    pub gallery: Opened,
}

impl System {
    /// Its file: the three volumes, a byte of bits (1 the captions go on of
    /// themselves, 2 the game has been cleared), and a bit for each thing of the
    /// gallery that is open.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut file = SYSTEM_MAGIC.to_vec();
        file.extend(VERSION.to_le_bytes());
        let options = &self.options;
        file.extend([options.music, options.effects, options.voices, u8::from(options.auto_skip) | u8::from(self.cleared) << 1]);
        file.extend(self.gallery.0);
        file
    }

    pub fn from_bytes(file: &[u8]) -> Result<System> {
        ensure!(file.len() >= 16 && &file[..8] == SYSTEM_MAGIC, "not the system data of this game");
        let version = u32::from_le_bytes(file[8..12].try_into().unwrap());
        ensure!(version == VERSION, "system data of layout {version}, which is not known");
        let [music, effects, voices, bits] = [file[12], file[13], file[14], file[15]];
        let options = Options { music: music.min(LOUDEST), effects: effects.min(LOUDEST), voices: voices.min(LOUDEST), auto_skip: bits & 1 != 0 };
        // A file written before the gallery was kept has nothing of it open.
        let mut gallery = Opened::default();
        for (bits, kept) in gallery.0.iter_mut().zip(&file[16..]) {
            *bits = *kept;
        }
        Ok(System { options, cleared: bits & 2 != 0, gallery })
    }
}

/// The title every save has (0x165C18).
pub const TITLE: &str = "BLACK★ROCK SHOOTER THE GAME";

/// How long the game has been played and how many times it has been cleared (the
/// block's + 4 and + 8). The original counts a sixtieth at every one of the
/// screen's frames while a game is on (0x2A80).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Played {
    pub sixtieths: u32,
    pub cleared: u8,
}

impl Played {
    /// So many sixtieths more.
    pub fn tick(&mut self, sixtieths: u32) {
        self.sixtieths = self.sixtieths.saturating_add(sixtieths).min(TIME_CAP);
    }

    /// Whether the game has been cleared (0x394): the system data says so, or
    /// the game there is has been, by how many times or by its variable. The
    /// system data is written saying what this says (0x264C).
    pub fn game_cleared(&self, system: bool, progress: &Progress) -> bool {
        system || self.cleared != 0 || progress.variable(crate::progress::variable::TIMES_ENDED) > 0
    }

    /// The hours, minutes and seconds it comes to (0x278, 0x2B0, 0x2F8).
    pub fn clock(&self) -> (u32, u32, u32) {
        let seconds = self.sixtieths / SIXTIETHS_A_SECOND;
        (seconds / 3600, seconds / 60 % 60, seconds % 60)
    }
}

/// Where a game was saved (variable 300, 0x2400; the block's + 0xFC0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// On the field: the map, the entrance she came in by, and where she stood
    /// and which way she faced, if that is known.
    Field { map: i32, entrance: i32, at: Option<(Vec3, f32)> },
    /// On the screen between missions.
    StageSelect,
}

/// A saved game.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub played: Played,
    pub progress: Progress,
    pub stats: PlayerStats,
    pub inventory: Inventory,
    pub place: Place,
    /// The camera's record, of a game saved on the field (the block's + 0xFC9
    /// and six floats).
    pub view: Option<crate::field::camera::Record>,
}

/// What the list of saves says of one (0xE698).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct About {
    pub level: i32,
    pub played: Played,
    pub place: Place,
}

impl About {
    /// Its second title: her level.
    pub fn level_line(&self) -> String {
        format!("Level      {}", self.level)
    }

    /// Its details: the time played, with "cleared" after it for a game that has
    /// been; and where it was saved, a map by its stage, counted from one, and
    /// the name of its place.
    pub fn details(&self, tables: &FieldTables) -> [String; 2] {
        let (hours, minutes, seconds) = self.played.clock();
        let cleared = if self.played.cleared > 0 { "cleared" } else { "" };
        let time = format!("Play Time: {hours:02}:{minutes:02}:{seconds:02} {cleared}");
        let place = match self.place {
            Place::StageSelect => "Save Point: STAGE SELECT".to_owned(),
            Place::Field { map, .. } => {
                let map = usize::try_from(map).ok().and_then(|map| tables.maps.get(map));
                let (stage, name) = map.map_or((0, ""), |map| (i32::from(map.stage), map.place.as_str()));
                format!("Save Point: STAGE {}:{name}", stage + 1)
            }
        };
        [time, place]
    }
}

impl Saved {
    pub fn about(&self) -> About {
        About { level: self.stats.level, played: self.played, place: self.place }
    }

    /// The file a save is.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut file = MAGIC.to_vec();
        file.extend(VERSION.to_le_bytes());
        let mut part = |name: &[u8; 4], bytes: Vec<u8>| {
            file.extend(name);
            file.extend((bytes.len() as u32).to_le_bytes());
            file.extend(bytes);
        };

        let mut time = self.played.sixtieths.to_le_bytes().to_vec();
        time.extend([self.played.cleared, 0, 0, 0]);
        part(PART_TIME, time);
        part(PART_PROGRESS, self.progress.to_block());
        part(PART_HUB, vec![self.progress.hub_stage, 0, 0, 0]);

        let stats = &self.stats;
        let mut hers = Vec::new();
        hers.extend(stats.level.to_le_bytes());
        hers.extend(stats.experience.to_le_bytes());
        hers.extend(stats.max_hp.to_le_bytes());
        hers.extend(stats.hp.to_le_bytes());
        for figure in [stats.attack, stats.special, stats.defence, stats.burst_bullets, stats.dodge_heat] {
            hers.extend(figure.to_le_bytes());
        }
        for bonus in stats.skill_bonus {
            hers.extend(bonus.to_le_bytes());
        }
        hers.extend(stats.skills);
        hers.extend(stats.weapon.to_le_bytes());
        hers.extend(stats.suit.to_le_bytes());
        part(PART_HERS, hers);
        part(PART_ITEMS, self.inventory.to_block());

        let mut place = Vec::new();
        let (kind, map, entrance, at) = match self.place {
            Place::Field { map, entrance, at } => (0i32, map, entrance, at),
            Place::StageSelect => (1, 0, 0, None),
        };
        for word in [kind, map, entrance, i32::from(at.is_some())] {
            place.extend(word.to_le_bytes());
        }
        let (stood, faced) = at.unwrap_or((Vec3::ZERO, 0.0));
        for figure in [stood.x, stood.y, stood.z, faced] {
            place.extend(figure.to_le_bytes());
        }
        part(PART_PLACE, place);
        if let Some(view) = self.view {
            let mut record = view.kind.to_le_bytes().to_vec();
            for figure in view.figures {
                record.extend(figure.to_le_bytes());
            }
            part(PART_VIEW, record);
        }
        file
    }

    /// The save a file is. A part it does not know is passed over; one it needs
    /// and does not find is an error.
    pub fn from_bytes(file: &[u8]) -> Result<Saved> {
        ensure!(file.len() >= 12 && &file[..8] == MAGIC, "not a save of this game");
        let version = u32::from_le_bytes(file[8..12].try_into().unwrap());
        ensure!(version == VERSION, "a save of layout {version}, which is not known");
        let mut parts: Vec<(&[u8], &[u8])> = Vec::new();
        let mut at = 12;
        while at < file.len() {
            ensure!(at + 8 <= file.len(), "the file ends inside a part's heading");
            let length = u32::from_le_bytes(file[at + 4..at + 8].try_into().unwrap()) as usize;
            ensure!(at + 8 + length <= file.len(), "the file ends inside a part");
            parts.push((&file[at..at + 4], &file[at + 8..at + 8 + length]));
            at += 8 + length;
        }
        let part = |name: &[u8; 4], least: usize| -> Result<&[u8]> {
            let found = parts.iter().find(|(of, _)| *of == name).map(|(_, bytes)| *bytes);
            let found = found.with_context(|| format!("the save has no part {}", String::from_utf8_lossy(name)))?;
            ensure!(found.len() >= least, "part {} is {} bytes", String::from_utf8_lossy(name), found.len());
            Ok(found)
        };
        let int = |bytes: &[u8], at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |bytes: &[u8], at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());

        let time = part(PART_TIME, 8)?;
        let played = Played { sixtieths: (int(time, 0) as u32).min(TIME_CAP), cleared: time[4] };
        let mut progress = Progress::from_block(part(PART_PROGRESS, Progress::BLOCK)?).context("the save's progress is not of the size")?;
        progress.hub_stage = part(PART_HUB, 1)?[0];
        let hers = part(PART_HERS, 64)?;
        let stats = PlayerStats {
            level: int(hers, 0),
            experience: int(hers, 4) as u32,
            max_hp: float(hers, 8),
            hp: float(hers, 12),
            attack: int(hers, 16),
            special: int(hers, 20),
            defence: int(hers, 24),
            burst_bullets: int(hers, 28),
            dodge_heat: int(hers, 32),
            skill_bonus: std::array::from_fn(|bonus| int(hers, 36 + bonus * 4)),
            skills: hers[52..56].try_into().unwrap(),
            weapon: int(hers, 56),
            suit: int(hers, 60),
        };
        ensure!((1..=PlayerStats::MAX_LEVEL as i32).contains(&stats.level) && stats.max_hp.is_finite() && stats.hp.is_finite(), "the save's figures are not a player's");
        let inventory = Inventory::from_block(part(PART_ITEMS, Inventory::BLOCK)?);
        let where_ = part(PART_PLACE, 32)?;
        let place = match int(where_, 0) {
            0 => {
                let at = (int(where_, 12) != 0).then(|| (Vec3::new(float(where_, 16), float(where_, 20), float(where_, 24)), float(where_, 28)));
                Place::Field { map: int(where_, 4), entrance: int(where_, 8), at }
            }
            1 => Place::StageSelect,
            other => bail!("the save was made in a place of kind {other}"),
        };
        // A save made before the camera was kept has none.
        let view = parts.iter().find(|(of, bytes)| *of == PART_VIEW && bytes.len() >= 40).map(|(_, bytes)| crate::field::camera::Record {
            kind: int(bytes, 0),
            figures: std::array::from_fn(|figure| float(bytes, 4 + figure * 4)),
        });
        Ok(Saved { played, progress, stats, inventory, place, view })
    }
}

/// A save as the list has it: what it says of itself, when its file was last
/// written and how big that is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listed {
    pub about: About,
    pub written: Option<SystemTime>,
    pub size: u64,
}

/// The folder the saves are kept in.
#[derive(Resource, Clone, Debug)]
pub struct Saves {
    folder: PathBuf,
}

impl Saves {
    pub fn new(folder: impl Into<PathBuf>) -> Saves {
        Saves { folder: folder.into() }
    }

    /// The file of a save, by its number: the original's names for them.
    fn file(&self, slot: usize) -> PathBuf {
        self.folder.join(format!("DATA{slot:02}.sav"))
    }

    /// Reads a save; nothing if there is none of that number.
    pub fn read(&self, slot: usize) -> Result<Option<Saved>> {
        let path = self.file(slot);
        if slot >= SLOTS || !path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        Saved::from_bytes(&bytes).with_context(|| format!("{} cannot be used", path.display())).map(Some)
    }

    /// Writes a save: to a file beside it first, which then takes its place, so
    /// that a save is never left half written.
    pub fn write(&self, slot: usize, saved: &Saved) -> Result<()> {
        ensure!(slot < SLOTS, "there are {SLOTS} saves, not a number {slot}");
        fs::create_dir_all(&self.folder).with_context(|| format!("making {}", self.folder.display()))?;
        let path = self.file(slot);
        let beside = path.with_extension("new");
        fs::write(&beside, saved.to_bytes()).with_context(|| format!("writing {}", beside.display()))?;
        fs::rename(&beside, &path).with_context(|| format!("putting {} in place", path.display()))
    }

    /// The system data, if there is any that can be read.
    pub fn read_system(&self) -> Option<System> {
        let path = self.folder.join(SYSTEM_FILE);
        let bytes = fs::read(&path).ok()?;
        System::from_bytes(&bytes).inspect_err(|error| warn!("{} cannot be used: {error:#}", path.display())).ok()
    }

    /// Writes the system data, unless it is there as it is.
    pub fn write_system(&self, system: &System) -> Result<()> {
        if self.read_system().as_ref() == Some(system) {
            return Ok(());
        }
        fs::create_dir_all(&self.folder).with_context(|| format!("making {}", self.folder.display()))?;
        let path = self.folder.join(SYSTEM_FILE);
        fs::write(&path, system.to_bytes()).with_context(|| format!("writing {}", path.display()))
    }

    /// Every save there is, by its number; one that cannot be read is listed as
    /// not there, and said.
    pub fn list(&self) -> Vec<Option<Listed>> {
        (0..SLOTS)
            .map(|slot| match self.read(slot) {
                Ok(saved) => saved.map(|saved| {
                    let file = fs::metadata(self.file(slot)).ok();
                    Listed { about: saved.about(), written: file.as_ref().and_then(|file| file.modified().ok()), size: file.map_or(0, |file| file.len()) }
                }),
                Err(error) => {
                    warn!("{error:#}");
                    None
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub fn folder(&self) -> &std::path::Path {
        &self.folder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::items::Items;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    fn stats() -> PlayerStats {
        PlayerStats { level: 3, experience: 150, max_hp: 496.0, hp: 300.5, attack: 52, special: 47, defence: 31, burst_bullets: 6, dodge_heat: 15, skill_bonus: [1, 2, 3, 4], skills: [1, 0, 23, 0], weapon: 129, suit: 133 }
    }

    fn saved(place: Place) -> Saved {
        let mut progress = Progress::start();
        progress.set_flag(6157, true);
        progress.set_variable(220, 115);
        progress.hub_stage = 2;
        let mut inventory = Inventory::default();
        // What she carries is kept by the items' numbers, whatever the items are.
        inventory = Inventory::from_block(&{
            let mut block = inventory.to_block();
            (block[1], block[0x81], block[0x85]) = (3, 1, 1);
            block
        });
        let view = matches!(place, Place::Field { at: Some(_), .. })
            .then_some(crate::field::camera::Record { kind: 2, figures: [1.0, 2.0, 3.0, -20.0, 135.0, 0.0, 7.0, 8.0, 9.0] });
        Saved { played: Played { sixtieths: 55_080, cleared: 0 }, progress, stats: stats(), inventory, place, view }
    }

    #[test]
    fn a_save_is_written_as_a_file_and_read_back_whole() {
        for place in [Place::StageSelect, Place::Field { map: 0, entrance: 2, at: None }, Place::Field { map: 3, entrance: 1, at: Some((Vec3::new(1425.0, 0.0, 5656.0), -132.0)) }] {
            let saved = saved(place);
            let file = saved.to_bytes();
            assert_eq!(&file[..8], MAGIC);
            assert_eq!(Saved::from_bytes(&file).unwrap(), saved);
            assert_eq!(saved.inventory.held(1), 3);
            // A part that is not known is passed over.
            let mut longer = file.clone();
            longer.extend(b"WHAT");
            longer.extend(3u32.to_le_bytes());
            longer.extend([1, 2, 3]);
            assert_eq!(Saved::from_bytes(&longer).unwrap(), saved);
            // A file cut short, or of something else, is refused.
            assert!(Saved::from_bytes(&file[..file.len() - 5]).is_err());
            assert!(Saved::from_bytes(b"something else entirely").is_err());
        }
    }

    #[test]
    fn the_time_played_is_told_in_hours_minutes_and_seconds_and_stops_at_its_most() {
        let mut played = Played { sixtieths: 55_080, cleared: 0 };
        assert_eq!(played.clock(), (0, 15, 18));
        played.tick(2);
        assert_eq!(played.sixtieths, 55_082);
        played.tick(u32::MAX);
        assert_eq!(played.clock(), (99, 59, 59));
    }

    #[test]
    fn the_list_says_of_a_save_what_the_original_does() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        let about = saved(Place::Field { map: 0, entrance: 2, at: None }).about();
        assert_eq!(about.level_line(), "Level      3");
        // The walkthrough's save, made at the first mission's terminal.
        assert_eq!(about.details(&tables), ["Play Time: 00:15:18 ".to_owned(), "Save Point: STAGE 1:THE END OF ANKH ST.".to_owned()]);
        let mut cleared = saved(Place::StageSelect).about();
        cleared.played.cleared = 1;
        assert_eq!(cleared.details(&tables), ["Play Time: 00:15:18 cleared".to_owned(), "Save Point: STAGE SELECT".to_owned()]);
        // A new game's own things can be saved as they are.
        let items = Items::load(&data).unwrap();
        let new = Saved { played: Played::default(), progress: Progress::start(), stats: PlayerStats::new_game(&data).unwrap(), inventory: Inventory::new_game(&items), place: Place::StageSelect, view: None };
        assert_eq!(Saved::from_bytes(&new.to_bytes()).unwrap(), new);
    }

    #[test]
    fn saves_are_kept_in_a_folder_by_their_numbers() {
        // Under the project's own build folder, not the system's for scratch files.
        let folder = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).join(format!("saves-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let saves = Saves::new(&folder);
        // Nothing there yet, and no folder made by looking.
        assert!(saves.list().iter().all(Option::is_none) && !folder.exists());
        assert_eq!(saves.read(3).unwrap(), None);
        let saved = saved(Place::StageSelect);
        saves.write(3, &saved).unwrap();
        assert_eq!(saves.read(3).unwrap(), Some(saved.clone()));
        let list = saves.list();
        assert_eq!(list.len(), SLOTS);
        assert_eq!(list.iter().filter(|listed| listed.is_some()).count(), 1);
        let listed = list[3].unwrap();
        assert_eq!((listed.about, listed.size), (saved.about(), saved.to_bytes().len() as u64));
        assert!(listed.written.is_some());
        // The system data is a file of its own, written only when it changes.
        assert_eq!(saves.read_system(), None);
        let mut gallery = Opened::default();
        gallery.open(1);
        gallery.open(208);
        let system = System { options: Options { music: 30, effects: 127, voices: 100, auto_skip: true }, cleared: true, gallery };
        saves.write_system(&system).unwrap();
        assert_eq!(saves.read_system(), Some(system));
        assert_eq!(System::from_bytes(&system.to_bytes()[..15]).ok(), None);
        // One written before the gallery was kept is read, with nothing of it open.
        assert_eq!(System::from_bytes(&system.to_bytes()[..16]).ok(), Some(System { gallery: Opened::default(), ..system }));
        assert_eq!(saves.list().iter().filter(|listed| listed.is_some()).count(), 1);
        // There is no twenty-first; and a file that is not a save is listed as
        // not there.
        assert!(saves.write(SLOTS, &saved).is_err());
        fs::write(folder.join("DATA05.sav"), b"junk").unwrap();
        assert!(saves.read(5).is_err() && saves.list()[5].is_none());
        fs::remove_dir_all(&folder).unwrap();
    }
}
