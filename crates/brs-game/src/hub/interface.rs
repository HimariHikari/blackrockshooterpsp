//! What the screen where a mission is chosen shows and keeps (the object made at
//! 0x61B40, and its parts up to 0x69330): the stages and their missions as it
//! lists them, the layout's three places, the texts and figures, the list of a
//! stage's missions, and the script that plays its comings and goings. See
//! `docs/STAGE_SELECT_NOTES.md`.
//!
//! The first place has the world and the panel down the right, the second the
//! frame, the root's entries, the list of missions and the strip along the
//! foot, and the third a wipe of squares. The world is a map of tiles with a
//! grid over it, which is moved so that the stage shown is at its middle, and
//! has the stages' marks on it. Of the scene her model stands in, what is here
//! is how far her animation and the camera's are (`scene.rs` shows them).

use std::sync::Arc;

use anyhow::{Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::animation::Clip;
use brs_engine::camera::CameraClip;
use brs_engine::canvas::{Canvas, Paint, Vertex, HEIGHT, WIDTH};
use brs_engine::layout::{draw_object, modulate, Layout, Look, Player};
use brs_formats::script::{self, Call, Machine, Run, Script};
use brs_formats::{cam, xtc};

use crate::data::GameData;
use crate::field::tables::FieldTables;
use crate::interface::{self, layer};
use crate::progress::Progress;
use crate::sound::Request;
use crate::text::{self, TextColours};
use crate::writing::{Align, Fonts, Placing, Style};

/// The archive the screen is of (0x1676BC).
pub const PACK: &str = "IF/MENU/IF_STAGE_SELECT.GZ";
// Its files (0x61BA0 on): the layout, the stages' texts, the strip's, the
// messages' and the script.
const LAYOUT: usize = 30;
const STAGE_TEXTS: usize = 33;
const STRIP_TEXTS: usize = 34;
pub const MESSAGES: usize = 35;
const SCRIPT: usize = 36;
/// The tiles the world's map is made of: the archive's first files
/// (0x61C0C), each so many pixels across and down (0x64FC4).
const TILES: usize = 14;
const TILE: f32 = 128.0;
/// The animations her model has here, and the one that is both of them once
/// the story is late (0x61EC4); and the scene's camera with its two
/// animations, and the one it has for both then (0x61F98).
const HER_MOTIONS: [[usize; 2]; 2] = [[38, 39], [40, 40]];
const CAMERAS: [(usize, [usize; 2]); 2] = [(41, [43, 42]), (44, [45, 45])];
/// The picture of each stage, which the layout's texture 14 is made as the
/// stage is shown (0x61D84).
const STAGE_PICTURES: [usize; STAGES] = [22, 24, 25, 26, 27, 28];
const PICTURE_TEXTURE: usize = 14;

pub const STAGES: usize = 6;
/// The root's entries (0x6168C answers 4 for how many): SAVE, LOAD, the stage
/// and the MENU.
pub const ENTRIES: usize = 4;
pub mod entry {
    pub const SAVE: usize = 0;
    pub const LOAD: usize = 1;
    pub const STAGE: usize = 2;
    pub const MENU: usize = 3;
}
/// Which of the strip's texts is said at a stage's missions (0x60CFC).
const STRIP_AT_MISSIONS: usize = 5;
/// The rows of the list of missions that are shown at once.
const ROWS: usize = 6;

// The flags of the game's progress it goes by: that a stage is there to be
// chosen, for the second stage (the later ones' follow it two apart), with the
// flag after each that it has been told of (0x61088); and the one late in the
// story with which a stage lists one mission only and cannot be changed
// (0x639B0, 0x62414).
const STAGE_THERE: i32 = 0x17f4;
const LATE: i32 = 0x180a;
const ONLY_MISSION: i32 = 44;
/// The stages that have no mark on the world's map with the others
/// (0x62A08): the fourth has one of its own, with its name on the other
/// side, and the sixth is the Moon.
const FOURTH_STAGE: usize = 3;
const THE_MOON: usize = 5;
/// The variable a mission's state is kept in is this and its number (0x14A4DC):
/// open from 2 (0x14A530), not begun below 3 (0x14A554), cleared from 201
/// (0x14A574).
const MISSION_STATE: i32 = 100;
const OPEN_FROM: i32 = 2;
const BEGUN_FROM: i32 = 3;
const CLEARED_FROM: i32 = 201;

/// The sounds the script asks for by number (0x674D0), of the bank that is
/// always there.
const SCRIPT_SOUNDS: [u32; 3] = [0x11, 0x12, 0xa];

/// What the layout's groups are to the code that draws it (0x62590, the tables
/// at 0x167AB0 and 0x167C28).
mod group {
    pub const WORLD: usize = 2;
    pub const MOON: usize = 3;
    pub const MARKS: usize = 10;
    pub const RING: usize = 14;
    pub const RING_BACK: usize = 15;
    pub const STAGE_TOLD: usize = 20;
    pub const ENTRY_BAR: usize = 22;
    pub const ON_STAGE: usize = 23;
    pub const STAGE_WORD: usize = 26;
    pub const ARROWS: [usize; 2] = [28, 29];
    pub const PANEL: std::ops::RangeInclusive<usize> = 30..=45;
    pub const SCENE: usize = 47;
    pub const ROW_BARS: usize = 49;
    pub const ROW_CURSOR: usize = 50;
    pub const TITLES: usize = 54;
    pub const KIND_BAR: usize = 56;
    pub const FROM: usize = 57;
    pub const HEADER: usize = 62;
    pub const FOURTH_MARK: usize = 65;
    pub const STRIP: usize = 69;
    pub const STAGE_CHOICE: usize = 70;
    pub const TITLE_AT_CURSOR: usize = 71;
    pub const ON_SAVE: usize = 73;
    pub const ON_LOAD: usize = 74;
    pub const ON_MENU: usize = 75;
    pub const KNOB: usize = 76;
    pub const LEVEL: usize = 77;
    pub const NO_LEVEL: usize = 78;
}
/// The groups that are drawn whether or not their tracks are of those left
/// out when all of a place is drawn (0x30F30 without 0x31B48).
const ALWAYS_DRAWN: [usize; 7] = [group::MOON, group::ON_STAGE, group::ON_SAVE, group::ON_LOAD, group::ON_MENU, group::LEVEL, group::NO_LEVEL];
/// A set no layout has: what a place plays before the script has given it one.
const NO_SET: usize = usize::MAX;
/// The groups of the panel that are cut to an object's box (0x167BE8).
const PANEL_CUTS: [(usize, usize); 4] = [(31, 71), (33, 75), (34, 77), (45, 81)];

/// The layout's objects that things are written and drawn at (0x68000,
/// 0x6852C, 0x65ADC, 0x63554).
mod object {
    pub const EARTH: usize = 4;
    pub const MARK: usize = 22;
    pub const NAME: usize = 23;
    pub const WORLD_MIDDLE: usize = 27;
    pub const SECOND_TEXT: usize = 43;
    pub const STAGE_NUMBER: usize = 44;
    pub const COUNTS: usize = 45;
    pub const STAGE_WORD: usize = 60;
    pub const STAGE_WORD_MARK: usize = 63;
    pub const ROW_MARK: usize = 110;
    pub const ROW_TITLE: usize = 111;
    pub const KIND: usize = 120;
    pub const OBJECTIVE: usize = 143;
    pub const HINT: usize = 154;
    pub const FACE: usize = 155;
    pub const BRIEFING: usize = 170;
    pub const HEADER: usize = 173;
    pub const FOURTH_MARK: usize = 176;
    pub const FOURTH_NAME: usize = 177;
    pub const STRIP: usize = 183;
    pub const CHOICE: usize = 186;
    pub const CHOICE_MARKED: usize = 187;
    pub const CHOICE_MARK: usize = 188;
    pub const ROW_TITLE_AT_CURSOR: usize = 189;
    pub const KNOB: usize = 196;
    pub const LEVEL: usize = 198;
}

/// The layout's textures that sprites are drawn from: the figures, the small
/// figures, the marks and the faces.
mod texture {
    pub const FIGURES: usize = 9;
    pub const SMALL_FIGURES: usize = 12;
    pub const MARKS: usize = 13;
    pub const FACES: usize = 17;
}

// How its texts are written (0x67BF8, the table at 0x167EA0; 0x657D8, the table
// at 0x167CF8).
const STRIP_STYLE: Style = Style::of_kind(0, 0x32, 1, 1.0, 3.0);
const STAGE_WORD_STYLE: Style = Style::of_kind(2, 0x14, 1, 2.0, 3.0);
const SECOND_TEXT_STYLE: Style = Style::of_kind(1, 0x14, 3, 1.0, 1.0);
const TITLE_STYLE: Style = Style::of_kind(0, 0x32, 1, 0.0, 2.0);
const KIND_STYLE: Style = Style::of_kind(2, 0x32, 1, 1.0, 3.0);
const HEADER_STYLE: Style = Style::of_kind(2, 0x28, 1, 1.0, 3.0);
const BRIEFING_STYLE: Style = Style::of_kind(0, 0x32, 5, 0.0, 2.0);
const TWO_LINES_STYLE: Style = Style::of_kind(0, 0x32, 2, 0.0, 2.0);
const LEVEL_STYLE: Style = Style::of_kind(2, 0x14, 1, 2.0, 3.0);

/// How far one of the root's entries, and one row of the list, is under the
/// one before (0x62808, 0x62880).
const PITCH: f32 = 20.0;
/// The frames the rings about the world take to turn once (0x62FA0, 0x62FFC).
const RING_FRAMES: [i32; 2] = [500, 270];
/// How wide the strip is (0x6204C).
const STRIP_ROOM: i32 = 480;
/// What the bar of the list's knob stands for, in pixels (0x630FC).
const KNOB_RUN: f32 = 119.0;
/// What a row past the last mission has for its title (0x167968), and how
/// much of its alpha it is written at (0x65C30).
const NO_TITLE: &str = "ーーーーーーー";
const PAST_THE_LAST: f32 = 0.7;
/// The colours a row's bar is drawn by, by the kind of its mission, and with
/// no mission (0x63250).
const KIND_COLOURS: [[u8; 4]; 3] = [[255, 255, 255, 255], [0x80, 0x80, 0x80, 255], [0xc0, 0xc0, 0xc0, 255]];
const NO_MISSION: [u8; 4] = [0x40, 0x40, 0x40, 255];
/// The alpha of the arrows by the stage's entry while the stage cannot be
/// changed (0x62790).
const ARROWS_OFF: u8 = 0x4d;
/// Where in the marks' texture each mark of what a mission has come to is, and
/// the two of a stage (0x65D00 on; 0x68084 on): each fifteen across and down.
const COME_TO_MARKS: [Option<Vec2>; 5] = [None, Some(Vec2::new(112.0, 82.0)), Some(Vec2::new(17.0, 66.0)), Some(Vec2::new(112.0, 114.0)), Some(Vec2::new(112.0, 98.0))];
const STAGE_MARKS: [Option<Vec2>; 3] = [None, Some(Vec2::new(1.0, 66.0)), Some(Vec2::new(17.0, 66.0))];
const MARK_SIZE: f32 = 15.0;

/// A stage as its texts have it (`033.xtc`): its name in full-width letters, a
/// second text in plain ones, and where it is on the map the world is made of.
#[derive(Clone, Debug, PartialEq)]
struct StageText {
    name: String,
    second: String,
    at: Vec2,
}

/// The world's map, as the script's tables have it (0x62308): which tile is
/// where, so many across; the lines of its grid, in the map's pixels; and
/// the colour they are drawn in.
struct Map {
    tiles: Vec<i32>,
    across: usize,
    latitudes: Vec<i32>,
    longitudes: Vec<i32>,
    lines: [u8; 4],
}

/// What the screen is drawn from.
pub struct Assets {
    pub layout: Layout,
    tiles: Vec<Handle<Image>>,
    map: Map,
    pictures: Vec<Handle<Image>>,
    stages: Vec<StageText>,
    /// What the strip says of the root's entries, and of the list of missions.
    strip: Vec<String>,
    script: Arc<Script>,
    /// The script's `local_str`: the word for a stage with its number, the
    /// same with its name, and the words for the kinds of mission.
    local: Vec<String>,
    /// The sizes of the textures sprites are drawn from, by their numbers.
    sizes: [Vec2; 4],
    /// Her two animations and the camera's two, as they are at first and as
    /// they are once the story is late.
    her: [[Arc<Clip>; 2]; 2],
    cameras: [[Arc<CameraClip>; 2]; 2],
}

impl Assets {
    pub fn load(data: &GameData, images: &mut bevy::prelude::Assets<Image>) -> Result<Assets> {
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        // The layout's textures (0x61C2C, the table at 0x167A58): five places are
        // left empty.
        let none = own(15);
        let textures = [
            shared(11),
            own(15),
            none.clone(),
            none.clone(),
            none.clone(),
            own(16),
            own(17),
            shared(12),
            none.clone(),
            shared(9),
            own(18),
            own(19),
            own(20),
            own(21),
            own(22),
            own(20),
            shared(10),
            own(23),
            none,
            own(14),
            own(29),
        ];
        let layout = interface::layout_with(data, images, PACK, LAYOUT, &textures)?;
        let mut tiles = Vec::new();
        for file in 0..TILES {
            tiles.push(interface::load_texture(data, images, &own(file))?.0);
        }
        let mut pictures = Vec::new();
        for file in STAGE_PICTURES {
            pictures.push(interface::load_texture(data, images, &own(file))?.0);
        }
        let size = |index: usize, images: &mut bevy::prelude::Assets<Image>| Ok::<Vec2, anyhow::Error>(interface::load_texture(data, images, &textures[index])?.1);
        let sizes = [size(texture::FIGURES, images)?, size(texture::SMALL_FIGURES, images)?, size(texture::MARKS, images)?, size(texture::FACES, images)?];

        let table = xtc::Table::parse(&data.read(interface::file(PACK, STAGE_TEXTS, "xtc"))?).context("the stages' texts")?;
        let mut stages = Vec::new();
        for index in 0..STAGES {
            let record = table.record(index).filter(|record| record.len() >= 16).with_context(|| format!("the stages' texts have no stage {index}"))?;
            let word = |at: usize| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            stages.push(StageText { name: table.string(word(0))?.to_owned(), second: table.string(word(4))?.to_owned(), at: Vec2::new(word(8) as f32, word(12) as f32) });
        }
        let table = xtc::Table::parse(&data.read(interface::file(PACK, STRIP_TEXTS, "xtc"))?).context("the strip's texts")?;
        let mut strip = Vec::new();
        for index in 0..6 {
            let at = table.record(index).and_then(|record| record.get(..4)).with_context(|| format!("the strip has no text {index}"))?;
            strip.push(table.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned());
        }
        let script = data.script(&interface::file(PACK, SCRIPT, "sscr"))?;
        let strings = script.extent("local_str").context("the script has no local_str")?;
        let local: Vec<String> = strings.chunks_exact(4).take(5).filter_map(|at| script.string(u32::from_le_bytes(at.try_into().unwrap()))).collect();
        anyhow::ensure!(local.len() == 5, "the script's local_str has {} strings", local.len());
        // The world's tables: `table` is the colour of the grid and how many
        // entries the three others have.
        let words = |name: &str| -> Result<Vec<i32>> {
            let bytes = script.extent(name).with_context(|| format!("the script has no {name}"))?;
            Ok(bytes.chunks_exact(4).map(|word| i32::from_le_bytes(word.try_into().unwrap())).collect())
        };
        let table = words("table")?;
        anyhow::ensure!(table.len() >= 5 && table[1..5].iter().all(|&count| count >= 0), "the script's table is {table:?}");
        let counted = |name: &str, count: i32| -> Result<Vec<i32>> {
            let mut all = words(name)?;
            anyhow::ensure!(all.len() >= count as usize, "the script's {name} has {} entries, not {count}", all.len());
            all.truncate(count as usize);
            Ok(all)
        };
        let map = Map {
            tiles: counted("map_tex", table[3] * table[4])?,
            across: table[3] as usize,
            latitudes: counted("latitude", table[1])?,
            longitudes: counted("longitude", table[2])?,
            lines: (table[0] as u32).to_le_bytes(),
        };
        let motion = |file: usize| data.clip(interface::file(PACK, file, "insa"));
        let camera = |rig: usize, file: usize| -> Result<Arc<CameraClip>> {
            let (rig, moved) = (data.read(interface::file(PACK, rig, "insm"))?, data.read(interface::file(PACK, file, "insa"))?);
            Ok(CameraClip::new(cam::Camera::from_parts(&rig, &moved).with_context(|| format!("the scene's camera of animation {file}"))?))
        };
        let [early, late] = HER_MOTIONS;
        let her = [[motion(early[0])?, motion(early[1])?], [motion(late[0])?, motion(late[1])?]];
        let [(rig, early), (late_rig, late)] = CAMERAS;
        let cameras = [[camera(rig, early[0])?, camera(rig, early[1])?], [camera(late_rig, late[0])?, camera(late_rig, late[1])?]];
        Ok(Assets { layout, tiles, map, pictures, stages, strip, script: Arc::new(script), local, sizes, her, cameras })
    }

    /// One of her two animations, and the camera with one of its two.
    pub fn her_clip(&self, late: bool, which: usize) -> &Arc<Clip> {
        &self.her[usize::from(late)][which.min(1)]
    }

    pub fn camera_clip(&self, late: bool, which: usize) -> &Arc<CameraClip> {
        &self.cameras[usize::from(late)][which.min(1)]
    }

    /// Where a stage is on the world's map, in whole pixels of it.
    fn place_of(&self, stage: usize) -> [i32; 2] {
        self.stages.get(stage).map_or([0; 2], |stage| [stage.at.x as i32, stage.at.y as i32])
    }

    fn size(&self, texture: usize) -> Vec2 {
        match texture {
            texture::FIGURES => self.sizes[0],
            texture::SMALL_FIGURES => self.sizes[1],
            texture::MARKS => self.sizes[2],
            _ => self.sizes[3],
        }
    }
}

/// A number in full-width figures, as the texts have theirs (0x618BC).
fn full_width(number: i32) -> String {
    number.to_string().chars().map(|figure| if figure.is_ascii_digit() { char::from_u32(figure as u32 + 0xfee0).unwrap_or(figure) } else { figure }).collect()
}

/// A stage as it is listed (0x60D98).
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    /// It can be chosen, and this is the first time it is seen that it can.
    pub there: bool,
    pub new: bool,
    /// 2 if every mission of it is cleared, 1 if every mission of the story.
    pub mark: usize,
    pub cleared: i32,
    pub open: i32,
}

