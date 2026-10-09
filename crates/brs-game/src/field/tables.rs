//! The field's resident tables: the list of maps, the list of character resources
//! and the tunables (`prm`), which are data symbols of an `SSCR`-format file. See
//! `docs/FIELD_NOTES.md`.

use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use bevy::prelude::*;
use brs_formats::script::Script;

use super::place::Places;
use crate::battle::tables::Starts;
use crate::data::GameData;
use crate::records::Records;

const RESIDENT_DATA: &str = "SYSTEM.VOL/fld_resident.bin/00.sscr";

// A record of `lst_map`.
const MAP_RECORD_LEN: usize = 24;
const MAP_NAME_LEN: usize = 12;
const MAP_STAGE_AT: usize = 0x0c;
const MAP_BATTLE_AT: usize = 0x0e;
const MAP_SIGN_AT: usize = 0x10;
const MAP_PLACE_AT: usize = 0x14;
/// A record of `lst_fchr` is a name and nothing else.
const CHARACTER_RECORD_LEN: usize = 16;
/// So is one of `lst_fevt`, the events a map's script can have played.
const EVENT_RECORD_LEN: usize = 17;
/// Where the field's table of counts (`dat_tbl`) has the number of entries of
/// `lst_mesh` (0x1102F8).
const COUNT_OF_MESHES: usize = 0x10;
/// And of `lst_bgm`, the tunes (0x1102B8): a record of that is a name.
const COUNT_OF_TUNES: usize = 0x0c;
const TUNE_RECORD_LEN: usize = 16;
/// And of `lst_mission`, the missions (0x1102D8): two bytes of it.
const COUNT_OF_MISSIONS: usize = 0x0e;
/// And of `lst_gnd_se`, the kinds of ground (0x110278): a record of that is two
/// sounds, one for each foot.
const COUNT_OF_GROUNDS: usize = 0x0a;

/// A field map as the game lists it.
#[derive(Clone, Debug)]
pub struct MapEntry {
    /// As in the file names, e.g. `d_stg01_01`.
    pub name: String,
    /// Which of the game's stages it is of, from 0.
    pub stage: u8,
    /// The battle map fought on from here: 101 is stage 1, map 1.
    pub battle: i16,
    /// Which of the places the sign has letters for it is, which is what is
    /// shown on coming into it (0xEDCC0).
    pub sign: i16,
    /// The name of the place, as a save has it.
    pub place: String,
}

impl MapEntry {
    /// The name battle maps go by, e.g. `STG01_01`.
    pub fn battle_stage(&self) -> String {
        format!("STG{:02}_{:02}", self.battle / 100, self.battle % 100)
    }
}

/// The field's tunables, which are read by offset.
#[derive(Clone, Debug)]
pub struct Tunables(Vec<u8>);

impl Tunables {
    pub fn u8(&self, at: usize) -> Result<u8> {
        Ok(self.bytes::<1>(at)?[0])
    }

    pub fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(at)?))
    }

    pub fn f32(&self, at: usize) -> Result<f32> {
        Ok(f32::from_le_bytes(self.bytes(at)?))
    }

    fn bytes<const N: usize>(&self, at: usize) -> Result<[u8; N]> {
        let bytes = self.0.get(at..at + N).with_context(|| format!("the field's tunables end before {at:#x}"))?;
        Ok(bytes.try_into().unwrap())
    }
}

/// Where the field's tables sit in the address space of a script that is handed
/// their addresses (the system script): clear of that script's own image.
pub const RESIDENT_BASE: u32 = 0x2000_0000;

// A record of `lst_mission`, and what the port reads of it.
const MISSION_RECORD_LEN: usize = 0x74;
const MISSION_TITLE_AT: usize = 0x04;
const MISSION_BRIEFING_AT: usize = 0x08;
const MISSION_OBJECTIVE_AT: usize = 0x0c;
const MISSION_HINT_AT: usize = 0x10;
const MISSION_LEVEL_AT: usize = 0x14;
const MISSION_MARKS_AT: usize = 0x6c;
const MISSION_MAP_AT: usize = 0x2c;
const MISSION_ENTRANCE_AT: usize = 0x2e;
const MISSION_LINES_AT: usize = 0x30;
// `lst_pop`: for each group, eight kinds of enemy that may pop up, each a resource
// and its share out of a hundred; resource 0xFF ends the group's list (0xD68C8).
const POP_GROUP_LEN: usize = 16;
const POP_KINDS: usize = 8;
const POP_END: u8 = 0xff;

