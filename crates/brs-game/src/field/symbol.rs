//! The behaviour of the enemies that roam the field (the original's "symbols"): each
//! is run by one of three small scripts of the field's resident data, which are sets
//! of states, through the engine functions ported here (the original's table at
//! 0x172900 and the update at 0xF9C2C). See `docs/FIELD_NOTES.md`.
//!
//! A state is a function of the script: it does what the state does, waiting inside
//! engine functions as it goes, and returns the state to go on to. Before the state
//! runs each frame the engine looks for interruptions (the player touching the
//! symbol), each of which is a function of the script that may name a new state.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{bail, Context as _, Result};
use bevy::prelude::*;
use brs_formats::script::{self, Call, Machine, Run, Script};

use super::motion::{self, Ground, Resources};
use super::scene::{flag, pace, Character, Scene, FULL_PACE};
use super::tables::Tunables;
use crate::data::GameData;
use crate::sound::Request;

/// Where the behaviour scripts are: one file for each behaviour number.
const SCRIPTS: &str = "SYSTEM.VOL/fld_resident.bin/04.bin";
const SCRIPT_COUNT: usize = 3;

// The functions of a script that the engine calls by name (the table at 0x1728F0).
// There is one more, `int_touch_enemy`, for one symbol running into another.
const INT_INIT: &str = "int_init";
const INT_TOUCH_PLAYER: &str = "int_touch_player";
/// A battle has been fought: the field's system script has every symbol told so
/// after each one, got away from or not (its action 8, 0x109054).
const INT_BATTLE_ESCAPE: &str = "int_battle_escape";

/// The engine functions, by their numbers in the scripts.
mod native {
    pub const SET_VARIABLE: u32 = 0x00;
    pub const VARIABLE: u32 = 0x01;
    pub const SET_BIT: u32 = 0x02;
    pub const BIT: u32 = 0x03;
    pub const WAIT: u32 = 0x04;
    pub const CHASE: u32 = 0x05;
    pub const EVENT: u32 = 0x06;
    pub const WAIT_UNTIL: u32 = 0x07;
    pub const ALERT: u32 = 0x08;
    pub const RESTART: u32 = 0x09;
    pub const REMEMBER_HOME: u32 = 0x0b;
    pub const WALK_HOME: u32 = 0x0c;
    pub const FLAG: u32 = 0x0d;
    pub const NOTHING: u32 = 0x0e;
    pub const STATE: u32 = 0x0f;
    pub const PRINT: u32 = 0x10;
    pub const ACT: u32 = 0x11;
    pub const SET: u32 = 0x12;
    pub const SET_FLAG: u32 = 0x13;
    pub const SET_INT: u32 = 0x14;
    pub const WAIT_ACTION: u32 = 0x15;
    pub const SYSTEM_VALUE: u32 = 0x18;
    pub const WAIT_READY: u32 = 0x19;
    pub const INT: u32 = 0x1c;
    pub const GAME_FLAG: u32 = 0x1d;
}

// What `WAIT_UNTIL` waits for, as bits of its argument, and what it answers for each.
const UNTIL_NOTICES: i32 = 1;
const UNTIL_AWARE: i32 = 2;
const UNTIL_NEAR: i32 = 4;
/// Not something to wait for: the symbol moves about in its own way meanwhile.
const UNTIL_MOVES: i32 = 8;
const ANSWER_NEAR: u32 = 0;
const ANSWER_NOTICED: u32 = 1;
const ANSWER_AWARE: u32 = 3;

/// The cry of a symbol that has noticed its target, of the bank that is always
/// there (0xFADD8).
const SOUND_NOTICED: u32 = 0x2f;

/// How near its target a chasing symbol has to be for `UNTIL_NEAR`.
const NEAR: f32 = 500.0;
/// How near home counts as being home again (0xFC864).
const HOME_REACH: f32 = 120.0;
/// A wandering symbol picks a new place when it has been on its way this long.
const WANDER_TIME: i32 = 0x96000;
/// How far above and below a place wandered to its ground is looked for.
const WANDER_GROUND_REACH: f32 = 500.0;
/// And that of a place on a route (0xFBCA4); how near its route one that patrols
/// has to be to be back on it (0xFC88C); by how much a place between two points
/// of a route has to be nearer than the nearest point to be taken (0x16D758); the
/// byte variable that holds which point of its route it was last at (0xFAC54);
/// and the bit that is set when it sets out on its route (0xFAC14).
const ROUTE_GROUND_REACH: f32 = 200.0;
const ROUTE_REACH: f32 = 300.0;
const ROUTE_NEARER: f32 = 1.01;
const ROUTE_AT: usize = 2;
const BIT_ON_ROUTE: u32 = 1 << 3;

// The ways of moving about a symbol can be given (its int parameter 0).
const MOVEMENT_STAND: i32 = 0;
const MOVEMENT_PATROL: i32 = 1;
const MOVEMENT_WANDER: i32 = 2;

// What `ACT` can have a symbol do: the numbers a map's script uses too.
const ACT_WALK: i32 = 0;
const ACT_TURN: i32 = 2;
const ACT_STOP: i32 = 7;
const ACT_DASH: i32 = 11;


// The system values a script asks for, and where they are in the field's tunables.
const SYSTEM_ALERT_FRAMES: i32 = 0x2e;
const SYSTEM_DASH_PACE: i32 = 0x30;
const PRM_ALERT_FRAMES: usize = 0x4a;
const PRM_DASH_PACE: usize = 0x46;

/// States a script may go through in one frame before it is taken to be stuck.
const STATE_LIMIT: usize = 64;

