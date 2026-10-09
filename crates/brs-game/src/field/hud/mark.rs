//! Drawing the mark over a character that has noticed her (0x46B5C): a red glow
//! with an exclamation mark in it, the first set of the signs' layout, which
//! pops up over its head and is gone in under a second. Where it is and how big
//! is the character's own to say each frame (0x109F4C on).

use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look};

use crate::interface::layer;

/// The set of the signs' layout the mark plays (0x469F0).
pub const SET: usize = 0;
/// How far above where a character stands the mark is put when its model has
/// no node to say (0xFD8B4), and how far above that node, or that, it is
/// (0x109FC0).
pub const NO_NODE: f32 = 150.0;
const ABOVE: f32 = 14.0;
// And ten more at 450 from the camera, less the farther off it is and more the
// nearer: a tenth for every 135 (0x10A014).
const LIFT: f32 = 10.0;
const LIFT_AT: f32 = 450.0;
const LIFT_OVER: f32 = 1350.0;
// How big it is drawn goes by how tall 56 of the world's units are on the
// screen there: at 48 pixels it is its own size, and it grows and shrinks by a
// part of what they are more or less than that (0x10A2DC to 0x10A3AC).
const TALL: f32 = 56.0;
const TALL_ON_SCREEN: f32 = 48.0;
const SHRINKS_BY: f32 = 0.4;
const GROWS_BY: f32 = 0.25;

/// Where over a character its mark is, given where its top is and how far in
/// front of the camera that is.
pub fn place(top: Vec3, depth: impl Fn(Vec3) -> f32) -> Vec3 {
    let point = top + Vec3::Y * ABOVE;
    point + Vec3::Y * (LIFT - (depth(point) - LIFT_AT) * LIFT / LIFT_OVER)
}

/// The point whose place on the screen, with the mark's, says how big the mark
/// is drawn.
pub fn above(point: Vec3) -> Vec3 {
    point + Vec3::Y * TALL
}

/// How big the mark is drawn where so much of the world is as tall on the
/// screen as `pixels`.
pub fn scale(pixels: f32) -> f32 {
    let share = pixels / TALL_ON_SCREEN;
    1.0 + (share - 1.0) * if share <= 1.0 { SHRINKS_BY } else { GROWS_BY }
}

/// The set of the signs' layout that is the sign of a trap (0x3DA4C): a soft
/// round, eighteen across, that comes over eight frames, and four corners
/// about it that go on turning for as long as the set is played.
pub const TRAP_SET: usize = 15;

/// Draws the sign of a trap about `at`, `frames` of its set played (0x3FCB4):
/// each of its two things moved from where the layout has it to there
/// (0x321F8). The second would be turned besides by a figure of the sign's
/// that nothing sets for a trap's. The set is not begun again: the sign is
/// kept when it has played through (its flag 0x80000000, 0x3D0C4), and its
/// corners turn on.
pub fn draw_of_trap(canvas: &mut Canvas, layout: &Layout, frames: f32, at: Vec2) {
    let mut player = layout.player(TRAP_SET);
    player.advance(layout, frames);
    for track in 0..player.len() {
        let Some(home) = player.group(track).and_then(|group| layout.first_of(group)).and_then(|first| layout.place(first)) else { continue };
        let look = Look { outer: Affine2::from_translation(at - home), ..Look::default() };
        layout.draw_track(canvas, layer::DISPLAY, &player, track, &look);
    }
}

/// Draws the mark, `frames` of its set played, with the foot of it at `at`
/// (0x46B5C): made as big as `scale` says about the middle of its glow, and put
/// half the glow's height above the point.
pub fn draw(canvas: &mut Canvas, layout: &Layout, frames: f32, at: Vec2, scale: f32) {
    let mut player = layout.player(SET);
    player.advance(layout, frames);
    let first = player.group(0).and_then(|group| layout.first_of(group));
    let Some((first, home)) = first.and_then(|first| Some((first, layout.place(first)?))) else { return };
    let half = f32::from(layout.objects.objects[first].size[1] / 2);
    let look = Look { inner: Affine2::from_scale(Vec2::splat(scale)), outer: Affine2::from_translation(at - Vec2::Y * half - home), ..Look::default() };
    layout.draw(canvas, layer::DISPLAY, &player, &look);
}

/// The set of the signs' layout that is the tile of one pointed out (0x46A04:
/// the object's second place): a black tile, twenty square, with a red chevron.
pub const TILE_SET: usize = 20;

