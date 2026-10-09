//! Items: the tables every item number runs through, and what she carries of them
//! (the original's functions at 0x12BED8 to 0x12C9C8 and 0x14D594; see
//! `docs/BATTLE_NOTES.md`).

use std::collections::BTreeMap;

use anyhow::{ensure, Context, Result};
use bevy::prelude::*;
use brs_formats::xtc;

use crate::data::GameData;

const DATABASE: &str = "SYSTEM.VOL/db_resident.lpk";

/// What kind of thing an item is: the table it is in. Item numbers start at 1 and
/// run through the three tables one after the other (0x12C9C8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Class {
    /// Used up, in a battle or from the menu.
    #[default]
    Consumable,
    /// A weapon or a suit.
    Equipment,
    /// Kept for the story's sake.
    Valuable,
}

/// A table of items: its file, the word of a record that has its flags, and the word
/// whose low seven bits are how many of the item can be carried (none: only one).
const TABLES: [(Class, &str, usize, Option<usize>); 3] = [
    (Class::Consumable, "000.xtc", 0x0c, Some(0x10)),
    (Class::Equipment, "001.xtc", 0x08, None),
    (Class::Valuable, "002.xtc", 0x08, Some(0x0c)),
];
/// The flag of an item that is in the game: the tables are padded out with records
/// that are not (0x12C24C).
const FLAG_REAL: u32 = 0x1_0000;
const LIMIT_BITS: u32 = 0x7f;
/// The five bits of the flags that say what kind of thing it is (0x12C3A4), by
/// which it has its icon.
const KIND_SHIFT: u32 = 21;
const KIND_BITS: u32 = 0x1f;
/// The low half of the flags is the item's place in the order the menu lists
/// items in (0x12C1C4).
const ORDER_BITS: u32 = 0xffff;
// Of a consumable (0x12C520, 0x12C640, 0x12C698): the three bits over its limit
// say where it can be used, the half above them is how much it gives back, and
// the byte after that word how many hundredths of all there is besides.
const USE_SHIFT: u32 = 7;
const USE_BITS: u32 = 7;
const GIVES_SHIFT: u32 = 16;
const SHARE_AT: usize = 0x14;
/// The status effect a consumable gives her in a battle (0x12C6F0).
const EFFECT_AT: usize = 0x18;
/// Where the three figures of a piece of equipment are (0x12C748 and the two
/// after it).
const FIGURES_AT: usize = 0x0c;

/// The kinds of thing the game's tables have (0x12C3A4): what gives HP back,
/// what is for battles, a weapon, a suit, and what is kept for the story's sake.
pub mod kind {
    pub const REPAIR: u8 = 1;
    pub const WEAPON: u8 = 7;
    pub const SUIT: u8 = 10;
}

/// Where a consumable can be used (0x12C520): this is in battles only; the
/// others the tables have can be used from the menu as well.
pub const USE_IN_BATTLES: u8 = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    /// What the menu says of it (0x12C110).
    pub description: String,
    pub class: Class,
    /// It is in the game, and can be had.
    pub real: bool,
    /// The most of it she can carry.
    pub limit: u8,
    /// What kind of thing it is, which its icon goes by.
    pub kind: u8,
    /// Its place in the order the menu lists items in.
    pub order: u16,
    /// Where a consumable can be used.
    pub usage: u8,
    /// What a consumable gives back: so much, and so many hundredths of all
    /// there is besides.
    pub gives: (i32, i32),
    /// The status effect a consumable gives her when it is used in a battle and
    /// gives no HP back, or nothing.
    pub effect: i32,
    /// What a piece of equipment has: what a weapon adds to her attack, a heat
    /// figure (for a suit, the heat a dodge costs), and the bullets of a burst.
    pub figures: [i32; 3],
}

impl Item {
    /// Whether it is one of the consumables that can be used from the menu
    /// (0x54850): those that can be used anywhere.
    pub fn used_from_the_menu(&self) -> bool {
        self.class == Class::Consumable && matches!(self.usage, 1 | 3)
    }
}

/// Every item there is, by number.
#[derive(Resource, Clone, Debug, Default)]
pub struct Items(Vec<Item>);

