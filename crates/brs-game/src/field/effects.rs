//! The effects the field makes of itself: what its characters come and go with.
//!
//! The field has objects of its own for them (0xE7808 to 0xE82F8; thirty-two, kept
//! at the field's + 0x26D0, 0x117A20). One is made of an effect of the field's own
//! pack (0xE7C78, 0xE78B4), put at a place with a way it is turned and sizes
//! (0xE7980), and has a frame of the field for each of the field's (0xE80A8, from
//! 0x1155CC: a frame of the effect for each, 0x127BA0 with the frame's time over
//! 0x800). It is done with when its effect has played out, or once it has gone:
//! told to go (0xE7904), it has its effect's own alpha come down to nothing over so
//! long and is then no more.
//!
//! The effects are in `SYSTEM.VOL/fld_resident.bin`, file 3, which is a bundle for
//! each of nine things (0x110104); each of those has its effect files in its own
//! file 3. The fifth has what a character comes and goes with, the sixth what
//! goes with her own actions, the seventh what a battle of note begins with.
//!
//! And the effects that are characters: what a map's script makes with `s53` of
//! one of the map's own effects, of one of those bundles, or of one an event has
//! brought (`scene::EffectOf`). The script puts such a character where it likes,
//! and its effect is there (0x102A2C); it has a frame for each of the
//! character's (0x102FD4, from 0xF5E50), and is drawn while the character is
//! not hidden (0x103074, from 0x1067F8).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use bevy::prelude::*;
use brs_engine::animation::Animator;
use brs_engine::effect::Camera as View;
use brs_engine::effects::{EffectCamera, EffectFile, EffectReach, EffectSystems, Lens, Playing};
use brs_engine::model::Skeleton;
use brs_engine::step::{Stepped, SIXTIETHS_PER_STEP};
use brs_engine::AssetStores;

use super::assets::CharacterAssets;
use super::camera::figure;
use super::scene::{flag, Character, EffectOf, ShakeAsked, Show, FULL_PACE};
use super::{Actor, FieldRoot, FieldState, FollowCamera};
use crate::data::GameData;
use crate::sound::Request;

/// The ring on the ground a character comes up through (0xF71CC), the smoke one
/// hops out of the ground in or goes in (0xF6E1C, 0xF957C), what rises where one
/// lands (0xF7740), and the burst of one that is destroyed (0xF98BC).
const RING: u32 = 510_001;
const SMOKE: u32 = 999_020;
const LANDING: u32 = 999_016;
const BURST: u32 = 509_001;
/// The aura about her as she rushes or flies where a trap takes her (0xF663C,
/// 0xF6A74).
const AURA: u32 = 999_018;

/// The files those are of.
const FILES: [&str; 5] = [
    "SYSTEM.VOL/fld_resident.bin/03.bin/05.bin/03.bin/00.efc",
    "SYSTEM.VOL/fld_resident.bin/03.bin/05.bin/03.bin/01.efc",
    "SYSTEM.VOL/fld_resident.bin/03.bin/05.bin/03.bin/04.efc",
    "SYSTEM.VOL/fld_resident.bin/03.bin/06.bin/03.bin/02.efc",
    "SYSTEM.VOL/fld_resident.bin/03.bin/06.bin/03.bin/04.efc",
];

/// How long the ring takes to go once what came up through it is up (0xF73A8), in
/// the field's units of time; and how long the aura takes (0xF67F8).
const RING_GOES_OVER: i32 = 0xf000;
const AURA_GOES_OVER: i32 = 0x5000;
/// How large what rises where she springs from is, of what rises where she
/// lands (0xF8558).
const SPRING_SIZE: f32 = 0.5;
/// The sound of a burst (0xF9998), of the bank that is always there.
const SOUND_BURST: u32 = 0x96;
/// How far the camera is shaken by a burst, up and down and from side to side, in
/// radians (0xF9960), and the frames over which the shake dies away (0xF997C).
const BURST_SHAKE: Vec2 = Vec2::new(0.022_689_3, 0.003_490_66);
const BURST_SHAKE_FRAMES: i32 = 18;

