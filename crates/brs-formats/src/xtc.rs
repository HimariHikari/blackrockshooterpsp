//! `XTC` tables: fixed-size records, followed by the strings they refer to. The files
//! of `SYSTEM.VOL/db_resident.lpk` are these (items, equipment, the level table, the
//! stats a game starts with), and so are the menus' texts. What the records of each
//! hold is in `docs/BATTLE_NOTES.md`.

use anyhow::{ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"XTC\0";

const SIZE_AT: usize = 0x04;
const COUNT_AT: usize = 0x08;
const RECORD_LEN_AT: usize = 0x0c;
const HEADER_LEN: usize = 0x10;

#[derive(Clone, Debug)]
pub struct Table {
    record_len: usize,
    count: usize,
    /// The whole file: a record refers to a string by its offset from the file's start.
    data: Vec<u8>,
}

impl Table {
    pub fn parse(d: &[u8]) -> Result<Table> {
        ensure!(d.starts_with(MAGIC), "not an XTC table");
        ensure!(d.len() >= HEADER_LEN, "file ends inside its header");
        let r = Reader(d);
        let (size, count) = (r.u32(SIZE_AT)? as usize, r.u32(COUNT_AT)? as usize);
        let record_len = r.u16(RECORD_LEN_AT)? as usize;
        ensure!(size == d.len(), "table says it is {size} bytes but the file is {}", d.len());
        ensure!(HEADER_LEN + record_len * count <= d.len(), "{count} records of {record_len} bytes do not fit the file");
        Ok(Table { record_len, count, data: d.to_vec() })
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The bytes of record `index`.
    pub fn record(&self, index: usize) -> Option<&[u8]> {
        let at = HEADER_LEN + index * self.record_len;
        (index < self.count).then(|| &self.data[at..at + self.record_len])
    }

    /// The text a string field of a record refers to.
    pub fn string(&self, offset: u32) -> Result<&str> {
        let tail = self.data.get(offset as usize..).context("string offset outside the file")?;
        let len = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        std::str::from_utf8(&tail[..len]).context("string is not UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_cut_by_the_length_in_the_header_and_strings_follow_them() {
        let mut d = Vec::new();
        d.extend(MAGIC);
        d.extend(28u32.to_le_bytes());
        d.extend(2u32.to_le_bytes());
        d.extend(3u32.to_le_bytes());
        d.extend([1, 2, 3, 4, 5, 6]);
        d.extend(b"name\0\0");
        let table = Table::parse(&d).unwrap();
        assert_eq!(table.len(), 2);
        assert_eq!(table.record(1), Some(&[4u8, 5, 6][..]));
        assert_eq!(table.record(2), None);
        assert_eq!(table.string(22).unwrap(), "name");

        // A size that does not match the file, and records that do not fit it.
        let mut longer = d.clone();
        longer.push(0);
        assert!(Table::parse(&longer).is_err());
        d[COUNT_AT] = 5;
        assert!(Table::parse(&d).is_err());
    }
}
