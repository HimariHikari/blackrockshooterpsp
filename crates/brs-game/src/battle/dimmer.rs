//! The sheet a battle can lay over what is behind her (`BtBgColor`, 0x85490): a
//! black one over the whole picture, as strong as whoever asks for it has it. Her
//! skills ask for it (0x8073C), over a third of a second in and again out.
//!
//! The original draws a battle in lists, one after the other: the sky in the
//! first, the ground in the third, and she, the enemies and all they fire in the
//! fourth. The sheet is the first thing of the fourth (0x855A4), so the map is
//! under it and nothing that fights is. The port draws a battle in the same
//! steps, with a camera for each: the map, the sheet over it, and then everything
//! else, which what is nearer of the map still hides.
//!
//! A sheet that is all black is drawn otherwise (0x855A4): it is the first thing of
//! the first list, and the map is not drawn at all (0x97084). So an effect that is
//! of the first list is under the map and the sheet while there is any of the map
//! to see, and is all there is behind her when there is none: one skill has such
//! a one for the sky of its scene. Here a camera before the map's clears the
//! picture, to black then, and draws what such an effect has on the screen
//! itself; what it has in the world the map's camera draws, with the map or
//! without it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bevy::sprite_render::{AlphaMode2d, ColorMaterial};
use brs_engine::effects;
use brs_engine::texture::camera_of_values;

use super::{Battle, BattleCamera};

/// The layer the map is drawn in, by a camera of its own.
pub const STAGE_LAYER: usize = 1;
/// The layer the sheet is drawn in, by another.
const SHEET_LAYER: usize = 2;
/// The layer what an effect of the scene's first draw list has in the world is
/// drawn in, by the map's camera.
pub const BACKDROP_LAYER: usize = 3;
/// When those cameras draw: before the battle's own, which is the first of the
/// scene's otherwise. The first of them all is the one that draws what is on the
/// screen itself of the scene's first draw list.
const SKY_ORDER: isize = -3;
const STAGE_ORDER: isize = -2;
const SHEET_ORDER: isize = -1;
/// The strength at which the sheet is all there is behind her (0x855A4).
const ALL_BLACK: i32 = 255;
/// The sheet's sides, in the pixels of whatever is drawn to: more than any has.
const SHEET_SIZE: f32 = 100_000.0;

/// How strong somebody has the sheet, and where that is going (0x856BC).
#[derive(Clone, Copy, Default, Debug)]
struct Way {
    /// Of 255.
    level: i32,
    from: i32,
    to: i32,
    /// The sixtieths its way takes, and how many of them have gone by.
    time: f32,
    run: f32,
}

impl Way {
    /// Has it go to a strength over so many sixtieths (0x85814): at once if it is
    /// there already, or if no time is given it.
    fn go(&mut self, to: i32, time: f32) {
        (self.from, self.to, self.run) = (self.level, to, 0.0);
        self.time = if to == self.level { 0.0 } else { time };
        if self.time <= 0.0 {
            self.level = to;
        }
    }

    /// A frame of it (0x8570C).
    fn step(&mut self, dt: f32) {
        if self.run < self.time {
            self.run = (self.run + dt).min(self.time);
            let share = (self.run / self.time).clamp(0.0, 1.0);
            self.level = (self.from as f32 + (self.to - self.from) as f32 * share) as i32;
        }
    }
}

/// The sheet: as strong as the strongest of those who ask for it has it (0x85930).
#[derive(Resource, Default, Debug)]
pub struct Dimmer {
    /// As she has it (her + 0x714).
    hers: Way,
    /// As each enemy that has asked has it (its + 0x80C), by the enemy's number.
    theirs: Vec<(usize, Way)>,
    /// The strength the sheet was last drawn at.
    shown: i32,
}

impl Dimmer {
    /// Has her sheet go to a strength over so many sixtieths (0x8073C).
    pub fn go(&mut self, to: i32, time: f32) {
        self.hers.go(to, time);
    }

