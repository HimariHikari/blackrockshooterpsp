//! The window people speak to her through while she is on the field: a face at each
//! end, the speaker's name, and what is said, a line at a time. The original's is
//! the "ifComWindowTask" (0x38B04) with a manager in front of it (0x12E4F0); an
//! event's script works the manager with the field's engine functions 1 to 6. See
//! `docs/FIELD_NOTES.md`.
//!
//! Both are here: the manager's part is [`Comm`]'s own, and the window's is
//! [`Shown`], which also counts the frames each of the window's motions has played,
//! so that `field/hud` can draw it from its layout as it is.
//!
//! The manager hands the window one line at a time, and none while a face is
//! coming or going or the window is closing. A line's recording is played on the
//! stream a scene's lines are (0x38E64), and the line lasts as long as that is
//! heard: here, for the frames the recording takes, whatever is or is not put out.
//! A line's text is all there from the first (0x38EDC brings it up whole), so
//! cross always cuts a line short (0x39164). The window has two sounds of its own:
//! one as it opens (0x39D08), and one for cross and for an answer (0x391D4,
//! 0x39240, 0x39C64).

use std::collections::VecDeque;

use crate::controls::{button, Pad};
use crate::sound::{Request, MAKE_WAY};
use crate::text::Span;

/// The places a face can be shown in; a line says which of them is speaking.
pub const PLACES: usize = 2;

/// The interface's frames that go by in one of the game's (0x850): a line's time
/// and the window's motions are counted in them.
pub const FRAMES_A_STEP: f32 = 2.0;
/// The stream its lines are spoken on.
const STREAM: usize = 0;
// Its sounds, of the bank that is always there.
const SOUND_OPEN: u32 = 0x31;
const SOUND_CROSS: u32 = 3;

/// The sets of the window's layout (`if_fldsys.lpk/000.lpk`) and what plays them
/// (0x39790 gives the layout eight players): the window's body, the answers, and
/// for each place the face's frame, what goes over the face, and the marks by the
/// name of whoever is speaking.
pub mod set {
    use super::PLACES;

    pub const FACE_IN: [usize; PLACES] = [0, 4];
    pub const FACE_OUT: [usize; PLACES] = [1, 5];
    pub const OVER_FACE: [usize; PLACES] = [2, 6];
    pub const SPEAKING: [usize; PLACES] = [3, 7];
    pub const OPEN: usize = 8;
    pub const CLOSE: usize = 9;
    pub const ANSWERS_IN: usize = 10;
    pub const ANSWERS_OUT: usize = 11;
    pub const ANSWERED: usize = 12;
    pub const SIGN_IN: usize = 13;
    pub const SIGN_OUT: usize = 14;
}

/// How long the motions that something waits for take, in the interface's frames:
/// what the layout's clips come to (`field/hud` checks them against it).
pub mod frames {
    pub const CLOSE: f32 = 10.0;
    pub const FACE_IN: f32 = 12.0;
    pub const FACE_OUT: f32 = 10.0;
    pub const ANSWERS_IN: f32 = 8.0;
    pub const ANSWERED: f32 = 10.0;
    pub const ANSWERS_OUT: f32 = 10.0;
}

/// What a line is spoken with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    /// It has none.
    None,
    /// A recording of so many frames.
    Frames(i32),
    /// It names a recording the event does not have: nothing is heard, and nothing
    /// ends.
    Missing,
}

/// Something said.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Which place's face is speaking.
    pub speaker: usize,
    pub text: Vec<Span>,
    pub voice: Voice,
    /// The file of the recording it is spoken with.
    pub recording: Option<String>,
    /// The interface's frames it stays for; below zero it stays for as long as its
    /// voice lasts.
    pub time: i32,
    /// The two answers it asks to choose between, if it asks: L gives the first and
    /// R the second.
    pub choices: Option<[String; 2]>,
    /// The answers are up to be chosen from.
    pub choosing: bool,
    /// The window has been given it.
    started: bool,
    /// The interface's frames it has been up, for one that is timed.
    elapsed: i32,
    /// Frames of its voice still to go, and whether the voice has been heard to
    /// start: only then can it be heard to end.
    voice_left: i32,
    heard: bool,
}

impl Line {
    pub fn new(speaker: usize, text: Vec<Span>, voice: Voice, time: i32) -> Line {
        let voice_left = if let Voice::Frames(frames) = voice { frames } else { 0 };
        Line { speaker, text, voice, recording: None, time, choices: None, choosing: false, started: false, elapsed: 0, voice_left, heard: false }
    }

    /// The same line, spoken with the recording in `file`.
    pub fn spoken(mut self, file: &str) -> Line {
        self.recording = Some(file.to_owned());
        self
    }

    /// Its voice is stopped and its time counted as up (0x391E8, 0x39384).
    fn cut_short(&mut self, sounds: &mut Vec<Request>) {
        if self.voice != Voice::None {
            if self.recording.is_some() {
                sounds.push(Request::Hush(STREAM, 0));
            }
            self.voice_left = 0;
            self.heard = true;
        }
        if self.time > 0 {
            self.elapsed = self.time;
        }
    }
}

