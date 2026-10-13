//! Sound: the music, what is spoken, and the sounds of the banks.
//!
//! The game's logic asks for sound as plain data: a script's functions leave
//! [`Request`]s where the field keeps them, and the field hands them to the
//! [`Board`]. The systems here carry them out through Bevy's audio, by the clock
//! rather than by the logic's frames: the original's sound runs in a thread of its
//! own, which decodes and plays a block of 2048 samples at a time and moves every
//! fade a step each block (0x121E4), so blocks are what the lengths of fades are
//! counted in.
//!
//! There is one tune at a time, and two streams for what is spoken: a new tune
//! takes the place of the old at once (0x9E84), and a recording asked of a stream
//! that is playing one has the old one fade out first (0xABA8). The sounds of the
//! banks play beside them, each known by the number it was given when it was asked
//! for, by which it can be stopped, made louder or softer, and moved between left
//! and right.
//!
//! The recordings are the game's own, made playable by `brs-extract --sound` (see
//! `brs_formats::at3`), and the banks are decoded as they are (`brs_formats::pphd`).
//! Each sound is decoded on another thread when it is first asked for, and kept for
//! a while in case it is asked for again.
//!
//! A run that nobody is watching puts no sound out (see `brs_engine::offscreen`).
//! It does all the rest, so that what takes time takes the same time. Muted, the
//! sound goes all the way to the speakers as silence.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use bevy::audio::PlaybackMode;
use bevy::prelude::*;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task, TaskPool};
use brs_engine::audio::{Gains, Pcm, Voice};
use brs_formats::at3::Recording;
use brs_formats::pphd;

use crate::data::GameData;
use crate::options::{self, Options};

/// A block of the original's sound thread, in seconds: 2048 samples at 44.1 kHz.
pub const BLOCK: f32 = 2048.0 / 44_100.0;
/// The loudest a volume goes.
pub const FULL_VOLUME: i32 = 127;
/// The place between left and right that is the middle (0x1616F4).
pub const MIDDLE: i32 = 0x40;
/// The note every tone of every bank is recorded at: asked for at another, a sound
/// is played that many half-tones higher or lower (0x15EEC4).
pub const NOTE: i32 = 0x40;
/// The fastest a sound of a bank is played, in samples a second: four times the
/// rate of the sound hardware (0x15ED14).
const FASTEST: f32 = 4.0 * 44_100.0;
/// The bank of the sounds that are always there (0x308C): the interface's, the
/// field's and the battle's.
pub const COMMON_BANK: &str = "SYSTEM.VOL/fld_pack.bin/06.bin/00.pphd";
/// The numbers the mixer gives the sounds that are asked for without one start
/// here, clear of those the field's scripts and the battle's give theirs.
const OWN_HANDLES: u32 = 0x4000_0000;
/// The streams what is spoken is played on.
pub const STREAMS: usize = 2;
/// The blocks a stream's recording takes to make way for the next, and to stop when
/// a script stops it (0xABA8, 0xC27EC).
pub const MAKE_WAY: i32 = 8;
/// The longest a fade out is, in blocks (0x11D5C).
const LONGEST_FADE: i32 = 0x100;
/// What the music's volume is out of where it is applied (0x134C4): a little more
/// than the 127 it goes up to.
const MUSIC_SCALE: f32 = 128.0;
/// How loud every tone of every bank is, as a part of full: 0xC80 of 0x1000
/// (0x16127C).
const TONE_VOLUME: f32 = 3200.0 / 4096.0;

/// The samples kept decoded, in bytes, before those longest unused are let go.
const KEPT_BYTES: usize = 96 << 20;
/// The seconds what plays a sound is given to go of itself once the sound has been
/// played through, before it is made to; and to fall silent when it is cut short.
const GRACE: f32 = 0.5;
const CUT_GRACE: f32 = 0.05;

/// The folder of the tunes, and what a tune's file is called.
const MUSIC: &str = "SOUND";

/// The file of the tune the game's list calls `name`, as in `field1`.
pub fn tune_file(name: &str) -> String {
    format!("{MUSIC}/BGM_{}.AT3", name.to_ascii_uppercase())
}

/// How loud the player has the music, the sounds and the voices, as parts of full.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volumes {
    pub music: f32,
    pub effects: f32,
    pub voices: f32,
}

impl Volumes {
    /// What the options have them at (0x9DD8, and 0x12E84 for the sounds).
    pub fn of(options: &Options) -> Volumes {
        let part = |volume: u8| f32::from(volume.min(options::LOUDEST)) / f32::from(options::LOUDEST);
        Volumes { music: part(options.music), effects: part(options.effects), voices: part(options.voices) }
    }
}

/// Something the game's logic asks of the sound.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// A tune from its start, in place of whatever is playing (0x9E84): its file.
    Music(String),
    /// The tune fades out over so many blocks and stops; at once with none (0xA10C).
    StopMusic(i32),
    /// The music's volume, of 127, goes to this over so many blocks (0xA204).
    MusicVolume(i32, f32),
    /// The volume last asked of the music is put by (0xA254), and the music goes
    /// back to what was put by at once (0xA204 with it): what a screen with a
    /// jingle of its own does about the tune, as it comes and as it goes.
    /// Each screen keeps it for itself.
    KeepMusicVolume(Keeper),
    MusicVolumeBack(Keeper),
    /// The music goes back to what was put by once a stream has nothing left to
    /// play: what the results of a battle do about the tune under their jingle,
    /// asking each frame whether it is over (0x387E0).
    MusicVolumeBackAfter(usize, Keeper),
    /// A recording is played on a stream, in place of what it is playing (0xABA8):
    /// its file. It is played at full volume.
    Say(usize, String),
    /// The same at a volume of 127.
    SayAt(usize, String, i32),
    /// A stream fades out over so many blocks and stops, and what waited to be
    /// played on it is dropped (0xA2DC).
    Hush(usize, i32),
    /// A sound of a bank is played (0xD34A0, 0xD3554): the number it is to be known
    /// by, the bank's header, the sound's number in it, its volume of 127, and the
    /// note it is played at.
    Effect { handle: u32, bank: String, sound: u32, volume: i32, note: i32 },
    /// A sound of a bank that nobody is going to stop or change: its bank's
    /// header, its number in it, and its volume.
    Play { bank: String, sound: u32, volume: i32 },
    /// A sound of a bank is stopped (0xD3614).
    StopEffect(u32),
    /// A sound's volume, of 127, and its place between left (0) and right (127)
    /// (0xD3674, 0xD36DC).
    EffectVolume(u32, i32),
    EffectPan(u32, i32),
    /// A bank is let go, and with it every sound of it that is playing (0xD3424):
    /// its header.
    DropBank(String),
    /// Every sound of the banks that is playing stops (0xD3648), but for those
    /// known by these numbers.
    StopEffects(Vec<u32>),
    /// All that is playing stands still where it is: the tune (0xA194 with 1),
    /// what the streams are saying (0xA484) and the sounds of the banks
    /// (0xD30C8). What is asked for after it plays as ever: it is what the pause
    /// does, whose own sounds are heard over it.
    Hold,
    /// And it goes on from where it stood (0xA194 with none, 0xA520, 0xD316C).
    LetGo,
    /// A film's sound is made ready: its file, an Ogg Vorbis file of the data. It
    /// waits, silent, until the film is started.
    Film(String),
    /// The film is started, and its sound with it; and the film is over or
    /// stopped, and its sound stops.
    FilmStart,
    FilmStop,
    /// The options' music is taken to be full, or to be what the player has
    /// again: the game makes it 127 about the opening film and puts back what
    /// it was (0xE64DC and 0xE6508 on the field, 0x11A0C0 and 0x11A2EC on the
    /// title).
    MusicOptionFull(bool),
}

