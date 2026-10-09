//! Chains: what hangs off a model and swings, as her two tails of hair do
//! (0xE9C48 to 0xEAC78).
//!
//! A character's script can describe a gimmick that is a chain (its function 1,
//! 0x10ADEC): a run of the model's nodes one under another, found by the id of
//! the first (0x1E20C, 0x1E5CC), the way each node's bone lies in the node's own
//! frame (function 4), how thick the air is to it (function 7), and for each
//! link how long its bone is, how heavy it is, how strongly it comes back to
//! where its animation has it and how much of it the air takes hold of
//! (function 3).
//!
//! Each link is a point at the far end of its bone, kept in the world, with a
//! speed. A frame of the model (0x10BDD8) has every link go on (0xEA5F0), and
//! then every node of the chain is turned so that its bone points at its link
//! (0xEA8D8):
//!
//! 1. Where the link would be is worked out, were the bone as its animation
//!    has it: the node's matrix in the world by the bone.
//! 2. A chain that is held has its links there, at rest, and that is all.
//! 3. The link is pushed towards that place, round the point it hangs from
//!    (the node itself for the first link, the link before for the others): by
//!    the link's strength, and more the wider the angle (0xE9F98).
//! 4. It is put back at its bone's length from the point it hangs from, and
//!    what it was moved by is added to its speed and to that of the link
//!    before (0xEA274).
//! 5. What its speed has along the bone is taken away (0xEA43C).
//! 6. The air slows it: by its area and the square of its speed, most when it
//!    moves across the bone and not at all along it (0xEA4B4).
//! 7. It moves by its speed (0xEA3AC).
//!
//! Nothing pulls it down: a tail hangs where its animation has it and is only
//! late in following.
//!
//! Turning the nodes starts from the node as the model's file has it under its
//! parent, not as its animation has it, and turns that about the node until the
//! bone points at the link; with a weight under one, at a point that far from
//! where the animation has the link towards where the link is.

use bevy::math::{Affine3A, Mat3A};
use bevy::prelude::*;

use crate::animation::Animator;
use crate::model::Skeleton;
use crate::step::Stepped;

/// The time a frame of the game is, as the original counts it, and a second.
const TIME_A_FRAME: f32 = 2048.0;
const TIME_A_SECOND: f32 = 61440.0;
/// Under this a length is nothing (0xE9FE4), and two ways are one when what
/// they have of one another is nearer one than this (0xEA07C).
const NOTHING: f32 = 1e-5;
const ONE_WAY: f32 = 0.99999;

/// What a script says of one link of a chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkFigures {
    /// How long its bone is; it counts without its sign where the link is kept
    /// at that length.
    pub length: f32,
    pub mass: f32,
    /// How strongly it comes back to where its animation has it.
    pub bend: f32,
    /// How much of it the air takes hold of.
    pub area: f32,
}

/// What a script says of a chain.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainFigures {
    /// The id of the first of its nodes.
    pub id: u8,
    /// The way a node's bone lies in the node's own frame.
    pub along: Vec3,
    /// How thick the air is to it.
    pub air: f32,
    pub links: Vec<LinkFigures>,
}

#[derive(Clone, Copy, Debug)]
struct Link {
    figures: LinkFigures,
    at: Vec3,
    speed: Vec3,
    /// Where its animation would have it.
    rest: Vec3,
}

/// A chain of a model.
#[derive(Clone, Debug)]
pub struct Chain {
    /// The first of its nodes; the others follow it.
    first: usize,
    along: Vec3,
    air: f32,
    links: Vec<Link>,
    /// Whether its links have been anywhere yet.
    begun: bool,
    /// How each of its nodes was turned in the model's space as of the step
    /// before and of the last.
    turns: Vec<[Quat; 2]>,
}

/// What pushes a link at `at`, which hangs from `from`, round towards `rest`
/// in so much `time` (0xE9F98).
fn push(at: Vec3, rest: Vec3, from: Vec3, bend: f32, time: f32) -> Vec3 {
    let (is, wanted) = (at - from, rest - from);
    if is.length() < NOTHING || wanted.length() < NOTHING {
        return Vec3::ZERO;
    }
    let (is, wanted) = (is.normalize(), wanted.normalize());
    let alike = is.dot(wanted);
    if !(-ONE_WAY..=ONE_WAY).contains(&alike) {
        return Vec3::ZERO;
    }
    let angle = alike.acos();
    let about = is.cross(wanted).normalize_or_zero();
    let wide = 1.0 - (1.0 - angle / std::f32::consts::PI).powi(2);
    about.cross(is) * (bend * wide * time / TIME_A_SECOND)
}

