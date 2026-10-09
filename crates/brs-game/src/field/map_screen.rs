//! The map of the stage, which square brings up on the field: the original's
//! task "ifFldMiniMap" (0x42D44 to 0x44614), the field's part in it (0xF30A4 to
//! 0xF3478) and the functions the maps' scripts set it by (0xC6830 on).
//!
//! Nothing of the map is worked out from where she is. A stage's map is a layout
//! of its own (`IF/EVT/MINI_MAP_STAGEnn.GZ`), with an object for each place, for
//! each way between two and for each name, and what is shown of it is in the
//! save, where the maps' scripts put it as each map is come into
//! (`SET_MMAP_STGnn`): which pieces are known, which are not to be seen at all,
//! the one or two she is on, the pins of the home points and where the mission
//! is. A table that comes with the layout says of each track of its first set
//! what kind of thing it is and which of the save's numbers it goes by.
//!
//! Round it goes a frame that is the same for every stage: the dark over the
//! field, the panel, the word for closing it, the pin and the two marks, and
//! seven squares that are never drawn, whose colours are the ones the map's
//! pieces are drawn in (so that the piece she is on can pulse as its square
//! does).

use anyhow::{ensure, Context, Result};
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{modulate, Layout, Look, Player};
use brs_formats::xtc;

use crate::controls::{button, Pad};
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::progress::Progress;
use crate::sound::Request;

/// The numbers of the save that the map is kept in (0xC6830 on, 0x432C0 on).
pub mod kept {
    /// Flags, one for each number of the stage's table from these on: what is
    /// known (`s267`), and what is not to be seen at all (`s271`).
    pub const KNOWN: i32 = 0x157c;
    pub const HIDDEN: i32 = 0x15f9;
    /// The first flag after them: all from `KNOWN` to here are cleared together.
    pub const FLAGS_END: i32 = 0x176f;
    /// The pieces she is on (`s270`), `NOT_A_PIECE` for none.
    pub const HERE: [i32; 3] = [0, 1, 2];
    pub const NOT_A_PIECE: i32 = 0xff;
    /// The marks (`s269` the first, `s278` any): the kind of each, and where it
    /// is on the map.
    pub const MARK_KINDS: [i32; 5] = [3, 4, 5, 6, 7];
    pub const MARKS: [[i32; 2]; 5] = [[0x4fd1, 0x4fd2], [0x4fdb, 0x4fdc], [0x4fdd, 0x4fde], [0x4fdf, 0x4fe0], [0x4fe1, 0x4fe2]];
    /// The pins (`s268`).
    pub const PINS: [[i32; 2]; 4] = [[0x4fd3, 0x4fd4], [0x4fd5, 0x4fd6], [0x4fd7, 0x4fd8], [0x4fd9, 0x4fda]];
    /// What a pin or a mark that is nowhere has for both its numbers.
    pub const NOWHERE: i32 = 0xffff;
}

/// `s266()`: the map as nothing has set it (0xC6830): she is on no piece, there
/// is no mark and no pin, and nothing is known or hidden.
pub fn reset(progress: &mut Progress) {
    for here in kept::HERE {
        progress.set_variable(here, kept::NOT_A_PIECE);
    }
    for kind in kept::MARK_KINDS {
        progress.set_variable(kind, 0);
    }
    for number in kept::MARKS.into_iter().chain(kept::PINS).flatten() {
        progress.set_variable(number, kept::NOWHERE);
    }
    for flag in kept::KNOWN..kept::FLAGS_END {
        progress.set_flag(flag, false);
    }
}

/// `s267(number, on)`: something of the map is known, or is not (0xC69A8).
pub fn know(progress: &mut Progress, number: i32, on: bool) {
    progress.set_flag(kept::KNOWN + number, on);
}

/// `s271(number, on)`: something of the map is not to be seen at all (0xC69C8).
pub fn hide(progress: &mut Progress, number: i32, on: bool) {
    progress.set_flag(kept::HIDDEN + number, on);
}

/// `s270(which, piece)`: one of the pieces she is on; -1 for none (0xC6CC8).
pub fn here(progress: &mut Progress, which: i32, piece: i32) {
    if let Some(&number) = usize::try_from(which).ok().and_then(|which| kept::HERE.get(which)) {
        progress.set_variable(number, if piece == -1 { kept::NOT_A_PIECE } else { piece });
    }
}