/// The effect files of the field's own, read the first time one is wanted.
#[derive(Resource, Default)]
pub struct FieldEffects {
    files: Option<Vec<Arc<EffectFile>>>,
    /// The numbers asked for that no file has, each told of once.
    missing: Vec<u32>,
    /// The files the characters that are effects are of, for as long as there is
    /// a character of each; one that could not be read is kept as that.
    of_characters: HashMap<String, Option<Arc<EffectFile>>>,
}

impl FieldEffects {
    /// The effect of a file that has one, which is what a character is made of
    /// (0x127A10): the file is read the first time it is wanted.
    fn of_file(&mut self, path: &str, data: &GameData, stores: &mut AssetStores) -> Option<Playing> {
        if !self.of_characters.contains_key(path) {
            let file = data.read(path).and_then(|bytes| EffectFile::load(&bytes, stores));
            self.of_characters.insert(path.to_owned(), file.inspect_err(|error| warn!("{path}: {error:#}")).ok());
        }
        let file = self.of_characters.get(path)?.as_ref()?;
        file.play(*file.numbers().first()?)
    }

    fn play(&mut self, id: u32, data: &GameData, stores: &mut AssetStores) -> Option<Playing> {
        let files = self.files.get_or_insert_with(|| {
            let mut files = Vec::new();
            for path in FILES {
                match data.read(path).and_then(|bytes| EffectFile::load(&bytes, stores)) {
                    Ok(file) => files.push(file),
                    Err(error) => warn!("{path}: {error:#}"),
                }
            }
            files
        });
        let found = files.iter().find_map(|file| file.play(id));
        if found.is_none() && !self.missing.contains(&id) {
            self.missing.push(id);
            warn!("the field has no effect {id}");
        }
        found
    }
}

/// On an effect of the field's own.
#[derive(Component)]
pub struct OfField {
    /// The character whose action made it, and its number.
    by: i32,
    id: u32,
    /// It is going: for how long it has been, and how long that takes, in the
    /// field's units of time (its + 0x58 and + 0x60).
    going: Option<(i32, i32)>,
    /// It goes where that character goes, turned as it is (its + 0x54, 0xE7E4C).
    with: bool,
}

/// The shake of the field's camera (the camera's + 0xC50 on; its frame is part of
/// the camera's own, 0xDD6A4). A point wanders about the middle of a round of one
/// across, a step of 1.4 at a time towards somewhere chosen by chance and then on
/// to the next (0xDBD88), and the camera is turned in its own frame by where the
/// point is, times how far it is shaken each way, times how strong the shake is.
#[derive(Resource)]
pub struct Shake {
    /// 0 it comes, 1 it is there, 2 it goes, 3 there is none (+ 0xC7C).
    state: u8,
    time: i32,
    frames: i32,
    /// How far, up and down and from side to side, in radians (+ 0xC60).
    amplitude: Vec2,
    at: Vec2,
    to: Vec2,
    random: u32,
    /// What the camera is turned by now.
    turn: Vec2,
}

/// How far the point goes in a frame (0xDC568), and how far each way the camera is
/// shaken until something says otherwise (0xDC57C).
const SHAKE_STEP: f32 = 1.4;
const SHAKE_AMPLITUDE: f32 = 0.013_962_634;

impl Default for Shake {
    fn default() -> Shake {
        Shake { state: 3, time: 0, frames: 30, amplitude: Vec2::splat(SHAKE_AMPLITUDE), at: Vec2::ZERO, to: Vec2::ZERO, random: 0x2545_f491, turn: Vec2::ZERO }
    }
}

impl Shake {
    /// How far it shakes the camera each way from here on (0xDC018).
    pub fn reach(&mut self, amplitude: Vec2) {
        self.amplitude = amplitude;
    }

