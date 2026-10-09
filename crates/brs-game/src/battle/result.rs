//! What a battle won was worth, as the original shows it (`ResultBtlTask`, 0x3814C;
//! its picture is an object of its own, 0x361D8 to 0x37958).
//!
//! Once her last state is over and the log has said its last, the battle's end
//! (0x9D988) makes the results with what the battle keeps of its worth (0x9D8B0):
//! the experience and up to four kinds of item. The panels come over eight frames:
//! the title, the experience earned, how much is wanting to the next level with a
//! bar of how far she is into this one, and four rows for the items. Then the
//! experience is counted onto hers, a thirtieth of the level's span each frame with
//! a tick each time; reaching a level stops the count while a band says so ("LEVEL
//! UP: LV1 >> 2", with a sound), and the count goes on into the next level. Cross
//! has it all counted at once. Only then is she given the experience and the items
//! (0x37E24, 0x37F44), and the results stand until cross or circle is pressed
//! (0x9D6A8).
//!
//! The layout is `if_btlsys.lpk/006.lpk` with five textures: two of its own, and
//! of the pack that is always there the figures' sheet (file 9), file 6 and the
//! icons (file 20). Its sets: 0 the panels, 1 a row (played once and drawn four
//! times, twenty apart), 2 the band of a level reached, 3 all of it going, which
//! this way out does not use.
//!
//! The tune a battle against a group that has her pose ends with (0x38288) is
//! asked for by whoever makes the results (`conclude`), and the results give way
//! to the field as the battle does (0x9D758). Not ported: the challenges a battle
//! can open, each shown in a window of its own before the results can be left
//! (0x37F8C).

use anyhow::Result;
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{modulate, Layout, Look, Player};

use super::tables::Levels;
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::items::Items;
use crate::text::{self, TextColours};
use crate::writing::{Block, Fonts, Placing, Style};

const PACK: &str = "SYSTEM.VOL/if_btlsys.lpk/006.lpk";
/// The files of the pack that is always there that the layout draws with besides
/// its own two (0x36298 on): the figures, a sheet of its own kind, and the icons.
const SHARED_TEXTURES: [usize; 3] = [9, 6, 20];

const SET_PANELS: usize = 0;
const SET_ROW: usize = 1;
const SET_LEVEL: usize = 2;

/// The rows of items, and how far apart they are drawn (0x37318).
pub const ROWS: usize = 4;
const ROW_PITCH: f32 = 20.0;

// The groups of the panels' set that are more than something to draw (0x36764): the
// bar, cut to how far she is into her level; how much is wanting to the next, in
// figures, each on a back of its own; and the experience earned, likewise.
const BAR: usize = 26;
const WANTING_BACK: usize = 27;
const WANTING: usize = 28;
const EARNED_BACK: usize = 30;
const EARNED: usize = 31;
// Those of a row's set (0x36C18): how many of the item, in two figures at the
// least, each on its back; its name; its icon; and what a row without an item
// has in their places.
const COUNT_BACK: usize = 13;
const COUNT: usize = 14;
const NAME: usize = 15;
const ICON: usize = 16;
const NO_NAME: usize = 17;
const NO_COUNT: usize = 18;
const NO_ICON: usize = 19;
// Those of the band's set (0x37414): the level she had and the one she has, in
// figures, and the two things before them that make room for more figures.
const LEVEL_BEFORE: usize = 3;
const LEVEL_AFTER: usize = 4;
const LEVEL_WORD: usize = 5;
const LEVEL_ARROW: usize = 6;
/// How much room a figure of the level she had takes, and one of the level she
/// has (0x374DC, 0x37468).
const BEFORE_ROOM: f32 = 11.0;
const AFTER_ROOM: f32 = 17.0;
/// What a row without an item has for its name is drawn four times, and for its
/// count twice, this far apart (0x37208, 0x37288).
const DASH_PITCH: f32 = 11.0;

