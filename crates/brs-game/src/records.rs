//! The game's challenges, which it calls records: `SYSTEM.VOL/rec_script.ss` and
//! what the engine does around it (0x14A5F8 to 0x14BA30).
//!
//! A record is something to do so many times: beat so many of a kind of enemy,
//! fight so many battles, clear a mission in some way. Its count goes up as the
//! game tells of what has happened, and reaching its goal clears it, which earns
//! what it is worth: a skill, an item, another record, a flag (the first one's is
//! the flag that lets her dodge). All of that is the script's doing. The engine
//! only tells it things by calling its functions by name (0x14B6DC), each on a
//! machine of its own with the field's table of engine functions:
//!
//! - `rec_initialize` as a game is started (0x13C);
//! - `update_kill_enemy(event)` as a hit of hers ends an enemy (0x9BDAC), and
//!   `rec_update_btl_finish(event)` as the results of a battle she has come out
//!   of alive are made (0x9BD28);
//! - `rec_update_level_up` as the results give her a level (0x37F24);
//! - `rec_inc_counter(record)` and `rec_unlock(record)` for a map's script (its
//!   functions 274 and 277);
//! - `rec_clear_reward(record)`, which shows a record's window and gives what it
//!   is worth (0x14B92C).
//!
//! It asks for `rec_update_guard`, `rec_update_dodge`, `rec_update_overheat` and
//! `rec_update_mis_finish` too, which this script does not have: nothing comes of
//! those. And it tells the script nothing at all while flag 6156 is set, which it
//! is through the story's opening.
//!
//! What a record has come to is kept in the save's flags and variables, by its
//! number: see [`flag`] and [`COUNT`].

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_formats::script::{self, Call, Machine, Run, Script};

use crate::battle::tables::PlayerStats;
use crate::data::GameData;
use crate::gallery::Gallery;
use crate::items::{Inventory, Items};
use crate::progress::Progress;
use crate::record_window::RecordWindow;
use crate::sound::Board;
use crate::text::{self, TextColours};
use crate::writing::Fonts;

const SCRIPT: &str = "SYSTEM.VOL/rec_script.ss";
/// The texts of the field's system script, which what a record is worth is said
/// with (0x2118, 0xE7310): the first table of the field pack's last part.
const TEXTS: &str = "SYSTEM.VOL/fld_pack.bin/09.bin/00.bin";

/// A record of the script's table `lst_record`.
const RECORD_LEN: usize = 60;
/// The most records there can be: the flags and variables have room for so many
/// (0x14AEB4).
pub const MOST: i32 = 128;
/// While this flag is set the script is told nothing (0x14B700).
const NOTHING_TOLD: i32 = 6156;

/// The flags a record has, each at its number from these.
pub mod flag {
    /// It is open: its count goes up (0x14A940).
    pub const OPEN: i32 = 6600;
    /// It is cleared (0x14A978).
    pub const CLEARED: i32 = 6800;
    /// Its count has come to something to tell of (0x14A85C).
    pub const NEWS: i32 = 9000;
    /// It has been cleared and not yet shown as cleared (0x14A908).
    pub const CLEARED_NOW: i32 = 9128;
    /// Its count has gone up (0x14A9CC).
    pub const COUNTED: i32 = 9256;
    /// It has been opened and not yet told of (0x14A89C); and not yet shown as
    /// opened (0x14A8E4). These two are not of the save.
    pub const OPENED_NEWS: i32 = 100_000;
    pub const OPENED_NOW: i32 = 100_128;
}
/// The variable a record's count is, at its number from this (0x14A9E8).
pub const COUNT: i32 = 20_000;

/// Where the save's backup keeps what a record has come to (0x14AF08): whether it
/// is cleared, whether it is open, and its count.
mod kept {
    pub const CLEARED: i32 = 9384;
    pub const OPEN: i32 = 9512;
    pub const COUNT: i32 = 20_200;
}

/// A record's own flags (its byte 10).
pub mod trait_of {
    /// Open from the start.
    pub const OPEN: u8 = 1;
    /// Put back as it was when a mission that was failed is left (0x14AFC0).
    pub const PUT_BACK: u8 = 4;
    /// Only on its own stage.
    pub const OF_ITS_STAGE: u8 = 8;
}

// The engine functions the script calls, of the field's table (0x1726D0).
mod native {
    pub const VARIABLE: u32 = 0x13;
    pub const FLAG: u32 = 0x14;
    pub const SET_FLAG: u32 = 0x15;
    pub const VALUE: u32 = 0x16;
    pub const FIGURE: u32 = 0x17;
    pub const SET_VARIABLE: u32 = 0x18;
    pub const PRINT: u32 = 0x26;
    pub const LOG: u32 = 0x39;
    /// Whether a record's count can go up (0x14A6A4).
    pub const LIVE: u32 = 0x5e;
    /// Looks through a table for what an event counts for (0x14A6E0).
    pub const MATCH: u32 = 0x5f;

    // What `rec_clear_reward` works its window and gives a record's worth with.
    /// Sets one of her figures (0xB98).
    pub const SET_FIGURE: u32 = 0x4c;
    /// What a message is to name: a thing by its kind and number (0xEC5F8), or
    /// words of the script's own (0xEC7C4).
    pub const MESSAGE_NAME: u32 = 0x1a;
    pub const MESSAGE_WORDS: u32 = 0x74;
    /// Gives her so many of an item (0xECB30).
    pub const GIVE: u32 = 0x56;
    /// The name of a thing, for what the script prints (0xEC5A0).
    pub const NAME: u32 = 0x57;
    /// A message of one of three kinds, which is waited for (0x1303C4).
    pub const MESSAGE: u32 = 0x63;
    /// Makes a screen of a kind (0x130138), and waits for it to be made (0x1305C8).
    pub const SCREEN: u32 = 0x6c;
    pub const WAIT_FOR_SCREEN: u32 = 0x6d;
    /// A piece of the gallery is opened (0x14A818).
    pub const GALLERY: u32 = 0x6f;
    /// Tells a screen something (0x13028C), and waits for it (0x130324).
    pub const TELL_SCREEN: u32 = 0x70;
    pub const WAIT_ON_SCREEN: u32 = 0x71;
}

