//! `EDXD` (the `.edx` file of an enemy's archive): figures the enemy's script asks the
//! engine for by number. Only that part of the file is understood (the reader at
//! 0xBDD80 of the executable); the rest is kept out of this.

use anyhow::{ensure, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"EDXD";

/// The number scripts give for the first figure; the others follow it.
pub const FIRST_ID: i32 = -1000;

/// Where each figure is, in the order of their numbers, and whether it is signed.
/// The sixth number has no figure.
const FIELDS: [Option<(usize, bool)>; 11] = [
    Some((0x04, false)),
    Some((0x10, false)),
    Some((0x12, false)),
    Some((0x14, false)),
    Some((0x16, false)),
    None,
    Some((0x2a, false)),
    Some((0x2e, true)),
    Some((0x30, true)),
    Some((0x32, true)),
    Some((0x36, false)),
];

/// Where the byte is that says how the enemy gets about, and what it is for one
/// that flies (read at 0xC0BB8).
const MOVEMENT_AT: usize = 0x18;
const FLIES: u8 = 1;
/// Which of the figures are the room the enemy takes up: its width and its depth.
const WIDTH: usize = 6;
const DEPTH: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Figures {
    figures: [i32; FIELDS.len()],
    movement: u8,
}

impl Figures {
    pub fn parse(d: &[u8]) -> Result<Figures> {
        ensure!(d.starts_with(MAGIC), "not an EDXD file");
        let r = Reader(d);
        let mut figures = [0; FIELDS.len()];
        for (figure, field) in figures.iter_mut().zip(FIELDS) {
            *figure = match field {
                Some((at, true)) => r.i16(at)? as i32,
                Some((at, false)) => r.u16(at)? as i32,
                None => 0,
            };
        }
        Ok(Figures { figures, movement: r.u8(MOVEMENT_AT)? })
    }

    /// The figure a script asks for as `id`; zero for a number there is none for, as
    /// the original answers.
    pub fn get(&self, id: i32) -> i32 {
        let index = id.checked_sub(FIRST_ID).and_then(|index| usize::try_from(index).ok());
        index.and_then(|index| self.figures.get(index)).copied().unwrap_or(0)
    }

    /// Whether the enemy flies: it then comes into a battle at a place in the air,
    /// and the others at one on the ground.
    pub fn flies(&self) -> bool {
        self.movement == FLIES
    }

    /// The room the enemy takes up: its width and its depth. A place it comes into
    /// the battle at has to have room for both.
    pub fn room(&self) -> (i32, i32) {
        (self.figures[WIDTH], self.figures[DEPTH])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_are_found_by_the_numbers_scripts_use() {
        let mut d = vec![0u8; 0x40];
        d[..4].copy_from_slice(MAGIC);
        d[0x10..0x12].copy_from_slice(&60u16.to_le_bytes());
        d[0x30..0x32].copy_from_slice(&(-100i16).to_le_bytes());
        d[0x36..0x38].copy_from_slice(&0xfff0u16.to_le_bytes());
        let figures = Figures::parse(&d).unwrap();
        assert_eq!(figures.get(-999), 60);
        assert_eq!(figures.get(-992), -100);
        assert_eq!(figures.get(-990), 0xfff0);
        assert_eq!(figures.get(-995), 0);
        assert_eq!(figures.get(-989), 0);
        assert_eq!(figures.get(5), 0);
    }

    #[test]
    fn the_room_an_enemy_takes_and_whether_it_flies_are_read_too() {
        // The Hornet's: 200 wide, 100 deep, and it flies.
        let mut d = vec![0u8; 0x40];
        d[..4].copy_from_slice(MAGIC);
        d[0x18] = 1;
        d[0x2a..0x2c].copy_from_slice(&200u16.to_le_bytes());
        d[0x36..0x38].copy_from_slice(&100u16.to_le_bytes());
        let figures = Figures::parse(&d).unwrap();
        assert!(figures.flies());
        assert_eq!(figures.room(), (200, 100));
        d[0x18] = 0;
        assert!(!Figures::parse(&d).unwrap().flies());
    }

    #[test]
    fn a_file_too_short_for_its_figures_is_an_error() {
        let mut d = vec![0u8; 0x20];
        d[..4].copy_from_slice(MAGIC);
        assert!(Figures::parse(&d).is_err());
    }
}
