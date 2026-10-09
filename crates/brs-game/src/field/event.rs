//! The field's events: what a map's script has played with `s237`. Each is a bundle
//! under `FLD/FEVT`, named in the game's list of them, with a script of its own, the
//! texts it says, the recordings it says them with and the faces of those who speak.
//! See `docs/FIELD_NOTES.md`.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use brs_engine::step::STEPS_PER_SECOND;
use brs_formats::script::Script;

use super::tables::FieldTables;
use crate::data::GameData;

// The slots of an event's bundle.
const SCRIPT: &str = "08.bin/00.sscr";
const TEXTS: &str = "09.bin/00.bin";
const VOICES: &str = "05.bin";
const FACES: &str = "07.bin";
const BANK: &str = "06.bin/00.pphd";
/// What a recording's file ends in; the sound made of it lies beside it.
const RECORDING: &str = ".riff";

/// The functions the engine looks for in an event's script, in its order: the
/// first there is the one it starts (0xD7DEC).
const ENTRIES: [&str; 3] = ["script_entry", "async_event", "comm_event"];

const TEXTS_MAGIC: &[u8; 2] = b"TD";

/// An event, loaded.
pub struct EventData {
    /// Its bundle, e.g. `FLD/FEVT/FEVT_C_S1M1_AD06.BIN`.
    pub bundle: String,
    pub script: Arc<Script>,
    /// The function the engine starts.
    pub entry: u32,
    pub texts: Vec<String>,
    /// Its recordings, by number.
    pub voices: BTreeMap<i32, Recording>,
    /// The header of its bank of sounds, if it has one.
    pub bank: Option<String>,
}

/// A recording of an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording {
    pub file: String,
    /// How long it lasts, in frames.
    pub frames: i32,
}

impl EventData {
    /// Loads event `number` of the game's list. Not every name in the list has a
    /// bundle, and not every bundle a script to run: those are no events.
    pub fn load(data: &GameData, tables: &FieldTables, number: i32) -> Result<Option<EventData>> {
        let Some(bundle) = tables.event_bundle(number) else { return Ok(None) };
        let path = format!("{bundle}/{SCRIPT}");
        if !data.exists(&path) {
            return Ok(None);
        }
        let script = data.script(&path)?;
        let Some(entry) = ENTRIES.iter().find_map(|name| script.symbol(name)) else { return Ok(None) };

        let path = format!("{bundle}/{TEXTS}");
        let texts = if data.exists(&path) { texts(&data.read(&path)?).with_context(|| format!("reading {path}"))? } else { Vec::new() };

        // The recordings are the files of their slot, each named by its number.
        let folder = format!("{bundle}/{VOICES}");
        let mut voices = BTreeMap::new();
        if data.has_folder(&folder) {
            for name in data.list(&folder)?.into_iter().filter(|name| name.ends_with(RECORDING)) {
                let Some(number) = name.split('.').next().and_then(|stem| stem.parse::<i32>().ok()) else { continue };
                let file = format!("{folder}/{name}");
                let frames = frames(&data.read(&file)?).with_context(|| format!("{file} is not a recording"))?;
                voices.insert(number, Recording { file, frames });
            }
        }
        let bank = Some(format!("{bundle}/{BANK}")).filter(|bank| data.exists(bank));
        Ok(Some(EventData { bundle, script: Arc::new(script), entry, texts, voices, bank }))
    }
}

/// The files of a face of an event's bundle (0x39584: a pack of three): the objects
/// its name is drawn as and their texture, and its picture.
pub struct FaceFiles {
    pub name: String,
    pub name_texture: String,
    pub picture: String,
}

pub fn face_files(bundle: &str, face: i32) -> FaceFiles {
    let file = |name: &str| format!("{bundle}/{FACES}/{face:02}.lpk/{name}");
    FaceFiles { name: file("000.sdoa"), name_texture: file("001.ptm"), picture: file("002.ptm") }
}

