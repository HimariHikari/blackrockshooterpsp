//! The tables of the ride on the bike: what the system's script, a course's road
//! and a course's own script carry as data (`docs/BIKE_NOTES.md`, "Building it",
//! "As she comes" and "The system's tables").

use anyhow::{ensure, Context, Result};
use brs_formats::script::Script;

/// The pack every ride reads, and the members of it that are not in its tables.
pub const SYSTEM_PACK: &str = "MINI_GAME/BMG_SYS.BIN";
pub const SYSTEM_TABLES: usize = 0;
pub const CAMERA_SCRIPT: usize = 2;
/// A course's road tables and its own script.
pub const COURSE_ROAD: usize = 0;
pub const COURSE_SCRIPT: usize = 1;

/// The pack of a course, by the name the stage table has it under.
pub fn course_pack(name: &str) -> String {
    format!("MINI_GAME/BMG_{}.BIN", name.to_ascii_uppercase())
}

/// How many lanes a road has, and how long a row of it is.
pub const LANES: usize = 5;
pub const ROW: f32 = 10000.0;

fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    let four = bytes.get(at..at + 4).with_context(|| format!("a table ends before {at:#x}"))?;
    Ok(u32::from_le_bytes(four.try_into().unwrap()))
}

fn i32_at(bytes: &[u8], at: usize) -> Result<i32> {
    u32_at(bytes, at).map(|word| word as i32)
}

fn f32_at(bytes: &[u8], at: usize) -> Result<f32> {
    u32_at(bytes, at).map(f32::from_bits)
}

fn i8_at(bytes: &[u8], at: usize) -> Result<i8> {
    bytes.get(at).map(|&byte| byte as i8).with_context(|| format!("a table ends before {at:#x}"))
}

fn text_at(bytes: &[u8], at: usize, len: usize) -> Result<String> {
    let field = bytes.get(at..at + len).with_context(|| format!("a table ends before {at:#x}"))?;
    let end = field.iter().position(|&byte| byte == 0).unwrap_or(len);
    Ok(String::from_utf8_lossy(&field[..end]).into_owned())
}

fn table<'a>(script: &'a Script, name: &str) -> Result<&'a [u8]> {
    script.extent(name).with_context(|| format!("the script has no `{name}`"))
}

/// Which pack a resource is a member of (0x14950C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pack {
    System,
    Course,
    /// The common game's: she herself is all there is of it.
    Common,
}

/// A resource: a member of a pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resource {
    pub pack: Pack,
    pub member: usize,
}

/// A pair of bytes naming a resource; nought for the pack is none.
fn resource(bytes: &[u8], at: usize) -> Result<Option<Resource>> {
    let pair = bytes.get(at..at + 2).with_context(|| format!("a table ends before {at:#x}"))?;
    let pack = match pair[0] {
        1 => Pack::System,
        2 => Pack::Course,
        3 => Pack::Common,
        _ => return Ok(None),
    };
    Ok(Some(Resource { pack, member: pair[1] as usize }))
}

/// How many animations a model's record has room for.
pub const ANIMATIONS: usize = 17;

/// A model and its animations.
#[derive(Clone, Debug)]
pub struct ModelResource {
    pub model: Option<Resource>,
    pub animations: [Option<Resource>; ANIMATIONS],
}

/// A stage: a course and the field's events that are talked in it.
#[derive(Clone, Debug)]
pub struct Stage {
    pub name: String,
    /// The field's events its talk is of, as in `C_S2M2_BALL`.
    pub events: String,
    /// The tune it is ridden to, as the game's list numbers them (0x149880).
    pub tune: i32,
}

/// The course's flags and numbers as the system's script has them at first
/// (`state`): the scripts and the rules work on them where they lie, so they
/// are kept as the bytes they are (see `state` below for where each is).
pub const STATE_LEN: usize = 0x50;

/// Where the course's flags and numbers are in `state`.
pub mod state {
    pub const FLAGS: usize = 0x00;
    pub const GOING: usize = 0x04;
    pub const START_CHIP: usize = 0x08;
    pub const GOAL_CHIP: usize = 0x0c;
    pub const SPECIAL_TIME: usize = 0x10;
    pub const ROW_TIME: usize = 0x14;
    pub const ROW_FULL: usize = 0x18;
    pub const SLOWING: usize = 0x1c;
    /// Five speeds: where gear 0 begins, and where gears 0 to 3 end.
    pub const SPEEDS: usize = 0x20;
    /// How long each of the four gears takes.
    pub const GEAR_TIMES: usize = 0x34;
    pub const HELD: usize = 0x44;
    pub const AFTER_A_BLOW: usize = 0x48;
    pub const TIME_LEFT: usize = 0x4c;

