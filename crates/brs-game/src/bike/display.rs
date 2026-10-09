//! The display of the ride on the bike (0x132CA8 to 0x135C2C; "The display" in
//! `docs/BIKE_NOTES.md`): the signs at the top, the bar of how far she has come,
//! the time left, her life, and the dial with her speed and the row of those
//! she has just avoided.
//!
//! [`Display`] is what the rules and the scripts set of it and what comes of
//! that each frame; [`Gauges`] is the layout it is drawn from, which plays what
//! the display asks for.

use anyhow::{ensure, Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_engine::texture;
use brs_formats::{container, ptmd};

use super::rules::Shown;
use super::tables::SYSTEM_PACK;
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::text::TextColours;
use crate::writing::{Align, Block, Fonts, Placing, Style};

/// The member of the system's pack that is the gauges' pack (0x132EEC), and what
/// its own members are: nine textures, then the layout's three files.
const PACK_MEMBER: usize = 3;
const OBJECTS: usize = 9;
const MOTIONS: usize = 10;
const GROUPS: usize = 11;
/// The layout's eleven textures by the pack's members (0x133DB8); the eighth is
/// the common game's figures (0x2CD24) and the tenth is none.
const TEXTURES: [Option<usize>; 11] = [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), Some(6), None, Some(7), Some(0), Some(8)];
const FIGURES: usize = 7;
const FIGURES_FILE: usize = 9;
/// The textures the dial's needle, her life and the row's time are cut from.
const NEEDLE_TEXTURE: usize = 1;
const LIFE_TEXTURE: usize = 5;
const ROW_TEXTURE: usize = 8;

/// The display's flags (what 0x1335D8's 0 to 7 set, and its own fade's two).
mod flag {
    pub const AVOID: u32 = 1;
    pub const SIGNS: [u32; 2] = [2, 4];
    pub const PARTS: [u32; 4] = [8, 0x10, 0x20, 0x40];
    pub const FADE: u32 = 0x80;
    pub const FADING: u32 = 0x100;
}

/// How many places the gauges' layout is played in.
pub const PLACES: usize = 7;
/// What the gauges play (0x133914): a place and a set, by what is asked of them.
const PLAYS: [(usize, usize); 20] = [
    (0, 8),
    (0, 9),
    (1, 0xa),
    (2, 0xc),
    (1, 0xb),
    (2, 0xd),
    (3, 0xe),
    (3, 0xf),
    (4, 0x10),
    (4, 0x11),
    (5, 0x12),
    (5, 0x13),
    (6, 0x14),
    (6, 0x15),
    (5, 0x16),
    (6, 0x17),
    (6, 0x18),
    (6, 0x19),
    (6, 0x1a),
    (6, 0x1b),
];
/// What is asked of the gauges besides what a flag's coming and going asks.
mod play {
    pub const SIGN_COMES: [usize; 2] = [2, 3];
    pub const SIGN_GOES: [usize; 2] = [4, 5];
    /// The parts but the dial: each comes and goes.
    pub const PART_COMES: [usize; 3] = [6, 8, 0xa];
    pub const DIAL_COMES: usize = 0xc;
    pub const DIAL_GOES: usize = 0xd;
    pub const BLOW: usize = 0xe;
    pub const ROW_LONGER: usize = 0xf;
    pub const ROW_FULL: usize = 0x10;
    pub const SPECIAL: usize = 0x11;
    pub const FULL_DIAL_COMES: usize = 0x12;
    pub const FULL_DIAL_GOES: usize = 0x13;
}
/// The row that fills the gauge.
const FULL_ROW: i32 = 5;
/// How many frames the two signs take to move apart when both are up, and how
/// far each goes (0x134140).
const APART_FRAMES: i32 = 10;
const APART: [f32; 2] = [-24.0, 25.0];

