//! Counts the commands of the render state that the materials of the models in the
//! effect files carry, with the arguments each is given, and what the models are
//! made of. Writes nothing.
//!
//! Usage: `cargo run --release -p brs-formats --example effect_materials [-- <unpacked folder>]`

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use brs_formats::{efc, insm};

fn main() -> Result<()> {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "game/unpacked".into()));
    let mut files = Vec::new();
    collect_files(&root, &mut files)?;

    let mut commands: BTreeMap<u8, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut lists: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    let (mut models, mut materials, mut untextured, mut skinned, mut submeshes, mut coloured, mut plain) = (0, 0, 0, 0, 0, 0, 0);
    let mut most_nodes = 0;
    for path in &files {
        let data = fs::read(path)?;
        if !(data.starts_with(efc::MAGIC) || data.starts_with(efc::MAGIC_OF_MANY)) {
            continue;
        }
        let file = efc::File::parse(&data)?;
        for part in file.parts.iter().filter(|part| part.kind == efc::Kind::Model) {
            let model = insm::Model::parse(&data[part.data.clone()])?;
            models += 1;
            most_nodes = most_nodes.max(model.nodes.len());
            for material in &model.materials {
                materials += 1;
                untextured += usize::from(material.texture.is_none());
                *lists.entry(material.state.iter().map(|(command, _)| *command).collect()).or_default() += 1;
                for &(command, argument) in &material.state {
                    *commands.entry(command).or_default().entry(argument).or_default() += 1;
                }
            }
            for submesh in &model.submeshes {
                submeshes += 1;
                skinned += usize::from(!submesh.rigid);
                coloured += usize::from(submesh.vertices.iter().any(|vertex| vertex.color != [255; 4]));
                plain += usize::from(!submesh.coloured);
            }
        }
    }
    println!("{models} models with {materials} materials ({untextured} with no texture), {submeshes} submeshes ({skinned} skinned, {coloured} with colours at their corners that are not all white, {plain} with no colours there at all), at most {most_nodes} nodes");
    println!("the commands of the materials' lists:");
    for (list, count) in &lists {
        println!("  {count:6}  {}", list.iter().map(|command| format!("{command:02x}")).collect::<Vec<_>>().join(" "));
    }
    println!("each command's arguments:");
    for (command, arguments) in &commands {
        let shown: Vec<String> = arguments.iter().take(8).map(|(argument, count)| format!("{argument:06x} x{count}")).collect();
        println!("  {command:02x}: {}{}", shown.join(", "), if arguments.len() > 8 { format!(" and {} more", arguments.len() - 8) } else { String::new() });
    }
    Ok(())
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