    /// The flags.
    pub const PAD: u32 = 1;
    pub const GOAL: u32 = 2;
    pub const CAN_DIE: u32 = 4;
    pub const TIME_CAN_RUN_OUT: u32 = 8;
    pub const TIME_RUNS: u32 = 0x10;
    pub const SHOWN: [u32; 4] = [0x20, 0x40, 0x80, 0x100];
    pub const PAD_CHANGES_LANES: u32 = 0x200;
    /// Gears 1, 2 and 3 can be come to.
    pub const GEARS: [u32; 3] = [0x1000, 0x2000, 0x4000];
    pub const SPECIAL_OUT: u32 = 0x8000;
    pub const SPECIAL_CAN_BE_HAD: u32 = 0x10000;
    pub const WORLD_STANDS: u32 = 0x8000_0000;

    /// How she goes.
    pub const NOT_DRIVEN: i32 = 0;
    pub const HIT: i32 = 1;
    pub const RIDING: i32 = 2;
    pub const SPECIAL: i32 = 3;
}

/// What the system's script carries (member 0 of the system's pack).
#[derive(Clone, Debug)]
pub struct SystemTables {
    pub stages: Vec<Stage>,
    pub models: Vec<ModelResource>,
    pub pieces: Vec<Option<Resource>>,
    pub scripts: Vec<Option<Resource>>,
    pub effects: Vec<Option<Resource>>,
    pub voices: Vec<Option<Resource>>,
    /// The script of each kind of character, as a number in `scripts`.
    pub kinds: Vec<i8>,
    pub state: [u8; STATE_LEN],
}

impl SystemTables {
    pub fn read(script: &Script) -> Result<SystemTables> {
        let counts = table(script, "tbl_data")?;
        ensure!(counts.len() >= 7, "`tbl_data` is {} bytes", counts.len());
        let count = |index: usize| counts[index] as usize;

        let stage_table = table(script, "stage_tbl")?;
        let stages = (0..count(0))
            .map(|index| {
                let at = index * 0x54;
                Ok(Stage {
                    name: text_at(stage_table, at, 0x10)?,
                    events: text_at(stage_table, at + 0x30, 0x20)?,
                    tune: i32_at(stage_table, at + 0x50)?,
                })
            })
            .collect::<Result<_>>()?;

        let model_table = table(script, "mdlres_tbl")?;
        let models = (0..count(1))
            .map(|index| {
                let at = index * 0x24;
                let mut animations = [None; ANIMATIONS];
                for (slot, animation) in animations.iter_mut().enumerate() {
                    *animation = resource(model_table, at + 2 + slot * 2)?;
                }
                Ok(ModelResource { model: resource(model_table, at)?, animations })
            })
            .collect::<Result<_>>()?;

        let pairs = |name: &str, count: usize| -> Result<Vec<Option<Resource>>> {
            let bytes = table(script, name)?;
            (0..count).map(|index| resource(bytes, index * 2)).collect()
        };
        let kind_table = table(script, "chr_tbl")?;
        let kinds = (0..count(6)).map(|index| i8_at(kind_table, index)).collect::<Result<_>>()?;
        let state = table(script, "state")?.get(..STATE_LEN).context("`state` is short")?.try_into().unwrap();
        Ok(SystemTables {
            stages,
            models,
            pieces: pairs("smres_tbl", count(2))?,
            scripts: pairs("aires_tbl", count(3))?,
            effects: pairs("effres_tbl", count(4))?,
            voices: pairs("voice_tbl", count(5))?,
            kinds,
            state,
        })
    }

    /// The stage of a course's name.
    pub fn stage(&self, name: &str) -> Option<&Stage> {
        self.stages.iter().find(|stage| stage.name.eq_ignore_ascii_case(name))
    }
}

/// A piece of road in a pattern of scenery: which piece, and four numbers for it.
#[derive(Clone, Copy, Debug)]
pub struct PatternPiece {
    pub piece: i8,
    pub numbers: [f32; 4],
}

/// A place in a pattern where the light is another (0x138F68): a box about a
/// place in the row, turned about the upright, and which of the road's lights is
/// in it. One of kind nought is none.
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub kind: i8,
    pub place: [f32; 3],
    pub half: [f32; 3],
    pub heading: f32,
}

/// One of the three views a ride is drawn through (0x139368): its fog, how far
/// off a thing is still drawn in it (0x131128), and how near and how far it sees.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub fog: [f32; 3],
    pub fog_near: f32,
    pub fog_far: f32,
    pub sight: f32,
    pub near: f32,
    pub far: f32,
}

