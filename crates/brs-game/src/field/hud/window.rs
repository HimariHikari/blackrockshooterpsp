//! Drawing the window a script puts up (0x6C0F8), from its layout
//! (`if_rsdsys.lpk/000.lpk`).
//!
//! The layout has the window two lines tall. For a taller one the game draws it in
//! parts, each through a matrix of its own: the band across the screen and the
//! frame's sides are stretched, what is above the text is moved up and what is
//! below it down, nine pixels for each line more than two. The text and the choices
//! are written where objects of the layout mark, in the colours their tracks have
//! come to.

use bevy::math::{Affine2, Mat2};
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};

use crate::field::window::{set, Window, FRAMES_A_STEP, TEXT_TRACK};
use crate::interface::layer;
use crate::text::{self, TextColours};
use crate::writing::{Align, Fonts, Placing, Style};

/// How the window's text is written (0x6B718): forty-eight glyphs to a line and
/// six lines, a pixel between glyphs and four between lines; and each of its
/// choices (0x6B748), twenty glyphs on one line.
const TEXT_STYLE: Style = Style::of_kind(0, 0x30, 6, 1.0, 4.0);
const CHOICE_STYLE: Style = Style::of_kind(0, 0x14, 1, 1.0, 4.0);

// The tracks of the sets that open and close the window: the same in the sets of a
// window that only tells and of one that asks, but for the text's.
const BODY: usize = 0;
const TOP_FRAME: usize = 1;
const BOTTOM_FRAME: usize = 2;
const SIDES: usize = 3;
const LABEL: usize = 4;
const TEXT_TRACK_ASKING: usize = 5;
/// The sign that there is more to read or that cross is waited for: its word and
/// its button.
const NEXT_SIGN: [usize; 2] = [7, 8];
// The objects of the body's group, top to bottom, and of the label's: the bar down
// the window's left, and the label itself.
const BAND_TOP: usize = 0;
const BAND_MIDDLE: usize = 1;
const BAND_BOTTOM: usize = 2;
const LABEL_BAR: usize = 0;
const LABEL_WORD: usize = 1;

// The tracks of the sets that bring the choices and take them away: the mark by a
// choice the cursor is not on, the bar under the one it is on, and the two whose
// colours the choices are written in.
const CHOICE_MARK: usize = 0;
const CHOICE_CURSOR: usize = 1;
const CHOICE_CHOSEN: usize = 2;
const CHOICE_OTHER: usize = 3;
/// The clip a choice the cursor has left is dimmed by (0x6CCEC).
const CHOICE_LEFT_CLIP: usize = 16;

/// The objects whose sizes and places the drawing goes by, for a window that only
/// tells and for one that asks (0x6C1DC, 0x6C414, 0x6C4B4, 0x6C58C; 0x6C71C on).
struct Marks {
    /// The band's middle part, which is stretched.
    middle: usize,
    /// One of the frame's sides, and the bar down the window's left.
    side: usize,
    bar: usize,
    /// Where the text is written, and how wide.
    text: usize,
}

const TELLING: Marks = Marks { middle: 2, side: 8, bar: 12, text: 14 };
const ASKING: Marks = Marks { middle: 2, side: 19, bar: 23, text: 25 };
/// Where the first choice is written (0x6CBB0).
const FIRST_CHOICE: usize = 32;

/// A window is two lines tall as its layout has it.
const LINES_AS_LAID_OUT: usize = 2;
/// What each line more adds to its height, half above and half below (0x6C1D8), and
/// how far one choice is under another (0x6CE18).
const LINE_HEIGHT: f32 = 18.0;
const CHOICE_PITCH: f32 = 18.0;
/// The height the band's middle part is stretched about, in its own corners
/// (0x6C1F8).
const BAND_MIDDLE_CENTRE: f32 = 5.0;

/// A matrix that only scales heights and moves down.
fn tall(scale: f32, down: f32) -> Affine2 {
    Affine2::from_mat2_translation(Mat2::from_diagonal(Vec2::new(1.0, scale)), Vec2::new(0.0, down))
}

/// Draws a window as it is `ahead` of a frame of the game past its last one.
pub fn draw(canvas: &mut Canvas, layout: &Layout, fonts: &Fonts, colours: &TextColours, window: &Window, ahead: f32) {
    draw_on(canvas, layer::WINDOW, layout, fonts, colours, window, ahead);
}

