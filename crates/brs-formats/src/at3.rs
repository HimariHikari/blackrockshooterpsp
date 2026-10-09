//! Recordings: the game's music (`SOUND/*.AT3`) and everything that is spoken or
//! heard in an event, each a `RIFF` file of ATRAC3plus.
//!
//! This reads what such a file says about itself. The sound itself is Sony's codec,
//! which the port does not decode: `brs-extract --sound` has ffmpeg turn each
//! recording into an Ogg Vorbis file beside it, cut to the samples this says are the
//! recording's own.
//!
//! The chunks: `fmt ` (the extensible kind of format, with the codec in a GUID),
//! `fact` (how many samples the recording has, and how many a decoder gives before
//! the first of them: a frame of 2048 and the codec's own 184, more for music), `smpl`
//! (where it loops, counted in the decoder's samples), `data`.

use anyhow::{ensure, Context, Result};

use crate::reader::Reader;

/// What a recording says about itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recording {
    pub channels: u16,
    /// Samples a second.
    pub rate: u32,
    /// How many samples it has, of each channel.
    pub samples: u32,
    /// How many samples a decoder gives before the first of them.
    pub lead_in: u32,
    /// The stretch it repeats, if it loops: its first sample and the one after its
    /// last, counted from the recording's own first sample.
    pub looped: Option<(u32, u32)>,
}

/// Whether `d` is a recording: a `RIFF` file of the `WAVE` kind.
pub fn is_recording(d: &[u8]) -> bool {
    d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WAVE"
}

impl Recording {
    pub fn parse(d: &[u8]) -> Result<Recording> {
        ensure!(is_recording(d), "not a RIFF recording");
        let r = Reader(d);
        let (mut format, mut fact, mut looped) = (None, None, None);
        let mut at = 12;
        while at + 8 <= d.len() {
            let (name, size) = (&d[at..at + 4], r.u32(at + 4)? as usize);
            let body = at + 8;
            match name {
                b"fmt " => format = Some((r.u16(body + 2)?, r.u32(body + 4)?)),
                // The samples, and after them the lead-in: the last of the chunk's
                // words, of which ATRAC3plus has three.
                b"fact" => {
                    ensure!(size >= 8, "a fact chunk of {size} bytes");
                    fact = Some((r.u32(body)?, r.u32(body + size.min(12) - 4)?));
                }
                // The sampler's chunk: how many loops at + 28, the first from + 36,
                // with its first and last sample at + 8 and + 12 of it.
                b"smpl" if size >= 60 && r.u32(body + 28)? > 0 => {
                    looped = Some((r.u32(body + 44)?, r.u32(body + 48)?));
                }
                _ => {}
            }
            // Chunks are padded to an even length.
            at = body + size + (size & 1);
        }
        let (channels, rate) = format.context("the recording has no format")?;
        let (samples, lead_in) = fact.context("the recording does not say how long it is")?;
        ensure!(channels > 0 && rate > 0, "a recording of {channels} channels at {rate} Hz");
        // The loop is given in the decoder's samples, its end the last one of it.
        let looped = looped.and_then(|(first, last): (u32, u32)| {
            let (first, end) = (first.checked_sub(lead_in)?, (last + 1).checked_sub(lead_in)?.min(samples));
            (first < end).then_some((first, end))
        });
        Ok(Recording { channels, rate, samples, lead_in, looped })
    }

    pub fn seconds(&self) -> f64 {
        f64::from(self.samples) / f64::from(self.rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording's file without its sound: the chunks that describe it.
    fn file(channels: u16, fact: &[u32], looped: Option<(u32, u32)>) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend(b"RIFF\0\0\0\0WAVE");
        let mut chunk = |name: &[u8; 4], body: &[u8]| {
            d.extend(name);
            d.extend((body.len() as u32).to_le_bytes());
            d.extend(body);
            if body.len() % 2 == 1 {
                d.push(0);
            }
        };
        let mut format = vec![0xfe, 0xff];
        format.extend(channels.to_le_bytes());
        format.extend(44_100u32.to_le_bytes());
        format.extend([0; 44]);
        chunk(b"fmt ", &format);
        chunk(b"fact", &fact.iter().flat_map(|word| word.to_le_bytes()).collect::<Vec<u8>>());
        if let Some((first, last)) = looped {
            let mut sampler = vec![0u8; 60];
            sampler[28] = 1;
            sampler[44..48].copy_from_slice(&first.to_le_bytes());
            sampler[48..52].copy_from_slice(&last.to_le_bytes());
            chunk(b"smpl", &sampler);
        }
        chunk(b"data", &[0; 7]);
        d
    }

    #[test]
    fn a_tune_says_how_long_it_is_and_where_it_loops() {
        // The first battle tune: its loop is given in the decoder's samples, which
        // start 3033 before the tune's own.
        let tune = Recording::parse(&file(2, &[4_639_600, 2849, 3033], Some((159_176, 4_642_631)))).unwrap();
        assert_eq!((tune.channels, tune.rate, tune.samples, tune.lead_in), (2, 44_100, 4_639_600, 3033));
        assert_eq!(tune.looped, Some((156_143, 4_639_599)));
        assert!((tune.seconds() - 105.2).abs() < 0.01);
    }

    #[test]
    fn a_line_does_not_loop() {
        let line = Recording::parse(&file(1, &[79_245, 2048, 2232], None)).unwrap();
        assert_eq!((line.channels, line.samples, line.lead_in, line.looped), (1, 79_245, 2232, None));
        // The older codec's files have two words: the samples and the lead-in.
        let old = Recording::parse(&file(2, &[1000, 1024], None)).unwrap();
        assert_eq!((old.samples, old.lead_in), (1000, 1024));
    }

    #[test]
    fn what_is_not_a_recording_is_refused() {
        assert!(!is_recording(b"RIFF\0\0\0\0AVI "));
        assert!(Recording::parse(b"INSM").is_err());
        // No word of its length.
        let mut d = file(1, &[100, 0, 0], None);
        let at = d.windows(4).position(|w| w == b"fact").unwrap();
        d[at..at + 4].copy_from_slice(b"junk");
        assert!(Recording::parse(&d).is_err());
        // A loop that lies before the recording's own first sample is no loop.
        let odd = Recording::parse(&file(1, &[100, 0, 50], Some((10, 20)))).unwrap();
        assert_eq!(odd.looped, None);
    }
}
