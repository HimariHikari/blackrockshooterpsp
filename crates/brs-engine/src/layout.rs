//! The interface's layouts at work: what the game's layout library (0x2E424 to
//! 0x33C44) does with a layout's objects, motions and groups.
//!
//! A [`Player`] plays the layout's sets: each track a set gives it keeps nine values
//! for its group of objects (where it is moved to, how it is scaled and turned, its
//! colour), which the keys of the track's clip change as the frames go by. A set
//! played after another starts anew the tracks of the groups it has and leaves the
//! rest as they are, which is how a set of one group changes one part of what is
//! on the screen. Drawing a player draws its tracks' groups in their order, each
//! object where the layout has it, moved and coloured by its track. Frames are
//! sixtieths of a second (0x850).

use anyhow::{ensure, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_formats::layout::{Channel, Ease, Groups, Key, Mark, Motions, Object, Objects, Track};

use crate::canvas::{Canvas, Paint, Vertex};

/// How much of a half turn of the sine the eases add or take, in hundredths
/// (0x2F978, 0x2F9C0).
const EASE_IN: f32 = -100.0;
const EASE_OUT: f32 = 100.0;
/// What an ease's hundredths are divided by: a hundred times pi (0x2DDBC).
const EASE_SCALE: f32 = 314.159;

const SLOTS: usize = Channel::ALL.len();
/// A track's values before any key changes them (0x2E608): where its objects are,
/// their own size, unturned, and all of their colour.
const AT_REST: [f32; SLOTS] = [0.0, 0.0, 1.0, 1.0, 0.0, 255.0, 255.0, 255.0, 255.0];

/// How something of a layout is put on the screen: the colour all of it is drawn by,
/// and the two matrices a layout has (0x32174, 0x321F8). The inner one moves an
/// object's corners before its track does; the outer one moves what comes of it
/// all. And how far what its objects are drawn from is moved in their textures
/// (0x15AE38), as a part of the texture's width and height: which is how one
/// object shows any figure of a sheet of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub tint: [u8; 4],
    pub inner: Affine2,
    pub outer: Affine2,
    pub shift: Vec2,
}

impl Default for Look {
    fn default() -> Look {
        Look { tint: [255; 4], inner: Affine2::IDENTITY, outer: Affine2::IDENTITY, shift: Vec2::ZERO }
    }
}

impl Look {
    pub fn tinted(tint: [u8; 4]) -> Look {
        Look { tint, ..default() }
    }

    pub fn inner(inner: Affine2) -> Look {
        Look { inner, ..default() }
    }
}

/// A layout, with the textures its objects are drawn with.
#[derive(Clone, Debug)]
pub struct Layout {
    pub objects: Objects,
    pub motions: Motions,
    pub groups: Groups,
    pub textures: Vec<Handle<Image>>,
}

impl Layout {
    /// A layout from its three files.
    pub fn parse(objects: &[u8], motions: &[u8], groups: &[u8], textures: Vec<Handle<Image>>) -> Result<Layout> {
        let (objects, motions, groups) = (Objects::parse(objects)?, Motions::parse(motions)?, Groups::parse(groups)?);
        ensure!(groups.colours.len() == objects.objects.len(), "the groups are of {} objects, the layout has {}", groups.colours.len(), objects.objects.len());
        let most = motions.sets.iter().flatten().map(|track| track.group as usize).max();
        ensure!(most.is_none_or(|most| most < groups.groups.len()), "a track moves group {most:?} of {}", groups.groups.len());
        Ok(Layout { objects, motions, groups, textures })
    }

    /// Something to play the layout's sets with (0x308A8), playing set `set` from
    /// its start (0x30710).
    pub fn player(&self, set: usize) -> Player {
        let mut player = Player { set, tracks: Vec::new(), order: Vec::new() };
        player.play(self, set);
        player
    }

    fn tracks_of(&self, set: usize) -> &[Track] {
        self.motions.sets.get(set).map_or(&[], Vec::as_slice)
    }

    fn clip(&self, track: &Track) -> &[(Channel, Vec<Key>)] {
        self.motions.clips.get(track.clip as usize).map_or(&[], Vec::as_slice)
    }

    /// Where on the screen an object is put before anything moves it.
    pub fn place(&self, object: usize) -> Option<Vec2> {
        self.objects.objects.get(object).map(|object| Vec2::new(object.position[0] as f32, object.position[1] as f32))
    }

