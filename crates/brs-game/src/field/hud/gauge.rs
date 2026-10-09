//! Drawing her HP gauge (0x40774), from its layout (`if_rsdsys.lpk`, files 13 to
//! 15): at the foot of the screen on the right, a bar as long as the share of her
//! HP she has, and on it "HP" and the number.
//!
//! The game makes the gauge once, as it starts (0x42630: an entry of the kind 8 in
//! an object that keeps such things, playing set 0 of the layout), and the field
//! and a battle both show it. The set has two tracks for each of three of its
//! groups, one for when she is well and one, which goes round and pulses, for when
//! she is not: which of the two is drawn goes by her HP.

use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};

use crate::interface::layer;

/// The set the gauge plays (0x3DAAC).
const SET: usize = 0;

// The groups that are more than something to draw (0x40828): two tracks each for
// the glow, the "HP" and the number, and the bar, which is cut to her share.
const GLOW: usize = 1;
const BAR: usize = 2;
const LABEL: usize = 3;
const NUMBER: usize = 4;

/// How far apart the number's figures stand (0x40BEC).
const FIGURE_PITCH: f32 = 11.0;
/// The sheet the number's figures are cells of (0x2D064, the first of its table at
/// 0x171078): ten to a row, each twenty across.
const FIGURES_A_ROW: i32 = 10;
const FIGURE_CELL: f32 = 20.0;

// The colours the bar is drawn by (0x40998 on): while she is in danger, while she
// is to take care, and otherwise.
const BAR_IN_DANGER: [u8; 4] = [0xdc, 0x50, 0x46, 0xff];
const BAR_TAKING_CARE: [u8; 4] = [0xff, 0xbe, 0x50, 0xff];
const BAR_WELL: [u8; 4] = [0x3c, 0x64, 0xe6, 0xff];

/// How far from where its layout has it the gauge stands on the field (0x40784,
/// by the object's + 0x70, which the field's frame sets to 0 at 0x44C34): a pixel
/// left and ten lower. A battle has it where the layout does (1, set at 0x36154),
/// over the gauge of her heat, and 2 is a pixel left and ten higher (0x55FC8).
pub const ON_THE_FIELD: Vec2 = Vec2::new(-1.0, 10.0);

/// What the gauge is drawn from.
pub struct Gauge {
    pub layout: Layout,
    /// The width of the texture the number's figures are in.
    pub sheet_width: f32,
    /// The shares of her HP at or under which she is to take care and is in danger
    /// (0xD2BBC: the battle system's values 79 and 80, in hundredths).
    pub care: f32,
    pub danger: f32,
}

/// How many figures a number is written with (0x22ACC).
fn figures(number: i32) -> i32 {
    let mut left = number.unsigned_abs();
    let mut count = 1;
    while left >= 10 {
        left /= 10;
        count += 1;
    }
    count
}

impl Gauge {
    /// The gauge's player as it is `frames` after the game made it.
    pub fn player(&self, frames: f32) -> Player {
        Player::at(&self.layout, SET, frames)
    }

    /// Draws the gauge as its player has it, moved so far from where its layout
    /// has it, for the HP she has of the most she can have.
    pub fn draw(&self, canvas: &mut Canvas, player: &Player, hp: i32, max_hp: i32, moved: Vec2) {
        self.draw_on(canvas, layer::DISPLAY, player, hp, max_hp, moved, 255);
    }

