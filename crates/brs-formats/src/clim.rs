//! `CLIM` collision meshes (slots 3 and 6 of a field map's inner bundle).
//!
//! An octree over a triangle soup. The tree only speeds up lookups, so this reader
//! walks it once and returns the flat triangle list. See `docs/FORMAT_NOTES.md`.

use std::collections::BTreeSet;

use anyhow::{ensure, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"CLIM";

const HEADER_LEN: usize = 0x18;
const NODE_LEN: usize = 0x40;
const GROUP_LEN: usize = 8;
const ATTRIBUTES_AT: usize = 0x14;

#[derive(Clone, Debug, Default)]
pub struct Collision {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<Triangle>,
    /// Four bytes per attribute (0x13EC8): bits (1 the triangle stops nothing and
    /// only marks the place, 2 the next byte counts, 4 the one after it counts),
    /// the kind of ground, which a step's sound is chosen by, and the number of
    /// the scene's light that falls there. The fourth is not read.
    pub attributes: Vec<[u8; 4]>,
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub indices: [u16; 3],
    /// Index into [`Collision::attributes`].
    pub attribute: u8,
}

impl Collision {
    pub fn parse(d: &[u8]) -> Result<Collision> {
        ensure!(d.starts_with(MAGIC), "not a CLIM file");
        ensure!(d.len() >= HEADER_LEN, "file ends inside its header");
        let r = Reader(d);
        let root_at = r.u32(4)? as usize;
        let vertices_at = r.u32(8)? as usize;
        let node_count = r.u16(0x10)? as usize;
        let attribute_count = r.u16(0x12)? as usize;
        ensure!(vertices_at <= d.len() && (d.len() - vertices_at) % 12 == 0, "bad vertex area at {vertices_at:#x}");

        let attributes = (0..attribute_count).map(|i| r.bytes(ATTRIBUTES_AT + i * 4)).collect::<Result<Vec<_>>>()?;
        let vertices =
            (vertices_at..d.len()).step_by(12).map(|at| r.f32s(at)).collect::<Result<Vec<[f32; 3]>>>()?;

        // Node: flags, eight child offsets, offset of its triangle groups, bounding box.
        // The flags' second byte is how many groups the node owns.
        let mut groups = BTreeSet::new();
        let mut pending = vec![root_at];
        let mut visited = 0;
        while let Some(at) = pending.pop() {
            visited += 1;
            ensure!(visited <= node_count, "octree has more than the {node_count} nodes its header declares");
            let group_count = (r.u32(at)? >> 8) as usize;
            let groups_at = r.u32(at + 36)? as usize;
            groups.extend((0..group_count).map(|g| groups_at + g * GROUP_LEN));
            for child in 0..8 {
                let child_at = r.u32(at + 4 + child * 4)? as usize;
                if child_at != 0 {
                    ensure!(child_at + NODE_LEN <= d.len(), "child node at {child_at:#x} is out of range");
                    pending.push(child_at);
                }
            }
        }

        // Group: attribute (high byte), triangle count, offset of u16 index triples.
        let mut triangles = Vec::new();
        for group_at in groups {
            let attribute = r.u8(group_at + 1)?;
            let count = r.u16(group_at + 2)? as usize;
            let indices_at = r.u32(group_at + 4)? as usize;
            for t in 0..count {
                let at = indices_at + t * 6;
                let indices = [r.u16(at)?, r.u16(at + 2)?, r.u16(at + 4)?];
                ensure!(
                    indices.iter().all(|&i| (i as usize) < vertices.len()),
                    "triangle at {at:#x} refers past the {} vertices",
                    vertices.len()
                );
                triangles.push(Triangle { indices, attribute });
            }
        }
        Ok(Collision { vertices, triangles, attributes })
    }

    /// The three corner positions of a triangle.
    pub fn corners(&self, triangle: &Triangle) -> [[f32; 3]; 3] {
        triangle.indices.map(|i| self.vertices[i as usize])
    }
}
