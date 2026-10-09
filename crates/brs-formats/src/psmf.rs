//! Films: `MOVIE/*.PMF`, which the PSP's own film player plays for the game
//! (`scePsmfPlayer`, by the game's 0x11863C).
//!
//! A film is a header of 0x800 bytes and then an MPEG program stream in packs of
//! 0x800 bytes. The header: `PSMF`, a version (`0015`), where the stream begins and
//! how long it is (both big-endian, at 8 and 12), and at 0x54 and 0x5A the times of
//! its first picture and of its end, six bytes each, in 90000ths of a second.
//!
//! The stream's packets (`00 00 01`, a stream's number, a length of two bytes,
//! big-endian; a pack's own header, `BA`, is fourteen bytes and its stuffing):
//!
//! - `E0`, the picture: H.264 as a stream of units that each begin `00 00 01`,
//!   480 by 272, 30000/1001 pictures a second.
//! - `BD`, private stream 1, the sound: after the packet's header four bytes (the
//!   sub-stream, which is 0, and where the first frame of the packet begins) and
//!   then ATRAC3plus, in frames that each begin `0F D0`, two bytes whose low ten
//!   bits are the frame's length in eights less one, and four bytes more. A film's
//!   frames are 0x2E8 bytes: 2048 samples of two channels at 44100 a second.
//! - `BB` (the system's header), `BE` (padding) and `BF` (private stream 2, which
//!   the player keeps its own index in) carry nothing of either.
//!
//! The port decodes the picture itself. The sound is Sony's codec, as the game's
//! recordings are ([`crate::at3`]): [`Film::recording`] writes it out the way they
//! are kept, for `brs-extract --sound` to have ffmpeg turn into Ogg Vorbis.

use anyhow::{ensure, Context, Result};

/// The length of the header, and of a pack of the stream after it.
const HEADER: usize = 0x800;
/// A time stamp's ticks in a second.
pub const TICKS: u64 = 90_000;
/// The samples of a frame of ATRAC3plus, and the sound's rate and channels: the
/// same for every film.
pub const FRAME_SAMPLES: u32 = 2048;
pub const RATE: u32 = 44_100;
pub const CHANNELS: u16 = 2;

const VIDEO: u8 = 0xe0;
const PRIVATE_1: u8 = 0xbd;
const PACK: u8 = 0xba;
const END: u8 = 0xb9;
/// The streams whose packets have no header of the usual kind.
const PLAIN: [u8; 3] = [0xbb, 0xbe, 0xbf];

/// A film, taken apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Film {
    /// How long it is, in ticks: from the time of its first picture to its end.
    pub ticks: u64,
    /// The picture: the H.264 stream as the packets carry it.
    pub picture: Vec<u8>,
    /// The sound: its frames, each without its header.
    pub sound: Vec<Vec<u8>>,
    /// How far ahead of the first picture the sound begins, in ticks: what of it
    /// is not played.
    pub sound_lead: u64,
}

/// Whether `d` begins as a film does.
pub fn is_film(d: &[u8]) -> bool {
    d.len() >= HEADER && &d[..4] == b"PSMF"
}

/// A time stamp of five bytes, as a packet's header has it.
fn stamp(b: &[u8]) -> u64 {
    (u64::from(b[0] >> 1) & 7) << 30 | u64::from(b[1]) << 22 | u64::from(b[2] >> 1) << 15 | u64::from(b[3]) << 7 | u64::from(b[4] >> 1)
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0, |value, &byte| value << 8 | u64::from(byte))
}