/// Draws the tile that stands by the mark of a character a mission is after
/// (0x46C28: the object's second place, drawn while its bit 0x20 is set, which
/// the character's flag 0xA5 has it, 0x46CD0), where the mark's foot is at `at`.
/// It is put and made as big as the mark is (0x46B9C to 0x46BEC are for the
/// whole object): by the layout it is to the left of the glow, its foot three
/// under the glow's middle.
pub fn draw_tile(canvas: &mut Canvas, layout: &Layout, at: Vec2, scale: f32) {
    let mark = layout.player(SET);
    let first = mark.group(0).and_then(|group| layout.first_of(group));
    let Some((first, home)) = first.and_then(|first| Some((first, layout.place(first)?))) else { return };
    let half = f32::from(layout.objects.objects[first].size[1] / 2);
    let look = Look { inner: Affine2::from_scale(Vec2::splat(scale)), outer: Affine2::from_translation(at - Vec2::Y * half - home), ..Look::default() };
    layout.draw(canvas, layer::DISPLAY, &layout.player(TILE_SET), &look);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::field::scene::MARK_FRAMES;
    use crate::interface;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn the_mark_is_higher_the_nearer_and_drawn_by_how_tall_things_are_there() {
        // Fourteen over the top, and ten more at 450 from the camera.
        let top = Vec3::new(100.0, 150.0, -40.0);
        assert_eq!(place(top, |_| 450.0), top + Vec3::Y * 24.0);
        assert_eq!(place(top, |_| 1800.0), top + Vec3::Y * 14.0);
        assert_eq!(place(top, |_| 180.0), top + Vec3::Y * 26.0);
        assert_eq!(above(top), top + Vec3::Y * 56.0);
        // Its own size where 56 units are 48 pixels tall; less than that it
        // shrinks by two fifths of the difference, more it grows by a quarter.
        assert_eq!((scale(48.0), scale(24.0), scale(96.0)), (1.0, 0.8, 1.25));
    }

    #[test]
    fn the_sign_of_a_trap_is_eighteen_across_about_its_point() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = interface::signs_layout(&data, &mut images).unwrap();
        // What is seen of it so many frames into its set: how far it reaches
        // from its point, and how strong the first of it is drawn.
        let seen = |frames: f32| {
            let mut canvas = Canvas::default();
            draw_of_trap(&mut canvas, &layout, frames, Vec2::new(240.0, 136.0));
            let reach = canvas.vertices().iter().map(|vertex| (vertex.position - Vec2::new(240.0, 136.0)).abs().max_element()).fold(0.0, f32::max);
            (reach, canvas.vertices().first().map_or(0, |vertex| vertex.colour[3]))
        };
        // The round comes over eight frames, nine to every side of the point.
        assert_eq!(seen(0.0), (0.0, 0));
        let (reach, alpha) = seen(4.0);
        assert!(alpha > 150 && alpha < 255 && (9.0..10.0).contains(&reach), "{reach} {alpha}");
        assert_eq!(seen(8.0).1, 255);
        // The corners go on turning after that: they reach furthest when they
        // stand cornerwise, and are square to the screen again a quarter turn on.
        let reaches: Vec<f32> = (0..400).map(|frame| seen(8.0 + frame as f32 * 2.0).0).collect();
        let (least, most) = (reaches.iter().copied().fold(f32::MAX, f32::min), reaches.iter().copied().fold(0.0, f32::max));
        assert!((least - 9.0).abs() < 0.2 && (most - 9.0 * std::f32::consts::SQRT_2).abs() < 0.2, "{least} {most}");
    }

    #[test]
    fn the_tile_of_one_pointed_out_stands_still_by_the_marks_point() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = interface::signs_layout(&data, &mut images).unwrap();
        // Its set is one thing that does not move: it is through as it begins.
        let player = layout.player(TILE_SET);
        assert_eq!((player.len(), player.finished(&layout)), (1, true));
        let reach = |scale: f32| {
            let mut canvas = Canvas::default();
            draw_tile(&mut canvas, &layout, Vec2::new(240.0, 100.0), scale);
            let seen: Vec<_> = canvas.vertices().iter().filter(|vertex| vertex.colour[3] != 0).map(|vertex| vertex.position).collect();
            seen.iter().copied().reduce(Vec2::min).zip(seen.iter().copied().reduce(Vec2::max)).unwrap()
        };
        // Twenty square, to the left of the point with its foot level with it:
        // by the glow of the mark, which is 46 square over the point.
        assert_eq!(reach(1.0), (Vec2::new(212.0, 80.0), Vec2::new(232.0, 100.0)));
        // It is made bigger and smaller with the mark, about the glow's middle.
        assert_eq!(reach(1.5), (Vec2::new(198.0, 81.5), Vec2::new(228.0, 111.5)));
    }

    #[test]
    fn the_mark_pops_up_over_the_point_and_is_gone_when_its_set_is_through() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = interface::signs_layout(&data, &mut images).unwrap();
        // Its set is the glow and the exclamation mark, 54 frames long.
        let mut player = layout.player(SET);
        let groups: Vec<usize> = (0..player.len()).filter_map(|track| player.group(track)).collect();
        assert_eq!(groups, [0, 1]);
        let mut frames = 0.0;
        while !player.finished(&layout) {
            player.advance(&layout, 2.0);
            frames += 2.0;
            assert!(frames < 200.0);
        }
        assert_eq!(frames, MARK_FRAMES);

        let reach = |frames: f32, scale: f32| {
            let mut canvas = Canvas::default();
            draw(&mut canvas, &layout, frames, Vec2::new(240.0, 100.0), scale);
            let seen: Vec<_> = canvas.vertices().iter().filter(|vertex| vertex.colour[3] != 0).map(|vertex| vertex.position).collect();
            seen.iter().copied().reduce(Vec2::min).zip(seen.iter().copied().reduce(Vec2::max))
        };
        // Standing, the glow is 46 square with its foot at the point.
        let (least, most) = reach(30.0, 1.0).unwrap();
        assert_eq!((least, most), (Vec2::new(217.0, 54.0), Vec2::new(263.0, 100.0)));
        // Drawn half as big again it is so about the glow's middle.
        let (least, most) = reach(30.0, 1.5).unwrap();
        assert_eq!((least, most), (Vec2::new(205.5, 42.5), Vec2::new(274.5, 111.5)));
        // As it comes it is bigger than it will stand, and at the end nothing of
        // it is left.
        let (least, most) = reach(10.0, 1.0).unwrap();
        assert!(most.x - least.x > 46.0, "{least} {most}");
        assert!(reach(0.0, 1.0).is_none() && reach(54.0, 1.0).is_none());
    }
}
