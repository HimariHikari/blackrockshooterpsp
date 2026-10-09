//! The screen that lists the saves, to save a game in one or to load one: the
//! port's own, in place of the PSP's savedata dialog, which is what the original
//! hands over to (`docs/SAVE_NOTES.md`; `docs/compare/save_walkthrough.jpg` is
//! the dialog). It says of each save what the original has the dialog say: the
//! game's title, her level, the time played and where it was saved.
//!
//! Up and down go through the saves; cross saves in the one the cursor is on (it
//! asks first when there is a save there already) or loads it; circle goes back.

use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Seen, Vertex, HEIGHT, WIDTH};

use crate::controls::{button, Pad};
use crate::field::tables::FieldTables;
use crate::interface::layer;
use crate::save::{Listed, Saved, Saves, SLOTS, TITLE};
use crate::sound::Request;
use crate::text::{self, TextColours};
use crate::writing::{Align, Fonts, Placing, Style};

// Its sounds, of the bank that is always there: the cursor's, a choice's, going
// back, "no", and what is heard when it has done what was asked.
const SOUND_CURSOR: u32 = 2;
const SOUND_CHOSEN: u32 = 3;
const SOUND_BACK: u32 = 5;
const SOUND_REFUSED: u32 = 7;
const SOUND_DONE: u32 = 0x11;

/// The frames, in sixtieths, it takes to come and to go.
const COMES_OVER: i32 = 16;
/// How much of what is behind it its sheet takes away when it is all there.
const SHEET: f32 = 0.62;
/// The sixtieths in a frame of the game's logic.
const FRAMES_A_STEP: i32 = 2;

const TEXT: Style = Style::of_kind(0, 0x30, 1, 1.0, 2.0);
const WHITE: [u8; 4] = [255; 4];
const DIM: [u8; 4] = [255, 255, 255, 120];
const LINE: [u8; 4] = [255, 255, 255, 200];
const BAR: [u8; 4] = [40, 110, 200, 200];

// What it says besides what a save says of itself. The first is the original's
// own (0x165B88); the others stand in for the PSP's.
const NEW_DATA: &str = "Create new Save Data";
const OVERWRITE: &str = "Do you want to overwrite this data?";
const SAVED: &str = "Save completed.";
const LOADED: &str = "Load completed.";
const NOTHING: &str = "There is no data.";
// The system's own texts for what went wrong (`msg_system.bin`, 6 and 11).
const NOT_WRITTEN: &str = "An access error has occurred.";
const NOT_READ: &str = "Unable to read save data.";
const ANSWERS: [&str; 2] = ["Yes", "No"];
const NO: usize = 1;

/// The picture every save has: the game's own, 144 by 80.
pub const ICON: Vec2 = Vec2::new(144.0, 80.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Save,
    Load,
}

