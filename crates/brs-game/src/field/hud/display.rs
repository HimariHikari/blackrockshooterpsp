//! Drawing the mission's display (0x48834), from its layout (`if_fldsys.lpk`, files
//! 8 to 11): at the foot of the screen on the left, a panel for each of the two
//! things a mission can show, the second over the first.
//!
//! A panel is its line, written small, and after it what its kind has: a clock, a
//! gauge, or numbers. Nothing of it moves: the display has a player of the layout
//! for each kind, each at the start of its set. A figure is one object of the
//! layout drawn with its texture moved to the figure's cell of a sheet of them.

use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{modulate, Layout, Look, Player};

use crate::field::system::{Numbers, Panel};
use crate::interface::layer;
use crate::text::{self, TextColours};
use crate::writing::{Fonts, Placing, Style};

// The sets the display's players are given, one for each kind of panel (0x4872C on).
const SET_LINE: usize = 0;
const SET_CLOCK: usize = 2;
const SET_GAUGE: usize = 5;
const SET_NUMBERS: usize = 7;
const SET_FIGURES: usize = 11;

/// How a panel's line is written (0x487D4, 0x487EC): forty-eight glyphs on one
/// line, each drawn ten by ten.
const LINE_STYLE: Style = Style { size: 10.0, ..Style::of_kind(0, 0x30, 1, 1.0, 4.0) };
/// How much higher than its object a line is written (0x48A6C).
const LINE_RAISE: f32 = 5.0;
/// How far the second panel stands from the first (0x488CC).
const PANEL_PITCH: f32 = -13.0;

// The groups of each kind that are more than something to draw (0x489A4, 0x48C2C,
// 0x490EC, 0x49520, 0x49B4C): where the line is written, and what shows figures.
const LINE_GROUP: usize = 1;
const CLOCK_LINE: usize = 2;
const CLOCK_PARTS: [usize; 3] = [3, 4, 5];
const CLOCK_MARKS: usize = 6;
const GAUGE_LINE: usize = 7;
const GAUGE_FRAME: usize = 8;
const GAUGE_BAR: usize = 9;
const NUMBERS_LINE: usize = 10;
const NUMBERS_FIRST: usize = 11;
const NUMBERS_STROKE: usize = 12;
const NUMBERS_SECOND: usize = 13;
const FIGURES_FIRST: usize = 17;
const FIGURES_SECOND: usize = 18;

/// The big figures are five to a row of their sheet, the small ones nine (0x48FBC,
/// 0x49920).
const BIG_FIGURES_A_ROW: i32 = 5;
const SMALL_FIGURES_A_ROW: i32 = 9;
/// How far apart the big figures of the first of two numbers stand for each figure
/// the second has (0x494B4): one less than their cells are wide.
const FIRST_NUMBER_PITCH: f32 = 11.0;

/// What the display is drawn from.
pub struct Display {
    pub layout: Layout,
    /// The size of the layout's texture, which the figures' cells are parts of.
    pub sheet: Vec2,
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

impl Display {
    /// Draws the panels that are shown.
    pub fn draw(&self, canvas: &mut Canvas, fonts: &Fonts, colours: &TextColours, panels: &[Panel]) {
        for panel in panels {
            self.draw_panel(canvas, fonts, colours, panel);
        }
    }