/// What the answers of a line that asks are doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Answers {
    /// There are none up.
    #[default]
    None,
    /// They have come up to be chosen from.
    Up,
    /// One has been chosen, and shows it.
    Chosen,
    /// They are going.
    Going,
}

/// The sign that a line can be passed: it comes once a line is up, and goes with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sign {
    pub coming: bool,
    /// The frames it has been coming or going for.
    pub frames: f32,
}

/// A face as the window shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Face {
    pub shown: bool,
    /// It has come or is coming; when not, it is going.
    pub coming: bool,
    /// The bundle of the event it is of and its number among that event's faces;
    /// none for a place shown with no face in it.
    pub face: Option<(String, i32)>,
    /// The frames its frame has been coming or going for.
    pub frames: f32,
    /// The frames what goes over it has played for.
    pub over: f32,
}

/// The window itself, as its task has it (0x38B04), and how far its motions are.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub open: bool,
    /// It is on its way out: it is still open until it has gone.
    pub closing: bool,
    /// The frames its body has played since it was opened, and since it began to
    /// close.
    pub body: f32,
    pub closing_for: f32,
    pub sign: Option<Sign>,
    /// Which place's face is speaking: 1 or 2, and 0 for neither.
    pub speaker: usize,
    pub answers: Answers,
    /// The answer chosen: 0 or 1, and below zero while there is none.
    pub answer: i32,
    /// The frames the answers have played since they came up, since one was chosen
    /// and since they began to go.
    pub answers_for: f32,
    pub answered_for: f32,
    pub going_for: f32,
    pub faces: [Face; PLACES],
    /// The frames the marks by each place's name have played, which they do
    /// whenever the place's face is shown, from the first there ever was.
    pub speaking_for: [f32; PLACES],
    /// It has a line.
    has_line: bool,
    /// Everything it was given has been said.
    all_said: bool,
    /// A line that is over stays: it asks, and waits for its answer (0x39284).
    hold: bool,
    /// The line's voice or its time is over.
    line_done: bool,
    sign_wanted: bool,
    sign_shown: bool,
    /// Its sound has been heard since it was opened.
    sounded: bool,
    answer_sound: bool,
}

impl Default for Shown {
    fn default() -> Shown {
        Shown {
            open: false,
            closing: false,
            body: 0.0,
            closing_for: 0.0,
            sign: None,
            speaker: 0,
            answers: Answers::None,
            answer: -1,
            answers_for: 0.0,
            answered_for: 0.0,
            going_for: 0.0,
            faces: default_faces(),
            speaking_for: [0.0; PLACES],
            has_line: false,
            // A window that has never been opened holds nobody up.
            all_said: true,
            hold: false,
            line_done: false,
            sign_wanted: false,
            sign_shown: false,
            sounded: false,
            answer_sound: false,
        }
    }
}

fn default_faces() -> [Face; PLACES] {
    std::array::from_fn(|_| Face::default())
}

impl Shown {
    /// Opens it (0x38C20): with nothing to say and nobody speaking. The faces are
    /// left as they are.
    fn open(&mut self) {
        *self = Shown { open: true, faces: std::mem::replace(&mut self.faces, default_faces()), speaking_for: self.speaking_for, ..Shown::default() };
    }

    /// Sends it out (0x38D14): what it was saying is dropped, and the faces go
    /// with it.
    fn close(&mut self, line: Option<&Line>, sounds: &mut Vec<Request>) {
        self.drop_line(line, sounds);
        self.closing = true;
        self.closing_for = 0.0;
        for place in 0..PLACES {
            if self.faces[place].shown {
                self.send_face_out(place);
            }
        }
    }

    /// Forgets its line (0x39078): what was being said fades out.
    fn drop_line(&mut self, line: Option<&Line>, sounds: &mut Vec<Request>) {
        if self.has_line && line.is_some_and(|line| line.recording.is_some()) {
            sounds.push(Request::Hush(STREAM, MAKE_WAY));
        }
        self.has_line = false;
    }

    /// Whether it has gone (0x38E38).
    fn is_closed(&self) -> bool {
        !self.open
    }

    /// Is given a line to say (0x38E64). One given to a window that is not open is
    /// lost.
    fn say(&mut self, line: &mut Line, sounds: &mut Vec<Request>) {
        if !self.open || self.has_line {
            return;
        }
        self.has_line = true;
        self.line_done = false;
        if let Some(file) = &line.recording {
            sounds.push(Request::Say(STREAM, file.clone()));
        }
        self.all_said = false;
        self.speaker = if line.speaker == 0 { 1 } else { 2 };
        if self.answers == Answers::None {
            self.sign_wanted = true;
        }
    }

    /// Cross (0x39164): the line is cut short, and the sign goes.
    fn cross(&mut self, line: &mut Line, sounds: &mut Vec<Request>) {
        if !self.open || !self.has_line {
            return;
        }
        line.cut_short(sounds);
        self.line_done = true;
        self.sign = Some(Sign { coming: false, frames: 0.0 });
        sounds.push(Request::common(SOUND_CROSS));
    }