/// A mission as it is listed (0x60A84).
#[derive(Clone, Debug, PartialEq)]
pub struct Mission {
    pub number: i32,
    pub stage: usize,
    pub open: bool,
    pub kind: usize,
    pub from: usize,
    /// 1 not begun, 2 cleared, 3 and 4 by the two flags its record names.
    pub come_to: usize,
    pub title: String,
    pub briefing: String,
    pub objective: String,
    pub hint: String,
    pub level: i32,
}

/// Lists the stages (0x60D98): which are there, and how far each has been
/// cleared. Seeing a stage that is newly there sets the flag that it has been.
fn list_stages(progress: &mut Progress, tables: &FieldTables) -> Vec<Stage> {
    // For each stage: its open missions, those of them that are cleared, its
    // missions of the story, those of them that are cleared, and all it has.
    let mut counts = [[0i32; 5]; STAGES];
    for number in 0..tables.missions {
        let Some(record) = tables.mission_record(number).filter(|record| usize::from(record.stage) < STAGES) else { continue };
        let of = &mut counts[usize::from(record.stage)];
        of[4] += 1;
        if record.kind == 0 {
            of[2] += 1;
        }
        let state = progress.variable(MISSION_STATE + number);
        if state >= OPEN_FROM {
            of[0] += 1;
            if state >= CLEARED_FROM {
                of[1] += 1;
                if record.kind == 0 {
                    of[3] += 1;
                }
            }
        }
    }
    let mut stages = Vec::new();
    for (stage, [open, cleared, story, story_cleared, all]) in counts.into_iter().enumerate() {
        let (mut there, mut new) = (stage == 0, false);
        if stage > 0 {
            let flag = STAGE_THERE + 2 * (stage as i32 - 1);
            if progress.flag(flag) {
                there = true;
                if !progress.flag(flag + 1) {
                    new = true;
                    progress.set_flag(flag + 1, true);
                }
            }
        }
        let mark = match cleared {
            cleared if cleared <= 0 => 0,
            cleared if cleared == all => 2,
            _ if story == story_cleared => 1,
            _ => 0,
        };
        stages.push(Stage { there, new, mark, cleared, open });
    }
    stages
}

