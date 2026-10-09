//! How the screen goes from the field to a battle: the field's part of it (the
//! object at 0x172C30, which the executable calls "BtEncountFade" too: started at
//! 0x11ACDC, its frame 0x11B3AC and its draw 0x11B588, on the layer of the
//! screen's fade), the battle's object of that name (0x906E8) and `BtEnter`
//! (0x93D9C), which waits for them. See `docs/BATTLE_NOTES.md`.
//!
//! When a battle is asked for, the field has a picture taken of the screen with
//! all it has on it (0xD876C) and, once that is done, starts the encounter with
//! its sound and begins to load the battle (0xD8890). A table of two records
//! (0x172C10) says how it goes, by whether the enemy group is one she strikes her
//! pose over:
//!
//! - most groups: the picture stands (the screen's fade with kind 0x804) until
//!   the battle is ready, and then goes over thirty frames of the interface (kind
//!   0x803), with the battle already drawn under it, its display and all;
//! - those groups: the picture stands for 134 frames while an effect of the
//!   field's plays over it (of the eighth bundle of the field's own pack,
//!   0x11AD3C), the screen goes white, and when the battle is ready the white
//!   comes off over thirty frames.
//!
//! The effect is the field's to make and to give its frames (0x11B2A4: one for
//! each frame the picture stands), and is drawn over the picture by a camera of
//! its own that looks straight on from where the world begins (0x11B164): all
//! but three of its emitters are fixed to the screen, and those three to that
//! camera. Here a camera of the battle's draws it over nothing, and the canvas
//! lays what that drew over the picture.
//!
//! `BtEnter` lets the fight begin only when the encounter is over: when the
//! picture has gone, or as soon as the white has begun to come off.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use brs_engine::effect::Camera as View;
use brs_engine::effects::{EffectCamera, EffectFile, EffectSystems, Playing};
use brs_engine::step::SIXTIETHS_PER_STEP;
use brs_engine::texture::camera_of_values;
use brs_engine::AssetStores;

use super::Battle;
use crate::data::GameData;
use crate::field::scene::{fade_kind, Fade};

/// The effect a battle of note begins with, and the file of the field's own pack
/// it is of (0x11AD8C: the second of the bundle, for the second record).
const EFFECT: u32 = 1;
const EFFECT_FILE: &str = "SYSTEM.VOL/fld_resident.bin/03.bin/07.bin/03.bin/01.efc";
/// The layer what the effect has before its camera is drawn in, and when that
/// camera draws: after the battle's own, whose picture it clears away, and before
/// the one that draws what is fixed to the screen.
const EFFECT_LAYER: usize = 4;
const EFFECT_ORDER: isize = 2;
/// How wide its camera sees from top to bottom, and from how near to how far
/// (0x11B168, 0x11B1D0).
const EFFECT_VIEW: f32 = std::f32::consts::FRAC_PI_4;
const EFFECT_NEAR: f32 = 1.0;
const EFFECT_FAR: f32 = 20000.0;

/// The camera that draws the effect of a battle of note, while there is one.
#[derive(Component)]
pub struct PictureCamera;

/// On that effect.
#[derive(Component)]
pub struct OfEncounter;

/// Makes the camera the effect is drawn by, for a battle.
pub fn spawn(commands: &mut Commands) {
    commands.spawn((
        Battle,
        PictureCamera,
        Camera3d::default(),
        Camera { order: EFFECT_ORDER, clear_color: ClearColorConfig::Custom(Color::NONE), is_active: false, ..default() },
        camera_of_values(),
        Projection::Perspective(PerspectiveProjection { fov: EFFECT_VIEW, near: EFFECT_NEAR, far: EFFECT_FAR, ..default() }),
        Transform::IDENTITY,
        RenderLayers::layer(EFFECT_LAYER),
    ));
}

