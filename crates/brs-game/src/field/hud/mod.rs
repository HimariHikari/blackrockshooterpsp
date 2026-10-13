//! What is drawn over the field: the window a script has up and its pictures, the
//! window people speak to her through, a scene's captions, what the mission asks,
//! and her HP, drawn from the game's layouts with its fonts as the original draws
//! them, each over what the original has it over ([`layer`]).

mod action;
mod cleared;
mod comm;
mod display;
mod failed;
pub mod gauge;
mod mark;
mod notice;
mod place;
mod window;

use std::collections::HashMap;

use anyhow::Result;
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::layout::{Layout, Look, Player};
use brs_engine::texture;
use brs_formats::layout::Objects;
use brs_formats::ptmd;

use super::comm::PLACES;
use super::event;
use super::scene::{field_flag, flag, SceneFade};
use super::staging::Sign;
use super::system;
use super::tables::FieldTables;
use super::FieldState;
use crate::battle::tables::PlayerStats;
use crate::battle::tunables::{self, Tunables};
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::progress::Progress;
use crate::text::TextColours;
use crate::writing::{Align, Fonts, Placing, Style};

/// The pack the field's own layouts are in.
const FIELD_PACK: &str = "SYSTEM.VOL/if_fldsys.lpk";
/// The file of the pack the captions' layout starts at (0xD15BC).
const CAPTIONS_LAYOUT: usize = 1;

/// The pack the window a script puts up is laid out in (0x6B5E4), a pack inside the
/// one of the interface that is always there.
pub const RESIDENT_PACK_WINDOW: &str = "SYSTEM.VOL/if_rsdsys.lpk/000.lpk";

/// The pack the window people speak through is laid out in (0x397AC), a pack inside
/// the field's own; and the files of the pack that is always there which are the
/// first four of its eight textures (0x39824 to 0x39868), before the four of its
/// own.
const COMM_PACK: &str = "SYSTEM.VOL/if_fldsys.lpk/000.lpk";
const COMM_TEXTURES_OF_ALL: [usize; 4] = [6, 7, 8, 9];
const COMM_TEXTURES_OF_ITS_OWN: [usize; 4] = [3, 4, 5, 6];

/// The layout of the window people speak through, with its textures.
pub fn comm_layout(data: &GameData, images: &mut Assets<Image>) -> Result<Layout> {
    let of_all = COMM_TEXTURES_OF_ALL.map(|file| interface::file(interface::RESIDENT_PACK, file, "ptm"));
    let of_its_own = COMM_TEXTURES_OF_ITS_OWN.map(|file| interface::file(COMM_PACK, file, "ptm"));
    interface::layout_with(data, images, COMM_PACK, 0, &[of_all, of_its_own].concat())
}

/// The file of the field's pack the mission's display starts at (0x48628): its
/// objects, its motions, its groups and its one texture.
const DISPLAY_LAYOUT: usize = 8;

/// The mission's display: its layout, and the size of its texture.
pub fn display(data: &GameData, images: &mut Assets<Image>) -> Result<display::Display> {
    let layout = interface::layout(data, images, FIELD_PACK, DISPLAY_LAYOUT)?;
    let (_, sheet) = interface::load_texture(data, images, &interface::file(FIELD_PACK, DISPLAY_LAYOUT + 3, "ptm"))?;
    Ok(display::Display { layout, sheet })
}

/// The files of the field's pack the notice of a mission's start is made from
/// (0x44624): its layout, and its one texture, which is not among the layout's.
const NOTICE_LAYOUT: usize = 23;
const NOTICE_TEXTURE: usize = 18;

/// The layout of the notice a mission starts with.
pub fn notice_layout(data: &GameData, images: &mut Assets<Image>) -> Result<Layout> {
    interface::layout_with(data, images, FIELD_PACK, NOTICE_LAYOUT, &[interface::file(FIELD_PACK, NOTICE_TEXTURE, "ptm")])
}

/// The files of the pack that is always there which her HP gauge is made from
/// (0x3C988): its layout, and its nine textures. It is given the first and the last
/// three; the five between are never drawn from, and are the first here.
const GAUGE_LAYOUT: usize = 13;
const GAUGE_TEXTURES: [usize; 9] = [20, 20, 20, 20, 20, 20, 16, 17, 9];
/// Which of them the number's figures are in (0x40A80).
const GAUGE_FIGURES: usize = 8;

