//! `STCM` static map geometry (field and battle maps).
//!
//! A table of embedded textures followed by PSP GE display lists and vertex data. The
//! main list draws triangle strips; before each one it calls a texture list (one per
//! texture, stored in order straight after the main list) and a material list.
//! See `docs/FORMAT_NOTES.md`.

use anyhow::{bail, ensure, Context, Result};

use crate::insm::{
    read_commands, split, Vertex, VertexLayout, GE_BASE, GE_CALL, GE_CULL_ENABLE, GE_END, GE_NOP, GE_ORIGIN, GE_PRIM,
    GE_RET, GE_VADDR, GE_VTYPE, MAX_COMMANDS, PRIM_TRIANGLES, PRIM_TRIANGLE_STRIP,
};
use crate::ptmd;
use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"STCM";

const TEXTURE_TABLE_AT: usize = 0x30;

const GE_TEXTURE_ENABLE: u8 = 0x1e;
const GE_FOG_ENABLE: u8 = 0x1f;
const GE_TEXTURE_OFFSET_U: u8 = 0x4a;
const GE_TEXTURE_OFFSET_V: u8 = 0x4b;
const GE_TEXTURE_ADDRESS: u8 = 0xa0;
const GE_TEXTURE_SIZE: u8 = 0xb8;
const GE_TEXTURE_FUNCTION: u8 = 0xc9;
const GE_ALPHA_TEST: u8 = 0xdb;
const GE_BLEND: u8 = 0xdf;
const GE_BLEND_FIX_SOURCE: u8 = 0xe0;
const GE_BLEND_FIX_DESTINATION: u8 = 0xe1;
const GE_DEPTH_MASK: u8 = 0xe7;

// Texture function: how the texture and the vertex colour combine.
const TEXTURE_FUNCTION_MASK: u32 = 7;
const TEXTURE_FUNCTION_MODULATE: u32 = 0;
const TEXTURE_FUNCTION_ADD: u32 = 4;
const TEXTURE_FUNCTION_ALPHA: u32 = 1 << 8;
const TEXTURE_FUNCTION_DOUBLE: u32 = 1 << 16;

// Alpha test: the comparisons maps use, against a reference in bits 8-15.
const ALPHA_TEST_GREATER: u32 = 6;
const ALPHA_TEST_GREATER_OR_EQUAL: u32 = 7;

// Blend factors, as the source (bits 0-3) and destination (bits 4-7) of the blend
// command, whose equation is in bits 8-10.
const FACTOR_SOURCE_ALPHA: u32 = 2;
const FACTOR_ONE_MINUS_SOURCE_ALPHA: u32 = 3;
const FACTOR_FIXED: u32 = 10;
const EQUATION_ADD: u32 = 0;
const EQUATION_REVERSE_SUBTRACT: u32 = 2;
const FIXED_ONE: u32 = 0xff_ffff;

#[derive(Clone, Debug)]
pub struct Map {
    /// File offset and header of each texture.
    pub textures: Vec<(usize, ptmd::Header)>,
    /// Geometry in the order the map's list draws it: each batch is a run of
    /// strips drawn with one texture, material and culling.
    pub batches: Vec<Batch>,
}

#[derive(Clone, Debug)]
pub struct Batch {
    /// Index into [`Map::textures`]; `None` for untextured geometry.
    pub texture: Option<usize>,
    pub cull: bool,
    pub material: Material,
    /// Positions are in world space. Colours hold baked lighting.
    pub vertices: Vec<Vertex>,
    /// Triangle list.
    pub indices: Vec<u32>,
}

