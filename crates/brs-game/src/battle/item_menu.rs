//! Her items in a battle: the window that L brings up (`BtItemMenu`, 0x94DD4;
//! its list `BtItemMng`, 0x950EC; its picture the task `ifBtlMenuTask`, 0x34234).
//!
//! L, when she is free to choose what she does and can use items at all
//! (0x9A4FC, 0x98FA0), has the window come and the battle stand still: she, the
//! enemies, what flies and the slots of her skills all ask whether her items are
//! open before they have a frame (0x99620). The window lists what she carries
//! that can be used in a battle (0x953AC), four rows at a time: up and down move
//! the cursor, and at either end of the list a fresh press goes round to the
//! other (0x34814); cross chooses, with a sound of refusal if there is nothing;
//! circle or L leaves (0x3438C). As soon as she has chosen or left the battle
//! goes on, the window going over the next few frames, and what she chose is
//! used in her next frame (0x9A0E4, 0x981BC: `PcActor::use_item`).
//!
//! The layout is `if_btlsys.lpk/011.lpk`, with five textures: three of its own,
//! and of the pack that is always there the icons (file 20) and the figures
//! (file 9). It is played in five places, each with a set for coming and one for
//! going: the frame (sets 0 and 1), the row the cursor is on (2 and 3), a row (4
//! and 5), a row of an item of another sort, which no item of the game is (6 and
//! 7), and a row with nothing in it (8 and 9). The rows are drawn twenty apart.

use anyhow::Result;
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{modulate, Layout, Look, Player};

use super::actor::PcActor;
use super::damage::Random;
use super::effects::{Asked, BattleEffects};
use super::hud::Hud;
use super::flow::Flow;
use super::skills::Slots;
use super::BattleSource;
use brs_engine::step::SIXTIETHS_PER_STEP;
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::items::{Inventory, Items};
use crate::pause::Paused;
use crate::sound::{Board, Request, COMMON_BANK, FULL_VOLUME};
use crate::text::{self, TextColours};
use crate::writing::{Block, Fonts, Placing, Style};

const PACK: &str = "SYSTEM.VOL/if_btlsys.lpk/011.lpk";
/// The files of the pack that is always there that the layout draws with besides
/// its own three (0x34A74, 0x34A84): the icons and the figures.
const SHARED_TEXTURES: [usize; 2] = [20, 9];

/// The rows of the window, and how far apart they are drawn (0x354C0).
const ROWS: usize = 4;
const ROW_PITCH: f32 = 20.0;

// The places the layout is played in.
const FRAME: usize = 0;
const CURSOR_ROW: usize = 1;
const ROW: usize = 2;
const EMPTY_ROW: usize = 4;
const PLACES: usize = 5;

// The groups that are more than something to draw (0x34C58, 0x35058): the bar at
// the side and what goes along it when there are more items than rows, what is
// said of the item the cursor is on, and a row's icon, name and count.
const BAR: usize = 12;
const BAR_KNOB: usize = 13;
const ABOUT: usize = 15;
const ICON: usize = 19;
const NAME: usize = 20;
const COUNT: usize = 21;

/// The cells of the figures' sheet (the first of the table at 0x171078).
const FIGURE_CELL: f32 = 20.0;
/// An icon of the icons' sheet: fifteen square, in cells of sixteen, four to a row,
/// a pixel in from each cell's corner (0x2D198).
const ICON_SIZE: f32 = 15.0;
const ICON_CELL: f32 = 16.0;
const ICONS_ACROSS: i32 = 4;

/// How a row's name and what is said of an item are written (0x34B1C, 0x34B4C).
const NAME_STYLE: Style = Style::of_kind(0, 0x18, 1, 1.0, 1.0);
const ABOUT_STYLE: Style = Style::of_kind(0, 0x24, 2, 1.0, 1.0);

// The sounds of the bank that is always there (0x3445C on): leaving, the cursor,
// choosing, and that there is nothing to choose.
const SOUND_LEFT: u32 = 5;
const SOUND_CURSOR: u32 = 2;
const SOUND_CHOSEN: u32 = 3;
const SOUND_NOTHING: u32 = 7;

