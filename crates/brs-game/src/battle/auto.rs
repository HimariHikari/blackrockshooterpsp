//! Her skills that act of themselves (the table's kind 4). A battle keeps a list of
//! those she has and has not put aside (0x9F79C, at 0x1712E8), and whenever
//! something happens that one of them may act on, every one of the list is asked
//! (0x9FF58). What comes of it is the asker's to carry out.
//!
//! None of them is ever named in the log. Each that begins to act asks for its
//! line (0x9F654), but the line is asked of the record by what gives one only for
//! a skill of a slot (0x12B1AC), so nothing is written: the table's lines 39 to 47,
//! which name them, are never read. What is seen and heard of one that acts is a
//! sound and an effect on her (0x9F6D8).
//!
//! | Skill | When | If | What |
//! | --- | --- | --- | --- |
//! | 30, 31 Aftercare | the battle is won | 20 or 30 in 100 | she has a quarter of her HP back |
//! | 32 Max Up | (8: nothing asks) | her HP is whole | more experience |
//! | 33 Rocket Starter | the battle begins | 25 in 100 | status effect 1507 |
//! | 34 Dead Guard | any frame | her HP is a quarter or less | status effect 1608, once |
//! | 35 Six Pocket | she uses an item | 30 in 100 | what the item gives back is half as much again |
//! | 36 Attack-Free | she starts to shoot | 10 in 100 | the burst costs no heat |
//! | 37 Danger Guard | a hit two frames after she began to guard | always | the guard is that of effect 501 |
//! | 38 Sway Three | she starts to dodge | 10 in 100 | the dodge costs no heat |

use super::damage::Random;
use super::skills::Skill;

/// When a skill acts: its record's field 4 (0x12ADB8).
mod when {
    pub const BATTLE_BEGINS: i32 = 1;
    pub const ANY_FRAME: i32 = 2;
    pub const SHOOTING: i32 = 3;
    pub const DODGING: i32 = 4;
    pub const USING_AN_ITEM: i32 = 5;
    pub const GUARDING: i32 = 6;
    pub const BATTLE_WON: i32 = 7;
}

/// What a skill does: its record's field 7 (0x12AE18), by which 0x9FF58 picks the
/// routine that looks at what has happened. (2, more experience, is Max Up's alone,
/// which nothing ever has act; 3 is Six Pocket's.)
mod does {
    pub const HEALS: i32 = 1;
    pub const BETTERS_AN_ITEM: i32 = 3;
    pub const SPARES_HEAT: i32 = 4;
    pub const GUARDS: i32 = 5;
    pub const GIVES_EFFECT: i32 = 6;
}

/// What is heard and seen of one that begins to act (0x9F6D8): a sound of the
/// battle's bank and an effect on her. One that heals has the same sound and an
/// effect of its own (0x9F9E0).
pub const SOUND: u32 = 0x82;
pub const EFFECT: u32 = 998_045;
pub const EFFECT_HEALS: u32 = 998_042;
/// The sixtieths the end of a battle waits when one has acted on it (0x9DB24).
pub const END_WAIT: f32 = 60.0;
/// Her guard as it is unless one of them has it be another: a record of the status
/// effects (0x9AE9C).
pub const USUAL_GUARD: i32 = 500;

/// What has happened: the events 0x9FF58 is handed, by their numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happens {
    /// The fight begins (0x93FB4).
    BattleBegins = 0,
    /// A frame of hers in a battle that is on, nobody having the stage (0x99FD4).
    Frame = 1,
    /// She begins to guard (0x7D990), and stops (0x7D9F4).
    GuardBegins = 2,
    GuardEnds = 3,
    /// A hit lands on her as she guards (0x9AECC).
    GuardedHit = 4,
    /// She begins to shoot (0x7DF60), and stops (0x7DFF8).
    ShootBegins = 5,
    ShootEnds = 6,
    /// She begins to dodge (0x7DB74), and stops (0x7DC0C).
    DodgeBegins = 7,
    DodgeEnds = 8,
    /// She uses an item (0x98240), and has done with it (0x983B8).
    ItemBegins = 9,
    ItemEnds = 0xa,
    /// The battle is won (0x9DA70).
    BattleWon = 0xb,
    /// Asked as she begins to shoot or to dodge: whether it costs no heat (0x7DF98,
    /// 0x7DBAC).
    HeatSpared = 0xc,
    /// Asked as an item is about to give her HP back: by how much more (0x982B4).
    ItemGives = 0xe,
    /// Asked as a hit lands on her: whether her guard is another than the usual
    /// (0x9AF00).
    WhichGuard = 0xf,
}

/// Who asks: what the skills' conditions go by, and the frame it is.
#[derive(Clone, Copy, Debug)]
pub struct Her {
    pub hp: f32,
    pub max_hp: f32,
    /// The frames of hers that have gone by.
    pub frame: u32,
}

