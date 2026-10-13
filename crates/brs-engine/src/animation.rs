//! Skeletal animation playback.
//!
//! This follows the original's animation player (the slot functions around 0xA9664 in
//! the executable) closely, because the game's own logic is written against it: states
//! hold a pose by looping on a single frame, play "from frame 38 to 46", or wait for the
//! time to reach the start of the loop range. Times are in frames of 1/60 s.
//!
//! A node's pose is worked out the way the original's evaluator does it (0x20614): the
//! parts its animation gives it are applied one on top of the other, and the node is
//! then placed under its parent as a whole matrix. The nodes' entities are not nested
//! for this reason: a node may take its parent's scale back out, which no chain of
//! translation, rotation and scale under a scaled parent can say.

use std::sync::{Arc, OnceLock};

use bevy::math::{Affine3A, Mat3A};
use bevy::prelude::*;
use brs_formats::insa::{self, Order, Part};

use crate::model::{Skeleton, NO_ID};

pub use brs_formats::insa::FRAMES_PER_SECOND;

/// Lengths below this count as none (the original's 1e-5 at 0x20768).
const SMALL: f32 = 1e-5;

/// What a model's node gives an animation to work on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Joint {
    /// Where the node sits under its parent in the model.
    pub rest: Vec3,
    /// Whether the node's place is fitted to the model: see [`fit`].
    pub fitted: bool,
}

/// A node as an animation poses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Posed {
    /// The node relative to its parent.
    pub local: Affine3A,
    /// What the animation's rotation and translation came to before any blend. This,
    /// and not the pose shown, is what the next blend starts from (the original keeps
    /// them in the node's pose record and copies them at 0x23030).
    pub rotation: Quat,
    pub translation: Vec3,
    /// The node's own scale: what a child takes back out.
    pub scale: Vec3,
}

impl Posed {
    /// A node no animation drives: as the model has it.
    pub fn rest(rest: Transform) -> Posed {
        Posed { local: rest.compute_affine(), rotation: rest.rotation, translation: rest.translation, scale: rest.scale }
    }
}

/// Where a blend comes from, and how far along it is.
#[derive(Clone, Copy)]
pub(crate) struct Blend {
    weight: f32,
    rotation: Quat,
    translation: Vec3,
}

/// An animation and its lengths in frames.
pub struct Clip {
    pub animation: insa::Animation,
    frames: f32,
    loop_range: (f32, f32),
}

impl Clip {
    pub fn new(animation: insa::Animation) -> Arc<Clip> {
        let frames = animation.frames() as f32;
        let (from, to) = animation.loop_frames();
        Arc::new(Clip { animation, frames, loop_range: (from as f32, to as f32) })
    }

    /// Length in frames.
    pub fn frames(&self) -> f32 {
        self.frames
    }

    /// The frames a looping playback repeats.
    pub fn loop_range(&self) -> (f32, f32) {
        self.loop_range
    }

    pub fn seconds(&self) -> f32 {
        self.frames / FRAMES_PER_SECOND
    }

    /// Where `frame` is on the scale of the animation's key times.
    pub(crate) fn time_at(&self, frame: f32) -> f32 {
        self.animation.start as f32 + (frame * insa::UNITS_PER_FRAME as f32).trunc()
    }

    /// The clip's node `index` at `frame`, or `None` if the clip does not drive it.
    ///
    /// `parent_scale` is the scale of the node's parent, for the part that takes it
    /// back out; `carried` the rotation and translation the node was last given, which
    /// stand where the clip has no such part; `blend` what to come from.
    pub(crate) fn pose(
        &self,
        index: usize,
        frame: f32,
        joint: &Joint,
        parent_scale: Option<Vec3>,
        carried: (Quat, Vec3),
        blend: Option<Blend>,
    ) -> Option<Posed> {
        let tracks = self.animation.nodes.get(index).filter(|tracks| !tracks.masked)?;
        let at = self.time_at(frame);
        let vector = |channels: &[insa::Channel; 3]| Vec3::new(channels[0].at(at), channels[1].at(at), channels[2].at(at));
        let mut posed = Posed { local: Affine3A::IDENTITY, rotation: carried.0, translation: carried.1, scale: Vec3::ONE };
        for part in &tracks.parts {
            let applied = match part {
                Part::Scale(channels) => {
                    posed.scale = vector(channels);
                    Affine3A::from_scale(posed.scale)
                }
                Part::UniformScale(channel) => {
                    posed.scale = Vec3::splat(channel.at(at));
                    Affine3A::from_scale(posed.scale)
                }
                Part::Rotation(track) => {
                    let sampled = track.at(at);
                    posed.rotation = Quat::from_array(sampled);
                    Affine3A::from_quat(match blend {
                        Some(blend) => Quat::from_array(insa::slerp(blend.rotation.to_array(), sampled, blend.weight)),
                        None => posed.rotation,
                    })
                }
                Part::Angles { order, channels } => {
                    let angles = vector(channels);
                    let (x, y, z) =
                        (Quat::from_rotation_x(angles.x), Quat::from_rotation_y(angles.y), Quat::from_rotation_z(angles.z));
                    Affine3A::from_quat(match order {
                        Order::Xyz => z * y * x,
                        Order::Zxy => y * x * z,
                    })
                }
                Part::UnscaleParent => match parent_scale {
                    Some(scale) => Affine3A::from_scale(scale.max(Vec3::splat(SMALL)).recip()),
                    None => continue,
                },
                Part::Translation(channels) => {
                    let mut translation = vector(channels);
                    if joint.fitted {
                        translation = fit(translation, Vec3::from(tracks.reference), joint.rest);
                    }
                    posed.translation = translation;
                    if let Some(blend) = blend {
                        translation = blend.translation + (translation - blend.translation) * blend.weight;
                    }
                    Affine3A::from_translation(translation)
                }
            };
            posed.local = applied * posed.local;
        }
        Some(posed)
    }
}