/// How a batch is drawn: the render state its display list sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// The product of texture and vertex colour is doubled, so that a mid-grey vertex
    /// leaves the texture unchanged and a brighter one lights it up.
    pub double_color: bool,
    /// The texture is added to the vertex colour instead of multiplied by it. Four
    /// lists in the whole game do this.
    pub add_texture: bool,
    /// The texture's alpha counts: how much of a point there is is the corner's
    /// alpha by the texture's. Without it the corner's alone says.
    pub texture_alpha: bool,
    pub blend: Blend,
    /// Pixels with less alpha than this are not drawn.
    pub alpha_cutoff: u8,
    /// Distance fog applies.
    pub fog: bool,
    /// Drawing it updates the depth buffer.
    pub depth_write: bool,
    /// Added to every texture coordinate.
    pub uv_offset: [f32; 2],
}

/// How a batch's colour combines with what is already on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    /// Replaces it.
    Opaque,
    /// Covers it by the colour's alpha.
    Alpha,
    /// Is added to it, scaled by the colour's alpha: glows and light shafts.
    Add,
    /// Is subtracted from it, scaled by the colour's alpha. Two lists in the whole game.
    Subtract,
}

impl Default for Material {
    /// What the game's renderer has set before a map's own lists run is not known; these
    /// are the values almost every list sets explicitly.
    fn default() -> Self {
        Material {
            double_color: true,
            add_texture: false,
            texture_alpha: true,
            blend: Blend::Alpha,
            alpha_cutoff: 1,
            fog: true,
            depth_write: true,
            uv_offset: [0.0, 0.0],
        }
    }
}

/// GE commands carry floats as the top 24 bits of an IEEE single.
fn float24(arg: u32) -> f32 {
    f32::from_bits(arg << 8)
}

/// The render state after running a material list on top of `state`. `fixed` holds the
/// blend command's fixed source and destination colours, which a list sets separately.
fn apply_material(state: &mut Material, fixed: &mut (u32, u32), commands: &[(u8, u32)], at: usize) -> Result<()> {
    let mut blend = None;
    for &(cmd, arg) in commands {
        match cmd {
            GE_FOG_ENABLE => state.fog = arg & 1 != 0,
            GE_DEPTH_MASK => state.depth_write = arg & 1 == 0,
            GE_TEXTURE_OFFSET_U => state.uv_offset[0] = float24(arg),
            GE_TEXTURE_OFFSET_V => state.uv_offset[1] = float24(arg),
            GE_TEXTURE_FUNCTION => {
                state.add_texture = match arg & TEXTURE_FUNCTION_MASK {
                    TEXTURE_FUNCTION_MODULATE => false,
                    TEXTURE_FUNCTION_ADD => true,
                    other => bail!("list at {at:#x} uses texture function {other}"),
                };
                state.texture_alpha = arg & TEXTURE_FUNCTION_ALPHA != 0;
                state.double_color = arg & TEXTURE_FUNCTION_DOUBLE != 0;
            }
            GE_ALPHA_TEST => {
                let reference = ((arg >> 8) & 0xff) as u8;
                state.alpha_cutoff = match arg & 7 {
                    ALPHA_TEST_GREATER => reference.saturating_add(1),
                    ALPHA_TEST_GREATER_OR_EQUAL => reference,
                    other => bail!("list at {at:#x} uses alpha test {other}"),
                };
            }
            GE_BLEND => blend = Some(arg),
            GE_BLEND_FIX_SOURCE => fixed.0 = arg,
            GE_BLEND_FIX_DESTINATION => fixed.1 = arg,
            _ => {}
        }
    }
    if let Some(arg) = blend {
        let (source, destination, equation) = (arg & 0xf, (arg >> 4) & 0xf, (arg >> 8) & 7);
        state.blend = match (source, destination, equation) {
            (FACTOR_SOURCE_ALPHA, FACTOR_ONE_MINUS_SOURCE_ALPHA, EQUATION_ADD) => Blend::Alpha,
            (FACTOR_SOURCE_ALPHA, FACTOR_ONE_MINUS_SOURCE_ALPHA, EQUATION_REVERSE_SUBTRACT) => Blend::Subtract,
            (FACTOR_FIXED, FACTOR_FIXED, EQUATION_ADD) if *fixed == (FIXED_ONE, 0) => Blend::Opaque,
            (FACTOR_SOURCE_ALPHA, FACTOR_FIXED, EQUATION_ADD) if fixed.1 == FIXED_ONE => Blend::Add,
            _ => bail!("list at {at:#x} blends with {arg:#x} and fixed colours {fixed:x?}"),
        };
    }
    Ok(())
}