/// The layout's groups that are not drawn as they are (0x13446C).
mod group {
    /// The mark on the bar, the figures beside it, their unit and their point.
    pub const MARK: usize = 1;
    pub const COME: usize = 2;
    pub const COME_UNIT: usize = 3;
    pub const COME_POINT: usize = 33;
    pub const TIME: usize = 4;
    pub const NEEDLE: usize = 8;
    pub const SPEED: usize = 9;
    pub const SPEED_UNIT: usize = 10;
    pub const ROW_TIME: usize = 14;
    /// The lamps of the row: each is lit for one length of it.
    pub const LAMPS: [(usize, i32); 6] = [(15, 0), (34, 1), (35, 2), (16, 3), (17, 4), (12, 5)];
    pub const LIT: [(usize, i32); 5] = [(36, 1), (37, 2), (38, 3), (18, 4), (19, 5)];
    pub const LIFE_GAUGE: usize = 21;
    pub const LIFE: usize = 23;
    pub const LIFE_STROKE: usize = 24;
    pub const SIGN_PARTS: [usize; 3] = [28, 29, 30];
}

/// The layout's objects the code goes by: the bar (how high it is), the stroke
/// between her life and the most of it (how wide), and where the three gauges
/// are cut from.
mod object {
    pub const BAR: usize = 1;
    pub const NEEDLE: usize = 17;
    pub const ROW_TIME: usize = 24;
    pub const LIFE_GAUGE: usize = 32;
    pub const LIFE_STROKE: usize = 37;
    /// Where each of the eight texts is written (0x135190).
    pub const TEXTS: [usize; 8] = [7, 47, 36, 36, 18, 9, 9, 9];
}

/// The eight texts (0x133DB8): how far she has come in kilometres and metres,
/// her life and the most of it, her speed, and the time left.
mod text {
    pub const KILOMETRES: usize = 0;
    pub const METRES: usize = 1;
    pub const LIFE: usize = 2;
    pub const MOST: usize = 3;
    pub const SPEED: usize = 4;
    pub const TIME: [usize; 3] = [7, 6, 5];
}

/// What the gauges show: the numbers 0x133C1C keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Numbers {
    /// How far she has come, in centimetres, and as a share of the way.
    pub come: i32,
    pub share: f32,
    pub life: i32,
    pub most: i32,
    pub speed: i32,
    /// Where the dial's needle is, 0 to 1.
    pub needle: f32,
    /// How many she has just avoided in a row, and how much of the row's time is
    /// left.
    pub row: i32,
    pub row_time: f32,
    /// The time left: minutes, seconds, hundredths.
    pub time: [i32; 3],
}

/// The display as the rules and the scripts have it.
#[derive(Clone, Debug, Default)]
pub struct Display {
    flags: u32,
    before: u32,
    /// Her life and the row as they were last given.
    life: i32,
    row: i32,
    speed: i32,
    time_left: i32,
    pub numbers: Numbers,
    /// Which of the two signs the gauges have up, and how far apart they have
    /// moved, in frames.
    signs: [bool; 2],
    apart: i32,
    /// The display's own fade to black: how long it takes and how far in it is,
    /// in frames.
    fade_length: i32,
    fade_at: i32,
    /// What the gauges are to play, in the order it was asked.
    pub asked: Vec<usize>,
    /// What it is to say (`n28`).
    pub says: Option<String>,
}

impl Display {
    fn set(&mut self, flag: u32, on: bool) {
        self.flags = if on { self.flags | flag } else { self.flags & !flag };
    }

    fn play(&mut self, what: usize) {
        match what {
            2 | 3 => self.signs[what - 2] = true,
            4 | 5 => self.signs[what - 4] = false,
            _ => {}
        }
        self.asked.push(what);
    }

