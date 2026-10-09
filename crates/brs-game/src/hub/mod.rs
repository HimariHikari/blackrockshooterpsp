//! The screen between missions (`menuStageRootTask`, 0x5D104), where the stage
//! and the mission to play next are chosen: a root of four entries (SAVE, LOAD,
//! the stage, the MENU), the list of the stage's missions with what is told of
//! each, and the messages that are due after a mission. See
//! `docs/STAGE_SELECT_NOTES.md`.
//!
//! Like the menus it is drawn sixty times a second (0x5D51C), so a frame of the
//! port's logic is two of its frames. Its comings and goings are a script's
//! (`interface.rs`). SAVE and LOAD ask for the list of saves (`take_wanted`),
//! which is the port's own, and wait for it; circle asks about going back to
//! the title.

pub mod interface;
mod scene;

pub use self::scene::plugin;

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Seen};
use brs_engine::layout::Layout;
use brs_formats::xtc;

use self::interface::{entry, Game, Ink, Interface, ENTRIES};
use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::field::hud::draw_window_on;
use crate::field::scene::{fade_kind, value, Fade, Scene, LEAVING_ELSEWHERE, LEAVING_RETRY, RIDDEN_THROUGH};
use crate::field::system::{self, SystemScript};
use crate::field::tables::FieldTables;
use crate::field::window::Window;
use crate::interface::layer;
use crate::items::{Inventory, Items};
use crate::menu::{Menu, STAGE_SEEN};
use crate::record_window::RecordWindow;
use crate::records::{self, Records};
use crate::sound::{self, Board, Request};
use crate::text::{self, TextColours};
use crate::writing::Fonts;

/// The screen's frames in a frame of the port's logic.
pub const FRAMES_A_STEP: usize = 2;

// Its sounds, of the bank that is always there: as an entry is chosen
// (0x5EA18), as a cursor moves (0x5EC90), and as the list of missions is left
// (0x5F00C).
const SOUND_CHOSEN: u32 = 3;
const SOUND_CURSOR: u32 = 2;
const SOUND_BACK: u32 = 5;

/// The tune it has, by its number in the game's list, which is played at full
/// volume from when the screen is made (0x5D028); and the blocks it stops over
/// as the screen is left (0x5E020).
const TUNE: i32 = 1;
const TUNE_STOPS_OVER: i32 = 4;

/// The frames the screen takes to come (0x5E19C, 0x5E3B8), and to go dark for
/// the title (0x5E8E8).
const COMES_OVER: i32 = 30;
const GOES_OVER: i32 = 30;

// The messages (`035.xtc`): what is asked of a mission, what is said of a
// stage's challenges, the two answers, the six texts about Free Hunt, and the
// two questions about the title.
mod message {
    pub const BEGIN: usize = 0;
    pub const CHALLENGES: usize = 1;
    pub const YES: usize = 2;
    pub const NO: usize = 3;
    pub const LESSON: usize = 4;
    pub const LESSON_PAGES: usize = 6;
    pub const TITLE: usize = 10;
    pub const UNSAVED: usize = 11;
    pub const COUNT: usize = 12;
}
/// The window's first answer.
const YES: usize = 0;
/// The flag that the texts about Free Hunt are due (0x5F3C0), and the frames
/// before the first of them comes.
const LESSON_DUE: i32 = 0x1cee;
const LESSON_AFTER: i32 = 45;
/// The sheet the messages' task puts over the screen while the records' windows
/// are shown (0x5DC48).
const SHEET: [u8; 4] = [0, 0, 0, 0x90];

/// What the screen is drawn from.
#[derive(Resource)]
pub struct HubAssets {
    pub(crate) interface: interface::Assets,
    /// The window it asks and tells with (the one the maps' scripts put up).
    window: Layout,
    messages: Vec<String>,
}

impl HubAssets {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<HubAssets> {
        let table = xtc::Table::parse(&data.read(crate::interface::file(interface::PACK, interface::MESSAGES, "xtc"))?).context("the stage select's messages")?;
        let mut messages = Vec::new();
        for index in 0..message::COUNT {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("the stage select has no message {index}"))?;
            messages.push(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned());
        }
        Ok(HubAssets {
            interface: interface::Assets::load(data, images).context("what the stage select is drawn from")?,
            window: crate::interface::layout(data, images, crate::field::hud::RESIDENT_PACK_WINDOW, 0)?,
            messages,
        })
    }

    /// The two answers its questions have.
    fn answers(&self) -> [&str; 2] {
        [&self.messages[message::YES], &self.messages[message::NO]]
    }
}

/// How the screen ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A mission was chosen, and the system script has begun it.
    Mission(i32),
    /// Back to the title.
    Title,
}

/// What SAVE and LOAD ask for: the list of saves, to save in or to load from
/// (0x4F40, 0x4FF8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    Save,
    Load,
}

/// The states of the root's task (0x5E134), by the numbers it has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Root {
    First,
    /// The screen comes, with nothing on it yet (1).
    Coming,
    /// Its files are asked for and waited for (2, 3 and 4): a frame each.
    Loading(u8),
    /// The interface is made (7).
    Making,
    /// It is the player's to use (9).
    Open,
    /// The MENU is made (0xA), and has the screen (0x10).
    ToMenu,
    Menu,
    /// Back from the MENU (4, then 8): the screen comes again.
    Back,
    /// The list of saves is up, to save (0xD, 0xE) or to load (0xB, 0xC).
    Saves,
    /// The screen goes dark (0x11), and that is waited for (0x12).
    Going,
    Gone,
    Done,
}

/// What is chosen from (the root's + 0x18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    First = 0,
    Entries = 1,
    ToMissions = 2,
    Missions = 3,
    ToEntries = 4,
    /// "Begin Mission?" is being answered.
    Asked = 5,
    /// A mission was chosen: until the interface has done with that.
    Chosen = 7,
    /// The question about the title is being answered.
    AskedTitle = 8,
    Leaving = 9,
}

/// The states of the messages' task (0x5F204).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Telling {
    Idle,
    /// The system script's `enter_stage_select` is started (1, 2), and runs (3).
    Entering,
    Script,
    /// The records of missions there are to show are counted (4, 5), and shown
    /// one after another (6).
    Records,
    Showing,
    /// The texts about Free Hunt, if they are due (8): after a wait (9), page
    /// by page (0xA).
    Lesson,
    LessonWait,
    LessonPages,
    /// The questions about the title (0xC, 0xD).
    Asking,
    Asked,
    /// A window that was put up is waited for (0xF).
    Window,
}

