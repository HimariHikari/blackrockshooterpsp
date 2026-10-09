//! What a field character is made from: a bundle of `FLD/FCHR` with a script, a
//! model and the animations that go with its statuses. See `docs/FIELD_NOTES.md`.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use bevy::prelude::*;
use brs_engine::animation::Clip;
use brs_engine::chain::{ChainFigures, LinkFigures};
use brs_engine::model::ModelAsset;
use brs_engine::AssetStores;
use brs_formats::script;

use super::motion::Resources;
use super::scene::{object_of, status};
use super::tables::FieldTables;
use crate::data::GameData;

// A bundle's slots: its script, its model, and its animations, which are packed in
// one of two ways.
const SCRIPT: &str = "00.sscr";
const MODEL: &str = "01.bin/00.insm";
const ANIMATION_FOLDERS: [&str; 2] = ["02.sc4", "02.bin"];
const ANIMATION_SUFFIX: &str = ".insa";
/// Where in a map's bundle the things of its own are, a bundle each.
const OBJECTS: &str = "11.bin/02.bin";

/// The animations every character has the same numbers for (the original's field
/// character code plays them by status unless a script overrides it).
pub mod animation {
    pub const STAND: usize = 0;
    /// What one stands with while the going is heavy, if it has it (0xFD9C8).
    pub const STAND_HEAVY: usize = 5;
    pub const WALK: usize = 10;
    pub const RUN: usize = 11;
    /// What she trudges with then (0xFDA18).
    pub const TRUDGE: usize = 22;
    /// A turn on the spot (0xF4C4C): the one for a turn that its angle grows by,
    /// and the one for the other way. Only one that has the second has either
    /// (0xFDE7C).
    pub const TURN: [usize; 2] = [14, 15];
    pub const DESTROYED: usize = 60;
    /// Its faces, one every five frames of the game: what the layer of its face
    /// plays (the resident script's `fchr_z_rsd_set_face`).
    pub const FACES: usize = 51;
}

/// How far below the ground a character starts when it appears, unless its script
/// says: the default of `pop_height` (0x102840).
const DEFAULT_POP_HEIGHT: f32 = 100.0;
/// How large the shadow of a character is whose script does not say: the default
/// of `shadow_radius` (0x102868).
const DEFAULT_SHADOW_RADIUS: f32 = 60.0;
/// The speed of a character whose script gives none, in units a frame.
const DEFAULT_SPEED: f32 = 32.0;

/// The engine functions a script's `define_gimmick` uses for an animation layer.
const GIMMICK_SET_KIND: u32 = 0;
const GIMMICK_SET_ANIMATION: u32 = 2;
const GIMMICK_KIND_ANIMATION: i32 = 0;
// And of one that is a chain (0x10ADEC, 0x10B00C, 0x10B120, 0x10B1D4): the id
// of its first node; a figure of one of its links, which are its bone's length,
// its mass, its strength and its area; the way its bones lie; and its figure
// 0, how thick the air is to it.
const GIMMICK_SET_CHAIN: u32 = 1;
const GIMMICK_SET_LINK: u32 = 3;
const GIMMICK_SET_ALONG: u32 = 4;
const GIMMICK_SET_FIGURE: u32 = 7;
/// What a link is until its script says (0xE9D60: its mass; its strength is
/// thirty at the first link and ten at the last, which every script that has
/// a chain says over).
const LINK_UNSET: LinkFigures = LinkFigures { length: 0.0, mass: 0.02, bend: 30.0, area: 0.0 };

pub struct CharacterAsset {
    pub model: ModelAsset,
    clips: BTreeMap<usize, Arc<Clip>>,
    /// Animations that play all the time on top of the body's, each driving only its
    /// own nodes: the player has one for her hair and one for her face.
    pub layers: Vec<Arc<Clip>>,
    /// The speeds its script gives for walking, for running and for trudging, in
    /// units a frame.
    walk: f32,
    run: f32,
    trudge: f32,
    pub pop_height: f32,
    /// How wide the ring it comes up through is, and how big its burst, of a
    /// hundred; and the way it is destroyed.
    pub pop_radius: f32,
    pub destroy_radius: f32,
    pub destroy_kind: u8,
    /// How large its shadow is: half of how far across.
    pub shadow_radius: f32,
    /// Which of its layers is its face (its script's `gim_facial`), if it has
    /// one: the layer a script's faces are shown with.
    pub facial: Option<usize>,
    /// The chains its script describes: what of it hangs and swings.
    pub chains: Vec<ChainFigures>,
}