/// Her HP gauge: its layout, the width of its figures' sheet and the shares of
/// her HP at which it changes colour.
pub fn gauge(data: &GameData, images: &mut Assets<Image>) -> Result<gauge::Gauge> {
    let textures = GAUGE_TEXTURES.map(|file| interface::file(interface::RESIDENT_PACK, file, "ptm"));
    let layout = interface::layout_with(data, images, interface::RESIDENT_PACK, GAUGE_LAYOUT, &textures)?;
    let (_, sheet) = interface::load_texture(data, images, &textures[GAUGE_FIGURES])?;
    let tunables = Tunables::load(data)?;
    let share = |id: u32| tunables.float(id) / 100.0;
    Ok(gauge::Gauge { layout, sheet_width: sheet.x, care: share(tunables::id::HP_CARE_PERCENT), danger: share(tunables::id::HP_DANGER_PERCENT) })
}

/// The layouts of what is drawn over the field.
#[derive(Resource)]
pub struct Layouts {
    captions: Layout,
    window: Layout,
    comm: Layout,
    display: display::Display,
    notice: Layout,
    gauge: gauge::Gauge,
    failed: Layout,
    cleared: cleared::ClearedLayout,
    /// The sheet of the letters of the sign that names a place.
    place: place::Sheet,
    /// The signs' layout, for the sign of what a button would do.
    signs: Layout,
}

impl Layouts {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<Layouts> {
        Ok(Layouts {
            captions: interface::layout(data, images, FIELD_PACK, CAPTIONS_LAYOUT)?,
            window: interface::layout(data, images, RESIDENT_PACK_WINDOW, 0)?,
            comm: comm_layout(data, images)?,
            display: display(data, images)?,
            notice: notice_layout(data, images)?,
            gauge: gauge(data, images)?,
            failed: failed::layout(data, images)?,
            cleared: cleared::load(data, images)?,
            place: place::Sheet::load(data, images)?,
            signs: interface::signs_layout(data, images)?,
        })
    }

    /// Draws a window of the kind a script puts up, for whatever else has one.
    pub fn draw_window(&self, canvas: &mut Canvas, fonts: &Fonts, colours: &TextColours, shown: &crate::field::window::Window, ahead: f32) {
        window::draw(canvas, &self.window, fonts, colours, shown, ahead);
    }
}

/// Draws a window of that kind from a layout of it, on a layer: a screen of the
/// menus draws its own with the rest of itself (0x4DC04).
pub fn draw_window_on(canvas: &mut Canvas, layer: i32, layout: &Layout, fonts: &Fonts, colours: &TextColours, shown: &crate::field::window::Window, ahead: f32) {
    window::draw_on(canvas, layer, layout, fonts, colours, shown, ahead);
}

/// The whole of the original's screen.
pub const SCREEN: Rect = Rect { min: Vec2::ZERO, max: Vec2::new(texture::SCREEN_WIDTH as f32, texture::SCREEN_HEIGHT as f32) };

/// How a caption is written (0xD1724): the text's font, sixty glyphs to a line, a
/// pixel between glyphs and four between its two lines.
const CAPTION_STYLE: Style = Style::of_kind(0, 0x3c, 2, 1.0, 4.0);
/// The groups of the captions' layout whose place a caption is put at (0xD1928): one
/// of two lines, and one of a single line, which stands lower.
const CAPTION_OF_TWO_LINES: usize = 0;
const CAPTION_OF_ONE_LINE: usize = 1;
/// How much higher the captions stand when a script has them raised (0xD19B0).
const CAPTION_RAISE: f32 = 110.0;
/// The sets of the layout that the sign that a button is waited for plays (0xD1ED4,
/// 0xD1F60): it comes up and blinks, and it shrinks away.
const SIGN_WAITS: usize = 0;
const SIGN_GOES: usize = 1;

/// The pictures loaded so far, by the name scripts know them by. One that could not
/// be loaded is kept as none, so that it is tried and told of once.
#[derive(Resource, Default)]
struct Pictures(HashMap<String, Option<Handle<Image>>>);

/// The faces loaded so far, by their event's bundle and their number in it; one that
/// could not be loaded is kept as none.
#[derive(Resource, Default)]
struct Faces(HashMap<(String, i32), Option<comm::FaceArt>>);

