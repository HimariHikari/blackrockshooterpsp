//! The data tables behind a battle, read the way the original's accessors read them:
//! attacks, the player's figures, the enemy groups, and what each enemy's archive
//! holds. `docs/BATTLE_NOTES.md` has the layouts and where each was found.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use bevy::prelude::{Resource, Vec3};
use brs_engine::animation::Clip;
use brs_engine::camera::CameraClip;
use brs_formats::bscr::Script;
use brs_formats::edxd::Figures;
use brs_formats::egtb::{self, Groups};
use brs_formats::{bxcb, insm, xtc};

use crate::data::GameData;
use crate::items::{kind, Class, Inventory, Item, Items};
use crate::progress::Progress;

const ATTACKS: &str = "SYSTEM.VOL/btl_atk_tbl.bin";
const BULLETS: &str = "SYSTEM.VOL/btl_bullet_tbl.bin";
const GROUPS: &str = "SYSTEM.VOL/enmgrp2.dat";
const DATABASE: &str = "SYSTEM.VOL/db_resident.lpk";
const ENEMIES: &str = "BTL/CHR";

/// A record of the attack table: what a hit with this attack does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Attack {
    pub id: i32,
    /// Percent of the attacker's attack and of its special attack that make the power.
    pub attack_percent: i32,
    pub special_percent: i32,
    /// Percent of the target's defence taken off the power.
    pub defence_percent: i32,
    /// Percent of what is left that is dealt.
    pub dealt_percent: i32,
    /// The status effect it gives whoever it hits, by its id in the effect table;
    /// zero for none (0x129130).
    pub effect: i32,
    /// The damage varies by this many percent either way.
    pub spread: i32,
    /// Which enemy and which of its variants the record is for; 0 and -1 for any.
    pub enemy: i32,
    pub variant: i32,
    /// Which of her hit reactions the player plays.
    pub reaction: i32,
    /// She reels if this is at least her poise.
    pub strength: i32,
    /// How long the hit freezes her, in logic frames.
    pub hit_stop: i32,
    pub unguardable: bool,
    /// How much it wears down an enemy's lock-point gauge.
    pub break_power: i32,
    /// Which of the looks a hit of it has: the effect that is seen where it lands
    /// (0x129140).
    pub look: i32,
    /// Which shake of the camera a hit of it gives (0x129160).
    pub shake: i32,
    /// The sound of its landing, none if below zero, and whose bank the sound is
    /// in: 0 the battle's, 1 the attacker's own (0x129150, 0x129148).
    pub sound: i32,
    pub sound_bank: i32,
}

impl Attack {
    // Fields of a record, in 32-bit words.
    const FIELDS: usize = 22;

    fn from_record(record: &[u32]) -> Attack {
        let field = |index: usize| record[index] as i32;
        Attack {
            id: field(0),
            attack_percent: field(1),
            special_percent: field(2),
            defence_percent: field(3),
            dealt_percent: field(4),
            effect: field(8),
            spread: field(9),
            look: field(12),
            shake: field(15),
            enemy: field(10),
            variant: field(11),
            reaction: field(16),
            strength: field(17),
            hit_stop: field(18),
            unguardable: field(19) != 0,
            break_power: field(21),
            sound: field(13),
            sound_bank: field(20),
        }
    }
}

/// The attack table (`btl_atk_tbl.bin`).
#[derive(Clone, Debug, Default)]
pub struct Attacks(Vec<Attack>);

impl Attacks {
    pub fn load(data: &GameData) -> Result<Attacks> {
        let table = bxcb::Table::parse(&data.read(ATTACKS)?).with_context(|| format!("parsing {ATTACKS}"))?;
        ensure!(table.fields == Attack::FIELDS, "{ATTACKS} has {} fields per record", table.fields);
        Ok(Attacks(table.records().map(Attack::from_record).collect()))
    }

    #[cfg(test)]
    pub fn from_records(attacks: Vec<Attack>) -> Attacks {
        Attacks(attacks)
    }

    fn exact(&self, id: i32, enemy: i32, variant: i32) -> Option<&Attack> {
        self.0.iter().find(|attack| attack.id == id && attack.enemy == enemy && attack.variant == variant)
    }

    /// The record for attack `id` made by variant `variant` of enemy `enemy`: the one
    /// for exactly that variant, else for any variant of the enemy, else for anyone
    /// (0x129068).
    pub fn of_enemy(&self, id: i32, enemy: i32, variant: i32) -> Option<&Attack> {
        self.exact(id, enemy, variant).or_else(|| self.exact(id, enemy, -1)).or_else(|| self.exact(id, 0, -1))
    }

    /// The record for one of the player's attacks (0x129014).
    pub fn of_player(&self, id: i32) -> Option<&Attack> {
        self.exact(id, 0, -1)
    }
}

/// A record of the bullet table: one kind of shot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shot {
    pub id: i32,
    /// Which enemy and which of its variants the record is for; 0 and -1 for any.
    pub enemy: i32,
    pub variant: i32,
    /// The node of the shooter's model it leaves from, by id (-1 for the model
    /// itself), where it starts from there, and which way it is turned, in degrees.
    pub node: i32,
    pub offset: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// It starts at that node; otherwise the offset is from the middle of the stage.
    pub from_node: bool,
    /// The effect at the muzzle, the sound, and whose bank the sound is in: 0 the
    /// battle's, 1 the shooter's own.
    pub muzzle_effect: i32,
    pub sound: i32,
    pub sound_bank: i32,
    /// Which attack a hit is.
    pub attack: i32,
    /// How many of them make a volley.
    pub count: i32,
    /// Which of the bullet classes it is: 0 a ray, 1 flying straight at a point, 2
    /// guided, 3 a beam, 4 a bomb.
    pub kind: i32,
    /// The effect that is its look, and how much that is scaled.
    pub effect: i32,
    pub scale: f32,
    /// Sixtieths of a second it waits where it starts.
    pub delay: f32,
    /// Distance per sixtieth of a second.
    pub speed: f32,
    /// Sixtieths of a second it flies for; zero for as long as 8000 units take.
    pub life: f32,
    /// A guided bullet leaves turned this much further, in degrees...
    pub launch_yaw: f32,
    pub launch_pitch: f32,
    /// ...and takes this share of the turn towards its target each sixtieth of a second.
    pub homing: f32,
    /// It stops following its target after this long, this far from where it
    /// started, or this near the target.
    pub homing_time: f32,
    pub homing_range: f32,
    pub homing_until: f32,
    /// A beam lasts this long and strikes this often, in sixtieths of a second; so
    /// does the burst of a bomb.
    pub beam_time: f32,
    pub beam_tick: f32,
    /// A bomb's: how high above the ground its first hop takes it, how long it lies
    /// before it bursts, in sixtieths, how far the burst reaches, and the burst's
    /// effect and sound.
    pub peak: f32,
    pub fuse: f32,
    pub blast: f32,
    pub blast_effect: i32,
    pub blast_sound: i32,
    /// A strike's (class 5): the sixtieths before it lands, and how far it reaches.
    pub strike_delay: f32,
    pub strike_reach: f32,
    /// Another bullet fired along with it; zero for none.
    pub next: i32,
    /// It goes on after a hit, and hits each enemy once.
    pub pierces: bool,
    /// The radius of its collision sphere.
    pub radius: f32,
}

