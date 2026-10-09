//! The enemies a group keeps back, and how they come into the battle: a port of the
//! stage's record of them (`BtStageActor` + 0x1E8, set up at 0xA08E8), of its count
//! of what is left to beat (0xA0B7C), and of the engine function an enemy's script
//! calls one in with (0xC06E8).
//!
//! Nothing brings them in but the enemies' own scripts: each calls three times as it
//! dies, some while they wait, and a tower whenever its AI says so. The group's mode
//! decides whether a call is answered.

use bevy::prelude::*;
use brs_formats::egtb::{place, Member, Wave};

use crate::battle::damage::Random;
use crate::battle::tables::family;

/// The rules a group's later enemies come in by (the group's mode, 0x93108).
pub mod mode {
    pub const NONE: u8 = 0;
    /// Its towers call them, for as long as one stands.
    pub const TOWER: u8 = 1;
    /// One comes when nobody is left standing.
    pub const EMPTY: u8 = 2;
    /// One comes while fewer than two stand, or fewer than three.
    pub const UNDER_TWO: u8 = 3;
    pub const UNDER_THREE: u8 = 4;
    /// One comes whenever fewer stand than did at the start.
    pub const REFILL: u8 = 5;
}

/// A tower calls nobody while this many enemies stand.
const TOWER_CROWD: usize = 9;
/// An enemy with HP left whose home is nearer than this keeps a place taken (0xA0D50).
pub const PLACE_CLEARANCE: f32 = 100.0;

/// One of the enemies a group keeps back, with what its coming in depends on: the
/// room it takes and whether it flies (figures of its `.edx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reserve {
    pub wave: Wave,
    pub flies: bool,
    /// Its width and its depth.
    pub room: (i32, i32),
}

/// What a group has for its later enemies.
#[derive(Clone, Copy, Debug)]
pub struct Plan<'a> {
    pub mode: u8,
    pub reserves: &'a [Reserve],
    pub places: &'a [Member],
}

/// An answered call: which of the reserves comes in, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Call {
    pub reserve: usize,
    pub place: Member,
}

#[derive(Clone, Debug, Default)]
pub struct Reserves {
    /// Bits the scripts set and test for one another (engine functions 65 and 66).
    pub flags: u32,
    /// Towers standing. Their scripts count themselves out as they fall.
    pub towers: u8,
    towers_at_start: u8,
    /// Which of the reserves have come in.
    called: Vec<bool>,
    /// How many enemies stood when the battle began.
    standing_at_start: usize,
    /// How many of the reserves have yet to come in.
    left: usize,
    /// The group began with a tower: while one stands the battle cannot be won, and
    /// once none does the reserves no longer count (the stage's flag 1, 0xA03C4).
    tower_rule: bool,
}

impl Reserves {
    /// The record as the battle begins (0xA08E8): `families` are those of the
    /// enemies that stand there.
    pub fn new(plan: &Plan, families: &[i32]) -> Reserves {
        let towers = families.iter().filter(|kind| **kind == family::TOWER).count() as u8;
        let counted = plan.mode != mode::NONE;
        let total = if counted { plan.reserves.len() } else { 0 };
        Reserves {
            flags: 0,
            towers: if plan.mode == mode::TOWER { towers } else { 0 },
            towers_at_start: if plan.mode == mode::TOWER { towers } else { 0 },
            called: vec![false; total],
            standing_at_start: families.len(),
            left: total,
            tower_rule: towers > 0,
        }
    }

    /// How many enemies are left to beat with `standing` of them in the battle, or
    /// -1 while that cannot be told (0xA0B7C). At zero the battle is won.
    pub fn to_beat(&self, standing: usize) -> i32 {
        // While a tower stands nobody knows how many it will call (0xA0D0C).
        let coming = if self.towers > 0 { -1 } else { self.left as i32 };
        if self.tower_rule {
            if coming < 0 { -1 } else { standing as i32 }
        } else {
            coming.max(0) + standing as i32
        }
    }

