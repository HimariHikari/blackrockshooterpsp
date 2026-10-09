//! What a map's own script says its scene is like.
//!
//! As a map starts (0x111614) the field puts the scene's settings back to what they
//! are without a say, then has each of the up to eight scripts of the map's fifth
//! slot run its `init_scene`, with engine functions of its own (the table at
//! 0x172B80, twenty-five of them). The fog's: function 19 turns it on or off (the
//! field's flag 0x1A), 20 gives where it begins and where it is whole, 21 its
//! colour as three parts of one. The fog is the PSP's own, set before the scene is
//! drawn (0x10E6A0). Of the game's eighty-eight maps eighty-three have such a
//! script, and eighty-one of those turn the fog on. Four of the scripts are of the
//! script format's first version.
//!
//! The lights': the field has eight lights the characters are lit by, one each
//! (see `docs/FIELD_NOTES.md`, "The lights"), and a light that is all about. A
//! script says which light what follows is of (function 18), then its kind (11),
//! its colours (12 and 13), where it is (14) and the way it shines from (15); 17
//! is the light all about. And function 24 is the colour the screen is cleared to
//! before the scene is drawn (0x10EA90), which shows wherever nothing is drawn:
//! beyond a map's edge, and through a floor that is drawn first of all and is
//! partly seen through.

use anyhow::Result;
use bevy::prelude::*;
use brs_formats::script;

use crate::data::GameData;

/// The scripts of a map's bundle that set its scene up, by their number.
const SCRIPTS: usize = 8;
const FUNCTION: &str = "init_scene";

// The engine functions of a scene's script that the port has.
const LIGHT_KIND: u32 = 11;
const LIGHT_COLOUR: u32 = 12;
const LIGHT_COLOURS: u32 = 13;
const LIGHT_PLACE: u32 = 14;
const LIGHT_ANGLES: u32 = 15;
const AMBIENT: u32 = 17;
const CHOOSE: u32 = 18;
const FOG_ON: u32 = 19;
const FOG_RANGE: u32 = 20;
const FOG_COLOUR: u32 = 21;
const CAMERA_LEAN: u32 = 22;
const CAMERA_LEVEL: u32 = 23;
const BACKDROP: u32 = 24;

/// What function 18 is given first when it chooses a light.
const CHOOSE_LIGHT: i32 = 2;

/// The fog of a scene: nothing of it nearer than `near`, nothing but it from `far`
/// on, and its colour in parts of one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub near: f32,
    pub far: f32,
    pub colour: [f32; 3],
}

impl Fog {
    /// What the field starts with (0x118170): it is not put back as a map starts,
    /// so a map that turned the fog on without saying more would have the last
    /// map's; none does.
    const UNSET: Fog = Fog { near: 10.0, far: 20000.0, colour: [0.5, 0.5, 1.0] };
}

/// How many lights a scene has (0xF009C).
pub const LIGHTS: usize = 8;

/// One of a scene's lights (0x70 bytes of the field, from its + 0x1600 on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// It shines from a place (kind 1) and not from a way (kind 0).
    pub at_a_place: bool,
    /// The way it shines from: its angles about the axis across and the upright
    /// one, in radians (its + 0x10).
    pub angles: Vec3,
    /// Where it is (its + 0x20).
    pub place: Vec3,
    /// Its colours (its + 0x40 on): what it adds wherever it falls, what it adds
    /// by how far a surface faces it, and its shine.
    pub colours: [Vec3; 3],
}

impl Light {
    /// What every light is as a map starts (0xEFEC4): a pale violet all round
    /// that is made up to white where a surface faces it, and a white shine. Its
    /// kind, its place and its angles are not put back; no script leaves them
    /// unsaid for a light a character is lit by.
    pub const UNSET: Light = Light {
        at_a_place: false,
        angles: Vec3::ZERO,
        place: Vec3::ZERO,
        colours: [Vec3::new(0.75, 0.65, 1.0), Vec3::new(0.25, 0.35, 0.0), Vec3::ONE],
    };