/// A screen that turns the music down while it is up and puts by how loud it was,
/// each in a word of its own: the window of a mission failed (0x42AFC), the
/// results of a battle (0x37B94), the window a record is shown in (0x6F6A8),
/// and the window of a mission cleared (0x4A268).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keeper {
    Failed,
    Results,
    Record,
    Cleared,
}

/// What has been asked of the sound and is yet to be done.
#[derive(Resource, Default)]
pub struct Board {
    requests: Vec<Request>,
}

impl Board {
    pub fn ask(&mut self, request: Request) {
        self.requests.push(request);
    }

    pub fn ask_all(&mut self, requests: impl IntoIterator<Item = Request>) {
        self.requests.extend(requests);
    }

    /// Asks for a sound of a bank as it is recorded.
    pub fn play(&mut self, bank: &str, sound: u32, volume: i32) {
        self.requests.push(Request::Play { bank: bank.to_owned(), sound, volume });
    }
}

impl Request {
    /// A sound of the bank that is always there, at full volume: what most of the
    /// game's own sounds are (0xD34A0 with the bank of 0x8D33C).
    pub fn common(sound: u32) -> Request {
        Request::Play { bank: COMMON_BANK.to_owned(), sound, volume: FULL_VOLUME }
    }
}

/// A volume: where it is, where it is going and how fast, a second.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Glide {
    now: f32,
    to: f32,
    rate: f32,
}

impl Glide {
    fn at(value: f32) -> Glide {
        Glide { now: value, to: value, rate: 0.0 }
    }

    /// Has it go to `to` over `blocks` (0x11F60). With no blocks it is there at
    /// once; unless it is to go to nothing, which the original then never gets to:
    /// its step is the volume asked for, and a step of nothing moves nothing.
    fn go(&mut self, to: f32, blocks: f32) {
        self.to = to;
        self.rate = if blocks > 0.0 { (to - self.now).abs() / (blocks * BLOCK) } else { 0.0 };
        if blocks <= 0.0 && to > 0.0 {
            self.now = to;
        }
    }

    fn step(&mut self, seconds: f32) {
        let by = self.rate * seconds;
        self.now = if self.now < self.to { (self.now + by).min(self.to) } else { (self.now - by).max(self.to) };
    }

    /// Where it is going is where it is: what a new tune starts from (0x11724).
    fn arrive(&mut self) {
        self.now = self.to;
    }
}

/// How much of a sound goes to the left and to the right for its place between
/// them (0x1616F4): in the middle all of it to both, and to either side of that
/// the other side falls away, to nothing at the far left.
fn sides(place: i32) -> [f32; 2] {
    let place = place.clamp(0, FULL_VOLUME);
    if place <= MIDDLE {
        [1.0, place as f32 / MIDDLE as f32]
    } else {
        [(FULL_VOLUME - place) as f32 / MIDDLE as f32, 1.0]
    }
}

/// What can be played: a recording by its file, or a sound of a bank by the bank's
/// header and the sound's number.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Source {
    Recording(String),
    Bank(String, u32),
    /// The sound of a film: an Ogg Vorbis file by itself.
    Film(String),
}

impl Source {
    /// What it is kept and reported by.
    fn name(&self) -> String {
        match self {
            Source::Recording(file) | Source::Film(file) => file.clone(),
            Source::Bank(bank, sound) => format!("{bank}#{sound}"),
        }
    }
}

/// A sound that is being played, or is about to be.
#[derive(Debug)]
struct Playing {
    source: Source,
    /// What plays it: there once it has been decoded, in a run that puts sound out.
    entity: Option<Entity>,
    /// How loud it is to either side, which what plays it reads.
    gains: Gains,
    /// It has been decoded and its time is running.
    started: bool,
    /// The seconds of it yet to play, for one that ends.
    left: Option<f32>,
    /// How much of it is let through, from all to none, and how fast that falls, a
    /// second: the fade out.
    level: f32,
    fall: f32,
    /// Its own volume as a part of full, and its place between left and right.
    volume: f32,
    place: i32,
    /// How many times as fast as it was recorded it is played.
    pitch: f32,
    /// It stands still where it is, and nothing of it moves, until it is let go.
    held: bool,
}

impl Playing {
    fn new(source: Source) -> Playing {
        Playing {
            source,
            entity: None,
            gains: Gains::default(),
            started: false,
            left: None,
            level: 1.0,
            fall: 0.0,
            volume: 1.0,
            place: MIDDLE,
            pitch: 1.0,
            held: false,
        }
    }

    /// Has it fade out over `blocks`, unless it is fading out faster already.
    fn fade(&mut self, blocks: i32) {
        let fall = if blocks > 0 { 1.0 / (blocks.min(LONGEST_FADE) as f32 * BLOCK) } else { f32::INFINITY };
        self.fall = self.fall.max(fall);
    }

    /// Goes on by `seconds`; false once it is over, faded out or played through.
    /// Held, it neither fades nor comes any nearer its end.
    fn step(&mut self, seconds: f32) -> bool {
        if self.held {
            return true;
        }
        if self.fall > 0.0 {
            self.level = if self.fall.is_finite() { (self.level - self.fall * seconds).max(0.0) } else { 0.0 };
        }
        if self.started {
            if let Some(left) = &mut self.left {
                *left -= seconds;
            }
        }
        self.level > 0.0 && self.left.is_none_or(|left| left > 0.0)
    }

    /// How loud it is to the left and to the right now, `loudness` being what its
    /// kind of sound is played at.
    fn heard(&self, loudness: f32) -> [f32; 2] {
        sides(self.place).map(|side| side * loudness * self.volume * self.level)
    }
}

