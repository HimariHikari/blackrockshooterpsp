//! OPTIONS, the fourth of the screens the MENU's entries lead to
//! (`menuOptionTask`, 0x5AFCC): how loud the music, the sounds and the voices
//! are, and whether a scene's captions go on of themselves. See
//! `docs/MENU_NOTES.md`.
//!
//! It has five rows. The first three are volumes, each with a bar: left and
//! right move one, a step at a time and then, held long enough, a step a frame.
//! The last two are switches. Square asks whether everything is to be as a new
//! game has it, and cross on the sounds' row or the voices' plays something to
//! hear the volume by. The fifth row is the PSP's data install, of which a PC
//! has none: it reads as off, and cannot be changed.
//!
//! The title has the same screen up alone (`menuSingleBoot`, 0x5C798): it comes
//! and goes under the screen's fade then, with no wipe and no MENU behind it.

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};
use brs_formats::xtc;

use super::{step_cursor, Hers, Outcome, Wipe, FRAMES_A_STEP, LEAVING_FRAMES, WIPE_BETWEEN, WIPE_GOES};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::draw_window_on;
use crate::field::scene::{fade_kind, Fade};
use crate::field::window::Window;
use crate::interface::{self, layer};
use crate::options::{Options, LOUDEST};
use crate::sound::{Request, FULL_VOLUME};
use crate::text::{self, TextColours};
use crate::writing::Fonts;

/// The archive the screen is of (the fourth of the four named at 0x1711E8).
const PACK: &str = "IF/MENU/MENU_OPTION.GZ";
// Its files (0x5B6E4): a recording, which cross on the voices' row plays; its
// texts; and its layout.
const RECORDING: usize = 0;
const TEXTS: usize = 1;
const LAYOUT: usize = 2;

// Its texts: what the strip says of each row, what the window asks (0x5BA04),
// and its two answers.
const TEXT_QUESTION: usize = 5;
const TEXT_YES: usize = 6;
const TEXT_NO: usize = 7;
const TEXT_COUNT: usize = 8;
// The window's cursor begins on the second of its answers, and circle puts it
// there (0x6BC94 and 0x6BC84 with 1); the first is the one that puts the
// defaults back (0x5BB1C).
const ANSWER_YES: usize = 0;
const ANSWER_NO: i32 = 1;

// Its sounds, of the bank that is always there: as it has come (0x5BB98), as
// the cursor moves or a switch is thrown (0x5BACC, 0x5C578), as the menus are
// left (0x5B9A4), as it is left for the MENU (0x5B950), as a volume moves a
// step (0x5C3BC), as what cannot be changed is tried (0x5C648), and what cross
// plays on the sounds' row (0x5C6B8).
const SOUND_COME: u32 = 0x11;
const SOUND_CURSOR: u32 = 2;
const SOUND_OUT: u32 = 1;
const SOUND_BACK: u32 = 5;
const SOUND_STEP: u32 = 0xb;
const SOUND_REFUSED: u32 = 7;
const SOUND_SAMPLE: u32 = 0x66;
/// The frames it is open for before its sound is played (0x5BB74).
const SOUND_AFTER: i32 = 12;
/// The stream the recording is played on (0x5C6E8).
const SAMPLE_STREAM: usize = 1;

const ROWS: usize = 5;
/// The rows (0x5C108).
mod row {
    pub const MUSIC: usize = 0;
    pub const EFFECTS: usize = 1;
    pub const VOICES: usize = 2;
    pub const AUTO_SKIP: usize = 3;
}
/// The rows that are volumes, which have a bar.
const VOLUMES: usize = 3;
/// How far one row's bar is under another's (0x5BF18).
const ROW_PITCH: f32 = 22.0;
/// The frames left or right is held for before a volume moves a step in every
/// frame (0x5C218).
const HELD_BEFORE_FAST: i32 = 90;

/// The places of the layout's player (0x5B830): the panel and the screen's
/// name, and a place for each row, the first here.
mod place {
    pub const FRAME: usize = 0;
    pub const ROWS: usize = 1;
}
const SET_FRAME: usize = 2;
/// The set a row's place plays at first, and how far from it those are that it
/// plays as the cursor comes to it and as the cursor leaves it (0x5B264,
/// 0x5B3E0).
const SETS_AT_FIRST: [usize; ROWS] = [3, 6, 9, 12, 15];
const COME_TO: usize = 1;
const LEFT: usize = 2;
/// The set that nudges the first row's left arrow (0x5C374): its right arrow's
/// is the next, and each row's two follow the row before's.
const SET_NUDGE: usize = 0x12;

/// What the layout's groups are to the code that draws it (0x5BC98): a
/// volume's bar; and the two words of a switch, OFF to the left of its arrows
/// and ON to the right, of which each has two tracks: the first has the word
/// dim, for when the switch is not as it says, and the second has it lit.
mod group {
    pub const BAR: usize = 9;
    pub const OFF: usize = 12;
    pub const ON: usize = 15;
}