/// What the window is drawn from.
pub struct ItemLayout {
    layout: Layout,
    /// The width of the figures' sheet.
    figures: f32,
    icons: Handle<Image>,
    icons_size: Vec2,
}

impl ItemLayout {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<ItemLayout> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = SHARED_TEXTURES.map(|file| interface::file(interface::RESIDENT_PACK, file, "ptm"));
        let textures = [own(3), own(4), own(5), shared[0].clone(), shared[1].clone()];
        let (icons, icons_size) = interface::load_texture(data, images, &shared[0])?;
        let (_, figures) = interface::load_texture(data, images, &shared[1])?;
        Ok(ItemLayout { layout: interface::layout_with(data, images, PACK, 0, &textures)?, figures: figures.x, icons, icons_size })
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

/// A text of the window, with what it has put in (a button's sign in what is said
/// of an item).
fn write(fonts: &Fonts, colours: &TextColours, style: &Style, words: &str) -> Block {
    fonts.write(style, &text::pages(words, &text::Plain).swap_remove(0), colours)
}

/// A row as it is shown: an item, how many of it, its kind and its name.
#[derive(Clone, Debug)]
struct Row {
    count: i32,
    kind: u8,
    name: Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Closed,
    Coming,
    Open,
    Going,
}

/// What came of the window while it was open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Chosen(i32),
    Left,
}

/// The window (the task's record, 0x349C0).
pub struct Window {
    phase: Phase,
    players: [Player; PLACES],
    /// The items listed, with how many she has of each.
    list: Vec<(i32, i32)>,
    /// The row the cursor is on, and the first of the list that is shown (the
    /// record's + 0x60 and + 0x64).
    cursor: usize,
    top: usize,
    rows: [Option<Row>; ROWS],
    about: Option<Block>,
}

impl Window {
    fn new(what: &ItemLayout) -> Window {
        // Each place has its own sets: the one it comes by, and after it the one
        // it goes by.
        Window { phase: Phase::Closed, players: std::array::from_fn(|place| what.layout.player(place * 2)), list: Vec::new(), cursor: 0, top: 0, rows: [const { None }; ROWS], about: None }
    }

    /// The window comes (0x34584) with what she carries now.
    fn open(&mut self, what: &ItemLayout, list: Vec<(i32, i32)>, items: &Items, fonts: &Fonts, colours: &TextColours) {
        self.phase = Phase::Coming;
        for (place, player) in self.players.iter_mut().enumerate() {
            player.play(&what.layout, place * 2);
        }
        self.list = list;
        self.refresh(items, fonts, colours);
    }

    /// It goes (0x3460C).
    fn close(&mut self, what: &ItemLayout) {
        self.phase = Phase::Going;
        for (place, player) in self.players.iter_mut().enumerate() {
            player.play(&what.layout, place * 2 + 1);
        }
    }

    /// The item the cursor is on.
    fn chosen(&self) -> Option<i32> {
        self.list.get(self.top + self.cursor).map(|&(item, _)| item)
    }

    /// The rows are what the list has from its first shown on, and what is said
    /// is of the item the cursor is on (0x346BC), with both kept where there is
    /// something (0x3495C).
    fn refresh(&mut self, items: &Items, fonts: &Fonts, colours: &TextColours) {
        let count = self.list.len();
        if count == 0 {
            (self.cursor, self.top) = (0, 0);
        } else {
            if count <= ROWS {
                self.top = 0;
            } else if count < self.top + ROWS {
                self.top = count - ROWS;
            }
            self.cursor = self.cursor.min(count - 1);
        }
        for (row, shown) in self.rows.iter_mut().enumerate() {
            *shown = self.list.get(self.top + row).and_then(|&(number, held)| {
                let item = items.get(number)?;
                Some(Row { count: held, kind: item.kind, name: write(fonts, colours, &NAME_STYLE, &item.name) })
            });
        }
        let item = self.chosen().and_then(|number| items.get(number));
        self.about = item.map(|item| write(fonts, colours, &ABOUT_STYLE, &item.description));
    }