impl Items {
    pub fn load(data: &GameData) -> Result<Items> {
        let mut items = Vec::new();
        for (class, file, flags_at, limit_at) in TABLES {
            let path = format!("{DATABASE}/{file}");
            let table = xtc::Table::parse(&data.read(&path)?).with_context(|| format!("parsing {path}"))?;
            for index in 0..table.len() {
                let record = table.record(index).context("a record within the table's count")?;
                ensure!(record.len() > flags_at.max(limit_at.unwrap_or(0)) + 3, "{path}: records are {} bytes", record.len());
                let word = |at: usize| u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]);
                let consumable = limit_at.filter(|_| class == Class::Consumable && record.len() > SHARE_AT).map(word);
                let equipment = class == Class::Equipment && record.len() >= FIGURES_AT + 12;
                items.push(Item {
                    name: table.string(word(0)).unwrap_or_default().to_owned(),
                    description: table.string(word(4)).unwrap_or_default().to_owned(),
                    class,
                    real: word(flags_at) & FLAG_REAL != 0,
                    limit: limit_at.map_or(1, |at| (word(at) & LIMIT_BITS) as u8),
                    kind: ((word(flags_at) >> KIND_SHIFT) & KIND_BITS) as u8,
                    order: (word(flags_at) & ORDER_BITS) as u16,
                    usage: consumable.map_or(0, |limits| ((limits >> USE_SHIFT) & USE_BITS) as u8),
                    gives: consumable.map_or((0, 0), |limits| (i32::from((limits >> GIVES_SHIFT) as i16), i32::from(record[SHARE_AT] as i8))),
                    effect: consumable.filter(|_| record.len() >= EFFECT_AT + 4).map_or(0, |_| word(EFFECT_AT) as i32),
                    figures: if equipment { [0, 4, 8].map(|at| word(FIGURES_AT + at) as i32) } else { [0; 3] },
                });
            }
        }
        Ok(Items(items))
    }

    #[cfg(test)]
    pub fn from_list(items: Vec<Item>) -> Items {
        Items(items)
    }

    pub fn get(&self, number: i32) -> Option<&Item> {
        self.0.get(usize::try_from(number.checked_sub(1)?).ok()?)
    }

    /// The item's name, for telling of it.
    pub fn name(&self, number: i32) -> &str {
        self.get(number).map_or("an item that does not exist", |item| &item.name)
    }

    /// The items of a class that she has any of, as a list of the menu has them
    /// (0x533AC): those that are in the game, in the tables' order for them.
    pub fn carried(&self, class: Class, inventory: &Inventory) -> Vec<i32> {
        let of_the_list = |(index, item): (usize, &Item)| {
            let number = index as i32 + 1;
            (item.class == class && item.real && inventory.held(number) > 0).then_some((item.order, number))
        };
        let mut list: Vec<(u16, i32)> = self.0.iter().enumerate().filter_map(of_the_list).collect();
        list.sort_by_key(|&(order, _)| order);
        list.into_iter().map(|(_, number)| number).collect()
    }

    /// The items she can use in a battle, as its menu lists them (0x953AC): the
    /// consumables that are for battles only or for anywhere, of which she has
    /// any, in the tables' order for them.
    pub fn for_battles(&self, inventory: &Inventory) -> Vec<i32> {
        let mut list = self.carried(Class::Consumable, inventory);
        list.retain(|&number| self.get(number).is_some_and(|item| matches!(item.usage, 2 | 3)));
        list
    }
}

/// What she carries: how many of each item, a count of seven bits each in the save
/// data (0x1BA4).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory(BTreeMap<i32, u8>);

impl Inventory {
    /// What a new game starts her with (0x944, 0x950): the Normal Suit and the Rock
    /// Cannon.
    const NEW_GAME: [i32; 2] = [0x85, 0x81];

    pub fn new_game(items: &Items) -> Inventory {
        let mut inventory = Inventory::default();
        for item in Self::NEW_GAME {
            inventory.add(items, item, 1);
        }
        inventory
    }

    pub fn held(&self, number: i32) -> u8 {
        self.0.get(&number).copied().unwrap_or(0)
    }

    /// Gives her `count` of an item, or takes them with a count below zero (0x14D594):
    /// she has no fewer than none and no more than the item's limit, and nothing
    /// comes of an item that is not in the game. Returns how many she has then.
    pub fn add(&mut self, items: &Items, number: i32, count: i32) -> u8 {
        let Some(item) = items.get(number).filter(|item| item.real) else { return 0 };
        let held = (self.held(number) as i32 + count).clamp(0, item.limit as i32) as u8;
        if held == 0 {
            self.0.remove(&number);
        } else {
            self.0.insert(number, held);
        }
        held
    }

