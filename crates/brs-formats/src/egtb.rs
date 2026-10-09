//! `EGTB`: the table of enemy groups (`SYSTEM.VOL/enmgrp2.dat`). A battle is fought
//! against one group; the table says which enemies it has and where they stand, and
//! which can come in later, where, and by what rule.
//!
//! Read as the accessors at 0x93108-0x93CA8 of the executable read it;
//! `docs/BATTLE_NOTES.md` has the layout.

use anyhow::{ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"EGTB";

// Header fields.
const GROUP_COUNT_AT: usize = 0x0c;
const GROUPS_AT: usize = 0x10;
const WAVES_AT: usize = 0x18;
const WAVE_COUNT_AT: usize = 0x1c;
const MEMBERS_AT: usize = 0x20;
const MEMBER_COUNT_AT: usize = 0x24;
const HEADER_LEN: usize = 0x30;

const GROUP_LEN: usize = 0x14;
const WAVE_LEN: usize = 2;
const MEMBER_LEN: usize = 0x10;

/// The enemy number of a member that stands for nobody: a place kept for a wave.
pub const NO_ENEMY: u16 = 0x7f;

/// One enemy of a group, or one of the places its later enemies can appear at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    /// Which enemy, by the number its archive is found with; [`NO_ENEMY`] for none.
    pub enemy: u16,
    /// Bits of [`place`]: what a place is fit for.
    pub flags: u8,
    /// Which of the enemy's variants: the id of a record of its `param2` table.
    pub variant: u8,
    /// Where it stands on the stage.
    pub position: [f32; 3],
    /// How much room a place has: an enemy fits whose figures for width and depth
    /// are both at most twice this.
    pub size: u16,
}

/// Bits of [`Member::flags`].
pub mod place {
    /// The place is in the air: only enemies that fly appear at it, and they at no other.
    pub const AIR: u8 = 2;
}

/// An enemy that can come into the battle later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wave {
    pub enemy: u16,
    pub variant: u8,
    /// How it enters: 1 drops in, 2 rises out of the ground, else it is just there.
    pub entry: u8,
}

/// Bits of [`Group::flags`].
pub mod flag {
    /// The player strikes her victory pose when the battle is won (0x93154).
    pub const VICTORY_POSE: u16 = 2;
    /// The battle starts without the encounter's screen wipe (0x931AC).
    pub const NO_INTRO: u16 = 4;
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: u16,
    /// Bits of [`flag`].
    pub flags: u16,
    /// The rule its later enemies come in by (0x93108); zero for none.
    pub mode: u8,
    /// The enemies that are there when the battle starts.
    pub members: Vec<Member>,
    /// Those that can come in later, in the order they are called.
    pub waves: Vec<Wave>,
    /// The places they can appear at.
    pub places: Vec<Member>,
}

#[derive(Clone, Debug)]
pub struct Groups(Vec<Group>);

impl Groups {
    pub fn parse(d: &[u8]) -> Result<Groups> {
        ensure!(d.starts_with(MAGIC), "not an EGTB table");
        ensure!(d.len() >= HEADER_LEN, "file ends inside its header");
        let r = Reader(d);
        let field = |at: usize| -> Result<usize> { Ok(r.u32(at)? as usize) };
        let (group_count, groups_at) = (field(GROUP_COUNT_AT)?, field(GROUPS_AT)?);
        let (wave_count, waves_at) = (field(WAVE_COUNT_AT)?, field(WAVES_AT)?);
        let (member_count, members_at) = (field(MEMBER_COUNT_AT)?, field(MEMBERS_AT)?);
        ensure!(groups_at + group_count * GROUP_LEN <= d.len(), "the groups run past the end of the file");
        ensure!(waves_at + wave_count * WAVE_LEN <= d.len(), "the waves run past the end of the file");
        ensure!(members_at + member_count * MEMBER_LEN <= d.len(), "the members run past the end of the file");

        let members = |first: usize, count: usize| -> Result<Vec<Member>> {
            ensure!(first + count <= member_count, "members {first}..{} are not in the table", first + count);
            (first..first + count)
                .map(|index| {
                    let at = members_at + index * MEMBER_LEN;
                    Ok(Member {
                        enemy: r.u16(at)?,
                        flags: r.u8(at + 2)?,
                        variant: r.u8(at + 3)?,
                        position: [r.i16(at + 4)? as f32, r.i16(at + 6)? as f32, r.i16(at + 8)? as f32],
                        size: r.u16(at + 12)?,
                    })
                })
                .collect()
        };
        // A wave is one word: the enemy in its low seven bits, then six of variant
        // and three of how it enters (0x93270, 0x93334, 0x93404).
        let waves = |first: usize, count: usize| -> Result<Vec<Wave>> {
            ensure!(first + count <= wave_count, "waves {first}..{} are not in the table", first + count);
            (first..first + count)
                .map(|index| {
                    let word = r.u16(waves_at + index * WAVE_LEN)?;
                    Ok(Wave { enemy: word & 0x7f, variant: ((word >> 7) & 0x3f) as u8, entry: (word >> 13) as u8 })
                })
                .collect()
        };

        let mut groups = Vec::with_capacity(group_count);
        for index in 0..group_count {
            let at = groups_at + index * GROUP_LEN;
            let count = |offset: usize| -> Result<usize> { Ok(r.u8(at + offset)? as usize) };
            let first = |offset: usize| -> Result<usize> { Ok(r.u16(at + offset)? as usize) };
            groups.push(Group {
                id: r.u16(at)?,
                flags: r.u16(at + 2)?,
                mode: r.u8(at + 4)?,
                waves: waves(first(10)?, count(7)?).with_context(|| format!("group {index}"))?,
                members: members(first(12)?, count(8)?).with_context(|| format!("group {index}"))?,
                places: members(first(14)?, count(9)?).with_context(|| format!("group {index}"))?,
            });
        }
        Ok(Groups(groups))
    }

