//! Status effects: the table of them (`btl_eff_tbl.bin`) and the record of those in
//! force that the player and every enemy has (the player's at actor + 0x1E8, an
//! enemy's at + 0x284; the code from 0x8E164 to 0x9062C).
//!
//! An effect is of one of 29 kinds. Some act at once (a heal, a cure); most stay for
//! a time, one of a kind at most; a few are only ever a property of an attack. What
//! each kind does is spread over the battle's code, which asks the record: this
//! module keeps the record and does the sums. `docs/BATTLE_NOTES.md` has the list.
//!
//! An effect that stays and has an icon has a mark put up for it where its bearer's
//! gauges are (0x8E334): by her heat, or by the bar over an enemy. Only so many are
//! shown at a time, and when there are more they are shown a page at a time
//! (0x3DEE4 to 0x3E1A0). The record keeps the order the marks stand in and the page
//! that is up; `hud.rs` draws them. The battle's log has a line for an effect as it
//! is given and another as its time runs out (0x8DE7C, 0x8DEF8), which the record
//! keeps for the log to take.

use std::cmp::Ordering;

use anyhow::{ensure, Context, Result};
use brs_formats::bxcb;

use super::damage::{spread, Random};
use super::log::{Line, Names};
use crate::data::GameData;

const TABLE: &str = "SYSTEM.VOL/btl_eff_tbl.bin";

/// The kinds of effect, by the numbers the table has them under.
pub mod kind {
    /// HP back at once.
    pub const HEAL: i32 = 1;
    /// Takes away the ailments its figure names, a bit for each.
    pub const CURE: i32 = 2;
    /// Has the skills of her slots ready at once: those its figure names, a bit
    /// for each (0x900E0).
    pub const CHARGE: i32 = 3;
    /// Her heat is gone, and with it her overheating (0x978A8).
    pub const COOL: i32 = 4;
    /// The record of a guard: how much more than guarding itself it keeps off, and
    /// what guarding a hit with it gives her. Never given to anybody: her own
    /// guard is record 500, and a skill of hers can have it be another (0x9AE9C).
    pub const GUARD_SKILL: i32 = 5;
    /// HP off every so often. Only the player's is ever counted.
    pub const POISON: i32 = 6;
    /// She cannot shoot.
    pub const NO_SHOT: i32 = 7;
    /// She cannot dodge.
    pub const NO_DODGE: i32 = 8;
    /// She can do nothing at all, until it runs out or she is hit.
    pub const STUN: i32 = 9;
    /// She cannot use her skills.
    pub const NO_SKILL: i32 = 10;
    pub const ATTACK_DOWN: i32 = 11;
    pub const DEFENCE_DOWN: i32 = 12;
    /// Shortens the waits an enemy's script takes between its actions.
    pub const PACE_DOWN: i32 = 13;
    pub const ATTACK_UP: i32 = 15;
    pub const DEFENCE_UP: i32 = 16;
    /// Lengthens those waits.
    pub const PACE_UP: i32 = 17;
    /// Three boosts to what its bearer gives: damage, healing, and how long the
    /// effects it hands out last.
    pub const BOOST: i32 = 19;
    /// HP back every so often.
    pub const REGENERATION: i32 = 20;
    /// Every hit counts as guarded and takes off only a share, down to nothing.
    pub const WARD: i32 = 21;
    /// Nothing adds heat.
    pub const NO_HEAT: i32 = 22;
    /// An enemy with either of these cannot be locked onto.
    pub const UNSEEN: i32 = 24;
    pub const CLOUD: i32 = 25;
    /// Of an attack: it goes through a guard.
    pub const GUARD_BREAK: i32 = 26;
    /// Of an attack: so much more damage, in percent, to an enemy whose gauge is broken.
    pub const BREAK_BONUS: i32 = 27;
    /// Of an attack: its giver gets a share of the damage back as HP.
    pub const DRAIN: i32 = 28;
    /// Changes an enemy's own figures for good.
    pub const FIGURES: i32 = 29;

    /// The kinds that stay for a time. The ailments (up to 14) are not given again
    /// to one who has them; the others are, and start afresh (0x90170-0x904E8).
    pub const FIRST_TIMED: i32 = POISON;
    pub const LAST_AILMENT: i32 = 14;
    pub const LAST_TIMED: i32 = CLOUD;
}

// How many of the marks of its effects a bearer shows at a time (0x3D33C): she by
// the gauge of her heat, and an enemy by the bar over it.
pub const HER_ROOM: usize = 8;
pub const ENEMY_ROOM: usize = 3;
/// How long a page of marks is up when there are more than can be shown at a time,
/// in sixtieths of a second (0x3E088).
const PAGE_TIME: i32 = 120;

/// The sound an effect is given with, of the battle's bank (0x8FC80, from the table
/// at 0x169278): only an effect whose record says it is shown has one, and only of
/// the kinds that show something over their bearer. The table has sounds for the
/// ailments too (0xBB, 0xBA, 0x86), a ward (0x70) and the unseen (0x7B), but
/// nothing is shown for those and the sound goes unplayed with it.
pub fn sound(effect: &Effect) -> Option<u32> {
    if !effect.shown {
        return None;
    }
    match effect.kind {
        kind::HEAL..=4 | kind::REGENERATION => Some(0x82),
        kind::ATTACK_UP..=kind::BOOST => Some(0x84),
        kind::NO_HEAT => Some(0x8c),
        _ => None,
    }
}

/// What is seen where its bearer stands as an effect is given, as one of three
/// (0x8DFF0, from the same table): only of an effect that has a sound.
pub fn burst(effect: &Effect) -> Option<usize> {
    if !effect.shown {
        return None;
    }
    match effect.kind {
        kind::HEAL..=4 | kind::REGENERATION => Some(0),
        kind::DEFENCE_UP..=18 | kind::NO_HEAT => Some(1),
        kind::ATTACK_UP | kind::BOOST => Some(2),
        _ => None,
    }
}