    /// Whether the line's voice or its time is over (0x39268).
    fn line_done(&self) -> bool {
        self.open && self.line_done
    }

    /// The answers come up (0x392B0): the sign makes way for them, and whatever of
    /// the line was still being said is cut short.
    fn answers_come(&mut self, line: &mut Line, sounds: &mut Vec<Request>) {
        self.answers = Answers::Up;
        self.answer = -1;
        self.answers_for = 0.0;
        if std::mem::take(&mut self.sign_wanted) && std::mem::take(&mut self.sign_shown) {
            self.sign = Some(Sign { coming: false, frames: 0.0 });
        }
        if self.open && self.has_line {
            line.cut_short(sounds);
            self.line_done = true;
        }
    }

    /// One of the answers is chosen (0x39418).
    fn answer_chosen(&mut self, answer: i32) {
        self.answers = Answers::Chosen;
        self.answer = answer;
        self.answered_for = 0.0;
        self.answer_sound = true;
    }

    /// The answers go (0x393CC).
    fn answers_go(&mut self) {
        self.answers = Answers::Going;
        self.going_for = 0.0;
    }

    /// Whether what the answers are doing has been played through (0x394EC).
    fn answers_finished(&self) -> bool {
        match self.answers {
            Answers::None => true,
            Answers::Up => self.answers_for >= frames::ANSWERS_IN,
            // Those that are not the one chosen are still what they were.
            Answers::Chosen => self.answered_for >= frames::ANSWERED && self.answers_for >= frames::ANSWERS_IN,
            Answers::Going => self.going_for >= frames::ANSWERS_OUT,
        }
    }

    /// A face is put in a place (0x39530): it comes in, unless one is there already
    /// and only the picture changes.
    fn put_face(&mut self, place: usize, face: Option<(String, i32)>) {
        let shown = &mut self.faces[place];
        shown.face = face;
        if !shown.coming {
            shown.coming = true;
            shown.frames = 0.0;
            shown.over = 0.0;
        }
        shown.shown = true;
    }

    /// A place's face is sent out (0x3962C).
    fn send_face_out(&mut self, place: usize) {
        let shown = &mut self.faces[place];
        shown.shown = true;
        shown.coming = false;
        shown.frames = 0.0;
    }

    /// Whether a place's face has come or gone all the way (0x39718).
    fn face_finished(&self, place: usize) -> bool {
        let shown = &self.faces[place];
        !shown.shown || shown.frames >= if shown.coming { frames::FACE_IN } else { frames::FACE_OUT }
    }

    /// A frame of it (0x399C0).
    fn frame(&mut self, line: Option<&mut Line>, sounds: &mut Vec<Request>) {
        let step = FRAMES_A_STEP;
        if self.open {
            if let Some(line) = line.filter(|_| self.has_line) {
                let over = if line.elapsed < line.time {
                    line.elapsed += step as i32;
                    false
                } else if line.time >= 0 || line.voice == Voice::None {
                    true
                } else if !line.heard {
                    line.heard = line.voice_left > 0;
                    false
                } else {
                    line.voice_left <= 0
                };
                line.voice_left = (line.voice_left - 1).max(0);
                // One with neither a time nor a voice is over without more ado; any
                // other is noted as done, and stays if it is held (0x39AA4, 0x39ACC).
                let gone = if line.time < 0 && line.voice == Voice::None {
                    true
                } else {
                    self.line_done |= over;
                    over && !self.hold
                };
                if gone {
                    if line.recording.is_some() {
                        sounds.push(Request::Hush(STREAM, 0));
                    }
                    self.has_line = false;
                    self.all_said = true;
                    if std::mem::take(&mut self.sign_shown) {
                        self.sign = Some(Sign { coming: false, frames: 0.0 });
                    }
                }
            }
            if std::mem::take(&mut self.answer_sound) {
                sounds.push(Request::common(SOUND_CROSS));
            }
            // The sign comes as soon as there is nothing of the text still to come
            // up, which is at once (0x39C9C).
            if self.sign_wanted && !self.sign_shown {
                self.sign = Some(Sign { coming: true, frames: 0.0 });
                self.sign_shown = true;
            }

            // Its body, and the sign with it.
            if !self.closing {
                self.body += step;
                if let Some(sign) = &mut self.sign {
                    sign.frames += step;
                }
                if !std::mem::replace(&mut self.sounded, true) {
                    sounds.push(Request::common(SOUND_OPEN));
                }
            } else {
                self.closing_for += step;
                if self.closing_for >= frames::CLOSE {
                    self.closing = false;
                    self.open = false;
                } else if !std::mem::replace(&mut self.sounded, true) {
                    sounds.push(Request::common(SOUND_OPEN));
                }
            }
            match self.answers {
                Answers::None => {}
                Answers::Up => self.answers_for += step,
                Answers::Chosen => {
                    self.answers_for += step;
                    self.answered_for += step;
                }
                Answers::Going => {
                    self.going_for += step;
                    if self.going_for >= frames::ANSWERS_OUT {
                        self.answers = Answers::None;
                    }
                }
            }
        }
        for place in 0..PLACES {
            let shown = &mut self.faces[place];
            if !shown.shown {
                continue;
            }
            shown.frames += step;
            shown.over += step;
            self.speaking_for[place] += step;
            if !shown.coming && shown.frames >= frames::FACE_OUT {
                *shown = Face::default();
                if self.speaker == place + 1 {
                    self.speaker = 0;
                }
            }
        }
    }
}