/// What the screen is drawn from.
pub struct Assets {
    layout: Layout,
    texts: Vec<String>,
    /// The file of the recording cross plays on the voices' row.
    recording: String,
    /// The options as a new game has them, which square puts back (0x14D8E4).
    defaults: Options,
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        // Its textures (0x5B77C on): one of the archive, a place that is left
        // empty, and two more of the archive.
        let textures = [own(5), own(5), own(6), own(7)];
        let table = xtc::Table::parse(&data.read(interface::file(PACK, TEXTS, "xtc"))?).context("the texts of OPTIONS")?;
        let text = |index: usize| -> Result<String> {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("OPTIONS has no text {index}"))?;
            Ok(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
        };
        Ok(Assets {
            layout: interface::layout_with(data, images, PACK, LAYOUT, &textures)?,
            texts: (0..TEXT_COUNT).map(text).collect::<Result<_>>()?,
            recording: interface::file(PACK, RECORDING, "riff"),
            defaults: Options::load(data)?,
        })
    }
}

/// The states its task goes through (0x5B498), by the numbers it has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// It is made, and the wipe is still uncovering it.
    Coming = 0,
    Open = 1,
    /// It is left: until the wipe has covered it, or the screen has gone dark.
    Leaving = 3,
    /// A frame in which nothing of it is drawn, and whoever made it is told.
    Ending = 4,
    Done = 5,
}

/// What it is at (its + 0x18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    /// Its first row has yet to come.
    Coming = 0,
    Rows = 1,
    /// The window asks whether the defaults are to be put back.
    Asking = 2,
    /// It is being left for the MENU.
    Back = 3,
    /// All the menus are being left.
    Out = 5,
}