/// Which of an animation's nodes drives each node of a model, the way the original
/// finds them (0x21454). With as many nodes as the model they go by their place in
/// the table. Otherwise the two trees are walked together and matched by identifier:
/// an animation's node finds the model's by looking along the nodes beside it, one
/// without an identifier takes a model node without one, and what the animation has
/// nothing for stays as the model has it.
pub fn match_nodes(skeleton: &Skeleton, animation: &[insa::NodeTracks]) -> Vec<Option<usize>> {
    let count = skeleton.ids.len();
    if animation.len() == count {
        return (0..count).map(Some).collect();
    }
    let mut matched = vec![None; count];
    let Some(first) = animation.first() else { return matched };
    // The walk starts at the model's node the animation's first names.
    let mut model = if first.id == NO_ID { 0 } else { skeleton.ids.iter().position(|&id| id == first.id).unwrap_or(0) };
    let mut node = 0;
    while node < animation.len() && model < count {
        let tracks = &animation[node];
        if tracks.id == NO_ID {
            if skeleton.ids[model] != NO_ID {
                // Nothing here for it: on to what follows it and all under it.
                match tracks.next {
                    Some(next) => node = next,
                    None => break,
                }
                continue;
            }
        } else {
            while skeleton.ids[model] != tracks.id {
                match skeleton.after[model] {
                    Some(after) => model = after,
                    // The original gives the whole walk up here.
                    None => return matched,
                }
            }
        }
        matched[model] = Some(node);
        // Under a node the animation has nothing below, the model's own are left.
        model = match skeleton.after[model] {
            Some(after) if !tracks.has_child && skeleton.has_child[model] => after,
            _ => model + 1,
        };
        node += 1;
    }
    matched
}

/// A node's translation from an animation made on one skeleton, for a model whose
/// node sits elsewhere under its parent (0x20E1C): the turn that takes the place the
/// animation's skeleton gives the node to the place the animation asks for is applied
/// to the model's own, and the length changes by the same factor. On the skeleton
/// the animation was made on this changes nothing.
///
/// The original does it for the nodes of a skinned model's skeleton whose parent is
/// one too.
pub fn fit(translation: Vec3, reference: Vec3, rest: Vec3) -> Vec3 {
    let (length, reference_length) = (translation.length(), reference.length());
    if length <= SMALL || reference_length <= SMALL {
        return translation;
    }
    let (towards, from, own) = (translation / length, reference / reference_length, rest.normalize_or_zero());
    let scale = length * rest.length() / reference_length;
    let apart = towards - from;
    let axis = if apart.length() > SMALL { from.cross(apart.normalize()) } else { from.cross(towards) };
    if axis.length() < SMALL {
        return own * scale;
    }
    let cosine = from.dot(towards);
    let angle = if cosine > 1.0 {
        0.0
    } else if cosine < -1.0 {
        std::f32::consts::PI
    } else {
        cosine.acos()
    };
    Quat::from_axis_angle(axis.normalize(), angle) * own * scale
}

/// A matrix as a translation, a rotation and a scale. A node scaled away to nothing
/// on an axis has no direction left there; one is made up, as nothing of it shows.
pub(crate) fn transform_of(matrix: &Affine3A) -> Transform {
    /// An axis shorter than this has been scaled away.
    const GONE: f32 = 1e-6;
    let (x, y, z) = (Vec3::from(matrix.matrix3.x_axis), Vec3::from(matrix.matrix3.y_axis), Vec3::from(matrix.matrix3.z_axis));
    let mut scale = Vec3::new(x.length(), y.length(), z.length());
    if matrix.matrix3.determinant() < 0.0 {
        scale.x = -scale.x;
    }
    let unit = |axis: Vec3, length: f32| (length.abs() > GONE).then(|| axis / length);
    let axes = match (unit(x, scale.x), unit(y, scale.y), unit(z, scale.z)) {
        (Some(x), Some(y), Some(z)) => Some((x, y, z)),
        (Some(x), Some(y), None) => Some((x, y, x.cross(y))),
        (None, Some(y), Some(z)) => Some((y.cross(z), y, z)),
        (Some(x), None, Some(z)) => Some((x, z.cross(x), z)),
        _ => None,
    };
    let rotation = axes.map_or(Quat::IDENTITY, |(x, y, z)| Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize());
    Transform { translation: Vec3::from(matrix.translation), rotation, scale }
}

