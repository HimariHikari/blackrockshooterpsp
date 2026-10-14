//! The game's progress: the flags and variables its scripts keep in the save data
//! (the original's functions at 0x1354 to 0x18B4; see `docs/FIELD_NOTES.md`).

use bevy::prelude::*;

/// Flags 0 to 9999 are saved, one bit each.
const FLAGS: usize = 10_000;
/// Flags from here on live outside the save data and last only for the session.
const FIRST_PASSING_FLAG: i32 = 100_000;
const PASSING_FLAGS: usize = 257;

/// Variables of 32 bits. The setter takes one number more than the getter reads.
const FIRST_WORD: i32 = 10_000;
const WORDS: usize = 32;
/// Variables of 16 bits, read back unsigned.
const FIRST_SHORT: i32 = 20_000;
const SHORTS: usize = 501;
/// Variables outside the save data.
const FIRST_PASSING: i32 = 100_000;
const PASSING: usize = 3;
/// Every other number is a variable of 8 bits, read back unsigned; the original keeps
/// 512 of them.
const BYTES: usize = 512;

/// The one 32-bit variable with a ceiling: it is kept below 10000.
const CAPPED_WORD: i32 = 10_001;
const CAPPED_WORD_MAX: i32 = 9_999;

/// Variables with a known meaning.
pub mod variable {
    /// Set to 1 by a new game.
    pub const STARTED: i32 = 40;
    /// Set to 10 as the field is first entered.
    pub const FIRST_ENTRY: i32 = 100;
    /// How far the current mission has got; each mission's script counts in its own way.
    pub const MISSION_PROGRESS: i32 = 220;
    /// The enemies beaten in battles, up to 99.
    pub const ENEMIES_BEATEN: i32 = 237;
    /// Where the game is, for a save (0x23E0, 0x2400): 0 on the field, 1 on the
    /// screen between missions.
    pub const SAVED_AT: i32 = 300;
    /// How many times the story has been played to its end: the last map's
    /// script adds to it, and the game counts as cleared by it (0x394).
    pub const TIMES_ENDED: i32 = 10_001;
    /// The number of the mission (the "scenario") being played.
    pub const SCENARIO: i32 = 10_004;
    /// While above zero, the damage she takes in battles is counted off it.
    pub const DAMAGE_LEFT: i32 = 10_005;
    /// The damage she has dealt in battles, up to 999999.
    pub const DAMAGE_DEALT: i32 = 10_006;
}

/// A battle's own figures, as the battle leaves them for the field when it is won
/// (the result record at 0x2EAF8, + 0xC to + 0x18; filled at 0xA022C).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// How long it took, in whole seconds.
    pub seconds: i32,
    pub damage_taken: i32,
    pub damage_dealt: i32,
    pub enemies_beaten: i32,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Progress {
    flags: Vec<u32>,
    passing_flags: [bool; PASSING_FLAGS],
    words: [i32; WORDS],
    shorts: [u16; SHORTS],
    bytes: [u8; BYTES],
    passing: [i32; PASSING],
    /// The stage the screen where a mission is chosen shows as it comes
    /// (0x22B8, 0x2290): the one a mission was last chosen of.
    pub hub_stage: u8,
}

impl Progress {
    /// Everything clear, as before a game is started or loaded.
    fn empty() -> Progress {
        Progress {
            flags: vec![0; FLAGS.div_ceil(32)],
            passing_flags: [false; PASSING_FLAGS],
            words: [0; WORDS],
            shorts: [0; SHORTS],
            bytes: [0; BYTES],
            passing: [0; PASSING],
            hub_stage: 0,
        }
    }

    /// A new game as it stands when the field is first entered: what the game's own
    /// start clears and sets, and what choosing "new game" then sets.
    #[cfg(test)]
    pub fn new_game() -> Progress {
        let mut progress = Progress::start();
        progress.begin_first_mission();
        progress
    }

    /// What the game's own start clears and sets (0x13C, 0x908). The system
    /// script's `init_player` runs after it.
    pub fn start() -> Progress {
        let mut progress = Progress::empty();
        progress.set_variable(variable::STARTED, 1);
        progress
    }