    /// The first object of a group, which is what code that asks where a group is
    /// goes by (0x3315C).
    pub fn first_of(&self, group: usize) -> Option<usize> {
        self.groups.groups.get(group).and_then(|group| group.first().copied())
    }

    /// The frame at which a set has been played through (0x2E680 answers so): the
    /// last key of all its tracks' channels, each after its track's delay.
    pub fn length(&self, set: usize) -> f32 {
        let ends = self.tracks_of(set).iter().flat_map(|track| self.clip(track).iter().filter_map(|(_, keys)| keys.last()).map(|key| key.frame + track.delay as u32));
        ends.max().unwrap_or(0) as f32
    }

    /// The frame at which one track of a set has played its clip through (0x2EFA4):
    /// its channels' keys up to the first that marks where the clip goes round.
    pub fn track_length(&self, set: usize, track: usize) -> f32 {
        let Some(track) = self.motions.sets.get(set).and_then(|tracks| tracks.get(track)) else { return 0.0 };
        let ends = self.clip(track).iter().flat_map(|(_, keys)| keys.iter().take_while(|key| key.mark == Mark::None)).map(|key| key.frame + track.delay as u32);
        ends.max().unwrap_or(0) as f32
    }

    /// Which track of a set moves a group (0x2E8C4): the first that does.
    pub fn track_of(&self, set: usize, group: usize) -> Option<usize> {
        self.motions.sets.get(set)?.iter().position(|track| track.group as usize == group)
    }

    /// Draws what a player is playing (0x30CD4): every track of it that is not
    /// hidden, in their order.
    pub fn draw(&self, canvas: &mut Canvas, layer: i32, player: &Player, look: &Look) {
        for track in 0..player.len() {
            if !player.hidden(track) {
                self.draw_track(canvas, layer, player, track, look);
            }
        }
    }

    /// Draws one track of what a player is playing (0x30F30): every object of its
    /// group.
    pub fn draw_track(&self, canvas: &mut Canvas, layer: i32, player: &Player, track: usize, look: &Look) {
        let Some((group, moved, colour)) = self.seen(player, track, look.tint) else { return };
        for &object in &self.groups.groups[group] {
            self.draw_object(canvas, layer, object, moved * look.inner, colour, look.outer, look.shift);
        }
    }

    /// Draws one object of a track's group (0x31130): the one in place `member` of it.
    pub fn draw_member(&self, canvas: &mut Canvas, layer: i32, player: &Player, track: usize, member: usize, look: &Look) {
        let Some((group, moved, colour)) = self.seen(player, track, look.tint) else { return };
        if let Some(&object) = self.groups.groups[group].get(member) {
            self.draw_object(canvas, layer, object, moved * look.inner, colour, look.outer, look.shift);
        }
    }

    /// Where a track has its group and in what colour (0x31DE4); nothing while it
    /// has no alpha. The layout's own matrices are not in it.
    pub fn placed(&self, player: &Player, track: usize) -> Option<(Affine2, [u8; 4])> {
        let state = player.state(track)?;
        state.playing.placed(&state.track)
    }

    /// The colour an object is drawn in whatever its track (0x33190): the groups'
    /// file has one for each.
    pub fn colour(&self, object: usize) -> [u8; 4] {
        self.groups.colours.get(object).copied().unwrap_or([255; 4])
    }

    /// A track's group, how it is moved and its colour by a tint, if anything of it
    /// is to be seen.
    fn seen(&self, player: &Player, track: usize, tint: [u8; 4]) -> Option<(usize, Affine2, [u8; 4])> {
        let (moved, colour) = self.placed(player, track)?;
        let colour = modulate(colour, tint);
        let group = player.group(track)?;
        (colour[3] != 0 && group < self.groups.groups.len()).then_some((group, moved, colour))
    }

    /// What a clip comes to at a frame, played from its start by nothing in
    /// particular (0x3205C): how it moves and colours.
    pub fn clip_at(&self, clip: usize, frame: f32) -> Option<(Affine2, [u8; 4])> {
        let clip = self.motions.clips.get(clip)?;
        let mut playing = Playing::AT_START;
        playing.rewind(clip);
        playing.advance(clip, 0.0, frame);
        playing.placed(&Track { group: 0, clip: 0, delay: 0, hidden: false, offset: [0.0, 0.0] })
    }