/// Lists the missions (0x60A84).
fn list_missions(progress: &Progress, tables: &FieldTables) -> Vec<Mission> {
    let mut missions = Vec::new();
    for number in 0..tables.missions.min(120) {
        let Some(record) = tables.mission_record(number) else { continue };
        let state = progress.variable(MISSION_STATE + number);
        let mut come_to = 0;
        if state < BEGUN_FROM {
            come_to = 1;
        }
        if state >= CLEARED_FROM {
            come_to = 2;
        }
        if record.marks[0] >= 0 {
            if progress.flag(record.marks[0]) {
                come_to = 3;
            }
            if progress.flag(record.marks[1]) {
                come_to = 4;
            }
        }
        missions.push(Mission {
            number,
            stage: usize::from(record.stage),
            open: state >= OPEN_FROM,
            kind: usize::from(record.kind),
            from: usize::from(record.from),
            come_to,
            title: record.title,
            briefing: record.briefing,
            objective: record.objective,
            hint: record.hint,
            level: record.level,
        });
    }
    missions
}

/// What its own state is (its + 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Made = 0,
    Root = 1,
    Missions = 2,
    /// A mission was chosen: until the script has done with that.
    Closing = 3,
    Closed = 4,
}

/// The strip along the foot (the texts' + 0x28 to + 0x48).
#[derive(Clone, Debug, PartialEq)]
struct Strip {
    words: String,
    /// The text is wider than the strip, and goes along it.
    scrolls: bool,
    at: f32,
    start: f32,
    width: f32,
    speed: f32,
    /// The frames it still waits before it moves, and what that starts at.
    wait: i32,
    wait_from: i32,
    /// What is left between one writing of the text and the next.
    gap: i32,
}

/// The alpha of the panel down the right, and how it is on its way (the texts'
/// + 0x6C to + 0x7C).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Panel {
    alpha: f32,
    mode: i32,
    after: i32,
    count: i32,
    until: i32,
}

/// The world (0x64A88): where on its map the middle of what is shown is, and
/// how it is on its way to another place.
#[derive(Clone, Copy, Debug, PartialEq)]
struct World {
    now: [i32; 2],
    from: [i32; 2],
    to: [i32; 2],
    moving: bool,
    /// The frames it has been moving for, and those a move takes.
    frames: i32,
    over: i32,
    /// How big the map was last drawn: the stages' marks go by it.
    scale: f32,
}

impl World {
    /// Sets it moving to another place (0x64D18).
    fn go(&mut self, to: [i32; 2]) {
        (self.to, self.from) = (to, self.now);
        (self.moving, self.frames) = (true, 0);
    }

    /// A frame of it (0x64AFC).
    fn frame(&mut self) {
        if !self.moving {
            return;
        }
        self.frames += 1;
        if self.frames > self.over {
            (self.frames, self.moving) = (self.over, false);
        }
        if self.over <= 0 {
            self.now = self.to;
            return;
        }
        let part = (self.frames as f32 / self.over as f32).clamp(0.0, 1.0);
        for axis in 0..2 {
            self.now[axis] = ((self.to[axis] - self.from[axis]) as f32 * part + self.from[axis] as f32) as i32;
        }
    }
}

/// One of the scene's animations as the script has it played (0x66A0C with 1
/// for her model's, 0x646F8 for the camera's): which of the two it is, and
/// how far it has got.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Which it will be when it is next started, and which it is.
    chosen: usize,
    pub which: usize,
    /// It goes round rather than stopping at its end.
    looped: bool,
    frames: f32,
    pub playing: bool,
}

impl Motion {
    const AT_REST: Motion = Motion { chosen: 0, which: 0, looped: false, frames: 0.0, playing: false };

    fn start(&mut self) {
        (self.which, self.frames, self.playing) = (self.chosen, 0.0, true);
    }

