//! `CPMD`: the map of a camera's path (the second slot of each bundle in slot 8 of a
//! field map's inner bundle; the camera and its move are the two slots after it).
//!
//! A grid laid over the ground that says, for wherever she stands, how far along its
//! move the camera is: nought at the move's first frame, one at its last. See
//! `docs/FORMAT_NOTES.md`.

use anyhow::{ensure, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"CPMD";

const COLUMNS_AT: usize = 0x08;
const ROWS_AT: usize = 0x0a;
const BOUNDS_AT: usize = 0x0c;
const GRID_AT: usize = 0x1c;
/// What a point of the grid has for the end of the move.
const WHOLE: f32 = 65535.0;

#[derive(Clone, Debug, PartialEq)]
pub struct PathMap {
    /// How many cells the grid is across (along X) and deep (along Z); it has one
    /// more point than that each way.
    pub columns: usize,
    pub rows: usize,
    /// Where the grid begins and ends along X, and along Z.
    pub x: [f32; 2],
    pub z: [f32; 2],
    /// The grid's points row by row, a row running along X.
    pub points: Vec<u16>,
}

impl PathMap {
    pub fn parse(d: &[u8]) -> Result<PathMap> {
        ensure!(d.starts_with(MAGIC), "not a CPMD file");
        let r = Reader(d);
        let (columns, rows) = (r.u16(COLUMNS_AT)? as usize, r.u16(ROWS_AT)? as usize);
        ensure!(columns > 0 && rows > 0, "a grid of {columns} by {rows} cells");
        let [x_from, x_to, z_from, z_to] = r.f32s(BOUNDS_AT)?;
        let grid_at = r.u32(GRID_AT)? as usize;
        let points = (0..(columns + 1) * (rows + 1)).map(|i| r.u16(grid_at + i * 2)).collect::<Result<Vec<u16>>>()?;
        Ok(PathMap { columns, rows, x: [x_from, x_to], z: [z_from, z_to], points })
    }

    /// How far along the move the camera is with her at (`x`, `z`), from 0 to 1
    /// (0x18254): what the four points about her have, each by how near it is, and
    /// beyond the grid what its edge has there.
    pub fn along(&self, x: f32, z: f32) -> f32 {
        let (column, across) = cell(x, self.x, self.columns);
        let (row, deep) = cell(z, self.z, self.rows);
        let point = |row: usize, column: usize| {
            let index = (row * (self.columns + 1) + column).min(self.points.len() - 1);
            self.points[index] as f32 / WHOLE
        };
        let of_row = |row: usize| {
            let from = point(row, column);
            if across == 0.0 { from } else { from + (point(row, column + 1) - from) * across }
        };
        let from = of_row(row);
        if deep == 0.0 { from } else { from + (of_row(row + 1) - from) * deep }
    }
}

/// The grid line at or before `value` along one way of the grid, and how far past
/// it `value` is as a part of a cell. Before the grid that is its first line, and
/// at its end or past it its last.
fn cell(value: f32, [from, to]: [f32; 2], cells: usize) -> (usize, f32) {
    if value <= from {
        return (0, 0.0);
    }
    if value >= to {
        return (cells, 0.0);
    }
    let step = (to - from) / cells as f32;
    let line = ((value - from) / step) as usize;
    (line, (value - (line as f32 * step + from)) / step)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid of two cells by one over X 0..200 and Z 0..50, whose points go from
    /// nought at the near left to the whole at the far right.
    fn file() -> Vec<u8> {
        let mut d = vec![0u8; 0x20];
        d[..4].copy_from_slice(MAGIC);
        d[COLUMNS_AT..COLUMNS_AT + 2].copy_from_slice(&2u16.to_le_bytes());
        d[ROWS_AT..ROWS_AT + 2].copy_from_slice(&1u16.to_le_bytes());
        for (i, bound) in [0.0f32, 200.0, 0.0, 50.0].into_iter().enumerate() {
            d[BOUNDS_AT + i * 4..BOUNDS_AT + i * 4 + 4].copy_from_slice(&bound.to_le_bytes());
        }
        d[GRID_AT..GRID_AT + 4].copy_from_slice(&0x20u32.to_le_bytes());
        for point in [0u16, 0x4000, 0x8000, 0x8000, 0xc000, 0xffff] {
            d.extend(point.to_le_bytes());
        }
        d
    }

    #[test]
    fn a_path_map_says_how_far_along_the_camera_is() {
        let map = PathMap::parse(&file()).unwrap();
        assert_eq!((map.columns, map.rows), (2, 1));
        assert_eq!((map.x, map.z), ([0.0, 200.0], [0.0, 50.0]));
        assert_eq!(map.points.len(), 6);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        // At its corners, what the corners have.
        assert!(near(map.along(0.0, 0.0), 0.0));
        assert!(near(map.along(200.0, 50.0), 1.0));
        assert!(near(map.along(0.0, 50.0), 0.5));
        // Along its near edge, between two points.
        assert!(near(map.along(50.0, 0.0), 0.125));
        assert!(near(map.along(150.0, 0.0), 0.375));
        // In a cell, between all four: halfway across the first and halfway deep.
        assert!(near(map.along(50.0, 25.0), (0.0 + 0.25 + 0.5 + 0.75) / 4.0));
        // Beyond the grid it is what the edge has there.
        assert!(near(map.along(-500.0, -500.0), 0.0));
        assert!(near(map.along(900.0, 900.0), 1.0));
        assert!(near(map.along(-10.0, 25.0), 0.25));
        assert!(near(map.along(250.0, 25.0), 0.75));
        assert!(near(map.along(50.0, 900.0), 0.625));
    }

    #[test]
    fn what_is_not_a_path_map_is_refused() {
        let mut d = file();
        d.truncate(0x26);
        assert!(PathMap::parse(&d).is_err());
        d[0] = b'X';
        assert!(PathMap::parse(&d).is_err());
    }
}