/// The OPTIONS screen.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionScreen {
    state: State,
    at: At,
    /// The frames it has been waiting to sound in (+ 0x50), and whether it has
    /// sounded (flag 0x10000).
    waited: i32,
    sounded: bool,
    places: Vec<Player>,
    /// The row the cursor is on (+ 0x5C).
    row: usize,
    /// The frames left or right has been held for (+ 0x64).
    held: i32,
    /// The window that asks, while it is up, and what it answered once it has
    /// closed.
    window: Option<Window>,
    answer: Option<usize>,
    /// It is up alone, as the title has it (its + 0x54 and + 0x58): it was not
    /// come to under the menus' wipe, and triangle sounds as circle does.
    alone: bool,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl OptionScreen {
    /// The screen as it is made (0x5B6E4).
    pub fn new(assets: &Assets) -> OptionScreen {
        let layout = &assets.layout;
        let mut places = vec![layout.player(SET_FRAME)];
        places.extend(SETS_AT_FIRST.iter().map(|&set| layout.player(set)));
        OptionScreen { state: State::Coming, at: At::Coming, waited: 0, sounded: false, places, row: 0, held: 0, window: None, answer: None, alone: false, sounds: Vec::new() }
    }

    /// The screen as it is made to be up alone (0x5CB64).
    pub fn alone(assets: &Assets) -> OptionScreen {
        OptionScreen { alone: true, ..OptionScreen::new(assets) }
    }

    /// The interface's frames that one of its own is (0x850): the menus are
    /// drawn sixty times a second, the title thirty.
    fn frames(&self) -> i32 {
        if self.alone { FRAMES_A_STEP as i32 } else { 1 }
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// What the strip along the foot says (0x5B2C0): of the cursor's row.
    pub fn strip<'a>(&self, assets: &'a Assets) -> &'a str {
        &assets.texts[self.row]
    }

    /// Whether anything of it is drawn (0x5BCE4): not in the frame it ends in.
    pub fn drawn(&self) -> bool {
        self.state != State::Ending
    }

    /// A frame of its task (0x5B498), with the wipe and the screen's fade, which
    /// are the menus'. Answers how it ended, in the frame it does.
    pub fn frame(&mut self, assets: &Assets, hers: &mut Hers, wipe: &mut Wipe, fade: &mut Fade, pad: &Pad, first: bool) -> Option<Outcome> {
        match self.state {
            State::Coming => {
                self.screen(assets, hers, pad, first);
                if if self.alone { !fade.running() } else { wipe.idle() } {
                    self.state = State::Open;
                }
            }
            State::Open => {
                self.screen(assets, hers, pad, first);
                // The screen's fade is given the interface's frames, and takes
                // as many of the game's as they come to.
                let leaving = LEAVING_FRAMES / self.frames();
                match self.at {
                    At::Back if self.alone => {
                        fade.begin(fade_kind::OUT, leaving);
                        self.state = State::Leaving;
                    }
                    At::Back => {
                        wipe.play(WIPE_BETWEEN);
                        self.state = State::Leaving;
                    }
                    At::Out => {
                        if !self.alone {
                            wipe.play(WIPE_GOES);
                        }
                        self.state = State::Leaving;
                        fade.begin(fade_kind::OUT, leaving);
                    }
                    At::Coming | At::Rows | At::Asking => {}
                }
            }
            State::Leaving => {
                self.screen(assets, hers, pad, first);
                // The data install would be seen to here (0x5B65C): there is
                // none.
                if if self.at == At::Back && !self.alone { wipe.gone() } else { !fade.running() } {
                    self.state = State::Ending;
                }
            }
            State::Ending => {
                self.state = State::Done;
                return Some(if self.at == At::Back { Outcome::Back } else { Outcome::Out });
            }
            State::Done => {}
        }
        None
    }

    /// A frame of the screen itself (0x5B8D4). The pad's buttons that have newly
    /// gone down, and those that count as pressed again, are for the `first` of
    /// the frames a frame of the game's logic is; and the window, whose frames
    /// are the game's, has its own then.
    fn screen(&mut self, assets: &Assets, hers: &mut Hers, pad: &Pad, first: bool) {
        let frames = self.frames();
        if self.state == State::Open {
            let (pressed, repeat) = if first { (pad.pressed, pad.repeat) } else { (0, 0) };
            match self.at {
                // It takes the pad when its first row has come (0x31C20), and
                // sounds on the way.
                At::Coming => {
                    if self.places[place::ROWS].finished(&assets.layout) {
                        self.at = At::Rows;
                    }
                    if !self.sounded {
                        if self.waited < SOUND_AFTER {
                            self.waited += frames;
                        } else {
                            self.sound(SOUND_COME);
                            (self.waited, self.sounded) = (0, true);
                        }
                    }
                }
                At::Rows => self.at_the_rows(assets, hers, pad.held, pressed, repeat),
                // On the window's first answer everything is as a new game has
                // it (0x14D8E4).
                At::Asking => {
                    if let Some(answer) = self.answer.take() {
                        if answer == ANSWER_YES {
                            *hers.options = assets.defaults;
                        }
                        self.at = At::Rows;
                    }
                }
                At::Back | At::Out => {}
            }
        }
        for place in &mut self.places {
            place.advance(&assets.layout, frames as f32);
        }
        if first {
            if let Some(window) = &mut self.window {
                let answer = window.step(pad);
                self.sounds.extend(window.sounds.drain(..).map(Request::common));
                if answer.is_some() {
                    (self.answer, self.window) = (answer, None);
                }
            }
        }
    }

    /// What the pad does at the rows (0x5B938).
    fn at_the_rows(&mut self, assets: &Assets, hers: &mut Hers, held: u32, pressed: u32, repeat: u32) {
        let layout = &assets.layout;
        if pressed & button::CIRCLE != 0 {
            self.at = At::Back;
            self.sound(SOUND_BACK);
        } else if pressed & button::TRIANGLE != 0 {
            self.at = At::Out;
            self.sound(if self.alone { SOUND_BACK } else { SOUND_OUT });
        } else if pressed & button::SQUARE != 0 {
            let mut window = Window::new(text::pages(&assets.texts[TEXT_QUESTION], &text::Plain), &[&assets.texts[TEXT_YES], &assets.texts[TEXT_NO]], ANSWER_NO, ANSWER_NO);
            self.sounds.extend(window.sounds.drain(..).map(Request::common));
            (self.window, self.answer) = (Some(window), None);
            self.at = At::Asking;
        } else {
            let was = self.row;
            if step_cursor(button::UP, button::DOWN, &mut self.row, ROWS, pressed, repeat) {
                // The row come to and the row left play a set each, and the
                // strip says the new row's text.
                self.places[place::ROWS + self.row].play(layout, SETS_AT_FIRST[self.row] + COME_TO);
                self.held = 0;
                self.places[place::ROWS + was].play(layout, SETS_AT_FIRST[was] + LEFT);
                self.sound(SOUND_CURSOR);
            } else {
                self.change(assets, hers, held, pressed, repeat);
            }
        }
    }

    /// What the cursor's row has, and the most it can (0x5C144): a volume of
    /// 127, or a switch. The data install reads as off.
    fn value(&self, options: &Options) -> (i32, i32) {
        match self.row {
            row::MUSIC => (i32::from(options.music), i32::from(LOUDEST)),
            row::EFFECTS => (i32::from(options.effects), i32::from(LOUDEST)),
            row::VOICES => (i32::from(options.voices), i32::from(LOUDEST)),
            row::AUTO_SKIP => (i32::from(options.auto_skip), 1),
            _ => (0, 1),
        }
    }

    /// Left and right change the cursor's row, and cross plays what there is to
    /// hear a volume by (0x5C108). For the first frames a button is held a step
    /// is taken each time it counts as pressed, and from then on in every frame;
    /// a step that cannot be taken is heard all the same while steps are
    /// counted, and a volume's steps are heard only when the button counts as
    /// pressed.
    fn change(&mut self, assets: &Assets, hers: &mut Hers, held: u32, pressed: u32, repeat: u32) {
        let layout = &assets.layout;
        let (was, most) = self.value(hers.options);
        let (mut now, mut heard) = (was, false);
        if held & (button::LEFT | button::RIGHT) == 0 {
            self.held = 0;
        } else {
            let by = if self.held < HELD_BEFORE_FAST { repeat } else { held };
            if by & button::RIGHT != 0 {
                now = (was + 1).min(most);
            } else if by & button::LEFT != 0 {
                now = (was - 1).max(0);
            }
            heard = repeat & (button::LEFT | button::RIGHT) != 0;
            self.held += self.frames();
        }
        if now != was {
            let place = &mut self.places[place::ROWS + self.row];
            let nudge = SET_NUDGE + 2 * self.row;
            let options = &mut *hers.options;
            match self.row {
                row::MUSIC | row::EFFECTS | row::VOICES => {
                    match self.row {
                        row::MUSIC => options.music = now as u8,
                        row::EFFECTS => options.effects = now as u8,
                        _ => options.voices = now as u8,
                    }
                    // The arrow by the button that is held is nudged.
                    if held & button::LEFT != 0 {
                        place.play(layout, nudge);
                    } else if held & button::RIGHT != 0 {
                        place.play(layout, nudge + 1);
                    }
                    if heard {
                        self.sound(SOUND_STEP);
                    }
                }
                row::AUTO_SKIP => {
                    options.auto_skip = now != 0;
                    if pressed & button::LEFT != 0 {
                        place.play(layout, nudge);
                    } else if pressed & button::RIGHT != 0 {
                        place.play(layout, nudge + 1);
                    }
                    self.sound(SOUND_CURSOR);
                }
                // There is no data install to be had (0x5C5B0).
                _ => self.sound(SOUND_REFUSED),
            }
        }
        if pressed & button::CROSS != 0 {
            match self.row {
                row::EFFECTS => self.sound(SOUND_SAMPLE),
                row::VOICES => self.sounds.push(Request::SayAt(SAMPLE_STREAM, assets.recording.clone(), FULL_VOLUME)),
                _ => {}
            }
        }
    }
}