/// The cells of the figures' sheet (the first of the table at 0x171078).
const FIGURE_CELL: f32 = 20.0;
/// An icon of the icons' sheet: fifteen square, in cells of sixteen, four to a row,
/// a pixel in from each cell's corner (0x2D198).
const ICON_SIZE: f32 = 15.0;
const ICON_CELL: f32 = 16.0;
const ICONS_ACROSS: i32 = 4;

/// How a row's name is written (0x36390): twenty-four glyphs to a line.
const NAME_STYLE: Style = Style::of_kind(0, 0x18, 2, 1.0, 4.0);

/// The most experience there is, and the last level (0x38640, 0x383C4).
const MOST_EXPERIENCE: i32 = 999_999;
const LAST_LEVEL: i32 = 50;
/// A level's span is counted in so many frames (0x3795C), one of experience a frame
/// at the least.
const COUNT_FRAMES: f32 = 30.0;

/// The sounds of a level reached and of the count's tick (0x38434, 0x384DC), of the
/// bank that is always there.
pub const SOUND_LEVEL: u32 = 0x66;
pub const SOUND_COUNT: u32 = 0x0b;

/// What the results are drawn from.
pub struct ResultLayout {
    layout: Layout,
    /// The width of the figures' sheet.
    figures: f32,
    icons: Handle<Image>,
    icons_size: Vec2,
}

impl ResultLayout {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<ResultLayout> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = SHARED_TEXTURES.map(|file| interface::file(interface::RESIDENT_PACK, file, "ptm"));
        let textures = [own(3), own(4), shared[0].clone(), shared[1].clone(), shared[2].clone()];
        let (_, figures) = interface::load_texture(data, images, &shared[0])?;
        let (icons, icons_size) = interface::load_texture(data, images, &shared[2])?;
        Ok(ResultLayout { layout: interface::layout_with(data, images, PACK, 0, &textures)?, figures: figures.x, icons, icons_size })
    }
}

/// How many figures a number is written with (0x22ACC).
fn figures(number: i32) -> i32 {
    let (mut left, mut count) = (number.unsigned_abs(), 1);
    while left >= 10 {
        left /= 10;
        count += 1;
    }
    count
}

/// The figure in a place of a number of `count` figures, the first place the
/// highest (0x22AFC).
fn figure(number: i32, count: i32, place: i32) -> i32 {
    number.max(0) / 10_i32.pow((count - 1 - place) as u32) % 10
}

/// The icon of the icons' sheet an item of a kind is drawn with (0x2D1F4).
fn icon_of(kind: u8) -> i32 {
    match kind {
        1 | 3 => 4,
        2 => 5,
        4..=6 => 9,
        7..=9 => 6,
        10 => 7,
        11 => 8,
        _ => 0,
    }
}

/// A row of the results: an item left behind and how many of it.
#[derive(Clone, Debug, Default)]
struct Row {
    item: i32,
    count: i32,
    kind: u8,
    name: Option<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The panels are coming.
    Coming,
    /// The experience is being counted onto hers.
    Counting,
    /// All is told: she has been given it, and a button closes the results.
    Told,
}

/// The results of the battle that has been won.
#[derive(Resource)]
pub struct Results {
    phase: Phase,
    panels: Player,
    row: Player,
    /// The band of a level reached, once there has been one.
    band: Player,
    band_shown: bool,
    rows: [Row; ROWS],
    earned: i32,
    /// Her experience as the battle ended, and as the count has it now.
    had: i32,
    shown: f32,
    /// How much of what was earned has been counted, and how much is a frame.
    counted: f32,
    step: f32,
    /// The level the count has come to, and the experience it began and ends at.
    level: i32,
    base: i32,
    next: i32,
    /// The two levels the band names.
    named: (i32, i32),
    /// The count has just reached a level: the next frame goes on from there.
    reached: bool,
}

/// What a frame of the results did that is heard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    pub level: bool,
    pub count: bool,
}

