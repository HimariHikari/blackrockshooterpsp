//! Drawing the window of a mission cleared (0x45618), from its layout
//! (`if_fldsys.lpk`, files 12 to 14, with eighteen textures of that pack and of
//! the one that is always there): a white sheet over the field, the words
//! MISSION CLEAR, which come in from either side, stand a moment and go up to
//! head it, two rings turning behind them, the mission's title, and on the right
//! the challenges cleared in the mission, four rows of them at a time; at the
//! foot the button that sends it away.
//!
//! Its layout has three places: all of it but the list's rows, a row with a
//! challenge and a row without one, the last two drawn once for each row.

use anyhow::Result;
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Seen};
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_formats::layout::Channel;

use crate::data::GameData;
use crate::field::cleared::{set, Cleared, Phase, FRAMES_A_STEP, ROWS};
use crate::interface::{self, layer};
use crate::record_window::draw_small_picture;
use crate::text::{self, TextColours};
use crate::writing::{Align, Block, Fonts, Placing, Style};

/// The pack its layout is in and the file of it the layout starts at (0x453C8).
const PACK: &str = "SYSTEM.VOL/if_fldsys.lpk";
const LAYOUT: usize = 12;
/// The system's text for the button that sends it away (0x455E4).
const TEXT_NEXT: usize = 60;

/// How far apart the list's rows are (0x46488).
const ROW_PITCH: f32 = 20.0;
/// How much lower than its place the button's words are written (0x459A4).
const NEXT_LOWER: f32 = 2.0;

/// The groups of its layout that are more than something to draw.
mod group {
    /// The sheet over the screen.
    pub const SHEET: usize = 1;
    // Of all of it (0x166CD8): what the list's knob runs in, and the knob; the
    // two rings; where the mission's title and the button's words are written.
    pub const TROUGH: usize = 24;
    pub const KNOB: usize = 25;
    pub const RINGS: [usize; 2] = [35, 36];
    pub const TITLE: usize = 40;
    pub const NEXT: usize = 52;
    // Of a row with a challenge (0x166D50): its small picture, where its name is
    // written, and what a challenge that is not cleared yet has instead of the
    // word CLEAR, which is its hundredths and their sign.
    pub const SMALL_PICTURE: usize = 14;
    pub const NAME: usize = 15;
    pub const HUNDREDTHS: usize = 17;
    pub const PER_CENT: usize = 18;
}

// How its texts are written (0x44D24): the title and the names in the text's
// own font, thirty-six glyphs to a line; the button's words in the capitals at
// ten, to the right of their place.
const WORDS_STYLE: Style = Style::of_kind(0, 0x24, 1, 1.0, 4.0);
const NEXT_STYLE: Style = Style::of_kind(2, 0x24, 1, 1.0, 4.0);

/// What the window is drawn from.
pub struct ClearedLayout {
    layout: Layout,
    /// The sheet of the challenges' small pictures.
    small: (Handle<Image>, Vec2),
    /// The words of the button that sends it away.
    next: String,
}

pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<ClearedLayout> {
    // The layout's eighteen textures (0x45440 to 0x45568): of the pack that is
    // always there the first two, the figures, the small pictures and the
    // chequer; of the field's pack its files 15 to 22; and five places no object
    // is drawn from, which are the first here.
    let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
    let own = |file: usize| interface::file(PACK, file, "ptm");
    let unused = || shared(11);
    let textures = [
        shared(11),
        shared(10),
        unused(),
        own(15),
        unused(),
        unused(),
        own(16),
        shared(9),
        own(17),
        shared(20),
        shared(7),
        unused(),
        own(18),
        unused(),
        own(19),
        own(20),
        own(21),
        own(22),
    ];
    let layout = interface::layout_with(data, images, PACK, LAYOUT, &textures)?;
    let small = interface::load_texture(data, images, &shared(20))?;
    Ok(ClearedLayout { layout, small, next: interface::system_text(data, TEXT_NEXT)? })
}