/// What the screen where a mission is chosen reads of a mission's record
/// (0x60A84).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionRecord {
    /// The stage it is of, from 0; a record that is no mission has 0xFF.
    pub stage: u8,
    /// 0 for a mission of the story, 1 for an extra, 2 for a free hunt.
    pub kind: u8,
    /// Who it is from: one of the fourteen faces.
    pub from: u8,
    pub title: String,
    pub briefing: String,
    pub objective: String,
    pub hint: String,
    /// The level it is meant for, or less than nought.
    pub level: i32,
    /// Two flags that mark it in the list, or less than nought.
    pub marks: [i32; 2],
}

#[derive(Resource, Clone)]
pub struct FieldTables {
    pub maps: Vec<MapEntry>,
    /// The resource name of each character number, e.g. `m_egg01`.
    pub characters: Vec<String>,
    /// The name of each event, e.g. `FEVT_C_S1M1_AD06`.
    pub events: Vec<String>,
    /// The model parts scripts can show and hide: for each number they know one by,
    /// the id the part has in a model (`lst_mesh`).
    pub meshes: Vec<u8>,
    /// The name of each tune, as in `field1` (`lst_bgm`).
    pub tunes: Vec<String>,
    /// The sounds of a step on each kind of ground, one for each foot
    /// (`lst_gnd_se`).
    pub steps: Vec<[u8; 2]>,
    /// How many records the list of missions has (`dat_tbl`).
    pub missions: i32,
    pub tunables: Tunables,
    /// The whole file, as a script that runs with the tables sees it.
    pub resident: Arc<Script>,
    /// The records, which the system script has kept and put back around a
    /// mission.
    pub records: Records,
    /// The lines and the letters of the sign that names a place.
    pub places: Places,
    /// The figures a game starts her with, and those a script gives her (`s262`).
    pub starts: Starts,
}

impl FieldTables {
    pub fn load(data: &GameData) -> Result<FieldTables> {
        let resident = Script::parse_at(&data.read(RESIDENT_DATA)?, RESIDENT_BASE).with_context(|| format!("parsing {RESIDENT_DATA}"))?;
        let resident = Arc::new(resident);
        let table = |name: &str| resident.extent(name).with_context(|| format!("the field's data has no {name}"));

        let mut maps = Vec::new();
        for record in table("lst_map")?.chunks_exact(MAP_RECORD_LEN) {
            let place = u32::from_le_bytes(record[MAP_PLACE_AT..MAP_PLACE_AT + 4].try_into().unwrap());
            maps.push(MapEntry {
                name: text(&record[..MAP_NAME_LEN]),
                stage: record[MAP_STAGE_AT],
                battle: i16::from_le_bytes(record[MAP_BATTLE_AT..MAP_BATTLE_AT + 2].try_into().unwrap()),
                sign: i16::from_le_bytes(record[MAP_SIGN_AT..MAP_SIGN_AT + 2].try_into().unwrap()),
                place: place_name(&resident, place),
            });
        }
        let characters: Vec<String> = table("lst_fchr")?.chunks_exact(CHARACTER_RECORD_LEN).map(text).collect();
        let events: Vec<String> = table("lst_fevt")?.chunks_exact(EVENT_RECORD_LEN).map(text).collect();
        // The table of counts says how many of the list's bytes are entries.
        let counts = table("dat_tbl")?;
        let mesh_count = *counts.get(COUNT_OF_MESHES).context("the field's table of counts is too short")? as usize;
        let meshes = table("lst_mesh")?.get(..mesh_count).context("the field's list of model parts is too short")?.to_vec();
        let tune_count = *counts.get(COUNT_OF_TUNES).context("the field's table of counts is too short")? as usize;
        let tunes: Vec<String> = table("lst_bgm")?.chunks_exact(TUNE_RECORD_LEN).take(tune_count).map(text).collect();
        let ground_count = *counts.get(COUNT_OF_GROUNDS).context("the field's table of counts is too short")? as usize;
        let steps: Vec<[u8; 2]> = table("lst_gnd_se")?.chunks_exact(2).take(ground_count).map(|feet| [feet[0], feet[1]]).collect();
        let missions = counts.get(COUNT_OF_MISSIONS..COUNT_OF_MISSIONS + 2).context("the field's table of counts is too short")?;
        let missions = i32::from(u16::from_le_bytes([missions[0], missions[1]]));
        ensure!(!maps.is_empty() && !characters.is_empty(), "the field's lists are empty");
        for name in ["lst_mission", "lst_pop"] {
            table(name)?;
        }
        let tunables = Tunables(table("prm")?.to_vec());
        let records = Records::load(data).context("reading the records' script")?;
        let places = Places::load(data).context("reading the tables of the places' sign")?;
        let starts = Starts::load(data).context("reading the start table")?;
        Ok(FieldTables { maps, characters, events, meshes, tunes, steps, missions, tunables, resident: resident.clone(), records, places, starts })
    }