    /// It comes over so many frames from the middle, or over none is there at
    /// once, and then stays until it is told to die away (0xDBF74).
    pub fn come(&mut self, frames: i32) {
        self.at = Vec2::ZERO;
        if frames == 0 {
            self.state = 1;
        } else {
            self.choose();
            (self.time, self.frames, self.state) = (0, frames, 0);
        }
    }

    /// What a script has asked of it (`s228`, `s229`).
    fn carry_out(&mut self, asked: ShakeAsked) {
        match asked {
            ShakeAsked::Come { reach, frames } => {
                self.reach(reach);
                self.come(frames);
            }
            ShakeAsked::DieAway(frames) => self.die_away(frames),
        }
    }

    /// It is there at its strongest and dies away over so many frames; over none,
    /// it is no more at once (0xDBFD0).
    pub fn die_away(&mut self, frames: i32) {
        if frames == 0 {
            self.state = 3;
        } else {
            (self.time, self.frames, self.state) = (0, frames, 2);
        }
    }

    /// A number of twelve bits by chance. (The original takes the field's own
    /// random numbers, 0x10E45C, which nothing here shares with it.)
    fn chance(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random & 0xfff) as f32 / 4096.0
    }

    /// Somewhere else for the point to go (0xDBD88): any way from the middle, or
    /// from elsewhere about the way back through it; four to ten tenths out.
    fn choose(&mut self) {
        use std::f32::consts::{PI, TAU};
        let way = if self.at.length() < 1e-5 {
            self.chance() * TAU - PI
        } else {
            let draw = self.chance() * 2.0 - 1.0;
            let squared = if draw <= 0.0 { -draw * draw } else { draw * draw };
            let off = if squared <= 0.0 { squared - 1.0 } else { 1.0 - squared };
            let as_seen = (self.at.normalize_or_zero() * self.amplitude).normalize_or_zero();
            as_seen.y.atan2(as_seen.x) + PI + off * 0.3 * PI
        };
        let far = self.chance() * 0.6 + 0.4;
        self.to = Vec2::new(way.cos(), way.sin()) * far;
    }

    /// A frame of it.
    pub fn step(&mut self) {
        let strength = match self.state {
            0 => {
                self.time += 1;
                if self.time >= self.frames {
                    self.state = 1;
                }
                self.time as f32 / self.frames as f32
            }
            1 => 1.0,
            2 => {
                self.time += 1;
                if self.time >= self.frames {
                    self.state = 3;
                }
                1.0 - self.time as f32 / self.frames as f32
            }
            _ => {
                self.turn = Vec2::ZERO;
                return;
            }
        };
        let left = self.to - self.at;
        if left.length() < SHAKE_STEP {
            self.at = self.to;
            self.choose();
        } else {
            self.at += left.normalize() * SHAKE_STEP;
        }
        self.turn = self.at * self.amplitude * strength;
    }

    /// What the camera is turned by in its own frame, about the axis across and
    /// the upright one (0x24C54 on the view).
    pub fn turned(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, -self.turn.y, -self.turn.x, 0.0)
    }
}

/// The effects go by the field's camera, and are seen as far and from as near
/// as its lenses see (0xD9D54).
fn see(camera: Single<&Transform, With<FollowCamera>>, state: Res<FieldState>, mut seen: ResMut<EffectCamera>, mut reach: ResMut<EffectReach>) {
    seen.0 = View::of(camera.to_matrix());
    let now = EffectReach { near: state.scene.camera.figure(figure::NEAR), far: state.scene.camera.figure(figure::FAR) };
    if *reach != now {
        *reach = now;
    }
}

/// The place the effect a character is has in the order of drawing: the field
/// draws its characters one after another, those a script has given a number
/// (setting 123) by their numbers, the lower first (0x116B38, as each is put
/// into the list the field draws from, 0x116BC0), and the rest the further from
/// the camera first. The port has those with a number drawn in their order
/// after those with none, which are drawn by how far they are.
fn order_of(character: &Character) -> u32 {
    if character.flags.has(flag::ORDERED) { character.drawn_order.max(0) as u32 + 1 } else { 0 }
}