/// The numbers the original's wandering draws on: twelve bits of its generator.
#[derive(Clone)]
struct Random(u32);

impl Random {
    /// A fraction from 0 up to, but not reaching, 1, in steps of 1/4096.
    fn fraction(&mut self) -> f32 {
        // Xorshift: any even spread will do, the original's own sequence is not kept.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 & 0xfff) as f32 / 4096.0
    }
}

/// The nearest of a route to a place: how far away it is, whether it is between
/// two points or at one, the point it is at or the stretch begins at, and where
/// it is.
#[derive(Clone, Copy, Debug)]
struct Nearest {
    away: f32,
    between: bool,
    index: usize,
    place: Vec3,
}

/// A run of states on a machine of its own, and the state it is in.
struct Task {
    machine: Machine,
    state: u32,
}

impl Task {
    fn new(script: &Script, state: u32) -> Task {
        Task { machine: Machine::new(script), state }
    }
}

/// What runs one symbol.
struct Brain {
    script: Arc<Script>,
    /// The behaviour number the script is.
    behaviour: i32,
    /// The state it is in.
    state: u32,
    /// The machine its states run on; it is out of here while they run.
    machine: Option<Machine>,
    /// The states of a chase, which run inside the chase's engine function.
    chase: Option<Task>,
    /// What a wait of frames has counted, for its own states and for a chase's.
    waited: [i32; 2],
    /// An interruption has named a new state, which starts afresh (the mind's bit 1).
    restart: bool,
    /// The bits scripts keep with `SET_BIT`, and their three byte variables.
    bits: u32,
    variables: [i8; 3],
}

/// What a frame of the symbols works on.
pub struct Context<'a> {
    pub scene: &'a mut Scene,
    pub tunables: &'a Tunables,
    pub ground: &'a Ground<'a>,
    pub resources: &'a dyn Resources,
    /// The field is in its normal state: no event is running and no battle (0x115B68).
    pub normal: bool,
    /// The character whose event is running, which is left out of the frame.
    pub in_event: Option<i32>,
    /// Characters whose event a script has asked for this frame.
    pub events: Vec<i32>,
}

/// The behaviour scripts, and the symbols they run.
#[derive(Resource)]
pub struct Brains {
    scripts: Vec<Arc<Script>>,
    brains: BTreeMap<i32, Brain>,
    random: Random,
}

impl Brains {
    pub fn load(data: &GameData) -> Result<Brains> {
        let scripts = (0..SCRIPT_COUNT)
            .map(|number| data.script(format!("{SCRIPTS}/{number:02}.sscr")).map(Arc::new))
            .collect::<Result<Vec<_>>>()?;
        Ok(Brains { scripts, brains: BTreeMap::new(), random: Random(0x2545_f491) })
    }

    /// Forgets every symbol: the map they were on has been left.
    pub fn clear(&mut self) {
        self.brains.clear();
    }

    /// A frame of every character that a behaviour script runs: each is given its
    /// brain if it has none yet, thinks, and is moved by what it thinks. Returns
    /// which characters were seen to, so that the rest can be moved without thought.
    pub fn update(&mut self, context: &mut Context) -> Vec<i32> {
        let behaving: Vec<(i32, i32)> = context
            .scene
            .characters
            .iter()
            .filter_map(|(&id, character)| Some((id, character.behaviour?)))
            .collect();
        self.brains.retain(|id, brain| behaving.contains(&(*id, brain.behaviour)));

        for &(id, behaviour) in &behaving {
            if !self.brains.contains_key(&id) {
                match self.awaken(id, behaviour, context) {
                    Ok(brain) => {
                        self.brains.insert(id, brain);
                    }
                    Err(error) => {
                        warn!("character {id} cannot be given behaviour {behaviour}: {error:#}");
                        if let Some(character) = context.scene.characters.get_mut(&id) {
                            character.behaviour = None;
                        }
                        continue;
                    }
                }
            }
            // One too far from her has no frame, and so does not think (0x106060
            // comes before 0xF9C2C).
            if context.scene.characters.get(&id).is_some_and(|character| character.asleep) {
                continue;
            }
            let brain = self.brains.get_mut(&id).expect("the brain was just made");
            if let Err(error) = think(id, brain, &mut self.random, context) {
                warn!("the behaviour of character {id} failed and is taken from it: {error:#}");
                self.brains.remove(&id);
                if let Some(character) = context.scene.characters.get_mut(&id) {
                    character.behaviour = None;
                }
            }
        }
        behaving.into_iter().map(|(id, _)| id).collect()
    }

    /// Makes the brain of a character that has been given a behaviour (0xF9AE8): its
    /// script's `int_init` names its first state.
    fn awaken(&mut self, id: i32, behaviour: i32, context: &mut Context) -> Result<Brain> {
        let script = usize::try_from(behaviour).ok().and_then(|number| self.scripts.get(number));
        let script = script.with_context(|| format!("there is no behaviour script {behaviour}"))?.clone();
        let mut brain = Brain {
            machine: Some(Machine::new(&script)),
            script,
            behaviour,
            state: 0,
            chase: None,
            waited: [0; 2],
            restart: false,
            bits: 0,
            variables: [0; 3],
        };
        if let Some(character) = context.scene.characters.get_mut(&id) {
            character.flags.set(flag::BEHAVES, true);
        }
        let mut engine = Engine { id, brain: &mut brain, random: &mut self.random, context, level: 0 };
        engine.interrupt(INT_INIT)?;
        brain.restart = false;
        if brain.state == 0 {
            bail!("the script's {INT_INIT} names no state");
        }
        Ok(brain)
    }
}

