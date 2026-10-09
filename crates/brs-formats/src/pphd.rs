//! Sound banks: the short sounds of the game (steps, hits, the interface, a scene's
//! noises), each bank a header (`PPHD8`: `.phd`, or `.pphd` in a field bundle) and
//! the data it points into (`.pbd`, or the slot after the header's).
//!
//! The header has three chunks: `PPPG`, programs, which the game does not use;
//! `PPTN`, the tones, which are what a sound's number names; and `PPVA`, the
//! samples a tone plays. A tone has room for a volume, a place between left and
//! right, a note and an envelope, but all 779 tones of the game have the same: the
//! sample as it is, at its own rate. So a sound here is its sample.
//!
//! A sample is ADPCM as the PlayStation has always had it: frames of 16 bytes, a
//! byte of filter and shift, a byte of flags, and 28 samples of four bits. The
//! flags say where the sample ends and where it loops. See `docs/FORMAT_NOTES.md`.

use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};

use crate::reader::Reader;

pub const MAGIC: &[u8; 5] = b"PPHD8";

// Where the header has the offsets of its chunks.
const TONES_AT: usize = 0x14;
const SAMPLES_AT: usize = 0x18;
const TONES: &[u8; 4] = b"PPTN";
const SAMPLES: &[u8; 4] = b"PPVA";
// A chunk: its name, its size, the length of a record, and at + 0x10 and + 0x14
// the numbers of its first and its last record, which follow from + 0x20.
const CHUNK_RECORD_LEN_AT: usize = 0x08;
const CHUNK_FIRST_AT: usize = 0x10;
const CHUNK_LAST_AT: usize = 0x14;
const CHUNK_RECORDS_AT: usize = 0x20;
const TONE_LEN: usize = 0x60;
const SAMPLE_LEN: usize = 0x10;
/// What a word that is not there holds.
const NONE: u32 = 0xffff_ffff;

/// A frame of a sample, and the samples in it.
const FRAME_LEN: usize = 16;
const FRAME_SAMPLES: usize = 28;
// A frame's flags: the sample ends with it; it is of the part that loops (with the
// end: go back rather than stop); the part that loops starts with it.
const FLAG_END: u8 = 1;
const FLAG_LOOP: u8 = 2;
const FLAG_LOOP_START: u8 = 4;

/// The filters a frame chooses from: what of the last sample and of the one before
/// goes into the next, in 64ths.
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

/// Where a sound's sample is in the bank's data, and the rate it is played at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub offset: usize,
    pub len: usize,
    /// Samples a second.
    pub rate: u32,
}

/// A bank's header: the sounds it has, by number.
#[derive(Clone, Debug)]
pub struct Bank {
    /// The number of the first tone.
    first: u32,
    tones: Vec<Option<Tone>>,
}

/// A sound, decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sound {
    pub samples: Vec<i16>,
    pub rate: u32,
    /// The stretch it repeats for ever, if it loops: its first sample and the one
    /// after its last.
    pub looped: Option<(usize, usize)>,
}

impl Bank {
    pub fn parse(d: &[u8]) -> Result<Bank> {
        ensure!(d.starts_with(MAGIC), "not a sound bank");
        let r = Reader(d);
        let chunk = |at: usize, name: &[u8; 4], record_len: usize| -> Result<(usize, u32, usize)> {
            let at = r.u32(at)? as usize;
            ensure!(d.get(at..at + 4) == Some(&name[..]), "no {} chunk", String::from_utf8_lossy(name));
            ensure!(r.u32(at + CHUNK_RECORD_LEN_AT)? as usize == record_len, "records of another length");
            let (first, last) = (r.u32(at + CHUNK_FIRST_AT)?, r.u32(at + CHUNK_LAST_AT)?);
            ensure!(first <= last, "records {first} to {last}");
            let count = (last - first + 1) as usize;
            ensure!(at + CHUNK_RECORDS_AT + count * record_len <= d.len(), "the records run past the file");
            Ok((at + CHUNK_RECORDS_AT, first, count))
        };
        let (tones_at, first, tone_count) = chunk(TONES_AT, TONES, TONE_LEN)?;
        let (samples_at, first_sample, sample_count) = chunk(SAMPLES_AT, SAMPLES, SAMPLE_LEN)?;

        let mut tones = Vec::with_capacity(tone_count);
        for index in 0..tone_count {
            // A tone's first word is the number of its sample (0x15ED8C).
            let sample = r.u32(tones_at + index * TONE_LEN)?;
            let record = sample.checked_sub(first_sample).filter(|&sample| sample != NONE && (sample as usize) < sample_count);
            let tone = match record {
                Some(record) => {
                    let at = samples_at + record as usize * SAMPLE_LEN;
                    let (offset, rate, len) = (r.u32(at)?, r.u32(at + 4)?, r.u32(at + 8)?);
                    (offset != NONE && rate > 0).then_some(Tone { offset: offset as usize, len: len as usize, rate })
                }
                None => None,
            };
            tones.push(tone);
        }
        Ok(Bank { first, tones })
    }