    /// A frame of hers, which her own frame begins with (0x99FF8).
    pub fn step(&mut self, dt: f32) {
        self.hers.step(dt);
    }

    /// Has an enemy's sheet go to a strength over so many sixtieths (0xBE150).
    pub fn go_for(&mut self, enemy: usize, to: i32, time: f32) {
        match self.theirs.iter_mut().find(|(of, _)| *of == enemy) {
            Some((_, way)) => way.go(to, time),
            None => {
                let mut way = Way::default();
                way.go(to, time);
                self.theirs.push((enemy, way));
            }
        }
    }

    /// A frame of the enemies', which each one's own frame has (0x920FC). One
    /// that has come to nothing is no longer among them (0x85780).
    pub fn step_theirs(&mut self, dt: f32) {
        for (_, way) in &mut self.theirs {
            way.step(dt);
        }
        self.theirs.retain(|(_, way)| way.level != 0 || way.run < way.time);
    }

    /// The strength of the strongest.
    fn level(&self) -> i32 {
        self.theirs.iter().map(|(_, way)| way.level).fold(self.hers.level, i32::max)
    }

    /// How much of the sheet is seen, of one (0x85944: its alpha is the strength's
    /// low byte).
    fn strength(&self) -> f32 {
        (self.level() & 0xff) as f32 / 255.0
    }
}

/// The camera that draws the map: it sees what the battle's camera sees.
#[derive(Component)]
pub struct StageCamera;

/// The camera that clears the picture and draws what the scene's first draw list
/// has on the screen itself.
#[derive(Component)]
pub struct SkyCamera;

/// The camera that draws the sheet, which draws only while there is some of it.
#[derive(Component)]
pub struct SheetCamera;

#[derive(Component)]
pub struct Sheet;

/// Makes what draws the map and the sheet, for the battle's camera `main`, which
/// sees through `lens` and `fog`.
pub fn spawn(commands: &mut Commands, main: Entity, lens: Projection, fog: DistanceFog, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>) {
    commands.spawn((
        Battle,
        SkyCamera,
        Camera3d::default(),
        Camera { order: SKY_ORDER, ..default() },
        effects::screen_camera(),
        RenderLayers::layer(effects::SCREEN_BEHIND_LAYER),
    ));
    commands.spawn((
        Battle,
        StageCamera,
        Camera3d::default(),
        Camera { order: STAGE_ORDER, clear_color: ClearColorConfig::None, ..default() },
        camera_of_values(),
        lens,
        fog,
        RenderLayers::from_layers(&[STAGE_LAYER, BACKDROP_LAYER]),
        Transform::IDENTITY,
        ChildOf(main),
    ));
    commands.spawn((
        Battle,
        SheetCamera,
        Camera2d,
        Camera { order: SHEET_ORDER, clear_color: ClearColorConfig::None, is_active: false, ..default() },
        camera_of_values(),
        RenderLayers::layer(SHEET_LAYER),
    ));
    commands.spawn((
        Battle,
        Sheet,
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial2d(materials.add(ColorMaterial { color: Color::NONE, alpha_mode: AlphaMode2d::Blend, ..default() })),
        Transform::from_scale(Vec3::new(SHEET_SIZE, SHEET_SIZE, 1.0)),
        NoFrustumCulling,
        RenderLayers::layer(SHEET_LAYER),
    ));
}

/// The map's camera sees through the lens the battle's own does: a staged moment
/// has its own, and her skills change how near and how far it sees (0xA7678).
/// It is moved as the battle's is for the scope of a snipe (0x8BF28 sets the
/// middle of every one of the scene's cameras).
pub(super) fn follow(
    main: Single<(&Projection, &Camera), (With<BattleCamera>, Without<StageCamera>)>,
    stage: Single<(&mut Projection, &mut Camera), With<StageCamera>>,
) {
    let (main, main_camera) = *main;
    let (mut stage, mut stage_camera) = stage.into_inner();
    if stage_camera.sub_camera_view != main_camera.sub_camera_view {
        stage_camera.sub_camera_view = main_camera.sub_camera_view;
    }
    let Projection::Perspective(main) = main else { return };
    let seen = (main.fov, main.near, main.far);
    if matches!(&*stage, Projection::Perspective(stage) if (stage.fov, stage.near, stage.far) == seen) {
        return;
    }
    if let Projection::Perspective(stage) = &mut *stage {
        (stage.fov, stage.near, stage.far) = seen;
    }
}

