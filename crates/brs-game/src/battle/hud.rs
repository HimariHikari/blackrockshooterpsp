//! The battle's display, drawn from the game's own layouts as the original draws it
//! (0x406B0, 0x3EE60): her HP gauge at the foot of the screen on the right, the
//! gauge of her heat under it, at the foot on the left how many enemies are left,
//! over each enemy its mark, which is its level and a bar of its HP, the cursor she
//! aims with, or the mark on what she has locked on to, and at the top on the right
//! the column of her skills.
//!
//! The original keeps such things as entries of an object that plays a layout
//! (0x3C988 to 0x3EA58): a battle has one of the battle's own layout (0x3CCA4, the
//! first files of `if_btlsys.lpk`) with an entry for the heat (kind 9, playing set 7)
//! and one for the count (kind 12, set 8), and her HP is an entry of the one the game
//! makes as it starts (see `field/hud/gauge.rs`). Every enemy has an object of its
//! own of the signs' layout (0x3550C, `if_rsdsys.lpk/018.lpk`) with an entry for its
//! mark (kind 10, set 4). She has two of them, each with an entry that plays the
//! cursor's set (kind 11, set 1): the cursor, and the mark on what she has locked
//! on to, which is the cursor with one flag more (0x8D4F8). A third thing the
//! cursor's record can have, a sight that two of her skills aim with (0x41E18), is
//! not ported, as those two are not. The column of her skills is a task of its own
//! (`BtSkillPallet`, 0x9F068) with a record and an entry of its own (kind 15, sets 4
//! to 6 of the battle's layout); its rows have the names of the skills in her
//! slots (`skills.rs`), or the dashes of a slot with none. The line of information
//! is the battle's log (`log.rs`), drawn here in its turn.
//!
//! A status effect that has an icon has a mark put up for it (an entry of kind 13,
//! 0x3DEE4): hers on the battle's layout, drawn in the turn of one of the heat
//! gauge's groups, an enemy's on that enemy's own signs' layout, drawn in the turn
//! of one of its mark's groups (0x3D2D0). `status.rs` has which are up and in what
//! order; they are drawn here.
//!
//! All of it is drawn while the battle's display is on (the root's flag 11, 0x8D0C4),
//! which it is from the battle's start until her last state of a won battle is over
//! (0x9DBC4). A staged moment of hers leaves it on: only the cursor is taken away
//! (0x74650), the mark on what she had locked on to is not drawn (0x8D23C), and the
//! enemies' marks go with the enemies.

use anyhow::Result;
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{modulate, Layout, Look, Player};

use super::actor::PcActor;
use super::aim::{self, Aim};
use super::enemy::{Enemy, Horde, MarkAt};
use super::flow::Flow;
use super::log::{self, Log, Messages};
use super::numbers::Numbers;
use super::result::{ResultLayout, Results};
use super::scope::{self, Scope};
use super::skills::{Skills, Slots};
use super::tunables::{self, Tunables};
use super::{BattleCamera, BattleSource, SIXTIETHS_PER_STEP};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::gauge::Gauge;
use crate::interface::{self, layer};
use crate::sound::{Board, COMMON_BANK, FULL_VOLUME};
use crate::text::{self, TextColours};
use crate::writing::{Block, Fonts, Placing, Style};
use crate::Mode;

/// The pack the battle's own layouts are in (0x2CCF4).
const BATTLE_PACK: &str = "SYSTEM.VOL/if_btlsys.lpk";
/// The pack inside the one that is always there whose last texture has the
/// count's figures (0x2CE90): the signs' pack.
const SIGNS_PACK: &str = interface::SIGNS_PACK;

// The sets of the layout the two entries play (0x3DC38, 0x3E734).
const HEAT_SET: usize = 7;
const COUNT_SET: usize = 8;

// The groups of the heat's set that are more than something to draw (0x40C90): the
// bar, cut to her heat, and the bar of a gun that has overheated; her heat in
// figures; and what is only there while it has overheated, or only while not.
const HEAT_BAR: usize = 15;
const HEAT_BAR_OVERHEATED: usize = 16;
const HEAT_NUMBER: usize = 17;
const HEAT_WHILE_COOL: usize = 18;
const HEAT_WHILE_OVERHEATED: [usize; 2] = [19, 22];
/// Where the marks of the status effects on her go (0x3D2D0): the group has an
/// object for each icon there is, all in the one place.
const HEAT_MARKS: usize = 26;
/// The set a mark of hers plays (0x3DF30), which is that group and nothing more,
/// standing still; and how far apart the marks stand (0x3D440).
const HER_EFFECT_SET: usize = 10;
const EFFECT_PITCH: f32 = 16.0;
/// How far apart the figures of her heat stand, and the cells of their sheet
/// (0x41094, the first of the table at 0x171078).
const HEAT_FIGURE_PITCH: f32 = 6.0;
const HEAT_FIGURE_CELL: f32 = 20.0;
// The colours the bar is drawn by (0x40D68 on): up to the first of two shares of
// the most heat there is, up to the second, and beyond.
const HEAT_LOW: [u8; 4] = [0x8c, 0x8c, 0xff, 0xff];
const HEAT_MIDDLING: [u8; 4] = [0xff, 0xe6, 0x82, 0xff];
const HEAT_HIGH: [u8; 4] = [0xff, 0x78, 0x50, 0xff];

// The groups of the count's set (0x41130): the figures, what stands in for them
// when there is no count, and the rest, which makes room for the figures.
const COUNT_NUMBER: usize = 23;
const COUNT_NONE: usize = 24;
/// The cells of the sheet the count's figures are in, which is also how far apart
/// they stand but for a pixel (the second of the table at 0x171078).
const COUNT_FIGURE_CELL: f32 = 11.0;

/// The file of the signs' pack that has the figures of what a guarded hit takes
/// off her (0x3F9FC).
const GUARDED_FIGURES: usize = 14;

/// The set of the signs' layout an enemy's mark plays (0x3DD4C).
const MARK_SET: usize = 4;
// Its groups that are more than something to draw (0x3FEE8): the bar, cut to the
// share of its HP the enemy has; what says it has a second life to come; its level,
// in figures; and one in whose turn the marks of the status effects on the enemy
// are drawn (0x402D8), which is itself never drawn.
const MARK_BAR: usize = 35;
const MARK_SECOND_LIFE: usize = 37;
const MARK_LEVEL: usize = 39;
const MARK_EFFECTS: usize = 40;
/// The set a mark of an enemy's status effect plays (0x3DF50): one group with an
/// object for each icon there is, standing still.
const ENEMY_EFFECT_SET: usize = 5;
/// The cells of the sheet the level's figures are in, which is how far apart they
/// stand (the ninth of the table at 0x171078), and which of the layout's textures
/// the sheet is (0x3FEA4).
const LEVEL_FIGURE_CELL: f32 = 6.0;
const LEVEL_TEXTURE: usize = 19;