/// The effect each of the three is, on her and on an enemy (0x8FD30, 0x8FD8C).
pub const BURSTS_ON_HER: [u32; 3] = [998_042, 998_043, 998_041];
pub const BURSTS_ON_AN_ENEMY: [u32; 3] = [998_035, 998_036, 998_034];

/// The kinds a cure takes away, by the bits of its figure (0x8FF88).
const CURED: [(i32, i32); 8] = [
    (0x01, kind::POISON),
    (0x02, kind::NO_SHOT),
    (0x04, kind::STUN),
    (0x08, kind::NO_SKILL),
    (0x10, kind::ATTACK_DOWN),
    (0x20, kind::DEFENCE_DOWN),
    (0x40, kind::PACE_DOWN),
    (0x80, 14),
];

/// Every effect's clock runs by this much a frame, whatever the frame's own step
/// (0x8F01C).
pub const CLOCK_STEP: f32 = 2.0;

/// A record of the effect table.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effect {
    pub id: i32,
    /// One of [`kind`].
    pub kind: i32,
    /// The chance of it taking, in percent, before the levels are counted in.
    pub chance: i32,
    /// How long it stays, in sixtieths of a second; zero for until it is taken away.
    pub duration: i32,
    /// What it does, by its kind (record fields 0x10 to 0x1C).
    pub values: [i32; 4],
    /// Which icon the gauges show for it; negative for none.
    pub icon: i32,
    /// The two lines of the battle log: as it takes, and as it runs out.
    pub messages: [i32; 2],
    /// It shows an effect and plays a sound as it takes.
    pub shown: bool,
}

impl Effect {
    const FIELDS: usize = 13;

    fn from_record(record: &[u32]) -> Effect {
        let field = |index: usize| record[index] as i32;
        Effect {
            id: field(0),
            kind: field(1),
            chance: field(2),
            duration: field(3),
            values: [field(4), field(5), field(6), field(7)],
            icon: field(9),
            messages: [field(10), field(11)],
            shown: field(12) != 0,
        }
    }

    /// A figure of the effect, kept within the bounds the original reads it by
    /// (0x129B1C and the accessors after it).
    fn value(&self, index: usize, low: i32, high: i32) -> i32 {
        self.values[index].clamp(low, high)
    }

    /// The one figure of the kinds that have just a strength.
    fn strength(&self) -> i32 {
        match self.kind {
            kind::ATTACK_DOWN | kind::DEFENCE_DOWN | kind::GUARD_SKILL => self.value(0, 0, 100),
            kind::PACE_DOWN => self.value(0, 0, 0xffff),
            kind::ATTACK_UP | kind::DEFENCE_UP | kind::PACE_UP | kind::BREAK_BONUS | kind::DRAIN => self.value(0, 0, 999),
            _ => 0,
        }
    }

    /// The share of a hit's damage that an attack with this effect gives back to
    /// whoever dealt it, in percent (0x8FB68).
    pub fn drain(&self) -> i32 {
        if self.kind == kind::DRAIN { self.strength() } else { 0 }
    }

    /// Of the record of a guard (kind 5): the hundredths of a hit that it keeps off
    /// her on top of what guarding does (0x129A98), and the effect she is given
    /// as a hit lands on it, none if zero (0x129B14). The two figures between
    /// are read by nothing.
    pub fn guard(&self) -> Option<(i32, i32)> {
        (self.kind == kind::GUARD_SKILL).then(|| (self.strength(), self.values[3]))
    }

    /// Whether an attack with this effect goes through a guard (0x8FA88).
    pub fn breaks_guard(&self) -> bool {
        self.kind == kind::GUARD_BREAK
    }

    /// How much more an attack with this effect does to an enemy whose gauge is
    /// broken, in percent (0x8FAF8).
    pub fn break_bonus(&self) -> i32 {
        if self.kind == kind::BREAK_BONUS { self.strength() } else { 0 }
    }
}

/// The effect table (`btl_eff_tbl.bin`).
#[derive(Clone, Debug, Default)]
pub struct Effects(Vec<Effect>);

impl Effects {
    pub fn load(data: &GameData) -> Result<Effects> {
        let table = bxcb::Table::parse(&data.read(TABLE)?).with_context(|| format!("parsing {TABLE}"))?;
        ensure!(table.fields == Effect::FIELDS, "{TABLE} has {} fields per record", table.fields);
        Ok(Effects(table.records().map(Effect::from_record).collect()))
    }

    #[cfg(test)]
    pub fn from_records(effects: Vec<Effect>) -> Effects {
        Effects(effects)
    }

    /// The effect with that id (0x1299FC). Attacks name theirs by it, zero for none.
    pub fn get(&self, id: i32) -> Option<&Effect> {
        self.0.iter().find(|effect| effect.id == id)
    }
}

/// An effect in force.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub effect: Effect,
    /// The special attack of whoever gave it, which poison and regeneration can go
    /// by (the node's field 0x1C).
    special: i32,
    /// Its number among all that its bearer has been given (the node's field 0xC).
    serial: u32,
    /// Sixtieths of a second it lasts and has run. One with no duration stays.
    duration: f32,
    elapsed: f32,
    /// Time since it last took its toll, for the kinds that do so every so often.
    since: f32,
}

impl Entry {
    fn lasting(&self) -> bool {
        self.duration <= 0.0
    }