/// The task that tells and asks ("stage_select_message", 0x5DAD4).
struct Messages {
    state: Telling,
    window: Option<Window>,
    /// What the window last answered (0x6BCA8).
    answer: usize,
    /// The interface is not to have its frame (+ 0x20).
    busy: bool,
    shown: Option<records::Shown>,
    page: usize,
    wait: i32,
    question: usize,
}

/// What the screen works with and on.
pub struct Around<'a> {
    pub assets: &'a HubAssets,
    pub pad: &'a Pad,
    pub fonts: &'a Fonts,
    pub colours: &'a TextColours,
    pub system: &'a mut SystemScript,
    pub world: system::World<'a>,
    pub records: &'a Records,
    pub record_window: &'a mut RecordWindow,
    pub board: &'a mut Board,
    pub items: &'a Items,
    pub inventory: &'a mut Inventory,
    pub menu: &'a mut Menu,
    /// Windows are taken as read: there is no one to read them.
    pub skip_messages: bool,
}

impl Around<'_> {
    fn game(&mut self) -> Game<'_> {
        Game { progress: &mut *self.world.progress, tables: self.world.tables, fonts: self.fonts, colours: self.colours }
    }

    /// The pad as a window sees it: read for it when there is no one to.
    fn reader(&self) -> Pad {
        if self.skip_messages {
            Pad { pressed: button::CROSS, held: button::CROSS, ..Pad::default() }
        } else {
            Pad { pressed: self.pad.pressed, held: self.pad.held, repeat: self.pad.repeat, ..Pad::default() }
        }
    }
}

impl Messages {
    fn new() -> Messages {
        Messages { state: Telling::Idle, window: None, answer: 0, busy: false, shown: None, page: 0, wait: 0, question: 0 }
    }

    fn idle(&self) -> bool {
        self.state == Telling::Idle
    }

    /// Puts the window up with a text and its answers, if it has any (0x6BAFC,
    /// 0x6BE38): the cursor on one of them, and circle for the last.
    fn put_up(&mut self, words: &str, answers: &[&str], cursor: i32, sounds: &mut Vec<Request>) {
        let mut window = Window::new(text::pages(words, &text::Plain), answers, cursor, answers.len() as i32 - 1);
        sounds.extend(window.sounds.drain(..).map(Request::common));
        self.window = Some(window);
    }

    /// Says something in the window, and is busy until it has been read (0xF).
    fn say(&mut self, words: &str, sounds: &mut Vec<Request>) {
        self.put_up(words, &[], 0, sounds);
        self.state = Telling::Window;
    }

    /// Asks something with "Yes" and "No", the cursor on "Yes" (0x5DE44).
    fn ask(&mut self, assets: &HubAssets, words: &str, sounds: &mut Vec<Request>) {
        self.put_up(words, &assets.answers(), YES as i32, sounds);
        self.state = Telling::Window;
    }

    /// A frame of the task (0x5F204). The window, whose frames are the game's,
    /// has its own on the `first` of the frames a frame of the game's logic is.
    fn frame(&mut self, around: &mut Around, first: bool, sounds: &mut Vec<Request>) {
        if first {
            if let Some(window) = &mut self.window {
                let answer = window.step(&around.reader());
                sounds.extend(window.sounds.drain(..).map(Request::common));
                if let Some(answer) = answer {
                    (self.answer, self.window) = (answer, None);
                }
            }
        }
        let assets = around.assets;
        let no_window = self.window.is_none();
        match self.state {
            Telling::Idle => {}
            Telling::Entering => {
                if let Err(error) = around.system.begin("enter_stage_select", &[], &mut around.world) {
                    error!("the system script failed: {error:#}");
                }
                self.state = Telling::Script;
            }
            Telling::Script => {
                // What the script has to say is said in the window, and it
                // goes on when that has been read.
                if let Some(words) = around.world.scene.said.take() {
                    self.put_up(&words, &[], 0, sounds);
                    around.world.scene.saying = true;
                } else if no_window {
                    around.world.scene.saying = false;
                    match around.system.go_on(&mut around.world) {
                        Ok(true) => {}
                        Ok(false) => self.state = Telling::Records,
                        Err(error) => {
                            error!("the system script failed: {error:#}");
                            self.state = Telling::Records;
                        }
                    }
                }
            }
            Telling::Records => {
                let count = around.records.list(records::OF_MISSIONS, around.world.progress).len();
                if count > 0 {
                    self.busy = true;
                }
                self.shown = Some(records::Shown::begin(records::OF_MISSIONS, Some(count)));
                self.state = Telling::Showing;
            }
            Telling::Showing => {
                // The records' windows go by the game's frames.
                if !first {
                    return;
                }
                let stage = i32::from(around.world.progress.hub_stage);
                let mut world = records::World { progress: &mut *around.world.progress, stats: &mut *around.world.stats, stage };
                let mut screen = records::Screen {
                    window: &mut *around.record_window,
                    inventory: &mut *around.inventory,
                    items: around.items,
                    fonts: around.fonts,
                    colours: around.colours,
                    board: &mut *around.board,
                };
                let more = self.shown.as_mut().is_some_and(|shown| shown.step(around.records, &mut world, &mut screen));
                let pad = around.reader();
                around.record_window.step(&pad, around.board);
                if !more {
                    around.records.shown(records::OF_MISSIONS, around.world.progress);
                    around.record_window.shut();
                    (self.shown, self.busy) = (None, false);
                    self.state = Telling::Lesson;
                }
            }
            Telling::Lesson => {
                if around.world.progress.flag(LESSON_DUE) {
                    around.world.progress.set_flag(LESSON_DUE, false);
                    (self.page, self.wait) = (0, LESSON_AFTER);
                    self.state = Telling::LessonWait;
                } else {
                    self.state = Telling::Idle;
                }
            }
            Telling::LessonWait => {
                if self.wait > 0 {
                    self.wait -= 1;
                } else {
                    self.state = Telling::LessonPages;
                }
            }
            Telling::LessonPages if no_window => {
                if self.page < message::LESSON_PAGES {
                    self.put_up(&assets.messages[message::LESSON + self.page], &[], 0, sounds);
                    self.page += 1;
                } else {
                    self.state = Telling::Idle;
                }
            }
            // The cursor is on "No" for both questions.
            Telling::Asking => {
                self.put_up(&assets.messages[message::TITLE], &assets.answers(), 1, sounds);
                self.question = 0;
                self.state = Telling::Asked;
            }
            Telling::Asked if no_window => {
                if self.question == 0 && self.answer == YES {
                    self.put_up(&assets.messages[message::UNSAVED], &assets.answers(), 1, sounds);
                    self.question = 1;
                } else {
                    self.state = Telling::Idle;
                }
            }
            Telling::Window if no_window => self.state = Telling::Idle,
            Telling::LessonPages | Telling::Asked | Telling::Window => {}
        }
    }
}