    /// Draws one object (0x25430): its corners moved as its track has them, then put
    /// at the object's place.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_object(&self, canvas: &mut Canvas, layer: i32, object: usize, moved: Affine2, colour: [u8; 4], outer: Affine2, shift: Vec2) {
        if let Some(object) = self.objects.objects.get(object) {
            draw_object(canvas, layer, object, &self.textures, moved, colour, outer, shift);
        }
    }
}

/// Draws an object of a file of objects with the file's textures (0x25430): its
/// corners moved by one matrix, put at the object's place, and all of it moved by
/// the other; its colours by the one given, and what it is drawn from moved so
/// far in its texture.
#[allow(clippy::too_many_arguments)]
pub fn draw_object(canvas: &mut Canvas, layer: i32, object: &Object, textures: &[Handle<Image>], moved: Affine2, colour: [u8; 4], outer: Affine2, shift: Vec2) {
    let Some(quad) = &object.quad else { return };
    let at = Vec2::new(object.position[0] as f32, object.position[1] as f32);
    let corners = quad.corners.map(|corner| {
        let position = outer.transform_point2(at + moved.transform_point2(Vec2::from(corner.position)));
        Vertex::new(position, Vec2::from(corner.uv) + shift, modulate(corner.colour, colour))
    });
    let texture = if quad.textured { object.texture.and_then(|texture| textures.get(texture)) } else { None };
    canvas.strip(layer, Paint { texture, additive: quad.additive, fine: false }, &corners);
}

/// One colour by another, as the PSP's light has an object's corners by the colour
/// it is given (0x30D84): each part by the other's over 255.
pub fn modulate(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    std::array::from_fn(|part| (a[part] as u32 * b[part] as u32 / 255) as u8)
}

/// What plays a layout's sets (0x308A8). It keeps a track for every group a set
/// has given it; its tracks are numbered in the order they are drawn in.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    set: usize,
    /// In the order they were made in.
    tracks: Vec<State>,
    /// The order they are drawn in (0x2F27C).
    order: Vec<usize>,
}

/// One of a player's tracks: the track of a set it is playing, and how far it is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    track: Track,
    /// Its number among the tracks of the set it was made for.
    made_as: usize,
    playing: Playing,
}

/// A track as it is played: for each channel the frame it has come to and its value.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    frames: [f32; SLOTS],
    values: [f32; SLOTS],
    /// A clip that goes round is let run to its end instead (bit 1 of the track's
    /// flags).
    finishing: bool,
}

impl Player {
    /// The set it was last given.
    pub fn set(&self) -> usize {
        self.set
    }

    /// How many tracks it has (0x320FC).
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    fn state(&self, track: usize) -> Option<&State> {
        self.order.get(track).and_then(|&slot| self.tracks.get(slot))
    }

    /// The group a track moves (0x31BA8).
    pub fn group(&self, track: usize) -> Option<usize> {
        self.state(track).map(|state| state.track.group as usize)
    }

    /// Which of its tracks moves a group: the first that does.
    pub fn track_of(&self, group: usize) -> Option<usize> {
        (0..self.len()).find(|&track| self.group(track) == Some(group))
    }

    /// A track's number among those the player has for its group (0x31BE4): the
    /// first a set had for the group is nought, the next one, and so on.
    pub fn number_in_group(&self, track: usize) -> Option<usize> {
        let &slot = self.order.get(track)?;
        let group = self.tracks.get(slot)?.track.group;
        Some(self.tracks[..slot].iter().filter(|state| state.track.group == group).count())
    }

    /// Whether a track is left out when all of the player is drawn (0x31B48).
    pub fn hidden(&self, track: usize) -> bool {
        self.state(track).is_none_or(|state| state.track.hidden)
    }