    /// Up and down move the cursor, held or pressed anew; at the first or the last
    /// of the list only a fresh press does anything, and goes round to the other
    /// end (0x34814). Says whether it moved.
    fn move_cursor(&mut self, pad: &Pad) -> bool {
        let count = self.list.len();
        if count == 0 {
            return false;
        }
        if pad.repeat & button::UP != 0 {
            if self.top + self.cursor > 0 {
                match self.cursor {
                    0 => self.top -= 1,
                    _ => self.cursor -= 1,
                }
            } else if pad.pressed & button::UP != 0 {
                self.cursor = (count - 1).min(ROWS - 1);
                self.top = count - self.cursor - 1;
            } else {
                return false;
            }
            true
        } else if pad.repeat & button::DOWN != 0 {
            if self.top + self.cursor < count - 1 {
                match self.cursor {
                    cursor if cursor < ROWS - 1 => self.cursor += 1,
                    _ => self.top += 1,
                }
            } else if pad.pressed & button::DOWN != 0 {
                (self.cursor, self.top) = (0, 0);
            } else {
                return false;
            }
            true
        } else {
            false
        }
    }

    /// A frame of it (0x3438C): what the pad has it do while it is open, and its
    /// motions. What came of it and what is heard of it are handed back.
    fn frame(&mut self, what: &ItemLayout, pad: &Pad, items: &Items, fonts: &Fonts, colours: &TextColours) -> (Option<Outcome>, Option<u32>) {
        let (mut outcome, mut heard) = (None, None);
        match self.phase {
            Phase::Closed => return (None, None),
            Phase::Coming => {
                if self.players[FRAME].finished(&what.layout) {
                    self.phase = Phase::Open;
                }
            }
            Phase::Open => {
                if pad.pressed & (button::L | button::CIRCLE) != 0 {
                    (outcome, heard) = (Some(Outcome::Left), Some(SOUND_LEFT));
                } else if self.move_cursor(pad) {
                    self.refresh(items, fonts, colours);
                    heard = Some(SOUND_CURSOR);
                } else if pad.pressed & button::CROSS != 0 {
                    match self.chosen() {
                        Some(item) => (outcome, heard) = (Some(Outcome::Chosen(item)), Some(SOUND_CHOSEN)),
                        None => heard = Some(SOUND_NOTHING),
                    }
                }
            }
            Phase::Going => {
                if self.players[FRAME].finished(&what.layout) {
                    self.phase = Phase::Closed;
                }
            }
        }
        for player in &mut self.players {
            player.advance(&what.layout, SIXTIETHS_PER_STEP);
        }
        (outcome, heard)
    }

