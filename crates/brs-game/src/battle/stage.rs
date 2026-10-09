//! A battle map: its models, fog and view distance, as set up by the map's own script
//! (the original's `BtMapActor`, built at 0x96D0C).

use anyhow::{bail, ensure, Context, Result};
use bevy::prelude::*;
use brs_formats::bscr::{FunctionKind, Host, Machine, Reply, Script, Table};
use brs_formats::stcm;

use crate::data::GameData;

/// The signal the engine sends a map's script to have it set the map up.
const SIGNAL_INIT: i32 = 1;

// The properties a map script defines, by the numbers the engine reads them with.
const PROPERTY_FOG_START: usize = 3;
const PROPERTY_FOG_END: usize = 4;
const PROPERTY_FOG_COLOR: usize = 5;

// Engine functions (of the second table) that map scripts call.
const ENGINE_PRINT: usize = 1;
const ENGINE_LOAD_MODEL: usize = 22;
const ENGINE_PROPERTY_COUNT: usize = 37;
const ENGINE_SET_PROPERTY: usize = 38;
const ENGINE_NAME_PROPERTY: usize = 40;
const ENGINE_PROPERTY_RANGE: usize = 41;
const ENGINE_MODEL_LAYER: usize = 42;
const ENGINE_SET_COLOR_PROPERTY: usize = 43;
const ENGINE_VIEW_DISTANCE: usize = 44;
const ENGINE_TURN_MODEL: usize = 48;

/// The player's two standing places lie on a circle around the stage's middle, this
/// far apart along it (set at 0x99B54 and 0x99BA4).
#[derive(Resource, Clone, Copy)]
pub struct Stage {
    pub radius: f32,
    pub arc: f32,
}

impl Default for Stage {
    fn default() -> Self {
        Stage { radius: 1500.0, arc: 800.0 }
    }
}

impl Stage {
    /// The middle of the stage, where the enemies are and which the player faces.
    pub const CENTRE: Vec3 = Vec3::ZERO;

    /// The player's standing place `index` (0 or 1), on the near side of the stage
    /// (0xA0A80).
    pub fn slot(&self, index: u32) -> Vec3 {
        let side = if index == 0 { -0.5 } else { 0.5 };
        Quat::from_rotation_y(self.arc * side / self.radius) * Vec3::new(0.0, 0.0, -self.radius)
    }
}

/// One of the map's models.
pub struct MapModel {
    pub map: stcm::Map,
    /// The file's bytes, which its textures are decoded from.
    pub data: Vec<u8>,
    /// Which of the scene's layers draws it: 0 for a sky, 2 for the ground.
    pub layer: i32,
    /// Turn about the vertical, in degrees.
    pub turn: f32,
}

/// A map as its script leaves it.
pub struct BattleMap {
    pub models: Vec<MapModel>,
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_color: Color,
    /// How near and how far the camera sees.
    pub near: f32,
    pub far: f32,
}

impl BattleMap {
    /// Loads the battle map named e.g. `STG01_01` by running its script.
    pub fn load(data: &GameData, stage: &str) -> Result<BattleMap> {
        let folder = format!("BTL/MAP/MAP_B_{stage}.VOL");
        // The script is the archive's one `.bms`, whatever it is called: four of the
        // maps (a boss's, in stages 2, 3, 4 and 6) have theirs as `btlmap02.bms`.
        let named = format!("{folder}/map_b_{}.bms", stage.to_lowercase());
        let script_path = match data.exists(&named) {
            true => named,
            false => {
                let found = data.list(&folder)?.into_iter().find(|file| file.to_lowercase().ends_with(".bms"));
                format!("{folder}/{}", found.with_context(|| format!("{folder} has no script"))?)
            }
        };
        let script = Script::parse(&data.read(&script_path)?).with_context(|| format!("parsing {script_path}"))?;

        let mut setup = Setup::default();
        let mut machine = Machine::new(&script);
        if let Some(init) = script.entry(FunctionKind::Init) {
            machine.call(&script, init, &[], &mut setup)?;
        }
        let signal = script.entry(FunctionKind::Signal).with_context(|| format!("{script_path} takes no signals"))?;
        machine.signal(&script, signal, &[SIGNAL_INIT, 0, 0], &mut setup).context("running the map's script")?;

        let mut models = Vec::new();
        for (slot, model) in setup.models.iter().enumerate() {
            let Some(model) = model else { continue };
            let (map, bytes) = data.map(format!("{folder}/{}", model.file)).with_context(|| format!("map model {slot}"))?;
            models.push(MapModel { map, data: bytes, layer: model.layer, turn: model.turn });
        }
        ensure!(!models.is_empty(), "the map's script loaded no model");

        let (near, far) = setup.view.context("the map's script set no view distance")?;
        // The script gives the fog's colour in percent of full brightness (0x96E34).
        let [red, green, blue, _] = setup.color(PROPERTY_FOG_COLOR)?;
        let percent = |channel: i32| (channel * 255 / 100).clamp(0, 255) as u8;
        Ok(BattleMap {
            models,
            fog_start: setup.int(PROPERTY_FOG_START)? as f32,
            fog_end: setup.int(PROPERTY_FOG_END)? as f32,
            fog_color: Color::srgb_u8(percent(red), percent(green), percent(blue)),
            near,
            far,
        })
    }
}