/// The kind of screen the record's window is (0x130168), what it can be told
/// (0x130218): to show the record that the variable names, and to go.
const SCREEN_OF_RECORDS: i32 = 2;
const TOLD_TO_SHOW: i32 = 0;
const TOLD_TO_GO: i32 = 1;
const SHOWN_RECORD: i32 = 100_000;
/// The kinds of message (0x130490): in the window a script puts up, in a panel,
/// and the record's window shown again.
const MESSAGE_IN_A_WINDOW: i32 = 0;
const MESSAGE_AS_WINDOW: i32 = 2;
/// The code in a text that stands for words given beforehand (0x12FBE8): two
/// figures follow it, which of them.
const WORDS_GIVEN: char = '\x1a';
/// How many such words a text can have (0x12FBB4 keeps them by number).
const WORDS: usize = 8;
// The kinds of thing whose name can be given so (0xECD04), of those the script
// asks for: an item, a text of the system script's, a record.
const NAME_OF_ITEM: i32 = 1;
const NAME_OF_TEXT: i32 = 2;
const NAME_OF_RECORD: i32 = 10;

/// The one of the field's figures the script reads (0x16): the stage the map she
/// is on is of.
const VALUE_STAGE: i32 = 61;
/// Her figures it reads (0x9C4): her level, and the highest level there is, which
/// is fifty whatever is asked of it (0xB58). The record of the highest level is
/// counted by a level gained that is not less.
const FIGURE_LEVEL: i32 = 1;
const FIGURE_MOST_LEVELS: i32 = 22;
const MOST_LEVELS: u32 = 50;
/// And the skills in her four slots (0xB60: the save's fields 11 to 14), which
/// the script looks through for a skill that one made better takes the place of.
const FIGURE_SLOTS: std::ops::RangeInclusive<i32> = 13..=16;

/// Where in a machine's memory what the script is told of is put, and after it a
/// word of nothing: the script is handed its address as that of whether it is to
/// say what it does, which nothing asks of it (0x14B710).
const EVENT_AT: u32 = 0x2000_0000;
const EVENT_LEN: usize = 20;
const QUIET_AT: u32 = EVENT_AT + EVENT_LEN as u32;

/// Something that has happened, as the script is told of it (0x14ADC8): what it
/// names is nothing unless set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// Bit 0: she came through the battle unharmed. Bit 2: the enemy was beaten
    /// while its first gauge was broken. Bit 3: the group is one she strikes her
    /// pose over.
    pub flags: u32,
    pub enemy: i32,
    pub item: i32,
    pub skill: i32,
    pub group: i32,
}

impl Event {
    const NOTHING: Event = Event { flags: 0, enemy: -1, item: -1, skill: -1, group: -1 };

    /// A hit of hers has ended an enemy (0x9BDAC): the kind of enemy it was,
    /// whether its first gauge was broken as the hit was done with, and the skill
    /// the hit was of, nought for none.
    pub fn kill(enemy: i32, broken: bool, skill: i32) -> Event {
        Event { flags: u32::from(broken) << 2, enemy, skill, ..Event::NOTHING }
    }

    /// She has come out of a battle alive (0x9BD28): the group it was against,
    /// whether that is one she strikes her pose over (0x14AE74), and whether
    /// nothing took any HP off her in it.
    pub fn battle(group: i32, pose: bool, unharmed: bool) -> Event {
        Event { flags: u32::from(pose) << 3 | u32::from(unharmed), group, ..Event::NOTHING }
    }

    fn bytes(&self) -> Vec<u8> {
        let words = [self.flags, self.enemy as u32, self.item as u32, self.skill as u32, self.group as u32];
        let mut bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        bytes.extend(0u32.to_le_bytes());
        bytes
    }
}

/// The kind of thing a record is worth that is a flag set, and the kind that is
/// a group of the gallery's things opened.
const WORTH_A_FLAG: u8 = 6;
const WORTH_OF_THE_GALLERY: u8 = 7;
/// The first of the records, "First Challenge": four battles, for the flag that
/// lets her dodge.
pub const FIRST: i32 = 0;

/// What a record is worth, one of up to three things.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reward {
    /// 1 an item, 2 a skill, 5 another record opened, 6 a flag set, 7 a piece of
    /// the gallery, 8 a skill made better, 9 only what is said.
    pub kind: u8,
    pub numbers: (i32, i32),
    /// What is said of it, if not what its kind is said with.
    pub words: Option<String>,
}

/// A record as the script's table has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    /// The count that clears it, and the span of counts between one telling of
    /// how far it is and the next.
    pub goal: i32,
    pub span: i32,
    /// 0 for one of battles, 1 for one of missions.
    pub category: u8,
    pub stage: u8,
    /// See [`trait_of`].
    pub traits: u8,
    /// Which of ten pictures it is shown with.
    pub icon: u8,
    /// What is to be done, what kind of thing that is, and what it is worth.
    pub words: [String; 3],
    pub rewards: Vec<Reward>,
}

/// What the script's functions work on besides the records themselves.
pub struct World<'a> {
    pub progress: &'a mut Progress,
    pub stats: &'a mut PlayerStats,
    /// The stage the map she is on is of.
    pub stage: i32,
}

/// The records' script.
#[derive(Resource, Clone)]
pub struct Records {
    script: Arc<Script>,
    count: i32,
    /// The system script's texts, by number.
    texts: Arc<Vec<Option<String>>>,
    /// What the gallery has, of which a record's reward opens a group.
    gallery: Arc<Gallery>,
}