impl Shot {
    const FIELDS: usize = 49;

    /// How much its look is scaled: a record that gives no size leaves it as it
    /// is (0x889B0).
    pub fn look_scale(&self) -> f32 {
        if self.scale == 0.0 { 1.0 } else { self.scale }
    }

    /// What a time or range the table leaves open counts as (0x129444, 0x129468).
    const NO_LIMIT: f32 = 10_000.0;

    fn from_record(record: &[u32]) -> Shot {
        let field = |index: usize| record[index] as i32;
        let number = |index: usize| record[index] as i32 as f32;
        let limit = |index: usize| if field(index) < 0 { Shot::NO_LIMIT } else { number(index) };
        Shot {
            id: field(0),
            enemy: field(1),
            variant: field(2),
            node: field(3),
            offset: Vec3::new(number(4), number(5), number(6)),
            yaw: number(7),
            pitch: number(8),
            muzzle_effect: field(9),
            sound: field(10),
            attack: field(11),
            count: field(12),
            sound_bank: field(13),
            kind: field(14),
            effect: field(15),
            delay: number(16),
            speed: number(17),
            launch_pitch: number(18),
            launch_yaw: number(19),
            // A percentage, kept within 0 and 100 (0x129400).
            homing: field(20).clamp(0, 100) as f32 / 100.0,
            homing_time: limit(21),
            homing_range: limit(22),
            homing_until: number(23).max(0.0),
            beam_time: number(30),
            beam_tick: number(31),
            peak: number(32),
            fuse: number(33),
            blast: number(34),
            blast_effect: field(35),
            blast_sound: field(36),
            strike_delay: number(37),
            strike_reach: number(38),
            next: field(39),
            pierces: field(40) != 0,
            radius: number(42),
            scale: number(44) / 100.0,
            from_node: field(46) == 1,
            life: number(48),
        }
    }
}

/// The bullet table (`btl_bullet_tbl.bin`).
#[derive(Clone, Debug, Default)]
pub struct Shots(Vec<Shot>);

impl Shots {
    pub fn load(data: &GameData) -> Result<Shots> {
        let table = bxcb::Table::parse(&data.read(BULLETS)?).with_context(|| format!("parsing {BULLETS}"))?;
        ensure!(table.fields == Shot::FIELDS, "{BULLETS} has {} fields per record", table.fields);
        Ok(Shots(table.records().map(Shot::from_record).collect()))
    }

    #[cfg(test)]
    pub fn from_records(shots: Vec<Shot>) -> Shots {
        Shots(shots)
    }

    fn exact(&self, id: i32, enemy: i32, variant: i32) -> Option<&Shot> {
        self.0.iter().find(|shot| shot.id == id && shot.enemy == enemy && shot.variant == variant)
    }

    /// The record for shot `id` of variant `variant` of enemy `enemy`, with the same
    /// fallbacks as an attack has (0x1292D0).
    pub fn of_enemy(&self, id: i32, enemy: i32, variant: i32) -> Option<&Shot> {
        self.exact(id, enemy, variant).or_else(|| self.exact(id, enemy, -1)).or_else(|| self.exact(id, 0, -1))
    }

    /// The record for a bullet of one of her skills, found by its number alone
    /// (0x12927C).
    pub fn of_player(&self, id: i32) -> Option<&Shot> {
        self.exact(id, 0, -1)
    }
}

/// What the player brings into a battle: the figures the damage sums use, and what
/// her equipment changes.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PlayerStats {
    /// Her level (stat 16), which the chance of a status effect taking goes by.
    pub level: i32,
    /// The experience she has (stat 8).
    pub experience: u32,
    pub max_hp: f32,
    pub hp: f32,
    /// Attack with the weapon's bonus (the original's stat 22).
    pub attack: i32,
    /// Special attack, which skills use (stat 24).
    pub special: i32,
    /// Defence (stats 2 and 6).
    pub defence: i32,
    /// Bullets in a burst while the heat gauge is low: the weapon's.
    pub burst_bullets: i32,
    /// Heat a dodge costs: the suit's.
    pub dodge_heat: i32,
    /// What her skills add to her HP, attack, special attack and defence, which
    /// is in those figures (0x14CF18).
    pub skill_bonus: [i32; 4],
    /// The skills in her four slots (the save's fields 11 to 14), which a battle
    /// has on triangle, circle, cross and square; nought for a slot with none.
    pub skills: [u8; 4],
    /// What she has on (stats 9 and 10): the items her weapon and her suit are.
    pub weapon: i32,
    pub suit: i32,
}

impl PlayerStats {
    // The tables of `db_resident.lpk`, and what the port reads of their records.
    const LEVEL_TABLE: &'static str = "004.xtc";
    const START_TABLE: &'static str = "005.xtc";
    /// The highest level there is (0x14D240).
    pub const MAX_LEVEL: u32 = 50;
    /// What maximum HP and the other figures can grow to (0x14D2EC).
    pub(super) const HP_CAP: f32 = 9999.0;
    pub(super) const FIGURE_CAP: i32 = 999;

    /// The most experience there is to have (0x38644).
    const EXPERIENCE_CAP: u32 = 999_999;
    /// The record of the start table she has her figures from once the opening
    /// is over (`s262(1)`).
    const NEW_GAME: i32 = 1;
    /// Her figures before anything has given her any (0x990 clears them all).
    pub const NONE: PlayerStats = PlayerStats {
        level: 0,
        experience: 0,
        max_hp: 0.0,
        hp: 0.0,
        attack: 0,
        special: 0,
        defence: 0,
        burst_bullets: 0,
        dodge_heat: 0,
        skill_bonus: [0; 4],
        skills: [0; 4],
        weapon: 0,
        suit: 0,
    };

    /// The figures as they are once a new game's player has reached `level`, rested
    /// and with what she started out wearing.
    pub fn at_level(mut self, levels: &Levels, level: u32) -> Result<PlayerStats> {
        ensure!((1..=Self::MAX_LEVEL).contains(&level), "levels run from 1 to {}", Self::MAX_LEVEL);
        while (self.level as u32) < level {
            self.experience = levels.next(self.level).context("a level below the last has a next")?;
            self.level_up(levels)?;
        }
        self.hp = self.max_hp;
        Ok(self)
    }

