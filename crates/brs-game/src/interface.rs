//! What the interface is drawn from: the layouts of the game's packs, loaded with
//! their textures, and the order the parts of the screen are drawn in.

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_engine::layout::Layout;
use brs_engine::texture;
use brs_formats::layout::Objects;
use brs_formats::ptmd;

use crate::data::GameData;

/// The frames of the interface in a second: its clips' keys and its texts' glyphs are
/// timed in these (0x850 counts one for a frame drawn at sixty a second, two at
/// thirty).
pub const FRAMES_A_SECOND: f32 = 60.0;

/// The interface's frames that have gone by since the last frame drawn.
pub fn frames(time: &Time) -> f32 {
    time.delta_secs() * FRAMES_A_SECOND
}

/// The layers of the canvas, lowest first: what is drawn over what. They are the
/// original's own numbers for its tasks (0xAE3C; `tools_re/task_makers.py` lists
/// them): each frame every task has its turn to draw, those with the lower numbers
/// first (0xB800).
pub mod layer {
    /// The screen between missions ("menuStageRootTask"), and what its
    /// messages' task puts over it ("stage_select_message").
    pub const HUB: i32 = 0x100;
    pub const HUB_MESSAGES: i32 = 0x101;
    /// The fade of the scene alone that a script asks for ("Fade Priority").
    pub const SCENE_FADE: i32 = 0x229;
    /// The picture of the screen that the screen's fade keeps: the field draws it
    /// first of what it draws itself (0x10F324), and a battle likewise (0x73DD0),
    /// over the scene and under all of the interface.
    pub const SCREEN_PICTURE: i32 = 0x2ff;
    /// What the field shows of the mission and of her ("fldTask2d").
    pub const DISPLAY: i32 = 0x300;
    /// The window of her items in a battle (`ifBtlMenuTask`).
    pub const BATTLE_ITEMS: i32 = 0x301;
    /// The title ("Title"), which has the same number: the field is not there
    /// while it is.
    pub const TITLE: i32 = 0x300;
    /// The window a record is shown in ("ifRecInfoTask").
    pub const RECORD: i32 = 0x33f;
    /// The pictures a script puts over the screen ("Pict Task").
    pub const PICTURES: i32 = 0x340;
    /// The window a script puts up ("SystemWin", "SelectWin").
    pub const WINDOW: i32 = 0x341;
    /// The window people speak to her through ("ifComWindowTask").
    pub const COMM: i32 = 0x37d;
    /// A film ("Movie").
    pub const FILM: i32 = 0x37e;
    /// The window of a mission failed ("ifFldFailedTask").
    pub const FAILED: i32 = 0x400;
    /// The map of the stage ("ifFldMiniMap"), which has the same number.
    pub const MAP: i32 = 0x400;
    /// The menus: the ground behind them ("menuBgTask"), a menu's own screen
    /// ("menuMainTask"), the screen one of its entries leads to ("menuCustomTask"
    /// and the like) and the wipe they come and go under ("menuFadeTask").
    pub const MENU_GROUND: i32 = 0x401;
    pub const MENU: i32 = 0x402;
    pub const MENU_SCREEN: i32 = 0x403;
    pub const MENU_WIPE: i32 = 0x404;
    /// The screen's fade ("Trans").
    pub const FADE: i32 = 0x600;
    /// The title of a scene's place ("Location").
    pub const LOCATION: i32 = 0x601;
    /// What is spoken in a scene ("telop_auto_task").
    pub const CAPTIONS: i32 = 0x602;
    /// The pause ("Pause"), over the screen's fade; and what darkens the screen
    /// over the pause in its turn ("sysFadeTask").
    pub const PAUSE: i32 = 0x610;
    pub const PAUSE_FADE: i32 = 0x6fe;
    /// The panels that tell of a record ("recMsgTask").
    pub const RECORD_NEWS: i32 = 0x700;
    /// The list of saves, which is the port's own: over everything, as the
    /// PSP's dialog is that the original hands over to.
    pub const SAVES: i32 = 0x900;
}

/// The pack of the interface that is always there (0x2CC94), and the textures of
/// it that layouts of other packs are drawn with.
pub const RESIDENT_PACK: &str = "SYSTEM.VOL/if_rsdsys.lpk";

/// The name of a file of a pack, by its number and what it is.
pub fn file(pack: &str, index: usize, extension: &str) -> String {
    format!("{pack}/{index:03}.{extension}")
}

/// The pack of the signs' layout, a pack inside the one that is always there
/// (its + 0x54): the battle's cursor and marks are of it (0x3550C), and the
/// field's sign of what a button would do (0x464FC).
pub const SIGNS_PACK: &str = "SYSTEM.VOL/if_rsdsys.lpk/018.lpk";

/// The files of the signs' pack that its layout's twenty-one textures are
/// (0x2CF24); the first is file 20 of the pack that is always there, and for some
/// it is given none.
const SIGNS_TEXTURES: [Option<usize>; 21] = [
    None,
    None,
    None,
    Some(3),
    None,
    None,
    Some(4),
    None,
    Some(5),
    Some(6),
    Some(7),
    Some(8),
    Some(9),
    Some(10),
    None,
    Some(11),
    Some(12),
    Some(13),
    Some(14),
    Some(15),
    Some(16),
];

