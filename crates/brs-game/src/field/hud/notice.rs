//! Drawing the notice a mission starts with (0x44864), from its layout
//! (`if_fldsys.lpk`, files 23 to 25, with file 18 for its texture): a band across
//! the middle of the screen, and on it the mission's title.

use brs_engine::canvas::Canvas;
use brs_engine::layout::{modulate, Layout, Look, Player};

use crate::field::scene::Notice;
use crate::interface::layer;
use crate::text::{self, TextColours};
use crate::writing::{Fonts, Placing, Style};

/// The sets that bring it and take it (0x44798, 0x447C8).
pub const SET_COMING: usize = 0;
pub const SET_GOING: usize = 1;
/// The group whose object the title is written at, in its colour (0x448EC).
const TITLE_GROUP: usize = 4;
/// How the title is written (0x446C0): forty-eight glyphs on one line.
const TITLE_STYLE: Style = Style::of_kind(0, 0x30, 1, 1.0, 4.0);

/// Draws the notice as it is `ahead` of a frame of the game past its last one.
pub fn draw(canvas: &mut Canvas, layout: &Layout, fonts: &Fonts, colours: &TextColours, notice: &Notice, ahead: f32) {
    let layer = layer::DISPLAY;
    let set = if notice.going { SET_GOING } else { SET_COMING };
    let player = Player::at(layout, set, notice.frames + ahead * 2.0);
    for track in 0..player.len() {
        if player.group(track) != Some(TITLE_GROUP) {
            layout.draw_track(canvas, layer, &player, track, &Look::default());
            continue;
        }
        let Some((moved, colour)) = layout.placed(&player, track) else { continue };
        let Some((mark, place)) = layout.first_of(TITLE_GROUP).and_then(|object| Some((object, layout.place(object)?))) else { continue };
        let words = text::pages(&notice.text, &text::Plain).swap_remove(0);
        let title = fonts.write(&TITLE_STYLE, &words, colours);
        let at = place + moved.translation.trunc();
        fonts.draw(canvas, layer, &title, &Placing { tint: modulate(layout.colour(mark), colour), ..Placing::at(at) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::field::hud::notice_layout;
    use bevy::prelude::*;

    #[test]
    fn the_notice_comes_and_goes_in_the_frames_its_layout_has() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = notice_layout(&data, &mut images).unwrap();
        assert_eq!((layout.length(SET_COMING), layout.length(SET_GOING)), (Notice::COMING, Notice::GOING));
        // Its jingle is as long as it is taken to be.
        let jingle = crate::field::event::frames(&data.read(Notice::JINGLE).unwrap());
        assert_eq!(jingle, Some(Notice::JINGLE_FRAMES));

        // Up, its title is written at (257, 135), in black.
        let (fonts, colours) = (Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap());
        let mut notice = Notice::new("The Final Weapon Awakens".into(), false);
        let mut canvas = Canvas::default();
        draw(&mut canvas, &layout, &fonts, &colours, &notice, 0.0);
        assert!(canvas.vertices().is_empty(), "nothing of it is there as it starts");
        for _ in 0..6 {
            assert!(notice.step());
        }
        draw(&mut canvas, &layout, &fonts, &colours, &notice, 0.0);
        let vertices = canvas.vertices();
        let glyphs = "TheFinalWeaponAwakens".len();
        // (The last thing drawn is the mark beside the title.)
        let title = &vertices[vertices.len() - (glyphs + 1) * 6..vertices.len() - 6];
        let left = title.iter().map(|vertex| vertex.position.x).fold(f32::MAX, f32::min);
        let top = title.iter().map(|vertex| vertex.position.y).fold(f32::MAX, f32::min);
        assert_eq!((left, top), (257.0, 135.0));
        assert!(title.iter().all(|vertex| vertex.colour[..3] == [0, 0, 0]));
        // It stands until its jingle is over, then goes over three frames.
        let mut frames = 6;
        while notice.step() {
            frames += 1;
            assert!(frames < 1000);
        }
        assert_eq!(frames, Notice::JINGLE_FRAMES as usize + 3);
    }
}
