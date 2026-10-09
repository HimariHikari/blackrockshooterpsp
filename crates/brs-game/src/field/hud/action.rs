//! Drawing the sign of what a button would do (0x466F8): the place of its word
//! in the signs' layout, drawn about where the layout has the first of its
//! objects (0x32288) and put where on the screen the sign is.

use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look};

use crate::field::action::{Showing, FRAMES_A_STEP};
use crate::interface::layer;

/// Draws the sign at `at`, as it is `ahead` of a frame of the game past its last
/// one.
pub fn draw(canvas: &mut Canvas, layout: &Layout, showing: &Showing, at: Vec2, ahead: f32) {
    let mut player = layout.player(showing.set);
    player.advance(layout, showing.frames + if showing.playing { ahead * FRAMES_A_STEP } else { 0.0 });
    // All of it is moved by as much as takes the first object of its first
    // group from where the layout has it to where the sign is.
    let home = player.group(0).and_then(|group| layout.first_of(group)).and_then(|object| layout.place(object));
    let Some(home) = home else { return };
    layout.draw(canvas, layer::DISPLAY, &player, &Look { outer: Affine2::from_translation(at - home), ..Look::default() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::field::action::{SETS, SET_FRAMES};
    use crate::interface;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn each_word_has_a_place_that_comes_and_goes_in_twenty_frames() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = interface::signs_layout(&data, &mut images).unwrap();
        // The word of each: the row of the words' sheet its label is drawn from
        // (ACTION, CHECK and TALK are the sheet's three rows, in that order), and
        // how wide the label is.
        let words: [(usize, f32, i16); 3] = [(24, 1.0, 59), (29, 23.0, 54), (34, 12.0, 43)];
        for (kind, sets) in SETS.iter().enumerate() {
            let (label, row, width) = words[kind];
            for &set in sets {
                let mut player = layout.player(set);
                let groups: Vec<usize> = (0..player.len()).filter_map(|track| player.group(track)).collect();
                assert_eq!(groups, [19 + 2 * kind, 20 + 2 * kind], "set {set}");
                assert!(layout.groups.groups[groups[0]].contains(&label));
                let mut frames = 0.0;
                while !player.finished(&layout) {
                    player.advance(&layout, FRAMES_A_STEP);
                    frames += FRAMES_A_STEP;
                    assert!(frames < 100.0);
                }
                assert_eq!(frames, SET_FRAMES, "set {set}");
            }
            let object = &layout.objects.objects[label];
            let quad = object.quad.as_ref().unwrap();
            assert_eq!((object.size[0], (quad.corners[0].uv[1] * 64.0).round()), (width, row), "kind {kind}");
        }
    }

    #[test]
    fn the_sign_is_drawn_about_where_it_is_put() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = interface::signs_layout(&data, &mut images).unwrap();
        let drawn = |showing: Showing, at: Vec2| {
            let mut canvas = Canvas::default();
            draw(&mut canvas, &layout, &showing, at, 0.0);
            canvas.vertices().to_vec()
        };
        // ACTION, come: the dark disc reaches from 45 to 19 left of where the sign
        // is and the bar under the word 45 right of it, thirteen above and below.
        let at = Vec2::new(240.0, 100.0);
        let all = drawn(Showing { kind: 0, set: SETS[0][0], frames: SET_FRAMES, playing: false }, at);
        let (mut least, mut most) = (Vec2::MAX, Vec2::MIN);
        for vertex in &all {
            (least, most) = (least.min(vertex.position), most.max(vertex.position));
        }
        assert_eq!((least, most), (at + Vec2::new(-45.0, -13.0), at + Vec2::new(45.0, 13.0)));
        assert!(all.iter().any(|vertex| vertex.colour[3] == 0xff));
        // As it begins to come nothing of it is there yet, and it is ten lower.
        let first = drawn(Showing { kind: 0, set: SETS[0][0], frames: 0.0, playing: true }, at);
        assert!(first.iter().all(|vertex| vertex.colour[3] == 0));
        let half = drawn(Showing { kind: 0, set: SETS[0][0], frames: 10.0, playing: true }, at);
        let lowest = half.iter().map(|vertex| vertex.position.y).fold(f32::MIN, f32::max);
        assert!(lowest > at.y + 13.0 && lowest < at.y + 23.0, "{lowest}");
        // TALK is drawn about its own first object, as far from it as ACTION is
        // from its.
        let talk = drawn(Showing { kind: 2, set: SETS[2][0], frames: SET_FRAMES, playing: false }, at);
        let left = talk.iter().map(|vertex| vertex.position.x).fold(f32::MAX, f32::min);
        assert_eq!(left, at.x - 37.0);
    }
}