impl Faces {
    /// Loads a face if it has not been asked for before.
    fn want(&mut self, data: &GameData, images: &mut Assets<Image>, face: &(String, i32)) {
        if self.0.contains_key(face) {
            return;
        }
        let loaded = load_face(data, images, &face.0, face.1);
        if let Err(error) = &loaded {
            warn!("face {} of {} cannot be shown: {error:#}", face.1, face.0);
        }
        self.0.insert(face.clone(), loaded.ok());
    }
}

/// Loads a face of an event: its picture, and its name.
fn load_face(data: &GameData, images: &mut Assets<Image>, bundle: &str, face: i32) -> Result<comm::FaceArt> {
    use anyhow::Context;
    let files = event::face_files(bundle, face);
    let (picture, picture_size) = interface::load_texture(data, images, &files.picture)?;
    let name = Objects::parse(&data.read(&files.name)?).with_context(|| format!("parsing {}", files.name))?;
    let (name_texture, _) = interface::load_texture(data, images, &files.name_texture)?;
    Ok(comm::FaceArt { picture, picture_size, name, name_textures: vec![name_texture] })
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Pictures>().init_resource::<Faces>().add_systems(
        Update,
        (
            draw_scene_fade,
            draw_pictures,
            draw_window,
            draw_comm,
            draw_captions,
            draw_marks,
            draw_mission,
            draw_gauge,
            draw_failed,
            draw_cleared,
            draw_place,
            draw_action,
        )
            .run_if(in_state(crate::Mode::Field).and_then(field_there)),
    );
    // The ride on the bike has its talk through the same window.
    app.add_systems(Update, draw_comm.run_if(in_state(crate::Mode::Ride)));
}

/// The field's flag that has the marks over characters left out, whatever the
/// one that shows them says (0x10F288). Nothing has been found to set it.
const MARKS_OFF: u32 = 0x28;
/// And the one that has the signs of traps left out (0xD3E84); where in the
/// field's tunables how near she has to be for a trap's sign to show is
/// (0xD3E28).
const TRAP_SIGNS_OFF: u32 = 0x5a;
const PRM_TRAP_SIGN_REACH: usize = 0x12;

/// Draws the marks over the characters that have noticed her (0x10F29C, before
/// what the field shows of the mission): each over its character's head, where
/// the camera has that, and as big as the character is on the screen there. A
/// script can have them left out with the rest of that part of the screen
/// (`s104(2, ..)`), and a character behind the camera has none.
#[allow(clippy::too_many_arguments)]
fn draw_marks(
    state: Res<FieldState>,
    progress: Res<Progress>,
    tables: Res<FieldTables>,
    layouts: Res<Layouts>,
    actors: Query<(&super::Actor, &super::MarkNode)>,
    places: Query<&GlobalTransform>,
    camera: Single<(&Camera, &GlobalTransform), With<super::FollowCamera>>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    let scene = &state.scene;
    if !interface_shown(scene, &progress) || !scene.flag(field_flag::PART_2_SHOWN) || scene.flag(MARKS_OFF) {
        return;
    }
    let (camera, view) = *camera;
    let depth = |point: Vec3| -view.affine().inverse().transform_point3(point).z;
    // Where on the screen the camera has a point: what it sees is the screen.
    let on_screen = |point: Vec3| camera.world_to_ndc(view, point).map(|to| Vec2::new(to.x + 1.0, 1.0 - to.y) * 0.5 * SCREEN.size());
    for (actor, node) in &actors {
        let Some(character) = scene.characters.get(&actor.id).filter(|character| !character.hidden()) else { continue };
        // The tile of one a mission is after stands for as long as it is that.
        let pointed_out = character.flags.has(flag::POINTED_OUT);
        if character.mark.is_none() && !pointed_out {
            continue;
        }
        // The node of its model that is for this, or so far above where it stands.
        let top = node.0.and_then(|node| places.get(node).ok()).map_or(character.position + character.up() * mark::NO_NODE, GlobalTransform::translation);
        let point = mark::place(top, depth);
        if depth(point) <= 0.0 {
            continue;
        }
        let (Some(at), Some(higher)) = (on_screen(point), on_screen(mark::above(point))) else { continue };
        let scale = mark::scale(at.y - higher.y);
        if let Some(frames) = character.mark {
            mark::draw(&mut canvas, &layouts.signs, frames + steps.overstep_fraction() * 2.0, at, scale);
        }
        if pointed_out {
            mark::draw_tile(&mut canvas, &layouts.signs, at, scale);
        }
    }

    // The sign of a character that has one (0x1077EC, after its mark): where
    // the camera has the place it was given at, however far she is from it.
    if !scene.flag(TRAP_SIGNS_OFF) {
        for character in scene.characters.values().filter(|character| !character.hidden()) {
            let Some((place, frames)) = character.sign.filter(|&(place, _)| depth(place) > 0.0) else { continue };
            if let Some(at) = on_screen(place).filter(|&at| SCREEN.contains(at)) {
                mark::draw_of_trap(&mut canvas, &layouts.signs, (frames as f32 + steps.overstep_fraction()) * 2.0, at);
            }
        }
    }

    // Then the signs of the traps that have one (0x10F2E8, 0xD3DB8): where the
    // camera has the middle of the trap's box, while she is no further from it
    // than the field's tunables say and it is before the camera and on the
    // screen.
    let Some(her) = scene.player().filter(|_| !scene.flag(TRAP_SIGNS_OFF)) else { return };
    let reach = f32::from(tables.tunables.u16(PRM_TRAP_SIGN_REACH).unwrap_or(0));
    for trap in scene.traps.values().filter(|trap| trap.sign.is_some() && !trap.sign_hidden) {
        if trap.centre.distance(her.position) > reach || depth(trap.centre) < 0.0 {
            continue;
        }
        if let Some(at) = on_screen(trap.centre).filter(|&at| SCREEN.contains(at)) {
            let frames = (trap.sign_frames as f32 + steps.overstep_fraction()) * 2.0;
            mark::draw_of_trap(&mut canvas, &layouts.signs, frames, at);
        }
    }
}