/// Which lens the effect a character is is drawn through: the scene's, or the
/// one that sees from just before the camera for a character that asks for it
/// (its flag 0xA7: 0x103010, 0x103094).
fn lens_of(character: &Character) -> Lens {
    if character.flags.has(flag::CLOSE_LENS) { Lens::Close } else { Lens::Scene }
}

/// Makes what the characters' actions have asked to be seen, and has the camera
/// shake and the sound be heard where one bursts.
fn show(
    mut commands: Commands,
    data: Res<GameData>,
    mut stores: AssetStores,
    assets: Res<CharacterAssets>,
    mut state: ResMut<FieldState>,
    mut effects: ResMut<FieldEffects>,
    mut shake: ResMut<Shake>,
    camera: Single<&Transform, With<FollowCamera>>,
    root: Single<Entity, With<FieldRoot>>,
    mut playing: Query<&mut OfField>,
) {
    let scene = &mut state.scene;
    let mut heard = Vec::new();
    for (&by, character) in scene.characters.iter_mut() {
        for asked in std::mem::take(&mut character.shows) {
            let asset = assets.get(character.chr);
            // Turned as the character is, by its frame too (0xF8648).
            let lay = character.lay;
            let turned = |facing: f32| lay * Quat::from_rotation_y(facing);
            let (id, at) = match asked {
                // As wide as its script has it be, of a hundred (0xF71F8).
                Show::Ring { at, facing } => {
                    let wide = asset.map_or(1.0, |asset| asset.pop_radius / 100.0);
                    (RING, Transform { translation: at, rotation: turned(facing), scale: Vec3::new(wide, 1.0, wide) })
                }
                Show::RingGoes => {
                    for mut effect in &mut playing {
                        if effect.by == by && effect.id == RING && effect.going.is_none() {
                            effect.going = Some((0, RING_GOES_OVER));
                        }
                    }
                    continue;
                }
                Show::Smoke { at, facing } => (SMOKE, Transform::from_translation(at).with_rotation(turned(facing))),
                Show::Landing { at, facing } => (LANDING, Transform::from_translation(at).with_rotation(turned(facing))),
                Show::Spring { at, facing } => {
                    (LANDING, Transform { translation: at, rotation: turned(facing), scale: Vec3::splat(SPRING_SIZE) })
                }
                Show::Aura => (AURA, about(character)),
                Show::AuraGoes { let_go } => {
                    for mut effect in &mut playing {
                        if effect.by == by && effect.id == AURA && effect.going.is_none() {
                            effect.going = Some((0, AURA_GOES_OVER));
                            effect.with &= !let_go;
                        }
                    }
                    continue;
                }
                Show::Heard(sound) => {
                    heard.push(Request::common(sound));
                    continue;
                }
                // Half as high as it is wide by its script, and then as far again
                // towards the camera, so that it is before what bursts (0xF97D8).
                Show::Burst => {
                    let wide = asset.map_or(100.0, |asset| asset.destroy_radius);
                    let rotation = turned(character.facing());
                    let middle = character.position + rotation * Vec3::new(0.0, wide * 0.5, 0.0);
                    let at = middle + (camera.translation - middle).normalize_or_zero() * wide;
                    shake.reach(BURST_SHAKE);
                    shake.die_away(BURST_SHAKE_FRAMES);
                    heard.push(Request::common(SOUND_BURST));
                    (BURST, Transform::from_translation(at).with_rotation(rotation))
                }
            };
            let Some(mut effect) = effects.play(id, &data, &mut stores) else { continue };
            effect.place(at.to_matrix());
            // Through the scene's lens, as all the field's own are (0xE7ACC).
            effect.lens = Lens::Scene;
            commands.spawn((OfField { by, id, going: None, with: id == AURA }, effect, ChildOf(*root)));
        }
    }
    scene.sounds.append(&mut heard);
}

