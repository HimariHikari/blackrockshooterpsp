//! Unpacking: peels compression and splits archives into folders, and counts what
//! parses of every known format.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use brs_formats::{clim, container, insa, insm, ptmd, stcm};

/// What is asked of the unpacking.
pub struct Options {
    /// Write a PNG for every texture found.
    pub png: bool,
    /// Take the `PDK` packs apart too (the nine of the ride on the bike). The game
    /// reads those as they are, each one file, and a pack of them that is itself
    /// a pack it wants whole: so this is for looking into them by hand, into a
    /// folder the game is not run from.
    pub split_pdk: bool,
}

#[derive(Default)]
pub struct Stats {
    pub source_files: usize,
    pub archives: usize,
    pub textures: usize,
    pub collisions: usize,
    pub collision_triangles: usize,
    pub collision_failures: BTreeMap<String, usize>,
    /// `LPK` blobs that are not a whole pack, counted by reason.
    pub pack_failures: BTreeMap<String, usize>,
    pub models: usize,
    pub model_triangles: usize,
    /// `INSM` blobs that did not parse as a model, counted by reason.
    pub model_failures: BTreeMap<String, usize>,
    pub maps: usize,
    pub map_triangles: usize,
    pub map_failures: BTreeMap<String, usize>,
    pub animations: usize,
    pub animation_failures: BTreeMap<String, usize>,
    /// Greatest common divisor of every keyframe time, to reveal the time step.
    pub key_time_gcd: u32,
    pub longest_animation: u32,
    /// Leaf blobs counted by their 4-byte magic.
    pub magics: BTreeMap<String, usize>,
    pub errors: Vec<String>,
}

/// Every file under a folder.
pub fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
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

/// Writes `data` to `out`, recursing into it if it is a wrapper or archive.
/// Archives become directories; everything else becomes a file.
pub fn process(data: &[u8], out: &Path, options: &Options, stats: &mut Stats) -> Result<()> {
    if let Some(inner) = container::unwrap(data)? {
        return process(&inner, out, options, stats);
    }

    if data.starts_with(b"RTDP") {
        stats.archives += 1;
        fs::create_dir_all(out)?;
        let mut used = HashSet::new();
        for (i, entry) in container::parse_rtdp(data)?.iter().enumerate() {
            let mut name = entry.name.replace(['/', '\\'], "_");
            // An archive may hold two members with the same name (case-insensitively).
            if !used.insert(name.to_ascii_lowercase()) {
                name = format!("{i:03}_{name}");
            }
            process(&entry.data, &out.join(name), options, stats).with_context(|| entry.name.clone())?;
        }
        return Ok(());
    }

    if data.starts_with(b"LPK\0") {
        match container::parse_lpk(data) {
            Ok(entries) => {
                stats.archives += 1;
                fs::create_dir_all(out)?;
                for (i, entry) in entries.iter().enumerate() {
                    let path = out.join(format!("{i:03}.{}", guess_extension(entry)));
                    process(entry, &path, options, stats).with_context(|| format!("entry {i}"))?;
                }
                return Ok(());
            }
            // Some packs are a bare header (an entry count and nothing after it); such
            // a one is kept as the file it is.
            Err(e) => *stats.pack_failures.entry(e.to_string()).or_default() += 1,
        }
    }

    if options.split_pdk && data.starts_with(container::PDK_MAGIC) {
        if let Ok(members) = container::parse_pdk(data) {
            stats.archives += 1;
            fs::create_dir_all(out)?;
            // A member's number is what the game finds it by, so it is kept.
            for (i, member) in members.iter().enumerate().filter(|(_, member)| !member.is_empty()) {
                let path = out.join(format!("{i:03}.{}", guess_extension(member)));
                process(member, &path, options, stats).with_context(|| format!("member {i}"))?;
            }
            return Ok(());
        }
    }

    if let Some(slots) = container::parse_sc(data) {
        stats.archives += 1;
        fs::create_dir_all(out)?;
        // Slot numbers are meaningful (each has a fixed role), so keep them in the names.
        for (i, slot) in slots.iter().enumerate().filter(|(_, slot)| !slot.is_empty()) {
            let path = out.join(format!("{i:02}.{}", guess_extension(slot)));
            process(slot, &path, options, stats).with_context(|| format!("slot {i}"))?;
        }
        return Ok(());
    }

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, data)?;
    *stats.magics.entry(magic_label(data)).or_default() += 1;

    if data.starts_with(insm::MAGIC) {
        match insm::Model::parse(data) {
            Ok(model) => {
                stats.models += 1;
                stats.model_triangles += model.submeshes.iter().map(|s| s.indices.len() / 3).sum::<usize>();
            }
            // Offsets differ per file; keep only the outermost message so reasons group.
            Err(e) => *stats.model_failures.entry(e.to_string()).or_default() += 1,
        }
    }

    if data.starts_with(stcm::MAGIC) {
        match stcm::Map::parse(data) {
            Ok(map) => {
                stats.maps += 1;
                stats.map_triangles += map.batches.iter().map(|b| b.indices.len() / 3).sum::<usize>();
            }
            Err(e) => *stats.map_failures.entry(e.to_string()).or_default() += 1,
        }
    }

    if data.starts_with(insa::MAGIC) {
        match insa::Animation::parse(data) {
            Ok(animation) => {
                stats.animations += 1;
                stats.longest_animation = stats.longest_animation.max(animation.end.saturating_sub(animation.start));
                for node in &animation.nodes {
                    for time in node.key_times() {
                        stats.key_time_gcd = gcd(stats.key_time_gcd, time);
                    }
                }
            }
            Err(e) => *stats.animation_failures.entry(format!("{e:#}")).or_default() += 1,
        }
    }

    if data.starts_with(clim::MAGIC) {
        match clim::Collision::parse(data) {
            Ok(collision) => {
                stats.collisions += 1;
                stats.collision_triangles += collision.triangles.len();
            }
            Err(e) => *stats.collision_failures.entry(e.to_string()).or_default() += 1,
        }
    }

    // With `--png`, a bare texture gets a sibling PNG and textures embedded in other
    // formats (models, maps, effects) go into a `<name>.tex` directory.
    let textures = ptmd::scan(data);
    stats.textures += textures.len();
    if options.png {
        let bare = textures.len() == 1 && textures[0].0 == 0;
        for (i, (offset, header)) in textures.iter().enumerate() {
            let path = if bare {
                append_extension(out, "png")
            } else {
                append_extension(out, "tex").join(format!("{i:03}.png"))
            };
            write_png(&path, header, &ptmd::decode(&data[*offset..], header))?;
        }
    }
    Ok(())
}

fn write_png(path: &Path, header: &ptmd::Header, rgba: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = BufWriter::new(fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, header.width as u32, header.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn append_extension(path: &Path, ext: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

fn magic_label(data: &[u8]) -> String {
    let magic = &data[..data.len().min(4)];
    if magic.len() == 4 && magic.iter().all(|b| b.is_ascii_graphic() || *b == 0) {
        String::from_utf8_lossy(magic).trim_end_matches('\0').to_owned()
    } else {
        "(other)".to_owned()
    }
}

/// File extension for an unnamed LPK member, from its magic.
fn guess_extension(data: &[u8]) -> String {
    let label = magic_label(data);
    match label.as_str() {
        "PTMD" => "ptm".to_owned(),
        "(other)" | "" => "bin".to_owned(),
        _ if label.bytes().all(|b| b.is_ascii_alphanumeric()) => label.to_ascii_lowercase(),
        _ => "bin".to_owned(),
    }
}
