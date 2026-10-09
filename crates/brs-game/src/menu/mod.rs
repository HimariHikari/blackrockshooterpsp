//! The MENU that triangle brings up on the field (`menuMainTask`, 0x59DCC): the
//! four entries CUSTOMIZE, ITEMS, CHALLENGES and OPTIONS, and beside them her
//! level, experience and figures, with her HP gauge at the foot. See
//! `docs/MENU_NOTES.md`.
//!
//! It is a screen of its own: once it is made nothing of the field is drawn, and
//! what is behind it is the menus' own ground, which comes under a wipe of
//! chequers (`menuBgTask`, `menuFadeTask`). While it is up the game is drawn sixty
//! times a second (0x59FE4), so a frame of the port's logic is two of its frames.
//!
//! Her model stands in it, as the menus' viewer shows it (0x3B404): seen by a camera
//! of the menu's own, over the panel and under her gauge, and turned by L and R.
//!
//! The four screens its entries lead to are CUSTOMIZE, ITEMS, CHALLENGES and
//! OPTIONS (`custom.rs`, `item.rs`, `library.rs`, `option.rs`).

mod custom;
mod gallery;
mod item;
mod library;
mod option;

use std::collections::HashMap;

use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::camera::SubCameraView;
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::{Animator, Clip};
use brs_engine::canvas::{Canvas, Seen};
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_engine::model::ModelAsset;
use brs_engine::AssetStores;
use brs_formats::{insm, xtc};

use self::custom::Custom;
use self::gallery::GalleryScreen;
use self::item::ItemScreen;
use self::library::Library;
use self::option::OptionScreen;
pub use self::library::STAGE_SEEN;
use crate::battle::actor::WEAPON_NODE;
use crate::battle::skills::{self, Skills};
use crate::battle::tables::{Levels, PlayerStats};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::FieldSource;
use crate::field::scene::FilmAsked;
use crate::gallery::{Gallery, Opened};
use crate::items::{Inventory, Items};
use crate::options::Options;
use crate::player::{self, part, ShownParts};
use crate::progress::Progress;
use crate::records::Records;
use crate::field::hud::gauge::Gauge;
use crate::field::scene::{fade_kind, Fade};
use crate::interface::{self, layer};
use crate::sound::Request;
use crate::text::{self, TextColours};
use crate::writing::{Fonts, Placing, Style};

/// The pack the screen is of: file 19 of the pack that is always there (0x2CC94,
/// its + 0x58).
const PACK: &str = "SYSTEM.VOL/if_rsdsys.lpk/019.lpk";
/// Its layout, that of the ground behind it, and its texts (the pack's + 0xC,
/// + 0x34 and + 0x48).
const PANEL_LAYOUT: usize = 0;
const GROUND_LAYOUT: usize = 10;
const TEXTS: usize = 15;
/// The pack and the layout of the wipe (0x4E91C): squares of a chequer, with no
/// texture.
const WIPE_PACK: &str = "SYSTEM.VOL/if_fldsys.lpk";
const WIPE_LAYOUT: usize = 5;

/// The menu's frames in a frame of the port's logic.
pub const FRAMES_A_STEP: usize = 2;

/// The mission in which the field has no menu (0x10DA0C): triangle is answered
/// with the sound of "no".
pub const NO_MENU_SCENARIO: i32 = 44;

// Its sounds, of the bank that is always there: as triangle brings it up
// (0xF1988), as it has come (0x5AEE4), as the cursor moves (0x5AE28), as an entry
// is chosen (0x5ADCC), as it is left (0x5AABC), and "no".
const SOUND_OPENS: u32 = 0;
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_CHOSEN: u32 = 3;
const SOUND_LEFT: u32 = 1;
const SOUND_REFUSED: u32 = 7;

const ENTRIES: usize = 4;

// How it comes (0x5AF0C): nothing of it is there for twelve frames, then the
// panel, the ground and the strip come over fourteen, and at the twenty-seventh
// it is all there.
const COMES_FROM: i32 = 12;
const COMES_OVER: f32 = 14.0;
const COME_AT: i32 = 27;
/// The frames in which it can be used before its sound is played (0x5AEA8).
const SOUND_AFTER: i32 = 3;
/// The frames the panel has been coming for when her gauge is first drawn
/// (0x5F6DC).
const GAUGE_AFTER: i32 = 0x16;
/// The frames of the interface the screen takes to go dark as the menu is left
/// (0x5AAEC), and to come back on the field after it (0xF1E10): a frame of the
/// field is two of them.
const LEAVING_FRAMES: i32 = 15;
pub const BACK_FRAMES: i32 = LEAVING_FRAMES / 2;

// The sets of the wipe (0x4E838 to 0x4E8A8): it comes with the first and goes
// with the second; the third, which covers the screen and uncovers it again, is
// for going to a screen of the menu and back.
const WIPE_COMES: usize = 0;
const WIPE_GOES: usize = 1;
const WIPE_BETWEEN: usize = 2;
/// The frames a set has been playing for when the wipe answers that it has done
/// going (0x4EBFC): the third has the screen covered by then.
const WIPE_GONE_AFTER: f32 = 14.0;
/// The frames of the game the screen's fade takes to come off a screen that is
/// up alone (0x5CC1C): fifteen of the interface's, while the title has the game
/// drawn thirty times a second.
const ALONE_COMES_OVER: i32 = 15 / FRAMES_A_STEP as i32;

// The panel's set, and what its groups are to the code that draws it (0x5FE00).
const PANEL_SET: usize = 0;
/// The entries: of each group's tracks the first is drawn while the cursor is not
/// on the entry and the others while it is.
const ENTRY_GROUPS: [usize; ENTRIES] = [6, 7, 48, 8];
/// Drawn only while the cursor is on the entry.
const MARK_GROUPS: [usize; ENTRIES] = [13, 14, 15, 16];
/// The group whose track's end is the panel's being ready (0x5F738).
const READY_GROUP: usize = 13;
/// The figures: the group one is drawn with, and the group that is drawn under
/// each of its digits.
const ATTACK: (usize, usize) = (23, 20);
const DEFENCE: (usize, usize) = (24, 21);
const SPECIAL: (usize, usize) = (25, 22);
const EXPERIENCE: (usize, usize) = (31, 29);
const NEXT: (usize, usize) = (32, 30);
const LEVEL: (usize, usize) = (38, 37);
const MOST_HP: (usize, usize) = (54, 53);
const UNDER_DIGITS: [usize; 7] = [ATTACK.1, DEFENCE.1, SPECIAL.1, EXPERIENCE.1, NEXT.1, LEVEL.1, MOST_HP.1];
/// The bar of her experience towards the next level.
const BAR_GROUP: usize = 50;

// The ground's sets: the ground itself, and the strip along the foot, in which
// the group of this number is where the text about the entry is written
// (0x4AC94).
const GROUND_SET: usize = 0;
const STRIP_SET: usize = 1;
const STRIP_TEXT_GROUP: usize = 22;
/// How the strip's text is written (0x4AA54).
const STRIP_STYLE: Style = Style::of_kind(0, 0x30, 6, 1.0, 4.0);

/// The sheet the figures are cells of (the first of the table at 0x171078): ten
/// to a row, each twenty across.
const FIGURE_CELL: f32 = 20.0;

/// How far from where its layout has it her HP gauge stands in the menus (0x605D8
/// sets its place to 2): a pixel left and ten higher.
const GAUGE_PLACE: Vec2 = Vec2::new(-1.0, -10.0);

// Her model in the panel. The animations it is given are files of the menu's pack
// (0x5F850: the pack's + 0x2C, and 0x2CEE8 for the one over it).
const HER_POSE: usize = 8;
const HER_POSE_OVER: usize = 9;
/// What of her model's parts the panel has shown (0x5F894): an empty left hand,
/// and the right hand in the second of its shapes.
const HER_PARTS: [u16; 2] = [part::LEFT_HAND_EMPTY, part::RIGHT_HAND_OTHER];
/// How the menus' viewer has her model seen by a screen: how she is turned, in
/// degrees about x, y and z; the camera that sees her, where it is, what it looks
/// at, its lens in degrees, and how near and how far it sees; and how far from
/// the middle of the screen the middle of its picture is, in the screen's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub turns: Vec3,
    pub eye: Vec3,
    pub looked_at: Vec3,
    pub lens: f32,
    pub near: f32,
    pub far: f32,
    pub picture: Vec2,
}

/// The panel's (0x5FB2C, 0x5FBC0, and 0x5FC34: 0x860 and 0x804 of the PSP's own
/// space); L and R add to the second of its turns.
const PANEL_VIEW: View = View {
    turns: Vec3::new(-1.0, 5.0, -1.0),
    eye: Vec3::new(11.0, 148.0, 341.0),
    looked_at: Vec3::new(1.0, 93.0, 16.0),
    lens: 21.0,
    near: 100.0,
    far: 1000.0,
    picture: Vec2::new(96.0, 4.0),
};
/// Where the port has her stand for it: nowhere near anything of a map, so that
/// the menu's camera sees nothing else.
const HER_PLACE: Vec3 = Vec3::new(0.0, 100_000.0, 0.0);
/// The menu's camera draws after the field's and before the canvas's.
const HER_CAMERA_ORDER: isize = 5;