/// Writes a text where a track's group has its object (0x2D900): moved as the
/// track is and by `by` besides, in the object's colour by the track's, and to
/// the right of the object's width if it is one that is.
#[allow(clippy::too_many_arguments)]
fn write(canvas: &mut Canvas, fonts: &Fonts, layout: &Layout, player: &Player, track: usize, words: &Block, by: Vec2, to_the_right: bool) {
    let Some((moved, colour)) = layout.placed(player, track) else { return };
    let Some(object) = player.group(track).and_then(|group| layout.first_of(group)) else { return };
    let (Some(place), size) = (layout.place(object), layout.objects.objects[object].size) else { return };
    let tint = modulate(layout.colour(object), colour);
    let placing = Placing { tint, ..Placing::at(place + moved.translation.trunc() + by) };
    let placing = if to_the_right { Placing { width: f32::from(size[0]), align: Align::Right, ..placing } } else { placing };
    fonts.draw(canvas, layer::DISPLAY, words, &placing);
}

/// The three places of the layout as the window has them playing, `ahead` of its
/// frames past its last one; and the turn each of the two rings is drawn by over
/// its own: as the window goes they keep the turn they had come to (0x44FD8).
fn places(layout: &Layout, shown: &Cleared, ahead: f32) -> ([Player; 3], [f32; 2]) {
    let going = matches!(shown.phase, Phase::Going | Phase::Gone);
    let short = !shown.many();
    let mut places: [Player; 3] = std::array::from_fn(|place| {
        let mut player = layout.player(set::COMING[place]);
        if place == 0 && short {
            player.play(layout, set::SHORT_LIST_COMING);
        }
        player.advance(layout, shown.shown + if going { 0.0 } else { ahead });
        player
    });
    if !going {
        return (places, [0.0; 2]);
    }
    let all = &places[0];
    let rings = group::RINGS.map(|group| all.track_of(group).and_then(|track| all.value(track, Channel::Turn)).unwrap_or(0.0));
    for (place, player) in places.iter_mut().enumerate() {
        player.play(layout, set::GOING[place]);
        if place == 0 && short {
            player.play(layout, set::SHORT_LIST_GOING);
        }
        player.advance(layout, shown.going + ahead);
    }
    (places, rings)
}