    /// An enemy of `family` calls for one of the reserves (engine function 73).
    /// `standing` enemies are in the battle; `taken` tells whether somebody is in
    /// the way at a place. With `first` at zero or above only the places from it to
    /// `last` are tried; `keep` leaves the one that comes free to be called again.
    #[allow(clippy::too_many_arguments)]
    pub fn call(
        &mut self,
        plan: &Plan,
        family: i32,
        first: i32,
        last: i32,
        keep: bool,
        standing: usize,
        taken: &dyn Fn(Vec3) -> bool,
        random: &mut Random,
    ) -> Option<Call> {
        let mut tower = false;
        match plan.mode {
            mode::NONE => return None,
            mode::TOWER => {
                if self.towers == 0 || family != family::TOWER || standing >= TOWER_CROWD {
                    return None;
                }
                tower = true;
            }
            mode::EMPTY | mode::UNDER_TWO | mode::UNDER_THREE | mode::REFILL => {
                let wanted = match plan.mode {
                    mode::EMPTY => standing == 0,
                    mode::UNDER_TWO => standing < 2,
                    mode::UNDER_THREE => standing < 3,
                    _ => standing < self.standing_at_start,
                };
                if self.towers_at_start != 0 || !wanted {
                    return None;
                }
            }
            _ => {}
        }

        // The places it may come in at, in an order drawn afresh each time.
        let mut places: Vec<Member> = plan.places.to_vec();
        if places.is_empty() {
            return None;
        }
        let count = places.len() as i32;
        if first >= 0 && count >= 2 {
            let mut last = if last < count { last } else { count - 1 };
            if last < first {
                last = count - 1;
            }
            places = (first..=last).filter_map(|index| places.get(index as usize).copied()).collect();
        }
        if places.len() >= 2 {
            for _ in 0..places.len() {
                let (a, b) = (random.below(places.len() as i32), random.below(places.len() as i32));
                places.swap(a as usize, b as usize);
            }
        }

        // The first of the reserves not yet called that fits one of the places.
        let mut call = None;
        for pass in 0..2 {
            if call.is_some() {
                break;
            }
            'reserves: for (index, reserve) in plan.reserves.iter().enumerate().take(self.called.len()) {
                if self.called[index] {
                    continue;
                }
                for place in &places {
                    let room = place.size as i32 * 2;
                    if room < reserve.room.0 || room < reserve.room.1 {
                        continue;
                    }
                    if reserve.flies != (place.flags & place::AIR != 0) {
                        continue;
                    }
                    if taken(Vec3::from(place.position)) && family != family::ANYWHERE {
                        continue;
                    }
                    call = Some(Call { reserve: index, place: *place });
                    if !keep {
                        self.called[index] = true;
                    }
                    break 'reserves;
                }
            }
            self.left = self.called.iter().filter(|called| !**called).count();
            // A tower's reserves never run out: once it has gone through them it
            // starts over, and tries again at once if this time nobody came.
            if !tower || pass != 0 {
                break;
            }
            self.called.fill(false);
        }
        call
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EATER: Reserve = Reserve { wave: Wave { enemy: 5, variant: 10, entry: 1 }, flies: false, room: (100, 100) };
    const HORNET: Reserve = Reserve { wave: Wave { enemy: 14, variant: 1, entry: 1 }, flies: true, room: (200, 100) };

    fn ground(x: f32) -> Member {
        Member { enemy: 0x7f, flags: 1, variant: 0, position: [x, 0.0, -100.0], size: 100 }
    }

    fn air(x: f32) -> Member {
        Member { enemy: 0x7f, flags: 3, variant: 0, position: [x, 300.0, 150.0], size: 200 }
    }

    fn nobody(_: Vec3) -> bool {
        false
    }