    /// Plays a set (0x319BC). Each of the set's tracks takes over the track the
    /// player has for its group, the first of the set's for a group the first the
    /// player has for it and so on, and starts it anew; a group the player has no
    /// track for yet is given one. The tracks of groups the set does not have are
    /// left as they are, playing what they were.
    pub fn play(&mut self, layout: &Layout, set: usize) {
        self.set = set;
        let tracks = layout.tracks_of(set);
        for (index, track) in tracks.iter().enumerate() {
            let nth = tracks[..index].iter().filter(|earlier| earlier.group == track.group).count();
            let had = self.tracks.iter().enumerate().filter(|(_, state)| state.track.group == track.group).map(|(slot, _)| slot).nth(nth);
            let slot = match had {
                Some(slot) => {
                    // One that is to play the clip it has keeps its delay, its flags
                    // and its offset as well (0x2F194).
                    if self.tracks[slot].track.clip != track.clip {
                        self.tracks[slot].track = *track;
                    }
                    slot
                }
                None => {
                    self.tracks.push(State { track: *track, made_as: index, playing: Playing::AT_START });
                    self.tracks.len() - 1
                }
            };
            let state = &mut self.tracks[slot];
            state.playing.rewind(layout.clip(&state.track));
        }
        // The order they are drawn in is made anew (0x2F27C, 0x2F534): each in turn
        // is put at the place its number in its set says, which the track already
        // there and those after it give up.
        self.order.clear();
        for (slot, state) in self.tracks.iter().enumerate() {
            let place = self.tracks.get(state.made_as).map_or(0, |at_its_number| at_its_number.made_as);
            self.order.insert(place.min(self.order.len()), slot);
        }
    }

    /// Back to the start (0x2E5A8): every channel at its first key.
    pub fn rewind(&mut self, layout: &Layout) {
        for state in &mut self.tracks {
            state.playing.rewind(layout.clip(&state.track));
        }
    }

    /// One track back to its start (0x2E664), the others going on as they are.
    pub fn rewind_track(&mut self, layout: &Layout, track: usize) {
        if let Some(state) = self.order.get(track).and_then(|&slot| self.tracks.get_mut(slot)) {
            state.playing.rewind(layout.clip(&state.track));
        }
    }

    /// On by so many frames (0x2E680). Says whether every track has come to the end
    /// of its clip; one whose clip goes round counts as having.
    pub fn advance(&mut self, layout: &Layout, frames: f32) -> bool {
        let mut done = true;
        for state in &mut self.tracks {
            done &= state.playing.advance(layout.clip(&state.track), state.track.delay as f32, frames);
        }
        done
    }

    /// Straight to the end (0x2ED30): every channel at its last key.
    pub fn finish(&mut self, layout: &Layout) {
        for state in &mut self.tracks {
            state.playing.values = AT_REST;
            state.playing.frames = [0.0; SLOTS];
            for (channel, keys) in layout.clip(&state.track) {
                if let Some(last) = keys.last() {
                    state.playing.frames[channel.slot()] = last.frame as f32;
                    state.playing.values[channel.slot()] = last.value;
                }
            }
        }
    }

    /// To a frame of the clips (0x2EB38), whatever the tracks' delays.
    pub fn set_frame(&mut self, layout: &Layout, frame: f32) {
        for state in &mut self.tracks {
            for (channel, keys) in layout.clip(&state.track) {
                let slot = channel.slot();
                state.playing.frames[slot] = frame;
                // The frame of a key itself belongs to the stretch before it.
                let stretch = keys.windows(2).find(|pair| (pair[0].frame as f32) < frame && frame <= pair[1].frame as f32);
                if let Some([from, to]) = stretch {
                    state.playing.values[slot] = between(from, to, frame);
                }
            }
        }
    }

    /// The furthest frame any channel has come to (0x2E934).
    pub fn frame(&self) -> f32 {
        self.tracks.iter().flat_map(|state| state.playing.frames).fold(0.0, f32::max)
    }

    /// Whether every track has played its clip through (0x2EF1C).
    pub fn finished(&self, layout: &Layout) -> bool {
        (0..self.len()).all(|track| self.track_finished(layout, track))
    }

    /// Whether a track has played its clip through (0x2EFA4): each channel is past
    /// its keys, up to the first that marks where the clip goes round.
    pub fn track_finished(&self, layout: &Layout, track: usize) -> bool {
        let Some(state) = self.state(track) else { return true };
        layout.clip(&state.track).iter().all(|(channel, keys)| {
            let before_loop = keys.iter().take_while(|key| key.mark == Mark::None);
            before_loop.into_iter().all(|key| state.playing.frames[channel.slot()] >= (key.frame + state.track.delay as u32) as f32)
        })
    }

    /// A player of `set` that has been playing for so many frames.
    pub fn at(layout: &Layout, set: usize, frames: f32) -> Player {
        let mut player = layout.player(set);
        player.advance(layout, frames);
        player
    }

    /// Lets clips that go round run to their ends instead, or go round again.
    pub fn let_finish(&mut self, finishing: bool) {
        for state in &mut self.tracks {
            state.playing.finishing = finishing;
        }
    }

