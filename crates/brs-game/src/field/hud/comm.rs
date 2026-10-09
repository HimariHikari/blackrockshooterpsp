//! Drawing the window people speak to her through (0x39F2C), from its layout
//! (`if_fldsys.lpk/000.lpk`).
//!
//! The window's task keeps eight players of the layout: one for the body, one for
//! the answers, and for each of the two places one for the face's frame, one for
//! what goes over the face and one for the marks by the speaker's name. They are
//! made here anew for every frame drawn, from the frames `field/comm` has counted
//! each of them to have played: nothing of the layout plays differently for being
//! played in one go.
//!
//! What the layout does not have is put where objects of it mark: what is said,
//! across the middle of the screen; the two answers; each face's picture, with
//! lines and noise over it that are cut to its rectangle; and the speaker's name,
//! which is two objects of a file of the face's own.

use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{self, modulate, Layout, Look, Player};
use brs_formats::layout::Objects;

use crate::field::comm::{set, Answers, Comm, Shown, FRAMES_A_STEP, PLACES};
use crate::interface::layer;
use crate::text::{self, TextColours};
use crate::writing::{Align, Fonts, Placing, Style};

/// How what is said is written (0x38C7C): thirty-six glyphs to a line and three
/// lines, a pixel between glyphs and four between lines; and each of the answers
/// (0x38CB0), thirty-two glyphs on one line.
const TEXT_STYLE: Style = Style::of_kind(0, 0x24, 3, 1.0, 4.0);
const ANSWER_STYLE: Style = Style::of_kind(0, 0x20, 1, 1.0, 4.0);

// The groups of the body that are not drawn: their tracks' colours are what the
// text and the speaker's name are drawn in (0x39FEC, 0x3A024).
const TEXT_GROUP: usize = 0x14;
const NAME_GROUP: usize = 0x22;
/// The objects that say how high what is said stands: a single line, and more
/// (0x3A0A0).
const TEXT_OF_ONE_LINE: usize = 0x15;
const TEXT_OF_MORE: usize = 0x16;
const LINE_HEIGHT: f32 = 15.0;

// The groups of the answers (0x3A248 on), the first answer's and the second's: where
// each is written and the button that gives it, as they are and as they are once
// chosen.
const ANSWER_TEXT: [usize; 2] = [9, 0xa];
const ANSWER_TEXT_CHOSEN: [usize; 2] = [0xb, 0xc];
const ANSWER_BUTTON: [usize; 2] = [0x3d, 0x3f];
const ANSWER_BUTTON_CHOSEN: [usize; 2] = [0x3e, 0x40];

/// The group of each place's frame whose track's colour the face is drawn in, and
/// the object whose rectangle it is drawn in (0x3A5B4, 0x3A5DC; 0x3A9B4, 0x3A9DC).
const FACE_GROUP: [usize; PLACES] = [0x26, 0x32];
const FACE_OBJECT: [usize; PLACES] = [40, 55];
/// The objects of a name's file that are drawn (0x3A7A8): its shadow, and itself.
const NAME_OBJECTS: [usize; 2] = [1, 2];
/// What is laid over a face while the other place's is speaking (0x3A808): so much
/// in 255 of the colour the face is drawn in, red, green, blue and alpha.
const DIMMED: [u32; 4] = [27, 27, 27, 64];

/// What a face is drawn from: its picture, and the objects and the texture of its
/// name.
pub struct FaceArt {
    pub picture: Handle<Image>,
    pub picture_size: Vec2,
    pub name: Objects,
    pub name_textures: Vec<Handle<Image>>,
}