    /// Which of two effects has its mark before the other's (0x8F068): one that
    /// runs out before one that stays, of two that run out the one with less time
    /// left, and of two alike the one given first.
    fn before(&self, other: &Entry) -> Ordering {
        match (self.lasting(), other.lasting()) {
            (false, true) => Ordering::Less,
            (true, false) => Ordering::Greater,
            (true, true) => self.serial.cmp(&other.serial),
            (false, false) => {
                let (left, others) = (self.duration - self.elapsed, other.duration - other.elapsed);
                left.partial_cmp(&others).unwrap_or(Ordering::Equal).then(self.serial.cmp(&other.serial))
            }
        }
    }

    /// What a poison takes or a regeneration gives each time (0x8E538, 0x8EC84): a
    /// share of its giver's special attack and a share of the bearer's most HP.
    fn toll(&self, max_hp: i32) -> i32 {
        let (of_special, of_most) = (self.effect.value(1, 0, 999), self.effect.value(2, 0, 100));
        (of_special * self.special / 100 + of_most * max_hp / 100).max(0)
    }

    /// Whether its toll is due now, `dt` on from the last look (0x8E504, 0x8ED24).
    fn due(&mut self, dt: f32, most: i32) -> bool {
        self.since += dt;
        if self.effect.value(0, 0, most) as f32 >= self.since {
            return false;
        }
        self.since = 0.0;
        true
    }
}

/// The effects in force on one fighter, the newest first as the original chains them.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    /// The name the battle's log has for its bearer (0x8DC1C).
    bearer: String,
    entries: Vec<Entry>,
    /// How many effects it has been given to keep: the next one's number.
    given: u32,
    /// How many of its marks are shown at a time; which page of them is up; and,
    /// while there are more than a page of them, the sixtieths until the next is.
    room: usize,
    page: usize,
    turning: Option<i32>,
    /// The sounds of the effects given since the battle last took them up to play
    /// them.
    pub heard: Vec<u32>,
    /// What is to be seen of them, each as one of the three there are (`burst`).
    pub seen: Vec<usize>,
    /// The lines the battle's log has to say of the effects given and gone since it
    /// last took them.
    pub said: Vec<Line>,
}

impl Default for Status {
    /// A record of nobody in particular, for sums.
    fn default() -> Status {
        Status::of("", HER_ROOM)
    }
}

/// What a fighter's record shows to others, small enough to pass about: the kinds
/// in force, and the effects by id.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Marks {
    kinds: u32,
    ids: [i32; Marks::ROOM],
}

impl Marks {
    /// One of each kind that stays is all there can be.
    const ROOM: usize = (kind::LAST_TIMED - kind::FIRST_TIMED + 1) as usize;

    pub fn has(&self, kind: i32) -> bool {
        (0..32).contains(&kind) && self.kinds & (1 << kind) != 0
    }

    pub fn has_effect(&self, id: i32) -> bool {
        id != 0 && self.ids.contains(&id)
    }
}

/// Whoever gives an effect, as far as the effect goes by them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Giver {
    pub level: i32,
    /// Their special attack (0x8DD94).
    pub special: i32,
    /// The figures of a boost on them, if they have one.
    pub boost: [i32; 3],
    /// The name the battle's log has for them (0x8DC1C).
    pub name: String,
}

/// Whoever is given one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Taker {
    pub level: i32,
    /// How well they resist effects of this kind: an enemy's figure for it (its
    /// variant record, 0x70 to 0x84); the player has none.
    pub resistance: i32,
    pub max_hp: i32,
}

/// What giving an effect came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// The chance went against it.
    Missed,
    /// It is of a kind that does nothing as it is given: a property of an attack,
    /// or something of the player's menus that is not ported.
    Nothing,
    /// So much HP back.
    Healed(i32),
    /// Ailments taken away: how many.
    Cured(usize),
    /// The skills of these slots of hers are to be ready at once: a bit each, 1
    /// square's, 2 triangle's, 4 circle's, 8 cross's (0x900F0 on).
    Charged(u32),
    /// Her heat is to be gone.
    Cooled,
    /// It is in force now.
    Given,
    /// The bearer has an ailment of the kind already, and keeps that one.
    Kept,
    /// An enemy's attack, special attack, defence and guard figure change by these.
    Figures([i32; 4]),
}

impl Status {
    /// The record of somebody the battle's log has that name for, who shows so
    /// many of the marks of their effects at a time.
    pub fn of(bearer: &str, room: usize) -> Status {
        Status { bearer: bearer.to_owned(), entries: Vec::new(), given: 0, room, page: 0, turning: None, heard: Vec::new(), seen: Vec::new(), said: Vec::new() }
    }

    /// The name the battle's log has for its bearer.
    pub fn bearer(&self) -> &str {
        &self.bearer
    }