impl Results {
    /// The results of a battle that was worth `earned` experience and `items` (each
    /// an item and how many), to one who was of `level` with `had` experience
    /// (0x37A9C, 0x37C24).
    pub fn new(what: &ResultLayout, levels: &Levels, items: &Items, fonts: &Fonts, colours: &TextColours, level: i32, had: i32, earned: i32, left: &[(i32, i32)]) -> Results {
        let layout = &what.layout;
        let mut rows: [Row; ROWS] = default();
        for (row, &(item, count)) in rows.iter_mut().zip(left) {
            let known = items.get(item);
            let name = (item > 0).then(|| fonts.write(&NAME_STYLE, &text::pages(items.name(item), &text::Plain).swap_remove(0), colours));
            *row = Row { item, count, kind: known.map_or(0, |known| known.kind), name };
        }
        let mut results = Results {
            phase: Phase::Coming,
            panels: layout.player(SET_PANELS),
            row: layout.player(SET_ROW),
            band: layout.player(SET_LEVEL),
            band_shown: false,
            rows,
            earned,
            had,
            shown: had as f32,
            counted: 0.0,
            step: 1.0,
            level,
            base: 0,
            next: 0,
            named: (level, level),
            reached: false,
        };
        results.span(levels);
        results
    }

    /// The experience the level the count is at begins and ends with, and what a
    /// frame of the count is worth in it (0x38450, 0x3795C).
    fn span(&mut self, levels: &Levels) {
        let at = |level: i32| levels.threshold(level).map_or(MOST_EXPERIENCE, |experience| experience.min(MOST_EXPERIENCE as u32) as i32);
        (self.base, self.next) = (at(self.level), at(self.level + 1));
        let span = (self.next - self.base) as f32;
        self.step = if span < COUNT_FRAMES { 1.0 } else { span / COUNT_FRAMES };
    }

    /// She is at the last level: there is nothing more to count to (0x3848C).
    fn stop_at_the_last(&mut self, levels: &Levels) {
        let last = levels.threshold(LAST_LEVEL).map_or(MOST_EXPERIENCE, |experience| experience as i32);
        (self.shown, self.base, self.next) = (last as f32, last, last);
    }

    /// The band says she has gone from one level to the next (0x365A4, 0x36600):
    /// it comes, or if it is there its figure comes anew.
    fn name_levels(&mut self, layout: &Layout, before: i32, after: i32) {
        if self.band_shown {
            if let Some(track) = self.band.track_of(LEVEL_AFTER) {
                self.band.rewind_track(layout, track);
            }
        } else {
            self.band.rewind(layout);
            self.band_shown = true;
        }
        self.named = (before, after);
    }

    /// Whether all is told, and a button can close them (0x37BC4 answers so of its
    /// state 8).
    pub fn told(&self) -> bool {
        self.phase == Phase::Told
    }