/// What the menus are drawn from.
#[derive(Resource)]
pub struct MenuAssets {
    panel: Layout,
    ground: Layout,
    wipe: Layout,
    gauge: Gauge,
    /// What the strip says of each entry.
    texts: Vec<String>,
    /// The width of the texture the figures are in.
    figures: f32,
    /// Her model and the cannon she holds, and the animations the panel has her
    /// in: one, and another over it.
    her: (insm::Model, Vec<u8>),
    cannon: (insm::Model, Vec<u8>),
    pose: Arc<Clip>,
    pose_over: Arc<Clip>,
    /// What CUSTOMIZE, ITEMS, CHALLENGES and OPTIONS are drawn from, and the
    /// window a screen asks with (the one the maps' scripts put up).
    custom: custom::Assets,
    item: item::Assets,
    library: library::Assets,
    option: option::Assets,
    gallery: gallery::Assets,
    window: Layout,
}

impl MenuAssets {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<MenuAssets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        // The panel's textures (0x5FCC8): four of the pack, the figures (0x2CD24),
        // one more of the pack, one place that is left empty, and one of the pack
        // that is always there (0x2CE28).
        let panel = [own(3), own(4), own(5), own(6), shared(9), own(7), own(3), shared(12)];
        // The ground's (0x4A9E8): one of the pack, one that is always there
        // (0x2CE28), one of the pack, an empty place and another that is always
        // there (0x2CD58).
        let ground = [own(13), shared(12), own(14), own(13), shared(10)];
        let table = xtc::Table::parse(&data.read(interface::file(PACK, TEXTS, "xtc"))?).context("the menu's texts")?;
        let text = |index: usize| -> Result<String> {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("the menu has no text {index}"))?;
            Ok(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
        };
        Ok(MenuAssets {
            panel: interface::layout_with(data, images, PACK, PANEL_LAYOUT, &panel)?,
            ground: interface::layout_with(data, images, PACK, GROUND_LAYOUT, &ground)?,
            wipe: interface::layout(data, images, WIPE_PACK, WIPE_LAYOUT)?,
            gauge: crate::field::hud::gauge(data, images)?,
            texts: (0..ENTRIES).map(text).collect::<Result<_>>()?,
            figures: interface::load_texture(data, images, &shared(9))?.1.x,
            her: data.model(crate::player::model())?,
            cannon: data.model(crate::battle::WEAPON)?,
            pose: data.clip(interface::file(PACK, HER_POSE, "insa"))?,
            pose_over: data.clip(interface::file(PACK, HER_POSE_OVER, "insa"))?,
            custom: custom::Assets::load(data, images).context("what CUSTOMIZE is drawn from")?,
            item: item::Assets::load(data, images).context("what ITEMS is drawn from")?,
            library: library::Assets::load(data, images).context("what CHALLENGES is drawn from")?,
            option: option::Assets::load(data, images).context("what OPTIONS is drawn from")?,
            gallery: gallery::Assets::load(data, images).context("what the GALLERY is drawn from")?,
            window: interface::layout(data, images, crate::field::hud::RESIDENT_PACK_WINDOW, 0)?,
        })
    }

    /// The frame at which the panel is ready to be used (0x31CBC with the set and
    /// group 13): when that group's track has played its clip through.
    fn ready_at(&self) -> f32 {
        self.panel.track_of(PANEL_SET, READY_GROUP).map_or(0.0, |track| self.panel.track_length(PANEL_SET, track))
    }
}

/// The states the menu's task goes through (0x5A9C0), by the numbers it has for
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Its first frame: nothing of it is made yet.
    First = 1,
    /// The screen is made (0x5A7DC) and the wipe begins to come.
    Made = 2,
    /// It comes, until the wipe has.
    Coming = 0,
    /// It is the player's to use.
    Open = 3,
    /// An entry was chosen: until the wipe has covered the screen. It has no
    /// frame of its own meanwhile, and is still drawn.
    Chosen = 4,
    /// The entry's screen has its turn: nothing of this one is drawn.
    Screen = 6,
    /// Back from the entry's screen (0x5A660).
    Back = 5,
    /// It is left: until the screen has gone dark.
    Leaving = 8,
    Done = 10,
}

/// The wipe of chequers (`menuFadeTask`, 0x4E734): it plays one of its sets, and is
/// drawn only until that has been played through.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Wipe {
    set: usize,
    frames: f32,
    playing: bool,
}

impl Wipe {
    const IDLE: Wipe = Wipe { set: WIPE_COMES, frames: 0.0, playing: false };

    /// It is told to play a set (0x4EB38, 0x4EB80).
    fn play(&mut self, set: usize) {
        *self = Wipe { set, frames: 0.0, playing: true };
    }

    /// A frame of it (0x4EA08).
    fn frame(&mut self, layout: &Layout) {
        if self.playing {
            self.frames += 1.0;
            self.playing = self.frames < layout.length(self.set);
        }
    }

    /// Whether it has done going (0x4EBFC): its set has been playing long enough.
    fn gone(&self) -> bool {
        self.frames >= WIPE_GONE_AFTER
    }

    /// Whether it is neither coming nor going any more (0x4EBD4).
    fn idle(&self) -> bool {
        !self.playing
    }
}

/// What of her the menus go by and change: the tables of skills, of items and
/// of records, her flags, her figures and what she carries; the stage the map
/// she is on is of (0x1294, 0x110368); and the player's options.
pub struct Hers<'a> {
    pub skills: &'a Skills,
    pub items: &'a Items,
    pub records: &'a Records,
    pub stage: i32,
    pub progress: &'a mut Progress,
    pub stats: &'a mut PlayerStats,
    pub inventory: &'a mut Inventory,
    pub options: &'a mut Options,
    /// What the gallery has, and what of it is open.
    pub gallery: &'a Gallery,
    pub opened: &'a Opened,
    /// Something of her was changed.
    pub changed: bool,
}

/// How the screen of an entry ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Back to the MENU (0x5C738).
    Back,
    /// Out of the menus altogether (0x5C768).
    Out,
}

impl Hers<'_> {
    /// Has her figures worked out anew (0x14D008): what her skills add to them.
    fn figure(&mut self) {
        let bonuses = self.skills.bonuses(self.stats.skills, self.progress);
        if bonuses != self.stats.skill_bonus {
            self.stats.take_bonuses(bonuses);
            self.changed = true;
        }
    }

    /// Puts a skill that is always in force aside, or takes it up again
    /// (0x14D994), which has her figures worked out anew (0x13DC, 0x1418).
    fn put_aside(&mut self, skill: i32, aside: bool) {
        self.progress.set_flag(skills::SET + skill, aside);
        self.changed = true;
        self.figure();
    }

    /// Has one of her four slots hold a skill, or none (0x14CCF0).
    fn set_slot(&mut self, slot: usize, skill: i32) {
        let skill = u8::try_from(skill).unwrap_or(0);
        if self.stats.skills[slot] != skill {
            self.stats.skills[slot] = skill;
            self.changed = true;
        }
    }

    /// Has her use one of a consumable she carries (0x56854, 0x14D594): she
    /// takes what it gives and has one fewer. Answers how much HP she has more.
    fn take(&mut self, item: i32) -> i32 {
        let gave = self.items.get(item).map_or(0, |of| self.stats.take(of));
        self.inventory.add(self.items, item, -1);
        self.changed = true;
        gave
    }

    /// Has her change to a weapon or a suit (0x14D0CC), and her figures worked
    /// out anew.
    fn equip(&mut self, item: i32) {
        self.stats.equip(self.items, item);
        self.changed = true;
        self.figure();
    }
}

/// Where the text of a track's group is written and the colour it is drawn by
/// (the same for every text of 0x4C1C8 and 0x551A8): at the group's first
/// object, moved as the track has it and `down`, each cut to a whole number; in
/// the track's colour by the object's. Nothing while the track has no alpha.
fn text_place(layout: &Layout, player: &Player, track: usize, down: f32) -> Option<(Vec2, [u8; 4])> {
    let (moved, colour) = layout.placed(player, track)?;
    let object = layout.first_of(player.group(track)?)?;
    let place = layout.place(object)?;
    let at = Vec2::new(moved.translation.x.trunc() + place.x, moved.translation.y.trunc() + down.trunc() + place.y);
    Some((at, modulate(colour, layout.colour(object))))
}