/// How it ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Nothing was done.
    Back,
    /// The game was saved.
    Saved,
    Loaded(Box<Saved>),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Coming,
    List,
    /// It asks whether the save under the cursor is to be written over.
    Asking,
    /// It says what came of it, until a button.
    Told(&'static str),
    Going,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaveScreen {
    mode: Mode,
    /// The game to save.
    game: Option<Box<Saved>>,
    list: Vec<Option<Listed>>,
    /// The saves the list has, by their numbers: when saving all there are and
    /// then the first that is free; when loading those there are.
    rows: Vec<usize>,
    cursor: usize,
    state: State,
    /// Sixtieths since it began to come, or to go.
    frames: i32,
    /// The answer the cursor is on while it asks.
    answer: usize,
    outcome: Outcome,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl SaveScreen {
    /// The screen to save `game` with.
    pub fn save(saves: &Saves, game: Saved) -> SaveScreen {
        SaveScreen::new(Mode::Save, saves, Some(Box::new(game)))
    }

    /// The screen to load a game with.
    pub fn load(saves: &Saves) -> SaveScreen {
        SaveScreen::new(Mode::Load, saves, None)
    }

    fn new(mode: Mode, saves: &Saves, game: Option<Box<Saved>>) -> SaveScreen {
        let list = saves.list();
        let mut rows: Vec<usize> = (0..SLOTS).filter(|&slot| list[slot].is_some()).collect();
        if mode == Mode::Save {
            rows.extend((0..SLOTS).find(|&slot| list[slot].is_none()));
        }
        // The cursor begins on the save last written: the one a player means, as
        // a rule.
        let newest = rows.iter().enumerate().filter_map(|(row, &slot)| Some((list[slot]?.written?, row))).max().map_or(0, |(_, row)| row);
        SaveScreen { mode, game, list, rows, cursor: newest, state: State::Coming, frames: 0, answer: NO, outcome: Outcome::Back, sounds: Vec::new() }
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// The save the cursor is on, by its number.
    fn slot(&self) -> Option<usize> {
        self.rows.get(self.cursor).copied()
    }

    /// A frame of the game's logic. Answers how it ended, once it has gone.
    pub fn step(&mut self, pad: &Pad, saves: &Saves) -> Option<Outcome> {
        let pressed = |buttons: u32| pad.pressed & buttons != 0;
        match self.state {
            State::Coming => {
                self.frames += FRAMES_A_STEP;
                if self.frames >= COMES_OVER {
                    self.state = State::List;
                }
            }
            State::List => {
                let rows = self.rows.len();
                if pressed(button::CIRCLE) {
                    self.sound(SOUND_BACK);
                    self.go();
                } else if pressed(button::CROSS) {
                    match (self.mode, self.slot()) {
                        (_, None) => self.sound(SOUND_REFUSED),
                        (Mode::Save, Some(slot)) if self.list[slot].is_some() => {
                            self.sound(SOUND_CHOSEN);
                            (self.state, self.answer) = (State::Asking, NO);
                        }
                        (Mode::Save, Some(slot)) => self.write(saves, slot),
                        (Mode::Load, Some(slot)) => self.read(saves, slot),
                    }
                } else if rows > 1 {
                    let to = (self.cursor + rows + usize::from(pad.repeat & button::DOWN != 0) - usize::from(pad.repeat & button::UP != 0)) % rows;
                    if to != self.cursor {
                        self.cursor = to;
                        self.sound(SOUND_CURSOR);
                    }
                }
            }
            State::Asking => {
                if pressed(button::CIRCLE) {
                    self.sound(SOUND_BACK);
                    self.state = State::List;
                } else if pressed(button::CROSS) {
                    match (self.answer, self.slot()) {
                        (NO, _) | (_, None) => {
                            self.sound(SOUND_BACK);
                            self.state = State::List;
                        }
                        (_, Some(slot)) => self.write(saves, slot),
                    }
                } else if pressed(button::UP | button::DOWN) {
                    self.answer = 1 - self.answer;
                    self.sound(SOUND_CURSOR);
                }
            }
            State::Told(_) => {
                if pressed(button::CROSS | button::CIRCLE) {
                    // Something that went wrong is said, and the list is there
                    // again; what went right ends it.
                    if self.outcome == Outcome::Back && !self.rows.is_empty() {
                        self.state = State::List;
                    } else {
                        self.go();
                    }
                }
            }
            State::Going => {
                self.frames += FRAMES_A_STEP;
                if self.frames >= COMES_OVER {
                    return Some(std::mem::replace(&mut self.outcome, Outcome::Back));
                }
            }
        }
        None
    }

    fn go(&mut self) {
        (self.state, self.frames) = (State::Going, 0);
    }

    fn write(&mut self, saves: &Saves, slot: usize) {
        let written = self.game.as_ref().map(|game| saves.write(slot, game));
        match written {
            Some(Ok(())) => {
                self.list = saves.list();
                self.outcome = Outcome::Saved;
                self.sound(SOUND_DONE);
                self.state = State::Told(SAVED);
            }
            failed => {
                if let Some(Err(error)) = failed {
                    error!("the game was not saved: {error:#}");
                }
                self.sound(SOUND_REFUSED);
                self.state = State::Told(NOT_WRITTEN);
            }
        }
    }

    fn read(&mut self, saves: &Saves, slot: usize) {
        match saves.read(slot) {
            Ok(Some(saved)) => {
                self.outcome = Outcome::Loaded(Box::new(saved));
                self.sound(SOUND_DONE);
                self.state = State::Told(LOADED);
            }
            failed => {
                if let Err(error) = failed {
                    error!("the game was not loaded: {error:#}");
                }
                self.sound(SOUND_REFUSED);
                self.state = State::Told(NOT_READ);
            }
        }
    }

    /// How much of it there is, from none to all, `ahead` of a frame of the
    /// game's logic past its last one.
    fn strength(&self, ahead: f32) -> f32 {
        let frames = self.frames as f32 + ahead * FRAMES_A_STEP as f32;
        match self.state {
            State::Coming => (frames / COMES_OVER as f32).clamp(0.0, 1.0),
            State::Going => (1.0 - frames / COMES_OVER as f32).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    /// Draws it over everything: its sheet, its heading, the save the cursor is
    /// on with its picture and what it says of itself, and what is asked or
    /// told.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, canvas: &mut Canvas, fonts: &Fonts, colours: &TextColours, tables: &FieldTables, icon: Option<&Handle<Image>>, seen: &Seen, ahead: f32) {
        let layer = layer::SAVES;
        let strength = self.strength(ahead);
        let faded = |colour: [u8; 4]| [colour[0], colour[1], colour[2], (colour[3] as f32 * strength) as u8];
        canvas.fill(layer, seen.0, [0, 0, 0, (255.0 * SHEET * strength) as u8]);
        let write = |canvas: &mut Canvas, words: &str, at: Vec2, align: Align, width: f32, tint: [u8; 4]| {
            let block = fonts.write(&TEXT, &text::pages(words, &text::Plain).swap_remove(0), colours);
            fonts.draw(canvas, layer, &block, &Placing { width, align, tint: faded(tint), ..Placing::at(at) });
        };

        // The heading.
        canvas.fill(layer, Rect::new(14.0, 9.0, 26.0, 21.0), faded([200, 200, 200, 255]));
        write(canvas, if self.mode == Mode::Save { "Save" } else { "Load" }, Vec2::new(32.0, 7.0), Align::Left, WIDTH, WHITE);
        // What the buttons do.
        let buttons = if matches!(self.state, State::Told(_)) { "\\b(00)Back" } else { "\\b(01)Enter    \\b(00)Back" };
        write(canvas, buttons, Vec2::new(0.0, HEIGHT - 22.0), Align::Centre, WIDTH, WHITE);

        if let State::Told(words) = &self.state {
            // Between two lines, as the PSP says what it has done.
            for y in [106.0, 150.0] {
                canvas.fill(layer, Rect::new(200.0, y, 460.0, y + 1.0), faded(LINE));
            }
            write(canvas, words, Vec2::new(200.0, 121.0), Align::Centre, 260.0, WHITE);
            self.draw_icon(canvas, icon, Vec2::new(24.0, 96.0), 1.0, faded(WHITE));
            return;
        }
        let Some(slot) = self.slot() else {
            write(canvas, NOTHING, Vec2::new(0.0, 121.0), Align::Centre, WIDTH, WHITE);
            return;
        };

        // The saves either side of the cursor's, smaller and dim, and its own.
        let small = 0.5;
        if self.cursor > 0 {
            self.draw_icon(canvas, icon, Vec2::new(60.0, 44.0), small, faded(DIM));
        }
        if self.cursor + 1 < self.rows.len() {
            self.draw_icon(canvas, icon, Vec2::new(60.0, 188.0), small, faded(DIM));
        }
        self.draw_icon(canvas, icon, Vec2::new(24.0, 96.0), 1.0, faded(WHITE));
        write(canvas, &format!("{} / {}", self.cursor + 1, self.rows.len()), Vec2::new(84.0, 7.0), Align::Left, WIDTH, DIM);

        let left = 180.0;
        match &self.list[slot] {
            Some(listed) => {
                write(canvas, TITLE, Vec2::new(left, 96.0), Align::Left, WIDTH, WHITE);
                canvas.fill(layer, Rect::new(left, 114.0, WIDTH - 8.0, 115.0), faded(LINE));
                // When it was written, by this machine's clock, and how big it is:
                // what the PSP's own list says of a save.
                let when = listed.written.and_then(crate::clock::moment).map_or(String::new(), |moment| format!("{}    ", moment.written()));
                let size = format!("DATA{slot:02}    {when}{} KB", listed.size.div_ceil(1024));
                let [time, place] = listed.about.details(tables);
                for (row, words) in [size, listed.about.level_line(), time, place].iter().enumerate() {
                    write(canvas, words, Vec2::new(left, 119.0 + row as f32 * 18.0), Align::Left, WIDTH, WHITE);
                }
            }
            None => {
                write(canvas, NEW_DATA, Vec2::new(left, 96.0), Align::Left, WIDTH, WHITE);
                canvas.fill(layer, Rect::new(left, 114.0, WIDTH - 8.0, 115.0), faded(LINE));
                write(canvas, &format!("DATA{slot:02}"), Vec2::new(left, 119.0), Align::Left, WIDTH, DIM);
            }
        }

        if self.state == State::Asking {
            let top = 196.0;
            canvas.fill(layer, Rect::new(left - 6.0, top - 4.0, WIDTH - 8.0, top + 58.0), faded([0, 0, 0, 200]));
            write(canvas, OVERWRITE, Vec2::new(left, top), Align::Left, WIDTH, WHITE);
            for (answer, words) in ANSWERS.iter().enumerate() {
                let at = Vec2::new(left + 40.0, top + 18.0 + answer as f32 * 18.0);
                if answer == self.answer {
                    canvas.fill(layer, Rect::new(at.x - 6.0, at.y - 1.0, at.x + 60.0, at.y + 16.0), faded(BAR));
                }
                write(canvas, words, at, Align::Left, WIDTH, WHITE);
            }
        }
    }

    /// A save's picture, or a plain panel where the game's has not been found.
    fn draw_icon(&self, canvas: &mut Canvas, icon: Option<&Handle<Image>>, at: Vec2, scale: f32, colour: [u8; 4]) {
        let to = at + ICON * scale;
        match icon {
            Some(icon) => canvas.sprite(layer::SAVES, Paint::textured(icon), Vertex::new(at, Vec2::ZERO, colour), Vertex::new(to, Vec2::ONE, colour)),
            None => canvas.fill(layer::SAVES, Rect::from_corners(at, to), [60, 70, 90, colour[3]]),
        }
    }

    #[cfg(test)]
    pub fn at(&self) -> (usize, Option<usize>) {
        (self.cursor, self.slot())
    }

    #[cfg(test)]
    pub fn told(&self) -> Option<&'static str> {
        match self.state {
            State::Told(words) => Some(words),
            _ => None,
        }
    }
}

/// The list of saves while it is up, and how it last ended, until whoever had it
/// put up takes that.
#[derive(Resource, Default)]
pub struct SaveDesk {
    screen: Option<SaveScreen>,
    ended: Option<Outcome>,
}

impl SaveDesk {
    pub fn open(&mut self, screen: SaveScreen) {
        (self.screen, self.ended) = (Some(screen), None);
    }

    /// How the screen ended, once: nothing while it is up, or if none was.
    pub fn take(&mut self) -> Option<Outcome> {
        self.ended.take()
    }
}

/// The picture a save is shown with, if the game's own was found.
#[derive(Resource, Default)]
pub struct Icon(pub Option<Handle<Image>>);

/// A frame of the list of saves, while it is up.
fn step(pad: Res<Pad>, saves: Res<Saves>, mut desk: ResMut<SaveDesk>, mut board: ResMut<crate::sound::Board>) {
    let Some(screen) = &mut desk.screen else { return };
    let ended = screen.step(&pad, &saves);
    board.ask_all(std::mem::take(&mut screen.sounds));
    if ended.is_some() {
        (desk.screen, desk.ended) = (None, ended);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(desk: Res<SaveDesk>, fonts: Res<Fonts>, colours: Res<TextColours>, tables: Res<FieldTables>, icon: Res<Icon>, seen: Res<Seen>, steps: Res<Time<Fixed>>, mut canvas: ResMut<Canvas>) {
    if let Some(screen) = &desk.screen {
        screen.draw(&mut canvas, &fonts, &colours, &tables, icon.0.as_ref(), &seen, steps.overstep_fraction());
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SaveDesk>().init_resource::<Icon>().add_systems(FixedUpdate, step).add_systems(Update, draw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::tables::PlayerStats;
    use crate::items::Inventory;
    use crate::progress::Progress;
    use crate::save::{Place, Played};
    use std::path::PathBuf;

    fn folder(name: &str) -> PathBuf {
        let folder = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).join(format!("save-screen-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    fn game(level: i32) -> Saved {
        let stats = PlayerStats { level, experience: 150, max_hp: 496.0, hp: 300.0, attack: 52, special: 47, defence: 31, burst_bullets: 6, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 };
        Saved { played: Played { sixtieths: 3600, cleared: 0 }, progress: Progress::start(), stats, inventory: Inventory::default(), place: Place::StageSelect, view: None }
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    /// Steps with nothing pressed until it takes the pad.
    fn come(screen: &mut SaveScreen, saves: &Saves) {
        while screen.state == State::Coming {
            assert_eq!(screen.step(&Pad::default(), saves), None);
        }
    }

    /// Steps with nothing pressed until it has gone, and says how it ended.
    fn gone(screen: &mut SaveScreen, saves: &Saves) -> Outcome {
        for _ in 0..40 {
            if let Some(outcome) = screen.step(&Pad::default(), saves) {
                return outcome;
            }
        }
        panic!("the screen did not go");
    }

    #[test]
    fn a_game_is_saved_in_a_new_save_at_once_and_over_another_only_when_that_is_answered_yes() {
        let saves = Saves::new(folder("save"));
        // With no save yet the list is the one that would be new.
        let mut screen = SaveScreen::save(&saves, game(3));
        assert_eq!(screen.at(), (0, Some(0)));
        // It takes nothing until it has come.
        assert_eq!(screen.step(&press(button::CROSS), &saves), None);
        assert!(saves.read(0).unwrap().is_none());
        come(&mut screen, &saves);
        screen.sounds.clear();
        screen.step(&press(button::CROSS), &saves);
        assert_eq!((screen.told(), std::mem::take(&mut screen.sounds)), (Some(SAVED), vec![Request::common(SOUND_DONE)]));
        assert_eq!(saves.read(0).unwrap(), Some(game(3)));
        screen.step(&press(button::CIRCLE), &saves);
        assert_eq!(gone(&mut screen, &saves), Outcome::Saved);

        // Now there is one, and after it the next that is free; the cursor is on
        // the one last written.
        let mut screen = SaveScreen::save(&saves, game(7));
        come(&mut screen, &saves);
        assert_eq!((screen.rows.clone(), screen.at()), (vec![0, 1], (0, Some(0))));
        // Over a save it asks, with its cursor on "No", which writes nothing.
        screen.step(&press(button::CROSS), &saves);
        assert_eq!((screen.state.clone(), screen.answer), (State::Asking, NO));
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(screen.state, State::List);
        assert_eq!(saves.read(0).unwrap(), Some(game(3)));
        screen.step(&press(button::CROSS), &saves);
        screen.step(&press(button::UP), &saves);
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(screen.told(), Some(SAVED));
        assert_eq!(saves.read(0).unwrap(), Some(game(7)));
        // Down from the last goes round; a new save is written without asking.
        let mut screen = SaveScreen::save(&saves, game(9));
        come(&mut screen, &saves);
        screen.step(&press(button::DOWN), &saves);
        assert_eq!(screen.at(), (1, Some(1)));
        screen.step(&press(button::DOWN), &saves);
        assert_eq!(screen.at(), (0, Some(0)));
        screen.step(&press(button::UP), &saves);
        screen.step(&press(button::CROSS), &saves);
        assert_eq!((screen.told(), saves.read(1).unwrap()), (Some(SAVED), Some(game(9))));
        std::fs::remove_dir_all(saves.folder()).unwrap();
    }

    #[test]
    fn a_game_is_loaded_from_the_saves_there_are_and_circle_goes_back() {
        let saves = Saves::new(folder("load"));
        // With none there is nothing to load: cross is refused, circle goes back.
        let mut screen = SaveScreen::load(&saves);
        come(&mut screen, &saves);
        assert_eq!(screen.at(), (0, None));
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(std::mem::take(&mut screen.sounds), [Request::common(SOUND_REFUSED)]);
        screen.step(&press(button::CIRCLE), &saves);
        assert_eq!(gone(&mut screen, &saves), Outcome::Back);

        saves.write(4, &game(3)).unwrap();
        saves.write(11, &game(12)).unwrap();
        let mut screen = SaveScreen::load(&saves);
        come(&mut screen, &saves);
        // Only the saves there are, and no new one.
        assert_eq!(screen.rows, [4, 11]);
        let on = screen.at().1.unwrap();
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(screen.told(), Some(LOADED));
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(gone(&mut screen, &saves), Outcome::Loaded(Box::new(saves.read(on).unwrap().unwrap())));

        // A save that cannot be read is said to be so, and the list is there again.
        std::fs::write(saves.folder().join("DATA04.sav"), b"junk").unwrap();
        let mut screen = SaveScreen::load(&saves);
        come(&mut screen, &saves);
        assert_eq!(screen.rows, [11]);
        std::fs::write(saves.folder().join("DATA11.sav"), b"junk").unwrap();
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(screen.told(), Some(NOT_READ));
        screen.step(&press(button::CROSS), &saves);
        assert_eq!(screen.state, State::List);
        std::fs::remove_dir_all(saves.folder()).unwrap();
    }
}