    /// The effect of that kind in force, if there is one (0x8EFA0).
    pub fn of_kind(&self, kind: i32) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.effect.kind == kind)
    }

    pub fn has(&self, kind: i32) -> bool {
        self.of_kind(kind).is_some()
    }

    /// The effects in force, newest first.
    #[cfg(test)]
    pub fn effects(&self) -> impl Iterator<Item = &Effect> {
        self.entries.iter().map(|entry| &entry.effect)
    }

    /// The effects in force that have a mark: those whose record has an icon
    /// (0x8E334).
    fn marked(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.effect.icon >= 0)
    }

    /// The marks that are shown now (0x3D2D0), each by its icon, in the order they
    /// stand in: those of the page that is up. They are put in order every frame
    /// (0x8F128); only for the frame an effect is given in can the original show
    /// them otherwise, with the new mark not yet among them.
    pub fn icons(&self) -> Vec<usize> {
        let mut marked: Vec<&Entry> = self.marked().collect();
        marked.sort_by(|one, other| one.before(other));
        marked.iter().skip(self.page * self.room).take(self.room).map(|entry| entry.effect.icon as usize).collect()
    }

    /// A mark has been put up or taken down (0x3DEE4, 0x3DFB0): with more of them
    /// than are shown at a time their pages begin to turn, and once there are no
    /// more than that again the first page is up and stays.
    fn remark(&mut self) {
        match (self.marked().count() > self.room, self.turning) {
            (true, None) => self.turning = Some(PAGE_TIME),
            (false, Some(_)) => (self.turning, self.page) = (None, 0),
            _ => {}
        }
    }

    /// A frame of the display its marks are on (0x3CFD4): the wait for the next
    /// page runs down by the frame's sixtieths, and the frame after it has run out
    /// the next page is up (0x3E0E4), or the first again after the last. A page
    /// that marks taken down have left with nothing on it stays up for its time
    /// all the same.
    pub fn turn(&mut self, sixtieths: i32) {
        let Some(wait) = self.turning else { return };
        if wait > 0 {
            self.turning = Some((wait - sixtieths).max(0));
            return;
        }
        self.page += 1;
        if self.marked().count() <= self.page * self.room {
            self.page = 0;
        }
        self.turning = Some(PAGE_TIME);
    }

    /// Has the battle's log say one of an effect's two lines of its bearer, as the
    /// doing of whoever has that name (0x8DE7C, 0x8DEF8). The table of the log's
    /// lines has none numbered nothing, which is what an effect without a line has.
    fn say(&mut self, message: i32, by: &str) {
        if message != 0 {
            let names = Names { actor: by.to_owned(), user: by.to_owned(), target: self.bearer.clone() };
            self.said.push(Line { message, names });
        }
    }

    /// The log says an effect is over (0x8DEF8): all the line's names are its
    /// bearer's.
    fn say_over(&mut self, effect: &Effect) {
        let bearer = self.bearer.clone();
        self.say(effect.messages[1], &bearer);
    }

    pub fn marks(&self) -> Marks {
        let mut marks = Marks::default();
        for (slot, entry) in self.entries.iter().enumerate() {
            if (0..32).contains(&entry.effect.kind) {
                marks.kinds |= 1 << entry.effect.kind;
            }
            if let Some(id) = marks.ids.get_mut(slot) {
                *id = entry.effect.id;
            }
        }
        marks
    }

    fn strength(&self, kind: i32) -> i32 {
        self.of_kind(kind).map_or(0, |entry| entry.effect.strength())
    }

    /// What its bearer's attacks gain or lose, in percent (0x8F474).
    pub fn attack_percent(&self) -> i32 {
        self.strength(kind::ATTACK_UP) - self.strength(kind::ATTACK_DOWN)
    }

    /// What hits on its bearer lose or gain, in percent (0x8F4E4).
    pub fn defence_percent(&self) -> i32 {
        self.strength(kind::DEFENCE_UP) - self.strength(kind::DEFENCE_DOWN)
    }

    /// The length, in percent, that an enemy's waits come to (0x8F69C).
    pub fn pace_percent(&self) -> i32 {
        self.strength(kind::PACE_UP) - self.strength(kind::PACE_DOWN) + 100
    }

    /// The figures of a boost on the bearer: for the damage it deals, the healing it
    /// gives and how long what it hands out lasts, each in percent on top
    /// (0x8F554, 0x8F584, 0x8F5B4).
    pub fn boost(&self) -> [i32; 3] {
        self.of_kind(kind::BOOST).map_or([0; 3], |entry| [0, 1, 2].map(|index| entry.effect.value(index, 0, 999)))
    }

    /// The share of a hit's damage a ward lets through, if there is one (0x8FA00).
    pub fn ward(&self) -> Option<f32> {
        self.of_kind(kind::WARD).map(|entry| (100 - entry.effect.value(0, 0, 100)) as f32 / 100.0)
    }

    /// What a ward on the bearer does to the chance of an effect taking (0x8FA44).
    fn ward_chance(&self) -> f32 {
        self.of_kind(kind::WARD).map_or(1.0, |entry| (100 - entry.effect.value(1, 0, 100)) as f32 / 100.0)
    }

    /// Whether an effect takes (0x8F8F8): its chance, the levels counting double for
    /// their side. What comes out below nothing is read as a very large chance, as
    /// the original compares it without its sign: a bearer far above the giver's
    /// level is always caught.
    fn takes(&self, effect: &Effect, giver: &Giver, taker: &Taker, random: &mut Random) -> bool {
        let against = (taker.resistance + taker.level * 2) as f32 * self.ward_chance();
        let chance = ((effect.chance + giver.level * 2) as f32 - against) as i32;
        (random.below(100) as u32) < chance as u32
    }

    /// Gives the bearer an effect (0x8FE28), if its chance lets it take.
    pub fn give(&mut self, effect: &Effect, giver: &Giver, taker: &Taker, random: &mut Random) -> Outcome {
        if !self.takes(effect, giver, taker, random) {
            return Outcome::Missed;
        }
        // The log says so whatever comes of it (0x8FEB4), an ailment its bearer
        // keeps the old one of too: the line's actor and user are the giver.
        self.say(effect.messages[0], &giver.name);
        self.heard.extend(sound(effect));
        self.seen.extend(burst(effect));
        match effect.kind {
            kind::HEAL => {
                // A set amount, a share of the giver's special attack and a share of
                // the bearer's most HP, more by the giver's boost, give or take
                // (0x8E078).
                let flat = effect.value(0, 0, 9999) as f32;
                let of_special = (giver.special * effect.value(1, 0, 999)) as f32 / 100.0;
                let of_most = (taker.max_hp * effect.value(2, 0, 100)) as f32 / 100.0;
                let boost = (giver.boost[1] + 100) as f32 / 100.0;
                let amount = (flat + of_special + of_most) * boost * spread(effect.value(3, -100, 100), random);
                Outcome::Healed(amount as i32)
            }
            kind::CURE => {
                let mask = effect.value(0, 0, 0x100);
                let before = self.entries.len();
                self.entries.retain(|entry| !CURED.iter().any(|(bit, kind)| mask & bit != 0 && entry.effect.kind == *kind));
                self.remark();
                Outcome::Cured(before - self.entries.len())
            }
            kind::FIRST_TIMED..=kind::LAST_TIMED => {
                if self.has(effect.kind) {
                    if effect.kind <= kind::LAST_AILMENT {
                        return Outcome::Kept;
                    }
                    self.entries.retain(|entry| entry.effect.kind != effect.kind);
                    self.remark();
                }
                // It lasts longer by the giver's boost (0x8E1F8).
                let duration = effect.duration * (giver.boost[2] + 100) / 100;
                let entry = Entry { effect: *effect, special: giver.special, serial: self.given, duration: duration as f32, elapsed: 0.0, since: 0.0 };
                self.given += 1;
                self.entries.insert(0, entry);
                self.remark();
                Outcome::Given
            }
            kind::FIGURES => Outcome::Figures([0, 1, 2, 3].map(|index| effect.value(index, -999, 999))),
            kind::CHARGE => Outcome::Charged(effect.value(0, 0, 0xf) as u32),
            kind::COOL => Outcome::Cooled,
            _ => Outcome::Nothing,
        }
    }

    /// Takes the effect with that id away (0x8FC30). The log has nothing to say of
    /// it, as it has not of what a cure takes away.
    pub fn take(&mut self, id: i32) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.effect.id != id);
        self.remark();
        self.entries.len() != before
    }

    /// The bearer has been hit: a stun ends there (0x8F420), and the log says so.
    /// Returns the effect.
    pub fn struck(&mut self) -> Option<Effect> {
        let index = self.entries.iter().position(|entry| entry.effect.kind == kind::STUN)?;
        let effect = self.entries.remove(index).effect;
        self.remark();
        self.say_over(&effect);
        Some(effect)
    }

    /// A boost is spent (0x8F5E4): the skill it was for has been used, and the log
    /// says it is over. Returns the effect.
    pub fn spend_boost(&mut self) -> Option<Effect> {
        let index = self.entries.iter().position(|entry| entry.effect.kind == kind::BOOST)?;
        let effect = self.entries.remove(index).effect;
        self.remark();
        self.say_over(&effect);
        Some(effect)
    }

    /// Runs every effect's clock on by a frame (0x8F01C); the ones whose time is up
    /// go (0x8E3D0), the log says so of each, and they are handed back.
    pub fn tick(&mut self) -> Vec<Effect> {
        let mut over = Vec::new();
        self.entries.retain_mut(|entry| {
            if entry.lasting() {
                return true;
            }
            entry.elapsed += CLOCK_STEP;
            if entry.duration > entry.elapsed {
                return true;
            }
            over.push(entry.effect);
            false
        });
        // The chain is gone through from its newest.
        for effect in &over {
            self.say_over(effect);
        }
        self.remark();
        over
    }

    /// The HP a poison takes off its bearer now, `dt` on from the last look, if it
    /// is due (0x99F64).
    pub fn poison(&mut self, dt: f32, max_hp: i32) -> Option<i32> {
        let entry = self.entries.iter_mut().find(|entry| entry.effect.kind == kind::POISON)?;
        entry.due(dt, 600).then(|| entry.toll(max_hp))
    }

    /// The HP a regeneration gives its bearer now, if it is due (0x8F634).
    pub fn regeneration(&mut self, dt: f32, max_hp: i32) -> Option<i32> {
        let entry = self.entries.iter_mut().find(|entry| entry.effect.kind == kind::REGENERATION)?;
        entry.due(dt, 3600).then(|| entry.toll(max_hp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records of the table as it has them. The icon and the two lines of the log
    /// of an effect that stays go by its kind there: kinds 14 and 18 share the icon
    /// of the kind before them, and the unseen have no lines.
    fn effect(id: i32) -> Effect {
        let record = |kind: i32, chance, duration, values: [i32; 4]| {
            let (icon, messages) = match kind {
                kind::CURE => (-1, [3101, 0]),
                kind::FIRST_TIMED..=kind::LAST_TIMED => {
                    let nth = kind - kind::FIRST_TIMED;
                    let shared = [14, 18].iter().filter(|&&twin| kind >= twin).count() as i32;
                    (nth - shared, if kind == kind::UNSEEN { [0, 0] } else { [3500 + nth * 100, 3501 + nth * 100] })
                }
                _ => (-1, [0, 0]),
            };
            Effect { id, kind, chance, duration, values, icon, messages, shown: true }
        };
        match id {
            100 => record(kind::HEAL, 100, 0, [50, 0, 0, 0]),
            101 => record(kind::HEAL, 100, 0, [0, 0, 10, 0]),
            200 => record(kind::CURE, 100, 0, [255, 0, 0, 0]),
            205 => record(kind::CURE, 100, 0, [176, 0, 0, 0]),
            600 => record(kind::POISON, 50, 900, [180, 0, 2, 0]),
            602 => record(kind::POISON, 100, 900, [180, 0, 3, 0]),
            800 => record(kind::NO_DODGE, 100, 450, [0; 4]),
            900 => record(kind::STUN, 100, 300, [0; 4]),
            1101 => record(kind::ATTACK_DOWN, 100, 900, [20, 0, 0, 0]),
            1200 => record(kind::DEFENCE_DOWN, 100, 900, [25, 0, 0, 0]),
            1300 => record(kind::PACE_DOWN, 100, 900, [25, 0, 0, 0]),
            1500 => record(kind::ATTACK_UP, 100, 900, [20, 0, 0, 0]),
            1505 => record(kind::ATTACK_UP, 100, 900, [75, 0, 0, 0]),
            1600 => record(kind::DEFENCE_UP, 100, 900, [20, 0, 0, 0]),
            1700 => record(kind::PACE_UP, 100, 1800, [50, 0, 0, 0]),
            1900 => record(kind::BOOST, 100, 0, [50, 50, 50, 0]),
            2005 => record(kind::REGENERATION, 100, 0, [900, 0, 1, 0]),
            2100 => record(kind::WARD, 100, 300, [200, 255, 255, 0]),
            2400 => record(kind::UNSEEN, 100, 0, [0; 4]),
            2500 => record(kind::CLOUD, 100, 900, [0; 4]),
            2800 => record(kind::DRAIN, 100, 0, [100, 0, 0, 0]),
            2900 => record(kind::FIGURES, 100, 0, [30, -2000, 0, 5]),
            _ => panic!("no such effect in the tests' table"),
        }
    }

    const NOBODY: Giver = Giver { level: 1, special: 0, boost: [0; 3], name: String::new() };
    const ANYBODY: Taker = Taker { level: 1, resistance: 0, max_hp: 372 };

    fn given(status: &mut Status, id: i32) -> Outcome {
        status.give(&effect(id), &NOBODY, &ANYBODY, &mut Random::new(3))
    }

    #[test]
    fn what_is_shown_over_its_bearer_is_given_with_a_sound() {
        let mut status = Status::default();
        // A heal, a cure and a regeneration; the boons; nothing for an ailment, a
        // ward or a property of an attack.
        for id in [100, 200, 2005, 1500, 1600, 1700, 1900, 602, 900, 1101, 2100, 2800] {
            given(&mut status, id);
        }
        assert_eq!(status.heard, [0x82, 0x82, 0x82, 0x84, 0x84, 0x84, 0x84]);
        // One whose record does not have it shown has none.
        let mut status = Status::default();
        let quiet = Effect { shown: false, ..effect(100) };
        assert_eq!(status.give(&quiet, &NOBODY, &ANYBODY, &mut Random::new(3)), Outcome::Healed(50));
        assert!(status.heard.is_empty());
        // Nor one that does not take: no chance at all, the levels being even.
        let never = Effect { chance: 0, ..effect(100) };
        assert_eq!(status.give(&never, &NOBODY, &ANYBODY, &mut Random::new(3)), Outcome::Missed);
        assert!(status.heard.is_empty());
        assert_eq!(sound(&Effect { kind: kind::NO_HEAT, shown: true, ..Effect::default() }), Some(0x8c));
        // What has a sound has something seen with it, one of three things.
        let shown = |kind: i32| burst(&Effect { kind, shown: true, ..Effect::default() });
        assert_eq!([kind::HEAL, kind::REGENERATION, kind::DEFENCE_UP, kind::NO_HEAT, kind::ATTACK_UP, kind::BOOST].map(shown), [Some(0), Some(0), Some(1), Some(1), Some(2), Some(2)]);
        assert_eq!([kind::POISON, kind::STUN, kind::WARD].map(shown), [None; 3]);
        for kind in 0..30 {
            let effect = Effect { kind, shown: true, ..Effect::default() };
            assert_eq!(sound(&effect).is_some(), burst(&effect).is_some(), "kind {kind}");
        }
        assert_eq!(burst(&Effect { kind: kind::HEAL, shown: false, ..Effect::default() }), None);
    }

    #[test]
    fn an_ailment_is_kept_and_a_boon_starts_afresh() {
        let mut status = Status::default();
        assert_eq!(given(&mut status, 602), Outcome::Given);
        for _ in 0..100 {
            assert!(status.tick().is_empty());
        }
        // Poisoned again she keeps the poison she has, with the time it has left.
        assert_eq!(given(&mut status, 602), Outcome::Kept);
        let left = (0..).take_while(|_| status.tick().is_empty()).count();
        assert_eq!(left, 349);
        assert!(!status.has(kind::POISON));

        // A second helping of a boon takes the place of the first.
        assert_eq!(given(&mut status, 1500), Outcome::Given);
        assert_eq!(status.attack_percent(), 20);
        for _ in 0..100 {
            status.tick();
        }
        assert_eq!(given(&mut status, 1505), Outcome::Given);
        assert_eq!(status.attack_percent(), 75);
        assert_eq!((0..).take_while(|_| status.tick().is_empty()).count(), 449);
    }

    #[test]
    fn the_chance_counts_the_levels_and_goes_wrong_below_nothing() {
        let mut random = Random::new(5);
        let count = |giver: i32, taker: i32, random: &mut Random| {
            let (giver, taker) = (Giver { level: giver, ..NOBODY }, Taker { level: taker, ..ANYBODY });
            (0..400).filter(|_| Status::default().give(&effect(600), &giver, &taker, random) == Outcome::Given).count()
        };
        // Level with each other: the table's 50 in a hundred.
        let even = count(20, 20, &mut random);
        assert!((160..240).contains(&even), "{even} of 400");
        // Ten levels below the giver: 70; ten above: 30.
        let below = count(20, 10, &mut random);
        assert!((250..310).contains(&below), "{below} of 400");
        let above = count(20, 30, &mut random);
        assert!((90..150).contains(&above), "{above} of 400");
        // Twenty-five above the chance is exactly none.
        assert_eq!(count(20, 45, &mut random), 0);
        // Further above still it comes out below nothing, which is read as certain.
        assert_eq!(count(20, 50, &mut random), 400);

        // What the bearer resists comes off the chance.
        let stubborn = Taker { level: 20, resistance: 100, max_hp: 100 };
        let giver = Giver { level: 30, ..NOBODY };
        let caught = (0..400).filter(|_| Status::default().give(&effect(602), &giver, &stubborn, &mut random) == Outcome::Given).count();
        assert!((50..110).contains(&caught), "{caught} of 400");
    }

    #[test]
    fn poison_takes_its_share_every_three_seconds() {
        let mut status = Status::default();
        given(&mut status, 600);
        let mut taken = Vec::new();
        for frame in 1..=460 {
            status.tick();
            if let Some(amount) = status.poison(2.0, 372) {
                taken.push((frame, amount));
            }
        }
        // Two in a hundred of 372 is 7, once the 180 sixtieths are past: every 91
        // frames, and four times before its 900 are up.
        assert_eq!(taken, vec![(91, 7), (182, 7), (273, 7), (364, 7)]);
        assert!(!status.has(kind::POISON));
    }

    #[test]
    fn regeneration_gives_a_share_back_and_can_last_for_good() {
        let mut status = Status::default();
        given(&mut status, 2005);
        let mut back = 0;
        for _ in 0..2000 {
            assert!(status.tick().is_empty());
            back += status.regeneration(2.0, 6000).unwrap_or(0);
        }
        // One in a hundred of 6000 every 451 frames.
        assert_eq!(back, 240);
        assert!(status.take(2005));
        assert!(!status.has(kind::REGENERATION));
        assert!(!status.take(2005));
    }

    #[test]
    fn a_heal_is_a_set_amount_or_a_share_and_a_boost_adds_to_it() {
        let mut status = Status::default();
        assert_eq!(given(&mut status, 100), Outcome::Healed(50));
        assert_eq!(given(&mut status, 101), Outcome::Healed(37));
        let boosted = Giver { boost: [50, 50, 50], ..NOBODY };
        assert_eq!(status.give(&effect(100), &boosted, &ANYBODY, &mut Random::new(1)), Outcome::Healed(75));
        // The same boost makes what its bearer hands out last half as long again.
        status.give(&effect(800), &boosted, &ANYBODY, &mut Random::new(1));
        assert_eq!((0..).take_while(|_| status.tick().is_empty()).count(), 337);
    }

    #[test]
    fn a_cure_takes_away_the_ailments_its_bits_name() {
        let mut status = Status::default();
        for id in [602, 900, 1101, 1200, 800, 1500] {
            given(&mut status, id);
        }
        // Attack down, defence down and one more: not the poison, nor the stun.
        assert_eq!(given(&mut status, 205), Outcome::Cured(2));
        assert!(status.has(kind::POISON) && status.has(kind::STUN) && !status.has(kind::ATTACK_DOWN));
        // All of them: but being unable to dodge has no bit, and a boon is no ailment.
        assert_eq!(given(&mut status, 200), Outcome::Cured(2));
        let left: Vec<i32> = status.effects().map(|effect| effect.id).collect();
        assert_eq!(left, vec![1500, 800]);
    }

    #[test]
    fn ups_and_downs_are_set_against_each_other() {
        let mut status = Status::default();
        for id in [1101, 1500, 1200, 1300, 1700] {
            given(&mut status, id);
        }
        assert_eq!(status.attack_percent(), 0);
        assert_eq!(status.defence_percent(), -25);
        assert_eq!(status.pace_percent(), 125);
        given(&mut status, 1600);
        assert_eq!(status.defence_percent(), -5);
        assert_eq!(Status::default().pace_percent(), 100);
    }

    #[test]
    fn a_stun_ends_with_a_hit_and_a_ward_lets_nothing_through() {
        let mut status = Status::default();
        given(&mut status, 900);
        given(&mut status, 602);
        assert_eq!(status.struck().map(|effect| effect.id), Some(900));
        assert_eq!(status.struck(), None);
        assert!(status.has(kind::POISON));

        assert_eq!(status.ward(), None);
        given(&mut status, 2100);
        assert_eq!(status.ward(), Some(0.0));
        // And with its second figure as it is, nothing gets past it either: the
        // bearer's own level no longer counts against the chance.
        let (giver, far_above) = (Giver { level: 1, ..NOBODY }, Taker { level: 40, ..ANYBODY });
        assert_eq!(status.give(&effect(800), &giver, &far_above, &mut Random::new(2)), Outcome::Given);
    }

    #[test]
    fn what_a_record_shows_to_others() {
        let mut status = Status::default();
        given(&mut status, 2500);
        given(&mut status, 602);
        let marks = status.marks();
        assert!(marks.has(kind::CLOUD) && marks.has(kind::POISON) && !marks.has(kind::STUN));
        assert!(marks.has_effect(602) && marks.has_effect(2500) && !marks.has_effect(600) && !marks.has_effect(0));
        assert_eq!(Status::default().marks(), Marks::default());
    }

    #[test]
    fn the_marks_stand_by_the_time_their_effects_have_left() {
        let mut status = Status::default();
        // A boost, which stays; a poison of 900 sixtieths; a stun of 300; the
        // unseen, who stay so; and a ward of 300.
        for id in [1900, 602, 900, 2400, 2100] {
            given(&mut status, id);
        }
        // First those that run out, the one with least time left before the others
        // and of two alike the one given first; then those that stay, as they were
        // given.
        assert_eq!(status.icons(), [3, 13, 0, 11, 16]);
        // One given later with more time left than the poison has by then.
        for _ in 0..50 {
            status.tick();
        }
        given(&mut status, 1500);
        assert_eq!(status.icons(), [3, 13, 0, 8, 11, 16]);
        // Its mark goes with an effect, however that goes.
        status.struck();
        assert_eq!(status.icons(), [13, 0, 8, 11, 16]);
        for _ in 0..100 {
            status.tick();
        }
        assert_eq!(status.icons(), [0, 8, 11, 16]);
        status.take(1900);
        assert_eq!(status.icons(), [0, 8, 16]);
        // An effect whose record has no icon has no mark.
        let plain = Effect { icon: -1, ..effect(1600) };
        assert_eq!(status.give(&plain, &NOBODY, &ANYBODY, &mut Random::new(3)), Outcome::Given);
        assert_eq!(status.icons(), [0, 8, 16]);
        assert!(Status::default().icons().is_empty());
    }

    #[test]
    fn more_marks_than_can_be_shown_take_turns_a_page_at_a_time() {
        // By an enemy's bar there is room for three.
        let mut status = Status::of("Eater", ENEMY_ROOM);
        for id in [602, 1101, 1200] {
            given(&mut status, id);
        }
        for _ in 0..200 {
            status.turn(2);
        }
        assert_eq!(status.icons(), [0, 5, 6]);
        // With more, the first three are up for 120 sixtieths, and the frame after
        // those have run out the rest are; then the first again.
        for id in [1300, 1500, 1600, 2400] {
            given(&mut status, id);
        }
        for _ in 0..60 {
            status.turn(2);
            assert_eq!(status.icons(), [0, 5, 6]);
        }
        status.turn(2);
        assert_eq!(status.icons(), [7, 8, 9]);
        for _ in 0..61 {
            status.turn(2);
        }
        assert_eq!(status.icons(), [16]);
        // A page that is left with nothing on it stays up for its time all the same.
        status.take(2400);
        for _ in 0..60 {
            assert!(status.icons().is_empty());
            status.turn(2);
        }
        status.turn(2);
        assert_eq!(status.icons(), [0, 5, 6]);
        for _ in 0..61 {
            status.turn(2);
        }
        assert_eq!(status.icons(), [7, 8, 9]);
        // Once no more are left than can be shown, the first page is up and stays.
        for id in [1300, 1500, 1600] {
            status.take(id);
        }
        assert_eq!(status.icons(), [0, 5, 6]);
        for _ in 0..200 {
            status.turn(2);
        }
        assert_eq!(status.icons(), [0, 5, 6]);
        // A boon given anew takes the place of the old one: with that the pages
        // start from the first again.
        let mut status = Status::of("Eater", ENEMY_ROOM);
        for id in [602, 1101, 1200, 1500] {
            given(&mut status, id);
        }
        for _ in 0..61 {
            status.turn(2);
        }
        assert_eq!(status.icons(), [8]);
        given(&mut status, 1505);
        assert_eq!(status.icons(), [0, 5, 6]);
        // By her gauge there is room for eight.
        let mut hers = Status::of("BRS", HER_ROOM);
        for id in [602, 800, 900, 1101, 1200, 1300, 1500, 1600] {
            given(&mut hers, id);
        }
        for _ in 0..200 {
            hers.turn(2);
        }
        assert_eq!(hers.icons().len(), 8);
        given(&mut hers, 1700);
        for _ in 0..61 {
            hers.turn(2);
        }
        assert_eq!(hers.icons(), [10]);
    }

    #[test]
    fn the_log_is_told_of_an_effect_given_and_of_its_time_running_out() {
        let mut status = Status::of("Mii", ENEMY_ROOM);
        let her = Giver { name: "BRS".into(), ..NOBODY };
        let line = |message: i32, by: &str| Line { message, names: Names { actor: by.into(), user: by.into(), target: "Mii".into() } };
        // As it is given: the giver is the line's actor and its user, the bearer
        // its target.
        assert_eq!(status.give(&effect(900), &her, &ANYBODY, &mut Random::new(3)), Outcome::Given);
        assert_eq!(status.said, [line(3800, "BRS")]);
        // An ailment the bearer has already is kept, and the log says it is given
        // all the same.
        assert_eq!(status.give(&effect(900), &her, &ANYBODY, &mut Random::new(3)), Outcome::Kept);
        assert_eq!(status.said, [line(3800, "BRS"), line(3800, "BRS")]);
        // One that does not take is not spoken of, and a heal has no line.
        let never = Effect { chance: 0, ..effect(602) };
        assert_eq!(status.give(&never, &her, &ANYBODY, &mut Random::new(3)), Outcome::Missed);
        given(&mut status, 100);
        assert_eq!(status.said.len(), 2);
        // A cure has a line for itself, and none for what it takes away.
        status.said.clear();
        assert_eq!(status.give(&effect(200), &her, &ANYBODY, &mut Random::new(3)), Outcome::Cured(1));
        assert_eq!(status.said, [line(3101, "BRS")]);
        // As its time runs out: all the line's names are the bearer's.
        given(&mut status, 2100);
        status.said.clear();
        for _ in 0..149 {
            assert!(status.tick().is_empty());
        }
        assert!(status.said.is_empty());
        assert_eq!(status.tick().len(), 1);
        assert_eq!(status.said, [line(5001, "Mii")]);
        // So too of a stun that a hit ends.
        given(&mut status, 900);
        status.said.clear();
        status.struck();
        assert_eq!(status.said, [line(3801, "Mii")]);
        // Of one that is taken away nothing is said.
        given(&mut status, 2500);
        status.said.clear();
        assert!(status.take(2500));
        assert!(status.said.is_empty());
    }

    #[test]
    fn the_properties_of_an_attack_do_nothing_as_they_are_given() {
        let mut status = Status::default();
        assert_eq!(given(&mut status, 2800), Outcome::Nothing);
        assert_eq!(effect(2800).drain(), 100);
        assert_eq!(effect(602).drain(), 0);
        assert!(!effect(2800).breaks_guard());
        // An enemy's figures change by what the record says, each kept within 999.
        assert_eq!(given(&mut status, 2900), Outcome::Figures([30, -999, 0, 5]));
        assert_eq!(status, Status::default());
    }
}