/// What the skills made of what happened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    /// How many of them have acted on it (the event's + 4): the asker has that
    /// heard and seen (0x9F6D8).
    pub acted: u32,
    /// Status effects for her, given as she would give them herself (0xA00F8).
    pub gives: Vec<i32>,
    /// HP she has back (the event's + 0xC, carried out by 0x9F9E0): the most that
    /// any of them gives.
    pub heals: i32,
    /// What she begins costs no heat (the event's + 0x20).
    pub heat_spared: bool,
    /// Her guard is the one of this status effect (the event's + 0x24 and + 0x28).
    pub guard: Option<i32>,
    /// What an item gives back is so many hundredths of what it would (the
    /// event's + 0x14 is what that is more than its + 0x10).
    pub item_hundredths: Option<i32>,
}

/// One of them as a battle keeps it (a node of the list at 0x1712E8).
#[derive(Clone, Debug, PartialEq)]
struct Auto {
    skill: Skill,
    /// It has acted (bit 0 of the node's + 8): for good, or until what it acted on
    /// is over.
    acted: bool,
    /// The frame she began to guard on (the node's + 0xC).
    since: u32,
}

/// Those a battle has.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Autos(Vec<Auto>);

impl Autos {
    /// A battle's list of them (0x9F79C). Each is put before those found so far, so
    /// they are asked from the highest number down.
    pub fn new(mut skills: Vec<Skill>) -> Autos {
        skills.sort_by_key(|skill| -skill.id);
        Autos(skills.into_iter().map(|skill| Auto { skill, acted: false, since: 0 }).collect())
    }

    /// What one of them may have acted on is over: her guard, a burst or a dodge
    /// (0x9FDF0 with 3, 0x9FC94 with 6 and 8). It acts no longer. Nothing is
    /// asked of her or of chance for that.
    pub fn ends(&mut self, what: Happens) {
        for auto in &mut self.0 {
            let over = match (auto.skill.auto[3], what) {
                (does::GUARDS, Happens::GuardEnds) => when::GUARDING,
                (does::SPARES_HEAT, Happens::ShootEnds) => when::SHOOTING,
                (does::SPARES_HEAT, Happens::DodgeEnds) => when::DODGING,
                (does::BETTERS_AN_ITEM, Happens::ItemEnds) => when::USING_AN_ITEM,
                _ => continue,
            };
            if auto.skill.auto[0] == over {
                auto.acted = false;
            }
        }
    }

    /// Has every one of them look at what has happened (0x9FF58).
    pub fn ask(&mut self, what: Happens, her: Her, random: &mut Random) -> Answer {
        let mut answer = Answer::default();
        if matches!(what, Happens::GuardEnds | Happens::ShootEnds | Happens::DodgeEnds | Happens::ItemEnds) {
            self.ends(what);
            return answer;
        }
        for auto in &mut self.0 {
            let (acts_when, kind) = (auto.skill.auto[0], auto.skill.auto[3]);
            // One begins to act, and is counted.
            let begins = |auto: &mut Auto, answer: &mut Answer| {
                auto.acted = true;
                answer.acted += 1;
            };
            match (kind, what) {
                // Aftercare (0x9F8D8): it is not named, and can act every time.
                (does::HEALS, Happens::BattleWon) if acts_when == when::BATTLE_WON && holds(&auto.skill, her, random) => {
                    answer.acted += 1;
                    answer.heals = answer.heals.max((her.max_hp * auto.skill.auto[4] as f32 / 100.0) as i32);
                }
                // Attack-Free and Sway Three (0x9FC94): for as long as what they
                // acted on lasts.
                (does::SPARES_HEAT, Happens::ShootBegins) if acts_when == when::SHOOTING && holds(&auto.skill, her, random) => begins(auto, &mut answer),
                (does::SPARES_HEAT, Happens::DodgeBegins) if acts_when == when::DODGING && holds(&auto.skill, her, random) => begins(auto, &mut answer),
                (does::SPARES_HEAT, Happens::HeatSpared) if auto.acted => answer.heat_spared = true,
                // Six Pocket (0x9FB6C): for the one item.
                (does::BETTERS_AN_ITEM, Happens::ItemBegins) if acts_when == when::USING_AN_ITEM && holds(&auto.skill, her, random) => begins(auto, &mut answer),
                (does::BETTERS_AN_ITEM, Happens::ItemGives) if auto.acted => answer.item_hundredths = Some(auto.skill.auto[4]),
                // Danger Guard (0x9FDF0): only the hit that lands two frames after
                // she began to guard.
                (does::GUARDS, Happens::GuardBegins) if acts_when == when::GUARDING => auto.since = her.frame,
                (does::GUARDS, Happens::GuardedHit) if acts_when == when::GUARDING => {
                    if auto.since + 2 != her.frame {
                        auto.acted = false;
                    } else if !auto.acted && holds(&auto.skill, her, random) {
                        begins(auto, &mut answer);
                    }
                }
                (does::GUARDS, Happens::WhichGuard) if auto.acted => answer.guard = Some(auto.skill.auto[4]),
                // Rocket Starter and Dead Guard (0xA0054): once in a battle.
                (does::GIVES_EFFECT, Happens::BattleBegins) if acts_when == when::BATTLE_BEGINS && !auto.acted && holds(&auto.skill, her, random) => {
                    begins(auto, &mut answer);
                    answer.gives.push(auto.skill.auto[4]);
                }
                (does::GIVES_EFFECT, Happens::Frame) if acts_when == when::ANY_FRAME && !auto.acted && holds(&auto.skill, her, random) => {
                    begins(auto, &mut answer);
                    answer.gives.push(auto.skill.auto[4]);
                }
                _ => {}
            }
        }
        answer
    }
}