// The sets of the battle's layout the column of her skills plays (0x3E8AC, 0x3E910,
// 0x3EA58): coming in from the right, where all but its buttons is off the screen;
// going back there; and the flash of a row whose skill has come to be ready.
const PALETTE_IN: usize = 4;
const PALETTE_OUT: usize = 5;
const PALETTE_FLASHES: usize = 6;
/// Its rows, one to a button, and how far apart they are (0x41520).
const PALETTE_ROWS: usize = 4;
const PALETTE_PITCH: f32 = 21.0;
// Its groups that are more than something to draw (0x41318): the bar of how far a
// skill is to being ready, drawn for each row and cut to that; the four buttons,
// each dim until its row's skill is ready; the skills' names, which are written
// where the group's object is; what is drawn for a row that is not ready, and for
// one that is; and the flashes.
const PALETTE_BAR: usize = 5;
const PALETTE_BUTTONS: usize = 7;
const PALETTE_NAMES: usize = 11;
const PALETTE_NOT_READY: usize = 12;
const PALETTE_READY: usize = 13;
const PALETTE_FLASH: usize = 20;
// The colours a row's name is written in (0x41794): its own once the skill is
// ready, half as strong until then; and how it is written (0x3E828): twenty glyphs
// with nothing between them.
const PALETTE_NAME_READY: [u8; 4] = [0x2e, 0xcf, 0xff, 0xff];
const PALETTE_NAME_WAITING: [u8; 4] = [0xff, 0xff, 0xff, 0x80];
const PALETTE_NAME_STYLE: Style = Style::of_kind(0, 0x14, 1, 0.0, 4.0);
/// What a row with no skill has for a name (the text at 0x1669FC): seven dashes.
const PALETTE_NO_SKILL: &str = "\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}\u{2015}";
// The colours a row's bar is drawn by (0x414B0), and a button that is not ready
// (0x41B24).
const PALETTE_FILLING: [u8; 4] = [0x70, 0xa7, 0xf0, 0x80];
const PALETTE_FULL: [u8; 4] = [0x00, 0x71, 0xff, 0x80];
const PALETTE_DIM: [u8; 4] = [0xff, 0xff, 0xff, 0x80];
/// The sound of the column coming in (0x9F41C), of the battle's bank.
const SOUND_PALETTE: u32 = 0x64;

/// The set of the signs' layout the cursor plays (0x3E388).
const CURSOR_SET: usize = 1;

/// The sight of a snipe (0x41E18): a layout of its own, the battle pack's files 7
/// to 9, with the pack's file 10 for the one texture it has (its first is none).
const SIGHT_FILE: usize = 7;
const SIGHT_TEXTURE: usize = 10;
/// Its groups that are not simply drawn (0x421E8): the lens, which is made of
/// corners where the group's track has come to; what is drawn only until the
/// sight has locked on; and what is drawn only once it has.
const SIGHT_LENS: usize = 0;
const SIGHT_UNLOCKED: usize = 9;
const SIGHT_LOCKED: [usize; 2] = [10, 11];
/// The lens (0x423AC): about the middle of the screen, a clear round of this
/// radius times the track's scale along x, tinted, and all the screen beyond it in
/// the track's colour. It has 64 corners round.
const LENS_RADIUS: f32 = 100.0;
const LENS_REACH: f32 = 300.0;
const LENS_CORNERS: usize = 64;
/// The tint of the clear round: its alpha is of the track's.
const LENS_TINT: [u8; 4] = [0x05, 0x96, 0xf5, 0x3c];
/// The first of the four groups each state of the cursor is drawn from, and the two
/// of a gun that has overheated (0x40338).
const CURSOR_GROUPS: [usize; 3] = [2, 6, 10];
const CURSOR_OVERHEATED: [usize; 2] = [14, 15];
/// The shares of the most heat there is up to which the cursor is in its first
/// state, and in its second (0x97770).
const CURSOR_HEAT: [f32; 2] = [0.33, 0.66];

/// The state her heat puts the cursor in (0x97770).
fn cursor_state(share: f32, overheated: bool) -> usize {
    match share {
        _ if overheated => 3,
        share if share <= CURSOR_HEAT[0] => 0,
        share if share <= CURSOR_HEAT[1] => 1,
        _ => 2,
    }
}

/// Whether a group of the cursor's set is drawn (0x40338), by the state the cursor
/// is in and by whether it is the mark on what she has locked on to. Of a state's
/// four groups the first is of both, the second of the cursor alone and the others
/// of the mark alone; an overheated gun's two are of both.
fn cursor_shows(group: usize, state: usize, mark: bool) -> bool {
    match CURSOR_GROUPS.get(state) {
        Some(&first) => group == first || (group == first + 1 && !mark) || (mark && (group == first + 2 || group == first + 3)),
        None => CURSOR_OVERHEATED.contains(&group),
    }
}

/// What the battle's display is drawn from.
#[derive(Resource)]
pub struct Hud {
    gauge: Gauge,
    status: Layout,
    signs: Layout,
    /// The sight of a snipe.
    sight: Layout,
    /// What a mark of a status effect plays, hers and an enemy's: each mark is an
    /// entry of its own (kind 13, 0x3DEE4), but all of them stand still alike.
    her_effects: Player,
    enemy_effects: Player,
    /// The width of the texture the figures of an enemy's level are in.
    level_sheet: f32,
    /// The widths of the textures the heat's and the count's figures are in.
    /// The count's are also those of the numbers a battle puts up, but for what
    /// a guarded hit takes off her, which has smaller ones of its own (0x3F9FC).
    heat_sheet: f32,
    count_sheet: f32,
    guarded_sheet: f32,
    /// The shares of the most heat there is at which the bar changes colour
    /// (0xD2C64: the battle system's values 81 and 82, in hundredths).
    heat_shares: [f32; 2],
    /// The lines the battle's log can say.
    messages: Messages,
    /// What the results of a battle won are drawn from.
    results: ResultLayout,
    /// What the window of her items is drawn from.
    pub items: super::item_menu::ItemLayout,
}

impl Hud {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<Hud> {
        // The layout's seven textures (0x3CD58 on): three of its own pack, none,
        // and three of the pack that is always there.
        let own = |file: usize| interface::file(BATTLE_PACK, file, "ptm");
        let (heat_figures, count_figures) = (interface::file(interface::RESIDENT_PACK, 9, "ptm"), interface::file(SIGNS_PACK, 8, "ptm"));
        let textures = [own(3), own(4), own(5), own(3), interface::file(interface::RESIDENT_PACK, 20, "ptm"), heat_figures.clone(), count_figures.clone()];
        let status = interface::layout_with(data, images, BATTLE_PACK, 0, &textures)?;
        let (_, heat_sheet) = interface::load_texture(data, images, &heat_figures)?;
        let (_, count_sheet) = interface::load_texture(data, images, &count_figures)?;
        let (_, guarded_sheet) = interface::load_texture(data, images, &interface::file(SIGNS_PACK, GUARDED_FIGURES, "ptm"))?;
        let tunables = Tunables::load(data)?;
        let share = |id: u32| tunables.float(id) / 100.0;
        let signs = interface::signs_textures();
        let (_, level_sheet) = interface::load_texture(data, images, &signs[LEVEL_TEXTURE])?;
        let signs = interface::layout_with(data, images, SIGNS_PACK, 0, &signs)?;
        let sight = interface::layout_with(data, images, BATTLE_PACK, SIGHT_FILE, &[own(SIGHT_TEXTURE), own(SIGHT_TEXTURE)])?;
        Ok(Hud {
            sight,
            gauge: crate::field::hud::gauge(data, images)?,
            her_effects: status.player(HER_EFFECT_SET),
            enemy_effects: signs.player(ENEMY_EFFECT_SET),
            status,
            signs,
            level_sheet: level_sheet.x,
            heat_sheet: heat_sheet.x,
            count_sheet: count_sheet.x,
            guarded_sheet: guarded_sheet.x,
            heat_shares: [share(tunables::id::HEAT_FIRST_PERCENT), share(tunables::id::HEAT_SECOND_PERCENT)],
            messages: Messages::load(data)?,
            results: ResultLayout::load(data, images)?,
            items: super::item_menu::ItemLayout::load(data, images)?,
        })
    }