    /// The group with the given id.
    pub fn get(&self, id: u16) -> Option<&Group> {
        self.0.iter().find(|group| group.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Group> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (enemy, flags, variant, position, size) of a member or a place.
    type Row = (u16, u8, u8, [i16; 3], u16);

    /// A table of one group with the given members, places and waves.
    fn table(members: &[Row], places: &[Row], waves: &[u16]) -> Vec<u8> {
        let mut d = vec![0u8; HEADER_LEN];
        d[..4].copy_from_slice(MAGIC);
        let waves_at = HEADER_LEN + GROUP_LEN;
        let members_at = waves_at + waves.len() * WAVE_LEN;
        let mut field = |at: usize, value: usize| d[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        field(GROUP_COUNT_AT, 1);
        field(GROUPS_AT, HEADER_LEN);
        field(WAVES_AT, waves_at);
        field(WAVE_COUNT_AT, waves.len());
        field(MEMBERS_AT, members_at);
        field(MEMBER_COUNT_AT, members.len() + places.len());

        let mut group = [0u8; GROUP_LEN];
        group[..2].copy_from_slice(&7u16.to_le_bytes());
        group[2] = 3;
        group[4] = 5;
        group[7] = waves.len() as u8;
        group[8] = members.len() as u8;
        group[9] = places.len() as u8;
        group[14..16].copy_from_slice(&(members.len() as u16).to_le_bytes());
        d.extend(group);
        for wave in waves {
            d.extend(wave.to_le_bytes());
        }
        for (enemy, flags, variant, position, size) in members.iter().chain(places) {
            let mut member = [0u8; MEMBER_LEN];
            member[..2].copy_from_slice(&enemy.to_le_bytes());
            member[2] = *flags;
            member[3] = *variant;
            for (axis, value) in position.iter().enumerate() {
                member[4 + axis * 2..6 + axis * 2].copy_from_slice(&value.to_le_bytes());
            }
            member[12..14].copy_from_slice(&size.to_le_bytes());
            d.extend(member);
        }
        d
    }

    #[test]
    fn a_group_lists_its_members_and_where_later_ones_appear() {
        let members = [(8, 0, 10, [600, 0, 0], 100), (19, 0, 10, [0, 300, -5], 100)];
        let places = [(NO_ENEMY, 3, 0, [200, 300, -600], 200)];
        let d = table(&members, &places, &[]);
        let groups = Groups::parse(&d).unwrap();
        let group = groups.get(7).unwrap();
        assert!(group.flags & flag::VICTORY_POSE != 0 && group.flags & flag::NO_INTRO == 0);
        assert_eq!(group.mode, 5);
        assert_eq!(group.members.len(), 2);
        assert_eq!(group.members[1], Member { enemy: 19, flags: 0, variant: 10, position: [0.0, 300.0, -5.0], size: 100 });
        assert_eq!(group.places.len(), 1);
        assert_eq!(group.places[0].enemy, NO_ENEMY);
        assert_eq!(group.places[0].size, 200);
        assert!(group.places[0].flags & place::AIR != 0);
        assert!(group.waves.is_empty());
        assert!(groups.get(8).is_none());
    }

    #[test]
    fn a_wave_is_an_enemy_a_variant_and_a_way_in_packed_into_a_word() {
        // Enemy 14, variant 20, dropping in; enemy 2, variant 0, rising.
        let words = [14 | 20 << 7 | 1 << 13, 2 | 2 << 13];
        let d = table(&[(92, 0, 0, [300, 0, 0], 100)], &[], &words);
        let group = &Groups::parse(&d).unwrap().0[0];
        assert_eq!(group.waves, vec![Wave { enemy: 14, variant: 20, entry: 1 }, Wave { enemy: 2, variant: 0, entry: 2 }]);
    }

    #[test]
    fn members_and_waves_outside_the_table_are_an_error() {
        let mut d = table(&[(8, 0, 10, [0, 0, 0], 100)], &[], &[]);
        d[HEADER_LEN + 8] = 2;
        assert!(Groups::parse(&d).is_err());
        let mut d = table(&[(8, 0, 10, [0, 0, 0], 100)], &[], &[5]);
        d[HEADER_LEN + 7] = 2;
        assert!(Groups::parse(&d).is_err());
    }
}
