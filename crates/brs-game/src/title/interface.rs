//! What the title shows and takes the pad for (the object at 0x6D3D4): the ground
//! and the logo, PRESS START, and the menu of five entries. All of it is the
//! layout of `TITLE.VOL/if_title.zzz`, whose sets this plays as it is told to. See
//! `docs/TITLE_NOTES.md`.

use anyhow::Result;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};

use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::interface::{self, layer};

/// The archive it is of, and the files of its layout: its objects, its motions
/// and its groups (0x6DCE8).
pub const PACK: &str = "TITLE.VOL/if_title.zzz";
const LAYOUT: [usize; 3] = [9, 11, 10];

// Its sounds, of the bank that is always there: as the menu comes (0x6D8E0), as
// an entry is chosen (0x6E190), as the cursor moves (0x6E410).
const SOUND_COME: u32 = 0x11;
const SOUND_CHOSEN: u32 = 3;
const SOUND_CURSOR: u32 = 2;
/// The frames after which the menu sounds when it is come straight to
/// (0x6E2A0).
const SOUND_AFTER: i32 = 0x46;

pub const ENTRIES: usize = 5;

/// The sets of the layout, as they are played (0x6DF58): see the notes.
mod set {
    use super::ENTRIES;

    /// Nothing of the first place, nothing of the second.
    pub const NOTHING: [usize; 2] = [0, 1];
    /// The ground: the plates of metal, or the sky of a game cleared.
    pub const METAL: usize = 2;
    pub const SKY: usize = 3;
    /// The title comes, and PRESS START with it.
    pub const COMES: usize = 4;
    pub const PRESS_START: usize = 5;
    /// PRESS START goes and the entries come.
    pub const ENTRIES_COME: usize = 6;
    /// The black sheet comes over everything.
    pub const DARKENS: usize = 27;
    /// The title is there at once, for the menu, and the entries come later.
    pub const THERE: usize = 28;
    pub const ENTRIES_LATER: usize = 29;
    // An entry's own, by its place: as the cursor is on it at first; as the
    // cursor comes to it and as it leaves; as another was chosen; and as the
    // cursor is on it when the menu is come straight to.
    pub const CURSOR_AT_FIRST: [usize; ENTRIES] = [7, 8, 26, 24, 25];
    pub const CURSOR_COMES: [usize; ENTRIES] = [9, 10, 13, 11, 12];
    pub const CURSOR_LEAVES: [usize; ENTRIES] = [14, 15, 18, 16, 17];
    pub const NOT_CHOSEN: [usize; ENTRIES] = [19, 20, 23, 21, 22];
    pub const CURSOR_LATER: [usize; ENTRIES] = [30, 31, 34, 32, 33];
}

/// The groups that are cut to a box when they are drawn, each with the object
/// whose box it is (0x6DDE0): the bar that slides at the left, and the one
/// along the top right.
const CUTS: [(usize, usize); 2] = [(15, 17), (20, 23)];

/// The menu's entries, by the numbers the task knows them by (0x6D698).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    NewGame = 0,
    Load = 1,
    Options = 2,
    Install = 3,
    Gallery = 4,
}

/// The entries from the top down: GALLERY is the third.
const PLACES: [Entry; ENTRIES] = [Entry::NewGame, Entry::Load, Entry::Gallery, Entry::Options, Entry::Install];

/// What the screen is drawn from.
pub struct Assets {
    pub layout: Layout,
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        // The layout's textures (0x6DC0C, the table at 0x168140): one of the
        // archive, a place that is left empty, four more of the archive, one of
        // the pack that is always there (0x2CDC0) and the archive's last four.
        let textures = [own(0), own(0), own(1), own(2), own(3), own(4), interface::file(interface::RESIDENT_PACK, 7, "ptm"), own(5), own(6), own(7), own(8)];
        Ok(Assets { layout: interface::layout_of(data, images, PACK, LAYOUT, &textures)? })
    }
}

/// Its states (0x6D9A0), by the numbers it has for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Reset = 0,
    /// The title comes.
    Coming = 1,
    PressStart = 2,
    /// The entries come.
    Opening = 3,
    Menu = 4,
    /// An entry was chosen, and the screen goes dark.
    Going = 5,
    /// It comes straight to the menu.
    ToMenu = 6,
    Gone = 7,
}