    /// What choosing "new game" at the title then sets (0x11A1B4): the first mission,
    /// at its beginning.
    pub fn begin_first_mission(&mut self) {
        self.set_variable(variable::FIRST_ENTRY, 10);
        self.set_variable(variable::SCENARIO, 0);
        self.set_variable(variable::MISSION_PROGRESS, 100);
    }

    /// What the story's opening has left set by the time it hands over to the first
    /// mission's map. A new game starts on another map, `d_stg05_0a1` (0x95C), whose
    /// script plays the opening (`EV01_01_0100B`: the story so far, the first lesson
    /// and its battle, the opening film) and then goes on to the first mission's
    /// (`s68(0, 2)`). The port starts there, with the opening behind it: what the
    /// opening set and did not clear again are two flags, one that the first
    /// mission's own opening clears (6156, 0x14B704), and the one that keeps "STAGE
    /// SELECT" out of reach until the first mission is cleared (6169).
    pub fn after_the_opening(&mut self) {
        for flag in [6156, crate::field::failed::NO_STAGE_SELECT] {
            self.set_flag(flag, true);
        }
    }

    /// Takes a battle's figures into the counts the missions go by, as the field does
    /// when it comes back from one (0xD893C). The battle's time goes to the mission's
    /// clock as well, which the field's system script keeps (`update_mis_time`,
    /// called by `field::end_battle`).
    pub fn count_battle(&mut self, tally: &Tally) {
        const MOST: i32 = 999_999;
        let left = self.variable(variable::DAMAGE_LEFT);
        if left > 0 {
            self.set_variable(variable::DAMAGE_LEFT, (left - tally.damage_taken.min(MOST)).max(0));
        }
        let dealt = self.variable(variable::DAMAGE_DEALT).saturating_add(tally.damage_dealt);
        self.set_variable(variable::DAMAGE_DEALT, dealt.min(MOST));
        let beaten = self.variable(variable::ENEMIES_BEATEN) + tally.enemies_beaten;
        self.set_variable(variable::ENEMIES_BEATEN, beaten.min(99));
    }

    /// The progress as the save's block has it (the block's + 0xC, 0x2444): the
    /// 32 words, the flags from + 0x80, the bytes from + 0x564 and the halfwords
    /// from + 0x764. What is not saved is not in it.
    pub fn to_block(&self) -> Vec<u8> {
        let mut block = Vec::with_capacity(Self::BLOCK);
        block.extend(self.words.iter().flat_map(|word| word.to_le_bytes()));
        block.extend(self.flags.iter().flat_map(|word| word.to_le_bytes()));
        block.extend(self.bytes);
        block.extend(self.shorts.iter().flat_map(|short| short.to_le_bytes()));
        debug_assert_eq!(block.len(), Self::BLOCK);
        block
    }

