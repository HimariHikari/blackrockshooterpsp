//! Sound: recordings and the sounds of the game's banks as samples in memory, played
//! through Bevy's audio.
//!
//! A tune loops where its recording says, which no ready-made source does: so the
//! port keeps every sound as samples ([`Pcm`]) and gives the mixer a source of its own
//! that goes back to the loop's start when it reaches its end.
//!
//! What is played is a [`Voice`]: a sound and how loud it is to the left and to the
//! right ([`Gains`]), which whoever has it played can change while it plays. The
//! source reads them as it goes and comes to them over a few milliseconds, so that a
//! change is not heard as a click.
//!
//! A run that nobody is watching makes no sound either (see [`crate::offscreen`]): the
//! sounds are loaded and asked for all the same, and nothing plays them. A sound can
//! also be played with nothing to hear in it ([`Pcm::hushed`]), which is how the way
//! sound takes to the speakers is checked without anyone having to listen.

use std::io::Cursor;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bevy::audio::{AddAudioSource, Decodable};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use brs_formats::at3::Recording;
use lewton::inside_ogg::OggStreamReader;

/// The seconds a voice takes to come most of the way to a loudness it is given.
const GAIN_SECONDS: f32 = 0.004;
/// How loud a voice that is held still is when it stops where it is: it falls
/// silent first, as it would come to any other loudness.
const HELD_SILENT: f32 = 1e-3;

/// A sound as samples in memory.
#[derive(Asset, TypePath, Clone)]
pub struct Pcm {
    /// The samples of each frame after one another: a frame has one for each channel.
    samples: Arc<[i16]>,
    channels: NonZero<u16>,
    rate: NonZero<u32>,
    /// The stretch it repeats for ever once it has got to it, in frames: its first
    /// frame and the one after its last. Without one it is played once.
    looped: Option<(usize, usize)>,
    /// It is played as silence: every sample of it as nothing.
    hushed: bool,
}

impl Pcm {
    /// A sound of `samples`, which hold `channels` samples for each frame. `None` for
    /// a sound of no channels or no rate.
    pub fn new(samples: Vec<i16>, channels: u16, rate: u32) -> Option<Pcm> {
        let (channels, rate) = (NonZero::new(channels)?, NonZero::new(rate)?);
        Some(Pcm { samples: samples.into(), channels, rate, looped: None, hushed: false })
    }

    /// Decodes an Ogg Vorbis file.
    pub fn from_ogg(bytes: &[u8]) -> Result<Pcm> {
        let mut reader = OggStreamReader::new(Cursor::new(bytes)).context("not an Ogg Vorbis file")?;
        let (channels, rate) = (reader.ident_hdr.audio_channels, reader.ident_hdr.audio_sample_rate);
        let mut samples = Vec::new();
        while let Some(packet) = reader.read_dec_packet_itl().context("decoding Ogg Vorbis")? {
            samples.extend(packet);
        }
        Pcm::new(samples, u16::from(channels), rate).context("an Ogg Vorbis file of no channels or no rate")
    }

    /// The sound of a recording, from the Ogg Vorbis file made of it (`brs-extract
    /// --sound`) and what the recording says about itself: its length to the sample,
    /// and where it loops.
    pub fn of_recording(ogg: &[u8], recording: &Recording) -> Result<Pcm> {
        let mut pcm = Pcm::from_ogg(ogg)?;
        // The file is the recording's own samples, give or take one.
        let wanted = recording.samples as usize * pcm.channels.get() as usize;
        if pcm.samples.len() != wanted {
            let mut samples = pcm.samples.to_vec();
            samples.resize(wanted, 0);
            pcm.samples = samples.into();
        }
        pcm.looped = recording.looped.map(|(first, end)| (first as usize, end as usize));
        Ok(pcm)
    }

    /// The same sound, repeating the frames from `first` up to `end` for ever.
    pub fn looping(mut self, first: usize, end: usize) -> Pcm {
        let end = end.min(self.frames());
        self.looped = (first < end).then_some((first, end));
        self
    }

    /// The same sound, played once whatever its recording said.
    pub fn once(mut self) -> Pcm {
        self.looped = None;
        self
    }

    /// The same sound played faster or slower, and so higher or lower: `ratio` times
    /// as fast.
    pub fn pitched(mut self, ratio: f32) -> Pcm {
        let rate = (self.rate.get() as f32 * ratio).round().max(1.0) as u32;
        self.rate = NonZero::new(rate).unwrap_or(self.rate);
        self
    }

    /// The same sound with nothing to hear in it: it takes the time it takes, and
    /// is silence throughout.
    pub fn hushed(mut self) -> Pcm {
        self.hushed = true;
        self
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.get() as usize
    }

    pub fn channels(&self) -> u16 {
        self.channels.get()
    }

    pub fn rate(&self) -> u32 {
        self.rate.get()
    }

    pub fn looped(&self) -> Option<(usize, usize)> {
        self.looped
    }

