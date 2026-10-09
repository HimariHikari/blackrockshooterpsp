//! The title (`TitleTask`, 0x1193A8): the maker's mark, the title with PRESS
//! START, and its menu of NEW GAME, LOAD GAME, GALLERY, OPTIONS and INSTALL. See
//! `docs/TITLE_NOTES.md`.
//!
//! The mark is drawn sixty times a second and the title thirty, so a frame of the
//! port's logic is two of the mark's frames and one of the title's; the layout's
//! own frames are sixtieths either way.
//!
//! Left out, as a PC has no use for them: the window about the auto-save and the
//! memory stick's light, the windows of the system data, and the data install,
//! whose entry comes straight back to the menu. OPTIONS and GALLERY are the
//! menus' screens, up alone (`menu::Single`), and LOAD GAME the list of saves:
//! the title asks for each (`take_wanted`) and waits until it is done with.

pub mod interface;
mod rows;

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Seen, Vertex, HEIGHT, WIDTH};

use self::interface::{Answer, Entry, Interface, Told};
use self::rows::Rows;
use crate::battle::damage::Random;
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::scene::{fade_kind, Fade, FilmAsked};
use crate::field::tables::FieldTables;
use crate::field::FieldState;
use crate::interface::layer;
use crate::sound::{self, Request};

/// The texture of the maker's mark (the archive's first file).
const MARK: &str = "TITLE.VOL/ie.zzz";

/// The interface's frames in a frame of the port's logic (0x850).
const FRAMES_A_STEP: i32 = 2;

/// The tunes it plays, by their numbers in the game's list (0x119368): the
/// title's, and another once the game has been cleared.
const TUNE: i32 = 0;
const TUNE_CLEARED: i32 = 5;
/// The blocks the music stops over as a new game is begun (0x119F24), and as the
/// film is gone to (0x11A074).
const MUSIC_STOPS_OVER: i32 = 4;
const MUSIC_STOPS_FOR_FILM: i32 = 8;

/// The tenths the mark's light goes up and down by in one of its frames, and
/// the frames it stays whole for (0x119C5C, 0x119C90).
const LIGHT: i32 = 10;
const MARK_FRAMES: i32 = 120;
/// The buttons that send the mark away at once (0x119CCC).
const MARK_SKIPPED_BY: u32 = button::TRIANGLE | button::CIRCLE | button::CROSS | button::SQUARE;
/// The frames of the game the screen's fade comes off over after the mark
/// (0x119DD0): thirty of the interface's, asked for while the game is drawn
/// sixty times a second.
const COMES_OVER: i32 = 30;
/// The interface's frames the screen's fade takes otherwise (0x2983C): the game
/// is drawn thirty times a second by then, so half as many of its own.
const FADE_FRAMES: i32 = 30 / FRAMES_A_STEP;

/// The seconds the title is left alone for before the film plays (0x119E40), in
/// a game not cleared and in one that is; the frames of a second it counts by
/// (0x11A05C); and the frames between the screen's going dark and the film
/// (0x11A0B4).
const IDLE_SECONDS: [i32; 2] = [0x3c, 0x41];
const IDLE_FRAMES_A_SECOND: i32 = 60;
const FILM_AFTER: i32 = 0x3d;
/// The film it plays then (0x11A0F0), the buttons that end it (0x11A1D8), and the
/// frames the screen's fade takes to come off for it and to go over after it
/// (0x11A160, 0x11A224): four of the interface's.
const FILM: &str = "movie/BRS_OP_final.pmf";
const FILM_ENDED_BY: u32 = button::CROSS | button::START;
const FILM_FADE: i32 = 4 / FRAMES_A_STEP;

/// What the title is drawn from.
#[derive(Resource)]
pub struct TitleAssets {
    interface: interface::Assets,
    /// The maker's mark, and the size its texture is drawn from by.
    mark: Handle<Image>,
    mark_size: Vec2,
}

impl TitleAssets {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<TitleAssets> {
        let (mark, mark_size) = crate::interface::load_texture(data, images, MARK).context("the maker's mark")?;
        Ok(TitleAssets { interface: interface::Assets::load(data, images).context("what the title is drawn from")?, mark, mark_size })
    }
}

