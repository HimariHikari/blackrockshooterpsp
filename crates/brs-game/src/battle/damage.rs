//! What a hit takes off: the sum the original does for a hit on an enemy (0x92510) and
//! for one on the player (0x9AD90), and the random numbers it draws on.

use bevy::prelude::Resource;

use super::status::Status;
use super::tables::Attack;

/// The battle's random numbers. The original's generator (0x93A0) is seeded from the
/// clock, so no particular sequence is the right one; this is a small generator of
/// the same use: whole numbers below a bound.
#[derive(Resource, Clone, Debug)]
pub struct Random(u64);

impl Random {
    pub fn new(seed: u64) -> Random {
        // The state must not be zero.
        Random(seed | 1)
    }

    /// A number from 0 up to, but not including, `bound`; 0 for a bound of 0 or less.
    pub fn below(&mut self, bound: i32) -> i32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let value = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33;
        if bound <= 0 { 0 } else { (value % bound as u64) as i32 }
    }

    /// A fraction from 0 to 1, both included, in the original's 32768 steps (0x94084).
    pub fn fraction(&mut self) -> f32 {
        self.below(0x8000) as f32 / 32767.0
    }
}

/// The random factor of a hit: from `percent` below to `percent` above 1 (0x972D4).
pub fn spread(percent: i32, random: &mut Random) -> f32 {
    let low = (100 - percent) as f32 / 100.0;
    let high = (100 + percent) as f32 / 100.0;
    low + (high - low) * random.fraction()
}

/// The figures of whoever attacks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Attacker {
    pub attack: i32,
    pub special: i32,
}

/// Whoever lands a hit, with everything the hit goes by: their figures for the
/// damage, their level for the chance of a status effect taking, and the status
/// effects on themselves.
#[derive(Clone, Copy, Debug)]
pub struct Assailant<'a> {
    pub figures: Attacker,
    pub level: i32,
    pub status: &'a Status,
}

/// The damage of `attack` by `attacker` on a target with `defence`. `factors` are
/// what scales it besides: status effects on either side, a guard, a weak point,
/// multiplied on one after the other in the order given, as the original does.
pub fn damage(attack: &Attack, attacker: Attacker, defence: i32, factors: &[f32], random: &mut Random) -> i32 {
    // Whole-number steps, as the original takes them.
    let power = attack.attack_percent * attacker.attack / 100 + attack.special_percent * attacker.special / 100;
    let resisted = attack.defence_percent * defence / 100;
    let scale = factors.iter().fold(attack.dealt_percent as f32 / 100.0, |scale, factor| scale * factor);
    let dealt = (power - resisted) as f32 * scale * spread(attack.spread, random);
    (dealt as i32).max(0)
}

/// What the status effects on the two sides of a hit scale its damage by (0x9B0D8
/// and 0x926EC, which agree): the attacker's ups and downs of attack, the target's
/// of defence, a ward on the target, and a boost on the attacker.
pub fn status_factors(attacker: &Status, target: &Status) -> [f32; 4] {
    [
        (attacker.attack_percent() + 100) as f32 / 100.0,
        (100 - target.defence_percent()) as f32 / 100.0,
        target.ward().unwrap_or(1.0),
        (attacker.boost()[0] + 100) as f32 / 100.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The player's normal shot (attack 5) as the table has it.
    fn normal_shot() -> Attack {
        Attack { id: 5, attack_percent: 100, defence_percent: 50, dealt_percent: 100, spread: 5, variant: -1, ..Attack::default() }
    }

    #[test]
    fn a_new_games_shot_on_the_first_eater_does_about_thirty() {
        // Attack 29 with the first cannon's 15, against defence 24.
        let attacker = Attacker { attack: 44, special: 41 };
        let mut random = Random::new(7);
        let mut seen = (i32::MAX, i32::MIN);
        for _ in 0..2000 {
            let dealt = damage(&normal_shot(), attacker, 24, &[], &mut random);
            seen = (seen.0.min(dealt), seen.1.max(dealt));
        }
        // (44 - 12) = 32, give or take 5%, cut to a whole number.
        assert_eq!(seen, (30, 33));
    }

    #[test]
    fn a_guard_scales_the_damage_and_it_never_goes_below_zero() {
        let mut random = Random::new(1);
        let attack = Attack { spread: 0, ..normal_shot() };
        let attacker = Attacker { attack: 49, special: 56 };
        assert_eq!(damage(&attack, attacker, 24, &[], &mut random), 37);
        assert_eq!(damage(&attack, attacker, 24, &[0.5], &mut random), 18);
        assert_eq!(damage(&attack, attacker, 500, &[], &mut random), 0);
        // Several things at once: a fifth more for the attacker, a quarter off for
        // the target, through a guard.
        assert_eq!(damage(&attack, attacker, 24, &[1.2, 0.75, 1.0, 1.0, 0.5], &mut random), 16);
    }

    #[test]
    fn status_effects_on_both_sides_scale_a_hit() {
        use super::super::status::{Effect, Giver, Taker};
        let give = |status: &mut Status, kind: i32, values: [i32; 4]| {
            let effect = Effect { id: kind * 100, kind, chance: 100, duration: 900, values, ..Effect::default() };
            status.give(&effect, &Giver::default(), &Taker::default(), &mut Random::new(1));
        };
        let (mut attacker, mut target) = (Status::default(), Status::default());
        assert_eq!(status_factors(&attacker, &target), [1.0; 4]);
        give(&mut attacker, 15, [20, 0, 0, 0]);
        give(&mut attacker, 19, [50, 0, 0, 0]);
        give(&mut target, 12, [25, 0, 0, 0]);
        assert_eq!(status_factors(&attacker, &target), [1.2, 1.25, 1.0, 1.5]);
        // A ward on the target lets nothing through at all.
        give(&mut target, 21, [100, 0, 0, 0]);
        assert_eq!(status_factors(&attacker, &target)[2], 0.0);
    }

    #[test]
    fn special_attacks_use_the_special_figure() {
        let mut random = Random::new(1);
        let attack = Attack { attack_percent: 0, special_percent: 100, defence_percent: 50, dealt_percent: 900, ..Attack::default() };
        // (41 - 12) nine times over.
        assert_eq!(damage(&attack, Attacker { attack: 44, special: 41 }, 24, &[], &mut random), 261);
    }

    #[test]
    fn random_numbers_stay_below_their_bound_and_cover_it() {
        let mut random = Random::new(3);
        let mut seen = [false; 5];
        for _ in 0..200 {
            seen[random.below(5) as usize] = true;
        }
        assert_eq!(seen, [true; 5]);
        assert_eq!(random.below(0), 0);
        assert_eq!(random.below(-3), 0);
        assert!((0..200).all(|_| (0.0..=1.0).contains(&random.fraction())));
    }
}