/// The part of a slot's state that moves as time passes.
#[derive(Clone, Copy, Default)]
struct Playhead {
    time: f32,
    /// Frames to wait before the time starts to advance.
    delay: f32,
    /// How many frames of the blend from the previous pose have passed.
    blend_elapsed: f32,
    loops: u32,
}

/// One animation playing on a skeleton. A model has one slot for the body and may have
/// more layered on top (the player's hair has its own).
#[derive(Default)]
pub struct Slot {
    clip: Option<Arc<Clip>>,
    head: Playhead,
    looping: bool,
    /// Where playback wraps: on reaching the second frame it jumps back to the first.
    /// With both equal it stops there.
    range: (f32, f32),
    /// Frames over which the pose blends in from `from`: each node's rotation and
    /// translation as the animation before had them.
    blend: f32,
    from: Vec<(Quat, Vec3)>,
    /// Which of the clip's nodes drives each of the model's: see [`match_nodes`].
    /// Worked out when the clip is first posed, which is when the model is at hand.
    nodes: OnceLock<Vec<Option<usize>>>,
}

impl Slot {
    /// Switches to `clip` at frame 0, set to play through once. With `blend` above
    /// zero the pose blends in from `from` (the skeleton's pose at this moment) over
    /// that many frames.
    fn set(&mut self, clip: Option<&Arc<Clip>>, blend: f32, from: &[Posed]) {
        *self = Slot::default();
        let Some(clip) = clip else { return };
        self.clip = Some(clip.clone());
        self.range = (clip.frames, clip.frames);
        if blend > 0.0 && !from.is_empty() {
            self.blend = blend;
            self.from = from.iter().map(|posed| (posed.rotation, posed.translation)).collect();
        }
    }

    pub fn clip(&self) -> Option<&Arc<Clip>> {
        self.clip.as_ref()
    }

    pub fn is(&self, clip: &Arc<Clip>) -> bool {
        self.clip.as_ref().is_some_and(|current| Arc::ptr_eq(current, clip))
    }

    /// Has the pose be the clip's own from now on, whatever it was blending in from.
    pub fn drop_blend(&mut self) {
        self.blend = 0.0;
        self.from.clear();
    }

    /// Current frame.
    pub fn time(&self) -> f32 {
        self.head.time
    }

    /// Jumps to `frame`, kept within the clip.
    pub fn set_time(&mut self, frame: f32) {
        if let Some(clip) = &self.clip {
            self.head.time = frame.max(0.0).min(clip.frames);
        }
    }

    /// Length of the current clip in frames; zero with no clip.
    pub fn length(&self) -> f32 {
        self.clip.as_ref().map_or(0.0, |clip| clip.frames)
    }

    /// Holds the clip still for `frames` before it starts to play.
    pub fn set_delay(&mut self, frames: f32) {
        if self.clip.is_some() {
            self.head.delay = frames;
        }
    }

    /// Repeats the clip's own loop range.
    pub fn loop_clip(&mut self) {
        if let Some(clip) = &self.clip {
            self.looping = true;
            self.range = clip.loop_range;
        }
    }

    /// Repeats the frames from `first` to `last`. With both the same, playback runs up
    /// to that frame and holds it.
    pub fn loop_between(&mut self, first: f32, last: f32) {
        if let Some(clip) = &self.clip {
            self.looping = true;
            self.range = (first.max(0.0), last.min(clip.frames));
        }
    }

    /// Stops looping: playback runs on to the end of the clip and stays there.
    pub fn play_out(&mut self) {
        if let Some(clip) = &self.clip {
            self.looping = false;
            self.range = (clip.frames, clip.frames);
        }
    }

    /// The frames playback wraps between; both are the clip's length when not looping.
    pub fn range(&self) -> (f32, f32) {
        self.range
    }

    pub fn looping(&self) -> bool {
        self.looping
    }

    /// How many times playback has wrapped since the clip was set.
    pub fn loops(&self) -> u32 {
        self.head.loops
    }

    /// True once playback has reached the frame it stops on.
    pub fn at_end(&self) -> bool {
        self.clip.is_none() || self.head.time >= self.range.0
    }

    /// Where the playhead will be after `frames` more.
    fn ahead(&self, frames: f32) -> Playhead {
        let mut head = self.head;
        if frames <= 0.0 || self.clip.is_none() {
            return head;
        }
        if self.blend > 0.0 && head.blend_elapsed < self.blend {
            head.blend_elapsed = (head.blend_elapsed + frames).min(self.blend);
        }
        if head.delay > 0.0 {
            // The step on which the delay runs out is not played either.
            head.delay = (head.delay - frames).max(0.0);
            return head;
        }
        head.time += frames;
        let (first, last) = self.range;
        while head.time >= last {
            if first < last {
                head.time = head.time - last + first;
                head.loops += 1;
            } else {
                head.time = last;
                break;
            }
        }
        head
    }

    fn advance(&mut self, frames: f32) {
        self.head = self.ahead(frames);
    }

