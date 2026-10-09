//! Unpacks the game's `GAMEDATA` tree: peels compression and splits archives, and
//! checks that every file of a known format parses.
//!
//! Usage: `brs-extract [--png] [GAMEDATA dir] [output dir]`
//!        `brs-extract --sound [--jobs <n>] [output dir]`
//!        `brs-extract --layout <objects> <groups> <motions>`
//!
//! `brs-setup`, of this crate too, does all of it from a disc image.
//!
//! `--png` also writes every texture as a PNG for looking at by hand (about 330 MB in
//! 29,000 files). The viewer and the game read the original files and do not need them.
//!
//! `--split-pdk` also takes apart the nine packs of the ride on the bike, which the
//! game reads whole: for looking into them, into a folder the game is not run from.
//!
//! `--sound` does not unpack: it goes through what has been unpacked and has ffmpeg,
//! which must be on the PATH, make an `.ogg` beside each recording (`sound.rs`), and
//! one for the sound of each film (`MOVIE/<NAME>.ogg` under what was unpacked). The
//! game plays those. It can be stopped and run again: it goes on where it left off.
//!
//! `--layout` does not unpack either: it lists a layout of the interface, given its
//! objects (`.sdoa`), its groups (the `.bin` with them) and its motions (`.xbn`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use brs_extract::sound;
use brs_extract::unpack::{collect_files, process, Options, Stats};

mod layout;

const DEFAULT_INPUT: &str = "game/extracted/PSP_GAME/USRDIR/GAMEDATA";
const DEFAULT_OUTPUT: &str = "game/unpacked";
/// How many recordings are converted at once unless `--jobs` says otherwise.
const DEFAULT_JOBS: usize = 3;

/// Lists the batches of a map's geometry: how each is drawn and where it lies.
fn list_map(file: &Path) -> Result<()> {
    let data = fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let map = brs_formats::stcm::Map::parse(&data)?;
    println!("{} textures, {} batches", map.textures.len(), map.batches.len());
    for (number, batch) in map.batches.iter().enumerate() {
        let material = &batch.material;
        let alpha = batch.vertices.iter().map(|vertex| vertex.color[3]);
        let height = batch.vertices.iter().map(|vertex| vertex.position[1]);
        let (low, high) = height.fold((f32::MAX, f32::MIN), |(low, high), y| (low.min(y), high.max(y)));
        let texture = batch.texture.map(|texture| {
            let (at, header) = &map.textures[texture];
            let pixels = brs_formats::ptmd::decode(&data[*at..], header);
            let alphas = pixels.chunks_exact(4).map(|pixel| pixel[3]);
            format!("{texture} ({}x{}, alpha {}..{})", header.width, header.height, alphas.clone().min().unwrap_or(0), alphas.max().unwrap_or(0))
        });
        println!(
            "{number:3}: {:5} triangles, y {low:.0}..{high:.0}, texture {}, {:?}, cut under {}, corner alpha {}..{}{}{}{}{}{}{}",
            batch.indices.len() / 3,
            texture.unwrap_or_else(|| "none".into()),
            material.blend,
            material.alpha_cutoff,
            alpha.clone().min().unwrap_or(0),
            alpha.max().unwrap_or(0),
            if material.double_color { ", twice over" } else { "" },
            if material.add_texture { ", texture added" } else { "" },
            if material.texture_alpha { "" } else { ", texture's alpha not counted" },
            if material.fog { "" } else { ", no fog" },
            if material.depth_write { "" } else { ", no depth written" },
            if batch.cull { ", one side" } else { "" },
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut options = Options { png: false, split_pdk: false };
    let mut positional = Vec::new();
    let (mut sound, mut jobs, mut layout, mut map) = (false, DEFAULT_JOBS, false, false);
    let mut arguments = std::env::args().skip(1);
    while let Some(arg) = arguments.next() {
        match arg.as_str() {
            "--png" => options.png = true,
            "--split-pdk" => options.split_pdk = true,
            "--sound" => sound = true,
            "--layout" => layout = true,
            "--map" => map = true,
            "--jobs" => jobs = arguments.next().context("--jobs needs a number")?.parse().context("--jobs takes a number")?,
            _ if arg.starts_with("--") => bail!("unknown option {arg}"),
            _ => positional.push(arg),
        }
    }
    if map {
        let [file] = &positional[..] else { bail!("--map needs a map's geometry, an .stcm file") };
        return list_map(Path::new(file));
    }
    if layout {
        let [objects, groups, motions] = &positional[..] else { bail!("--layout needs the objects, the groups and the motions") };
        return layout::list(Path::new(objects), Path::new(groups), Path::new(motions));
    }
    if sound {
        let unpacked = PathBuf::from(positional.into_iter().next().unwrap_or_else(|| DEFAULT_OUTPUT.into()));
        return sound::convert(&unpacked, jobs);
    }
    let mut positional = positional.into_iter();
    let input = PathBuf::from(positional.next().unwrap_or_else(|| DEFAULT_INPUT.into()));
    let output = PathBuf::from(positional.next().unwrap_or_else(|| DEFAULT_OUTPUT.into()));

    let mut files = Vec::new();
    collect_files(&input, &mut files).with_context(|| format!("reading {}", input.display()))?;
    files.sort();

    let mut stats = Stats::default();
    for path in &files {
        let rel = path.strip_prefix(&input).unwrap();
        let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        stats.source_files += 1;
        if let Err(e) = process(&data, &output.join(rel), &options, &mut stats) {
            stats.errors.push(format!("{}: {e:#}", rel.display()));
        }
    }

    println!("source files : {}", stats.source_files);
    println!("archives     : {}", stats.archives);
    for (reason, count) in &stats.pack_failures {
        println!("  {count:6}  pack kept whole: {reason}");
    }
    println!("textures     : {}{}", stats.textures, if options.png { " (written as PNG)" } else { "" });
    println!("collision    : {} ({} triangles)", stats.collisions, stats.collision_triangles);
    for (reason, count) in &stats.collision_failures {
        println!("  {count:6}  not parsed: {reason}");
    }
    println!("models       : {} ({} triangles)", stats.models, stats.model_triangles);
    for (reason, count) in &stats.model_failures {
        println!("  {count:6}  not parsed: {reason}");
    }
    println!("maps         : {} ({} triangles)", stats.maps, stats.map_triangles);
    for (reason, count) in &stats.map_failures {
        println!("  {count:6}  not parsed: {reason}");
    }
    println!(
        "animations   : {} (key time step {:#x}, longest {:#x})",
        stats.animations, stats.key_time_gcd, stats.longest_animation
    );
    for (reason, count) in &stats.animation_failures {
        println!("  {count:6}  not parsed: {reason}");
    }
    println!("leaf blobs by magic:");
    let mut magics: Vec<_> = stats.magics.iter().collect();
    magics.sort_by(|a, b| b.1.cmp(a.1));
    for (magic, count) in magics {
        println!("  {count:6}  {magic}");
    }
    println!("errors       : {}", stats.errors.len());
    for e in &stats.errors {
        println!("  {e}");
    }
    Ok(())
}