/// The turn that takes the way from `about` to `from` round to the way from
/// `about` to `to`, if there is one to speak of (0xE9DE4).
fn turn_between(from: Vec3, to: Vec3, about: Vec3) -> Option<Quat> {
    let (from, to) = (from - about, to - about);
    if from.length() < NOTHING || to.length() < NOTHING {
        return None;
    }
    let (from, to) = (from.normalize(), to.normalize());
    let alike = from.dot(to);
    if !(-ONE_WAY..=ONE_WAY).contains(&alike) {
        return None;
    }
    Some(Quat::from_axis_angle(from.cross(to).normalize_or_zero(), alike.acos()))
}

impl Chain {
    /// The chain a script describes, on a model with this skeleton: none if the
    /// model has no node of that id.
    pub fn new(figures: &ChainFigures, skeleton: &Skeleton) -> Option<Chain> {
        let first = skeleton.index_of(figures.id)?;
        // No more links than the nodes that hang one under another there.
        let nodes = skeleton.after.get(first).copied().flatten().unwrap_or(skeleton.rest.len()) - first;
        let links: Vec<Link> =
            figures.links.iter().take(nodes).map(|&figures| Link { figures, at: Vec3::ZERO, speed: Vec3::ZERO, rest: Vec3::ZERO }).collect();
        Some(Chain { first, along: figures.along, air: figures.air, turns: vec![[Quat::IDENTITY; 2]; links.len()], links, begun: false })
    }

    /// The nodes it turns.
    pub fn nodes(&self) -> std::ops::Range<usize> {
        self.first..self.first + self.links.len()
    }

    /// A frame of its links (0xEA870). `animated` has each of its nodes as the
    /// animation has it, in the world; `time` is what the model has of the
    /// frame; a chain that is `held` is put where its animation has it.
    pub fn step(&mut self, animated: &[Affine3A], time: f32, held: bool) {
        // The original's links begin at the middle of the world and are flung
        // from there in their first frame; here they begin where they belong.
        let held = held || !self.begun;
        self.begun = true;
        let seconds = time / TIME_A_SECOND;
        for index in 0..self.links.len() {
            let node = animated[index];
            let before = index.checked_sub(1).map(|before| self.links[before].at);
            let link = &mut self.links[index];
            let figures = link.figures;
            link.rest = node.transform_point3(self.along * figures.length);
            if held {
                (link.at, link.speed) = (link.rest, Vec3::ZERO);
                continue;
            }
            let from = before.unwrap_or(node.translation.into());
            link.speed += push(link.at, link.rest, from, figures.bend, time) / figures.mass;
            // Back at its length from what it hangs from; what that moved it
            // by goes to its speed, and to that of the link before.
            let was = link.at;
            link.at = from + (link.at - from).normalize_or_zero() * figures.length.abs();
            let moved = (link.at - was) * (figures.mass * seconds);
            link.speed += moved / figures.mass;
            let along = (link.at - from).normalize_or_zero();
            if index > 0 {
                let before = &mut self.links[index - 1];
                before.speed += moved / before.figures.mass;
            }
            let link = &mut self.links[index];
            link.speed -= along * along.dot(link.speed);
            // The air.
            let fast = link.speed.length();
            let against = (-link.speed).normalize_or_zero();
            let across = 1.0 - against.dot(along).abs();
            let drag = self.air * figures.area * 0.5 * fast * fast;
            link.speed += against * (across * drag * (time / TIME_A_FRAME)) / figures.mass;
            link.at += link.speed * seconds;
        }
    }

    /// Its nodes in the world, each turned so that its bone points at its link
    /// (0xEABB0). `above` is what the first of them hangs from, in the world;
    /// `rest` has each of its nodes as the model's file has it under its
    /// parent; `weight` is how much of where the links are counts against
    /// where the animation has them.
    pub fn turned(&self, above: Affine3A, rest: &[Affine3A], weight: f32) -> Vec<Affine3A> {
        let mut turned: Vec<Affine3A> = Vec::with_capacity(self.links.len());
        for (index, link) in self.links.iter().enumerate() {
            let aim = link.rest + (link.at - link.rest) * weight;
            let parent = turned.last().copied().unwrap_or(above);
            let mut node = parent * rest[index];
            let tip = node.transform_point3(self.along * link.figures.length);
            if let Some(turn) = turn_between(tip, aim, node.translation.into()) {
                node.matrix3 = Mat3A::from_quat(turn) * node.matrix3;
            }
            turned.push(node);
        }
        turned
    }
}