    /// The pose this slot gives node `index` with its playhead at `head`; `None` if it
    /// leaves the node alone. While a blend runs the node's rotation and translation
    /// come over from what they were; its scale does not blend (0x209FC, 0x21040).
    fn pose(
        &self,
        head: Playhead,
        skeleton: &Skeleton,
        index: usize,
        joint: &Joint,
        parent_scale: Option<Vec3>,
        carried: (Quat, Vec3),
    ) -> Option<Posed> {
        let clip = self.clip.as_ref()?;
        let node = (*self.nodes.get_or_init(|| match_nodes(skeleton, &clip.animation.nodes)).get(index)?)?;
        let weight = if self.blend > 0.0 { (head.blend_elapsed / self.blend).clamp(0.0, 1.0) } else { 1.0 };
        let blend = self
            .from
            .get(index)
            .filter(|_| weight < 1.0)
            .map(|&(rotation, translation)| Blend { weight, rotation, translation });
        clip.pose(node, head.time, joint, parent_scale, carried, blend)
    }
}

/// An extra turn of one node: the original's node overrides (applied at 0xA9CCC),
/// which change the node's matrix in the model's space once it has been posed and
/// carry everything under the node along (0xA92F0).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Turn {
    /// In the node's own frame: its matrix times the turn (override mode 2).
    Own(Quat),
    /// In the model's space, about the node (override mode 4).
    Model(Quat),
}

/// Plays clips on the [`Skeleton`] of the same entity.
#[derive(Component)]
pub struct Animator {
    slots: Vec<Slot>,
    /// Each node as of the last step, which a blend starts from.
    pose: Vec<Posed>,
    /// Extra turns given to single nodes on top of their animation: node index and
    /// turn, in node order.
    twists: Vec<(usize, Turn)>,
    /// How single nodes are turned in the model's space whatever their
    /// animation says, as of the step before and of the last: node index and
    /// the two turns, in node order. A chain's nodes are (see [`crate::chain`]).
    facings: Vec<(usize, [Quat; 2])>,
    /// Playback rate; 1.0 is normal speed.
    pub speed: f32,
    pub paused: bool,
    /// Only the body's slot plays and poses: what is layered on it stands still
    /// and drives nothing, until it is let go on again.
    pub layers_off: bool,
}

impl Default for Animator {
    fn default() -> Self {
        Animator::new()
    }
}

impl Animator {
    /// An animator with one slot, for the body.
    pub fn new() -> Self {
        Animator::with_slots(1)
    }

    pub fn with_slots(count: usize) -> Self {
        Animator {
            slots: std::iter::repeat_with(Slot::default).take(count.max(1)).collect(),
            pose: Vec::new(),
            twists: Vec::new(),
            facings: Vec::new(),
            speed: 1.0,
            paused: false,
            layers_off: false,
        }
    }

    /// How many of its slots play and pose now.
    fn slots_on(&self) -> usize {
        if self.layers_off { 1 } else { self.slots.len() }
    }

    /// Turns node `index`, and with it everything that hangs off it, by `rotation` on
    /// top of its animation. The rotation is in the model's space, not the node's own,
    /// and happens about the node: the original's way of bending a character towards
    /// what it aims at (its node override mode 4, set at 0xAA868).
    pub fn set_twist(&mut self, index: usize, rotation: Quat) {
        self.set_turn(index, Turn::Model(rotation));
    }

    /// Turns node `index` by `rotation` in its own frame on top of its animation:
    /// the rotation is multiplied onto the node's own (the original's node override
    /// mode 2, set at 0xAA730). A turret turning on its mount is done this way.
    pub fn set_own_turn(&mut self, index: usize, rotation: Quat) {
        self.set_turn(index, Turn::Own(rotation));
    }

    /// A node has one extra turn at most: a new one takes the place of the old.
    fn set_turn(&mut self, index: usize, turn: Turn) {
        match self.twists.binary_search_by_key(&index, |(node, _)| *node) {
            Ok(at) => self.twists[at].1 = turn,
            Err(at) => self.twists.insert(at, (index, turn)),
        }
    }

    pub fn clear_twists(&mut self) {
        self.twists.clear();
    }

    /// Has node `index` turned so in the model's space, whatever its animation
    /// says of its turn: as of the step before and of the last. It stays where
    /// its animation and what it hangs from put it, and what hangs off it is
    /// carried along.
    pub fn set_facing(&mut self, index: usize, turns: [Quat; 2]) {
        match self.facings.binary_search_by_key(&index, |(node, _)| *node) {
            Ok(at) => self.facings[at].1 = turns,
            Err(at) => self.facings.insert(at, (index, turns)),
        }
    }

    pub fn clear_facings(&mut self) {
        self.facings.clear();
    }

    /// Every node in the model's space as of the last step, as its animation
    /// and its twists have it and no more; nothing before the first step.
    pub fn animated(&self, skeleton: &Skeleton) -> Vec<Affine3A> {
        self.placed(skeleton, &self.pose, None)
    }

    /// Advances every slot by `frames`.
    pub fn step(&mut self, frames: f32) {
        let on = self.slots_on();
        for slot in self.slots.iter_mut().take(on) {
            slot.advance(frames);
        }
    }

    /// Where node `index` is in the model's space right now, twists included.
    pub fn model_space(&self, skeleton: &Skeleton, index: usize) -> Transform {
        let pose = self.locals(skeleton, 0.0);
        self.placed(skeleton, &pose, Some(1.0)).get(index).map_or(Transform::IDENTITY, transform_of)
    }