    /// One frame of them (0x3821C), of so many of the interface's frames; `skip` is
    /// cross having been pressed.
    pub fn step(&mut self, what: &ResultLayout, levels: &Levels, frames: f32, skip: bool) -> Heard {
        let layout = &what.layout;
        let mut heard = Heard::default();
        match self.phase {
            Phase::Coming => {
                if self.panels.finished(layout) {
                    self.phase = if self.earned > 0 { Phase::Counting } else { Phase::Told };
                }
            }
            Phase::Counting if skip => {
                // All of it at once (0x383C0).
                if self.level < LAST_LEVEL {
                    self.shown = (self.had + self.earned) as f32;
                    let before = self.level;
                    self.level = levels.level_for(self.shown as u32);
                    if before < self.level {
                        self.name_levels(layout, self.level - 1, self.level);
                        heard.level = true;
                    }
                    if self.level < LAST_LEVEL {
                        self.span(levels);
                    } else {
                        self.stop_at_the_last(levels);
                    }
                    heard.count = true;
                }
                self.phase = Phase::Told;
            }
            Phase::Counting if self.level >= LAST_LEVEL => self.phase = Phase::Told,
            Phase::Counting => 'count: {
                // While the band of a level reached is coming the count waits
                // (0x3660C), then goes on from the level reached.
                if self.band_shown && !self.band.finished(layout) {
                    break 'count;
                }
                if std::mem::take(&mut self.reached) {
                    self.level += 1;
                    if self.level < LAST_LEVEL {
                        self.span(levels);
                    } else {
                        self.stop_at_the_last(levels);
                    }
                }
                let earned = self.earned as f32;
                if self.counted >= earned || self.level >= LAST_LEVEL {
                    self.phase = Phase::Told;
                    break 'count;
                }
                // A frame's worth more, but no further than the next level and
                // no more than was earned.
                let next = self.next as f32;
                let mut counted = self.counted + self.step;
                if counted < earned {
                    if self.shown + self.step >= next {
                        counted = self.counted + (next - self.shown);
                    }
                } else {
                    counted = earned;
                }
                self.counted = counted;
                self.shown = ((self.had as f32 + counted) as i32).min(MOST_EXPERIENCE) as f32;
                if self.shown >= next {
                    self.name_levels(layout, self.level, self.level + 1);
                    self.reached = true;
                    heard.level = true;
                }
                heard.count = true;
            }
            Phase::Told => {}
        }
        // The motions go on (0x36458).
        self.panels.advance(layout, frames);
        self.row.advance(layout, frames);
        if self.band_shown {
            self.band.advance(layout, frames);
        }
        heard
    }

    /// Draws a number's figures with a track (0x36938): each from its cell of the
    /// figures' sheet, the first at `from` and the others `pitch` further on each,
    /// every one on the track of `back` if there is one.
    #[allow(clippy::too_many_arguments)]
    fn draw_number(&self, what: &ResultLayout, canvas: &mut Canvas, player: &Player, track: usize, back: Option<usize>, number: i32, count: i32, from: Vec2, pitch: f32) {
        let layout = &what.layout;
        for place in 0..count {
            let inner = Affine2::from_translation(from + Vec2::new(pitch * place as f32, 0.0));
            if let Some(back) = back.and_then(|group| player.track_of(group)) {
                layout.draw_track(canvas, layer::DISPLAY, player, back, &Look { inner, ..default() });
            }
            let shift = Vec2::new(FIGURE_CELL * figure(number, count, place) as f32 / what.figures, 0.0);
            layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner, shift, ..default() });
        }
    }

    /// Draws them (0x366AC).
    pub fn draw(&self, what: &ResultLayout, canvas: &mut Canvas, fonts: &Fonts) {
        let layout = &what.layout;
        let width = |group: usize| layout.first_of(group).map_or(0.0, |object| layout.objects.objects[object].size[0] as f32);
        // The panels.
        for track in 0..self.panels.len() {
            let Some(group) = self.panels.group(track) else { continue };
            match group {
                BAR => {
                    let Some(bar) = layout.first_of(group).map(|object| &layout.objects.objects[object]) else { continue };
                    let whole = bar.size[0] as f32;
                    let filling = self.shown < MOST_EXPERIENCE as f32 && self.shown < self.next as f32;
                    let across = if filling { (whole * ((self.shown - self.base as f32) / (self.next - self.base) as f32)).trunc() } else { whole };
                    let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32);
                    if across > 0.0 {
                        canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(across, bar.size[1] as f32))));
                        layout.draw_track(canvas, layer::DISPLAY, &self.panels, track, &Look::default());
                        canvas.cut_to(None);
                    }
                }
                WANTING => {
                    let wanting = if self.named.0 < LAST_LEVEL { self.next - self.shown as i32 } else { 0 };
                    self.draw_number(what, canvas, &self.panels, track, Some(WANTING_BACK), wanting, figures(wanting), Vec2::ZERO, width(group) + 1.0);
                }
                EARNED => self.draw_number(what, canvas, &self.panels, track, Some(EARNED_BACK), self.earned, figures(self.earned), Vec2::ZERO, width(group) + 1.0),
                WANTING_BACK | EARNED_BACK => {}
                _ => layout.draw_track(canvas, layer::DISPLAY, &self.panels, track, &Look::default()),
            }
        }
        // The rows, one under the other.
        for (number, row) in self.rows.iter().enumerate() {
            let down = Vec2::new(0.0, ROW_PITCH * number as f32);
            let look = Look { inner: Affine2::from_translation(down), ..default() };
            for track in 0..self.row.len() {
                let Some(group) = self.row.group(track) else { continue };
                match group {
                    COUNT_BACK => {}
                    COUNT => {
                        if row.count > 0 {
                            let count = figures(row.count).max(2);
                            let pitch = width(group) + 1.0;
                            let from = down - Vec2::new(pitch * (count - 1) as f32, 0.0);
                            self.draw_number(what, canvas, &self.row, track, Some(COUNT_BACK), row.count, count, from, pitch);
                        }
                    }
                    NAME | ICON => {
                        if row.item <= 0 {
                            continue;
                        }
                        let Some((moved, colour)) = layout.placed(&self.row, track) else { continue };
                        let Some((object, place)) = layout.first_of(group).and_then(|object| Some((object, layout.place(object)?))) else { continue };
                        let tint = modulate(colour, layout.colour(object));
                        if group == NAME {
                            let Some(name) = &row.name else { continue };
                            let at = (moved.translation + down + place).trunc();
                            fonts.draw(canvas, layer::DISPLAY, name, &Placing { tint, shadow: true, ..Placing::at(at) });
                        } else {
                            // Each part without its fraction (0x3705C).
                            let at = moved.translation.trunc() + down + place;
                            let icon = icon_of(row.kind);
                            let from = Vec2::new((icon % ICONS_ACROSS) as f32, (icon / ICONS_ACROSS) as f32) * ICON_CELL + Vec2::ONE;
                            let to = from + Vec2::splat(ICON_SIZE);
                            canvas.sprite(
                                layer::DISPLAY,
                                Paint::textured(&what.icons),
                                Vertex::new(at, from / what.icons_size, tint),
                                Vertex::new(at + Vec2::splat(ICON_SIZE), to / what.icons_size, tint),
                            );
                        }
                    }
                    NO_NAME | NO_COUNT => {
                        if row.item > 0 {
                            continue;
                        }
                        let (times, pitch) = if group == NO_NAME { (4, DASH_PITCH) } else { (2, -DASH_PITCH) };
                        for time in 0..times {
                            let inner = Affine2::from_translation(down + Vec2::new(pitch * time as f32, 0.0));
                            if let Some(back) = self.row.track_of(COUNT_BACK).filter(|_| group == NO_COUNT) {
                                layout.draw_track(canvas, layer::DISPLAY, &self.row, back, &Look { inner, ..default() });
                            }
                            layout.draw_track(canvas, layer::DISPLAY, &self.row, track, &Look { inner, ..default() });
                        }
                    }
                    NO_ICON => {
                        if row.item <= 0 {
                            layout.draw_track(canvas, layer::DISPLAY, &self.row, track, &look);
                        }
                    }
                    _ => layout.draw_track(canvas, layer::DISPLAY, &self.row, track, &look),
                }
            }
        }
        // The band of a level reached: what is before the figures makes room for
        // them.
        if !self.band_shown {
            return;
        }
        let (before, after) = self.named;
        let after_room = AFTER_ROOM - AFTER_ROOM * figures(after) as f32;
        let before_room = BEFORE_ROOM - BEFORE_ROOM * figures(before) as f32;
        for track in 0..self.band.len() {
            let Some(group) = self.band.group(track) else { continue };
            let aside = |by: f32| Look { inner: Affine2::from_translation(Vec2::new(by, 0.0)), ..default() };
            match group {
                LEVEL_BEFORE => {
                    let from = Vec2::new(after_room + before_room, 0.0);
                    self.draw_number(what, canvas, &self.band, track, None, before, figures(before), from, width(group) + 1.0);
                }
                LEVEL_AFTER => self.draw_number(what, canvas, &self.band, track, None, after, figures(after), Vec2::new(after_room, 0.0), width(group) + 1.0),
                LEVEL_WORD => layout.draw_track(canvas, layer::DISPLAY, &self.band, track, &aside(before_room + after_room)),
                LEVEL_ARROW => layout.draw_track(canvas, layer::DISPLAY, &self.band, track, &aside(after_room)),
                _ => layout.draw_track(canvas, layer::DISPLAY, &self.band, track, &Look::default()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Option<(ResultLayout, Levels, Items, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let what = ResultLayout::load(&data, &mut images).unwrap();
        Some((what, Levels::load(&data).unwrap(), Items::load(&data).unwrap(), Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    #[test]
    fn a_numbers_figures_are_read_from_its_first() {
        assert_eq!([figures(0), figures(9), figures(10), figures(95), figures(100)], [1, 1, 2, 2, 3]);
        assert_eq!([figure(95, 2, 0), figure(95, 2, 1), figure(7, 2, 0), figure(7, 2, 1)], [9, 5, 0, 7]);
        assert_eq!([icon_of(1), icon_of(2), icon_of(3), icon_of(5), icon_of(8), icon_of(10), icon_of(11), icon_of(0)], [4, 5, 4, 9, 6, 7, 8, 0]);
    }

    #[test]
    fn the_experience_is_counted_onto_hers_and_a_level_reached_is_named() {
        let Some((what, levels, items, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // As in the walkthrough's first battle: she had eight, and sixteen more take
        // her past the sixteen of the second level, ninety-five short of the third.
        let mut results = Results::new(&what, &levels, &items, &fonts, &colours, 1, 8, 16, &[(1, 1)]);
        assert_eq!((results.base, results.next, results.step), (0, 16, 1.0));
        assert!(results.rows[0].name.is_some() && results.rows[1].name.is_none());
        let drawn = |results: &Results| {
            let mut canvas = Canvas::default();
            results.draw(&what, &mut canvas, &fonts);
            canvas.vertices().len()
        };
        // The panels come over eight frames.
        let mut frames = 0;
        while results.phase == Phase::Coming {
            assert_eq!(results.step(&what, &levels, 2.0, false), Heard::default());
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(frames, 5);
        let panels = drawn(&results);
        assert!(panels > 0);
        // One of experience a frame, a tick each, until the level's sixteen are
        // hers: the band comes, with its sound, and the count waits for it.
        let mut ticks = 0;
        let mut heard = Heard::default();
        while !heard.level {
            heard = results.step(&what, &levels, 2.0, false);
            assert!(heard.count);
            ticks += 1;
            assert!(ticks < 100);
        }
        assert_eq!((ticks, results.shown, results.named), (8, 16.0, (1, 2)));
        assert!(results.band_shown && results.phase == Phase::Counting);
        let mut waited = 0;
        while results.step(&what, &levels, 2.0, false) == Heard::default() && results.phase == Phase::Counting {
            waited += 1;
            assert!(waited < 100);
        }
        assert!(waited >= 3, "the band's motion is six frames and its figure's eight: {waited}");
        assert!(drawn(&results) > panels);
        // The rest is counted into the second level, whose span is a hundred and
        // three: more than three of experience a frame.
        assert_eq!((results.level, results.base, results.next), (2, 16, 119));
        while results.phase == Phase::Counting {
            results.step(&what, &levels, 2.0, false);
        }
        assert_eq!((results.shown, results.next - results.shown as i32), (24.0, 95));
        assert!(results.told());

        // Cross has it all counted at once.
        let mut results = Results::new(&what, &levels, &items, &fonts, &colours, 1, 8, 16, &[]);
        while results.phase == Phase::Coming {
            results.step(&what, &levels, 2.0, true);
        }
        assert_eq!(results.step(&what, &levels, 2.0, true), Heard { level: true, count: true });
        assert_eq!((results.shown, results.level, results.named, results.told()), (24.0, 2, (1, 2), true));
        // A battle worth nothing has nothing to count.
        let mut results = Results::new(&what, &levels, &items, &fonts, &colours, 3, 200, 0, &[]);
        while !results.told() {
            assert_eq!(results.step(&what, &levels, 2.0, false), Heard::default());
        }
        assert!(!results.band_shown);
    }
}
