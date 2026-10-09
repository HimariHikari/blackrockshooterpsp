//! How a battle begins and ends: a port of the states its root goes through
//! (`BtRoot`, 0x8D624) and of the stage's count of the enemies that are left
//! (`BtStageActor`, 0xA05F0).

use bevy::prelude::Resource;

/// With nobody on the stage though some are still to come, the battle is given up as
/// won after this long, in sixtieths of a second (0xA0670).
const EMPTY_STAGE_TIME: f32 = 600.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The battle is coming into view; nobody acts yet.
    Enter,
    Fight,
    /// The battle is decided: the player's last state plays, and the results follow
    /// (`BtResult`, then `BtGameOver` if she has fallen).
    Result,
}

#[derive(Resource, Debug)]
pub struct Flow {
    phase: Phase,
    /// How long the stage has stood empty with enemies still to come.
    empty_for: f32,
    outcome: Option<Outcome>,
    /// Times the enemies have yet to be told that the battle is over.
    announcements: u32,
    /// How long the battle has gone on, in sixtieths of a second (the root's clock,
    /// 0x8CF6C).
    time: f32,
    /// The battle is a boss's: `BtEnter` has the enemies think from the first
    /// frame and leaves the rest to their script (0x93DE8 on), which lets the
    /// encounter go and says when the fight begins (0x93FA0 through 0xBEA44).
    scripted: bool,
    /// The script has said so.
    begun: bool,
}

impl Default for Flow {
    fn default() -> Self {
        Flow { phase: Phase::Enter, empty_for: 0.0, outcome: None, announcements: 0, time: 0.0, scripted: false, begun: false }
    }
}

impl Flow {
    /// The battle's clock goes on by the step. What is read of it afterwards is its
    /// time as the battle was decided (0xA022C), so it stops there.
    pub fn clock(&mut self, dt: f32) {
        if self.outcome.is_none() {
            self.time += dt;
        }
    }

    /// How long the battle took, in whole seconds.
    pub fn seconds(&self) -> i32 {
        (self.time / 60.0) as i32
    }

    /// The battle is decided (the root's + 0x88): nothing takes HP off anyone any
    /// more (0x98C3C, 0x9283C).
    pub fn decided(&self) -> bool {
        self.outcome.is_some()
    }

    /// One frame of the root's own progress. The battle is under way once the
    /// screen has `come` to it from the field (`BtEnter`, 0x93D9C, waits for the
    /// encounter to be over: `encounter.rs`). Returns the outcome on the frame the
    /// battle is found to be decided: the enemies are then told (0x91B58), and a
    /// player who has won goes into her last state.
    pub fn advance(&mut self, come: bool) -> Option<Outcome> {
        match self.phase {
            Phase::Enter => {
                if if self.scripted { self.begun } else { come } {
                    self.phase = Phase::Fight;
                }
                None
            }
            Phase::Fight => {
                let outcome = self.outcome?;
                self.phase = Phase::Result;
                self.announcements += 1;
                Some(outcome)
            }
            Phase::Result => None,
        }
    }

    /// How many times the enemies are now to be sent the signal that the battle is
    /// over: once when the root finds it decided, and once before that if it was the
    /// player's fall that decided it.
    pub fn take_announcements(&mut self) -> u32 {
        std::mem::take(&mut self.announcements)
    }

    #[cfg(test)]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// The root has found the battle decided and gone on to what follows it
    /// (0x9D7C0): a player who has won is in her last state by then.
    pub fn moved_on(&self) -> bool {
        self.phase == Phase::Result
    }

    /// The battle is a boss's, whose script sees to how it begins.
    pub fn leave_to_script(&mut self) {
        self.scripted = true;
    }

    /// A script has the fight begin (0xBEA44).
    pub fn begin(&mut self) {
        self.begun = true;
    }

    /// The enemies have come in and may think (0x93F0C): at once in a boss's
    /// battle, where `BtEnter` waits for nothing.
    pub fn thinking(&self) -> bool {
        self.scripted || self.phase != Phase::Enter
    }

    /// The fight has begun (0x93FA0), whether or not it is over since.
    pub fn under_way(&self) -> bool {
        self.phase != Phase::Enter
    }

    /// The fight is on: the player can act (the root's flag 8, set at 0x93FF4 and
    /// cleared the moment the battle is decided).
    pub fn running(&self) -> bool {
        self.phase == Phase::Fight && self.outcome.is_none()
    }

