//! Readers for the data formats used by the PSP game's `GAMEDATA` tree.
//!
//! Everything here was worked out by inspecting the files and the game's executable;
//! field names marked "unknown" are exactly that.

pub mod at3;
pub mod bscr;
pub mod bxcb;
pub mod cam;
pub mod chunks;
pub mod clim;
pub mod container;
pub mod cpmd;
pub mod edxd;
pub mod efc;
pub mod egtb;
pub mod esb;
pub mod font;
pub mod insa;
pub mod insm;
pub mod layout;
pub mod pphd;
pub mod psmf;
pub mod ptmd;
mod reader;
pub mod sbs;
pub mod script;
pub mod stcm;
pub mod xtc;

fn u32_at(d: &[u8], off: usize) -> Option<u32> {
    let b = d.get(off..off + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