    /// Goes up a level: the gains of the level she leaves are added to her figures
    /// (0x37E24).
    fn level_up(&mut self, levels: &Levels) -> Result<()> {
        let gains = levels.of(self.level)?;
        self.max_hp = (self.max_hp + gains.hp as f32).min(Self::HP_CAP);
        self.attack = (self.attack + gains.attack).min(Self::FIGURE_CAP);
        self.special = (self.special + gains.special).min(Self::FIGURE_CAP);
        self.defence = (self.defence + gains.defence).min(Self::FIGURE_CAP);
        self.level += 1;
        Ok(())
    }

    /// Takes the experience a battle was worth, as the result screen does (0x383A0,
    /// 0x37E24): she goes up as many levels as it comes to, and going up makes her
    /// whole unless `heals` says otherwise. At the last level nothing more is taken.
    /// Returns how many levels she went up.
    pub fn gain(&mut self, levels: &Levels, experience: u32, heals: bool) -> Result<i32> {
        if self.level as u32 >= Self::MAX_LEVEL {
            return Ok(0);
        }
        let before = self.level;
        let total = self.experience.saturating_add(experience);
        let reached = levels.level_for(total);
        // Reaching the last level leaves her with just what it takes to reach it.
        self.experience = match levels.threshold(reached) {
            Some(threshold) if reached as u32 >= Self::MAX_LEVEL => threshold,
            _ => total.min(Self::EXPERIENCE_CAP),
        };
        while self.level < reached {
            self.level_up(levels)?;
        }
        if self.level > before && heals {
            self.hp = self.max_hp;
        }
        Ok(self.level - before)
    }

    /// The figures she has once the opening is over, which a game begun past it
    /// starts with: the start table's record for them, taken by one who has none.
    pub fn new_game(data: &GameData) -> Result<PlayerStats> {
        let (starts, items) = (Starts::load(data)?, Items::load(data)?);
        let mut stats = Self::NONE;
        stats.start(starts.get(Self::NEW_GAME).context("the start table has no record for a new game")?, &items);
        Ok(stats)
    }

    /// Takes the figures of a record of the start table (0x14C9D4), each that the
    /// record has: her experience and the level it comes to (0x14D240), all her
    /// HP, her attack, defence and special attack, what she has on (0x14D0CC)
    /// and the skills of her slots. The port keeps what her weapon and her
    /// skills add in the figures themselves, so they are added to the record's.
    pub fn start(&mut self, start: &Start, items: &Items) {
        if let Some((experience, level)) = start.experience {
            (self.experience, self.level) = (experience, level);
        }
        if let Some(hp) = start.hp {
            (self.max_hp, self.hp) = ((hp + self.skill_bonus[0]) as f32, hp as f32);
        }
        if let Some(attack) = start.attack {
            let added = items.get(self.weapon).map_or(0, |worn| worn.figures[0]);
            self.attack = (attack + self.skill_bonus[1] + added).clamp(0, Self::FIGURE_CAP);
        }
        if let Some(special) = start.special {
            self.special = special + self.skill_bonus[2];
        }
        if let Some(defence) = start.defence {
            self.defence = defence + self.skill_bonus[3];
        }
        for worn in start.worn() {
            self.equip(items, worn);
        }
        for (slot, skill) in self.skills.iter_mut().zip(start.skills) {
            *slot = skill.unwrap_or(*slot);
        }
    }

    /// Whether a consumable would do anything for her now (0x14D804): one that
    /// gives HP back, while not all her HP is there. (The other kinds the
    /// original asks about, 3 and 5, no item of its tables is of.)
    pub fn helped_by(&self, item: &Item) -> bool {
        item.real && item.class == Class::Consumable && item.kind == kind::REPAIR && self.hp < self.max_hp
    }

    /// Takes what a consumable gives (0x56854): so much HP, then so many
    /// hundredths of all her HP, cut to a whole number, and never more than all
    /// of it. Answers how much HP she has more.
    pub fn take(&mut self, item: &Item) -> i32 {
        if !item.real || item.class != Class::Consumable || item.kind != kind::REPAIR {
            return 0;
        }
        let before = self.hp;
        self.hp = (self.hp + item.gives.0 as f32).clamp(0.0, self.max_hp);
        self.hp = (self.hp + (self.max_hp * (item.gives.1 as f32 / 100.0)).trunc()).clamp(0.0, self.max_hp);
        (self.hp - before) as i32
    }

    /// Changes to a weapon or a suit (0x14D0CC). A weapon that is not the one she
    /// has takes the old one's addition out of her attack and puts in its own;
    /// a suit is only put on. What a battle takes from them goes with them: the
    /// bullets of a burst and the heat a dodge costs.
    pub fn equip(&mut self, items: &Items, number: i32) {
        let Some(item) = items.get(number) else { return };
        match item.kind {
            kind::SUIT => {
                self.suit = number;
                self.dodge_heat = item.figures[1];
            }
            kind::WEAPON if number != self.weapon => {
                let added = items.get(self.weapon).map_or(0, |worn| worn.figures[0]);
                self.attack = (self.attack - added).clamp(0, Self::FIGURE_CAP);
                self.weapon = number;
                self.attack = (self.attack + item.figures[0]).clamp(0, Self::FIGURE_CAP);
                self.burst_bullets = item.figures[2];
            }
            _ => {}
        }
    }
}

/// One record of the start table (0x12B5EC, read by 0x12B470 to 0x12B550): figures
/// to give her, each of them there or, below nought, not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    /// Her experience, and the level it comes to (0x14D240).
    pub experience: Option<(u32, i32)>,
    pub hp: Option<i32>,
    pub attack: Option<i32>,
    pub special: Option<i32>,
    pub defence: Option<i32>,
    pub weapon: Option<i32>,
    pub suit: Option<i32>,
    /// The skills of her slots, as `PlayerStats::skills` has them.
    pub skills: [Option<u8>; 4],
}

impl Start {
    /// Where a record has the skill of each of her slots (0x12B550).
    const SKILLS_AT: [usize; 4] = [16, 19, 18, 17];

    fn read(record: &[u8], levels: &Levels) -> Result<Start> {
        ensure!(record.len() >= 20, "start records are {} bytes", record.len());
        let word = |at: usize| Some(i16::from_le_bytes([record[at], record[at + 1]]) as i32).filter(|&figure| figure >= 0);
        let experience = u32::try_from(i32::from_le_bytes([record[0], record[1], record[2], record[3]])).ok();
        Ok(Start {
            experience: experience.map(|experience| (experience, levels.level_for(experience))),
            hp: word(4),
            attack: word(6),
            special: word(8),
            defence: word(10),
            weapon: word(12),
            suit: word(14),
            skills: Self::SKILLS_AT.map(|at| Some(record[at]).filter(|&skill| skill < 0x80)),
        })
    }

