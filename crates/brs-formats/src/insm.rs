//! `INSM` models (`.mdl`, and the `.ZZZ` files in `BTL/WEP` and `BTL/DBG`).
//!
//! A model is a set of tables (materials, submeshes, skeleton nodes, bone bindings)
//! followed by PSP GE display lists, the vertex data they point at, and the textures.
//! See `docs/FORMAT_NOTES.md` for the layout. `.cam` files share the magic but hold
//! camera data: they are rejected here and read by [`crate::cam`].

use anyhow::{bail, ensure, Context, Result};

use crate::ptmd;
use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"INSM";

const MATERIAL_LEN: usize = 0x24;
const SUBMESH_LEN: usize = 8;
const NODE_LEN: usize = 0x20;
const BINDING_LEN: usize = 12;
const PART_LEN: usize = 16;
const NONE: u8 = 0xff;
/// Submesh flag: the engine loads this submesh's node matrix before drawing it.
const SUBMESH_NEW_MATRIX: u8 = 0x02;

/// Most bone weights a PSP vertex can carry.
pub const MAX_WEIGHTS: usize = 8;

#[derive(Clone, Debug)]
pub struct Model {
    pub materials: Vec<Material>,
    pub nodes: Vec<Node>,
    pub bindings: Vec<Binding>,
    pub submeshes: Vec<Submesh>,
    pub parts: Vec<Part>,
    /// File offset and header of each texture.
    pub textures: Vec<(usize, ptmd::Header)>,
}

/// Submeshes the engine shows or hides together, by id. The player's model carries all
/// the things she can hold as parts, and the game shows the ones in use.
#[derive(Clone, Debug)]
pub struct Part {
    pub id: u16,
    /// Indices into [`Model::submeshes`].
    pub submeshes: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
pub struct Material {
    /// Index into [`Model::textures`]; `None` for untextured materials.
    pub texture: Option<usize>,
    /// Render state: the material's display list as `(command, argument)` pairs.
    pub state: Vec<(u8, u32)>,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub flags: u8,
    /// Unknown identifier; `0xff` on most nodes.
    pub id: u8,
    pub parent: Option<usize>,
    /// The first node after this one and all that is under it.
    pub next: Option<usize>,
    /// Relative to the parent node.
    pub translation: [f32; 3],
    /// Quaternion `(x, y, z, w)`, relative to the parent node.
    pub rotation: [f32; 4],
}

impl Node {
    /// Flag: the node has nodes under it.
    const HAS_CHILD: u8 = 0x02;
    /// Flag: the node is part of the skeleton of a skinned model.
    const SKELETON: u8 = 0x08;

    pub fn has_child(&self) -> bool {
        self.flags & Node::HAS_CHILD != 0
    }

