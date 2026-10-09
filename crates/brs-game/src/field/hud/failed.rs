//! Drawing the window of a mission failed (0x6EC74), from its layout
//! (`IF/MENU/IF_MISSION_FAILED.GZ`): a dark sheet over the field with a pattern
//! drifting up it, "MISSION FAILED" by the game's emblem, and the three choices
//! at the foot on the right, the one the cursor is on in blue.
//!
//! The menu draws its player's tracks one by one, each by a colour of its own
//! choosing: all of it as it is, but "STAGE SELECT" at a quarter while that is
//! not to be had, and, once a choice is made, the drifting pattern and the two
//! rings about the emblem fading as the menu counts, which no set does for them.

use anyhow::Result;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};

use crate::data::GameData;
use crate::field::failed::{set, Failed, Phase, FRAMES_A_STEP};
use crate::interface::{self, layer};

/// The window's own file: three textures, and then its layout.
const FILE: &str = "IF/MENU/IF_MISSION_FAILED.GZ";
const LAYOUT: usize = 3;

/// The sixteen textures the menu gives its layout (0x6EAE8): one of the pack that
/// is always there, the file's own three, and four of the field's pack. The other
/// eight it leaves empty and nothing is drawn from; they are the first here.
fn textures() -> [String; 16] {
    let own = |file: usize| interface::file(FILE, file, "ptm");
    let field = |file: usize| interface::file(super::FIELD_PACK, file, "ptm");
    let unused = || own(0);
    [
        interface::file(interface::RESIDENT_PACK, 11, "ptm"),
        own(0),
        own(1),
        field(15),
        unused(),
        unused(),
        field(16),
        unused(),
        unused(),
        unused(),
        unused(),
        unused(),
        unused(),
        own(2),
        field(19),
        field(20),
    ]
}

/// The window's layout, with its textures.
pub fn layout(data: &GameData, images: &mut Assets<Image>) -> Result<Layout> {
    interface::layout_with(data, images, FILE, LAYOUT, &textures())
}

/// The group of "STAGE SELECT", which is drawn faint while it cannot be chosen
/// (0x6ED30), and how faint.
const STAGE_SELECT_GROUP: usize = 10;
const OUT_OF_REACH: u8 = 0x40;
/// The groups the menu fades itself as a choice goes (0x6EE04): the pattern that
/// drifts up the sheet, over sixty frames, and the two rings, over fifty.
const PATTERN_GROUP: usize = 6;
const RING_GROUPS: [usize; 2] = [19, 20];

/// What the menu's player has come to: the sets it has played, each from the
/// frame it was played at.
fn playing(layout: &Layout, failed: &Failed, ahead: f32) -> Player {
    let mut plays = failed.plays.iter();
    let Some(&(first, mut at)) = plays.next() else { return layout.player(set::COMING) };
    let mut player = layout.player(first);
    for &(set, when) in plays {
        player.advance(layout, when - at);
        player.play(layout, set);
        at = when;
    }
    player.advance(layout, failed.clock + ahead - at);
    player
}