/// `s268(pin, x, y)`: where a pin is; -1 and -1 for nowhere (0xC69E8).
pub fn pin(progress: &mut Progress, pin: i32, x: i32, y: i32) {
    if let Some(numbers) = usize::try_from(pin).ok().and_then(|pin| kept::PINS.get(pin)) {
        put(progress, numbers, x, y);
    }
}

/// `s278(mark, x, y)`, and `s269(x, y)` for the first: where a mark is (0xC6AC8,
/// 0xC6C78).
pub fn mark(progress: &mut Progress, mark: i32, x: i32, y: i32) {
    if let Some(numbers) = usize::try_from(mark).ok().and_then(|mark| kept::MARKS.get(mark)) {
        put(progress, numbers, x, y);
    }
}

/// `s279(mark, kind)`: which of the two marks one is (0xC6BC8). No script calls
/// it: every mark is of the first kind, the mission's.
pub fn mark_kind(progress: &mut Progress, mark: i32, kind: i32) {
    if let Some(&number) = usize::try_from(mark).ok().and_then(|mark| kept::MARK_KINDS.get(mark)) {
        progress.set_variable(number, kind);
    }
}

fn put(progress: &mut Progress, numbers: &[i32; 2], x: i32, y: i32) {
    // Only both at -1 is nowhere.
    let (x, y) = if x & y == -1 { (kept::NOWHERE, kept::NOWHERE) } else { (x, y) };
    progress.set_variable(numbers[0], x);
    progress.set_variable(numbers[1], y);
}

/// The stages that have a map (0x42E8C).
const STAGES: usize = 6;
/// The frames of a layout that go by in one of the game's.
const FRAMES_A_STEP: f32 = 2.0;
/// The common bank's sounds as it comes and as it goes (0x4305C, 0x430A4).
const SOUND_COMES: u32 = 0;
const SOUND_GOES: u32 = 1;

/// The sets of the frame's layout (0x43270 on): it comes, it goes, a pin, the
/// mark of where the mission is, the squares of the colours, the other mark.
mod frame_set {
    pub const COMING: usize = 0;
    pub const GOING: usize = 1;
    pub const PIN: usize = 2;
    pub const MISSION: usize = 3;
    pub const COLOURS: usize = 4;
    pub const OTHER: usize = 5;
}

/// The sets of a stage's layout: its pieces, and the stage's name.
mod map_set {
    pub const PIECES: usize = 0;
    pub const NAME: usize = 1;
}

/// The squares of the colours are the seven groups of the frame's from this one
/// (0x43E60), and this is what each is the colour of (0x44284 to 0x4444C).
const FIRST_COLOUR: usize = 7;
const COLOURS: usize = 7;
mod colour {
    pub const NAME_KNOWN: usize = 0;
    pub const NAME: usize = 1;
    pub const WAY_KNOWN: usize = 2;
    pub const WAY: usize = 3;
    pub const HERE: usize = 4;
    pub const PLACE_KNOWN: usize = 5;
    pub const PLACE: usize = 6;
}

/// What a track of a stage's pieces is (the stage's table): its kind, 1 a way
/// between places, 2 a name and anything else a place; and the number it has
/// among the save's flags, none if it is below nought.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub kind: i16,
    pub number: i16,
}

/// A stage's map: the frame, the map's own layout and its table.
struct Stage {
    frame: Layout,
    map: Layout,
    pieces: Vec<Piece>,
}