/// Where an effect that goes with a character is (0xE7E4C): where the character
/// is, turned as it is, and no larger or smaller for it.
fn about(character: &Character) -> Transform {
    Transform { scale: Vec3::ONE, ..character.frame() }
}

/// On the effect of a character that is one.
#[derive(Component)]
pub struct OfCharacter {
    id: i32,
    /// The file it is of, and which of the character's beginnings of it this is:
    /// with another of either, the character's effect is another.
    file: String,
    begun: u32,
}

/// The models of the field's characters, for where their nodes are.
type Models<'w, 's> = Query<'w, 's, (&'static Actor, &'static Stepped, Option<&'static Skeleton>, Option<&'static Animator>)>;

/// Where a character that is an effect has its effect (0x102A2C): where the
/// character is, turned and as large as it is; and that from the node it is
/// fastened to rather than from where the world begins, if it is fastened to a
/// character that has a model with such a node (0xFF7D4: the first node of the
/// number, which for 255 is the first that has none).
fn put(character: &Character, effect: &EffectOf, models: &Models) -> Mat4 {
    let own = character.frame().to_matrix();
    let node = effect.fastened.and_then(|(id, node)| {
        let node = u8::try_from(node).ok()?;
        let (_, stepped, skeleton, animator) = models.iter().find(|(actor, ..)| actor.id == id)?;
        let (skeleton, animator) = (skeleton?, animator?);
        Some(stepped.current.to_matrix() * animator.model_space(skeleton, skeleton.index_of(node)?).to_matrix())
    });
    node.map_or(own, |node| node * own)
}

/// The effects of the characters that are effects: one is made when its
/// character's is set playing, is where the character has it, has a frame for
/// each of the character's, and goes with the character. One of a character that
/// is hidden goes on and is not drawn.
fn of_characters(
    mut commands: Commands,
    data: Res<GameData>,
    mut stores: AssetStores,
    state: Res<FieldState>,
    mut effects: ResMut<FieldEffects>,
    root: Single<Entity, With<FieldRoot>>,
    models: Models,
    mut playing: Query<(Entity, &OfCharacter, &mut Playing)>,
) {
    let scene = &state.scene;
    let of = |id: i32| scene.characters.get(&id).and_then(|character| Some((character, character.effect.as_ref()?)));
    let mut there = BTreeSet::new();
    for (entity, made, mut playing) in &mut playing {
        let Some((character, effect)) = of(made.id).filter(|(_, effect)| effect.begun == made.begun && effect.file == made.file) else {
            commands.entity(entity).despawn();
            continue;
        };
        there.insert(made.id);
        playing.place(put(character, effect, &models));
        playing.unseen = !character.drawn();
        playing.lens = lens_of(character);
        playing.order = order_of(character);
        // As much of a frame as the character has of one (0x103428).
        playing.give(SIXTIETHS_PER_STEP * character.pace_fraction());
    }
    for (&id, character) in &scene.characters {
        let Some(effect) = character.effect.as_ref().filter(|_| !there.contains(&id)) else { continue };
        let Some(mut playing) = effects.of_file(&effect.file, &data, &mut stores) else { continue };
        playing.place(put(character, effect, &models));
        playing.unseen = !character.drawn();
        playing.lens = lens_of(character);
        playing.order = order_of(character);
        // The character is there when its effect is over, until a script takes
        // it away.
        playing.stays = true;
        commands.spawn((OfCharacter { id, file: effect.file.clone(), begun: effect.begun }, playing, ChildOf(*root)));
    }
    effects.of_characters.retain(|file, _| scene.characters.values().any(|character| character.effect.as_ref().is_some_and(|effect| effect.file == *file)));
}