    /// What it has her wear: a weapon, a suit.
    pub fn worn(&self) -> impl Iterator<Item = i32> {
        [self.weapon, self.suit].into_iter().flatten()
    }
}

/// The start table (`005.xtc` of `db_resident.lpk`). Record 0 is who she is while
/// the opening shows her, every figure at its most (the game's own start gives
/// it, 0x908); record 1 who she is when it is over (`s262(1)`).
#[derive(Clone, Debug)]
pub struct Starts(Vec<Start>);

impl Starts {
    /// The record the game's own start gives her (0x910).
    pub const OPENING: i32 = 0;

    pub fn load(data: &GameData) -> Result<Starts> {
        let levels = Levels::load(data)?;
        let path = format!("{DATABASE}/{}", PlayerStats::START_TABLE);
        let table = xtc::Table::parse(&data.read(&path)?).with_context(|| format!("parsing {path}"))?;
        let mut starts = Vec::new();
        while let Some(record) = table.record(starts.len()) {
            starts.push(Start::read(record, &levels)?);
        }
        Ok(Starts(starts))
    }

    pub fn get(&self, record: i32) -> Option<&Start> {
        usize::try_from(record).ok().and_then(|record| self.0.get(record))
    }

    /// Gives her a record (0x14C9D4): its figures; one of what it has her wear,
    /// if she has none of it (0x14D720, 0x14D594); and the skills of its slots
    /// are marked as had and as set (0x14D95C, 0x14D994).
    pub fn give(&self, record: i32, stats: &mut PlayerStats, items: &Items, inventory: &mut Inventory, progress: &mut Progress) {
        let Some(start) = self.get(record) else { return };
        stats.start(start, items);
        for worn in start.worn() {
            if inventory.held(worn) == 0 {
                inventory.add(items, worn, 1);
            }
        }
        for skill in start.skills.into_iter().flatten() {
            progress.set_flag(super::skills::HAS + i32::from(skill), true);
            progress.set_flag(super::skills::SET + i32::from(skill), true);
        }
    }
}

/// One record of the level table (0x12B3D0 to 0x12B450). Record `n` is about going
/// from level `n` to the next: the experience at which that happens, and what it
/// adds to her figures. Record 0 is level 1's own start, at no experience.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub experience: u32,
    pub hp: i32,
    pub attack: i32,
    pub special: i32,
    pub defence: i32,
}

/// The level table (`004.xtc` of `db_resident.lpk`).
#[derive(Resource, Clone, Debug)]
pub struct Levels(Vec<Level>);

impl Levels {
    pub fn load(data: &GameData) -> Result<Levels> {
        let path = format!("{DATABASE}/{}", PlayerStats::LEVEL_TABLE);
        let table = xtc::Table::parse(&data.read(&path)?).with_context(|| format!("parsing {path}"))?;
        let mut levels = Vec::new();
        while let Some(record) = table.record(levels.len()) {
            ensure!(record.len() >= 12, "level records are {} bytes", record.len());
            let gain = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]) as i32;
            levels.push(Level {
                experience: u32::from_le_bytes([record[0], record[1], record[2], record[3]]),
                hp: gain(4),
                attack: gain(6),
                special: gain(8),
                defence: gain(10),
            });
        }
        Levels::from_records(levels)
    }

    fn from_records(levels: Vec<Level>) -> Result<Levels> {
        ensure!(levels.len() >= PlayerStats::MAX_LEVEL as usize, "the level table has only {} records", levels.len());
        Ok(Levels(levels))
    }

    /// A table shaped like the game's: its first records are the game's own, the
    /// last level is reached at 999999, and the records past it can never be.
    #[cfg(test)]
    pub fn for_tests() -> Levels {
        let first = [(0, 0, 0, 0, 0), (16, 62, 1, 3, 1), (119, 63, 3, 3, 2), (374, 62, 2, 5, 2), (843, 62, 3, 5, 2), (1584, 63, 3, 5, 2)];
        let record = |index: usize| match index {
            0..=5 => first[index],
            6..=48 => (index as u32 * 20_000, 15, 1, 1, 1),
            49 => (999_999, 14, 1, 1, 1),
            _ => (u32::MAX, 15, 0, 1, 1),
        };
        let levels = (0..100).map(record).map(|(experience, hp, attack, special, defence)| Level { experience, hp, attack, special, defence });
        Levels::from_records(levels.collect()).unwrap()
    }

    /// The record about going up from `level`.
    fn of(&self, level: i32) -> Result<&Level> {
        let record = usize::try_from(level).ok().and_then(|level| self.0.get(level));
        record.with_context(|| format!("the level table has no record for level {level}"))
    }

    /// The experience at which `level` is reached.
    pub fn threshold(&self, level: i32) -> Option<u32> {
        Some(self.0.get(usize::try_from(level.checked_sub(1)?).ok()?)?.experience)
    }

    /// The experience at which the level after `level` is reached; none past the
    /// last level.
    pub fn next(&self, level: i32) -> Option<u32> {
        ((level as u32) < PlayerStats::MAX_LEVEL).then(|| self.threshold(level + 1))?
    }

    /// The level that goes with having `experience` (0x14D240): the last one whose
    /// threshold it has reached.
    pub fn level_for(&self, experience: u32) -> i32 {
        let last = PlayerStats::MAX_LEVEL as i32;
        (1..last).find(|&level| self.threshold(level + 1).is_none_or(|next| experience < next)).unwrap_or(last)
    }
}