/// The sounds that have been decoded, and those that are being.
struct Library {
    data: GameData,
    kept: HashMap<Source, Kept>,
    decoding: HashMap<Source, Task<Result<Pcm, String>>>,
    /// Those that could not be: each is reported once and not tried again.
    failed: HashSet<Source>,
    /// What is decoded is kept as silence of the same length.
    hushed: bool,
    /// The rate each was recorded at.
    rates: HashMap<Source, u32>,
    /// Counts the times one was asked for: which was used last.
    clock: u64,
}

struct Kept {
    sound: Handle<Pcm>,
    bytes: usize,
    used: u64,
}

/// Whether a sound can be played yet.
enum Ready {
    Yes(Handle<Pcm>),
    NotYet,
    Never,
}

impl Library {
    fn new(data: GameData, hushed: bool) -> Library {
        Library { data, kept: HashMap::new(), decoding: HashMap::new(), failed: HashSet::new(), hushed, rates: HashMap::new(), clock: 0 }
    }

    /// The decoded sound, if it is there; if not it is set to be decoded.
    fn get(&mut self, source: &Source) -> Ready {
        self.clock += 1;
        if let Some(kept) = self.kept.get_mut(source) {
            kept.used = self.clock;
            return Ready::Yes(kept.sound.clone());
        }
        if self.failed.contains(source) {
            return Ready::Never;
        }
        if !self.decoding.contains_key(source) {
            let (data, wanted) = (self.data.clone(), source.clone());
            let task = AsyncComputeTaskPool::get_or_init(TaskPool::new).spawn(async move { decode(&data, &wanted).map_err(|error| format!("{error:#}")) });
            self.decoding.insert(source.clone(), task);
        }
        Ready::NotYet
    }

    /// The rate a sound was recorded at, once it has been decoded.
    fn rate(&self, source: &Source) -> Option<u32> {
        self.rates.get(source).copied()
    }

    /// Takes in what has been decoded since, and lets go of what was used longest
    /// ago once more is kept than there is room for, but for what is `in_use`.
    fn collect(&mut self, sounds: &mut Assets<Pcm>, in_use: &HashSet<Source>) {
        let done: Vec<Source> = self.decoding.iter().filter(|(_, task)| task.is_finished()).map(|(source, _)| source.clone()).collect();
        for source in done {
            let Some(task) = self.decoding.remove(&source) else { continue };
            match block_on(task) {
                Ok(pcm) => {
                    let pcm = if self.hushed { pcm.hushed() } else { pcm };
                    let bytes = pcm.frames() * pcm.channels() as usize * size_of::<i16>();
                    debug!("{} is decoded: {:.1} s", source.name(), pcm.seconds());
                    self.rates.insert(source.clone(), pcm.rate());
                    self.clock += 1;
                    self.kept.insert(source, Kept { sound: sounds.add(pcm), bytes, used: self.clock });
                }
                Err(error) => {
                    warn!("{} cannot be played: {error}", source.name());
                    self.failed.insert(source);
                }
            }
        }
        while self.kept.values().map(|kept| kept.bytes).sum::<usize>() > KEPT_BYTES {
            let oldest = self.kept.iter().filter(|(source, _)| !in_use.contains(source)).min_by_key(|(_, kept)| kept.used);
            let Some(oldest) = oldest.map(|(source, _)| source.clone()) else { break };
            self.kept.remove(&oldest);
        }
    }
}

/// Decodes a sound. A recording's own file says how long it is and where it loops,
/// and the Ogg Vorbis file beside it is its sound; a bank's sound is decoded from
/// the bank's data.
fn decode(data: &GameData, source: &Source) -> Result<Pcm> {
    match source {
        Source::Recording(file) => {
            let recording = Recording::parse(&data.read(file)?)?;
            let sound = Path::new(file).with_extension("ogg");
            let ogg = data.read(&sound).context("its sound has not been converted: run `brs-extract --sound`")?;
            Pcm::of_recording(&ogg, &recording)
        }
        Source::Bank(header, number) => {
            let bank = pphd::Bank::parse(&data.read(header)?)?;
            let body = pphd::data_file(Path::new(header)).context("a bank with no data")?;
            let sound = bank.decode(*number, &data.read(&body)?)?;
            let pcm = Pcm::new(sound.samples, 1, sound.rate).context("a sound of no rate")?;
            Ok(match sound.looped {
                Some((first, end)) => pcm.looping(first, end),
                None => pcm,
            })
        }
        Source::Film(file) => Pcm::from_ogg(&data.read(file).context("its sound has not been made: run `brs-extract --sound`")?),
    }
}

/// What becomes of the sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// It is heard.
    On,
    /// It is played as silence.
    Muted,
    /// Nothing is played.
    Off,
}

/// What is playing, and how loud.
#[derive(Resource)]
pub struct Mixer {
    out: Out,
    volumes: Volumes,
    /// The options' music is taken to be full whatever the player has it at.
    music_full: bool,
    /// The music's volume as the game sets it, of 127.
    music_volume: Glide,
    /// The volume of it that was last put by, and the stream whose falling silent
    /// has the music go back to that.
    music_kept: [f32; 4],
    back_after: Option<(usize, Keeper)>,
    tune: Option<Playing>,
    /// The sound of a film, and whether it waits for the film to be started.
    film: Option<Playing>,
    film_waits: bool,
    streams: [Option<Playing>; STREAMS],
    /// What each stream plays next, once what it is playing has made way, and how
    /// loud.
    next: [Option<(String, f32)>; STREAMS],
    /// The sounds of the banks that are playing, by the numbers they were given.
    effects: BTreeMap<u32, Playing>,
    /// How many sounds have been asked for without a number.
    played: u32,
    /// What played sounds that are over, and the seconds each has left to go of
    /// itself.
    ending: Vec<(Entity, f32)>,
    library: Library,
}

/// What a step of the mixer works with.
struct Stage<'a, P: FnMut(Pcm, Gains) -> Entity> {
    out: Out,
    seconds: f32,
    library: &'a mut Library,
    sounds: &'a Assets<Pcm>,
    ending: &'a mut Vec<(Entity, f32)>,
    play: &'a mut P,
}