/// What the task tells it to do (0x6D4BC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Told {
    Reset = 0,
    Come = 1,
    ToMenu = 2,
}

/// What it answers of where it is (0x6D774).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Nothing = 0,
    PressStart = 1,
    /// An entry was chosen, and has gone.
    Chosen = 2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interface {
    /// It is drawn (flag 1).
    drawn: bool,
    state: State,
    /// The frames nothing has been done for (+ 8).
    idle: i32,
    /// The ground is the sky rather than the metal (the player's flag 1).
    sky: bool,
    /// The cursor's place among the entries, from the top.
    cursor: usize,
    /// The second place's sets are waited for (the player's + 8).
    waits: bool,
    /// The ground and the logo; and what is chosen from.
    places: [Player; 2],
    /// The frames until the menu's sound, if it is due (flag 2, + 0x48).
    sound_in: Option<i32>,
    /// The sounds it has made since they were last taken up.
    pub sounds: Vec<u32>,
}

impl Interface {
    /// As it is made (0x6D3D4): nothing of it plays yet, and it is not drawn.
    pub fn new(assets: &Assets) -> Interface {
        let nothing = || assets.layout.player(usize::MAX);
        Interface { drawn: false, state: State::Reset, idle: 0, sky: false, cursor: 0, waits: false, places: [nothing(), nothing()], sound_in: None, sounds: Vec::new() }
    }

    /// The task tells it what to do (0x6D4BC).
    pub fn tell(&mut self, assets: &Assets, what: Told) {
        let layout = &assets.layout;
        match what {
            Told::Reset => {
                self.nothing(layout);
                (self.idle, self.state) = (0, State::Reset);
            }
            Told::Come => {
                self.nothing(layout);
                // The ground and the title over it; PRESS START.
                self.places[0].play(layout, self.ground());
                self.places[0].play(layout, set::COMES);
                self.places[1].play(layout, set::PRESS_START);
                self.waits = true;
                (self.idle, self.drawn, self.state) = (0, true, State::Coming);
            }
            Told::ToMenu => {
                self.places[0].play(layout, self.ground());
                self.places[0].play(layout, set::THERE);
                self.places[1].play(layout, set::ENTRIES_LATER);
                self.places[1].play(layout, set::CURSOR_LATER[self.cursor]);
                self.sound_in = Some(SOUND_AFTER);
                self.waits = true;
                (self.drawn, self.state) = (true, State::ToMenu);
            }
        }
    }

    /// Nothing of either place shows (0x6DF8C).
    fn nothing(&mut self, layout: &Layout) {
        for (place, set) in self.places.iter_mut().zip(set::NOTHING) {
            place.play(layout, set);
        }
        self.waits = false;
    }

    fn ground(&self) -> usize {
        if self.sky { set::SKY } else { set::METAL }
    }

    /// Puts the cursor on an entry (0x6D57C with 0).
    pub fn set_cursor(&mut self, entry: Entry) {
        self.cursor = PLACES.iter().position(|place| *place == entry).unwrap_or(0);
    }

    /// Which ground it has: the sky is a game cleared's (0x6D57C with 1).
    pub fn set_sky(&mut self, sky: bool) {
        self.sky = sky;
    }

    /// The entry the cursor is on (0x6D698 with 0).
    pub fn entry(&self) -> Entry {
        PLACES[self.cursor]
    }

    /// The frames nothing has been done for (0x6D698 with 2).
    pub fn idle(&self) -> i32 {
        self.idle
    }

    pub fn answer(&self) -> Answer {
        match self.state {
            State::PressStart => Answer::PressStart,
            State::Gone => Answer::Chosen,
            _ => Answer::Nothing,
        }
    }

