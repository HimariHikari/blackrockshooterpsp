//! Parses every file of a known format under an unpacked data folder and reports how
//! many of each parsed, with the reasons for any that did not. Writes nothing.
//!
//! Usage: `cargo run --release -p brs-formats --example parse_all [-- <unpacked folder>]`

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use brs_formats::{at3, bscr, bxcb, cam, chunks, clim, edxd, efc, egtb, esb, font, insa, insm, layout, pphd, ptmd, sbs, stcm, xtc};

const DEFAULT_ROOT: &str = "game/unpacked";

#[derive(Default)]
struct Count {
    parsed: usize,
    failures: BTreeMap<String, usize>,
}

fn main() -> Result<()> {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| DEFAULT_ROOT.into()));
    let mut files = Vec::new();
    collect_files(&root, &mut files)?;

    let mut counts: BTreeMap<&str, Count> = BTreeMap::new();
    // The sounds of the banks: how many, how many of them loop, and their seconds.
    let mut sounds = (0usize, 0usize, 0.0f64);
    // The effects of the effect files: their scripts, emitters, models and textures.
    let mut effects = [0usize; 4];
    for path in &files {
        let data = fs::read(path)?;
        let result = if data.starts_with(efc::MAGIC) || data.starts_with(efc::MAGIC_OF_MANY) {
            // The whole of the reason: what is wrong in a script is at the end of it.
            ("effect files", effect_file(&data, &mut effects).map_err(|error| anyhow::anyhow!("{error:#}")))
        } else if cam::is_camera(&data) {
            ("cameras", cam::Camera::parse(&data).map(drop))
        } else if data.starts_with(insm::MAGIC) {
            ("models", insm::Model::parse(&data).map(drop))
        } else if data.starts_with(stcm::MAGIC) {
            ("maps", stcm::Map::parse(&data).map(drop))
        } else if data.starts_with(insa::MAGIC) {
            ("animations", insa::Animation::parse(&data).map(drop))
        } else if data.starts_with(clim::MAGIC) {
            ("collision", clim::Collision::parse(&data).map(drop))
        } else if data.starts_with(bxcb::MAGIC) {
            ("tables", bxcb::Table::parse(&data).map(drop))
        } else if data.starts_with(bscr::MAGIC) {
            ("battle scripts", bscr::Script::parse(&data).map(drop))
        } else if data.starts_with(egtb::MAGIC) {
            ("enemy groups", egtb::Groups::parse(&data).map(drop))
        } else if data.starts_with(edxd::MAGIC) {
            ("enemy figures", edxd::Figures::parse(&data).map(drop))
        } else if data.starts_with(xtc::MAGIC) {
            ("record tables", xtc::Table::parse(&data).map(drop))
        } else if is_camera_cut(path, &data) {
            ("camera cuts", cam::Camera::parse_cut(&data).map(drop))
        } else if at3::is_recording(&data) {
            ("recordings", at3::Recording::parse(&data).map(drop))
        } else if data.starts_with(pphd::MAGIC) {
            ("sound banks", bank(path, &data, &mut sounds))
        } else if data.starts_with(layout::OBJECTS_MAGIC) {
            ("layout objects", layout::Objects::parse(&data).map(drop))
        } else if data.starts_with(layout::MOTIONS_MAGIC) {
            ("layout motions", layout::Motions::parse(&data).map(drop))
        } else if is_layout_groups(path) {
            ("layout groups", layout::Groups::parse(&data).map(drop))
        } else if in_font_pack(path, &["000.bin", "001.bin"]) {
            ("fonts", font::Font::parse(&data).map(drop))
        } else if in_font_pack(path, &["002.sbs"]) {
            ("icon sheets", font::Icons::parse(&data).map(drop))
        } else if path.file_name().is_some_and(|name| name == "ucs2jis.bin") {
            ("code tables", font::Codes::parse(&data).map(drop))
        } else if path.extension().is_some_and(|extension| extension == "sbs") {
            ("sprite sheets", chunks::find(&data, b"SBS").context("no sprite table").and_then(sbs::parse).map(drop))
        } else {
            continue;
        };
        let count = counts.entry(result.0).or_default();
        match result.1 {
            Ok(()) => count.parsed += 1,
            // Offsets differ per file; keep only the outermost message so reasons group.
            Err(e) => *count.failures.entry(e.to_string()).or_default() += 1,
        }
    }

    println!("{} files under {}", files.len(), root.display());
    println!("{} sounds in the banks, {} of them looping, {:.0} s in all", sounds.0, sounds.1, sounds.2);
    println!("{} effects with {} emitters, {} models and {} textures in the effect files", effects[0], effects[1], effects[2], effects[3]);
    for (format, count) in &counts {
        let failed: usize = count.failures.values().sum();
        println!("{format:<14}: {} parsed, {failed} failed", count.parsed);
        for (reason, n) in &count.failures {
            println!("  {n:6}  {reason}");
        }
    }
    Ok(())
}