/// The views: the sky's, the one of what is far off beside the road, and the
/// one of the road and all that is on it.
pub const VIEWS: usize = 3;
pub const VIEW_FAR: usize = 1;
pub const VIEW_ROAD: usize = 2;
/// How many lights a road has: the one of the open road, and those of its zones.
pub const LIGHTS: usize = 5;

/// A row of the road: its two patterns of scenery (-1 none) and its kind of line.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub patterns: [i8; 2],
    pub line: usize,
}

/// A course's road (member 0 of its pack): `map_tbl`, `chip_lst`, `line_lst`
/// and `pattern_lst`.
#[derive(Clone, Debug)]
pub struct RoadTables {
    /// How far apart the lanes are, and where lane 0 is across the road.
    pub lane_width: f32,
    pub first_lane: f32,
    /// The piece that is the sky, or -1.
    pub sky: i8,
    /// How strong the glare over the ride is, of 255 (+ 0x18: 0x139368 hands it to
    /// the game's word 5, 0x149024, which gives it to the glare's task): 100 on
    /// every course.
    pub glare: i32,
    /// What the screen is cleared to before the ride is drawn (0x147620).
    pub backdrop: [f32; 3],
    /// The colour of the light on what is on the road: the open road's, then
    /// each kind of zone's (0x139278).
    pub lights: [[f32; 3]; LIGHTS],
    pub views: [View; VIEWS],
    pub rows: Vec<Row>,
    /// The marks of each lane, for each kind of line.
    pub lines: Vec<[u32; LANES]>,
    pub patterns: Vec<[PatternPiece; 5]>,
    /// Each pattern's zones of another light.
    pub zones: Vec<[Zone; 5]>,
}

impl RoadTables {
    pub fn read(script: &Script) -> Result<RoadTables> {
        let map = table(script, "map_tbl")?;
        let row_count = i32_at(map, 0x0c)?;
        ensure!((0..=100).contains(&row_count), "the road has {row_count} rows");
        let three = |at: usize| -> Result<[f32; 3]> { Ok([f32_at(map, at)?, f32_at(map, at + 4)?, f32_at(map, at + 8)?]) };
        let mut lights = [[0.0; 3]; LIGHTS];
        for (index, light) in lights.iter_mut().enumerate() {
            *light = three(0x28 + index * 12)?;
        }
        let mut views = [View { fog: [0.0; 3], fog_near: 0.0, fog_far: 0.0, sight: 0.0, near: 0.0, far: 0.0 }; VIEWS];
        for (index, view) in views.iter_mut().enumerate() {
            let at = 0x64 + index * 0x20;
            *view = View {
                fog: three(at)?,
                fog_near: f32_at(map, at + 0x0c)?,
                fog_far: f32_at(map, at + 0x10)?,
                sight: f32_at(map, at + 0x14)?,
                near: f32_at(map, at + 0x18)?,
                far: f32_at(map, at + 0x1c)?,
            };
        }

        let line_table = table(script, "line_lst")?;
        let lines: Vec<[u32; LANES]> = (0..line_table.len() / 20)
            .map(|kind| {
                let mut marks = [0; LANES];
                for (lane, mark) in marks.iter_mut().enumerate() {
                    *mark = u32_at(line_table, kind * 20 + lane * 4)?;
                }
                Ok(marks)
            })
            .collect::<Result<_>>()?;

        let pattern_table = table(script, "pattern_lst")?;
        let patterns: Vec<[PatternPiece; 5]> = (0..pattern_table.len() / 260)
            .map(|pattern| {
                let mut pieces = [PatternPiece { piece: -1, numbers: [0.0; 4] }; 5];
                for (slot, piece) in pieces.iter_mut().enumerate() {
                    let at = pattern * 260 + slot * 20;
                    piece.piece = i8_at(pattern_table, at)?;
                    for (number, value) in piece.numbers.iter_mut().enumerate() {
                        *value = f32_at(pattern_table, at + 4 + number * 4)?;
                    }
                }
                Ok(pieces)
            })
            .collect::<Result<_>>()?;
        // After a pattern's five pieces, its five zones: whole numbers all.
        let zones: Vec<[Zone; 5]> = (0..pattern_table.len() / 260)
            .map(|pattern| {
                let mut zones = [Zone { kind: 0, place: [0.0; 3], half: [0.0; 3], heading: 0.0 }; 5];
                for (slot, zone) in zones.iter_mut().enumerate() {
                    let at = pattern * 260 + 0x64 + slot * 0x20;
                    let whole = |number: usize| -> Result<f32> { Ok(i32_at(pattern_table, at + 4 + number * 4)? as f32) };
                    *zone = Zone {
                        kind: i8_at(pattern_table, at)?,
                        place: [whole(0)?, whole(1)?, whole(2)?],
                        half: [whole(3)?, whole(4)?, whole(5)?],
                        heading: whole(6)?,
                    };
                    ensure!((zone.kind as usize) < LIGHTS || zone.kind < 0, "pattern {pattern} has a zone of light {}", zone.kind);
                }
                Ok(zones)
            })
            .collect::<Result<_>>()?;

        let chips = table(script, "chip_lst")?;
        let rows = (0..row_count as usize)
            .map(|row| {
                let line = i8_at(chips, row * 3 + 2)?;
                ensure!((line as usize) < lines.len(), "row {row} has line {line} of {}", lines.len());
                Ok(Row { patterns: [i8_at(chips, row * 3)?, i8_at(chips, row * 3 + 1)?], line: line as usize })
            })
            .collect::<Result<_>>()?;
        Ok(RoadTables {
            lane_width: f32_at(map, 0)?,
            first_lane: f32_at(map, 4)?,
            sky: i8_at(map, 0x14)?,
            glare: i32_at(map, 0x18)?,
            backdrop: three(0x1c)?,
            lights,
            views,
            rows,
            lines,
            patterns,
            zones,
        })
    }
}