/// A frame of one symbol (0xF9C2C).
fn think(id: i32, brain: &mut Brain, random: &mut Random, context: &mut Context) -> Result<()> {
    let Some(character) = context.scene.characters.get(&id) else { return Ok(()) };
    let was_acting = character.acting();
    let mut engine = Engine { id, brain, random, context, level: 0 };
    // A battle has been fought: its script is told, whatever it is in the middle of
    // (0xFA3C0). The state that names starts when it next thinks.
    if std::mem::take(&mut engine.me().mind.stand_down) {
        engine.interrupt(INT_BATTLE_ESCAPE)?;
    }
    if engine.context.in_event == Some(id) {
        // While its own event runs it does not think, though it goes on moving.
        engine.step();
    } else {
        // Interruptions first (0xF9CB4): the player touching it.
        if engine.touches_player() {
            engine.interrupt(INT_TOUCH_PLAYER)?;
        }
        // A state an interruption named starts from nothing: whatever the old one
        // was in the middle of is dropped (0xF9DE8).
        if engine.brain.restart {
            engine.brain.restart = false;
            engine.brain.chase = None;
            engine.brain.machine = Some(Machine::new(&engine.brain.script));
            engine.me().flags.set(flag::ACTING, false);
        }
        // Then its state (0xF9E6C).
        let Some(mut machine) = engine.brain.machine.take() else { bail!("the symbol's machine is missing") };
        let mut state = engine.brain.state;
        let ran = run_states(&mut machine, &mut state, &mut engine);
        engine.brain.machine = Some(machine);
        ran?;
    }
    // A character whose action has ended in this frame goes back to standing
    // (0x1061B8).
    let character = engine.me();
    if was_acting && !character.acting() && !character.flags.has(flag::KEEPS_POSE) {
        character.stop();
    }
    Ok(())
}

/// Runs a state on `machine` until it waits; a state that returns names the next,
/// which starts at once. The symbol's own state is kept where its script can ask
/// for it; a chase's is the caller's to keep.
fn run_states(machine: &mut Machine, state: &mut u32, engine: &mut Engine) -> Result<()> {
    for _ in 0..STATE_LIMIT {
        let run = if machine.waiting() { machine.resume(engine)? } else { machine.start(*state, &[], engine)? };
        match run {
            Run::Waiting => return Ok(()),
            Run::Returned(next) => {
                if next != 0 {
                    *state = next;
                    if engine.level == 0 {
                        engine.brain.state = next;
                    }
                }
            }
        }
    }
    bail!("the script went through {STATE_LIMIT} states without waiting")
}

/// The engine as a behaviour script sees it, for one symbol.
struct Engine<'a, 'c> {
    id: i32,
    brain: &'a mut Brain,
    random: &'a mut Random,
    context: &'a mut Context<'c>,
    /// 0 while the symbol's own states run, 1 inside a chase's.
    level: usize,
}

impl Engine<'_, '_> {
    fn me(&mut self) -> &mut Character {
        self.context.scene.characters.get_mut(&self.id).expect("a symbol that thinks is in the scene")
    }

    fn mine(&self) -> &Character {
        &self.context.scene.characters[&self.id]
    }

    /// The frame of the character itself: its action and its movement (0xF483C).
    fn step(&mut self) {
        let Context { scene, resources, ground, .. } = &mut *self.context;
        if let Some(character) = scene.characters.get_mut(&self.id) {
            motion::step(character, *resources, ground);
        }
    }

    /// Where the character it is after is, or the middle of the map if there is none
    /// such (0xFA434).
    fn target_position(&self) -> Vec3 {
        let target = self.mine().mind.target;
        self.context.scene.characters.get(&target).map_or(Vec3::ZERO, |target| target.position)
    }

    /// Whether it is in a state to notice or chase anything at all (0xFAAD4).
    fn able(&self) -> bool {
        let me = self.mine();
        self.context.normal && me.flags.has(flag::SYMBOL) && me.flags.has(flag::READY)
    }

    /// Whether `place` is outside its territory: further from its home than its
    /// territory reaches, or for one that patrols from its route (0xFA9EC,
    /// 0xFA7EC).
    fn outside_territory(&self, place: Vec3) -> bool {
        let me = self.mine();
        let away = match me.mind.movement {
            MOVEMENT_STAND | MOVEMENT_WANDER => place.distance(me.mind.home),
            MOVEMENT_PATROL => self.from_route(place, 0).map_or(f32::MAX, |nearest| nearest.away),
            _ => 0.0,
        };
        me.territory < away
    }

    /// Which point of its route it was last at.
    fn at_point(&self) -> usize {
        self.brain.variables[ROUTE_AT] as u8 as usize
    }

    /// The point of its route after one, round the route (0xFAEC8).
    fn after(&self, index: usize) -> usize {
        if (index as i32) < self.mine().mind.route_length - 1 { index + 1 } else { 0 }
    }

    /// A point of its route as the map's script gave it (0xF9BD8, 0x117994):
    /// its place and how far about it one that goes there may end up.
    fn route_point(&self, index: usize) -> Option<(Vec3, f32)> {
        let number = *self.mine().mind.route.get(index)?;
        self.context.scene.route_points.get(&i32::from(number)).copied()
    }

    /// Where it goes when it goes to a point of its route (0xFBAA0): somewhere
    /// within the point's reach of it, any way from it, and on the ground if
    /// there is ground near.
    fn route_place(&mut self, index: usize) -> Option<Vec3> {
        let (point, reach) = self.route_point(index)?;
        let (far, turn) = (reach * self.random.fraction(), self.random.fraction() * std::f32::consts::TAU);
        // About the point over the ground of its own frame, whose upright the
        // ground is looked for along.
        let (lay, lie) = (self.mine().lay, self.context.ground.lie(self.mine()));
        let place = point + lay * Vec3::new(far * turn.cos(), 0.0, far * turn.sin());
        Some(lie.under(place, ROUTE_GROUND_REACH, ROUTE_GROUND_REACH).unwrap_or(place))
    }