    /// Draws it (0x34B9C).
    fn draw(&self, what: &ItemLayout, canvas: &mut Canvas, fonts: &Fonts) {
        if self.phase == Phase::Closed {
            return;
        }
        let layout = &what.layout;
        let object_of = |group: usize| layout.first_of(group).map(|object| &layout.objects.objects[object]);
        let height = |group: usize| object_of(group).map_or(0.0, |object| object.size[1] as f32);
        let count = self.list.len();
        // The frame.
        let frame = &self.players[FRAME];
        for track in 0..frame.len() {
            let Some(group) = frame.group(track) else { continue };
            match group {
                ABOUT => {
                    let (Some(about), Some((moved, colour))) = (&self.about, layout.placed(frame, track)) else { continue };
                    let Some((object, place)) = layout.first_of(group).and_then(|object| Some((object, layout.place(object)?))) else { continue };
                    let tint = modulate(colour, layout.colour(object));
                    if tint[3] != 0 {
                        fonts.draw(canvas, layer::BATTLE_ITEMS, about, &Placing { tint, shadow: false, ..Placing::at((moved.translation + place).trunc()) });
                    }
                }
                // What goes along the bar is as long as four rows are of all the
                // list, and as far down it as the first row shown is.
                BAR_KNOB => {
                    if count > ROWS {
                        let a_row = (height(BAR).trunc() - 2.0) / count as f32;
                        let inner = Affine2::from_scale_angle_translation(Vec2::new(1.0, a_row * ROWS as f32 / height(BAR_KNOB).trunc().max(1.0)), 0.0, Vec2::new(0.0, a_row * self.top as f32));
                        layout.draw_track(canvas, layer::BATTLE_ITEMS, frame, track, &Look { inner, ..default() });
                    }
                }
                _ => layout.draw_track(canvas, layer::BATTLE_ITEMS, frame, track, &Look::default()),
            }
        }
        // The rows, one under the other.
        for (number, row) in self.rows.iter().enumerate() {
            let down = Vec2::new(0.0, ROW_PITCH * number as f32);
            let look = Look { inner: Affine2::from_translation(down), ..default() };
            let Some(row) = row else {
                layout.draw(canvas, layer::BATTLE_ITEMS, &self.players[EMPTY_ROW], &look);
                continue;
            };
            let player = &self.players[if number == self.cursor { CURSOR_ROW } else { ROW }];
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                match group {
                    ICON | NAME => {
                        let Some((moved, colour)) = layout.placed(player, track) else { continue };
                        if colour[3] == 0 {
                            continue;
                        }
                        let Some((object, place)) = layout.first_of(group).and_then(|object| Some((object, layout.place(object)?))) else { continue };
                        let at = (moved.translation + down).trunc() + place.trunc();
                        if group == NAME {
                            let tint = modulate(colour, layout.colour(object));
                            fonts.draw(canvas, layer::BATTLE_ITEMS, &row.name, &Placing { tint, shadow: false, ..Placing::at(at) });
                        } else {
                            // The icon is drawn uncoloured (0x35438).
                            let icon = icon_of(row.kind);
                            let from = Vec2::new((icon % ICONS_ACROSS) as f32, (icon / ICONS_ACROSS) as f32) * ICON_CELL + Vec2::ONE;
                            let to = from + Vec2::splat(ICON_SIZE);
                            let white = [0xff; 4];
                            canvas.sprite(
                                layer::BATTLE_ITEMS,
                                Paint::textured(&what.icons),
                                Vertex::new(at, from / what.icons_size, white),
                                Vertex::new(at + Vec2::splat(ICON_SIZE), to / what.icons_size, white),
                            );
                        }
                    }
                    // How many, in two figures at the least, from the left on.
                    COUNT => {
                        let places = figures(row.count).max(2);
                        let pitch = object_of(group).map_or(0.0, |object| object.size[0] as f32) + 1.0;
                        for place in 0..places {
                            let figure = row.count.max(0) / 10_i32.pow((places - 1 - place) as u32) % 10;
                            let inner = Affine2::from_translation(down + Vec2::new(pitch * place as f32, 0.0));
                            let shift = Vec2::new(FIGURE_CELL * figure as f32 / what.figures, 0.0);
                            layout.draw_track(canvas, layer::BATTLE_ITEMS, player, track, &Look { inner, shift, ..default() });
                        }
                    }
                    _ => layout.draw_track(canvas, layer::BATTLE_ITEMS, player, track, &look),
                }
            }
        }
    }
}

/// Her items' menu in a battle.
#[derive(Resource, Default)]
pub struct ItemMenu {
    window: Option<Window>,
    /// It is open, and the battle stands still for it (the menu's flag 1,
    /// 0x94F90; her + 0x734).
    pub holding: bool,
}

impl ItemMenu {
    /// A battle begins, or is over: there is no window.
    pub fn clear(&mut self) {
        *self = ItemMenu::default();
    }
}

