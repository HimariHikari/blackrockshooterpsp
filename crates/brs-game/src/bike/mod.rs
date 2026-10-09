//! The ride on the bike: the game within the game that the second stage's story
//! has her ride through (`startBikeMiniGame`, 0x147360). All that was read of it
//! is in `docs/BIKE_NOTES.md`.

pub mod camera;
pub mod chara;
pub mod display;
pub mod ride;
pub mod road;
pub mod rules;
pub mod script;
pub mod show;
pub mod tables;

use anyhow::{Context, Result};

pub use show::plugin;

/// The course a ride's number names (0x147344): the stage of that number in the
/// system's table, which is what a map's script chooses with `s96(41, n)`.
pub fn course(data: &crate::data::GameData, number: i32) -> Result<String> {
    let bytes = data.member(tables::SYSTEM_PACK, tables::SYSTEM_TABLES)?.context("the mini-game's tables are not there")?;
    let system = tables::SystemTables::read(&brs_formats::script::Script::parse(&bytes)?)?;
    let stage = usize::try_from(number).ok().and_then(|number| system.stages.get(number)).with_context(|| format!("the mini-game has no stage {number}"))?;
    Ok(stage.name.clone())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use brs_formats::{at3, pphd};

    use super::tables::tests::data;
    use super::*;

    #[test]
    fn a_rides_number_names_its_course() {
        let Some(data) = data() else { return };
        let names: Vec<String> = (0..8).map(|number| course(&data, number).unwrap()).collect();
        assert_eq!(names, ["easy", "normal", "hard", "ex00", "ex01", "ex_easy", "ex_normal", "ex_hard"]);
        assert!(course(&data, 8).is_err() && course(&data, -1).is_err());
    }

    #[test]
    fn the_mini_games_sounds_are_read_out_of_its_pack() {
        let Some(data) = data() else { return };
        // The bank is a bundle in a bundle in the pack, and every slot's sound
        // of it is decoded.
        let bank = pphd::Bank::parse(&data.read(show::BANK).unwrap()).unwrap();
        let body = data.read(pphd::data_file(Path::new(show::BANK)).unwrap()).unwrap();
        for slot in 0..ride::SOUND_SLOTS as u32 {
            let sound = bank.decode(slot, &body).unwrap_or_else(|error| panic!("sound {slot}: {error:#}"));
            assert!(sound.rate > 0 && !sound.samples.is_empty(), "sound {slot}");
        }
        // The voices are recordings, members of the pack by the system's table.
        let system = tables::SystemTables::read(&tables::tests::script(&data, tables::SYSTEM_PACK, tables::SYSTEM_TABLES)).unwrap();
        let members: Vec<usize> = system.voices.iter().flatten().map(|voice| voice.member).collect();
        assert_eq!(members, (104..120).collect::<Vec<_>>());
        for member in members {
            assert!(at3::is_recording(&data.read(format!("{}/{member:03}.at3", tables::SYSTEM_PACK)).unwrap()), "member {member}");
        }
    }
}