    /// What the results of a battle won are drawn from.
    pub(super) fn results(&self) -> &ResultLayout {
        &self.results
    }

    /// The battle's own layout and the lines of its log, for the log's tests.
    #[cfg(test)]
    pub(super) fn of_the_log(&self) -> (&Layout, &Messages) {
        (&self.status, &self.messages)
    }

    /// Draws the gauge of her heat (0x40C5C): `share` of the most there is, and
    /// whether the gun has overheated; and with it the marks of the status effects
    /// on her that are up, each by its icon.
    fn draw_heat(&self, canvas: &mut Canvas, player: &Player, share: f32, overheated: bool, icons: &[usize]) {
        let layout = &self.status;
        let look = Look::default();
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            match group {
                HEAT_BAR | HEAT_BAR_OVERHEATED => {
                    if (group == HEAT_BAR_OVERHEATED) != overheated {
                        continue;
                    }
                    // Cut to her share of its object's width.
                    let Some(bar) = layout.first_of(group).and_then(|object| layout.objects.objects.get(object)) else { continue };
                    let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32);
                    let width = (bar.size[0] as f32 * share) as i32;
                    let tint = if overheated {
                        [255; 4]
                    } else if share <= self.heat_shares[0] {
                        HEAT_LOW
                    } else if share <= self.heat_shares[1] {
                        HEAT_MIDDLING
                    } else if share <= 1.0 {
                        HEAT_HIGH
                    } else {
                        [255; 4]
                    };
                    canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(width as f32, bar.size[1] as f32))));
                    layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { tint, ..look });
                    canvas.cut_to(None);
                }
                // Her heat in hundredths, with two figures at the least, each drawn
                // from its figure's cell of the sheet, the last where the layout
                // has it and the others before it.
                HEAT_NUMBER => {
                    if overheated {
                        continue;
                    }
                    let mut left = (share * 100.0) as i32;
                    let figures = if left < 100 { 2 } else { 3 };
                    for place in 0..figures {
                        let figure = left % 10;
                        left /= 10;
                        let inner = Affine2::from_translation(Vec2::new(-HEAT_FIGURE_PITCH * place as f32, 0.0));
                        let shift = Vec2::new(HEAT_FIGURE_CELL * figure as f32 / self.heat_sheet, 0.0);
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner, shift, ..look });
                    }
                }
                HEAT_WHILE_COOL => {
                    if !overheated {
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &look);
                    }
                }
                group if HEAT_WHILE_OVERHEATED.contains(&group) => {
                    if overheated {
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &look);
                    }
                }
                // The first of her marks where the layout has them, and each one
                // after it sixteen to the left of the last (0x3D3DC): of the group's
                // objects the one that is the mark's icon (0x31130).
                HEAT_MARKS => {
                    for (place, &icon) in icons.iter().enumerate() {
                        let inner = Affine2::from_translation(Vec2::new(-EFFECT_PITCH * place as f32, 0.0));
                        layout.draw_member(canvas, layer::DISPLAY, &self.her_effects, 0, icon, &Look { inner, ..look });
                    }
                }
                _ => layout.draw_track(canvas, layer::DISPLAY, player, track, &look),
            }
        }
    }

    /// Draws how many enemies are left (0x41108): the figures from the right, and
    /// the word after them moved over to make room for all but one of them.
    fn draw_count(&self, canvas: &mut Canvas, player: &Player, count: i32) {
        let layout = &self.status;
        let figures = figures(count.max(0));
        let room = (figures - 1) as f32 * COUNT_FIGURE_CELL;
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            match group {
                COUNT_NUMBER => {
                    if count < 0 {
                        continue;
                    }
                    let mut left = count;
                    for place in 0..figures {
                        let figure = left % 10;
                        left /= 10;
                        let inner = Affine2::from_translation(Vec2::new(room - (COUNT_FIGURE_CELL + 1.0) * place as f32, 0.0));
                        let shift = Vec2::new(COUNT_FIGURE_CELL * figure as f32 / self.count_sheet, 0.0);
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner, shift, ..Look::default() });
                    }
                }
                COUNT_NONE => {
                    if count < 0 {
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &Look::default());
                    }
                }
                _ => {
                    let inner = Affine2::from_translation(Vec2::new(room, 0.0));
                    layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner, ..Look::default() });
                }
            }
        }
    }
}

impl Hud {
    /// Draws the mark of an enemy about `at`, where it hangs as the screen has it
    /// (0x3FE94). A mark is drawn about where it hangs, not where the layout has
    /// its objects (0x314DC).
    fn draw_mark(&self, canvas: &mut Canvas, player: &Player, at: Vec2, mark: &MarkAt) {
        let layout = &self.signs;
        // To whole pixels (0x3EFA8).
        let at = at.trunc();
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            let Some(first) = layout.first_of(group) else { continue };
            let Some(home) = layout.place(first) else { continue };
            let look = Look { inner: Affine2::from_translation(at), outer: Affine2::from_translation(-home), ..Look::default() };
            match group {
                // Cut to its share of its object's width, and to the screen.
                MARK_BAR => {
                    let size = layout.objects.objects[first].size;
                    let (mut x, mut y) = ((at.x - size[0] as f32 * 0.5 - 1.0) as i32, (at.y - size[1] as f32) as i32);
                    let (mut width, mut height) = ((size[0] as f32 * mark.share) as i32, size[1] as i32);
                    if x < 0 {
                        (width, x) = ((width + x).max(0), 0);
                    }
                    if y < 0 {
                        (height, y) = ((height + y).max(0), 0);
                    }
                    if width > 0 && height > 0 {
                        let from = Vec2::new(x as f32, y as f32);
                        canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(width as f32, height as f32))));
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &look);
                        canvas.cut_to(None);
                    }
                }
                MARK_SECOND_LIFE => {
                    if mark.second_life {
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &look);
                    }
                }
                // Its figures from the left, each from its cell of the sheet.
                MARK_LEVEL => {
                    let count = figures(mark.level);
                    for place in 0..count {
                        let figure = mark.level.max(0) / 10_i32.pow((count - 1 - place) as u32) % 10;
                        let inner = Affine2::from_translation(at + Vec2::new(LEVEL_FIGURE_CELL * place as f32, 0.0));
                        let shift = Vec2::new(LEVEL_FIGURE_CELL * figure as f32 / self.level_sheet, 0.0);
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner, shift, ..look });
                    }
                }
                // The marks of the status effects on it (0x3D410): the last of them
                // where the icons' own group has them about the point the mark
                // hangs at, and each one before it sixteen further to the right. An
                // icon is drawn about that point as the rest of the mark is, by the
                // place of its group's first object (0x31768).
                MARK_EFFECTS => {
                    let effects = &self.enemy_effects;
                    let Some(home) = effects.group(0).and_then(|group| layout.first_of(group)).and_then(|first| layout.place(first)) else { continue };
                    let outer = Affine2::from_translation(at - home);
                    for (place, &icon) in mark.icons.iter().enumerate() {
                        let inner = Affine2::from_translation(Vec2::new(EFFECT_PITCH * (mark.icons.len() - 1 - place) as f32, 0.0));
                        layout.draw_member(canvas, layer::DISPLAY, effects, 0, icon, &Look { inner, outer, ..Look::default() });
                    }
                }
                _ => layout.draw_track(canvas, layer::DISPLAY, player, track, &look),
            }
        }
    }
}