/// How the title ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// NEW GAME was chosen.
    NewGame,
}

/// The states of its task (0x1195FC), by the numbers it has for them. Those of
/// the windows it begins with on a PSP (1 to 6) are left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// The mark comes (8), stays (9) and goes (0xA).
    MarkComes = 8,
    Mark = 9,
    MarkGoes = 0xa,
    /// The title is set going (0xB).
    Start = 0xb,
    Menu = 0xc,
    /// A new game was chosen: until the screen has gone dark (0xD).
    Leaving = 0xd,
    Done = 0xe,
    /// OPTIONS has the screen (0x18), or the list of saves (0x13).
    Screen = 0x18,
    /// The film is made ready (0xF, 0x10), plays (0x11), and is done with: the
    /// screen goes dark again (the end of 0x11).
    FilmComes = 0xf,
    Film = 0x11,
    FilmGoes = 0x110,
    /// Back from the film (0x12).
    AfterFilm = 0x12,
}

pub struct Title {
    state: State,
    interface: Interface,
    /// The mark's light, in tenths (+ 0x2C), and the frames it has left (+ 0x18).
    light: i32,
    count: i32,
    /// The game has been cleared (0x394): another ground and another tune.
    cleared: bool,
    /// The film is on its way (+ 0x24, + 0x28), and the frames since the screen
    /// began to go dark for it (+ 0x394).
    film: bool,
    film_frames: i32,
    /// The film itself, from when it is made ready until it is done with: what
    /// plays films (`crate::film`) is told of it by whoever has the title's
    /// frames, and says in it how far it has got.
    pub showing: Option<FilmAsked>,
    /// The screen's fade, and its sheet in rows while that is what it is.
    fade: Fade,
    rows: Option<Rows>,
    random: Random,
    tune: Option<String>,
    /// The entry whose screen is to be made, until whoever makes it has taken
    /// that up.
    wanted: Option<Entry>,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl Title {
    /// The title as it is launched (0x7369C), with the mark about to come.
    /// `seed` is what its random numbers start from.
    pub fn new(assets: &TitleAssets, tables: &FieldTables, cleared: bool, seed: u64) -> Title {
        let tune = tables.tune(if cleared { TUNE_CLEARED } else { TUNE }).map(sound::tune_file);
        Title {
            state: State::MarkComes,
            interface: Interface::new(&assets.interface),
            light: 0,
            count: 0,
            cleared,
            film: false,
            film_frames: 0,
            showing: None,
            fade: Fade::CLEAR,
            rows: None,
            random: Random::new(seed),
            tune,
            wanted: None,
            // Whatever was playing stops (0x119648).
            sounds: vec![Request::StopMusic(0)],
        }
    }

    /// A frame of the port's logic (0x1195C0).
    pub fn step(&mut self, assets: &TitleAssets, pad: &Pad) -> Option<Outcome> {
        self.fade.advance();
        if let Some(rows) = &mut self.rows {
            rows.advance();
        }
        match self.state {
            State::MarkComes => {
                self.light += FRAMES_A_STEP;
                if self.light >= LIGHT {
                    (self.light, self.count) = (LIGHT, MARK_FRAMES);
                    self.state = State::Mark;
                }
            }
            State::Mark => {
                self.count -= FRAMES_A_STEP;
                if pad.pressed & MARK_SKIPPED_BY != 0 {
                    self.count = 0;
                }
                if self.count <= 0 {
                    self.state = State::MarkGoes;
                }
            }
            State::MarkGoes => {
                self.light -= FRAMES_A_STEP;
                if self.light <= 0 {
                    self.light = 0;
                    self.fade.begin(fade_kind::IN, COMES_OVER);
                    self.state = State::Start;
                }
            }
            State::Start => {
                // The tune, which was held while the mark was up, is heard from
                // its start (0x119E00).
                self.sounds.extend(self.tune.clone().map(Request::Music));
                self.film = false;
                self.interface.set_sky(self.cleared);
                // The cursor is put on LOAD GAME when a save was found (0x119E54):
                // there is none to find yet.
                self.interface.set_cursor(Entry::NewGame);
                self.interface.tell(&assets.interface, Told::Come);
                self.state = State::Menu;
            }
            State::Menu => self.menu(assets, pad),
            State::Leaving => {
                if self.rows.as_ref().is_none_or(Rows::there) {
                    self.state = State::Done;
                    return Some(Outcome::NewGame);
                }
            }
            // The film is waited for; when it can be played the screen comes
            // back, with the film on it (0x11A118 on).
            State::FilmComes => {
                if let Some(film) = self.showing.as_mut().filter(|film| film.ready || film.over) {
                    film.started = true;
                    self.rows = None;
                    self.fade = Fade::DARK;
                    self.fade.begin(fade_kind::IN, FILM_FADE);
                    self.state = State::Film;
                }
            }
            // It plays until it is over, or cross or start ends it (0x11A1D8);
            // then the screen goes dark.
            State::Film => {
                if self.showing.as_ref().is_none_or(|film| film.over) || pad.pressed & FILM_ENDED_BY != 0 {
                    self.fade.begin(fade_kind::OUT, FILM_FADE);
                    self.state = State::FilmGoes;
                }
            }
            State::FilmGoes => {
                if !self.fade.running() {
                    self.showing = None;
                    self.state = State::AfterFilm;
                }
            }
            // The options' music is what the player has again (0x11A2EC).
            State::AfterFilm => {
                self.sounds.push(Request::MusicOptionFull(false));
                self.sounds.extend(self.tune.clone().map(Request::Music));
                self.rows = None;
                self.fade.begin(fade_kind::IN, FADE_FRAMES);
                self.interface.tell(&assets.interface, Told::Reset);
                self.state = State::Start;
            }
            State::Done | State::Screen => {}
        }
        self.sounds.extend(self.interface.sounds.drain(..).map(Request::common));
        None
    }

    /// The entry whose screen is to be made now, if one was chosen that has one
    /// (0x119F5C).
    pub fn take_wanted(&mut self) -> Option<Entry> {
        self.wanted.take()
    }

    /// The screen an entry led to is done with: the menu is come straight back
    /// to; after OPTIONS (0x11A8CC) with the screen's fade coming off, after
    /// the list of saves with no game loaded (0x11A358) without.
    pub fn screen_done(&mut self, assets: &TitleAssets, faded: bool) {
        if self.state == State::Screen {
            if faded {
                self.fade.begin(fade_kind::IN, FADE_FRAMES);
            }
            self.interface.tell(&assets.interface, Told::ToMenu);
            self.state = State::Menu;
        }
    }

    /// The gallery played something (0x11A914, its `0x1004`): the title's tune
    /// is heard again from its start.
    pub fn tune_again(&mut self) {
        self.sounds.extend(self.tune.clone().map(Request::Music));
    }

    /// A game was loaded (0x11A33C): the title's music stops, and it is done.
    pub fn loaded(&mut self) {
        self.sounds.push(Request::StopMusic(MUSIC_STOPS_OVER));
        self.state = State::Done;
    }

    /// A frame of the menu (0x119EA0).
    fn menu(&mut self, assets: &TitleAssets, pad: &Pad) {
        // Two random numbers are drawn every frame the title is up.
        self.random.below(100);
        self.random.below(100);
        if !self.film && self.interface.answer() == Answer::Chosen {
            match self.interface.entry() {
                Entry::NewGame => {
                    self.sounds.push(Request::StopMusic(MUSIC_STOPS_OVER));
                    self.rows = Some(Rows::over(FADE_FRAMES, &mut self.random));
                    self.state = State::Leaving;
                }
                entry @ (Entry::Options | Entry::Load | Entry::Gallery) => {
                    self.wanted = Some(entry);
                    self.state = State::Screen;
                }
                // There is nothing to install: it comes straight back, as it
                // does when its screen is left with nothing done.
                Entry::Install => self.interface.tell(&assets.interface, Told::ToMenu),
            }
        }
        if !self.film && self.interface.idle() >= IDLE_SECONDS[usize::from(self.cleared)] * IDLE_FRAMES_A_SECOND {
            self.sounds.push(Request::StopMusic(MUSIC_STOPS_FOR_FILM));
            self.rows = Some(Rows::over(FADE_FRAMES, &mut self.random));
            (self.film, self.film_frames) = (true, 0);
        }
        if self.film {
            self.film_frames += FRAMES_A_STEP;
            // The film's task is made (0x11A0DC), with the options' music made
            // full for it (0x11A0C0).
            if self.film_frames >= FILM_AFTER && self.state == State::Menu {
                self.sounds.push(Request::MusicOptionFull(true));
                self.showing = Some(FilmAsked::new(FILM.into()));
                self.state = State::FilmComes;
            }
        } else {
            self.interface.frame(&assets.interface, pad, FRAMES_A_STEP);
        }
    }

    /// Draws it as it is `ahead` of a frame of the port's logic past its last one
    /// (0x11A970): the mark, or the title; and the screen's fade over it. What it
    /// does not draw is black.
    pub fn draw(&self, canvas: &mut Canvas, assets: &TitleAssets, seen: &Seen, ahead: f32) {
        canvas.fill(layer::TITLE, seen.0, [0, 0, 0, 255]);
        match self.state {
            State::MarkComes | State::Mark | State::MarkGoes => {
                let on = match self.state {
                    State::MarkComes => ahead,
                    State::MarkGoes => -ahead,
                    _ => 0.0,
                };
                let light = ((self.light as f32 + on * FRAMES_A_STEP as f32) / LIGHT as f32).clamp(0.0, 1.0);
                let grey = (light * 255.0) as u8;
                let colour = [grey, grey, grey, 255];
                let shown = Vec2::new(WIDTH, HEIGHT);
                canvas.sprite(layer::TITLE, Paint::textured(&assets.mark), Vertex::new(Vec2::ZERO, Vec2::ZERO, colour), Vertex::new(shown, shown / assets.mark_size, colour));
            }
            State::Start | State::Menu | State::Leaving => self.interface.draw(canvas, &assets.interface, ahead * FRAMES_A_STEP as f32),
            State::Done | State::FilmComes | State::Film | State::FilmGoes | State::AfterFilm | State::Screen => {}
        }
        let dark = (self.fade.level.clamp(0.0, 1.0) * 255.0) as u8;
        if dark != 0 {
            canvas.fill(layer::FADE, seen.0, [0, 0, 0, dark]);
        }
        if let Some(rows) = &self.rows {
            rows.draw(canvas, layer::FADE, seen.0);
        }
    }
}

/// Draws the title while it is up.
fn draw(state: Res<FieldState>, assets: Res<TitleAssets>, seen: Res<Seen>, steps: Res<Time<Fixed>>, mut canvas: ResMut<Canvas>) {
    if let Some(title) = state.title() {
        title.draw(&mut canvas, &assets, &seen, steps.overstep_fraction());
    }
}

/// Nothing of a scene is drawn under the title: the scenes' cameras are put out
/// for as long as it is up.
fn put_scenes_out(state: Res<FieldState>, mut cameras: Query<(Entity, &mut Camera), With<Camera3d>>, mut put_out: Local<Vec<Entity>>) {
    if state.title().is_some() {
        for (entity, mut camera) in &mut cameras {
            if camera.is_active {
                camera.is_active = false;
                put_out.push(entity);
            }
        }
    } else {
        for entity in put_out.drain(..) {
            if let Ok((_, mut camera)) = cameras.get_mut(entity) {
                camera.is_active = true;
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (put_scenes_out, draw).run_if(in_state(crate::Mode::Field)));
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    struct Game {
        assets: TitleAssets,
        tables: FieldTables,
    }

    fn game() -> Option<Game> {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return None;
        };
        let mut images = Assets::<Image>::default();
        Some(Game { assets: TitleAssets::load(&data, &mut images).unwrap(), tables: FieldTables::load(&data).unwrap() })
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    impl Game {
        fn title(&self, cleared: bool) -> Title {
            Title::new(&self.assets, &self.tables, cleared, 1)
        }

        /// Steps with nothing pressed until the title is in a state, and says
        /// how many steps that took.
        fn until(&self, title: &mut Title, state: State) -> usize {
            let mut steps = 0;
            while title.state != state {
                assert_eq!(title.step(&self.assets, &Pad::default()), None);
                steps += 1;
                assert!(steps < 10_000, "the title never came to {state:?}");
            }
            steps
        }

        /// Past the mark and PRESS START, to the menu.
        fn at_the_menu(&self, title: &mut Title) {
            self.until(title, State::Menu);
            while title.interface.answer() != Answer::PressStart {
                title.step(&self.assets, &Pad::default());
            }
            title.step(&self.assets, &press(button::START));
            for _ in 0..20 {
                title.step(&self.assets, &Pad::default());
            }
            title.sounds.clear();
        }
    }

    #[test]
    fn the_mark_comes_stays_two_seconds_and_goes_and_the_title_comes_with_its_tune() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        // Whatever was playing stops.
        assert_eq!(std::mem::take(&mut title.sounds), [Request::StopMusic(0)]);
        // A tenth of its light a frame, two frames a step.
        assert_eq!(game.until(&mut title, State::Mark), 5);
        assert_eq!((title.light, title.count), (LIGHT, MARK_FRAMES));
        assert_eq!(game.until(&mut title, State::MarkGoes), 60);
        assert_eq!(game.until(&mut title, State::Start), 5);
        // The screen's fade comes off over thirty frames of the game from here.
        assert_eq!((title.light, title.fade.level), (0, 1.0));
        assert!(title.sounds.is_empty());
        assert_eq!(game.until(&mut title, State::Menu), 1);
        assert_eq!(std::mem::take(&mut title.sounds), [Request::Music("SOUND/BGM_TITLE.AT3".into())]);
        // The ground of metal with the title over it, and PRESS START.
        assert_eq!(title.interface.sets(), [4, 5]);
        assert!(title.interface.drawn());
        for _ in 0..28 {
            title.step(&game.assets, &Pad::default());
            assert!(title.fade.running());
        }
        title.step(&game.assets, &Pad::default());
        assert_eq!((title.fade.level, title.fade.running()), (0.0, false));
    }

    #[test]
    fn a_button_sends_the_mark_away_at_once() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.until(&mut title, State::Mark);
        // Not start: only the four buttons.
        title.step(&game.assets, &press(button::START));
        assert_eq!(title.state, State::Mark);
        title.step(&game.assets, &press(button::CIRCLE));
        assert_eq!(title.state, State::MarkGoes);
    }

    #[test]
    fn start_brings_the_menu_and_the_cursor_goes_round_only_on_a_new_press() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.until(&mut title, State::Menu);
        // PRESS START can be answered once the second place has played its sets
        // through: twenty frames, ten steps, one more in which that is seen and
        // one in which it is acted on.
        let mut steps = 0;
        while title.interface.answer() != Answer::PressStart {
            // Nothing is taken before.
            title.step(&game.assets, &press(button::START));
            steps += 1;
        }
        assert_eq!(steps, 12);
        title.sounds.clear();
        title.step(&game.assets, &press(button::CROSS));
        // The entries come, NEW GAME lit, and the menu sounds at once.
        assert_eq!(title.interface.sets(), [4, 7]);
        assert_eq!(std::mem::take(&mut title.sounds), [Request::common(0x11)]);
        assert_eq!((title.interface.entry(), title.interface.answer()), (Entry::NewGame, Answer::Nothing));
        // The cursor is not to be moved until the entries have come: sixteen
        // frames.
        for _ in 0..8 {
            title.step(&game.assets, &press(button::DOWN));
            assert_eq!(title.interface.cursor(), 0);
        }
        title.step(&game.assets, &Pad::default());
        title.step(&game.assets, &press(button::DOWN));
        assert_eq!((title.interface.cursor(), title.interface.entry()), (1, Entry::Load));
        assert_eq!(std::mem::take(&mut title.sounds), [Request::common(2)]);
        // The third entry is GALLERY, the fourth OPTIONS and the last INSTALL.
        let mut entries = Vec::new();
        for _ in 0..3 {
            title.step(&game.assets, &press(button::DOWN));
            entries.push(title.interface.entry());
        }
        assert_eq!(entries, [Entry::Gallery, Entry::Options, Entry::Install]);
        // Held down at the last entry the cursor stays; pressed anew it goes
        // round to the first.
        title.step(&game.assets, &Pad { held: button::DOWN, repeat: button::DOWN, ..Pad::default() });
        assert_eq!(title.interface.cursor(), 4);
        title.step(&game.assets, &press(button::DOWN));
        assert_eq!(title.interface.cursor(), 0);
        title.step(&game.assets, &Pad { held: button::UP, repeat: button::UP, ..Pad::default() });
        assert_eq!(title.interface.cursor(), 0);
        title.step(&game.assets, &press(button::UP));
        assert_eq!(title.interface.cursor(), 4);
        // Held, and counting as pressed again, it moves within the entries.
        title.step(&game.assets, &Pad { held: button::UP, repeat: button::UP, ..Pad::default() });
        assert_eq!(title.interface.cursor(), 3);
        // Held without counting as pressed it does not.
        title.step(&game.assets, &Pad { held: button::UP, ..Pad::default() });
        assert_eq!(title.interface.cursor(), 3);
    }