/// The enemies' archives by enemy number: the table at 0x169AB8 of the executable.
const ARCHIVES: [(u16, &str); 88] = [
    (1, "EGG00"),
    (2, "EGG01"),
    (3, "EGG02"),
    (4, "EGG03"),
    (5, "EGG04"),
    (6, "EGG05"),
    (7, "EGG06"),
    (8, "EGG07"),
    (9, "CAT00"),
    (10, "CAT01"),
    (11, "CAT02"),
    (12, "CAT03"),
    (13, "CAT04"),
    (14, "BEE00"),
    (15, "BEE01"),
    (16, "BEE02"),
    (17, "BEE03"),
    (18, "BEE04"),
    (19, "BEE05"),
    (20, "APL00"),
    (21, "APL01"),
    (22, "APL02"),
    (23, "APL03"),
    (26, "HAD00"),
    (27, "HAD01"),
    (28, "HAD02"),
    (29, "HAD03"),
    (32, "TIR00"),
    (33, "TIR01"),
    (34, "TIR02"),
    (35, "TIR03"),
    (38, "TWI00"),
    (39, "TWI01"),
    (40, "TWI02"),
    (41, "TWI03"),
    (44, "NAFC00"),
    (45, "NAFC01"),
    (50, "COF00"),
    (51, "COF01"),
    (53, "COF03"),
    (54, "COF04"),
    (56, "YUB00"),
    (57, "YUB01"),
    (58, "YUB02"),
    (59, "YUB03"),
    (62, "SHI00"),
    (63, "SHI01"),
    (64, "SHI02"),
    (68, "HUL00"),
    (69, "HUL01"),
    (70, "HUL02"),
    (74, "HUA00"),
    (75, "HUA01"),
    (76, "HUA02"),
    (80, "EGB00"),
    (81, "EGB01"),
    (82, "EGB02"),
    (83, "EGB03"),
    (86, "CUB00"),
    (87, "CUB01"),
    (88, "CUB02"),
    (89, "CUB03"),
    (90, "CUB04"),
    (91, "CUB05"),
    (92, "BAT00"),
    (93, "BAT01"),
    (98, "MEE00EX"),
    (99, "MEE00SB"),
    (100, "MAZ00EX"),
    (101, "MAZ00SB"),
    (102, "CYZKAL00EX"),
    (103, "CYZKAL00SB"),
    (104, "RIR00EX"),
    (105, "RIR00SB"),
    (106, "NAF00EX"),
    (107, "NAF00SB"),
    (108, "ZAH01EX"),
    (109, "ZAH01SB"),
    (110, "WRS01SB"),
    (111, "WRS02SB"),
    (116, "MEE00"),
    (117, "MAZ00"),
    (118, "CYZKAL00"),
    (119, "RIR00"),
    (120, "NAF00"),
    (121, "ZAH01"),
    (122, "WRS01"),
    (123, "WRS02"),
];

/// An item an enemy may leave behind, and its chance out of 255 (0x94C54).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drop {
    pub chance: i32,
    pub item: i32,
}

/// One variant of an enemy: a record of its `param2` table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Variant {
    pub id: i32,
    pub name: String,
    /// Which of the script's AIs it runs.
    pub ai: i32,
    pub level: i32,
    pub hp: i32,
    /// HP of a second life, if it has one.
    pub second_hp: i32,
    pub attack: i32,
    pub special: i32,
    pub defence: i32,
    /// The experience beating it is worth.
    pub experience: i32,
    /// What it may leave behind, and what it may leave instead of that.
    pub drop: Drop,
    pub rare: Drop,
    /// How strong an attack has to be to make it reel, unless its script says otherwise.
    pub poise: i32,
    /// The three lock-point gauges: strength, recovery, and how long one stays broken.
    pub gauges: [[i32; 3]; 3],
    /// Percent of damage that gets through its guard.
    pub guard_percent: i32,
    /// How well it resists six kinds of status effect (record fields 0x70 to 0x84).
    pub resistances: [i32; 6],
    /// What the parts its second and third gauges are of are called (record fields
    /// 0xBC and 0xC0).
    pub part_names: [String; 2],
}

impl Variant {
    const FIELDS: usize = 49;

    /// What comes off the chance of a status effect of that kind taking on it
    /// (0x8F7DC): it has a figure for six of the ailments and none for the rest.
    pub fn resistance(&self, kind: i32) -> i32 {
        use super::status::kind::*;
        let index = match kind {
            POISON => 0,
            NO_DODGE => 1,
            STUN => 2,
            ATTACK_DOWN => 3,
            DEFENCE_DOWN => 4,
            PACE_DOWN => 5,
            _ => return 0,
        };
        self.resistances[index]
    }

    /// What the log calls it, or the part of it one of its gauges is of
    /// (0x12974C): the first gauge is of all of it.
    pub fn part_name(&self, gauge: usize) -> &str {
        match gauge {
            0 => &self.name,
            _ => self.part_names.get(gauge - 1).map_or("", String::as_str),
        }
    }

    fn from_record(table: &bxcb::Table, record: &[u32]) -> Result<Variant> {
        let field = |index: usize| record[index] as i32;
        Ok(Variant {
            id: field(0),
            name: table.string(record[1])?.to_owned(),
            ai: field(2),
            level: field(3),
            hp: field(4),
            second_hp: field(5),
            attack: field(6),
            special: field(7),
            defence: field(8),
            experience: field(9).max(0),
            drop: Drop { chance: field(10), item: field(11) },
            rare: Drop { chance: field(12), item: field(13) },
            poise: field(14),
            gauges: [[field(15), field(16), field(17)], [field(41), field(42), field(43)], [field(44), field(45), field(46)]],
            guard_percent: field(40),
            resistances: [28, 29, 30, 31, 32, 33].map(field),
            part_names: [table.string(record[47])?.to_owned(), table.string(record[48])?.to_owned()],
        })
    }
}

/// The families of enemies that the engine itself treats differently from the rest
/// (the third field of an enemy's `param1` record, kept at actor + 0x22C).
pub mod family {
    /// The tower that calls other enemies in. While one stands the battle goes on.
    pub const TOWER: i32 = 0x10;
    /// The enemies it calls come in even where somebody is in the way.
    pub const ANYWHERE: i32 = 0x12;
}

/// Everything in one enemy's archive (`BTL/CHR/BCHR_<NAME>.VOL`).
pub struct Archive {
    /// The enemy's number.
    pub enemy: u16,
    /// The folder, for messages.
    pub folder: String,
    /// Which family of enemies it is of: see [`family`].
    pub family: i32,
    pub script: Script,
    pub figures: Figures,
    pub variants: Vec<Variant>,
    /// Its animations, by file name.
    clips: HashMap<String, Arc<Clip>>,
    /// Its models and the file bytes their textures are decoded from, by file name.
    models: HashMap<String, Arc<(insm::Model, Vec<u8>)>>,
    /// The header of its bank of sounds, if it has one.
    pub bank: Option<String>,
    /// The header of the bank of its finishing move's sounds (`fin_*.phd`), which a
    /// script knows as bank 2.
    pub finish_bank: Option<String>,
    /// Its recordings, by file name, each with the folder it is in.
    voices: Vec<(String, String)>,
    /// The archives its script has loaded besides its own (0x95F78): a boss's
    /// scenes, its battle's own and its finishing move's. Their animations and
    /// recordings are among the archive's own.
    extras: Vec<String>,
    /// The camera moves of all of them, by file name.
    cameras: HashMap<String, Arc<CameraClip>>,
    /// The layouts they have as folders of their own, by name: the cut-in's and
    /// the finish's.
    layouts: HashMap<String, String>,
}