/// Draws the window as it is `ahead` of a frame of the game past its last one.
pub fn draw(canvas: &mut Canvas, layout: &Layout, fonts: &Fonts, colours: &TextColours, comm: &Comm, art: [Option<&FaceArt>; PLACES], ahead: f32) {
    let shown = &comm.window;
    let ahead = ahead * FRAMES_A_STEP;
    let layer = layer::COMM;
    let mut name_colour = None;
    if shown.open {
        let body = body(layout, shown, ahead);
        let mut text_colour = None;
        for track in 0..body.len() {
            match body.group(track) {
                Some(TEXT_GROUP) => text_colour = layout.placed(&body, track).map(|(_, colour)| colour),
                Some(NAME_GROUP) => name_colour = layout.placed(&body, track).map(|(_, colour)| colour),
                _ => layout.draw_track(canvas, layer, &body, track, &Look::default()),
            }
        }
        // What is said: across the middle of the screen, as high as the object for
        // its number of lines is.
        if let (Some(line), Some(colour)) = (comm.line(), text_colour) {
            let block = fonts.write(&TEXT_STYLE, &line.text, colours);
            let mark = if block.size().y <= LINE_HEIGHT { TEXT_OF_ONE_LINE } else { TEXT_OF_MORE };
            let top = layout.place(mark).map_or(0.0, |place| place.y);
            let tint = modulate(layout.colour(mark), colour);
            fonts.draw(canvas, layer, &block, &Placing { align: Align::Centre, tint, shadow: true, ..Placing::at(Vec2::new(0.0, top)) });
        }
        if shown.answers != Answers::None {
            draw_answers(canvas, layout, fonts, colours, comm, ahead);
        }
    }

    for place in 0..PLACES {
        let face = &shown.faces[place];
        let Some(area) = layout.objects.objects.get(FACE_OBJECT[place]).filter(|_| face.shown) else { continue };
        let from = Vec2::new(area.position[0] as f32, area.position[1] as f32);
        let size = Vec2::new(area.size[0] as f32, area.size[1] as f32);
        let area = Rect::from_corners(from, from + size);

        // The face's frame, and in it the picture, in the colour its track has
        // come to.
        let frame = Player::at(layout, if face.coming { set::FACE_IN[place] } else { set::FACE_OUT[place] }, face.frames + ahead);
        let mut colour = [0; 4];
        for track in 0..frame.len() {
            if frame.group(track) != Some(FACE_GROUP[place]) {
                layout.draw_track(canvas, layer, &frame, track, &Look::default());
                continue;
            }
            colour = layout.placed(&frame, track).map_or([0; 4], |(_, colour)| colour);
            if let Some(art) = art[place] {
                let paint = Paint::textured(&art.picture);
                canvas.sprite(layer, paint, Vertex::new(area.min, Vec2::ZERO, colour), Vertex::new(area.max, size / art.picture_size, colour));
            }
        }
        // What goes over the face is cut to the face's rectangle.
        canvas.cut_to(Some(area));
        layout.draw(canvas, layer, &Player::at(layout, set::OVER_FACE[place], face.over + ahead), &Look::tinted(colour));
        canvas.cut_to(None);

        let other = PLACES - 1 - place;
        if shown.speaker == place + 1 {
            // Whoever is speaking has the marks by the name moving, and the name.
            let marks = Player::at(layout, set::SPEAKING[place], shown.speaking_for[place] + ahead);
            layout.draw(canvas, layer, &marks, &Look::tinted(colour));
            if let (Some(name_colour), Some(art)) = (name_colour, art[place]) {
                for object in NAME_OBJECTS.iter().filter_map(|&object| art.name.objects.get(object)) {
                    layout::draw_object(canvas, layer, object, &art.name_textures, Affine2::IDENTITY, name_colour, Affine2::IDENTITY, Vec2::ZERO);
                }
            }
        } else if shown.speaker == other + 1 && shown.faces[other].shown {
            // The one who is not is darkened.
            canvas.fill(layer, area, std::array::from_fn(|part| (colour[part] as u32 * DIMMED[part] / 255) as u8));
        }
    }
}

/// The player of the window's body as it is: opened, with the sign that a line can
/// be passed come or gone since, or closing.
fn body(layout: &Layout, shown: &Shown, ahead: f32) -> Player {
    if shown.closing {
        return Player::at(layout, set::CLOSE, shown.closing_for + ahead);
    }
    let mut player = layout.player(set::OPEN);
    match shown.sign {
        Some(sign) => {
            player.advance(layout, shown.body - sign.frames);
            player.play(layout, if sign.coming { set::SIGN_IN } else { set::SIGN_OUT });
            player.advance(layout, sign.frames + ahead);
        }
        None => {
            player.advance(layout, shown.body + ahead);
        }
    }
    player
}

/// The player of the answers as it is: come up, with one chosen since, or going.
fn answers(layout: &Layout, shown: &Shown, ahead: f32) -> Player {
    match shown.answers {
        Answers::Going => Player::at(layout, set::ANSWERS_OUT, shown.going_for + ahead),
        Answers::Chosen => {
            let mut player = Player::at(layout, set::ANSWERS_IN, shown.answers_for - shown.answered_for);
            player.play(layout, set::ANSWERED);
            player.advance(layout, shown.answered_for + ahead);
            player
        }
        Answers::Up | Answers::None => Player::at(layout, set::ANSWERS_IN, shown.answers_for + ahead),
    }
}