    /// Draws it in the turn of something else that shows it, with so much of it
    /// there (0x3D7F0 gives it a colour of its own: a menu has it come with the
    /// rest of its screen).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_on(&self, canvas: &mut Canvas, layer: i32, player: &Player, hp: i32, max_hp: i32, moved: Vec2, alpha: u8) {
        let layout = &self.layout;
        let hp = hp.max(0);
        let share = hp as f32 / max_hp.max(0) as f32;
        let in_danger = share <= self.danger;
        let look = Look { tint: [255, 255, 255, alpha], outer: Affine2::from_translation(moved), ..Look::default() };
        // Of a group's two tracks the first is drawn while she is well and the
        // second, which comes right after it, while she is in danger.
        let mut before = Some(0);
        for track in 0..player.len() {
            let group = player.group(track);
            let second = group == before;
            before = group;
            match group {
                Some(GLOW | LABEL) => {
                    if second == in_danger {
                        layout.draw_track(canvas, layer, player, track, &look);
                    }
                }
                // The bar, cut to her share of its object's width: a pixel at the
                // least while she has any HP.
                Some(BAR) => {
                    let Some(bar) = layout.first_of(BAR).and_then(|object| layout.objects.objects.get(object)) else { continue };
                    let mut width = (bar.size[0] as f32 * share) as i32;
                    if hp > 0 {
                        width = width.max(1);
                    }
                    let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32) + moved;
                    let tint = if in_danger {
                        BAR_IN_DANGER
                    } else if share <= self.care {
                        BAR_TAKING_CARE
                    } else if share <= 1.0 {
                        BAR_WELL
                    } else {
                        [255; 4]
                    };
                    canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(width as f32, bar.size[1] as f32))));
                    let tint = brs_engine::layout::modulate(tint, look.tint);
                    layout.draw_track(canvas, layer, player, track, &Look { tint, ..look });
                    canvas.cut_to(None);
                }
                // The number: its one object for each figure, the last where the
                // layout has it and the others before it, each drawn from its
                // figure's cell of the sheet.
                Some(NUMBER) => {
                    if second != in_danger {
                        continue;
                    }
                    let mut left = hp;
                    for place in 0..figures(hp) {
                        let figure = left % FIGURES_A_ROW;
                        left /= FIGURES_A_ROW;
                        let inner = Affine2::from_translation(Vec2::new(-FIGURE_PITCH * place as f32, 0.0));
                        let shift = Vec2::new(FIGURE_CELL * figure as f32 / self.sheet_width, 0.0);
                        layout.draw_track(canvas, layer, player, track, &Look { inner, shift, ..look });
                    }
                }
                _ => layout.draw_track(canvas, layer, player, track, &look),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::field::hud::gauge;
    use brs_engine::canvas::Vertex;
    use brs_engine::layout::modulate;

    fn game() -> Option<Gauge> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        Some(gauge(&data, &mut images).unwrap())
    }

    fn drawn(gauge: &Gauge, frames: f32, hp: i32, max_hp: i32) -> Vec<Vertex> {
        let mut canvas = Canvas::default();
        gauge.draw(&mut canvas, &gauge.player(frames), hp, max_hp, ON_THE_FIELD);
        canvas.vertices().to_vec()
    }

    fn extent(vertices: &[Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    /// The corners of the bar: what is drawn after the frame's four objects and
    /// before the pattern over it, "HP", the number's figures and, when it is up,
    /// the glow. A bar that is cut has more corners than a whole one.
    fn bar(vertices: &[Vertex], hp: i32, glowing: bool) -> &[Vertex] {
        let after = 2 + figures(hp) as usize + glowing as usize;
        &vertices[4 * 6..vertices.len() - after * 6]
    }

    #[test]
    fn the_gauge_is_a_bar_of_her_share_with_her_hp_on_it() {
        let Some(gauge) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!((gauge.care, gauge.danger, gauge.sheet_width), (0.5, 0.25, 256.0));
        // The frame is a pixel left of where its layout has it and ten lower.
        let well = drawn(&gauge, 0.0, 357, 372);
        assert_eq!(extent(&well[..6]).min, Vec2::new(312.0, 242.0));
        // The bar: from (319, 243), eighteen high, and of 152 as much as her share.
        let all = extent(bar(&well, 357, false));
        assert_eq!((all.min, all.max), (Vec2::new(319.0, 243.0), Vec2::new(319.0 + (152.0 * 357.0 / 372.0_f32).trunc(), 261.0)));
        // It is blue, and all there.
        assert!(bar(&well, 357, false).iter().all(|vertex| vertex.colour[2] > vertex.colour[0] && vertex.colour[3] == 255));
        // The figures are the last drawn, from the right: 7, 5, 3, eleven apart,
        // each from its own cell of the sheet.
        let figure = |index: usize| &well[well.len() - (3 - index) * 6..][..6];
        let across = |vertices: &[Vertex]| vertices.iter().map(|vertex| vertex.uv.x).fold(f32::MAX, f32::min);
        assert_eq!(extent(figure(0)).min, Vec2::new(457.0, 247.0));
        assert_eq!(extent(figure(1)).min, Vec2::new(446.0, 247.0));
        assert_eq!(extent(figure(2)).min, Vec2::new(435.0, 247.0));
        let cell = |number: f32| number * 20.0 / 256.0;
        assert_eq!([across(figure(0)), across(figure(1)), across(figure(2))], [cell(7.0), cell(5.0), cell(3.0)]);
        // "HP" and the figures are not all there while she is well.
        assert!(figure(0).iter().all(|vertex| vertex.colour[3] == 153));
        // With all her HP the bar is whole: nothing of it is cut away.
        let whole = drawn(&gauge, 0.0, 372, 372);
        assert_eq!(whole.len(), (4 + 1 + 1 + 1 + 3) * 6);
        assert_eq!(extent(bar(&whole, 372, false)).width(), 152.0);
    }

    #[test]
    fn the_bar_changes_colour_as_her_hp_falls_and_pulses_in_danger() {
        let Some(gauge) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The bar's width, and the colour of its left end, where its object is two
        // thirds as bright as at its right.
        let look = |hp: i32| {
            let all = drawn(&gauge, 0.0, hp, 372);
            let bar = bar(&all, hp, false);
            (extent(bar).width(), bar.iter().map(|vertex| vertex.colour).min().unwrap())
        };
        let left_end = |colour: [u8; 4]| modulate([0xaa, 0xaa, 0xaa, 0xff], colour);
        // Half and under: amber. A quarter and under: red.
        assert_eq!(look(187), (76.0, left_end(BAR_WELL)));
        assert_eq!(look(186), (76.0, left_end(BAR_TAKING_CARE)));
        assert_eq!(look(94), (38.0, left_end(BAR_TAKING_CARE)));
        assert_eq!(look(93), (38.0, left_end(BAR_IN_DANGER)));
        // With a single point left the bar is a pixel; with none there is no bar.
        assert_eq!((look(1).0, look(0).0), (1.0, 0.0));
        // In danger the second track of each pair is drawn: the glow comes and goes
        // over the bar every twenty-four frames, and the number with it.
        let (glowing, dark) = (drawn(&gauge, 6.0, 50, 372), drawn(&gauge, 0.0, 50, 372));
        assert_eq!(glowing.len(), dark.len() + 6);
        assert_eq!(extent(bar(&glowing, 50, true)), extent(bar(&dark, 50, false)));
        let figure = &glowing[glowing.len() - 6..];
        assert!(figure.iter().all(|vertex| vertex.colour == [255, 229, 124, 255]), "{:?}", figure[0].colour);
        assert_eq!(drawn(&gauge, 24.0, 50, 372), dark);
    }
}
