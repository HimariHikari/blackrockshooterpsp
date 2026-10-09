//! Bounds-checked little-endian reads at absolute offsets.

use anyhow::{Context, Result};

pub(crate) struct Reader<'a>(pub &'a [u8]);

impl Reader<'_> {
    pub fn bytes<const N: usize>(&self, off: usize) -> Result<[u8; N]> {
        let slice = off.checked_add(N).and_then(|end| self.0.get(off..end));
        Ok(slice.with_context(|| format!("read of {N} bytes at {off:#x} is out of range"))?.try_into().unwrap())
    }

    pub fn u8(&self, off: usize) -> Result<u8> {
        Ok(self.bytes::<1>(off)?[0])
    }

    pub fn u16(&self, off: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(off)?))
    }

    pub fn i16(&self, off: usize) -> Result<i16> {
        Ok(i16::from_le_bytes(self.bytes(off)?))
    }

    pub fn u32(&self, off: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(off)?))
    }

    pub fn f32(&self, off: usize) -> Result<f32> {
        Ok(f32::from_le_bytes(self.bytes(off)?))
    }

    pub fn f32s<const N: usize>(&self, off: usize) -> Result<[f32; N]> {
        let mut out = [0.0; N];
        for (i, v) in out.iter_mut().enumerate() {
            *v = self.f32(off + i * 4)?;
        }
        Ok(out)
    }
}