/// Reads a table of texts (0xE7390, 0xE73A4): after its first word, where each
/// text is, or nought for one there is none of.
fn texts(table: &[u8]) -> Result<Vec<Option<String>>> {
    let word = |at: usize| table.get(at..at + 4).map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()) as usize);
    let mut read = Vec::new();
    // The places run up to the first of the texts.
    let mut end = table.len();
    while 4 + 4 * read.len() < end {
        let at = word(4 + 4 * read.len()).context("the table of texts is cut short")?;
        if at == 0 {
            read.push(None);
            continue;
        }
        let text = table.get(at..).context("a text lies outside its table")?;
        end = end.min(at);
        let text = &text[..text.iter().position(|&byte| byte == 0).unwrap_or(text.len())];
        read.push(Some(String::from_utf8_lossy(text).into_owned()));
    }
    Ok(read)
}

impl Records {
    pub fn load(data: &GameData) -> Result<Records> {
        let script = data.script(SCRIPT)?;
        let count = script.integer("n_record").context("the records' script does not say how many there are")? as i32;
        script.symbol("lst_record").context("the records' script has no table of them")?;
        let texts = texts(&data.read(TEXTS)?).with_context(|| format!("reading {TEXTS}"))?;
        let gallery = Gallery::load(data).context("reading the gallery's tables")?;
        Ok(Records { script: Arc::new(script), count: count.clamp(0, MOST), texts: Arc::new(texts), gallery: Arc::new(gallery) })
    }

    /// A text of the system script's (0xE73A4).
    pub(crate) fn text(&self, number: i32) -> Option<&str> {
        self.texts.get(usize::try_from(number).ok()?)?.as_deref()
    }

    /// How many records there are (0x14B658).
    pub fn count(&self) -> i32 {
        self.count
    }

    /// A record of the table (0x14B688).
    pub fn record(&self, number: i32) -> Option<Record> {
        let script = &self.script;
        let index = usize::try_from(number).ok().filter(|_| number < self.count)?;
        let record = script.data(script.symbol("lst_record")?)?.get(index * RECORD_LEN..(index + 1) * RECORD_LEN)?;
        let word = |at: usize| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let short = |at: usize| i16::from_le_bytes(record[at..at + 2].try_into().unwrap()) as i32;
        let text = |at: usize| Some(word(at)).filter(|&address| address != 0).and_then(|address| script.string(address));
        let rewards = (0..3).map(|nth| 24 + nth * 12).filter(|&at| record[at] != 0);
        Some(Record {
            name: text(0).unwrap_or_default(),
            goal: short(4),
            span: short(6),
            category: record[8],
            stage: record[9],
            traits: record[10],
            icon: record[11],
            words: [12, 16, 20].map(|at| text(at).unwrap_or_default()),
            rewards: rewards.map(|at| Reward { kind: record[at], numbers: (short(at + 2), word(at + 4) as i32), words: text(at + 8) }).collect(),
        })
    }

    /// Whether a record is open (0x14AD8C): from the start, or since it was opened.
    pub fn is_open(&self, number: i32, progress: &Progress) -> bool {
        self.record(number).is_some_and(|record| record.traits & trait_of::OPEN != 0) || progress.flag(flag::OPEN + number)
    }

    /// Whether a record's count can go up (0x14A5F8): it is not cleared, and if it
    /// is of its stage only, the map she is on is of that stage.
    pub fn live(&self, number: i32, progress: &Progress, stage: i32) -> bool {
        if number < 0 || number >= self.count || progress.flag(flag::CLEARED + number) {
            return false;
        }
        self.record(number).is_some_and(|record| record.traits & trait_of::OF_ITS_STAGE == 0 || i32::from(record.stage) == stage)
    }

    /// A machine for the script, with what it is told of in its memory.
    fn machine(&self, event: &[u8]) -> Machine {
        let mut machine = Machine::new(&self.script);
        machine.map(EVENT_AT, event.to_vec());
        machine
    }

    /// The records as a game starts with them (0x14B9C8): those that are open
    /// from the start are opened.
    pub fn initialize(&self, world: &mut World) -> Result<()> {
        let Some(function) = self.script.symbol("rec_initialize") else { return Ok(()) };
        let event = Event::NOTHING.bytes();
        let mut host = Host { records: self, world, event: &event, unported: BTreeSet::new() };
        self.machine(&event).call(function, &[], &mut host).context("in the records' rec_initialize").map(|_| ())
    }

    /// Tells the script of something by one of its functions (0x14B6DC), which is
    /// given `argument` and where to find whether it is to say what it does.
    fn tell(&self, name: &str, argument: Option<i32>, event: Event, world: &mut World) -> Result<()> {
        let event = event.bytes();
        let mut host = Host { records: self, world, event: &event, unported: BTreeSet::new() };
        host.tell(name, argument.map_or(EVENT_AT, |argument| argument as u32))
    }

    /// A hit of hers has ended an enemy (0x14B830).
    pub fn enemy_beaten(&self, event: Event, world: &mut World) -> Result<()> {
        self.tell("update_kill_enemy", None, event, world)
    }

    /// She has come out of a battle alive (0x14B854).
    pub fn battle_finished(&self, event: Event, world: &mut World) -> Result<()> {
        self.tell("rec_update_btl_finish", None, event, world)
    }

    /// She has gone up a level (0x14B908).
    pub fn level_up(&self, world: &mut World) -> Result<()> {
        self.tell("rec_update_level_up", Some(0), Event::NOTHING, world)
    }

    /// A record's count goes up by one (0x14B9E8), if it is open.
    pub fn count_up(&self, number: i32, world: &mut World) -> Result<()> {
        self.tell("rec_inc_counter", Some(number), Event::NOTHING, world)
    }