/// Has the effect of a battle of note play while the field's picture stands
/// (0x11B2A4): it is made as the picture begins to, has a frame for each frame
/// after, and is no more when the screen goes white (0x11AC70).
pub(super) fn play(
    mut commands: Commands,
    encounter: Res<Encounter>,
    data: Res<GameData>,
    mut stores: AssetStores,
    mut seen: ResMut<EffectCamera>,
    mut camera: Single<&mut Camera, With<PictureCamera>>,
    mut others: Query<(Entity, &mut Camera), (With<Battle>, Without<PictureCamera>)>,
    mut put_out: Local<Vec<Entity>>,
    mut playing: Query<(Entity, &mut Playing), With<OfEncounter>>,
) {
    if !encounter.effect_plays() {
        for (entity, _) in &playing {
            commands.entity(entity).despawn();
        }
        if camera.is_active {
            camera.is_active = false;
        }
        for entity in put_out.drain(..) {
            if let Ok((_, mut other)) = others.get_mut(entity) {
                other.is_active = true;
            }
        }
        return;
    }
    // Nothing else of the battle is drawn while it plays: all of that is under the
    // field's picture, and the effect is to be drawn over nothing, which it is
    // only if its camera is the first to draw (a later camera's clearing does not
    // count).
    for (entity, mut other) in &mut others {
        if other.is_active {
            other.is_active = false;
            put_out.push(entity);
        }
    }
    camera.is_active = true;
    // It is seen from where the world begins, whatever the battle's camera does.
    seen.0 = View::of(Mat4::IDENTITY);
    if let Some((_, mut playing)) = playing.iter_mut().next() {
        playing.give(SIXTIETHS_PER_STEP);
        return;
    }
    let made = data.read(EFFECT_FILE).and_then(|bytes| EffectFile::load(&bytes, &mut stores));
    match made.map(|file| file.play(EFFECT)) {
        Ok(Some(mut effect)) => {
            effect.apart = Some(RenderLayers::layer(EFFECT_LAYER));
            commands.spawn((Battle, OfEncounter, effect));
        }
        Ok(None) => warn!("{EFFECT_FILE} has no effect {EFFECT}"),
        Err(error) => warn!("{EFFECT_FILE}: {error:#}"),
    }
}

/// The system that plays the effect, to run once the frame's effects have been
/// seen to: theirs is the battle's camera, and this one's its own.
pub(super) fn systems() -> impl bevy::ecs::schedule::IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    play.before(EffectSystems::Step)
}

/// The frames of the game the field's picture stands before a battle of note
/// (the second record's first word), and those the screen takes to come back
/// (0x11B4EC: thirty of the interface's).
const STANDS_BEFORE_A_BATTLE_OF_NOTE: u32 = 134;
const COMING_FRAMES: i32 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// The field's picture stands and then goes.
    Picture,
    /// The field's picture stands, the screen goes white, and the white comes off.
    White,
    /// There is no field to come from: a battle run alone takes the frames of
    /// the first way with nothing over it.
    Alone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// The field's picture stands: so many frames of it have gone by (the
    /// field's object in its state 1).
    Standing(u32),
    /// The screen is covered, and the battle's readiness is waited for (2).
    Covered,
    /// The screen is coming back (3): for so many frames.
    Coming(u32),
    Over,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Encounter {
    way: Way,
    state: State,
    /// The screen's fade as the encounter has it.
    fade: Fade,
    /// The battle does not say it is ready of itself: `BtEnter` leaves that to
    /// the script of whoever is fought (0x93DE8, for the groups that are a
    /// boss's), which says so when its own scene is set (0xBE760).
    held: bool,
}

impl Default for Encounter {
    fn default() -> Encounter {
        Encounter::new(Way::Alone)
    }
}

impl Encounter {
    /// The encounter as the field starts it (0x11ACDC).
    pub fn new(way: Way) -> Encounter {
        let mut fade = Fade::CLEAR;
        // The picture was taken to be shown as it is (0xD8794).
        if way != Way::Alone {
            fade.hold_picture();
        }
        Encounter { way, state: State::Standing(0), fade, held: false }
    }

    /// Has it wait, once the screen is covered, until it is let go.
    pub fn hold(&mut self) {
        self.held = true;
    }

    /// The battle is ready (0x90800).
    pub fn let_go(&mut self) {
        self.held = false;
    }

    /// The encounter with a group, by whether she strikes her pose over it.
    pub fn with(victory_pose: bool) -> Encounter {
        Encounter::new(if victory_pose { Way::White } else { Way::Picture })
    }

    /// Has a battle that is run alone begin as one of note does, if it is against
    /// such a group: its effect over nothing, and then the white.
    pub fn alone_against(&mut self, victory_pose: bool) {
        if self.way == Way::Alone && victory_pose {
            *self = Encounter::new(Way::White);
        }
    }

    /// Whether the fight may begin (0x907F4).
    pub fn over(&self) -> bool {
        self.state == State::Over
    }

    /// Whether the effect of a battle of note is playing over the field's
    /// picture: while that stands before such a battle, from its first frame.
    pub fn effect_plays(&self) -> bool {
        self.way == Way::White && matches!(self.state, State::Standing(frames) if frames > 0)
    }

    /// The colour the field's picture is drawn by, and how much of the screen is
    /// white.
    pub fn shown(&self) -> (Option<[u8; 4]>, f32) {
        (self.fade.picture(), if self.fade.white { self.fade.level } else { 0.0 })
    }