impl Film {
    pub fn parse(d: &[u8]) -> Result<Film> {
        ensure!(is_film(d), "not a film: it does not begin PSMF");
        let (start, size) = (be(&d[8..12]) as usize, be(&d[12..16]) as usize);
        ensure!(start >= HEADER && start <= d.len(), "the film's stream is said to begin at {start:#x}");
        let (first, last) = (be(&d[0x54..0x5a]), be(&d[0x5a..0x60]));
        ensure!(last > first, "the film is said to run from {first} to {last}");
        let stream = &d[start..d.len().min(start + size)];

        let (mut picture, mut sound_stream) = (Vec::new(), Vec::new());
        let (mut first_picture, mut first_sound) = (None, None);
        let mut at = 0;
        while at + 6 <= stream.len() {
            if stream[at..at + 3] != [0, 0, 1] {
                at += 1;
                continue;
            }
            let code = stream[at + 3];
            if code == PACK {
                let Some(&stuffing) = stream.get(at + 13) else { break };
                at += 14 + usize::from(stuffing & 7);
                continue;
            }
            if code == END {
                break;
            }
            let length = be(&stream[at + 4..at + 6]) as usize;
            let body = stream.get(at + 6..at + 6 + length).context("the film ends in the middle of a packet")?;
            at += 6 + length;
            if PLAIN.contains(&code) || body.len() < 3 {
                continue;
            }
            // The packet's own header: two bytes of flags and how much more of it
            // there is, in which a time stamp comes first if the flags say so.
            let (flags, more) = (body[1], usize::from(body[2]));
            let payload = body.get(3 + more..).context("a packet's header is longer than the packet")?;
            let time = (flags & 0x80 != 0 && more >= 5).then(|| stamp(&body[3..8]));
            match code {
                VIDEO => {
                    first_picture = first_picture.or(time);
                    picture.extend_from_slice(payload);
                }
                // Sub-stream 0 is the sound a film has; the three bytes after it
                // say where in the packet a frame begins, which the frames' own
                // beginnings say as well.
                PRIVATE_1 if payload.first() == Some(&0) && payload.len() > 4 => {
                    first_sound = first_sound.or(time);
                    sound_stream.extend_from_slice(&payload[4..]);
                }
                _ => {}
            }
        }

        let mut sound = Vec::new();
        let mut at = 0;
        while at + 8 <= sound_stream.len() {
            if sound_stream[at] != 0x0f || sound_stream[at + 1] != 0xd0 {
                at += 1;
                continue;
            }
            let length = ((usize::from(sound_stream[at + 2]) & 3) << 8 | usize::from(sound_stream[at + 3])) * 8 + 8;
            let Some(frame) = sound_stream.get(at + 8..at + 8 + length) else { break };
            sound.push(frame.to_vec());
            at += 8 + length;
        }
        ensure!(!picture.is_empty(), "the film has no picture");
        let sound_lead = match (first_picture, first_sound) {
            (Some(picture), Some(sound)) => picture.saturating_sub(sound),
            _ => 0,
        };
        Ok(Film { ticks: last - first, picture, sound, sound_lead })
    }

    /// How long it is, in seconds.
    pub fn seconds(&self) -> f64 {
        self.ticks as f64 / TICKS as f64
    }

    /// The samples of the sound that come before the first picture.
    pub fn lead_samples(&self) -> u32 {
        (self.sound_lead * u64::from(RATE) / TICKS) as u32
    }