    /// Where it will be so many frames on, and whether it will still be
    /// playing (0x234C8): one that goes round comes back from the end of
    /// what it repeats to its start, and one that does not stops at its
    /// end.
    fn on(&self, by: f32, length: f32, repeats: (f32, f32)) -> (f32, bool) {
        if !self.playing || by <= 0.0 {
            return (self.frames, self.playing);
        }
        let (first, last) = if self.looped { repeats } else { (length, length) };
        let mut frames = self.frames + by;
        while frames >= last {
            if first < last {
                frames = frames - last + first;
            } else {
                return (last, false);
            }
        }
        (frames, true)
    }

    fn advance(&mut self, length: f32, repeats: (f32, f32)) {
        (self.frames, self.playing) = self.on(1.0, length, repeats);
    }

    /// The frame it is at, so much of a frame past the last one.
    pub fn at(&self, ahead: f32, length: f32, repeats: (f32, f32)) -> f32 {
        self.on(ahead, length, repeats).0
    }
}

/// What the scene shows (the layout's group 47): her model and the camera
/// that sees it, each in one of its two animations; whether the story is
/// late, which says which animations those are; and whether the scene is
/// drawn at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene {
    pub her: Motion,
    pub camera: Motion,
    pub late: bool,
    pub drawn: bool,
}

/// The first place as it was when its picture was kept (0x29864): what turns
/// and moves in it stands still from then on.
#[derive(Clone, Debug, PartialEq)]
struct Kept {
    place: Player,
    rings: [i32; 2],
    world: World,
}

/// The screen's interface.
pub struct Interface {
    state: State,
    /// The stage can be changed (flag 0x200000 of its + 0x44).
    changes_stage: bool,
    places: [Player; 3],
    /// How each place is drawn (the script's + 0x78 on): 1 as it is, 2 as the
    /// picture that was kept, anything else not at all.
    modes: [i32; 3],
    /// The script has done with what it was told (its flag 1).
    ready: bool,
    /// A picture is to be kept after this place is drawn (its flag 2, + 0x84).
    picture_after: Option<usize>,
    kept: Option<Kept>,
    /// A stage is newly there (its flag 4).
    new_stage: bool,
    pub stages: Vec<Stage>,
    missions: Vec<Mission>,
    /// The missions of the stage shown, as places among all of them, and
    /// those of the six rows.
    listed: Vec<usize>,
    /// Where the missions' cursor and the list were left for each stage.
    left: [Option<(usize, usize)>; STAGES],
    /// The root's cursor, the stage shown and the one it was, and how many
    /// stages there are (the texts' + 0x4C, + 0x54, + 0x58, + 0x5C).
    cursor: usize,
    stage: usize,
    stage_was: usize,
    count: usize,
    strip: Strip,
    panel: Panel,
    /// The missions' cursor, and how far their list is scrolled.
    mission: usize,
    scroll: usize,
    /// The frames the rings have turned for (+ 0x556C, + 0x5570).
    rings: [i32; 2],
    world: World,
    /// The story is late (flag 0x180A as the screen was made), and what her
    /// model and the scene's camera are doing.
    late: bool,
    her: Motion,
    camera: Motion,
    script: Option<Machine>,
    /// The frames the script has waited in the wait it is in.
    waited: i32,
    /// What it has asked of the sound since that was last taken up.
    pub sounds: Vec<Request>,
}

/// What the interface works with: the game's progress and tables, and the
/// fonts its texts are measured by.
pub struct Game<'a> {
    pub progress: &'a mut Progress,
    pub tables: &'a FieldTables,
    pub fonts: &'a Fonts,
    pub colours: &'a TextColours,
}

impl Interface {
    /// The interface as it is made (0x61B40).
    pub fn new(assets: &Assets, world: &mut Game) -> Interface {
        let layout = &assets.layout;
        let stages = list_stages(world.progress, world.tables);
        let count = stages.iter().filter(|stage| stage.there).count();
        // The stage shown at first is the one a mission was last chosen of.
        let stage = usize::from(world.progress.hub_stage).min(count.saturating_sub(1));
        let late = world.progress.flag(LATE);
        let middle = assets.place_of(stage);
        let mut interface = Interface {
            state: State::Made,
            changes_stage: !late,
            places: [layout.player(NO_SET), layout.player(NO_SET), layout.player(NO_SET)],
            modes: [0; 3],
            ready: false,
            picture_after: None,
            kept: None,
            new_stage: false,
            stages,
            missions: Vec::new(),
            listed: Vec::new(),
            left: [None; STAGES],
            cursor: 0,
            stage,
            stage_was: 0,
            count,
            strip: Strip { words: String::new(), scrolls: false, at: 0.0, start: 0.0, width: 0.0, speed: 0.0, wait: 0, wait_from: 0, gap: 0 },
            panel: Panel { alpha: 1.0, mode: 0, after: 0, count: 0, until: 0 },
            mission: 0,
            scroll: 0,
            rings: [0; 2],
            world: World { now: middle, from: middle, to: middle, moving: false, frames: 0, over: 0, scale: 0.0 },
            late,
            her: Motion::AT_REST,
            camera: Motion::AT_REST,
            script: None,
            waited: 0,
            sounds: Vec::new(),
        };
        // A stage that is newly there: the one a mission was last chosen of
        // becomes the next (0x623D0), and the script shows the way there.
        if interface.stages.iter().any(|stage| stage.new) {
            interface.new_stage = true;
            world.progress.hub_stage = world.progress.hub_stage.wrapping_add(1);
        }
        interface
    }

    /// Whether the script has done with what it was told (0x61ADC).
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Whether the stage's missions cannot be come to, or it be left (0x61344).
    pub fn busy(&self) -> bool {
        matches!(self.state, State::Made | State::Closing)
    }