    /// Every node relative to its parent with each slot `ahead` frames past its
    /// playhead, before any twist. A node no slot drives stays as the model has it.
    fn locals(&self, skeleton: &Skeleton, ahead: f32) -> Vec<Posed> {
        let heads: Vec<Playhead> = self.slots.iter().map(|slot| slot.ahead(ahead)).collect();
        let mut pose: Vec<Posed> = Vec::with_capacity(skeleton.rest.len());
        for (index, &rest) in skeleton.rest.iter().enumerate() {
            let joint = Joint { rest: rest.translation, fitted: skeleton.fitted.get(index).copied().unwrap_or(false) };
            // Parents come before their children, so the parent's scale is this step's.
            let parent_scale = skeleton.parents[index].map(|parent| pose[parent].scale);
            let mut posed = Posed::rest(rest);
            let carried = self.pose.get(index).map_or((rest.rotation, rest.translation), |last| (last.rotation, last.translation));
            for (slot, &head) in self.slots.iter().zip(&heads).take(self.slots_on()) {
                if let Some(driven) = slot.pose(head, skeleton, index, &joint, parent_scale, carried) {
                    posed = driven;
                }
            }
            pose.push(posed);
        }
        pose
    }

    /// Every node of a pose from [`Animator::locals`] in the model's space, with the
    /// twists applied; and with `between`, the nodes that are turned of
    /// themselves turned so, that far from the step before to the last.
    fn placed(&self, skeleton: &Skeleton, pose: &[Posed], between: Option<f32>) -> Vec<Affine3A> {
        let mut placed: Vec<Affine3A> = Vec::with_capacity(pose.len());
        // The twists are in node order too.
        let mut twists = self.twists.iter().peekable();
        let mut facings = self.facings.iter().filter(|_| between.is_some()).peekable();
        for (index, posed) in pose.iter().enumerate() {
            let turn = twists.next_if(|(node, _)| *node == index).map(|&(_, turn)| turn);
            let mut local = posed.local;
            if let Some(Turn::Own(rotation)) = turn {
                local = local * Affine3A::from_quat(rotation);
            }
            let mut matrix = match skeleton.parents[index] {
                Some(parent) => placed[parent] * local,
                None => local,
            };
            if let Some(Turn::Model(rotation)) = turn {
                matrix.matrix3 = Mat3A::from_quat(rotation) * matrix.matrix3;
            }
            if let Some(&(_, [from, to])) = facings.next_if(|(node, _)| *node == index) {
                // As big along each of its ways as it was.
                let turned = Mat3A::from_quat(from.slerp(to, between.unwrap_or(1.0)));
                let sizes = Vec3::new(matrix.matrix3.x_axis.length(), matrix.matrix3.y_axis.length(), matrix.matrix3.z_axis.length());
                matrix.matrix3 = turned * Mat3A::from_diagonal(sizes);
            }
            placed.push(matrix);
        }
        placed
    }

    /// The transforms of the nodes' entities for a pose from [`Animator::locals`]:
    /// each node in the model's space, since the entities all hang off the model's.
    fn transforms(&self, skeleton: &Skeleton, pose: &[Posed], between: f32) -> Vec<Transform> {
        self.placed(skeleton, pose, Some(between)).iter().map(transform_of).collect()
    }

    pub fn slot(&self, index: usize) -> &Slot {
        &self.slots[index]
    }

    pub fn slot_mut(&mut self, index: usize) -> &mut Slot {
        &mut self.slots[index]
    }

    /// Switches slot `index` to `clip` at once.
    pub fn set_slot(&mut self, index: usize, clip: &Arc<Clip>) {
        self.slots[index].set(Some(clip), 0.0, &[]);
    }

    /// Switches slot `index` to `clip`, what it drives coming over from the
    /// current pose in `frames`.
    pub fn set_slot_blended(&mut self, index: usize, clip: &Arc<Clip>, frames: f32) {
        self.slots[index].set(Some(clip), frames, &self.pose);
    }

    /// Switches the body to `clip` at once: frame 0, playing through once.
    pub fn set(&mut self, clip: &Arc<Clip>) {
        self.set_slot(0, clip);
    }

    /// Switches the body to `clip`, blending from the current pose over `frames`.
    pub fn set_blended(&mut self, clip: &Arc<Clip>, frames: f32) {
        self.slots[0].set(Some(clip), frames, &self.pose);
    }

    /// The body slot.
    pub fn body(&self) -> &Slot {
        &self.slots[0]
    }

    pub fn body_mut(&mut self) -> &mut Slot {
        &mut self.slots[0]
    }

    /// Frames one step of `seconds` advances this animator by.
    fn frames_in(&self, seconds: f32) -> f32 {
        if self.paused { 0.0 } else { seconds * FRAMES_PER_SECOND * self.speed }
    }
}

/// Advances every [`Animator`] by one step of game time. Runs wherever the game's logic
/// runs (see [`crate::EnginePlugin`]), after it.
pub fn advance(time: Res<Time>, mut animators: Query<(&mut Animator, &Skeleton)>) {
    for (mut animator, skeleton) in &mut animators {
        let frames = animator.frames_in(time.delta_secs());
        animator.step(frames);
        animator.pose = animator.locals(skeleton, 0.0);
    }
}

