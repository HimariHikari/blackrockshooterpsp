//! `BXCB` tables: fixed-size records of 32-bit fields, plus a pool of strings.
//!
//! The battle system's data lives in these: its tunables, attacks, bullets, status
//! effects, messages and skills, and each enemy's variants. What the fields of each
//! table mean is in `docs/BATTLE_NOTES.md`.

use anyhow::{ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 4] = b"BXCB";

const HEADER_LEN: usize = 0x20;
const FIELD_LEN: usize = 4;

#[derive(Clone, Debug)]
pub struct Table {
    /// Number of 32-bit fields in each record.
    pub fields: usize,
    /// Every record's fields, back to back.
    values: Vec<u32>,
    pool: Vec<u8>,
}

impl Table {
    pub fn parse(d: &[u8]) -> Result<Table> {
        ensure!(d.starts_with(MAGIC), "not a BXCB table");
        let r = Reader(d);
        let size = r.u32(0x04)? as usize;
        let records_at = r.u32(0x0c)? as usize;
        let record_len = r.u32(0x10)? as usize;
        let count = r.u32(0x14)? as usize;
        let pool_at = r.u32(0x18)? as usize;
        ensure!(size == d.len(), "table says it is {size} bytes but the file is {}", d.len());
        ensure!(records_at == HEADER_LEN, "records start at {records_at:#x}, not after the header");
        ensure!(record_len % FIELD_LEN == 0, "record length {record_len:#x} is not a whole number of fields");
        ensure!(
            records_at + record_len * count == pool_at && pool_at <= d.len(),
            "{count} records of {record_len:#x} bytes do not end where the string pool starts"
        );

        let fields = record_len / FIELD_LEN;
        let mut values = Vec::with_capacity(fields * count);
        for i in 0..fields * count {
            values.push(r.u32(records_at + i * FIELD_LEN)?);
        }
        Ok(Table { fields, values, pool: d[pool_at..].to_vec() })
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        if self.fields == 0 { 0 } else { self.values.len() / self.fields }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The fields of record `index`.
    pub fn record(&self, index: usize) -> &[u32] {
        &self.values[index * self.fields..(index + 1) * self.fields]
    }

    pub fn records(&self) -> impl Iterator<Item = &[u32]> {
        self.values.chunks_exact(self.fields.max(1))
    }

    /// The text a string field refers to. Offset 0 is the empty string.
    pub fn string(&self, offset: u32) -> Result<&str> {
        let tail = self.pool.get(offset as usize..).context("string offset outside the pool")?;
        let len = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        std::str::from_utf8(&tail[..len]).context("string in the pool is not UTF-8")
    }
}