    /// The tone of sound `number` (0x15F054), if the bank has it.
    pub fn tone(&self, number: u32) -> Option<Tone> {
        self.tones.get(number.checked_sub(self.first)? as usize).copied().flatten()
    }

    /// The numbers of the sounds the bank has.
    pub fn sounds(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.tones.len()).filter(|&index| self.tones[index].is_some()).map(|index| self.first + index as u32)
    }

    /// Decodes sound `number` out of the bank's data.
    pub fn decode(&self, number: u32, data: &[u8]) -> Result<Sound> {
        let tone = self.tone(number).with_context(|| format!("the bank has no sound {number}"))?;
        let frames = data.get(tone.offset..tone.offset + tone.len).context("the sound's sample runs past the bank's data")?;
        Ok(decode(frames, tone.rate))
    }
}

/// The file of the data that goes with a bank's header: the `.pbd` of the same name,
/// or for the header in a slot of a field bundle (`00.pphd`) the slot after it
/// (`01.bin`).
pub fn data_file(header: &Path) -> Option<PathBuf> {
    if header.extension()? == "phd" {
        return Some(header.with_extension("pbd"));
    }
    let slot: u32 = header.file_stem()?.to_str()?.parse().ok()?;
    Some(header.with_file_name(format!("{:02}.bin", slot + 1)))
}

