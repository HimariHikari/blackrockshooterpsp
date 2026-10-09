//! The numbers a battle puts up where something lands (0x3D81C): what a hit takes
//! off (0x357E8), HP that is given back (0x35868), and what a hit she guarded takes
//! off her (0x358A8).
//!
//! Each is an entry of the signs' layout that plays a set of its own once, where
//! the camera has its place on the screen as it is put up, and is gone when the set
//! has played out (0x3CFD4). One group of the set is the number's figures: a single
//! object, drawn once for every figure from that figure's cell of a sheet of them
//! (0x3F0C4).

use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{Layout, Look, Player};
use brs_engine::model::Skeleton;

use super::actor::PcActor;
use super::enemy::Horde;
use crate::interface::layer;

/// What a number is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// What a hit takes off (the entries' kind 0).
    Damage,
    /// HP given back (kind 2).
    Heal,
    /// What a hit takes off her when she guarded it and stands (kind 3).
    Guarded,
}

impl Kind {
    /// The set of the signs' layout a number of the kind plays (0x3D938, 0x3D968,
    /// 0x3D950).
    fn set(self) -> usize {
        match self {
            Kind::Damage => 3,
            Kind::Heal => 18,
            Kind::Guarded => 24,
        }
    }

    /// The group of that set that is the figures (0x3F0C4, 0x3F394, 0x3FA40).
    fn figures(self) -> usize {
        match self {
            Kind::Damage | Kind::Heal => 17,
            Kind::Guarded => 18,
        }
    }

    /// The cells of the sheet the figures are in: the second of the table at
    /// 0x171078 for a hit's and a gift's, its fifth for a guarded hit's.
    fn cell(self) -> f32 {
        match self {
            Kind::Damage | Kind::Heal => 11.0,
            Kind::Guarded => 7.0,
        }
    }
}

/// The colours what a hit takes off is drawn by (0x3F058): off an enemy, and off
/// her. The other kinds are as their sets have them.
const OFF_AN_ENEMY: [u8; 4] = [0xe6, 0xe6, 0xe6, 0xff];
const OFF_HER: [u8; 4] = [0xe6, 0x64, 0x1e, 0xff];

/// The node of her model a number of hers is put by when nowhere is given for it
/// (0x99E14, 0x98B30).
const HER_NODE: u8 = 0;

/// A number to put up, at a place in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Asked {
    pub kind: Kind,
    pub value: i32,
    pub at: Vec3,
    /// It is of her, and no enemy.
    pub hers: bool,
}

/// One that is up.
struct Number {
    kind: Kind,
    value: i32,
    /// Where on the screen.
    at: Vec2,
    hers: bool,
    player: Player,
}

/// The numbers of a battle: those asked for and not yet put up, and those that
/// are up.
#[derive(Resource, Default)]
pub struct Numbers {
    asked: Vec<Asked>,
    up: Vec<Number>,
}

/// The figures of a number from the left, each with how far across it is from the
/// number's place (0x3F178): they stand a pixel further apart than their cells are
/// wide, about the place by half their cells, in whole pixels.
fn figures(value: i32, cell: f32) -> Vec<(i32, f32)> {
    let value = value.max(0);
    let count = value.checked_ilog10().map_or(1, |most| most as i32 + 1);
    (0..count).map(|place| (value / 10_i32.pow((count - 1 - place) as u32) % 10, (place as f32 * (cell + 1.0) - count as f32 * cell * 0.5).trunc())).collect()
}

impl Numbers {
    pub fn ask(&mut self, asked: Asked) {
        self.asked.push(asked);
    }

    /// Has those that are up go on by so many frames of the layout, and takes away
    /// those that have played out; then puts up those asked for, each where
    /// `place` has its place on the screen. One whose place is not before the
    /// camera is not put up. Says whether what a hit took off her was among
    /// those asked for, which has her HP gauge flash (0x35830 to 0x35848: a
    /// number over nought, wherever it is, of one whose numbers are not marked;
    /// every enemy's are, as they are made, 0x91E10, and hers are not, 0x99B7C).
    pub fn step(&mut self, layout: &Layout, frames: f32, place: impl Fn(Vec3) -> Option<Vec2>) -> bool {
        self.up.retain_mut(|number| !number.player.advance(layout, frames));
        let mut struck = false;
        for Asked { kind, value, at, hers } in self.asked.drain(..) {
            struck |= hers && kind == Kind::Damage && value > 0;
            if let Some(at) = place(at) {
                self.up.push(Number { kind, value, at, hers, player: layout.player(kind.set()) });
            }
        }
        struck
    }

