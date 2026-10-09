//! How an enemy moves: a port of the "movement block" of the original's enemy actor
//! (its fields from 0x840) and of the code that runs it each frame (0xB9B50).
//!
//! A script starts a move of some kind and length; each frame the move works out
//! where the enemy is now, and the enemy turns towards an angle the move has set.
//! Every kind of move a script starts is here, the bosses' own (10000 and up)
//! with the rest; a kind there is no code for ends at once, as an unknown kind does
//! in the original. `docs/BATTLE_NOTES.md` lists them.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::battle::damage::Random;

/// Bits of [`Mover::flags`].
pub mod flag {
    pub const MOVING: u32 = 1;
    /// The move is held where it is; only the turn goes on.
    pub const HOLD: u32 = 2;
    pub const ENDED: u32 = 4;
    // Three bits scripts set and read for their own purposes.
    pub const SCRIPT_8: u32 = 8;
    pub const SCRIPT_10: u32 = 0x10;
    pub const SCRIPT_20: u32 = 0x20;
}

/// The kinds of move, by the numbers scripts start them with. Each family of enemies
/// has its own hundred or so; they are named here by the first of the family as the
/// game calls it (`tools_re/enemy_variants.py` lists the names).
pub mod kind {
    // Everybody's turns and waits, and the Eaters' (`egg`).
    pub const FACE_PLAYER: i32 = 10;
    pub const DROP_IN: i32 = 11;
    pub const FACE_PLAYER_FROM_HERE: i32 = 12;
    pub const RISE: i32 = 13;
    pub const KEEP_FACING: i32 = 14;
    pub const WAIT: i32 = 15;
    /// Turn to a bearing given in degrees in place of a length.
    pub const FACE_BEARING: i32 = 16;
    /// Turn a node of the model towards the player, the body staying as it is.
    pub const TURN_HEAD: i32 = 17;
    pub const HOME: i32 = 100;
    pub const NEAR_HOME: i32 = 101;
    pub const APPROACH: i32 = 102;
    pub const APPROACH_CLOSER: i32 = 103;
    pub const APPROACH_CLOSEST: i32 = 104;
    pub const HOP_HOME: i32 = 105;
    pub const BACK_AWAY: i32 = 106;
    pub const BACK_AWAY_FAR: i32 = 107;
    pub const ADVANCE_FIRST: i32 = 108;
    pub const ADVANCE_LAST: i32 = 110;
    pub const RUSH: i32 = 111;
    // The Hornets' (`bee`): drifting between places close to home, darting between
    // ones further apart, falling to the ground and rising back to its height.
    pub const HOVER: i32 = 301;
    pub const DART: i32 = 302;
    pub const FALL: i32 = 303;
    pub const RISE_HOME: i32 = 304;
    // The Chicken's (`cat`): a hop to another of nine places round home, an approach
    // in two moves, a high vault between those places, down onto the ground where
    // it is, and a hop back from what it aims at.
    pub const SPRING: i32 = 201;
    pub const PROWL: i32 = 202;
    pub const PROWL_CLOSER: i32 = 203;
    pub const VAULT: i32 = 204;
    pub const GROUND: i32 = 205;
    pub const RECOIL: i32 = 206;
    /// A leap to a set distance from the player, or on to where the last one aimed.
    pub const LEAP_FIRST: i32 = 207;
    pub const LEAP_LAST: i32 = 209;
    // The Spider's (`apl`): to six set places off home, to in front of the player,
    // home.
    pub const TRUDGE_NEAR_FIRST: i32 = 400;
    pub const TRUDGE_NEAR_LAST: i32 = 401;
    pub const TRUDGE_FAR_FIRST: i32 = 410;
    pub const TRUDGE_FAR_LAST: i32 = 412;
    pub const TRUDGE_AT_PLAYER: i32 = 413;
    pub const TRUDGE_HOME: i32 = 414;
    /// A leap home backwards, and a hop on the spot.
    pub const LEAP_HOME: i32 = 415;
    pub const HOP_ON_SPOT: i32 = 416;
    // The Hand's (`had`): three advances that end on the ground, and a step to
    // another of a few places round home.
    pub const TREAD_FIRST: i32 = 501;
    pub const TREAD_LAST: i32 = 503;
    pub const STEP: i32 = 504;
    // The Wheel's (`tir`). A turn to the player that is over at once; up close to
    // her, straight or with a swerve, and on to a stop an approach noted.
    pub const TURN_TO_PLAYER: i32 = 601;
    pub const CLOSE_IN: i32 = 602;
    pub const SWERVE_IN: i32 = 603;
    pub const CLOSE_IN_FURTHER: i32 = 604;
    // Round a wide arc to a place beside home, on either side, and straight home.
    pub const ARC_FIRST: i32 = 605;
    pub const ARC_LAST: i32 = 606;
    pub const STRAIGHT_HOME_FIRST: i32 = 607;
    pub const STRAIGHT_HOME_LAST: i32 = 608;
    // Off the stage to either side; waiting out there level with a point near the
    // player, at either of two distances from her; in to either side of that
    // point, or to one nearer her; the same facing her side of the stage; and back
    // in from either side to home.
    pub const OFFSTAGE_FIRST: i32 = 609;
    pub const OFFSTAGE_LAST: i32 = 610;
    pub const LURK_FIRST: i32 = 611;
    pub const LURK_LAST: i32 = 612;
    pub const LURK_NEAR_FIRST: i32 = 613;
    pub const LURK_NEAR_LAST: i32 = 614;
    pub const CHARGE_FIRST: i32 = 615;
    pub const CHARGE_LAST: i32 = 616;
    pub const CHARGE_AT: i32 = 617;
    pub const SIDLE_FIRST: i32 = 618;
    pub const SIDLE_LAST: i32 = 619;
    pub const RETURN_FIRST: i32 = 620;
    pub const RETURN_LAST: i32 = 621;
    // The Mini Rabbits' (`nafc`). A drift like the Cubes', between places closer in,
    // which no script starts; flying in to home from one of four places far off.
    pub const DRIFT: i32 = 800;
    pub const ARRIVE_FIRST: i32 = 801;
    pub const ARRIVE_LAST: i32 = 804;
    /// Sixteen swerves to places off home: four formations of four.
    pub const WEAVE_FIRST: i32 = 805;
    pub const WEAVE_LAST: i32 = 820;
    /// A glide that follows the player, ending close in front of her.
    pub const CHASE: i32 = 821;
    /// Darts to three set places off home.
    pub const SWOOP_FIRST: i32 = 822;
    pub const SWOOP_LAST: i32 = 824;
    // John Doe's (`cof`), who vanishes and is somewhere else: at six places off home
    // in two rows, in front of the player, at home.
    pub const VANISH_NEAR_FIRST: i32 = 900;
    pub const VANISH_NEAR_LAST: i32 = 902;
    pub const VANISH_FAR_FIRST: i32 = 910;
    pub const VANISH_FAR_LAST: i32 = 912;
    pub const VANISH_TO_PLAYER: i32 = 913;
    pub const VANISH_HOME: i32 = 914;
    // Big Mouth's (`yub`): stride by stride towards the player, to a nearer or a
    // further distance from her, and back out to a set distance.
    pub const STOMP: i32 = 1001;
    pub const STOMP_FAR: i32 = 1002;
    pub const STOMP_BACK: i32 = 1003;
    // The Wall's (`shi`): three advances through the air, facing the player's side
    // of the stage; up by a set height, and down again by as much.
    pub const GLIDE_FIRST: i32 = 1101;
    pub const GLIDE_LAST: i32 = 1103;
    pub const LIFT: i32 = 1104;
    pub const SINK: i32 = 1105;
    // The Gunner's (`hul`): to five places across the stage, striding, and with a
    // hop.
    pub const CROSS_FIRST: i32 = 1200;
    pub const CROSS_LAST: i32 = 1204;
    pub const CROSS_HOP_FIRST: i32 = 1205;
    pub const CROSS_HOP_LAST: i32 = 1209;
    // The Masher's (`hua`): to six places off home, in a nearer and a further row,
    // running, and hopping.
    pub const LUNGE_NEAR_FIRST: i32 = 1300;
    pub const LUNGE_NEAR_LAST: i32 = 1301;
    pub const LUNGE_FAR_FIRST: i32 = 1310;
    pub const LUNGE_FAR_LAST: i32 = 1312;
    pub const HOP_NEAR_FIRST: i32 = 1320;
    pub const HOP_NEAR_LAST: i32 = 1321;
    pub const HOP_FAR_FIRST: i32 = 1330;
    pub const HOP_FAR_LAST: i32 = 1332;
    /// A low hop to in front of the player, and one back home.
    pub const POUNCE: i32 = 1333;
    pub const POUNCE_HOME: i32 = 1334;
    /// The Cubes' drift between places round home (`cub`).
    pub const FLOAT: i32 = 1400;
    // Cone Head's (`egb`), which are like the Eaters': home, an approach in three
    // moves to distances of its own, a high bound towards the player, a hop home,
    // and the same three advances.
    pub const ROLL_HOME: i32 = 1500;
    pub const ROLL_UP: i32 = 1501;
    pub const ROLL_CLOSER: i32 = 1502;
    pub const ROLL_CLOSEST: i32 = 1503;
    pub const BOUND: i32 = 1504;
    pub const ROLL_HOP_HOME: i32 = 1505;
    pub const ROLL_FIRST: i32 = 1506;
    pub const ROLL_LAST: i32 = 1508;
    // Mefe's (`mee`), the first boss. A leap to the first of three places before
    // home, and the three after it to the place each names; then the same four to
    // home and the places beside it.
    pub const MEFE_FRONT: i32 = 10000;
    pub const MEFE_FRONT_LAST: i32 = 10003;
    pub const MEFE_BACK: i32 = 10004;
    pub const MEFE_BACK_LAST: i32 = 10007;
    /// Up to the player, to one side of the line to her, noting three stops close
    /// by her; on to the second of them, and to the third.
    pub const MEFE_CLOSE: i32 = 10008;
    pub const MEFE_CLOSER: i32 = 10010;
    pub const MEFE_ACROSS: i32 = 10011;
    /// Down onto the ground where it is. (Two more, 10012 and 10014, no script
    /// starts.)
    pub const MEFE_GROUND: i32 = 10013;
    // Mazuma's (`maz`), the second boss, who is in one place and then all at once
    // in another: at one of four places off home, before the player, at home.
    pub const MAZUMA_PLACE: i32 = 10100;
    pub const MAZUMA_PLACE_LAST: i32 = 10103;
    pub const MAZUMA_AT_PLAYER: i32 = 10104;
    pub const MAZUMA_HOME: i32 = 10105;
    /// A leap home.
    pub const MAZUMA_LEAP_HOME: i32 = 10106;
    /// Flying in from far off to either side, at one of four heights.
    pub const MAZUMA_FLY_IN: i32 = 10107;
    pub const MAZUMA_FLY_IN_LAST: i32 = 10110;
    // The third boss's (`cyzkal`): to nine places about home, turning to the
    // player; up to the player's ankle, and closer; to the nine places again,
    // turned back the way it came; and a pass across the stage before her, one way
    // and the other.
    pub const PAIR_PLACE: i32 = 10202;
    pub const PAIR_PLACE_LAST: i32 = 10210;
    pub const PAIR_CLOSE: i32 = 10211;
    pub const PAIR_CLOSER: i32 = 10212;
    pub const PAIR_BACK: i32 = 10213;
    pub const PAIR_BACK_LAST: i32 = 10221;
    pub const PAIR_PASS: i32 = 10222;
    pub const PAIR_PASS_BACK: i32 = 10223;
    // The fourth's (`rir`), who is somewhere else all at once: at five places off
    // home, before the player, at home, far behind home; and a glide to three
    // places beside home.
    pub const LIRIO_PLACE: i32 = 10300;
    pub const LIRIO_PLACE_LAST: i32 = 10304;
    pub const LIRIO_AT_PLAYER: i32 = 10305;
    pub const LIRIO_HOME: i32 = 10306;
    pub const LIRIO_AWAY: i32 = 10307;
    pub const LIRIO_GLIDE: i32 = 10308;
    pub const LIRIO_GLIDE_LAST: i32 = 10310;
    // The fifth's (`naf`), who hangs over home: to its place there, down to the
    // ground where it is, and back up.
    pub const NAFEHE_HOME: i32 = 10400;
    pub const NAFEHE_DOWN: i32 = 10402;
    pub const NAFEHE_UP: i32 = 10403;
    // The sixth's (`zah`): to home or beside it by a path that goes past its end
    // and comes back; after the player; straight home; and at once to before the
    // player, or home.
    pub const ZAHA_PLACE: i32 = 10500;
    pub const ZAHA_PLACE_LAST: i32 = 10502;
    pub const ZAHA_CHASE: i32 = 10503;
    pub const ZAHA_HOME: i32 = 10504;
    pub const ZAHA_AT_PLAYER: i32 = 10505;
    pub const ZAHA_TO_HOME: i32 = 10506;
    // The seventh's (`wrs01`), all of them at once: at five places off home, at
    // the player's feet, at home, at three places beside home.
    pub const WHITE_PLACE: i32 = 10600;
    pub const WHITE_PLACE_LAST: i32 = 10604;
    pub const WHITE_AT_PLAYER: i32 = 10605;
    pub const WHITE_HOME: i32 = 10606;
    pub const WHITE_SIDE: i32 = 10607;
    pub const WHITE_SIDE_LAST: i32 = 10609;
    // The last's (`wrs02`): at once at three places off home, before the player,
    // at home; a sweep to three places beside home, and at once to them; after
    // the player; and away, far over the stage.
    pub const LAST_PLACE: i32 = 10700;
    pub const LAST_PLACE_LAST: i32 = 10702;
    pub const LAST_AT_PLAYER: i32 = 10703;
    pub const LAST_HOME: i32 = 10704;
    pub const LAST_SWEEP: i32 = 10705;
    pub const LAST_SWEEP_LAST: i32 = 10707;
    pub const LAST_SIDE: i32 = 10708;
    pub const LAST_SIDE_LAST: i32 = 10710;
    pub const LAST_CHASE: i32 = 10711;
    pub const LAST_GONE: i32 = 10712;
}

/// Every kind of move the original has code for, as runs of numbers: the cases of
/// the switch at 0xB9D74 (`tools_re/move_kinds_all.py` follows it for each number).
/// Any other kind ends at once there too.
const ORIGINAL_KINDS: [(i32, i32); 31] = [
    (10, 17),
    (100, 111),
    (201, 209),
    (301, 304),
    (400, 401),
    (410, 416),
    (501, 504),
    (601, 621),
    (800, 824),
    (900, 902),
    (910, 914),
    (1001, 1003),
    (1101, 1105),
    (1200, 1209),
    (1300, 1301),
    (1310, 1312),
    (1320, 1321),
    (1330, 1334),
    (1400, 1400),
    (1500, 1508),
    (10000, 10008),
    (10010, 10014),
    (10100, 10110),
    (10200, 10200),
    (10202, 10223),
    (10300, 10310),
    (10400, 10400),
    (10402, 10403),
    (10500, 10506),
    (10600, 10609),
    (10700, 10712),
];

/// Whether the original does anything for a move of this kind.
pub fn original_has(kind: i32) -> bool {
    ORIGINAL_KINDS.iter().any(|(first, last)| (*first..=*last).contains(&kind))
}

/// Places near its home an enemy picks between (the table at 0x16A718).
const NEAR_HOME: [Vec3; 5] = [
    Vec3::ZERO,
    Vec3::new(0.0, 0.0, 200.0),
    Vec3::new(0.0, 0.0, 400.0),
    Vec3::new(50.0, 0.0, 0.0),
    Vec3::new(-50.0, 0.0, 0.0),
];

/// Places round its home a flying enemy drifts between (the table at 0x16A628), and
/// the ones further out it darts between (0x16A6A0).
const HOVER_PLACES: [Vec3; 10] = [
    Vec3::ZERO,
    Vec3::new(0.0, 50.0, 0.0),
    Vec3::new(0.0, -50.0, 0.0),
    Vec3::new(-50.0, 0.0, 0.0),
    Vec3::new(-50.0, 50.0, 0.0),
    Vec3::new(-50.0, -50.0, 0.0),
    Vec3::new(50.0, 0.0, 0.0),
    Vec3::new(50.0, 50.0, 0.0),
    Vec3::new(50.0, -50.0, 0.0),
    Vec3::new(0.0, 0.0, -250.0),
];
const DART_PLACES: [Vec3; 10] = [
    Vec3::ZERO,
    Vec3::new(0.0, 125.0, 0.0),
    Vec3::new(0.0, -125.0, 0.0),
    Vec3::new(-250.0, 0.0, 0.0),
    Vec3::new(-250.0, 125.0, 0.0),
    Vec3::new(-250.0, -125.0, 0.0),
    Vec3::new(250.0, 0.0, 0.0),
    Vec3::new(250.0, 125.0, 0.0),
    Vec3::new(250.0, -125.0, 0.0),
    Vec3::new(0.0, 0.0, -100.0),
];