    /// The sound as a recording of the kind the game keeps its others in: a `RIFF`
    /// file of ATRAC3plus that says of itself what a film's sound is. None for a
    /// film without sound.
    pub fn recording(&self) -> Option<Vec<u8>> {
        let block = self.sound.first()?.len();
        if self.sound.iter().any(|frame| frame.len() != block) {
            return None;
        }
        let bytes_a_second = RATE * block as u32 / FRAME_SAMPLES;
        // The extensible format, with the codec's GUID and what it wants to know
        // of the frames: their kind, and two bytes that go by their length.
        let mut format = Vec::new();
        format.extend_from_slice(&0xfffeu16.to_le_bytes());
        format.extend_from_slice(&CHANNELS.to_le_bytes());
        format.extend_from_slice(&RATE.to_le_bytes());
        format.extend_from_slice(&bytes_a_second.to_le_bytes());
        format.extend_from_slice(&(block as u16).to_le_bytes());
        format.extend_from_slice(&0u16.to_le_bytes());
        format.extend_from_slice(&34u16.to_le_bytes());
        format.extend_from_slice(&(FRAME_SAMPLES as u16).to_le_bytes());
        format.extend_from_slice(&3u32.to_le_bytes());
        format.extend_from_slice(&[0xbf, 0xaa, 0x23, 0xe9, 0x58, 0xcb, 0x71, 0x44, 0xa1, 0x19, 0xff, 0xfa, 0x01, 0xe4, 0xce, 0x62]);
        format.extend_from_slice(&1u16.to_le_bytes());
        format.extend_from_slice(&(((block / 8 - 1) as u16) | 0x2800).to_be_bytes());
        format.extend_from_slice(&[0; 8]);

        let samples = self.sound.len() as u32 * FRAME_SAMPLES;
        let mut fact = Vec::new();
        fact.extend_from_slice(&samples.to_le_bytes());
        fact.extend_from_slice(&FRAME_SAMPLES.to_le_bytes());

        let data: usize = self.sound.iter().map(Vec::len).sum();
        let mut out = Vec::with_capacity(data + 128);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((4 + 8 + format.len() + 8 + fact.len() + 8 + data) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        for (name, body) in [(b"fmt ", &format), (b"fact", &fact)] {
            out.extend_from_slice(name);
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(body);
        }
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data as u32).to_le_bytes());
        for frame in &self.sound {
            out.extend_from_slice(frame);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOVIES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/extracted/PSP_GAME/USRDIR/MOVIE");

    #[test]
    fn a_time_stamp_is_thirty_three_bits_in_five_bytes() {
        // One second, as the first picture of a film has it.
        assert_eq!(stamp(&[0x21, 0x00, 0x05, 0xbf, 0x21]), 90_000);
        assert_eq!(be(&[0x00, 0x01, 0x5f, 0x90]), 90_000);
    }

    #[test]
    fn the_opening_film_comes_apart_into_its_picture_and_its_sound() {
        let Ok(bytes) = std::fs::read(format!("{MOVIES}/BRS_OP_FINAL.PMF")) else {
            eprintln!("skipped: the game's films are not there");
            return;
        };
        let film = Film::parse(&bytes).expect("the film parses");
        // A minute and thirty-six seconds, of which the sound has every frame.
        assert!((film.seconds() - 96.363).abs() < 0.001, "{}", film.seconds());
        assert_eq!(film.sound.len(), 2078);
        assert!(film.sound.iter().all(|frame| frame.len() == 0x2e8));
        // The picture begins with a unit that says a picture begins, and what it
        // is to be decoded by.
        assert_eq!(film.picture[..6], [0, 0, 0, 1, 9, 0x10]);
        assert_eq!(film.picture.len(), 12_054_861);
        // The sound begins a little before the picture.
        assert_eq!(film.sound_lead, 4931);
        assert_eq!(film.lead_samples(), 2416);
        let recording = film.recording().expect("the film has sound");
        let read = crate::at3::Recording::parse(&recording).expect("the sound reads as a recording");
        assert_eq!((read.channels, read.rate, read.samples), (2, 44_100, 2078 * 2048));
    }

    #[test]
    fn every_film_of_the_game_comes_apart() {
        let Ok(folder) = std::fs::read_dir(MOVIES) else {
            eprintln!("skipped: the game's films are not there");
            return;
        };
        let mut films = 0;
        for entry in folder {
            let path = entry.unwrap().path();
            let film = Film::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
            assert!(film.seconds() > 1.0 && !film.picture.is_empty(), "{}", path.display());
            // What sound a film has is as long as the film, within a frame or two.
            if !film.sound.is_empty() {
                let sound = film.sound.len() as f64 * f64::from(FRAME_SAMPLES) / f64::from(RATE);
                assert!((sound - film.seconds()).abs() < 0.3, "{}: {sound} s of sound for {} s", path.display(), film.seconds());
                assert!(film.recording().is_some(), "{}", path.display());
            }
            films += 1;
        }
        assert_eq!(films, 36);
    }
}