    pub fn in_skeleton(&self) -> bool {
        self.flags & Node::SKELETON != 0
    }
}

/// Ties a node to the submeshes it moves.
#[derive(Clone, Debug)]
pub struct Binding {
    pub node: usize,
    /// Raw 4x4 matrix as stored; present when the node skins vertices.
    pub inverse_bind: Option<[f32; 16]>,
}

#[derive(Clone, Debug)]
pub struct Submesh {
    pub flags: u8,
    pub material: usize,
    /// Back-face culling requested by the display list.
    pub cull: bool,
    /// `true`: vertices are in the space of `nodes[0]` and carry no weights.
    /// `false`: vertices are in model space; weight `i` applies to `nodes[i]`.
    pub rigid: bool,
    /// Node indices, one per matrix slot.
    pub nodes: Vec<usize>,
    /// The vertices carry colours of their own: those of a submesh whose do not
    /// are read as white.
    pub coloured: bool,
    pub vertices: Vec<Vertex>,
    /// Triangle list.
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub weights: [f32; MAX_WEIGHTS],
}

impl Model {
    pub fn parse(d: &[u8]) -> Result<Model> {
        ensure!(d.starts_with(MAGIC), "not an INSM file");
        let r = Reader(d);
        ensure!(r.u8(4)? == 1 && r.u8(7)? == 0, "INSM file is not a model (camera data?)");

        let texture_count = r.u8(5)? as usize;
        let material_count = r.u8(6)? as usize;
        let node_count = r.u8(8)? as usize;
        let binding_count = r.u8(9)? as usize;
        let part_count = r.u8(10)? as usize;
        let submesh_count = r.u32(0x0c)? as usize;
        let materials_at = r.u32(0x10)? as usize;
        let submeshes_at = r.u32(0x14)? as usize;
        let nodes_at = r.u32(0x18)? as usize;
        let bindings_at = r.u32(0x1c)? as usize;
        let parts_at = r.u32(0x20)? as usize;
        let textures_at = r.u32(0x24)? as usize;

        let mut materials = Vec::with_capacity(material_count);
        for i in 0..material_count {
            let at = materials_at + i * MATERIAL_LEN;
            let state = read_commands(&r, r.u32(at + 4)? as usize).with_context(|| format!("material {i}"))?;
            let texture = (r.u8(at)? != 0).then_some(r.u8(at + 1)? as usize);
            ensure!(texture.is_none_or(|t| t < texture_count), "material {i} uses texture {texture:?} of {texture_count}");
            materials.push(Material { texture, state });
        }

        let nodes = read_nodes(&r, nodes_at, node_count)?;

        // Each binding lists where its node's matrix goes in a command buffer the engine
        // builds at load time. That buffer is laid out in submesh order, so sorting the
        // positions recovers which node drives each submesh's matrix slots.
        let mut bindings = Vec::with_capacity(binding_count);
        let mut slots = Vec::new();
        for i in 0..binding_count {
            let at = bindings_at + i * BINDING_LEN;
            let skinned = r.u8(at)? as usize;
            let rigid = r.u8(at + 1)? as usize;
            let node = r.u16(at + 2)? as usize;
            ensure!(node < node_count, "binding {i} refers to node {node} of {node_count}");
            let list = r.u32(at + 4)? as usize;
            for k in 0..skinned + rigid {
                slots.push((r.u32(list + k * 4)?, node));
            }
            let inverse_bind = if skinned > 0 { Some(r.f32s(r.u32(at + 8)? as usize)?) } else { None };
            bindings.push(Binding { node, inverse_bind });
        }
        slots.sort_unstable();
        let mut slots = slots.into_iter().map(|(_, node)| node);

        let mut submeshes: Vec<Submesh> = Vec::with_capacity(submesh_count);
        for i in 0..submesh_count {
            let at = submeshes_at + i * SUBMESH_LEN;
            let flags = r.u8(at)?;
            let bone_count = r.u8(at + 1)? as usize;
            let material = r.u8(at + 2)? as usize;
            ensure!(material < material_count, "submesh {i} uses material {material} of {material_count}");
            // A skinned submesh loads one matrix per bone. A rigid one loads its node's
            // matrix only when flagged, and otherwise is drawn with the previous one.
            let slot_count = if bone_count > 0 { bone_count } else { (flags & SUBMESH_NEW_MATRIX != 0) as usize };
            let nodes: Vec<usize> = if slot_count > 0 {
                slots.by_ref().take(slot_count).collect()
            } else {
                submeshes.last().filter(|s| s.rigid).map(|s| s.nodes.clone()).unwrap_or_default()
            };
            ensure!(nodes.len() == slot_count.max(1), "submesh {i} has no matrix to draw with");
            let geometry = read_geometry(&r, r.u32(at + 4)? as usize).with_context(|| format!("submesh {i}"))?;
            submeshes.push(Submesh {
                flags,
                material,
                cull: geometry.cull,
                rigid: bone_count == 0,
                nodes,
                coloured: geometry.coloured,
                vertices: geometry.vertices,
                indices: geometry.indices,
            });
        }
        ensure!(slots.next().is_none(), "matrix slots left over after the last submesh");

        let mut parts = Vec::with_capacity(part_count);
        for i in 0..part_count {
            let at = parts_at + i * PART_LEN;
            let first = r.u16(at + 4)? as usize;
            let end = first + r.u16(at + 6)? as usize;
            ensure!(end <= submesh_count, "part {i} covers submeshes {first}..{end} of {submesh_count}");
            parts.push(Part { id: r.u16(at + 2)?, submeshes: first..end });
        }

        let textures: Vec<_> = ptmd::scan(d.get(textures_at..).context("texture offset out of range")?)
            .into_iter()
            .take(texture_count)
            .map(|(offset, header)| (textures_at + offset, header))
            .collect();
        ensure!(textures.len() == texture_count, "found {} of {texture_count} textures", textures.len());

        Ok(Model { materials, nodes, bindings, submeshes, parts, textures })
    }
}

/// The node table: `count` records at `at`. Camera files have one too.
pub(crate) fn read_nodes(r: &Reader, at: usize, count: usize) -> Result<Vec<Node>> {
    let mut nodes = Vec::with_capacity(count);
    for i in 0..count {
        let at = at + i * NODE_LEN;
        let (parent, next) = (r.u8(at + 2)?, r.u8(at + 3)?);
        ensure!(parent == NONE || (parent as usize) < i, "node {i} has parent {parent} at or after itself");
        nodes.push(Node {
            flags: r.u8(at)?,
            id: r.u8(at + 1)?,
            parent: (parent != NONE).then_some(parent as usize),
            next: (next != NONE).then_some(next as usize),
            translation: r.f32s(at + 4)?,
            rotation: r.f32s(at + 16)?,
        });
    }
    Ok(nodes)
}

// PSP GE commands that appear in model and map display lists.
pub(crate) const GE_NOP: u8 = 0x00;
pub(crate) const GE_VADDR: u8 = 0x01;
pub(crate) const GE_IADDR: u8 = 0x02;
pub(crate) const GE_PRIM: u8 = 0x04;
pub(crate) const GE_CALL: u8 = 0x0a;
pub(crate) const GE_RET: u8 = 0x0b;
pub(crate) const GE_END: u8 = 0x0c;
pub(crate) const GE_BASE: u8 = 0x10;
pub(crate) const GE_VTYPE: u8 = 0x12;
pub(crate) const GE_ORIGIN: u8 = 0x14;
pub(crate) const GE_CULL_ENABLE: u8 = 0x1d;

pub(crate) const PRIM_TRIANGLES: u32 = 3;
pub(crate) const PRIM_TRIANGLE_STRIP: u32 = 4;

/// Longest display list accepted, in commands; guards against running off into data.
pub(crate) const MAX_COMMANDS: usize = 1 << 16;

pub(crate) fn split(word: u32) -> (u8, u32) {
    ((word >> 24) as u8, word & 0x00ff_ffff)
}

/// Reads a display list up to and including its `RET`.
pub(crate) fn read_commands(r: &Reader, start: usize) -> Result<Vec<(u8, u32)>> {
    let mut out = Vec::new();
    for i in 0..MAX_COMMANDS {
        let (cmd, arg) = split(r.u32(start + i * 4)?);
        if matches!(cmd, GE_RET | GE_END) {
            return Ok(out);
        }
        out.push((cmd, arg));
    }
    bail!("display list at {start:#x} has no RET");
}

struct Geometry {
    cull: bool,
    /// Every primitive drawn had colours in its vertices.
    coloured: bool,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

/// Runs a submesh display list, collecting everything it draws as one triangle list.
fn read_geometry(r: &Reader, start: usize) -> Result<Geometry> {
    let mut geometry = Geometry { cull: false, coloured: true, vertices: Vec::new(), indices: Vec::new() };
    // Addresses are `BASE | argument`, relative to the list's ORIGIN command.
    let (mut origin, mut base) = (start, 0usize);
    let (mut vertex_at, mut index_at) = (0usize, 0usize);
    let mut vertex_type = 0u32;

    for i in 0..MAX_COMMANDS {
        let pc = start + i * 4;
        let (cmd, arg) = split(r.u32(pc)?);
        match cmd {
            GE_NOP => {}
            GE_ORIGIN => origin = pc,
            GE_BASE => base = ((arg as usize) << 8) & 0xff00_0000,
            GE_VTYPE => vertex_type = arg,
            GE_VADDR => vertex_at = origin + (base | arg as usize),
            GE_IADDR => index_at = origin + (base | arg as usize),
            GE_CULL_ENABLE => geometry.cull = arg & 1 != 0,
            GE_PRIM => {
                let count = (arg & 0xffff) as usize;
                let layout = VertexLayout::new(vertex_type)?;
                geometry.coloured &= (vertex_type >> 2) & 7 != 0;
                let first = geometry.vertices.len() as u32;
                for k in 0..count {
                    let index = match layout.index_size {
                        0 => k,
                        1 => r.u8(index_at + k)? as usize,
                        2 => r.u16(index_at + k * 2)? as usize,
                        _ => r.u32(index_at + k * 4)? as usize,
                    };
                    geometry.vertices.push(layout.read(r, vertex_at + index * layout.size)?);
                }
                match (arg >> 16) & 7 {
                    PRIM_TRIANGLES => geometry.indices.extend(first..first + (count - count % 3) as u32),
                    PRIM_TRIANGLE_STRIP => {
                        for k in 0..count.saturating_sub(2) as u32 {
                            // Every other strip triangle is wound the opposite way.
                            let (a, b) = if k % 2 == 0 { (k, k + 1) } else { (k + 1, k) };
                            geometry.indices.extend([first + a, first + b, first + k + 2]);
                        }
                    }
                    other => bail!("unsupported primitive type {other} at {pc:#x}"),
                }
                // The GE advances whichever pointer the primitive consumed.
                if layout.index_size == 0 {
                    vertex_at += count * layout.size;
                } else {
                    index_at += count * layout.index_size;
                }
            }
            GE_RET | GE_END => return Ok(geometry),
            other => bail!("unexpected GE command {other:#04x} at {pc:#x}"),
        }
    }
    bail!("display list at {start:#x} has no RET");
}

/// Byte layout of one vertex for a GE vertex type.
///
/// Components are stored in the order weights, texture coordinates, colour, normal,
/// position; each is aligned to its element size and the vertex to the largest of them.
pub(crate) struct VertexLayout {
    vertex_type: u32,
    pub(crate) size: usize,
    pub(crate) index_size: usize,
    weight_count: usize,
    weights_at: usize,
    uv_at: usize,
    color_at: usize,
    normal_at: usize,
    position_at: usize,
}

/// Element size in bytes for the 2-bit numeric format fields (0 = absent).
const ELEMENT_SIZE: [usize; 4] = [0, 1, 2, 4];

impl VertexLayout {
    pub(crate) fn new(vertex_type: u32) -> Result<Self> {
        let field = |shift: u32, mask: u32| ((vertex_type >> shift) & mask) as usize;
        let (uv, color, normal, position, weight) = (field(0, 3), field(2, 7), field(5, 3), field(7, 3), field(9, 3));
        ensure!(position != 0, "vertex type {vertex_type:#x} has no position");
        ensure!(field(18, 7) == 0, "vertex type {vertex_type:#x} uses morph targets");
        let color_size = match color {
            0 => 0,
            4..=6 => 2,
            7 => 4,
            _ => bail!("vertex type {vertex_type:#x} has unknown colour format {color}"),
        };

        let (mut offset, mut align) = (0usize, 1usize);
        let mut place = |size: usize, count: usize| {
            if size == 0 {
                return 0;
            }
            let at = offset.next_multiple_of(size);
            offset = at + size * count;
            align = align.max(size);
            at
        };
        let weight_count = if weight == 0 { 0 } else { field(14, 7) + 1 };
        let weights_at = place(ELEMENT_SIZE[weight], weight_count);
        let uv_at = place(ELEMENT_SIZE[uv], 2);
        let color_at = place(color_size, 1);
        let normal_at = place(ELEMENT_SIZE[normal], 3);
        let position_at = place(ELEMENT_SIZE[position], 3);

        Ok(Self {
            vertex_type,
            size: offset.next_multiple_of(align),
            index_size: ELEMENT_SIZE[field(11, 3)],
            weight_count,
            weights_at,
            uv_at,
            color_at,
            normal_at,
            position_at,
        })
    }