/// The places round home of a step (0x16A9DC), the three a swoop goes to (0x16A904)
/// and the ones the Cubes float between (0x16AAE4).
const STEP_PLACES: [Vec3; 5] = [
    Vec3::ZERO,
    Vec3::new(50.0, 0.0, 80.0),
    Vec3::new(-50.0, 0.0, 80.0),
    Vec3::new(50.0, 0.0, -80.0),
    Vec3::new(-50.0, 0.0, -80.0),
];
const SWOOP_PLACES: [Vec3; 3] = [Vec3::new(100.0, 100.0, -50.0), Vec3::new(-80.0, -100.0, 20.0), Vec3::new(40.0, 1000.0, 400.0)];
/// Where the four ways of flying in start from, off home (0x16A814), and what the
/// middle of the way is stretched by to make its curve (0xB1A18).
const ARRIVE_FROM: [Vec3; 4] = [
    Vec3::new(2000.0, 1200.0, 500.0),
    Vec3::new(1800.0, 500.0, 500.0),
    Vec3::new(-1800.0, 600.0, 500.0),
    Vec3::new(-2000.0, 1000.0, 500.0),
];
const ARRIVE_CURVE: Vec3 = Vec3::new(0.5, 3.5, 4.0);
/// The places off home of the four formations a swerve ends in (0x16A844).
const WEAVE_PLACES: [[Vec3; 4]; 4] = [
    [Vec3::ZERO; 4],
    [Vec3::new(0.0, 200.0, -100.0), Vec3::new(0.0, 300.0, -50.0), Vec3::new(0.0, 300.0, -50.0), Vec3::new(0.0, 200.0, -100.0)],
    [Vec3::new(0.0, 300.0, -100.0), Vec3::new(0.0, 300.0, -50.0), Vec3::new(0.0, 300.0, -50.0), Vec3::new(0.0, 300.0, -100.0)],
    [
        Vec3::new(-250.0, 200.0, -100.0),
        Vec3::new(-100.0, 225.0, -250.0),
        Vec3::new(100.0, 300.0, -100.0),
        Vec3::new(250.0, 125.0, -250.0),
    ],
];
/// How far a swerve's path is pulled to one side, up or down, and towards the
/// player's side of the stage (0xB1C60).
const WEAVE_SIDE: f32 = 400.0;
const WEAVE_UP: f32 = 300.0;
const WEAVE_DOWN: f32 = -100.0;
const WEAVE_DEPTH: f32 = -300.0;
/// The five places across the stage (0x16AA60), the two rows of three off home
/// that lunges and hops go to (0x16AA9C), and the Spider's two rows (0x16A994).
const CROSS_PLACES: [Vec3; 5] = [
    Vec3::new(600.0, 0.0, -90.0),
    Vec3::new(300.0, 0.0, -50.0),
    Vec3::ZERO,
    Vec3::new(-300.0, 0.0, -50.0),
    Vec3::new(-600.0, 0.0, -90.0),
];
const LUNGE_PLACES: [[Vec3; 3]; 2] = [
    [Vec3::new(300.0, 0.0, -300.0), Vec3::new(-300.0, 0.0, -280.0), Vec3::ZERO],
    [Vec3::new(600.0, 0.0, -710.0), Vec3::new(0.0, 0.0, -700.0), Vec3::new(-600.0, 0.0, -720.0)],
];
const TRUDGE_PLACES: [[Vec3; 3]; 2] = [
    [Vec3::new(150.0, 0.0, -300.0), Vec3::new(-150.0, 0.0, -280.0), Vec3::ZERO],
    [Vec3::new(300.0, 0.0, -710.0), Vec3::new(0.0, 0.0, -700.0), Vec3::new(-300.0, 0.0, -720.0)],
];
/// How far short of the player the Spider's walk up to her ends (0xB2F7C).
const TRUDGE_STOP: f32 = 450.0;
/// How far short of her each tread and each glide stops (0xB3474, 0xB541C), and the
/// two leaps (0xB2CB8), whose paths are drawn this high (0xB2D6C).
const TREAD_STOPS: [f32; 3] = [1000.0, 650.0, 350.0];
const LEAP_STOPS: [f32; 2] = [800.0, 350.0];
const LEAP_HEIGHT: f32 = 150.0;
const FLOAT_PLACES: [Vec3; 11] = [
    Vec3::ZERO,
    Vec3::new(0.0, 50.0, 0.0),
    Vec3::new(0.0, -50.0, 0.0),
    Vec3::new(-150.0, 0.0, 0.0),
    Vec3::new(-150.0, 50.0, 0.0),
    Vec3::new(-150.0, -50.0, 0.0),
    Vec3::new(150.0, 0.0, 0.0),
    Vec3::new(150.0, 50.0, 0.0),
    Vec3::new(150.0, -50.0, 0.0),
    Vec3::new(0.0, 0.0, -300.0),
    Vec3::new(0.0, 0.0, 300.0),
];

/// How far short of the player a chase ends, and how high (0xB1F04).
const CHASE_STOP: f32 = 150.0;
const CHASE_HEIGHT: f32 = 70.0;
/// How far short of her Big Mouth's walks end: its two advances and its
/// way back out (0xB4E40, 0xB4E68, 0xB50D0).
const STOMP_STOP: f32 = 575.0;
const STOMP_FAR_STOP: f32 = 800.0;
const STOMP_BACK_STOP: f32 = 900.0;
/// Its stride, in sixtieths at the usual step: it moves only while a foot is off
/// the ground, which is outside these two stretches of each stride (0xB4CB0).
const STRIDE: f32 = 126.0;
const PLANTED: [(f32, f32); 2] = [(8.0, 50.0), (70.0, 112.0)];
const STRIDES: usize = 10;
/// How far a lift goes, and how far past that its path is drawn (0xB5684).
const LIFT_HEIGHT: f32 = 300.0;
/// How far short of the player a pounce lands, and how high its path is drawn
/// (0xB5CBC, 0xB5E10).
const POUNCE_STOP: f32 = 600.0;
const POUNCE_HEIGHT: f32 = 80.0;

/// The nine places round home the Chicken hops and vaults between (0x16A928),
/// and how high the paths of the two are drawn (0xB2358, 0xB2894).
const SPRING_PLACES: [Vec3; 9] = [
    Vec3::ZERO,
    Vec3::new(0.0, 0.0, -100.0),
    Vec3::new(50.0, 0.0, -100.0),
    Vec3::new(-50.0, 0.0, -100.0),
    Vec3::new(0.0, 0.0, 100.0),
    Vec3::new(50.0, 0.0, 100.0),
    Vec3::new(-50.0, 0.0, 100.0),
    Vec3::new(50.0, 0.0, 0.0),
    Vec3::new(-50.0, 0.0, 0.0),
];
const SPRING_HEIGHT: f32 = 300.0;
const VAULT_HEIGHT: f32 = 800.0;
/// How far its hop back takes it from what it aims at, and how high (0xB2A44).
const RECOIL_REACH: f32 = 250.0;
const RECOIL_HEIGHT: f32 = 100.0;
/// The stops of the other two approaches: the Chicken's (0xB25A4) and Cone
/// Head's (0xB0B0C).
const PROWL_STOPS: [f32; 3] = [600.0, 300.0, 50.0];
const ROLL_UP_STOPS: [f32; 3] = [400.0, 300.0, 250.0];
/// How far short of the player a bound ends, and how high its path is drawn
/// (0xB0FAC).
const BOUND_STOP: f32 = 350.0;
const BOUND_HEIGHT: f32 = 800.0;
/// How high the path of a leap home is drawn (0xB3278). How long a hop on the spot
/// takes, the degrees its angle runs through in a sixtieth, and how high it goes
/// (0xAF044).
const LEAP_HOME_HEIGHT: f32 = 500.0;
const HOP_TIME: f32 = 120.0;
const HOP_RATE: f32 = 1.5;
const HOP_RISE: f32 = 200.0;

/// How far short of the player the Wheel closes in to, and how far to one side a
/// swerve's path is pulled (0xB3964, 0xB3C24).
const CLOSE_IN_STOP: f32 = 200.0;
const SWERVE: f32 = 1500.0;
/// Where beside home an arc ends, how far out its path swings (0xB3F68), and the
/// share of its frames it spends facing the way it goes (0xBAD18).
const ARC_ASIDE: f32 = 400.0;
const ARC_BEHIND: f32 = 100.0;
const ARC_SWING: f32 = 3000.0;
const ARC_DEPTH: f32 = 500.0;
const ARC_HEADLONG: f32 = 0.8;
/// How far to the side "off the stage" is (0xB4288); how far from the player the
/// points are that it waits level with (0xB43C8, 0xB450C) and charges to (0xB4650,
/// 0xB4768); how far to either side of the point a charge ends (0xB468C); and the
/// frames it turns in as it sets off (0xBAEA4).
const OFFSTAGE: f32 = 2500.0;
const LURK_STOPS: [f32; 2] = [500.0, 100.0];
const CHARGE_STOP: f32 = 500.0;
const CHARGE_AT_STOP: f32 = 300.0;
const CHARGE_ASIDE: f32 = 300.0;
const WHEEL_TURN: f32 = 6.0;

/// The places of a drift (0x16A790): the Cubes', closer in.
const DRIFT_PLACES: [Vec3; 11] = [
    Vec3::ZERO,
    Vec3::new(0.0, 50.0, 0.0),
    Vec3::new(0.0, -50.0, 0.0),
    Vec3::new(-150.0, 0.0, 0.0),
    Vec3::new(-150.0, 50.0, 0.0),
    Vec3::new(-150.0, -50.0, 0.0),
    Vec3::new(150.0, 0.0, 0.0),
    Vec3::new(150.0, 50.0, 0.0),
    Vec3::new(150.0, -50.0, 0.0),
    Vec3::new(0.0, 0.0, -150.0),
    Vec3::new(0.0, 0.0, 150.0),
];
/// The two rows of places off home that John Doe comes up at (0x16AA18),
/// and how far short of the player (0xB4AF4).
const VANISH_PLACES: [[Vec3; 3]; 2] = [
    [Vec3::new(0.0, 0.0, -200.0), Vec3::new(600.0, 0.0, -180.0), Vec3::new(-600.0, 0.0, -190.0)],
    [Vec3::new(0.0, 0.0, -500.0), Vec3::new(400.0, 0.0, -510.0), Vec3::new(-400.0, 0.0, -520.0)],
];
const VANISH_STOP: f32 = 750.0;

/// The places off home Mefe leaps between (0x16AB68): three before it and three
/// in line with it. How high the paths of her leaps are drawn (0xB63B0), the share
/// of the way across the ground at which a leap forward is highest (0xB6360), and
/// the height above which she rises only a fifth as fast (0xB6454).
const MEFE_PLACES: [Vec3; 6] = [
    Vec3::new(0.0, 0.0, -200.0),
    Vec3::new(300.0, 0.0, -180.0),
    Vec3::new(-300.0, 0.0, -190.0),
    Vec3::ZERO,
    Vec3::new(300.0, 0.0, 0.0),
    Vec3::new(-300.0, 0.0, 0.0),
];
const MEFE_LEAP_HEIGHT: f32 = 250.0;
const MEFE_LEAP_PEAK: f32 = 0.95;
const MEFE_LEAP_EASES_AT: f32 = 200.0;
const MEFE_LEAP_EASE: f32 = 5.0;
/// How far short of the player her way up to her ends and how far to one side of
/// the line to her (0xB6908, 0xB6938); how far short the three stops are that it
/// notes, and how far to the other side the last of them (0xB698C to 0xB6A18);
/// how far the path on to the second stop is pulled back (0xB6CB0); and the frames
/// she turns in as she sets off (0xBB704).
const MEFE_CLOSE_STOP: f32 = 600.0;
const MEFE_CLOSE_ASIDE: f32 = 150.0;
const MEFE_STOPS: [f32; 3] = [140.0, 125.0, 120.0];
const MEFE_ACROSS_ASIDE: f32 = -100.0;
const MEFE_CLOSER_PULL: f32 = -100.0;
const MEFE_CLOSE_TURN: f32 = 2.0;

/// The places off home Mazuma comes up at (0x16ABB0), how far short of the
/// player's ankle (0xB7474), and how far to one side or the other of where she
/// would be one of her that was called in is instead (0xB74E4). How high the path
/// of her leap home is drawn (0xB76C8).
const MAZUMA_PLACES: [Vec3; 4] = [Vec3::new(100.0, 0.0, -240.0), Vec3::new(400.0, 0.0, -415.0), Vec3::new(0.0, 0.0, -415.0), Vec3::new(-500.0, 0.0, -225.0)];
const MAZUMA_STOP: f32 = 250.0;
const MAZUMA_ASIDE: f32 = 90.0;
const MAZUMA_LEAP_HEIGHT: f32 = 200.0;
/// Where the four ways of flying in start from (0xB7864 to 0xB78F4); how far either
/// way across from where it is, and up or down from that height, a way in ends
/// (0xB78FC, 0xB7920).
const MAZUMA_FLY_FROM: [Vec3; 4] = [Vec3::new(-1500.0, 400.0, 0.0), Vec3::new(1500.0, 200.0, 0.0), Vec3::new(-1500.0, 50.0, 0.0), Vec3::new(1500.0, 300.0, 0.0)];
const MAZUMA_FLY_ACROSS: i32 = 200;
const MAZUMA_FLY_UP: i32 = 100;
/// The frames the bosses after the first turn in (the switch's own figure, 0xB9C08).
const BOSS_TURN: f32 = 1.0;
/// How far short of the player's ankle the third boss's two ways up to her end
/// (0xB7DAC, 0xB7D84), and the frames it turns in for the first (0xBB968). Where
/// its pass across the stage begins and ends, and how far before the stage's
/// middle it runs (0xB8034 to 0xB8050).
const PAIR_STOPS: [f32; 2] = [250.0, 100.0];
const PAIR_TURN: f32 = 4.0;
/// The nine places about home it goes between (0x16ABE0): three far behind home,
/// three behind it and three before it.
const PAIR_PLACES: [Vec3; 9] = [
    Vec3::new(560.0, 0.0, 715.0),
    Vec3::new(20.0, 0.0, 715.0),
    Vec3::new(-560.0, 0.0, 715.0),
    Vec3::new(495.0, 0.0, 200.0),
    Vec3::new(40.0, 0.0, 205.0),
    Vec3::new(-360.0, 0.0, 210.0),
    Vec3::new(375.0, 0.0, -300.0),
    Vec3::new(45.0, 0.0, -310.0),
    Vec3::new(-370.0, 0.0, -305.0),
];
const PAIR_PASS_FROM: f32 = 1500.0;
const PAIR_PASS_DEPTH: f32 = -1300.0;
/// The fourth boss's places off home (0x16AC4C), how far short of the player
/// (0xB831C), how far behind home "away" is (0xB8260), and the places beside home
/// it glides to (0x16AC88).
const LIRIO_PLACES: [Vec3; 5] = [
    Vec3::new(595.0, 0.0, -240.0),
    Vec3::new(360.0, 0.0, -315.0),
    Vec3::new(10.0, 0.0, -300.0),
    Vec3::new(60.0, 0.0, -315.0),
    Vec3::new(-180.0, 0.0, -225.0),
];
const LIRIO_STOP: f32 = 200.0;
const LIRIO_AWAY: f32 = 1500.0;
const LIRIO_GLIDES: [Vec3; 3] = [Vec3::new(500.0, 0.0, 10.0), Vec3::ZERO, Vec3::new(-500.0, 0.0, 10.0)];
/// Where over home the fifth boss hangs (the first of the table at 0x16ACAC, whose
/// other four the one script that could ask for them never does).
const NAFEHE_PLACE: Vec3 = Vec3::new(0.0, 100.0, 0.0);
/// The sixth boss's places (0x16ACE8), how far past the middle of the way the
/// control point of its path to one is (0xB8C40: two thirds of the way more), and
/// how far short of the player its chase and its coming up before her end
/// (0xB8F68, 0xB9200).
const ZAHA_PLACES: [Vec3; 3] = [Vec3::ZERO, Vec3::new(600.0, 0.0, 0.0), Vec3::new(-600.0, 0.0, 0.0)];
const OVERSHOOT: f32 = 0.5 + 2.0 / 3.0;
const ZAHA_STOP: f32 = 700.0;
/// The seventh boss's places off home (0x16AD0C) and beside it (0x16AD48); how far
/// short of the player's ankle, and how far under home's height (0xB93C4, 0xB93F0).
const WHITE_PLACES: [Vec3; 5] = [
    Vec3::new(395.0, 0.0, -240.0),
    Vec3::new(60.0, 0.0, -415.0),
    Vec3::new(5.0, 0.0, -300.0),
    Vec3::new(60.0, 0.0, -415.0),
    Vec3::new(-380.0, 0.0, -225.0),
];
const WHITE_SIDES: [Vec3; 3] = [Vec3::new(500.0, 0.0, 10.0), Vec3::ZERO, Vec3::new(-500.0, 0.0, 10.0)];
const WHITE_STOP: f32 = 100.0;
const WHITE_SUNK: f32 = 10.0;
/// The last boss's places off home (0x16AD6C), at one of five heights (0xB9680),
/// and beside it (0x16AD90); how far short of the player's ankle (0xB95E8), how
/// far beyond it its chase ends (0xB99E0), and how high "gone" is (0xBC03C).
const LAST_PLACES: [Vec3; 3] = [Vec3::new(160.0, 0.0, -415.0), Vec3::new(0.0, 0.0, -300.0), Vec3::new(160.0, 0.0, -415.0)];
const LAST_HEIGHTS: i32 = 5;
const LAST_HEIGHT_STEP: f32 = 10.0;
const LAST_SIDES: [Vec3; 3] = [Vec3::new(400.0, 0.0, 10.0), Vec3::ZERO, Vec3::new(-400.0, 0.0, 10.0)];
const LAST_STOP: f32 = 400.0;
const LAST_CHASE_BEYOND: f32 = 400.0;
const LAST_GONE_HEIGHT: f32 = 5000.0;

/// The node of its model that an enemy with a head of its own turns to the player
/// (0xBA698), and how far off the point is that a bearing is faced by (0xBA4E0).
const HEAD_NODE: u8 = 0x33;
const BEARING_REACH: f32 = 1000.0;

/// Gravity on an enemy dropping in, per sixtieth of a second per frame.
const GRAVITY: f32 = 0.98;
/// How far short of the player an approach stops, and its two further stops.
const APPROACH_STOPS: [f32; 3] = [400.0, 200.0, 50.0];
/// How far short of the player each of the three advances stops.
const ADVANCE_STOPS: [f32; 3] = [1000.0, 750.0, 500.0];
const RUSH_STOP: f32 = 100.0;
/// A rush weaves from side to side this many times.
const RUSH_WEAVES: f32 = 4.0;
/// Height of the control point of a hop back home.
const HOP_HEIGHT: f32 = 1000.0;
/// Logic frames in which an enemy turns to the player after going home, and to where
/// it is headed when it sets off.
const HOME_TURN: f32 = 12.0;
const SET_OFF_TURN: f32 = 2.0;

/// What a move reads of the battle around it.
pub struct Around<'a> {
    /// Length of the step in sixtieths of a second.
    pub dt: f32,
    /// Where the enemy belongs.
    pub home: Vec3,
    /// Where the player is: the middle of her body, which is what the enemies go by
    /// (0x91108).
    pub player: Vec3,
    /// Where her right ankle is, which one move goes by instead (0xB5C40).
    pub ankle: Vec3,
    /// The enemy is one that was called into the battle (its + 0x23A).
    pub called: bool,
    /// The point the enemy's moves aim at: the player's place as of when a move last
    /// looked (0x91108), unless its script has set another.
    pub aim: &'a mut Vec3,
    pub random: &'a mut Random,
}