    /// A record is opened (0x14BA0C).
    pub fn open(&self, number: i32, world: &mut World) -> Result<()> {
        self.tell("rec_unlock", Some(number), Event::NOTHING, world)
    }

    /// Keeps what every record has come to (0x14AF08), as a mission is chosen.
    pub fn back_up(&self, progress: &mut Progress) {
        for number in 0..MOST {
            let (cleared, open, count) = (progress.flag(flag::CLEARED + number), progress.flag(flag::OPEN + number), progress.variable(COUNT + number));
            progress.set_flag(kept::CLEARED + number, cleared);
            progress.set_flag(kept::OPEN + number, open);
            progress.set_variable(kept::COUNT + number, count);
        }
    }

    /// Puts back what was kept (0x14AF74), as a mission that was failed is left:
    /// only what a record that is not cleared had come to, and only of those that
    /// are put back.
    pub fn restore(&self, progress: &mut Progress) {
        for number in 0..self.count {
            let put_back = self.record(number).is_some_and(|record| record.traits & trait_of::PUT_BACK != 0);
            if progress.flag(flag::CLEARED + number) || !put_back {
                continue;
            }
            let (cleared, open, count) = (progress.flag(kept::CLEARED + number), progress.flag(kept::OPEN + number), progress.variable(kept::COUNT + number));
            progress.set_flag(flag::CLEARED + number, cleared);
            progress.set_flag(flag::OPEN + number, open);
            progress.set_variable(COUNT + number, count);
        }
    }

    /// Forgets what there was to tell of every record's count (0x14AEB4), as a
    /// mission is chosen.
    pub fn forget_news(&self, progress: &mut Progress) {
        for number in 0..MOST {
            for flag in [flag::NEWS, flag::CLEARED_NOW, flag::COUNTED] {
                progress.set_flag(flag + number, false);
            }
        }
    }

    /// A record taken as cleared in a game the port starts in the middle of: it
    /// is open and cleared with its count at its goal, and the flags it is worth
    /// are set. Nothing of it is left to be shown.
    pub fn take_as_cleared(&self, number: i32, progress: &mut Progress) {
        let Some(record) = self.record(number) else { return };
        progress.set_flag(flag::OPEN + number, true);
        progress.set_flag(flag::CLEARED + number, true);
        progress.set_variable(COUNT + number, record.goal);
        for reward in record.rewards.iter().filter(|reward| reward.kind == WORTH_A_FLAG) {
            progress.set_flag(reward.numbers.0, true);
        }
        for reward in record.rewards.iter().filter(|reward| reward.kind == WORTH_OF_THE_GALLERY) {
            self.gallery.open_group(reward.numbers.0, true, progress);
        }
    }
}

/// What the script's engine functions work with while one of its functions runs.
struct Host<'a, 'w> {
    records: &'a Records,
    world: &'a mut World<'w>,
    /// What every machine made meanwhile has in its memory.
    event: &'a [u8],
    /// Engine functions that were called and are not ported, so that each is told
    /// of once.
    unported: BTreeSet<u32>,
}

impl Host<'_, '_> {
    /// Runs one of the script's functions by name on a machine of its own
    /// (0x14B6DC). One the script does not have is not run, and none is while the
    /// story's opening has its flag set.
    fn tell(&mut self, name: &str, argument: u32) -> Result<()> {
        if self.world.progress.flag(NOTHING_TOLD) {
            return Ok(());
        }
        let Some(function) = self.records.script.symbol(name) else { return Ok(()) };
        let mut machine = self.records.machine(self.event);
        machine.call(function, &[argument, QUIET_AT], self).with_context(|| format!("in the records' {name}")).map(|_| ())
    }

    /// Looks through a table of the script's for what an event counts for
    /// (0x14A6E0). Each of its entries begins with a record's number: for every
    /// one whose record's count can go up, the script's own function says whether
    /// the event is one for that entry, and if it is the record's count goes up.
    fn count_matches(&mut self, call: &Call) -> Result<u32> {
        let (table, size, entries) = (call.int(0)? as u32, call.int(1)? as u32, call.int(2)?);
        let (compare, event) = (call.int(3)? as u32, call.int(4)? as u32);
        for index in 0..entries.max(0) as u32 {
            let entry = table + index * size;
            let record = call.machine().read_u32(entry)? as i32;
            if !self.records.live(record, self.world.progress, self.world.stage) {
                continue;
            }
            // The original runs it on the machine that asked, whose state it puts
            // by meanwhile (0x113644): a machine of its own comes to the same.
            let mut machine = self.records.machine(self.event);
            if machine.call(compare, &[entry, event], self).context("in a record's test of an event")? != 0 {
                self.tell("rec_inc_counter", record as u32)?;
            }
        }
        Ok(0)
    }
}

impl script::Host for Host<'_, '_> {
    fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
        let int = |index: u32| call.int(index);
        let progress = &mut *self.world.progress;
        Ok(match number {
            native::VARIABLE => progress.variable(int(0)?) as u32,
            native::SET_VARIABLE => {
                progress.set_variable(int(0)?, int(1)?);
                0
            }
            native::FLAG => u32::from(progress.flag(int(0)?)),
            native::SET_FLAG => {
                progress.set_flag(int(0)?, int(1)? != 0);
                0
            }
            native::VALUE => match int(0)? {
                VALUE_STAGE => self.world.stage as u32,
                other => {
                    debug!("the records' script reads the field's figure {other}, which is not kept here");
                    0
                }
            },
            native::FIGURE => match int(0)? {
                FIGURE_LEVEL => self.world.stats.level as u32,
                FIGURE_MOST_LEVELS => MOST_LEVELS,
                slot if FIGURE_SLOTS.contains(&slot) => u32::from(self.world.stats.skills[(slot - FIGURE_SLOTS.start()) as usize]),
                other => {
                    debug!("the records' script reads her figure {other}, which is not ported");
                    0
                }
            },
            // What the script would say of what it does, to nobody.
            native::PRINT | native::LOG => 0,
            native::LIVE => u32::from(self.records.live(int(0)?, progress, self.world.stage)),
            native::MATCH => self.count_matches(call)?,
            other => {
                if self.unported.insert(other) {
                    warn!("the records' script calls engine function {other:#x}, which is not ported");
                }
                0
            }
        })
    }
}