    /// A frame of it (0x11B3AC, with the battle's 0x906E8). The battle is ready
    /// as soon as it is asked: there is nothing here that takes time to load.
    pub fn step(&mut self) {
        self.fade.advance();
        match self.state {
            State::Standing(frames) => {
                let stands = if self.way == Way::White { STANDS_BEFORE_A_BATTLE_OF_NOTE } else { 0 };
                if frames < stands {
                    self.state = State::Standing(frames + 1);
                    return;
                }
                // The white is all there at once (0x2BF5C).
                if self.way == Way::White {
                    self.fade.begin(fade_kind::OUT_WHITE, 1);
                    self.fade.advance();
                }
                self.state = State::Covered;
            }
            State::Covered => {
                if self.held {
                    return;
                }
                match self.way {
                    Way::Picture => self.fade.dissolve(COMING_FRAMES),
                    Way::White => self.fade.begin(fade_kind::IN_WHITE, COMING_FRAMES),
                    Way::Alone => {}
                }
                self.state = State::Coming(0);
            }
            // Only the picture's going is waited for (0x11B548): the white comes
            // off as the fight begins.
            State::Coming(frames) => {
                let gone = frames + 1 >= COMING_FRAMES as u32;
                self.state = if self.way == Way::White || gone { State::Over } else { State::Coming(frames + 1) };
            }
            State::Over => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frames an encounter takes to be over.
    fn frames(mut encounter: Encounter) -> u32 {
        let mut frames = 0;
        while !encounter.over() {
            encounter.step();
            frames += 1;
            assert!(frames < 1000);
        }
        frames
    }

    #[test]
    fn the_fields_picture_stands_and_then_goes_over_the_battle() {
        let mut encounter = Encounter::with(false);
        assert_eq!(encounter.shown(), (Some([255; 4]), 0.0));
        // A frame for the picture to stand, one for the battle to be ready, and
        // then it goes over fifteen.
        encounter.step();
        encounter.step();
        assert_eq!(encounter.shown(), (Some([255; 4]), 0.0));
        for _ in 0..8 {
            encounter.step();
        }
        assert_eq!(encounter.shown(), (Some([255, 255, 255, 119]), 0.0));
        assert!(!encounter.over());
        assert_eq!(frames(Encounter::with(false)), 17);
        // Gone, nothing is over the battle.
        let mut gone = Encounter::with(false);
        for _ in 0..17 {
            gone.step();
        }
        assert_eq!(gone.shown(), (None, 0.0));
    }

    #[test]
    fn an_encounter_that_is_held_waits_covered_until_it_is_let_go() {
        let mut encounter = Encounter::with(false);
        encounter.hold();
        for _ in 0..100 {
            encounter.step();
            assert!(!encounter.over());
            assert_eq!(encounter.shown(), (Some([255; 4]), 0.0));
        }
        // Let go, the picture goes over its fifteen frames as it does otherwise.
        encounter.let_go();
        assert_eq!(frames(encounter), 16);
    }

    #[test]
    fn a_battle_of_note_comes_out_of_white() {
        let mut encounter = Encounter::with(true);
        // The picture stands for its effect's time.
        assert!(!encounter.effect_plays());
        for _ in 0..134 {
            encounter.step();
            assert_eq!(encounter.shown(), (Some([255; 4]), 0.0));
            assert!(encounter.effect_plays());
        }
        // Then all is white, and the fight may begin as the white starts to go.
        encounter.step();
        assert_eq!(encounter.shown(), (None, 1.0));
        assert!(!encounter.effect_plays());
        // No other battle has it.
        let mut plain = Encounter::with(false);
        plain.step();
        assert!(!plain.effect_plays());
        // A battle run alone against such a group begins the same way, and one
        // that came from the field is as the field had it.
        let mut alone = Encounter::default();
        alone.alone_against(false);
        assert_eq!(alone, Encounter::default());
        alone.alone_against(true);
        assert_eq!(alone, Encounter::with(true));
        let mut from_the_field = Encounter::with(false);
        from_the_field.alone_against(true);
        assert_eq!(from_the_field, Encounter::with(false));
        encounter.step();
        assert!(!encounter.over());
        encounter.step();
        assert!(encounter.over());
        let (_, white) = encounter.shown();
        assert!(white < 1.0 && white > 0.8);
        for _ in 0..15 {
            encounter.step();
        }
        assert_eq!(encounter.shown(), (None, 0.0));
    }

    #[test]
    fn a_battle_run_alone_takes_as_long_with_nothing_over_it() {
        let mut encounter = Encounter::default();
        assert_eq!(frames(encounter), frames(Encounter::with(false)));
        for _ in 0..10 {
            encounter.step();
            assert_eq!(encounter.shown(), (None, 0.0));
        }
    }
}