    /// The progress a block has, with nothing of what is not saved; nothing if
    /// the block is not of the size.
    pub fn from_block(block: &[u8]) -> Option<Progress> {
        if block.len() != Self::BLOCK {
            return None;
        }
        let mut progress = Progress::empty();
        let word = |at: usize| u32::from_le_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]]);
        for (index, kept) in progress.words.iter_mut().enumerate() {
            *kept = word(index * 4) as i32;
        }
        for (index, kept) in progress.flags.iter_mut().enumerate() {
            *kept = word(Self::BLOCK_FLAGS_AT + index * 4);
        }
        progress.bytes.copy_from_slice(&block[Self::BLOCK_BYTES_AT..Self::BLOCK_SHORTS_AT]);
        for (index, kept) in progress.shorts.iter_mut().enumerate() {
            let at = Self::BLOCK_SHORTS_AT + index * 2;
            *kept = u16::from_le_bytes([block[at], block[at + 1]]);
        }
        Some(progress)
    }

    // Where the parts of the block are, and its size: the flags are 313 words
    // (0x12FC clears as many).
    const BLOCK_FLAGS_AT: usize = WORDS * 4;
    const BLOCK_BYTES_AT: usize = Self::BLOCK_FLAGS_AT + FLAGS.div_ceil(32) * 4;
    const BLOCK_SHORTS_AT: usize = Self::BLOCK_BYTES_AT + BYTES;
    pub const BLOCK: usize = Self::BLOCK_SHORTS_AT + SHORTS * 2;

    pub fn flag(&self, number: i32) -> bool {
        match usize::try_from(number) {
            Ok(index) if index < FLAGS => self.flags[index / 32] & (1 << (index % 32)) != 0,
            _ => passing_index(number, FIRST_PASSING_FLAG, PASSING_FLAGS).is_some_and(|index| self.passing_flags[index]),
        }
    }

    pub fn set_flag(&mut self, number: i32, on: bool) {
        match usize::try_from(number) {
            Ok(index) if index < FLAGS => {
                let bit = 1 << (index % 32);
                let word = &mut self.flags[index / 32];
                *word = if on { *word | bit } else { *word & !bit };
            }
            _ => {
                if let Some(index) = passing_index(number, FIRST_PASSING_FLAG, PASSING_FLAGS) {
                    self.passing_flags[index] = on;
                }
            }
        }
    }

    pub fn variable(&self, number: i32) -> i32 {
        if let Some(index) = passing_index(number, FIRST_WORD, WORDS) {
            self.words[index]
        } else if let Some(index) = passing_index(number, FIRST_SHORT, SHORTS) {
            self.shorts[index] as i32
        } else if let Some(index) = passing_index(number, FIRST_PASSING, PASSING) {
            self.passing[index]
        } else {
            // A number outside the 512 reads whatever lies beyond them in the
            // original; nothing asks for one.
            usize::try_from(number).ok().and_then(|index| self.bytes.get(index)).map_or(0, |&byte| byte as i32)
        }
    }

    pub fn set_variable(&mut self, number: i32, value: i32) {
        if let Some(index) = passing_index(number, FIRST_WORD, WORDS) {
            self.words[index] = if number == CAPPED_WORD { value.min(CAPPED_WORD_MAX) } else { value };
        } else if let Some(index) = passing_index(number, FIRST_SHORT, SHORTS) {
            self.shorts[index] = value as u16;
        } else if let Some(index) = passing_index(number, FIRST_PASSING, PASSING) {
            self.passing[index] = value;
        } else if let Some(byte) = usize::try_from(number).ok().and_then(|index| self.bytes.get_mut(index)) {
            *byte = value as u8;
        } else {
            // The original would write past its 512 bytes here.
            warn!("variable {number} does not exist; {value} is not kept");
        }
    }
}

