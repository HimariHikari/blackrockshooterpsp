//! The battle system's tunable values, from `SYSTEM.VOL/btl_system.bin`.
//!
//! The original looks each one up by id wherever it needs it (0xA111C), which is how
//! the ids below were matched to what they do. Times are in sixtieths of a second.

use std::collections::HashMap;

use anyhow::{ensure, Context, Result};
use bevy::prelude::Resource;
use brs_formats::bxcb;

use crate::data::GameData;

const FILE: &str = "SYSTEM.VOL/btl_system.bin";

// Fields of a record.
const FIELD_ID: usize = 0;
const FIELD_KIND: usize = 1;
const FIELD_VALUE: usize = 2;
const FIELD_MAX: usize = 3;
const FIELD_MIN: usize = 4;
const FIELDS: usize = 8;

/// The value is stored in hundredths.
const KIND_HUNDREDTHS: u32 = 1;

/// Ids of the tunables the port reads.
pub mod id {
    /// How long the camera of a snipe's scope takes to close in (0x8C844). (Value
    /// 3, "sniper lens size", is 200, as wide as the lens that is drawn; the code
    /// that draws it has the figure itself, 0x42438.)
    pub const SNIPER_ENTER: u32 = 4;
    /// The times of a snipe's sight, on the clock that starts as it comes: when
    /// the window to fire may open, when it starts to lock on, and how long the
    /// locking on takes (0x7FEF4, 0x7FF58, 0x7FFA8).
    pub const SNIPE_UI_ENTER: u32 = 76;
    pub const SNIPE_LOCK_START: u32 = 77;
    pub const SNIPE_LOCK_WAIT: u32 = 78;
    /// Debug switch: the heat gauge never fills.
    pub const NO_OVERHEAT: u32 = 11;
    /// Heat lost per second, normally and while overheated.
    pub const HEAT_RECOVERY: u32 = 13;
    pub const HEAT_RECOVERY_OVERHEATED: u32 = 83;
    /// How long after gaining heat the gauge starts to fall again.
    pub const HEAT_RECOVERY_WAIT: u32 = 14;
    /// Extra wait before heat falls that a dodge adds.
    pub const DODGE_RECOVERY_WAIT: u32 = 15;
    /// How far the aim can tilt the camera, in degrees: looking down (X) and to
    /// either side (Y).
    pub const CAMERA_TILT_MIN: u32 = 24;
    pub const CAMERA_TILT_MAX: u32 = 25;
    pub const CAMERA_TURN_MIN: u32 = 26;
    pub const CAMERA_TURN_MAX: u32 = 27;
    /// Debug switch: her skills can be used whether they are ready or not
    /// (0x9A5C4).
    pub const SKILLS_ALWAYS_READY: u32 = 17;
    /// Debug switch: effects are not scaled up.
    pub const NO_EFFECT_SCALE: u32 = 30;
    /// Debug switch: dodging is allowed before the story unlocks it.
    pub const DODGE_ALWAYS: u32 = 38;
    /// Height of the point the camera tilts and turns about.
    pub const CAMERA_PIVOT_HEIGHT: u32 = 39;
    /// Where the camera sits relative to the player, and which way it looks, in
    /// degrees.
    pub const CAMERA_X: u32 = 40;
    pub const CAMERA_Y: u32 = 41;
    pub const CAMERA_Z: u32 = 42;
    pub const CAMERA_PITCH: u32 = 43;
    pub const CAMERA_YAW: u32 = 44;
    /// The tilt and turn a battle starts with, in degrees.
    pub const CAMERA_TILT_START: u32 = 45;
    pub const CAMERA_TURN_START: u32 = 46;
    /// How long she cannot shoot again after a burst.
    pub const SHOT_STIFFNESS: u32 = 47;
    /// Heat each burst, dodge and guard costs.
    pub const SHOT_HEAT: u32 = 48;
    pub const DODGE_HEAT: u32 = 51;
    pub const GUARD_HEAT: u32 = 52;
    /// Heat a guarded hit adds.
    pub const GUARDED_HIT_HEAT: u32 = 53;
    /// Percent of a hit's damage that gets through her guard.
    pub const GUARD_PERCENT: u32 = 54;
    pub const HEAT_MAX: u32 = 49;
    /// The heat using an item adds (0x98384): none, as the game's table has it.
    pub const HEAT_ITEM: u32 = 50;
    /// The shares of the most heat there is, in hundredths, at which the bar of the
    /// heat gauge changes colour (0xD2C64).
    pub const HEAT_FIRST_PERCENT: u32 = 81;
    pub const HEAT_SECOND_PERCENT: u32 = 82;
    /// Heat at which a burst drops to its second, third, fourth and fifth stage.
    pub const BURST_STAGES: [u32; 4] = [55, 56, 57, 58];
    /// Bullets in a burst, and the time between them, at each of the five stages.
    pub const BURST_BULLETS: [u32; 5] = [59, 61, 63, 65, 67];
    pub const BURST_INTERVAL: [u32; 5] = [60, 62, 64, 66, 68];
    /// Percent of her HP at or under which its gauge is amber, and red (0xD2BBC).
    pub const HP_CARE_PERCENT: u32 = 79;
    pub const HP_DANGER_PERCENT: u32 = 80;
    /// How long a line of the log stays up when another waits, in sixtieths
    /// (0x96C00).
    pub const LOG_SHORT_TIME: u32 = 73;
    /// How dark what is behind her is made for a skill, of 255 (0x82124).
    pub const SKILL_DIM: u32 = 84;
    /// Debug switch: what follows a battle won does not wait for the log (0x9DB78).
    pub const LOG_NOT_WAITED_FOR: u32 = 86;
    /// The highest a blade's hit is seen on what it strikes.
    pub const BLADE_HIT_TOP: u32 = 87;
}