    /// How long it is, played once through.
    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.rate.get() as f32
    }

    /// The loudest sample, as a part of the loudest there can be: for checking that
    /// a sound has come out as sound.
    pub fn peak(&self) -> f32 {
        self.samples.iter().map(|sample| sample.unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0
    }

    /// What frame `index` has for the left and for the right, as parts of full: a
    /// sound of one channel has the same for both.
    fn frame(&self, index: usize) -> Option<[f32; 2]> {
        let channels = self.channels.get() as usize;
        let frame = self.samples.get(index * channels..(index + 1) * channels)?;
        if self.hushed {
            return Some([0.0; 2]);
        }
        let part = |sample: i16| f32::from(sample) / 32768.0;
        Some([part(frame[0]), part(frame[channels.min(2) - 1])])
    }
}

/// How loud a sound that is being played is, to the left and to the right, and
/// whether it is held still: set by whoever has it played, and read by what plays
/// it.
#[derive(Clone, Debug, Default)]
pub struct Gains(Arc<Shared>);

#[derive(Debug, Default)]
struct Shared {
    sides: [AtomicU32; 2],
    held: AtomicBool,
}

impl Gains {
    pub fn new(left: f32, right: f32) -> Gains {
        let gains = Gains::default();
        gains.set(left, right);
        gains
    }

    pub fn set(&self, left: f32, right: f32) {
        self.0.sides[0].store(left.to_bits(), Ordering::Relaxed);
        self.0.sides[1].store(right.to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> [f32; 2] {
        [f32::from_bits(self.0.sides[0].load(Ordering::Relaxed)), f32::from_bits(self.0.sides[1].load(Ordering::Relaxed))]
    }

    /// Has the sound stand still where it is, silent, or go on from there.
    pub fn hold(&self, held: bool) {
        self.0.held.store(held, Ordering::Relaxed);
    }

    pub fn held(&self) -> bool {
        self.0.held.load(Ordering::Relaxed)
    }
}

/// A sound that is played, and how loud.
#[derive(Asset, TypePath, Clone)]
pub struct Voice {
    pcm: Pcm,
    gains: Gains,
}

impl Voice {
    pub fn new(pcm: Pcm, gains: Gains) -> Voice {
        Voice { pcm, gains }
    }
}

/// A [`Voice`] as the mixer takes it: a sample for the left and one for the right,
/// frame after frame.
pub struct VoiceSource {
    pcm: Pcm,
    gains: Gains,
    /// The frame that is next.
    at: usize,
    /// How loud it is now: on its way to what it has been given.
    now: [f32; 2],
    /// The part of the way it goes with each frame.
    slew: f32,
    /// The right of the frame whose left has just been given, if that is what is
    /// next.
    right: Option<f32>,
}

impl Iterator for VoiceSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        // Held, it falls silent as it would come to any loudness, and then stands
        // where it is and gives nothing until it is let go.
        let held = self.gains.held();
        if held && self.now.iter().all(|gain| gain.abs() < HELD_SILENT) {
            self.right = Some(0.0);
            return Some(0.0);
        }
        if let Some((first, end)) = self.pcm.looped {
            if self.at >= end {
                self.at = first;
            }
        }
        let frame = self.pcm.frame(self.at)?;
        self.at += 1;
        let to = if held { [0.0; 2] } else { self.gains.get() };
        for side in 0..2 {
            self.now[side] += (to[side] - self.now[side]) * self.slew;
        }
        self.right = Some(frame[1] * self.now[1]);
        Some(frame[0] * self.now[0])
    }
}

impl bevy::audio::Source for VoiceSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::<u16>::new(2).unwrap()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        self.pcm.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        match self.pcm.looped {
            Some(_) => None,
            None => Some(Duration::from_secs_f32(self.pcm.seconds())),
        }
    }
}

impl Decodable for Voice {
    type Decoder = VoiceSource;

    fn decoder(&self) -> VoiceSource {
        // It starts as loud as it has been given, and goes to what it is given
        // after that by a part of the way with each frame.
        let slew = 1.0 - (-1.0 / (GAIN_SECONDS * self.pcm.rate.get() as f32)).exp();
        VoiceSource { pcm: self.pcm.clone(), gains: self.gains.clone(), at: 0, now: self.gains.get(), slew, right: None }
    }
}