/// Where the course's own tables lie in what its script carries: the scripts
/// and the game both work on the records' first bytes where they are.
pub const ENEMY_RECORD: usize = 7;
pub const TRAP_RECORD: usize = 28;
/// A record that has been made has this bit of its first byte.
pub const MADE: u8 = 1;

/// A record of `emy_tbl`: one that is met.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enemy {
    /// The number it is given, or -1 for a new one.
    pub number: i32,
    pub kind: usize,
    pub with: i32,
    pub lane: usize,
    pub row: usize,
    /// How far along its row it is, in hundredths.
    pub along: i32,
}

impl Enemy {
    pub fn read(record: &[u8]) -> Enemy {
        let byte = |at: usize| record[at] as i8;
        Enemy { number: byte(1) as i32, kind: record[2] as usize, with: byte(3) as i32, lane: record[4] as usize, row: record[5] as usize, along: byte(6) as i32 }
    }
}

/// A record of `trp_tbl`: a gate that tells the course's script.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub number: i32,
    pub lane: usize,
    pub row: usize,
    pub along: i32,
    pub size: [f32; 3],
    pub event: i32,
    pub with: i32,
}

impl Gate {
    pub fn read(record: &[u8]) -> Gate {
        let word = |at: usize| i32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        Gate {
            number: record[1] as i8 as i32,
            lane: record[2] as usize,
            row: record[3] as usize,
            along: record[4] as i8 as i32,
            size: [word(8) as f32, word(12) as f32, word(16) as f32],
            event: word(20),
            with: word(24),
        }
    }
}

/// How many records a course's `emy_tbl` and `trp_tbl` have (`table`), and the
/// tables themselves as they lie in the script.
pub struct CourseTables {
    pub enemies: Vec<u8>,
    pub gates: Vec<u8>,
}

impl CourseTables {
    pub fn read(script: &Script) -> Result<CourseTables> {
        let counts = table(script, "table")?;
        let (enemies, gates) = (u32_at(counts, 0)? as usize, u32_at(counts, 4)? as usize);
        let take = |name: &str, len: usize| -> Result<Vec<u8>> {
            Ok(table(script, name)?.get(..len).with_context(|| format!("`{name}` is shorter than `table` says"))?.to_vec())
        };
        Ok(CourseTables { enemies: take("emy_tbl", enemies * ENEMY_RECORD)?, gates: take("trp_tbl", gates * TRAP_RECORD)? })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::data::GameData;

    pub fn data() -> Option<GameData> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok();
        if data.is_none() {
            eprintln!("skipped: game data not unpacked");
        }
        data
    }

    pub fn script(data: &GameData, pack: &str, member: usize) -> Script {
        Script::parse(&data.member(pack, member).unwrap().unwrap()).unwrap()
    }