    /// The way to it from `from` (0x10F44C): for one that shines from a way the
    /// way ahead turned about the axis across by its first angle and then about
    /// the upright one by its second (0x162224, 0x1621C8, each of which turns what
    /// has been turned already), for one at a place the way there.
    pub fn towards(&self, from: Vec3) -> Vec3 {
        if self.at_a_place {
            (self.place - from).normalize_or_zero()
        } else {
            Quat::from_rotation_y(self.angles.y) * Quat::from_rotation_x(self.angles.x) * Vec3::Z
        }
    }
}

/// The lights of a scene: the light all about (the field's + 0x1D00, whose fourth
/// part is not used for a colour) and the eight a character can be lit by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lights {
    pub ambient: Vec3,
    pub lights: [Light; LIGHTS],
}

impl Default for Lights {
    /// As a map starts, before its scripts have their say (0xEFEC4).
    fn default() -> Lights {
        Lights { ambient: Vec3::splat(0.2), lights: [Light::UNSET; LIGHTS] }
    }
}

impl Lights {
    /// `s166(light, r, g, b, r, g, b)` (0xC55CC): the light's first two colours,
    /// in hundredths: what it adds everywhere is the first three, and what it adds
    /// by how far a surface faces it is what the last three are more than those.
    pub fn colour(&mut self, light: i32, first: [i32; 3], second: [i32; 3]) {
        let Some(light) = usize::try_from(light).ok().and_then(|light| self.lights.get_mut(light)) else { return };
        let parts = |of: [i32; 3]| Vec3::from(of.map(|part| part as f32 / 100.0));
        light.colours[0] = parts(first);
        light.colours[1] = parts([second[0] - first[0], second[1] - first[1], second[2] - first[2]]);
    }

    /// `s164(r, g, b, a)` (0xC5520): the light all about, in hundredths; nothing
    /// is done if a part is not from 0 to 100.
    pub fn all_about(&mut self, parts: [i32; 3]) {
        if parts.iter().all(|part| (0..=100).contains(part)) {
            self.ambient = Vec3::from(parts.map(|part| part as f32 / 100.0));
        }
    }

    /// `s165()` (0xF0264): the first two lights and the light all about are put
    /// back as `kept` has them.
    pub fn put_back(&mut self, kept: &Lights) {
        self.lights[..2].copy_from_slice(&kept.lights[..2]);
        self.ambient = kept.ambient;
    }
}

/// A map's scene as its scripts leave it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Scenery {
    pub fog: Option<Fog>,
    pub lights: Lights,
    /// The colour the screen is cleared to before the scene is drawn (the field's
    /// + 0x1E20), in parts of one.
    pub backdrop: [f32; 3],
    /// How far the pitch of a camera that follows her leans to the slope from
    /// what it looks at up to itself, where that is less than its pitch and where
    /// it is more (function 22, 0x1120D4: the camera's + 0xBD0 and + 0xBD4, said
    /// in hundredths); and how much of its pitch is taken away (function 23,
    /// 0x112140: its + 0xBD8). All nought where a script does not say (0x1116F8).
    pub lean: [f32; 2],
    pub level: f32,
}

/// The colour the screen is cleared to where a script does not say (0x11170C): 161
/// in 255 of each part.
pub const BACKDROP_UNSET: [f32; 3] = [161.0 / 255.0; 3];

impl Default for Scenery {
    fn default() -> Scenery {
        Scenery { fog: None, lights: Lights::default(), backdrop: BACKDROP_UNSET, lean: [0.0; 2], level: 0.0 }
    }
}