#[derive(Clone, Default)]
struct ModelSlot {
    file: String,
    layer: i32,
    turn: f32,
}

/// A value a script exposes to the engine by number, with the range it may take.
#[derive(Clone, Copy)]
enum Property {
    Int { value: i32, range: Option<(i32, i32)> },
    Color([i32; 4]),
}

/// What a map script asks of the engine, gathered for [`BattleMap::load`].
#[derive(Default)]
struct Setup {
    models: Vec<Option<ModelSlot>>,
    properties: Vec<Property>,
    view: Option<(f32, f32)>,
}

impl Setup {
    fn property(&mut self, index: i32) -> Result<&mut Property> {
        let slot = usize::try_from(index).ok().and_then(|index| self.properties.get_mut(index));
        slot.with_context(|| format!("the script has no property {index}"))
    }

    fn model(&mut self, slot: i32) -> Result<&mut ModelSlot> {
        let model = usize::try_from(slot).ok().and_then(|slot| self.models.get_mut(slot)?.as_mut());
        model.with_context(|| format!("no model was loaded into slot {slot}"))
    }

    fn int(&self, index: usize) -> Result<i32> {
        match self.properties.get(index) {
            Some(Property::Int { value, .. }) => Ok(*value),
            _ => bail!("the map's script did not set property {index} to a number"),
        }
    }

    fn color(&self, index: usize) -> Result<[i32; 4]> {
        match self.properties.get(index) {
            Some(Property::Color(color)) => Ok(*color),
            _ => bail!("the map's script did not set property {index} to a colour"),
        }
    }
}

impl Host for Setup {
    fn engine(&mut self, table: Table, number: usize, arguments: &[i32], script: &Script) -> Result<Reply> {
        ensure!(table == Table::Two, "map scripts are not expected to call the first table");
        match (number, arguments) {
            (ENGINE_PRINT, [text]) => debug!("map script: {}", script.string(*text)?.trim_end()),
            (ENGINE_LOAD_MODEL, [slot, name]) => {
                let slot = usize::try_from(*slot).ok().filter(|slot| *slot < 16).context("model slot out of range")?;
                if self.models.len() <= slot {
                    self.models.resize(slot + 1, None);
                }
                self.models[slot] = Some(ModelSlot { file: script.string(*name)?.to_owned(), ..default() });
            }
            (ENGINE_PROPERTY_COUNT, [count]) => {
                let count = usize::try_from(*count).ok().filter(|count| *count <= 256).context("property count")?;
                self.properties = vec![Property::Int { value: 0, range: None }; count];
            }
            (ENGINE_SET_PROPERTY, [index, value]) => {
                let property = self.property(*index)?;
                // A property that was a colour becomes a number again; one with a range
                // keeps the value within it (0xA4D6C).
                let range = match *property {
                    Property::Int { range, .. } => range,
                    Property::Color(_) => None,
                };
                let value = range.map_or(*value, |(low, high)| (*value).clamp(low, high));
                *property = Property::Int { value, range };
            }
            // The names are for a debug menu.
            (ENGINE_NAME_PROPERTY, [index, _, _]) => {
                self.property(*index)?;
            }
            (ENGINE_PROPERTY_RANGE, [index, low, high]) => {
                if let Property::Int { range, .. } = self.property(*index)? {
                    *range = Some((*low, *high));
                }
            }
            (ENGINE_MODEL_LAYER, [slot, layer]) => self.model(*slot)?.layer = *layer,
            (ENGINE_SET_COLOR_PROPERTY, [index, red, green, blue, alpha]) => {
                *self.property(*index)? = Property::Color([*red, *green, *blue, *alpha]);
            }
            // Called for each of the scene's two cameras with the same distances.
            (ENGINE_VIEW_DISTANCE, [_, near, far]) => self.view = Some((*near as f32, *far as f32)),
            (ENGINE_TURN_MODEL, [slot, degrees]) => self.model(*slot)?.turn = *degrees as f32,
            _ => bail!("map scripts are not expected to call engine function {number}"),
        }
        Ok(Reply::NONE)
    }
}