    #[test]
    fn new_game_darkens_the_screen_stops_the_music_and_ends_the_title_under_the_rows() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.at_the_menu(&mut title);
        assert_eq!(title.step(&game.assets, &press(button::CROSS)), None);
        // The other entries go, the layout's black sheet comes over twenty
        // frames, and the entry is heard.
        assert_eq!(std::mem::take(&mut title.sounds), [Request::common(3)]);
        assert_eq!(title.interface.sets()[1], 27);
        let mut steps = 0;
        while title.state == State::Menu {
            assert_eq!(title.step(&game.assets, &Pad::default()), None);
            steps += 1;
            assert!(steps < 100);
        }
        // Ten steps for the sheet, one in which the interface is seen to have
        // gone, and one in which the task acts on it.
        assert_eq!(steps, 12);
        assert!(!title.interface.drawn());
        assert_eq!(std::mem::take(&mut title.sounds), [Request::StopMusic(4)]);
        assert!(title.rows.is_some());
        let mut ended = None;
        for step in 1..40 {
            if let Some(outcome) = title.step(&game.assets, &Pad::default()) {
                ended = Some((outcome, step));
                break;
            }
        }
        let (outcome, after) = ended.expect("the title ends");
        assert_eq!(outcome, Outcome::NewGame);
        assert!((7..=10).contains(&after), "{after}");
    }

    #[test]
    fn load_game_asks_for_the_list_of_saves_and_comes_back_with_no_fade_or_ends_the_title() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.at_the_menu(&mut title);
        title.step(&game.assets, &press(button::DOWN));
        title.step(&game.assets, &press(button::CROSS));
        game.until(&mut title, State::Screen);
        assert_eq!(title.take_wanted(), Some(Entry::Load));
        // Nothing loaded: the menu is come straight back to, with no fade.
        title.screen_done(&game.assets, false);
        assert_eq!((title.state, title.fade.running(), title.interface.sets()), (State::Menu, false, [28, 31]));
        // A game loaded: the music stops over four blocks and the title is done.
        let mut title = game.title(false);
        game.at_the_menu(&mut title);
        title.step(&game.assets, &press(button::DOWN));
        title.step(&game.assets, &press(button::CROSS));
        game.until(&mut title, State::Screen);
        title.sounds.clear();
        title.loaded();
        assert_eq!((title.state, std::mem::take(&mut title.sounds)), (State::Done, vec![Request::StopMusic(4)]));
        assert_eq!(title.step(&game.assets, &Pad::default()), None);
    }

    #[test]
    fn an_entry_with_nothing_behind_it_comes_straight_back_to_the_menu() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.at_the_menu(&mut title);
        // INSTALL, the last entry, which up from the first goes round to.
        title.step(&game.assets, &press(button::UP));
        assert_eq!(title.interface.entry(), Entry::Install);
        title.step(&game.assets, &press(button::CROSS));
        game.until(&mut title, State::Menu);
        let mut steps = 0;
        while title.interface.sets()[0] != 28 {
            title.step(&game.assets, &Pad::default());
            steps += 1;
            assert!(steps < 100);
        }
        // The title is there at once and the entries come later, INSTALL lit.
        assert_eq!(title.interface.sets(), [28, 33]);
        assert_eq!((title.interface.entry(), title.interface.drawn(), title.take_wanted()), (Entry::Install, true, None));
        title.sounds.clear();
        // The menu sounds after seventy frames, and can be used after eighty-six:
        // the step it was told in was the first of them.
        let mut heard = Vec::new();
        for step in 2..=50 {
            title.step(&game.assets, &press(button::UP));
            heard.extend(title.sounds.drain(..).map(|sound| (step, sound)));
            if title.interface.cursor() != 4 {
                break;
            }
        }
        assert_eq!(heard, [(36, Request::common(0x11)), (46, Request::common(2))]);
        assert_eq!(title.interface.entry(), Entry::Options);
    }

    #[test]
    fn the_gallery_has_its_screen_made_and_the_tune_begins_again_if_it_played_anything() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.at_the_menu(&mut title);
        // GALLERY, the third entry.
        title.step(&game.assets, &press(button::DOWN));
        title.step(&game.assets, &press(button::DOWN));
        assert_eq!(title.interface.entry(), Entry::Gallery);
        title.step(&game.assets, &press(button::CROSS));
        game.until(&mut title, State::Screen);
        assert_eq!((title.take_wanted(), title.take_wanted()), (Some(Entry::Gallery), None));
        // When it is done with the menu is come back to, GALLERY lit; and the
        // tune is heard from its start again if the gallery says it played.
        title.sounds.clear();
        title.screen_done(&game.assets, true);
        assert_eq!((title.state, title.fade.running(), title.sounds.len()), (State::Menu, true, 0));
        title.tune_again();
        assert!(matches!(title.sounds[..], [Request::Music(_)]));
        assert_eq!(title.interface.sets(), [28, 34]);
    }

    #[test]
    fn options_has_its_screen_made_and_the_menu_comes_back_when_that_is_done_with() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.at_the_menu(&mut title);
        // OPTIONS is the fourth entry.
        title.step(&game.assets, &press(button::UP));
        title.step(&game.assets, &press(button::UP));
        assert_eq!(title.interface.entry(), Entry::Options);
        title.step(&game.assets, &press(button::CROSS));
        assert_eq!(title.take_wanted(), None);
        assert_eq!(game.until(&mut title, State::Screen), 12);
        assert_eq!((title.take_wanted(), title.take_wanted()), (Some(Entry::Options), None));
        // The title draws nothing of itself meanwhile, and waits.
        let mut canvas = Canvas::default();
        title.draw(&mut canvas, &game.assets, &Seen(Rect::new(0.0, 0.0, WIDTH, HEIGHT)), 0.0);
        assert_eq!(canvas.vertices().len(), 6);
        for _ in 0..100 {
            assert_eq!(title.step(&game.assets, &Pad::default()), None);
        }
        assert_eq!(title.state, State::Screen);
        // When the screen is done with, the fade comes off over fifteen frames
        // and the title is there at once, OPTIONS lit.
        title.screen_done(&game.assets, true);
        assert_eq!((title.state, title.fade.level, title.fade.running()), (State::Menu, 1.0, true));
        assert_eq!(title.interface.sets(), [28, 32]);
        for _ in 0..15 {
            title.step(&game.assets, &Pad::default());
        }
        assert!(!title.fade.running());
    }

    #[test]
    fn left_alone_for_a_minute_the_title_goes_dark_and_comes_again() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        game.until(&mut title, State::Menu);
        title.sounds.clear();
        // The idle frames are counted from when PRESS START can be answered:
        // sixty seconds of sixty, two a step.
        let mut steps = 0;
        while !title.film {
            title.step(&game.assets, &Pad::default());
            steps += 1;
            assert!(steps < 4000);
        }
        assert_eq!(steps, 12 + 1800);
        assert_eq!(std::mem::take(&mut title.sounds), [Request::StopMusic(8)]);
        assert!(title.rows.is_some());
        // Sixty-one frames on the film is made ready, and the title waits for it.
        assert_eq!(game.until(&mut title, State::FilmComes), 30);
        assert_eq!(title.showing, Some(FilmAsked::new("movie/BRS_OP_final.pmf".into())));
        // Its sound is to be heard in full, whatever the options have the music at.
        assert_eq!(std::mem::take(&mut title.sounds), [Request::MusicOptionFull(true)]);
        title.step(&game.assets, &Pad::default());
        assert_eq!(title.state, State::FilmComes);
        // Ready, it is started and the screen comes back over four frames.
        title.showing.as_mut().unwrap().ready = true;
        title.step(&game.assets, &Pad::default());
        assert!(title.state == State::Film && title.showing.as_ref().unwrap().started && title.rows.is_none() && title.fade.running());
        for _ in 0..40 {
            title.step(&game.assets, &Pad::default());
        }
        assert_eq!(title.state, State::Film);
        // Start ends it (as its playing through does): the screen goes dark over
        // four frames, the film is done with, and the title comes again.
        title.step(&game.assets, &press(button::START));
        assert_eq!(title.state, State::FilmGoes);
        assert_eq!(game.until(&mut title, State::AfterFilm), 2);
        assert_eq!(title.showing, None);
        game.until(&mut title, State::Menu);
        let tune = || Request::Music("SOUND/BGM_TITLE.AT3".into());
        assert_eq!(std::mem::take(&mut title.sounds), [Request::MusicOptionFull(false), tune(), tune()]);
        assert!(title.rows.is_none() && !title.film && title.fade.running());
        assert_eq!(title.interface.sets(), [4, 5]);
    }

    #[test]
    fn a_game_cleared_has_the_sky_another_tune_and_five_seconds_more() {
        let Some(game) = game() else { return };
        let mut title = game.title(true);
        game.until(&mut title, State::Menu);
        assert_eq!(title.sounds.last(), Some(&Request::Music("SOUND/BGM_FIELD4.AT3".into())));
        // The sky is the first place's ground: the metal's group has no track
        // playing set 2.
        let mut steps = 0;
        while !title.film {
            title.step(&game.assets, &Pad::default());
            steps += 1;
            assert!(steps < 4000);
        }
        assert_eq!(steps, 12 + 1950);
    }

    #[test]
    fn the_title_draws_the_mark_and_then_the_layout_with_two_groups_cut() {
        let Some(game) = game() else { return };
        let mut title = game.title(false);
        let seen = Seen(Rect::new(0.0, 0.0, WIDTH, HEIGHT));
        game.until(&mut title, State::Mark);
        let mut canvas = Canvas::default();
        title.draw(&mut canvas, &game.assets, &seen, 0.0);
        // The mark over the whole screen, white: its texture is 512 by 512 as
        // the PSP has it.
        let mark = canvas.drawn_from(&game.assets.mark);
        assert_eq!(mark.len(), 6);
        assert!(mark.iter().all(|corner| corner.colour == [255; 4]));
        assert!(mark.iter().any(|corner| corner.position == Vec2::new(WIDTH, HEIGHT) && corner.uv == Vec2::new(WIDTH / 512.0, HEIGHT / 512.0)));

        game.at_the_menu(&mut title);
        let mut canvas = Canvas::default();
        title.draw(&mut canvas, &game.assets, &seen, 0.0);
        let layout = &game.assets.interface.layout;
        // The ground of metal fills the screen.
        let ground = canvas.drawn_from(&layout.textures[0]);
        assert!(ground.iter().any(|corner| corner.position == Vec2::new(WIDTH, HEIGHT)));
        // The entries' words are drawn: five of them, LOAD GAME being two.
        let words = canvas.drawn_from(&layout.textures[7]);
        assert!(words.len() >= 6 * 6, "{}", words.len());
        assert!(canvas.drawn_from(&game.assets.mark).is_empty());
    }
}