/// Makes [`Pcm`] and [`Voice`] kinds of sound Bevy knows. With `silent` they are only
/// loaded and kept: for a run without the plugin that plays sound.
pub fn plugin(app: &mut App, silent: bool) {
    app.init_asset::<Pcm>();
    if silent {
        app.init_asset::<Voice>();
    } else {
        app.add_audio_source::<Voice>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(frames: usize, channels: u16) -> Pcm {
        let samples = (0..frames * channels as usize).map(|n| (n / channels as usize) as i16 * 100).collect();
        Pcm::new(samples, channels, 44_100).unwrap()
    }

    /// What a sound comes out as at full loudness: the frames' left samples, in
    /// hundreds.
    fn played(pcm: Pcm) -> impl Iterator<Item = i32> {
        Voice::new(pcm, Gains::new(1.0, 1.0)).decoder().step_by(2).map(|sample| (sample * 32768.0).round() as i32 / 100)
    }

    #[test]
    fn a_sound_is_played_once_through_to_both_sides() {
        let pcm = ramp(4, 1);
        assert_eq!((pcm.frames(), pcm.channels(), pcm.rate()), (4, 1, 44_100));
        // One channel goes to the left and to the right alike.
        let out: Vec<f32> = Voice::new(pcm.clone(), Gains::new(1.0, 0.5)).decoder().collect();
        assert_eq!(out.len(), 8);
        assert_eq!((out[2], out[3]), (100.0 / 32768.0, 50.0 / 32768.0));
        assert_eq!((out[6], out[7]), (300.0 / 32768.0, 150.0 / 32768.0));
        assert!(Pcm::new(Vec::new(), 0, 44_100).is_none());
        assert!((pcm.peak() - 300.0 / 32768.0).abs() < 1e-6);
        // Two channels keep their sides.
        let stereo = Pcm::new(vec![100, -100, 200, -200], 2, 44_100).unwrap();
        let out: Vec<f32> = Voice::new(stereo, Gains::new(1.0, 1.0)).decoder().collect();
        assert_eq!(out, [100.0 / 32768.0, -100.0 / 32768.0, 200.0 / 32768.0, -200.0 / 32768.0]);
        // Hushed it is as long, and nothing.
        let hushed: Vec<f32> = Voice::new(pcm.clone().hushed(), Gains::new(1.0, 1.0)).decoder().collect();
        assert_eq!(hushed, [0.0; 8]);
        // An octave up it is the same samples in half the time.
        let higher = pcm.clone().pitched(2.0);
        assert_eq!((higher.rate(), higher.frames(), higher.seconds()), (88_200, 4, pcm.seconds() / 2.0));
    }

    #[test]
    fn a_tune_goes_back_to_where_its_loop_starts() {
        // Ten frames, of which the fourth to the eighth repeat.
        let pcm = ramp(10, 1).looping(3, 8);
        let out: Vec<i32> = played(pcm.clone()).take(16).collect();
        assert_eq!(out, [0, 1, 2, 3, 4, 5, 6, 7, 3, 4, 5, 6, 7, 3, 4, 5]);
        let voice = Voice::new(pcm.clone(), Gains::new(1.0, 1.0));
        assert!(bevy::audio::Source::total_duration(&voice.decoder()).is_none());
        // A loop of nothing is no loop, and a loop can be taken off again.
        assert_eq!(ramp(10, 1).looping(8, 8).looped(), None);
        assert_eq!(played(pcm.once()).count(), 10);
        // Two channels: the loop is counted in frames.
        let frames: Vec<i32> = played(ramp(4, 2).looping(1, 3)).take(6).collect();
        assert_eq!(frames, [0, 1, 2, 1, 2, 1]);
    }

    #[test]
    fn a_voice_that_is_held_stands_still_and_goes_on_from_there_when_it_is_let_go() {
        let level = Pcm::new(vec![16_384; 20_000], 1, 44_100).unwrap();
        let gains = Gains::new(1.0, 1.0);
        let mut source = Voice::new(level, gains.clone()).decoder();
        let heard = |source: &mut VoiceSource, frames: usize| -> Vec<f32> { source.by_ref().take(2 * frames).step_by(2).collect() };
        assert_eq!(heard(&mut source, 4), [0.5; 4]);
        // Held, it is silent within a few milliseconds, and then stays where it is
        // for as long as it is held: it does not come to its end.
        gains.hold(true);
        assert!(gains.held());
        heard(&mut source, 4000);
        let stood = source.at;
        assert!(stood > 4 && stood < 4004, "{stood}");
        assert_eq!(heard(&mut source, 50_000), vec![0.0; 50_000]);
        assert_eq!(source.at, stood);
        // Let go, it goes on from the frame it stood at and comes back to its
        // loudness; and it ends when it has been played through.
        gains.hold(false);
        let after = heard(&mut source, 2000);
        assert_eq!(source.at, stood + 2000);
        assert!(after[0] < 0.01 && (after[1999] - 0.5).abs() < 0.005, "{} then {}", after[0], after[1999]);
        assert_eq!(source.by_ref().count(), 2 * (20_000 - stood - 2000));
    }

    #[test]
    fn a_voice_comes_to_the_loudness_it_is_given_in_a_few_milliseconds() {
        let level = Pcm::new(vec![16_384; 4410], 1, 44_100).unwrap();
        let gains = Gains::new(0.0, 0.0);
        let mut source = Voice::new(level, gains.clone()).decoder();
        // At nothing it is nothing.
        assert_eq!(source.by_ref().take(20).fold(0.0, f32::max), 0.0);
        // Given all of it, it is not there at once, and is there within 20 ms; the
        // right, given half, comes to half.
        gains.set(1.0, 0.5);
        let (first, _) = (source.next().unwrap(), source.next());
        assert!(first > 0.0 && first < 0.01);
        let rest: Vec<f32> = source.by_ref().take(2 * 882).collect();
        let (left, right) = (rest[rest.len() - 2], rest[rest.len() - 1]);
        assert!((left - 0.5).abs() < 0.005 && (right - 0.25).abs() < 0.005, "{left} {right}");
    }
}