impl Scenery {
    /// Reads the scene of the map whose bundle's eleventh slot is `bundle`.
    pub fn load(data: &GameData, bundle: &str) -> Result<Scenery> {
        let mut setup = Setup { on: false, fog: Fog::UNSET, lights: Lights::default(), chosen: 0, backdrop: BACKDROP_UNSET, lean: [0.0; 2], level: 0.0 };
        for number in 0..SCRIPTS {
            let path = format!("{bundle}/05.bin/{number:02}.sscr");
            if !data.exists(&path) {
                continue;
            }
            let script = data.script(path)?;
            if let Some(function) = script.symbol(FUNCTION) {
                script::Machine::new(&script).call(function, &[], &mut setup)?;
            }
        }
        Ok(Scenery { fog: setup.on.then_some(setup.fog), lights: setup.lights, backdrop: setup.backdrop, lean: setup.lean, level: setup.level })
    }
}

/// What a scene's script asks of the engine.
struct Setup {
    on: bool,
    fog: Fog,
    lights: Lights,
    /// The light what is said of a light is said of (the field's + 0x1D20).
    chosen: i32,
    backdrop: [f32; 3],
    lean: [f32; 2],
    level: f32,
}

impl Setup {
    fn light(&mut self) -> Option<&mut Light> {
        usize::try_from(self.chosen).ok().and_then(|chosen| self.lights.lights.get_mut(chosen))
    }
}

impl script::Host for Setup {
    fn native(&mut self, number: u32, call: &script::Call) -> Result<u32> {
        let three = |from: u32| -> Result<Vec3> { Ok(Vec3::new(call.float(from)?, call.float(from + 1)?, call.float(from + 2)?)) };
        match number {
            FOG_ON => self.on = call.int(0)? != 0,
            FOG_RANGE => (self.fog.near, self.fog.far) = (call.float(0)?, call.float(1)?),
            FOG_COLOUR => self.fog.colour = [call.float(0)?, call.float(1)?, call.float(2)?],
            // 0x111F60: only with 2 does it choose a light.
            CHOOSE => {
                if call.int(0)? == CHOOSE_LIGHT {
                    self.chosen = call.int(1)?;
                }
            }
            // 0x111A14: only with 0 first, and only the two kinds there are.
            LIGHT_KIND => {
                let (what, kind) = (call.int(0)?, call.int(1)?);
                if let Some(light) = self.light().filter(|_| what == 0 && (0..=1).contains(&kind)) {
                    light.at_a_place = kind == 1;
                }
            }
            // 0x111AA4: one of its three colours.
            LIGHT_COLOUR => {
                let (which, colour) = (call.int(0)?, three(1)?);
                if let Some(of) = usize::try_from(which).ok().and_then(|which| self.light()?.colours.get_mut(which)) {
                    *of = colour;
                }
            }
            // 0x111BB0: its first two at once, the second being what the last three
            // parts are more than the first three, held between nothing and one.
            LIGHT_COLOURS => {
                let (first, second) = (three(0)?, three(3)?);
                if let Some(light) = self.light() {
                    light.colours[0] = first;
                    light.colours[1] = (second - first).clamp(Vec3::ZERO, Vec3::ONE);
                }
            }
            LIGHT_PLACE => {
                let place = three(0)?;
                if let Some(light) = self.light() {
                    light.place = place;
                }
            }
            // 0x111E10: said in degrees.
            LIGHT_ANGLES => {
                let angles = three(0)?;
                if let Some(light) = self.light() {
                    light.angles = Vec3::new(angles.x.to_radians(), angles.y.to_radians(), angles.z.to_radians());
                }
            }
            AMBIENT => self.lights.ambient = three(0)?,
            // 0x11218C.
            BACKDROP => self.backdrop = three(0)?.to_array(),
            // 0x1120D4 and 0x112140: said in hundredths.
            CAMERA_LEAN => self.lean = [call.float(0)? / 100.0, call.float(1)? / 100.0],
            CAMERA_LEVEL => self.level = call.float(0)? / 100.0,
            // The rest: nothing the port draws by.
            _ => {}
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn the_maps_of_the_game_say_what_their_fog_is() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        // The first stage's first map: a pale green haze from two hundred on.
        let first = Scenery::load(&data, "FLD/MAP/MAP_D_STG01_01.BIN/11.bin").unwrap();
        assert_eq!(first.fog, Some(Fog { near: 200.0, far: 20000.0, colour: [0.7, 0.95, 0.9] }));
        // Every map's scripts run, and most have a fog.
        let (mut maps, mut fogged) = (0, 0);
        let bundles = std::fs::read_dir(format!("{DATA}/FLD/MAP")).unwrap();
        for name in bundles.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()) {
            let bundle = format!("FLD/MAP/{name}/11.bin");
            if !data.has_folder(&bundle) {
                continue;
            }
            let scenery = Scenery::load(&data, &bundle).unwrap_or_else(|error| panic!("{name}: {error:#}"));
            maps += 1;
            if scenery.lean != [0.0; 2] || scenery.level != 0.0 {
                println!("{name}: the camera leans by {:?} and is levelled by {}", scenery.lean, scenery.level);
            }
            if let Some(fog) = scenery.fog {
                fogged += 1;
                assert!(fog.near < fog.far && fog.colour.iter().all(|part| (0.0..=1.0).contains(part)), "{name}: {fog:?}");
            }
            let lights = &scenery.lights;
            let said: Vec<String> = lights
                .lights
                .iter()
                .enumerate()
                .filter(|(_, light)| **light != Light::UNSET)
                .map(|(number, light)| {
                    let [first, second, shine] = light.colours;
                    let angles = light.angles.map(f32::to_degrees);
                    format!("{number}: {first:.2} + {second:.2}, shine {shine:.2}, from ({:.0}, {:.0}) = {:.2}", angles.x, angles.y, light.towards(Vec3::ZERO))
                })
                .collect();
            eprintln!("{name}: cleared to {:.2?}, all about {:.2}; {}", scenery.backdrop, lights.ambient, said.join("; "));
        }
        eprintln!("{fogged} of {maps} maps have a fog");
        assert_eq!((fogged, maps), (81, 83));
    }