    /// What the rules give it each frame (0x1335D8's 0 to 15).
    pub fn given(&mut self, shown: &Shown) {
        self.set(flag::AVOID, shown.avoid_sign);
        for (sign, on) in flag::SIGNS.into_iter().zip(shown.signs) {
            self.set(sign, on);
        }
        for (part, on) in flag::PARTS.into_iter().zip(shown.parts) {
            self.set(part, on);
        }
        self.numbers.come = shown.come;
        self.numbers.share = shown.part_of_the_way.clamp(0, 100) as f32 / 100.0;
        let life = shown.left.max(0);
        if life < self.life {
            self.play(play::BLOW);
        }
        (self.life, self.numbers.life, self.numbers.most) = (life, life, shown.could);
        self.speed = shown.speed;
        let row = shown.row.clamp(0, FULL_ROW);
        if row > self.row {
            self.play(if row >= FULL_ROW { play::ROW_FULL } else { play::ROW_LONGER });
        }
        (self.row, self.numbers.row) = (row, row);
        self.numbers.row_time = shown.row_time.clamp(0, 100) as f32 / 100.0;
        self.time_left = shown.time_left;
    }

    /// Her special has begun (0x132EEC with 1).
    pub fn special(&mut self) {
        self.play(play::SPECIAL);
    }

    /// Begins the display's own fade to black, over so many frames (0x1335D8's
    /// 0x10, then 0x132EEC with 2).
    pub fn fade_out(&mut self, frames: i32) {
        (self.fade_length, self.fade_at) = (frames.max(0), 0);
        self.flags |= flag::FADE | flag::FADING;
    }

    /// Whether its fade is still going (0x132F74).
    pub fn fading(&self) -> bool {
        self.flags & flag::FADE != 0 && self.flags & flag::FADING != 0
    }

    /// How dark its fade has the screen, 0 to 255 (0x132D60).
    pub fn dark(&self) -> u8 {
        if self.flags & flag::FADE == 0 {
            0
        } else if self.fade_length > 0 {
            (self.fade_at as f32 / self.fade_length as f32 * 255.0) as u8
        } else {
            255
        }
    }

    /// A frame (0x132F9C, and 0x134140 of the gauges): what a flag's coming or
    /// going asks of the gauges, the time and the needle. `speeds` are the
    /// course's five, `frames` the interface's that have gone by. Says whether
    /// the signs' sound is to be played.
    pub fn frame(&mut self, speeds: [f32; 5], frames: i32) -> bool {
        let (now, before) = (self.flags, self.before);
        let came = |flag: u32| now & flag != 0 && before & flag == 0;
        let went = |flag: u32| now & flag == 0 && before & flag != 0;
        if came(flag::AVOID) {
            self.play(0);
        }
        if went(flag::AVOID) {
            self.play(1);
        }
        for (index, sign) in flag::SIGNS.into_iter().enumerate() {
            if came(sign) {
                self.play(play::SIGN_COMES[index]);
            }
            if went(sign) {
                self.play(play::SIGN_GOES[index]);
            }
        }
        let both = flag::SIGNS[0] | flag::SIGNS[1];
        let warned = now & both != 0 && before & both == 0;
        for (index, part) in flag::PARTS.into_iter().take(3).enumerate() {
            if came(part) {
                self.play(play::PART_COMES[index]);
            }
            if went(part) {
                self.play(play::PART_COMES[index] + 1);
            }
        }
        // The dial comes and goes another way once the row is full.
        let full = self.row >= FULL_ROW;
        if came(flag::PARTS[3]) {
            self.play(if full { play::FULL_DIAL_COMES } else { play::DIAL_COMES });
        }
        if went(flag::PARTS[3]) {
            self.play(if full { play::FULL_DIAL_GOES } else { play::DIAL_GOES });
        }

        let seconds = self.time_left / 100;
        self.numbers.time = [seconds / 60 % 100, seconds % 60, self.time_left % 100];
        self.numbers.speed = self.speed;
        self.numbers.needle = needle(self.speed as f32, speeds);

        // The two signs move apart while both are up.
        self.apart = if self.signs[0] && self.signs[1] { (self.apart + frames).min(APART_FRAMES) } else { (self.apart - frames).max(0) };

        if self.fading() {
            self.fade_at += frames;
            if self.fade_at > self.fade_length {
                self.fade_at = self.fade_length;
                self.flags &= !flag::FADING;
            }
        }
        self.before = self.flags;
        warned
    }