/// On a model's root entity, with its [`Animator`] and [`Skeleton`]: its chains,
/// and what the game says of them each step.
#[derive(Component)]
pub struct Chains {
    chains: Vec<Chain>,
    /// How much of where the links are counts: nothing of it has the model as
    /// its animation has it.
    pub weight: f32,
    /// The links are put where the animation has them, at rest.
    pub held: bool,
    /// The chains are left as they hang: they neither go on nor turn anything.
    pub off: bool,
    /// What the model has of a frame, as the original counts time: 2048 in a
    /// frame of the game.
    pub time: f32,
}

impl Chains {
    pub fn new(chains: Vec<Chain>) -> Chains {
        Chains { chains, weight: 1.0, held: false, off: false, time: TIME_A_FRAME }
    }
}

/// How a node's matrix is turned, whatever it is scaled by.
fn turn_of(matrix: &Affine3A) -> Quat {
    let (_, turn, _) = matrix.to_scale_rotation_translation();
    turn
}

/// A step of every model's chains, once its animation has had its step: the
/// links go on, and the animator is told how the chains' nodes are turned.
pub fn swing(mut models: Query<(&mut Chains, &mut Animator, &Skeleton, Option<&Stepped>)>) {
    for (mut chains, mut animator, skeleton, stepped) in &mut models {
        // Off, its nodes are as their animation has them; and while the model
        // has no frames (a menu is up) they are left as they were.
        if chains.off {
            animator.clear_facings();
            continue;
        }
        if animator.paused {
            continue;
        }
        // Where the model is in the world as of this step, and where its
        // animation has its nodes.
        let model = stepped.map_or(Affine3A::IDENTITY, |stepped| stepped.current.compute_affine());
        let animated = animator.animated(skeleton);
        if animated.is_empty() {
            continue;
        }
        let back = model.inverse();
        let (time, held, weight) = (chains.time, chains.held, chains.weight);
        for chain in &mut chains.chains {
            let nodes = chain.nodes();
            let in_world: Vec<Affine3A> = animated[nodes.clone()].iter().map(|&node| model * node).collect();
            chain.step(&in_world, time, held);
            let above = skeleton.parents[chain.first].map_or(model, |parent| model * animated[parent]);
            let rest: Vec<Affine3A> = skeleton.rest[nodes.clone()].iter().map(Transform::compute_affine).collect();
            let turned = chain.turned(above, &rest, weight);
            for ((node, turn), kept) in nodes.zip(&turned).zip(&mut chain.turns) {
                *kept = [kept[1], turn_of(&(back * *turn))];
                animator.set_facing(node, *kept);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(links: usize) -> Chain {
        let figures = LinkFigures { length: 20.0, mass: 0.01, bend: 10.0, area: 0.001 };
        Chain {
            first: 0,
            along: Vec3::X,
            air: 0.02,
            links: vec![Link { figures, at: Vec3::ZERO, speed: Vec3::ZERO, rest: Vec3::ZERO }; links],
            begun: false,
            turns: vec![[Quat::IDENTITY; 2]; links],
        }
    }

    /// Three nodes one under another along x, twenty apart, the first at `at`.
    fn animated(at: Vec3) -> Vec<Affine3A> {
        (0..3).map(|index| Affine3A::from_translation(at + Vec3::X * 20.0 * index as f32)).collect()
    }

    #[test]
    fn a_chain_begins_where_its_animation_has_it_and_stays_while_nothing_moves() {
        let mut chain = chain(3);
        let nodes = animated(Vec3::new(5.0, 100.0, 0.0));
        for _ in 0..30 {
            chain.step(&nodes, TIME_A_FRAME, false);
        }
        for (index, link) in chain.links.iter().enumerate() {
            let rest = Vec3::new(25.0 + 20.0 * index as f32, 100.0, 0.0);
            assert!(link.at.abs_diff_eq(rest, 1e-3) && link.speed.length() < 1e-3, "{index}: {} {}", link.at, link.speed);
        }
        // Turned to its links it is as the file has it under what it hangs from.
        let rest = vec![Affine3A::from_translation(Vec3::X * 20.0); 3];
        let turned = chain.turned(Affine3A::from_translation(Vec3::new(-15.0, 100.0, 0.0)), &rest, 1.0);
        assert!(turned[2].translation.abs_diff_eq(Vec3::new(45.0, 100.0, 0.0).into(), 1e-3), "{}", turned[2].translation);
        assert!(turn_of(&turned[2]).abs_diff_eq(Quat::IDENTITY, 1e-4));
    }

    #[test]
    fn a_chain_is_late_in_following_and_comes_back_to_rest() {
        let mut chain = chain(3);
        chain.step(&animated(Vec3::ZERO), TIME_A_FRAME, false);
        // What it hangs from is thirty higher all at once: the links keep
        // their lengths, are behind, and come back in time.
        let up = animated(Vec3::Y * 30.0);
        chain.step(&up, TIME_A_FRAME, false);
        let first = chain.links[0].at;
        assert!(((first - Vec3::Y * 30.0).length() - 20.0).abs() < 0.5, "{first}");
        assert!(first.y < 29.0, "the first link should hang behind: {first}");
        assert!(((chain.links[1].at - first).length() - 20.0).abs() < 0.5);
        // Only the air takes its swing from it, and less the slower it goes:
        // it swings on about where it belongs, less and less.
        let mut furthest = |frames: usize| {
            let mut furthest: f32 = 0.0;
            for _ in 0..frames {
                chain.step(&up, TIME_A_FRAME, false);
                furthest = furthest.max(chain.links[2].at.distance(Vec3::new(60.0, 30.0, 0.0)));
            }
            furthest
        };
        let (at_first, _, later) = (furthest(30), furthest(240), furthest(30));
        assert!(at_first > 10.0 && later < at_first / 2.0, "{at_first} {later}");
    }

    #[test]
    fn a_node_is_turned_about_itself_until_its_bone_points_at_its_link() {
        let mut chain = chain(1);
        chain.step(&animated(Vec3::ZERO)[..1], TIME_A_FRAME, false);
        // The link is straight over the node: the bone, which lay along x,
        // stands up, and the node stays where it hangs.
        chain.links[0].at = Vec3::Y * 20.0;
        let rest = [Affine3A::from_translation(Vec3::X * 7.0)];
        let turned = chain.turned(Affine3A::from_translation(Vec3::new(-7.0, 0.0, 0.0)), &rest, 1.0);
        assert!(turned[0].translation.abs_diff_eq(Vec3::ZERO.into(), 1e-4));
        assert!(turned[0].transform_vector3(Vec3::X).abs_diff_eq(Vec3::Y, 1e-4));
        // With half the weight it points half way there from where its
        // animation has the link, which is along x.
        let half = chain.turned(Affine3A::from_translation(Vec3::new(-7.0, 0.0, 0.0)), &rest, 0.5);
        let way = half[0].transform_vector3(Vec3::X);
        assert!((way.x - way.y).abs() < 1e-4 && way.x > 0.7, "{way}");
        // A chain that is held is where its animation has it.
        chain.step(&animated(Vec3::ZERO)[..1], TIME_A_FRAME, true);
        assert_eq!((chain.links[0].at, chain.links[0].speed), (Vec3::X * 20.0, Vec3::ZERO));
    }

    #[test]
    fn the_push_towards_rest_is_across_the_bone_and_more_the_wider_the_angle() {
        // A link over what it hangs from, to be along x: pushed along x.
        let quarter = push(Vec3::Y * 20.0, Vec3::X * 20.0, Vec3::ZERO, 10.0, TIME_A_FRAME);
        assert!(quarter.abs_diff_eq(Vec3::X * (10.0 * 0.75 / 30.0), 1e-5), "{quarter}");
        // Nearly opposite it is pushed with nearly all its strength; where it
        // is to be, or straight opposite, not at all.
        let wide = push(Vec3::new(-20.0, 1.0, 0.0), Vec3::X * 20.0, Vec3::ZERO, 10.0, TIME_A_FRAME);
        assert!(wide.length() > 0.33 && wide.length() < 0.334, "{wide}");
        assert_eq!(push(Vec3::X * 5.0, Vec3::X * 20.0, Vec3::ZERO, 10.0, TIME_A_FRAME), Vec3::ZERO);
        assert_eq!(push(Vec3::X * -5.0, Vec3::X * 20.0, Vec3::ZERO, 10.0, TIME_A_FRAME), Vec3::ZERO);
    }
}