/// Whether there is a field to draw anything of. While the screen between
/// missions or the title is up the field has been left, and what its scripts had
/// over it is gone with it: in the original those are tasks of the field's and
/// of its scripts, which end as it does. Here the scene is kept until the next
/// one is made, so that a mission whose script had darkened the scene alone
/// before it ended (`s197`) would else leave that dark over the screen between
/// missions.
fn field_there(state: Res<FieldState>) -> bool {
    state.hub.is_none() && state.title.is_none()
}

/// Draws the sign of what a button would do, with what the field shows of the
/// mission (0xEDF20): over her, at the place on the screen the camera has what
/// is fifty above where she stands (0xEE178); not while that is behind the
/// camera, nor while the interface is off (0x46718).
#[allow(clippy::too_many_arguments)]
fn draw_action(
    state: Res<FieldState>,
    progress: Res<Progress>,
    layouts: Res<Layouts>,
    camera: Single<(&Camera, &GlobalTransform), With<super::FollowCamera>>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    let scene = &state.scene;
    let (Some(showing), Some(player)) = (scene.action_sign.showing(), scene.player()) else { return };
    if !interface_shown(scene, &progress) {
        return;
    }
    let (camera, transform) = *camera;
    let over = super::action::place(player.position, player.up());
    // In front of the camera, it is where the camera has it of all it sees.
    let ahead = -transform.affine().inverse().transform_point3(over).z;
    let Some(to) = camera.world_to_ndc(transform, over).filter(|_| ahead > 0.0) else { return };
    let at = Vec2::new(to.x + 1.0, 1.0 - to.y) * 0.5 * SCREEN.size();
    action::draw(&mut canvas, &layouts.signs, &showing, at, steps.overstep_fraction());
}

/// Draws the sign that names a place, which the save's flag that has the
/// interface off leaves out too (0x46EE8): the name of the place she has come
/// into, with what the field shows of the mission (0x10F388); and the title of
/// a scene's place, over the screen's fade and under the captions.
fn draw_place(state: Res<FieldState>, progress: Res<Progress>, layouts: Res<Layouts>, mut canvas: ResMut<Canvas>) {
    let scene = &state.scene;
    if progress.flag(INTERFACE_OFF) {
        return;
    }
    if let Some(name) = scene.place_name.as_ref().filter(|_| interface_shown(scene, &progress)) {
        place::draw(&mut canvas, layer::DISPLAY, &layouts.place, &name.sign);
    }
    if let Some(title) = scene.location.as_ref().filter(|title| title.shown()) {
        place::draw(&mut canvas, layer::LOCATION, &layouts.place, &title.sign);
    }
}

/// Draws the window of a mission failed, as far on from the game's last frame as
/// the frame drawn is: over all the field shows, and under the screen's fade.
fn draw_failed(state: Res<FieldState>, layouts: Res<Layouts>, steps: Res<Time<Fixed>>, mut canvas: ResMut<Canvas>) {
    if let Some(shown) = &state.scene.failed {
        failed::draw(&mut canvas, &layouts.failed, shown, steps.overstep_fraction());
    }
}

