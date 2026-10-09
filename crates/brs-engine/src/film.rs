//! The picture of a film: H.264, decoded a frame at a time on a thread of its own.
//!
//! The game's films (`brs_formats::psmf`) are 480 by 272 at 30000/1001 frames a
//! second, which is little work to decode: the thread keeps a few frames ahead of
//! whoever shows them and waits when it is that far ahead.

use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::sync::Mutex;
use std::thread;

use anyhow::{anyhow, Result};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

/// How many frames are decoded ahead of the one that is shown.
const AHEAD: usize = 6;

/// The frames of a film a second, as a fraction: every film of the game has this.
pub const FRAMES: (u32, u32) = (30_000, 1001);

/// A frame, as four bytes a pixel: red, green, blue and an alpha of all.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The frames of a film's picture, in the order they are shown.
pub struct Pictures {
    /// Behind a lock only so that what holds this can be shared between threads,
    /// as a resource is: one of them takes the frames.
    frames: Mutex<Receiver<Result<Frame>>>,
    over: bool,
}

/// What asking for the next frame comes to.
pub enum Next {
    Frame(Frame),
    /// It has not been decoded yet.
    NotYet,
    /// There are no more.
    Over,
}

impl Pictures {
    /// Starts decoding `stream`, the film's H.264 as its packets carry it.
    pub fn decode(stream: Vec<u8>) -> Pictures {
        let (send, frames) = sync_channel(AHEAD);
        // The thread ends when the film does, or when nobody takes its frames
        // any more.
        let spawned = thread::Builder::new().name("film".into()).spawn(move || {
            let outcome = each_frame(&stream, |frame| send.send(Ok(frame)).is_ok());
            if let Err(error) = outcome {
                let _ = send.send(Err(error));
            }
        });
        if let Err(error) = spawned {
            bevy::log::warn!("the film's decoder could not be started: {error}");
        }
        Pictures { frames: Mutex::new(frames), over: false }
    }

    /// The next frame, if it is there.
    pub fn next(&mut self) -> Next {
        if self.over {
            return Next::Over;
        }
        let Ok(frames) = self.frames.get_mut() else { return Next::Over };
        match frames.try_recv() {
            Ok(Ok(frame)) => Next::Frame(frame),
            Ok(Err(error)) => {
                bevy::log::warn!("the film's picture cannot be decoded: {error:#}");
                self.over = true;
                Next::Over
            }
            Err(TryRecvError::Empty) => Next::NotYet,
            Err(TryRecvError::Disconnected) => {
                self.over = true;
                Next::Over
            }
        }
    }
}

/// Decodes `stream` and hands each frame to `take`, in the order they are shown,
/// until there are no more or `take` answers that it wants no more.
pub fn each_frame(stream: &[u8], mut take: impl FnMut(Frame) -> bool) -> Result<()> {
    let mut decoder = Decoder::new().map_err(|error| anyhow!("the decoder cannot be made: {error}"))?;
    let frame = |picture: &openh264::decoder::DecodedYUV| {
        let (width, height) = picture.dimensions();
        let mut rgba = vec![0; width * height * 4];
        picture.write_rgba8(&mut rgba);
        Frame { width: width as u32, height: height as u32, rgba }
    };
    for unit in openh264::nal_units(stream) {
        // A unit the decoder cannot make anything of is passed over: the film
        // goes on with the next picture that stands by itself.
        let Ok(decoded) = decoder.decode(unit) else { continue };
        if let Some(picture) = decoded {
            if !take(frame(&picture)) {
                return Ok(());
            }
        }
    }
    // What the decoder was keeping back to put in order.
    if let Ok(left) = decoder.flush_remaining() {
        for picture in &left {
            if !take(frame(picture)) {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/extracted/PSP_GAME/USRDIR/MOVIE/BRS_OP_FINAL.PMF");

    /// The mean brightness of a frame, which is enough to tell frames apart.
    fn brightness(frame: &Frame) -> f64 {
        let sum: u64 = frame.rgba.chunks_exact(4).map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2])).sum();
        sum as f64 / (frame.rgba.len() / 4 * 3) as f64
    }

    #[test]
    fn the_opening_film_decodes_to_every_one_of_its_frames() {
        let Ok(bytes) = std::fs::read(OPENING) else {
            eprintln!("skipped: the game's films are not there");
            return;
        };
        let film = brs_formats::psmf::Film::parse(&bytes).expect("the film parses");
        let mut frames = 0u32;
        let mut sizes = std::collections::BTreeSet::new();
        let mut levels = Vec::new();
        // With `BRS_FILM_DUMP` naming a folder, every thirtieth frame of the first
        // six hundred is written there as a PPM, to be looked at.
        let dump = std::env::var_os("BRS_FILM_DUMP");
        each_frame(&film.picture, |frame| {
            sizes.insert((frame.width, frame.height));
            levels.push(brightness(&frame));
            if let Some(folder) = dump.as_ref().filter(|_| frames < 600 && frames.is_multiple_of(30)) {
                let mut ppm = format!("P6\n{} {}\n255\n", frame.width, frame.height).into_bytes();
                ppm.extend(frame.rgba.chunks_exact(4).flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]));
                std::fs::write(std::path::Path::new(folder).join(format!("film_{frames:04}.ppm")), ppm).unwrap();
            }
            frames += 1;
            true
        })
        .expect("the film decodes");
        // 96.363 seconds at 30000/1001 a second.
        let expected = (film.seconds() * f64::from(FRAMES.0) / f64::from(FRAMES.1)).round() as u32;
        assert_eq!(sizes.into_iter().collect::<Vec<_>>(), [(480, 272)]);
        assert!(frames.abs_diff(expected) <= 2, "{frames} frames for {expected}");
        // Frames that follow one another are alike: were they given in the order
        // they are decoded in and not the order they are shown in, the level would
        // jump back and forth wherever the picture changes, which is most of the
        // film. As it is, only its flashes do (two dozen frames).
        let jumps = levels.windows(3).filter(|w| (w[1] - w[0]).abs() > 6.0 && (w[2] - w[1]).abs() > 6.0 && (w[2] - w[0]).abs() < 3.0).count();
        assert!(jumps < 60, "{jumps} frames stand out from both their neighbours");
    }
}