#[derive(Clone, Debug)]
pub struct Mover {
    /// How far the current move's own code has got: 0 before its first frame.
    phase: i32,
    /// Frames the current move has run.
    updates: i32,
    /// Time left of the turn, in sixtieths.
    turn_left: f32,
    /// A timer scripts set, read and run down themselves.
    pub action_timer: f32,
    /// A clock that runs by itself, in sixtieths: scripts set it as an action begins
    /// and wait for it to reach the moments of the action (block + 0x160).
    pub clock: f32,
    /// Bits of [`flag`].
    pub flags: u32,
    /// The slot being moved, a second one placed along with it, and the kind of move;
    /// -1 for none.
    pub slot: i32,
    pub second_slot: i32,
    kind: i32,
    /// Which of the places near home was picked last.
    near_home: i32,
    /// Time run and length of the move, in sixtieths.
    elapsed: f32,
    length: f32,
    /// How long a walk has been going, for the rhythm of its strides (block + 0x2C).
    stride_clock: f32,
    start: Vec3,
    target: Vec3,
    /// The two further stops of an approach, and a third that one boss's notes
    /// (block + 0x70, 0x80 and 0x90).
    stops: [Vec3; 2],
    third_stop: Vec3,
    /// The control point of the path, or the velocity of a fall.
    control: Vec3,
    /// Where a rush would be without its weave, and the weave's angle and its step.
    straight: Vec3,
    weave: f32,
    weave_step: f32,
    /// Turn per frame, the angle to end on and the angle now, about the vertical.
    turn_step: f32,
    yaw_target: f32,
    pub yaw: f32,
    /// The bearing its head was at when it last finished turning, and the one it is
    /// turning to (block + 0x3C and 0x40).
    head: f32,
    head_target: f32,
    /// A node of the model in the first slot and the turn it has been given about
    /// its own vertical, on top of its animation.
    pub node_turn: Option<(u8, f32)>,
    /// The kind of a move that has just ended for want of any code for it here. If
    /// [`original_has`] it, the port falls short.
    pub unknown: Option<i32>,
    pub position: Vec3,
    /// The position as of the last frame a move ran.
    last: Vec3,
}

impl Default for Mover {
    /// The block as the actor's constructor leaves it (0xBC364).
    fn default() -> Self {
        Mover {
            phase: 0,
            updates: 0,
            turn_left: 0.0,
            action_timer: 0.0,
            clock: 0.0,
            flags: 0,
            slot: -1,
            second_slot: -1,
            kind: 0,
            near_home: 0,
            elapsed: 0.0,
            length: 0.0,
            stride_clock: 0.0,
            start: Vec3::ZERO,
            target: Vec3::ZERO,
            stops: [Vec3::ZERO; 2],
            third_stop: Vec3::ZERO,
            control: Vec3::ZERO,
            straight: Vec3::ZERO,
            weave: 0.0,
            weave_step: 0.0,
            turn_step: 0.0,
            yaw_target: 0.0,
            yaw: 0.0,
            head: 0.0,
            head_target: 0.0,
            node_turn: None,
            unknown: None,
            position: Vec3::ZERO,
            last: Vec3::ZERO,
        }
    }
}

/// How far to one side of her place one of Mazuma that was called in is: so far to
/// either side, by chance (0xB74D8); the one that was there from the start is at
/// the place itself.
fn mazuma_aside(around: &mut Around) -> f32 {
    match (around.called, around.random.below(2)) {
        (false, _) => 0.0,
        (true, 0) => MAZUMA_ASIDE,
        (true, _) => -MAZUMA_ASIDE,
    }
}

/// The angle about the vertical that faces from `from` towards `to`.
fn heading(from: Vec3, to: Vec3) -> f32 {
    (to.x - from.x).atan2(to.z - from.z)
}

/// An angle brought into one turn from zero.
fn wrapped(mut angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    while !(0.0..TAU).contains(&angle) {
        if angle >= TAU {
            angle -= TAU;
        }
        if angle < 0.0 {
            angle += TAU;
        }
    }
    angle
}

/// Whether a walker `clock` sixtieths into its walk has both feet on the ground, and
/// so does not move (0xB4CF8). The stretches are counted in halves of the step.
fn planted(clock: f32, dt: f32) -> bool {
    (0..STRIDES).any(|stride| {
        let from = stride as f32 * STRIDE;
        PLANTED.iter().any(|(first, last)| clock >= (from + first) * dt * 0.5 && clock < (from + last) * dt * 0.5)
    })
}

/// The unit vector from `to` to `from` on the ground, and how far apart they are on it.
fn away_on_ground(from: Vec3, to: Vec3) -> (Vec3, f32) {
    let apart = Vec3::new(from.x - to.x, 0.0, from.z - to.z);
    (apart.normalize_or_zero(), apart.length())
}

impl Mover {
    /// Sets the enemy at `home`, facing `aim` (engine function 12, 0xBF2EC).
    pub fn init(&mut self, home: Vec3, aim: Vec3) {
        self.updates = 0;
        self.turn_left = 0.0;
        self.yaw = heading(home, aim);
        self.yaw_target = self.yaw;
        self.slot = -1;
        self.second_slot = -1;
        self.kind = -1;
        self.near_home = -1;
        self.flags = (self.flags & !(flag::MOVING | flag::SCRIPT_10 | flag::SCRIPT_20)) | flag::ENDED;
        self.position = home;
    }

    /// Starts a move of `slot` (engine function 30, 0xBCE40). `frames` is the move's
    /// length: in sixtieths of a second for most kinds, in logic frames for a turn.
    pub fn start(&mut self, slot: i32, kind: i32, frames: i32) {
        self.flags = (self.flags | flag::MOVING) & !(flag::HOLD | flag::ENDED);
        self.updates = 0;
        // Two kinds only name a second slot to carry along.
        if kind == 10201 || kind == 10401 {
            self.second_slot = slot;
            return;
        }
        // The same slot making the same kind of move again carries on from where the
        // last one started.
        if self.slot != slot || self.kind != kind {
            self.slot = slot;
            self.kind = kind;
            self.start = self.position;
        }
        self.phase = 0;
        self.elapsed = 0.0;
        self.length = frames as f32;
    }

    /// Ends whatever move is running (engine function 25 with its flag set).
    pub fn stop(&mut self) {
        self.kind = -1;
        self.near_home = -1;
        self.flags = (self.flags & !flag::MOVING) | flag::ENDED;
    }

    pub fn ended(&self) -> bool {
        self.flags & flag::ENDED != 0
    }

    /// Puts the block in the frame of a scene (0xBD040): at the origin, facing half
    /// way round and turning no further. Answers where it was and how it was turned
    /// and turning, for [`Mover::leave_scene`].
    pub fn enter_scene(&mut self) -> (Vec3, f32, f32) {
        let before = (self.position, self.yaw, self.yaw_target);
        (self.position, self.yaw, self.yaw_target, self.turn_left) = (Vec3::ZERO, PI, PI, 0.0);
        before
    }

    /// Puts it back where a scene found it (0xBD2A8).
    pub fn leave_scene(&mut self, position: Vec3, yaw: f32, yaw_target: f32) {
        (self.position, self.yaw, self.yaw_target) = (position, yaw, yaw_target);
    }

    /// Has it turned a way, and turning no further (0xBEC28).
    pub fn turned(&mut self, angle: f32) {
        (self.yaw, self.yaw_target, self.turn_left) = (angle, angle, 0.0);
    }

    /// Sets it down plainly at a place, turned no way (0xBF168).
    pub fn set_down(&mut self, at: Vec3) {
        self.turned(0.0);
        self.position = at;
    }

    /// The kind of the move last started; -1 for none.
    pub fn kind(&self) -> i32 {
        self.kind
    }

    /// Where the moved slot stands and how it is turned.
    pub fn placement(&self) -> Transform {
        Transform { translation: self.position, rotation: Quat::from_rotation_y(self.yaw), scale: Vec3::ONE }
    }

    /// Has the enemy turn to face from `from` towards `to` over `frames` logic frames,
    /// the short way round (0xAE910).
    pub fn face(&mut self, from: Vec3, to: Vec3, frames: f32, dt: f32) {
        let current = wrapped(self.yaw);
        let mut target = wrapped(heading(from, to));
        let difference = target - current;
        if difference.abs() > PI {
            target += if difference <= 0.0 { TAU } else { -TAU };
        }
        let frames = if frames <= 0.0 { 1.0 } else { frames };
        self.yaw = current;
        self.yaw_target = target;
        self.turn_step = (target - current) / frames;
        self.turn_left = if dt <= 1.0 { frames } else { frames * dt };
    }

    /// One frame (0xB9B50). Returns true if the slots being moved are to be placed
    /// anew, at [`Mover::placement`].
    pub fn update(&mut self, around: &mut Around) -> bool {
        self.clock += around.dt;
        if self.flags & flag::MOVING == 0 || self.slot == -1 {
            return false;
        }
        let mut position = self.position;
        let mut turns = true;
        if self.flags & (flag::ENDED | flag::HOLD) == 0 {
            turns = self.run(&mut position, around);
            self.last = position;
            self.updates += 1;
        }

        self.turn_left -= around.dt;
        if turns {
            if self.turn_left <= 0.0 {
                self.turn_left = 0.0;
                self.yaw = self.yaw_target;
            } else {
                self.yaw += self.turn_step;
            }
        }
        self.position = position;
        true
    }