    /// The nearest of its route to `place` (0xFB378): the nearest of its points,
    /// or a place on the stretch from one point to the next where that is
    /// nearer by more than a hundredth, the stretches taken in turn from the
    /// one that begins at `from`. None if its route has no points.
    fn from_route(&self, place: Vec3, from: usize) -> Option<Nearest> {
        let length = usize::try_from(self.mine().mind.route_length).unwrap_or(0);
        let mut nearest: Option<Nearest> = None;
        for index in 0..length {
            let Some((point, _)) = self.route_point(index) else { continue };
            let away = place.distance(point);
            if nearest.is_none_or(|nearest| away < nearest.away) {
                nearest = Some(Nearest { away, between: false, index, place: point });
            }
        }
        let mut index = from;
        for _ in 0..length {
            let next = self.after(index);
            if let (Some((first, _)), Some((second, _))) = (self.route_point(index), self.route_point(next)) {
                let along = second - first;
                let way = along.normalize_or_zero();
                let reached = way.dot(place - first);
                if reached > 0.0 && reached < along.length() {
                    let on = first + way * reached;
                    let away = place.distance(on);
                    if nearest.is_none_or(|nearest| nearest.away > away * ROUTE_NEARER) {
                        nearest = Some(Nearest { away, between: true, index, place: on });
                    }
                }
            }
            index = next;
        }
        nearest
    }

    /// Sets it walking to the point of its route after the one it was last at
    /// (0xFAEA0).
    fn walk_route(&mut self) {
        let next = self.after(self.at_point());
        if let Some(place) = self.route_place(next) {
            let me = self.me();
            me.target = place;
            me.walk();
        }
        self.me().flags.set(flag::ROUTE_PAUSED, false);
    }

    /// It has come to a point of its route and goes on to the one after
    /// (0xFAC30).
    fn route_on(&mut self) {
        let reached = self.after(self.at_point());
        let on = self.after(reached);
        debug!("symbol {} has come to point {reached} of its route at {:.0} and goes on to point {on}", self.id, self.mine().position);
        if let Some(place) = self.route_place(on) {
            let me = self.me();
            me.target = place;
            me.walk();
        }
        self.step();
        self.me().mind.on_its_way = true;
        self.brain.variables[ROUTE_AT] = reached as i8;
    }

    /// A frame of patrolling (0xFC4D4). Stopping at a point, it goes on when it
    /// has stopped for as long as it does. On its way, it stands while the field
    /// is not in its normal state, and walks on to the same point when it is
    /// again; when its walk is over it has come to the point, and stops there
    /// if it stops at points, or goes on.
    fn patrol_on(&mut self) {
        let me = self.mine();
        if !me.mind.on_its_way {
            let stopped = me.mind.wander_time + FULL_PACE;
            self.me().mind.wander_time = stopped;
            if stopped < self.mine().mind.route_pause {
                self.step();
            } else {
                self.route_on();
            }
            return;
        }
        // (0xFAE58.)
        let may = self.context.normal || !me.flags.has(flag::SYMBOL);
        if me.flags.has(flag::ROUTE_PAUSED) {
            if !may {
                self.step();
                return;
            }
            self.walk_route();
        }
        if !may {
            let me = self.me();
            me.stop();
            me.flags.set(flag::ROUTE_PAUSED, true);
            self.step();
            return;
        }
        if self.mine().acting() {
            self.step();
            return;
        }
        if self.mine().mind.route_pause > 0 {
            let me = self.me();
            me.stop();
            (me.mind.wander_time, me.mind.on_its_way) = (0, false);
            self.step();
        } else {
            self.route_on();
        }
    }

    /// Whether it notices its target (0xFB750): near enough, and inside its territory.
    fn notices(&self) -> bool {
        let me = self.mine();
        if me.flags.has(flag::UNAWARE) || !self.able() {
            return false;
        }
        let target = self.target_position();
        me.position.distance(target) < me.mind.notice_range && !self.outside_territory(target)
    }

    /// Whether the player is touching it (0xFB868).
    fn touches_player(&self) -> bool {
        let me = self.mine();
        let Some(player) = self.context.scene.player() else { return false };
        self.context.normal && !me.hidden() && me.position.distance(player.position) < me.touch_radius
    }

    /// Calls one of the script's functions that the engine calls by name, on a
    /// machine of its own (0xF9F3C). A state it names is taken up afresh.
    fn interrupt(&mut self, name: &str) -> Result<()> {
        let script = self.brain.script.clone();
        let Some(function) = script.symbol(name) else { return Ok(()) };
        let state = Machine::new(&script).call(function, &[], self).with_context(|| format!("in {name}"))?;
        if state != 0 {
            self.brain.state = state;
            self.brain.restart = true;
        }
        Ok(())
    }

    /// What the wait of frames of the states now running has counted.
    fn waited(&mut self) -> &mut i32 {
        &mut self.brain.waited[self.level]
    }

    /// Counts the hide timer and the unaware timer down (0xFBD7C).
    fn count_down(&mut self, hide: bool, unaware: bool) {
        let me = self.me();
        if hide && me.flags.has(flag::HIDES_SOON) {
            me.mind.hide_timer = (me.mind.hide_timer - FULL_PACE).max(0);
            if me.mind.hide_timer == 0 {
                me.flags.set(flag::HIDDEN, true);
            }
        }
        if unaware && me.flags.has(flag::UNAWARE) {
            me.mind.unaware_timer = (me.mind.unaware_timer - FULL_PACE).max(0);
            if me.mind.unaware_timer == 0 {
                me.flags.set(flag::UNAWARE, false);
            }
        }
    }