/// Decodes a sample: its frames up to the one that ends it.
pub fn decode(frames: &[u8], rate: u32) -> Sound {
    let mut samples = Vec::with_capacity(frames.len() / FRAME_LEN * FRAME_SAMPLES);
    let (mut last, mut before) = (0i32, 0i32);
    let (mut loop_start, mut looped) = (None, None);
    for frame in frames.chunks_exact(FRAME_LEN) {
        let (filter, shift, flags) = (frame[0] >> 4, frame[0] & 0x0f, frame[1]);
        // The frame after a sample that does not loop marks its end and holds
        // nothing: it asks to loop from itself.
        if flags == FLAG_END | FLAG_LOOP | FLAG_LOOP_START {
            break;
        }
        if flags & FLAG_LOOP_START != 0 {
            loop_start = Some(samples.len());
        }
        let (of_last, of_before) = FILTERS[(filter as usize).min(FILTERS.len() - 1)];
        // A shift of more than twelve leaves nothing of four bits: the hardware
        // takes it for nine.
        let shift = if shift > 12 { 9 } else { shift };
        for nibble in frame[2..].iter().flat_map(|&byte| [byte & 0x0f, byte >> 4]) {
            // Four bits, signed, at the top of sixteen, then down by the shift.
            let delta = i32::from((u16::from(nibble) << 12) as i16) >> shift;
            let carried = (last * of_last + before * of_before) >> 6;
            let sample = (delta + carried).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
            (before, last) = (last, sample);
            samples.push(sample as i16);
        }
        if flags & FLAG_END != 0 {
            if flags & FLAG_LOOP != 0 {
                looped = Some((loop_start.unwrap_or(0), samples.len()));
            }
            break;
        }
    }
    Sound { samples, rate, looped }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bank's header with a tone for each sample given (none for `None`), the
    /// tones numbered from `first`.
    fn header(first: u32, samples: &[Option<(u32, u32, u32)>]) -> Vec<u8> {
        let mut d = vec![0xffu8; 0x40];
        d[..8].copy_from_slice(b"PPHD8\0\0\0");
        let chunk = |name: &[u8; 4], record_len: usize, first: u32, records: Vec<Vec<u8>>| {
            let mut c = vec![0xffu8; CHUNK_RECORDS_AT];
            c[..4].copy_from_slice(name);
            c[CHUNK_RECORD_LEN_AT..CHUNK_RECORD_LEN_AT + 4].copy_from_slice(&(record_len as u32).to_le_bytes());
            c[CHUNK_FIRST_AT..CHUNK_FIRST_AT + 4].copy_from_slice(&first.to_le_bytes());
            c[CHUNK_LAST_AT..CHUNK_LAST_AT + 4].copy_from_slice(&(first + records.len() as u32 - 1).to_le_bytes());
            c.extend(records.into_iter().flatten());
            c
        };
        let tones: Vec<Vec<u8>> = (0..samples.len())
            .map(|index| {
                let mut tone = vec![0xffu8; TONE_LEN];
                if samples[index].is_some() {
                    tone[..4].copy_from_slice(&(index as u32).to_le_bytes());
                }
                tone
            })
            .collect();
        let records: Vec<Vec<u8>> = samples
            .iter()
            .map(|sample| {
                let (offset, rate, len) = sample.unwrap_or((NONE, NONE, NONE));
                [offset, rate, len, NONE].iter().flat_map(|word| word.to_le_bytes()).collect()
            })
            .collect();
        let tones = chunk(TONES, TONE_LEN, first, tones);
        let records = chunk(SAMPLES, SAMPLE_LEN, 0, records);
        d[TONES_AT..TONES_AT + 4].copy_from_slice(&0x40u32.to_le_bytes());
        d[SAMPLES_AT..SAMPLES_AT + 4].copy_from_slice(&(0x40 + tones.len() as u32).to_le_bytes());
        d.extend(tones);
        d.extend(records);
        d
    }

    /// A frame of samples all of the nibble `value`, with a filter, a shift and flags.
    fn frame(filter: u8, shift: u8, flags: u8, value: u8) -> [u8; 16] {
        let mut frame = [value | value << 4; 16];
        frame[0] = filter << 4 | shift;
        frame[1] = flags;
        frame
    }

    #[test]
    fn a_sounds_number_names_a_tone_and_the_tone_a_sample() {
        let d = header(2, &[Some((0, 22_050, 0x40)), None, Some((0x40, 44_100, 0x20))]);
        let bank = Bank::parse(&d).unwrap();
        assert_eq!(bank.tone(2), Some(Tone { offset: 0, len: 0x40, rate: 22_050 }));
        assert_eq!(bank.tone(4), Some(Tone { offset: 0x40, len: 0x20, rate: 44_100 }));
        // The tone between has no sample, and the numbers start at two.
        assert_eq!((bank.tone(3), bank.tone(0), bank.tone(5)), (None, None, None));
        assert_eq!(bank.sounds().collect::<Vec<u32>>(), [2, 4]);
        assert!(Bank::parse(b"PPHD7...").is_err());
        assert!(bank.decode(2, &[0; 0x20]).is_err());
    }

    #[test]
    fn a_banks_data_is_the_file_beside_its_header() {
        assert_eq!(data_file(Path::new("BTL/CHR/X.VOL/se_x.phd")), Some(PathBuf::from("BTL/CHR/X.VOL/se_x.pbd")));
        assert_eq!(data_file(Path::new("FLD/MAP/M.BIN/06.bin/00.pphd")), Some(PathBuf::from("FLD/MAP/M.BIN/06.bin/01.bin")));
        assert_eq!(data_file(Path::new("FLD/MAP/M.BIN/06.bin/bank.pphd")), None);
    }

    #[test]
    fn a_sample_is_decoded_frame_by_frame_up_to_its_end() {
        // With no filter a sample is its four bits, signed, at the top of sixteen
        // and down by the shift: 7 is 0x7000, and 0xF, minus one, is -0x1000.
        let data = [frame(0, 0, 0, 7), frame(0, 4, FLAG_END, 0xf), frame(0, 0, 7, 0), frame(0, 0, 0, 7)].concat();
        let sound = decode(&data, 22_050);
        assert_eq!(sound.samples.len(), 56);
        assert_eq!((sound.samples[0], sound.samples[27], sound.samples[28]), (0x7000, 0x7000, -0x100));
        assert_eq!((sound.rate, sound.looped), (22_050, None));

        // The first filter adds 60/64 of the sample before: it climbs to the top.
        let sound = decode(&frame(1, 0, FLAG_END, 1), 44_100);
        assert_eq!(sound.samples[..3], [0x1000, 0x1000 + 0xf00, 0x1000 + 7440]);
        assert_eq!(*sound.samples.last().unwrap(), i16::MAX);
    }

    #[test]
    fn a_sample_that_loops_says_from_where() {
        let data = [
            frame(0, 0, 0, 1),
            frame(0, 0, FLAG_LOOP | FLAG_LOOP_START, 2),
            frame(0, 0, FLAG_LOOP, 3),
            frame(0, 0, FLAG_LOOP | FLAG_END, 4),
            frame(0, 0, 0, 5),
        ]
        .concat();
        let sound = decode(&data, 44_100);
        assert_eq!(sound.samples.len(), 4 * 28);
        assert_eq!(sound.looped, Some((28, 112)));
    }
}