/// Writes a text of a screen (0x2D900): with no shadow, by a colour.
#[allow(clippy::too_many_arguments)]
fn write_text(canvas: &mut Canvas, layer: i32, fonts: &Fonts, colours: &TextColours, style: &Style, words: &str, at: Vec2, tint: [u8; 4]) {
    let block = fonts.write(style, &text::pages(words, &text::Plain).swap_remove(0), colours);
    fonts.draw(canvas, layer, &block, &Placing { tint, ..Placing::at(at) });
}

/// Draws a number with a track of a layout (0x60374, 0x55DA4, 0x56254): each
/// digit is the track's group drawn from that digit's cell of the figures'
/// sheet, which is `figures` wide, over the first track of the group that goes
/// under digits; and they stand a digit's width and one more apart, on from
/// where the group is, or back from it when the number is set to the right. It
/// has `at_least` so many digits, noughts before it if it is shorter.
#[allow(clippy::too_many_arguments)]
fn draw_number(canvas: &mut Canvas, layer: i32, layout: &Layout, figures: f32, player: &Player, track: usize, under: usize, number: i32, at_least: i32, to_the_right: bool, look: &Look) {
    let Some(width) = player.group(track).and_then(|group| layout.first_of(group)).map(|object| layout.objects.objects[object].size[0] as f32) else { return };
    let (count, mut left) = (self::figures(number).max(at_least), number.max(0));
    let under = player.track_of(under);
    for place in 0..count {
        let unit = 10_i32.pow((count - 1 - place) as u32);
        let (digit, rest) = (left / unit, left % unit);
        left = rest;
        let across = if to_the_right { place - count + 1 } else { place } as f32 * (width + 1.0);
        let inner = look.inner * Affine2::from_translation(Vec2::new(across, 0.0));
        if let Some(under) = under {
            layout.draw_track(canvas, layer, player, under, &Look { inner, ..*look });
        }
        let shift = Vec2::new(FIGURE_CELL * digit as f32 / figures, 0.0);
        layout.draw_track(canvas, layer, player, track, &Look { inner, shift, ..*look });
    }
}

/// The screen of an entry, while it has its turn (the MENU's + 0xB4).
#[derive(Clone, Debug, PartialEq)]
enum Screen {
    Custom(Custom),
    Items(ItemScreen),
    Library(Library),
    Options(OptionScreen),
}

impl Screen {
    /// A frame of its task. Answers how it ended, in the frame it does.
    fn frame(&mut self, assets: &MenuAssets, hers: &mut Hers, wipe: &mut Wipe, fade: &mut Fade, pad: &Pad, first: bool) -> Option<Outcome> {
        match self {
            Screen::Custom(custom) => custom.frame(&assets.custom, hers, wipe, fade, pad, first),
            Screen::Items(items) => items.frame(&assets.item, hers, wipe, fade, pad, first),
            Screen::Library(library) => library.frame(&assets.library, hers, wipe, fade, pad, first),
            Screen::Options(options) => options.frame(&assets.option, hers, wipe, fade, pad, first),
        }
    }

    /// What it has asked of the sound.
    fn sounds(&mut self) -> &mut Vec<Request> {
        match self {
            Screen::Custom(custom) => &mut custom.sounds,
            Screen::Items(items) => &mut items.sounds,
            Screen::Library(library) => &mut library.sounds,
            Screen::Options(options) => &mut options.sounds,
        }
    }

    /// What it has the strip along the foot say.
    fn strip<'a>(&self, assets: &'a MenuAssets) -> &'a str {
        match self {
            Screen::Custom(custom) => custom.strip(&assets.custom),
            Screen::Items(items) => items.strip(&assets.item),
            Screen::Library(library) => library.strip(&assets.library),
            Screen::Options(options) => options.strip(&assets.option),
        }
    }
}

/// Two buttons move a cursor through so many places (0x2E314): a step for each
/// time one counts as pressed, and round from the last place to the first or back
/// only when it has newly gone down. Answers whether the cursor moved.
fn step_cursor(back: u32, on: u32, cursor: &mut usize, places: usize, pressed: u32, repeat: u32) -> bool {
    let last = places.saturating_sub(1);
    let to = if repeat & on != 0 {
        match *cursor {
            at if at < last => Some(at + 1),
            _ => (pressed & on != 0).then_some(0),
        }
    } else if repeat & back != 0 {
        match *cursor {
            0 => (pressed & back != 0).then_some(last),
            at => Some(at - 1),
        }
    } else {
        None
    };
    if let Some(to) = to {
        *cursor = to;
    }
    to.is_some()
}