    /// The value a track has for a channel now.
    pub fn value(&self, track: usize, channel: Channel) -> Option<f32> {
        self.state(track).map(|state| state.playing.values[channel.slot()])
    }
}

impl Playing {
    /// A track that has not begun: everything as it is at rest.
    const AT_START: Playing = Playing { frames: [0.0; SLOTS], values: AT_REST, finishing: false };

    /// Back to the start of a clip (0x2F6CC): at rest, but for the channels the
    /// clip has, which are at their first keys.
    fn rewind(&mut self, clip: &[(Channel, Vec<Key>)]) {
        self.frames = [0.0; SLOTS];
        self.values = AT_REST;
        for (channel, keys) in clip {
            if let Some(first) = keys.first() {
                self.values[channel.slot()] = first.value;
            }
        }
    }

    /// One track on by `step` frames (0x2FA54).
    fn advance(&mut self, clip: &[(Channel, Vec<Key>)], delay: f32, step: f32) -> bool {
        let mut done = true;
        for (channel, keys) in clip {
            let slot = channel.slot();
            let frame = (self.frames[slot] + step).max(0.0);
            self.frames[slot] = frame;
            let Some(last) = keys.last() else { continue };
            let mut time = (frame - delay).max(0.0);
            if time < last.frame as f32 {
                done = false;
            } else {
                match loop_start(keys).filter(|_| last.mark == Mark::LoopEnd && !self.finishing) {
                    Some(start) => time = wrapped(time, start, last.frame as f32),
                    None => {
                        self.values[slot] = last.value;
                        continue;
                    }
                }
            }
            for pair in keys.windows(2) {
                let (from, to) = (&pair[0], &pair[1]);
                if time < to.frame as f32 {
                    // Before its first key a channel keeps the value it has.
                    if time >= from.frame as f32 {
                        self.values[slot] = between(from, to, time);
                    }
                    break;
                }
                // A key in the middle of a clip that sends it back: the channel's own
                // frame goes back with it, and its value waits for the next step.
                if to.mark == Mark::LoopEnd && !self.finishing {
                    if let Some(start) = loop_start(keys) {
                        self.frames[slot] = wrapped(time, start, to.frame as f32);
                        break;
                    }
                }
            }
        }
        done
    }

    /// Where the track has its group and in what colour (0x2E70C); nothing when it
    /// has no alpha at all.
    fn placed(&self, track: &Track) -> Option<(Affine2, [u8; 4])> {
        let value = |channel: Channel| self.values[channel.slot()];
        if value(Channel::Alpha) <= 0.0 {
            return None;
        }
        let moved = Affine2::from_translation(Vec2::new(value(Channel::X) + track.offset[0], value(Channel::Y) + track.offset[1]))
            * Affine2::from_angle(value(Channel::Turn).to_radians())
            * Affine2::from_scale(Vec2::new(value(Channel::ScaleX), value(Channel::ScaleY)));
        let colour = [Channel::Red, Channel::Green, Channel::Blue, Channel::Alpha].map(|channel| value(channel) as u8);
        Some((moved, colour))
    }
}

/// The frame of the first key that marks where a clip goes back to.
fn loop_start(keys: &[Key]) -> Option<f32> {
    keys.iter().find(|key| key.mark == Mark::LoopStart).map(|key| key.frame as f32)
}

/// A time past the end of a stretch that goes round, brought back into it.
fn wrapped(time: f32, start: f32, end: f32) -> f32 {
    if end > start { (time - start) % (end - start) + start } else { start }
}

/// The value between two keys at `frame` (0x2F8B8): evenly, or eased by half a turn
/// of the sine (0x2DD80).
fn between(from: &Key, to: &Key, frame: f32) -> f32 {
    let along = (frame - from.frame as f32) / (to.frame - from.frame) as f32;
    let eased = match from.ease {
        Ease::Even => along,
        Ease::In => along + EASE_IN / EASE_SCALE * (along * std::f32::consts::PI).sin(),
        Ease::Out => along + EASE_OUT / EASE_SCALE * (along * std::f32::consts::PI).sin(),
    };
    from.value + (to.value - from.value) * eased
}

#[cfg(test)]
mod tests {
    use super::*;
    use brs_formats::layout::{Clip, Corner, Quad};