    /// How far across each of the two signs is moved.
    fn apart(&self) -> [f32; 2] {
        APART.map(|far| (self.apart as f32 / APART_FRAMES as f32 * far).trunc())
    }
}

/// Where the dial's needle is for a speed (0x132F9C): each of the course's five
/// speeds has its place round the dial, and the needle goes evenly between them.
fn needle(speed: f32, speeds: [f32; 5]) -> f32 {
    const PLACES: [f32; 5] = [0.3, 0.8, 0.87, 0.94, 1.0];
    let between = |from: usize| (speed - speeds[from]) / (speeds[from + 1] - speeds[from]) * (PLACES[from + 1] - PLACES[from]) + PLACES[from];
    let at = if speed > speeds[4] {
        1.0
    } else if speed > speeds[3] {
        between(3)
    } else if speed > speeds[2] {
        between(2)
    } else if speed > speeds[1] {
        between(1)
    } else if speed > speeds[0] {
        between(0)
    } else {
        speed / speeds[0] * PLACES[0]
    };
    // It is kept in hundredths.
    (at * 100.0).trunc() / 100.0
}

/// A part of a round picture, from its middle out (0x32E40, 0x135760): the
/// picture's place and size in its texture, the angle it begins at and how far
/// round all of it goes, in degrees, how far out it is drawn and in how many
/// pieces.
#[derive(Clone, Copy, Debug)]
struct Pie {
    texture: usize,
    from: Vec2,
    size: Vec2,
    begins: f32,
    round: f32,
    radius: f32,
    pieces: f32,
}

/// The dial's needle: the arc that fills as she goes faster.
const NEEDLE: Pie = Pie { texture: NEEDLE_TEXTURE, from: Vec2::new(3.0, 40.0), size: Vec2::new(58.0, 58.0), begins: -67.5, round: -225.0, radius: 80.0, pieces: 8.0 };
const NEEDLE_COLOUR: [u8; 3] = [0x72, 0xf7, 0xff];
/// The ring of the row's time.
const ROW_TIME: Pie = Pie { texture: ROW_TEXTURE, from: Vec2::new(1.0, 1.0), size: Vec2::new(38.0, 38.0), begins: 0.0, round: -360.0, radius: 40.0, pieces: 8.0 };
const ROW_TIME_COLOUR: [u8; 3] = [0x00, 0xf0, 0xff];
/// Her life: its colour goes from the first to the second round it.
const LIFE: Pie = Pie { texture: LIFE_TEXTURE, from: Vec2::new(1.0, -12.0), size: Vec2::new(92.0, 92.0), begins: -45.0, round: -90.0, radius: 60.0, pieces: 8.0 };
const LIFE_COLOURS: [[u8; 3]; 2] = [[0x57, 0xff, 0xf9], [0x2f, 0x74, 0xff]];
/// How far right of its object's corner the middle of her life's gauge is.
const LIFE_MIDDLE: f32 = 46.0;

/// The gauges: the layout, what plays it in its seven places, and the textures
/// the three round gauges are cut from, with their sizes.
#[derive(Resource)]
pub struct Gauges {
    layout: Layout,
    places: [Player; PLACES],
    sizes: Vec<Vec2>,
}