    pub(crate) fn read(&self, r: &Reader, at: usize) -> Result<Vertex> {
        let field = |shift: u32, mask: u32| (self.vertex_type >> shift) & mask;
        let mut v = Vertex { color: [255; 4], ..Default::default() };

        for i in 0..self.weight_count {
            v.weights[i] = match field(9, 3) {
                1 => r.u8(at + self.weights_at + i)? as f32 / 128.0,
                2 => r.u16(at + self.weights_at + i * 2)? as f32 / 32768.0,
                _ => r.f32(at + self.weights_at + i * 4)?,
            };
        }
        for i in 0..2 {
            v.uv[i] = match field(0, 3) {
                0 => 0.0,
                1 => r.u8(at + self.uv_at + i)? as f32 / 128.0,
                2 => r.u16(at + self.uv_at + i * 2)? as f32 / 32768.0,
                _ => r.f32(at + self.uv_at + i * 4)?,
            };
        }
        v.color = match field(2, 7) {
            0 => [255; 4],
            7 => r.bytes(at + self.color_at)?,
            format => {
                let c = r.u16(at + self.color_at)?;
                let channel = |shift: u32, bits: u32| {
                    let max = (1u32 << bits) - 1;
                    ((c as u32 >> shift & max) * 255 / max) as u8
                };
                match format {
                    4 => [channel(0, 5), channel(5, 6), channel(11, 5), 255],
                    5 => [channel(0, 5), channel(5, 5), channel(10, 5), channel(15, 1)],
                    _ => [channel(0, 4), channel(4, 4), channel(8, 4), channel(12, 4)],
                }
            }
        };
        for i in 0..3 {
            v.normal[i] = match field(5, 3) {
                0 => 0.0,
                1 => r.u8(at + self.normal_at + i)? as i8 as f32 / 127.0,
                2 => r.u16(at + self.normal_at + i * 2)? as i16 as f32 / 32767.0,
                _ => r.f32(at + self.normal_at + i * 4)?,
            };
            v.position[i] = match field(7, 3) {
                1 => r.u8(at + self.position_at + i)? as i8 as f32 / 127.0,
                2 => r.u16(at + self.position_at + i * 2)? as i16 as f32 / 32767.0,
                _ => r.f32(at + self.position_at + i * 4)?,
            };
        }
        Ok(v)
    }
}
