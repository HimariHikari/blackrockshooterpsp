//! The screen's fade in rows (kinds 0x100 and 0x200 of the original's "Trans"
//! task): a sheet of black that goes over the screen from the left, or comes off
//! to it, each of the screen's rows at a pace of its own, so that its edge is
//! ragged. The title leaves by it. See `docs/TITLE_NOTES.md`.

use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex, HEIGHT, WIDTH};

use crate::battle::damage::Random;

/// The screen's rows, and how far a row's place goes.
const ROWS: usize = HEIGHT as usize;
const ACROSS: i32 = WIDTH as i32;
/// What a row's pace is drawn below, besides what the frames come to (0x2B944).
const PACE_SPREAD: i32 = 0x20;
const BLACK: [u8; 4] = [0, 0, 0, 255];
const NOTHING: [u8; 4] = [0, 0, 0, 0];

#[derive(Clone, Debug, PartialEq)]
pub struct Rows {
    /// Each row's place, from nothing of the sheet on it to all of it, and what
    /// its place changes by in a frame.
    rows: Vec<(i32, i32)>,
    /// The sheet comes off (0x100) rather than goes over (0x200).
    off: bool,
    /// How much of itself the sheet is once the rows are there, of 255, and what
    /// that changes by in a frame (the fade's + 4 and + 0xC).
    level: f32,
    step: f32,
    there: bool,
}

impl Rows {
    /// The sheet goes over the screen over so many frames of the game (0x2B99C).
    pub fn over(frames: i32, random: &mut Random) -> Rows {
        Rows::new(false, frames, random)
    }

    /// The sheet comes off the screen (0x2B92C).
    #[allow(dead_code)]
    pub fn off(frames: i32, random: &mut Random) -> Rows {
        Rows::new(true, frames, random)
    }

    fn new(off: bool, frames: i32, random: &mut Random) -> Rows {
        let frames = frames.max(1);
        let pace = ACROSS / frames * 2;
        let rows = (0..ROWS).map(|_| (if off { ACROSS } else { 0 }, random.below(PACE_SPREAD) + pace)).collect();
        let step = 255.0 / frames as f32;
        Rows { rows, off, level: if off { 255.0 } else { 0.0 }, step: if off { -step } else { step }, there: false }
    }

    /// Whether it has got to where it was going (0x29948).
    pub fn there(&self) -> bool {
        self.there
    }

    /// A frame of it (0x2A61C): every row moves by its pace, and when none could
    /// any more it is there.
    pub fn advance(&mut self) {
        if self.there {
            return;
        }
        let mut moved = false;
        for (place, pace) in &mut self.rows {
            if self.off {
                if *place - *pace >= 0 {
                    *place -= *pace;
                    moved = true;
                } else {
                    *place = 0;
                }
            } else if *place + *pace <= ACROSS {
                *place += *pace;
                moved = true;
            } else {
                *place = ACROSS;
            }
        }
        self.level = (self.level + self.step * 2.0).clamp(0.0, 255.0);
        self.there = !moved;
    }

    /// Draws it (0x2AEEC): once it is there the whole of what is seen in the
    /// sheet's colour; on its way each row black from the left as far as twice
    /// its place less the screen's width, and from there, over the screen's
    /// width, from black to nothing.
    pub fn draw(&self, canvas: &mut Canvas, layer: i32, seen: Rect) {
        if self.there {
            let alpha = self.level as u8;
            if alpha != 0 {
                canvas.fill(layer, seen, [0, 0, 0, alpha]);
            }
            return;
        }
        let left = seen.min.x.min(-WIDTH);
        for (row, &(place, _)) in self.rows.iter().enumerate() {
            let (top, foot) = (row as f32, row as f32 + 1.0);
            let edge = (2 * place - ACROSS) as f32;
            if edge > left {
                canvas.fill(layer, Rect::new(left, top, edge, foot), BLACK);
            }
            let corner = |x: f32, y: f32, colour: [u8; 4]| Vertex::new(Vec2::new(x, y), Vec2::ZERO, colour);
            let thins = edge + WIDTH;
            canvas.strip(layer, Paint::PLAIN, &[corner(edge, top, BLACK), corner(thins, top, NOTHING), corner(edge, foot, BLACK), corner(thins, foot, NOTHING)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_goes_over_row_by_row_and_is_there_when_no_row_can_move() {
        // Over fifteen frames a row's pace is 64 and up to 31 more: the slowest
        // rows are across in eight frames.
        let mut random = Random::new(7);
        let mut rows = Rows::over(15, &mut random);
        assert!(rows.rows.iter().all(|&(place, pace)| place == 0 && (64..96).contains(&pace)));
        assert!(rows.rows.iter().any(|&(_, pace)| pace != rows.rows[0].1), "the rows have paces of their own");
        let mut frames = 0;
        while !rows.there() {
            rows.advance();
            frames += 1;
            assert!(frames < 20);
        }
        // A row that cannot take a whole step is put at the end, and the frame in
        // which none could is the one it is there in.
        assert!((7..=9).contains(&frames), "{frames}");
        assert!(rows.rows.iter().all(|&(place, _)| place == ACROSS));
        assert_eq!(rows.level, 255.0);
    }

    #[test]
    fn the_sheet_comes_off_the_same_way() {
        let mut random = Random::new(7);
        let mut rows = Rows::off(30, &mut random);
        assert!(rows.rows.iter().all(|&(place, pace)| place == ACROSS && (32..64).contains(&pace)));
        rows.advance();
        assert!(rows.rows.iter().all(|&(place, pace)| place == ACROSS - pace));
        for _ in 0..20 {
            rows.advance();
        }
        assert!(rows.there() && rows.rows.iter().all(|&(place, _)| place == 0));
        assert_eq!(rows.level, 0.0);
    }

    #[test]
    fn a_row_on_its_way_is_black_to_its_edge_and_thins_out_over_the_screens_width() {
        let mut random = Random::new(7);
        let mut rows = Rows::over(15, &mut random);
        rows.rows = vec![(360, 0); ROWS];
        let mut canvas = Canvas::default();
        rows.draw(&mut canvas, 0, Rect::new(0.0, 0.0, WIDTH, HEIGHT));
        // Two rectangles a row, of two triangles each.
        let corners = canvas.vertices();
        assert_eq!(corners.len(), ROWS * 12);
        let row = &corners[..12];
        // Black as far as 240, and from there to 720 thinning to nothing.
        assert!(row[..6].iter().all(|corner| corner.colour == BLACK && corner.position.x <= 240.0));
        assert!(row[6..].iter().all(|corner| if corner.position.x == 240.0 { corner.colour == BLACK } else { corner.position.x == 720.0 && corner.colour == NOTHING }));
    }
}