    /// Sets it off to a place within its wandering radius of home, if there is
    /// ground there (0xFAF38).
    fn wander(&mut self) {
        let (reach, turn) = (self.random.fraction(), self.random.fraction() * std::f32::consts::TAU);
        let me = self.mine();
        let reach = me.mind.wander_radius * reach;
        let place = me.mind.home + me.lay * Vec3::new(reach * turn.cos(), 0.0, reach * turn.sin());
        let Some(on) = self.context.ground.lie(me).under(place, WANDER_GROUND_REACH, WANDER_GROUND_REACH) else { return };
        let me = self.me();
        me.target = on;
        me.walk();
        me.mind.wander_time = 0;
        me.flags.set(flag::ROUTE_PAUSED, false);
    }

    /// A frame of wandering (0xFAD18): a new place once it has got to the last one,
    /// or has been on its way too long.
    fn wander_on(&mut self) {
        let me = self.mine();
        if !me.acting() || me.mind.wander_time > WANDER_TIME {
            self.wander();
        }
        self.step();
        self.me().mind.wander_time += FULL_PACE;
    }

    /// `WAIT_UNTIL`: waits for one of the things its argument names, the symbol
    /// standing or moving about in its own way meanwhile (0xFC248).
    fn wait_until(&mut self, call: &Call) -> Result<u32> {
        let until = call.int(0)?;
        let movement = self.mine().mind.movement;
        if !call.resumed() {
            if until & UNTIL_MOVES != 0 {
                match movement {
                    MOVEMENT_STAND => self.me().stop(),
                    MOVEMENT_WANDER => {
                        self.wander();
                        self.step();
                    }
                    // It sets out on its route (0xFABF4).
                    MOVEMENT_PATROL => {
                        self.walk_route();
                        self.step();
                        self.brain.bits |= BIT_ON_ROUTE;
                    }
                    _ => {}
                }
            }
            call.wait();
            return Ok(0);
        }

        self.count_down(true, true);
        if self.mine().hidden() {
            call.wait();
            return Ok(0);
        }
        if until & UNTIL_NOTICES != 0 && self.notices() {
            return Ok(ANSWER_NOTICED);
        }
        if until & UNTIL_AWARE != 0 && !self.mine().flags.has(flag::UNAWARE) {
            return Ok(ANSWER_AWARE);
        }
        if until & UNTIL_NEAR != 0 && self.mine().position.distance(self.target_position()) < NEAR {
            return Ok(ANSWER_NEAR);
        }
        if until & UNTIL_MOVES != 0 && movement == MOVEMENT_WANDER {
            self.wander_on();
        } else if until & UNTIL_MOVES != 0 && movement == MOVEMENT_PATROL {
            self.patrol_on();
        } else if until & UNTIL_MOVES == 0 || movement == MOVEMENT_STAND {
            self.step();
        }
        call.wait();
        Ok(0)
    }

    /// `CHASE`: goes after its target, running the states it is given inside itself,
    /// until the target is out of its territory (0xFBEBC). Answers 1.
    fn chase(&mut self, call: &Call) -> Result<u32> {
        // Each frame its walk is to where the target now is, and the way to it is
        // the way a turn would face it.
        let target = self.target_position();
        let me = self.me();
        me.target = target;
        let way = me.in_frame(target - me.position);
        if Vec2::new(way.x, way.z).length() > 1e-5 {
            me.target_facing = way.x.atan2(way.z);
        }
        if !call.resumed() {
            me.pace = pace(me.own_pace);
            me.walk();
            me.flags.set(flag::CHASING, true);
            self.brain.chase = Some(Task::new(&self.brain.script, call.int(0)? as u32));
        }

        let lost = !self.able() || (!self.touches_player() && self.outside_territory(target));
        if lost {
            // The chase is over (0xFAA68).
            self.brain.chase = None;
            let me = self.me();
            me.stop();
            me.flags.set(flag::CHASING, false);
            return Ok(1);
        }
        let Some(mut task) = self.brain.chase.take() else { bail!("a chase has lost its states") };
        self.level = 1;
        let ran = run_states(&mut task.machine, &mut task.state, self);
        self.level = 0;
        self.brain.chase = Some(task);
        ran?;
        call.wait();
        Ok(0)
    }

    /// `WALK_HOME`: walks back to its home; answers 1 if it notices its target on
    /// the way, and 0 when it is home (0xFC6CC).
    fn walk_home(&mut self, call: &Call) -> Result<u32> {
        let patrols = self.mine().mind.movement == MOVEMENT_PATROL;
        if !call.resumed() {
            // One that patrols goes back to its route (0xFA894): to the nearest
            // point of it, or, from between two points, on to the second of
            // them; and that is where it takes up its route again.
            let home = if patrols {
                let nearest = self.from_route(self.mine().position, self.at_point());
                if let Some(nearest) = nearest {
                    self.brain.variables[ROUTE_AT] = nearest.index as i8;
                }
                nearest.and_then(|nearest| match nearest.between {
                    true => self.route_point(self.after(nearest.index)).map(|(point, _)| point),
                    false => Some(nearest.place),
                })
            } else {
                Some(self.mine().mind.home)
            };
            let me = self.me();
            me.target = home.unwrap_or(me.position);
            me.walk();
            self.step();
            call.wait();
            return Ok(0);
        }
        self.count_down(true, false);
        if self.mine().hidden() {
            call.wait();
            return Ok(0);
        }
        if self.notices() {
            return Ok(1);
        }
        // Home is being near where it walks to; for one that patrols, being
        // near its route (0xFC874).
        let me = self.mine();
        let home = if patrols {
            self.from_route(me.position, 0).is_some_and(|nearest| nearest.away < ROUTE_REACH)
        } else {
            me.position.distance(me.target) < HOME_REACH
        };
        if home {
            self.step();
            self.me().stop();
            return Ok(0);
        }
        self.step();
        call.wait();
        Ok(0)
    }