/// A place as the manager has it (0x12ED3C on).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Place {
    shown: bool,
    /// The face it has, or had.
    face: i32,
    /// The face it is to have once the one it has is gone.
    next: Option<i32>,
    going: bool,
    coming: bool,
}

impl Place {
    /// Nothing is said while a place's face is on its way (0x12F0F4).
    fn busy(&self) -> bool {
        self.next.is_some() || self.going || self.coming
    }
}

/// The window, with what is being said in it and what is still to be said.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comm {
    /// It is open, as far as the scripts go: the window itself may be on its way.
    pub open: bool,
    /// The window is on its way out: nothing is said, and it is not opened again,
    /// until it has gone.
    closing: bool,
    /// It was told to open while it was on its way out.
    reopen: bool,
    places: [Place; PLACES],
    /// The bundle of the event whose faces the scripts speak of.
    pub bundle: String,
    /// The answers the next line said will ask to choose between (0x12EBF8).
    pub choices: Option<[String; 2]>,
    /// What the last line that asked was answered with; none while one is still to
    /// be answered.
    pub answer: Option<i32>,
    /// The answers are up, the window has been told which was chosen, and they are
    /// on their way out.
    asking: bool,
    told: bool,
    leaving: bool,
    lines: VecDeque<Line>,
    /// What it has asked of the sound and has yet to be handed on.
    pub sounds: Vec<Request>,
    /// The window as it is to be seen.
    pub window: Shown,
}

impl Comm {
    /// The line being said.
    pub fn line(&self) -> Option<&Line> {
        self.lines.front().filter(|_| self.window.has_line)
    }

    /// The answers the line at the head of the queue asks to choose between.
    pub fn asked(&self) -> Option<&[String; 2]> {
        self.lines.front().and_then(|line| line.choices.as_ref())
    }

    /// The face each place has, as the scripts left them.
    pub fn faces(&self) -> [Option<i32>; PLACES] {
        self.places.map(|place| place.shown.then_some(place.next.unwrap_or(place.face)))
    }

    /// Nothing is begun while a face is coming or going or the window is on its way
    /// out (0x12F074).
    fn blocked(&self) -> bool {
        self.closing || self.places.iter().any(Place::busy)
    }

    /// Whether there is anything still to be said, or to be waited for (0x12E9D0).
    pub fn busy(&self) -> bool {
        !self.lines.is_empty() || self.blocked()
    }

    /// Opens the window for the faces of an event's bundle (0x12EA28). One that is
    /// on its way out opens once it has gone.
    pub fn open_for(&mut self, bundle: String) {
        self.bundle = bundle;
        if self.open {
            return;
        }
        if self.closing {
            self.reopen = true;
        } else {
            self.open_now();
        }
    }

    fn open_now(&mut self) {
        self.window.open();
        self.open = true;
    }

    /// Queues a line (0x12E964), and has it said at once if nothing is in the way.
    pub fn say(&mut self, mut line: Line) {
        // A line takes the answers that were waiting for one, and what was
        // answered before is forgotten (0x12E530).
        line.choices = self.choices.take();
        if line.choices.is_some() {
            self.answer = None;
        }
        let first = self.lines.is_empty();
        self.lines.push_back(line);
        if first && !self.blocked() {
            self.start();
        }
    }

    /// Gives the window the line at the head of the queue (0x12E600).
    fn start(&mut self) {
        let Some(line) = self.lines.front_mut() else { return };
        let asks = line.choices.is_some();
        line.choosing = false;
        line.started = true;
        self.window.hold = asks;
        self.window.say(line, &mut self.sounds);
        // Nothing can be asked through a window that is not open: it is as if the
        // first answer had been given, so that no script waits for ever.
        if asks && !self.window.has_line && self.answer.is_none() {
            self.answer = Some(0);
        }
    }

    /// Puts a face in a place (0x12ED3C). A place that has another sends that out
    /// first.
    pub fn put_face(&mut self, place: usize, face: i32) {
        let Some(had) = self.places.get(place).copied() else { return };
        if !had.shown {
            self.show(place, face);
        } else if had.face != face {
            if !had.going {
                self.send_out(place);
            }
            self.places[place].next = Some(face);
        } else if had.going {
            // The same face again, while it is going: it comes back.
            self.places[place].next = Some(face);
        }
    }