    /// The items she has any of, with how many.
    #[cfg(test)]
    pub fn all(&self) -> impl Iterator<Item = (i32, u8)> + '_ {
        self.0.iter().map(|(&number, &count)| (number, count))
    }

    /// What she carries as the save's block has it (the block's + 0xBC0,
    /// 0x246C): a byte for each item by its number, the count in its low seven
    /// bits (0x1BA4).
    pub fn to_block(&self) -> Vec<u8> {
        let mut block = vec![0; Self::BLOCK];
        for (&number, &count) in &self.0 {
            if let Some(byte) = usize::try_from(number).ok().and_then(|number| block.get_mut(number)) {
                *byte = count & Self::COUNT;
            }
        }
        block
    }

    /// What a block has her carry (0x1BF4): its counts as they are.
    pub fn from_block(block: &[u8]) -> Inventory {
        let held = block.iter().take(Self::BLOCK).enumerate().map(|(number, &byte)| (number as i32, byte & Self::COUNT));
        Inventory(held.filter(|&(_, count)| count != 0).collect())
    }

    pub const BLOCK: usize = 0x400;
    const COUNT: u8 = 0x7f;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Items {
        let item = |name: &str, class, real, limit| Item { name: name.into(), class, real, limit, ..Item::default() };
        Items::from_list(vec![
            item("Repairer 200", Class::Consumable, true, 5),
            item("Item 09", Class::Consumable, false, 0),
            item("Rock Cannon", Class::Equipment, true, 1),
        ])
    }

    #[test]
    fn item_numbers_start_at_one() {
        let items = items();
        assert!(items.get(0).is_none() && items.get(-4).is_none() && items.get(4).is_none());
        assert_eq!(items.name(1), "Repairer 200");
        assert_eq!(items.get(3).map(|item| item.class), Some(Class::Equipment));
    }

    #[test]
    fn she_carries_no_more_of_an_item_than_its_limit() {
        let (items, mut inventory) = (items(), Inventory::default());
        assert_eq!(inventory.add(&items, 1, 2), 2);
        assert_eq!(inventory.add(&items, 1, 2), 4);
        assert_eq!(inventory.add(&items, 1, 2), 5);
        assert_eq!(inventory.held(1), 5);
        // There is only ever one of a piece of equipment.
        assert_eq!(inventory.add(&items, 3, 3), 1);
        // Taking them away stops at none.
        assert_eq!(inventory.add(&items, 1, -7), 0);
        assert_eq!(inventory.all().collect::<Vec<_>>(), vec![(3, 1)]);
    }

    #[test]
    fn the_games_items_run_through_three_tables() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let items = Items::load(&data).expect("the item tables load");
        assert_eq!(items.0.len(), 384);
        let real = |class: Class| items.0.iter().filter(|item| item.class == class && item.real).count();
        assert_eq!((real(Class::Consumable), real(Class::Equipment), real(Class::Valuable)), (21, 10, 19));
        let limit = |number: i32| items.get(number).unwrap().limit;
        assert_eq!((items.name(1), limit(1)), ("Repairer 200", 5));
        assert_eq!((items.name(129), limit(129)), ("Rock Cannon", 1));
        assert_eq!((items.name(257), limit(257)), ("Gum", 10));
        // The tables are padded out with items that are not in the game.
        assert!(!items.get(8).unwrap().real && !items.get(139).unwrap().real);
        // What the menu goes by: what is said of an item, what one that gives HP
        // back gives, where one can be used, and a weapon's and a suit's figures.
        let item = |number: i32| items.get(number).unwrap();
        assert_eq!((item(1).description.as_str(), item(1).kind, item(1).gives, item(1).usage), ("Heals 200 HP.\nD'dario Next quality.", kind::REPAIR, (200, 0), 3));
        assert_eq!((item(6).name.as_str(), item(6).gives, item(7).gives), ("Nano Repair 50%", (0, 50), (0, 100)));
        assert_eq!((item(14).name.as_str(), item(14).kind, item(14).usage), ("Body Cooler", 2, USE_IN_BATTLES));
        // What those for battles give her: a status effect each.
        assert_eq!((item(14).effect, item(13).effect, item(20).effect, item(1).effect), (400, 300, 1501, 0));
        assert!(item(1).used_from_the_menu() && !item(14).used_from_the_menu() && !item(129).used_from_the_menu());
        assert_eq!((item(129).kind, item(129).figures, item(132).name.as_str(), item(132).figures), (kind::WEAPON, [15, 10, 6], "Hyperion", [60, 8, 8]));
        assert_eq!((item(133).kind, item(133).figures, item(134).name.as_str(), item(134).figures), (kind::SUIT, [15, 15, 0], "Lost All", [0, 10, 0]));

        // A new game has what she wears and carries, and nothing else.
        let inventory = Inventory::new_game(&items);
        let held: Vec<(&str, u8)> = inventory.all().map(|(item, count)| (items.name(item), count)).collect();
        assert_eq!(held, [("Rock Cannon", 1), ("Normal Suit", 1)]);
        // A list of the menu is what she has of a class, in the tables' order:
        // Rest Up is written after the Boosts and listed before Seal Break.
        let mut inventory = inventory;
        for number in [22, 18, 19, 2, 1, 130, 257, 8] {
            inventory.add(&items, number, 1);
        }
        assert_eq!(items.carried(Class::Consumable, &inventory), [1, 2, 22, 18, 19]);
        assert_eq!(items.carried(Class::Equipment, &inventory), [129, 130, 133]);
        assert_eq!(items.carried(Class::Valuable, &inventory), [257]);
    }

    #[test]
    fn an_item_that_is_not_in_the_game_cannot_be_had() {
        let (items, mut inventory) = (items(), Inventory::default());
        assert_eq!(inventory.add(&items, 2, 1), 0);
        assert_eq!(inventory.add(&items, 9, 1), 0);
        assert_eq!(inventory, Inventory::default());
    }
}
