//! Reading the unpacked game data from disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use bevy::prelude::Resource;
use brs_engine::animation::Clip;
use brs_engine::camera::{CameraClip, CameraPath};
use brs_formats::{cam, clim, container, cpmd, insa, insm, psmf, script, stcm};

/// Where the disc's own files are, beside the data folder: the films' `MOVIE`
/// folder is in it.
const FILMS: &str = "extracted/PSP_GAME/USRDIR";
/// What a recording is made into to be played, and where those of a pack that
/// lies as one file are kept: in a folder of the pack's name with this after it.
const CONVERTED: &str = "ogg";
const CONVERTED_BESIDE: &str = ".sound";

/// The folder `brs-extract` writes, e.g. `game/unpacked`.
#[derive(Resource, Clone)]
pub struct GameData {
    root: PathBuf,
}

impl GameData {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        ensure!(
            root.join("FLD").is_dir(),
            "{} does not look like unpacked game data; run `cargo run --release -p brs-extract` first",
            root.display()
        );
        Ok(GameData { root })
    }

    /// A game with no data: for tests of what reads none.
    #[cfg(test)]
    pub fn nowhere() -> Self {
        GameData { root: PathBuf::new() }
    }

    /// Reads a file, given by its path relative to the data folder. A member of a
    /// pack that was unpacked before the extractor knew the pack's kind (the
    /// `PDK` packs of the ride on the bike) is read out of the pack as it is.
    pub fn read(&self, relative: impl AsRef<Path>) -> Result<Vec<u8>> {
        let path = self.root.join(&relative);
        if !path.is_file() {
            if let Some(member) = self.packed(relative.as_ref()) {
                return Ok(container::unwrap(&member)?.unwrap_or(member));
            }
        }
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(container::unwrap(&data)?.unwrap_or(data))
    }

    pub fn exists(&self, relative: impl AsRef<Path>) -> bool {
        self.root.join(&relative).is_file() || self.packed(relative.as_ref()).is_some()
    }

    /// A member of a `PDK` pack, by its number: out of the pack if it lies in the
    /// data folder as one file, else the file `NNN.<kind>` of the folder it was
    /// unpacked into. `None` for a place the pack has nothing in.
    pub fn member(&self, pack: &str, number: usize) -> Result<Option<Vec<u8>>> {
        let path = self.root.join(pack);
        if path.is_file() {
            let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let members = container::parse_pdk(&bytes).with_context(|| format!("taking {} apart", path.display()))?;
            return Ok(members.get(number).filter(|member| !member.is_empty()).map(|member| member.to_vec()));
        }
        for name in self.list(pack)? {
            if Path::new(&name).file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse().ok()) == Some(number) {
                return std::fs::read(path.join(&name)).with_context(|| format!("reading {name} of {}", path.display())).map(Some);
            }
        }
        Ok(None)
    }

    /// What `relative` names inside a `PDK` pack that lies in the data folder as
    /// one file: its member `NNN.ext`, or something of a bundle or a pack that
    /// member is, as in `001.bin/07.bin/00.pphd`. What a recording of such a
    /// pack was made into to be played (`brs-extract --sound`) is beside the
    /// pack, in `<pack>.sound`.
    fn packed(&self, relative: &Path) -> Option<Vec<u8>> {
        let parts: Vec<&str> = relative.iter().map(|part| part.to_str()).collect::<Option<_>>()?;
        let depth = (1..parts.len()).find(|&depth| self.root.join(parts[..depth].iter().collect::<PathBuf>()).is_file())?;
        let pack = self.root.join(parts[..depth].iter().collect::<PathBuf>());
        if let [name] = parts[depth..] {
            if Path::new(name).extension().is_some_and(|extension| extension == CONVERTED) {
                let mut beside = pack.clone().into_os_string();
                beside.push(CONVERTED_BESIDE);
                return std::fs::read(Path::new(&beside).join(name)).ok();
            }
        }
        let number = |name: &str| Path::new(name).file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse::<usize>().ok());
        let bytes = std::fs::read(pack).ok()?;
        let members = container::parse_pdk(&bytes).ok()?;
        let mut within: Vec<u8> = members.get(number(parts[depth])?).filter(|member| !member.is_empty())?.to_vec();
        for name in &parts[depth + 1..] {
            let inner = container::unwrap(&within).ok()?.unwrap_or(within);
            let slots = container::parse_sc(&inner).or_else(|| container::parse_lpk(&inner).ok())?;
            within = slots.get(number(name)?).filter(|slot| !slot.is_empty())?.to_vec();
        }
        Some(within)
    }

    /// A film, by the name a script gives it (`movie/BRS_OP_final.pmf`), taken
    /// apart. The films are not in the tree that is unpacked but beside it on the
    /// disc: in `extracted/PSP_GAME/USRDIR` next to the data folder, under the
    /// names the disc has them by, which are all capitals.
    pub fn film(&self, name: &str) -> Result<psmf::Film> {
        let beside = self.root.parent().context("the data folder has nothing beside it")?;
        let path = beside.join(FILMS).join(name.to_ascii_uppercase());
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        psmf::Film::parse(&bytes).with_context(|| format!("taking {} apart", path.display()))
    }

    /// Where the sound of that film is, once `brs-extract --sound` has made it:
    /// its path in the data folder.
    pub fn film_sound(name: &str) -> String {
        let stem = Path::new(name).file_stem().unwrap_or_default().to_string_lossy().to_ascii_uppercase();
        format!("MOVIE/{stem}.ogg")
    }

    /// Whether there is a folder (an unpacked archive) at this path.
    pub fn has_folder(&self, relative: impl AsRef<Path>) -> bool {
        self.root.join(relative).is_dir()
    }

    /// The names of the files in a folder (an unpacked archive), sorted.
    pub fn list(&self, relative: impl AsRef<Path>) -> Result<Vec<String>> {
        let path = self.root.join(relative);
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&path).with_context(|| format!("listing {}", path.display()))? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// A model and the file bytes its textures are decoded from.
    pub fn model(&self, relative: impl AsRef<Path>) -> Result<(insm::Model, Vec<u8>)> {
        let relative = relative.as_ref();
        let data = self.read(relative)?;
        let model = insm::Model::parse(&data).with_context(|| format!("parsing {}", relative.display()))?;
        Ok((model, data))
    }

    pub fn clip(&self, relative: impl AsRef<Path>) -> Result<Arc<Clip>> {
        let relative = relative.as_ref();
        let animation =
            insa::Animation::parse(&self.read(relative)?).with_context(|| format!("parsing {}", relative.display()))?;
        Ok(Clip::new(animation))
    }

    /// A camera move (`.cam`).
    pub fn camera(&self, relative: impl AsRef<Path>) -> Result<Arc<CameraClip>> {
        let relative = relative.as_ref();
        let camera =
            cam::Camera::parse(&self.read(relative)?).with_context(|| format!("parsing {}", relative.display()))?;
        Ok(CameraClip::new(camera))
    }

    /// A path for the camera of a field map: a bundle with the map of the path, the
    /// camera's rig and its move in its slots 1 to 3 (0xDAB58, 0x110C4C).
    pub fn camera_path(&self, relative: impl AsRef<Path>) -> Result<Arc<CameraPath>> {
        let relative = relative.as_ref();
        let data = self.read(relative)?;
        let slot = |slot: usize| container::sc_slot(&data, slot).with_context(|| format!("{} has no slot {slot}", relative.display()));
        let map = cpmd::PathMap::parse(slot(1)?).with_context(|| format!("parsing the map of {}", relative.display()))?;
        let camera = cam::Camera::from_parts(slot(2)?, container::sc_slot(&data, 3).unwrap_or(&[]))
            .with_context(|| format!("parsing the camera of {}", relative.display()))?;
        Ok(CameraPath::new(map, camera))
    }

    /// A map and the file bytes its textures are decoded from.
    pub fn map(&self, relative: impl AsRef<Path>) -> Result<(stcm::Map, Vec<u8>)> {
        let relative = relative.as_ref();
        let data = self.read(relative)?;
        let map = stcm::Map::parse(&data).with_context(|| format!("parsing {}", relative.display()))?;
        Ok((map, data))
    }

    pub fn script(&self, relative: impl AsRef<Path>) -> Result<script::Script> {
        let relative = relative.as_ref();
        script::Script::parse(&self.read(relative)?).with_context(|| format!("parsing {}", relative.display()))
    }

    pub fn collision(&self, relative: impl AsRef<Path>) -> Result<clim::Collision> {
        let relative = relative.as_ref();
        clim::Collision::parse(&self.read(relative)?).with_context(|| format!("parsing {}", relative.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_of_every_maps_camera_reads() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let (mut maps, mut paths) = (0, 0);
        for entry in std::fs::read_dir(data.root.join("FLD/MAP")).unwrap() {
            let map = entry.unwrap().file_name().to_string_lossy().into_owned();
            let folder = format!("FLD/MAP/{map}/11.bin/08.bin");
            if !data.has_folder(&folder) {
                continue;
            }
            maps += 1;
            for name in data.list(&folder).unwrap() {
                let path = data.camera_path(format!("{folder}/{name}")).unwrap_or_else(|error| panic!("{map} {name}: {error:#}"));
                // Wherever she is the camera is somewhere along its move.
                let pose = path.pose(0.0, 0.0);
                assert!(pose.transform.translation.is_finite() && pose.focal_length > 0.0, "{map} {name}");
                paths += 1;
            }
        }
        eprintln!("{paths} paths of {maps} maps");
        assert!(maps >= 40 && paths >= maps, "{paths} paths of {maps} maps");
    }
}