/// The texts of an event (its slot 9): `TD`, how many, and where each starts; each
/// ends with a zero (0x12E668 finds one).
fn texts(d: &[u8]) -> Result<Vec<String>> {
    ensure!(d.len() >= 4 && d.starts_with(TEXTS_MAGIC), "not an event's texts");
    let count = u16::from_le_bytes([d[2], d[3]]) as usize;
    let mut texts = Vec::with_capacity(count);
    for index in 0..count {
        let at = d.get(4 + index * 4..8 + index * 4).context("the list of texts runs past the file")?;
        let at = u32::from_le_bytes(at.try_into().unwrap()) as usize;
        let tail = d.get(at..).context("a text starts past the file")?;
        let len = tail.iter().position(|&byte| byte == 0).unwrap_or(tail.len());
        texts.push(String::from_utf8_lossy(&tail[..len]).into_owned());
    }
    Ok(texts)
}

/// How long a recording lasts, in frames: the samples its `fact` chunk counts, at
/// the rate its `fmt ` chunk gives.
pub fn frames(riff: &[u8]) -> Option<i32> {
    if riff.len() < 12 || &riff[..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return None;
    }
    let word = |at: usize| riff.get(at..at + 4).map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()));
    let (mut rate, mut samples) = (None, None);
    let mut at = 12;
    while let (Some(id), Some(size)) = (riff.get(at..at + 4), word(at + 4)) {
        match id {
            b"fmt " => rate = word(at + 12),
            b"fact" => samples = word(at + 8),
            _ => {}
        }
        // Chunks are padded to an even length.
        at += 8 + size as usize + (size as usize & 1);
    }
    let (rate, samples) = (rate.filter(|&rate| rate > 0)?, samples?);
    Some((samples as f64 * STEPS_PER_SECOND / rate as f64).ceil() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn an_events_texts_are_found_by_the_list_at_their_start() {
        let mut d = Vec::new();
        d.extend(b"TD");
        d.extend(2u16.to_le_bytes());
        d.extend(12u32.to_le_bytes());
        d.extend(16u32.to_le_bytes());
        d.extend(b"One\0Two\nlines\0");
        assert_eq!(texts(&d).unwrap(), ["One", "Two\nlines"]);
        assert!(texts(b"XX\x01\x00").is_err());
    }

    #[test]
    fn a_recording_lasts_its_samples_at_its_rate() {
        let mut riff = Vec::new();
        riff.extend(b"RIFF\0\0\0\0WAVE");
        riff.extend(b"fmt ");
        riff.extend(16u32.to_le_bytes());
        riff.extend([0xfe, 0xff, 1, 0]);
        riff.extend(44_100u32.to_le_bytes());
        riff.extend([0; 8]);
        riff.extend(b"fact");
        riff.extend(4u32.to_le_bytes());
        // A second and a half, and a little: the frame it ends in counts.
        riff.extend(66_200u32.to_le_bytes());
        assert_eq!(frames(&riff), Some(46));
        assert_eq!(frames(b"RIFF\0\0\0\0WAVEdata"), None);
        assert_eq!(frames(b"not a recording"), None);
    }

    #[test]
    fn a_message_of_the_first_mission_loads_with_its_text_and_its_voice() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        // What Phobos says when the Hornets come.
        let event = EventData::load(&data, &tables, 116).unwrap().expect("the event has a script");
        assert_eq!(event.bundle, "FLD/FEVT/FEVT_C_S1M1_AD06.BIN");
        assert_eq!(event.texts, ["More enemies inbound! Be quick\nor they'll just keep coming!"]);
        // 150913 samples at 44100 a second.
        let voice = Recording { file: "FLD/FEVT/FEVT_C_S1M1_AD06.BIN/05.bin/00.riff".into(), frames: 103 };
        assert_eq!(event.voices, BTreeMap::from([(0, voice)]));
        assert_eq!(event.script.symbol("async_event"), Some(event.entry));
        // A name the list has with no bundle behind it is no event.
        assert!(EventData::load(&data, &tables, 101).unwrap().is_none());
        assert!(EventData::load(&data, &tables, 9999).unwrap().is_none());
    }
}