impl Hud {
    /// Draws the cursor about `at` (0x40338), or the mark on what she has locked on
    /// to. All of it is moved to where it is (0x321F8) from where the layout has
    /// each group's first object (0x314DC), so what turns does so about itself.
    fn draw_cursor(&self, canvas: &mut Canvas, player: &Player, at: Vec2, state: usize, mark: bool) {
        let layout = &self.signs;
        for track in 0..player.len() {
            let Some(group) = player.group(track).filter(|&group| cursor_shows(group, state, mark)) else { continue };
            let Some(home) = layout.first_of(group).and_then(|first| layout.place(first)) else { continue };
            let look = Look { outer: Affine2::from_translation(at - home), ..Look::default() };
            layout.draw_track(canvas, layer::DISPLAY, player, track, &look);
        }
    }
}

impl Hud {
    /// Draws the sight of a snipe (0x4217C): its lens where the lens's track has
    /// come to, and the rest as the layout has it, by whether it has locked on.
    fn draw_sight(&self, canvas: &mut Canvas, player: &Player, locked: bool) {
        let layout = &self.sight;
        let look = Look::default();
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            match group {
                SIGHT_LENS => {
                    let Some((moved, colour)) = layout.placed(player, track) else { continue };
                    lens(canvas, LENS_RADIUS * moved.matrix2.x_axis.length(), colour);
                }
                SIGHT_UNLOCKED if locked => {}
                group if SIGHT_LOCKED.contains(&group) && !locked => {}
                _ => layout.draw_track(canvas, layer::DISPLAY, player, track, &look),
            }
        }
    }

    /// Draws the column of her skills (0x41318).
    fn draw_palette(&self, canvas: &mut Canvas, palette: &Palette, fonts: &Fonts) {
        let layout = &self.status;
        let player = &palette.column;
        let look = Look::default();
        let row_at = |row: usize| Affine2::from_translation(Vec2::new(0.0, PALETTE_PITCH * row as f32));
        let ready = |row: usize| palette.shares[row] >= 1.0;
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            match group {
                // For each row, cut to its share of its object's width where the
                // column has come to, and to the screen.
                PALETTE_BAR => {
                    let Some(bar) = layout.first_of(group).and_then(|object| layout.objects.objects.get(object)) else { continue };
                    let Some((moved, _)) = layout.placed(player, track) else { continue };
                    for row in 0..PALETTE_ROWS {
                        let share = palette.shares[row];
                        let x = (moved.translation.x + bar.position[0] as f32).trunc();
                        let y = (PALETTE_PITCH * row as f32 + bar.position[1] as f32).trunc();
                        let width = (x + (bar.size[0] as f32 * share).trunc()).min(aim::SCREEN.x) - x;
                        if width <= 0.0 {
                            continue;
                        }
                        let tint = if share < 1.0 { PALETTE_FILLING } else { PALETTE_FULL };
                        canvas.cut_to(Some(Rect::from_corners(Vec2::new(x, y), Vec2::new(x + width, y + bar.size[1] as f32))));
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner: row_at(row), tint, ..look });
                        canvas.cut_to(None);
                    }
                }
                PALETTE_NOT_READY | PALETTE_READY => {
                    for row in (0..PALETTE_ROWS).filter(|&row| ready(row) == (group == PALETTE_READY)) {
                        layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner: row_at(row), ..look });
                    }
                }
                // Each row's flash is a track of its own of another set, moved as
                // the column's first track is.
                PALETTE_FLASH => {
                    let Some((moved, _)) = layout.placed(player, 0) else { continue };
                    for row in (0..PALETTE_ROWS).filter(|&row| ready(row)) {
                        layout.draw_track(canvas, layer::DISPLAY, &palette.flashes, row, &Look { inner: moved, ..look });
                    }
                }
                // Each row's name, with a shadow, where the group's object is for
                // that row: in a colour of its own once the skill is ready, and
                // at half strength until then (0x41794, 0x41A54).
                PALETTE_NAMES => {
                    let Some((moved, colour)) = layout.placed(player, track) else { continue };
                    let Some(object) = layout.first_of(group) else { continue };
                    let Some(place) = layout.place(object) else { continue };
                    let own = modulate(colour, layout.colour(object));
                    for (row, name) in palette.names.iter().enumerate() {
                        let Some(name) = name else { continue };
                        let at = (moved.translation + place + Vec2::new(0.0, PALETTE_PITCH * row as f32)).trunc();
                        let tint = modulate(own, if ready(row) { PALETTE_NAME_READY } else { PALETTE_NAME_WAITING });
                        fonts.draw(canvas, layer::DISPLAY, name, &Placing { tint, shadow: true, ..Placing::at(at) });
                    }
                }
                group if (PALETTE_BUTTONS..PALETTE_BUTTONS + PALETTE_ROWS).contains(&group) => {
                    let tint = if ready(group - PALETTE_BUTTONS) { [255; 4] } else { PALETTE_DIM };
                    layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { tint, ..look });
                }
                _ => layout.draw_track(canvas, layer::DISPLAY, player, track, &look),
            }
        }
    }
}

/// The lens of a snipe's sight (0x423AC, drawn by 0x41F8C): all of the screen
/// beyond a round of `radius` about its middle in `colour`, and the round itself
/// in the tint, as strongly as the colour's alpha has it.
fn lens(canvas: &mut Canvas, radius: f32, colour: [u8; 4]) {
    let middle = aim::SCREEN * 0.5;
    let tint = [LENS_TINT[0], LENS_TINT[1], LENS_TINT[2], (LENS_TINT[3] as u32 * colour[3] as u32 / 255) as u8];
    let (mut beyond, mut round) = (Vec::with_capacity(LENS_CORNERS * 2), Vec::with_capacity(LENS_CORNERS * 2));
    for corner in 0..LENS_CORNERS {
        let angle = (corner as f32 / (LENS_CORNERS - 1) as f32 * 360.0).to_radians();
        let way = Vec2::new(angle.sin(), angle.cos());
        beyond.push(Vertex::new(middle + way * LENS_REACH, Vec2::ZERO, colour));
        beyond.push(Vertex::new(middle + way * radius, Vec2::ZERO, colour));
        round.push(Vertex::new(middle + way * radius, Vec2::ZERO, tint));
        round.push(Vertex::new(middle, Vec2::ZERO, tint));
    }
    canvas.strip(layer::DISPLAY, Paint::PLAIN, &beyond);
    canvas.strip(layer::DISPLAY, Paint::PLAIN, &round);
}

/// The sight of a snipe as the display plays it: its layout's player, and what of
/// the scope's sight that has been made to follow.
struct Sight {
    player: Player,
    follows: scope::Sight,
}

impl Sight {
    /// Has the player follow the scope's sight (0x42060): a set the sight has
    /// started is played over what the player has, the coming from nothing; and
    /// otherwise it goes on by the frames. Says whether a sight that is going
    /// has played its going out.
    fn follow(kept: &mut Option<Sight>, layout: &Layout, now: scope::Sight, frames: f32) -> bool {
        match kept {
            Some(sight) if now.set == sight.follows.set && now.time >= sight.follows.time => {
                sight.follows = now;
                sight.player.advance(layout, frames);
            }
            Some(sight) if now.set != scope::set::COMES => {
                sight.follows = now;
                sight.player.play(layout, now.set);
            }
            _ => *kept = Some(Sight { player: layout.player(now.set), follows: now }),
        }
        now.going && kept.as_ref().is_some_and(|sight| sight.player.finished(layout))
    }
}