impl Gauges {
    /// Reads the gauges' pack (0x133DB8).
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<Gauges> {
        let pack = data.member(SYSTEM_PACK, PACK_MEMBER)?.with_context(|| format!("{SYSTEM_PACK} has no member {PACK_MEMBER}"))?;
        let members = container::parse_lpk(&pack).context("the gauges' pack")?;
        ensure!(members.len() > GROUPS, "the gauges' pack has {} members", members.len());
        let (figures, figures_size) = interface::load_texture(data, images, &interface::file(interface::RESIDENT_PACK, FIGURES_FILE, "ptm"))?;
        let mut loaded: Vec<Option<(Handle<Image>, Vec2)>> = vec![None; OBJECTS];
        let (mut textures, mut sizes) = (Vec::new(), Vec::new());
        for (index, member) in TEXTURES.into_iter().enumerate() {
            let (handle, size) = match member {
                None if index == FIGURES => (figures.clone(), figures_size),
                None => unreachable!("only the figures are of another pack"),
                Some(member) => match &loaded[member] {
                    Some(had) => had.clone(),
                    None => {
                        let header = ptmd::parse_header(members[member]).with_context(|| format!("member {member} of the gauges' pack is not a texture"))?;
                        let made = (images.add(texture::image(members[member], &header)), texture::size(&header));
                        loaded[member] = Some(made.clone());
                        made
                    }
                },
            };
            textures.push(handle);
            sizes.push(size);
        }
        let layout = Layout::parse(members[OBJECTS], members[MOTIONS], members[GROUPS], textures).context("the gauges' layout")?;
        // A place plays nothing until it is asked to.
        let none = layout.motions.sets.len();
        let places = std::array::from_fn(|_| layout.player(none));
        Ok(Gauges { layout, places, sizes })
    }

    /// Plays what the display has asked for and goes on by so many frames
    /// (0x133914, and the end of 0x134140).
    pub fn advance(&mut self, display: &mut Display, frames: f32) {
        for what in display.asked.drain(..) {
            if let Some(&(place, set)) = PLAYS.get(what) {
                self.places[place].play(&self.layout, set);
            }
        }
        for place in &mut self.places {
            place.advance(&self.layout, frames);
        }
    }

    /// The set each place was last given.
    #[cfg(test)]
    fn sets(&self) -> [usize; PLACES] {
        std::array::from_fn(|place| self.places[place].set())
    }

    fn texts(&self, numbers: &Numbers, fonts: &Fonts, colours: &TextColours) -> [Block; 8] {
        // Capitals, ten to a line and one apart: the small ones and those twice
        // their size (0x2D788 with 1 and with 2).
        const SMALL: Style = Style::of_kind(1, 10, 1, 1.0, 0.0);
        const BIG: Style = Style::of_kind(2, 10, 1, 1.0, 0.0);
        let write = |style: &Style, words: String| fonts.write_plain(style, &words, colours);
        [
            write(&SMALL, format!("{}", numbers.come / 100_000)),
            write(&SMALL, format!("{:03}", numbers.come % 100_000 / 100)),
            write(&SMALL, format!("{}", numbers.life)),
            write(&SMALL, format!("{}", numbers.most)),
            write(&BIG, format!("{}", numbers.speed)),
            write(&BIG, format!("{}", numbers.time[0])),
            write(&BIG, format!("{:02}", numbers.time[1])),
            write(&BIG, format!("{:02}", numbers.time[2])),
        ]
    }