    /// `ACT`: has it do something (0xD4D74).
    fn act(&mut self, action: i32) {
        let depth = self.context.resources.pop_height(self.mine().chr);
        let me = self.me();
        match action {
            ACT_WALK => me.walk(),
            ACT_TURN => me.turn(),
            ACT_STOP => me.stop(),
            ACT_DASH => me.dash(),
            // One of the ways of coming to where it stands, or something else.
            _ => {
                if !me.come_by(action, depth) {
                    debug!("a behaviour script asked for action {action}, which is not ported");
                }
            }
        }
    }

    fn set_int(&mut self, number: i32, value: i32) {
        if !self.me().set_int(number, value) {
            debug!("a behaviour script set int parameter {number}, which is not ported");
        }
    }
}

impl script::Host for Engine<'_, '_> {
    fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
        Ok(match number {
            native::SET_VARIABLE => {
                let (index, value) = (call.int(0)?, call.int(1)?);
                let variable = usize::try_from(index).ok().and_then(|index| self.brain.variables.get_mut(index));
                *variable.with_context(|| format!("there is no variable {index}"))? = value as i8;
                0
            }
            native::VARIABLE => {
                let index = call.int(0)?;
                let variable = usize::try_from(index).ok().and_then(|index| self.brain.variables.get(index));
                *variable.with_context(|| format!("there is no variable {index}"))? as i32 as u32
            }
            native::SET_BIT => {
                let (bit, on) = (1u32.wrapping_shl(call.int(0)? as u32), call.int(1)? != 0);
                self.brain.bits = if on { self.brain.bits | bit } else { self.brain.bits & !bit };
                0
            }
            native::BIT => self.brain.bits & 1u32.wrapping_shl(call.int(0)? as u32),
            // Waits so many frames, the character going on with what it does (0xFA638).
            native::WAIT => {
                let frames = call.int(0)?;
                if !call.resumed() {
                    if frames != 0 {
                        *self.waited() = 0;
                        self.step();
                        call.wait();
                    }
                } else {
                    *self.waited() += FULL_PACE;
                    self.step();
                    if *self.waited() < frames * FULL_PACE {
                        call.wait();
                    }
                }
                0
            }
            native::CHASE => self.chase(call)?,
            // Has the map's script run its event for this character, and waits for
            // the event to be over (0xFAB54).
            native::EVENT => {
                if !call.resumed() {
                    self.context.events.push(self.id);
                    call.wait();
                } else if !self.context.normal {
                    call.wait();
                }
                0
            }
            native::WAIT_UNTIL => self.wait_until(call)?,
            // It has noticed its target (0xFAD98): its cry is heard, the mark of
            // it comes up over it, and it is after her.
            native::ALERT => {
                self.context.scene.sounds.push(Request::common(SOUND_NOTICED));
                let me = self.me();
                me.show_mark();
                me.flags.set(flag::CHASING, true);
                0
            }
            native::RESTART => {
                self.brain.restart = true;
                0
            }
            native::REMEMBER_HOME => {
                let me = self.me();
                me.mind.home = me.position;
                0
            }
            native::WALK_HOME => self.walk_home(call)?,
            native::FLAG => self.mine().flags.has(call.int(0)? as u32) as u32,
            native::NOTHING => 0,
            // The state it is in: a chase's own states do not count (0xFB254).
            native::STATE => self.brain.state,
            native::PRINT => 0,
            native::ACT => {
                self.act(call.int(0)?);
                0
            }
            native::SET => {
                let (what, value) = (call.int(0)?, call.int(1)?);
                self.me().set(what, value);
                0
            }
            native::SET_FLAG => {
                let (number, on) = (call.int(0)? as u32, call.int(1)? != 0);
                self.me().flags.set(number, on);
                0
            }
            native::SET_INT => {
                self.set_int(call.int(0)?, call.int(1)?);
                0
            }
            // Waits for what it is doing to end (0xFA738).
            native::WAIT_ACTION => {
                if self.mine().acting() {
                    self.step();
                    call.wait();
                }
                0
            }
            native::SYSTEM_VALUE => {
                let tunables = self.context.tunables;
                match call.int(0)? {
                    SYSTEM_ALERT_FRAMES => tunables.u16(PRM_ALERT_FRAMES)? as u32,
                    SYSTEM_DASH_PACE => tunables.u16(PRM_DASH_PACE)? as u32,
                    other => {
                        debug!("a behaviour script asked for system value {other:#x}, which is not ported");
                        0
                    }
                }
            }
            // Waits for its model to have loaded; here it always has (0xFB2BC).
            native::WAIT_READY => 0,
            native::INT => self.mine().int(call.int(0)?) as u32,
            // One of the game's own switches for testing: none is on (0x14D9B0).
            native::GAME_FLAG => 0,
            _ => bail!("the behaviour script called engine function {number:#x}, which is not ported"),
        })
    }
}

#[cfg(test)]
mod tests {
    use brs_engine::collision::{Body, CollisionWorld};