/// The MENU screen.
#[derive(Clone, Debug, PartialEq)]
pub struct MainMenu {
    state: State,
    /// Leaving has been asked for (its + 0x18 is 2 or 4).
    leaving_asked: bool,
    /// The entry the cursor is on (+ 0x20).
    pub entry: usize,
    /// It is still coming (flag 8), and for how many frames it has been (+ 0xB8).
    coming: bool,
    came: i32,
    /// The frames it has been waiting to sound in (+ 0xBC), and whether it has
    /// sounded (flag 0x10000).
    waited: i32,
    sounded: bool,
    /// How much there is of the panel, the ground and the strip (0x5F9B8, 0x609B8,
    /// 0x609D4).
    alpha: u8,
    /// The frames the panel's layout has played (its player's), and those it has
    /// been coming for until her gauge is drawn (its + 0x24 and flag 8).
    panel_frames: f32,
    panel_came: i32,
    /// The panel's first motion is over (its flag 1): it can be used.
    ready: bool,
    /// How far L and R have turned her model (the panel's + 0x44).
    pub turn: f32,
    /// The frames the ground's layout has played.
    ground_frames: f32,
    wipe: Wipe,
    /// The screen's fade, which is the menu's to start as it is left.
    fade: Fade,
    /// The screen of an entry, while it has its turn (+ 0xB4).
    screen: Option<Screen>,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl MainMenu {
    fn new() -> MainMenu {
        MainMenu {
            state: State::First,
            leaving_asked: false,
            entry: 0,
            coming: false,
            came: 0,
            waited: 0,
            sounded: false,
            alpha: 0,
            panel_frames: 0.0,
            panel_came: 0,
            ready: false,
            turn: 0.0,
            ground_frames: 0.0,
            wipe: Wipe::IDLE,
            fade: Fade::CLEAR,
            screen: None,
            sounds: Vec::new(),
        }
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// Whether anything of it has been made: from then on nothing of the field is
    /// drawn (0x10F224).
    fn made(&self) -> bool {
        self.state != State::First
    }

    /// Whether the panel is drawn (0x5A8FC).
    fn shows_panel(&self) -> bool {
        matches!(self.state, State::Coming | State::Open | State::Chosen | State::Leaving)
    }

    /// A frame of the menu's task (0x5A9C0), with what the pad has for it: the
    /// buttons newly down and those that count as pressed again are for the
    /// `first` of the frames a frame of the game's logic is.
    fn frame(&mut self, assets: &MenuAssets, hers: &mut Hers, pad: &Pad, first: bool) {
        let held = pad.held;
        let (pressed, repeat) = if first { (pad.pressed, pad.repeat) } else { (0, 0) };
        // The ground behind is a task of its own, and has its frame first
        // (0x4AAF0): it is there from when the screen is made.
        if self.made() && self.state != State::Made {
            self.ground_frames += 1.0;
        }
        match self.state {
            State::First => self.state = State::Made,
            State::Made => {
                // The screen is made with nothing of it to be seen yet, and the
                // wipe is told to come.
                (self.alpha, self.coming, self.came) = (0, true, 0);
                self.wipe.play(WIPE_COMES);
                self.state = State::Coming;
            }
            State::Coming => {
                self.screen(assets, held, pressed, repeat);
                if !self.fade.running() && !self.wipe.playing {
                    self.state = State::Open;
                }
            }
            State::Open => {
                self.screen(assets, held, pressed, repeat);
                if self.leaving_asked {
                    self.sound(SOUND_LEFT);
                    self.wipe.play(WIPE_GOES);
                    self.fade.begin(fade_kind::OUT, LEAVING_FRAMES);
                    self.state = State::Leaving;
                }
            }
            State::Chosen => {
                // With the screen covered, and nothing of its files to wait for,
                // the entry's screen is made (0x5A398).
                if self.wipe.gone() {
                    self.screen = Some(match self.entry {
                        0 => Screen::Custom(Custom::new(&assets.custom, hers)),
                        1 => Screen::Items(ItemScreen::new(&assets.item, hers)),
                        2 => Screen::Library(Library::new(&assets.library, hers)),
                        _ => Screen::Options(OptionScreen::new(&assets.option)),
                    });
                    self.state = State::Screen;
                }
            }
            State::Screen => {
                let mut ended = None;
                if let Some(screen) = &mut self.screen {
                    ended = screen.frame(assets, hers, &mut self.wipe, &mut self.fade, pad, first);
                    self.sounds.append(screen.sounds());
                }
                // As it ends it says whether this screen is to come back (0x5A5EC)
                // or the menus are done with (0x5A718).
                match ended {
                    Some(Outcome::Back) => (self.screen, self.state) = (None, State::Back),
                    Some(Outcome::Out) => (self.screen, self.state) = (None, State::Done),
                    None => {}
                }
            }
            State::Back => {
                // The panel is all there, and has to find itself ready again
                // (0x5F7D4), which its next frame does.
                (self.alpha, self.ready) = (255, false);
                self.state = State::Open;
            }
            State::Leaving => {
                self.screen(assets, held, pressed, repeat);
                // With nothing of its files to wait for it is done at once.
                if !self.fade.running() {
                    self.state = State::Done;
                }
            }
            State::Done => {}
        }
        // The wipe and the screen's fade have their frames after it.
        self.wipe.frame(&assets.wipe);
        self.fade.advance();
    }

    /// A frame of the screen itself (0x5ACC4).
    fn screen(&mut self, assets: &MenuAssets, held: u32, pressed: u32, repeat: u32) {
        if self.state == State::Open {
            if self.ready {
                if pressed & (button::CIRCLE | button::TRIANGLE) != 0 {
                    self.leaving_asked = true;
                } else if pressed & button::CROSS != 0 {
                    // The wipe covers the screen, and the entry's screen is made
                    // under it (0x5ADCC).
                    self.wipe.play(WIPE_BETWEEN);
                    self.sound(SOUND_CHOSEN);
                    self.state = State::Chosen;
                } else if step_cursor(button::UP, button::DOWN, &mut self.entry, ENTRIES, pressed, repeat) {
                    self.sound(SOUND_CURSOR);
                } else if held & button::L != 0 {
                    self.turn -= 1.0;
                } else if held & button::R != 0 {
                    self.turn += 1.0;
                }
            } else if !self.sounded {
                if self.waited < SOUND_AFTER {
                    self.waited += 1;
                } else {
                    self.sound(SOUND_COME);
                    (self.waited, self.sounded) = (0, true);
                }
            }
        }
        if self.coming {
            if self.came >= COME_AT {
                self.coming = false;
            } else if self.came >= COMES_FROM {
                self.alpha = (255.0 * (self.came - COMES_FROM) as f32 / COMES_OVER) as u8;
            }
            self.came += 1;
        }
        // The panel's own frame (0x5F6A4): its layout plays on, her gauge is to be
        // drawn once it has been coming long enough, and it is ready when the
        // track of one of its groups has played its clip through.
        self.panel_frames += 1.0;
        if self.panel_came < GAUGE_AFTER {
            self.panel_came += 1;
        }
        if !self.ready && self.panel_frames >= assets.ready_at() {
            self.ready = true;
        }
    }

    /// Whether her gauge is drawn yet (the panel's flag 8).
    fn shows_gauge(&self) -> bool {
        self.panel_came >= GAUGE_AFTER
    }
}

/// The screen that is up alone.
#[derive(Clone, Debug, PartialEq)]
enum Alone {
    Options(OptionScreen),
    Gallery(GalleryScreen),
}

/// A screen of the menus up alone, with the menus' ground behind it and no MENU
/// to go back to (`menuSingleBoot`, 0x5C798): how the title has OPTIONS and the
/// GALLERY. The title has the game drawn thirty times a second, so a frame of
/// the port's logic is one of its own, and two of the interface's.
#[derive(Clone, Debug, PartialEq)]
pub struct Single {
    screen: Alone,
    /// The frames the ground's layout has played.
    ground_frames: f32,
    /// The screen's fade, which the screen comes and goes under.
    fade: Fade,
    sounds: Vec<Request>,
}

impl Single {
    /// OPTIONS, as the title makes it (0x5CAE0): the ground and the strip all
    /// there, and the screen's fade coming off from the whole of it.
    fn options(assets: &MenuAssets) -> Single {
        let mut fade = Fade::DARK;
        fade.begin(fade_kind::IN, ALONE_COMES_OVER);
        Single { screen: Alone::Options(OptionScreen::alone(&assets.option)), ground_frames: 0.0, fade, sounds: Vec::new() }
    }

    /// The GALLERY, likewise (0x5CAE0 with 1).
    fn gallery(assets: &MenuAssets, gallery: &Gallery, opened: &Opened) -> Single {
        let mut fade = Fade::DARK;
        fade.begin(fade_kind::IN, ALONE_COMES_OVER);
        Single { screen: Alone::Gallery(GalleryScreen::new(&assets.gallery, gallery, opened)), ground_frames: 0.0, fade, sounds: Vec::new() }
    }

    /// A frame of the game's logic (0x5CD78). Answers whether the screen is
    /// done with (0x5CCE4).
    fn step(&mut self, assets: &MenuAssets, pad: &Pad, hers: &mut Hers) -> bool {
        self.ground_frames += FRAMES_A_STEP as f32;
        let ended = match &mut self.screen {
            Alone::Options(screen) => {
                let mut wipe = Wipe::IDLE;
                let ended = screen.frame(&assets.option, hers, &mut wipe, &mut self.fade, pad, true);
                self.sounds.append(&mut screen.sounds);
                ended
            }
            Alone::Gallery(screen) => {
                let ended = screen.frame(&assets.gallery, hers.gallery, hers.opened, &mut self.fade, pad);
                self.sounds.append(&mut screen.sounds);
                ended
            }
        };
        self.fade.advance();
        ended.is_some()
    }

    /// The GALLERY, if that is what is up.
    fn gallery_screen(&mut self) -> Option<&mut GalleryScreen> {
        match &mut self.screen {
            Alone::Gallery(screen) => Some(screen),
            Alone::Options(_) => None,
        }
    }

    /// Draws it as it is `ahead` of a frame of the game's logic past its last
    /// one: the ground, the screen, the strip with what the screen has it say,
    /// and the screen's fade.
    #[allow(clippy::too_many_arguments)]
    fn draw(&self, canvas: &mut Canvas, assets: &MenuAssets, fonts: &Fonts, colours: &TextColours, options: &Options, seen: &Seen, ahead: f32) {
        let frames = self.ground_frames + ahead * FRAMES_A_STEP as f32;
        // Under a film of the gallery's nothing of the menus is drawn (0x4F524).
        let film = matches!(&self.screen, Alone::Gallery(screen) if screen.film_up());
        if !film {
            canvas.fill(layer::MENU_GROUND, seen.0, [0, 0, 0, 255]);
            let ground = Player::at(&assets.ground, GROUND_SET, frames);
            assets.ground.draw(canvas, layer::MENU_GROUND, &ground, &Look::default());
        }
        match &self.screen {
            Alone::Options(screen) => {
                screen.draw(canvas, &assets.option, &assets.window, fonts, colours, options, ahead);
                if screen.drawn() {
                    draw_strip(canvas, assets, fonts, colours, layer::MENU_SCREEN, screen.strip(&assets.option), frames, 255);
                }
            }
            Alone::Gallery(screen) => {
                screen.draw(canvas, &assets.gallery, fonts, colours, ahead);
                if screen.shows_strip() {
                    draw_strip(canvas, assets, fonts, colours, layer::MENU_SCREEN, screen.strip(&assets.gallery), frames, 255);
                }
            }
        }
        let dark = (self.fade.level.clamp(0.0, 1.0) * 255.0) as u8;
        if dark != 0 {
            canvas.fill(layer::FADE, seen.0, [0, 0, 0, dark]);
        }
    }
}

/// Draws the strip along the foot (0x4AC08) as it is after so many frames, by an
/// alpha, with what it is to say: every track of its set as it is, but the one
/// in whose place that is written.
#[allow(clippy::too_many_arguments)]
fn draw_strip(canvas: &mut Canvas, assets: &MenuAssets, fonts: &Fonts, colours: &TextColours, layer: i32, words: &str, frames: f32, alpha: u8) {
    let layout = &assets.ground;
    let player = Player::at(layout, STRIP_SET, frames);
    let tint = [255, 255, 255, alpha];
    for track in 0..player.len() {
        if player.group(track) != Some(STRIP_TEXT_GROUP) {
            layout.draw_track(canvas, layer, &player, track, &Look::tinted(tint));
            continue;
        }
        let (Some((moved, colour)), Some(object)) = (layout.placed(&player, track), layout.first_of(STRIP_TEXT_GROUP)) else { continue };
        let Some(place) = layout.place(object) else { continue };
        let block = fonts.write(&STRIP_STYLE, &text::pages(words, &text::Plain).swap_remove(0), colours);
        let tint = modulate(modulate(colour, layout.colour(object)), tint);
        fonts.draw(canvas, layer, &block, &Placing { tint, ..Placing::at(place + moved.translation.trunc()) });
    }
}

/// The menus, as the field has them.
#[derive(Resource, Default)]
pub struct Menu {
    main: Option<MainMenu>,
    /// A screen that is up alone, as the title has OPTIONS.
    single: Option<Single>,
    /// What is to be asked of the sound.
    pub sounds: Vec<Request>,
    /// The menu's camera is drawing her this frame.
    her_shown: bool,
    /// The gallery, which has just been left, played something (its `0x1004`):
    /// the title has its tune begin again.
    pub tune_again: bool,
}

impl Menu {
    pub fn is_open(&self) -> bool {
        self.main.is_some() || self.single.is_some()
    }

    /// The title makes OPTIONS, alone (0x119F5C).
    pub fn open_options(&mut self, assets: &MenuAssets) {
        self.single = Some(Single::options(assets));
    }

    /// And the GALLERY (0x119F78).
    pub fn open_gallery(&mut self, assets: &MenuAssets, gallery: &Gallery, opened: &Opened) {
        self.single = Some(Single::gallery(assets, gallery, opened));
    }

    /// The film the gallery has asked for, while it has one: what plays films is
    /// told of it, and says here how far it has got.
    pub fn showing(&mut self) -> Option<&mut FilmAsked> {
        self.single.as_mut()?.gallery_screen()?.showing.as_mut()
    }

    /// The picture the gallery has asked for is read, and is up (0x3C028).
    pub fn read_picture(&mut self, data: &GameData, images: &mut Assets<Image>, gallery: &Gallery) {
        let Some(screen) = self.single.as_mut().and_then(Single::gallery_screen) else { return };
        let Some(archive) = screen.take_wanted() else { return };
        match gallery::read_picture(data, images, gallery, &archive) {
            Ok(picture) => screen.show(picture),
            Err(error) => error!("a picture of the gallery cannot be shown: {error:#}"),
        }
    }

    /// Triangle has gone down where she is free to be moved (0x10D9FC): the menu
    /// comes up, unless the mission is the one that has none.
    pub fn ask(&mut self, scenario: i32) {
        if scenario == NO_MENU_SCENARIO {
            self.sounds.push(Request::common(SOUND_REFUSED));
        } else {
            self.sounds.push(Request::common(SOUND_OPENS));
            self.main = Some(MainMenu::new());
        }
    }

    /// The screen between missions makes it (0x5D1D4): with no sound of its
    /// own, that screen having made one.
    pub fn open(&mut self) {
        self.main = Some(MainMenu::new());
    }

    /// Whether anything of it is drawn yet: until then what was on the
    /// screen still is.
    pub fn made(&self) -> bool {
        self.main.as_ref().is_some_and(MainMenu::made)
    }

    /// A frame of the game's logic with the menu up, and what of her its screens
    /// change. Answers whether the menu is done with, and the field to come back.
    pub fn step(&mut self, assets: &MenuAssets, pad: &Pad, hers: &mut Hers) -> bool {
        if let Some(single) = &mut self.single {
            let done = single.step(assets, pad, hers);
            self.sounds.append(&mut single.sounds);
            if done {
                self.tune_again = single.gallery_screen().is_some_and(|screen| screen.played);
                self.single = None;
            }
            return done;
        }
        let Some(main) = &mut self.main else { return false };
        for frame in 0..FRAMES_A_STEP {
            main.frame(assets, hers, pad, frame == 0);
        }
        self.sounds.append(&mut main.sounds);
        if main.state == State::Done {
            self.main = None;
            return true;
        }
        false
    }
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

/// What the menu shows of her: her figures as the game has them, and what her
/// level begins with and the next takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub level: i32,
    pub experience: i32,
    /// The experience her level began with and the next one takes, if there is one.
    pub level_from: i32,
    pub next: Option<i32>,
    pub hp: i32,
    pub most_hp: i32,
    pub attack: i32,
    pub defence: i32,
    pub special: i32,
}

impl Shown {
    pub fn of(stats: &PlayerStats, levels: &Levels) -> Shown {
        Shown {
            level: stats.level,
            experience: stats.experience as i32,
            level_from: levels.threshold(stats.level).unwrap_or(0) as i32,
            next: levels.next(stats.level).map(|next| next as i32),
            hp: stats.hp as i32,
            most_hp: stats.max_hp as i32,
            attack: stats.attack,
            defence: stats.defence,
            special: stats.special,
        }
    }
}

impl MainMenu {
    /// Draws a number with a track of the panel (0x60374): each digit is the
    /// track's group drawn from that digit's cell of the figures' sheet, over the
    /// group that goes under digits, and they stand a digit's width and one more
    /// apart: on from where the group is, or back from it when the number is set
    /// to the right.
    #[allow(clippy::too_many_arguments)]
    fn number(&self, canvas: &mut Canvas, assets: &MenuAssets, player: &Player, track: usize, under: usize, number: i32, to_the_right: bool, look: &Look) {
        draw_number(canvas, layer::MENU, &assets.panel, assets.figures, player, track, under, number, 1, to_the_right, look);
    }

    /// Draws the panel (0x5FE00): every track of its set by what its group is.
    fn draw_panel(&self, canvas: &mut Canvas, assets: &MenuAssets, shown: &Shown, ahead: f32) {
        let layout = &assets.panel;
        let player = Player::at(layout, PANEL_SET, self.panel_frames + ahead);
        let look = Look::tinted([255, 255, 255, self.alpha]);
        let mut seen = [0; ENTRIES];
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            if let Some(entry) = ENTRY_GROUPS.iter().position(|&of| of == group) {
                let first = seen[entry] == 0;
                seen[entry] += 1;
                if first != (self.entry == entry) {
                    layout.draw_track(canvas, layer::MENU, &player, track, &look);
                }
                continue;
            }
            if let Some(entry) = MARK_GROUPS.iter().position(|&of| of == group) {
                if self.entry == entry {
                    layout.draw_track(canvas, layer::MENU, &player, track, &look);
                }
                continue;
            }
            let number = |of: (usize, usize), number: i32, to_the_right: bool, canvas: &mut Canvas| self.number(canvas, assets, &player, track, of.1, number, to_the_right, &look);
            match group {
                group if UNDER_DIGITS.contains(&group) => {}
                group if group == ATTACK.0 => number(ATTACK, shown.attack, false, canvas),
                group if group == DEFENCE.0 => number(DEFENCE, shown.defence, false, canvas),
                group if group == SPECIAL.0 => number(SPECIAL, shown.special, false, canvas),
                group if group == EXPERIENCE.0 => number(EXPERIENCE, shown.experience, false, canvas),
                // What is wanting to the next level: nought when there is none.
                group if group == NEXT.0 => number(NEXT, shown.next.map_or(0, |next| (next - shown.experience).max(0)), false, canvas),
                group if group == LEVEL.0 => number(LEVEL, shown.level, false, canvas),
                group if group == MOST_HP.0 => number(MOST_HP, shown.most_hp, true, canvas),
                // The bar of her experience: whole when there is no next level,
                // and otherwise cut to as much of its width as she has come of
                // the way from where her level began to the next.
                BAR_GROUP => {
                    let cut = shown.next.and_then(|next| {
                        let bar = layout.objects.objects.get(layout.first_of(BAR_GROUP)?)?;
                        let mut width = bar.size[0] as i32;
                        if shown.experience < next && shown.level_from < next {
                            width = (width as f32 * ((shown.experience - shown.level_from) as f32 / (next - shown.level_from) as f32)) as i32;
                        }
                        let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32);
                        Some(Rect::from_corners(from, from + Vec2::new(width as f32, bar.size[1] as f32)))
                    });
                    canvas.cut_to(cut);
                    layout.draw_track(canvas, layer::MENU, &player, track, &look);
                    canvas.cut_to(None);
                }
                _ => layout.draw_track(canvas, layer::MENU, &player, track, &look),
            }
        }
    }