    /// What it answers (0x6168C): the root's cursor, the stage shown and how
    /// many there are, the missions' cursor and how many the stage has.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn stage(&self) -> usize {
        self.stage
    }

    pub fn stage_count(&self) -> usize {
        self.count
    }

    pub fn mission_cursor(&self) -> usize {
        self.mission
    }

    pub fn mission_count(&self) -> usize {
        self.listed.len()
    }

    /// The mission the cursor is on (0x6182C, 0x61878, 0x61640): its number,
    /// its title, and whether it can be begun.
    pub fn chosen(&self) -> Option<&Mission> {
        self.listed.get(self.mission).map(|&at| &self.missions[at])
    }

    fn sound(&mut self, sound: u32) {
        self.sounds.push(Request::common(sound));
    }

    /// Has the strip say something from its start (0x677B8 with nought,
    /// 0x67938): the text stands to the right of the strip's place if it has
    /// room, and goes along it if it is wider than the strip.
    fn say(&mut self, assets: &Assets, world: &Game, words: &str) {
        let room = assets.layout.objects.objects.get(object::STRIP).map_or(0.0, |strip| strip.size[0] as f32);
        let width = world.fonts.write(&STRIP_STYLE, &text::pages(words, &text::Plain).swap_remove(0), world.colours).size().x;
        let start = (room - width).max(0.0);
        self.strip = Strip { words: words.to_owned(), scrolls: (STRIP_ROOM as f32) < width, at: start, start, width, wait: self.strip.wait_from, ..self.strip.clone() };
    }

    /// The root's cursor is put on an entry (0x63BF8 with nought): the strip
    /// says the entry's text.
    pub fn set_cursor(&mut self, assets: &Assets, world: &Game, to: usize) {
        let to = to.min(ENTRIES - 1);
        if to != self.cursor {
            self.cursor = to;
            self.say(assets, world, &assets.strip[to].clone());
        }
    }

    /// Another stage is shown (0x63BF8 with 2): the world is set moving to it.
    fn show_stage(&mut self, assets: &Assets, to: usize) {
        let to = to.min(self.count.saturating_sub(1));
        if to != self.stage {
            (self.stage_was, self.stage) = (self.stage, to);
            self.world.go(assets.place_of(to));
        }
    }

    /// The missions' cursor is moved (0x63BF8 with 4): past either end it goes
    /// round to the other, and the list is scrolled to keep it among the rows.
    /// On another mission, what is told is made anew (0x61484), and with it
    /// what the strip says.
    pub fn set_mission(&mut self, assets: &Assets, world: &Game, to: i32) {
        let count = self.listed.len() as i32;
        if !self.changes_stage || count == 0 {
            return;
        }
        let to = if to < 0 { count - 1 } else if to >= count { 0 } else { to };
        let mut scroll = self.scroll as i32;
        if to < scroll {
            scroll = to;
        } else if to >= scroll + ROWS as i32 {
            scroll = to - (ROWS as i32 - 1);
        }
        self.scroll = scroll.clamp(0, (count - ROWS as i32).max(0)) as usize;
        if self.mission != to as usize {
            self.mission = to as usize;
            self.say(assets, world, &assets.strip[STRIP_AT_MISSIONS].clone());
        }
    }

    /// Lists the missions of the stage shown (0x639B0): the open ones, or once
    /// the story is late the one there is left; with the cursor on the first
    /// that has not been begun the first time, and after that where it was
    /// left.
    fn list_stage(&mut self, world: &Game) {
        let late = world.progress.flag(LATE);
        let of_it = |mission: &&Mission| mission.stage == self.stage && mission.open && (!late || mission.number == ONLY_MISSION);
        self.listed = self.missions.iter().enumerate().filter(|(_, mission)| of_it(mission)).map(|(at, _)| at).collect();
        let count = self.listed.len();
        let (mission, scroll) = self.left[self.stage].unwrap_or_else(|| {
            let first = self.listed.iter().position(|&at| self.missions[at].come_to == 1).unwrap_or(0);
            (first, first.saturating_sub(ROWS - 1))
        });
        self.mission = mission.min(count.saturating_sub(1));
        self.scroll = scroll.min(count.saturating_sub(ROWS));
        self.left[self.stage] = Some((self.mission, self.scroll));
    }

    /// Has the script run one of its functions (0x66D4C): it runs until it
    /// waits.
    fn run(&mut self, assets: &Assets, function: &str) {
        let Some(address) = assets.script.symbol(function) else {
            warn!("the stage select's script has no {function}");
            return;
        };
        let mut machine = Machine::new(&assets.script);
        self.waited = 0;
        let run = machine.start(address, &[], &mut Host { interface: self, assets });
        self.ran(machine, run);
    }

    fn ran(&mut self, machine: Machine, run: Result<Run>) {
        match run {
            Ok(Run::Waiting) => self.script = Some(machine),
            Ok(Run::Returned(_)) => self.script = None,
            Err(error) => {
                error!("the stage select's script failed: {error:#}");
                // Rather than leave the screen waiting for it for ever.
                (self.script, self.ready) = (None, true);
            }
        }
    }

    /// What it is told (0x64028).
    pub fn to_missions(&mut self, assets: &Assets, world: &Game) {
        if self.state == State::Root {
            self.state = State::Missions;
            self.list_stage(world);
            self.say(assets, world, &assets.strip[STRIP_AT_MISSIONS].clone());
            self.run(assets, "to_mission");
        }
    }

    pub fn to_root(&mut self, assets: &Assets, world: &Game) {
        if self.state == State::Missions {
            self.state = State::Root;
            self.left[self.stage] = Some((self.mission, self.scroll));
            self.say(assets, world, &assets.strip[self.cursor].clone());
            self.run(assets, "to_menu");
        }
    }

    /// A mission was chosen: the stage shown is the one to come back to.
    pub fn close(&mut self, assets: &Assets, world: &mut Game) {
        if self.state == State::Missions {
            let function = if world.progress.flag(LATE) { "close_m0305" } else { "close" };
            self.run(assets, function);
            world.progress.hub_stage = self.stage as u8;
            self.state = State::Closing;
        }
    }

    /// The next stage, or the one before: round the ends.
    pub fn change_stage(&mut self, assets: &Assets, on: bool) {
        if !self.changes_stage || self.count == 0 {
            return;
        }
        let to = if on { (self.stage + 1) % self.count } else { (self.stage + self.count - 1) % self.count };
        if to != self.stage {
            self.show_stage(assets, to);
            self.run(assets, if on { "next" } else { "prev" });
        }
    }

    /// The stages and the missions are listed anew.
    pub fn relist(&mut self, world: &mut Game) {
        self.stages = list_stages(world.progress, world.tables);
        self.count = self.stages.iter().take_while(|stage| stage.there).count();
        self.missions = list_missions(world.progress, world.tables);
    }

    /// It is made to come (0x5D670): the script's `open`, and the root's
    /// cursor on the stage.
    pub fn open(&mut self, assets: &Assets, world: &mut Game) {
        self.missions = list_missions(world.progress, world.tables);
        // Her model is made of what she has on, and its animation started
        // (0x66A0C with nought).
        self.her.start();
        self.run(assets, "open");
        self.cursor = ENTRIES;
        self.set_cursor(assets, world, entry::STAGE);
    }

    /// What the scene shows.
    pub fn scene(&self) -> Scene {
        let drawn = matches!(self.modes[1], 1 | 2) && self.places[1].track_of(group::SCENE).is_some();
        Scene { her: self.her, camera: self.camera, late: self.late, drawn }
    }

    /// A frame of it (0x6246C).
    pub fn frame(&mut self, assets: &Assets) {
        match self.state {
            State::Made => self.state = State::Root,
            State::Closing if self.script.is_none() => self.state = State::Closed,
            _ => {}
        }
        // The strip (0x67E38): once its wait is out a text that scrolls moves
        // on, and comes round when it is its width and the gap past the left.
        let strip = &mut self.strip;
        if strip.scrolls {
            strip.wait -= 1;
            if strip.wait < 0 {
                strip.wait = 0;
                if strip.width > 0.0 {
                    let round = strip.width + strip.gap as f32;
                    strip.at += strip.speed;
                    while strip.at < -round {
                        strip.at += round;
                    }
                }
            }
        }
        // The panel's alpha.
        let panel = &mut self.panel;
        if matches!(panel.mode, 1 | 2) {
            panel.count = (panel.count + 1).min(panel.until);
            let span = panel.until - panel.after;
            let part = if span > 0 { (panel.count - panel.after) as f32 / span as f32 } else { 1.0 };
            panel.alpha = if panel.mode == 1 { part } else { 1.0 - part }.clamp(0.0, 1.0);
            if panel.count >= panel.until {
                panel.mode = 0;
            }
        }
        // The scene's camera and her model (0x6446C, 0x66160).
        let camera = assets.camera_clip(self.late, self.camera.which);
        self.camera.advance(camera.frames(), (0.0, camera.frames()));
        let her = assets.her_clip(self.late, self.her.which);
        self.her.advance(her.frames(), her.loop_range());
        self.world.frame();
        // The script goes on (0x66D00).
        if let Some(mut machine) = self.script.take() {
            let run = machine.resume(&mut Host { interface: self, assets });
            self.ran(machine, run);
        }
        for place in &mut self.places {
            place.advance(&assets.layout, 1.0);
        }
        for (ring, frames) in self.rings.iter_mut().zip(RING_FRAMES) {
            *ring += 1;
            if *ring >= frames {
                *ring = 0;
            }
        }
        // How big the map is drawn is how big the track of its group has it.
        let place = &self.places[0];
        let earth = place.track_of(group::WORLD).and_then(|track| assets.layout.placed(place, track));
        if let Some((moved, _)) = earth {
            self.world.scale = moved.matrix2.x_axis.x;
        }
        // The picture the script asked for is taken as this frame is drawn.
        if let Some(place) = self.picture_after.take() {
            if place == 0 {
                self.kept = Some(Kept { place: self.places[0].clone(), rings: self.rings, world: self.world });
            }
        }
    }
}

/// What tests look into it by.
#[cfg(test)]
impl Interface {
    /// The set each place was last given.
    pub fn places_sets(&self) -> [usize; 3] {
        [self.places[0].set(), self.places[1].set(), self.places[2].set()]
    }

    pub fn modes(&self) -> [i32; 3] {
        self.modes
    }

    pub fn strip_words(&self) -> &str {
        &self.strip.words
    }

    pub fn stage_was(&self) -> usize {
        self.stage_was
    }

    /// The place of the world's map that is at the middle.
    pub fn world_now(&self) -> [i32; 2] {
        self.world.now
    }
}

#[cfg(test)]
impl Assets {
    pub fn tiles(&self) -> &[Handle<Image>] {
        &self.tiles
    }
}

