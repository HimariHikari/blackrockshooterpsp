//! Lists a layout of the interface for reading by hand: its objects, the groups
//! they are in, the sets of tracks that play them and the clips those play.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use brs_formats::layout::{Ease, Groups, Mark, Motions, Objects};

pub fn list(objects: &Path, groups: &Path, motions: &Path) -> Result<()> {
    let read = |path: &Path| fs::read(path).with_context(|| format!("reading {}", path.display()));
    let objects = Objects::parse(&read(objects)?).context("the objects")?;
    let groups = Groups::parse(&read(groups)?).context("the groups")?;
    let motions = Motions::parse(&read(motions)?).context("the motions")?;
    println!(
        "{} objects with {} textures, {} groups, {} sets, {} clips",
        objects.objects.len(),
        objects.textures,
        groups.groups.len(),
        motions.sets.len(),
        motions.clips.len()
    );

    println!("objects:");
    for (index, object) in objects.objects.iter().enumerate() {
        let [x, y] = object.position;
        let [w, h] = object.size;
        let colour = groups.colours.get(index).map(|c| format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])).unwrap_or_default();
        let texture = object.texture.map(|texture| format!("t{texture}")).unwrap_or_else(|| "-".into());
        let mut line = format!("  {index:3}: {texture:>3} at {x:4},{y:4} size {w:4}x{h:<4} colour {colour}");
        match &object.quad {
            None => line.push_str(" (a place)"),
            Some(quad) => {
                let (first, last) = (quad.corners[0], quad.corners[3]);
                line.push_str(&format!(
                    " corners {},{}..{},{} uv {:.4},{:.4}..{:.4},{:.4}{}{}{}",
                    first.position[0],
                    first.position[1],
                    last.position[0],
                    last.position[1],
                    first.uv[0],
                    first.uv[1],
                    last.uv[0],
                    last.uv[1],
                    if quad.textured { "" } else { " plain" },
                    if quad.smooth { "" } else { " sharp" },
                    if quad.additive { " added" } else { "" }
                ));
                let colours: Vec<[u8; 4]> = quad.corners.iter().map(|corner| corner.colour).collect();
                if colours.iter().any(|colour| *colour != [255; 4]) {
                    let shown: Vec<String> = colours.iter().map(|c| format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])).collect();
                    line.push_str(&format!(" corner colours {}", shown.join(" ")));
                }
            }
        }
        println!("{line}");
    }

    println!("groups:");
    for (index, members) in groups.groups.iter().enumerate() {
        println!("  {index:3}: {members:?}");
    }

    let length = |clip: usize| motions.clips[clip].iter().filter_map(|(_, keys)| keys.last()).map(|key| key.frame).max().unwrap_or(0);
    println!("sets:");
    for (index, tracks) in motions.sets.iter().enumerate() {
        let whole = tracks.iter().map(|track| track.delay as u32 + length(track.clip as usize)).max().unwrap_or(0);
        println!("  set {index} ({index:#x}): {} tracks, {whole} frames", tracks.len());
        for (number, track) in tracks.iter().enumerate() {
            println!(
                "    {number:3}: group {:3} clip {:3} ({} frames) after {}{}{}",
                track.group,
                track.clip,
                length(track.clip as usize),
                track.delay,
                if track.offset == [0.0, 0.0] { String::new() } else { format!(" moved {},{}", track.offset[0], track.offset[1]) },
                if track.hidden { " hidden" } else { "" }
            );
        }
    }

    println!("clips:");
    for (index, clip) in motions.clips.iter().enumerate() {
        println!("  clip {index}:");
        for (channel, keys) in clip {
            let keys: Vec<String> = keys
                .iter()
                .map(|key| {
                    let ease = match key.ease {
                        Ease::Even => "",
                        Ease::In => "i",
                        Ease::Out => "o",
                    };
                    let mark = match key.mark {
                        Mark::None => "",
                        Mark::LoopStart => "[",
                        Mark::LoopEnd => "]",
                    };
                    format!("{}{mark}={}{ease}", key.frame, key.value)
                })
                .collect();
            println!("    {channel:?}: {}", keys.join(" "));
        }
    }
    Ok(())
}