impl Stage {
    fn load(data: &GameData, images: &mut Assets<Image>, stage: usize) -> Result<Stage> {
        let pack = format!("IF/EVT/MINI_MAP_STAGE{:02}.GZ", stage + 1);
        // The frame is a pack inside the stage's own, the same in each.
        let frame_pack = interface::file(&pack, 0, "lpk");
        let frame = interface::layout(data, images, &frame_pack, 0)?;
        // The textures of the map's layout are listed stage by stage (0x43514 to
        // 0x43BCC): the stage's own, with the frame's sheet of marks among them
        // and, for the first stage, its sheet of words.
        let (words, marks) = (interface::file(&frame_pack, 5, "ptm"), interface::file(&frame_pack, 6, "ptm"));
        let own = |file: usize| interface::file(&pack, file, "ptm");
        let textures = match stage {
            0 => vec![words, marks, own(5), own(6), own(7)],
            1 => vec![own(5), own(6), own(7), marks, own(8)],
            2 => vec![marks, own(7), own(5), own(6)],
            3 | 4 => vec![marks, own(5), own(6), own(7)],
            _ => vec![own(5), own(6), own(7), own(8), marks, own(9)],
        };
        let map = interface::layout_with(data, images, &pack, 2, &textures)?;

        let table = interface::file(&pack, 1, "xtc");
        let table = xtc::Table::parse(&data.read(&table)?).with_context(|| format!("parsing {table}"))?;
        let short = |record: &[u8], at: usize| record.get(at..at + 2).map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]));
        let piece = |index: usize| table.record(index).and_then(|record| Some(Piece { kind: short(record, 0)?, number: short(record, 2)? }));
        let pieces: Vec<Piece> = (0..table.len()).map_while(piece).collect();
        ensure!(pieces.len() == table.len(), "the table of stage {}'s map is cut short", stage + 1);
        Ok(Stage { frame, map, pieces })
    }
}

/// What the maps are drawn from.
#[derive(Resource)]
pub struct MapAssets {
    stages: Vec<Stage>,
}

impl MapAssets {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<MapAssets> {
        let stages = (0..STAGES).map(|stage| Stage::load(data, images, stage).with_context(|| format!("the map of stage {}", stage + 1)));
        Ok(MapAssets { stages: stages.collect::<Result<_>>()? })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Coming,
    Up,
    Going,
    Gone,
}

/// The map while it is up.
pub struct MapScreen {
    stage: usize,
    phase: Phase,
    /// What plays the frame's sets: the frame itself, a pin, the two marks and
    /// the squares of the colours.
    frame: Player,
    pin: Player,
    marks: [Player; 2],
    squares: Player,
    /// And the map's: its pieces, and the stage's name.
    pieces: Player,
    name: Player,
    /// The colour each of the pieces is drawn in, of the seven; none for one that
    /// is not drawn.
    shades: Vec<Option<usize>>,
    colours: [[u8; 4]; COLOURS],
    /// Where the pins are, and the marks with their kinds.
    pins: Vec<Vec2>,
    marked: Vec<(Vec2, i32)>,
    pub sounds: Vec<Request>,
}

impl MapScreen {
    /// The map of a stage as the save has it now (0x42EA4, 0x43174), coming up;
    /// none for a stage that has none.
    pub fn open(assets: &MapAssets, progress: &Progress, stage: i32) -> Option<MapScreen> {
        let (index, stage) = usize::try_from(stage).ok().and_then(|index| Some((index, assets.stages.get(index)?)))?;
        let here = kept::HERE.map(|number| progress.variable(number));
        let place = |numbers: &[i32; 2]| {
            let (x, y) = (progress.variable(numbers[0]), progress.variable(numbers[1]));
            (x != kept::NOWHERE && y != kept::NOWHERE).then(|| Vec2::new(x as f32, y as f32))
        };
        let marks = kept::MARKS.iter().zip(kept::MARK_KINDS);
        let mut map = MapScreen {
            stage: index,
            phase: Phase::Coming,
            frame: stage.frame.player(frame_set::COMING),
            pin: stage.frame.player(frame_set::PIN),
            marks: [stage.frame.player(frame_set::MISSION), stage.frame.player(frame_set::OTHER)],
            squares: stage.frame.player(frame_set::COLOURS),
            pieces: stage.map.player(map_set::PIECES),
            name: stage.map.player(map_set::NAME),
            shades: stage.pieces.iter().map(|&piece| shade(piece, progress, &here)).collect(),
            colours: [[0; 4]; COLOURS],
            pins: kept::PINS.iter().filter_map(place).collect(),
            marked: marks.filter_map(|(numbers, kind)| Some((place(numbers)?, progress.variable(kind)))).collect(),
            sounds: vec![Request::common(SOUND_COMES)],
        };
        // The frame it is made in is a frame of it already (0x43CB0).
        map.advance(stage);
        Some(map)
    }