    /// The current move's own code for one frame. Returns false for the kinds that
    /// leave the enemy's angle alone.
    fn run(&mut self, position: &mut Vec3, around: &mut Around) -> bool {
        let dt = around.dt;
        let first = self.updates == 0;
        match self.kind {
            kind::FACE_PLAYER => {
                if first {
                    *around.aim = around.player;
                    self.face(self.last, around.player, self.length, dt);
                    self.flags |= flag::ENDED;
                }
            }
            kind::DROP_IN | kind::RISE => {
                let rising = self.kind == kind::RISE;
                self.fall(position, rising, around);
                if first {
                    *around.aim = around.player;
                    self.face(self.last, around.player, self.length, dt);
                }
            }
            kind::FACE_PLAYER_FROM_HERE => {
                if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, self.length, dt);
                }
                self.flags |= flag::ENDED;
            }
            kind::KEEP_FACING => {
                self.elapsed += dt;
                *around.aim = around.player;
                self.face(self.last, around.player, self.length / 10.0, dt);
                if self.elapsed >= self.length {
                    self.flags |= flag::ENDED;
                }
            }
            kind::WAIT => {
                self.elapsed += dt;
                if self.elapsed >= self.length {
                    self.flags |= flag::ENDED;
                }
                return false;
            }
            kind::FACE_BEARING => {
                // The length is a bearing in degrees: none is towards the player's
                // side of the stage, and more is further round to the left as she
                // sees it (0xBA4A8).
                let bearing = self.length.to_radians();
                let to = *position + Vec3::new(bearing.sin(), 0.0, -bearing.cos()) * BEARING_REACH;
                self.face(*position, to, 1.0, dt);
                self.flags |= flag::ENDED;
            }
            kind::TURN_HEAD => {
                // The head goes round to the player's bearing over the move's length,
                // the short way; the body does not move (0xBA568).
                self.elapsed += dt;
                *around.aim = around.player;
                let bearing = heading(*position, around.player);
                self.head_target = bearing;
                let turn = bearing - self.head;
                if turn.abs() > PI {
                    self.head_target += if turn <= 0.0 { TAU } else { -TAU };
                }
                if self.elapsed >= self.length {
                    self.head = bearing;
                    self.flags |= flag::ENDED;
                }
                // A move of no length would divide nothing by nothing here.
                let turned = (self.head_target - self.head) * self.elapsed / self.length;
                let now = self.head + if turned.is_finite() { turned } else { 0.0 };
                self.node_turn = Some((HEAD_NODE, now - self.yaw_target));
            }
            kind::TURN_TO_PLAYER => {
                // Over at once, the turn going on after it (0xBAC3C).
                self.flags |= flag::ENDED;
                if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                }
            }
            kind::GROUND => {
                // Down onto the ground where it is (0xBA920).
                position.y = 0.0;
                self.flags |= flag::ENDED;
            }
            kind::HOME | kind::NEAR_HOME | kind::ROLL_HOME => {
                // Two families have this, with tables that say the same (0xAF788,
                // 0xB07C8).
                if self.phase == 0 {
                    let place = if self.kind == kind::NEAR_HOME { self.pick_place(NEAR_HOME.len(), around.random) } else { 0 };
                    self.near_home = place;
                    self.set_off(*position, around.home + NEAR_HOME[place as usize]);
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                    *position = self.target;
                }
                if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                }
            }
            kind::APPROACH | kind::PROWL | kind::ROLL_UP => {
                // Three families go up to her like this, each to distances of its
                // own, noting two stops further on (0xAF990, 0xB2468, 0xB09D0).
                if self.phase == 0 {
                    let stops = match self.kind {
                        kind::PROWL => PROWL_STOPS,
                        kind::ROLL_UP => ROLL_UP_STOPS,
                        _ => APPROACH_STOPS,
                    };
                    let (away, apart) = away_on_ground(*position, around.player);
                    let stop = |short: f32| *position - away * (apart - short);
                    self.stops = [stop(stops[1]), stop(stops[2])];
                    self.set_off(*position, stop(stops[0]));
                }
                self.travel(position, dt);
                if first {
                    self.face(self.start, self.target, SET_OFF_TURN, dt);
                }
            }
            kind::APPROACH_CLOSER
            | kind::APPROACH_CLOSEST
            | kind::PROWL_CLOSER
            | kind::ROLL_CLOSER
            | kind::ROLL_CLOSEST
            | kind::CLOSE_IN_FURTHER => {
                // On to one of the stops the last approach noted, whichever family's
                // it was (0xAFCE8, 0xB0D28, 0xB3D28 and the second halves of the
                // approaches).
                if self.phase == 0 {
                    let further = matches!(self.kind, kind::APPROACH_CLOSEST | kind::ROLL_CLOSEST);
                    self.set_off(*position, self.stops[further as usize]);
                }
                self.travel(position, dt);
                return false;
            }
            kind::SPRING | kind::VAULT => {
                // A hop to another of nine places round home, which ends on the
                // ground and turns to the player (0xB2264); or a high vault to one,
                // turning no more than it already was (0xB27C0).
                let spring = self.kind == kind::SPRING;
                if self.phase == 0 {
                    let place = self.pick_place(SPRING_PLACES.len(), around.random);
                    self.near_home = place;
                    self.set_off(*position, around.home + SPRING_PLACES[place as usize]);
                    self.control.y = if spring { SPRING_HEIGHT } else { VAULT_HEIGHT };
                }
                if self.travel(position, dt) {
                    *position = self.target;
                    if spring {
                        self.elapsed = 0.0;
                        position.y = 0.0;
                    }
                }
                if spring && first {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                }
            }
            kind::RECOIL => {
                // A low hop back from what it aims at, onto the ground (0xB29A4).
                if self.phase == 0 {
                    let (away, _) = away_on_ground(*position, *around.aim);
                    self.set_off(*position, *position + away * RECOIL_REACH);
                    self.control.y = RECOIL_HEIGHT;
                }
                if self.travel(position, dt) {
                    position.y = 0.0;
                }
            }
            kind::BOUND => {
                // A high bound to a set distance from her (0xB0EC0).
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.player);
                    self.set_off(*position, *position - away * (apart - BOUND_STOP));
                    self.control.y = BOUND_HEIGHT;
                }
                self.travel(position, dt);
                if first {
                    self.face(self.start, self.target, SET_OFF_TURN, dt);
                }
            }
            kind::LEAP_HOME => {
                // A leap home backwards: it faces the way it came (0xB31C4).
                if self.phase == 0 {
                    self.set_off(*position, around.home);
                    self.control.y = LEAP_HOME_HEIGHT;
                }
                self.travel(position, dt);
                if first {
                    self.face(self.target, self.start, SET_OFF_TURN, dt);
                }
            }
            kind::HOP_ON_SPOT => {
                // Up and down where it stands, in a time of its own whatever length
                // the script gave (0xAEFE0). The turn it is given is from where it
                // is to where it is, which is no bearing at all: it comes out as
                // the far side of the stage.
                if self.phase == 0 {
                    self.start = *position;
                    self.target = *position;
                    self.length = HOP_TIME;
                    self.stride_clock = 0.0;
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.elapsed += dt;
                    position.y = (self.elapsed * HOP_RATE).to_radians().sin() * HOP_RISE;
                    if self.elapsed >= self.length {
                        position.y = 0.0;
                        self.phase = 2;
                        self.flags |= flag::ENDED;
                    }
                }
                if first {
                    self.face(self.target, self.start, HOME_TURN, dt);
                }
            }
            kind::HOP_HOME | kind::ROLL_HOP_HOME => {
                if self.phase == 0 {
                    let place = self.pick_place(NEAR_HOME.len(), around.random);
                    self.near_home = place;
                    self.set_off(*position, around.home + NEAR_HOME[place as usize]);
                    self.control.y = HOP_HEIGHT;
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
            }
            kind::BACK_AWAY | kind::BACK_AWAY_FAR => {
                if self.phase == 0 {
                    let far = if self.kind == kind::BACK_AWAY { 200.0 } else { 500.0 };
                    let (away, _) = away_on_ground(*position, *around.aim);
                    self.set_off(*position, *position + away * far);
                }
                self.travel(position, dt);
            }
            kind::ADVANCE_FIRST..=kind::ADVANCE_LAST | kind::ROLL_FIRST..=kind::ROLL_LAST | kind::CLOSE_IN => {
                // Two families have the same three advances (0xB0260, 0xB12F0), and
                // the Wheel has one that ends closer (0xB3860).
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.player);
                    let short = match self.kind {
                        kind::CLOSE_IN => CLOSE_IN_STOP,
                        kind::ROLL_FIRST.. => ADVANCE_STOPS[(self.kind - kind::ROLL_FIRST) as usize],
                        _ => ADVANCE_STOPS[(self.kind - kind::ADVANCE_FIRST) as usize],
                    };
                    self.set_off(*position, *position - away * (apart - short));
                }
                self.travel(position, dt);
                if first {
                    self.face(self.start, self.target, SET_OFF_TURN, dt);
                }
            }
            kind::SWERVE_IN => {
                // As close, along a path pulled far out to one side, facing the way
                // it goes (0xB3AB8).
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.player);
                    self.set_off(*position, *position - away * (apart - CLOSE_IN_STOP));
                    self.control.x += SWERVE;
                }
                self.travel(position, dt);
                self.face(self.last, *position, SET_OFF_TURN, dt);
            }
            kind::ARC_FIRST..=kind::ARC_LAST | kind::STRAIGHT_HOME_FIRST..=kind::STRAIGHT_HOME_LAST => {
                // Home: straight, or round a wide arc to a place on one side of it
                // (0xB3EC0). On an arc it faces the way it goes for most of the
                // move and then turns to what it aims at; going straight it turns
                // to that throughout.
                let arc = self.kind <= kind::ARC_LAST;
                if self.phase == 0 {
                    if arc {
                        let side = if self.kind == kind::ARC_FIRST { 1.0 } else { -1.0 };
                        self.set_off(*position, around.home + Vec3::new(side * ARC_ASIDE, 0.0, ARC_BEHIND));
                        self.control += Vec3::new(side * ARC_SWING, 0.0, ARC_DEPTH);
                    } else {
                        self.set_off(*position, around.home);
                    }
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                if arc && self.updates <= (self.length / dt * ARC_HEADLONG) as i32 {
                    self.face(self.last, *position, SET_OFF_TURN, dt);
                } else {
                    self.face(*position, *around.aim, HOME_TURN, dt);
                }
            }
            kind::OFFSTAGE_FIRST..=kind::RETURN_LAST => {
                // The Wheel's way round the back of the stage (0xB41CC). Each of
                // these comes in pairs, one for either side of the stage: the
                // first of a pair is the side x grows towards.
                if self.phase == 0 {
                    let (kind, home) = (self.kind, around.home);
                    let side = |pair: i32| if kind == pair { OFFSTAGE } else { -OFFSTAGE };
                    // The point so far from the player on the line from her to
                    // its home.
                    let (away, apart) = away_on_ground(home, around.player);
                    let towards_home = |far: f32| home - away * (apart - far);
                    self.start = *position;
                    match kind {
                        kind::OFFSTAGE_FIRST..=kind::OFFSTAGE_LAST => {
                            // Out to the side, slowly at first.
                            self.target = Vec3::new(side(kind::OFFSTAGE_FIRST), self.start.y, self.start.z);
                            self.control = self.start + (self.target - self.start) * 0.25;
                        }
                        kind::LURK_FIRST..=kind::LURK_LAST | kind::LURK_NEAR_FIRST..=kind::LURK_NEAR_LAST => {
                            // Out of sight it is simply put where it will come
                            // in from, level with a point near her, and stays
                            // there for the move.
                            let near = kind >= kind::LURK_NEAR_FIRST;
                            let level = towards_home(LURK_STOPS[near as usize]);
                            let pair = if near { kind::LURK_NEAR_FIRST } else { kind::LURK_FIRST };
                            self.start = Vec3::new(side(pair), level.y, level.z);
                            self.target = self.start;
                            self.control = self.start;
                        }
                        kind::CHARGE_AT => {
                            self.target = towards_home(CHARGE_AT_STOP);
                            self.control = self.start + (self.target - self.start) * 2.0 / 3.0;
                        }
                        kind::CHARGE_FIRST..=kind::CHARGE_LAST | kind::SIDLE_FIRST..=kind::SIDLE_LAST => {
                            let aside = if matches!(kind, kind::CHARGE_FIRST | kind::SIDLE_FIRST) { CHARGE_ASIDE } else { -CHARGE_ASIDE };
                            self.target = towards_home(CHARGE_STOP) + Vec3::X * aside;
                            self.control = (self.start + self.target) * 0.5;
                        }
                        _ => {
                            // In from the side to home, slowing into it.
                            self.start = Vec3::new(side(kind::RETURN_FIRST), home.y, home.z);
                            self.target = home;
                            self.control = self.start + (self.target - self.start) * 2.0 / 3.0;
                        }
                    }
                    self.phase = 1;
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                if matches!(self.kind, kind::SIDLE_FIRST..=kind::SIDLE_LAST) {
                    // These keep facing the player's side of the stage.
                    self.yaw = PI;
                    self.yaw_target = PI;
                    return false;
                }
                if first {
                    self.face(self.start, self.target, WHEEL_TURN, dt);
                }
            }
            kind::VANISH_NEAR_FIRST..=kind::VANISH_NEAR_LAST
            | kind::VANISH_FAR_FIRST..=kind::VANISH_FAR_LAST
            | kind::VANISH_TO_PLAYER
            | kind::VANISH_HOME => {
                // It stays where it is for the length of the move and is then at
                // its place all at once, facing the player: one of six places off
                // home, in front of her, or home (0xB49F0). The kind's last two
                // digits are the row and the place in it.
                if self.phase == 0 {
                    let (row, place) = ((self.kind - kind::VANISH_NEAR_FIRST) / 10, (self.kind - kind::VANISH_NEAR_FIRST) % 10);
                    self.start = *position;
                    self.target = match self.kind {
                        kind::VANISH_TO_PLAYER => {
                            self.near_home = 0;
                            let (away, apart) = away_on_ground(*position, around.ankle);
                            *position - away * (apart - VANISH_STOP)
                        }
                        kind::VANISH_HOME => {
                            self.near_home = 1;
                            around.home
                        }
                        _ => {
                            self.near_home = place;
                            around.home + VANISH_PLACES[row as usize][place as usize]
                        }
                    };
                    self.phase = 1;
                }
                self.elapsed += dt;
                if self.elapsed >= self.length {
                    *position = self.target;
                    self.flags |= flag::ENDED;
                    *around.aim = around.player;
                    self.face(self.target, around.player, 0.0, dt);
                }
            }
            kind::TREAD_FIRST..=kind::TREAD_LAST => {
                // Three more, to other distances, that end on the ground (0xB3368).
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.player);
                    let short = TREAD_STOPS[(self.kind - kind::TREAD_FIRST) as usize];
                    self.set_off(*position, *position - away * (apart - short));
                }
                if self.travel(position, dt) {
                    position.y = 0.0;
                }
                if first {
                    self.face(self.start, self.target, SET_OFF_TURN, dt);
                }
            }
            kind::GLIDE_FIRST..=kind::GLIDE_LAST => {
                // The same distances for one that flies: the line it goes along
                // slopes with its own height, and it keeps facing the player's side
                // of the stage (0xB5310).
                if self.phase == 0 {
                    let (_, apart) = away_on_ground(*position, around.player);
                    let slope = Vec3::new(position.x - around.player.x, position.y, position.z - around.player.z);
                    let short = TREAD_STOPS[(self.kind - kind::GLIDE_FIRST) as usize];
                    self.set_off(*position, *position - slope.normalize_or_zero() * (apart - short));
                }
                self.travel(position, dt);
                self.yaw = PI;
                self.yaw_target = PI;
                return false;
            }
            kind::LEAP_FIRST..=kind::LEAP_LAST => {
                // A leap to one of two distances from her; the third kind goes on to
                // wherever the leap before it aimed (0xB2BAC).
                if self.phase == 0 {
                    self.start = *position;
                    if let Some(short) = LEAP_STOPS.get((self.kind - kind::LEAP_FIRST) as usize) {
                        let (away, apart) = away_on_ground(self.start, around.player);
                        self.target = self.start - away * (apart - short);
                    }
                    self.control = (self.start + self.target) * 0.5;
                    self.control.y = LEAP_HEIGHT;
                    self.phase = 1;
                }
                self.travel(position, dt);
                if first {
                    self.face(self.start, self.target, SET_OFF_TURN, dt);
                }
            }
            kind::ARRIVE_FIRST..=kind::ARRIVE_LAST => {
                // In from far off to home, along a wide curve, facing home (0xB1944).
                if self.phase == 0 {
                    self.start = around.home + ARRIVE_FROM[(self.kind - kind::ARRIVE_FIRST) as usize];
                    self.target = around.home;
                    self.control = (self.start + self.target) * 0.5 * ARRIVE_CURVE;
                    self.phase = 1;
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                }
                *around.aim = around.player;
                self.face(self.last, self.target, 1.0, dt);
            }
            kind::WEAVE_FIRST..=kind::WEAVE_LAST => {
                // To its place in a formation along a path that swings out to one
                // side and towards the player, facing the way it goes (0xB1B3C).
                let before = *position;
                if self.phase == 0 {
                    let index = (self.kind - kind::WEAVE_FIRST) as usize;
                    let (formation, which) = (index / 4, index % 4);
                    // Worked out from home, as the original has it.
                    let (start, target) = (*position - around.home, WEAVE_PLACES[formation][which]);
                    let mut control = (start + target) * 0.5;
                    control.x += if which / 2 != 0 { -WEAVE_SIDE } else { WEAVE_SIDE };
                    control.y += if around.random.below(2) != 0 { WEAVE_DOWN } else { WEAVE_UP };
                    control.z = WEAVE_DEPTH;
                    self.start = start + around.home;
                    self.target = target + around.home;
                    self.control = control + around.home;
                    self.phase = 1;
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                }
                *around.aim = around.player;
                self.face(before, *position, 1.0, dt);
            }
            kind::CROSS_FIRST..=kind::CROSS_LAST | kind::CROSS_HOP_FIRST..=kind::CROSS_HOP_LAST => {
                // To one of five places across the stage, striding or with a hop,
                // turning to the player all the while (0xB5984).
                if self.phase == 0 {
                    let hop = self.kind >= kind::CROSS_HOP_FIRST;
                    let place = self.kind - if hop { kind::CROSS_HOP_FIRST } else { kind::CROSS_FIRST };
                    self.near_home = place;
                    self.hop_off(*position, around.home + CROSS_PLACES[place as usize], hop);
                }
                self.land(position, dt);
                *around.aim = around.player;
                self.face(*position, around.player, SET_OFF_TURN, dt);
            }
            kind::LUNGE_NEAR_FIRST..=kind::LUNGE_NEAR_LAST
            | kind::LUNGE_FAR_FIRST..=kind::LUNGE_FAR_LAST
            | kind::HOP_NEAR_FIRST..=kind::HOP_NEAR_LAST
            | kind::HOP_FAR_FIRST..=kind::HOP_FAR_LAST
            | kind::POUNCE
            | kind::POUNCE_HOME => {
                // To one of six places off home, to in front of the player, or back
                // home; all but the first two rows with a low hop (0xB5B9C). The
                // kind's last two digits are the row and the place in it.
                let homeward = self.kind == kind::POUNCE_HOME;
                if self.phase == 0 {
                    let (row, place) = ((self.kind - kind::LUNGE_NEAR_FIRST) / 10, (self.kind - kind::LUNGE_NEAR_FIRST) % 10);
                    let target = match self.kind {
                        kind::POUNCE => {
                            self.near_home = 0;
                            let (away, apart) = away_on_ground(*position, around.ankle);
                            *position - away * (apart - POUNCE_STOP)
                        }
                        kind::POUNCE_HOME => {
                            self.near_home = 1;
                            around.home
                        }
                        _ => {
                            self.near_home = place;
                            around.home + LUNGE_PLACES[(row % 2) as usize][place as usize]
                        }
                    };
                    self.hop_off(*position, target, self.kind >= kind::HOP_NEAR_FIRST);
                }
                self.land(position, dt);
                if homeward {
                    // It hops home backwards: facing the way it came.
                    if first {
                        *around.aim = around.player;
                        self.face(self.target, self.start, HOME_TURN, dt);
                    }
                } else if self.flags & flag::ENDED != 0 {
                    *around.aim = around.player;
                    self.face(self.target, around.player, SET_OFF_TURN, dt);
                }
            }
            kind::TRUDGE_NEAR_FIRST..=kind::TRUDGE_NEAR_LAST | kind::TRUDGE_FAR_FIRST..=kind::TRUDGE_FAR_LAST => {
                // The Spider's walk to one of six places off home, turning to the
                // player as it goes (0xB2E64).
                if self.phase == 0 {
                    let (row, place) = ((self.kind - kind::TRUDGE_NEAR_FIRST) / 10, (self.kind - kind::TRUDGE_NEAR_FIRST) % 10);
                    self.near_home = place;
                    self.set_off(*position, around.home + TRUDGE_PLACES[row as usize][place as usize]);
                }
                self.land(position, dt);
                *around.aim = around.player;
                self.face(*position, around.player, SET_OFF_TURN, dt);
            }
            kind::TRUDGE_AT_PLAYER | kind::TRUDGE_HOME => {
                // Up to the player, or back home: facing where it goes, and her once
                // it is there.
                let homeward = self.kind == kind::TRUDGE_HOME;
                if self.phase == 0 {
                    self.near_home = homeward as i32;
                    let target = if homeward {
                        around.home
                    } else {
                        let (away, apart) = away_on_ground(*position, around.ankle);
                        *position - away * (apart - TRUDGE_STOP)
                    };
                    self.set_off(*position, target);
                }
                self.land(position, dt);
                if self.flags & flag::ENDED != 0 {
                    *around.aim = around.player;
                    self.face(self.target, around.player, if homeward { HOME_TURN } else { SET_OFF_TURN }, dt);
                } else {
                    self.face(*position, self.target, SET_OFF_TURN, dt);
                }
            }
            kind::RUSH => {
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.player);
                    self.start = *position;
                    self.straight = *position;
                    self.target = *position - away * (apart - RUSH_STOP);
                    let frames = self.length / dt;
                    self.control = Vec3::new(self.target.x - self.start.x, 0.0, self.target.z - self.start.z) / frames;
                    self.weave = 0.0;
                    self.weave_step = RUSH_WEAVES * TAU / frames;
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.elapsed += dt;
                    self.straight += self.control;
                    self.weave += self.weave_step;
                    *position = self.straight;
                    position.x += self.weave.sin() * (around.random.below(20) as f32 + 20.0);
                    if self.elapsed >= self.length {
                        self.phase = 2;
                        self.flags |= flag::ENDED;
                    }
                }
                self.face(self.start, self.target, SET_OFF_TURN, dt);
            }
            kind::HOVER | kind::DART => {
                if self.phase == 0 {
                    // To another of its places (0xAF4F4). A drift keeps an even pace;
                    // a dart sets off fast and slows into its place.
                    let dart = self.kind == kind::DART;
                    let places = if dart { &DART_PLACES } else { &HOVER_PLACES };
                    let place = self.pick_place(places.len(), around.random);
                    self.near_home = place;
                    self.start = *position;
                    self.target = around.home + places[place as usize];
                    self.control = match dart {
                        true => self.start + (self.target - self.start) * 2.0 / 3.0,
                        false => (self.start + self.target) * 0.5,
                    };
                    self.phase = 1;
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                }
                if first {
                    *around.aim = around.player;
                    self.face(self.last, around.player, HOME_TURN, dt);
                }
            }
            kind::FALL => {
                // Straight down, ever faster, until it lies on the ground (0xAED90).
                let pull = dt * GRAVITY;
                if self.phase == 0 {
                    self.start = *position;
                    self.target = Vec3::new(position.x, 0.0, position.z);
                    self.control = Vec3::new(0.0, -pull, 0.0);
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.start += self.control;
                    *position = self.start;
                    if self.start.y <= 0.0 {
                        position.y = 0.0;
                        self.start.y = 0.0;
                        self.flags |= flag::ENDED;
                    }
                    self.control.y -= pull;
                }
            }
            kind::RISE_HOME => {
                // Straight up again, to the height of its home (0xAEEB8).
                let pull = dt * GRAVITY;
                if self.phase == 0 {
                    self.start = *position;
                    self.target = around.home;
                    self.control = Vec3::new(0.0, pull, 0.0);
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.start += self.control;
                    *position = self.start;
                    if self.start.y >= self.target.y {
                        position.y = self.target.y;
                        self.start.y = self.target.y;
                        self.flags |= flag::ENDED;
                    }
                    self.control.y += pull;
                }
            }
            kind::STEP => {
                // To another of five places round home, along the ground (0xB366C).
                if self.phase == 0 {
                    let place = self.pick_place(STEP_PLACES.len(), around.random);
                    self.near_home = place;
                    self.set_off(*position, around.home + STEP_PLACES[place as usize]);
                    self.control.y = 0.0;
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
            }
            kind::CHASE => {
                // Each frame it takes the share of the way that the frame is of the
                // time left, to a point in front of where she is now (0xB1DE8).
                if self.phase == 0 {
                    self.stride_clock = 0.0;
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.start = *position;
                    let (away, apart) = away_on_ground(self.start, around.player);
                    self.target = self.start - away * (apart - CHASE_STOP);
                    self.target.y = CHASE_HEIGHT;
                    self.control = (self.start + self.target) * 0.5;
                    *position = self.follow(dt);
                    if self.length <= 0.0 {
                        self.phase = 2;
                        self.flags |= flag::ENDED;
                    }
                }
                *around.aim = around.player;
                self.face(*position, around.player, 0.0, dt);
            }
            kind::SWOOP_FIRST..=kind::SWOOP_LAST => {
                // A dart to one of three places off home (0xB2088).
                if self.phase == 0 {
                    self.start = *position;
                    self.target = around.home + SWOOP_PLACES[(self.kind - kind::SWOOP_FIRST) as usize];
                    self.control = self.start + (self.target - self.start) * 2.0 / 3.0;
                    self.phase = 1;
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                }
                if first {
                    *around.aim = around.player;
                    self.face(*position, self.target, HOME_TURN, dt);
                }
            }
            kind::STOMP | kind::STOMP_FAR => {
                // Towards her stride by stride, the way a chase goes, the time
                // counting only while it moves (0xB4C1C).
                if self.phase == 0 {
                    self.start = *position;
                    self.stride_clock = 0.0;
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.stride_clock += dt;
                    if !planted(self.stride_clock, dt) {
                        let stop = if self.kind == kind::STOMP_FAR { STOMP_FAR_STOP } else { STOMP_STOP };
                        self.start = *position;
                        let (away, apart) = away_on_ground(self.start, around.player);
                        self.target = self.start - away * (apart - stop);
                        self.control = (self.start + self.target) * 0.5;
                        *position = self.follow(dt);
                    }
                    if self.elapsed >= self.length {
                        self.phase = 2;
                        self.flags |= flag::ENDED;
                    }
                }
                if self.flags & flag::ENDED != 0 {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                } else {
                    self.face(self.start, self.target, HOME_TURN, dt);
                }
            }
            kind::STOMP_BACK => {
                // Out to a set distance from where she was as it set off, with the
                // same strides (0xB4FE0).
                if self.phase == 0 {
                    self.start = *position;
                    let (away, apart) = away_on_ground(self.start, around.player);
                    self.target = self.start - away * (apart - STOMP_BACK_STOP);
                    self.control = (self.start + self.target) * 0.5;
                    self.control.y = 0.0;
                    self.stride_clock = 0.0;
                    self.phase = 1;
                }
                if self.phase == 1 {
                    self.stride_clock += dt;
                    if !planted(self.stride_clock, dt) {
                        self.elapsed += dt;
                        *position = self.on_path(if self.length > 0.0 { self.elapsed / self.length } else { 1.0 });
                    }
                    if self.elapsed >= self.length {
                        self.phase = 2;
                        self.flags |= flag::ENDED;
                    }
                }
                self.face(*position, *around.aim, HOME_TURN, dt);
            }
            kind::LIFT | kind::SINK => {
                // Straight up or down, a little past its mark and back to it, facing
                // the player's side of the stage throughout (0xB560C, 0xB57C8).
                if self.phase == 0 {
                    let rise = if self.kind == kind::LIFT { LIFT_HEIGHT } else { -LIFT_HEIGHT };
                    self.start = *position;
                    self.target = self.start + Vec3::Y * rise;
                    self.control = self.start + Vec3::Y * (rise * 3.0 * 0.5);
                    self.phase = 1;
                }
                self.travel(position, dt);
                self.yaw = PI;
                self.yaw_target = PI;
                return false;
            }
            kind::FLOAT | kind::DRIFT => {
                // To another of eleven places round home (0xB5F2C); another family
                // has the same with places closer in (0xB15EC).
                let drift = self.kind == kind::DRIFT;
                let places = if drift { &DRIFT_PLACES } else { &FLOAT_PLACES };
                if self.phase == 0 {
                    let place = self.pick_place(places.len(), around.random);
                    self.near_home = place;
                    self.set_off(*position, around.home + places[place as usize]);
                }
                if self.travel(position, dt) {
                    // On arriving it readies another leg, which the next move throws
                    // away again; the numbers it draws for it are drawn all the same.
                    self.elapsed = 0.0;
                    let place = self.pick_place(places.len(), around.random);
                    self.near_home = place;
                    self.start = self.target;
                    self.target = around.home + places[place as usize];
                    self.control = if around.random.below(2) != 0 {
                        (self.start + self.target) * 0.5
                    } else {
                        let mut draw = || (around.random.below(2) - 1) as f32 * 100.0;
                        let (x, y, z) = (draw(), draw(), draw());
                        Vec3::new(x, y, z)
                    };
                }
                if first {
                    *around.aim = around.player;
                    let from = if drift { *position } else { self.last };
                    self.face(from, around.player, HOME_TURN, dt);
                }
            }
            kind::MEFE_FRONT..=kind::MEFE_FRONT_LAST | kind::MEFE_BACK..=kind::MEFE_BACK_LAST => {
                // A leap to one of her places (0xB6284, 0xB64F4): the first of its
                // three, or the one the kind names. The path is drawn up to a set
                // height, and what of it is above another is a fifth as steep.
                let back = self.kind >= kind::MEFE_BACK;
                if self.phase == 0 {
                    let which = self.kind - if back { kind::MEFE_BACK } else { kind::MEFE_FRONT };
                    let place = (which - 1).max(0) + if back { 3 } else { 0 };
                    self.near_home = place;
                    let (from, to) = (*position, around.home + MEFE_PLACES[place as usize]);
                    let peak = if back { 0.5 } else { MEFE_LEAP_PEAK };
                    self.start = from;
                    self.target = to;
                    self.control = Vec3::new(from.x + (to.x - from.x) * peak, MEFE_LEAP_HEIGHT, from.z + (to.z - from.z) * peak);
                    self.phase = 1;
                }
                if self.phase == 1 {
                    let arrived = self.travel(position, dt);
                    if position.y > MEFE_LEAP_EASES_AT {
                        position.y = position.y / MEFE_LEAP_EASE + MEFE_LEAP_EASES_AT;
                    }
                    if arrived {
                        self.elapsed = 0.0;
                        *position = self.target;
                    }
                }
                if first {
                    *around.aim = around.player;
                    let from = if self.kind == kind::MEFE_FRONT { self.last } else { *position };
                    self.face(from, around.player, HOME_TURN, dt);
                }
            }
            kind::MEFE_CLOSE => {
                // Up to her (0xB6750): to a point short of her and to one side of
                // the line from her to where it stands, noting three stops close by
                // her on that line, the last of them to the other side.
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.player);
                    let aside = Quat::from_rotation_y(heading(around.player, *position)) * Vec3::X;
                    let stop = |short: f32| *position - away * (apart - short);
                    self.stops = [stop(MEFE_STOPS[0]), stop(MEFE_STOPS[1])];
                    self.third_stop = stop(MEFE_STOPS[2]) + aside * MEFE_ACROSS_ASIDE;
                    self.set_off(*position, stop(MEFE_CLOSE_STOP) + aside * MEFE_CLOSE_ASIDE);
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, MEFE_CLOSE_TURN, dt);
                }
            }
            kind::MEFE_CLOSER | kind::MEFE_ACROSS => {
                // On to the second of those stops, along a path pulled back from
                // the straight (0xB6C14), or to the third (0xB6DD0).
                if self.phase == 0 {
                    let closer = self.kind == kind::MEFE_CLOSER;
                    self.set_off(*position, if closer { self.stops[1] } else { self.third_stop });
                    if closer {
                        self.control.z += MEFE_CLOSER_PULL;
                    }
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
            }
            kind::MAZUMA_PLACE..=kind::MAZUMA_PLACE_LAST | kind::MAZUMA_AT_PLAYER | kind::MAZUMA_HOME => {
                // She stays where she is for the move's length and is then, all at
                // once, somewhere else (0xB7378): at one of her places off home,
                // short of the player's ankle on the line to it, or at home.
                if self.phase == 0 {
                    self.start = *position;
                    self.target = match self.kind {
                        kind::MAZUMA_AT_PLAYER => {
                            let (away, apart) = away_on_ground(*position, around.ankle);
                            *position - away * (apart - MAZUMA_STOP)
                        }
                        kind::MAZUMA_HOME => around.home + Vec3::X * mazuma_aside(around),
                        _ => around.home + MAZUMA_PLACES[(self.kind - kind::MAZUMA_PLACE) as usize] + Vec3::X * mazuma_aside(around),
                    };
                    self.phase = 1;
                }
                self.appear(position, dt);
                if first {
                    *around.aim = around.player;
                    self.face(self.target, around.player, BOSS_TURN, dt);
                }
            }
            kind::MAZUMA_LEAP_HOME => {
                // A leap home (0xB75FC).
                if self.phase == 0 {
                    self.set_off(*position, around.home + Vec3::X * mazuma_aside(around));
                    self.control.y = MAZUMA_LEAP_HEIGHT;
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                if first {
                    *around.aim = around.player;
                    self.face(self.target, around.player, BOSS_TURN, dt);
                }
            }
            kind::MAZUMA_FLY_IN..=kind::MAZUMA_FLY_IN_LAST => {
                // In from far off to one side, to somewhere about where it was
                // across the stage and about the height it came in at, in the
                // stage's middle from front to back (0xB77D4); facing the player
                // all the way.
                if self.phase == 0 {
                    let from = MAZUMA_FLY_FROM[(self.kind - kind::MAZUMA_FLY_IN) as usize];
                    let across = (around.random.below(MAZUMA_FLY_ACROSS * 2) - MAZUMA_FLY_ACROSS) as f32;
                    let up = (around.random.below(MAZUMA_FLY_UP * 2) - MAZUMA_FLY_UP) as f32;
                    self.hop_off(from, Vec3::new(position.x + across, from.y + up, from.z), false);
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                *around.aim = around.player;
                self.face(*position, around.player, BOSS_TURN, dt);
            }
            kind::MEFE_GROUND => {
                // Down onto the ground where it is (0xBB78C).
                position.y = 0.0;
                self.flags |= flag::ENDED;
            }
            kind::PAIR_PLACE..=kind::PAIR_PLACE_LAST | kind::PAIR_BACK..=kind::PAIR_BACK_LAST => {
                // To one of its nine places (0xB7AB0): turning to the player as it
                // sets off, or turned back the way it came.
                let back = self.kind >= kind::PAIR_BACK;
                if self.phase == 0 {
                    let place = self.kind - if back { kind::PAIR_BACK } else { kind::PAIR_PLACE };
                    self.near_home = place;
                    self.set_off(*position, around.home + PAIR_PLACES[place as usize]);
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                if first && back {
                    self.face(self.target, self.start, PAIR_TURN, dt);
                } else if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                }
            }
            kind::PAIR_CLOSE | kind::PAIR_CLOSER => {
                // Up to the player's ankle on the line to it (0xB7C84). The first
                // turns to her as it sets off; the second leaves the turn be.
                let closer = self.kind == kind::PAIR_CLOSER;
                if self.phase == 0 {
                    let (away, apart) = away_on_ground(*position, around.ankle);
                    self.set_off(*position, *position - away * (apart - PAIR_STOPS[closer as usize]));
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                if first && !closer {
                    *around.aim = around.player;
                    self.face(*position, around.player, PAIR_TURN, dt);
                }
            }
            kind::PAIR_PASS | kind::PAIR_PASS_BACK => {
                // A pass across the stage before her, from far to one side to as
                // far to the other, at the height it set off from (0xB7F14); it is
                // turned one way or the other throughout.
                let back = self.kind == kind::PAIR_PASS_BACK;
                if self.phase == 0 {
                    let side = if back { -PAIR_PASS_FROM } else { PAIR_PASS_FROM };
                    let from = Vec3::new(side, 0.0, PAIR_PASS_DEPTH);
                    self.set_off(from, Vec3::new(-side, position.y, PAIR_PASS_DEPTH));
                    *position = from;
                }
                if self.travel(position, dt) {
                    *position = self.target;
                }
                self.yaw = if back { PI } else { 0.0 };
                self.yaw_target = self.yaw;
                return false;
            }
            kind::LIRIO_PLACE..=kind::LIRIO_PLACE_LAST
            | kind::LIRIO_AT_PLAYER
            | kind::LIRIO_HOME
            | kind::LIRIO_AWAY
            | kind::WHITE_PLACE..=kind::WHITE_PLACE_LAST
            | kind::WHITE_AT_PLAYER
            | kind::WHITE_HOME
            | kind::WHITE_SIDE..=kind::WHITE_SIDE_LAST
            | kind::LAST_PLACE..=kind::LAST_PLACE_LAST
            | kind::LAST_AT_PLAYER
            | kind::LAST_HOME
            | kind::LAST_SIDE..=kind::LAST_SIDE_LAST => {
                // Three bosses are where they are for a move's length and then,
                // all at once, somewhere else (0xB81DC, 0xB92CC, 0xAF2B8, 0xB94F0,
                // 0xAF3B4): at a place off home, before the player, or at home.
                if self.phase == 0 {
                    let to_ankle = |short: f32| {
                        let (away, apart) = away_on_ground(*position, around.ankle);
                        *position - away * (apart - short)
                    };
                    self.start = *position;
                    self.target = match self.kind {
                        kind::LIRIO_PLACE..=kind::LIRIO_PLACE_LAST => around.home + LIRIO_PLACES[(self.kind - kind::LIRIO_PLACE) as usize],
                        kind::LIRIO_AT_PLAYER => {
                            let (away, apart) = away_on_ground(*position, around.player);
                            *position - away * (apart - LIRIO_STOP)
                        }
                        kind::LIRIO_AWAY => around.home + Vec3::Z * LIRIO_AWAY,
                        kind::WHITE_PLACE..=kind::WHITE_PLACE_LAST => around.home + WHITE_PLACES[(self.kind - kind::WHITE_PLACE) as usize],
                        kind::WHITE_AT_PLAYER => to_ankle(WHITE_STOP).with_y(around.home.y - WHITE_SUNK),
                        kind::WHITE_SIDE..=kind::WHITE_SIDE_LAST => around.home + WHITE_SIDES[(self.kind - kind::WHITE_SIDE) as usize],
                        kind::LAST_PLACE..=kind::LAST_PLACE_LAST => {
                            let place = around.home + LAST_PLACES[(self.kind - kind::LAST_PLACE) as usize];
                            place.with_y(around.random.below(LAST_HEIGHTS) as f32 * LAST_HEIGHT_STEP)
                        }
                        kind::LAST_AT_PLAYER => to_ankle(LAST_STOP),
                        kind::LAST_SIDE..=kind::LAST_SIDE_LAST => around.home + LAST_SIDES[(self.kind - kind::LAST_SIDE) as usize],
                        _ => around.home,
                    };
                    self.phase = 1;
                }
                self.appear(position, dt);
                if first {
                    *around.aim = around.player;
                    self.face(self.target, around.player, BOSS_TURN, dt);
                }
            }
            kind::LIRIO_GLIDE..=kind::LIRIO_GLIDE_LAST => {
                // A glide to a place beside home, quick at first, along the
                // ground's height and ending on it (0xB8430).
                if self.phase == 0 {
                    self.hop_off(*position, around.home + LIRIO_GLIDES[(self.kind - kind::LIRIO_GLIDE) as usize], false);
                    self.control.y = 0.0;
                }
                self.land(position, dt);
                if first {
                    *around.aim = around.player;
                    self.face(self.target, around.player, BOSS_TURN, dt);
                }
            }
            kind::NAFEHE_HOME | kind::NAFEHE_UP | kind::NAFEHE_DOWN => {
                // To its place over home (0xB863C, 0xB89D8), or straight down to the
                // ground where it is (0xB882C).
                if self.phase == 0 {
                    let to = if self.kind == kind::NAFEHE_DOWN { position.with_y(0.0) } else { around.home + NAFEHE_PLACE };
                    self.set_off(*position, to);
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                    if self.kind == kind::NAFEHE_DOWN {
                        position.y = 0.0;
                    }
                }
                if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                }
            }
            kind::ZAHA_PLACE..=kind::ZAHA_PLACE_LAST | kind::LAST_SWEEP..=kind::LAST_SWEEP_LAST => {
                // To home or a place beside it by a path that goes past its end
                // and comes back to it (0xB8B80, 0xB9738). The sixth boss turns to
                // the player as it sets off; the last faces her all the way.
                let last = self.kind >= kind::LAST_SWEEP;
                if self.phase == 0 {
                    let place = if last { LAST_SIDES[(self.kind - kind::LAST_SWEEP) as usize] } else { ZAHA_PLACES[(self.kind - kind::ZAHA_PLACE) as usize] };
                    self.set_off(*position, around.home + place);
                    self.control = self.start + (self.target - self.start) * OVERSHOOT;
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                    *position = self.target;
                }
                if last {
                    *around.aim = around.player;
                    self.face(*position, around.player, BOSS_TURN, dt);
                } else if first {
                    *around.aim = around.player;
                    self.face(*position, around.player, HOME_TURN, dt);
                }
            }
            kind::ZAHA_CHASE | kind::LAST_CHASE => {
                // After the player (0xB8D9C, 0xB9960): each frame the way is laid
                // afresh from where it is to a point by her, and it is taken as
                // far along that as the move's time has run.
                let last = self.kind == kind::LAST_CHASE;
                if self.phase < 2 {
                    self.phase = 1;
                    let to = if last {
                        around.ankle + Vec3::Z * LAST_CHASE_BEYOND
                    } else {
                        let (away, apart) = away_on_ground(*position, around.player);
                        *position - away * (apart - ZAHA_STOP)
                    };
                    self.set_off(*position, to);
                    self.elapsed += dt;
                    *position = self.on_path(if self.length < dt { 1.0 } else { self.elapsed / self.length });
                    if self.elapsed >= self.length {
                        self.elapsed = 0.0;
                        *position = self.target;
                        self.phase = 2;
                        self.flags |= flag::ENDED;
                    }
                }
                if last || first {
                    *around.aim = around.player;
                    self.face(*position, around.player, if last { BOSS_TURN } else { HOME_TURN }, dt);
                }
            }
            kind::ZAHA_HOME => {
                // Straight home, turned back the way it came (0xB8D9C).
                if self.phase == 0 {
                    self.set_off(*position, around.home);
                }
                if self.travel(position, dt) {
                    self.elapsed = 0.0;
                    *position = self.target;
                }
                if first {
                    self.face(self.target, self.start, HOME_TURN, dt);
                }
            }
            kind::ZAHA_AT_PLAYER | kind::ZAHA_TO_HOME => {
                // At once before the player, or at home (0xB9110); it turns to her
                // when it is there.
                if self.phase == 0 {
                    self.start = *position;
                    self.target = if self.kind == kind::ZAHA_AT_PLAYER {
                        let (away, apart) = away_on_ground(*position, around.ankle);
                        *position - away * (apart - ZAHA_STOP)
                    } else {
                        around.home
                    };
                    self.phase = 1;
                }
                self.appear(position, dt);
                if self.flags & flag::ENDED != 0 {
                    *around.aim = around.player;
                    self.face(*position, around.player, BOSS_TURN, dt);
                }
            }
            kind::LAST_GONE => {
                // Away: far over the stage, at once (0xBC03C).
                position.y = LAST_GONE_HEIGHT;
                self.flags |= flag::ENDED;
            }
            // A kind that is not known ends at once, as it does in the original for
            // one its switch has no case for.
            _ => {
                self.flags |= flag::ENDED;
                self.unknown = Some(self.kind);
            }
        }
        true
    }

    /// One of `count` places other than the one picked last (0xAF7FC, 0xAF570).
    fn pick_place(&mut self, count: usize, random: &mut Random) -> i32 {
        loop {
            let place = random.below(count as i32);
            if place != self.near_home {
                return place;
            }
        }
    }

    /// Begins a straight path from `from` to `to`.
    fn set_off(&mut self, from: Vec3, to: Vec3) {
        self.start = from;
        self.target = to;
        self.control = (from + to) * 0.5;
        self.phase = 1;
    }

    /// Begins a path from `from` to `to` that is quick at first and slows into its
    /// end, its control point two thirds of the way along; a hop lifts that point
    /// to a set height (0xB5A24, 0xB5DE0).
    fn hop_off(&mut self, from: Vec3, to: Vec3, hop: bool) {
        self.start = from;
        self.target = to;
        self.control = from + (to - from) * 2.0 / 3.0;
        if hop {
            self.control.y = POUNCE_HEIGHT;
        }
        self.phase = 1;
    }

    /// A frame of a move that leaves the enemy where it is for its length and then
    /// has it be where the move was going, all at once (0xB7590 and its like).
    fn appear(&mut self, position: &mut Vec3, dt: f32) {
        self.elapsed += dt;
        if self.elapsed >= self.length {
            *position = self.target;
            self.flags |= flag::ENDED;
        }
    }

    /// A frame of a path that ends standing on the ground at its target.
    fn land(&mut self, position: &mut Vec3, dt: f32) {
        if self.travel(position, dt) {
            *position = Vec3::new(self.target.x, 0.0, self.target.z);
        }
    }

    /// Carries `position` along the path for one frame; true on the frame it ends.
    /// The path is a curve through a control point, which for a straight path is its
    /// middle (the code shared by 0xAF788 and its neighbours).
    fn travel(&mut self, position: &mut Vec3, dt: f32) -> bool {
        if self.phase != 1 {
            return false;
        }
        self.elapsed += dt;
        *position = self.on_path(if self.length > 0.0 { self.elapsed / self.length } else { 1.0 });
        if self.elapsed < self.length {
            return false;
        }
        self.phase = 2;
        self.flags |= flag::ENDED;
        true
    }

    /// The point `along` the path: 0 its start, 1 its target.
    fn on_path(&self, along: f32) -> Vec3 {
        let left = 1.0 - along;
        self.start * (left * left) + self.control * (2.0 * left * along) + self.target * (along * along)
    }

    /// One frame of a move whose path is laid afresh each frame from where it is to
    /// where it is headed now: it takes the share of the path that the frame is of
    /// the time left, and the time left runs down (0xB1F7C, 0xB4ECC).
    fn follow(&mut self, dt: f32) -> Vec3 {
        self.elapsed += dt;
        let along = if self.length < dt { 1.0 } else { self.elapsed / self.length };
        let at = self.on_path(along);
        self.elapsed = 0.0;
        self.length -= dt;
        at
    }

    /// Dropping in from above, or rising from below, to home (0xAEACC, 0xAEC28).
    fn fall(&mut self, position: &mut Vec3, rising: bool, around: &mut Around) {
        // A drop speeds up by gravity; a rise by a fifth of it, upwards.
        let pull = if rising { around.dt * GRAVITY / 5.0 } else { -around.dt * GRAVITY };
        if self.phase == 0 {
            let far = around.random.below(1000) as f32;
            self.start = around.home;
            self.target = around.home;
            self.start.y = if rising { -400.0 - far } else { far + 1000.0 };
            self.control = Vec3::new(0.0, if rising { pull } else { pull * 2.0 }, 0.0);
            self.phase = 1;
        }
        if self.phase != 1 {
            return;
        }
        self.start += self.control;
        *position = self.start;
        let arrived = if rising { self.start.y >= around.home.y } else { self.start.y <= around.home.y };
        if arrived {
            position.y = around.home.y;
            self.start.y = around.home.y;
            self.flags |= flag::ENDED;
        }
        self.control.y += pull;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 2.0;
    const PLAYER: Vec3 = Vec3::new(395.0, 0.0, -1447.0);
    const HOME: Vec3 = Vec3::new(600.0, 0.0, 0.0);

    /// The home of an enemy that flies.
    const PERCH: Vec3 = Vec3::new(-300.0, 250.0, 100.0);

    /// Her right ankle, from the middle of her body as the benches have it.
    const ANKLE: Vec3 = Vec3::new(-7.0, 12.0, 0.0);

    /// A mover at home, as an enemy's script sets it up, and what is around it.
    struct Bench {
        mover: Mover,
        home: Vec3,
        player: Vec3,
        aim: Vec3,
        random: Random,
    }

    impl Bench {
        fn new() -> Bench {
            Bench::at(HOME)
        }

        fn at(home: Vec3) -> Bench {
            let mut mover = Mover::default();
            mover.init(home, Vec3::ZERO);
            Bench { mover, home, player: PLAYER, aim: Vec3::ZERO, random: Random::new(11) }
        }

        fn step(&mut self) -> bool {
            let around = &mut Around {
                dt: DT,
                home: self.home,
                player: self.player,
                ankle: self.player + ANKLE,
                called: false,
                aim: &mut self.aim,
                random: &mut self.random,
            };
            self.mover.update(around)
        }

        /// Starts a move and runs it to its end; returns the frames it took.
        fn run(&mut self, kind: i32, frames: i32) -> usize {
            self.mover.start(0, kind, frames);
            let mut steps = 0;
            while !self.mover.ended() {
                self.step();
                steps += 1;
                assert!(steps < 1000, "move {kind} never ended");
            }
            steps
        }

        fn facing(&self) -> Vec3 {
            self.mover.placement().rotation * Vec3::Z
        }
    }

    #[test]
    fn a_fresh_enemy_faces_the_middle_of_the_stage_and_does_not_move() {
        let mut bench = Bench::new();
        assert!(bench.facing().distance(Vec3::NEG_X) < 1e-5);
        assert!(bench.mover.ended());
        assert!(!bench.step());
        assert_eq!(bench.mover.position, HOME);
    }

    #[test]
    fn turning_to_the_player_ends_at_once_and_takes_the_frames_given() {
        let mut bench = Bench::new();
        // The enemy's first move, as the Eater's script makes it: over two frames.
        assert_eq!(bench.run(kind::FACE_PLAYER, 2), 1);
        assert_eq!(bench.aim, PLAYER);
        bench.step();
        bench.step();
        // The first turn is measured from the stage's middle, where the block's
        // record of the last position still is.
        let expected = (PLAYER - Vec3::ZERO).normalize();
        assert!(bench.facing().distance(expected) < 1e-4, "facing {}", bench.facing());
        assert_eq!(bench.mover.position, HOME);
    }

    #[test]
    fn a_turn_goes_the_short_way_round() {
        let mut mover = Mover::default();
        mover.yaw = 0.1;
        mover.face(Vec3::ZERO, Vec3::new(-1.0, 0.0, 10.0), 4.0, DT);
        // From just right of ahead to just left of it: a small turn to the left, not
        // nearly a whole turn to the right.
        assert!(mover.turn_step < 0.0 && mover.turn_step > -0.1, "step {}", mover.turn_step);
        assert_eq!(mover.turn_left, 8.0);
    }

    #[test]
    fn dropping_in_falls_from_above_and_lands_at_home() {
        let mut bench = Bench::new();
        bench.mover.start(0, kind::DROP_IN, 12);
        bench.step();
        let top = bench.mover.position.y;
        assert!((990.0..2000.0).contains(&top), "started at {top}");
        let mut steps = 1;
        let mut last = top;
        while !bench.mover.ended() {
            bench.step();
            steps += 1;
            assert!(bench.mover.position.y < last, "it should only fall");
            last = bench.mover.position.y;
        }
        assert_eq!(bench.mover.position, HOME);
        // From 1000 to 2000 up, at 3.92 a frame and 1.96 more each frame.
        assert!((30..46).contains(&steps), "took {steps} frames");
    }

    #[test]
    fn rising_comes_up_from_below() {
        let mut bench = Bench::new();
        bench.mover.start(0, kind::RISE, 12);
        bench.step();
        assert!(bench.mover.position.y < -390.0);
        while !bench.mover.ended() {
            bench.step();
        }
        assert_eq!(bench.mover.position, HOME);
    }

    #[test]
    fn a_wait_lasts_its_length_in_sixtieths_and_moves_nothing() {
        let mut bench = Bench::new();
        assert_eq!(bench.run(kind::WAIT, 30), 15);
        assert_eq!(bench.mover.position, HOME);
    }

    #[test]
    fn a_bearing_is_faced_at_once() {
        let mut bench = Bench::new();
        // No bearing at all is straight at the player's side of the stage.
        assert_eq!(bench.run(kind::FACE_BEARING, 0), 1);
        assert!(bench.facing().distance(Vec3::NEG_Z) < 1e-5, "facing {}", bench.facing());
        // More is further towards +X, which is her left.
        bench.run(kind::FACE_BEARING, 15);
        let expected = Vec3::new(15f32.to_radians().sin(), 0.0, -15f32.to_radians().cos());
        assert!(bench.facing().distance(expected) < 1e-5, "facing {}", bench.facing());
        bench.run(kind::FACE_BEARING, 90);
        assert!(bench.facing().distance(Vec3::X) < 1e-5, "facing {}", bench.facing());
        assert_eq!(bench.mover.position, HOME);
    }

    #[test]
    fn a_head_turns_to_the_player_while_the_body_stays() {
        let mut bench = Bench::new();
        bench.run(kind::FACE_BEARING, 0);
        let body = bench.facing();
        let bearing = heading(HOME, PLAYER);

        // Six sixtieths: a third of the way each frame, from where the head was.
        bench.mover.start(0, kind::TURN_HEAD, 6);
        bench.step();
        let (node, turn) = bench.mover.node_turn.unwrap();
        assert_eq!(node, HEAD_NODE);
        assert!((bench.mover.yaw + turn - bearing / 3.0).abs() < 1e-5);
        assert!(!bench.mover.ended());
        bench.step();
        bench.step();
        assert!(bench.mover.ended());

        // The head's turn is on top of the body's: together they are her bearing.
        let (_, turn) = bench.mover.node_turn.unwrap();
        let head = Quat::from_rotation_y(bench.mover.yaw + turn) * Vec3::Z;
        let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - HOME).normalize();
        assert!(head.distance(expected) < 1e-4, "the head faces {head}");
        assert!(bench.facing().distance(body) < 1e-6);
        assert_eq!(bench.mover.position, HOME);
        assert_eq!(bench.aim, PLAYER);

        // Turned again with her where she was, it stays as it is.
        bench.run(kind::TURN_HEAD, 2);
        assert!((bench.mover.node_turn.unwrap().1 - turn).abs() < 1e-5);
    }

    #[test]
    fn an_approach_stops_short_of_the_player_in_three_moves() {
        let mut bench = Bench::new();
        let apart = |bench: &Bench| {
            let position = bench.mover.position;
            Vec2::new(position.x - PLAYER.x, position.z - PLAYER.z).length()
        };
        assert_eq!(bench.run(kind::APPROACH, 10), 5);
        assert!((apart(&bench) - 400.0).abs() < 0.01, "stopped {} away", apart(&bench));
        assert_eq!(bench.run(kind::APPROACH_CLOSER, 6), 3);
        assert!((apart(&bench) - 200.0).abs() < 0.01);
        assert_eq!(bench.run(kind::APPROACH_CLOSEST, 6), 3);
        assert!((apart(&bench) - 50.0).abs() < 0.01);
        // It set off facing the way it went.
        let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - HOME).normalize();
        assert!(bench.facing().distance(expected) < 1e-3);
    }

    #[test]
    fn an_approach_travels_at_an_even_pace() {
        let mut bench = Bench::new();
        bench.mover.start(0, kind::APPROACH, 20);
        let mut places = vec![bench.mover.position];
        for _ in 0..10 {
            bench.step();
            places.push(bench.mover.position);
        }
        let strides: Vec<f32> = places.windows(2).map(|pair| pair[0].distance(pair[1])).collect();
        assert!(strides.iter().all(|stride| (stride - strides[0]).abs() < 0.01), "strides {strides:?}");
    }

    #[test]
    fn the_hop_home_arcs_up_and_lands_near_home() {
        let mut bench = Bench::new();
        bench.run(kind::APPROACH, 10);
        bench.mover.start(0, kind::HOP_HOME, 60);
        let mut highest = 0.0f32;
        let mut steps = 0;
        while !bench.mover.ended() {
            bench.step();
            highest = highest.max(bench.mover.position.y);
            steps += 1;
        }
        assert_eq!(steps, 30);
        // A control point 1000 up lifts the middle of the path to half that.
        assert!((highest - 500.0).abs() < 1.0, "peaked at {highest}");
        assert!(NEAR_HOME.iter().any(|place| (HOME + *place).distance(bench.mover.position) < 0.01));
    }

    #[test]
    fn going_home_twice_picks_different_places() {
        let mut bench = Bench::new();
        let mut last = Vec3::splat(f32::MAX);
        for _ in 0..20 {
            bench.run(kind::NEAR_HOME, 50);
            assert_ne!(bench.mover.position, last);
            last = bench.mover.position;
        }
        // Afterwards it has turned to the player: to where she was from where it set
        // off, which is near enough.
        for _ in 0..12 {
            bench.step();
        }
        let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - bench.mover.position).normalize();
        assert!(bench.facing().dot(expected) > 0.95, "facing {}", bench.facing());
    }

    #[test]
    fn the_advances_stop_ever_closer() {
        let mut bench = Bench::new();
        for (index, short) in ADVANCE_STOPS.iter().enumerate() {
            bench.run(kind::ADVANCE_FIRST + index as i32, 50);
            let position = bench.mover.position;
            let apart = Vec2::new(position.x - PLAYER.x, position.z - PLAYER.z).length();
            assert!((apart - short).abs() < 0.01, "advance {index} stopped {apart} away");
        }
    }

    #[test]
    fn a_rush_weaves_its_way_to_the_player() {
        let mut bench = Bench::new();
        assert_eq!(bench.run(kind::RUSH, 300), 150);
        let position = bench.mover.position;
        let apart = Vec2::new(position.x - PLAYER.x, position.z - PLAYER.z).length();
        assert!(apart < RUSH_STOP + 41.0, "ended {apart} away");
    }

    #[test]
    fn a_flyer_drifts_between_the_places_round_its_home() {
        let mut bench = Bench::at(PERCH);
        let mut last = Vec3::splat(f32::MAX);
        for _ in 0..20 {
            // Forty sixtieths, as the Hornet's script asks for.
            assert_eq!(bench.run(kind::HOVER, 40), 20);
            let at = bench.mover.position;
            assert!(HOVER_PLACES.iter().any(|place| (PERCH + *place).distance(at) < 0.01), "ended at {at}");
            assert!(at.distance(last) > 1.0, "it picked the same place twice");
            last = at;
        }
        // It turns to the player as it goes.
        for _ in 0..12 {
            bench.step();
        }
        assert_eq!(bench.aim, PLAYER);
        assert!(bench.facing().z < 0.0);
    }

    #[test]
    fn a_drift_keeps_its_pace_and_a_dart_slows_into_its_place() {
        let strides = |kind: i32| {
            let mut bench = Bench::at(PERCH);
            // Away from the middle first, so that there is some way to go.
            while bench.mover.position.distance(PERCH) < 1.0 {
                bench.run(kind, 16);
            }
            bench.mover.start(0, kind, 16);
            let mut places = vec![bench.mover.position];
            while !bench.mover.ended() {
                bench.step();
                places.push(bench.mover.position);
            }
            assert_eq!(places.len(), 9);
            places.windows(2).map(|pair| pair[0].distance(pair[1])).collect::<Vec<f32>>()
        };
        let drift = strides(kind::HOVER);
        assert!(drift.iter().all(|stride| (stride - drift[0]).abs() < 0.01), "strides {drift:?}");
        let dart = strides(kind::DART);
        assert!(dart.windows(2).all(|pair| pair[0] > pair[1]), "strides {dart:?}");
        // It sets off a third faster than an even pace and arrives a third slower.
        let even = dart.iter().sum::<f32>() / dart.len() as f32;
        assert!(dart[0] > even * 1.2 && dart[7] < even * 0.8, "strides {dart:?}");
    }

    #[test]
    fn a_flyer_shot_down_falls_to_the_ground_and_rises_again() {
        let mut bench = Bench::at(PERCH);
        bench.mover.start(0, kind::FALL, 20);
        let mut last = PERCH.y;
        let mut drops = Vec::new();
        while !bench.mover.ended() {
            bench.step();
            drops.push(last - bench.mover.position.y);
            last = bench.mover.position.y;
        }
        assert_eq!(bench.mover.position, Vec3::new(PERCH.x, 0.0, PERCH.z));
        // 1.96 further each frame than the frame before: 250 down takes 16 frames.
        assert_eq!(drops.len(), 16);
        assert!((drops[0] - 1.96).abs() < 1e-4 && (drops[1] - 3.92).abs() < 1e-4, "drops {drops:?}");

        bench.mover.start(0, kind::RISE_HOME, 20);
        let mut steps = 0;
        while !bench.mover.ended() {
            bench.step();
            steps += 1;
        }
        assert_eq!(steps, 16);
        assert_eq!(bench.mover.position, PERCH);
    }

    /// How far `at` is from the player on the ground.
    fn apart(at: Vec3, from: Vec3) -> f32 {
        Vec2::new(at.x - from.x, at.z - from.z).length()
    }

    #[test]
    fn a_step_goes_to_another_place_round_home_along_the_ground() {
        let mut bench = Bench::new();
        let mut last = Vec3::splat(f32::MAX);
        for _ in 0..12 {
            assert_eq!(bench.run(kind::STEP, 20), 10);
            let at = bench.mover.position;
            assert!(STEP_PLACES.iter().any(|place| HOME + *place == at), "ended at {at}");
            assert_ne!(at, last);
            last = at;
        }
    }

    #[test]
    fn a_chase_ends_in_front_of_the_player_wherever_she_goes() {
        let mut bench = Bench::at(PERCH);
        bench.mover.start(0, kind::CHASE, 40);
        for _ in 0..10 {
            bench.step();
            assert!(!bench.mover.ended());
        }
        // Half-way through she goes over to her other place.
        bench.player = Vec3::new(-395.0, 90.0, -1447.0);
        let mut steps = 10;
        while !bench.mover.ended() {
            bench.step();
            steps += 1;
            assert!(steps < 100);
        }
        // It still takes its forty sixtieths, and ends 150 short of her, 70 up.
        assert_eq!(steps, 20);
        let at = bench.mover.position;
        assert!((apart(at, bench.player) - CHASE_STOP).abs() < 0.01, "ended at {at}");
        assert!((at.y - CHASE_HEIGHT).abs() < 1e-3);
        let expected = (Vec3::new(bench.player.x, 0.0, bench.player.z) - Vec3::new(at.x, 0.0, at.z)).normalize();
        assert!(bench.facing().distance(expected) < 1e-3);
        assert_eq!(bench.aim, bench.player);
    }

    #[test]
    fn a_swoop_darts_to_its_place_off_home() {
        for (index, place) in SWOOP_PLACES.iter().enumerate() {
            let mut bench = Bench::at(PERCH);
            assert_eq!(bench.run(kind::SWOOP_FIRST + index as i32, 16), 8);
            assert!(bench.mover.position.distance(PERCH + *place) < 1e-3, "swoop {index} ended at {}", bench.mover.position);
        }
    }

    #[test]
    fn big_mouth_moves_only_between_footfalls() {
        // A stride is 126 sixtieths; from 8 to 50 and from 70 to 112 of it both
        // feet are down.
        for (clock, down) in [(2.0, false), (6.0, false), (8.0, true), (48.0, true), (50.0, false), (68.0, false), (70.0, true), (110.0, true), (112.0, false)] {
            assert_eq!(planted(clock, DT), down, "at {clock}");
            assert_eq!(planted(clock + STRIDE * 3.0, DT), down, "at {clock} of the fourth stride");
        }
        // The rhythm is kept for ten strides, and is counted in halves of the step.
        assert!(!planted(STRIDE * 10.0 + 8.0, DT));
        assert!(planted(4.0, 1.0) && !planted(3.0, 1.0));

        // Forty sixtieths of walking: three frames, a footfall, ten frames, another
        // footfall, and the last seven.
        let mut bench = Bench::new();
        bench.mover.start(0, kind::STOMP, 40);
        let mut places = vec![bench.mover.position];
        while !bench.mover.ended() {
            bench.step();
            places.push(bench.mover.position);
            assert!(places.len() < 500);
        }
        assert_eq!(places.len() - 1, 62);
        let moved: Vec<bool> = places.windows(2).map(|pair| pair[0] != pair[1]).collect();
        assert!(moved[..3].iter().all(|moved| *moved) && moved[3..24].iter().all(|moved| !*moved));
        assert_eq!(moved.iter().filter(|moved| **moved).count(), 20);
        assert!((apart(bench.mover.position, PLAYER) - STOMP_STOP).abs() < 0.01);
        assert_eq!(bench.mover.position.y, 0.0);

        // The longer advance stops further off, and the walk back out goes to its
        // own distance from where she was.
        let mut bench = Bench::new();
        assert_eq!(bench.run(kind::STOMP_FAR, 40), 62);
        assert!((apart(bench.mover.position, PLAYER) - STOMP_FAR_STOP).abs() < 0.01);
        assert_eq!(bench.run(kind::STOMP_BACK, 40), 62);
        assert!((apart(bench.mover.position, PLAYER) - STOMP_BACK_STOP).abs() < 0.01);
    }

    #[test]
    fn a_lift_goes_up_past_its_mark_and_settles_on_it() {
        let mut bench = Bench::new();
        bench.mover.start(0, kind::LIFT, 40);
        let mut highest = 0f32;
        let mut steps = 0;
        while !bench.mover.ended() {
            bench.step();
            highest = highest.max(bench.mover.position.y);
            steps += 1;
        }
        assert_eq!(steps, 20);
        assert!(bench.mover.position.distance(HOME + Vec3::Y * LIFT_HEIGHT) < 1e-3);
        // Three quarters of the way through it is an eighth of the height above.
        assert!((highest - LIFT_HEIGHT * 1.125).abs() < 1e-2, "peaked at {highest}");
        assert!(bench.facing().distance(Vec3::NEG_Z) < 1e-5);

        assert_eq!(bench.run(kind::SINK, 40), 20);
        assert!(bench.mover.position.distance(HOME) < 1e-3);
    }

    #[test]
    fn a_pounce_lands_in_front_of_the_player_and_hops_home_again() {
        let mut bench = Bench::new();
        bench.mover.start(0, kind::POUNCE, 20);
        let mut highest = 0f32;
        while !bench.mover.ended() {
            bench.step();
            highest = highest.max(bench.mover.position.y);
        }
        // Low over the ground, to 600 short of her ankle.
        let at = bench.mover.position;
        assert!((apart(at, PLAYER + ANKLE) - POUNCE_STOP).abs() < 0.01, "landed at {at}");
        assert_eq!(at.y, 0.0);
        assert!(highest > 20.0 && highest < POUNCE_HEIGHT, "peaked at {highest}");
        // Landed, it turns to her within two frames.
        bench.step();
        let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - at).normalize();
        assert!(bench.facing().distance(expected) < 1e-3, "facing {}", bench.facing());

        assert_eq!(bench.run(kind::POUNCE_HOME, 20), 10);
        assert_eq!(bench.mover.position, HOME);
    }

    /// Runs a move to its end and returns how high it got on the way.
    fn peak(bench: &mut Bench, kind: i32, frames: i32) -> f32 {
        bench.mover.start(0, kind, frames);
        let mut highest = f32::MIN;
        while !bench.mover.ended() {
            bench.step();
            highest = highest.max(bench.mover.position.y);
        }
        highest
    }

    #[test]
    fn the_other_families_advances_stop_at_their_own_distances() {
        // One family's end on the ground, wherever they set off from.
        let mut bench = Bench::at(PERCH);
        for (index, short) in TREAD_STOPS.iter().enumerate() {
            assert_eq!(bench.run(kind::TREAD_FIRST + index as i32, 40), 20);
            assert!((apart(bench.mover.position, PLAYER) - short).abs() < 0.01, "tread {index}");
            assert_eq!(bench.mover.position.y, 0.0);
        }
        // Cone Head has the Eaters' distances.
        let mut bench = Bench::new();
        for (index, short) in ADVANCE_STOPS.iter().enumerate() {
            bench.run(kind::ROLL_FIRST + index as i32, 50);
            assert!((apart(bench.mover.position, PLAYER) - short).abs() < 0.01, "roll {index}");
        }
        // The flier's line slopes with its height, and it looks at her side of the
        // stage all the way.
        let mut bench = Bench::at(PERCH);
        bench.run(kind::GLIDE_FIRST, 40);
        let slope = Vec3::new(PERCH.x - PLAYER.x, PERCH.y, PERCH.z - PLAYER.z).normalize();
        let expected = PERCH - slope * (apart(PERCH, PLAYER) - TREAD_STOPS[0]);
        assert!(bench.mover.position.distance(expected) < 0.01, "glided to {}", bench.mover.position);
        assert!(bench.mover.position.y < PERCH.y);
        assert!(bench.facing().distance(Vec3::NEG_Z) < 1e-5);
    }

    #[test]
    fn a_leap_arcs_to_its_distance_and_the_third_kind_goes_on_to_the_same_spot() {
        let mut bench = Bench::new();
        let highest = peak(&mut bench, kind::LEAP_FIRST, 32);
        assert!((apart(bench.mover.position, PLAYER) - LEAP_STOPS[0]).abs() < 0.01);
        assert!((highest - LEAP_HEIGHT * 0.5).abs() < 0.5, "peaked at {highest}");
        bench.run(kind::LEAP_FIRST + 1, 32);
        let there = bench.mover.position;
        assert!((apart(there, PLAYER) - LEAP_STOPS[1]).abs() < 0.01);
        // Put elsewhere, the third kind takes it to where the leap before aimed:
        // whatever the block still has for a target.
        bench.mover.position = HOME;
        bench.run(kind::LEAP_LAST, 32);
        assert!(bench.mover.position.distance(there) < 0.01);
    }

    #[test]
    fn the_mini_rabbits_fly_in_and_swerve_into_formation() {
        for which in 0..4 {
            let mut bench = Bench::at(PERCH);
            bench.mover.start(0, kind::ARRIVE_FIRST + which, 60);
            bench.step();
            // It starts from its far place, wherever it was put.
            let far = PERCH + ARRIVE_FROM[which as usize];
            assert!(bench.mover.position.distance(far) < far.distance(PERCH) * 0.5, "way {which}");
            while !bench.mover.ended() {
                bench.step();
            }
            assert_eq!(bench.mover.position, PERCH);
        }
        for index in 0..16 {
            let mut bench = Bench::at(PERCH);
            assert_eq!(bench.run(kind::WEAVE_FIRST + index, 40), 20);
            let place = PERCH + WEAVE_PLACES[(index / 4) as usize][(index % 4) as usize];
            assert!(bench.mover.position.distance(place) < 0.01, "swerve {index} ended at {}", bench.mover.position);
        }
    }

    #[test]
    fn set_places_are_gone_to_and_landed_on() {
        // Across the stage, striding and hopping.
        for place in 0..5 {
            for (first, hop) in [(kind::CROSS_FIRST, false), (kind::CROSS_HOP_FIRST, true)] {
                let mut bench = Bench::new();
                let highest = peak(&mut bench, first + place, 30);
                assert_eq!(bench.mover.position, HOME + CROSS_PLACES[place as usize], "crossing {place}");
                assert_eq!(highest > 20.0, hop, "crossing {place} got {highest} high");
                assert_eq!(bench.aim, PLAYER);
            }
        }
        // The two rows off home: the kind's digits name the row and the place.
        for (kind, row, place, hop) in [
            (1300, 0, 0, false),
            (1301, 0, 1, false),
            (1310, 1, 0, false),
            (1312, 1, 2, false),
            (1320, 0, 0, true),
            (1321, 0, 1, true),
            (1330, 1, 0, true),
            (1332, 1, 2, true),
        ] {
            let mut bench = Bench::new();
            let highest = peak(&mut bench, kind, 40);
            assert_eq!(bench.mover.position, HOME + LUNGE_PLACES[row][place], "move {kind}");
            assert_eq!(highest > 20.0, hop, "move {kind} got {highest} high");
        }
        // The Spider's rows, its walk up to her and its way home.
        for (kind, row, place) in [(400, 0, 0), (401, 0, 1), (410, 1, 0), (411, 1, 1), (412, 1, 2)] {
            let mut bench = Bench::new();
            bench.run(kind, 100);
            assert_eq!(bench.mover.position, HOME + TRUDGE_PLACES[row][place], "move {kind}");
        }
        let mut bench = Bench::new();
        bench.run(kind::TRUDGE_AT_PLAYER, 100);
        assert!((apart(bench.mover.position, PLAYER + ANKLE) - TRUDGE_STOP).abs() < 0.01);
        bench.run(kind::TRUDGE_HOME, 100);
        assert_eq!(bench.mover.position, HOME);
        // Home again it turns to her, over twelve frames.
        for _ in 0..12 {
            bench.step();
        }
        let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - HOME).normalize();
        assert!(bench.facing().distance(expected) < 1e-3, "facing {}", bench.facing());
    }

    #[test]
    fn the_original_and_the_port_agree_on_which_kinds_there_are() {
        // What the port has no code for it flags; what the original has none for
        // either is nothing to flag.
        let mut bench = Bench::new();
        bench.run(10200, 30);
        assert_eq!(bench.mover.unknown, Some(10200));
        assert!(original_has(900) && original_has(17) && original_has(10712));
        assert!(!original_has(18) && !original_has(1302) && !original_has(9999));
        // Everything below the bosses' own kinds is here, and the first boss's
        // that a script starts.
        let mefe = [
            (10000, 10008),
            (10010, 10011),
            (10013, 10013),
            (10100, 10110),
            (10202, 10223),
            (10300, 10310),
            (10400, 10400),
            (10402, 10403),
            (10500, 10506),
            (10600, 10609),
            (10700, 10712),
        ];
        for (first, last) in ORIGINAL_KINDS.iter().filter(|(first, _)| *first < 10000).chain(&mefe) {
            for kind in *first..=*last {
                let mut bench = Bench::new();
                bench.mover.start(0, kind, 30);
                bench.step();
                assert_eq!(bench.mover.unknown, None, "kind {kind}");
            }
        }
    }

    #[test]
    fn mefe_leaps_between_her_places_and_is_slow_to_rise_past_a_height() {
        // To the place before home that the kind names, and to the one beside it.
        for (kind, place) in [(10000, Vec3::new(0.0, 0.0, -200.0)), (10002, Vec3::new(300.0, 0.0, -180.0)), (10004, Vec3::ZERO), (10007, Vec3::new(-300.0, 0.0, 0.0))] {
            let mut bench = Bench::new();
            bench.mover.position = HOME + Vec3::new(40.0, 0.0, 700.0);
            // Forty sixtieths are twenty frames. As she sets off she begins to turn
            // to the player, from where the first frame has taken her.
            bench.mover.start(0, kind, 40);
            bench.step();
            let to_player = (PLAYER - bench.mover.position).with_y(0.0).normalize();
            let frames = 1 + (0..40).position(|_| bench.step() && bench.mover.ended()).unwrap() + 1;
            assert_eq!(frames, 20, "kind {kind}");
            assert_eq!(bench.mover.position, HOME + place, "kind {kind}");
            if kind != 10000 {
                assert!(bench.facing().distance(to_player) < 1e-3, "kind {kind}");
            }
        }
        // The path is drawn 250 high, so a leap along the ground would come to 125;
        // from 300 up it would come to more than 200, and what is over that counts
        // for a fifth.
        let mut bench = Bench::new();
        bench.mover.position = HOME + Vec3::new(0.0, 300.0, 600.0);
        let plain = |along: f32| 300.0 * (1.0 - along) * (1.0 - along) + 250.0 * 2.0 * (1.0 - along) * along;
        bench.mover.start(0, 10005, 40);
        bench.step();
        let drawn = plain(0.05);
        assert!(drawn > 200.0 && (bench.mover.position.y - (drawn / 5.0 + 200.0)).abs() < 1e-3, "{}", bench.mover.position.y);
    }

    #[test]
    fn mefe_closes_on_the_player_from_one_side_and_cuts_across_her() {
        let mut bench = Bench::new();
        let from = bench.mover.position;
        let (away, _) = away_on_ground(from, PLAYER);
        let aside = Vec3::new(away.z, 0.0, -away.x);
        let near = |short: f32| Vec3::new(PLAYER.x, from.y, PLAYER.z) + away * short;
        // Up to 600 short of her, 150 to one side of the line to her.
        assert_eq!(bench.run(10008, 20), 10);
        assert!(bench.mover.position.distance(near(600.0) + aside * 150.0) < 1e-2, "{}", bench.mover.position);
        // On to 125 short of her on that line, by a path pulled back from the
        // straight; and across her, to 120 short and 100 to the other side.
        bench.mover.start(0, 10010, 4);
        bench.step();
        let half_way = (near(600.0) + aside * 150.0 + near(125.0)) * 0.5;
        assert!(bench.mover.position.distance(half_way + Vec3::Z * -50.0) < 1e-2, "{}", bench.mover.position);
        bench.step();
        assert!(bench.mover.ended() && bench.mover.position.distance(near(125.0)) < 1e-2);
        assert_eq!(bench.run(10011, 6), 3);
        assert!(bench.mover.position.distance(near(120.0) - aside * 100.0) < 1e-2, "{}", bench.mover.position);
        // And one kind only sets her down where she is.
        bench.mover.position.y = 80.0;
        assert_eq!(bench.run(10013, 0), 1);
        assert_eq!(bench.mover.position.y, 0.0);
    }

    #[test]
    fn mazuma_is_in_one_place_and_then_in_another() {
        // She stays put for the move's length, and is then at the place.
        let to_ankle = PLAYER + ANKLE;
        let (away, _) = away_on_ground(HOME, to_ankle);
        let before_her = Vec3::new(to_ankle.x, HOME.y, to_ankle.z) + away * 250.0;
        for (kind, place) in [(10100, HOME + Vec3::new(100.0, 0.0, -240.0)), (10103, HOME + Vec3::new(-500.0, 0.0, -225.0)), (10104, before_her)] {
            let mut bench = Bench::new();
            bench.mover.start(0, kind, 20);
            for _ in 0..9 {
                bench.step();
                assert_eq!(bench.mover.position, HOME, "kind {kind}");
            }
            bench.step();
            assert!(bench.mover.ended() && bench.mover.position.distance(place) < 1e-2, "kind {kind}: {}", bench.mover.position);
        }
        // Home again from one of them, at once or by a leap that is drawn 200 high.
        let mut bench = Bench::new();
        bench.run(10101, 2);
        bench.run(10105, 2);
        assert_eq!(bench.mover.position, HOME);
        bench.run(10102, 2);
        bench.mover.start(0, 10106, 40);
        let top = (0..20).map(|_| (bench.step(), bench.mover.position.y).1).fold(0.0, f32::max);
        assert!(bench.mover.ended() && bench.mover.position == HOME && (top - 100.0).abs() < 1e-3, "{top}");
    }

    #[test]
    fn mazumas_swords_fly_in_from_either_side() {
        for (kind, from) in [(10107, Vec3::new(-1500.0, 400.0, 0.0)), (10108, Vec3::new(1500.0, 200.0, 0.0)), (10109, Vec3::new(-1500.0, 50.0, 0.0)), (10110, Vec3::new(1500.0, 300.0, 0.0))] {
            let mut bench = Bench::new();
            bench.mover.start(0, kind, 60);
            bench.step();
            // A thirtieth of the way in on its first frame, quick at first.
            assert!((bench.mover.position.x - from.x).abs() < 120.0 && (bench.mover.position.y - from.y).abs() < 10.0, "kind {kind}: {}", bench.mover.position);
            while !bench.mover.ended() {
                bench.step();
            }
            let at = bench.mover.position;
            assert!((at.x - HOME.x).abs() <= 200.0 && (at.y - from.y).abs() <= 100.0 && at.z == 0.0, "kind {kind}: {at}");
            // It faces the player as it comes.
            let to_player = (PLAYER - at).with_y(0.0).normalize();
            assert!(bench.facing().distance(to_player) < 0.2, "kind {kind}");
        }
    }

    #[test]
    fn the_later_bosses_are_all_at_once_somewhere_else() {
        let ankle = PLAYER + ANKLE;
        let before = |from: Vec3, of: Vec3, short: f32| Vec3::new(of.x, from.y, of.z) + away_on_ground(from, of).0 * short;
        let places = [
            (10300, HOME + Vec3::new(595.0, 0.0, -240.0)),
            (10304, HOME + Vec3::new(-180.0, 0.0, -225.0)),
            (10305, before(HOME, PLAYER, 200.0)),
            (10306, HOME),
            (10307, HOME + Vec3::Z * 1500.0),
            (10505, before(HOME, ankle, 700.0)),
            (10506, HOME),
            (10601, HOME + Vec3::new(60.0, 0.0, -415.0)),
            (10605, before(HOME, ankle, 100.0).with_y(HOME.y - 10.0)),
            (10606, HOME),
            (10609, HOME + Vec3::new(-500.0, 0.0, 10.0)),
            (10703, before(HOME, ankle, 400.0)),
            (10704, HOME),
            (10708, HOME + Vec3::new(400.0, 0.0, 10.0)),
        ];
        for (kind, place) in places {
            let mut bench = Bench::new();
            bench.mover.start(0, kind, 12);
            for _ in 0..5 {
                bench.step();
                assert_eq!(bench.mover.position, HOME, "kind {kind}");
            }
            bench.step();
            assert!(bench.mover.ended() && bench.mover.position.distance(place) < 1e-2, "kind {kind}: {}", bench.mover.position);
        }
        // The last boss's places are at one of five heights, ten apart.
        let mut bench = Bench::new();
        bench.run(10701, 2);
        let at = bench.mover.position;
        assert!((at.x, at.z) == (HOME.x, HOME.z - 300.0) && at.y % 10.0 == 0.0 && (0.0..=40.0).contains(&at.y), "{at}");
        // And one kind only has it gone, far over the stage.
        bench.run(10712, 0);
        assert_eq!(bench.mover.position.y, 5000.0);
    }

    #[test]
    fn the_later_bosses_ways_across_the_stage() {
        // The third boss goes between nine places, facing the player or the way it
        // came from.
        let ankle = PLAYER + ANKLE;
        let mut bench = Bench::new();
        bench.run(10205, 20);
        assert_eq!(bench.mover.position, HOME + Vec3::new(495.0, 0.0, 200.0));
        bench.run(10221, 20);
        assert_eq!(bench.mover.position, HOME + Vec3::new(-370.0, 0.0, -305.0));
        assert!(bench.facing().distance(Vec3::new(865.0, 0.0, 505.0).normalize()) < 1e-3);
        // It comes up to her ankle, and closer.
        let mut bench = Bench::new();
        assert_eq!(bench.run(10211, 20), 10);
        assert!((away_on_ground(bench.mover.position, ankle).1 - 250.0).abs() < 1e-2);
        bench.run(10212, 8);
        assert!((away_on_ground(bench.mover.position, ankle).1 - 100.0).abs() < 1e-2);
        // Its pass goes across before her, turned one way; and back, turned the other.
        bench.mover.start(0, 10222, 60);
        bench.step();
        assert!((bench.mover.position - Vec3::new(1400.0, 0.0, -1300.0)).length() < 1e-2, "{}", bench.mover.position);
        assert_eq!(bench.mover.yaw, 0.0);
        while !bench.mover.ended() {
            bench.step();
        }
        assert_eq!(bench.mover.position, Vec3::new(-1500.0, 0.0, -1300.0));
        bench.run(10223, 60);
        assert_eq!((bench.mover.position, bench.mover.yaw), (Vec3::new(1500.0, 0.0, -1300.0), PI));

        // The fourth glides to a place beside home and ends on the ground.
        let mut bench = Bench::at(PERCH);
        bench.run(10308, 30);
        assert_eq!(bench.mover.position, (PERCH + Vec3::new(500.0, 0.0, 10.0)).with_y(0.0));
        // The fifth comes down to the ground where it is and goes back up over home.
        let mut bench = Bench::new();
        bench.run(10400, 20);
        assert_eq!(bench.mover.position, HOME + Vec3::Y * 100.0);
        bench.run(10402, 20);
        assert_eq!(bench.mover.position, HOME);
        bench.run(10403, 20);
        assert_eq!(bench.mover.position, HOME + Vec3::Y * 100.0);

        // The sixth goes past a place beside home and comes back to it.
        let mut bench = Bench::new();
        bench.mover.start(0, 10501, 40);
        let furthest = (0..20).map(|_| (bench.step(), bench.mover.position.x).1).fold(0.0, f32::max);
        assert!(bench.mover.ended() && bench.mover.position == HOME + Vec3::X * 600.0 && furthest > HOME.x + 610.0, "{furthest}");
        // Its chase ends 700 short of her wherever she has gone meanwhile, and it
        // goes straight home turned back the way it came.
        bench.mover.start(0, 10503, 40);
        for _ in 0..10 {
            bench.step();
        }
        bench.player += Vec3::new(-300.0, 0.0, 80.0);
        while !bench.mover.ended() {
            bench.step();
        }
        assert!((away_on_ground(bench.mover.position, bench.player).1 - 700.0).abs() < 1e-2);
        let from = bench.mover.position;
        bench.run(10504, 40);
        assert_eq!(bench.mover.position, HOME);
        assert!(bench.facing().distance((from - HOME).with_y(0.0).normalize()) < 1e-3);

        // The last sweeps to a place beside home facing her, and chases her to a
        // point beyond her.
        let mut bench = Bench::new();
        bench.run(10707, 30);
        assert_eq!(bench.mover.position, HOME + Vec3::new(-400.0, 0.0, 10.0));
        assert!(bench.facing().distance((PLAYER - bench.mover.position).with_y(0.0).normalize()) < 1e-3);
        bench.run(10711, 30);
        assert!(bench.mover.position.distance(ankle + Vec3::Z * 400.0) < 1e-2);
    }

    #[test]
    fn the_chicken_hops_between_its_places_and_up_to_the_player() {
        let mut bench = Bench::new();
        // A hop goes half as high as its path is drawn, and ends on the ground at
        // one of the nine places; a vault does the same much higher.
        for (kind, height) in [(kind::SPRING, SPRING_HEIGHT), (kind::VAULT, VAULT_HEIGHT)] {
            for _ in 0..6 {
                let highest = peak(&mut bench, kind, 40);
                let at = bench.mover.position;
                assert!(SPRING_PLACES.iter().any(|place| (HOME + *place).distance(at) < 0.01), "ended at {at}");
                assert!((highest - height * 0.5).abs() < 1.0, "move {kind} peaked at {highest}");
            }
        }
        assert_eq!(bench.aim, PLAYER);
        // Up to her in two moves.
        assert_eq!(bench.run(kind::PROWL, 40), 20);
        assert!((apart(bench.mover.position, PLAYER) - PROWL_STOPS[0]).abs() < 0.01);
        bench.run(kind::PROWL_CLOSER, 20);
        assert!((apart(bench.mover.position, PLAYER) - PROWL_STOPS[1]).abs() < 0.01);
        // And back off from what it aims at, with a low hop.
        let highest = peak(&mut bench, kind::RECOIL, 20);
        assert!((apart(bench.mover.position, PLAYER) - PROWL_STOPS[1] - RECOIL_REACH).abs() < 0.01);
        assert!((highest - RECOIL_HEIGHT * 0.5).abs() < 0.5, "peaked at {highest}");
        assert_eq!(bench.mover.position.y, 0.0);

        // One in the air is put on the ground where it is.
        let mut bench = Bench::at(PERCH);
        assert_eq!(bench.run(kind::GROUND, 30), 1);
        assert_eq!(bench.mover.position, Vec3::new(PERCH.x, 0.0, PERCH.z));
    }

    #[test]
    fn cone_head_has_the_eaters_moves_to_its_own_distances() {
        let mut bench = Bench::new();
        for (kind, short) in [(kind::ROLL_UP, 400.0), (kind::ROLL_CLOSER, 300.0), (kind::ROLL_CLOSEST, 250.0)] {
            bench.run(kind, 30);
            assert!((apart(bench.mover.position, PLAYER) - short).abs() < 0.01, "move {kind}");
        }
        bench.run(kind::ROLL_HOME, 30);
        assert_eq!(bench.mover.position, HOME);
        assert_eq!(bench.aim, PLAYER);
        let highest = peak(&mut bench, kind::BOUND, 40);
        assert!((apart(bench.mover.position, PLAYER) - BOUND_STOP).abs() < 0.01);
        assert!((highest - BOUND_HEIGHT * 0.5).abs() < 1.0, "bounded {highest} high");
        let highest = peak(&mut bench, kind::ROLL_HOP_HOME, 40);
        let at = bench.mover.position;
        assert!(NEAR_HOME.iter().any(|place| (HOME + *place).distance(at) < 0.01), "ended at {at}");
        assert!((highest - HOP_HEIGHT * 0.5).abs() < 1.0, "hopped {highest} high");
    }

    #[test]
    fn the_spider_leaps_home_backwards_and_hops_on_the_spot_in_its_own_time() {
        let mut bench = Bench::new();
        let from = Vec3::new(450.0, 0.0, -900.0);
        bench.mover.position = from;
        let highest = peak(&mut bench, kind::LEAP_HOME, 40);
        assert!(bench.mover.position.distance(HOME) < 0.01);
        assert!((highest - LEAP_HOME_HEIGHT * 0.5).abs() < 1.0, "leapt {highest} high");
        // It looks back at where it leapt from.
        assert!(bench.facing().distance((from - HOME).normalize()) < 1e-3, "facing {}", bench.facing());

        // The hop takes its two seconds whatever the script says.
        bench.mover.start(0, kind::HOP_ON_SPOT, 30);
        let (mut steps, mut highest) = (0, 0.0f32);
        while !bench.mover.ended() {
            bench.step();
            steps += 1;
            highest = highest.max(bench.mover.position.y);
        }
        assert_eq!(steps, 60);
        assert!((highest - HOP_RISE).abs() < 0.01, "hopped {highest} high");
        assert!(bench.mover.position.distance(HOME) < 0.01);
        // Its turn has no bearing to go by, and ends on the far side of the stage.
        assert!(bench.facing().distance(Vec3::Z) < 1e-3, "facing {}", bench.facing());
    }

    /// The point `far` from the player on the line from her to the bench's home.
    fn towards_home(far: f32) -> Vec3 {
        let line = Vec3::new(HOME.x - PLAYER.x, 0.0, HOME.z - PLAYER.z);
        HOME - line.normalize() * (line.length() - far)
    }

    #[test]
    fn the_wheel_closes_in_and_comes_home_round_an_arc() {
        let mut bench = Bench::new();
        // The turn is over at once and goes on for its twelve frames.
        assert_eq!(bench.run(kind::TURN_TO_PLAYER, 30), 1);
        assert_eq!(bench.aim, PLAYER);
        for _ in 0..12 {
            bench.step();
        }
        let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - HOME).normalize();
        assert!(bench.facing().distance(expected) < 1e-3, "facing {}", bench.facing());

        bench.run(kind::CLOSE_IN, 30);
        assert!((apart(bench.mover.position, PLAYER) - CLOSE_IN_STOP).abs() < 0.01);
        // The arcs end beside home, one on either side, and swing far out on
        // the way; it looks where it goes until near the end.
        for (kind, side) in [(kind::ARC_FIRST, 1.0), (kind::ARC_LAST, -1.0)] {
            let mut bench = Bench::new();
            bench.run(kind::CLOSE_IN, 30);
            bench.mover.start(0, kind, 100);
            let mut furthest = 0.0f32;
            for step in 0..50 {
                let before = bench.mover.position;
                bench.step();
                furthest = furthest.max((bench.mover.position.x - HOME.x) * side);
                if step == 20 {
                    // Two frames into a turn of two frames it has turned to it.
                    let going = (bench.mover.position - before).normalize();
                    assert!(bench.facing().distance(Vec3::new(going.x, 0.0, going.z).normalize()) < 0.2, "arc {kind} facing {}", bench.facing());
                }
            }
            assert!(bench.mover.ended());
            assert_eq!(bench.mover.position, HOME + Vec3::new(side * ARC_ASIDE, 0.0, ARC_BEHIND), "arc {kind}");
            assert!(furthest > 1000.0, "arc {kind} swung {furthest} out");
            // And straight home from there.
            bench.run(kind::STRAIGHT_HOME_FIRST, 10);
            assert_eq!(bench.mover.position, HOME);
        }
        // The swerve goes to the same distance by way of one side.
        let mut bench = Bench::new();
        bench.mover.start(0, kind::SWERVE_IN, 40);
        let mut furthest = 0.0f32;
        while !bench.mover.ended() {
            bench.step();
            furthest = furthest.max(bench.mover.position.x);
        }
        assert!((apart(bench.mover.position, PLAYER) - CLOSE_IN_STOP).abs() < 0.01);
        assert!(furthest > HOME.x + 300.0, "swerved out to {furthest}");
    }

    #[test]
    fn the_wheel_leaves_the_stage_and_charges_in_from_the_side() {
        for (off, lurk, back, side) in [
            (kind::OFFSTAGE_FIRST, kind::LURK_FIRST, kind::RETURN_FIRST, OFFSTAGE),
            (kind::OFFSTAGE_LAST, kind::LURK_LAST, kind::RETURN_LAST, -OFFSTAGE),
        ] {
            let mut bench = Bench::new();
            assert_eq!(bench.run(off, 30), 15);
            assert_eq!(bench.mover.position, Vec3::new(side, HOME.y, HOME.z));
            // Out there it is put level with a point near her at once, and waits.
            for (kind, far) in [(lurk, LURK_STOPS[0]), (lurk + 2, LURK_STOPS[1])] {
                bench.mover.start(0, kind, 8);
                bench.step();
                let level = Vec3::new(side, HOME.y, towards_home(far).z);
                assert!(bench.mover.position.distance(level) < 0.01, "move {kind} put it at {}", bench.mover.position);
                assert!(!bench.mover.ended());
                bench.run(kind, 8);
                assert!(bench.mover.position.distance(level) < 0.01);
            }
            // It comes back in from the side, wherever it was.
            bench.mover.position = Vec3::ZERO;
            bench.mover.start(0, back, 45);
            bench.step();
            assert!((bench.mover.position.x - side).abs() < 500.0, "came in from {}", bench.mover.position);
            while !bench.mover.ended() {
                bench.step();
            }
            assert_eq!(bench.mover.position, HOME);
        }
        // The charges end to either side of the point, or on one nearer her.
        for (kind, far, aside) in [
            (kind::CHARGE_FIRST, CHARGE_STOP, CHARGE_ASIDE),
            (kind::CHARGE_LAST, CHARGE_STOP, -CHARGE_ASIDE),
            (kind::CHARGE_AT, CHARGE_AT_STOP, 0.0),
            (kind::SIDLE_FIRST, CHARGE_STOP, CHARGE_ASIDE),
            (kind::SIDLE_LAST, CHARGE_STOP, -CHARGE_ASIDE),
        ] {
            let mut bench = Bench::new();
            bench.mover.position = Vec3::new(OFFSTAGE, 0.0, -900.0);
            bench.run(kind, 24);
            let expected = towards_home(far) + Vec3::X * aside;
            assert!(bench.mover.position.distance(expected) < 0.01, "move {kind} ended at {}", bench.mover.position);
            if kind >= kind::SIDLE_FIRST {
                assert!(bench.facing().distance(Vec3::NEG_Z) < 1e-5, "move {kind} facing {}", bench.facing());
            }
        }
    }

    #[test]
    fn john_doe_stays_put_and_is_then_somewhere_else() {
        for (kind, place) in [
            (900, HOME + VANISH_PLACES[0][0]),
            (901, HOME + VANISH_PLACES[0][1]),
            (902, HOME + VANISH_PLACES[0][2]),
            (910, HOME + VANISH_PLACES[1][0]),
            (911, HOME + VANISH_PLACES[1][1]),
            (912, HOME + VANISH_PLACES[1][2]),
            (kind::VANISH_HOME, HOME),
        ] {
            let mut bench = Bench::new();
            let from = Vec3::new(100.0, 0.0, -300.0);
            bench.mover.position = from;
            bench.mover.start(0, kind, 40);
            for _ in 0..19 {
                bench.step();
                assert_eq!(bench.mover.position, from, "move {kind}");
            }
            bench.step();
            assert!(bench.mover.ended());
            assert_eq!(bench.mover.position, place, "move {kind}");
            // It is facing her as it appears.
            let expected = (Vec3::new(PLAYER.x, 0.0, PLAYER.z) - place).normalize();
            assert!(bench.facing().distance(expected) < 1e-3, "move {kind} facing {}", bench.facing());
            assert_eq!(bench.aim, PLAYER);
        }
        let mut bench = Bench::new();
        bench.run(kind::VANISH_TO_PLAYER, 40);
        assert!((apart(bench.mover.position, PLAYER + ANKLE) - VANISH_STOP).abs() < 0.01);
    }

    #[test]
    fn a_drift_goes_between_the_places_closer_in() {
        let mut bench = Bench::at(PERCH);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..40 {
            assert_eq!(bench.run(kind::DRIFT, 60), 30);
            let at = bench.mover.position;
            let place = DRIFT_PLACES.iter().position(|place| (PERCH + *place).distance(at) < 0.01);
            seen.insert(place.unwrap_or_else(|| panic!("ended at {at}")));
        }
        assert!(seen.len() > 6, "only went to {seen:?}");
    }

    #[test]
    fn the_cubes_float_between_places_round_home() {
        let mut bench = Bench::at(PERCH);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..40 {
            assert_eq!(bench.run(kind::FLOAT, 60), 30);
            let at = bench.mover.position;
            let place = FLOAT_PLACES.iter().position(|place| (PERCH + *place).distance(at) < 0.01);
            seen.insert(place.unwrap_or_else(|| panic!("ended at {at}")));
        }
        assert!(seen.len() > 6, "only went to {seen:?}");
        assert_eq!(bench.aim, PLAYER);
    }

    #[test]
    fn stopping_a_move_ends_it_where_it_is() {
        let mut bench = Bench::new();
        bench.mover.start(0, kind::APPROACH, 20);
        bench.step();
        let at = bench.mover.position;
        bench.mover.stop();
        assert!(bench.mover.ended());
        assert!(!bench.step());
        assert_eq!(bench.mover.position, at);
    }

    #[test]
    fn a_kind_that_is_not_ported_ends_at_once() {
        // (One the original has code for that no script starts.)
        let mut bench = Bench::new();
        assert_eq!(bench.run(10200, 30), 1);
        assert_eq!(bench.mover.position, HOME);
    }
}