impl OptionScreen {
    /// Draws it as it is `ahead` of a frame of the game's logic past its last one
    /// (0x5BC98). The strip along the foot, which is drawn last, is the menus'.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, window: &Layout, fonts: &Fonts, colours: &TextColours, options: &Options, ahead: f32) {
        if !self.drawn() {
            return;
        }
        let layout = &assets.layout;
        let layer = layer::MENU_SCREEN;
        let on = |place: &Player| {
            let mut place = place.clone();
            place.advance(layout, ahead * FRAMES_A_STEP as f32);
            place
        };
        let places: Vec<Player> = self.places.iter().map(on).collect();
        let plain = Look::default();
        layout.draw(canvas, layer, &places[place::FRAME], &plain);

        // The volumes: each row's bar is cut to as much of its width as the
        // volume is of the loudest, where its object is and so much lower for
        // each row.
        let volumes = [options.music, options.effects, options.voices];
        for (row, volume) in volumes.into_iter().enumerate() {
            let player = &places[place::ROWS + row];
            for track in 0..player.len() {
                if player.group(track) != Some(group::BAR) {
                    layout.draw_track(canvas, layer, player, track, &plain);
                    continue;
                }
                let Some(bar) = layout.first_of(group::BAR).map(|object| &layout.objects.objects[object]) else { continue };
                let from = Vec2::new(bar.position[0] as f32, (bar.position[1] as f32 + row as f32 * ROW_PITCH).trunc());
                let width = (bar.size[0] as f32 * (f32::from(volume) / f32::from(LOUDEST))).trunc();
                if width > 0.0 {
                    canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(width, bar.size[1] as f32))));
                    layout.draw_track(canvas, layer, player, track, &plain);
                    canvas.cut_to(None);
                }
            }
        }

        // The switches: the word that says how the switch is is drawn by the
        // second of its tracks, the other by its first. There is no data
        // install to have on.
        for (row, switched) in [options.auto_skip, false].into_iter().enumerate() {
            let player = &places[place::ROWS + VOLUMES + row];
            for track in 0..player.len() {
                let shown = match player.group(track) {
                    Some(group::OFF) => player.number_in_group(track) == Some(usize::from(!switched)),
                    Some(group::ON) => player.number_in_group(track) == Some(usize::from(switched)),
                    _ => true,
                };
                if shown {
                    layout.draw_track(canvas, layer, player, track, &plain);
                }
            }
        }

        if let Some(asking) = &self.window {
            draw_window_on(canvas, layer, window, fonts, colours, asking, ahead);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{common, game, press, Game};
    use super::super::{Menu, Screen, State as MenuState};
    use super::*;

    const AS_NEW: Options = Options { music: 60, effects: 127, voices: 127, auto_skip: false };

    fn screen(menu: &Menu) -> Option<&OptionScreen> {
        match menu.main.as_ref()?.screen.as_ref()? {
            Screen::Options(screen) => Some(screen),
            _ => None,
        }
    }

    /// Brings the MENU up and has its cursor on its fourth entry.
    fn at_the_entry(game: &mut Game) -> Menu {
        let mut menu = Menu::default();
        menu.ask(1);
        while !menu.main.as_ref().unwrap().ready {
            game.step(&mut menu, &Pad::default());
        }
        for _ in 0..3 {
            game.step(&mut menu, &press(button::DOWN));
        }
        menu.sounds.clear();
        menu
    }

    /// Brings the MENU up and chooses its fourth entry: the menus as OPTIONS
    /// takes the pad.
    fn come(game: &mut Game) -> Menu {
        let mut menu = at_the_entry(game);
        game.step(&mut menu, &press(button::CROSS));
        let mut steps = 0;
        while !screen(&menu).is_some_and(|screen| screen.at == At::Rows) {
            assert!(!game.step(&mut menu, &Pad::default()));
            steps += 1;
            assert!(steps < 200);
        }
        menu.sounds.clear();
        menu
    }

    /// A button that is held and counts as pressed again, and one that is only
    /// held.
    fn again(buttons: u32) -> Pad {
        Pad { held: buttons, repeat: buttons, ..Pad::default() }
    }

    fn kept(buttons: u32) -> Pad {
        Pad { held: buttons, ..Pad::default() }
    }

    fn drawn(menu: &Menu, game: &Game) -> Canvas {
        let mut canvas = Canvas::default();
        screen(menu).unwrap().draw(&mut canvas, &game.assets.option, &game.assets.window, &game.fonts, &game.colours, &game.options, 0.0);
        canvas
    }

    #[test]
    fn cross_on_the_menus_fourth_entry_brings_it_and_it_takes_the_pad_once_its_first_row_has_come() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let assets = &game.assets.option;
        assert_eq!(assets.texts[0], "Change the music volume. \\b(03)Default");
        assert_eq!(assets.texts[2], "Change the voiceover volume. \\b(03)Default \\b(01)Play");
        assert_eq!((assets.texts[TEXT_QUESTION].as_str(), assets.texts[TEXT_YES].as_str(), assets.texts[TEXT_NO].as_str()), ("Restore to default?", "Yes", "No"));
        assert_eq!((assets.layout.textures.len(), assets.layout.motions.sets.len(), assets.defaults), (4, 28, AS_NEW));
        assert_eq!(game.options, AS_NEW);
        // The first row has come after forty-two frames.
        let tracks = assets.layout.motions.sets[SETS_AT_FIRST[0]].len();
        let longest = (0..tracks).map(|track| assets.layout.track_length(SETS_AT_FIRST[0], track)).fold(0.0, f32::max);
        assert_eq!(longest, 42.0);

        let mut menu = at_the_entry(&mut game);
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [3]);
        for _ in 0..6 {
            game.step(&mut menu, &Pad::default());
            assert!(menu.main.as_ref().unwrap().state == MenuState::Chosen && screen(&menu).is_none());
        }
        game.step(&mut menu, &Pad::default());
        let made = screen(&menu).unwrap().clone();
        assert_eq!((made.state, made.at, made.row, made.held), (State::Coming, At::Coming, 0, 0));
        assert_eq!(made.places.iter().map(Player::set).collect::<Vec<usize>>(), [2, 3, 6, 9, 12, 15]);
        // It takes the pad when its first row has come, forty-two of its frames
        // after it was made; and its sound is played twelve frames after the
        // wipe is through, which is before that.
        let (mut steps, mut sounded_at) = (7, None);
        while screen(&menu).unwrap().at != At::Rows {
            game.step(&mut menu, &press(button::CIRCLE | button::DOWN | button::RIGHT));
            steps += 1;
            if !menu.sounds.is_empty() {
                assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_COME]);
                assert_eq!(sounded_at.replace(steps), None);
            }
        }
        assert_eq!((steps, sounded_at), (28, Some(24)));
        let came = screen(&menu).unwrap();
        assert_eq!((came.state, came.row, menu.main.as_ref().unwrap().wipe.playing, game.options), (State::Open, 0, false, AS_NEW));
    }

    #[test]
    fn up_and_down_go_through_its_five_rows_and_the_strip_says_what_each_is() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        assert_eq!(now(&menu).strip(&game.assets.option), "Change the music volume. \\b(03)Default");
        // Down: the row come to and the row left play a set each.
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!((now(&menu).row, now(&menu).places[place::ROWS].set(), now(&menu).places[place::ROWS + 1].set()), (1, 5, 7));
        assert_eq!(now(&menu).strip(&game.assets.option), "Change the sound effect volume. \\b(03)Default \\b(01)Play");
        for (row, come_to) in [(2, 10), (3, 13), (4, 16)] {
            game.step(&mut menu, &again(button::DOWN));
            assert_eq!((now(&menu).row, now(&menu).places[place::ROWS + row].set()), (row, come_to));
        }
        assert_eq!(now(&menu).strip(&game.assets.option), "Turn on Data Install.  \\b(03)Default");
        // Held, it does not go round from the last row; a new press does.
        game.step(&mut menu, &again(button::DOWN));
        assert_eq!(now(&menu).row, 4);
        game.step(&mut menu, &press(button::DOWN));
        assert_eq!((now(&menu).row, now(&menu).places[place::ROWS].set(), now(&menu).places[place::ROWS + 4].set()), (0, 4, 17));
        game.step(&mut menu, &press(button::UP));
        assert_eq!(now(&menu).row, 4);
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 6]);
        assert_eq!(game.options, AS_NEW);
    }

    #[test]
    fn left_and_right_move_a_volume_a_step_and_held_long_enough_a_step_a_frame() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        // Right: the music is a step louder, its right arrow is nudged, and the
        // step is heard. Left likewise.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((game.options.music, now(&menu).places[place::ROWS].set()), (61, 0x13));
        game.step(&mut menu, &again(button::LEFT));
        game.step(&mut menu, &again(button::LEFT));
        assert_eq!((game.options.music, now(&menu).places[place::ROWS].set()), (59, 0x12));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_STEP; 3]);
        // Held without counting as pressed it does nothing, for ninety frames:
        // then it moves the volume a step in each of the menu's frames, which
        // is not heard.
        game.step(&mut menu, &Pad::default());
        assert_eq!(now(&menu).held, 0);
        for _ in 0..45 {
            game.step(&mut menu, &kept(button::RIGHT));
        }
        assert_eq!((game.options.music, now(&menu).held), (59, 90));
        game.step(&mut menu, &kept(button::RIGHT));
        assert_eq!((game.options.music, now(&menu).held), (61, 92));
        assert!(menu.sounds.is_empty());
        // The frames in which it counts as pressed are heard.
        game.step(&mut menu, &again(button::RIGHT));
        assert_eq!(game.options.music, 63);
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_STEP]);
        // It stops at the loudest, where a step that cannot be taken is not
        // heard; and going the other way at nothing.
        for _ in 0..40 {
            game.step(&mut menu, &kept(button::RIGHT));
        }
        assert_eq!(game.options.music, 127);
        game.step(&mut menu, &again(button::RIGHT));
        assert!(game.options.music == 127 && menu.sounds.is_empty());
        game.step(&mut menu, &Pad::default());
        for _ in 0..120 {
            game.step(&mut menu, &kept(button::LEFT));
        }
        assert_eq!(game.options.music, 0);
        // The other two volumes are the rows under it.
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::LEFT));
        game.step(&mut menu, &press(button::DOWN));
        game.step(&mut menu, &press(button::LEFT));
        game.step(&mut menu, &press(button::LEFT));
        assert_eq!(game.options, Options { music: 0, effects: 126, voices: 125, auto_skip: false });
        assert_eq!((now(&menu).places[place::ROWS + 1].set(), now(&menu).places[place::ROWS + 2].set()), (5 + 3, 0x16));
    }

    #[test]
    fn the_captions_switch_is_thrown_and_the_data_install_cannot_be() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        for _ in 0..3 {
            game.step(&mut menu, &press(button::DOWN));
        }
        menu.sounds.clear();
        // Right switches it on, which is heard as the cursor is; again does
        // nothing; left switches it off.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((game.options.auto_skip, now(&menu).places[place::ROWS + 3].set()), (true, 0x19));
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR]);
        game.step(&mut menu, &press(button::LEFT));
        assert_eq!((game.options.auto_skip, now(&menu).places[place::ROWS + 3].set()), (false, 0x18));
        // A button that only counts as pressed again throws it too, but its
        // arrow is nudged only for one newly down.
        game.step(&mut menu, &again(button::RIGHT));
        assert_eq!((game.options.auto_skip, now(&menu).places[place::ROWS + 3].set()), (true, 0x18));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_CURSOR; 2]);
        // There is no data install to switch on: "no".
        game.step(&mut menu, &press(button::DOWN));
        menu.sounds.clear();
        let before = game.options;
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_REFUSED]);
        game.step(&mut menu, &press(button::LEFT));
        assert!(menu.sounds.is_empty() && game.options == before);
        assert_eq!(now(&menu).places[place::ROWS + 4].set(), 16);
    }

    #[test]
    fn square_asks_and_its_first_answer_puts_everything_back_as_a_new_game_has_it() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        let now = |menu: &Menu| screen(menu).unwrap().clone();
        game.options = Options { music: 10, effects: 20, voices: 30, auto_skip: true };
        let changed = game.options;
        // Square: the window asks, with its cursor on "No", which changes
        // nothing.
        game.step(&mut menu, &press(button::SQUARE));
        let asking = now(&menu).window.unwrap();
        assert_eq!((now(&menu).at, text::plain(asking.page()), asking.choices.clone(), asking.selected), (At::Asking, "Restore to default?".to_owned(), vec!["Yes".to_owned(), "No".to_owned()], 1));
        let answer = |game: &mut Game, menu: &mut Menu, buttons: &[u32]| {
            for _ in 0..30 {
                game.step(menu, &Pad::default());
            }
            for &button in buttons {
                game.step(menu, &press(button));
            }
            let mut steps = 0;
            while screen(menu).unwrap().at == At::Asking {
                game.step(menu, &Pad::default());
                steps += 1;
                assert!(steps < 60);
            }
        };
        // While it asks the rows take nothing from the pad.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((now(&menu).row, game.options), (0, changed));
        answer(&mut game, &mut menu, &[button::CROSS]);
        assert_eq!((now(&menu).at, now(&menu).window, game.options), (At::Rows, None, changed));
        // "Yes" puts it all back.
        game.step(&mut menu, &press(button::SQUARE));
        answer(&mut game, &mut menu, &[button::UP, button::CROSS]);
        assert_eq!((now(&menu).at, game.options), (At::Rows, AS_NEW));
    }

    #[test]
    fn cross_plays_what_the_sounds_and_the_voices_are_heard_by() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        game.step(&mut menu, &press(button::CROSS));
        assert!(menu.sounds.is_empty());
        game.step(&mut menu, &press(button::DOWN));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_SAMPLE]);
        game.step(&mut menu, &press(button::DOWN));
        menu.sounds.clear();
        game.step(&mut menu, &press(button::CROSS));
        assert_eq!(std::mem::take(&mut menu.sounds), [Request::SayAt(1, "IF/MENU/MENU_OPTION.GZ/000.riff".to_owned(), 127)]);
        // The recording is there to be played.
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).unwrap();
        assert!(data.exists(&game.assets.option.recording));
    }

    #[test]
    fn it_draws_each_volume_as_so_much_of_its_bar_and_the_word_of_each_switch() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let menu = come(&mut game);
        // The bars are 256 across from 180, twenty-two apart: the music's is cut
        // to sixty of 127 of that, the other two are whole.
        let canvas = drawn(&menu, &game);
        let cut = (256.0_f32 * 60.0 / 127.0).trunc();
        assert_eq!(cut, 120.0);
        assert!(canvas.vertices().iter().any(|vertex| vertex.position == Vec2::new(180.0 + cut, 112.0)));
        assert!(canvas.vertices().iter().any(|vertex| vertex.position == Vec2::new(436.0, 112.0 + 22.0)) && canvas.vertices().iter().any(|vertex| vertex.position == Vec2::new(436.0, 112.0 + 44.0)));
        // With nothing of a volume there is nothing of its bar.
        game.options.voices = 0;
        let canvas = drawn(&menu, &game);
        assert!(!canvas.vertices().iter().any(|vertex| vertex.position == Vec2::new(436.0, 112.0 + 44.0)));

        // A switch's two words stand either side of its arrows, OFF to the left
        // and ON to the right: the one that says how it is is lit, the other
        // dim.
        // (What is drawn is triangles; a word is thirty-eight by ten.)
        let word = |canvas: &Canvas, at: Vec2| -> [u8; 4] {
            let within = |triangle: &&[brs_engine::canvas::Vertex]| triangle.iter().all(|vertex| Rect::from_corners(at, at + Vec2::new(38.0, 10.0)).contains(vertex.position));
            let found = canvas.vertices().chunks(3).find(within);
            found.unwrap_or_else(|| panic!("nothing is drawn at {at}"))[0].colour
        };
        let (off_word, on_word) = (Vec2::new(232.0, 175.0), Vec2::new(345.0, 175.0));
        let off = drawn(&menu, &game);
        let (lit, dim) = (word(&off, off_word), word(&off, on_word));
        let bright = |colour: [u8; 4]| colour.iter().map(|&part| u32::from(part)).sum::<u32>();
        assert!(bright(lit) > bright(dim), "{lit:?} {dim:?}");
        game.options.auto_skip = true;
        let on = drawn(&menu, &game);
        assert_eq!((word(&on, off_word), word(&on, on_word)), (dim, lit));
        // The data install's, a row lower, say that it is off whatever.
        let lower = Vec2::new(0.0, 22.0);
        assert_eq!((word(&on, off_word + lower), word(&on, on_word + lower)), (lit, dim));
    }

    #[test]
    fn up_alone_it_comes_and_goes_under_the_screens_fade_at_the_titles_pace() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = Menu::default();
        menu.open_options(&game.assets);
        let alone = |menu: &Menu| menu.single.clone().unwrap();
        let screen = |menu: &Menu| match alone(menu).screen {
            super::super::Alone::Options(screen) => screen,
            super::super::Alone::Gallery(_) => panic!("OPTIONS should be up"),
        };
        // No MENU is made: the screen alone, under the screen's fade.
        assert!(menu.is_open() && !menu.made() && menu.sounds.is_empty());
        assert_eq!((screen(&menu).state, alone(&menu).fade.level), (State::Coming, 1.0));
        // The fade comes off over seven frames of the game, and the screen is
        // open in the frame after. A frame of it is two of the interface's: it
        // sounds twelve of those after that, and takes the pad when its first
        // row has come, forty-two after it was made.
        let (mut steps, mut sounded_at, mut open_at) = (0, None, None);
        while screen(&menu).at != At::Rows {
            assert!(!game.step(&mut menu, &press(button::CIRCLE | button::DOWN | button::RIGHT)));
            steps += 1;
            if screen(&menu).state == State::Open {
                open_at.get_or_insert(steps);
            }
            if !menu.sounds.is_empty() {
                assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_COME]);
                assert_eq!(sounded_at.replace(steps), None);
            }
            assert!(steps < 100);
        }
        assert_eq!((open_at, sounded_at, steps), (Some(8), Some(15), 22));
        assert_eq!((alone(&menu).fade.level, alone(&menu).ground_frames, game.options), (0.0, 44.0, AS_NEW));

        // The rows are as in the menus, a frame of the game at a time.
        game.step(&mut menu, &press(button::RIGHT));
        assert_eq!((game.options.music, screen(&menu).held), (61, 2));
        game.step(&mut menu, &Pad::default());
        for _ in 0..45 {
            game.step(&mut menu, &kept(button::RIGHT));
        }
        assert_eq!((game.options.music, screen(&menu).held), (61, 90));
        game.step(&mut menu, &kept(button::RIGHT));
        assert_eq!(game.options.music, 62);
        menu.sounds.clear();

        // Triangle sounds as circle does, and either has the screen go dark over
        // seven frames; then a frame in which nothing of it is drawn, and it is
        // done with.
        for button in [button::TRIANGLE, button::CIRCLE] {
            let mut menu = Menu::default();
            menu.open_options(&game.assets);
            while screen(&menu).at != At::Rows {
                game.step(&mut menu, &Pad::default());
            }
            menu.sounds.clear();
            assert!(!game.step(&mut menu, &press(button)));
            assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_BACK]);
            assert_eq!((screen(&menu).state, alone(&menu).fade.running()), (State::Leaving, true));
            let mut steps = 1;
            while !game.step(&mut menu, &press(button::DOWN)) {
                steps += 1;
                assert!(steps < 60);
            }
            assert_eq!(steps, 8);
            assert!(!menu.is_open() && menu.sounds.is_empty());
        }

        // It is drawn over the menus' ground, with the strip saying what the
        // cursor's row is.
        let mut menu = Menu::default();
        menu.open_options(&game.assets);
        for _ in 0..30 {
            game.step(&mut menu, &Pad::default());
        }
        let mut canvas = Canvas::default();
        let seen = brs_engine::canvas::Seen(Rect::new(0.0, 0.0, 480.0, 272.0));
        alone(&menu).draw(&mut canvas, &game.assets, &game.fonts, &game.colours, &game.options, &seen, 0.0);
        assert!(!canvas.drawn_from(&game.assets.ground.textures[0]).is_empty());
        assert!(!canvas.drawn_from(&game.assets.option.layout.textures[0]).is_empty());
    }

    #[test]
    fn circle_goes_back_to_the_menu_and_triangle_out_of_the_menus() {
        let Some(mut game) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut menu = come(&mut game);
        game.step(&mut menu, &press(button::RIGHT));
        menu.sounds.clear();
        // Circle: the wipe covers it again; fourteen frames on it has a frame in
        // which nothing of it is drawn, and the MENU is back with its cursor
        // where it was. What was changed stays changed.
        game.step(&mut menu, &press(button::CIRCLE));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_BACK]);
        let main = |menu: &Menu| menu.main.clone().unwrap();
        assert_eq!((screen(&menu).unwrap().state, main(&menu).wipe.set, main(&menu).wipe.playing), (State::Leaving, WIPE_BETWEEN, true));
        let mut steps = 1;
        while screen(&menu).is_some() {
            assert!(!game.step(&mut menu, &press(button::DOWN)));
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 8);
        assert_eq!((main(&menu).state, main(&menu).entry, game.options.music), (MenuState::Back, 3, 61));
        assert!(menu.is_open());

        // In the frame it ends in it draws nothing.
        let mut ending = OptionScreen::new(&game.assets.option);
        ending.state = State::Ending;
        let mut canvas = Canvas::default();
        ending.draw(&mut canvas, &game.assets.option, &game.assets.window, &game.fonts, &game.colours, &game.options, 0.0);
        assert!(canvas.vertices().is_empty() && !ending.drawn());

        // Triangle: the screen goes dark as it does when the MENU is left, and
        // the menus are done with: a frame of the menus later than from the
        // other screens, for the frame it ends in.
        let mut menu = come(&mut game);
        assert!(!game.step(&mut menu, &press(button::TRIANGLE)));
        assert_eq!(common(&std::mem::take(&mut menu.sounds)), [SOUND_OUT]);
        assert_eq!((screen(&menu).unwrap().state, main(&menu).wipe.set), (State::Leaving, WIPE_GOES));
        let mut steps = 1;
        while !game.step(&mut menu, &Pad::default()) {
            steps += 1;
            assert!(steps < 60);
        }
        assert_eq!(steps, 8);
        assert!(!menu.is_open());
    }
}