    use super::*;
    use crate::field::motion::tests::{floor, Speeds};
    use crate::field::scene::{setting, status, test_defaults, Action, PLAYER};
    use crate::field::tables::FieldTables;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    struct Field {
        brains: Brains,
        tables: FieldTables,
        scene: Scene,
        world: CollisionWorld,
        body: Body,
    }

    /// A flat field with the player at `player` and one Eater symbol at the middle.
    fn field(player: Vec3) -> Option<Field> {
        let data = GameData::new(DATA).ok()?;
        let brains = Brains::load(&data).expect("the behaviour scripts load");
        let tables = FieldTables::load(&data).expect("the field's tables load");
        let defaults = test_defaults();
        let mut scene = Scene::new(0, 0, defaults);
        scene.characters.insert(PLAYER, Character::new(0, player, 0.0, &defaults));
        let mut symbol = Character::new(21, Vec3::ZERO, 0.0, &defaults);
        symbol.set(setting::BEHAVIOUR, 0);
        symbol.set(setting::SYMBOL, 1);
        scene.characters.insert(500, symbol);
        Some(Field { brains, tables, scene, world: floor(), body: motion::body(40.0) })
    }

    impl Field {
        /// A frame of the symbols; answers the events asked for.
        fn frame(&mut self) -> Vec<i32> {
            let mut context = Context {
                scene: &mut self.scene,
                tunables: &self.tables.tunables,
                ground: &Ground { world: &self.world, body: &self.body, walls: &[], solids: &[], frames: None },
                resources: &Speeds,
                normal: true,
                in_event: None,
                events: Vec::new(),
            };
            self.brains.update(&mut context);
            context.events
        }

        fn symbol(&self) -> &Character {
            &self.scene.characters[&500]
        }
    }

