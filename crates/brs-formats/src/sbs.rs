//! Sprite tables: the `SBS` chunk of a sprite sheet, which says where in the sheet's
//! texture each sprite is.
//!
//! Read as the game does (0x2E15C). The table starts with a word and a count:
//!
//! | off | size | meaning                                                      |
//! |-----|------|--------------------------------------------------------------|
//! | 0   | 2    | how the four fields of a record are stored, three bits each   |
//! | 2   | 1    | number of sprites                                            |
//! | 3   | ...  | the records: x, y, width and height of each sprite            |
//!
//! A field is stored as one of [`Kind`]: the font's icons have all four as one byte.

use anyhow::{bail, ensure, Context, Result};

const KINDS_AT: usize = 0;
const COUNT_AT: usize = 2;
const RECORDS_AT: usize = 3;
const FIELDS: usize = 4;
const KIND_BITS: usize = 3;

/// A sprite's place in its texture, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sprite {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// What a field is stored as (the jump table at 0x166630): the kind's number halved
/// and one more is its length in bytes (0x2E010).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
}

impl Kind {
    fn of(bits: u16) -> Result<Kind> {
        Ok(match bits {
            0 => Kind::U8,
            1 => Kind::I8,
            2 => Kind::U16,
            3 => Kind::I16,
            7 => Kind::U32,
            // The game reads these as -1, whatever is stored.
            other => bail!("fields of kind {other} hold nothing"),
        })
    }

    fn len(self) -> usize {
        match self {
            Kind::U8 | Kind::I8 => 1,
            Kind::U16 | Kind::I16 => 2,
            Kind::U32 => 4,
        }
    }

    fn read(self, d: &[u8]) -> i32 {
        match self {
            Kind::U8 => d[0] as i32,
            Kind::I8 => d[0] as i8 as i32,
            Kind::U16 => u16::from_le_bytes([d[0], d[1]]) as i32,
            Kind::I16 => i16::from_le_bytes([d[0], d[1]]) as i32,
            Kind::U32 => i32::from_le_bytes([d[0], d[1], d[2], d[3]]),
        }
    }
}

/// The sprites of a table, in its order.
pub fn parse(d: &[u8]) -> Result<Vec<Sprite>> {
    ensure!(d.len() >= RECORDS_AT, "table ends inside its header");
    let word = u16::from_le_bytes([d[KINDS_AT], d[KINDS_AT + 1]]);
    let mut kinds = [Kind::U8; FIELDS];
    for (field, kind) in kinds.iter_mut().enumerate() {
        *kind = Kind::of((word >> (field * KIND_BITS)) & 7)?;
    }
    let record_len: usize = kinds.iter().map(|kind| kind.len()).sum();
    let count = d[COUNT_AT] as usize;
    let records = d.get(RECORDS_AT..RECORDS_AT + count * record_len).context("table ends inside its records")?;
    Ok(records
        .chunks_exact(record_len)
        .map(|record| {
            let mut at = 0;
            let mut fields = [0; FIELDS];
            for (field, kind) in fields.iter_mut().zip(kinds) {
                *field = kind.read(&record[at..]);
                at += kind.len();
            }
            Sprite { x: fields[0], y: fields[1], width: fields[2], height: fields[3] }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_read_as_the_tables_first_word_says() {
        // All four as a byte: what the font's icons have.
        let d = [0, 0, 2, 1, 17, 15, 15, 41, 129, 23, 15];
        let sprites = parse(&d).unwrap();
        assert_eq!(sprites, [Sprite { x: 1, y: 17, width: 15, height: 15 }, Sprite { x: 41, y: 129, width: 23, height: 15 }]);

        // x and y as signed words, width as a word, height as a byte.
        let word: u16 = 3 | (3 << 3) | (2 << 6);
        let mut d = word.to_le_bytes().to_vec();
        d.push(1);
        d.extend((-5i16).to_le_bytes());
        d.extend(300i16.to_le_bytes());
        d.extend(480u16.to_le_bytes());
        d.push(200);
        assert_eq!(parse(&d).unwrap(), [Sprite { x: -5, y: 300, width: 480, height: 200 }]);

        // Records that do not fit, and a kind that holds nothing.
        assert!(parse(&d[..d.len() - 1]).is_err());
        assert!(parse(&[4, 0, 0]).is_err());
    }
}