/// A frame of the menu (0x94FBC, with her part of it at 0x9A0BC). While it is
/// open nothing else of the battle has one.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run(
    (pad, paused, flow): (Res<Pad>, Res<Paused>, Res<Flow>),
    hud: Res<Hud>,
    source: Res<BattleSource>,
    (items, inventory): (Option<Res<Items>>, Option<ResMut<Inventory>>),
    (fonts, colours): (Res<Fonts>, Res<TextColours>),
    mut menu: ResMut<ItemMenu>,
    mut board: ResMut<Board>,
    mut random: ResMut<Random>,
    mut effects: ResMut<BattleEffects>,
    slots: Option<ResMut<Slots>>,
    player: Option<Single<(Entity, &mut PcActor)>>,
    mut animators: Query<&mut Animator>,
) {
    let (Some(items), Some(mut inventory), Some(player)) = (items, inventory, player) else { return };
    // Under the pause it is not there (0x94EA8).
    if paused.up() {
        return;
    }
    let (her, mut actor) = player.into_inner();
    let what = &hud.items;
    let menu = &mut *menu;
    let window = menu.window.get_or_insert_with(|| Window::new(what));
    let mut hold = |held: bool| {
        for mut animator in &mut animators {
            animator.paused = held;
        }
    };
    let asked = std::mem::take(&mut actor.items_asked);
    if !menu.holding {
        if asked && flow.running() {
            let list = items.for_battles(&inventory).into_iter().map(|item| (item, i32::from(inventory.held(item)))).collect();
            window.open(what, list, &items, &fonts, &colours);
            menu.holding = true;
            hold(true);
        }
        window.frame(what, &pad, &items, &fonts, &colours);
        return;
    }
    let (outcome, heard) = window.frame(what, &pad, &items, &fonts, &colours);
    if let Some(sound) = heard {
        board.ask(Request::common(sound));
    }
    let Some(outcome) = outcome else { return };
    window.close(what);
    menu.holding = false;
    hold(false);
    // What she chose is used (0x981BC): she has one fewer of it.
    let Outcome::Chosen(number) = outcome else { return };
    let Some(item) = items.get(number) else { return };
    inventory.add(&items, number, -1);
    let (heard, seen) = actor.use_item(item, &source.effects, &source.stats, &source.tunables, slots.map(|slots| slots.into_inner()), &mut random);
    info!("she uses {} in the battle: {} of {} HP, and {} left of it", item.name, actor.hp, actor.max_hp, inventory.held(number));
    for sound in heard {
        board.play(COMMON_BANK, sound, FULL_VOLUME);
    }
    for (effect, on_her) in seen {
        effects.ask(if on_her { Asked::on(effect, her, Mat4::IDENTITY, true) } else { Asked::at(effect, actor.position, false) });
    }
}

/// A battle begins with no window.
fn begin(mut menu: ResMut<ItemMenu>) {
    menu.clear();
}

pub fn plugin(app: &mut App) {
    use crate::Mode;
    use brs_engine::EngineSystems;
    app.init_resource::<ItemMenu>()
        .add_systems(OnEnter(Mode::Battle), begin)
        .add_systems(FixedUpdate, run.after(super::pause).before(EngineSystems::Animate).run_if(in_state(Mode::Battle)))
        .add_systems(Update, draw.run_if(in_state(Mode::Battle)));
}