impl Map {
    pub fn parse(d: &[u8]) -> Result<Map> {
        ensure!(d.starts_with(MAGIC), "not an STCM file");
        let r = Reader(d);
        let texture_count = r.u8(5)? as usize;
        let list_at = r.u32(0x10)? as usize;

        let mut textures = Vec::with_capacity(texture_count);
        for i in 0..texture_count {
            let at = r.u32(TEXTURE_TABLE_AT + i * 4)? as usize;
            let header = d.get(at..).and_then(ptmd::parse_header);
            textures.push((at, header.with_context(|| format!("texture {i} at {at:#x} is not a PTMD"))?));
        }

        // Batches are keyed by the address of the texture list in force, which is only
        // resolvable to an index once the end of the main list is known.
        let mut batches: Vec<(Option<usize>, Batch)> = Vec::new();
        let (mut origin, mut base) = (list_at, 0usize);
        let (mut vertex_at, mut vertex_type) = (0usize, 0u32);
        let (mut cull, mut textured, mut texture_list) = (false, true, None);
        let (mut material, mut fixed) = (Material::default(), (FIXED_ONE, 0));
        let mut main_end = None;

        for i in 0..MAX_COMMANDS {
            let pc = list_at + i * 4;
            let (cmd, arg) = split(r.u32(pc)?);
            match cmd {
                GE_NOP => {}
                GE_ORIGIN => origin = pc,
                GE_BASE => base = ((arg as usize) << 8) & 0xff00_0000,
                GE_VTYPE => vertex_type = arg,
                GE_VADDR => vertex_at = origin + (base | arg as usize),
                GE_CULL_ENABLE => cull = arg & 1 != 0,
                GE_CALL => {
                    let target = origin + (base | arg as usize);
                    let commands = read_commands(&r, target).with_context(|| format!("list called at {pc:#x}"))?;
                    // A called list either selects a texture or sets the material state.
                    if commands.iter().any(|(c, _)| *c == GE_TEXTURE_ADDRESS) {
                        texture_list = Some(target);
                    } else {
                        if let Some((_, a)) = commands.iter().find(|(c, _)| *c == GE_TEXTURE_ENABLE) {
                            textured = a & 1 != 0;
                        }
                        apply_material(&mut material, &mut fixed, &commands, target)?;
                    }
                }
                GE_PRIM => {
                    let count = (arg & 0xffff) as usize;
                    let layout = VertexLayout::new(vertex_type)?;
                    ensure!(layout.index_size == 0, "indexed primitive at {pc:#x}");
                    let key = if textured { texture_list } else { None };
                    let same = |(k, b): &(Option<usize>, Batch)| *k == key && b.cull == cull && b.material == material;
                    // What is drawn is kept in the order the list draws it: a strip
                    // goes with the one before it if it is drawn the same way, and
                    // begins a batch of its own if not, though an earlier batch be
                    // drawn so. The PSP draws them one after another, and what a
                    // see-through strip is mixed with is what was drawn before it.
                    let slot = match batches.last().filter(|last| same(last)).map(|_| batches.len() - 1) {
                        Some(slot) => slot,
                        None => {
                            let batch = Batch { texture: None, cull, material, vertices: Vec::new(), indices: Vec::new() };
                            batches.push((key, batch));
                            batches.len() - 1
                        }
                    };
                    let batch = &mut batches[slot].1;
                    let first = batch.vertices.len() as u32;
                    for k in 0..count {
                        batch.vertices.push(layout.read(&r, vertex_at + k * layout.size)?);
                    }
                    match (arg >> 16) & 7 {
                        PRIM_TRIANGLES => batch.indices.extend(first..first + (count - count % 3) as u32),
                        PRIM_TRIANGLE_STRIP => {
                            for k in 0..count.saturating_sub(2) as u32 {
                                let (a, b) = if k % 2 == 0 { (k, k + 1) } else { (k + 1, k) };
                                batch.indices.extend([first + a, first + b, first + k + 2]);
                            }
                        }
                        other => bail!("unsupported primitive type {other} at {pc:#x}"),
                    }
                    vertex_at += count * layout.size;
                }
                GE_RET | GE_END => {
                    main_end = Some(pc + 4);
                    break;
                }
                other => bail!("unexpected GE command {other:#04x} at {pc:#x}"),
            }
        }
        let main_end = main_end.with_context(|| format!("display list at {list_at:#x} has no RET"))?;

        // Texture lists follow the main list in table order. Their length varies: twelve
        // commands for a paletted texture, eight for a direct-colour one.
        let mut texture_lists = Vec::with_capacity(texture_count);
        let mut at = main_end;
        for _ in 0..texture_count {
            texture_lists.push(at);
            at += (read_commands(&r, at)?.len() + 1) * 4;
        }

        let mut resolved = Vec::with_capacity(batches.len());
        for (key, mut batch) in batches {
            if let Some(list) = key {
                let index = texture_lists.iter().position(|&start| start == list);
                let index = index.with_context(|| format!("texture list at {list:#x} is not one of the {texture_count}"))?;
                // The list states the texture's width; it must agree with the texture itself.
                let size = read_commands(&r, list)?.into_iter().find(|(c, _)| *c == GE_TEXTURE_SIZE);
                let width = size.map(|(_, a)| 1usize << (a & 0xff));
                ensure!(
                    width == Some(textures[index].1.width),
                    "texture list {index} says width {width:?}, texture is {} wide",
                    textures[index].1.width
                );
                batch.texture = Some(index);
            }
            resolved.push(batch);
        }
        Ok(Map { textures, batches: resolved })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked/FLD/MAP");

    /// Every map of the game parses, and the few batches that are not laid over what
    /// is there in the ordinary way are told (run with `--nocapture` to see them).
    #[test]
    fn the_maps_of_the_game_parse_and_say_how_their_batches_mix() {
        let Ok(folders) = std::fs::read_dir(MAPS) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut files, mut batches, mut added, mut taken) = (0, 0, 0, 0);
        for folder in folders.map(|entry| entry.unwrap().path()) {
            let Ok(pieces) = std::fs::read_dir(folder.join("11.bin/01.bin")) else { continue };
            for piece in pieces.map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|kind| kind == "stcm")) {
                let data = std::fs::read(&piece).unwrap();
                let map = Map::parse(&data).unwrap_or_else(|error| panic!("{}: {error:#}", piece.display()));
                files += 1;
                batches += map.batches.len();
                for (number, batch) in map.batches.iter().enumerate().filter(|(_, batch)| matches!(batch.material.blend, Blend::Add | Blend::Subtract)) {
                    match batch.material.blend {
                        Blend::Add => added += 1,
                        _ => taken += 1,
                    }
                    let name = folder.file_name().unwrap().to_string_lossy();
                    let file = piece.file_name().unwrap().to_string_lossy();
                    let alpha = batch.vertices.iter().map(|vertex| vertex.color[3]);
                    let colour = batch.vertices.first().map(|vertex| vertex.color);
                    eprintln!(
                        "{name} {file} batch {number}: {:?}, {} triangles, texture {:?}, alpha {:?}..{:?}, first colour {colour:?}, doubled {}, adds its texture {}, fog {}, writes depth {}",
                        batch.material.blend,
                        batch.indices.len() / 3,
                        batch.texture,
                        alpha.clone().min(),
                        alpha.max(),
                        batch.material.double_color,
                        batch.material.add_texture,
                        batch.material.fog,
                        batch.material.depth_write,
                    );
                }
            }
        }
        eprintln!("{files} files, {batches} batches: {added} added to what is there, {taken} taken from it");
        assert!(files >= 83 && batches > files);
    }
}