    /// The stage's look at its enemies, once a frame (0xA05F0): `standing` of them
    /// are in the battle and neither dead nor dying, and `to_beat` are left counting
    /// those that have yet to come in (-1 while that cannot be told). With none left
    /// the battle is won.
    pub fn count(&mut self, dt: f32, standing: usize, to_beat: i32) {
        if self.phase != Phase::Fight || self.outcome.is_some() {
            return;
        }
        if to_beat == 0 {
            self.outcome = Some(Outcome::Won);
        } else if standing == 0 {
            self.empty_for += dt;
            if self.empty_for > EMPTY_STAGE_TIME {
                self.outcome = Some(Outcome::Won);
            }
        } else {
            self.empty_for = 0.0;
        }
    }

    /// The player has fallen (the end of 0x78D38).
    pub fn lose(&mut self) {
        if self.outcome.is_none() {
            self.outcome = Some(Outcome::Lost);
            self.announcements += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn under_way() -> Flow {
        let mut flow = Flow::default();
        assert_eq!(flow.advance(true), None);
        assert!(flow.under_way());
        flow
    }

    #[test]
    fn nobody_acts_until_the_battle_has_come_into_view() {
        let mut flow = Flow::default();
        for _ in 0..40 {
            flow.advance(false);
            assert!(!flow.running() && !flow.under_way());
            // Nothing is counted yet either.
            flow.count(2.0, 0, 0);
        }
        flow.advance(true);
        assert!(flow.running() && flow.under_way());
        assert_eq!(flow.outcome(), None);
    }

    #[test]
    fn a_bosss_battle_begins_when_its_script_says_so() {
        let mut flow = Flow::default();
        flow.leave_to_script();
        // The enemies think from the first frame, and the screen's coming is not
        // what the fight waits for.
        for _ in 0..40 {
            assert_eq!(flow.advance(true), None);
            assert!(flow.thinking() && !flow.under_way() && !flow.running());
        }
        flow.begin();
        flow.advance(false);
        assert!(flow.under_way() && flow.running());
    }

    #[test]
    fn the_battle_is_won_when_no_enemy_is_left_or_to_come() {
        let mut flow = under_way();
        flow.count(2.0, 2, 2);
        flow.count(2.0, 0, 1);
        // While it cannot be told how many are left, nobody has won either.
        flow.count(2.0, 1, -1);
        assert_eq!(flow.outcome(), None);
        flow.count(2.0, 0, 0);
        assert_eq!(flow.outcome(), Some(Outcome::Won));
        // The fight stops at once; the root moves on a frame later, and only once.
        assert!(!flow.running());
        assert_eq!(flow.phase(), Phase::Fight);
        assert_eq!(flow.take_announcements(), 0);
        assert_eq!(flow.advance(true), Some(Outcome::Won));
        assert_eq!(flow.phase(), Phase::Result);
        assert_eq!(flow.take_announcements(), 1);
        assert_eq!(flow.advance(true), None);
        // A fall after that changes nothing.
        flow.lose();
        assert_eq!(flow.outcome(), Some(Outcome::Won));
        assert_eq!(flow.take_announcements(), 0);
    }

    #[test]
    fn an_empty_stage_is_given_up_as_won_after_ten_seconds() {
        let mut flow = under_way();
        for _ in 0..300 {
            flow.count(2.0, 0, 3);
        }
        assert_eq!(flow.outcome(), None);
        // Someone turning up starts the count afresh.
        flow.count(2.0, 1, 3);
        for _ in 0..300 {
            flow.count(2.0, 0, 2);
        }
        assert_eq!(flow.outcome(), None);
        flow.count(2.0, 0, 2);
        assert_eq!(flow.outcome(), Some(Outcome::Won));
    }

    #[test]
    fn the_clock_runs_until_the_battle_is_decided() {
        let mut flow = under_way();
        for _ in 0..45 {
            flow.clock(2.0);
        }
        assert_eq!(flow.seconds(), 1);
        assert!(!flow.decided());
        flow.count(2.0, 0, 0);
        assert!(flow.decided());
        for _ in 0..300 {
            flow.clock(2.0);
        }
        assert_eq!(flow.seconds(), 1);
    }

    #[test]
    fn a_fallen_player_loses_the_battle() {
        let mut flow = under_way();
        flow.lose();
        assert!(!flow.running());
        // Enemies dying afterwards do not turn it into a win.
        flow.count(2.0, 0, 0);
        assert_eq!(flow.advance(true), Some(Outcome::Lost));
        // The enemies are told as she falls, and again as the root moves on.
        assert_eq!(flow.take_announcements(), 2);
    }
}