/// Draws a window on a layer: a screen of the menus has one of its own, which it
/// draws with the rest of itself.
pub fn draw_on(canvas: &mut Canvas, layer: i32, layout: &Layout, fonts: &Fonts, colours: &TextColours, window: &Window, ahead: f32) {
    let asking = window.asking();
    let marks = if asking { &ASKING } else { &TELLING };
    let ahead = ahead * FRAMES_A_STEP;
    let body = match (window.closing, asking) {
        (None, false) => Player::at(layout, set::OPEN, window.frames + ahead),
        (None, true) => Player::at(layout, set::OPEN_ASKING, window.frames + ahead),
        (Some(closing), false) => Player::at(layout, set::CLOSE, closing + ahead),
        (Some(closing), true) => Player::at(layout, set::CLOSE_ASKING, closing + ahead),
    };
    let height = |object: usize| layout.objects.objects.get(object).map_or(1.0, |object| object.size[1] as f32);

    // The lines more than the layout is made for, and the growth they come to.
    let more = window.lines.saturating_sub(LINES_AS_LAID_OUT) as f32;
    let growth = more * LINE_HEIGHT;
    let (up, down) = (tall(1.0, -growth / 2.0), tall(1.0, growth / 2.0));
    let member = |canvas: &mut Canvas, track: usize, member: usize, inner: Affine2| layout.draw_member(canvas, layer, &body, track, member, &Look::inner(inner));
    let part = |canvas: &mut Canvas, track: usize, inner: Affine2| layout.draw_track(canvas, layer, &body, track, &Look::inner(inner));

    // The band across the screen: its middle stretched about its own centre, its
    // top and bottom moved apart; and the frame's top with the band's.
    let stretch = growth / height(marks.middle);
    member(canvas, BODY, BAND_MIDDLE, tall(1.0 + stretch, -BAND_MIDDLE_CENTRE * stretch));
    member(canvas, BODY, BAND_TOP, up);
    part(canvas, TOP_FRAME, up);
    member(canvas, LABEL, LABEL_WORD, up);
    if !asking {
        for track in NEXT_SIGN {
            layout.draw_track(canvas, layer, &body, track, &Look { outer: up, ..default() });
        }
    }
    member(canvas, BODY, BAND_BOTTOM, down);
    part(canvas, BOTTOM_FRAME, down);
    // The sides grow by whole lines of their own height.
    let side = (height(marks.side) / LINE_HEIGHT).trunc() * LINE_HEIGHT;
    part(canvas, SIDES, tall(1.0 + growth / side.max(1.0), 0.0));
    member(canvas, LABEL, LABEL_BAR, tall(1.0 + growth / height(marks.bar), -growth / 2.0));

    // The text, in the colour its track has come to: it comes up after the window.
    let text_track = if asking { TEXT_TRACK_ASKING } else { TEXT_TRACK };
    let Some((moved, colour)) = layout.placed(&body, text_track) else { return };
    let Some(area) = layout.objects.objects.get(marks.text) else { return };
    let at = Vec2::new(
        (moved.translation.x.trunc() + area.position[0] as f32).trunc(),
        ((moved.translation.y - growth / 2.0).trunc() + area.position[1] as f32).trunc(),
    );
    let block = fonts.write(&TEXT_STYLE, window.page(), colours);
    let placing = Placing { width: area.size[0] as f32, align: Align::Right, tint: colour, shadow: true, ..Placing::at(at) };
    fonts.draw(canvas, layer, &block, &placing);

    // The choices come with the last page.
    if !window.choosing() {
        return;
    }
    let choices = match window.closing {
        None => Player::at(layout, set::CHOICES_IN, window.choice_frames + ahead),
        Some(closing) => Player::at(layout, set::CHOICES_OUT, closing + ahead),
    };
    let Some(first) = layout.place(FIRST_CHOICE) else { return };
    // Fewer choices than lines stand in the middle of the window's height.
    let spare = window.lines.saturating_sub(window.choices.len()) as f32;
    let top = if more > 0.0 { -growth / 2.0 + spare * LINE_HEIGHT / 2.0 } else { 0.0 };
    for (index, choice) in window.choices.iter().enumerate() {
        let down = top + index as f32 * CHOICE_PITCH;
        let look = Look::inner(tall(1.0, down));
        let colour = if index == window.selected {
            layout.draw_track(canvas, layer, &choices, CHOICE_CURSOR, &look);
            layout.placed(&choices, CHOICE_CHOSEN).map(|(_, colour)| colour)
        } else {
            layout.draw_track(canvas, layer, &choices, CHOICE_MARK, &look);
            // One the cursor has just left dims from what it was.
            match window.left[index].filter(|_| window.closing.is_none()) {
                Some(frames) => layout.clip_at(CHOICE_LEFT_CLIP, frames).map(|(_, colour)| colour),
                None => layout.placed(&choices, CHOICE_OTHER).map(|(_, colour)| colour),
            }
        };
        let Some(colour) = colour else { continue };
        let words = text::pages(choice, &text::Plain).swap_remove(0);
        let block = fonts.write(&CHOICE_STYLE, &words, colours);
        let at = Vec2::new(first.x.trunc(), (down.trunc() + first.y).trunc());
        fonts.draw(canvas, layer, &block, &Placing { tint: colour, shadow: true, ..Placing::at(at) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Pad;
    use crate::data::GameData;
    use crate::field::hud::RESIDENT_PACK_WINDOW;
    use crate::interface;

    fn game() -> Option<(Layout, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let layout = interface::layout(&data, &mut images, RESIDENT_PACK_WINDOW, 0).unwrap();
        Some((layout, Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    /// The window after so many frames of the game with nothing pressed.
    fn after(mut window: Window, frames: usize) -> Window {
        for _ in 0..frames {
            window.step(&Pad::default());
        }
        window
    }

    fn drawn(layout: &Layout, fonts: &Fonts, colours: &TextColours, window: &Window) -> Vec<brs_engine::canvas::Vertex> {
        let mut canvas = Canvas::default();
        draw(&mut canvas, layout, fonts, colours, window, 0.0);
        canvas.vertices().to_vec()
    }

    fn extent(vertices: &[brs_engine::canvas::Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    #[test]
    fn a_window_opens_from_nothing_and_its_text_comes_up_last() {
        let Some((layout, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let pages = text::pages("HP has been restored.", &text::Plain);
        let window = Window::new(pages, &[], 0, 0);
        // As it opens there is nothing of it yet.
        assert!(drawn(&layout, &fonts, &colours, &window).is_empty());
        // A frame later the band is there and thin, and nothing else is.
        let begun = drawn(&layout, &fonts, &colours, &after(window.clone(), 1));
        assert_eq!(begun.len(), 3 * 6);
        assert!(extent(&begun).height() < 60.0);
        // Open, the band is all the screen's width and the text is written in it,
        // its line ending where the text's place does.
        let open = drawn(&layout, &fonts, &colours, &after(window.clone(), 30));
        let area = extent(&open);
        assert_eq!((area.min.x, area.max.x), (-1.0, 481.0));
        assert_eq!((area.min.y, area.max.y), (98.0, 188.0));
        let glyphs = 2 * "HPhasbeenrestored.".len();
        let text = extent(&open[open.len() - glyphs * 6..]);
        // (The full stop is 5 wide in a cell of 15, and the shadow is one further.)
        assert_eq!((text.max.x, text.min.y), (83.0 + 319.0 - 5.0 + 15.0 + 1.0, 130.0));
        // Closing, it is gone after its twenty frames.
        let mut closing = after(window, 30);
        closing.closing = Some(20.0);
        assert!(drawn(&layout, &fonts, &colours, &closing).is_empty());
    }

    #[test]
    fn a_window_of_more_lines_is_taller_by_eighteen_for_each() {
        let Some((layout, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tall = |lines: &str| {
            let window = after(Window::new(text::pages(lines, &text::Plain), &[], 0, 0), 30);
            extent(&drawn(&layout, &fonts, &colours, &window))
        };
        let (two, four) = (tall("one\ntwo"), tall("one\ntwo\nthree\nfour"));
        assert_eq!((two.min.y, two.max.y), (98.0, 188.0));
        assert_eq!((four.min.y, four.max.y), (98.0 - 18.0, 188.0 + 18.0));
    }

    #[test]
    fn the_choices_stand_one_under_the_other_with_the_cursors_bar_under_one() {
        let Some((layout, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let window = after(Window::new(text::pages("Save Game Data?", &text::Plain), &["Yes", "No"], 0, 1), 40);
        let open = drawn(&layout, &fonts, &colours, &window);
        // The last things drawn are the second choice's two letters, each with its
        // shadow: 18 under the first choice, which is at (359, 131).
        let no = extent(&open[open.len() - 4 * 6..]);
        assert_eq!((no.min.x, no.min.y), (359.0, 131.0 + 18.0));
        // Before the choices have come up there is nothing of them.
        let early = after(Window::new(text::pages("Save Game Data?", &text::Plain), &["Yes", "No"], 0, 1), 4);
        let fewer = drawn(&layout, &fonts, &colours, &early);
        assert!(fewer.iter().all(|vertex| vertex.position.x < 355.0 || vertex.position.y < 125.0 || vertex.position.x > 472.0));
    }
}