/// The files the signs' layout's textures are, in its order; where it is given
/// none, the first of them again.
pub fn signs_textures() -> Vec<String> {
    let of_all = file(RESIDENT_PACK, 20, "ptm");
    SIGNS_TEXTURES.iter().map(|texture| texture.map_or(of_all.clone(), |texture| file(SIGNS_PACK, texture, "ptm"))).collect()
}

/// The signs' layout, with its textures. Whatever shows signs has one of its own.
pub fn signs_layout(data: &GameData, images: &mut Assets<Image>) -> Result<Layout> {
    layout_with(data, images, SIGNS_PACK, 0, &signs_textures())
}

/// The system's own texts (0x97DC with 4, 0x2C54C), which the windows that are
/// not of one place take their words from.
const SYSTEM_TEXTS: &str = "SYSTEM.VOL/msg_system.bin";

/// One of the system's texts, by its number.
pub fn system_text(data: &GameData, number: usize) -> Result<String> {
    let texts = brs_formats::xtc::Table::parse(&data.read(SYSTEM_TEXTS)?).with_context(|| format!("parsing {SYSTEM_TEXTS}"))?;
    let at = texts.record(number).and_then(|record| record.get(..4)).with_context(|| format!("the system has no text {number}"))?;
    Ok(texts.string(u32::from_le_bytes(at.try_into().unwrap()))?.to_owned())
}

/// Loads a texture that is a file of its own; answers with it the size it is drawn
/// from by, which is the PSP's for it (its rows made up to a power of two).
pub fn load_texture(data: &GameData, images: &mut Assets<Image>, path: &str) -> Result<(Handle<Image>, Vec2)> {
    let bytes = data.read(path)?;
    let header = ptmd::parse_header(&bytes).with_context(|| format!("{path} is not a texture"))?;
    Ok((images.add(texture::image(&bytes, &header)), texture::size(&header)))
}

/// Loads a layout from a pack: its objects are file `first` of the pack, its
/// motions and its groups the two after, and its textures those that follow.
pub fn layout(data: &GameData, images: &mut Assets<Image>, pack: &str, first: usize) -> Result<Layout> {
    let objects = data.read(file(pack, first, "sdoa"))?;
    let count = Objects::parse(&objects).with_context(|| format!("parsing {}", file(pack, first, "sdoa")))?.textures;
    let textures: Vec<String> = (0..count).map(|index| file(pack, first + 3 + index, "ptm")).collect();
    layout_with(data, images, pack, first, &textures)
}

/// Loads a layout whose textures are not all its own pack's: the code that makes
/// such a layout lists them one by one.
pub fn layout_with(data: &GameData, images: &mut Assets<Image>, pack: &str, first: usize, textures: &[String]) -> Result<Layout> {
    let textures = textures_of(data, images, textures)?;
    // The groups' file has no mark of its own to be named by: where it is the last
    // file of what a pack was made from it has been taken for another kind.
    let groups = [file(pack, first + 2, "bin"), file(pack, first + 2, "l")];
    let [groups, other] = groups;
    let groups = if data.exists(&groups) { groups } else { other };
    let (objects, motions) = (file(pack, first, "sdoa"), file(pack, first + 1, "xbn"));
    Layout::parse(&data.read(&objects)?, &data.read(motions)?, &data.read(groups)?, textures).with_context(|| format!("the layout at {objects}"))
}

/// Loads a layout whose three files are not in the usual order in their pack, with
/// the textures its code lists for it: its objects, its motions and its groups by
/// their numbers.
pub fn layout_of(data: &GameData, images: &mut Assets<Image>, pack: &str, [objects, motions, groups]: [usize; 3], textures: &[String]) -> Result<Layout> {
    let textures = textures_of(data, images, textures)?;
    let (objects, motions, groups) = (file(pack, objects, "sdoa"), file(pack, motions, "xbn"), file(pack, groups, "bin"));
    Layout::parse(&data.read(&objects)?, &data.read(motions)?, &data.read(groups)?, textures).with_context(|| format!("the layout at {objects}"))
}

/// Loads the textures a layout is drawn with; one named more than once is loaded
/// once.
fn textures_of(data: &GameData, images: &mut Assets<Image>, textures: &[String]) -> Result<Vec<Handle<Image>>> {
    let mut loaded: Vec<(&String, Handle<Image>)> = Vec::new();
    let mut handles = Vec::with_capacity(textures.len());
    for path in textures {
        let handle = match loaded.iter().find(|(known, _)| *known == path) {
            Some((_, handle)) => handle.clone(),
            None => {
                let handle = load_texture(data, images, path)?.0;
                loaded.push((path, handle.clone()));
                handle
            }
        };
        handles.push(handle);
    }
    Ok(handles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_captions_layout_loads_with_its_texture() {
        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let layout = layout(&data, &mut images, "SYSTEM.VOL/if_fldsys.lpk", 1).unwrap();
        assert_eq!((layout.objects.objects.len(), layout.textures.len(), layout.motions.sets.len()), (3, 1, 3));
        // The marks a caption of two lines and one of one line are put at.
        let place = |group: usize| layout.first_of(group).and_then(|object| layout.place(object));
        assert_eq!((place(0), place(1)), (Some(Vec2::new(48.0, 229.0)), Some(Vec2::new(480.0, 238.0))));
        // The sign that a button is waited for blinks: by frame 8 it is all there.
        let mut sign = layout.player(0);
        sign.advance(&layout, 8.0);
        assert_eq!(sign.value(0, brs_formats::layout::Channel::Alpha), Some(255.0));
    }
}