/// The answers (0x3A210): the box they are in, and of each its text and its button
/// as they are, or as they are once chosen for the one that has been.
fn draw_answers(canvas: &mut Canvas, layout: &Layout, fonts: &Fonts, colours: &TextColours, comm: &Comm, ahead: f32) {
    let shown = &comm.window;
    let layer = layer::COMM;
    let player = answers(layout, shown, ahead);
    let chosen = |which: usize| shown.answer == which as i32;
    for track in 0..player.len() {
        let Some(group) = player.group(track) else { continue };
        let among = |groups: [usize; 2]| groups.iter().position(|&of| of == group);
        let mut written = None;
        if let Some(which) = among(ANSWER_BUTTON_CHOSEN) {
            if chosen(which) {
                layout.draw_track(canvas, layer, &player, track, &Look::default());
            }
        } else if let Some(which) = among(ANSWER_BUTTON) {
            if !chosen(which) {
                layout.draw_track(canvas, layer, &player, track, &Look::default());
            }
        } else if let Some(which) = among(ANSWER_TEXT_CHOSEN) {
            written = chosen(which).then_some(which);
        } else if let Some(which) = among(ANSWER_TEXT) {
            written = (!chosen(which)).then_some(which);
        } else {
            layout.draw_track(canvas, layer, &player, track, &Look::default());
        }

        // An answer is written in the middle of its group's object, in that
        // object's colour by its track's.
        let (Some(which), Some(answers)) = (written, comm.asked()) else { continue };
        let Some((_, colour)) = layout.placed(&player, track) else { continue };
        let Some((mark, area)) = layout.first_of(group).and_then(|object| Some((object, layout.objects.objects.get(object)?))) else { continue };
        let words = text::pages(&answers[which], &text::Plain).swap_remove(0);
        let block = fonts.write(&ANSWER_STYLE, &words, colours);
        let placing = Placing {
            width: area.size[0] as f32,
            align: Align::Centre,
            tint: modulate(layout.colour(mark), colour),
            shadow: true,
            ..Placing::at(Vec2::new(area.position[0] as f32, area.position[1] as f32))
        };
        fonts.draw(canvas, layer, &block, &placing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{button, Pad};
    use crate::data::GameData;
    use crate::field::comm::{frames, Line, Voice};
    use crate::field::hud::comm_layout;

    fn game() -> Option<(Layout, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let layout = comm_layout(&data, &mut images).unwrap();
        Some((layout, Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    fn drawn(layout: &Layout, fonts: &Fonts, colours: &TextColours, comm: &Comm) -> Vec<Vertex> {
        let mut canvas = Canvas::default();
        draw(&mut canvas, layout, fonts, colours, comm, [None, None], 0.0);
        canvas.vertices().to_vec()
    }

    fn extent(vertices: &[Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    fn line(words: &str) -> Line {
        Line::new(0, text::pages(words, &text::Plain).remove(0), Voice::Frames(400), -1)
    }

    fn after(mut comm: Comm, count: usize) -> Comm {
        for _ in 0..count {
            comm.step(&Pad::default(), false);
        }
        comm
    }

    #[test]
    fn the_motions_take_as_long_as_the_window_waits_for_them() {
        let Some((layout, _, _)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!((layout.objects.objects.len(), layout.textures.len(), layout.motions.sets.len()), (76, 8, 15));
        let played_through = |set: usize| (0..layout.motions.sets[set].len()).map(|track| layout.track_length(set, track)).fold(0.0, f32::max);
        assert_eq!(layout.length(set::CLOSE), frames::CLOSE);
        assert_eq!(layout.length(set::ANSWERS_OUT), frames::ANSWERS_OUT);
        assert_eq!(played_through(set::ANSWERS_IN), frames::ANSWERS_IN);
        assert_eq!(played_through(set::ANSWERED), frames::ANSWERED);
        for place in 0..PLACES {
            assert_eq!(played_through(set::FACE_IN[place]), frames::FACE_IN);
            assert_eq!(layout.length(set::FACE_OUT[place]), frames::FACE_OUT);
        }
        // A set played over another has the same groups in the same order, but for
        // those that change one part of what is there.
        let groups = |set: usize| layout.motions.sets[set].iter().map(|track| track.group).collect::<Vec<u16>>();
        assert_eq!(groups(set::OPEN), groups(set::CLOSE));
        assert_eq!(groups(set::ANSWERS_IN), groups(set::ANSWERS_OUT));
        assert_eq!((groups(set::SIGN_IN), groups(set::SIGN_OUT)), (vec![65], vec![65]));
        assert_eq!(groups(set::ANSWERED), [0xb, 0xc, 0x3e, 0x40]);
        for place in 0..PLACES {
            assert_eq!(groups(set::FACE_IN[place]), groups(set::FACE_OUT[place]));
        }
    }

    #[test]
    fn what_is_said_stands_across_the_middle_between_the_faces() {
        let Some((layout, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut comm = Comm::default();
        comm.open_for("bundle".into());
        // As it opens there is nothing of it yet.
        assert!(drawn(&layout, &fonts, &colours, &comm).is_empty());
        comm.say(line("Kid, can you hear me!?"));
        let open = after(comm, 10);
        let all = drawn(&layout, &fonts, &colours, &open);
        // The body lies between the two places, and the text in the middle of the
        // screen, a line of it 32 down.
        let area = extent(&all);
        assert_eq!((area.min.x, area.max.x), (90.0, 390.0));
        let glyphs = 2 * "Kid,canyouhearme!?".len();
        let text = extent(&all[all.len() - glyphs * 6..]);
        assert_eq!(text.min.y, 32.0);
        assert!((text.center().x - 240.0).abs() < 8.0, "{text:?}");
        // Two lines stand higher.
        let mut comm = Comm::default();
        comm.open_for("bundle".into());
        comm.say(line("More enemies inbound! Be quick\nor they'll just keep coming!"));
        let all = drawn(&layout, &fonts, &colours, &after(comm, 10));
        let glyphs = 2 * "Moreenemiesinbound!Bequickorthey'lljustkeepcoming!".len();
        assert_eq!(extent(&all[all.len() - glyphs * 6..]).min.y, 26.0);
    }

    #[test]
    fn a_face_is_drawn_in_its_place_and_the_answers_under_the_body() {
        let Some((layout, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut comm = Comm::default();
        comm.open_for("bundle".into());
        comm.put_face(0, 1);
        comm.put_face(1, 2);
        comm.choices = Some(["Yes.".to_string(), "No.".to_string()]);
        comm.say(line("Kid, can you hear me!?"));
        let mut comm = after(comm, 12);
        // The two frames reach from 4 to 476 across.
        let area = extent(&drawn(&layout, &fonts, &colours, &comm));
        assert_eq!((area.min.x, area.max.x), (4.0, 476.0));
        assert!(area.max.y <= 95.0);
        // Cross brings the answers up: their box is under the body.
        let cross = Pad { pressed: button::CROSS, held: button::CROSS, ..Pad::default() };
        comm.step(&cross, false);
        let comm = after(comm, 6);
        let area = extent(&drawn(&layout, &fonts, &colours, &comm));
        assert_eq!((area.min.x, area.max.x, area.max.y), (4.0, 476.0, 94.0));
        // Nothing of what goes over a face is drawn outside the face.
        let mut canvas = Canvas::default();
        let over = Player::at(&layout, set::OVER_FACE[0], 60.0);
        canvas.cut_to(Some(Rect::new(10.0, 10.0, 82.0, 88.0)));
        layout.draw(&mut canvas, 0, &over, &Look::default());
        let cut = extent(canvas.vertices());
        assert!(cut.min.x >= 10.0 && cut.max.x <= 82.0 && cut.min.y >= 10.0 && cut.max.y <= 88.0, "{cut:?}");

        // With no faces the answers are the last things drawn: the second answer is
        // written in the right half of the box, before the two buttons' signs.
        let mut comm = Comm::default();
        comm.open_for("bundle".into());
        comm.choices = Some(["Yes.".to_string(), "No.".to_string()]);
        comm.say(line("Kid, can you hear me!?"));
        let mut comm = after(comm, 4);
        comm.step(&cross, false);
        let comm = after(comm, 6);
        let all = drawn(&layout, &fonts, &colours, &comm);
        let no = extent(&all[all.len() - 2 * 6 - 2 * 3 * 6..all.len() - 2 * 6]);
        assert!(no.min.x >= 262.0 && no.max.x <= 262.0 + 95.0 + 1.0 && no.min.y == 74.0, "{no:?}");
        // Nothing of them is blue until one is chosen: answered with R, the second
        // answer's button has its blue sign.
        let blue = |vertices: &[Vertex]| vertices.iter().any(|vertex| vertex.colour[..3] == [0, 0x80, 0xfa]);
        assert!(!blue(&all));
        let mut comm = comm;
        comm.step(&Pad { pressed: button::R, held: button::R, ..Pad::default() }, false);
        assert!(blue(&drawn(&layout, &fonts, &colours, &after(comm, 1))));
    }
}