#[derive(Clone, Copy)]
struct Tunable {
    value: f32,
    min: f32,
    max: f32,
}

#[derive(Resource, Clone)]
pub struct Tunables(HashMap<u32, Tunable>);

impl Tunables {
    pub fn load(data: &GameData) -> Result<Tunables> {
        let table = bxcb::Table::parse(&data.read(FILE)?).with_context(|| format!("parsing {FILE}"))?;
        ensure!(table.fields == FIELDS, "{FILE} has {} fields per record, not {FIELDS}", table.fields);
        let mut tunables = HashMap::new();
        for record in table.records() {
            let scale = if record[FIELD_KIND] == KIND_HUNDREDTHS { 0.01 } else { 1.0 };
            let field = |index: usize| record[index] as i32 as f32 * scale;
            let tunable = Tunable { value: field(FIELD_VALUE), min: field(FIELD_MIN), max: field(FIELD_MAX) };
            tunables.insert(record[FIELD_ID], tunable);
        }
        Ok(Tunables(tunables))
    }

    /// The value of tunable `id`. Panics if the table has no such tunable: every id the
    /// port uses is one the original's code looks up too.
    pub fn float(&self, id: u32) -> f32 {
        self.0.get(&id).unwrap_or_else(|| panic!("btl_system.bin has no tunable {id}")).value
    }

    pub fn int(&self, id: u32) -> i32 {
        self.float(id) as i32
    }

    pub fn flag(&self, id: u32) -> bool {
        self.float(id) != 0.0
    }

    /// Changes tunable `id`, kept within its allowed range, as the original does when
    /// a battle starts with values that depend on what is equipped (0xA4D6C).
    pub fn set(&mut self, id: u32, value: f32) {
        if let Some(tunable) = self.0.get_mut(&id) {
            tunable.value = value.clamp(tunable.min, tunable.max);
        }
    }

    /// A table with the given values, for tests.
    #[cfg(test)]
    pub fn from_values(values: &[(u32, f32)]) -> Tunables {
        Tunables(values.iter().map(|&(id, value)| (id, Tunable { value, min: f32::MIN, max: f32::MAX })).collect())
    }
}