impl<P: FnMut(Pcm, Gains) -> Entity> Stage<'_, P> {
    /// A sound goes on: it starts once it has been decoded, is as loud as it is to
    /// be, and when it is over what played it is given a moment to end. False once
    /// it is over.
    fn advance(&mut self, playing: &mut Playing, loudness: f32) -> bool {
        playing.gains.hold(playing.held);
        // One that was held before it had started starts when it is let go.
        if !playing.started && !playing.held {
            match self.library.get(&playing.source) {
                Ready::Yes(sound) => {
                    if let Some(pcm) = self.sounds.get(&sound) {
                        let pcm = if playing.pitch == 1.0 { pcm.clone() } else { pcm.clone().pitched(playing.pitch) };
                        playing.started = true;
                        // One that loops does not end of itself.
                        playing.left = pcm.looped().is_none().then(|| pcm.seconds());
                        if self.out != Out::Off {
                            let [left, right] = playing.heard(loudness);
                            playing.gains.set(left, right);
                            playing.entity = Some((self.play)(pcm, playing.gains.clone()));
                        }
                    }
                }
                Ready::NotYet => {}
                Ready::Never => playing.level = 0.0,
            }
        }
        let goes_on = playing.step(self.seconds);
        let [left, right] = if goes_on { playing.heard(loudness) } else { [0.0; 2] };
        playing.gains.set(left, right);
        if let (Some(entity), false) = (playing.entity, goes_on) {
            let played_through = playing.left.is_some_and(|left| left <= 0.0);
            self.ending.push((entity, if played_through { GRACE } else { CUT_GRACE }));
        }
        goes_on
    }
}

impl Mixer {
    fn new(data: GameData, volumes: Volumes, out: Out) -> Mixer {
        Mixer {
            out,
            volumes,
            music_full: false,
            music_volume: Glide::at(FULL_VOLUME as f32),
            music_kept: [FULL_VOLUME as f32; 4],
            back_after: None,
            tune: None,
            film: None,
            film_waits: false,
            streams: [None, None],
            next: [None, None],
            effects: BTreeMap::new(),
            played: 0,
            ending: Vec::new(),
            library: Library::new(data, out == Out::Muted),
        }
    }

    /// The file of the tune that is playing.
    #[cfg(test)]
    fn tune(&self) -> Option<String> {
        self.tune.as_ref().map(|tune| tune.source.name())
    }

    /// Whether the tune of this file is playing (0xA6CC, the music's stream): it
    /// has begun, or never will because it cannot be read.
    pub fn tune_begun(&self, file: &str) -> bool {
        self.tune.as_ref().is_some_and(|tune| tune.source.name() == file && (tune.started || self.library.failed.contains(&tune.source)))
    }

    /// Whether a stream has something to say (0xA3DC), and if it has, whether
    /// that has begun to be heard: the sound's own frame tells the field's
    /// script as a recording begins (0xA8A8), which is what the faces of a
    /// scene go by.
    pub fn saying(&self, stream: usize) -> Option<bool> {
        if self.next.get(stream)?.is_some() {
            return Some(false);
        }
        self.streams[stream].as_ref().map(|line| line.started)
    }

    /// The file of the recording a stream is playing.
    #[cfg(test)]
    fn said(&self, stream: usize) -> Option<String> {
        self.streams.get(stream)?.as_ref().map(|line| line.source.name())
    }

    /// Does what was asked.
    fn carry_out(&mut self, request: Request) {
        debug!("sound: {request:?}");
        match request {
            Request::Music(file) => {
                if let Some(old) = self.tune.take() {
                    self.cut(old);
                }
                self.music_volume.arrive();
                self.tune = Some(self.begin(Source::Recording(file)));
            }
            Request::StopMusic(blocks) => {
                if let Some(tune) = &mut self.tune {
                    tune.fade(blocks);
                }
            }
            Request::MusicVolume(volume, blocks) => {
                self.music_volume.go(volume.clamp(0, FULL_VOLUME) as f32, blocks);
            }
            Request::KeepMusicVolume(keeper) => self.music_kept[keeper as usize] = self.music_volume.to,
            Request::MusicVolumeBack(keeper) => {
                if self.back_after.is_some_and(|(_, waiting)| waiting == keeper) {
                    self.back_after = None;
                }
                self.music_volume.go(self.music_kept[keeper as usize], 0.0);
            }
            Request::MusicVolumeBackAfter(stream, keeper) if stream < STREAMS => self.back_after = Some((stream, keeper)),
            Request::Say(stream, file) => self.carry_out(Request::SayAt(stream, file, FULL_VOLUME)),
            Request::SayAt(stream, file, volume) if stream < STREAMS => {
                let level = volume.clamp(0, FULL_VOLUME) as f32 / FULL_VOLUME as f32;
                match &mut self.streams[stream] {
                    Some(line) => {
                        line.fade(MAKE_WAY);
                        self.next[stream] = Some((file, level));
                    }
                    None => self.streams[stream] = Some(self.begin_at(Source::Recording(file), level)),
                }
            }
            Request::Hush(stream, blocks) if stream < STREAMS => {
                self.next[stream] = None;
                if let Some(line) = &mut self.streams[stream] {
                    line.fade(blocks);
                }
            }
            Request::SayAt(..) | Request::Hush(..) | Request::MusicVolumeBackAfter(..) => {}
            Request::Effect { handle, bank, sound, volume, note } => {
                let mut effect = self.begin(Source::Bank(bank, sound));
                effect.volume = volume.clamp(0, FULL_VOLUME) as f32 / FULL_VOLUME as f32;
                effect.pitch = 2f32.powf((note - NOTE) as f32 / 12.0);
                if let Some(old) = self.effects.insert(handle, effect) {
                    self.cut(old);
                }
            }
            Request::Play { bank, sound, volume } => {
                let handle = OWN_HANDLES + self.played;
                self.played = (self.played + 1) % OWN_HANDLES;
                self.carry_out(Request::Effect { handle, bank, sound, volume, note: NOTE });
            }
            Request::StopEffect(handle) => {
                if let Some(effect) = self.effects.get_mut(&handle) {
                    effect.fade(0);
                }
            }
            Request::EffectVolume(handle, volume) => {
                if let Some(effect) = self.effects.get_mut(&handle) {
                    effect.volume = volume.clamp(0, FULL_VOLUME) as f32 / FULL_VOLUME as f32;
                }
            }
            Request::EffectPan(handle, place) => {
                if let Some(effect) = self.effects.get_mut(&handle) {
                    effect.place = place;
                }
            }
            Request::DropBank(bank) => {
                for effect in self.effects.values_mut() {
                    if matches!(&effect.source, Source::Bank(of, _) if *of == bank) {
                        effect.fade(0);
                    }
                }
            }
            Request::StopEffects(spared) => {
                for (_, effect) in self.effects.iter_mut().filter(|(handle, _)| !spared.contains(handle)) {
                    effect.fade(0);
                }
            }
            Request::Hold => self.hold(true),
            Request::LetGo => self.hold(false),
            Request::Film(file) => {
                // The tune and whatever is being said stop at once (0xA264): the
                // film's sound has the music's stream.
                self.carry_out(Request::StopMusic(0));
                for stream in 0..STREAMS {
                    self.carry_out(Request::Hush(stream, 0));
                }
                if let Some(old) = self.film.take() {
                    self.cut(old);
                }
                let mut film = self.begin(Source::Film(file));
                film.held = true;
                (self.film, self.film_waits) = (Some(film), true);
            }
            Request::FilmStart => {
                if let Some(film) = &mut self.film {
                    film.held = false;
                }
                self.film_waits = false;
            }
            Request::FilmStop => {
                if let Some(old) = self.film.take() {
                    self.cut(old);
                }
                self.film_waits = false;
            }
            Request::MusicOptionFull(full) => self.music_full = full,
        }
    }