    /// A frame of it (0x43BF0). Once it is up, square or circle sends it away;
    /// answers whether it has gone.
    pub fn step(&mut self, assets: &MapAssets, pad: &Pad) -> bool {
        let Some(stage) = assets.stages.get(self.stage) else { return true };
        if self.phase == Phase::Coming && self.frame.finished(&stage.frame) {
            self.phase = Phase::Up;
        }
        match self.phase {
            Phase::Up if pad.pressed & (button::SQUARE | button::CIRCLE) != 0 => {
                self.frame.play(&stage.frame, frame_set::GOING);
                self.sounds.push(Request::common(SOUND_GOES));
                self.phase = Phase::Going;
            }
            Phase::Going if self.frame.finished(&stage.frame) => self.phase = Phase::Gone,
            _ => {}
        }
        self.advance(stage);
        self.phase == Phase::Gone
    }

    /// Everything that plays goes on, and the colours are what their squares have
    /// come to (0x43D48): each square's own colour by its track's.
    fn advance(&mut self, stage: &Stage) {
        let frame = &stage.frame;
        for player in [&mut self.frame, &mut self.pin, &mut self.squares].into_iter().chain(&mut self.marks) {
            player.advance(frame, FRAMES_A_STEP);
        }
        for track in 0..self.squares.len() {
            let square = self.squares.group(track).and_then(|group| Some((group.checked_sub(FIRST_COLOUR).filter(|&square| square < COLOURS)?, frame.first_of(group)?)));
            if let Some((square, object)) = square {
                let moved = frame.placed(&self.squares, track).map_or([0; 4], |(_, colour)| colour);
                self.colours[square] = modulate(moved, frame.colour(object));
            }
        }
        self.pieces.advance(&stage.map, FRAMES_A_STEP);
        self.name.advance(&stage.map, FRAMES_A_STEP);
    }