/// Draws the window while there is any of it.
fn draw(menu: Res<ItemMenu>, hud: Res<Hud>, paused: Res<Paused>, fonts: Res<Fonts>, mut canvas: ResMut<Canvas>) {
    if let Some(window) = menu.window.as_ref().filter(|_| !paused.up()) {
        window.draw(&hud.items, &mut canvas, &fonts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Item;

    fn game() -> Option<(ItemLayout, Items, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let (fonts, colours) = (Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap());
        Some((ItemLayout::load(&data, &mut images).unwrap(), Items::load(&data).unwrap(), fonts, colours))
    }

    fn pad(pressed: u32) -> Pad {
        Pad { held: pressed, pressed, repeat: pressed, ..default() }
    }

    #[test]
    fn the_window_comes_lists_what_she_carries_and_gives_what_is_chosen() {
        let Some((what, items, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The layout's five places each have a set to come by and one to go by.
        assert_eq!((0..10).map(|set| what.layout.length(set)).collect::<Vec<_>>(), [6.0; 10]);
        let mut window = Window::new(&what);
        // Six kinds of item: more than the four rows.
        let list = vec![(1, 3), (2, 1), (6, 2), (9, 1), (14, 5), (20, 1)];
        window.open(&what, list, &items, &fonts, &colours);
        let still = pad(0);
        let mut frame = |window: &mut Window, pad: &Pad| window.frame(&what, pad, &items, &fonts, &colours);
        // It takes no button until it has come: six frames of the interface, two
        // to a frame of the battle.
        assert_eq!(frame(&mut window, &pad(button::CROSS)), (None, None));
        for _ in 0..3 {
            frame(&mut window, &still);
        }
        assert_eq!(window.phase, Phase::Open);
        assert_eq!(window.rows.iter().map(|row| row.as_ref().map(|row| row.count)).collect::<Vec<_>>(), [Some(3), Some(1), Some(2), Some(1)]);
        // Down the rows, then the list moves under the cursor, and at its end a
        // held button stops while a fresh press goes round.
        for expected in [(1, 0), (2, 0), (3, 0), (3, 1), (3, 2)] {
            assert_eq!(frame(&mut window, &pad(button::DOWN)), (None, Some(SOUND_CURSOR)));
            assert_eq!((window.cursor, window.top), expected);
        }
        assert_eq!(window.chosen(), Some(20));
        let held = Pad { held: button::DOWN, repeat: button::DOWN, ..default() };
        assert_eq!(frame(&mut window, &held), (None, None));
        assert_eq!(frame(&mut window, &pad(button::DOWN)), (None, Some(SOUND_CURSOR)));
        assert_eq!((window.cursor, window.top, window.chosen()), (0, 0, Some(1)));
        // And up from the first goes to the last, shown in the last row.
        assert_eq!(frame(&mut window, &pad(button::UP)), (None, Some(SOUND_CURSOR)));
        assert_eq!((window.cursor, window.top, window.chosen()), (3, 2, Some(20)));
        // It draws with all its rows, and cross chooses what the cursor is on.
        let mut canvas = Canvas::default();
        window.draw(&what, &mut canvas, &fonts);
        assert!(!canvas.vertices().is_empty());
        assert_eq!(frame(&mut window, &pad(button::CROSS)), (Some(Outcome::Chosen(20)), Some(SOUND_CHOSEN)));
        window.close(&what);
        for _ in 0..4 {
            frame(&mut window, &still);
        }
        assert_eq!(window.phase, Phase::Closed);
    }

    #[test]
    fn with_nothing_to_use_cross_is_refused_and_circle_or_l_leaves() {
        let Some((what, items, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut window = Window::new(&what);
        window.open(&what, Vec::new(), &items, &fonts, &colours);
        for _ in 0..4 {
            window.frame(&what, &pad(0), &items, &fonts, &colours);
        }
        assert_eq!(window.frame(&what, &pad(button::DOWN), &items, &fonts, &colours), (None, None));
        assert_eq!(window.frame(&what, &pad(button::CROSS), &items, &fonts, &colours), (None, Some(SOUND_NOTHING)));
        assert_eq!(window.frame(&what, &pad(button::L), &items, &fonts, &colours), (Some(Outcome::Left), Some(SOUND_LEFT)));
        assert_eq!(window.frame(&what, &pad(button::CIRCLE), &items, &fonts, &colours), (Some(Outcome::Left), Some(SOUND_LEFT)));
    }

    #[test]
    fn the_items_for_battles_are_those_that_can_be_used_there() {
        let Some((_, items, ..)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut inventory = Inventory::default();
        // A repairer, a charge, a weapon and something kept for the story.
        for number in [1, 9, 130, 257] {
            inventory.add(&items, number, 1);
        }
        assert_eq!(items.for_battles(&inventory), [1, 9]);
        // What those for battles only give her, by the kind of the effect.
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        let effects = crate::battle::status::Effects::load(&data).unwrap();
        let kinds: Vec<(String, i32, i32, i32)> = (9..=22)
            .filter_map(|number| {
                let item: &Item = items.get(number)?;
                let effect = effects.get(item.effect)?;
                Some((item.name.clone(), item.effect, effect.kind, effect.values[0]))
            })
            .collect();
        let of = |name: &str| kinds.iter().find(|(named, ..)| named == name).map(|&(_, effect, kind, figure)| (effect, kind, figure));
        use crate::battle::status::kind;
        // A charge names its slot by a bit (square's, triangle's, circle's,
        // cross's), and the one for all of them has every bit.
        assert_eq!(of("□ Charge"), Some((301, kind::CHARGE, 1)));
        assert_eq!(of("△ Charge"), Some((302, kind::CHARGE, 2)));
        assert_eq!(of("〇 Charge"), Some((303, kind::CHARGE, 4)));
        assert_eq!(of("× Charge"), Some((304, kind::CHARGE, 8)));
        assert_eq!(of("Max Charge"), Some((300, kind::CHARGE, 255)));
        assert_eq!(of("Body Cooler"), Some((400, kind::COOL, 0)));
        assert_eq!(of("Poison Break"), Some((201, kind::CURE, 1)));
        assert_eq!(of("ATK Boost"), Some((1501, kind::ATTACK_UP, 20)));
        assert_eq!(kinds.len(), 14);
    }
}