/// The column of her skills, as a battle keeps it (0x9F068).
#[derive(Clone)]
struct Palette {
    column: Player,
    /// Its motion has not been started: until she first has it come in it stands
    /// where coming in starts from (the entry's bit 30, 0x3CFD4).
    waiting: bool,
    /// She is holding it open.
    open: bool,
    flashes: Player,
    /// How far each row's skill is to being ready; a row without one stays at
    /// nothing (0x3E844).
    shares: [f32; PALETTE_ROWS],
    /// The rows' names, as they are written (0x35FA8).
    names: [Option<Block>; PALETTE_ROWS],
}

impl Palette {
    fn new(status: &Layout) -> Palette {
        Palette {
            column: status.player(PALETTE_IN),
            waiting: true,
            open: false,
            flashes: status.player(PALETTE_FLASHES),
            shares: [0.0; PALETTE_ROWS],
            names: [const { None }; PALETTE_ROWS],
        }
    }

    /// Takes what her slots have come to (0x35FDC), and has the rows whose skills
    /// have just come to be ready flash (0x36010).
    fn tell(&mut self, status: &Layout, slots: &mut Slots) {
        self.shares = slots.rows().map(|row| row.map_or(0.0, |(_, share)| share));
        for row in slots.ready_now.drain(..) {
            self.flashes.rewind_track(status, row);
        }
    }

    /// Has it come in while `held`, and go back when not (0x9F3A4). Says whether it
    /// has just started to come in, which is heard.
    fn hold(&mut self, status: &Layout, held: bool) -> bool {
        if held == self.open {
            return false;
        }
        self.open = held;
        self.waiting = false;
        self.column.play(status, if held { PALETTE_IN } else { PALETTE_OUT });
        held
    }

    fn advance(&mut self, status: &Layout, frames: f32) {
        if !self.waiting {
            self.column.advance(status, frames);
        }
        self.flashes.advance(status, frames);
    }
}

/// The cursor and the mark on what she has locked on to, as a battle keeps them.
struct Cursor {
    cursor: Player,
    /// Whether there is a cursor (the battle's +0x194): her states that aim make it
    /// and those that do not take it away.
    made: bool,
    /// The state its look is of, and whether it is out of sight for the mark's sake.
    look: usize,
    hidden: bool,
    /// The mark, which is made once and out of sight until she locks on, and where
    /// it was last put.
    mark: Player,
    mark_shown: bool,
    mark_at: Vec2,
    /// The state the cursor was last told of (the battle's +0x190).
    told: usize,
    locked: bool,
}

impl Cursor {
    fn new(signs: &Layout) -> Cursor {
        Cursor {
            cursor: signs.player(CURSOR_SET),
            made: false,
            look: 0,
            hidden: false,
            mark: signs.player(CURSOR_SET),
            mark_shown: false,
            mark_at: Vec2::ZERO,
            told: 0,
            locked: false,
        }
    }

    /// What a frame makes of it: whether she is in a state that has a cursor, the
    /// state her heat would put it in, and whether she has locked on to something.
    fn tell(&mut self, signs: &Layout, there: bool, state: usize, locked: bool) {
        // A cursor that is made is in sight, with the look of a cool gun whatever
        // her heat (0x3E388): it is told of the state of her heat only when that
        // changes, and what it was last told is not forgotten when it is taken away.
        if there && !self.made {
            self.cursor.rewind(signs);
            (self.look, self.hidden) = (0, false);
        }
        self.made = there;
        // Locking on puts the mark in the cursor's place, and letting go the cursor
        // back (0xA1608, 0xA1684); what comes into sight starts its motion anew
        // (0x3E680). Neither does anything while there is no cursor.
        if locked != self.locked {
            self.locked = locked;
            if self.made {
                (self.hidden, self.mark_shown) = (locked, locked);
                if locked {
                    self.mark.rewind(signs);
                } else {
                    self.cursor.rewind(signs);
                }
            }
        }
        // The mark is never told of her heat, and keeps the look of a cool gun.
        if self.made && state != self.told {
            (self.told, self.look) = (state, state);
            self.cursor.rewind(signs);
        }
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

/// What the display of the battle that is on is playing.
#[derive(Resource)]
struct Shown {
    gauge: Player,
    heat: Player,
    count: Player,
    /// The enemies' marks all play the one set; the port has them play it together.
    marks: Player,
    cursor: Cursor,
    palette: Palette,
    /// The sight of a snipe, while there is one.
    sight: Option<Sight>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Mode::Battle), begin)
        .add_systems(FixedUpdate, (hold_palette, turn_pages, keep_log).after(super::conclude).run_if(in_state(Mode::Battle).and_then(super::not_paused)))
        .add_systems(Update, draw.run_if(in_state(Mode::Battle)));
}

/// A battle's display starts its motions from their beginning (0x358E8).
fn begin(mut commands: Commands, hud: Res<Hud>, source: Res<BattleSource>, skills: Res<Skills>, fonts: Res<Fonts>, colours: Res<TextColours>) {
    // The column is made with the skills of her slots (0x9F068), each row's name
    // written once (0x3E84C, 0x35FA8): a row with no skill keeps its dashes.
    let mut slots = Slots::new(&skills, source.stats.skills);
    let mut palette = Palette::new(&hud.status);
    palette.names = slots.rows().map(|row| {
        let name = row.map_or(PALETTE_NO_SKILL, |(name, _)| name);
        Some(fonts.write(&PALETTE_NAME_STYLE, &text::pages(name, &text::Plain).swap_remove(0), &colours))
    });
    palette.tell(&hud.status, &mut slots);
    commands.insert_resource(slots);
    commands.insert_resource(Shown {
        gauge: hud.gauge.player(0.0),
        heat: hud.status.player(HEAT_SET),
        count: hud.status.player(COUNT_SET),
        marks: hud.signs.player(MARK_SET),
        cursor: Cursor::new(&hud.signs),
        palette,
        sight: None,
    });
    commands.insert_resource(Log::new(&hud.status));
}

/// A frame of the records the marks of status effects are on (0x3CFD4): where
/// somebody has more of them than are shown at a time, the wait for the next page
/// of them runs down.
fn turn_pages(player: Option<Single<&mut PcActor>>, mut enemies: Query<&mut Enemy>) {
    let sixtieths = SIXTIETHS_PER_STEP as i32;
    if let Some(mut player) = player {
        player.status.turn(sixtieths);
    }
    for mut enemy in &mut enemies {
        enemy.actor.body.status.turn(sixtieths);
    }
}

/// The log takes what the enemies have for it, and what her status effects have
/// (0x8DE7C, 0x8DEF8), and goes on by a frame (0x96A9C).
fn keep_log(
    source: Res<BattleSource>,
    hud: Res<Hud>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    horde: Option<ResMut<Horde>>,
    log: Option<ResMut<Log>>,
    player: Option<Single<&mut PcActor>>,
    mut board: ResMut<Board>,
) {
    let (Some(mut horde), Some(mut log)) = (horde, log) else { return };
    let hers = player.map(|mut player| std::mem::take(&mut player.status.said)).unwrap_or_default();
    for line in horde.lines.drain(..).chain(hers) {
        log.write(&hud.messages, &line);
    }
    let short = source.tunables.int(tunables::id::LOG_SHORT_TIME) as f32;
    let heard = log.step(&hud.status, SIXTIETHS_PER_STEP, short, &mut |words| log::written(&fonts, &colours, words));
    for _ in 0..heard {
        board.play(COMMON_BANK, log::SOUND_WORDS, FULL_VOLUME);
    }
}