    /// The address of one of the tables, as a script sees it.
    pub fn address(&self, name: &str) -> u32 {
        self.resident.symbol(name).unwrap_or(0)
    }

    /// The address of mission `number`'s record (0x11049C).
    pub fn mission_address(&self, number: i32) -> u32 {
        self.address("lst_mission").wrapping_add((number as u32).wrapping_mul(MISSION_RECORD_LEN as u32))
    }

    fn mission(&self, number: i32) -> Option<&[u8]> {
        let at = usize::try_from(number).ok()? * MISSION_RECORD_LEN;
        self.resident.extent("lst_mission")?.get(at..at + MISSION_RECORD_LEN)
    }

    /// The text a word of a mission's record points at.
    fn mission_text(&self, number: i32, at: usize) -> Option<String> {
        let record = self.mission(number)?;
        let address = u32::from_le_bytes(record.get(at..at + 4)?.try_into().unwrap());
        self.resident.is_address(address).then(|| self.resident.string(address)).flatten()
    }

    /// A mission's title.
    pub fn mission_title(&self, number: i32) -> Option<String> {
        self.mission_text(number, MISSION_TITLE_AT)
    }

    /// What the screen where a mission is chosen reads of a mission's record.
    pub fn mission_record(&self, number: i32) -> Option<MissionRecord> {
        let record = self.mission(number)?;
        let word = |at: usize| i32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let text = |at: usize| self.mission_text(number, at).unwrap_or_default();
        Some(MissionRecord {
            stage: record[0],
            kind: record[1],
            from: record[2],
            title: text(MISSION_TITLE_AT),
            briefing: text(MISSION_BRIEFING_AT),
            objective: text(MISSION_OBJECTIVE_AT),
            hint: text(MISSION_HINT_AT),
            level: word(MISSION_LEVEL_AT),
            marks: [word(MISSION_MARKS_AT), word(MISSION_MARKS_AT + 4)],
        })
    }

    /// The map a mission starts on, and the entrance she comes into it by.
    pub fn mission_start(&self, number: i32) -> Option<(i32, i32)> {
        let record = self.mission(number)?;
        let short = |at: usize| i16::from_le_bytes([record[at], record[at + 1]]) as i32;
        Some((short(MISSION_MAP_AT), short(MISSION_ENTRANCE_AT)))
    }

    /// One of the lines of a mission that say what is to be done.
    pub fn mission_line(&self, number: i32, line: i32) -> Option<String> {
        let line = usize::try_from(line).ok()?;
        self.mission_text(number, MISSION_LINES_AT + line * 4)
    }