    #[test]
    fn the_systems_tables_read_as_the_notes_have_them() {
        let Some(data) = data() else { return };
        let tables = SystemTables::read(&script(&data, SYSTEM_PACK, SYSTEM_TABLES)).unwrap();
        assert_eq!(tables.stages.len(), 8);
        assert_eq!((tables.stages[1].name.as_str(), tables.stages[1].events.as_str(), tables.stages[1].tune), ("normal", "C_S2M3_BALL", 8));
        assert_eq!((tables.models.len(), tables.pieces.len(), tables.scripts.len(), tables.effects.len(), tables.voices.len()), (13, 50, 11, 37, 16));
        // The bike, with eleven animations; she herself, out of the common game.
        assert_eq!(tables.models[0].model, Some(Resource { pack: Pack::System, member: 5 }));
        assert_eq!(tables.models[0].animations.iter().flatten().count(), 11);
        assert_eq!(tables.models[1].model, Some(Resource { pack: Pack::Common, member: 0 }));
        // She is kind 0, and her script the system's member 56.
        assert_eq!(tables.kinds, [0, 1, 5, 7, 8, 2, 6, 9, 10]);
        assert_eq!(tables.scripts[0], Some(Resource { pack: Pack::System, member: 56 }));
        let number = |at: usize| f32::from_le_bytes(tables.state[at..at + 4].try_into().unwrap());
        assert_eq!((number(state::SPECIAL_TIME), number(state::ROW_TIME), number(state::SLOWING)), (5.0, 10.0, 0.99));
        assert_eq!([0, 1, 2, 3, 4].map(|gear| number(state::SPEEDS + gear * 4)), [80.0, 140.0, 200.0, 290.0, 360.0]);
        assert_eq!([0, 1, 2, 3].map(|gear| number(state::GEAR_TIMES + gear * 4)), [2.0, 30.0, 15.0, 15.0]);
    }

    #[test]
    fn the_cameras_rig_and_its_moves_read() {
        let Some(data) = data() else { return };
        let tables = SystemTables::read(&script(&data, SYSTEM_PACK, SYSTEM_TABLES)).unwrap();
        // The twelfth model is the camera's own.
        let rig = data.member(SYSTEM_PACK, tables.models[11].model.unwrap().member).unwrap().unwrap();
        assert!(brs_formats::cam::is_camera(&rig));
        let mut moves = 0;
        for animation in tables.models[11].animations.iter().flatten() {
            let bytes = data.member(SYSTEM_PACK, animation.member).unwrap().unwrap();
            let camera = brs_formats::cam::Camera::from_parts(&rig, &bytes).unwrap_or_else(|error| panic!("member {}: {error:#}", animation.member));
            assert!(camera.animation.is_some_and(|animation| animation.frames() > 0), "member {}", animation.member);
            moves += 1;
        }
        assert_eq!(moves, 13);
    }

    #[test]
    fn every_courses_tables_read() {
        let Some(data) = data() else { return };
        let system = SystemTables::read(&script(&data, SYSTEM_PACK, SYSTEM_TABLES)).unwrap();
        for stage in &system.stages {
            let pack = course_pack(&stage.name);
            let road = RoadTables::read(&script(&data, &pack, COURSE_ROAD)).unwrap_or_else(|error| panic!("{pack}: {error:#}"));
            assert!(road.rows.len() >= 50 && road.lane_width == 250.0 && road.first_lane == 500.0, "{pack}: {} rows", road.rows.len());
            let course = CourseTables::read(&script(&data, &pack, COURSE_SCRIPT)).unwrap_or_else(|error| panic!("{pack}: {error:#}"));
            for record in course.enemies.chunks(ENEMY_RECORD) {
                let enemy = Enemy::read(record);
                assert!(enemy.lane < LANES && enemy.row < road.rows.len() && enemy.kind < system.kinds.len(), "{pack}: {enemy:?}");
            }
            for record in course.gates.chunks(TRAP_RECORD) {
                let gate = Gate::read(record);
                assert!(gate.lane < LANES && gate.row < road.rows.len(), "{pack}: {gate:?}");
            }
            assert_eq!(road.glare, 100, "{pack}");
            eprintln!("{pack}: {} rows, {} met, {} gates", road.rows.len(), course.enemies.len() / ENEMY_RECORD, course.gates.len() / TRAP_RECORD);
        }
        // The story's second course: a hundred rows, the first gate the start's.
        let normal = CourseTables::read(&script(&data, &course_pack("normal"), COURSE_SCRIPT)).unwrap();
        assert_eq!((normal.enemies.len() / ENEMY_RECORD, normal.gates.len() / TRAP_RECORD), (215, 14));
        let first = Gate::read(&normal.gates[..TRAP_RECORD]);
        assert_eq!(first, Gate { number: -1, lane: 2, row: 3, along: 0, size: [8000.0, 400.0, 1.0], event: 50, with: 0 });
    }
}