/// The engine functions of the screen's script (the table at 0x1711F8).
struct Host<'a> {
    interface: &'a mut Interface,
    assets: &'a Assets,
}

impl script::Host for Host<'_> {
    fn native(&mut self, number: u32, call: &Call) -> Result<u32> {
        let interface = &mut *self.interface;
        let layout = &self.assets.layout;
        Ok(match number {
            // The scene's camera and her model start the animation that was
            // chosen for them; the effect, which is not ported; that it is
            // ready, or is not; and a picture to be kept after a place is
            // drawn.
            1 => {
                match call.int(0)? {
                    0 => interface.camera.start(),
                    2 => interface.her.start(),
                    3 => interface.ready = true,
                    4 => interface.ready = false,
                    place @ 5..=7 => interface.picture_after = Some((place - 5) as usize),
                    _ => {}
                }
                0
            }
            2 => {
                let (which, value) = (call.int(0)?, call.int(1)?);
                match which {
                    2 => interface.her.chosen = usize::try_from(value).unwrap_or(0),
                    3 => interface.her.looped = value != 0,
                    6 => interface.camera.chosen = usize::try_from(value).unwrap_or(0),
                    7 => interface.camera.looped = value != 0,
                    8 => interface.strip.wait_from = value.max(0),
                    9 => interface.strip.gap = value.max(0),
                    10 => interface.world.over = value.max(0),
                    11..=13 => {
                        let place = (which - 11) as usize;
                        interface.modes[place] = value;
                        if place == 0 && value != 2 {
                            interface.kept = None;
                        }
                    }
                    14 => interface.show_stage(self.assets, usize::try_from(value).unwrap_or(0)),
                    _ => {}
                }
                0
            }
            3 => {
                if call.int(0)? == 9 {
                    interface.strip.speed = -call.float(1)?.abs();
                }
                0
            }
            // Waits so many frames (0x6710C).
            4 => {
                let frames = call.int(0)?.max(0);
                if call.resumed() {
                    interface.waited += 1;
                    if interface.waited < frames {
                        call.wait();
                    }
                } else {
                    interface.waited = 0;
                    if frames > 0 {
                        call.wait();
                    }
                }
                0
            }
            // A place plays a set (0x671D4), and is waited for (0x67230).
            5 => {
                let (place, set) = (call.int(0)?, call.int(1)?);
                if let (Some(place), Ok(set)) = (usize::try_from(place).ok().and_then(|place| interface.places.get_mut(place)), usize::try_from(set)) {
                    place.play(layout, set);
                }
                0
            }
            6 => {
                let place = usize::try_from(call.int(0)?).ok().and_then(|place| interface.places.get(place));
                if place.is_some_and(|place| !place.finished(layout)) {
                    call.wait();
                }
                0
            }
            // The panel's alpha is set going (0x67BA4).
            7 => {
                let (mode, over, after) = (call.int(0)?, call.int(1)?, call.int(2)?);
                interface.panel = Panel { mode, after, count: 0, until: over + after, ..interface.panel };
                0
            }
            // Her model's animation is waited for (0x67364).
            8 => {
                if interface.her.playing {
                    call.wait();
                }
                0
            }
            9 => match call.int(0)? {
                14 => interface.stage as u32,
                15 => interface.stage_was as u32,
                16 => u32::from(interface.new_stage),
                _ => 0,
            },
            11 => {
                if let Some(&sound) = usize::try_from(call.int(0)?).ok().and_then(|which| SCRIPT_SOUNDS.get(which)) {
                    interface.sound(sound);
                }
                0
            }
            _ => 0,
        })
    }
}

/// What the interface is drawn with.
pub struct Ink<'a> {
    pub fonts: &'a Fonts,
    pub colours: &'a TextColours,
}

impl Interface {
    /// Writes a text at one of the layout's objects (0x67994, 0x655D0): in the
    /// object's box, moved as a track has it, in the track's colour by the
    /// object's, with no shadow.
    #[allow(clippy::too_many_arguments)]
    fn write(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, style: &Style, words: &str, object: usize, moved: Vec2, colour: [u8; 4], align: Align) {
        let layout = &assets.layout;
        let (Some(place), Some(of)) = (layout.place(object), layout.objects.objects.get(object)) else { return };
        if words.is_empty() {
            return;
        }
        let block = ink.fonts.write(style, &text::pages(words, &text::Plain).swap_remove(0), ink.colours);
        let placing = Placing { width: of.size[0] as f32, align, tint: modulate(colour, layout.colour(object)), ..Placing::at(place + moved.trunc()) };
        ink.fonts.draw(canvas, layer::HUB, &block, &placing);
    }

    /// Draws a piece of one of the layout's textures (0x32AD4): so much of it
    /// from a corner, at a place.
    fn sprite(&self, canvas: &mut Canvas, assets: &Assets, texture: usize, at: Vec2, from: Vec2, size: Vec2, colour: [u8; 4]) {
        let Some(handle) = assets.layout.textures.get(texture) else { return };
        let whole = assets.size(texture);
        canvas.sprite(layer::HUB, Paint::textured(handle), Vertex::new(at, from / whole, colour), Vertex::new(at + size, (from + size) / whole, colour));
    }

    /// The word for a stage with its number (the script's first `local_str`).
    fn stage_word(&self, assets: &Assets, stage: usize) -> String {
        assets.local[0].replace("%s", &full_width(stage as i32 + 1))
    }

    /// Draws the word for a stage with the stage's mark before it (0x68000,
    /// and 0x6852C with 8): set to the right at one object, or centred at one
    /// of two by whether there is a mark.
    #[allow(clippy::too_many_arguments)]
    fn draw_stage_word(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, stage: usize, moved: Vec2, mut colour: [u8; 4], centred: bool) {
        let layout = &assets.layout;
        let Some(of) = self.stages.get(stage) else { return };
        let mark = STAGE_MARKS.get(of.mark).copied().flatten();
        if let Some(from) = mark {
            let at = if centred { object::CHOICE_MARK } else { object::STAGE_WORD_MARK };
            if let Some(place) = layout.place(at) {
                // The mark is in its object's colour at the track's alpha, and
                // so is the word after it.
                let own = layout.colour(at);
                colour = [own[0], own[1], own[2], (own[3] as u32 * colour[3] as u32 / 255) as u8];
                self.sprite(canvas, assets, texture::MARKS, place + moved, from, Vec2::splat(MARK_SIZE), colour);
            }
        }
        let words = self.stage_word(assets, stage);
        let (at, align) = match (centred, mark.is_some()) {
            (false, _) => (object::STAGE_WORD, Align::Right),
            (true, false) => (object::CHOICE, Align::Centre),
            (true, true) => (object::CHOICE_MARKED, Align::Centre),
        };
        self.write(canvas, assets, ink, &STAGE_WORD_STYLE, &words, at, moved, colour, align);
    }

    /// Draws what is told of the stage shown (0x6852C with 1): its second
    /// text, its number as a big figure, and how many of its missions are
    /// cleared and how many are open, as two small figures each.
    fn draw_stage_told(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, moved: Vec2, colour: [u8; 4]) {
        let layout = &assets.layout;
        let (Some(of), Some(texts)) = (self.stages.get(self.stage), assets.stages.get(self.stage)) else { return };
        self.write(canvas, assets, ink, &SECOND_TEXT_STYLE, &texts.second, object::SECOND_TEXT, moved, colour, Align::Left);
        let faded = |object: usize| {
            let own = layout.colour(object);
            [own[0], own[1], own[2], (own[3] as u32 * colour[3] as u32 / 255) as u8]
        };
        if let Some(place) = layout.place(object::STAGE_NUMBER) {
            let from = Vec2::new(20.0 * (self.stage as f32 + 1.0), 0.0);
            self.sprite(canvas, assets, texture::FIGURES, place + moved, from, Vec2::splat(20.0), faded(object::STAGE_NUMBER));
        }
        if let Some(place) = layout.place(object::COUNTS) {
            let tint = modulate(colour, layout.colour(object::COUNTS));
            for (row, count) in [of.cleared, of.open].into_iter().enumerate() {
                let figures = format!("{:02}", count.clamp(0, 99));
                for (across, figure) in figures.chars().take(2).enumerate() {
                    let Some(figure) = figure.to_digit(10) else { continue };
                    let at = place + moved + Vec2::new(6.0 * across as f32, 10.0 * row as f32);
                    self.sprite(canvas, assets, texture::SMALL_FIGURES, at, Vec2::new(5.0 * figure as f32, 0.0), Vec2::splat(5.0), tint);
                }
            }
        }
    }