    #[test]
    fn a_symbol_stands_until_the_player_comes_within_its_notice() {
        let Some(mut field) = field(Vec3::new(0.0, 0.0, 1900.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        for _ in 0..60 {
            assert!(field.frame().is_empty());
        }
        let symbol = field.symbol();
        assert_eq!(symbol.position, Vec3::ZERO);
        assert!(!symbol.flags.has(flag::CHASING));
        assert_eq!(symbol.status, status::STAND);

        // She comes within its 1800: it is alerted, waits half a second, and comes.
        assert!(field.scene.sounds.is_empty() && field.symbol().mark.is_none());
        field.scene.characters.get_mut(&PLAYER).unwrap().position.z = 1700.0;
        field.frame();
        field.frame();
        assert!(field.symbol().flags.has(flag::CHASING));
        assert_eq!(field.symbol().position, Vec3::ZERO);
        // With its cry, once, and the mark of it over it, which is gone when its
        // set has been played: 54 frames of it, at two a frame of the game.
        assert_eq!(field.scene.sounds, [Request::common(0x2f)]);
        assert!(field.symbol().mark.is_some());
        let mut frames = 0;
        while field.scene.characters[&500].mark.is_some() {
            field.scene.characters.get_mut(&500).unwrap().step_mark();
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(frames, 27);
        for _ in 0..40 {
            field.frame();
        }
        assert_eq!(field.scene.sounds.len(), 1);
        let symbol = field.symbol();
        assert!(symbol.position.z > 300.0, "it should be on its way: {}", symbol.position);
        assert_eq!(symbol.action, Action::Walk);
        // It goes about at its own pace, which is a little short of full.
        assert_eq!(symbol.pace, pace(95));
    }

    #[test]
    fn a_chasing_symbol_that_reaches_the_player_asks_for_its_event() {
        let Some(mut field) = field(Vec3::new(0.0, 0.0, 900.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut events = Vec::new();
        for _ in 0..200 {
            events = field.frame();
            if !events.is_empty() {
                break;
            }
        }
        assert_eq!(events, [500]);
        let symbol = field.symbol();
        assert!(symbol.position.distance(Vec3::new(0.0, 0.0, 900.0)) < symbol.touch_radius);
    }

    #[test]
    fn a_symbol_gives_up_when_the_player_leaves_its_territory_and_walks_home() {
        let Some(mut field) = field(Vec3::new(0.0, 0.0, 1500.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        for _ in 0..30 {
            field.frame();
        }
        assert!(field.symbol().flags.has(flag::CHASING));
        // She is gone: far outside the 2000 of its territory.
        field.scene.characters.get_mut(&PLAYER).unwrap().position.z = 9000.0;
        field.frame();
        assert!(!field.symbol().flags.has(flag::CHASING));
        // After a pause of two seconds it walks home and stands there.
        for _ in 0..400 {
            field.frame();
        }
        let symbol = field.symbol();
        assert!(symbol.position.length() < HOME_REACH, "it should be home: {}", symbol.position);
        assert_eq!(symbol.status, status::STAND);
        assert_eq!(symbol.pace, FULL_PACE);
    }

    #[test]
    fn a_symbol_told_of_a_battle_stands_down_for_two_seconds() {
        let Some(mut field) = field(Vec3::new(0.0, 0.0, 1500.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        for _ in 0..30 {
            field.frame();
        }
        assert!(field.symbol().flags.has(flag::CHASING));
        // A battle has been fought: the system script has every symbol told.
        field.scene.characters.get_mut(&500).unwrap().mind.stand_down = true;
        field.frame();
        let symbol = field.symbol();
        assert!(!symbol.mind.stand_down);
        assert!(symbol.flags.has(flag::UNAWARE) && symbol.status == status::STAND);
        let stood_at = symbol.position;
        // For sixty frames she is nothing to it, near as she is.
        for _ in 0..58 {
            assert!(field.frame().is_empty());
            assert_eq!(field.symbol().position, stood_at);
        }
        // Then it notices her again and comes on.
        for _ in 0..60 {
            field.frame();
        }
        let symbol = field.symbol();
        assert!(!symbol.flags.has(flag::UNAWARE));
        assert!(symbol.position.z > stood_at.z, "it should be after her again: {}", symbol.position);
    }

    #[test]
    fn a_symbol_that_has_not_noticed_the_player_is_not_touched_by_her() {
        let Some(mut field) = field(Vec3::new(0.0, 0.0, 50.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // Unaware of everything, as after a battle that was got away from.
        let symbol = field.scene.characters.get_mut(&500).unwrap();
        symbol.flags.set(flag::UNAWARE, true);
        symbol.mind.unaware_timer = 60 * FULL_PACE;
        for _ in 0..30 {
            assert!(field.frame().is_empty(), "it is not chasing, so her touch is nothing to it");
        }
    }

    /// The symbol given a route of two of the map's points, six hundred apart
    /// along the way ahead, to patrol.
    fn patrolling(player: Vec3) -> Option<Field> {
        let mut field = field(player)?;
        field.scene.route_points.insert(4, (Vec3::ZERO, 0.0));
        field.scene.route_points.insert(5, (Vec3::new(0.0, 0.0, 600.0), 0.0));
        let symbol = field.scene.characters.get_mut(&500).unwrap();
        symbol.set(setting::MOVEMENT, MOVEMENT_PATROL);
        (symbol.mind.route[0], symbol.mind.route[1]) = (4, 5);
        symbol.set(setting::ROUTE_LENGTH, 2);
        Some(field)
    }

    #[test]
    fn a_symbol_that_patrols_goes_round_its_route() {
        let Some(mut field) = patrolling(Vec3::new(0.0, 0.0, 9000.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // It sets out for the point after the one it is at, comes to it, and
        // goes on to the next, which is back where it began: to and fro.
        let mut turns = Vec::new();
        let mut last = 0.0;
        let mut going_out = true;
        for frame in 0..200 {
            assert!(field.frame().is_empty());
            let z = field.symbol().position.z;
            if (z > last) != going_out && z != last {
                turns.push((frame, last));
                going_out = !going_out;
            }
            last = z;
        }
        assert!(turns.len() >= 4, "{turns:?}");
        // It turns back within two frames' walk of each point.
        for (count, (_, at)) in turns.iter().enumerate() {
            let point = if count % 2 == 0 { 600.0 } else { 0.0 };
            assert!((at - point).abs() <= 64.0, "{turns:?}");
        }
        assert_eq!(field.symbol().action, Action::Walk);

        // One that stops at its points stands at each for as long as it is told.
        let mut field = patrolling(Vec3::new(0.0, 0.0, 9000.0)).unwrap();
        field.scene.characters.get_mut(&500).unwrap().set(setting::ROUTE_PAUSE, 20);
        let mut stood = 0;
        let mut frames = 0;
        while stood == 0 || field.symbol().status == status::STAND {
            field.frame();
            frames += 1;
            stood += (field.symbol().status == status::STAND) as i32;
            assert!(frames < 200, "it should go on");
        }
        assert!((20..=22).contains(&stood), "{stood}");
        assert!(field.symbol().position.z > 500.0 && field.symbol().mind.on_its_way);

        // And while the field is not in its normal state it stands where it is,
        // to go on to the same point after.
        let mut field = patrolling(Vec3::new(0.0, 0.0, 9000.0)).unwrap();
        for _ in 0..5 {
            field.frame();
        }
        let at = field.symbol().position;
        assert!(at.z > 0.0);
        for _ in 0..10 {
            let mut context = Context {
                scene: &mut field.scene,
                tunables: &field.tables.tunables,
                ground: &Ground { world: &field.world, body: &field.body, walls: &[], solids: &[], frames: None },
                resources: &Speeds,
                normal: false,
                in_event: None,
                events: Vec::new(),
            };
            field.brains.update(&mut context);
        }
        assert!(field.symbol().flags.has(flag::ROUTE_PAUSED) && field.symbol().position == at);
        for _ in 0..5 {
            field.frame();
        }
        assert!(!field.symbol().flags.has(flag::ROUTE_PAUSED) && field.symbol().position.z > at.z);
    }

    #[test]
    fn the_territory_of_a_symbol_that_patrols_is_about_its_route() {
        // She stands 2300 from where it begins, which is outside the 2000 of a
        // territory about its home, and no nearer than 1700 to its route.
        let Some(mut field) = patrolling(Vec3::new(0.0, 0.0, 2300.0)) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // It does not notice her from where it begins, 2300 off, but does from
        // the far end of its route, 1700 off: and that is inside its territory.
        let mut noticed = None;
        for frame in 0..100 {
            field.frame();
            if field.symbol().flags.has(flag::CHASING) {
                noticed = Some((frame, field.symbol().position.z));
                break;
            }
        }
        let (_, from) = noticed.expect("it should notice her from its route");
        assert!(from > 500.0, "{from}");
        // She goes: it gives her up and walks back to its route, not to where
        // it began, and takes its route up again.
        field.scene.characters.get_mut(&PLAYER).unwrap().position = Vec3::new(5000.0, 0.0, 300.0);
        let mut back = false;
        for _ in 0..600 {
            field.frame();
            let symbol = field.symbol();
            back |= !symbol.flags.has(flag::CHASING) && symbol.pace == FULL_PACE && symbol.action == Action::Walk && symbol.position.x.abs() < 1.0;
        }
        assert!(back, "it should be on its route again: {}", field.symbol().position);
    }
}