    /// Draws them (0x3EE60): each about its place, in whole pixels (0x3EFA8), as
    /// the enemies' marks are. `sheets` are the widths of the two sheets of
    /// figures. While she has the stage those of the enemies are not drawn, as
    /// the enemies are not.
    pub fn draw(&self, canvas: &mut Canvas, layout: &Layout, sheets: [f32; 2], staged: bool) {
        for number in self.up.iter().filter(|number| number.hers || !staged) {
            let (kind, player) = (number.kind, &number.player);
            let at = number.at.trunc();
            let tint = match (kind, number.hers) {
                (Kind::Damage, false) => OFF_AN_ENEMY,
                (Kind::Damage, true) => OFF_HER,
                _ => [255; 4],
            };
            let sheet = if kind == Kind::Guarded { sheets[1] } else { sheets[0] };
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let Some(home) = layout.first_of(group).and_then(|first| layout.place(first)) else { continue };
                let look = Look { tint, inner: Affine2::from_translation(at), outer: Affine2::from_translation(-home), ..Look::default() };
                if group != kind.figures() {
                    layout.draw_track(canvas, layer::DISPLAY, player, track, &look);
                    continue;
                }
                for (figure, across) in figures(number.value, kind.cell()) {
                    let inner = Affine2::from_translation(at + Vec2::new(across, 0.0));
                    let shift = Vec2::new(kind.cell() * figure as f32 / sheet, 0.0);
                    layout.draw_track(canvas, layer::DISPLAY, player, track, &Look { inner, shift, ..look });
                }
            }
        }
    }
}

/// Takes what the enemies and she have to put up, once everybody has had a turn.
/// Hers that have no place of their own are put by a node of her model.
pub fn gather(mut numbers: ResMut<Numbers>, mut horde: ResMut<Horde>, player: Single<(&mut PcActor, &Animator, &Skeleton)>) {
    numbers.asked.append(&mut horde.numbers);
    let (mut actor, animator, skeleton) = player.into_inner();
    if actor.numbers.is_empty() {
        return;
    }
    let whole = actor.transform();
    let place = skeleton.index_of(HER_NODE).map_or(whole, |node| whole * animator.model_space(skeleton, node)).translation;
    for (kind, value, at) in std::mem::take(&mut actor.numbers) {
        numbers.ask(Asked { kind, value, at: at.unwrap_or(place), hers: true });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numbers_figures_stand_about_its_place_a_pixel_apart() {
        // Two figures of cells eleven wide: twelve apart, the first eleven left.
        assert_eq!(figures(31, 11.0), vec![(3, -11.0), (1, 1.0)]);
        // One alone is half a cell left, cut to a whole pixel.
        assert_eq!(figures(5, 11.0), vec![(5, -5.0)]);
        assert_eq!(figures(0, 11.0), vec![(0, -5.0)]);
        assert_eq!(figures(100, 11.0), vec![(1, -16.0), (0, -4.0), (0, 7.0)]);
        // The smaller figures of a guarded hit.
        assert_eq!(figures(12, 7.0), vec![(1, -7.0), (2, 1.0)]);
        // Nothing is ever below nought (0x3D890).
        assert_eq!(figures(-4, 11.0), vec![(0, -5.0)]);
    }

    fn signs() -> Option<Layout> {
        let data = crate::data::GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        Some(crate::interface::signs_layout(&data, &mut images).unwrap())
    }

    #[test]
    fn a_number_is_put_up_where_the_camera_has_its_place_and_goes_when_it_has_played() {
        let Some(layout) = signs() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut numbers = Numbers::default();
        numbers.ask(Asked { kind: Kind::Damage, value: 31, at: Vec3::new(1.0, 2.0, 3.0), hers: false });
        numbers.ask(Asked { kind: Kind::Heal, value: 7, at: Vec3::ZERO, hers: true });
        // What is not before the camera is not put up.
        numbers.step(&layout, 2.0, |at| (at != Vec3::ZERO).then_some(Vec2::new(200.4, 100.7)));
        assert_eq!(numbers.up.len(), 1);
        assert!(numbers.asked.is_empty());
        // It comes in over four frames, and is then drawn about its place: its
        // set is nothing but its figures, two of them here, each a quad.
        let mut canvas = Canvas::default();
        numbers.draw(&mut canvas, &layout, [128.0, 128.0], false);
        assert!(canvas.vertices().is_empty());
        numbers.step(&layout, 4.0, |_| None);
        numbers.draw(&mut canvas, &layout, [128.0, 128.0], false);
        let drawn = canvas.vertices();
        assert_eq!(drawn.len(), 12);
        // The second figure is twelve to the right of the first.
        assert_eq!(drawn[6].position - drawn[0].position, Vec2::new(12.0, 0.0));
        // An enemy's is not drawn while she has the stage.
        let mut hidden = Canvas::default();
        numbers.draw(&mut hidden, &layout, [128.0, 128.0], true);
        assert!(hidden.vertices().is_empty());
        // Its set plays once, thirty-two frames, and it is gone.
        let length = layout.length(Kind::Damage.set());
        assert_eq!(length, 32.0);
        numbers.step(&layout, length - 6.0, |_| None);
        assert_eq!(numbers.up.len(), 1);
        numbers.step(&layout, 4.0, |_| None);
        assert!(numbers.up.is_empty());
    }
}