    /// Draws the strip's text (0x6852C with nought): cut to the strip's place,
    /// and while it scrolls written again and again until the place is full.
    fn draw_strip(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, moved: Vec2, colour: [u8; 4]) {
        let layout = &assets.layout;
        let strip = &self.strip;
        let (Some(place), Some(of)) = (layout.place(object::STRIP), layout.objects.objects.get(object::STRIP)) else { return };
        if strip.width <= 0.0 {
            return;
        }
        let room = Vec2::new(of.size[0] as f32, of.size[1] as f32);
        canvas.cut_to(Some(Rect::from_corners(place, place + room)));
        let mut at = strip.at;
        loop {
            self.write(canvas, assets, ink, &STRIP_STYLE, &strip.words, object::STRIP, moved + Vec2::new(at, 0.0), colour, Align::Left);
            at += strip.width + strip.gap as f32;
            if !strip.scrolls || at >= room.x {
                break;
            }
        }
        canvas.cut_to(None);
    }

    /// Draws the title of one of the list's rows, and the mark of what its
    /// mission has come to (0x65ADC with the row).
    fn draw_row(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, row: usize, moved: Vec2, mut colour: [u8; 4]) {
        let layout = &assets.layout;
        let at = row + self.scroll;
        let mission = self.listed.get(at).map(|&of| &self.missions[of]);
        let down = Vec2::new(0.0, PITCH * row as f32);
        // A row past the last mission is fainter, and has no mark.
        if mission.is_none() {
            colour[3] = (colour[3] as f32 * PAST_THE_LAST) as u8;
        }
        let object = if at == self.mission { object::ROW_TITLE_AT_CURSOR } else { object::ROW_TITLE };
        let title = mission.map_or(NO_TITLE, |mission| mission.title.as_str());
        self.write(canvas, assets, ink, &TITLE_STYLE, title, object, moved.trunc() + down, colour, Align::Left);
        let Some(mission) = mission else { return };
        if let (Some(from), Some(place)) = (COME_TO_MARKS.get(mission.come_to).copied().flatten(), layout.place(object::ROW_MARK)) {
            self.sprite(canvas, assets, texture::MARKS, place + moved + down, from, Vec2::splat(MARK_SIZE), colour);
        }
    }

    /// Draws who the cursor's mission is from and what is told of it (0x63554).
    fn draw_told(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, moved: Vec2, colour: [u8; 4]) {
        let layout = &assets.layout;
        let Some(mission) = self.chosen() else { return };
        // The faces stand five to a row in their texture.
        if let (Some(place), Some(of)) = (layout.place(object::FACE), layout.objects.objects.get(object::FACE)) {
            let size = Vec2::new(of.size[0] as f32, of.size[1] as f32);
            let (across, down) = (mission.from % 5, mission.from / 5);
            let from = Vec2::new((size.x + 1.0) * across as f32 + 1.0, size.y * down as f32 + if matches!(down, 1 | 2) { 1.0 } else { 0.0 });
            let own = layout.colour(object::FACE);
            let tint = [own[0], own[1], own[2], (own[3] as u32 * colour[3] as u32 / 255) as u8];
            self.sprite(canvas, assets, texture::FACES, (place + moved).trunc(), from, size, tint);
        }
        let kind = assets.local.get(2 + mission.kind).map_or("", String::as_str);
        self.write(canvas, assets, ink, &KIND_STYLE, kind, object::KIND, moved, colour, Align::Centre);
        self.write(canvas, assets, ink, &BRIEFING_STYLE, &mission.briefing, object::BRIEFING, moved, colour, Align::Left);
        self.write(canvas, assets, ink, &TWO_LINES_STYLE, &mission.hint, object::HINT, moved, colour, Align::Left);
        self.write(canvas, assets, ink, &TWO_LINES_STYLE, &mission.objective, object::OBJECTIVE, moved, colour, Align::Left);
        if mission.level > 0 {
            self.write(canvas, assets, ink, &LEVEL_STYLE, &mission.level.to_string(), object::LEVEL, moved, colour, Align::Right);
        }
    }

    /// Draws the world (0x64E0C): the tiles of its map, so much bigger or
    /// smaller, with the place it has come to at the middle; and over them the
    /// lines of its grid, from edge to edge of the screen.
    fn draw_world(&self, canvas: &mut Canvas, assets: &Assets, world: &World, scale: f32, colour: [u8; 4]) {
        let layout = &assets.layout;
        let Some(middle) = layout.place(object::WORLD_MIDDLE) else { return };
        let origin = middle - Vec2::new(world.now[0] as f32, world.now[1] as f32) * scale;
        let tint = modulate(colour, layout.colour(object::EARTH));
        let (pitch, across) = (TILE * scale, assets.map.across.max(1));
        let size = Vec2::splat(pitch.trunc());
        for (cell, &tile) in assets.map.tiles.iter().enumerate() {
            let Some(texture) = usize::try_from(tile).ok().and_then(|tile| assets.tiles.get(tile)) else { continue };
            let at = origin + Vec2::new(pitch * (cell % across) as f32, pitch * (cell / across) as f32);
            // A tile that is off the screen is not drawn.
            if at.x + pitch < 0.0 || at.x > WIDTH || at.y + pitch < 0.0 || at.y > HEIGHT {
                continue;
            }
            canvas.sprite(layer::HUB, Paint::textured(texture), Vertex::new(at, Vec2::ZERO, tint), Vertex::new(at + size, Vec2::ONE, tint));
        }
        let lines = modulate(assets.map.lines, colour);
        for &latitude in &assets.map.latitudes {
            let down = (latitude as f32 * scale + origin.y).floor();
            canvas.fill(layer::HUB, Rect::new(0.0, down, WIDTH, down + 1.0), lines);
        }
        for &longitude in &assets.map.longitudes {
            let across = (longitude as f32 * scale + origin.x).floor();
            canvas.fill(layer::HUB, Rect::new(across, 0.0, across + 1.0, HEIGHT), lines);
        }
    }

    /// Draws the marks of the stages a track of theirs is for (0x628DC,
    /// 0x62B3C), each where its stage is on the world's map (0x64D7C), with
    /// the stage's name by it. The group has three tracks: the first is for
    /// the stage shown, the second for the one it was, the third for the
    /// others. The fourth stage has a group of its own, whose name is set to
    /// the right; the Moon has no mark.
    #[allow(clippy::too_many_arguments)]
    fn draw_marks(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, player: &Player, track: usize, number: usize, world: &World, scale: f32) {
        let layout = &assets.layout;
        let fourth = player.group(track) == Some(group::FOURTH_MARK);
        let (mark, name, align) = if fourth { (object::FOURTH_MARK, object::FOURTH_NAME, Align::Right) } else { (object::MARK, object::NAME, Align::Left) };
        let (Some((moved, colour)), Some(mark), Some(middle)) = (layout.placed(player, track), layout.place(mark), layout.place(object::WORLD_MIDDLE)) else { return };
        for stage in 0..self.count {
            let of = if stage == self.stage { 0 } else if stage == self.stage_was { 1 } else { 2 };
            if of != number || fourth != (stage == FOURTH_STAGE) || stage == THE_MOON {
                continue;
            }
            let Some(texts) = assets.stages.get(stage) else { continue };
            let on_screen = middle + (texts.at.trunc() - Vec2::new(world.now[0] as f32, world.now[1] as f32)) * scale;
            let by = on_screen - mark;
            layout.draw_track(canvas, layer::HUB, player, track, &Look::inner(Affine2::from_translation(by)));
            self.write(canvas, assets, ink, &SECOND_TEXT_STYLE, &texts.second, name, moved.translation + by, colour, align);
        }
    }