    /// The kinds of enemy that pop up in a map of pop group `group`: each one's
    /// character resource and its share out of a hundred.
    pub fn pops(&self, group: i32) -> Vec<(i32, i32)> {
        let records = usize::try_from(group).ok().and_then(|group| {
            let at = group * POP_GROUP_LEN;
            self.resident.extent("lst_pop")?.get(at..at + POP_GROUP_LEN)
        });
        let Some(records) = records else { return Vec::new() };
        let kinds = records.chunks_exact(2).take(POP_KINDS).take_while(|kind| kind[0] != POP_END);
        kinds.map(|kind| (kind[0] as i32, kind[1] as i32)).collect()
    }

    /// The number of the map called `name` (as in `d_stg01_01`), whatever its case.
    pub fn map_number(&self, name: &str) -> Option<usize> {
        self.maps.iter().position(|map| map.name.eq_ignore_ascii_case(name))
    }

    /// Which of a model's switchable parts are shown, given the character's flags
    /// for them: of the parts `ids`, those the field's list does not have, and those
    /// it has whose flag is set. Where the list has an id twice the later entry
    /// decides, as it is the last to be applied (0xFE2EC).
    pub fn shown_parts(&self, ids: impl Iterator<Item = u16>, flag: impl Fn(u32) -> bool) -> Vec<u16> {
        ids.filter(|&id| match self.meshes.iter().rposition(|&mesh| u16::from(mesh) == id) {
            Some(part) => flag(part as u32),
            None => true,
        })
        .collect()
    }

    /// The sound of a step of one foot or the other on ground of kind `kind`
    /// (0xFC954), if the list has the kind.
    pub fn step_sound(&self, kind: u8, foot: usize) -> Option<u32> {
        self.steps.get(kind as usize).map(|feet| u32::from(feet[foot.min(1)]))
    }

    /// The name of tune `number` (0x888), if the list has it.
    pub fn tune(&self, number: i32) -> Option<&str> {
        usize::try_from(number).ok().and_then(|index| self.tunes.get(index)).map(String::as_str)
    }

    /// The number the list has an event under by its name, as in
    /// `FEVT_C_S2M2_BALL`.
    pub fn event_number(&self, name: &str) -> Option<i32> {
        self.events.iter().position(|event| event.eq_ignore_ascii_case(name)).map(|index| index as i32)
    }

    /// The bundle of event `number`, e.g. `FLD/FEVT/FEVT_C_S1M1_AD06.BIN`.
    pub fn event_bundle(&self, number: i32) -> Option<String> {
        let name = usize::try_from(number).ok().and_then(|index| self.events.get(index))?;
        (!name.is_empty()).then(|| format!("FLD/FEVT/{}.BIN", name.to_ascii_uppercase()))
    }

    /// The bundle of character resource `number`, e.g. `FLD/FCHR/FCHR_M_EGG01.BIN`.
    pub fn character_bundle(&self, number: i32) -> Option<String> {
        let name = usize::try_from(number).ok().and_then(|index| self.characters.get(index))?;
        (!name.is_empty()).then(|| format!("FLD/FCHR/FCHR_{}.BIN", name.to_ascii_uppercase()))
    }
}

/// The text of a fixed-length, zero-padded field.
/// The bundle of the field character a suit has her be (0x14BE78, 0x1105C4),
/// from the list of characters alone: for a battle, which has no use for the
/// rest of the field's tables.
pub fn suit_bundle(data: &GameData, suit: i32) -> Result<String> {
    let resident = Script::parse_at(&data.read(RESIDENT_DATA)?, RESIDENT_BASE).with_context(|| format!("parsing {RESIDENT_DATA}"))?;
    let list = resident.extent("lst_fchr").context("the field's data has no lst_fchr")?;
    let number = usize::try_from(crate::player::suit_character(suit)).unwrap_or(0);
    let name = list.chunks_exact(CHARACTER_RECORD_LEN).nth(number).map(text).filter(|name| !name.is_empty());
    let name = name.with_context(|| format!("the field's list has no character {number}"))?;
    Ok(format!("FLD/FCHR/FCHR_{}.BIN", name.to_ascii_uppercase()))
}

fn text(field: &[u8]) -> String {
    let len = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..len]).into_owned()
}

fn place_name(resident: &Script, address: u32) -> String {
    if resident.is_address(address) { resident.string(address).unwrap_or_default() } else { String::new() }
}