    fn key(frame: u32, value: f32) -> Key {
        Key { frame, value, ease: Ease::Even, mark: Mark::None }
    }

    fn corner(x: f32, y: f32) -> Corner {
        Corner { position: [x, y], uv: [x / 10.0, y / 10.0], colour: [255, 255, 255, 128] }
    }

    /// A layout of one rectangle of 10 by 10 at (100, 50), in a group of its own,
    /// with sets that play the clips given.
    fn layout(clips: Vec<Clip>, tracks: Vec<Track>) -> Layout {
        let quad = Quad { corners: [corner(0.0, 0.0), corner(0.0, 10.0), corner(10.0, 0.0), corner(10.0, 10.0)], textured: false, smooth: true, additive: false };
        let object = Object { texture: None, position: [100, 50], size: [10, 10], quad: Some(quad) };
        Layout {
            objects: Objects { objects: vec![object], textures: 0 },
            motions: Motions { sets: vec![tracks], clips },
            groups: Groups { colours: vec![[255; 4]], groups: vec![vec![0]] },
            textures: Vec::new(),
        }
    }

    fn track(clip: u16, delay: u16) -> Track {
        Track { group: 0, clip, delay, hidden: false, offset: [0.0, 0.0] }
    }

    #[test]
    fn a_clip_is_played_key_to_key_and_stays_at_its_last() {
        let clip = vec![(Channel::X, vec![key(0, 0.0), key(10, 20.0), key(20, 60.0)])];
        let layout = layout(vec![clip], vec![track(0, 0)]);
        let mut player = layout.player(0);
        assert_eq!(player.value(0, Channel::X), Some(0.0));
        assert_eq!(player.value(0, Channel::ScaleX), Some(1.0));
        assert!(!player.advance(&layout, 5.0));
        assert_eq!(player.value(0, Channel::X), Some(10.0));
        assert!(!player.advance(&layout, 10.0));
        assert_eq!(player.value(0, Channel::X), Some(40.0));
        assert!(!player.finished(&layout));
        assert!(player.advance(&layout, 10.0));
        assert_eq!(player.value(0, Channel::X), Some(60.0));
        assert!(player.finished(&layout));
        assert_eq!(player.frame(), 25.0);
        // Back to the start, straight to the end, and to a frame.
        player.rewind(&layout);
        assert_eq!((player.value(0, Channel::X), player.frame()), (Some(0.0), 0.0));
        player.finish(&layout);
        assert_eq!((player.value(0, Channel::X), player.frame()), (Some(60.0), 20.0));
        player.set_frame(&layout, 15.0);
        assert_eq!(player.value(0, Channel::X), Some(40.0));
    }

    #[test]
    fn a_track_waits_out_its_delay_and_an_ease_starts_or_ends_slowly() {
        let eased = |ease| vec![(Channel::Y, vec![Key { frame: 0, value: 0.0, ease, mark: Mark::None }, key(10, 100.0)])];
        let layout = layout(vec![eased(Ease::In), eased(Ease::Out)], vec![track(0, 4), track(1, 0)]);
        let mut player = layout.player(0);
        // Four frames in, the first track has not begun.
        player.advance(&layout, 4.0);
        assert_eq!(player.value(0, Channel::Y), Some(0.0));
        player.advance(&layout, 5.0);
        // Half way through its frames it is well short of half way there.
        let slow = player.value(0, Channel::Y).unwrap();
        assert!((slow - (50.0 - 100.0 / 314.159 * 100.0)).abs() < 1e-3, "{slow}");
        let mut player = layout.player(0);
        player.advance(&layout, 2.5);
        let quick = player.value(1, Channel::Y).unwrap();
        assert!((quick - (25.0 + 100.0 / 314.159 * 100.0 * (0.25 * std::f32::consts::PI).sin())).abs() < 1e-3, "{quick}");
        assert!(quick > 45.0);
        player.advance(&layout, 4.0);
        let slow = player.value(0, Channel::Y).unwrap();
        assert!(slow < 5.0 && slow > 0.0, "{slow}");
    }