    /// A frame of it (0x6D9A0) that is so many of the interface's (0x850).
    pub fn frame(&mut self, assets: &Assets, pad: &Pad, frames: i32) {
        let layout = &assets.layout;
        match self.state {
            State::Reset | State::Gone => {}
            State::Coming => {
                if !self.waits {
                    (self.state, self.idle) = (State::PressStart, 0);
                }
            }
            State::PressStart => {
                if pad.pressed & (button::CROSS | button::START) != 0 {
                    // PRESS START goes, the entries come, and the cursor's is lit.
                    self.places[1].play(layout, set::ENTRIES_COME);
                    self.places[1].play(layout, set::CURSOR_AT_FIRST[self.cursor]);
                    self.sound_in = Some(0);
                    self.waits = true;
                    (self.state, self.idle) = (State::Opening, 0);
                }
            }
            State::Opening | State::ToMenu => {
                if !self.waits {
                    (self.state, self.idle) = (State::Menu, 0);
                }
            }
            State::Menu => {
                if pad.pressed & button::CROSS != 0 {
                    // Every entry but the cursor's goes, and the screen darkens.
                    for place in (0..ENTRIES).filter(|place| *place != self.cursor) {
                        self.places[1].play(layout, set::NOT_CHOSEN[place]);
                    }
                    self.sounds.push(SOUND_CHOSEN);
                    self.places[1].play(layout, set::DARKENS);
                    self.waits = true;
                    self.state = State::Going;
                } else {
                    // The cursor goes round the ends only as a button newly goes
                    // down, not as one held comes again.
                    let was = self.cursor;
                    for (way, end, to) in [(button::UP, 0, was.checked_sub(1).unwrap_or(ENTRIES - 1)), (button::DOWN, ENTRIES - 1, (was + 1) % ENTRIES)] {
                        if pad.held & way == 0 {
                            continue;
                        }
                        if pad.repeat & way != 0 && (was != end || pad.pressed & way != 0) {
                            self.move_cursor(layout, to);
                        }
                        break;
                    }
                    if self.cursor != was {
                        self.idle = 0;
                    }
                }
            }
            State::Going => {
                if !self.waits {
                    (self.drawn, self.state) = (false, State::Gone);
                }
            }
        }
        self.idle += frames;

        // What plays the layout has its frame (0x6D7E8): the wait is over when
        // the second place has played its sets through.
        if self.waits && self.places[1].finished(layout) {
            self.waits = false;
        }
        if let Some(left) = &mut self.sound_in {
            *left -= frames;
            if *left < 0 {
                self.sound_in = None;
                self.sounds.push(SOUND_COME);
            }
        }
        for place in &mut self.places {
            place.advance(layout, frames as f32);
        }
    }

    /// The cursor goes to another entry (0x6E2BC, 0x6E424): the one it leaves
    /// and the one it comes to play a set each.
    fn move_cursor(&mut self, layout: &Layout, to: usize) {
        if to == self.cursor {
            return;
        }
        self.places[1].play(layout, set::CURSOR_LEAVES[self.cursor]);
        self.cursor = to;
        self.places[1].play(layout, set::CURSOR_COMES[to]);
        self.sounds.push(SOUND_CURSOR);
    }

    /// Draws it as it is so many frames of the interface past its last one
    /// (0x6DD60): both places, track by track, two of the groups cut to their
    /// boxes.
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, ahead: f32) {
        if !self.drawn {
            return;
        }
        let layout = &assets.layout;
        for place in &self.places {
            let mut place = place.clone();
            place.advance(layout, ahead);
            for track in 0..place.len() {
                let cut = place.group(track).and_then(|group| CUTS.iter().find(|(of, _)| *of == group)).and_then(|&(_, to)| {
                    let object = layout.objects.objects.get(to)?;
                    let from = Vec2::new(object.position[0] as f32, object.position[1] as f32);
                    Some(Rect::from_corners(from, from + Vec2::new(object.size[0] as f32, object.size[1] as f32)))
                });
                if cut.is_some() {
                    canvas.cut_to(cut);
                }
                layout.draw_track(canvas, layer::TITLE, &place, track, &Look::default());
                if cut.is_some() {
                    canvas.cut_to(None);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn sets(&self) -> [usize; 2] {
        [self.places[0].set(), self.places[1].set()]
    }

    #[cfg(test)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether anything of it is drawn.
    #[cfg(test)]
    pub fn drawn(&self) -> bool {
        self.drawn
    }
}