/// A frame of the field for each of its effects, and of the camera's shake, which
/// is first told what the scripts have asked of it.
fn tick(mut commands: Commands, mut state: ResMut<FieldState>, mut shake: ResMut<Shake>, mut playing: Query<(Entity, &mut OfField, &mut Playing)>) {
    if !state.scene.shakes.is_empty() {
        for asked in state.scene.shakes.drain(..) {
            shake.carry_out(asked);
        }
    }
    shake.step();
    for (entity, mut of, mut playing) in &mut playing {
        // One that goes with a character is put where that is again (0xE80A8).
        if let Some(character) = state.scene.characters.get(&of.by).filter(|_| of.with) {
            playing.place(about(character).to_matrix());
        }
        playing.give(SIXTIETHS_PER_STEP);
        let Some((time, over)) = &mut of.going else { continue };
        *time += FULL_PACE;
        if *time < *over {
            playing.effect.colour = [1.0, 1.0, 1.0, 1.0 - *time as f32 / *over as f32];
        } else {
            commands.entity(entity).despawn();
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<FieldEffects>().init_resource::<Shake>();
}

/// The systems of a frame of the field that see to its effects, to run once its
/// characters have had theirs.
pub fn systems() -> impl bevy::ecs::schedule::IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    (see, show, of_characters, tick).chain().before(EffectSystems::Step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shake_that_dies_away_turns_the_camera_less_each_frame_and_is_then_no_more() {
        let mut shake = Shake::default();
        shake.step();
        assert_eq!(shake.turn, Vec2::ZERO);
        shake.reach(BURST_SHAKE);
        shake.die_away(BURST_SHAKE_FRAMES);
        let mut most = Vec2::ZERO;
        for frame in 1..=BURST_SHAKE_FRAMES {
            shake.step();
            // The point is never further than one from the middle, so the camera
            // is never turned by more than the shake reaches, less as it dies.
            let left = 1.0 - frame as f32 / BURST_SHAKE_FRAMES as f32;
            assert!(shake.turn.x.abs() <= BURST_SHAKE.x * left + 1e-6 && shake.turn.y.abs() <= BURST_SHAKE.y * left + 1e-6, "frame {frame}");
            most = most.max(shake.turn.abs());
        }
        assert!(most.x > 0.002 && most.y > 0.0003, "{most}");
        assert_eq!((shake.state, shake.turn), (3, Vec2::ZERO));
        shake.step();
        assert_eq!(shake.turn, Vec2::ZERO);
        // Over no frames it is no more at once.
        shake.die_away(5);
        shake.die_away(0);
        assert_eq!(shake.state, 3);
        // One a script asks for comes over its frames, as strong as the part of
        // them that has gone by, and is then there until it is told to go.
        shake.carry_out(ShakeAsked::Come { reach: Vec2::new(0.014, 0.014), frames: 8 });
        for frame in 1..=8 {
            shake.step();
            let part = frame as f32 / 8.0;
            assert!(shake.turn.length() <= 0.014 * part * std::f32::consts::SQRT_2 + 1e-6, "frame {frame}");
        }
        assert_eq!(shake.state, 1);
        let mut most: f32 = 0.0;
        for _ in 0..100 {
            shake.step();
            most = most.max(shake.turn.length());
        }
        assert!(shake.state == 1 && most > 0.005, "{most}");
        shake.carry_out(ShakeAsked::DieAway(5));
        for _ in 0..5 {
            shake.step();
        }
        assert_eq!((shake.state, shake.turn), (3, Vec2::ZERO));
        // Over no frames it is there at once.
        shake.carry_out(ShakeAsked::Come { reach: Vec2::splat(0.01), frames: 0 });
        assert_eq!((shake.state, shake.at), (1, Vec2::ZERO));
        // Where the point goes next is four to ten tenths from the middle.
        for _ in 0..50 {
            shake.choose();
            assert!((0.4..=1.0).contains(&shake.to.length()), "{}", shake.to);
        }
    }
}