/// Draws the sheet as strong as it is now, when that has changed.
pub fn shade(
    mut dimmer: ResMut<Dimmer>,
    sheet: Single<&MeshMaterial2d<ColorMaterial>, With<Sheet>>,
    mut camera: Single<&mut Camera, (With<SheetCamera>, Without<SkyCamera>)>,
    mut sky: Single<&mut Camera, (With<SkyCamera>, Without<SheetCamera>)>,
    mut stage: Single<&mut RenderLayers, With<StageCamera>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let level = dimmer.level();
    if level == dimmer.shown {
        return;
    }
    dimmer.shown = level;
    let strength = dimmer.strength();
    if let Some(mut material) = materials.get_mut(&sheet.0) {
        material.color = Color::linear_rgba(0.0, 0.0, 0.0, strength);
    }
    // All black, it is under whatever else is of the scene's first draw list,
    // and there is no map: the picture is black before anything is drawn.
    let black = level == ALL_BLACK;
    camera.is_active = strength > 0.0 && !black;
    sky.clear_color = if black { ClearColorConfig::Custom(Color::BLACK) } else { ClearColorConfig::Default };
    **stage = if black { RenderLayers::layer(BACKDROP_LAYER) } else { RenderLayers::from_layers(&[STAGE_LAYER, BACKDROP_LAYER]) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_comes_over_a_third_of_a_second_and_goes_the_same_way() {
        let mut dimmer = Dimmer::default();
        assert_eq!((dimmer.hers.level, dimmer.strength()), (0, 0.0));
        // A skill has it come to 196 of 255 over twenty sixtieths: ten frames of
        // the game, each a tenth of the way, in whole numbers.
        dimmer.go(196, 20.0);
        assert_eq!(dimmer.hers.level, 0);
        let levels: Vec<i32> = (0..12)
            .map(|_| {
                dimmer.step(2.0);
                dimmer.hers.level
            })
            .collect();
        assert_eq!(levels, [19, 39, 58, 78, 98, 117, 137, 156, 176, 196, 196, 196]);
        assert!((dimmer.strength() - 196.0 / 255.0).abs() < 1e-6);
        // And back.
        dimmer.go(0, 20.0);
        dimmer.step(2.0);
        assert_eq!(dimmer.hers.level, 176);
        for _ in 0..9 {
            dimmer.step(2.0);
        }
        assert_eq!((dimmer.hers.level, dimmer.strength()), (0, 0.0));
        // Asked to go where it is, or in no time, it is there at once.
        dimmer.go(0, 20.0);
        dimmer.step(2.0);
        assert_eq!(dimmer.hers.level, 0);
        dimmer.go(128, 0.0);
        assert_eq!(dimmer.hers.level, 128);
    }

    #[test]
    fn the_sheet_is_as_strong_as_the_strongest_who_asks_has_it() {
        let mut dimmer = Dimmer::default();
        // An enemy that takes the stage has it at 196 over twenty sixtieths.
        dimmer.go_for(3, 196, 20.0);
        for _ in 0..10 {
            dimmer.step_theirs(2.0);
        }
        assert_eq!(dimmer.level(), 196);
        // Hers, weaker, changes nothing; stronger, it is hers that is seen.
        dimmer.go(100, 0.0);
        assert_eq!(dimmer.level(), 196);
        dimmer.go(255, 0.0);
        assert_eq!(dimmer.level(), 255);
        dimmer.go(0, 0.0);
        // The enemy has it off at once, with no time given.
        dimmer.go_for(3, 0, 0.0);
        dimmer.step_theirs(2.0);
        assert_eq!((dimmer.level(), dimmer.theirs.len()), (0, 0));
    }
}