/// Draws the window as it is `ahead` of a frame of the game past its last one.
pub fn draw(canvas: &mut Canvas, what: &ClearedLayout, fonts: &Fonts, colours: &TextColours, shown: &Cleared, seen: &Seen, ahead: f32) {
    let (layout, layer) = (&what.layout, layer::DISPLAY);
    let ([all, row, bare], rings) = places(layout, shown, ahead * FRAMES_A_STEP);
    let written = |words: &str, style: &Style| fonts.write(style, &text::pages(words, &text::Plain).into_iter().next().unwrap_or_default(), colours);
    let height = |group: usize| layout.first_of(group).map_or(0.0, |object| f32::from(layout.objects.objects[object].size[1]));
    let plain = Look::default();

    for track in 0..all.len() {
        let Some(group) = all.group(track) else { continue };
        match group {
            // The sheet covers all that is seen, whatever the window's shape.
            group::SHEET => {
                if let Some((_, colour)) = layout.placed(&all, track) {
                    canvas.fill(layer, seen.0, colour);
                }
            }
            group::TITLE => write(canvas, fonts, layout, &all, track, &written(&shown.title, &WORDS_STYLE), Vec2::ZERO, false),
            group::NEXT => write(canvas, fonts, layout, &all, track, &written(&what.next, &NEXT_STYLE), Vec2::new(0.0, NEXT_LOWER), true),
            group if group::RINGS.contains(&group) => {
                let turn = rings[(group == group::RINGS[1]) as usize];
                layout.draw_track(canvas, layer, &all, track, &Look::inner(Affine2::from_angle(turn.to_radians())));
            }
            // The knob, for a list that moves (0x45BA0): as much of what it runs in
            // as four rows are of all the challenges, as far down it as the first
            // row shown is of them.
            group::KNOB => {
                if shown.many() {
                    let a_row = (height(group::TROUGH) - 2.0) / shown.entries.len() as f32;
                    let scale = Vec2::new(1.0, a_row * ROWS as f32 / height(group::KNOB));
                    let inner = Affine2::from_scale_angle_translation(scale, 0.0, Vec2::new(0.0, a_row * shown.top as f32));
                    layout.draw_track(canvas, layer, &all, track, &Look::inner(inner));
                }
            }
            _ => layout.draw_track(canvas, layer, &all, track, &plain),
        }
    }

    // The list's rows, each as far down as its number (0x45CD8).
    for line in 0..ROWS {
        let down = Vec2::new(0.0, line as f32 * ROW_PITCH);
        let look = Look { outer: Affine2::from_translation(down), ..plain };
        let Some(entry) = shown.entries.get(shown.top + line) else {
            layout.draw(canvas, layer, &bare, &look);
            continue;
        };
        for track in 0..row.len() {
            let Some(group) = row.group(track) else { continue };
            match group {
                group::SMALL_PICTURE => {
                    let object = layout.first_of(group).and_then(|object| Some((object, layout.place(object)?)));
                    let (Some((moved, colour)), Some((object, place))) = (layout.placed(&row, track), object) else { continue };
                    let at = (place + moved.translation + down).trunc();
                    draw_small_picture(canvas, layer, &what.small, entry.icon, at, modulate(layout.colour(object), colour));
                }
                group::NAME => write(canvas, fonts, layout, &row, track, &written(&entry.name, &WORDS_STYLE), down, false),
                // What a challenge that is not cleared yet would have (0x45FEC,
                // 0x46024): its hundredths in figures and their sign, and its bar
                // cut to how far it has come (0x46144). Every challenge listed
                // has been cleared, so they never are: the word CLEAR and the
                // whole bar are drawn as they are.
                group::HUNDREDTHS | group::PER_CENT => {}
                _ => layout.draw_track(canvas, layer, &row, track, &look),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{button, Pad};
    use crate::field::cleared::{Entry, COMING, GOING};

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    fn loaded() -> Option<(ClearedLayout, Fonts, TextColours)> {
        let data = GameData::new(DATA).ok()?;
        let mut images = Assets::<Image>::default();
        let what = load(&data, &mut images).unwrap();
        Some((what, Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    fn entries(count: usize) -> Vec<Entry> {
        (0..count).map(|number| Entry { name: format!("challenge {number}"), icon: 0 }).collect()
    }

    /// A window with so many challenges that has come and asks.
    fn asking(count: usize) -> Cleared {
        let mut cleared = Cleared::new("The Final Weapon Awakens".into());
        cleared.list(|| entries(count));
        while cleared.phase != Phase::Asking {
            assert!(cleared.step());
        }
        cleared
    }

    #[test]
    fn the_windows_timing_is_that_of_its_layout() {
        let Some((what, ..)) = loaded() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let layout = &what.layout;
        assert_eq!((layout.objects.objects.len(), layout.groups.groups.len(), layout.motions.sets.len(), layout.textures.len()), (65, 53, 8, 18));
        assert_eq!(what.next, "\\b(01)NEXT");
        // All of it has come when its first place has played its set through, and
        // gone when that has played the set that takes it away.
        let played = |player: &mut Player| {
            let mut frames = 0.0;
            while !player.finished(layout) {
                player.advance(layout, FRAMES_A_STEP);
                frames += FRAMES_A_STEP;
                assert!(frames < 1000.0);
            }
            frames
        };
        let mut all = layout.player(set::COMING[0]);
        assert_eq!(played(&mut all), COMING);
        all.play(layout, set::GOING[0]);
        assert_eq!(played(&mut all), GOING);
        // The sets of a short list are of what the knob runs in alone.
        let groups = |set: usize| layout.motions.sets[set].iter().map(|track| track.group as usize).collect::<Vec<_>>();
        assert_eq!((groups(set::SHORT_LIST_COMING), groups(set::SHORT_LIST_GOING)), (vec![group::TROUGH], vec![group::TROUGH]));
        // The objects its texts and its knob go by.
        let object = |group: usize| layout.first_of(group).map(|object| (layout.objects.objects[object].position, layout.objects.objects[object].size));
        assert_eq!(object(group::TITLE), Some(([36, 129], [180, 15])));
        assert_eq!(object(group::NEXT), Some(([12, 250], [457, 15])));
        assert_eq!(object(group::NAME), Some(([267, 161], [150, 15])));
        assert_eq!(object(group::SMALL_PICTURE), Some(([251, 161], [15, 15])));
        assert_eq!((object(group::TROUGH).map(|(_, size)| size[1]), object(group::KNOB).map(|(_, size)| size[1])), (Some(77), Some(34)));
    }

    #[test]
    fn it_is_drawn_over_the_screen_with_a_row_for_each_challenge() {
        let Some((what, fonts, colours)) = loaded() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let drawn = |cleared: &Cleared| {
            let mut canvas = Canvas::default();
            draw(&mut canvas, &what, &fonts, &colours, cleared, &Seen::default(), 0.0);
            (canvas.vertices().to_vec(), canvas.drawn_from(&what.small.0))
        };
        // Nothing of it is there as it starts.
        let (all, _) = drawn(&Cleared::new(String::new()));
        assert!(all.iter().all(|vertex| vertex.colour[3] == 0), "{:?}", all.first());

        // Come, with two challenges: a small picture for each, twenty apart, and
        // none for the two rows without one.
        let cleared = asking(2);
        let (all, pictures) = drawn(&cleared);
        let picture = |at: Vec2| pictures.iter().any(|vertex| vertex.position == at);
        assert!(picture(Vec2::new(251.0, 161.0)) && picture(Vec2::new(266.0, 196.0)));
        assert!(!picture(Vec2::new(251.0, 201.0)) && !picture(Vec2::new(251.0, 221.0)));
        // Its sheet is over all of the screen.
        let corner = |at: Vec2| all.iter().any(|vertex| vertex.position == at);
        assert!(corner(Vec2::ZERO) && corner(Vec2::new(480.0, 272.0)));
        // A list of four or fewer has no knob; one of six has, and it is as much
        // of the seventy-five it runs in as four rows are of six, moved down as
        // the list is.
        let knob = |cleared: &Cleared| {
            let object = &what.layout.objects.objects[what.layout.first_of(group::KNOB).unwrap()];
            let left = f32::from(object.position[0]);
            let its = |vertex: &&brs_engine::canvas::Vertex| vertex.position.x == left && matches!(vertex.colour[..3], [0x0e, 0xb2, 0xff] | [0x00, 0x48, 0xff]);
            let ys: Vec<f32> = drawn(cleared).0.iter().filter(its).map(|vertex| vertex.position.y).collect();
            ys.iter().copied().reduce(f32::min).zip(ys.iter().copied().reduce(f32::max))
        };
        let near = |ends: Option<(f32, f32)>, top: f32, foot: f32| ends.is_some_and(|ends| (ends.0 - top).abs() < 1e-3 && (ends.1 - foot).abs() < 1e-3);
        assert_eq!(knob(&cleared), None);
        let mut long = asking(6);
        assert!(near(knob(&long), 162.0, 212.0), "{:?}", knob(&long));
        long.take_buttons(&Pad { pressed: button::DOWN, held: button::DOWN, repeat: button::DOWN, ..Pad::default() });
        assert!(near(knob(&long), 174.5, 224.5), "{:?}", knob(&long));

        // As it goes all of it fades, and gone it is not there.
        let mut cleared = asking(2);
        cleared.take_buttons(&Pad { pressed: button::CROSS, ..Pad::default() });
        while cleared.step() {}
        assert_eq!(cleared.phase, Phase::Gone);
        let (all, _) = drawn(&cleared);
        assert!(all.iter().all(|vertex| vertex.colour[3] == 0));
    }
}
