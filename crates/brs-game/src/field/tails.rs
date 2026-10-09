//! Her tails: the two chains of her hair, and how much they swing.
//!
//! Her script (`FLD/FCHR/FCHR_P_BRS00.BIN/00.sscr`, and the same in her other
//! suits', ten in all) describes two gimmicks that are chains, of four nodes
//! from node 100 and of three from node 101 (see [`brs_engine::chain`]). The
//! engine has them go on with every frame of her model (0x10BDD8), unless her
//! gimmicks are off (her flag 0x9D, which a scene sets for its cast), with two
//! things her script says each frame:
//!
//! - how much of the swing counts (its function 0xA with 0: the resource's
//!   + 0x14, which goes to every chain as its weight, 0x10BE54);
//! - whether the chains are held where her animation has them (function 8
//!   with 2: the resource's bit 2, 0x10BE64 and 0xEACC0).
//!
//! `set_status` (called as her status is set, 0x10A800 and 0x10C67C) has the
//! swing whole and the chains let go for any status but standing. `update_time`
//! (0xFCFE4: with her status, the time she has been in it and the frame's time)
//! does nothing in any other status; standing, the swing is the square of what
//! is left of the first four seconds (0x3C000 of her time), and after those
//! the chains are held and the swing is nothing. So her tails fly as she runs,
//! settle as she stops, and are where her standing animation has them once she
//! has stood four seconds: only the air stops a chain, and it would never
//! quite hang still.
//!
//! A status set again while she is in it starts her time over without letting
//! the chains go; the port sees only a status that changes.

use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::chain::{Chain, Chains};
use brs_engine::model::Skeleton;

use super::assets::CharacterAssets;
use super::scene::{flag, status, FULL_PACE};
use super::{Actor, FieldState};

/// The time standing over which the swing goes out: 120 frames.
const SETTLES_IN: i32 = 120 * FULL_PACE;

/// On a model that has chains: the status its character was last in and for
/// how long, and what its script has said of the chains.
#[derive(Component)]
pub struct Tails {
    status: i32,
    time: i32,
    held: bool,
    weight: f32,
}

impl Tails {
    fn new(status: i32) -> Tails {
        Tails { status, time: 0, held: false, weight: 1.0 }
    }

    /// A frame of its character, in `status` with so much `time` of the frame.
    fn frame(&mut self, status: i32, time: i32) {
        if status != self.status {
            (self.status, self.time) = (status, 0);
            if status != status::STAND {
                (self.held, self.weight) = (false, 1.0);
            }
        }
        if status == status::STAND {
            let stood = self.time + time;
            if stood > SETTLES_IN {
                (self.held, self.weight) = (true, 0.0);
            } else {
                self.weight = (1.0 - stood.max(0) as f32 / SETTLES_IN as f32).powi(2);
            }
        }
        self.time = self.time.saturating_add(time);
    }
}

/// Gives the models whose characters have chains their chains, and each step
/// tells the chains what the character's script says of them.
#[allow(clippy::type_complexity)]
pub fn swing(
    mut commands: Commands,
    state: Res<FieldState>,
    assets: Res<CharacterAssets>,
    mut actors: Query<(Entity, &Actor, &Skeleton, &Animator, Option<(&mut Chains, &mut Tails)>)>,
) {
    for (entity, actor, skeleton, animator, chains) in &mut actors {
        let Some(character) = state.scene.characters.get(&actor.id) else { continue };
        let Some(asset) = assets.get(actor.chr).filter(|asset| !asset.chains.is_empty()) else { continue };
        let Some((mut chains, mut tails)) = chains else {
            let chains: Vec<Chain> = asset.chains.iter().filter_map(|figures| Chain::new(figures, skeleton)).collect();
            commands.entity(entity).insert((Chains::new(chains), Tails::new(character.status)));
            continue;
        };
        // A model with no frames (a menu is up) has none of this either.
        if animator.paused {
            continue;
        }
        let time = character.time();
        tails.frame(character.status, time);
        (chains.weight, chains.held, chains.time) = (tails.weight, tails.held, time as f32);
        chains.off = character.flags.has(flag::NO_LAYERS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn her_tails_swing_as_she_moves_and_settle_over_four_seconds_of_standing() {
        let mut tails = Tails::new(status::RUN);
        tails.frame(status::RUN, FULL_PACE);
        assert_eq!((tails.weight, tails.held), (1.0, false));
        // Standing: the square of what is left of 120 frames.
        tails.frame(status::STAND, FULL_PACE);
        assert!((tails.weight - (119.0f32 / 120.0).powi(2)).abs() < 1e-6);
        for _ in 1..60 {
            tails.frame(status::STAND, FULL_PACE);
        }
        assert_eq!((tails.weight, tails.held), (0.25, false));
        for _ in 60..120 {
            tails.frame(status::STAND, FULL_PACE);
        }
        assert_eq!((tails.weight, tails.held), (0.0, false));
        // After them the chains are held, until she is in another status.
        tails.frame(status::STAND, FULL_PACE);
        assert_eq!((tails.weight, tails.held), (0.0, true));
        tails.frame(status::WALK, FULL_PACE);
        assert_eq!((tails.weight, tails.held), (1.0, false));
        // At half her pace she takes twice as long to settle.
        for _ in 0..120 {
            tails.frame(status::STAND, FULL_PACE / 2);
        }
        assert_eq!((tails.weight, tails.held), (0.25, false));
    }
}