/// The column of her skills comes in while R is held in a battle that is on
/// (0x9F3D4: the root's flag 8), and goes back when it is let go or the battle is
/// decided.
fn hold_palette(pad: Res<Pad>, flow: Res<Flow>, hud: Res<Hud>, shown: Option<ResMut<Shown>>, slots: Option<ResMut<Slots>>, mut board: ResMut<Board>) {
    let Some(mut shown) = shown else { return };
    if let Some(mut slots) = slots {
        shown.palette.tell(&hud.status, &mut slots);
    }
    if shown.palette.hold(&hud.status, flow.running() && pad.held & button::R != 0) {
        board.play(COMMON_BANK, SOUND_PALETTE, FULL_VOLUME);
    }
}

/// Draws the display, until her last state of a won battle is over: the original
/// takes all of it away then (0x94C28).
fn draw(
    hud: Res<Hud>,
    shown: Option<ResMut<Shown>>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    player: Option<Single<&PcActor>>,
    horde: Option<Res<Horde>>,
    aim: Option<Res<Aim>>,
    camera: Option<Single<&Projection, With<BattleCamera>>>,
    log: Option<Res<Log>>,
    results: Option<Res<Results>>,
    (conclusion, show): (Res<super::Conclusion>, Option<Res<super::boss::Show>>),
    fonts: Res<Fonts>,
    mut scope: ResMut<Scope>,
    mut numbers: Option<ResMut<Numbers>>,
    mut canvas: ResMut<Canvas>,
) {
    let (Some(mut shown), Some(player), Some(horde), Some(aim), Some(camera)) = (shown, player, horde, aim, camera) else { return };
    let frames = interface::frames(&time);
    let aspect = match &**camera {
        Projection::Perspective(perspective) => perspective.aspect_ratio,
        _ => aim::SCREEN.x / aim::SCREEN.y,
    };
    // The numbers that are up go on, and those asked for are put up where the
    // camera has their places now (0x3D8CC).
    let struck = numbers.as_mut().is_some_and(|numbers| numbers.step(&hud.signs, frames, |at| aim.view.project(at, aspect)));
    let shown = &mut *shown;
    // The sight of a snipe follows what the scope has it do, and is done with
    // once its going has played out (0x42150).
    match scope.sight {
        Some(now) if Sight::follow(&mut shown.sight, &hud.sight, now, frames) => (scope.sight, shown.sight) = (None, None),
        Some(_) => {}
        None => shown.sight = None,
    }
    // What a hit takes off her has her gauge flash (0x3DB94).
    if struck {
        shown.gauge.play(&hud.gauge.layout, crate::field::hud::gauge::FLASH);
    }
    shown.gauge.advance(&hud.gauge.layout, frames);
    shown.heat.advance(&hud.status, frames);
    shown.count.advance(&hud.status, frames);
    shown.marks.advance(&hud.signs, frames);
    shown.cursor.cursor.advance(&hud.signs, frames);
    shown.cursor.mark.advance(&hud.signs, frames);
    shown.palette.advance(&hud.status, frames);
    let heat = if player.heat.max > 0.0 { player.heat.value / player.heat.max } else { 0.0 };
    // Her states that take the stage take the cursor away, and those that follow
    // them make it again (0x74650, 0x74CB8).
    let aiming = !player.staged && !player.finished;
    shown.cursor.tell(&hud.signs, aiming, cursor_state(heat, player.heat.overheated), aim.locked);
    // The log is drawn whether the display is on or not (0x964BC); with the display
    // gone come the results of a battle won. After a battle lost the display goes
    // too, and nothing comes.
    // Nor is there a display in a boss's battle until its script has it come
    // (0xBEA90), nor while an enemy that has the stage has taken it away
    // (0x35BF4).
    let off = show.as_ref().is_some_and(|show| !show.display());
    if player.finished || conclusion.display_gone || off {
        if let Some(log) = &log {
            log.draw(&mut canvas, &hud.status, &fonts);
        }
        if let Some(results) = results.as_ref().filter(|_| !conclusion.showing_a_record()) {
            results.draw(&hud.results, &mut canvas, &fonts);
        }
        return;
    }
    // The sight of a snipe is drawn first of all the display (0x356D8), so that
    // her gauges and the rest are over what it darkens.
    if let Some(sight) = &shown.sight {
        hud.draw_sight(&mut canvas, &sight.player, sight.follows.locked);
    }
    // The enemies' marks, where the camera has them on the PSP's screen (0x3EED8);
    // one that is not before the camera is not drawn, and while she has the stage
    // none is but those of the enemies a moment of hers has seen again, as the
    // enemies themselves are (0x9107C).
    for mark in horde.marks.iter().filter(|mark| !horde.out_of_sight(mark.enemy)) {
        if let Some(at) = aim.view.project(mark.at, aspect) {
            hud.draw_mark(&mut canvas, &shown.marks, at, mark);
        }
    }
    // The numbers of what has landed, the enemies' with their marks.
    if let Some(numbers) = &numbers {
        numbers.draw(&mut canvas, &hud.signs, [hud.count_sheet, hud.guarded_sheet], player.staged);
    }
    // The mark is put on the point she has locked on to while that is before the
    // camera, and the cursor where she aims (0xA194C).
    let cursor = &mut shown.cursor;
    let point = aim.target.and_then(|(enemy, gauge)| horde.lock_points.iter().find(|lock| lock.enemy == enemy && lock.gauge == gauge));
    if let Some(at) = point.and_then(|lock| aim.view.project(lock.at, aspect)) {
        cursor.mark_at = at;
    }
    if cursor.mark_shown && !player.staged {
        hud.draw_cursor(&mut canvas, &cursor.mark, cursor.mark_at, 0, true);
    }
    if cursor.made && !cursor.hidden {
        let at = aim.previous_cursor.lerp(aim.cursor, fixed.overstep_fraction());
        hud.draw_cursor(&mut canvas, &cursor.cursor, at, cursor.look, false);
    }
    // In a battle her HP gauge is where its layout has it (0x36154).
    hud.gauge.draw(&mut canvas, &shown.gauge, player.hp as i32, player.max_hp as i32, Vec2::ZERO);
    hud.draw_heat(&mut canvas, &shown.heat, heat, player.heat.overheated, &player.status.icons());
    hud.draw_count(&mut canvas, &shown.count, horde.to_beat);
    // The log's line is drawn over her gauges and under the column of her skills.
    if let Some(log) = &log {
        log.draw(&mut canvas, &hud.status, &fonts);
    }
    // A finish takes the column away (0x9EEAC), and nothing gives it back.
    if !show.is_some_and(|show| show.finishing()) {
        hud.draw_palette(&mut canvas, &shown.palette, &fonts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Option<Hud> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        Some(Hud::load(&data, &mut images).unwrap())
    }

    fn extent(vertices: &[Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    #[test]
    fn the_heat_gauge_is_a_bar_of_her_heat_with_its_hundredths_on_it() {
        let Some(hud) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let drawn = |share: f32, overheated: bool| {
            let mut canvas = Canvas::default();
            hud.draw_heat(&mut canvas, &Player::at(&hud.status, HEAT_SET, 60.0), share, overheated, &[]);
            canvas.vertices().to_vec()
        };
        // It is under her HP gauge, at the foot of the screen on the right.
        let cool = drawn(0.3, false);
        let all = extent(&cool);
        assert!(all.min.x > 330.0 && all.min.y > 250.0 && all.max.x <= 480.0 && all.max.y <= 272.0, "{all:?}");
        // With more heat there is more of the bar: it is cut where her share of its
        // object ends.
        let hot = drawn(0.9, false);
        let bar = hud.status.first_of(HEAT_BAR).map(|object| &hud.status.objects.objects[object]).unwrap();
        let edge = |vertices: &[Vertex], share: f32| {
            let end = bar.position[0] as f32 + (bar.size[0] as f32 * share).trunc();
            vertices.iter().any(|vertex| vertex.position.x == end)
        };
        assert!(edge(&cool, 0.3) && edge(&hot, 0.9) && !edge(&cool, 0.9));
        // The bar is of the colour of its share: blue while cool, then yellow, then
        // red (the shares are the battle system's).
        assert!(hud.heat_shares[0] > 0.0 && hud.heat_shares[0] < hud.heat_shares[1] && hud.heat_shares[1] < 1.0, "{:?}", hud.heat_shares);
        eprintln!("the heat gauge's shares: {:?}", hud.heat_shares);
        // Overheated, the figures give way to what says so.
        assert_ne!(drawn(1.0, true).len(), drawn(1.0, false).len());
    }

    #[test]
    fn the_count_of_enemies_makes_room_for_its_figures() {
        let Some(hud) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let drawn = |count: i32| {
            let mut canvas = Canvas::default();
            hud.draw_count(&mut canvas, &Player::at(&hud.status, COUNT_SET, 60.0), count);
            canvas.vertices().to_vec()
        };
        // At the foot of the screen on the left.
        let few = drawn(4);
        let all = extent(&few);
        assert!(all.max.x < 120.0 && all.min.y > 230.0, "{all:?}");
        // A second figure is one more thing drawn, and the word is eleven further on.
        let many = drawn(12);
        assert_eq!(many.len(), few.len() + 6);
        assert_eq!(extent(&many).max.x, all.max.x + 11.0);
        assert_eq!(figures(0), 1);
        assert_eq!(figures(100), 3);
    }

    #[test]
    fn an_enemys_mark_is_drawn_about_where_it_hangs() {
        let Some(hud) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let drawn = |at: Vec2, share: f32, level: i32, second_life: bool| {
            let mut canvas = Canvas::default();
            let mark = MarkAt { enemy: Entity::PLACEHOLDER, at: Vec3::ZERO, share, level, second_life, icons: Vec::new() };
            hud.draw_mark(&mut canvas, &Player::at(&hud.signs, MARK_SET, 60.0), at, &mark);
            canvas.vertices().to_vec()
        };
        // Its frame is fifty-one across and eight high about the point, wherever
        // that is: not where the layout keeps the mark's objects.
        let whole = drawn(Vec2::new(200.4, 100.7), 1.0, 2, false);
        let all = extent(&whole);
        assert_eq!((all.min, all.max), (Vec2::new(175.0, 96.0), Vec2::new(226.0, 107.0)), "{all:?}");
        assert_eq!(extent(&drawn(Vec2::new(300.0, 50.0), 1.0, 2, false)).min, Vec2::new(275.0, 46.0));
        // With half of its HP the bar ends halfway along its forty-three.
        let half = drawn(Vec2::new(200.0, 100.0), 0.5, 2, false);
        assert!(half.iter().any(|vertex| vertex.position.x == 177.0 + 21.0) && !whole.iter().any(|vertex| vertex.position.x == 177.0 + 21.0));
        // A level of two figures is one more thing drawn, and a second life another.
        assert_eq!(drawn(Vec2::new(200.0, 100.0), 1.0, 12, false).len(), whole.len() + 6);
        assert_eq!(drawn(Vec2::new(200.0, 100.0), 1.0, 2, true).len(), whole.len() + 6);
        // With no HP there is no bar.
        assert!(drawn(Vec2::new(200.0, 100.0), 0.0, 2, false).len() < whole.len());
    }

    #[test]
    fn the_marks_of_status_effects_stand_in_a_row_by_the_gauges() {
        let Some(hud) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // What a list of marks adds to what is drawn without any.
        let added = |with: Vec<Vertex>, without: &[Vertex]| with.into_iter().filter(|vertex| !without.contains(vertex)).collect::<Vec<Vertex>>();
        // Where in the sheet of icons a mark is drawn from.
        let from = |vertices: &[Vertex]| vertices.iter().map(|vertex| vertex.uv).reduce(Vec2::min).unwrap();

        // Hers: by the gauge of her heat, the first where the layout has them and
        // each one after it sixteen to its left.
        let hers = |icons: &[usize]| {
            let mut canvas = Canvas::default();
            hud.draw_heat(&mut canvas, &Player::at(&hud.status, HEAT_SET, 60.0), 0.3, false, icons);
            canvas.vertices().to_vec()
        };
        let none = hers(&[]);
        let one = added(hers(&[0]), &none);
        assert_eq!(one.len(), 6);
        assert_eq!((extent(&one).min, extent(&one).max), (Vec2::new(460.0, 217.0), Vec2::new(475.0, 232.0)));
        let three = added(hers(&[0, 5, 17]), &none);
        assert_eq!((extent(&three).min, extent(&three).max), (Vec2::new(428.0, 217.0), Vec2::new(475.0, 232.0)));
        let leftmost: Vec<Vertex> = three.iter().copied().filter(|vertex| vertex.position.x < 444.0).collect();
        assert_eq!(leftmost.len(), 6);
        // Each icon is a picture of its own of the one sheet: the first sixteen
        // four to a row, a quarter of the sheet's width and a sixteenth of its
        // height apart, and the eighteenth under the first.
        let first = from(&one);
        let apart = |icons: &[usize]| from(&added(hers(icons), &none)) - first;
        assert!(apart(&[1]).abs_diff_eq(Vec2::new(0.25, 0.0), 1e-5), "{}", apart(&[1]));
        assert!(apart(&[4]).abs_diff_eq(Vec2::new(0.0, 0.0625), 1e-5), "{}", apart(&[4]));
        assert!((from(&leftmost) - first).abs_diff_eq(Vec2::new(0.0, 0.25), 1e-5));
        // There are eighteen icons, and nothing is drawn for a number past them.
        assert!(added(hers(&[18]), &none).is_empty());

        // An enemy's: about the point its mark hangs at, the last of them over the
        // left of the bar and each one before it sixteen further to the right.
        let enemys = |at: Vec2, icons: &[usize]| {
            let mut canvas = Canvas::default();
            let mark = MarkAt { enemy: Entity::PLACEHOLDER, at: Vec3::ZERO, share: 1.0, level: 2, second_life: false, icons: icons.to_vec() };
            hud.draw_mark(&mut canvas, &Player::at(&hud.signs, MARK_SET, 60.0), at, &mark);
            canvas.vertices().to_vec()
        };
        let at = Vec2::new(200.4, 100.7);
        let none = enemys(at, &[]);
        let one = added(enemys(at, &[0]), &none);
        assert_eq!((extent(&one).min, extent(&one).max), (Vec2::new(176.0, 81.0), Vec2::new(191.0, 96.0)));
        assert!(from(&one).abs_diff_eq(first, 1e-5));
        let two = added(enemys(at, &[0, 1]), &none);
        assert_eq!((extent(&two).min, extent(&two).max), (Vec2::new(176.0, 81.0), Vec2::new(207.0, 96.0)));
        let right: Vec<Vertex> = two.iter().copied().filter(|vertex| vertex.position.x > 191.0).collect();
        assert!(right.len() == 6 && from(&right).abs_diff_eq(first, 1e-5));
        // It goes where the mark goes.
        let moved = added(enemys(at + Vec2::new(50.0, 20.0), &[0]), &enemys(at + Vec2::new(50.0, 20.0), &[]));
        assert_eq!(extent(&moved).min, Vec2::new(226.0, 101.0));
    }

    #[test]
    fn the_cursor_is_of_her_heat_and_gives_way_to_the_mark_on_what_she_locks_on_to() {
        let Some(hud) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // A third of the most heat there is is still the first state, and two thirds
        // the second.
        assert_eq!([cursor_state(0.33, false), cursor_state(0.34, false), cursor_state(0.66, false), cursor_state(0.67, false), cursor_state(0.1, true)], [0, 1, 1, 2, 3]);
        // The set has the fourteen groups the four states are drawn from, and each
        // state has something to draw as the cursor and as the mark.
        let player = Player::at(&hud.signs, CURSOR_SET, 10.0);
        let groups: Vec<usize> = (0..player.len()).filter_map(|track| player.group(track)).collect();
        assert!((2..16).all(|group| groups.contains(&group)), "{groups:?}");
        let drawn = |at: Vec2, state: usize, mark: bool| {
            let mut canvas = Canvas::default();
            hud.draw_cursor(&mut canvas, &player, at, state, mark);
            canvas.vertices().to_vec()
        };
        let at = Vec2::new(240.5, 136.25);
        for state in 0..4 {
            // A ring sixty-four across about where it is, wherever that is, to the
            // part of a pixel.
            let (cursor, mark) = (drawn(at, state, false), drawn(at, state, true));
            assert!(!cursor.is_empty() && !mark.is_empty(), "state {state}");
            let all = extent(&cursor);
            assert!(all.center().distance(at) < 6.0 && all.width() <= 80.0 && all.width() >= 30.0, "state {state}: {all:?}");
            let moved = extent(&drawn(at + Vec2::new(100.0, -50.0), state, false));
            assert!((moved.min - all.min).distance(Vec2::new(100.0, -50.0)) < 1e-3, "state {state}");
            // The mark is not the cursor, but for an overheated gun's.
            assert_eq!(cursor == mark, state == 3, "state {state}");
        }
        // A cool gun's cursor and a hot one's are of different groups.
        assert_ne!(drawn(at, 0, false), drawn(at, 2, false));

        // It is made when she can aim, with a cool gun's look whatever her heat was
        // last; told of her heat when that changes; and out of sight while the mark
        // is on something.
        let mut cursor = Cursor::new(&hud.signs);
        cursor.tell(&hud.signs, false, 0, false);
        assert!(!cursor.made);
        cursor.tell(&hud.signs, true, 2, false);
        assert!(cursor.made && !cursor.hidden && !cursor.mark_shown && cursor.look == 2);
        cursor.cursor.advance(&hud.signs, 20.0);
        cursor.tell(&hud.signs, true, 2, true);
        assert!(cursor.hidden && cursor.mark_shown && cursor.mark.frame() == 0.0);
        cursor.tell(&hud.signs, true, 2, false);
        assert!(!cursor.hidden && !cursor.mark_shown && cursor.cursor.frame() == 0.0);
        // Taken away and made again with her heat as it was, it has a cool gun's
        // look until her heat is of another state.
        cursor.tell(&hud.signs, false, 2, false);
        cursor.tell(&hud.signs, true, 2, false);
        assert_eq!((cursor.look, cursor.told), (0, 2));
        cursor.tell(&hud.signs, true, 1, false);
        assert_eq!((cursor.look, cursor.told), (1, 1));
    }

    #[test]
    fn the_column_of_her_skills_stands_at_the_edge_until_it_is_held_open() {
        let Some(hud) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        let mut images = Assets::<Image>::default();
        let (fonts, colours) = (Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap());
        let drawn = |palette: &Palette| {
            let mut canvas = Canvas::default();
            hud.draw_palette(&mut canvas, palette, &fonts);
            canvas.vertices().to_vec()
        };
        let on_screen = |vertices: &[Vertex]| extent(&vertices.iter().copied().filter(|vertex| vertex.position.x < aim::SCREEN.x).collect::<Vec<_>>());
        // Before she has ever opened it, it is at the top of the screen on the
        // right with most of it beyond the edge, and time does not move it.
        let mut palette = Palette::new(&hud.status);
        let closed = drawn(&palette);
        let seen = on_screen(&closed);
        assert!(seen.min.x > 440.0 && seen.max.y < 110.0, "{seen:?}");
        assert!(extent(&closed).max.x > 560.0, "{:?}", extent(&closed));
        palette.advance(&hud.status, 30.0);
        assert_eq!(drawn(&palette), closed);
        // Held, it comes in by a hundred and forty, which is heard once.
        assert!(palette.hold(&hud.status, true));
        assert!(!palette.hold(&hud.status, true));
        palette.advance(&hud.status, 30.0);
        let open = drawn(&palette);
        assert_eq!(extent(&open).min.x, extent(&closed).min.x - 140.0);
        // Its frame runs on a little way past the edge even then.
        assert!(extent(&open).max.x < aim::SCREEN.x + 16.0, "{:?}", extent(&open));
        // Let go, it goes back to where it was, without a sound.
        assert!(!palette.hold(&hud.status, false));
        palette.advance(&hud.status, 30.0);
        assert_eq!(extent(&drawn(&palette)), extent(&closed));
        // A row whose skill is ready has its button bright, a bar and what says so.
        let mut ready = Palette::new(&hud.status);
        ready.shares[1] = 1.0;
        let with = drawn(&ready);
        assert!(with.len() > closed.len());
        let half = Palette { shares: [0.0, 0.5, 0.0, 0.0], ..Palette::new(&hud.status) };
        assert!(drawn(&half).len() > closed.len());

        // With skills in her slots each row has its skill's name, written twice
        // for its shadow where the row is: the rows are twenty-one apart, and the
        // third one down is circle's.
        let skills = Skills::load(&data).unwrap();
        let mut slots = Slots::new(&skills, [1, 25, 0, 0]);
        let mut named = Palette::new(&hud.status);
        named.names = slots.rows().map(|row| row.map(|(name, _)| fonts.write(&PALETTE_NAME_STYLE, &text::pages(name, &text::Plain).swap_remove(0), &colours)));
        named.tell(&hud.status, &mut slots);
        assert!(slots.ready_now.is_empty());
        assert_eq!(named.shares, [1.0, 0.0, 1.0, 0.0]);
        assert_eq!(named.names.iter().map(Option::is_some).collect::<Vec<_>>(), [true, false, true, false]);
        named.hold(&hud.status, true);
        named.advance(&hud.status, 30.0);
        let glyphs = |palette: &Palette| drawn(palette).len() / 6;
        let both = glyphs(&named);
        let none = glyphs(&Palette { names: [const { None }; PALETTE_ROWS], ..named.clone() });
        assert_eq!(both - none, 2 * ("ChargeShot".len() + "Defender".len()));
        // A name is written in its own colour once its skill is ready.
        let place = hud.status.first_of(PALETTE_NAMES).and_then(|object| hud.status.place(object)).unwrap();
        let of_row = |palette: &Palette, row: usize| {
            let top = place.y + PALETTE_PITCH * row as f32;
            drawn(palette).into_iter().filter(|vertex| vertex.position.x >= place.x && vertex.position.y >= top && vertex.position.y < top + 15.0 && vertex.colour == PALETTE_NAME_READY).count()
        };
        assert!(of_row(&named, 0) > 0 && of_row(&named, 2) > 0 && of_row(&named, 1) == 0);
        let waiting = Palette { shares: [0.5, 0.0, 1.0, 0.0], ..named.clone() };
        assert!(of_row(&waiting, 0) == 0 && of_row(&waiting, 2) > 0);
    }
}