    /// How much of full the options have the music's stream at (0x1200C: the
    /// stream's + 0x12088).
    fn music_share(&self) -> f32 {
        if self.music_full { 1.0 } else { self.volumes.music }
    }

    /// Whether the sound of the film that was asked for can be played at once:
    /// it has been decoded, or there is none to be had.
    pub fn film_ready(&self) -> bool {
        self.film.as_ref().is_none_or(|film| self.library.kept.contains_key(&film.source) || self.library.failed.contains(&film.source))
    }

    /// All that is playing now stands still, or goes on.
    fn hold(&mut self, held: bool) {
        let playing = self.tune.iter_mut().chain(self.streams.iter_mut().flatten()).chain(self.effects.values_mut());
        for playing in playing {
            playing.held = held;
        }
        // A film's sound that waits for its film waits on.
        if let Some(film) = self.film.as_mut().filter(|_| !self.film_waits) {
            film.held = held;
        }
    }

    fn begin(&mut self, source: Source) -> Playing {
        // Asked for now, so that it is being decoded by the time it is looked for.
        self.library.get(&source);
        Playing::new(source)
    }

    /// The same, as loud as a part of full.
    fn begin_at(&mut self, source: Source, level: f32) -> Playing {
        let mut playing = self.begin(source);
        playing.volume = level;
        playing
    }

    /// Something that was playing is silent from now on, and what played it goes.
    fn cut(&mut self, old: Playing) {
        old.gains.set(0.0, 0.0);
        if let Some(entity) = old.entity {
            self.ending.push((entity, CUT_GRACE));
        }
    }

    /// Goes on by `seconds`: fades move, what has been decoded starts, what is over
    /// goes, and what waited for a stream begins on it. What plays a sound is made
    /// by `play`, and what is to go is handed to `stop`.
    fn step(&mut self, seconds: f32, sounds: &mut Assets<Pcm>, play: &mut impl FnMut(Pcm, Gains) -> Entity, stop: &mut impl FnMut(Entity)) {
        let playing = self.tune.iter().chain(self.film.iter()).chain(self.streams.iter().flatten()).chain(self.effects.values());
        let in_use: HashSet<Source> = playing.map(|playing| playing.source.clone()).collect();
        self.library.collect(sounds, &in_use);
        self.music_volume.step(seconds);

        for (_, left) in &mut self.ending {
            *left -= seconds;
        }
        self.ending.retain(|&(entity, left)| {
            if left <= 0.0 {
                stop(entity);
            }
            left > 0.0
        });

        let share = self.music_share();
        let music = self.music_volume.now / MUSIC_SCALE * share;
        let (voices, effects) = (self.volumes.voices, TONE_VOLUME * self.volumes.effects);
        let mut stage = Stage { out: self.out, seconds, library: &mut self.library, sounds, ending: &mut self.ending, play };
        // A sound is never played faster than the hardware goes.
        for effect in self.effects.values_mut().filter(|effect| !effect.started && effect.pitch > 1.0) {
            if let Some(rate) = stage.library.rate(&effect.source) {
                effect.pitch = effect.pitch.min(FASTEST / rate as f32);
            }
        }
        if self.tune.as_mut().is_some_and(|tune| !stage.advance(tune, music)) {
            self.tune = None;
        }
        // A film's sound goes out through the music's stream (0xA264, 0x12070),
        // as loud as the options have the music and whatever the game has the
        // music's own volume at (0x12384: the stream's + 0x12088 times the PSP's
        // full volume).
        if self.film.as_mut().is_some_and(|film| !stage.advance(film, share)) {
            self.film = None;
        }
        for line in &mut self.streams {
            if line.as_mut().is_some_and(|line| !stage.advance(line, voices)) {
                *line = None;
            }
        }
        self.effects.retain(|_, effect| stage.advance(effect, effects));
        for stream in 0..STREAMS {
            if self.streams[stream].is_none() {
                if let Some((file, level)) = self.next[stream].take() {
                    self.streams[stream] = Some(self.begin_at(Source::Recording(file), level));
                }
            }
        }
        // The stream whose jingle the music was turned down for has fallen silent.
        if let Some((_, keeper)) = self.back_after.filter(|&(stream, _)| self.streams[stream].is_none()) {
            self.back_after = None;
            self.music_volume.go(self.music_kept[keeper as usize], 0.0);
        }
    }
}

/// The sound of the game, for a game whose data is `data`.
pub fn plugin(app: &mut App, data: &GameData, out: Out) -> Result<()> {
    let options = Options::load(data).context("reading the options a new game starts with")?;
    app.insert_resource(options)
        .insert_resource(Mixer::new(data.clone(), Volumes::of(&options), out))
        .init_resource::<Board>()
        .add_systems(Update, mix);
    Ok(())
}