/// Draws the window of a mission cleared, as far on from the game's last frame
/// as the frame drawn is: where the field shows what the mission asks.
fn draw_cleared(
    state: Res<FieldState>,
    layouts: Res<Layouts>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    seen: Res<brs_engine::canvas::Seen>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    if let Some(shown) = &state.scene.cleared {
        cleared::draw(&mut canvas, &layouts.cleared, &fonts, &colours, shown, &seen, steps.overstep_fraction());
    }
}

/// Draws the fade of the scene alone that a script has asked for (0xD0470): a sheet
/// over the whole screen, which everything else here is drawn over.
fn draw_scene_fade(state: Res<FieldState>, mut canvas: ResMut<Canvas>) {
    let colour = state.scene.scene_fade.as_ref().and_then(SceneFade::colour);
    if let Some(colour) = colour.filter(|colour| colour[3] != 0) {
        canvas.fill(layer::SCENE_FADE, SCREEN, colour);
    }
}

/// Draws the pictures the script has up over the whole screen (0x6B2CC, 0x2DE94),
/// each as far as it has come or gone: over what the field shows of the mission
/// and of her, and under the windows. They are drawn in the order of their slots,
/// which is the order a lesson makes them in.
fn draw_pictures(
    data: Res<GameData>,
    state: Res<FieldState>,
    mut pictures: ResMut<Pictures>,
    mut images: ResMut<Assets<Image>>,
    mut canvas: ResMut<Canvas>,
) {
    for picture in state.scene.pictures.values().filter(|picture| picture.alpha() > 0.0) {
        let loaded = pictures.0.entry(picture.name.clone()).or_insert_with(|| match load(&data, &picture.name) {
            Ok(picture) => Some(images.add(picture)),
            Err(error) => {
                warn!("picture {} cannot be shown: {error:#}", picture.name);
                None
            }
        });
        let Some(handle) = loaded else { continue };
        let colour = [255, 255, 255, (picture.alpha() * 255.0) as u8];
        canvas.sprite(layer::PICTURES, Paint::textured(handle), Vertex::new(SCREEN.min, Vec2::ZERO, colour), Vertex::new(SCREEN.max, Vec2::ONE, colour));
    }
}

/// Loads a picture that fills the screen (`IF/PICT`), by the name scripts give it.
fn load(data: &GameData, name: &str) -> anyhow::Result<Image> {
    use anyhow::Context;
    let path = format!("IF/PICT/{}.PTM.ZZZ", name.to_ascii_uppercase());
    let file = data.read(&path)?;
    let header = ptmd::parse_header(&file).with_context(|| format!("{path} is not a texture"))?;
    texture::screen_picture(&file, &header).with_context(|| format!("{path} is not the size of a picture of the screen"))
}

/// Draws the window the script has up, as far on from the game's last frame as the
/// frame drawn is.
fn draw_window(
    state: Res<FieldState>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    layouts: Res<Layouts>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    if let Some(shown) = &state.scene.window {
        window::draw(&mut canvas, &layouts.window, &fonts, &colours, shown, steps.overstep_fraction());
    }
}

/// Draws the window people speak to her through, as far on from the game's last
/// frame as the frame drawn is.
#[allow(clippy::too_many_arguments)]
fn draw_comm(
    data: Res<GameData>,
    state: Res<FieldState>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    layouts: Res<Layouts>,
    steps: Res<Time<Fixed>>,
    mut faces: ResMut<Faces>,
    mut images: ResMut<Assets<Image>>,
    mut canvas: ResMut<Canvas>,
) {
    let comm = &state.scene.comm;
    for face in comm.window.faces.iter().filter_map(|face| face.face.as_ref()) {
        faces.want(&data, &mut images, face);
    }
    let art: [Option<&comm::FaceArt>; PLACES] =
        std::array::from_fn(|place| comm.window.faces[place].face.as_ref().and_then(|face| faces.0.get(face)?.as_ref()));
    comm::draw(&mut canvas, &layouts.comm, &fonts, &colours, comm, art, steps.overstep_fraction());
}