    /// Draws the strip along the foot (0x4AC08), which the screen that is up does
    /// last of all, with what it has the strip say: every track of its set as it
    /// is, but the one in whose place that is written.
    #[allow(clippy::too_many_arguments)]
    fn draw_strip(&self, canvas: &mut Canvas, assets: &MenuAssets, fonts: &Fonts, colours: &TextColours, layer: i32, words: &str, ahead: f32) {
        draw_strip(canvas, assets, fonts, colours, layer, words, self.ground_frames + ahead, self.alpha);
    }

    /// Draws the menu as it is `ahead` of a frame of the game's logic past its
    /// last one: the ground, the panel, then her as the menu's camera has drawn
    /// her if it has (`her`), her gauge and the strip; or, while an entry's screen
    /// has its turn, that; and over all of it the wipe and the screen's fade.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, canvas: &mut Canvas, assets: &MenuAssets, fonts: &Fonts, colours: &TextColours, shown: &Shown, options: &Options, seen: &Seen, her: bool, ahead: f32) {
        if !self.made() {
            return;
        }
        let of_a_step = ahead;
        let ahead = ahead * FRAMES_A_STEP as f32;
        // Nothing of the field is drawn under it.
        canvas.fill(layer::MENU_GROUND, seen.0, [0, 0, 0, 255]);
        if self.alpha != 0 {
            let ground = Player::at(&assets.ground, GROUND_SET, self.ground_frames + ahead);
            assets.ground.draw(canvas, layer::MENU_GROUND, &ground, &Look::tinted([255, 255, 255, self.alpha]));
        }
        if self.shows_panel() {
            self.draw_panel(canvas, assets, shown, ahead);
            if self.shows_gauge() {
                // Her model comes in whole, however much of the screen there is.
                if her {
                    canvas.scene_here(layer::MENU);
                }
                let gauge = assets.gauge.player(self.panel_frames + ahead);
                assets.gauge.draw_on(canvas, layer::MENU, &gauge, shown.hp, shown.most_hp, GAUGE_PLACE, self.alpha);
                if let Some(words) = assets.texts.get(self.entry) {
                    self.draw_strip(canvas, assets, fonts, colours, layer::MENU, words, ahead);
                }
            }
        }
        if let Some(screen) = &self.screen {
            match screen {
                Screen::Custom(custom) => custom.draw(canvas, &assets.custom, &assets.window, fonts, colours, her, of_a_step),
                Screen::Items(items) => items.draw(canvas, &assets.item, &assets.gauge, &assets.window, fonts, colours, (shown.hp, shown.most_hp), her, of_a_step),
                Screen::Library(library) => library.draw(canvas, &assets.library, fonts, colours, of_a_step),
                Screen::Options(screen) => screen.draw(canvas, &assets.option, &assets.window, fonts, colours, options, of_a_step),
            }
            // OPTIONS draws nothing in the frame it ends in, the strip neither.
            if !matches!(screen, Screen::Options(options) if !options.drawn()) {
                self.draw_strip(canvas, assets, fonts, colours, layer::MENU_SCREEN, screen.strip(assets), ahead);
            }
        }
        if self.wipe.playing {
            let wipe = Player::at(&assets.wipe, self.wipe.set, self.wipe.frames + ahead);
            assets.wipe.draw(canvas, layer::MENU_WIPE, &wipe, &Look::default());
        }
        let dark = (self.fade.level.clamp(0.0, 1.0) * 255.0) as u8;
        if dark != 0 {
            canvas.fill(layer::FADE, seen.0, [0, 0, 0, dark]);
        }
    }
}