impl CharacterAsset {
    pub fn clip(&self, number: usize) -> Option<&Arc<Clip>> {
        self.clips.get(&number)
    }

    fn speed(&self, status: i32) -> f32 {
        match status {
            status::WALK => self.walk,
            status::TRUDGE => self.trudge,
            _ => self.run,
        }
    }
}

/// Stands in for the engine while asking a script something it should be able to
/// answer on its own; an unexpected engine call is reported rather than ignored.
struct NoEngine;

impl script::Host for NoEngine {
    fn native(&mut self, number: u32, _call: &script::Call) -> Result<u32> {
        bail!("script called engine function {number} where none was expected")
    }
}

/// Notes the engine calls a `define_gimmick` makes, to see what kind of gimmick it is.
#[derive(Default)]
struct GimmickProbe {
    /// Function number and first argument of each call.
    calls: Vec<(u32, i32)>,
    /// The chain it describes, if it is one.
    chain: Option<ChainFigures>,
}

impl script::Host for GimmickProbe {
    fn native(&mut self, number: u32, call: &script::Call) -> Result<u32> {
        self.calls.push((number, call.int(0)?));
        match (number, &mut self.chain) {
            (GIMMICK_SET_CHAIN, chain) => {
                *chain = u8::try_from(call.int(0)?).ok().map(|id| ChainFigures { id, along: Vec3::X, air: 0.0, links: Vec::new() });
            }
            (GIMMICK_SET_ALONG, Some(chain)) => chain.along = Vec3::new(call.float(0)?, call.float(1)?, call.float(2)?),
            (GIMMICK_SET_FIGURE, Some(chain)) if call.int(0)? == 0 => chain.air = call.float(1)?,
            (GIMMICK_SET_LINK, Some(chain)) => {
                let (link, value) = (usize::try_from(call.int(0)?).unwrap_or(usize::MAX), call.float(2)?);
                if link < 64 {
                    if chain.links.len() <= link {
                        chain.links.resize(link + 1, LINK_UNSET);
                    }
                    let figures = &mut chain.links[link];
                    match call.int(1)? {
                        0 => figures.length = value,
                        1 => figures.mass = value,
                        2 => figures.bend = value,
                        3 => figures.area = value,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(0)
    }
}

/// What a character's script says about it, without its model.
pub struct Traits {
    pub walk: f32,
    pub run: f32,
    pub trudge: f32,
    pub pop_height: f32,
    /// The floats `pop_radius` and `destroy_radius`, a hundred each if it has
    /// them not (0x102938, 0x10296C), and the byte `type_destroy` (0x1027CC).
    pub pop_radius: f32,
    pub destroy_radius: f32,
    pub destroy_kind: u8,
    /// The float `shadow_radius` (0x1028D0), sixty if it has it not.
    pub shadow_radius: f32,
    /// The numbers of the animations of its gimmicks that just play an animation.
    pub layers: Vec<usize>,
    /// Which of those layers the gimmick `gim_facial` names is (0x1027F4).
    pub facial: Option<usize>,
    /// Its gimmicks that are chains.
    pub chains: Vec<ChainFigures>,
}

impl Traits {
    /// What one with no script has: the engine's own figures.
    fn of_no_script() -> Traits {
        Traits {
            walk: DEFAULT_SPEED,
            run: DEFAULT_SPEED,
            trudge: DEFAULT_SPEED,
            pop_height: DEFAULT_POP_HEIGHT,
            pop_radius: DEFAULT_POP_HEIGHT,
            destroy_radius: DEFAULT_POP_HEIGHT,
            destroy_kind: 0,
            shadow_radius: DEFAULT_SHADOW_RADIUS,
            layers: Vec::new(),
            facial: None,
            chains: Vec::new(),
        }
    }

    pub fn read(script: &script::Script) -> Result<Traits> {
        // Scripts give speeds as distance per logic step. One with nothing to say
        // about speed leaves its character with the engine's own (0x10A63C).
        let function = script.symbol("get_move_speed");
        let speed = |status: i32| -> Result<f32> {
            let Some(function) = function else { return Ok(DEFAULT_SPEED) };
            Ok(f32::from_bits(script::Machine::new(script).call(function, &[status as u32], &mut NoEngine)?))
        };

        // The script describes the character's "gimmicks" one by one: one that
        // just plays an animation, or a chain (the player's two are her tails).
        let mut layers = Vec::new();
        let mut facial = None;
        let mut chains = Vec::new();
        let face = script.integer("gim_facial");
        if let (Some(count), Some(define)) = (script.integer("num_gimmick"), script.symbol("define_gimmick")) {
            for gimmick in 0..count {
                let mut probe = GimmickProbe::default();
                script::Machine::new(script).call(define, &[gimmick], &mut probe)?;
                if let [(GIMMICK_SET_KIND, GIMMICK_KIND_ANIMATION), (GIMMICK_SET_ANIMATION, number)] = probe.calls[..] {
                    if face == Some(gimmick) {
                        facial = Some(layers.len());
                    }
                    layers.push(number as usize);
                }
                chains.extend(probe.chain.filter(|chain| !chain.links.is_empty()));
            }
        }
        Ok(Traits {
            walk: speed(status::WALK)?,
            run: speed(status::RUN)?,
            trudge: speed(status::TRUDGE)?,
            pop_height: script.floats::<1>("pop_height").map_or(DEFAULT_POP_HEIGHT, |[height]| height),
            pop_radius: script.floats::<1>("pop_radius").map_or(DEFAULT_POP_HEIGHT, |[radius]| radius),
            destroy_radius: script.floats::<1>("destroy_radius").map_or(DEFAULT_POP_HEIGHT, |[radius]| radius),
            destroy_kind: script.symbol("type_destroy").and_then(|at| script.data(at)).and_then(|bytes| bytes.first().copied()).unwrap_or(0),
            shadow_radius: script.floats::<1>("shadow_radius").map_or(DEFAULT_SHADOW_RADIUS, |[radius]| radius),
            layers,
            facial,
            chains,
        })
    }
}

impl CharacterAsset {
    /// Loads the bundle at `bundle`, e.g. `FLD/FCHR/FCHR_M_EGG01.BIN`.
    pub fn load(data: &GameData, bundle: &str, stores: &mut AssetStores) -> Result<CharacterAsset> {
        let (model, bytes) = data.model(format!("{bundle}/{MODEL}"))?;
        let mut clips = BTreeMap::new();
        for folder in ANIMATION_FOLDERS.map(|folder| format!("{bundle}/{folder}")) {
            if !data.has_folder(&folder) {
                continue;
            }
            for name in data.list(&folder)? {
                let Some(number) = name.strip_suffix(ANIMATION_SUFFIX).and_then(|number| number.parse::<usize>().ok())
                else {
                    continue;
                };
                // An animation that cannot be read is left out, so that the character
                // is at least there.
                match data.clip(format!("{folder}/{name}")) {
                    Ok(clip) => {
                        clips.insert(number, clip);
                    }
                    Err(error) => warn!("animation {number} of {bundle} is left out: {error:#}"),
                }
            }
        }
        // A thing of a map's own is a model and its animations, with no script.
        let script = format!("{bundle}/{SCRIPT}");
        let traits = if data.exists(&script) { Traits::read(&data.script(script)?)? } else { Traits::of_no_script() };
        let layers = traits.layers.iter().filter_map(|number| clips.get(number).cloned()).collect();
        Ok(CharacterAsset {
            model: ModelAsset::build(&model, &bytes, stores),
            clips,
            layers,
            walk: traits.walk,
            run: traits.run,
            trudge: traits.trudge,
            pop_height: traits.pop_height,
            pop_radius: traits.pop_radius,
            destroy_radius: traits.destroy_radius,
            destroy_kind: traits.destroy_kind,
            shadow_radius: traits.shadow_radius,
            facial: traits.facial,
            chains: traits.chains,
        })
    }
}

/// The character resources that have been asked for, by their number in the field's
/// list. One that could not be loaded is remembered as such, and reported once.
/// The player's resource is not the bundle the list has for its number but the
/// one of the suit she has on (0x14BE78), which the original keeps loaded
/// whatever the field does: the second figure here is the character that is.
/// The third figure is the map whose own things the ones loaded are of
/// (`s110`: the bundles of file 2 of the map's scene, 0x1108DC).
#[derive(Resource, Default)]
pub struct CharacterAssets(BTreeMap<i32, Option<CharacterAsset>>, i32, Option<i32>);

/// The player's resource.
const PLAYER_RESOURCE: i32 = 0;
/// The frames of an animation that go by in a frame of the game.
const ANIMATION_FRAMES_A_STEP: f32 = 2.0;

impl CharacterAssets {
    pub fn get(&self, chr: i32) -> Option<&CharacterAsset> {
        self.0.get(&chr)?.as_ref()
    }

    /// The character of the list the player's resource is.
    pub fn worn(&self) -> i32 {
        self.1
    }

    /// Has the player's resource be a character of the list: what was loaded
    /// for it is let go if it was another's.
    pub fn wear(&mut self, character: i32) {
        if self.1 != character {
            self.1 = character;
            self.0.remove(&PLAYER_RESOURCE);
        }
    }

    /// The map that is up, and that another is: the things of the first's own
    /// are let go.
    pub fn map(&self) -> Option<i32> {
        self.2
    }

    pub fn on_map(&mut self, map: i32) {
        self.2 = Some(map);
        self.0.retain(|&chr, _| object_of(chr).is_none());
    }

    /// Makes sure resource `chr` has been loaded, if it can be.
    pub fn ensure(&mut self, chr: i32, data: &GameData, tables: &FieldTables, stores: &mut AssetStores) {
        if self.0.contains_key(&chr) {
            return;
        }
        let of = if chr == PLAYER_RESOURCE { self.1 } else { chr };
        let bundle = match object_of(chr) {
            Some(object) => {
                let map = self.2.and_then(|map| usize::try_from(map).ok()).and_then(|map| tables.maps.get(map));
                map.map(|map| format!("FLD/MAP/MAP_{}.BIN/{OBJECTS}/{object:02}.bin", map.name.to_ascii_uppercase()))
            }
            None => tables.character_bundle(of),
        };
        let loaded = match bundle {
            Some(bundle) => CharacterAsset::load(data, &bundle, stores).with_context(|| format!("loading {bundle}")),
            None => Err(anyhow::anyhow!("the field's list has no character {chr}")),
        };
        match (&loaded, object_of(chr)) {
            (Err(error), _) => warn!("character resource {chr} is not shown: {error:#}"),
            (Ok(asset), Some(object)) => debug!("thing {object} of the map's own is a model from {:.0} to {:.0}", asset.model.min, asset.model.max),
            _ => {}
        }
        self.0.insert(chr, loaded.ok());
    }

}

impl Resources for CharacterAssets {
    fn speed(&self, chr: i32, status: i32) -> f32 {
        self.get(chr).map_or(0.0, |asset| asset.speed(status))
    }

    fn pop_height(&self, chr: i32) -> f32 {
        self.get(chr).map_or(DEFAULT_POP_HEIGHT, |asset| asset.pop_height)
    }

    fn destroy_kind(&self, chr: i32) -> u8 {
        self.get(chr).map_or(0, |asset| asset.destroy_kind)
    }

    /// An animation's frames are sixtieths of a second, two to a frame of the
    /// game.
    fn motion_frames(&self, chr: i32, motion: i32) -> Option<f32> {
        let clip = self.get(chr)?.clip(usize::try_from(motion).ok()?)?;
        Some(clip.frames() / ANIMATION_FRAMES_A_STEP)
    }

    fn motion_lead_in(&self, chr: i32, motion: i32) -> Option<f32> {
        let clip = self.get(chr)?.clip(usize::try_from(motion).ok()?)?;
        Some(clip.loop_range().0 / ANIMATION_FRAMES_A_STEP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    fn traits(name: &str) -> Option<Traits> {
        let data = GameData::new(DATA).ok()?;
        let script = data.script(format!("FLD/FCHR/FCHR_{name}.BIN/{SCRIPT}")).expect("the character's script parses");
        Some(Traits::read(&script).expect("the script answers"))
    }

    #[test]
    fn the_player_runs_at_thirty_two_and_has_two_animation_layers() {
        let Some(player) = traits("P_BRS00") else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        assert!((player.walk - 40.0 / 7.0).abs() < 1e-4);
        assert_eq!(player.run, 32.0);
        assert_eq!(player.layers.len(), 2);
        assert_eq!(player.pop_height, DEFAULT_POP_HEIGHT);
        assert_eq!((player.pop_radius, player.destroy_radius, player.destroy_kind), (100.0, 100.0, 0));
        // The second of her layers is her face, and plays her faces.
        assert_eq!((player.layers.as_slice(), player.facial), ([50, animation::FACES].as_slice(), Some(1)));
        assert_eq!(player.shadow_radius, 60.0);
        // An enemy has no face to give.
        assert_eq!(traits("M_EGG01").unwrap().facial, None);
    }

    #[test]
    fn her_two_tails_are_chains_and_which_other_characters_have_any() {
        let Some(player) = traits("P_BRS00") else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // Four links from node 100 along x and three from node 101 the other
        // way, in air of a fiftieth.
        let [long, short] = &player.chains[..] else { panic!("{:?}", player.chains) };
        assert_eq!((long.id, long.along, long.air, long.links.len()), (100, Vec3::X, 0.02, 4));
        assert_eq!((short.id, short.along, short.air, short.links.len()), (101, Vec3::NEG_X, 0.02, 3));
        eprintln!("{long:?}\n{short:?}");
        // Heavier, stiffer and wider at the root than at the tip, by the square
        // of how far along the link is.
        let (root, tip) = (long.links[0], long.links[3]);
        assert_eq!((root.mass, root.bend, root.area), (0.0125, 20.5, 0.002));
        assert!((tip.mass - 0.005).abs() < 1e-6 && (tip.bend - 0.5).abs() < 1e-5 && (tip.area - 0.0004).abs() < 1e-7, "{tip:?}");
        assert!(long.links.iter().chain(&short.links).all(|link| link.length.abs() > 5.0));
        // Who else has chains.
        let data = GameData::new(DATA).unwrap();
        let mut with = Vec::new();
        let mut names: Vec<String> =
            std::fs::read_dir(format!("{DATA}/FLD/FCHR")).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in names {
            let script = format!("FLD/FCHR/{name}/{SCRIPT}");
            if !data.exists(&script) {
                continue;
            }
            let Ok(traits) = data.script(script).and_then(|script| Traits::read(&script)) else { continue };
            if !traits.chains.is_empty() {
                with.push((name, traits.chains.iter().map(|chain| (chain.id, chain.links.len())).collect::<Vec<_>>()));
            }
        }
        eprintln!("characters with chains: {with:?}");
        assert!(with.iter().any(|(name, _)| name == "FCHR_P_BRS00.BIN"));
    }

    #[test]
    fn enemies_get_their_speeds_from_the_code_they_share() {
        let (Some(eater), Some(hornet)) = (traits("M_EGG01"), traits("M_BEE00")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        eprintln!("eater walks {} runs {}; hornet walks {} runs {}", eater.walk, eater.run, hornet.walk, hornet.run);
        assert!(eater.run > 0.0 && hornet.run > 0.0);
        assert!(eater.layers.is_empty());
    }
}