/// What `rec_clear_reward` works besides the records: the window a record is
/// shown in, and what a record's worth is given into.
pub struct Screen<'a> {
    pub window: &'a mut RecordWindow,
    pub inventory: &'a mut Inventory,
    pub items: &'a Items,
    pub fonts: &'a Fonts,
    pub colours: &'a TextColours,
    pub board: &'a mut Board,
}

/// A record being shown (0x14B92C): the script's `rec_clear_reward` on a machine
/// of its own, which goes on a frame at a time (0x130BE0). It shows the record's
/// window, waits for cross, gives what a record that is cleared is worth, and has
/// the window go.
pub struct Showing {
    machine: Machine,
    record: i32,
    started: bool,
    /// The words a message's text is to have put in, by number (0x12FBB4).
    words: [String; WORDS],
}

impl Records {
    /// Starts showing a record; nothing if the script cannot.
    pub fn showing(&self, record: i32) -> Option<Showing> {
        self.script.symbol("rec_clear_reward")?;
        Some(Showing { machine: self.machine(&Event::NOTHING.bytes()), record, started: false, words: Default::default() })
    }
}

impl Showing {
    /// A frame of it. False once the script has run to its end.
    pub fn step(&mut self, records: &Records, world: &mut World, screen: &mut Screen) -> Result<bool> {
        let Some(function) = records.script.symbol("rec_clear_reward") else { return Ok(false) };
        let event = Event::NOTHING.bytes();
        let mut host = ScreenHost { host: Host { records, world, event: &event, unported: BTreeSet::new() }, screen, words: &mut self.words };
        let run = if std::mem::replace(&mut self.started, true) {
            self.machine.resume(&mut host)
        } else {
            self.machine.start(function, &[self.record as u32], &mut host)
        };
        Ok(run.context("in the records' rec_clear_reward")? == Run::Waiting)
    }
}

/// The lists of records there are to show (0x14AAE4): those of battles, which a
/// battle's results show, and those of missions, which a map's script has shown.
pub const OF_BATTLES: i32 = 0;
pub const OF_MISSIONS: i32 = 1;

/// The records of a list being shown one after another, each in its window
/// (0x37F8C for a battle's results, 0xEF790 for the field). The list is asked
/// anew for each next one, by its place: what the showing of one opens can put
/// another before those that are left.
pub struct Shown {
    list: i32,
    /// How many are shown: as many as the list had as the showing began, which
    /// the results go by (0x38758); the field goes by as many as it has (0xEF870).
    most: Option<usize>,
    next: usize,
    showing: Option<Showing>,
}

impl Shown {
    pub fn begin(list: i32, most: Option<usize>) -> Shown {
        Shown { list, most, next: 0, showing: None }
    }

    /// A frame of it: the record being shown goes on, or the next has its turn.
    /// False once there is no more to show.
    pub fn step(&mut self, records: &Records, world: &mut World, screen: &mut Screen) -> bool {
        if let Some(showing) = &mut self.showing {
            match showing.step(records, world, screen) {
                Ok(true) => {}
                Ok(false) => self.showing = None,
                Err(error) => {
                    error!("the records' script failed: {error:#}");
                    screen.window.shut();
                    self.showing = None;
                }
            }
            return true;
        }
        let list = records.list(self.list, world.progress);
        let Some(&record) = list.get(self.next).filter(|_| self.most.is_none_or(|most| self.next < most)) else { return false };
        self.next += 1;
        info!("record {record} is shown");
        // Its turn has come: it is held back no longer (0x38024).
        screen.window.reveal(record);
        self.showing = records.showing(record);
        self.showing.is_some()
    }
}

/// What the script's engine functions work with while a record is shown.
struct ScreenHost<'a, 'w, 's> {
    host: Host<'a, 'w>,
    screen: &'a mut Screen<'s>,
    words: &'a mut [String; WORDS],
}

/// A text with the words it asks for put in (0x12FBE8).
pub(crate) fn with_words(text: &str, words: &[String]) -> String {
    let mut whole = String::new();
    let mut rest = text.chars();
    while let Some(character) = rest.next() {
        if character != WORDS_GIVEN {
            whole.push(character);
            continue;
        }
        let which: String = rest.by_ref().take(2).collect();
        if let Some(given) = usize::from_str_radix(&which, 16).ok().and_then(|which| words.get(which)) {
            whole += given;
        }
    }
    whole
}

