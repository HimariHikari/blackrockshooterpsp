//! The player character's model, which the field and battles share.

use bevy::prelude::*;
use brs_engine::model::Parts;

/// The player's field character bundle: script, model and field animations. The
/// original picks one by the costume worn; this is the default costume.
pub const FIELD_BUNDLE: &str = "FLD/FCHR/FCHR_P_BRS00.BIN";

/// The model, which battles use too.
pub fn model() -> String {
    model_of(FIELD_BUNDLE)
}

/// The model of a field character's bundle.
pub fn model_of(bundle: &str) -> String {
    format!("{bundle}/01.bin/00.insm")
}

/// The field character a suit has her be (0x14BE78), by the suit's item: the
/// five after the Normal Suit's, the Moon Rabbit's far down the list, and the
/// Normal Suit's for anything else.
pub fn suit_character(item: i32) -> i32 {
    match item {
        0x86..=0x89 => item - 0x85,
        0x8a => 0x98,
        _ => 0,
    }
}

/// The model of a weapon (0x14BF58), by the weapon's item: the four cannons.
pub fn weapon_model(item: i32) -> Option<String> {
    (0x81..=0x84).contains(&item).then(|| format!("BTL/WEP/ITM_EQU_ASSUALT_LV{}.ZZZ", item - 0x80))
}

/// The model's switchable parts, by the ids the original's code shows and hides them
/// with (its function at 0x9B608).
pub mod part {
    /// The cannon that is part of the model itself.
    pub const CANNON: u16 = 50;
    pub const BLADE: u16 = 51;
    /// The right hand in the shape that holds the cannon.
    pub const RIGHT_HAND_CANNON: u16 = 60;
    pub const RIGHT_HAND_OTHER: u16 = 61;
    pub const RIGHT_HAND_THIRD: u16 = 72;
    /// The left hand in the shape that holds the blade.
    pub const LEFT_HAND_BLADE: u16 = 70;
    pub const LEFT_HAND_EMPTY: u16 = 71;
}

/// On a model's root: which of its parts are shown. The rest are hidden.
#[derive(Component)]
pub struct ShownParts(pub Vec<u16>);

/// What she has in her hand after one of the original's part modes: her model's
/// second slot, where the cannon she wears is in a battle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holds {
    /// The mode has nothing to do with it.
    AsBefore,
    /// Nothing: the slot is emptied (0xA7788).
    Nothing,
    /// The cannon she wears, made anew (0x998A4).
    Cannon,
}

impl ShownParts {
    /// One of the original's part modes (0x9B608): it shows some parts and hides
    /// others, leaves the rest as they are, and says what it does to what she
    /// holds. Modes 0 to 2 are shapes of her right hand with nothing in it, 5 the
    /// first of those with the cannon in it; 3, 4 and 6 are shapes of her left
    /// hand, the last with the blade.
    pub fn set_mode(&mut self, mode: u8) -> Holds {
        use part::*;
        let right = |shown: u16| [RIGHT_HAND_CANNON, RIGHT_HAND_OTHER, RIGHT_HAND_THIRD].map(|part| (part, part == shown)).into_iter().chain([(CANNON, false)]);
        let left = |hand: u16, blade: bool| [(LEFT_HAND_BLADE, hand == LEFT_HAND_BLADE), (LEFT_HAND_EMPTY, hand == LEFT_HAND_EMPTY), (BLADE, blade)];
        let (changes, holds): (Vec<(u16, bool)>, Holds) = match mode {
            0 => (right(RIGHT_HAND_CANNON).collect(), Holds::Nothing),
            1 => (right(RIGHT_HAND_OTHER).collect(), Holds::Nothing),
            2 => (right(RIGHT_HAND_THIRD).collect(), Holds::Nothing),
            3 => (left(LEFT_HAND_BLADE, false).into(), Holds::AsBefore),
            4 => (left(LEFT_HAND_EMPTY, false).into(), Holds::AsBefore),
            5 => (right(RIGHT_HAND_CANNON).collect(), Holds::Cannon),
            6 => (left(LEFT_HAND_BLADE, true).into(), Holds::AsBefore),
            _ => (Vec::new(), Holds::AsBefore),
        };
        for (part, shown) in changes {
            self.0.retain(|&other| other != part);
            if shown {
                self.0.push(part);
            }
        }
        holds
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, show_parts);
}

fn show_parts(
    models: Query<(&Parts, &ShownParts), Or<(Changed<ShownParts>, Added<Parts>)>>,
    mut visibility: Query<&mut Visibility>,
) {
    for (parts, shown) in &models {
        parts.show_only(&shown.0, &mut visibility);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_mode_changes_one_hand_and_says_what_she_holds() {
        // As a battle has her: an empty left hand, and the right hand's shape for
        // the cannon, which is a model of its own.
        let mut shown = ShownParts(vec![part::LEFT_HAND_EMPTY, part::RIGHT_HAND_CANNON]);
        let sorted = |shown: &ShownParts| {
            let mut parts = shown.0.clone();
            parts.sort();
            parts
        };
        // For a skill's gun: the left hand closes, and the right hand lets the
        // cannon go.
        assert_eq!(shown.set_mode(3), Holds::AsBefore);
        assert_eq!(shown.set_mode(0), Holds::Nothing);
        assert_eq!(sorted(&shown), [part::RIGHT_HAND_CANNON, part::LEFT_HAND_BLADE]);
        // The cannon back.
        assert_eq!(shown.set_mode(5), Holds::Cannon);
        assert_eq!(sorted(&shown), [part::RIGHT_HAND_CANNON, part::LEFT_HAND_BLADE]);
        assert_eq!(shown.set_mode(4), Holds::AsBefore);
        assert_eq!(sorted(&shown), [part::RIGHT_HAND_CANNON, part::LEFT_HAND_EMPTY]);
        // The other shapes of the right hand, and the blade in the left.
        assert_eq!((shown.set_mode(1), sorted(&shown)), (Holds::Nothing, vec![part::RIGHT_HAND_OTHER, part::LEFT_HAND_EMPTY]));
        assert_eq!((shown.set_mode(2), sorted(&shown)), (Holds::Nothing, vec![part::LEFT_HAND_EMPTY, part::RIGHT_HAND_THIRD]));
        assert_eq!((shown.set_mode(6), sorted(&shown)), (Holds::AsBefore, vec![part::BLADE, part::LEFT_HAND_BLADE, part::RIGHT_HAND_THIRD]));
        // A mode there is not changes nothing.
        assert_eq!((shown.set_mode(7), shown.0.len()), (Holds::AsBefore, 3));
    }
}