/// The layout of the finish, as every archive that has one names it (0x94528).
pub const FINISH_LAYOUT: &str = "if_btl_finish.lpk";

impl Archive {
    pub fn load(data: &GameData, enemy: u16) -> Result<Archive> {
        let name = ARCHIVES.iter().find(|(number, _)| *number == enemy).map(|(_, name)| *name);
        let name = name.with_context(|| format!("there is no enemy {enemy}"))?;
        let folder = format!("{ENEMIES}/BCHR_{name}.VOL");
        let script_path = format!("{folder}/{}.bms", name.to_lowercase());
        let script = Script::parse(&data.read(&script_path)?).with_context(|| format!("parsing {script_path}"))?;

        let files = data.list(&folder)?;
        let with_extension = |extension: &'static str| files.iter().filter(move |file| file.ends_with(extension));
        // The figures are shared between an enemy's archives and named after the first.
        let figures = match with_extension(".edx").next() {
            Some(file) => Figures::parse(&data.read(format!("{folder}/{file}"))?).with_context(|| format!("parsing {file}"))?,
            None => Figures::default(),
        };

        // One record about the enemy as a whole: its number, name and family.
        let about_path = format!("{folder}/param1.bin");
        let about = bxcb::Table::parse(&data.read(&about_path)?).with_context(|| format!("parsing {about_path}"))?;
        let family = about.records().next().and_then(|record| record.get(2)).map(|family| *family as i32);
        let family = family.with_context(|| format!("{about_path} has no record"))?;

        let table_path = format!("{folder}/param2.bin");
        let table = bxcb::Table::parse(&data.read(&table_path)?).with_context(|| format!("parsing {table_path}"))?;
        ensure!(table.fields == Variant::FIELDS, "{table_path} has {} fields per record", table.fields);
        let variants: Vec<Variant> = table.records().map(|record| Variant::from_record(&table, record)).collect::<Result<_>>()?;
        ensure!(!variants.is_empty(), "{table_path} has no records");

        let mut clips = HashMap::new();
        for file in with_extension(".anm") {
            // Some archives carry animations of a kind that is not decoded yet; an
            // enemy that asks for one of those stands still for it.
            match data.clip(format!("{folder}/{file}")) {
                Ok(clip) => {
                    clips.insert(file.clone(), clip);
                }
                Err(error) => bevy::log::warn!("{folder}/{file}: {error:#}"),
            }
        }
        let mut models = HashMap::new();
        for file in with_extension(".mdl") {
            models.insert(file.clone(), Arc::new(data.model(format!("{folder}/{file}"))?));
        }
        let bank = with_extension(".phd").next().map(|file| format!("{folder}/{file}"));
        let mut voices: Vec<(String, String)> = with_extension(".at3").map(|file| (folder.clone(), file.clone())).collect();
        let mut cameras = HashMap::new();
        for file in with_extension(".cam") {
            cameras.insert(file.to_lowercase(), data.camera(format!("{folder}/{file}"))?);
        }

        // What the script has loaded as it goes (0x95F78) is here from the start:
        // every archive it names that there is. A file of the same name as one
        // the enemy has already is the same file again.
        let named: Vec<String> = script.strings().map(str::to_owned).collect();
        let (mut extras, mut layouts, mut finish_bank) = (Vec::new(), HashMap::new(), None);
        for name in named.iter().filter(|name| name.to_lowercase().ends_with(".vol")) {
            let extra = format!("{ENEMIES}/{}", name.to_uppercase());
            let Ok(files) = data.list(&extra) else { continue };
            for file in &files {
                let path = format!("{extra}/{file}");
                let lower = file.to_lowercase();
                if lower.ends_with(".anm") && !clips.contains_key(file) {
                    match data.clip(&path) {
                        Ok(clip) => {
                            clips.insert(file.clone(), clip);
                        }
                        Err(error) => bevy::log::warn!("{path}: {error:#}"),
                    }
                } else if lower.ends_with(".cam") && !cameras.contains_key(&lower) {
                    cameras.insert(lower, data.camera(&path)?);
                } else if lower.ends_with(".at3") && !voices.iter().any(|(_, known)| known.eq_ignore_ascii_case(file)) {
                    voices.push((extra.clone(), file.clone()));
                } else if lower.ends_with(".phd") && finish_bank.is_none() {
                    finish_bank = Some(path);
                }
            }
            // A layout is a folder of the archive: the cut-in's, by the name the
            // script asks for it by, and the finish's.
            for layout in named.iter().map(String::as_str).filter(|name| name.to_lowercase().ends_with(".bin")).chain([FINISH_LAYOUT]) {
                let pack = format!("{extra}/{layout}");
                if data.exists(format!("{pack}/000.sdoa")) {
                    layouts.entry(layout.to_lowercase()).or_insert(pack);
                }
            }
            extras.push(extra);
        }
        // The enemy's own archive can have them too: one boss's has its cut-in, and
        // the last two bosses' have the finish's layout.
        for layout in named.iter().map(String::as_str).filter(|name| name.to_lowercase().ends_with(".bin")).chain([FINISH_LAYOUT]) {
            let own = format!("{folder}/{layout}");
            if data.exists(format!("{own}/000.sdoa")) {
                layouts.entry(layout.to_lowercase()).or_insert(own);
            }
        }
        Ok(Archive { enemy, folder, family, script, figures, variants, clips, models, bank, finish_bank, voices, extras, cameras, layouts })
    }

    /// The file of the recording called `name` (0x9CCAC), if the archive has it.
    pub fn voice(&self, name: &str) -> Option<String> {
        let (folder, file) = self.voices.iter().find(|(_, file)| file.eq_ignore_ascii_case(name))?;
        Some(format!("{folder}/{file}"))
    }

    /// The folders of the archive and of those its script loads, its own first.
    pub fn folders(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.folder).chain(&self.extras)
    }

    /// The camera move called `name` (0x9C9F0).
    pub fn camera(&self, name: &str) -> Option<&Arc<CameraClip>> {
        self.cameras.get(&name.to_lowercase())
    }

    /// The pack a layout of its archives is, by the layout's name (0x9CD28, 0x9CDA4).
    pub fn layout(&self, name: &str) -> Option<&str> {
        self.layouts.get(&name.to_lowercase()).map(String::as_str)
    }

    /// Every layout its archives have, by name.
    pub fn layouts(&self) -> impl Iterator<Item = (&str, &str)> {
        self.layouts.iter().map(|(name, pack)| (name.as_str(), pack.as_str()))
    }

    /// The variant with the given id, or the first if there is none (0x91D20).
    pub fn variant(&self, id: i32) -> &Variant {
        self.variants.iter().find(|variant| variant.id == id).unwrap_or(&self.variants[0])
    }

    pub fn clip(&self, name: &str) -> Option<&Arc<Clip>> {
        self.clips.get(name)
    }

    pub fn model(&self, name: &str) -> Option<&Arc<(insm::Model, Vec<u8>)>> {
        self.models.get(name)
    }
}