impl script::Host for ScreenHost<'_, '_, '_> {
    fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
        let int = |index: u32| call.int(index);
        let (world, screen) = (&mut *self.host.world, &mut *self.screen);
        let show = |screen: &mut Screen| {
            let record = world.progress.variable(SHOWN_RECORD);
            screen.window.show(record, self.host.records, world.progress, screen.fonts, screen.colours, screen.board);
        };
        Ok(match number {
            // The window is there to be shown: there is nothing to make or wait for.
            native::SCREEN | native::WAIT_FOR_SCREEN => 0,
            native::TELL_SCREEN => {
                if int(0)? == SCREEN_OF_RECORDS {
                    match int(1)? {
                        TOLD_TO_SHOW => show(screen),
                        TOLD_TO_GO => screen.window.leave(),
                        _ => {}
                    }
                }
                0
            }
            native::WAIT_ON_SCREEN => {
                if int(0)? == SCREEN_OF_RECORDS && screen.window.busy() {
                    call.wait();
                }
                0
            }
            native::MESSAGE => {
                match int(0)? {
                    MESSAGE_AS_WINDOW => {
                        if !call.resumed() {
                            show(screen);
                        }
                        if screen.window.busy() {
                            call.wait();
                        }
                    }
                    // In the window a script puts up, which is read with cross
                    // (0x6BAFC, 0x6BCB0; it is waited for at 0x6BDE0).
                    MESSAGE_IN_A_WINDOW => {
                        if !call.resumed() {
                            let text = with_words(self.host.records.text(int(1)?).unwrap_or_default(), self.words);
                            info!("a record's message: {}", text.replace('\n', " / "));
                            screen.window.say(text::pages(&text, &text::Plain));
                        }
                        if screen.window.saying() {
                            call.wait();
                        }
                    }
                    // The panel is another way of saying it that this script
                    // has no use for.
                    kind => debug!("a record's message of kind {kind}, text {}, is not shown", int(1)?),
                }
                0
            }
            // The words a message's text is to have put in: the script's own
            // (0xEC7C4), or the name of a thing (0xEC5F8).
            native::MESSAGE_WORDS => {
                let given = self.host.records.script.string(int(1)? as u32).unwrap_or_default();
                if let Some(words) = usize::try_from(int(0)?).ok().and_then(|which| self.words.get_mut(which)) {
                    *words = given;
                }
                0
            }
            native::MESSAGE_NAME => {
                let (kind, number) = (int(1)?, int(2)?);
                let name = match kind {
                    NAME_OF_ITEM => screen.items.name(number).to_owned(),
                    NAME_OF_TEXT => self.host.records.text(number).unwrap_or_default().to_owned(),
                    NAME_OF_RECORD => self.host.records.record(number).map(|record| record.name).unwrap_or_default(),
                    _ => {
                        debug!("the records' script asks for the name of thing {number} of kind {kind}, which is not ported");
                        String::new()
                    }
                };
                if let Some(words) = usize::try_from(int(0)?).ok().and_then(|which| self.words.get_mut(which)) {
                    *words = name;
                }
                0
            }
            native::NAME => 0,
            // With the second argument nought (0xECB6C).
            native::GIVE => {
                let (item, count) = (int(0)?, int(2)?);
                if int(1)? == 0 {
                    screen.inventory.add(screen.items, item, count);
                    info!("a record cleared gives {count} of {}", screen.items.name(item));
                }
                0
            }
            // Every thing of the gallery that is of the group is opened (0x19AC).
            native::GALLERY => {
                let group = int(0)?;
                info!("a record cleared opens the gallery's group {group}");
                self.host.records.gallery.open_group(group, true, self.host.world.progress);
                0
            }
            // Her skills' slots are all it sets (0x4680): a skill made better
            // takes the slot of the one it was made from.
            native::SET_FIGURE => {
                let (figure, value) = (int(0)?, int(1)?);
                if FIGURE_SLOTS.contains(&figure) {
                    world.stats.skills[(figure - FIGURE_SLOTS.start()) as usize] = value.clamp(0, 127) as u8;
                } else {
                    debug!("the records' script sets her figure {figure} to {value}, which is not ported");
                }
                0
            }
            other => self.host.native(other, call)?,
        })
    }
}

// What the engine keeps of the records' news, apart from the script (0x14AA04 to
// 0x14AEB4).

/// What there is to tell of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum News {
    /// It has been opened.
    Opened,
    /// Its count has come to so much of its goal.
    Counted,
}

impl Records {
    /// The first record there is something to tell of, and what (0x14AA04): one
    /// newly opened comes before one whose count has news.
    pub fn news(&self, progress: &Progress) -> Option<(i32, News)> {
        (0..self.count).find_map(|number| {
            if progress.flag(flag::OPENED_NEWS + number) {
                Some((number, News::Opened))
            } else {
                progress.flag(flag::NEWS + number).then_some((number, News::Counted))
            }
        })
    }

    /// The same, as what is told of it is told (0x14B11C, 0x14B198): that news
    /// is forgotten.
    pub fn take_news(&self, progress: &mut Progress) -> Option<(i32, News)> {
        let (number, news) = self.news(progress)?;
        let flag = if news == News::Opened { flag::OPENED_NEWS } else { flag::NEWS };
        progress.set_flag(flag + number, false);
        Some((number, news))
    }

    /// Whether a record is of a list (0x14AAE4): of list 2 if its count has gone
    /// up; of the list of a category if it has been opened and not shown, or is of
    /// that category and has been cleared and not shown.
    pub fn listed(&self, number: i32, list: i32, progress: &Progress) -> bool {
        if list == 2 {
            return progress.flag(flag::COUNTED + number);
        }
        progress.flag(flag::OPENED_NOW + number)
            || (self.record(number).is_some_and(|record| i32::from(record.category) == list) && progress.flag(flag::CLEARED_NOW + number))
    }

    /// The records of a list, in their order (0x14ABAC, 0x14AC24).
    pub fn list(&self, list: i32, progress: &Progress) -> Vec<i32> {
        (0..self.count).filter(|&number| self.listed(number, list, progress)).collect()
    }

    /// The records that are open, each with its count and its goal, for the log.
    pub fn summary(&self, progress: &Progress) -> String {
        let open = (0..self.count).filter(|&number| progress.flag(flag::OPEN + number));
        let told = open.filter_map(|number| {
            let record = self.record(number)?;
            let cleared = if progress.flag(flag::CLEARED + number) { ", cleared" } else { "" };
            Some(format!("{} {}/{}{cleared}", record.name, progress.variable(COUNT + number), record.goal))
        });
        told.collect::<Vec<_>>().join("; ")
    }