/// The screen between missions.
pub struct Hub {
    root: Root,
    at: At,
    interface: Option<Interface>,
    messages: Messages,
    /// The screen's fade, which is the root's to start.
    fade: Fade,
    /// What is due after a mission is still to be told (flag 0x10), and the
    /// stages and missions are to be listed anew when it has been (0x20).
    to_tell: bool,
    to_relist: bool,
    /// The frames the screen takes to go dark as it is left (+ 0x54), and
    /// whether that is waited for (flag 0x400).
    goes_over: i32,
    waits_dark: bool,
    outcome: Option<Outcome>,
    /// The list of saves is asked for, until whoever makes it has taken that up.
    wanted: Option<Wanted>,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

impl Hub {
    pub fn new(tables: &FieldTables) -> Hub {
        let tune = tables.tune(TUNE).map(|tune| Request::Music(sound::tune_file(tune)));
        Hub {
            root: Root::First,
            at: At::First,
            interface: None,
            messages: Messages::new(),
            fade: Fade::DARK,
            to_tell: false,
            to_relist: false,
            goes_over: 0,
            waits_dark: false,
            outcome: None,
            wanted: None,
            sounds: tune.into_iter().chain([Request::MusicVolume(sound::FULL_VOLUME, 0.0)]).collect(),
        }
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// The MENU it made is done with (0x5D320): the screen comes again.
    pub fn menu_closed(&mut self) {
        if self.root == Root::Menu {
            self.root = Root::Back;
        }
    }

    /// The list of saves that SAVE or LOAD asks for, once.
    pub fn take_wanted(&mut self) -> Option<Wanted> {
        self.wanted.take()
    }

    /// The list of saves is done with and no game was loaded (0x5E5A0,
    /// 0x5E638): the entries are there again.
    pub fn saves_closed(&mut self) {
        if self.root == Root::Saves {
            self.root = Root::Open;
        }
    }

    /// A frame of the game's logic. Answers how the screen ended, once it has.
    pub fn step(&mut self, around: &mut Around) -> Option<Outcome> {
        for frame in 0..FRAMES_A_STEP {
            self.frame(around, frame == 0);
        }
        if let Some(interface) = &mut self.interface {
            self.sounds.append(&mut interface.sounds);
        }
        (self.root == Root::Done).then(|| self.outcome.unwrap_or(Outcome::Title))
    }

    /// A frame of the root's task (0x5E134), and then of the messages'.
    fn frame(&mut self, around: &mut Around, first: bool) {
        let assets = around.assets;
        match self.root {
            Root::First => {
                self.fade.begin(fade_kind::IN, COMES_OVER);
                self.root = Root::Coming;
            }
            Root::Coming => {
                if !self.fade.running() {
                    self.root = Root::Loading(2);
                }
            }
            Root::Loading(state) => self.root = if state < 4 { Root::Loading(state + 1) } else { Root::Making },
            // The interface is made (0x5D670), told to come, and its cursor
            // put on the stage.
            Root::Making => {
                (self.at, self.to_tell) = (At::Entries, true);
                let mut game = around.game();
                let mut interface = Interface::new(&assets.interface, &mut game);
                interface.open(&assets.interface, &mut game);
                self.interface = Some(interface);
                self.root = Root::Open;
            }
            Root::Back => {
                self.fade.begin(fade_kind::IN, COMES_OVER);
                self.root = Root::Open;
            }
            Root::Open => {
                if !self.fade.running() {
                    self.choose(around, first);
                    if self.at == At::Leaving {
                        self.root = Root::Going;
                    }
                }
            }
            Root::ToMenu => {
                around.menu.open();
                self.root = Root::Menu;
            }
            Root::Menu | Root::Saves => {}
            Root::Going => {
                self.fade.begin(fade_kind::OUT, self.goes_over);
                self.root = Root::Gone;
            }
            Root::Gone => {
                if !(self.waits_dark && self.fade.running()) {
                    self.sounds.push(Request::StopMusic(TUNE_STOPS_OVER));
                    self.root = Root::Done;
                }
            }
            Root::Done => {}
        }
        // The interface has its frame while the root is in one of these states,
        // unless the messages' task is showing the records.
        if let Some(interface) = &mut self.interface {
            if !self.messages.busy && matches!(self.root, Root::Coming | Root::Open | Root::ToMenu | Root::Going) {
                interface.frame(&assets.interface);
            }
        }
        let mut sounds = std::mem::take(&mut self.sounds);
        self.messages.frame(around, first, &mut sounds);
        self.sounds = sounds;
        self.fade.advance();
    }

    /// A frame of what is chosen from (0x5E764).
    fn choose(&mut self, around: &mut Around, first: bool) {
        let assets = around.assets;
        let Some(interface) = &mut self.interface else { return };
        // The first time the interface is ready, what is due after a mission
        // is told; and when that is done everything is listed anew.
        if self.to_tell && interface.ready() {
            self.messages.state = Telling::Entering;
            (self.to_tell, self.to_relist) = (false, true);
        }
        if !self.messages.idle() {
            return;
        }
        if std::mem::take(&mut self.to_relist) {
            interface.relist(&mut around.game());
        }
        match self.at {
            At::First => self.at = At::Entries,
            At::Entries => self.at_the_entries(around, first),
            At::ToMissions => self.at = At::Missions,
            At::Missions => self.at_the_missions(around, first),
            At::ToEntries => self.at = At::Entries,
            At::Asked => {
                if self.messages.answer == YES {
                    let Some(mission) = interface.chosen().map(|mission| mission.number) else {
                        self.at = At::Missions;
                        return;
                    };
                    interface.close(&assets.interface, &mut around.game());
                    // The system script begins the mission (0x5DD6C).
                    if let Err(error) = around.system.call("mission_select", &[mission as u32], &mut around.world) {
                        error!("the system script failed: {error:#}");
                    }
                    self.outcome = Some(Outcome::Mission(mission));
                    self.at = At::Chosen;
                } else {
                    self.at = At::Missions;
                }
            }
            // Once the script has done with it the screen is left as it is:
            // the wipe has covered it (0x5D6EC).
            At::Chosen => {
                if !interface.busy() {
                    (self.goes_over, self.waits_dark) = (0, false);
                    self.at = At::Leaving;
                }
            }
            At::AskedTitle => {
                if self.messages.answer == YES {
                    (self.goes_over, self.waits_dark) = (GOES_OVER, true);
                    self.outcome = Some(Outcome::Title);
                    self.at = At::Leaving;
                } else {
                    self.at = At::Entries;
                }
            }
            At::Leaving => {}
        }
    }

    /// What the pad does at the root's entries (0x5E918).
    fn at_the_entries(&mut self, around: &mut Around, first: bool) {
        let assets = &around.assets.interface;
        let (held, pressed, repeat) = (around.pad.held, if first { around.pad.pressed } else { 0 }, if first { around.pad.repeat } else { 0 });
        let Some(interface) = &mut self.interface else { return };
        if !interface.ready() {
            return;
        }
        if pressed & button::CIRCLE != 0 {
            self.messages.state = Telling::Asking;
            self.at = At::AskedTitle;
        } else if pressed & button::CROSS != 0 {
            match interface.cursor() {
                entry::SAVE | entry::LOAD => {
                    self.wanted = Some(if interface.cursor() == entry::SAVE { Wanted::Save } else { Wanted::Load });
                    self.root = Root::Saves;
                    self.sound(SOUND_CHOSEN);
                }
                entry::STAGE => {
                    if !interface.busy() {
                        interface.to_missions(assets, &around.game());
                        self.at = At::ToMissions;
                        self.sound(SOUND_CHOSEN);
                    }
                }
                _ => {
                    self.root = Root::ToMenu;
                    self.sound(SOUND_CHOSEN);
                }
            }
        } else if held & (button::UP | button::DOWN) != 0 {
            // A step each time the button counts as pressed, and round the
            // ends only when it has newly gone down.
            let was = interface.cursor();
            let to = if held & button::UP != 0 {
                if was == 0 { (pressed & button::UP != 0).then_some(ENTRIES - 1) } else { (repeat & button::UP != 0).then(|| was - 1) }
            } else if was == ENTRIES - 1 {
                (pressed & button::DOWN != 0).then_some(0)
            } else {
                (repeat & button::DOWN != 0).then_some(was + 1)
            };
            if let Some(to) = to {
                interface.set_cursor(assets, &around.game(), to);
            }
            if interface.cursor() != was {
                self.sound(SOUND_CURSOR);
            }
        } else if held & (button::LEFT | button::RIGHT) != 0 && interface.cursor() == entry::STAGE {
            let (was, last) = (interface.stage(), interface.stage_count().saturating_sub(1));
            let on = if held & button::LEFT != 0 {
                (if was == 0 { pressed } else { repeat } & button::LEFT != 0).then_some(false)
            } else {
                (if was == last { pressed } else { repeat } & button::RIGHT != 0).then_some(true)
            };
            if let Some(on) = on {
                interface.change_stage(assets, on);
            }
            if interface.stage() != was {
                self.sound(SOUND_CURSOR);
            }
        }
    }

    /// What the pad does at the stage's missions (0x5EE64).
    fn at_the_missions(&mut self, around: &mut Around, first: bool) {
        let assets = around.assets;
        let (held, pressed, repeat) = (around.pad.held, if first { around.pad.pressed } else { 0 }, if first { around.pad.repeat } else { 0 });
        let Some(interface) = &mut self.interface else { return };
        if !interface.ready() {
            return;
        }
        // A stage counts as seen from when its missions are first shown: its
        // challenges can be looked at from then on, which all but the first
        // stage's are told of.
        let stage = interface.stage();
        let seen = STAGE_SEEN + stage as i32;
        if !around.world.progress.flag(seen) {
            if stage != 0 {
                let words = assets.messages[message::CHALLENGES].replace("%d", &(stage + 1).to_string());
                self.messages.say(&words, &mut self.sounds);
            }
            around.world.progress.set_flag(seen, true);
            return;
        }
        if pressed & button::CROSS != 0 {
            if let Some(mission) = interface.chosen().filter(|mission| mission.open) {
                let words = assets.messages[message::BEGIN].replace("%s", &mission.title);
                self.messages.ask(assets, &words, &mut self.sounds);
                self.at = At::Asked;
            }
        } else if pressed & button::CIRCLE != 0 {
            interface.to_root(&assets.interface, &around.game());
            self.at = At::ToEntries;
            self.sound(SOUND_BACK);
        } else if held & (button::UP | button::DOWN) != 0 {
            let (was, count) = (interface.mission_cursor() as i32, interface.mission_count() as i32);
            let to = if held & button::UP != 0 {
                if was == 0 { (pressed & button::UP != 0).then_some(count - 1) } else { (repeat & button::UP != 0).then_some(was - 1) }
            } else if was == count - 1 {
                (pressed & button::DOWN != 0).then_some(0)
            } else {
                (repeat & button::DOWN != 0).then_some(was + 1)
            };
            if let Some(to) = to {
                interface.set_mission(&assets.interface, &around.game(), to);
            }
            if interface.mission_cursor() as i32 != was {
                self.sound(SOUND_CURSOR);
            }
        }
    }

    /// What the scene her model stands in shows, once there is one.
    pub fn scene(&self) -> Option<interface::Scene> {
        self.interface.as_ref().map(Interface::scene)
    }

    /// Draws the screen as it is `ahead` of a frame of the game's logic past
    /// its last one (0x5D760, 0x5DBFC): the interface, with the scene her
    /// model stands in if its camera has drawn it; what the messages' task
    /// has up; and the screen's fade over all of it.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, canvas: &mut Canvas, assets: &HubAssets, fonts: &Fonts, colours: &TextColours, seen: &Seen, scene: bool, ahead: f32) {
        // Nothing of the field is drawn under it.
        canvas.fill(layer::HUB, seen.0, [0, 0, 0, 255]);
        if let Some(interface) = &self.interface {
            interface.draw(canvas, &assets.interface, &Ink { fonts, colours }, scene, ahead * FRAMES_A_STEP as f32);
        }
        if self.messages.busy {
            canvas.fill(layer::HUB_MESSAGES, seen.0, SHEET);
        }
        if let Some(window) = &self.messages.window {
            draw_window_on(canvas, layer::WINDOW, &assets.window, fonts, colours, window, ahead);
        }
        let dark = (self.fade.level.clamp(0.0, 1.0) * 255.0) as u8;
        if dark != 0 {
            canvas.fill(layer::FADE, seen.0, [0, 0, 0, dark]);
        }
    }
}

/// What follows the field being left (`s173`, 0xE4B40): the system script is told
/// how it was left; a mission that is tried again, or a field left for the ride
/// on the bike, starts over at once on the map and entrance the scripts have
/// named, which is answered; otherwise the screen between missions is to come.
///
/// The ride itself (0xE6F8C: the task `BikeMiniGame`, `crate::bike`) is the
/// field's to have ridden before it starts over: until it has been, it counts
/// as ridden through, which is what the map it leads to goes by.
pub fn left(how: i32, system: &mut SystemScript, world: &mut system::World) -> Result<Option<(i32, i32)>> {
    system.call("sys_fld_exit_field", &[how as u32], world)?;
    if how == LEAVING_ELSEWHERE {
        world.scene.work.ride_outcome = RIDDEN_THROUGH;
    }
    Ok(match how {
        LEAVING_RETRY => Some(named(world.scene, value::MAP, value::ENTRANCE)),
        LEAVING_ELSEWHERE => Some(named(world.scene, value::NEXT_MAP, value::NEXT_ENTRANCE)),
        _ => None,
    })
}

/// Where the scripts have said to go next: it is kept as two of the field's
/// figures.
pub fn named(scene: &Scene, map: i32, entrance: i32) -> (i32, i32) {
    (scene.values.get(&map).copied().unwrap_or(scene.map), scene.values.get(&entrance).copied().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::tables::PlayerStats;
    use crate::field::host::defaults;
    use crate::progress::{variable, Progress};
    use brs_engine::canvas::Vertex;
    use brs_engine::collision::CollisionWorld;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");
    /// The flags that the second stage is there, and that it has been told of.
    const SECOND_STAGE: [i32; 2] = [0x17f4, 0x17f5];

    struct Game {
        assets: HubAssets,
        fonts: Fonts,
        colours: TextColours,
        tables: FieldTables,
        system: SystemScript,
        scene: Scene,
        progress: Progress,
        stats: PlayerStats,
        ground: CollisionWorld,
        records: Records,
        record_window: RecordWindow,
        board: Board,
        items: Items,
        inventory: Inventory,
        menu: Menu,
        /// Windows are taken as read.
        read: bool,
    }

    /// The game as it stands when the first mission has been cleared and the
    /// field left, with some flags set besides.
    fn game(flags: &[i32]) -> Option<Game> {
        let data = GameData::new(DATA).ok()?;
        let mut images = Assets::<Image>::default();
        let tables = FieldTables::load(&data).unwrap();
        let mut system = SystemScript::load(&data, &tables).unwrap();
        let mut scene = Scene::new(0, 0, defaults(&tables).unwrap());
        let mut progress = Progress::start();
        let mut stats = PlayerStats::new_game(&data).unwrap();
        let ground = CollisionWorld::default();
        let items = Items::load(&data).unwrap();
        {
            let mut world = system::World { progress: &mut progress, scene: &mut scene, stats: &mut stats, tables: &tables, ground: &ground };
            system.call("init_player", &[], &mut world).unwrap();
            world.progress.begin_first_mission();
            world.progress.set_variable(variable::MISSION_PROGRESS, 200);
            for &flag in flags {
                world.progress.set_flag(flag, true);
            }
            assert_eq!(left(1, &mut system, &mut world).unwrap(), None);
        }
        Some(Game {
            assets: HubAssets::load(&data, &mut images).unwrap(),
            fonts: Fonts::load(&data, &mut images).unwrap(),
            colours: TextColours::load(&data).unwrap(),
            records: Records::load(&data).unwrap(),
            record_window: RecordWindow::load(&data, &mut images).unwrap(),
            board: Board::default(),
            inventory: Inventory::new_game(&items),
            items,
            menu: Menu::default(),
            tables,
            system,
            scene,
            progress,
            stats,
            ground,
            read: false,
        })
    }

    impl Game {
        /// A frame of the game's logic with the screen up.
        fn step(&mut self, hub: &mut Hub, pad: &Pad) -> Option<Outcome> {
            let mut around = Around {
                assets: &self.assets,
                pad,
                fonts: &self.fonts,
                colours: &self.colours,
                system: &mut self.system,
                world: system::World { progress: &mut self.progress, scene: &mut self.scene, stats: &mut self.stats, tables: &self.tables, ground: &self.ground },
                records: &self.records,
                record_window: &mut self.record_window,
                board: &mut self.board,
                items: &self.items,
                inventory: &mut self.inventory,
                menu: &mut self.menu,
                skip_messages: self.read,
            };
            hub.step(&mut around)
        }

        /// Steps with nothing pressed until something is so; answers how many.
        fn until(&mut self, hub: &mut Hub, so: impl Fn(&Hub) -> bool) -> usize {
            let mut steps = 0;
            while !so(hub) {
                assert_eq!(self.step(hub, &Pad::default()), None);
                steps += 1;
                assert!(steps < 2000, "it never came to that");
            }
            steps
        }

        /// The screen come, with what was due told and read: the root's entries
        /// are the player's to use.
        fn come(&mut self) -> Hub {
            let mut hub = Hub::new(&self.tables);
            self.read = true;
            self.until(&mut hub, |hub| hub.at == At::Entries && !hub.to_tell && !hub.to_relist && hub.messages.idle() && hub.interface.as_ref().is_some_and(Interface::ready));
            self.read = false;
            hub.sounds.clear();
            hub
        }
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, repeat: buttons, ..Pad::default() }
    }

    fn held(buttons: u32, again: u32) -> Pad {
        Pad { held: buttons, repeat: again, ..Pad::default() }
    }

    /// The sounds of the bank that is always there among what was asked for.
    fn common(sounds: &mut Vec<Request>) -> Vec<u32> {
        let sound = |request: Request| match request {
            Request::Play { sound, .. } => Some(sound),
            _ => None,
        };
        sounds.drain(..).filter_map(sound).collect()
    }

    fn interface(hub: &Hub) -> &Interface {
        hub.interface.as_ref().expect("the interface is made")
    }

    /// Has the window that is up answered: a button is pressed until its cursor
    /// is on an answer, which is once it takes the pad, and then cross until it
    /// has gone, or another has taken its place.
    fn answer(game: &mut Game, hub: &mut Hub, with: u32, choice: usize) {
        let selected = |hub: &Hub| hub.messages.window.as_ref().map(|window| window.selected);
        let mut steps = 0;
        while selected(hub).is_some_and(|at| at != choice) {
            game.step(hub, &press(with));
            steps += 1;
            assert!(steps < 200, "the window never took the pad");
        }
        let asked = window(hub);
        while asked.is_some() && window(hub) == asked {
            game.step(hub, &press(button::CROSS));
            steps += 1;
            assert!(steps < 400, "the window never went");
        }
    }

    /// What the window says, if one is up.
    fn window(hub: &Hub) -> Option<String> {
        hub.messages.window.as_ref().map(|window| text::plain(window.page()))
    }

    #[test]
    fn it_comes_dark_and_its_script_brings_it_in_before_anything_is_told() {
        let Some(mut game) = game(&[]) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // It has its tune from when it is made.
        let mut hub = Hub::new(&game.tables);
        assert_eq!(std::mem::take(&mut hub.sounds), [Request::Music("SOUND/BGM_SYSTEM1.AT3".into()), Request::MusicVolume(127, 0.0)]);
        // The screen comes over thirty of its frames with nothing on it, its
        // files take three more, and in the next the interface is made: the
        // eighteenth frame of the game's logic.
        assert_eq!(game.until(&mut hub, |hub| hub.interface.is_some()), 18);
        assert_eq!((hub.root, hub.at), (Root::Open, At::Entries));
        // The script has the three places play their sets, the root's cursor is
        // on the stage, and the strip says what that entry is.
        let shown = interface(&hub);
        assert_eq!(shown.places_sets(), [3, 14, 0]);
        assert_eq!((shown.cursor(), shown.stage(), shown.stage_count(), shown.ready()), (entry::STAGE, 0, 1, false));
        assert_eq!(shown.strip_words(), "Select a stage.");
        // Its sound comes a hundred frames on, and it is ready when the second
        // place has played its set through.
        let steps = game.until(&mut hub, |hub| !hub.sounds.is_empty());
        assert_eq!((steps, common(&mut hub.sounds)), (49, vec![0x11]));
        let steps = game.until(&mut hub, |hub| interface(hub).ready());
        assert_eq!(steps, 10);
        // Then what is due is told: the system script closes the mission that
        // was cleared and opens the next, which it says in the window.
        assert!(hub.to_tell && hub.messages.idle());
        game.until(&mut hub, |hub| hub.messages.window.is_some());
        assert_eq!(window(&hub).as_deref(), Some("Mission: Generator Destruction\nhas been unlocked."));
        assert_eq!((game.progress.variable(100), game.progress.variable(101)), (255, 1));
        // Nothing can be chosen until it has been read.
        for _ in 0..40 {
            game.step(&mut hub, &press(button::DOWN));
        }
        assert_eq!(interface(&hub).cursor(), entry::STAGE);
        game.step(&mut hub, &press(button::CROSS));
        game.until(&mut hub, |hub| hub.messages.idle() && !hub.to_relist);
        assert_eq!((game.progress.variable(100), game.progress.variable(101), game.progress.variable(variable::SCENARIO)), (255, 2, -1));
        // And the stage's missions are counted anew: one cleared of two open.
        let stage = &interface(&hub).stages[0];
        assert_eq!((stage.there, stage.cleared, stage.open, stage.mark), (true, 1, 2, 0));
    }

    #[test]
    fn the_roots_cursor_goes_round_its_entries_and_left_and_right_change_the_stage() {
        let Some(mut game) = game(&SECOND_STAGE) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut hub = game.come();
        let at = |hub: &Hub| (interface(hub).cursor(), interface(hub).strip_words().to_owned());
        assert_eq!(at(&hub), (entry::STAGE, "Select a stage.".to_owned()));
        game.step(&mut hub, &press(button::DOWN));
        assert_eq!(at(&hub), (entry::MENU, "Open the Main Menu.".to_owned()));
        // Held, down comes again but does not go round; a new press does.
        game.step(&mut hub, &held(button::DOWN, button::DOWN));
        assert_eq!(at(&hub).0, entry::MENU);
        game.step(&mut hub, &press(button::DOWN));
        assert_eq!(at(&hub), (entry::SAVE, "Save game data.".to_owned()));
        game.step(&mut hub, &held(button::UP, button::UP));
        assert_eq!(at(&hub).0, entry::SAVE);
        game.step(&mut hub, &press(button::UP));
        assert_eq!(at(&hub).0, entry::MENU);
        // Left and right do nothing but on the stage's entry.
        game.step(&mut hub, &press(button::RIGHT));
        assert_eq!(interface(&hub).stage(), 0);
        assert_eq!(common(&mut hub.sounds), [SOUND_CURSOR; 3]);
        game.step(&mut hub, &press(button::UP));
        assert_eq!(at(&hub).0, entry::STAGE);
        hub.sounds.clear();

        // There are two stages: right shows the next, which the script plays.
        assert_eq!(interface(&hub).stage_count(), 2);
        game.step(&mut hub, &press(button::RIGHT));
        assert_eq!((interface(&hub).stage(), interface(&hub).stage_was()), (1, 0));
        assert_eq!(interface(&hub).places_sets()[..2], [9, 10]);
        // Held at the last it stays; a new press goes round to the first.
        game.step(&mut hub, &held(button::RIGHT, button::RIGHT));
        assert_eq!(interface(&hub).stage(), 1);
        game.step(&mut hub, &press(button::RIGHT));
        assert_eq!((interface(&hub).stage(), interface(&hub).stage_was()), (0, 1));
        game.step(&mut hub, &press(button::LEFT));
        assert_eq!(interface(&hub).stage(), 1);
        assert_eq!(interface(&hub).places_sets()[..2], [11, 12]);
        assert_eq!(common(&mut hub.sounds), [SOUND_CURSOR; 3]);
        // LOAD asks for the list of saves, and the screen waits for it: the pad
        // is not its own until that is done with.
        game.step(&mut hub, &press(button::UP));
        assert_eq!(hub.take_wanted(), None);
        game.step(&mut hub, &press(button::CROSS));
        assert_eq!(common(&mut hub.sounds), [SOUND_CURSOR, SOUND_CHOSEN]);
        assert_eq!((hub.take_wanted(), hub.take_wanted(), hub.root), (Some(Wanted::Load), None, Root::Saves));
        game.step(&mut hub, &press(button::UP));
        game.step(&mut hub, &press(button::CIRCLE));
        assert_eq!((at(&hub).0, hub.root, hub.messages.idle()), (entry::LOAD, Root::Saves, true));
        // With no game loaded the entries are there again; SAVE likewise.
        hub.saves_closed();
        assert_eq!(hub.root, Root::Open);
        game.step(&mut hub, &press(button::UP));
        game.step(&mut hub, &press(button::CROSS));
        assert_eq!((at(&hub).0, hub.take_wanted(), hub.root), (entry::SAVE, Some(Wanted::Save), Root::Saves));
    }

    #[test]
    fn a_stage_newly_there_is_gone_to_as_the_screen_comes() {
        let Some(mut game) = game(&SECOND_STAGE[..1]) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(game.progress.hub_stage, 0);
        let hub = game.come();
        // The script shows the way from the stage a mission was last chosen of
        // to the next, which is the one to come back to from now on; and the
        // stage has been told of.
        assert_eq!((interface(&hub).stage(), interface(&hub).stage_was(), interface(&hub).stage_count()), (1, 0, 2));
        assert_eq!(game.progress.hub_stage, 1);
        assert!(game.progress.flag(SECOND_STAGE[1]));
        let mut again = game.come();
        assert_eq!((interface(&again).stage(), interface(&again).stage_was()), (1, 0));
        assert_eq!(interface(&again).places_sets()[0], 3);
        again.sounds.clear();
    }

    #[test]
    fn a_stages_missions_are_listed_and_one_of_them_is_begun() {
        let Some(mut game) = game(&[]) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut hub = game.come();
        game.step(&mut hub, &press(button::CROSS));
        assert_eq!(common(&mut hub.sounds), [SOUND_CHOSEN]);
        // The script has the list come, and has the world behind it stand still
        // as a picture; the strip says what is to be done.
        let shown = interface(&hub);
        assert_eq!((shown.places_sets(), shown.ready()), ([21, 18, 0], false));
        assert_eq!(shown.strip_words(), "Select a mission.");
        // The cursor is on the first mission that has not been begun.
        assert_eq!((shown.mission_count(), shown.mission_cursor()), (2, 1));
        assert_eq!(shown.chosen().map(|mission| (mission.number, mission.title.as_str(), mission.kind, mission.from, mission.come_to, mission.level)), Some((1, "Generator Destruction", 0, 1, 1, 4)));
        assert!(!game.progress.flag(STAGE_SEEN));
        let steps = game.until(&mut hub, |hub| interface(hub).ready());
        assert_eq!(steps, 18);
        assert_eq!(interface(&hub).modes(), [2, 1, 0]);
        assert_eq!(common(&mut hub.sounds), [0x11]);
        // Its first frame at the missions has the stage count as seen, and the
        // first stage's is not told of.
        game.step(&mut hub, &Pad::default());
        assert!(game.progress.flag(STAGE_SEEN) && hub.messages.idle());

        // Up and down move the cursor, round the ends only on a new press.
        game.step(&mut hub, &held(button::DOWN, button::DOWN));
        assert_eq!(interface(&hub).mission_cursor(), 1);
        game.step(&mut hub, &press(button::DOWN));
        assert_eq!(interface(&hub).mission_cursor(), 0);
        game.step(&mut hub, &press(button::UP));
        assert_eq!(interface(&hub).mission_cursor(), 1);
        assert_eq!(common(&mut hub.sounds), [SOUND_CURSOR; 2]);

        // Cross asks, with the cursor on "Yes"; "No" changes nothing.
        game.step(&mut hub, &press(button::CROSS));
        assert_eq!(window(&hub).as_deref(), Some("Begin Mission \u{201c}Generator Destruction\u{201d}?"));
        let asked = hub.messages.window.as_ref().unwrap();
        assert_eq!((asked.choices.clone(), asked.selected), (vec!["Yes".to_owned(), "No".to_owned()], 0));
        // Circle puts the cursor on "No".
        answer(&mut game, &mut hub, button::CIRCLE, 1);
        game.until(&mut hub, |hub| hub.messages.idle());
        game.step(&mut hub, &Pad::default());
        assert_eq!((hub.at, hub.outcome), (At::Missions, None));

        // "Yes" has the system script begin the mission and the screen's script
        // cover it; and when that is through the screen is done with.
        game.step(&mut hub, &press(button::CROSS));
        assert!(hub.messages.window.is_some());
        answer(&mut game, &mut hub, button::UP, 0);
        game.until(&mut hub, |hub| hub.at == At::Chosen);
        assert_eq!(hub.outcome, Some(Outcome::Mission(1)));
        assert_eq!((game.progress.variable(variable::SCENARIO), game.progress.variable(101)), (1, 10));
        assert!(interface(&hub).busy());
        let mut steps = 0;
        let outcome = loop {
            steps += 1;
            assert!(steps < 400);
            if let Some(outcome) = game.step(&mut hub, &press(button::CIRCLE)) {
                break outcome;
            }
        };
        assert_eq!(outcome, Outcome::Mission(1));
        // Its tune stops as it is left.
        assert_eq!(hub.sounds.last(), Some(&Request::StopMusic(4)));
        assert_eq!(interface(&hub).places_sets(), [15, 16, 2]);
        assert_eq!(named(&game.scene, value::MAP, value::ENTRANCE), (0, 0));
    }

    #[test]
    fn circle_at_the_missions_goes_back_and_they_are_where_they_were_left() {
        let Some(mut game) = game(&[]) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut hub = game.come();
        game.step(&mut hub, &press(button::CROSS));
        game.until(&mut hub, |hub| interface(hub).ready());
        game.step(&mut hub, &Pad::default());
        game.step(&mut hub, &press(button::UP));
        assert_eq!(interface(&hub).mission_cursor(), 0);
        hub.sounds.clear();
        game.step(&mut hub, &press(button::CIRCLE));
        assert_eq!(common(&mut hub.sounds), [SOUND_BACK]);
        // The world is drawn as it is again, and the strip says the entry's text.
        assert_eq!((interface(&hub).places_sets(), interface(&hub).modes()), ([22, 20, 0], [1, 1, 0]));
        assert_eq!(interface(&hub).strip_words(), "Select a stage.");
        game.until(&mut hub, |hub| interface(hub).ready() && hub.at == At::Entries);
        game.step(&mut hub, &press(button::CROSS));
        assert_eq!(interface(&hub).mission_cursor(), 0);
    }

    #[test]
    fn circle_asks_twice_before_the_title() {
        let Some(mut game) = game(&[]) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut hub = game.come();
        // The cursor is on "No" as each question is asked.
        let on_no = |hub: &Hub| hub.messages.window.as_ref().map(|window| window.selected) == Some(1);
        game.step(&mut hub, &press(button::CIRCLE));
        game.until(&mut hub, |hub| hub.messages.window.is_some());
        assert_eq!(window(&hub).as_deref(), Some("Return to Title screen.\nAre you sure?"));
        assert!(on_no(&hub));
        answer(&mut game, &mut hub, button::UP, 1);
        game.until(&mut hub, |hub| hub.messages.idle());
        game.step(&mut hub, &Pad::default());
        assert_eq!((hub.at, hub.root), (At::Entries, Root::Open));

        game.step(&mut hub, &press(button::CIRCLE));
        answer(&mut game, &mut hub, button::UP, 0);
        game.until(&mut hub, |hub| window(hub).as_deref() == Some("Unsaved data will be lost.\nIs this okay?"));
        assert!(on_no(&hub));
        answer(&mut game, &mut hub, button::UP, 0);
        // The screen goes dark over thirty of its frames, and that is the end.
        game.until(&mut hub, |hub| hub.root == Root::Gone);
        let mut steps = 0;
        let outcome = loop {
            steps += 1;
            assert!(steps < 100);
            if let Some(outcome) = game.step(&mut hub, &Pad::default()) {
                break outcome;
            }
        };
        assert_eq!(outcome, Outcome::Title);
        assert!((14..=15).contains(&steps), "{steps}");
    }

    #[test]
    fn the_worlds_map_has_the_stage_shown_at_its_middle_and_the_others_marks_on_it() {
        let Some(mut game) = game(&SECOND_STAGE) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut hub = game.come();
        let middle = Vec2::new(269.0, 127.0);
        // Where the things drawn from a texture have their top left corners.
        fn drawn(game: &Game, hub: &Hub, texture: &Handle<Image>) -> Vec<Vec2> {
            let mut canvas = Canvas::default();
            hub.draw(&mut canvas, &game.assets, &game.fonts, &game.colours, &Seen::default(), false, 0.0);
            let corners: Vec<Vertex> = canvas.drawn_from(texture);
            corners.chunks(6).map(|sprite| sprite.iter().fold(sprite[0].position, |at, corner| at.min(corner.position))).collect()
        }
        // The first two tiles of the map say how big it is drawn; San Francisco,
        // which is at (107, 203) of the map, is at the middle.
        let tiles = game.assets.interface.tiles().to_vec();
        let (first, second) = (drawn(&game, &hub, &tiles[0])[0], drawn(&game, &hub, &tiles[1])[0]);
        let scale = (second.x - first.x) / 128.0;
        assert!(scale > 0.5 && first.y == second.y, "{first} {second}");
        assert!(first.distance(middle - Vec2::new(107.0, 203.0) * scale) < 0.01, "{first}");
        assert_eq!(interface(&hub).world_now(), [107, 203]);
        // New York's mark is where New York is on the map: (178, 201).
        let layout = &game.assets.interface.layout;
        let mark = layout.objects.objects[22].quad.as_ref().unwrap().corners.iter().fold(Vec2::MAX, |at, corner| at.min(Vec2::from(corner.position)));
        let new_york = middle + Vec2::new(178.0 - 107.0, 201.0 - 203.0) * scale + mark;
        let marks = drawn(&game, &hub, &layout.textures[13]);
        assert!(marks.iter().any(|at| at.distance(new_york) < 0.01), "{new_york}: {marks:?}");

        // Right has the map go to New York over fifteen frames, in whole pixels
        // of it.
        game.step(&mut hub, &press(button::RIGHT));
        assert_eq!(interface(&hub).world_now(), [116, 202]);
        for _ in 0..6 {
            game.step(&mut hub, &Pad::default());
        }
        assert_eq!(interface(&hub).world_now(), [173, 201]);
        game.step(&mut hub, &Pad::default());
        assert_eq!(interface(&hub).world_now(), [178, 201]);
        game.until(&mut hub, |hub| interface(hub).ready());
        let first = drawn(&game, &hub, &tiles[0])[0];
        assert!(first.distance(middle - Vec2::new(178.0, 201.0) * scale) < 0.01, "{first}");
    }

    #[test]
    fn what_is_told_of_a_mission_is_drawn_where_the_layout_has_it() {
        let Some(mut game) = game(&[]) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut hub = game.come();
        game.step(&mut hub, &press(button::CROSS));
        game.until(&mut hub, |hub| interface(hub).ready());
        let mut canvas = Canvas::default();
        hub.draw(&mut canvas, &game.assets, &game.fonts, &game.colours, &Seen::default(), false, 0.0);
        let layout = &game.assets.interface.layout;
        // Where what is drawn from a texture has its corners, and from where in it.
        let drawn = |texture: usize| -> Vec<(Vec2, Vec2)> {
            let corners: Vec<Vertex> = canvas.drawn_from(&layout.textures[texture]);
            corners.chunks(6).map(|sprite| sprite.iter().fold((sprite[0].position, sprite[0].uv), |(at, from), corner| (at.min(corner.position), from.min(corner.uv)))).collect()
        };
        // Who it is from: the second of the faces, which stand five to a row
        // forty-eight apart from the texture's second column.
        assert_eq!(drawn(17), [(Vec2::new(25.0, 160.0), Vec2::new(49.0 / 256.0, 0.0))]);
        // The marks of the two rows, twenty apart: a star for the mission that
        // is cleared, and the mark of one not begun. (The layout draws things of
        // its own from the texture as well.)
        let marks = drawn(13);
        assert!(marks.contains(&(Vec2::new(27.0, 33.0), Vec2::new(17.0 / 128.0, 66.0 / 256.0))), "{marks:?}");
        assert!(marks.contains(&(Vec2::new(27.0, 53.0), Vec2::new(112.0 / 128.0, 82.0 / 256.0))), "{marks:?}");
    }
}
