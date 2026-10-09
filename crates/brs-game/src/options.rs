//! The player's options, which the menu's OPTIONS changes: one word of the
//! original's system (the field manager's + 0x21E74), in which bits 0 to 6 are
//! how loud the music is, 7 to 13 the sounds and 14 to 20 the voices, each of
//! 127, and bit 21 says that a scene's captions go on of themselves. See
//! "OPTIONS" in `docs/MENU_NOTES.md`.
//!
//! The word has two bits more, for the PSP's data install (22: it is switched
//! on; 23: there is one to be had), which the port has nothing for: there is
//! none to be had, and the option reads as off (0x698).

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_formats::xtc;

use crate::data::GameData;

/// The table of the options a new game starts with, which are also what "Default"
/// puts back (0x14D8E4): one record of the music's volume, the sounds' and the
/// voices', and in bit 0 of its fourth byte the captions' way (0x12CA88 on).
const DEFAULTS: &str = "SYSTEM.VOL/db_resident.lpk/009.xtc";

/// The loudest a volume goes.
pub const LOUDEST: u8 = 127;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// How loud the music, the sounds and the voices are, of 127 (0x428, 0x4B4,
    /// 0x548).
    pub music: u8,
    pub effects: u8,
    pub voices: u8,
    /// A scene's captions go when their time is up (0x614); otherwise each stays
    /// until a button passes it, unless the scene's script says otherwise.
    pub auto_skip: bool,
}

impl Options {
    /// The options as a new game has them.
    pub fn load(data: &GameData) -> Result<Options> {
        let table = xtc::Table::parse(&data.read(DEFAULTS)?).with_context(|| format!("parsing {DEFAULTS}"))?;
        let record = table.record(0).filter(|record| record.len() >= 4).context("the table of options has no record")?;
        Ok(Options { music: record[0].min(LOUDEST), effects: record[1].min(LOUDEST), voices: record[2].min(LOUDEST), auto_skip: record[3] & 1 != 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_has_the_music_under_the_voices_and_captions_that_wait() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(Options::load(&data).unwrap(), Options { music: 60, effects: 127, voices: 127, auto_skip: false });
    }
}