    /// A category's list has been shown (0x14ACDC): its records' news and their
    /// having been cleared are forgotten, and every record's having been opened.
    pub fn shown(&self, category: i32, progress: &mut Progress) {
        for number in 0..self.count {
            if self.record(number).is_some_and(|record| i32::from(record.category) == category) {
                progress.set_flag(flag::NEWS + number, false);
                progress.set_flag(flag::CLEARED_NOW + number, false);
            }
        }
        for number in 0..self.count {
            progress.set_flag(flag::OPENED_NOW + number, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Option<Records> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        Some(Records::load(&data).unwrap())
    }

    fn figures() -> PlayerStats {
        PlayerStats { level: 1, experience: 0, max_hp: 300.0, hp: 300.0, attack: 20, special: 20, defence: 20, burst_bullets: 3, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 }
    }

    /// The count of a record and its flags, by what they are from.
    fn state(progress: &Progress, record: i32) -> (i32, Vec<i32>) {
        let flags = [flag::OPEN, flag::CLEARED, flag::NEWS, flag::CLEARED_NOW, flag::COUNTED, flag::OPENED_NEWS, flag::OPENED_NOW];
        (progress.variable(COUNT + record), flags.into_iter().filter(|&from| progress.flag(from + record)).collect())
    }

    #[test]
    fn the_table_has_ninety_six_records_with_what_each_is_worth() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(records.count(), 96);
        assert!(records.record(96).is_none() && records.record(-1).is_none());
        // The first: four battles to be fought, for the flag that lets her dodge.
        let first = records.record(0).unwrap();
        assert_eq!((first.name.as_str(), first.goal, first.span, first.category, first.stage, first.traits), ("First Challenge", 4, 1, 0, 0, 0x0e));
        assert_eq!(first.words[0], "Engage in battle 4 times");
        assert_eq!(first.rewards.len(), 1);
        assert_eq!((first.rewards[0].kind, first.rewards[0].numbers), (6, (7500, 0)));
        assert!(first.rewards[0].words.as_deref().is_some_and(|words| words.starts_with("Obtained basic skill")));
        // Seven Eaters, for a skill; and one worth two things.
        let eaters = records.record(3).unwrap();
        assert_eq!((eaters.name.as_str(), eaters.goal, eaters.rewards[0].kind, eaters.rewards[0].numbers), ("Eater Havoc", 7, 2, (1, -1)));
        assert_eq!(records.record(8).unwrap().rewards.iter().map(|reward| reward.kind).collect::<Vec<_>>(), [7, 5]);
        // Every record has a name and a goal.
        for number in 0..records.count() {
            let record = records.record(number).unwrap();
            assert!(!record.name.is_empty() && record.goal > 0 && record.span > 0, "record {number}: {record:?}");
        }
    }

    #[test]
    fn battles_fought_open_the_first_record_and_clear_it_at_the_fourth() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
        records.initialize(&mut world).unwrap();
        // None of the first stage's is open from the start.
        assert_eq!(state(world.progress, 0), (0, vec![]));
        // The first battle she comes out of, whatever the group, opens it and is
        // its first count: its opening is news, a first count is not, and a record
        // that has been opened is to be shown.
        let fought = Event::battle(100, false, true);
        records.battle_finished(fought, &mut world).unwrap();
        assert_eq!(state(world.progress, 0), (1, vec![flag::OPEN, flag::COUNTED, flag::OPENED_NEWS, flag::OPENED_NOW]));
        assert_eq!(records.news(world.progress), Some((0, News::Opened)));
        assert_eq!(records.list(0, world.progress), [0]);
        records.shown(0, world.progress);
        world.progress.set_flag(flag::OPENED_NEWS, false);
        assert_eq!(state(world.progress, 0), (1, vec![flag::OPEN, flag::COUNTED]));
        assert_eq!(records.news(world.progress), None);
        // The second and third counts are news, and the fourth clears it.
        records.battle_finished(fought, &mut world).unwrap();
        assert_eq!(state(world.progress, 0), (2, vec![flag::OPEN, flag::NEWS, flag::COUNTED]));
        assert_eq!(records.news(world.progress), Some((0, News::Counted)));
        records.battle_finished(fought, &mut world).unwrap();
        assert_eq!(world.progress.variable(COUNT), 3);
        assert!(records.list(0, world.progress).is_empty());
        records.battle_finished(fought, &mut world).unwrap();
        assert_eq!(state(world.progress, 0), (4, vec![flag::OPEN, flag::CLEARED, flag::NEWS, flag::CLEARED_NOW, flag::COUNTED]));
        assert_eq!(records.list(0, world.progress), [0]);
        assert_eq!(records.list(2, world.progress), [0]);
        // Cleared, it counts no more.
        assert!(!records.live(0, world.progress, 0));
        records.battle_finished(fought, &mut world).unwrap();
        assert_eq!(world.progress.variable(COUNT), 4);
    }