    /// Draws it over all the field has on the screen (0x43F78, 0x441B0).
    pub fn draw(&self, canvas: &mut Canvas, assets: &MapAssets) {
        let Some(stage) = assets.stages.get(self.stage) else { return };
        stage.frame.draw(canvas, layer::MAP, &self.frame, &Look::default());
        // All else is drawn by the colour the frame's first track has come to,
        // and so comes and goes with it.
        let Some((_, fade)) = stage.frame.placed(&self.frame, 0) else { return };
        for (track, shade) in self.shades.iter().enumerate() {
            if let Some(shade) = *shade {
                stage.map.draw_track(canvas, layer::MAP, &self.pieces, track, &Look::tinted(modulate(self.colours[shade], fade)));
            }
        }
        stage.map.draw(canvas, layer::MAP, &self.name, &Look::tinted(fade));
        for &pin in &self.pins {
            draw_at(canvas, &stage.frame, &self.pin, pin, fade);
        }
        for &(at, kind) in &self.marked {
            if let Some(mark) = usize::try_from(kind).ok().and_then(|kind| self.marks.get(kind)) {
                draw_at(canvas, &stage.frame, mark, at, fade);
            }
        }
    }
}

/// Which of the seven colours a piece is drawn in, if it is drawn (0x44238 to
/// 0x4444C): a place she is on has a colour of its own whether it is known or
/// not.
fn shade(piece: Piece, progress: &Progress, here: &[i32; 3]) -> Option<usize> {
    let number = i32::from(piece.number);
    if number < 0 || progress.flag(kept::HIDDEN + number) {
        return None;
    }
    let known = progress.flag(kept::KNOWN + number);
    Some(match piece.kind {
        1 if known => colour::WAY_KNOWN,
        1 => colour::WAY,
        2 if known => colour::NAME_KNOWN,
        2 => colour::NAME,
        _ if here.contains(&number) => colour::HERE,
        _ if known => colour::PLACE_KNOWN,
        _ => colour::PLACE,
    })
}

/// Draws what a player of the frame's is playing with its first object at a
/// place of the map (0x23F6C, 0x32288).
fn draw_at(canvas: &mut Canvas, frame: &Layout, player: &Player, at: Vec2, tint: [u8; 4]) {
    let home = player.group(0).and_then(|group| frame.first_of(group)).and_then(|object| frame.place(object));
    if let Some(home) = home {
        frame.draw(canvas, layer::MAP, player, &Look { tint, outer: Affine2::from_translation(at - home), ..Look::default() });
    }
}

/// The map, while it is up.
#[derive(Resource, Default)]
pub struct MapUp(pub Option<MapScreen>);

impl MapUp {
    pub fn up(&self) -> bool {
        self.0.is_some()
    }
}

fn draw(up: Res<MapUp>, assets: Res<MapAssets>, mut canvas: ResMut<Canvas>) {
    if let Some(map) = &up.0 {
        map.draw(&mut canvas, &assets);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<MapUp>().add_systems(Update, draw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::Progress;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    fn loaded() -> Option<MapAssets> {
        let data = GameData::new(DATA).ok()?;
        let mut images = Assets::<Image>::default();
        Some(MapAssets::load(&data, &mut images).expect("the maps load"))
    }

    /// The first stage's map as the first map's script sets it for the second
    /// mission (`SET_MMAP_STG01(1, 0, -1, 0, 0)`).
    fn second_mission() -> Progress {
        let mut progress = Progress::new_game();
        reset(&mut progress);
        here(&mut progress, 0, 0);
        here(&mut progress, 1, -1);
        pin(&mut progress, 0, 200, 245);
        pin(&mut progress, 1, 107, 198);
        mark(&mut progress, 0, 127, 194);
        for number in [52, 51, 53, 0, 1, 2, 3, 24, 25, 26] {
            know(&mut progress, number, true);
        }
        hide(&mut progress, 30, true);
        progress
    }

    #[test]
    fn the_scripts_settings_are_kept_in_the_save() {
        let mut progress = Progress::new_game();
        reset(&mut progress);
        assert_eq!(kept::HERE.map(|number| progress.variable(number)), [0xff; 3]);
        assert!(kept::MARKS.iter().chain(&kept::PINS).flatten().all(|&number| progress.variable(number) == kept::NOWHERE));
        here(&mut progress, 0, 3);
        here(&mut progress, 1, 4);
        here(&mut progress, 3, 9);
        assert_eq!(kept::HERE.map(|number| progress.variable(number)), [3, 4, 0xff]);
        here(&mut progress, 1, -1);
        assert_eq!(progress.variable(kept::HERE[1]), 0xff);
        // A pin and a mark are put by their numbers; only -1 and -1 is nowhere.
        pin(&mut progress, 1, 107, 198);
        mark(&mut progress, 0, 127, 194);
        mark(&mut progress, 2, 171, 16);
        pin(&mut progress, 4, 1, 1);
        assert_eq!((progress.variable(0x4fd5), progress.variable(0x4fd6)), (107, 198));
        assert_eq!((progress.variable(0x4fd1), progress.variable(0x4fd2)), (127, 194));
        assert_eq!((progress.variable(0x4fdd), progress.variable(0x4fde)), (171, 16));
        pin(&mut progress, 1, -1, -1);
        assert_eq!((progress.variable(0x4fd5), progress.variable(0x4fd6)), (0xffff, 0xffff));
        mark_kind(&mut progress, 2, 1);
        assert_eq!(progress.variable(5), 1);
        // What is known and what is hidden are flags of their own.
        know(&mut progress, 51, true);
        hide(&mut progress, 30, true);
        assert!(progress.flag(0x157c + 51) && progress.flag(0x15f9 + 30) && !progress.flag(0x157c + 30));
        reset(&mut progress);
        assert!(!progress.flag(0x157c + 51) && !progress.flag(0x15f9 + 30) && progress.variable(5) == 0);
    }

    #[test]
    fn every_stage_has_a_map_whose_table_is_of_its_pieces() {
        let Some(assets) = loaded() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert_eq!(assets.stages.len(), 6);
        for (number, stage) in assets.stages.iter().enumerate() {
            // A record for each track of the pieces, and a name for the stage.
            let pieces = stage.map.player(map_set::PIECES);
            assert_eq!(stage.pieces.len(), pieces.len(), "stage {}", number + 1);
            assert!(stage.pieces.iter().all(|piece| (0..=2).contains(&piece.kind)), "stage {}", number + 1);
            assert_eq!(stage.map.player(map_set::NAME).len(), 1);
            // The frame is the same for all: seven squares of colours among it.
            let squares = stage.frame.player(frame_set::COLOURS);
            let groups: Vec<usize> = (0..squares.len()).filter_map(|track| squares.group(track)).collect();
            assert_eq!(groups, [7, 8, 9, 9, 10, 11, 12, 13]);
        }
        // The first stage's: fourteen places, thirteen ways and three names.
        let kinds = |kind: i16| assets.stages[0].pieces.iter().filter(|piece| piece.kind == kind).map(|piece| piece.number).collect::<Vec<i16>>();
        assert_eq!(kinds(0), (0..14).collect::<Vec<i16>>());
        assert_eq!(kinds(1), (24..37).collect::<Vec<i16>>());
        assert_eq!(kinds(2), [51, 52, 53]);
    }

    #[test]
    fn it_comes_up_as_the_save_has_it_and_square_sends_it_away() {
        let Some(assets) = loaded() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let progress = second_mission();
        assert!(MapScreen::open(&assets, &progress, 6).is_none() && MapScreen::open(&assets, &progress, -1).is_none());
        let mut map = MapScreen::open(&assets, &progress, 0).expect("the first stage has a map");
        assert_eq!(map.sounds, [Request::common(SOUND_COMES)]);
        // She is on the first place; three more are known and the rest are not;
        // three of the ways are known and one is not to be seen; the names are.
        assert_eq!(map.shades[..5], [Some(colour::HERE), Some(colour::PLACE_KNOWN), Some(colour::PLACE_KNOWN), Some(colour::PLACE_KNOWN), Some(colour::PLACE)]);
        assert_eq!(map.shades[14..17], [Some(colour::WAY_KNOWN); 3]);
        assert_eq!((map.shades[17], map.shades[20]), (Some(colour::WAY), None));
        assert_eq!(map.shades[27..], [Some(colour::NAME_KNOWN); 3]);
        assert_eq!(map.pins, [Vec2::new(200.0, 245.0), Vec2::new(107.0, 198.0)]);
        assert_eq!(map.marked, [(Vec2::new(127.0, 194.0), 0)]);

        // It comes over three frames, and no button is taken before it is up.
        let (nothing, square) = (Pad::default(), Pad { pressed: button::SQUARE, ..Pad::default() });
        let drawn = |map: &MapScreen| {
            let mut canvas = Canvas::default();
            map.draw(&mut canvas, &assets);
            canvas.vertices().to_vec()
        };
        let sheet = |map: &MapScreen| drawn(map).first().map(|corner| corner.colour);
        assert_eq!(sheet(&map), Some([0, 0, 0, 46]));
        assert!(!map.step(&assets, &square) && map.phase == Phase::Coming);
        assert!(!map.step(&assets, &nothing) && map.phase == Phase::Coming);
        assert_eq!(sheet(&map), Some([0, 0, 0, 0x8c]));
        assert!(!map.step(&assets, &nothing) && map.phase == Phase::Up);

        // The colours are their squares': a place that is known is light grey, one
        // that is not is dark, and the one she is on is blue.
        assert_eq!(map.colours[colour::PLACE_KNOWN], [180, 180, 180, 204]);
        assert_eq!(map.colours[colour::PLACE], [80, 80, 80, 204]);
        assert_eq!(map.colours[colour::WAY_KNOWN], [180, 180, 180, 204]);
        assert_eq!(map.colours[colour::NAME_KNOWN], [255, 255, 255, 255]);
        assert_eq!(map.colours[colour::NAME], [242, 110, 45, 255]);
        assert_eq!(map.colours[colour::HERE][..3], [3, 66, 255]);
        let all = drawn(&map);
        // The dark is over all of the screen, and the pins and the mark are where
        // the save has them.
        let near = |at: Vec2, by: f32| all.iter().filter(|corner| (corner.position - at).abs().max_element() <= by).count();
        assert!(all.iter().any(|corner| corner.position == Vec2::new(480.0, 272.0)));
        assert!(near(Vec2::new(200.0, 245.0), 14.0) >= 6 && near(Vec2::new(107.0, 198.0), 14.0) >= 6 && near(Vec2::new(127.0, 194.0), 40.0) >= 6);
        // The piece she is on pulses with its square.
        let blue = map.colours[colour::HERE][3];
        for _ in 0..10 {
            map.step(&assets, &nothing);
        }
        assert_ne!(map.colours[colour::HERE][3], blue);

        // Square has it go, with its sound, over three frames.
        assert!(!map.step(&assets, &square) && map.phase == Phase::Going);
        assert_eq!(map.sounds, [Request::common(SOUND_COMES), Request::common(SOUND_GOES)]);
        assert!(!map.step(&assets, &nothing) && !drawn(&map).is_empty());
        assert!(!map.step(&assets, &nothing) && drawn(&map).is_empty());
        assert!(map.step(&assets, &nothing));
    }
}