    #[test]
    fn a_light_shines_from_the_way_its_angles_say() {
        // The first stage's lights are at (210, -80): thirty degrees up, from the
        // east and a little to the north.
        let light = Light { angles: Vec3::new(210f32.to_radians(), (-80f32).to_radians(), 0.0), ..Light::UNSET };
        assert!((light.towards(Vec3::ZERO) - Vec3::new(0.853, 0.5, -0.150)).length() < 1e-3, "{}", light.towards(Vec3::ZERO));
        assert!((Light::UNSET.towards(Vec3::ZERO) - Vec3::Z).length() < 1e-6);
        // One at a place shines from there.
        let lamp = Light { at_a_place: true, place: Vec3::new(0.0, 300.0, 400.0), ..Light::UNSET };
        assert!((lamp.towards(Vec3::ZERO) - Vec3::new(0.0, 0.6, 0.8)).length() < 1e-6);
    }

    #[test]
    fn a_map_s_script_changes_the_lights_and_puts_them_back() {
        let kept = Lights::default();
        let mut lights = kept;
        // The prologue's: one colour all round.
        lights.colour(0, [75, 85, 100], [75, 85, 100]);
        assert_eq!(lights.lights[0].colours[..2], [Vec3::new(0.75, 0.85, 1.0), Vec3::ZERO]);
        lights.colour(1, [20, 30, 40], [50, 50, 30]);
        assert!((lights.lights[1].colours[1] - Vec3::new(0.3, 0.2, -0.1)).length() < 1e-6);
        lights.colour(8, [0; 3], [0; 3]);
        lights.all_about([30, 40, 50]);
        assert_eq!(lights.ambient, Vec3::new(0.3, 0.4, 0.5));
        lights.all_about([30, 140, 50]);
        assert_eq!(lights.ambient, Vec3::new(0.3, 0.4, 0.5));
        lights.put_back(&kept);
        assert_eq!(lights, kept);
    }
}