    fn draw_panel(&self, canvas: &mut Canvas, fonts: &Fonts, colours: &TextColours, panel: &Panel) {
        let layout = &self.layout;
        let layer = layer::DISPLAY;
        let down = (panel.slot as f32 * PANEL_PITCH).trunc();
        let words = text::pages(&panel.line, &text::Plain).swap_remove(0);
        let line = fonts.write(&LINE_STYLE, &words, colours);
        let width = line.size().x.trunc();
        let [first, second] = panel.values.map(|value| value.max(0));
        let at = |across: f32| Look::inner(Affine2::from_translation(Vec2::new(across, down)));

        // The line, where a group's object is, in that object's colour by its track's.
        let write = |canvas: &mut Canvas, player: &Player, track: usize, group: usize| {
            let Some((moved, colour)) = layout.placed(player, track) else { return };
            let Some((mark, place)) = layout.first_of(group).and_then(|object| Some((object, layout.place(object)?))) else { return };
            let to = Vec2::new(place.x + moved.translation.x.trunc(), moved.translation.y.trunc() + down + place.y - LINE_RAISE);
            fonts.draw(canvas, layer, &line, &Placing { tint: modulate(layout.colour(mark), colour), shadow: true, ..Placing::at(to) });
        };
        // A figure: its group's object, so far across, drawn from the figure's cell.
        let figure = |canvas: &mut Canvas, player: &Player, track: usize, group: usize, across: f32, figure: i32, a_row: i32| {
            let Some(object) = layout.first_of(group).and_then(|object| layout.objects.objects.get(object)) else { return };
            let cell = Vec2::new(object.size[0] as f32 + 1.0, object.size[1] as f32 + 1.0);
            let shift = cell * Vec2::new((figure % a_row) as f32, (figure / a_row) as f32) / self.sheet;
            layout.draw_track(canvas, layer, player, track, &Look { shift, ..at(across) });
        };
        // The width of a group's figures' cells.
        let cell_width = |group: usize| layout.first_of(group).and_then(|object| layout.objects.objects.get(object)).map_or(0.0, |object| object.size[0] as f32 + 1.0);

        match panel.numbers {
            Numbers::None => {
                let player = layout.player(SET_LINE);
                for track in 0..player.len() {
                    match player.group(track) {
                        Some(LINE_GROUP) => write(canvas, &player, track, LINE_GROUP),
                        _ => layout.draw_track(canvas, layer, &player, track, &at(0.0)),
                    }
                }
            }
            // Minutes, seconds and hundredths of the first number, which is in
            // thousandths of a second (0x48538): two figures each, after the line.
            Numbers::One => {
                let seconds = first / 1000 % 3600;
                let parts = [seconds / 60, seconds % 60, first % 1000 / 10];
                let player = layout.player(SET_CLOCK);
                for track in 0..player.len() {
                    let Some(group) = player.group(track) else { continue };
                    if group == CLOCK_LINE {
                        write(canvas, &player, track, group);
                    } else if let Some(part) = CLOCK_PARTS.iter().position(|&of| of == group) {
                        let pitch = cell_width(group);
                        figure(canvas, &player, track, group, width + pitch, parts[part] % 10, BIG_FIGURES_A_ROW);
                        figure(canvas, &player, track, group, width, parts[part] / 10 % 10, BIG_FIGURES_A_ROW);
                    } else if group == CLOCK_MARKS {
                        layout.draw_track(canvas, layer, &player, track, &at(width));
                    } else {
                        layout.draw_track(canvas, layer, &player, track, &at(0.0));
                    }
                }
            }
            // The first number as a share of the second (0xEF1E4): the bar, after
            // the line, cut to that much of its width.
            Numbers::Gauge => {
                let share = first as f32 / second.max(1) as f32;
                let player = layout.player(SET_GAUGE);
                for track in 0..player.len() {
                    let Some(group) = player.group(track) else { continue };
                    match group {
                        GAUGE_LINE => write(canvas, &player, track, group),
                        GAUGE_FRAME => layout.draw_track(canvas, layer, &player, track, &at(width)),
                        GAUGE_BAR => {
                            let Some(bar) = layout.first_of(group).and_then(|object| layout.objects.objects.get(object)) else { continue };
                            let from = Vec2::new(width + bar.position[0] as f32, down + bar.position[1] as f32);
                            let size = Vec2::new((bar.size[0] as f32 * share).trunc(), bar.size[1] as f32);
                            canvas.cut_to(Some(Rect::from_corners(from, from + size)));
                            layout.draw_track(canvas, layer, &player, track, &at(width));
                            canvas.cut_to(None);
                        }
                        _ => layout.draw_track(canvas, layer, &player, track, &at(0.0)),
                    }
                }
            }
            // The first number in big figures, as many as the second has, from the
            // right; a stroke; and the second in small ones, from the left.
            Numbers::Both => {
                let count = figures(second);
                let after = width + FIRST_NUMBER_PITCH * (count - 1) as f32;
                let player = layout.player(SET_NUMBERS);
                for track in 0..player.len() {
                    let Some(group) = player.group(track) else { continue };
                    match group {
                        NUMBERS_LINE => write(canvas, &player, track, group),
                        NUMBERS_FIRST => {
                            let (pitch, mut left) = (cell_width(group), first);
                            for place in 0..count {
                                // Places the number has no figure for show a nought.
                                figure(canvas, &player, track, group, after - place as f32 * pitch, left % 10, BIG_FIGURES_A_ROW);
                                left /= 10;
                            }
                        }
                        NUMBERS_STROKE => layout.draw_track(canvas, layer, &player, track, &at(after)),
                        NUMBERS_SECOND => {
                            let pitch = cell_width(group);
                            for place in 0..count {
                                let of = second / 10i32.pow((count - 1 - place) as u32) % 10;
                                figure(canvas, &player, track, group, after + place as f32 * pitch, of, SMALL_FIGURES_A_ROW);
                            }
                        }
                        _ => layout.draw_track(canvas, layer, &player, track, &at(0.0)),
                    }
                }
            }
            // Two figures of each number at places of their own, and no line.
            Numbers::Other => {
                let player = layout.player(SET_FIGURES);
                for track in 0..player.len() {
                    let Some(group) = player.group(track) else { continue };
                    let number = match group {
                        FIGURES_FIRST => first,
                        FIGURES_SECOND => second,
                        _ => {
                            layout.draw_track(canvas, layer, &player, track, &at(0.0));
                            continue;
                        }
                    };
                    let pitch = cell_width(group);
                    figure(canvas, &player, track, group, 0.0, number % 10, BIG_FIGURES_A_ROW);
                    figure(canvas, &player, track, group, -pitch, number / 10 % 10, BIG_FIGURES_A_ROW);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::field::hud::display;
    use brs_engine::canvas::Vertex;

    fn game() -> Option<(Display, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        Some((display(&data, &mut images).unwrap(), Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    fn drawn(display: &Display, fonts: &Fonts, colours: &TextColours, panel: Panel) -> Vec<Vertex> {
        let mut canvas = Canvas::default();
        display.draw(&mut canvas, fonts, colours, &[panel]);
        canvas.vertices().to_vec()
    }

    fn extent(vertices: &[Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    fn panel(slot: usize, numbers: Numbers, values: [i32; 2]) -> Panel {
        Panel { slot, line: "Defeat all the Eaters".into(), numbers, values }
    }

    #[test]
    fn numbers_are_written_with_as_many_figures_as_they_have() {
        assert_eq!([figures(0), figures(6), figures(10), figures(99), figures(100), figures(-12)], [1, 1, 2, 2, 3, 2]);
    }

    #[test]
    fn a_panel_is_its_mark_its_line_and_its_numbers_at_the_foot_of_the_screen() {
        let Some((display, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(display.sheet, Vec2::new(64.0, 64.0));
        // The line alone: its mark at (5, 256), and the line from 17 across, five
        // higher than the object that marks its place.
        let plain = drawn(&display, &fonts, &colours, panel(0, Numbers::None, [0, 0]));
        let glyphs = 2 * "DefeatalltheEaters".len();
        assert_eq!(plain.len(), (1 + glyphs) * 6);
        assert_eq!(extent(&plain[..6]).min, Vec2::new(5.0, 256.0));
        let line = extent(&plain[6..]);
        assert_eq!((line.min.x, line.min.y), (17.0, 251.0));
        let width = (line.max.x - 17.0 - 1.0).floor();
        // "0 / 6": one big figure, the stroke and one small figure after the line,
        // the small one drawn from the sixth cell of its row.
        let both = drawn(&display, &fonts, &colours, panel(0, Numbers::Both, [0, 6]));
        assert_eq!(both.len(), plain.len() + 3 * 6);
        let figures = &both[both.len() - 3 * 6..];
        let (big, small) = (extent(&figures[..6]), extent(&figures[12..]));
        assert!((big.min.x - (24.0 + width)).abs() <= 6.0 && big.min.y == 256.0, "{big:?} after a line of {width}");
        assert!(small.min.x > big.max.x && small.width() == 6.0, "{small:?}");
        let across = |vertices: &[Vertex]| vertices.iter().map(|vertex| vertex.uv.x).fold(f32::MAX, f32::min);
        let nought = drawn(&display, &fonts, &colours, panel(0, Numbers::Both, [0, 0]));
        assert!((across(&figures[12..]) - across(&nought[nought.len() - 6..]) - 6.0 * 7.0 / 64.0).abs() < 1e-5);
        // A number of two figures has the first number written with two as well.
        let more = drawn(&display, &fonts, &colours, panel(0, Numbers::Both, [3, 12]));
        assert_eq!(more.len(), plain.len() + 5 * 6);
        // The second panel stands thirteen over the first.
        let over = drawn(&display, &fonts, &colours, panel(1, Numbers::None, [0, 0]));
        assert_eq!(extent(&over[..6]).min, Vec2::new(5.0, 243.0));
    }

    #[test]
    fn a_gauge_is_cut_to_its_share_and_a_clock_has_six_figures() {
        let Some((display, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let bar = |values: [i32; 2]| {
            let all = drawn(&display, &fonts, &colours, panel(0, Numbers::Gauge, values));
            extent(&all[all.len() - 6..]).width()
        };
        // The bar is 47 wide when it is whole.
        assert_eq!((bar([10, 10]), bar([5, 10])), (47.0, 23.0));
        // 83.456 seconds: 01:23:45, a figure being a cell of twelve of the sheet.
        let plain = drawn(&display, &fonts, &colours, panel(0, Numbers::None, [0, 0]));
        let clock = drawn(&display, &fonts, &colours, panel(0, Numbers::One, [83_456, 0]));
        assert_eq!(clock.len(), plain.len() + (6 + 2) * 6);
    }
}