    /// Takes a place's face away (0x12EE10).
    pub fn take_face(&mut self, place: usize) {
        let Some(had) = self.places.get_mut(place) else { return };
        had.next = None;
        if !had.going {
            self.send_out(place);
        }
    }

    /// 0x12EF84.
    fn show(&mut self, place: usize, face: i32) {
        self.window.put_face(place, (face >= 0).then(|| (self.bundle.clone(), face)));
        self.places[place] = Place { shown: true, face, coming: true, ..self.places[place] };
    }

    /// 0x12F024.
    fn send_out(&mut self, place: usize) {
        self.window.send_face_out(place);
        self.places[place] = Place { coming: false, going: true, ..self.places[place] };
    }

    /// Closes the window (0x12EA88): the faces go with it. Whatever was still to be
    /// said is dropped with it here; the original leaves its queue, and its scripts
    /// wait for everything to have been said before they close.
    pub fn close(&mut self) {
        for place in 0..PLACES {
            if self.places[place].shown {
                self.take_face(place);
            }
        }
        if std::mem::take(&mut self.asking) {
            self.window.answers_go();
            self.leaving = true;
        }
        self.window.close(self.lines.front(), &mut self.sounds);
        self.open = false;
        self.closing = true;
        self.unanswered();
        self.lines.clear();
    }

    /// Everything of it is dropped there and then (0x12E368), as when an event is
    /// skipped: nothing goes out, it is gone.
    pub fn reset(&mut self) {
        self.window.drop_line(self.lines.front(), &mut self.sounds);
        self.unanswered();
        let speaking_for = self.window.speaking_for;
        *self = Comm {
            bundle: std::mem::take(&mut self.bundle),
            answer: self.answer,
            sounds: std::mem::take(&mut self.sounds),
            window: Shown { speaking_for, ..Shown::default() },
            ..Comm::default()
        };
    }

    /// A question that is dropped unasked counts as answered with its first answer.
    fn unanswered(&mut self) {
        if self.lines.iter().any(|line| line.choices.is_some()) && self.answer.is_none() {
            self.answer = Some(0);
        }
    }

    /// A frame of it: the manager's (0x12E288), then the window's (0x399C0). With
    /// `skip` nothing is said at all: every line is over at once, a question is
    /// answered with its first answer and nothing takes its time, for runs with no
    /// one to listen.
    pub fn step(&mut self, pad: &Pad, skip: bool) {
        if skip {
            self.unanswered();
            self.window.drop_line(self.lines.front(), &mut self.sounds);
            self.lines.clear();
            if std::mem::take(&mut self.asking) {
                self.window.answers_go();
                self.leaving = true;
            }
            // A few frames in one, each with every motion at its end.
            for _ in 0..4 {
                self.hurry();
                self.frame(&Pad::default());
            }
            return;
        }
        self.frame(pad);
    }

    /// Has every motion that is waited for count as played through.
    fn hurry(&mut self) {
        let window = &mut self.window;
        window.closing_for = window.closing_for.max(frames::CLOSE);
        window.going_for = window.going_for.max(frames::ANSWERS_OUT);
        for face in &mut window.faces {
            face.frames = face.frames.max(frames::FACE_IN);
        }
    }

    fn frame(&mut self, pad: &Pad) {
        // The answers have gone (0x12EC2C); the window has (0x12EB78).
        if self.leaving && self.window.answers == Answers::None {
            self.leaving = false;
        }
        if self.closing && self.window.is_closed() {
            self.closing = false;
        }
        if self.reopen && !self.closing {
            self.open_now();
            self.reopen = false;
        }
        // The lines: the one at the head has its frame, and when it is over the
        // next is begun and has its frame at once.
        if !self.blocked() {
            while !self.lines.is_empty() {
                self.line_frame(pad);
                if self.line_busy() {
                    break;
                }
                self.lines.pop_front();
                if self.lines.is_empty() || self.blocked() {
                    break;
                }
                self.start();
            }
        }
        for place in 0..PLACES {
            self.place_frame(place);
        }
        self.window.frame(self.lines.front_mut(), &mut self.sounds);
    }

    /// The frame of the line at the head of the queue (0x12E6D0).
    fn line_frame(&mut self, pad: &Pad) {
        let pressed = |buttons: u32| pad.pressed & buttons != 0;
        if !self.lines[0].started {
            self.start();
        }
        let line = &mut self.lines[0];
        if line.choices.is_none() {
            if self.open && pressed(button::CROSS) {
                self.window.cross(line, &mut self.sounds);
            }
        } else if !line.choosing {
            // The answers come up when the line has been said, or at once on cross.
            if pressed(button::CROSS) || self.window.line_done() {
                self.window.answers_come(line, &mut self.sounds);
                self.asking = true;
                self.told = false;
                line.choosing = true;
            }
        } else {
            // L gives the first answer and R the second; L comes first when both
            // go down together.
            if pressed(button::L | button::R) {
                self.answer = Some(if pressed(button::L) { 0 } else { 1 });
            }
            if let (Some(answer), false) = (self.answer, self.told) {
                self.window.answer_chosen(answer);
                self.told = true;
            }
            // Once the answer has shown itself the answers go, and the line with them.
            if self.told && self.asking && self.window.answers_finished() && self.window.line_done() {
                self.window.hold = false;
                self.window.answers_go();
                self.asking = false;
                self.leaving = true;
            }
        }
    }