    /// Draws a track with a texture of the layout's changed for another: the
    /// picture of the stage shown.
    fn draw_with_picture(&self, canvas: &mut Canvas, assets: &Assets, player: &Player, track: usize, look: &Look) {
        let layout = &assets.layout;
        let (Some((moved, colour)), Some(group)) = (layout.placed(player, track), player.group(track)) else { return };
        let colour = modulate(colour, look.tint);
        if colour[3] == 0 {
            return;
        }
        let mut textures = layout.textures.clone();
        if let (Some(place), Some(picture)) = (textures.get_mut(PICTURE_TEXTURE), assets.pictures.get(self.stage)) {
            *place = picture.clone();
        }
        for &object in layout.groups.groups.get(group).map_or(&[][..], Vec::as_slice) {
            if let Some(object) = layout.objects.objects.get(object) {
                draw_object(canvas, layer::HUB, object, &textures, moved * look.inner, colour, look.outer, look.shift);
            }
        }
    }

    /// Draws the interface (0x62590) as it is `ahead` of a frame past its last
    /// one: each of the three places track by track, by what the track's group
    /// is, and with each track the texts that go with its group.
    pub fn draw(&self, canvas: &mut Canvas, assets: &Assets, ink: &Ink, scene: bool, ahead: f32) {
        let layout = &assets.layout;
        for (number, (place, &mode)) in self.places.iter().zip(&self.modes).enumerate() {
            // A place whose picture was kept is drawn as it was then.
            let (mut player, rings, world) = match (&self.kept, mode) {
                (Some(kept), 2) if number == 0 => (kept.place.clone(), kept.rings, kept.world),
                (_, 1 | 2) => (place.clone(), self.rings, self.world),
                _ => continue,
            };
            if !(mode == 2 && number == 0 && self.kept.is_some()) {
                player.advance(layout, ahead);
            }
            let mut stage_words = 0;
            // How many tracks of the stages' marks, and of the fourth
            // stage's, have had their turn; and how big the map is.
            let mut marks = [0; 2];
            let mut scale = world.scale;
            for track in 0..player.len() {
                let Some(group) = player.group(track) else { continue };
                let white = Look::default();
                let down = |by: f32| Look::inner(Affine2::from_translation(Vec2::new(0.0, by)));
                let of_cursor = self.chosen();
                match group {
                    // Not the group's own picture but the world, as big as
                    // the track has the group and in its colour.
                    group::WORLD => {
                        if let Some((moved, colour)) = layout.placed(&player, track) {
                            scale = moved.matrix2.x_axis.x;
                            self.draw_world(canvas, assets, &world, scale, colour);
                        }
                    }
                    group::MARKS | group::FOURTH_MARK => {
                        let number = &mut marks[usize::from(group == group::FOURTH_MARK)];
                        self.draw_marks(canvas, assets, ink, &player, track, *number, &world, scale);
                        *number += 1;
                    }
                    // The scene her model stands in, if its camera has drawn
                    // it: the picture of it has its place here.
                    group::SCENE => {
                        if scene {
                            canvas.scene_here(layer::HUB);
                        }
                    }
                    group if group::ARROWS.contains(&group) => {
                        if self.cursor == entry::STAGE {
                            let alpha = if self.count < 2 || !self.changes_stage { ARROWS_OFF } else { 255 };
                            layout.draw_track(canvas, layer::HUB, &player, track, &Look::tinted([255, 255, 255, alpha]));
                        }
                    }
                    group::ENTRY_BAR => layout.draw_track(canvas, layer::HUB, &player, track, &down(PITCH * self.cursor as f32)),
                    group::ON_STAGE if self.cursor != entry::STAGE => {}
                    group::ON_SAVE if self.cursor != entry::SAVE => {}
                    group::ON_LOAD if self.cursor != entry::LOAD => {}
                    group::ON_MENU if self.cursor != entry::MENU => {}
                    group::ROW_CURSOR => layout.draw_track(canvas, layer::HUB, &player, track, &down(PITCH * (self.mission as f32 - self.scroll as f32))),
                    group::ROW_BARS => {
                        for row in 0..ROWS {
                            let mission = self.listed.get(row + self.scroll).map(|&at| &self.missions[at]);
                            let tint = mission.map_or(NO_MISSION, |mission| KIND_COLOURS[mission.kind.min(2)]);
                            layout.draw_track(canvas, layer::HUB, &player, track, &Look { tint, ..down(PITCH * row as f32) });
                        }
                    }
                    group::KIND_BAR => {
                        let tint = of_cursor.map_or([255; 4], |mission| KIND_COLOURS[mission.kind.min(2)]);
                        layout.draw_track(canvas, layer::HUB, &player, track, &Look::tinted(tint));
                    }
                    group::KNOB => {
                        let count = self.listed.len();
                        let height = layout.objects.objects.get(object::KNOB).map_or(0.0, |knob| knob.size[1] as f32);
                        if count > ROWS && height > 0.0 {
                            let each = KNOB_RUN / count as f32;
                            let inner = Affine2::from_scale_angle_translation(Vec2::new(1.0, each * ROWS as f32 / height), 0.0, Vec2::new(0.0, each * self.scroll as f32));
                            layout.draw_track(canvas, layer::HUB, &player, track, &Look::inner(inner));
                        }
                    }
                    group::LEVEL if !of_cursor.is_some_and(|mission| mission.level > 0) => {}
                    group::NO_LEVEL if !of_cursor.is_some_and(|mission| mission.level <= 0) => {}
                    group if group::PANEL.contains(&group) => {
                        let cut = PANEL_CUTS.iter().find(|(of, _)| *of == group).and_then(|&(_, to)| {
                            let object = layout.objects.objects.get(to)?;
                            let from = Vec2::new(object.position[0] as f32, object.position[1] as f32);
                            Some(Rect::from_corners(from, from + Vec2::new(object.size[0] as f32, object.size[1] as f32)))
                        });
                        if cut.is_some() {
                            canvas.cut_to(cut);
                        }
                        layout.draw_track(canvas, layer::HUB, &player, track, &Look::tinted([255, 255, 255, (self.panel.alpha * 255.0) as u8]));
                        if cut.is_some() {
                            canvas.cut_to(None);
                        }
                    }
                    group::RING | group::RING_BACK => {
                        let which = usize::from(group == group::RING_BACK);
                        let turn = rings[which] as f32 / RING_FRAMES[which] as f32 * std::f32::consts::TAU;
                        let angle = if which == 0 { -turn } else { turn };
                        layout.draw_track(canvas, layer::HUB, &player, track, &Look::inner(Affine2::from_angle(angle)));
                    }
                    group::STAGE_TOLD if !player.hidden(track) => self.draw_with_picture(canvas, assets, &player, track, &white),
                    _ => {
                        if !player.hidden(track) || ALWAYS_DRAWN.contains(&group) {
                            layout.draw_track(canvas, layer::HUB, &player, track, &white);
                        }
                    }
                }
                // The texts that go with the track's group, where the track is.
                let Some((at, colour)) = layout.placed(&player, track) else { continue };
                let moved = at.translation;
                match group {
                    group::STAGE_TOLD => self.draw_stage_told(canvas, assets, ink, moved, colour),
                    group::STAGE_WORD => self.draw_stage_word(canvas, assets, ink, self.stage, moved, colour, false),
                    group::STAGE_CHOICE => {
                        if self.cursor == entry::STAGE {
                            match stage_words {
                                0 => self.draw_stage_word(canvas, assets, ink, self.stage, moved, colour, true),
                                1 => self.draw_stage_word(canvas, assets, ink, self.stage_was, moved, colour, true),
                                _ => {}
                            }
                        }
                        stage_words += 1;
                    }
                    group::STRIP => self.draw_strip(canvas, assets, ink, moved, colour),
                    group::TITLES => {
                        for row in (0..ROWS).filter(|&row| row + self.scroll != self.mission) {
                            self.draw_row(canvas, assets, ink, row, moved, colour);
                        }
                    }
                    group::TITLE_AT_CURSOR => {
                        if let Some(row) = self.mission.checked_sub(self.scroll).filter(|&row| row < ROWS) {
                            self.draw_row(canvas, assets, ink, row, moved, colour);
                        }
                    }
                    group::FROM => self.draw_told(canvas, assets, ink, moved, colour),
                    group::HEADER => {
                        if let Some(texts) = assets.stages.get(self.stage) {
                            let words = assets.local[1].replacen("%s", &full_width(self.stage as i32 + 1), 1).replacen("%s", &texts.name, 1);
                            self.write(canvas, assets, ink, &HEADER_STYLE, &words, object::HEADER, moved, colour, Align::Left);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

}