/// Draws the menu over the field.
#[allow(clippy::too_many_arguments)]
fn draw(
    menu: Res<Menu>,
    assets: Res<MenuAssets>,
    (fonts, colours): (Res<Fonts>, Res<TextColours>),
    (stats, levels, options): (Res<PlayerStats>, Res<Levels>, Res<Options>),
    seen: Res<Seen>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    if let Some(main) = &menu.main {
        main.draw(&mut canvas, &assets, &fonts, &colours, &Shown::of(&stats, &levels), &options, &seen, menu.her_shown, steps.overstep_fraction());
    }
    if let Some(single) = &menu.single {
        single.draw(&mut canvas, &assets, &fonts, &colours, &options, &seen, steps.overstep_fraction());
    }
}

/// Her model as the menus show it, with which screen's pose and view it has
/// been given (none as it is made); and the camera that sees it.
#[derive(Component)]
struct Her(Option<Pose>);
#[derive(Component)]
struct HerCamera;

/// The screens that have a pose and a view of their own for her model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pose {
    Panel,
    Custom,
    Items,
}

/// What the menus show of her model in a frame: by which screen's view, how far
/// L and R have turned her from it, whether she is drawn at all, whether she
/// stands still, and the suit and the weapon she has on if they are not hers.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Showing {
    pose: Pose,
    turn: f32,
    shown: bool,
    still: bool,
    wears: Option<(i32, i32)>,
}

impl MainMenu {
    fn showing(&self) -> Option<Showing> {
        if !self.made() {
            return None;
        }
        Some(match &self.screen {
            // A screen draws her whenever it is drawn (0x4C224, 0x5523C).
            Some(Screen::Custom(custom)) => Showing { pose: Pose::Custom, turn: 0.0, shown: true, still: custom.still(), wears: None },
            Some(Screen::Items(items)) => Showing { pose: Pose::Items, turn: 0.0, shown: true, still: items.still(), wears: Some(items.tried_on()) },
            // CHALLENGES and OPTIONS have nothing of her (0x57E34, 0x5BC98).
            Some(Screen::Library(_) | Screen::Options(_)) => Showing { pose: Pose::Panel, turn: self.turn, shown: false, still: true, wears: None },
            None => Showing { pose: Pose::Panel, turn: self.turn, shown: self.shows_panel() && self.shows_gauge(), still: false, wears: None },
        })
    }
}

/// What her model in the menus was made of: the suit and the weapon it has on,
/// by their items; and the models that have been read for it, by their files.
#[derive(Default)]
struct Worn {
    wears: Option<(i32, i32)>,
    models: HashMap<String, Option<(insm::Model, Vec<u8>)>>,
}

impl Worn {
    /// The model of a file, read the first time it is asked for.
    fn model(&mut self, data: &GameData, file: Option<String>) -> Option<&(insm::Model, Vec<u8>)> {
        let file = file?;
        let read = |file: &String| data.model(file).inspect_err(|error| warn!("{file} is not shown in the menus: {error:#}")).ok();
        self.models.entry(file).or_insert_with_key(read).as_ref()
    }
}

/// How her model is turned for a view (0x3B680): about z, then y, then x.
fn turned(view: &View, turn: f32) -> Quat {
    let turns = view.turns + Vec3::Y * turn;
    Quat::from_euler(EulerRot::ZYX, turns.z.to_radians(), turns.y.to_radians(), turns.x.to_radians())
}