    #[test]
    fn a_group_without_a_rule_keeps_nobody_back() {
        let plan = Plan { mode: mode::NONE, reserves: &[EATER], places: &[ground(0.0)] };
        let mut reserves = Reserves::new(&plan, &[1, 1]);
        assert_eq!(reserves.to_beat(2), 2);
        let mut random = Random::new(1);
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 0, &nobody, &mut random), None);
        assert_eq!(reserves.to_beat(0), 0);
    }

    #[test]
    fn a_refilled_group_sends_one_in_for_each_that_falls_until_they_run_out() {
        // Group 405: an egg and two Eaters, seven more Eaters, three places.
        let places = [ground(600.0), ground(0.0), ground(300.0)];
        let plan = Plan { mode: mode::REFILL, reserves: &[EATER; 7], places: &places };
        let mut reserves = Reserves::new(&plan, &[0xe, 1, 1]);
        let mut random = Random::new(3);
        assert_eq!(reserves.to_beat(3), 10);
        // Nobody comes while all three stand.
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 3, &nobody, &mut random), None);
        // One has fallen: its three calls bring one in, and only one.
        let call = reserves.call(&plan, 1, -1, -1, false, 2, &nobody, &mut random).unwrap();
        assert_eq!(call.reserve, 0);
        assert!(places.contains(&call.place));
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 3, &nobody, &mut random), None);
        assert_eq!(reserves.to_beat(3), 9);
        // And so on in their order, until all seven have come.
        for index in 1..7 {
            assert_eq!(reserves.call(&plan, 1, -1, -1, false, 2, &nobody, &mut random).unwrap().reserve, index);
        }
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 2, &nobody, &mut random), None);
        assert_eq!(reserves.to_beat(2), 2);
        assert_eq!(reserves.to_beat(0), 0);
    }

    #[test]
    fn the_counting_rules_wait_for_the_stage_to_thin_out() {
        let places = [ground(0.0)];
        for (rule, limit) in [(mode::EMPTY, 1), (mode::UNDER_TWO, 2), (mode::UNDER_THREE, 3)] {
            let plan = Plan { mode: rule, reserves: &[EATER], places: &places };
            let mut reserves = Reserves::new(&plan, &[1, 1, 1, 1]);
            let mut random = Random::new(3);
            assert_eq!(reserves.call(&plan, 1, -1, -1, false, limit, &nobody, &mut random), None, "rule {rule}");
            assert!(reserves.call(&plan, 1, -1, -1, false, limit - 1, &nobody, &mut random).is_some(), "rule {rule}");
        }
    }

    #[test]
    fn a_place_has_to_suit_the_one_that_comes() {
        let mut random = Random::new(7);
        // A flier comes in at a place in the air, the others on the ground.
        let places = [ground(0.0), air(600.0)];
        let plan = Plan { mode: mode::EMPTY, reserves: &[HORNET, EATER], places: &places };
        let mut reserves = Reserves::new(&plan, &[1]);
        for _ in 0..20 {
            let mut fresh = reserves.clone();
            let call = fresh.call(&plan, 1, -1, -1, false, 0, &nobody, &mut random).unwrap();
            assert_eq!((call.reserve, call.place), (0, places[1]));
        }
        // With no place for the first, the next that fits comes instead.
        let only_ground = Plan { places: &places[..1], ..plan };
        let call = reserves.call(&only_ground, 1, -1, -1, false, 0, &nobody, &mut random).unwrap();
        assert_eq!((call.reserve, call.place), (1, places[0]));

        // It has to have the room: twice the place's size, for width and depth both.
        let tight = [Member { size: 99, ..air(0.0) }];
        let plan = Plan { mode: mode::EMPTY, reserves: &[HORNET], places: &tight };
        let mut reserves = Reserves::new(&plan, &[1]);
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 0, &nobody, &mut random), None);
        assert_eq!(reserves.to_beat(0), 1);
    }

    #[test]
    fn nobody_comes_in_on_top_of_somebody_else() {
        let mut random = Random::new(7);
        let places = [ground(0.0), ground(300.0)];
        let plan = Plan { mode: mode::EMPTY, reserves: &[EATER, EATER], places: &places };
        let mut reserves = Reserves::new(&plan, &[1]);
        let taken = |at: Vec3| at.x == 0.0;
        for _ in 0..10 {
            let call = reserves.clone().call(&plan, 1, -1, -1, false, 0, &taken, &mut random).unwrap();
            assert_eq!(call.place, places[1]);
        }
        let everywhere = |_: Vec3| true;
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 0, &everywhere, &mut random), None);
        // One family's reserves come in regardless.
        assert!(reserves.call(&plan, family::ANYWHERE, -1, -1, false, 0, &everywhere, &mut random).is_some());
    }

    #[test]
    fn a_call_can_name_the_places_to_try() {
        let mut random = Random::new(7);
        let places = [ground(0.0), ground(300.0), ground(600.0), ground(900.0)];
        let plan = Plan { mode: mode::EMPTY, reserves: &[EATER], places: &places };
        let reserves = Reserves::new(&plan, &[1]);
        let called = |first: i32, last: i32, random: &mut Random| {
            reserves.clone().call(&plan, 1, first, last, false, 0, &nobody, random).map(|call| call.place.position[0])
        };
        for _ in 0..20 {
            // One place; a stretch of them; from one to the end, as the script
            // that passes -1 for the last means it.
            assert_eq!(called(2, 2, &mut random), Some(600.0));
            assert!(matches!(called(1, 2, &mut random), Some(x) if x == 300.0 || x == 600.0));
            assert!(matches!(called(1, -1, &mut random), Some(x) if x >= 300.0));
            assert!(matches!(called(2, 9, &mut random), Some(x) if x >= 600.0));
        }
        // A stretch past the last place has none in it.
        assert_eq!(called(5, 7, &mut random), None);
    }

    #[test]
    fn a_tower_calls_for_as_long_as_it_stands_and_the_battle_cannot_be_won_till_it_falls() {
        // Group 113: a tower and three Eaters, with one more Eater to call on.
        let places = [ground(600.0), ground(-5.0), ground(300.0)];
        let plan = Plan { mode: mode::TOWER, reserves: &[EATER], places: &places };
        let mut reserves = Reserves::new(&plan, &[family::TOWER, 1, 1, 1]);
        let mut random = Random::new(9);
        assert_eq!(reserves.to_beat(4), -1);
        assert_eq!(reserves.to_beat(1), -1);

        // Only a tower's call is answered, and the same one comes again and again.
        assert_eq!(reserves.call(&plan, 1, -1, -1, false, 2, &nobody, &mut random), None);
        for _ in 0..5 {
            let call = reserves.call(&plan, family::TOWER, -1, -1, false, 4, &nobody, &mut random);
            assert_eq!(call.map(|call| call.reserve), Some(0));
        }
        // Not into a crowd, though.
        assert_eq!(reserves.call(&plan, family::TOWER, -1, -1, false, 9, &nobody, &mut random), None);

        // The tower's script counts it out as it falls: then what stands is all
        // that is left, whatever the tower might still have called.
        reserves.towers = 0;
        assert_eq!(reserves.to_beat(2), 2);
        assert_eq!(reserves.call(&plan, family::TOWER, -1, -1, false, 2, &nobody, &mut random), None);
        assert_eq!(reserves.to_beat(0), 0);
    }
}