/// Draws the window as it is `ahead` of a frame of the game past its last one.
pub fn draw(canvas: &mut Canvas, layout: &Layout, failed: &Failed, ahead: f32) {
    // Gone, the screen is dark and the field is being left.
    if failed.phase == Phase::Gone {
        return;
    }
    let ahead = ahead * FRAMES_A_STEP;
    let player = playing(layout, failed, ahead);
    let going = failed.phase == Phase::Going;
    let (pattern, rings) = failed.going_shares(ahead);
    let part = |share: f32| (share * 255.0) as u8;
    for track in 0..player.len() {
        let alpha = match player.group(track) {
            Some(STAGE_SELECT_GROUP) if failed.no_stage_select => OUT_OF_REACH,
            Some(PATTERN_GROUP) if going => part(pattern),
            Some(group) if going && RING_GROUPS.contains(&group) => part(rings),
            _ => 255,
        };
        layout.draw_track(canvas, layer::FAILED, &player, track, &Look::tinted([255, 255, 255, alpha]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{button, Pad};
    use crate::field::failed::{COMING, GOING, LOAD, RETRY, STAGE_SELECT};
    use crate::field::scene::Fade;
    use brs_engine::canvas::Vertex;
    use brs_formats::layout::Channel;

    fn game() -> Option<Layout> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        Some(layout(&data, &mut images).unwrap())
    }

    fn drawn(layout: &Layout, failed: &Failed) -> Vec<Vertex> {
        let mut canvas = Canvas::default();
        draw(&mut canvas, layout, failed, 0.0);
        canvas.vertices().to_vec()
    }

    /// The colour a choice is drawn in.
    fn colour_of(layout: &Layout, failed: &Failed, choice: usize) -> [u8; 4] {
        let player = playing(layout, failed, 0.0);
        let track = player.track_of(9 + choice).expect("the choice has a track");
        layout.placed(&player, track).map_or([0; 4], |(_, colour)| colour)
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    #[test]
    fn the_menus_timing_is_that_of_its_layout() {
        let Some(layout) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!((layout.objects.objects.len(), layout.textures.len(), layout.motions.sets.len()), (28, 16, 14));
        // All of it has come when every track has played its clip through, those
        // that go round apart: the choice the cursor is on is the last.
        for choice in [RETRY, STAGE_SELECT, LOAD] {
            let mut player = layout.player(set::COMING);
            player.play(&layout, set::COMING_CHOSEN[choice]);
            let mut frames = 0.0;
            while !player.finished(&layout) {
                player.advance(&layout, FRAMES_A_STEP);
                frames += FRAMES_A_STEP;
                assert!(frames < 1000.0);
            }
            assert_eq!(frames, COMING);
            // And a choice made has gone, with nothing else left to play.
            player.play(&layout, set::MADE[choice]);
            let mut frames = 0.0;
            while !player.finished(&layout) {
                player.advance(&layout, FRAMES_A_STEP);
                frames += FRAMES_A_STEP;
                assert!(frames < 1000.0);
            }
            assert_eq!(frames, GOING);
        }
        // Each set of a choice moves that choice alone.
        for sets in [set::ONTO, set::OFF, set::MADE, set::COMING_CHOSEN] {
            for (choice, &set) in sets.iter().enumerate() {
                let groups: Vec<u16> = layout.motions.sets[set].iter().map(|track| track.group).collect();
                assert_eq!(groups, [9 + choice as u16]);
            }
        }
    }

    #[test]
    fn it_comes_over_the_field_with_the_cursor_on_the_first_choice() {
        let Some(layout) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut failed = Failed::new(false);
        let mut fade = Fade::CLEAR;
        let extent = |vertices: &[Vertex]| vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position));
        // As it starts nothing of it is to be seen: all there is are the emblem's
        // parts, which have no size yet.
        let start = extent(&drawn(&layout, &failed));
        assert!(start.width() <= 1.0 && start.height() <= 2.0, "{start:?}");
        while failed.phase == Phase::Coming {
            failed.step(&Pad::default(), &mut fade);
        }
        // Up, it covers the screen, and the choice under the cursor is blue while
        // the others are white, three quarters there.
        let area = extent(&drawn(&layout, &failed));
        assert!(area.min.x <= 0.0 && area.min.y <= 0.0 && area.max.x >= 480.0 && area.max.y >= 272.0);
        assert_eq!(colour_of(&layout, &failed, RETRY), [30, 130, 255, 255]);
        assert_eq!(colour_of(&layout, &failed, STAGE_SELECT), [255, 255, 255, 191]);
        assert_eq!(colour_of(&layout, &failed, LOAD), [255, 255, 255, 191]);
        // The cursor moved, the one it has left goes white over twelve frames and
        // the one it is on is blue within two.
        failed.step(&press(button::DOWN), &mut fade);
        failed.step(&Pad::default(), &mut fade);
        assert_eq!(colour_of(&layout, &failed, STAGE_SELECT), [30, 130, 255, 255]);
        for _ in 0..6 {
            failed.step(&Pad::default(), &mut fade);
        }
        assert_eq!(colour_of(&layout, &failed, RETRY), [255, 255, 255, 191]);
    }

    #[test]
    fn what_cannot_be_chosen_is_faint_and_what_is_chosen_goes() {
        let Some(layout) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // "STAGE SELECT" stands between the two other choices, at three quarters
        // of its alpha like them: out of reach it is drawn by a quarter of that.
        let stage_select = |vertex: &&Vertex| (339.0..=464.0).contains(&vertex.position.x) && (211.0..=221.0).contains(&vertex.position.y);
        let alphas_of = |failed: &Failed| -> Vec<u8> { drawn(&layout, failed).iter().rev().filter(stage_select).take(6).map(|vertex| vertex.colour[3]).collect() };
        let mut fade = Fade::CLEAR;
        let (mut open, mut closed) = (Failed::new(false), Failed::new(true));
        for failed in [&mut open, &mut closed] {
            while failed.phase == Phase::Coming {
                failed.step(&Pad::default(), &mut fade);
            }
        }
        assert_eq!(alphas_of(&open), [191; 6]);
        assert_eq!(alphas_of(&closed), [(191 * OUT_OF_REACH as u32 / 255) as u8; 6]);

        // A choice made fades over fifty frames, and with it the rings; the pattern
        // takes sixty. Gone, nothing is drawn.
        open.step(&press(button::CROSS), &mut fade);
        assert_eq!(open.phase, Phase::Going);
        for _ in 0..12 {
            fade.advance();
            open.step(&Pad::default(), &mut fade);
        }
        let player = playing(&layout, &open, 0.0);
        let chosen = player.track_of(9).unwrap();
        let alpha = player.value(chosen, Channel::Alpha).unwrap();
        assert!((alpha - (255.0 - 255.0 * 26.0 / 50.0)).abs() < 1e-3, "{alpha}");
        let (pattern, rings) = open.going_shares(0.0);
        assert!((pattern - (1.0 - 26.0 / 60.0)).abs() < 1e-6 && (rings - (1.0 - 26.0 / 50.0)).abs() < 1e-6);
        while open.phase != Phase::Gone {
            fade.advance();
            open.step(&Pad::default(), &mut fade);
        }
        assert!(drawn(&layout, &open).is_empty());
    }
}