    #[test]
    fn a_record_of_a_stage_counts_only_there_and_nothing_is_told_through_the_opening() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 3 };
        // The first record is of the first stage: on a map of another nothing
        // comes of a battle there.
        records.battle_finished(Event::battle(100, false, true), &mut world).unwrap();
        assert_eq!(state(world.progress, 0), (0, vec![]));
        assert!(!records.live(0, world.progress, 3) && records.live(0, world.progress, 0));
        // Nor of anything while the story's opening has its flag set.
        world.stage = 0;
        world.progress.set_flag(NOTHING_TOLD, true);
        records.battle_finished(Event::battle(100, false, true), &mut world).unwrap();
        records.count_up(0, &mut world).unwrap();
        assert_eq!(state(world.progress, 0), (0, vec![]));
        world.progress.set_flag(NOTHING_TOLD, false);
        // A map's script can open a record and count it itself.
        records.open(0, &mut world).unwrap();
        records.count_up(0, &mut world).unwrap();
        assert_eq!(state(world.progress, 0), (1, vec![flag::OPEN, flag::COUNTED, flag::OPENED_NEWS, flag::OPENED_NOW]));
    }

    #[test]
    fn enemies_beaten_count_for_the_records_of_their_kind() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
        // "Eater Havoc" is record 3, and its Eater is enemy 2. The first one beaten
        // opens it and counts, and the seventh clears it.
        let eater = Event::kill(2, false, 0);
        records.enemy_beaten(eater, &mut world).unwrap();
        assert_eq!(state(world.progress, 3), (1, vec![flag::OPEN, flag::COUNTED, flag::OPENED_NEWS, flag::OPENED_NOW]));
        for _ in 0..6 {
            records.enemy_beaten(eater, &mut world).unwrap();
        }
        assert_eq!(world.progress.variable(COUNT + 3), 7);
        assert!(world.progress.flag(flag::CLEARED + 3));
        // Other kinds of enemy are nothing to it: not the Eater of the first
        // mission's own kind either (enemy 8), nor the Hornet of it (19).
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
        for enemy in [8, 19] {
            records.enemy_beaten(Event::kill(enemy, false, 0), &mut world).unwrap();
        }
        assert!((0..records.count()).all(|record| state(world.progress, record) == (0, vec![])));
        // A Hornet of the kind met later (enemy 14) opens "Hornet Havoc".
        records.enemy_beaten(Event::kill(14, false, 0), &mut world).unwrap();
        assert_eq!(world.progress.variable(COUNT + 4), 1);
        assert!(world.progress.flag(flag::OPEN + 4) && !world.progress.flag(flag::OPEN + 3));
    }

    #[test]
    fn the_system_scripts_texts_are_read_and_have_words_put_into_them() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The text a record's own words are said with is nothing but them.
        assert_eq!(records.text(9), Some("\u{1a}00"));
        assert!(records.text(0).is_some_and(|text| text.starts_with("Mission: ") && text.ends_with("has been unlocked.")));
        // Forty-one of them, and where they end.
        assert_eq!((records.text(40), records.text(41), records.text(42), records.text(-1)), (Some("スペシャル"), Some(""), None, None));
        let words = ["one".to_owned(), "two".to_owned()];
        assert_eq!(with_words("\u{1a}00 and \u{1a}01, \u{1a}07.", &words), "one and two, .");
        assert_eq!(with_words(records.text(9).unwrap(), &words), "one");
    }

    #[test]
    fn a_mission_failed_puts_back_the_records_that_are_not_cleared() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        let fought = Event::battle(100, false, true);
        let fight = |progress: &mut Progress, stats: &mut PlayerStats, battles: usize| {
            let mut world = World { progress, stats, stage: 0 };
            for _ in 0..battles {
                records.battle_finished(fought, &mut world).unwrap();
            }
        };
        // Two battles fought as a mission is chosen: that is kept, and what there
        // was to tell of them is forgotten.
        fight(&mut progress, &mut stats, 2);
        records.back_up(&mut progress);
        records.forget_news(&mut progress);
        assert_eq!(state(&progress, 0), (2, vec![flag::OPEN, flag::OPENED_NEWS, flag::OPENED_NOW]));
        // A third in the mission, and an Eater beaten; then the mission is failed.
        // The first record is one that is put back, the Eaters' is not.
        fight(&mut progress, &mut stats, 1);
        records.enemy_beaten(Event::kill(2, false, 0), &mut World { progress: &mut progress, stats: &mut stats, stage: 0 }).unwrap();
        assert_eq!((progress.variable(COUNT), progress.variable(COUNT + 3)), (3, 1));
        assert_eq!((records.record(0).unwrap().traits & trait_of::PUT_BACK, records.record(3).unwrap().traits & trait_of::PUT_BACK), (4, 0));
        records.restore(&mut progress);
        assert_eq!((progress.variable(COUNT), progress.variable(COUNT + 3)), (2, 1));
        assert!(progress.flag(flag::OPEN) && progress.flag(flag::OPEN + 3));
        // One cleared since it was kept stays cleared.
        fight(&mut progress, &mut stats, 2);
        assert!(progress.flag(flag::CLEARED));
        records.restore(&mut progress);
        assert!(progress.flag(flag::CLEARED));
        assert_eq!(progress.variable(COUNT), 4);
        // And one that was not open when it was kept is not open again.
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        records.back_up(&mut progress);
        fight(&mut progress, &mut stats, 3);
        records.restore(&mut progress);
        assert_eq!((progress.variable(COUNT), progress.flag(flag::OPEN)), (0, false));
    }

    #[test]
    fn only_the_highest_level_counts_for_its_record() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
        // "Level MAX" is record 94: a level gained short of the fiftieth is nothing
        // to it, and the fiftieth clears it.
        assert_eq!(records.record(94).unwrap().name, "Level MAX");
        for level in [2, 49] {
            world.stats.level = level;
            records.level_up(&mut world).unwrap();
            assert_eq!(state(world.progress, 94), (0, vec![]), "at level {level}");
        }
        world.stats.level = 50;
        records.level_up(&mut world).unwrap();
        assert_eq!(world.progress.variable(COUNT + 94), 1);
        assert!(world.progress.flag(flag::CLEARED + 94));
    }

    #[test]
    fn a_record_taken_as_cleared_is_worth_its_flags_and_has_nothing_to_show() {
        let Some(records) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut progress, mut stats) = (Progress::new_game(), figures());
        assert!(!progress.flag(7500));
        records.take_as_cleared(FIRST, &mut progress);
        assert_eq!(state(&progress, FIRST), (4, vec![flag::OPEN, flag::CLEARED]));
        assert!(progress.flag(7500));
        assert!(records.list(0, &progress).is_empty() && records.news(&progress).is_none());
        // And a battle is nothing more to it.
        let mut world = World { progress: &mut progress, stats: &mut stats, stage: 0 };
        records.battle_finished(Event::battle(100, false, true), &mut world).unwrap();
        assert_eq!(world.progress.variable(COUNT), 4);
    }
}