/// Has her model stand in the menus for as long as they are made (0x5FAA4,
/// 0x5F850), in the suit and with the weapon she has on (0x6075C) or that a
/// screen has her try on, turned as the screen that is up and L and R have
/// her, with the animations that screen gives her and her weapon on the node
/// it hangs on; and the menus' camera draw her as that screen sees her: the
/// panel once it has been coming long enough (0x60574), a screen always. The
/// camera draws to the scenes' picture, and to have that clear of all but her
/// the field's cameras are put out for as long as it does: a camera that is not
/// the first to draw to a picture draws over what is there.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show_her(
    mut commands: Commands,
    mut menu: ResMut<Menu>,
    assets: Res<MenuAssets>,
    (data, source, stats): (Res<GameData>, Res<FieldSource>, Res<PlayerStats>),
    mut stores: AssetStores,
    made: Query<Entity, Or<(With<Her>, With<HerCamera>)>>,
    mut her: Query<(&mut Her, &mut Transform, &mut Animator), Without<HerCamera>>,
    mut camera: Query<(&mut Camera, &mut Transform, &mut Projection), (With<HerCamera>, Without<Her>)>,
    mut others: Query<(Entity, &mut Camera), (With<Camera3d>, Without<HerCamera>)>,
    mut put_out: Local<Vec<Entity>>,
    mut worn: Local<Worn>,
) {
    let wanted = menu.main.as_ref().and_then(MainMenu::showing);
    // With something else on than she was made with, she is made anew.
    let wears = wanted.map(|showing| showing.wears.unwrap_or((stats.suit, stats.weapon)));
    let changed = !made.is_empty() && wears.is_some() && worn.wears != wears;
    let shown = wanted.is_some_and(|showing| showing.shown) && !made.is_empty() && !changed;
    // The scenes' other cameras are put out while hers draws, and lit again after.
    if shown && put_out.is_empty() {
        for (entity, mut other) in &mut others {
            if other.is_active {
                other.is_active = false;
                put_out.push(entity);
            }
        }
    } else if !shown {
        for entity in put_out.drain(..) {
            if let Ok((_, mut other)) = others.get_mut(entity) {
                other.is_active = true;
            }
        }
    }
    let (Some(showing), Some(wears), false) = (wanted, wears, changed) else {
        for entity in &made {
            commands.entity(entity).despawn();
        }
        if menu.her_shown {
            menu.her_shown = false;
        }
        return;
    };
    if made.is_empty() {
        // The model and the camera are made with no view yet: the frame after
        // gives them the one of the screen that is up. The suit is a character
        // of the field (0x14BE78), whose bundle has her model in it.
        let suit = source.character_bundle(player::suit_character(wears.0)).map(|bundle| player::model_of(&bundle));
        let built = {
            let (model, bytes) = worn.model(&data, suit).unwrap_or(&assets.her);
            ModelAsset::build(model, bytes, &mut stores)
        };
        let (root, [hand]) = built.spawn_with_nodes(&mut commands, Transform::from_translation(HER_PLACE), [WEAPON_NODE]);
        commands.entity(root).insert((Her(None), Animator::with_slots(2), ShownParts(HER_PARTS.to_vec())));
        if let Some(hand) = hand {
            let built = {
                let (model, bytes) = worn.model(&data, player::weapon_model(wears.1)).unwrap_or(&assets.cannon);
                ModelAsset::build(model, bytes, &mut stores)
            };
            let cannon = built.spawn(&mut commands, Transform::IDENTITY);
            commands.entity(cannon).insert(ChildOf(hand));
        }
        worn.wears = Some(wears);
        commands.spawn((
            HerCamera,
            Camera3d::default(),
            Camera { order: HER_CAMERA_ORDER, clear_color: ClearColorConfig::Custom(Color::NONE), is_active: false, ..default() },
            brs_engine::texture::camera_of_values(),
            Projection::Perspective(PerspectiveProjection::default()),
            Transform::from_translation(HER_PLACE),
        ));
        return;
    }
    let view = match showing.pose {
        Pose::Panel => PANEL_VIEW,
        Pose::Custom => custom::VIEW,
        Pose::Items => item::VIEW,
    };
    let mut anew = false;
    for (mut her, mut transform, mut animator) in &mut her {
        if her.0 != Some(showing.pose) {
            her.0 = Some(showing.pose);
            anew = true;
            // A screen has her in a pose: its animation is put at its end and
            // does not go round (0x4BD30, 0x4BD3C; 0x542D0, 0x542F8).
            let pose = match showing.pose {
                Pose::Panel => None,
                Pose::Custom => Some(&assets.custom.pose),
                Pose::Items => Some(&assets.item.pose),
            };
            match pose {
                Some(pose) => {
                    animator.set(pose);
                    let end = animator.body().length();
                    animator.body_mut().set_time(end);
                }
                None => {
                    animator.set(&assets.pose);
                    animator.body_mut().loop_clip();
                }
            }
            animator.set_slot(1, &assets.pose_over);
            animator.slot_mut(1).loop_clip();
        }
        transform.rotation = turned(&view, showing.turn);
        if animator.paused != showing.still {
            animator.paused = showing.still;
        }
    }
    for (mut camera, mut transform, mut projection) in &mut camera {
        if anew {
            let screen = UVec2::new(brs_engine::canvas::WIDTH as u32, brs_engine::canvas::HEIGHT as u32);
            camera.sub_camera_view = Some(SubCameraView { full_size: screen, offset: -view.picture, size: screen });
            *projection = Projection::Perspective(PerspectiveProjection { fov: view.lens.to_radians(), near: view.near, far: view.far, ..default() });
            *transform = Transform::from_translation(HER_PLACE + view.eye).looking_at(HER_PLACE + view.looked_at, Vec3::Y);
        }
        if camera.is_active != shown {
            camera.is_active = shown;
        }
    }
    if menu.her_shown != shown {
        menu.her_shown = shown;
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Menu>().add_systems(Update, (show_her, draw).chain().run_if(in_state(crate::Mode::Field)));
}

/// What the menus' tests share: the game's own data for them, and a her for them
/// to change.
#[cfg(test)]
mod testing {
    use super::*;

    pub struct Game {
        pub assets: MenuAssets,
        pub fonts: Fonts,
        pub colours: TextColours,
        pub skills: Skills,
        pub items: Items,
        pub records: Records,
        /// The stage the map she is on is of.
        pub stage: i32,
        pub progress: Progress,
        pub stats: PlayerStats,
        pub inventory: Inventory,
        pub options: Options,
        pub gallery: Gallery,
        pub opened: Opened,
    }

    impl Game {
        pub fn hers(&mut self) -> Hers<'_> {
            Hers { skills: &self.skills, items: &self.items, records: &self.records, stage: self.stage, progress: &mut self.progress, stats: &mut self.stats, inventory: &mut self.inventory, options: &mut self.options, gallery: &self.gallery, opened: &self.opened, changed: false }
        }

        /// A frame of the game's logic with the menu up.
        pub fn step(&mut self, menu: &mut Menu, pad: &Pad) -> bool {
            let mut hers = Hers { skills: &self.skills, items: &self.items, records: &self.records, stage: self.stage, progress: &mut self.progress, stats: &mut self.stats, inventory: &mut self.inventory, options: &mut self.options, gallery: &self.gallery, opened: &self.opened, changed: false };
            menu.step(&self.assets, pad, &mut hers)
        }
    }

    pub fn game() -> Option<Game> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let items = Items::load(&data).unwrap();
        Some(Game {
            assets: MenuAssets::load(&data, &mut images).unwrap(),
            fonts: Fonts::load(&data, &mut images).unwrap(),
            colours: TextColours::load(&data).unwrap(),
            skills: Skills::load(&data).unwrap(),
            inventory: Inventory::new_game(&items),
            items,
            records: Records::load(&data).unwrap(),
            stage: 0,
            options: Options::load(&data).unwrap(),
            gallery: Gallery::load(&data).unwrap(),
            opened: Opened::default(),
            progress: Progress::start(),
            stats: PlayerStats { level: 3, experience: 150, max_hp: 496.0, hp: 300.0, attack: 52, special: 47, defence: 31, burst_bullets: 6, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 },
        })
    }

    pub fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    /// The sounds of the bank that is always there among what was asked for.
    pub fn common(sounds: &[Request]) -> Vec<u32> {
        let sound = |request: &Request| match request {
            Request::Play { sound, .. } => Some(*sound),
            _ => None,
        };
        sounds.iter().filter_map(sound).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{common, game, press, Game};
    use super::*;
    use brs_engine::canvas::Vertex;

    const HER: Shown = Shown { level: 3, experience: 150, level_from: 119, next: Some(243), hp: 300, most_hp: 496, attack: 52, defence: 31, special: 47 };

    /// Brings the menu up and has it come: answers the frames of the game's logic
    /// that took.
    fn come(menu: &mut Menu, game: &mut Game) -> usize {
        menu.ask(1);
        let mut steps = 0;
        while !menu.main.as_ref().unwrap().ready {
            assert!(!game.step(menu, &Pad::default()));
            steps += 1;
            assert!(steps < 200);
        }
        steps
    }

    /// The corners of what was drawn, three to a triangle.
    fn drawn(menu: &Menu, game: &Game) -> Vec<Vertex> {
        let mut canvas = Canvas::default();
        if let Some(main) = &menu.main {
            main.draw(&mut canvas, &game.assets, &game.fonts, &game.colours, &HER, &game.options, &Seen::default(), false, 0.0);
        }
        canvas.vertices().to_vec()
    }

    /// Where the things drawn in a colour have their top left corners, and the
    /// left edge of what each is drawn from. (A thing is two triangles, and a
    /// thing that is cut can be any number of them.)
    fn of_colour(vertices: &[Vertex], colour: [u8; 4]) -> Vec<(Vec2, f32)> {
        let triangles = vertices.chunks(3).filter(|triangle| triangle.iter().all(|vertex| vertex.colour == colour));
        let mut things: Vec<(Vec2, f32)> = triangles.map(|triangle| triangle.iter().fold((triangle[0].position, triangle[0].uv.x), |(at, across), vertex| (at.min(vertex.position), across.min(vertex.uv.x)))).collect();
        things.dedup();
        things
    }

    #[test]
    fn it_comes_under_its_wipe_and_can_be_used_once_its_panel_is_ready() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!((game.assets.wipe.length(WIPE_COMES), game.assets.wipe.length(WIPE_GOES), game.assets.ready_at()), (36.0, 36.0, 52.0));
        assert_eq!(game.assets.texts, ["Check and set skills.", "Check and use items.", "Check all known challenges.", "Change in-game settings."]);

        // Triangle is heard, and for a frame of its own nothing of the menu is made.
        let mut menu = Menu::default();
        menu.ask(1);
        assert!(menu.is_open() && drawn(&menu, &game).is_empty());
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_OPENS]);

        // Then the field is gone, the wipe comes, and nothing else is there yet.
        let main = |menu: &Menu| menu.main.clone().unwrap();
        assert!(!game.step(&mut menu, &Pad::default()));
        assert_eq!((main(&menu).state, main(&menu).alpha, main(&menu).wipe), (State::Coming, 0, Wipe { set: WIPE_COMES, frames: 1.0, playing: true }));
        let black = drawn(&menu, &game);
        assert!(black[..6].iter().all(|vertex| vertex.colour == [0, 0, 0, 255]));

        // The screen comes from its twelfth frame to its twenty-seventh.
        let mut steps = 1;
        let mut to = |game: &mut Game, menu: &mut Menu, step: usize| {
            while steps < step {
                assert!(!game.step(menu, &Pad::default()));
                steps += 1;
            }
        };
        to(&mut game, &mut menu, 7);
        assert_eq!(main(&menu).alpha, 0);
        to(&mut game, &mut menu, 8);
        assert_eq!(main(&menu).alpha, 18);
        to(&mut game, &mut menu, 11);
        // Her gauge and the strip are not drawn until the panel has been coming
        // for twenty-two frames.
        assert!(!main(&menu).shows_gauge());
        to(&mut game, &mut menu, 12);
        assert!(main(&menu).shows_gauge());
        to(&mut game, &mut menu, 15);
        assert!(main(&menu).alpha == 255 && !main(&menu).coming);

        // With the wipe through, it is open; three frames on its sound is played;
        // and it takes the pad when the panel's motion is over.
        to(&mut game, &mut menu, 18);
        assert!(main(&menu).state == State::Coming && main(&menu).wipe.playing);
        to(&mut game, &mut menu, 19);
        assert!(main(&menu).state == State::Open && !main(&menu).wipe.playing);
        to(&mut game, &mut menu, 20);
        assert!(menu.sounds.is_empty());
        to(&mut game, &mut menu, 21);
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_COME]);
        to(&mut game, &mut menu, 26);
        assert!(!main(&menu).ready);
        // Until then the buttons do nothing.
        game.step(&mut menu, &press(button::CIRCLE | button::DOWN));
        assert!(main(&menu).ready && main(&menu).entry == 0 && !main(&menu).leaving_asked && menu.sounds.is_empty());
    }

    #[test]
    fn the_cursor_goes_round_its_four_entries_only_on_a_new_press() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = Menu::default();
        assert_eq!(come(&mut menu, &mut game), 27);
        menu.sounds.clear();
        let entry = |menu: &Menu| menu.main.as_ref().unwrap().entry;
        let held = |buttons: u32, again: u32| Pad { held: buttons, repeat: again, ..Pad::default() };
        for expected in [1, 2, 3] {
            game.step(&mut menu, &press(button::DOWN));
            assert_eq!(entry(&menu), expected);
        }
        // Held, down comes again but does not go round; a new press does.
        game.step(&mut menu, &held(button::DOWN, button::DOWN));
        assert_eq!(entry(&menu), 3);
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!(entry(&menu), 0);
        game.step(&mut menu, &held(button::UP, button::UP));
        assert_eq!(entry(&menu), 0);
        game.step(&mut menu, &press(button::UP));
        assert_eq!(entry(&menu), 3);
        game.step(&mut menu, &held(button::UP, button::UP));
        assert_eq!(entry(&menu), 2);
        // Each move is heard; the tries that moved nothing are not.
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 6]);
        // L and R turn her, a frame's worth for each of the menu's frames.
        game.step(&mut menu, &held(button::R, 0));
        game.step(&mut menu, &held(button::R, 0));
        game.step(&mut menu, &held(button::L, 0));
        assert_eq!(menu.main.as_ref().unwrap().turn, FRAMES_A_STEP as f32);
    }

    #[test]
    fn circle_or_triangle_leaves_it_as_the_screen_goes_dark() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        for button in [button::CIRCLE, button::TRIANGLE] {
            let mut menu = Menu::default();
            come(&mut menu, &mut game);
            menu.sounds.clear();
            assert!(!game.step(&mut menu, &press(button)));
            assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_LEFT]);
            let main = menu.main.clone().unwrap();
            assert_eq!((main.state, main.wipe.set, main.wipe.playing), (State::Leaving, WIPE_GOES, true));
            // Nothing is taken from the pad any more, and fifteen of its frames
            // on, which is in the seventh frame of the game's logic after the
            // button's, the screen is dark and the menu done with.
            let mut steps = 1;
            while !game.step(&mut menu, &press(button::DOWN)) {
                let main = menu.main.as_ref().unwrap();
                assert!(main.entry == 0 && main.fade.level > 0.0 && main.fade.level < 1.0);
                steps += 1;
                assert!(steps < 100);
            }
            assert_eq!(steps, 7);
            assert!(!menu.is_open() && menu.sounds.is_empty());
        }
        // In the one mission that has no menu, triangle is answered with "no".
        let mut menu = Menu::default();
        menu.ask(NO_MENU_SCENARIO);
        assert!(!menu.is_open());
        assert_eq!(common(&menu.sounds), [SOUND_REFUSED]);
    }

    #[test]
    fn the_panel_shows_her_figures_and_the_entry_the_cursor_is_on() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = Menu::default();
        come(&mut menu, &mut game);
        let cell = |digit: f32| digit * FIGURE_CELL / 256.0;
        let all = drawn(&menu, &game);
        // Her attack, 52, in the figures' pale blue: two digits eleven apart from
        // where their group is, each from its own cell of the sheet, and under
        // each the faint square that goes under a digit.
        assert_eq!(of_colour(&all, [121, 197, 249, 128])[..2], [(Vec2::new(156.0, 178.0), cell(5.0)), (Vec2::new(167.0, 178.0), cell(2.0))]);
        let under: Vec<Vec2> = of_colour(&all, [255, 255, 255, 26]).iter().map(|(at, _)| *at).collect();
        assert_eq!(under[..2], [Vec2::new(156.0, 178.0), Vec2::new(167.0, 178.0)]);
        // Her defence and special attack below it.
        assert_eq!(of_colour(&all, [121, 197, 249, 128])[2..], [(Vec2::new(156.0, 204.0), cell(3.0)), (Vec2::new(167.0, 204.0), cell(1.0)), (Vec2::new(156.0, 230.0), cell(4.0)), (Vec2::new(167.0, 230.0), cell(7.0))]);
        // Her experience, what is wanting to the next level, and all her HP, which
        // is set to the right: its last digit is where its group is.
        let yellow = of_colour(&all, [210, 219, 147, 230]);
        let digits = |row: f32| -> Vec<(f32, f32)> { yellow.iter().filter(|(at, _)| at.y == row).map(|(at, across)| (at.x, *across)).collect() };
        assert_eq!(digits(81.0), [(156.0, cell(1.0)), (167.0, cell(5.0)), (178.0, cell(0.0))]);
        assert_eq!(digits(107.0), [(156.0, cell(9.0)), (167.0, cell(3.0))]);

        assert_eq!(digits(145.0), [(442.0, cell(4.0)), (453.0, cell(9.0)), (464.0, cell(6.0))]);
        // Her level, twice the size.
        assert!(of_colour(&all, [20, 20, 20, 255]).contains(&(Vec2::new(167.0, 42.0), cell(3.0))));
        // The bar under them, which is drawn over a faint one of all its length,
        // is cut to a quarter of its sixty-five: she has 31 of the 124 her level
        // takes.
        let on_the_bar = |triangle: &&[Vertex]| triangle.iter().all(|vertex| (119.0..=121.0).contains(&vertex.position.y) && vertex.position.x >= 156.0);
        let bar: Vec<Vertex> = all.chunks(3).filter(on_the_bar).filter(|triangle| triangle[0].colour[3] == 255).flatten().copied().collect();

        let (left, right) = bar.iter().fold((f32::MAX, f32::MIN), |(left, right), vertex| (left.min(vertex.position.x), right.max(vertex.position.x)));
        assert_eq!((left, right), (156.0, 172.0));

        // The entry the cursor is on is blue, twelve to the right of where it
        // stands otherwise, with its mark under it; the others are grey.
        let blue = |menu: &Menu, game: &Game| -> Vec<Vec2> { of_colour(&drawn(menu, game), [0, 100, 255, 255]).iter().map(|(at, _)| *at).collect() };
        let grey = |menu: &Menu, game: &Game| -> Vec<Vec2> { of_colour(&drawn(menu, game), [200, 200, 200, 255]).iter().map(|(at, _)| *at).collect() };
        // The mark is a line that comes to the same blue along its length: the
        // rows such lines are drawn in.
        let marks = |menu: &Menu, game: &Game| -> Vec<f32> {
            let all = drawn(menu, game);
            let blue = |vertex: &Vertex| vertex.colour == [0, 100, 255, 255];
            let lines = all.chunks(3).filter(|triangle| triangle.iter().any(blue) && !triangle.iter().all(blue));
            let mut rows: Vec<f32> = lines.map(|triangle| triangle.iter().map(|vertex| vertex.position.y).fold(f32::MAX, f32::min)).collect();
            rows.dedup();
            rows
        };
        assert_eq!(blue(&menu, &game), [Vec2::new(39.0, 111.0), Vec2::new(112.0, 111.0)]);

        assert_eq!(marks(&menu, &game), [118.0]);
        assert!(!grey(&menu, &game).contains(&Vec2::new(27.0, 111.0)) && grey(&menu, &game).contains(&Vec2::new(76.0, 130.0)));
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!(blue(&menu, &game), [Vec2::new(88.0, 130.0)]);
        assert_eq!(marks(&menu, &game), [137.0]);
        assert!(grey(&menu, &game).contains(&Vec2::new(27.0, 111.0)) && !grey(&menu, &game).contains(&Vec2::new(76.0, 130.0)));
    }
}