    #[test]
    fn a_clip_that_goes_round_goes_back_to_its_mark() {
        let marked = |frame, value, mark| Key { frame, value, ease: Ease::Even, mark };
        let clip = vec![(Channel::Alpha, vec![key(4, 0.0), marked(8, 255.0, Mark::LoopStart), key(44, 255.0), key(58, 100.0), marked(82, 255.0, Mark::LoopEnd)])];
        let layout = layout(vec![clip], vec![track(0, 0)]);
        let mut player = layout.player(0);
        assert_eq!(player.value(0, Channel::Alpha), Some(0.0));
        player.advance(&layout, 6.0);
        assert_eq!(player.value(0, Channel::Alpha), Some(127.5));
        player.advance(&layout, 76.0);
        // Frame 82 is frame 8 again, and frame 82 + 50 is frame 58.
        assert_eq!(player.value(0, Channel::Alpha), Some(255.0));
        player.advance(&layout, 50.0);
        assert_eq!(player.value(0, Channel::Alpha), Some(100.0));
        // Having played up to its mark it counts as played through.
        assert!(player.finished(&layout));
        // Told to finish, it stops at its last key.
        player.let_finish(true);
        player.advance(&layout, 500.0);
        assert_eq!(player.value(0, Channel::Alpha), Some(255.0));
    }

    #[test]
    fn an_object_is_drawn_where_its_track_has_it_in_its_tracks_colour() {
        let clip = vec![
            (Channel::X, vec![key(0, 5.0)]),
            (Channel::ScaleX, vec![key(0, 2.0)]),
            (Channel::Turn, vec![key(0, 90.0)]),
            (Channel::Green, vec![key(0, 51.0)]),
        ];
        let layout = layout(vec![clip, Vec::new()], vec![track(0, 0)]);
        let player = layout.player(0);
        let (moved, colour) = layout.placed(&player, 0).unwrap();
        assert_eq!(colour, [255, 51, 255, 255]);
        // The corner at (10, 0) is scaled to (20, 0), turned to (0, 20), moved by 5.
        let turned = moved.transform_point2(Vec2::new(10.0, 0.0));
        assert!((turned - Vec2::new(5.0, 20.0)).length() < 1e-4, "{turned}");

        let mut canvas = Canvas::default();
        layout.draw(&mut canvas, 0, &player, &Look::tinted([255, 255, 255, 128]));
        let mut expected = Canvas::default();
        let at = |x: f32, y: f32, u: f32, v: f32| Vertex::new(Vec2::new(100.0 + x, 50.0 + y), Vec2::new(u, v), [255, 51, 255, 64]);
        expected.strip(0, Paint::PLAIN, &[at(5.0, 0.0, 0.0, 0.0), at(-5.0, 0.0, 0.0, 1.0), at(5.0, 20.0, 1.0, 0.0), at(-5.0, 20.0, 1.0, 1.0)]);
        let close = |a: &Vertex, b: &Vertex| (a.position - b.position).length() < 1e-3 && a.uv == b.uv && a.colour == b.colour;
        let (drawn, wanted) = (canvas.vertices(), expected.vertices());
        assert_eq!(drawn.len(), wanted.len());
        assert!(drawn.iter().zip(wanted).all(|(a, b)| close(a, b)), "{drawn:?}");

        // A track with no alpha, or one that is not drawn, draws nothing.
        let unseen = vec![(Channel::Alpha, vec![key(0, 0.0)])];
        let layout = super::tests::layout(vec![unseen], vec![track(0, 0)]);
        let mut canvas = Canvas::default();
        layout.draw(&mut canvas, 0, &layout.player(0), &Look::default());
        assert!(canvas.vertices().is_empty());
    }

    #[test]
    fn the_inner_matrix_moves_the_corners_before_the_track_and_the_outer_one_after() {
        let clip = vec![(Channel::ScaleY, vec![key(0, 2.0)])];
        let layout = layout(vec![clip], vec![track(0, 0)]);
        let player = layout.player(0);
        // The corner at (10, 10): 3 down by the inner matrix, doubled down by the
        // track, put at the object's place, and all of it 100 right by the outer.
        let look = Look { inner: Affine2::from_translation(Vec2::new(0.0, 3.0)), outer: Affine2::from_translation(Vec2::new(100.0, 0.0)), ..default() };
        let mut canvas = Canvas::default();
        layout.draw_member(&mut canvas, 0, &player, 0, 0, &look);
        let last = canvas.vertices().last().unwrap().position;
        assert_eq!(last, Vec2::new(100.0 + 100.0 + 10.0, 50.0 + 26.0));
        // The group has one object: there is no second to draw.
        let mut canvas = Canvas::default();
        layout.draw_member(&mut canvas, 0, &player, 0, 1, &look);
        assert!(canvas.vertices().is_empty());
        assert_eq!(layout.track_of(0, 0), Some(0));
        assert_eq!(layout.track_of(0, 1), None);
    }