/// The enemy groups (`enmgrp2.dat`).
pub fn groups(data: &GameData) -> Result<Groups> {
    Groups::parse(&data.read(GROUPS)?).with_context(|| format!("parsing {GROUPS}"))
}

/// The members of a group that are enemies, not places kept for later ones.
pub fn members(group: &egtb::Group) -> impl Iterator<Item = &egtb::Member> {
    group.members.iter().filter(|member| member.enemy != egtb::NO_ENEMY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attack(id: i32, enemy: i32, variant: i32, dealt_percent: i32) -> Attack {
        Attack { id, enemy, variant, dealt_percent, ..Attack::default() }
    }

    #[test]
    fn an_enemys_attack_falls_back_to_any_variant_and_then_to_anyone() {
        let attacks = Attacks::from_records(vec![
            attack(100, 0, -1, 1),
            attack(100, 8, -1, 2),
            attack(100, 8, 41, 3),
            attack(200, 11, -1, 4),
        ]);
        assert_eq!(attacks.of_enemy(100, 8, 41).unwrap().dealt_percent, 3);
        assert_eq!(attacks.of_enemy(100, 8, 10).unwrap().dealt_percent, 2);
        assert_eq!(attacks.of_enemy(100, 9, 10).unwrap().dealt_percent, 1);
        assert_eq!(attacks.of_player(100).unwrap().dealt_percent, 1);
        // An attack only some enemy has is nobody else's.
        assert!(attacks.of_enemy(200, 8, 10).is_none());
        assert!(attacks.of_player(200).is_none());
    }

    #[test]
    fn a_shot_of_one_enemy_comes_before_the_one_anyone_fires() {
        let shot = |id: i32, enemy: i32, count: i32| Shot { id, enemy, variant: -1, count, ..Shot::default() };
        let shots = Shots::from_records(vec![shot(810, 0, 5), shot(810, 44, 3), shot(300, 0, 5)]);
        assert_eq!(shots.of_enemy(810, 44, 0).unwrap().count, 3);
        assert_eq!(shots.of_enemy(810, 19, 0).unwrap().count, 5);
        assert!(shots.of_enemy(811, 19, 0).is_none());
    }

    #[test]
    fn a_shot_record_is_read_as_the_bullet_classes_read_it() {
        // The Hornet's guided volley, with the fields that differ made up.
        let mut record = [0u32; Shot::FIELDS];
        let values: [(usize, i32); 27] = [
            (30, 105),
            (31, 8),
            (32, 550),
            (33, 60),
            (34, 200),
            (35, 998_018),
            (36, -1),
            (0, 310),
            (2, -1),
            (3, 50),
            (5, -20),
            (7, 90),
            (10, 162),
            (11, 301),
            (12, 1),
            (14, 2),
            (17, 70),
            (18, 10),
            (19, 45),
            (20, 150),
            (21, -1),
            (22, 400),
            (23, -5),
            (39, 311),
            (42, 50),
            (44, 250),
            (46, 1),
        ];
        for (index, value) in values {
            record[index] = value as u32;
        }
        let shot = Shot::from_record(&record);
        assert_eq!((shot.id, shot.variant, shot.node, shot.attack, shot.kind, shot.next), (310, -1, 50, 301, 2, 311));
        assert_eq!(shot.offset, Vec3::new(0.0, -20.0, 0.0));
        assert_eq!((shot.yaw, shot.launch_pitch, shot.launch_yaw), (90.0, 10.0, 45.0));
        // Over 100 percent is all of it; no time limit is a long one; a negative
        // distance is none.
        assert_eq!((shot.homing, shot.homing_time, shot.homing_range, shot.homing_until), (1.0, 10_000.0, 400.0, 0.0));
        assert_eq!((shot.speed, shot.radius, shot.scale, shot.life), (70.0, 50.0, 2.5, 0.0));
        assert_eq!((shot.beam_time, shot.beam_tick), (105.0, 8.0));
        assert_eq!((shot.peak, shot.fuse, shot.blast), (550.0, 60.0, 200.0));
        assert_eq!((shot.blast_effect, shot.blast_sound), (998_018, -1));
        assert!(shot.from_node);
    }

    fn new_player() -> PlayerStats {
        PlayerStats {
            level: 1,
            experience: 0,
            max_hp: 372.0,
            hp: 100.0,
            attack: 44,
            special: 41,
            defence: 24,
            burst_bullets: 6,
            dodge_heat: 15,
            skill_bonus: [0; 4],
            skills: [0; 4],
            weapon: 129,
            suit: 133,
        }
    }

    #[test]
    fn the_level_goes_by_the_last_threshold_reached() {
        let levels = Levels::for_tests();
        let level = |experience: u32| levels.level_for(experience);
        assert_eq!((level(0), level(15), level(16), level(118), level(119)), (1, 1, 2, 2, 3));
        assert_eq!((level(999_998), level(999_999), level(u32::MAX)), (49, 50, 50));
        assert_eq!((levels.threshold(1), levels.threshold(2), levels.threshold(50)), (Some(0), Some(16), Some(999_999)));
        assert_eq!((levels.next(1), levels.next(49), levels.next(50)), (Some(16), Some(999_999), None));
    }

    #[test]
    fn experience_takes_her_up_as_many_levels_as_it_comes_to() {
        let levels = Levels::for_tests();
        let mut stats = new_player();
        // Short of the next level nothing changes but the experience.
        assert_eq!(stats.gain(&levels, 15, true).unwrap(), 0);
        assert_eq!((stats.level, stats.experience, stats.hp, stats.max_hp), (1, 15, 100.0, 372.0));
        // Going up adds the gains of the level she leaves and makes her whole.
        assert_eq!(stats.gain(&levels, 1, true).unwrap(), 1);
        assert_eq!((stats.level, stats.experience), (2, 16));
        assert_eq!((stats.max_hp, stats.hp, stats.attack, stats.special, stats.defence), (434.0, 434.0, 45, 44, 25));
        // Several levels at once add each one's gains; where a level does not heal,
        // her HP stays as it was.
        stats.hp = 50.0;
        assert_eq!(stats.gain(&levels, 843 - 16, false).unwrap(), 3);
        assert_eq!((stats.level, stats.experience, stats.hp), (5, 843, 50.0));
        assert_eq!((stats.max_hp, stats.attack, stats.special, stats.defence), (621.0, 53, 57, 31));
        // The same figures as a player started at that level has.
        let started = new_player().at_level(&levels, 5).unwrap();
        assert_eq!(started, PlayerStats { hp: 621.0, ..stats });
    }

    #[test]
    fn a_repairer_gives_hp_back_and_a_weapon_changes_her_attack() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let items = Items::load(&data).expect("the item tables load");
        let mut stats = PlayerStats::new_game(&data).expect("the start table loads");
        assert_eq!((stats.weapon, stats.suit, stats.attack, stats.burst_bullets, stats.dodge_heat), (129, 133, 44, 6, 15));
        // With all her HP there a Repairer would do nothing; what is for battles
        // and what is worn never does.
        let item = |number: i32| items.get(number).unwrap();
        assert!(!stats.helped_by(item(1)));
        stats.hp = 100.0;
        assert!(stats.helped_by(item(1)) && !stats.helped_by(item(14)) && !stats.helped_by(item(129)));
        // A Repairer 200 gives two hundred, and no more than all she can have.
        assert_eq!((stats.take(item(1)), stats.hp), (200, 300.0));
        assert_eq!((stats.take(item(1)), stats.hp), (72, 372.0));
        // A Nano Repair gives a share of all of it.
        stats.hp = 1.0;
        assert_eq!((stats.take(item(6)), stats.hp), (186, 187.0));
        assert_eq!((stats.take(item(14)), stats.take(item(129)), stats.hp), (0, 0, 187.0));

        // The Hyperion adds sixty to her attack where the Rock Cannon added
        // fifteen, and has eight bullets to a burst.
        stats.equip(&items, 132);
        assert_eq!((stats.weapon, stats.attack, stats.burst_bullets), (132, 89, 8));
        stats.equip(&items, 132);
        assert_eq!(stats.attack, 89);
        // A suit is only put on: with Lost All a dodge costs less heat.
        stats.equip(&items, 134);
        assert_eq!((stats.suit, stats.dodge_heat, stats.attack, stats.weapon), (134, 10, 89, 132));
        stats.equip(&items, 129);
        assert_eq!((stats.weapon, stats.attack, stats.burst_bullets), (129, 44, 6));
        // Nothing comes of what cannot be worn.
        let before = stats;
        stats.equip(&items, 1);
        stats.equip(&items, 257);
        stats.equip(&items, 0);
        assert_eq!(stats, before);
    }
    #[test]
    fn the_games_own_tables_say_what_she_starts_with_and_what_an_eater_is_worth() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let levels = Levels::load(&data).expect("the level table loads");
        assert_eq!((levels.threshold(2), levels.threshold(3), levels.threshold(50)), (Some(16), Some(119), Some(999_999)));
        let stats = PlayerStats::new_game(&data).expect("the start table loads");
        assert_eq!((stats.level, stats.experience, stats.max_hp, stats.attack), (1, 0, 372.0, 44));
        let twenty = stats.at_level(&levels, 20).unwrap();
        assert_eq!((twenty.max_hp, twenty.attack, twenty.special, twenty.defence), (1557.0, 124, 177, 85));
        assert_eq!(levels.level_for(twenty.experience), 20);

        // The first map's Eater: 8 experience, and a Repairer 200 or, rarely, a 500.
        let archive = Archive::load(&data, 8).expect("the Eater's archive loads");
        let eater = archive.variant(0);
        assert_eq!((eater.experience, eater.drop, eater.rare), (8, Drop { chance: 112, item: 1 }, Drop { chance: 48, item: 2 }));
    }

    #[test]
    fn the_start_table_has_her_at_her_most_for_the_opening_and_as_a_new_game_after_it() {
        use crate::battle::skills::{HAS, SET};
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (starts, items) = (Starts::load(&data).expect("the start table loads"), Items::load(&data).expect("the item tables load"));
        // The game's own start: the first record, given to one who has nothing.
        let (mut stats, mut inventory, mut progress) = (PlayerStats::NONE, Inventory::default(), Progress::start());
        starts.give(Starts::OPENING, &mut stats, &items, &mut inventory, &mut progress);
        assert_eq!((stats.level, stats.experience, stats.max_hp, stats.hp), (50, 999_999, 9999.0, 9999.0));
        assert_eq!((stats.attack, stats.special, stats.defence), (999, 999, 999));
        assert_eq!((stats.weapon, stats.suit, stats.skills), (131, 135, [2, 23, 12, 15]));
        assert_eq!(inventory.all().collect::<Vec<_>>(), [(131, 1), (135, 1)]);
        assert!([2, 23, 12, 15].iter().all(|&skill| progress.flag(HAS + skill) && progress.flag(SET + skill)));
        assert!(!progress.flag(HAS + 1) && !progress.flag(SET + 1));

        // The second, which the opening's script gives her: a new game's figures,
        // nothing in her slots, and one of what it has her wear as she had none.
        starts.give(1, &mut stats, &items, &mut inventory, &mut progress);
        assert_eq!(stats, PlayerStats::new_game(&data).unwrap());
        assert_eq!((stats.level, stats.experience, stats.attack, stats.skills), (1, 0, 44, [0; 4]));
        assert_eq!(inventory.all().collect::<Vec<_>>(), [(129, 1), (131, 1), (133, 1), (135, 1)]);

        // A record with a suit and nothing else changes nothing else, and gives
        // none of a thing she has; one the table has not gives nothing.
        let before = stats;
        starts.give(3, &mut stats, &items, &mut inventory, &mut progress);
        assert_eq!(stats, PlayerStats { suit: 135, dodge_heat: stats.dodge_heat, ..before });
        assert_eq!(inventory.held(135), 1);
        starts.give(8, &mut stats, &items, &mut inventory, &mut progress);
        starts.give(-1, &mut stats, &items, &mut inventory, &mut progress);
        assert_eq!(stats.suit, 135);
    }

    #[test]
    fn at_the_last_level_nothing_more_is_taken() {
        let levels = Levels::for_tests();
        let mut stats = new_player().at_level(&levels, 49).unwrap();
        assert_eq!(stats.level, 49);
        // Reaching the last level leaves her with just what it takes to reach it.
        assert_eq!(stats.gain(&levels, 5_000_000, true).unwrap(), 1);
        assert_eq!((stats.level, stats.experience), (50, 999_999));
        let before = stats;
        assert_eq!(stats.gain(&levels, 1_000, true).unwrap(), 0);
        assert_eq!(stats, before);
    }

    #[test]
    fn every_enemy_number_is_listed_once() {
        let mut numbers: Vec<u16> = ARCHIVES.iter().map(|(number, _)| *number).collect();
        numbers.dedup();
        assert_eq!(numbers.len(), ARCHIVES.len());
        assert!(numbers.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