/// Draws what is being said in a scene (0xD183C): each caption centred across the
/// screen with its shadow, as far as it has come or gone, and the sign that a
/// button is waited for.
fn draw_captions(
    state: Res<FieldState>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    layouts: Res<Layouts>,
    time: Res<Time>,
    mut canvas: ResMut<Canvas>,
    mut sign: Local<(u32, Option<Player>)>,
) {
    let Some(captions) = state.scene.captions.as_ref() else {
        sign.1 = None;
        return;
    };
    let layout = &layouts.captions;
    let raise = if captions.raised { CAPTION_RAISE } else { 0.0 };
    for caption in captions.shown() {
        let block = fonts.write(&CAPTION_STYLE, &caption.text, &colours);
        // Where the layout has a caption of its number of lines.
        let lines = block.size().y / CAPTION_STYLE.size;
        let group = if lines < 2.0 { CAPTION_OF_ONE_LINE } else { CAPTION_OF_TWO_LINES };
        let top = layout.first_of(group).and_then(|object| layout.place(object)).map_or(0.0, |place| place.y);
        let alpha = (caption.alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let placing = Placing { align: Align::Centre, tint: [255, 255, 255, alpha], shadow: true, ..Placing::at(Vec2::new(0.0, top - raise)) };
        fonts.draw(&mut canvas, layer::CAPTIONS, &block, &placing);
    }

    // The sign starts its motion again each time it is set to do something.
    if captions.sign == Sign::None {
        *sign = (captions.sign_set, None);
    } else if sign.0 != captions.sign_set || sign.1.is_none() && captions.sign == Sign::Waiting {
        let set = if captions.sign == Sign::Waiting { SIGN_WAITS } else { SIGN_GOES };
        *sign = (captions.sign_set, Some(layout.player(set)));
    }
    if let Some(player) = &mut sign.1 {
        let over = player.advance(layout, interface::frames(&time));
        if over && captions.sign == Sign::Leaving {
            sign.1 = None;
        } else {
            layout.draw(&mut canvas, layer::CAPTIONS, player, &Look::default());
        }
    }
}

/// The save's flag that has nothing of the mission's display and of her gauge
/// drawn (0x2D66C): scripts set it for what they show without them, as a mission's
/// opening is and its end.
const INTERFACE_OFF: i32 = 6167;

/// Whether the mission's display and her gauge are drawn at all (0x10F0F8 draws
/// them while the field is not held up by something of its own, as it is by the
/// window of a mission failed: its flag 0x56). The window of a mission cleared
/// and the leaving of the field, which are not the original's yet, stand
/// without them.
fn interface_shown(scene: &super::scene::Scene, progress: &Progress) -> bool {
    scene.cleared.is_none() && scene.failed.is_none() && scene.leaving.is_none() && !progress.flag(INTERFACE_OFF)
}

/// Draws the mission's display (what is to be done, and its numbers), at the foot
/// of the screen on the left: the panels a script has shown.
fn draw_mission(
    state: Res<FieldState>,
    tables: Res<FieldTables>,
    progress: Res<Progress>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    layouts: Res<Layouts>,
    steps: Res<Time<Fixed>>,
    mut canvas: ResMut<Canvas>,
) {
    let scene = &state.scene;
    // The notice of a mission's start is the display's own, and is drawn first.
    if let Some(shown) = &scene.notice {
        notice::draw(&mut canvas, &layouts.notice, &fonts, &colours, shown, steps.overstep_fraction());
    }
    if interface_shown(scene, &progress) {
        layouts.display.draw(&mut canvas, &fonts, &colours, &system::panels(&progress, &tables));
    }
}

/// Draws her HP gauge, at the foot of the screen on the right (0xEDAAC): unless a
/// script has that part of the screen closed, as for a scene. Its motion goes on
/// whether it is drawn or not (0x44C1C).
fn draw_gauge(
    state: Res<FieldState>,
    progress: Res<Progress>,
    stats: Res<PlayerStats>,
    layouts: Res<Layouts>,
    time: Res<Time>,
    mut canvas: ResMut<Canvas>,
    mut player: Local<Option<Player>>,
) {
    let gauge = &layouts.gauge;
    let player = player.get_or_insert_with(|| gauge.player(0.0));
    player.advance(&gauge.layout, interface::frames(&time));
    let scene = &state.scene;
    if interface_shown(scene, &progress) && !scene.flag(field_flag::GAUGE_HIDDEN) {
        gauge.draw(&mut canvas, player, stats.hp as i32, stats.max_hp as i32, gauge::ON_THE_FIELD);
    }
}