    #[test]
    fn a_set_is_as_long_as_its_last_key_and_a_track_as_its_keys_before_a_mark() {
        let marked = |frame, mark| Key { frame, value: 0.0, ease: Ease::Even, mark };
        let round = vec![(Channel::Alpha, vec![key(4, 0.0), marked(8, Mark::LoopStart), marked(82, Mark::LoopEnd)])];
        let plain = vec![(Channel::X, vec![key(0, 0.0), key(22, 1.0)]), (Channel::Y, vec![key(16, 0.0)])];
        let layout = layout(vec![round, plain], vec![track(0, 14), track(1, 0)]);
        assert_eq!(layout.length(0), 96.0);
        assert_eq!((layout.track_length(0, 0), layout.track_length(0, 1)), (18.0, 22.0));
        let mut player = Player::at(&layout, 0, 20.0);
        assert!(player.track_finished(&layout, 0) && !player.track_finished(&layout, 1));
        player.advance(&layout, 2.0);
        assert!(player.finished(&layout));
        // A clip on its own, at a frame.
        let (moved, colour) = layout.clip_at(1, 11.0).unwrap();
        assert_eq!((moved.translation, colour), (Vec2::new(0.5, 0.0), [255; 4]));
    }

    #[test]
    fn a_set_played_after_another_starts_its_own_groups_anew_and_leaves_the_rest() {
        let quad = Quad { corners: [corner(0.0, 0.0), corner(0.0, 10.0), corner(10.0, 0.0), corner(10.0, 10.0)], textured: false, smooth: true, additive: false };
        let object = Object { texture: None, position: [0, 0], size: [10, 10], quad: Some(quad) };
        let moving = |to: f32| vec![(Channel::X, vec![key(0, 0.0), key(10, to)])];
        let of = |group: u16, clip: u16, delay: u16| Track { group, clip, delay, hidden: false, offset: [0.0, 0.0] };
        let layout = Layout {
            objects: Objects { objects: vec![object.clone(), object.clone(), object], textures: 0 },
            motions: Motions {
                // The first set has two groups; the second only the second of them;
                // the third brings a group the first two have not, as its first track.
                sets: vec![vec![of(0, 0, 0), of(1, 0, 0)], vec![of(1, 1, 0)], vec![of(2, 1, 0), of(1, 1, 6)]],
                clips: vec![moving(10.0), moving(50.0)],
            },
            groups: Groups { colours: vec![[255; 4]; 3], groups: vec![vec![0], vec![1], vec![2]] },
            textures: Vec::new(),
        };
        let mut player = layout.player(0);
        player.advance(&layout, 10.0);
        assert_eq!((player.len(), player.value(0, Channel::X), player.value(1, Channel::X)), (2, Some(10.0), Some(10.0)));
        player.play(&layout, 1);
        // The second group is at the start of its new clip; the first is where it was.
        assert_eq!((player.set(), player.len()), (1, 2));
        assert_eq!((player.value(0, Channel::X), player.value(1, Channel::X)), (Some(10.0), Some(0.0)));
        player.advance(&layout, 5.0);
        assert_eq!((player.value(0, Channel::X), player.value(1, Channel::X)), (Some(10.0), Some(25.0)));
        assert!(!player.finished(&layout));
        assert!(player.advance(&layout, 5.0) && player.finished(&layout));
        // A track told to play the clip it has starts it anew with the delay it had,
        // and a group that is new to the player is drawn where its number in its set
        // says: here first.
        player.play(&layout, 2);
        assert_eq!((player.group(0), player.group(1), player.group(2)), (Some(2), Some(0), Some(1)));
        assert_eq!((player.track_of(1), player.track_of(7)), (Some(2), None));
        player.advance(&layout, 5.0);
        assert_eq!((player.value(0, Channel::X), player.value(2, Channel::X)), (Some(25.0), Some(25.0)));
    }

    #[test]
    fn colours_are_multiplied_part_by_part() {
        assert_eq!(modulate([255, 128, 0, 255], [255, 255, 255, 128]), [255, 128, 0, 128]);
        assert_eq!(modulate([200, 100, 50, 255], [128, 128, 128, 255]), [100, 50, 25, 255]);
    }
}