/// Writes each skeleton's pose as of the last step to its node entities.
pub fn pose(animators: Query<(&Animator, &Skeleton)>, mut transforms: Query<&mut Transform>) {
    for (animator, skeleton) in &animators {
        let pose = animator.transforms(skeleton, &animator.pose, 1.0);
        for (&entity, pose) in skeleton.nodes.iter().zip(&pose) {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                *transform = *pose;
            }
        }
    }
}

/// Like [`pose`], for games whose logic runs in fixed steps: each model is drawn in
/// the pose its last step left it in, as its place is (`crate::step`).
pub fn pose_between_steps(
    animators: Query<(&Animator, &Skeleton)>,
    mut transforms: Query<&mut Transform>,
) {
    for (animator, skeleton) in &animators {
        if animator.pose.is_empty() {
            continue;
        }
        let pose = animator.transforms(skeleton, &animator.pose, 1.0);
        for (&entity, pose) in skeleton.nodes.iter().zip(&pose) {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                *transform = *pose;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(frames: u32, loop_range: (u32, u32)) -> Arc<Clip> {
        let unit = insa::UNITS_PER_FRAME;
        Clip::new(insa::Animation {
            start: 0,
            end: frames * unit,
            loop_start: loop_range.0 * unit,
            loop_end: loop_range.1 * unit,
            nodes: Vec::new(),
        })
    }

    fn weight(slot: &Slot) -> f32 {
        slot.head.blend_elapsed / slot.blend
    }

    /// A clip of 60 frames whose nodes are made of `parts`.
    fn posing(nodes: Vec<insa::NodeTracks>) -> Arc<Clip> {
        let unit = insa::UNITS_PER_FRAME;
        Clip::new(insa::Animation { start: 0, end: 60 * unit, loop_start: 0, loop_end: 60 * unit, nodes })
    }

    fn node(id: u8, has_child: bool, parts: Vec<Part>) -> insa::NodeTracks {
        insa::NodeTracks { id, has_child, parts, ..Default::default() }
    }

    fn constants(x: f32, y: f32, z: f32) -> [insa::Channel; 3] {
        [insa::Channel::constant(x), insa::Channel::constant(y), insa::Channel::constant(z)]
    }

    fn turn(rotation: Quat) -> Part {
        Part::Rotation(insa::Rotation::constant(rotation.to_array()))
    }

    /// A skeleton of nodes given by identifier, parent and rest, with what a model's
    /// file would say of each.
    fn skeleton(nodes: &[(u8, Option<usize>, Transform)]) -> Skeleton {
        let parents: Vec<Option<usize>> = nodes.iter().map(|node| node.1).collect();
        let under = |node: usize, of: usize| {
            let mut at = parents[node];
            while let Some(parent) = at {
                if parent == of {
                    return true;
                }
                at = parents[parent];
            }
            false
        };
        Skeleton {
            nodes: Vec::new(),
            rest: nodes.iter().map(|node| node.2).collect(),
            fitted: vec![false; nodes.len()],
            has_child: (0..nodes.len()).map(|node| parents.contains(&Some(node))).collect(),
            after: (0..nodes.len()).map(|node| (node + 1..nodes.len()).find(|&other| !under(other, node))).collect(),
            ids: nodes.iter().map(|node| node.0).collect(),
            parents,
        }
    }

    const QUARTER: f32 = std::f32::consts::FRAC_PI_2;
    const NOTHING_CARRIED: (Quat, Vec3) = (Quat::IDENTITY, Vec3::ZERO);

    #[test]
    fn a_scene_joint_is_its_parts_one_on_top_of_the_other() {
        // Scale, the keyed turn, the joint's own turn, the parent's scale taken out,
        // the joint's place: each applies to what the ones before it made.
        let clip = posing(vec![node(
            NO_ID,
            false,
            vec![
                Part::UniformScale(insa::Channel::constant(2.0)),
                Part::Angles { order: Order::Xyz, channels: constants(0.0, 0.0, QUARTER) },
                Part::Angles { order: Order::Xyz, channels: constants(QUARTER, 0.0, 0.0) },
                Part::UnscaleParent,
                Part::Translation(constants(10.0, 0.0, 0.0)),
            ],
        )]);
        let posed = clip.pose(0, 0.0, &Joint::default(), Some(Vec3::splat(4.0)), NOTHING_CARRIED, None).unwrap();
        // X doubled, turned up onto Y, then over onto Z, shrunk by the parent's 4.
        assert!(posed.local.transform_point3(Vec3::X).distance(Vec3::new(10.0, 0.0, 0.5)) < 1e-5);
        assert_eq!(posed.scale, Vec3::splat(2.0));
        // Without a parent there is no scale to take out.
        let root = clip.pose(0, 0.0, &Joint::default(), None, NOTHING_CARRIED, None).unwrap();
        assert!(root.local.transform_point3(Vec3::X).distance(Vec3::new(10.0, 0.0, 2.0)) < 1e-5);
    }

    #[test]
    fn angles_turn_in_the_order_their_part_says() {
        let about = |order| {
            let clip = posing(vec![node(NO_ID, false, vec![Part::Angles { order, channels: constants(QUARTER, 0.0, QUARTER) }])]);
            clip.pose(0, 0.0, &Joint::default(), None, NOTHING_CARRIED, None).unwrap().local.transform_point3(Vec3::X)
        };
        // About X first, which leaves the X axis alone; then Z turns it onto Y.
        assert!(about(Order::Xyz).distance(Vec3::Y) < 1e-5);
        // About Z first, onto Y; then X turns that onto Z.
        assert!(about(Order::Zxy).distance(Vec3::Z) < 1e-5);
    }

    #[test]
    fn a_node_under_a_squashed_parent_keeps_its_shape() {
        let rest = Transform::IDENTITY;
        let skeleton = skeleton(&[(NO_ID, None, rest), (NO_ID, Some(0), rest)]);
        let clip = posing(vec![
            node(NO_ID, true, vec![Part::Scale(constants(1.0, 0.5, 1.0)), Part::Translation(constants(0.0, 0.0, 0.0))]),
            node(
                NO_ID,
                false,
                vec![turn(Quat::from_rotation_z(QUARTER)), Part::UnscaleParent, Part::Translation(constants(0.0, 10.0, 0.0))],
            ),
        ]);
        let mut animator = Animator::new();
        animator.set(&clip);
        let child = animator.model_space(&skeleton, 1);
        // Its place is squashed with the parent; it is not, and nothing is skewed.
        assert!(child.translation.distance(Vec3::new(0.0, 5.0, 0.0)) < 1e-5, "{}", child.translation);
        assert!(child.scale.distance(Vec3::ONE) < 1e-5, "{}", child.scale);
        assert!((child.rotation * Vec3::X).distance(Vec3::Y) < 1e-5);
        assert!(animator.model_space(&skeleton, 0).scale.distance(Vec3::new(1.0, 0.5, 1.0)) < 1e-5);
    }

    #[test]
    fn a_translation_is_fitted_to_the_models_own_bones() {
        let reference = Vec3::new(10.0, 0.0, 0.0);
        // The animation turns the bone up and makes it a fifth longer.
        let asked = Vec3::new(0.0, 12.0, 0.0);
        // On the skeleton it was made on, that is what it gets.
        assert!(fit(asked, reference, reference).distance(asked) < 1e-4);
        // A bone twice as long turns the same way and is still twice as long.
        assert!(fit(asked, reference, reference * 2.0).distance(asked * 2.0) < 1e-4);
        // A bone lying along the axis of that turn is not turned by it.
        assert!(fit(asked, reference, Vec3::new(0.0, 0.0, 5.0)).distance(Vec3::new(0.0, 0.0, 6.0)) < 1e-4);
        // An animation that leaves the bone where it was gives the model's own place.
        assert!(fit(reference, reference, Vec3::new(3.0, 4.0, 0.0)).distance(Vec3::new(3.0, 4.0, 0.0)) < 1e-4);
        // Only joints of a skeleton are fitted.
        let clip = posing(vec![node(NO_ID, false, vec![Part::Translation(constants(0.0, 12.0, 0.0))])]);
        let mut tracks = clip.animation.clone();
        tracks.nodes[0].reference = reference.to_array();
        let clip = Clip::new(tracks);
        let joint = |fitted| Joint { rest: reference * 2.0, fitted };
        let placed = |fitted| clip.pose(0, 0.0, &joint(fitted), None, NOTHING_CARRIED, None).unwrap().translation;
        assert!(placed(false).distance(asked) < 1e-4);
        assert!(placed(true).distance(asked * 2.0) < 1e-4);
    }

    #[test]
    fn an_animation_with_fewer_nodes_finds_the_models_by_identifier() {
        let rest = Transform::IDENTITY;
        // A body with a part hung on it (9, with 10 under it) that the animation,
        // made for the plain body, knows nothing of.
        let skeleton = skeleton(&[
            (NO_ID, None, rest),
            (1, Some(0), rest),
            (2, Some(1), rest),
            (9, Some(1), rest),
            (10, Some(3), rest),
            (3, Some(1), rest),
        ]);
        let animation =
            [node(NO_ID, true, Vec::new()), node(1, true, Vec::new()), node(2, false, Vec::new()), node(3, false, Vec::new())];
        assert_eq!(match_nodes(&skeleton, &animation), [Some(0), Some(1), Some(2), None, None, Some(3)]);
        // With as many nodes as the model they simply go in order.
        let same = [node(5, false, Vec::new()), node(6, false, Vec::new())];
        let pair = self::skeleton(&[(NO_ID, None, rest), (NO_ID, Some(0), rest)]);
        assert_eq!(match_nodes(&pair, &same), [Some(0), Some(1)]);
    }

    #[test]
    fn a_blend_comes_from_what_the_animation_before_gave() {
        let skeleton = skeleton(&[(NO_ID, None, Transform::IDENTITY)]);
        let clip = posing(vec![node(
            NO_ID,
            false,
            vec![
                Part::UniformScale(insa::Channel::constant(3.0)),
                turn(Quat::from_rotation_y(QUARTER)),
                Part::Translation(constants(10.0, 0.0, 0.0)),
            ],
        )]);
        let before = [Posed::rest(Transform::IDENTITY)];
        let mut slot = Slot::default();
        slot.set(Some(&clip), 10.0, &before);
        slot.advance(5.0);
        let posed = slot.pose(slot.head, &skeleton, 0, &Joint::default(), None, NOTHING_CARRIED).unwrap();
        // Half way there: an eighth of a turn and half the distance. The scale does
        // not blend.
        let shown = transform_of(&posed.local);
        assert!(shown.translation.distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);
        assert!(shown.rotation.angle_between(Quat::from_rotation_y(QUARTER / 2.0)) < 1e-3);
        assert!(shown.scale.distance(Vec3::splat(3.0)) < 1e-4);
        // What the next blend would start from is where the animation itself is.
        assert!(posed.rotation.angle_between(Quat::from_rotation_y(QUARTER)) < 1e-3);
        assert_eq!(posed.translation, Vec3::new(10.0, 0.0, 0.0));
    }

    #[test]
    fn a_node_scaled_away_still_has_a_transform() {
        let flat = Affine3A::from_scale_rotation_translation(Vec3::new(1.0, 0.0, 1.0), Quat::from_rotation_z(0.3), Vec3::X);
        let transform = transform_of(&flat);
        assert!(transform.rotation.is_finite() && transform.scale.is_finite());
        assert!(transform.scale.distance(Vec3::new(1.0, 0.0, 1.0)) < 1e-5);
        assert!(transform.rotation.angle_between(Quat::from_rotation_z(0.3)) < 1e-3);
        let gone = transform_of(&Affine3A::from_scale(Vec3::ZERO));
        assert_eq!((gone.rotation, gone.scale), (Quat::IDENTITY, Vec3::ZERO));
    }

    #[test]
    fn a_twist_turns_a_node_and_what_hangs_off_it() {
        let rest = Transform::IDENTITY;
        let skeleton = skeleton(&[(NO_ID, None, rest), (NO_ID, Some(0), rest)]);
        let clip = posing(vec![
            node(NO_ID, true, vec![Part::Translation(constants(1.0, 0.0, 0.0))]),
            node(NO_ID, false, vec![Part::Translation(constants(0.0, 0.0, -4.0))]),
        ]);
        let mut animator = Animator::new();
        animator.set(&clip);
        animator.set_twist(0, Quat::from_rotation_y(QUARTER));
        // Turned about the node itself, in the model's space: the node stays put and
        // what was in front of it is now to its left.
        assert!(animator.model_space(&skeleton, 0).translation.distance(Vec3::X) < 1e-5);
        assert!(animator.model_space(&skeleton, 1).translation.distance(Vec3::new(-3.0, 0.0, 0.0)) < 1e-5);
        animator.set_own_turn(0, Quat::from_rotation_y(-QUARTER));
        assert!(animator.model_space(&skeleton, 1).translation.distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);
    }

    #[test]
    fn plays_through_and_stops_at_the_end() {
        let mut slot = Slot::default();
        slot.set(Some(&clip(40, (0, 40))), 0.0, &[]);
        slot.advance(39.0);
        assert!(!slot.at_end());
        slot.advance(5.0);
        assert_eq!(slot.time(), 40.0);
        assert!(slot.at_end());
    }

    #[test]
    fn loops_over_the_clips_own_range() {
        let mut slot = Slot::default();
        slot.set(Some(&clip(72, (38, 46))), 0.0, &[]);
        slot.loop_clip();
        slot.advance(47.0);
        assert_eq!(slot.time(), 39.0);
        assert_eq!(slot.loops(), 1);
    }

    #[test]
    fn a_single_frame_range_holds_that_frame() {
        // How the shoot state plays one recoil: from frame 38 up to 46, then hold.
        let mut slot = Slot::default();
        slot.set(Some(&clip(72, (38, 46))), 0.0, &[]);
        slot.set_time(38.0);
        slot.loop_between(46.0, 46.0);
        slot.advance(6.0);
        assert_eq!(slot.time(), 44.0);
        assert!(!slot.at_end());
        slot.advance(6.0);
        assert_eq!(slot.time(), 46.0);
        assert!(slot.at_end());
        slot.play_out();
        assert!(!slot.at_end());
        slot.advance(100.0);
        assert_eq!(slot.time(), 72.0);
    }

    #[test]
    fn a_delay_holds_the_first_frame_while_the_blend_runs() {
        let rest = [Posed::rest(Transform::IDENTITY)];
        let mut slot = Slot::default();
        slot.set(Some(&clip(40, (0, 40))), 8.0, &rest);
        slot.set_delay(8.0);
        slot.advance(4.0);
        assert_eq!(slot.time(), 0.0);
        assert_eq!(weight(&slot), 0.5);
        slot.advance(4.0);
        assert_eq!(slot.time(), 0.0);
        assert_eq!(weight(&slot), 1.0);
        slot.advance(2.0);
        assert_eq!(slot.time(), 2.0);
    }

    #[test]
    fn looking_ahead_does_not_move_the_playhead() {
        let mut slot = Slot::default();
        slot.set(Some(&clip(40, (0, 40))), 0.0, &[]);
        slot.advance(10.0);
        assert_eq!(slot.ahead(1.5).time, 11.5);
        assert_eq!(slot.time(), 10.0);
    }
}