/// Carries out what has been asked for, and has what is playing go on.
fn mix(
    mut commands: Commands,
    mut board: ResMut<Board>,
    mut mixer: ResMut<Mixer>,
    options: Res<Options>,
    mut sounds: ResMut<Assets<Pcm>>,
    mut voices: ResMut<Assets<Voice>>,
    sinks: Query<(), With<AudioSink>>,
    (time, real): (Res<Time>, Res<Time<Real>>),
    mut heard: Local<usize>,
) {
    // What is heard goes on as the clock on the wall does, whatever the game's own
    // time makes of a frame that took long; a run nobody hears keeps the game's
    // time, so that it comes out the same every time.
    let passed = if mixer.out == Out::On { real.delta_secs() } else { time.delta_secs() };
    for request in board.requests.drain(..) {
        mixer.carry_out(request);
    }
    // The player's volumes are whatever the options have them at now.
    mixer.volumes = Volumes::of(&options);
    let muted = mixer.out == Out::Muted;
    let mut stopped = Vec::new();
    let mut play = |pcm: Pcm, gains: Gains| {
        let settings = PlaybackSettings { mode: PlaybackMode::Despawn, muted, ..PlaybackSettings::ONCE };
        commands.spawn((AudioPlayer(voices.add(Voice::new(pcm, gains))), settings)).id()
    };
    mixer.step(passed, &mut sounds, &mut play, &mut |entity| stopped.push(entity));
    for entity in stopped {
        commands.entity(entity).try_despawn();
    }
    let playing = sinks.iter().count();
    if std::mem::replace(&mut *heard, playing) != playing {
        debug!("sound: {playing} playing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");
    const FULL: Volumes = Volumes { music: 1.0, effects: 1.0, voices: 1.0 };

    /// Every recording the game has can be heard: its own file is read and the
    /// sound beside it decoded. (One that could not be would be over at once,
    /// and a mouth that waits for its voice with it.)
    #[test]
    #[ignore = "decodes every recording of the game, which takes minutes"]
    fn every_recording_can_be_played() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        fn walk(folder: &Path, found: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, found);
                } else if path.extension().is_some_and(|extension| extension == "riff") {
                    found.push(path);
                }
            }
        }
        let root = Path::new(DATA).canonicalize().unwrap();
        let mut found = Vec::new();
        walk(&root, &mut found);
        if !found.iter().any(|path| path.with_extension("ogg").exists()) {
            eprintln!("skipped: the recordings have not been converted");
            return;
        }
        let mut failed = Vec::new();
        for path in &found {
            let file = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            if let Err(error) = decode(&data, &Source::Recording(file.clone())) {
                failed.push(format!("{file}: {error:#}"));
            }
        }
        assert!(failed.is_empty(), "{} of {} recordings cannot be played:\n{}", failed.len(), found.len(), failed.join("\n"));
        eprintln!("{} recordings decoded", found.len());
    }
    /// The bank of the sounds the field always has, and that of a map with a sound
    /// about it that loops, its first.
    const FIELD_BANK: &str = COMMON_BANK;
    const MAP_BANK: &str = "FLD/MAP/MAP_D_STG02_08.BIN/06.bin/00.pphd";

    /// A mixer with nothing to play from, for what needs no recordings.
    fn bare() -> Mixer {
        Mixer::new(GameData::new(DATA).unwrap_or_else(|_| GameData::nowhere()), FULL, Out::Off)
    }

    fn playing(name: &str, left: Option<f32>) -> Playing {
        Playing { started: true, left, ..Playing::new(Source::Recording(name.into())) }
    }

    #[test]
    fn a_volume_gets_where_it_is_going_in_the_blocks_it_is_given() {
        let mut volume = Glide::at(127.0);
        volume.go(100.0, 15.0);
        volume.step(7.5 * BLOCK);
        assert!((volume.now - 113.5).abs() < 1e-3);
        volume.step(7.5 * BLOCK);
        assert!((volume.now - 100.0).abs() < 1e-3);
        volume.step(1.0);
        assert_eq!(volume.now, 100.0);
        // Back up, and at once with no blocks to take.
        volume.go(127.0, 0.0);
        assert_eq!(volume.now, 127.0);
        // To nothing at once is what the original never gets to; a tune that
        // starts after it starts there.
        volume.go(0.0, 0.0);
        volume.step(1.0);
        assert_eq!(volume.now, 127.0);
        volume.arrive();
        assert_eq!(volume.now, 0.0);
    }

    #[test]
    fn a_sound_fades_out_in_its_blocks_and_ends_when_it_is_played_through() {
        let mut line = playing("line", Some(1.0));
        assert!(line.step(0.5));
        assert!(!line.step(0.6));

        // A tune loops: it goes on until it is faded out, over eight blocks here.
        let mut tune = playing("tune", None);
        assert!(tune.step(1000.0));
        tune.fade(8);
        assert!(tune.step(4.0 * BLOCK));
        assert!((tune.level - 0.5).abs() < 1e-4);
        let heard = tune.heard(0.5);
        assert!((heard[0] - 0.25).abs() < 1e-4 && heard[0] == heard[1]);
        // A slower fade asked for on top of it does not slow it.
        tune.fade(100);
        assert!(!tune.step(4.0 * BLOCK));
        // No blocks is at once, and a fade is never longer than 256 blocks.
        let mut tune = playing("tune", None);
        tune.fade(100_000);
        assert!(tune.step(255.0 * BLOCK));
        assert!(!tune.step(1.5 * BLOCK));
        let mut tune = playing("tune", None);
        tune.fade(0);
        assert!(!tune.step(0.0));
    }

    #[test]
    fn a_sound_to_one_side_falls_away_on_the_other() {
        assert_eq!(sides(MIDDLE), [1.0, 1.0]);
        assert_eq!(sides(0), [1.0, 0.0]);
        assert_eq!(sides(0x20), [1.0, 0.5]);
        assert_eq!(sides(0x5f), [0.5, 1.0]);
        // At the far right the left is all but gone, as in the original.
        assert_eq!(sides(0x7f), [0.0, 1.0]);
        assert_eq!(sides(500), sides(0x7f));
        let mut effect = playing("effect", None);
        (effect.volume, effect.place) = (0.5, 0x20);
        assert_eq!(effect.heard(0.5), [0.25, 0.125]);
    }

    #[test]
    fn what_is_held_neither_fades_nor_ends_until_it_is_let_go() {
        let mut mixer = bare();
        let mut sounds = Assets::<Pcm>::default();
        mixer.tune = Some(playing("tune", None));
        mixer.streams[0] = Some(playing("line", Some(1.0)));
        mixer.effects.insert(7, playing("effect", Some(0.5)));
        mixer.carry_out(Request::Hold);
        // What is asked for after that is not held: the pause's own sounds are
        // heard. And a tune told to fade while it is held waits to.
        mixer.effects.insert(8, playing("later", Some(0.5)));
        mixer.carry_out(Request::StopMusic(8));
        run(&mut mixer, &mut sounds, 10.0);
        assert!(mixer.tune.is_some() && mixer.streams[0].is_some() && mixer.effects.contains_key(&7));
        assert!(!mixer.effects.contains_key(&8));
        assert_eq!(mixer.tune.as_ref().map(|tune| tune.level), Some(1.0));
        assert_eq!(mixer.streams[0].as_ref().and_then(|line| line.left), Some(1.0));
        // Let go, each goes on from where it stood.
        mixer.carry_out(Request::LetGo);
        run(&mut mixer, &mut sounds, 0.6);
        assert!(mixer.tune.is_none() && mixer.streams[0].is_some() && !mixer.effects.contains_key(&7));
        run(&mut mixer, &mut sounds, 0.5);
        assert!(mixer.streams[0].is_none());
    }

    #[test]
    fn the_options_of_a_new_game_have_the_music_under_the_voices() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let volumes = Volumes::of(&Options::load(&data).unwrap());
        assert_eq!(volumes, Volumes { music: 60.0 / 127.0, effects: 1.0, voices: 1.0 });
    }

    /// Runs a mixer for `seconds` a thirtieth at a time, with nothing put out.
    fn run(mixer: &mut Mixer, sounds: &mut Assets<Pcm>, seconds: f32) {
        let mut play = |_: Pcm, _: Gains| -> Entity { unreachable!("a silent run plays nothing") };
        let steps = (seconds * 30.0).round() as usize;
        for _ in 0..steps {
            mixer.step(1.0 / 30.0, sounds, &mut play, &mut |_| {});
        }
    }

    /// Waits for what a mixer has asked to be decoded.
    fn settle(mixer: &mut Mixer, sounds: &mut Assets<Pcm>) {
        let mut play = |_: Pcm, _: Gains| -> Entity { unreachable!("a silent run plays nothing") };
        for _ in 0..3000 {
            if mixer.library.decoding.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            mixer.step(0.0, sounds, &mut play, &mut |_| {});
        }
        assert!(mixer.library.decoding.is_empty(), "the sounds were not decoded in half a minute");
    }

    #[test]
    fn a_tune_takes_the_place_of_another_and_a_line_makes_way_for_the_next() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        if !data.exists("SOUND/BGM_MIS_START.ogg") {
            eprintln!("skipped: the recordings have not been converted");
            return;
        }
        let mut mixer = Mixer::new(data, FULL, Out::Off);
        let mut sounds = Assets::<Pcm>::default();

        // The jingle a mission starts with: four seconds, and then it is over.
        mixer.carry_out(Request::Music(tune_file("mis_start")));
        assert_eq!(mixer.tune().as_deref(), Some("SOUND/BGM_MIS_START.AT3"));
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 3.9);
        assert!(mixer.tune().is_some());
        run(&mut mixer, &mut sounds, 0.3);
        assert_eq!(mixer.tune(), None);

        // A tune that loops goes on, until another takes its place at once.
        mixer.carry_out(Request::Music(tune_file("event4")));
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 120.0);
        assert_eq!(mixer.tune().as_deref(), Some("SOUND/BGM_EVENT4.AT3"));
        let event4 = &mixer.library.kept[&Source::Recording("SOUND/BGM_EVENT4.AT3".into())];
        let pcm = sounds.get(&event4.sound).unwrap();
        assert_eq!((pcm.frames(), pcm.channels(), pcm.rate()), (2_798_345, 2, 44_100));
        assert_eq!(pcm.looped(), Some((457_045, 2_798_344)));
        assert!(pcm.peak() > 0.2);
        mixer.carry_out(Request::Music(tune_file("battle1")));
        assert_eq!(mixer.tune().as_deref(), Some("SOUND/BGM_BATTLE1.AT3"));
        // It fades out over thirty blocks, which is 1.4 seconds.
        mixer.carry_out(Request::StopMusic(30));
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 1.3);
        assert!(mixer.tune().is_some());
        run(&mut mixer, &mut sounds, 0.2);
        assert_eq!(mixer.tune(), None);

        // The opening's first line, 1.8 seconds of it; the next asked for while it
        // is said starts when it has faded out, eight blocks on.
        let line = |number: usize| format!("FLD/FEVT/FEVT_1_1_0200B_1.BIN/05.bin/{number:02}.riff");
        mixer.carry_out(Request::Say(0, line(0)));
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 1.0);
        assert_eq!(mixer.said(0), Some(line(0)));
        mixer.carry_out(Request::Say(0, line(2)));
        run(&mut mixer, &mut sounds, 0.3);
        assert_eq!(mixer.said(0), Some(line(0)));
        run(&mut mixer, &mut sounds, 0.2);
        assert_eq!(mixer.said(0), Some(line(2)));
        // The other stream is its own; and a stream that is hushed drops what
        // waited for it too.
        assert_eq!(mixer.said(1), None);
        mixer.carry_out(Request::Say(0, line(3)));
        mixer.carry_out(Request::Hush(0, MAKE_WAY));
        run(&mut mixer, &mut sounds, 0.5);
        assert_eq!(mixer.said(0), None);
        // A recording there is no file of is over at once.
        mixer.carry_out(Request::Say(1, line(1)));
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 0.1);
        assert_eq!(mixer.said(1), None);
    }

    #[test]
    fn the_music_is_turned_down_under_a_jingle_and_is_back_when_that_is_over() {
        const JINGLE: &str = "SYSTEM.VOL/jin_pack.bin/000.riff";
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        if !data.exists("SYSTEM.VOL/jin_pack.bin/000.ogg") {
            eprintln!("skipped: the recordings have not been converted");
            return;
        }
        let mut mixer = Mixer::new(data, FULL, Out::Off);
        let mut sounds = Assets::<Pcm>::default();
        mixer.carry_out(Request::MusicVolume(100, 0.0));
        // As the results of a battle won have it (0x37B94, 0x38288): the jingle of
        // five seconds at 96 of 127, and the music at 1 under it.
        let begin = |mixer: &mut Mixer| {
            let results = Keeper::Results;
            for request in [Request::KeepMusicVolume(results), Request::MusicVolume(1, 0.0), Request::SayAt(0, JINGLE.into(), 0x60), Request::MusicVolumeBackAfter(0, results)] {
                mixer.carry_out(request);
            }
        };
        begin(&mut mixer);
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 4.9);
        assert_eq!(mixer.said(0).as_deref(), Some(JINGLE));
        assert_eq!(mixer.music_volume.now, 1.0);
        let heard = mixer.streams[0].as_ref().unwrap().heard(1.0);
        assert!((heard[0] - 96.0 / 127.0).abs() < 1e-6, "{heard:?}");
        run(&mut mixer, &mut sounds, 0.3);
        assert_eq!(mixer.said(0), None);
        assert_eq!(mixer.music_volume.now, 100.0);
        // Nothing more comes of it once it has.
        mixer.carry_out(Request::MusicVolume(40, 0.0));
        run(&mut mixer, &mut sounds, 1.0);
        assert_eq!(mixer.music_volume.now, 40.0);

        // The results done with before the jingle is over (0x37D30): it stops, and
        // the music is back at once.
        begin(&mut mixer);
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 1.0);
        assert_eq!(mixer.music_volume.now, 1.0);
        mixer.carry_out(Request::Hush(0, 0));
        mixer.carry_out(Request::MusicVolumeBack(Keeper::Results));
        run(&mut mixer, &mut sounds, 0.1);
        assert_eq!((mixer.said(0), mixer.music_volume.now), (None, 40.0));
        assert_eq!(mixer.back_after, None);
    }

    #[test]
    fn a_banks_sound_plays_through_and_one_that_loops_until_it_is_stopped() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut mixer = Mixer::new(data.clone(), FULL, Out::Off);
        let mut sounds = Assets::<Pcm>::default();
        let effect = |handle: u32, sound: u32| Request::Effect { handle, bank: FIELD_BANK.into(), sound, volume: 100, note: NOTE };

        // The bank's first sound: 3696 samples at 22050 a second, a sixth of one.
        mixer.carry_out(effect(1, 0));
        settle(&mut mixer, &mut sounds);
        let kept = &mixer.library.kept[&Source::Bank(FIELD_BANK.into(), 0)];
        let pcm = sounds.get(&kept.sound).unwrap();
        assert_eq!((pcm.frames(), pcm.channels(), pcm.rate(), pcm.looped()), (3696, 1, 22_050, None));
        assert!(pcm.peak() > 0.2);
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.contains_key(&1));
        // It is as loud as a tone is, at 100 of 127.
        let heard = mixer.effects[&1].heard(TONE_VOLUME);
        assert!((heard[0] - 0.78125 * 100.0 / 127.0).abs() < 1e-6 && heard[0] == heard[1]);
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.is_empty());

        // A sound that loops goes on until it is stopped; its volume and its place
        // can be changed meanwhile.
        let effect = |handle: u32, sound: u32| Request::Effect { handle, bank: MAP_BANK.into(), sound, volume: 100, note: NOTE };
        let looping = 0;
        mixer.carry_out(effect(2, looping));
        settle(&mut mixer, &mut sounds);
        let kept = &mixer.library.kept[&Source::Bank(MAP_BANK.into(), looping)];
        assert!(sounds.get(&kept.sound).unwrap().looped().is_some());
        run(&mut mixer, &mut sounds, 60.0);
        assert!(mixer.effects.contains_key(&2));
        mixer.carry_out(Request::EffectVolume(2, 127));
        mixer.carry_out(Request::EffectPan(2, 0));
        assert_eq!(mixer.effects[&2].heard(1.0), [1.0, 0.0]);
        mixer.carry_out(Request::StopEffect(2));
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.is_empty());
        // Letting the bank go stops what is playing of it.
        mixer.carry_out(effect(5, looping));
        mixer.carry_out(Request::DropBank(FIELD_BANK.into()));
        run(&mut mixer, &mut sounds, 0.5);
        assert!(mixer.effects.contains_key(&5));
        mixer.carry_out(Request::DropBank(MAP_BANK.into()));
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.is_empty());
        // Everything can be stopped at once, but for what is to be spared.
        mixer.carry_out(effect(8, looping));
        mixer.carry_out(effect(9, looping));
        mixer.carry_out(Request::StopEffects(vec![9]));
        run(&mut mixer, &mut sounds, 0.1);
        assert_eq!(mixer.effects.keys().copied().collect::<Vec<u32>>(), [9]);
        mixer.carry_out(Request::StopEffects(Vec::new()));
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.is_empty());
        // A sound the bank does not have is over at once, and so is one of a bank
        // that is not there.
        mixer.carry_out(effect(3, 9999));
        mixer.carry_out(Request::Effect { handle: 4, bank: "nowhere/00.pphd".into(), sound: 0, volume: 127, note: NOTE });
        settle(&mut mixer, &mut sounds);
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.is_empty());

        // An octave up a sound is over in half its time; and however high it is
        // asked for, it is played no faster than four times the hardware's rate.
        mixer.carry_out(Request::Effect { handle: 6, bank: FIELD_BANK.into(), sound: 0, volume: 127, note: NOTE + 12 });
        mixer.carry_out(Request::Effect { handle: 7, bank: FIELD_BANK.into(), sound: 0, volume: 127, note: NOTE + 60 });
        assert_eq!(mixer.effects[&6].pitch, 2.0);
        let mut play = |_: Pcm, _: Gains| -> Entity { unreachable!("a silent run plays nothing") };
        mixer.step(0.0, &mut sounds, &mut play, &mut |_| {});
        assert_eq!(mixer.effects[&7].pitch, 8.0);
        assert!((mixer.effects[&6].left.unwrap() - 3696.0 / 44_100.0).abs() < 1e-5);
        assert!((mixer.effects[&7].left.unwrap() - 3696.0 / 176_400.0).abs() < 1e-5);
        run(&mut mixer, &mut sounds, 0.1);
        assert!(mixer.effects.is_empty());
    }

    #[test]
    fn the_options_music_is_full_about_the_opening_film_and_what_it_was_after() {
        let mut mixer = bare();
        mixer.volumes.music = 60.0 / 127.0;
        assert_eq!(mixer.music_share(), 60.0 / 127.0);
        mixer.carry_out(Request::MusicOptionFull(true));
        assert_eq!(mixer.music_share(), 1.0);
        // What the player sets meanwhile is what comes back.
        mixer.volumes.music = 0.25;
        assert_eq!(mixer.music_share(), 1.0);
        mixer.carry_out(Request::MusicOptionFull(false));
        assert_eq!(mixer.music_share(), 0.25);
    }

    #[test]
    fn the_volume_asked_of_the_music_is_what_its_tune_is_played_at() {
        let mut mixer = bare();
        mixer.volumes.music = 0.5;
        mixer.carry_out(Request::MusicVolume(100, 15.0));
        mixer.music_volume.step(15.0 * BLOCK);
        assert_eq!(mixer.music_volume.now, 100.0);
        // More than full is full, and less than nothing is nothing.
        mixer.carry_out(Request::MusicVolume(300, 0.0));
        assert_eq!(mixer.music_volume.now, 127.0);
        mixer.carry_out(Request::MusicVolume(-5, 1.0));
        assert_eq!(mixer.music_volume.to, 0.0);
        // A stream that is not there is not played on, nor a sound that is not
        // playing changed.
        mixer.carry_out(Request::Say(7, "x".into()));
        mixer.carry_out(Request::Hush(7, 0));
        mixer.carry_out(Request::StopEffect(7));
        mixer.carry_out(Request::EffectVolume(7, 1));
        mixer.carry_out(Request::EffectPan(7, 1));
        assert!(mixer.tune.is_none() && mixer.effects.is_empty());
        // Sounds asked for without a number are given numbers of the mixer's own.
        mixer.carry_out(Request::common(3));
        mixer.carry_out(Request::common(3));
        assert_eq!(mixer.effects.keys().copied().collect::<Vec<u32>>(), [OWN_HANDLES, OWN_HANDLES + 1]);
        assert_eq!(mixer.effects[&OWN_HANDLES].source, Source::Bank(COMMON_BANK.into(), 3));
    }

    #[test]
    fn the_musics_volume_is_put_by_and_gone_back_to() {
        let mut mixer = bare();
        // What is put by is the volume last asked for, wherever the music has got
        // to on its way there.
        mixer.carry_out(Request::MusicVolume(90, 30.0));
        mixer.carry_out(Request::KeepMusicVolume(Keeper::Failed));
        mixer.carry_out(Request::MusicVolume(1, 0.0));
        assert_eq!(mixer.music_volume.now, 1.0);
        mixer.carry_out(Request::MusicVolumeBack(Keeper::Failed));
        assert_eq!((mixer.music_volume.now, mixer.music_volume.to), (90.0, 90.0));
        // With nothing put by it is full.
        assert_eq!(bare().music_kept, [FULL_VOLUME as f32; 4]);
    }
}