/// Parses a bank's header and decodes every sound of it from the data that goes
/// with it.
fn bank(path: &Path, header: &[u8], sounds: &mut (usize, usize, f64)) -> Result<()> {
    let bank = pphd::Bank::parse(header)?;
    let data = pphd::data_file(path).context("a bank with no data")?;
    let data = fs::read(&data).with_context(|| format!("the bank's data, {}", data.display()))?;
    for number in bank.sounds() {
        let sound = bank.decode(number, &data)?;
        anyhow::ensure!(!sound.samples.is_empty(), "sound {number} is empty");
        sounds.0 += 1;
        sounds.1 += usize::from(sound.looped.is_some());
        sounds.2 += sound.samples.len() as f64 / f64::from(sound.rate);
    }
    Ok(())
}

/// Parses an effect file and every part of it: its scripts, its models with their
/// animations, and its textures.
fn effect_file(data: &[u8], counts: &mut [usize; 4]) -> Result<()> {
    let file = efc::File::parse(data)?;
    for part in &file.parts {
        let bytes = &data[part.data.clone()];
        match part.kind {
            efc::Kind::Script => {
                let script = esb::Script::parse(bytes, file.version).with_context(|| format!("the script {}", part.name))?;
                counts[0] += 1;
                counts[1] += script.entries.iter().filter(|entry| entry.emitter.is_some()).count();
            }
            efc::Kind::Model => {
                insm::Model::parse(bytes).with_context(|| format!("the model {}", part.name))?;
                counts[2] += 1;
            }
            efc::Kind::Animation => {
                insa::Animation::parse(bytes).with_context(|| format!("the animation {}", part.name))?;
            }
            efc::Kind::Texture => {
                let header = ptmd::parse_header(bytes).with_context(|| format!("the texture {}", part.name))?;
                anyhow::ensure!(header.total_len() <= bytes.len(), "the texture {} runs off its part", part.name);
                counts[3] += 1;
            }
        }
    }
    Ok(())
}

/// The groups of a layout: the file after its objects and its motions.
fn is_layout_groups(path: &Path) -> bool {
    let number = path.file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse::<usize>().ok());
    let (Some(number), Some(folder)) = (number.filter(|number| *number >= 2), path.parent()) else { return false };
    path.extension().is_some_and(|extension| extension == "bin")
        && folder.join(format!("{:03}.xbn", number - 1)).exists()
        && folder.join(format!("{:03}.sdoa", number - 2)).exists()
}

/// One of the named files of the font's pack.
fn in_font_pack(path: &Path, names: &[&str]) -> bool {
    let in_pack = path.parent().and_then(Path::file_name).is_some_and(|folder| folder == "font.lpk");
    in_pack && path.file_name().and_then(|name| name.to_str()).is_some_and(|name| names.contains(&name))
}

/// A cut of an event scene: an entry of slot 3 of an event's bundle that is a bundle
/// itself (the extractor leaves those whole).
fn is_camera_cut(path: &Path, data: &[u8]) -> bool {
    let in_slot_3 = path.parent().and_then(Path::file_name).is_some_and(|folder| folder == "03.bin");
    let in_event = path.components().any(|part| part.as_os_str() == "FEVT");
    in_slot_3 && in_event && data.starts_with(b"SC")
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