/// Whether what a skill asks for holds (0x9F49C), by its record's fields 5 and 6: a
/// chance in a hundred, her HP at just so many hundredths of all of it, at no more
/// than so many, or nothing at all.
fn holds(skill: &Skill, her: Her, random: &mut Random) -> bool {
    let (condition, figure) = (skill.auto[1], skill.auto[2]);
    let hundredths = her.hp * 100.0 / her.max_hp;
    match condition {
        1 => random.below(100) < figure,
        2 => hundredths == figure as f32,
        3 => hundredths <= figure as f32,
        4 => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The skills as the game's table has them.
    fn skill(id: i32) -> Skill {
        let auto = match id {
            30 => [7, 1, 20, 1, 25, 0],
            31 => [7, 1, 30, 1, 25, 0],
            32 => [8, 2, 100, 2, 125, 0],
            33 => [1, 1, 25, 6, 1507, 0],
            34 => [2, 3, 25, 6, 1608, 0],
            35 => [5, 1, 30, 3, 150, 200],
            36 => [3, 1, 10, 4, 0, 0],
            37 => [6, 4, 0, 5, 501, 0],
            38 => [4, 1, 10, 4, 0, 0],
            _ => panic!("skill {id} does not act of itself"),
        };
        Skill::of_itself(id, auto)
    }

    const WHOLE: Her = Her { hp: 400.0, max_hp: 400.0, frame: 0 };

    /// How many times out of `tries` something is so, each with random numbers of
    /// its own.
    fn times(tries: u64, mut so: impl FnMut(&mut Random) -> bool) -> usize {
        (0..tries).filter(|&seed| so(&mut Random::new(seed * 7919 + 13))).count()
    }

    #[test]
    fn they_are_asked_from_the_highest_number_down() {
        let autos = Autos::new(vec![skill(33), skill(38), skill(30)]);
        assert_eq!(autos.0.iter().map(|auto| auto.skill.id).collect::<Vec<i32>>(), [38, 33, 30]);
        assert!(Autos::default().0.is_empty());
    }

    #[test]
    fn rocket_starter_gives_its_effect_as_one_battle_in_four_begins() {
        let began = times(2000, |random| {
            let mut autos = Autos::new(vec![skill(33)]);
            let answer = autos.ask(Happens::BattleBegins, WHOLE, random);
            assert!(answer == Answer::default() || answer == Answer { acted: 1, gives: vec![1507], ..Answer::default() });
            // Nothing else has it act.
            assert_eq!(autos.ask(Happens::Frame, WHOLE, random), Answer::default());
            answer.acted == 1
        });
        assert!((420..580).contains(&began), "{began}");
    }

    #[test]
    fn dead_guard_gives_its_effect_once_when_her_hp_is_a_quarter_or_less() {
        let mut random = Random::new(4);
        let mut autos = Autos::new(vec![skill(34)]);
        assert_eq!(autos.ask(Happens::Frame, Her { hp: 101.0, ..WHOLE }, &mut random), Answer::default());
        assert_eq!(autos.ask(Happens::BattleBegins, Her { hp: 50.0, ..WHOLE }, &mut random), Answer::default());
        let answer = autos.ask(Happens::Frame, Her { hp: 100.0, ..WHOLE }, &mut random);
        assert_eq!(answer, Answer { acted: 1, gives: vec![1608], ..Answer::default() });
        // Not again in that battle, however low she goes.
        assert_eq!(autos.ask(Happens::Frame, Her { hp: 10.0, ..WHOLE }, &mut random), Answer::default());
    }

    #[test]
    fn attack_free_and_sway_three_spare_the_heat_of_one_burst_or_dodge_in_ten() {
        let spared = times(3000, |random| {
            let mut autos = Autos::new(vec![skill(36), skill(38)]);
            // Asked before she begins anything, the answer is no.
            assert!(!autos.ask(Happens::HeatSpared, WHOLE, random).heat_spared);
            let begun = autos.ask(Happens::ShootBegins, WHOLE, random);
            let spared = autos.ask(Happens::HeatSpared, WHOLE, random).heat_spared;
            assert_eq!(begun.acted, spared as u32);
            // When she stops it is over, and a dodge is asked about anew.
            autos.ask(Happens::ShootEnds, WHOLE, random);
            assert!(!autos.ask(Happens::HeatSpared, WHOLE, random).heat_spared);
            spared
        });
        assert!((240..360).contains(&spared), "{spared}");
        let dodges = times(3000, |random| {
            let mut autos = Autos::new(vec![skill(36), skill(38)]);
            let begun = autos.ask(Happens::DodgeBegins, WHOLE, random);
            let spared = autos.ask(Happens::HeatSpared, WHOLE, random).heat_spared;
            assert_eq!(begun.acted, spared as u32);
            // The end of a burst is nothing to a dodge.
            autos.ask(Happens::ShootEnds, WHOLE, random);
            assert_eq!(autos.ask(Happens::HeatSpared, WHOLE, random).heat_spared, spared);
            autos.ask(Happens::DodgeEnds, WHOLE, random);
            assert!(!autos.ask(Happens::HeatSpared, WHOLE, random).heat_spared);
            spared
        });
        assert!((240..360).contains(&dodges), "{dodges}");
    }

    #[test]
    fn danger_guard_is_for_the_hit_that_lands_two_frames_after_she_began_to_guard() {
        let mut random = Random::new(4);
        let mut autos = Autos::new(vec![skill(37)]);
        let at = |frame: u32| Her { frame, ..WHOLE };
        assert_eq!(autos.ask(Happens::WhichGuard, at(9), &mut random).guard, None);
        autos.ask(Happens::GuardBegins, at(10), &mut random);
        // A frame too soon it is the usual guard.
        assert_eq!(autos.ask(Happens::GuardedHit, at(11), &mut random), Answer::default());
        assert_eq!(autos.ask(Happens::WhichGuard, at(11), &mut random).guard, None);
        // On the frame itself it acts, and her guard is effect 501's.
        assert_eq!(autos.ask(Happens::GuardedHit, at(12), &mut random), Answer { acted: 1, ..Answer::default() });
        assert_eq!(autos.ask(Happens::WhichGuard, at(12), &mut random), Answer { guard: Some(501), ..Answer::default() });
        // A second hit on that frame has the same guard and is not counted again; a
        // later one has the usual guard.
        assert_eq!(autos.ask(Happens::GuardedHit, at(12), &mut random), Answer::default());
        assert_eq!(autos.ask(Happens::WhichGuard, at(12), &mut random).guard, Some(501));
        assert_eq!(autos.ask(Happens::GuardedHit, at(13), &mut random), Answer::default());
        assert_eq!(autos.ask(Happens::WhichGuard, at(13), &mut random).guard, None);
        // Letting go of the guard ends it too.
        autos.ask(Happens::GuardBegins, at(20), &mut random);
        autos.ask(Happens::GuardedHit, at(22), &mut random);
        autos.ask(Happens::GuardEnds, at(22), &mut random);
        assert_eq!(autos.ask(Happens::WhichGuard, at(22), &mut random).guard, None);
    }

    #[test]
    fn aftercare_heals_a_quarter_of_her_hp_after_some_battles_and_max_up_never_acts() {
        let healed = times(2000, |random| {
            let mut autos = Autos::new(vec![skill(30), skill(32), skill(35)]);
            let answer = autos.ask(Happens::BattleWon, Her { hp: 400.0, max_hp: 400.0, frame: 900 }, random);
            // Max Up, whose HP condition holds, and Six Pocket do nothing.
            assert!(answer.gives.is_empty() && !answer.heat_spared && answer.guard.is_none());
            assert_eq!((answer.acted, answer.heals), if answer.heals > 0 { (1, 100) } else { (0, 0) });
            answer.heals > 0
        });
        assert!((320..480).contains(&healed), "{healed}");
        // The better one acts oftener; with both, each has its chance and she has
        // the one quarter back.
        let both = times(2000, |random| {
            let mut autos = Autos::new(vec![skill(30), skill(31)]);
            let answer = autos.ask(Happens::BattleWon, WHOLE, random);
            assert!(answer.heals == 0 || answer.heals == 100);
            answer.heals > 0
        });
        assert!((800..960).contains(&both), "{both}");
    }
}