    /// Whether the line at the head of the queue is still to be waited for
    /// (0x12E894).
    fn line_busy(&self) -> bool {
        match self.lines.front() {
            Some(line) if line.choices.is_some() => self.answer.is_none() || self.asking || self.leaving,
            Some(_) => !self.window.all_said,
            None => false,
        }
    }

    /// A frame of a place (0x12EE6C): a face that has gone makes way for the one
    /// that is to follow it.
    fn place_frame(&mut self, place: usize) {
        let had = self.places[place];
        if had.going && !self.window.faces[place].shown {
            match had.next {
                Some(face) => {
                    self.show(place, face);
                    self.places[place].next = None;
                    self.places[place].going = false;
                }
                None => self.places[place] = Place { shown: false, going: false, ..had },
            }
        }
        if self.places[place].coming && self.window.face_finished(place) {
            self.places[place].coming = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text;

    fn line(words: &str, voice: Voice, time: i32) -> Line {
        Line::new(0, text::pages(words, &text::Plain).remove(0), voice, time)
    }

    fn open() -> Comm {
        let mut comm = Comm::default();
        comm.open_for("bundle".into());
        comm
    }

    fn press(buttons: u32) -> Pad {
        Pad { pressed: buttons, held: buttons, ..Pad::default() }
    }

    fn frames(comm: &mut Comm, count: usize) {
        for _ in 0..count {
            comm.step(&Pad::default(), false);
        }
    }

    fn said(comm: &Comm) -> Option<String> {
        comm.line().map(|line| text::plain(&line.text))
    }

    #[test]
    fn a_line_stays_for_as_long_as_its_voice_lasts() {
        let mut comm = open();
        comm.say(line("More enemies inbound!", Voice::Frames(100), -1));
        comm.say(line("Be quick.", Voice::Frames(50), -1));
        for _ in 0..100 {
            comm.step(&Pad::default(), false);
            assert_eq!(said(&comm).as_deref(), Some("More enemies inbound!"));
        }
        // The frame after its voice has ended the window has nothing to say, and
        // the frame after that the manager has given it the next line.
        comm.step(&Pad::default(), false);
        assert!(comm.line().is_none() && comm.busy());
        comm.step(&Pad::default(), false);
        assert_eq!(said(&comm).as_deref(), Some("Be quick."));
        for _ in 0..51 {
            assert!(comm.busy());
            comm.step(&Pad::default(), false);
        }
        assert!(!comm.busy() && comm.line().is_none());
    }

    #[test]
    fn a_lines_recording_starts_with_it_and_is_stopped_when_it_is_over() {
        let mut comm = open();
        comm.say(line("More enemies inbound!", Voice::Frames(3), -1).spoken("one.riff"));
        comm.say(line("Be quick.", Voice::Frames(50), -1).spoken("two.riff"));
        assert_eq!(comm.sounds, [Request::Say(0, "one.riff".into())]);
        // The window's sound is heard on its first frame.
        comm.step(&Pad::default(), false);
        assert_eq!(comm.sounds[1..], [Request::common(SOUND_OPEN)]);
        frames(&mut comm, 4);
        assert_eq!(comm.sounds[2..], [Request::Hush(0, 0), Request::Say(0, "two.riff".into())]);
        // Closing the window has what is being said fade out, and makes no sound of
        // its own once the window's has been heard.
        comm.sounds.clear();
        comm.close();
        assert_eq!(comm.sounds, [Request::Hush(0, MAKE_WAY)]);
        frames(&mut comm, 6);
        assert_eq!(comm.sounds, [Request::Hush(0, MAKE_WAY)]);
        assert!(!comm.busy() && !comm.window.open);
        // Opened again, it is heard again; a line with no recording asks nothing
        // of the sound.
        comm.sounds.clear();
        comm.open_for("bundle".into());
        comm.open_for("bundle".into());
        comm.say(line("...", Voice::None, 4));
        frames(&mut comm, 5);
        assert_eq!(comm.sounds, [Request::common(SOUND_OPEN)]);
        assert!(!comm.busy());
        // A window closed before it has had a frame is heard as it goes.
        comm.close();
        frames(&mut comm, 6);
        comm.sounds.clear();
        comm.open_for("bundle".into());
        comm.close();
        frames(&mut comm, 1);
        assert_eq!(comm.sounds, [Request::common(SOUND_OPEN)]);
    }

    #[test]
    fn cross_cuts_a_line_short() {
        let mut comm = open();
        comm.say(line("More enemies inbound!", Voice::Frames(100), -1).spoken("one.riff"));
        comm.say(line("Be quick.", Voice::Frames(50), -1));
        frames(&mut comm, 3);
        // The sign that a line can be passed came with the line.
        assert_eq!(comm.window.sign, Some(Sign { coming: true, frames: 3.0 * FRAMES_A_STEP }));
        comm.sounds.clear();
        // Its voice is stopped there and then, and cross is heard. The sign goes
        // with the line, and with the window still to say things comes anew in the
        // same frame, for whatever is said next.
        comm.step(&press(button::CROSS), false);
        assert_eq!(comm.sounds, [Request::Hush(0, 0), Request::common(SOUND_CROSS), Request::Hush(0, 0)]);
        assert!(comm.line().is_none());
        assert_eq!(comm.window.sign, Some(Sign { coming: true, frames: FRAMES_A_STEP }));
        comm.step(&Pad::default(), false);
        assert_eq!(said(&comm).as_deref(), Some("Be quick."));
        assert_eq!(comm.window.sign, Some(Sign { coming: true, frames: 2.0 * FRAMES_A_STEP }));
        // When the answers of a line that asks come up, the sign goes for good.
        let mut comm = open();
        comm.choices = Some(["Yes.".to_string(), "No.".to_string()]);
        comm.say(line("Well?", Voice::Frames(100), -1));
        frames(&mut comm, 3);
        comm.step(&press(button::CROSS), false);
        assert_eq!(comm.window.sign, Some(Sign { coming: false, frames: FRAMES_A_STEP }));
        frames(&mut comm, 3);
        assert_eq!(comm.window.sign, Some(Sign { coming: false, frames: 4.0 * FRAMES_A_STEP }));
    }

    #[test]
    fn a_timed_line_stays_its_time_whatever_its_voice() {
        let mut comm = open();
        // Sixty of the interface's frames are thirty of the game's.
        comm.say(line("Wait.", Voice::None, 60));
        for _ in 0..31 {
            assert!(comm.line().is_some());
            comm.step(&Pad::default(), false);
        }
        assert!(comm.line().is_none() && comm.busy());
        comm.step(&Pad::default(), false);
        assert!(!comm.busy());
        // With neither a time nor a voice it is over at once.
        comm.say(line("Gone.", Voice::None, -1));
        comm.step(&Pad::default(), false);
        assert!(comm.line().is_none());
        comm.step(&Pad::default(), false);
        assert!(!comm.busy());
    }

    #[test]
    fn a_line_whose_voice_is_missing_waits_to_be_cut_short() {
        let mut comm = open();
        comm.say(line("...", Voice::Missing, -1));
        frames(&mut comm, 500);
        assert!(comm.busy());
        comm.step(&press(button::CROSS), false);
        comm.step(&Pad::default(), false);
        assert!(!comm.busy());
    }

    #[test]
    fn a_line_that_asks_stays_until_it_is_answered_with_a_shoulder_button() {
        let choices = || Some(["Yes.".to_string(), "No.".to_string()]);
        let mut comm = open();
        comm.answer = Some(1);
        comm.choices = choices();
        comm.say(line("Kid, can you hear me!?", Voice::Frames(40), -1));
        assert!(comm.choices.is_none() && comm.answer.is_none());
        // The answers are not up until the line has been said, and before that no
        // button answers.
        let both = press(button::L | button::R);
        for _ in 0..42 {
            assert!(!comm.line().unwrap().choosing);
            comm.step(&both, false);
        }
        assert!(comm.line().unwrap().choosing && comm.answer.is_none());
        assert_eq!((comm.window.answers, comm.window.answer), (Answers::Up, -1));
        // Then it waits, for as long as it takes.
        frames(&mut comm, 300);
        assert!(comm.busy() && comm.line().is_some());
        comm.sounds.clear();
        comm.step(&press(button::R), false);
        // The answer is the script's at once, and is heard; the window shows it for
        // its ten frames before the answers go, and the line with them.
        assert_eq!(comm.answer, Some(1));
        assert_eq!((comm.window.answers, comm.window.answer), (Answers::Chosen, 1));
        assert_eq!(comm.sounds, [Request::common(SOUND_CROSS)]);
        frames(&mut comm, 4);
        assert_eq!(comm.window.answers, Answers::Chosen);
        frames(&mut comm, 1);
        assert_eq!(comm.window.answers, Answers::Going);
        assert!(comm.line().is_none() && comm.busy());
        frames(&mut comm, 4);
        assert_eq!(comm.window.answers, Answers::None);
        assert!(comm.busy());
        frames(&mut comm, 1);
        assert!(!comm.busy());

        // Cross brings the answers up at once, and L gives the first.
        comm.choices = choices();
        comm.say(line("You really can't hear me, kid?", Voice::Frames(400), -1));
        comm.step(&press(button::CROSS), false);
        assert!(comm.line().unwrap().choosing);
        comm.step(&press(button::L), false);
        assert_eq!((comm.answer, comm.busy()), (Some(0), true));
        frames(&mut comm, 11);
        assert!(!comm.busy());

        // With no one to answer, the first answer is taken.
        comm.choices = choices();
        comm.say(line("Well?", Voice::None, -1));
        comm.step(&Pad::default(), true);
        assert_eq!((comm.answer, comm.busy()), (Some(0), false));
    }

    #[test]
    fn nothing_is_said_to_a_closed_window_and_closing_it_drops_the_rest() {
        let mut comm = Comm::default();
        comm.say(line("Lost.", Voice::Frames(10), -1));
        assert!(comm.line().is_none());
        comm.step(&Pad::default(), false);
        assert!(!comm.busy());
        // Nor asked: the first answer counts as given.
        comm.choices = Some(["Yes.".to_string(), "No.".to_string()]);
        comm.say(line("Well?", Voice::Frames(10), -1));
        assert_eq!(comm.answer, Some(0));
        comm.step(&Pad::default(), false);
        assert!(!comm.busy());

        let mut comm = open();
        comm.put_face(0, 1);
        comm.say(line("One.", Voice::Frames(10), -1));
        comm.say(line("Two.", Voice::Frames(10), -1));
        frames(&mut comm, 8);
        assert_eq!(said(&comm).as_deref(), Some("One."));
        comm.close();
        // The window and the face take their ten frames to go, and until they have
        // there is still something to wait for.
        assert!(comm.line().is_none() && comm.busy() && comm.window.open && comm.window.closing);
        frames(&mut comm, 4);
        assert!(comm.busy() && comm.window.open);
        frames(&mut comm, 1);
        assert!(!comm.window.open && comm.window.faces[0] == Face::default());
        frames(&mut comm, 1);
        assert!(!comm.busy() && comm.faces() == [None, None]);
        // With no one to listen every line is over at once.
        comm.open_for("bundle".into());
        comm.put_face(0, 1);
        comm.say(line("One.", Voice::Frames(10), -1));
        comm.step(&Pad::default(), true);
        assert!(!comm.busy());
        comm.close();
        comm.step(&Pad::default(), true);
        assert!(!comm.busy() && !comm.window.open);
    }

    #[test]
    fn a_face_comes_in_before_anything_is_said_and_goes_out_before_another_comes() {
        let mut comm = open();
        comm.put_face(0, 3);
        comm.say(line("One.", Voice::Frames(100), -1));
        assert_eq!(comm.faces(), [Some(3), None]);
        // The face takes twelve frames to come, which the manager sees the frame
        // after, and the line is begun the frame after that.
        for _ in 0..7 {
            assert!(comm.line().is_none() && comm.busy());
            comm.step(&Pad::default(), false);
        }
        assert_eq!(comm.window.faces[0].frames, 14.0);
        comm.step(&Pad::default(), false);
        assert_eq!(said(&comm).as_deref(), Some("One."));
        assert_eq!((comm.window.speaker, comm.window.faces[0].face.clone()), (1, Some(("bundle".to_string(), 3))));
        // The same face again changes nothing.
        comm.put_face(0, 3);
        assert!(comm.window.faces[0].coming && comm.window.faces[0].frames == 16.0);
        // Another face: the one that is there goes first, over ten frames, and the
        // new one then comes.
        comm.put_face(0, 5);
        assert_eq!(comm.faces(), [Some(5), None]);
        assert!(!comm.window.faces[0].coming && comm.window.faces[0].frames == 0.0);
        frames(&mut comm, 5);
        assert!(!comm.window.faces[0].shown && comm.window.speaker == 0);
        frames(&mut comm, 1);
        assert_eq!(comm.window.faces[0].face, Some(("bundle".to_string(), 5)));
        assert!(comm.window.faces[0].coming);
        // The marks by the name have played for as long as a face has been shown.
        assert_eq!(comm.window.speaking_for[0], 2.0 * 14.0);
        // Taken away, it goes and none follows; a place with no face asked to give
        // one up shows its empty frame going.
        comm.take_face(0);
        comm.take_face(1);
        assert!(comm.window.faces[1].shown && comm.window.faces[1].face.is_none());
        frames(&mut comm, 6);
        assert_eq!(comm.faces(), [None, None]);
        assert!(comm.window.faces.iter().all(|face| !face.shown));
    }

    #[test]
    fn a_window_told_to_open_as_it_goes_opens_when_it_has_gone() {
        let mut comm = open();
        frames(&mut comm, 3);
        assert_eq!(comm.window.body, 6.0);
        comm.close();
        frames(&mut comm, 2);
        comm.open_for("other".into());
        assert!(!comm.open && comm.window.closing);
        comm.say(line("One.", Voice::Frames(10), -1));
        frames(&mut comm, 3);
        assert!(!comm.window.open && comm.line().is_none());
        // The frame after it has gone it is opened, and what was waiting is said.
        frames(&mut comm, 1);
        assert!(comm.open && comm.window.open && !comm.window.closing);
        assert_eq!((said(&comm).as_deref(), comm.window.body), (Some("One."), 2.0));
        // An event that is skipped drops all of it there and then.
        comm.put_face(1, 2);
        comm.reset();
        assert!(!comm.open && !comm.busy() && !comm.window.open && comm.faces() == [None, None]);
        assert_eq!(comm.bundle, "other");
    }
}