    /// Draws the gauges (0x13446C): every track of every place, those of some
    /// groups in ways of their own.
    pub fn draw(&self, canvas: &mut Canvas, display: &Display, fonts: &Fonts, colours: &TextColours) {
        let numbers = &display.numbers;
        let texts = self.texts(numbers, fonts, colours);
        let size = |object: usize| self.layout.objects.objects.get(object).map_or(Vec2::ZERO, |object| Vec2::new(f32::from(object.size[0]), f32::from(object.size[1])));
        let up_the_bar = Vec2::new(0.0, -(size(object::BAR).y * numbers.share));
        let most_wide = texts[text::MOST].size().x;
        let apart = display.apart();
        for (place, player) in self.places.iter().enumerate() {
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let number = player.number_in_group(track).unwrap_or(0);
                let mut moved = |by: Vec2| self.layout.draw_track(canvas, layer::DISPLAY, player, track, &Look::inner(Affine2::from_translation(by)));
                let lit = |lamps: &[(usize, i32)]| lamps.iter().find(|(lamp, _)| *lamp == group).map(|(_, row)| *row == numbers.row);
                match group {
                    group::TIME => {
                        if let Some(&text) = text::TIME.get(number) {
                            self.write(canvas, fonts, player, track, text, &texts[text], Vec2::ZERO);
                        }
                    }
                    // The two signs' own parts, moved apart; no other place has them.
                    _ if group::SIGN_PARTS.contains(&group) => match place {
                        1 | 2 => moved(Vec2::new(apart[place - 1], 0.0)),
                        _ => {}
                    },
                    group::MARK | group::COME_UNIT | group::COME_POINT => moved(up_the_bar),
                    group::COME => {
                        let by = up_the_bar.trunc();
                        self.write(canvas, fonts, player, track, text::KILOMETRES, &texts[text::KILOMETRES], by);
                        self.write(canvas, fonts, player, track, text::METRES, &texts[text::METRES], by);
                    }
                    group::LIFE => {
                        self.write(canvas, fonts, player, track, text::MOST, &texts[text::MOST], Vec2::ZERO);
                        let left = (-most_wide - size(object::LIFE_STROKE).x).trunc();
                        self.write(canvas, fonts, player, track, text::LIFE, &texts[text::LIFE], Vec2::new(left, 0.0));
                    }
                    group::LIFE_STROKE => moved(Vec2::new(-most_wide, 0.0)),
                    group::LIFE_GAUGE => {
                        let share = if numbers.most > 0 { (numbers.life as f32 / numbers.most as f32).clamp(0.0, 1.0) } else { 0.0 };
                        let (at, area) = (self.layout.place(object::LIFE_GAUGE).unwrap_or_default(), size(object::LIFE_GAUGE));
                        let cut = Rect::from_corners(at, at + area);
                        self.pie(canvas, player, track, &LIFE, at + Vec2::new(LIFE_MIDDLE, area.y * 0.5), cut, share, LIFE_COLOURS);
                    }
                    group::NEEDLE => {
                        let (at, area) = (self.layout.place(object::NEEDLE).unwrap_or_default(), size(object::NEEDLE));
                        let cut = Rect::from_corners(at, at + area);
                        self.pie(canvas, player, track, &NEEDLE, at + Vec2::splat(area.x * 0.5), cut, numbers.needle.clamp(0.0, 1.0), [NEEDLE_COLOUR; 2]);
                    }
                    group::SPEED => self.write(canvas, fonts, player, track, text::SPEED, &texts[text::SPEED], Vec2::ZERO),
                    group::SPEED_UNIT => moved(Vec2::new((texts[text::SPEED].size().x * 0.5 - 5.0).trunc(), 0.0)),
                    group::ROW_TIME => {
                        let (at, area) = (self.layout.place(object::ROW_TIME).unwrap_or_default(), size(object::ROW_TIME));
                        let corner = (at - area * 0.5).trunc();
                        let cut = Rect::from_corners(corner, corner + area);
                        self.pie(canvas, player, track, &ROW_TIME, at, cut, numbers.row_time.clamp(0.0, 1.0), [ROW_TIME_COLOUR; 2]);
                    }
                    _ => match (lit(&group::LAMPS), lit(&group::LIT)) {
                        // A lamp has two tracks, and the second is the one that is
                        // lit for its length of the row.
                        (Some(lit), _) => {
                            if lit && number == 1 {
                                moved(Vec2::ZERO);
                            }
                        }
                        (_, Some(lit)) => {
                            if lit {
                                moved(Vec2::ZERO);
                            }
                        }
                        _ => moved(Vec2::ZERO),
                    },
                }
            }
        }
    }

    /// Writes one of the texts where a track has its object (0x135190): moved as
    /// the track is and so much more, in the object's colour by the track's.
    #[allow(clippy::too_many_arguments)]
    fn write(&self, canvas: &mut Canvas, fonts: &Fonts, player: &Player, track: usize, text: usize, words: &Block, by: Vec2) {
        let Some((moved, colour)) = self.layout.placed(player, track) else { return };
        let object = object::TEXTS[text];
        let (Some(place), Some(of)) = (self.layout.place(object), self.layout.objects.objects.get(object)) else { return };
        // Her life and the time are set to the right of their objects, her speed
        // in the middle of its own; and her speed alone has no shadow.
        let align = match text {
            text::KILOMETRES | text::METRES => Align::Left,
            text::SPEED => Align::Centre,
            _ => Align::Right,
        };
        let placing = Placing {
            tint: modulate(self.layout.colour(object), colour),
            width: f32::from(of.size[0]),
            align,
            shadow: text != text::SPEED,
            ..Placing::at(place + (moved.translation + by).trunc())
        };
        fonts.draw(canvas, layer::DISPLAY, words, &placing);
    }

    /// Draws so much of a round picture where a track has it (0x32E40,
    /// 0x135760): a fan from the middle, as far round as the share says, cut to
    /// a part of the screen; its colour going from the first to the second by
    /// how far round a corner is, with the track's alpha.
    #[allow(clippy::too_many_arguments)]
    fn pie(&self, canvas: &mut Canvas, player: &Player, track: usize, pie: &Pie, middle: Vec2, cut: Rect, share: f32, colours: [[u8; 3]; 2]) {
        let Some((moved, colour)) = self.layout.placed(player, track) else { return };
        let (Some(texture), Some(&size)) = (self.layout.textures.get(pie.texture), self.sizes.get(pie.texture)) else { return };
        let placed = Affine2::from_mat2_translation(moved.matrix2, moved.translation + middle);
        let middle_of_picture = (pie.from + pie.size * 0.5) / size;
        let coloured = |part: f32| -> [u8; 4] {
            let mixed: [u8; 3] = std::array::from_fn(|index| ((f32::from(colours[1][index]) - f32::from(colours[0][index])) * part + f32::from(colours[0][index])) as u8);
            [mixed[0], mixed[1], mixed[2], colour[3]]
        };
        // The corners round it: one for each whole piece the share has, the last
        // where the share ends.
        let corners = (share * pie.pieces) as usize + 3;
        let corner = |index: usize| {
            let turned = if index == corners - 1 { pie.round * share } else { (index - 1) as f32 * pie.round / pie.pieces };
            let (sine, cosine) = (turned + pie.begins).to_radians().sin_cos();
            let out = Vec2::new(sine, cosine) * pie.radius;
            Vertex::new(placed.transform_point2(out), middle_of_picture + out / size, coloured((turned / pie.round).abs()))
        };
        let hub = Vertex::new(placed.transform_point2(Vec2::ZERO), middle_of_picture, coloured(0.5));
        let fan: Vec<Vertex> = (1..corners - 1).flat_map(|index| [hub, corner(index), corner(index + 1)]).collect();
        canvas.cut_to(Some(cut));
        canvas.triangles(layer::DISPLAY, Paint::textured(texture), &fan);
        canvas.cut_to(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bike::tables::tests::data;

    const SPEEDS: [f32; 5] = [80.0, 140.0, 200.0, 290.0, 360.0];

    fn shown() -> Shown {
        Shown { parts: [true; 4], left: 1000, could: 1000, speed: 140, come: 37_800, part_of_the_way: 5, time_left: 12_345, ..Shown::default() }
    }

    #[test]
    fn the_needle_has_a_place_for_each_of_the_five_speeds() {
        assert_eq!(needle(0.0, SPEEDS), 0.0);
        assert_eq!(needle(40.0, SPEEDS), 0.15);
        assert_eq!(needle(80.0, SPEEDS), 0.3);
        assert_eq!(needle(140.0, SPEEDS), 0.8);
        assert_eq!(needle(200.0, SPEEDS), 0.87);
        assert_eq!(needle(290.0, SPEEDS), 0.94);
        assert_eq!(needle(360.0, SPEEDS), 1.0);
        assert_eq!(needle(400.0, SPEEDS), 1.0);
        // Evenly between two of them.
        assert_eq!(needle(110.0, SPEEDS), 0.55);
    }

    #[test]
    fn a_flag_that_comes_or_goes_asks_the_gauges_to_play() {
        let mut display = Display::default();
        display.given(&shown());
        assert!(!display.frame(SPEEDS, 2));
        // The four parts come: the bar, the time, her life, the dial.
        assert_eq!(display.asked, [6, 8, 0xa, 0xc]);
        assert_eq!(display.numbers.time, [2, 3, 45]);
        assert_eq!((display.numbers.speed, display.numbers.needle, display.numbers.share), (140, 0.8, 0.05));
        display.asked.clear();

        // Nothing changes, nothing is asked.
        display.given(&shown());
        display.frame(SPEEDS, 2);
        assert!(display.asked.is_empty());

        // A blow, a longer row, and a sign that comes with its sound.
        display.given(&Shown { left: 600, row: 1, signs: [true, false], avoid_sign: true, ..shown() });
        assert!(display.frame(SPEEDS, 2));
        assert_eq!(display.asked, [play::BLOW, play::ROW_LONGER, 0, 2]);
        display.asked.clear();
        // The other sign comes while the first is up: no sound again, and the
        // two move apart.
        display.given(&Shown { left: 600, row: 1, signs: [true, true], ..shown() });
        assert!(!display.frame(SPEEDS, 2));
        assert_eq!(display.asked, [1, 3]);
        assert_eq!(display.apart(), [-4.0, 5.0]);
        for _ in 0..6 {
            display.frame(SPEEDS, 2);
        }
        assert_eq!(display.apart(), APART);
        display.asked.clear();

        // The row comes to five; and the dial goes another way then.
        display.given(&Shown { left: 600, row: 5, parts: [true, true, true, false], ..shown() });
        display.frame(SPEEDS, 2);
        assert_eq!(display.asked, [play::ROW_FULL, 4, 5, play::FULL_DIAL_GOES]);
    }

    #[test]
    fn the_displays_own_fade_goes_to_black_and_stays() {
        let mut display = Display::default();
        assert_eq!((display.dark(), display.fading()), (0, false));
        display.fade_out(30);
        assert!(display.fading());
        for _ in 0..5 {
            display.frame(SPEEDS, 2);
        }
        assert_eq!((display.dark(), display.fading()), (85, true));
        for _ in 0..11 {
            display.frame(SPEEDS, 2);
        }
        assert_eq!((display.dark(), display.fading()), (255, false));
    }

    #[test]
    fn the_gauges_pack_is_read_and_plays_what_is_asked() {
        let Some(data) = data() else { return };
        let mut images = Assets::<Image>::default();
        let mut gauges = Gauges::load(&data, &mut images).unwrap();
        assert_eq!((gauges.layout.objects.objects.len(), gauges.layout.groups.groups.len(), gauges.layout.motions.sets.len()), (55, 39, 28));
        // The bar is 103 high, and the three gauges are about one middle.
        let place = |object: usize| gauges.layout.place(object).unwrap();
        let size = |object: usize| gauges.layout.objects.objects[object].size;
        assert_eq!(size(object::BAR)[1], 103);
        assert_eq!(place(object::LIFE_GAUGE) + Vec2::new(LIFE_MIDDLE, f32::from(size(object::LIFE_GAUGE)[1]) * 0.5), Vec2::new(440.0, 232.0));
        assert_eq!(place(object::NEEDLE) + Vec2::splat(f32::from(size(object::NEEDLE)[0]) * 0.5), Vec2::new(440.0, 232.0));
        assert_eq!(place(object::ROW_TIME), Vec2::new(440.0, 232.0));

        let mut display = Display::default();
        display.given(&shown());
        display.frame(SPEEDS, 2);
        gauges.advance(&mut display, 2.0);
        assert!(display.asked.is_empty());
        assert_eq!(gauges.sets()[3..], [0xe, 0x10, 0x12, 0x14]);
        // The time's three figures are three tracks of one group.
        let time = &gauges.places[4];
        let figures: Vec<usize> = (0..time.len()).filter(|&track| time.group(track) == Some(group::TIME)).map(|track| time.number_in_group(track).unwrap()).collect();
        assert_eq!(figures.len(), 3);
        assert!(figures.contains(&0) && figures.contains(&1) && figures.contains(&2));
    }
}