/// Where `number` falls in a run of `count` numbers starting at `first`.
fn passing_index(number: i32, first: i32, count: usize) -> Option<usize> {
    usize::try_from(number.checked_sub(first)?).ok().filter(|&index| index < count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_starts_the_first_mission_from_the_top() {
        let progress = Progress::new_game();
        assert_eq!(progress.variable(variable::SCENARIO), 0);
        assert_eq!(progress.variable(variable::MISSION_PROGRESS), 100);
        assert_eq!(progress.variable(variable::FIRST_ENTRY), 10);
        assert_eq!(progress.variable(variable::STARTED), 1);
        // The story has not begun: no event has been seen and no flag is set.
        assert_eq!(progress.variable(10_003), 0);
        assert!((0..FLAGS as i32).all(|flag| !progress.flag(flag)));
    }

    #[test]
    fn flags_are_single_bits_that_do_not_disturb_their_neighbours() {
        let mut progress = Progress::empty();
        for flag in [0, 31, 32, 3001, 9999] {
            progress.set_flag(flag, true);
        }
        assert!(progress.flag(31) && progress.flag(32) && progress.flag(3001) && progress.flag(9999));
        assert!(!progress.flag(30) && !progress.flag(33) && !progress.flag(3002));
        progress.set_flag(32, false);
        assert!(progress.flag(31) && !progress.flag(32));
        // Flags that do not exist stay clear, and the session's own are kept apart.
        progress.set_flag(10_000, true);
        progress.set_flag(-1, true);
        assert!(!progress.flag(10_000) && !progress.flag(-1));
        progress.set_flag(100_256, true);
        assert!(progress.flag(100_256) && !progress.flag(100_255) && !progress.flag(100_257));
    }

    #[test]
    fn each_range_of_variables_has_its_own_width() {
        let mut progress = Progress::empty();
        // Plain numbers are bytes, read back unsigned.
        progress.set_variable(220, 300);
        assert_eq!(progress.variable(220), 300 % 256);
        progress.set_variable(239, -1);
        assert_eq!(progress.variable(239), 255);
        // 10000 and up are whole words.
        progress.set_variable(10_003, 1_010_550);
        progress.set_variable(10_005, -7);
        assert_eq!((progress.variable(10_003), progress.variable(10_005)), (1_010_550, -7));
        // 20000 and up are 16 bits, unsigned.
        progress.set_variable(20_500, 70_000);
        assert_eq!(progress.variable(20_500), 70_000 % 65_536);
        progress.set_variable(100_002, 12);
        assert_eq!(progress.variable(100_002), 12);
        // A number that is no variable reads as nothing.
        progress.set_variable(5_000, 9);
        assert_eq!(progress.variable(5_000), 0);
        assert_eq!(progress.variable(-3), 0);
    }

    #[test]
    fn a_battles_figures_go_into_the_missions_counts() {
        let mut progress = Progress::empty();
        let tally = Tally { seconds: 20, damage_taken: 30, damage_dealt: 700, enemies_beaten: 4 };
        progress.count_battle(&tally);
        // With no damage being counted off, none is.
        assert_eq!(progress.variable(variable::DAMAGE_LEFT), 0);
        assert_eq!(progress.variable(variable::DAMAGE_DEALT), 700);
        assert_eq!(progress.variable(variable::ENEMIES_BEATEN), 4);

        progress.set_variable(variable::DAMAGE_LEFT, 50);
        progress.count_battle(&tally);
        assert_eq!(progress.variable(variable::DAMAGE_LEFT), 20);
        progress.count_battle(&tally);
        assert_eq!(progress.variable(variable::DAMAGE_LEFT), 0);
        assert_eq!(progress.variable(variable::DAMAGE_DEALT), 2100);
        // Each count has its ceiling.
        progress.count_battle(&Tally { damage_dealt: 5_000_000, enemies_beaten: 200, ..tally });
        assert_eq!(progress.variable(variable::DAMAGE_DEALT), 999_999);
        assert_eq!(progress.variable(variable::ENEMIES_BEATEN), 99);
    }

    #[test]
    fn the_saves_block_holds_what_is_saved_and_gives_it_back() {
        let mut progress = Progress::start();
        for flag in [0, 31, 32, 6157, 9999] {
            progress.set_flag(flag, true);
        }
        progress.set_variable(10_004, 7);
        progress.set_variable(220, 115);
        progress.set_variable(20_500, 40_000);
        progress.hub_stage = 3;
        let block = progress.to_block();
        assert_eq!(block.len(), 0xb4e);
        // The words first, the flags from 0x80, the bytes from 0x564, the
        // halfwords from 0x764.
        assert_eq!(block[4 * 4], 7);
        assert_eq!((block[0x80], block[0x80 + 3], block[0x80 + 4]), (1, 0x80, 1));
        assert_eq!(block[0x80 + 9999 / 8], 1 << (9999 % 8));
        assert_eq!((block[0x564 + 40], block[0x564 + 220]), (1, 115));
        assert_eq!(u16::from_le_bytes([block[0x764 + 1000], block[0x764 + 1001]]), 40_000);
        let back = Progress::from_block(&block).unwrap();
        assert!([0, 31, 32, 6157, 9999].into_iter().all(|flag| back.flag(flag)) && !back.flag(1) && !back.flag(9998));
        assert_eq!((back.variable(10_004), back.variable(220), back.variable(20_500), back.variable(40)), (7, 115, 40_000, 1));
        // What is not saved is not in it: the session's flags, and the stage the
        // screen between missions shows, which the save keeps elsewhere.
        progress.set_flag(100_005, true);
        assert_eq!(progress.to_block(), block);
        assert_eq!(back.hub_stage, 0);
        assert!(Progress::from_block(&block[1..]).is_none());
    }

    #[test]
    fn one_variable_is_kept_below_ten_thousand() {
        let mut progress = Progress::empty();
        progress.set_variable(10_001, 123_456);
        assert_eq!(progress.variable(10_001), 9_999);
        progress.set_variable(10_001, 5);
        assert_eq!(progress.variable(10_001), 5);
        progress.set_variable(10_002, 123_456);
        assert_eq!(progress.variable(10_002), 123_456);
    }
}
