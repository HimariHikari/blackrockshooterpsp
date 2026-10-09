//! The panels that tell of a record while the game goes on (`recMsgTask`,
//! 0x14B080 to 0x14B5E4), at the foot of the screen on the left: one for a record
//! that has been opened (0x72D48), and one for how far a record's count has come
//! (0x721D8), which is another when the count has come to its goal. See "The
//! records (challenges)" in `docs/FIELD_NOTES.md`.
//!
//! The field and a battle ask each frame whether there is news of a record
//! (0x14B080): with no panel up, the first record that has news gets its panel,
//! one that has been opened before one that has been counted, and that news is
//! forgotten. A panel comes, stays two seconds and goes; no button is waited for.
//! What would show the records' windows waits for it to be gone, and so do a
//! battle's end and a map's script that asks to (`s275`).
//!
//! Its file is `021.lpk` of the pack of the interface that is always there: a
//! layout of six sets, a coming and a going for each of the three panels.

use anyhow::Result;
use bevy::math::Affine2;
use bevy::prelude::*;
use brs_engine::canvas::Canvas;
use brs_engine::layout::{modulate, Layout, Look, Player};

use crate::battle::BattleSource;
use crate::data::GameData;
use crate::interface::{self, layer};
use crate::progress::Progress;
use crate::record_window::{draw_small_picture, FIGURE_CELL};
use crate::records::{News, Records, COUNT};
use crate::sound::{Board, COMMON_BANK, FULL_VOLUME};
use crate::text::{self, TextColours};
use crate::writing::{Block, Fonts, Placing, Style};
use crate::Mode;

const PACK: &str = "SYSTEM.VOL/if_rsdsys.lpk/021.lpk";
/// The frames of the interface in a frame of the game.
const FRAMES_A_STEP: f32 = 2.0;
/// How long a panel stays once it has come, in frames of the interface (0x72450,
/// 0x72FAC).
const STAY: f32 = 120.0;

// The groups of its layout that are more than something to draw. Of the panel of
// a count (0x7279C): the bar, which is cut to how far the count has come; where
// the record's small picture and its name go; and the hundredths of how far, in
// two figures. Of the panel of a record opened (0x7327C): its small picture and
// its name, which is written twice, the first time as its own shadow.
const BAR: usize = 6;
const SMALL_PICTURE: usize = 10;
const NAME: usize = 11;
const HUNDREDTHS: usize = 13;
const OPENED_SMALL_PICTURE: usize = 24;
const OPENED_NAME: usize = 25;

/// How a record's name is written (0x72308): in the text's own font, a line of
/// thirty-two glyphs.
const NAME_STYLE: Style = Style::of_kind(0, 0x20, 1, 1.0, 1.0);

/// Which panel it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A record's count has come so far of its goal (0x724E0).
    Counted,
    /// It has come to all of it (0x724FC).
    Cleared,
    /// A record has been opened (0x73030).
    Opened,
}

impl Kind {
    /// The sets it comes and goes by (0x724AC and 0x7252C; 0x73030 and 0x73074).
    fn sets(self) -> (usize, usize) {
        match self {
            Kind::Counted => (0, 1),
            Kind::Cleared => (2, 3),
            Kind::Opened => (4, 5),
        }
    }

    /// What is heard as it comes (0x7243C, 0x72F94), of the bank that is always
    /// there.
    fn sound(self) -> u32 {
        match self {
            Kind::Counted | Kind::Cleared => 0x0d,
            Kind::Opened => 0x66,
        }
    }
}

/// Where a panel is in its turn (its states 1 to 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Coming,
    Staying,
    /// It has stayed its time, and is told to go on the next frame (0x14B404).
    Stayed,
    Going,
    /// Gone and drawn no more; the frame after, it is done with (0x14B478).
    Gone,
}

/// A panel that is up.
struct Panel {
    kind: Kind,
    record: i32,
    /// The record's kind, which its small picture goes by.
    icon: u8,
    name: Block,
    /// How much of its goal the count has come to.
    share: f32,
    phase: Phase,
    stayed: f32,
    player: Player,
    /// Its sound is heard on the first frame it moves (0x7240C).
    unheard: bool,
}

/// The panels, and what they are drawn from.
#[derive(Resource)]
pub struct RecordNews {
    layout: Layout,
    /// The width of the sheet of figures, and the sheet of small pictures.
    figures: f32,
    small: (Handle<Image>, Vec2),
    panel: Option<Panel>,
}

impl RecordNews {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<RecordNews> {
        // The layout's seven textures (0x72248): two of its own; three of the pack
        // that is always there, the sheet of small pictures and the figures among
        // them; one that nothing is drawn with; and one more of its own.
        let own = |file: usize| interface::file(PACK, file, "ptm");
        let shared = |file: usize| interface::file(interface::RESIDENT_PACK, file, "ptm");
        let textures = [own(3), own(4), shared(7), shared(20), shared(9), own(3), own(5)];
        let layout = interface::layout_with(data, images, PACK, 0, &textures)?;
        let (_, figures) = interface::load_texture(data, images, &shared(9))?;
        let small = interface::load_texture(data, images, &shared(20))?;
        Ok(RecordNews { layout, figures: figures.x, small, panel: None })
    }

    /// Whether a panel is up (0x14B29C), which some things wait for.
    pub fn busy(&self) -> bool {
        self.panel.is_some()
    }

    /// The panel that is up and the record it tells of.
    pub fn up(&self) -> Option<(Kind, i32)> {
        self.panel.as_ref().map(|panel| (panel.kind, panel.record))
    }

    /// Looks for news of a record and has its panel come (0x14B080); nothing while
    /// one is up.
    pub fn tell(&mut self, records: &Records, progress: &mut Progress, fonts: &Fonts, colours: &TextColours) -> bool {
        if self.busy() {
            return false;
        }
        let Some((number, news)) = records.take_news(progress) else { return false };
        let Some(record) = records.record(number) else { return false };
        let (kind, share) = match news {
            News::Opened => (Kind::Opened, 0.0),
            News::Counted => {
                let share = progress.variable(COUNT + number) as f32 / record.goal as f32;
                (if share < 1.0 { Kind::Counted } else { Kind::Cleared }, share)
            }
        };
        let name = fonts.write(&NAME_STYLE, &text::pages(&record.name, &text::Plain).swap_remove(0), colours);
        let player = self.layout.player(kind.sets().0);
        self.panel = Some(Panel { kind, record: number, icon: record.icon, name, share, phase: Phase::Coming, stayed: 0.0, player, unheard: true });
        true
    }

    /// A frame of it (0x14B39C).
    pub fn step(&mut self, board: &mut Board) {
        let layout = &self.layout;
        let Some(panel) = &mut self.panel else { return };
        // What has stayed its time is told to go (0x7252C, 0x73074).
        if panel.phase == Phase::Stayed {
            panel.phase = Phase::Going;
            panel.player.play(layout, panel.kind.sets().1);
        }
        match panel.phase {
            Phase::Coming => {
                if panel.player.advance(layout, FRAMES_A_STEP) {
                    (panel.phase, panel.stayed) = (Phase::Staying, 0.0);
                }
                if std::mem::take(&mut panel.unheard) {
                    board.play(COMMON_BANK, panel.kind.sound(), FULL_VOLUME);
                }
            }
            Phase::Staying => {
                if panel.stayed < STAY {
                    panel.stayed += FRAMES_A_STEP;
                } else {
                    panel.phase = Phase::Stayed;
                }
            }
            Phase::Stayed => {}
            Phase::Going => {
                if panel.player.advance(layout, FRAMES_A_STEP) {
                    panel.phase = Phase::Gone;
                }
            }
            Phase::Gone => self.panel = None,
        }
    }

    /// Draws the panel that is up (0x726B8, 0x731E4).
    pub fn draw(&self, canvas: &mut Canvas, fonts: &Fonts) {
        let (layout, Some(panel)) = (&self.layout, &self.panel) else { return };
        if panel.phase == Phase::Gone {
            return;
        }
        let player = &panel.player;
        // Where a track's group has its object, how far the track has moved it,
        // and the colour it is drawn by.
        let placed = |track: usize, group: usize| {
            let (moved, colour) = layout.placed(player, track)?;
            let object = layout.first_of(group)?;
            Some((layout.place(object)?, moved.translation, modulate(colour, layout.colour(object))))
        };
        for track in 0..player.len() {
            let Some(group) = player.group(track) else { continue };
            match group {
                // Written without a shadow where the group is (0x7296C, 0x73464).
                NAME | OPENED_NAME => {
                    if let Some((at, moved, tint)) = placed(track, group) {
                        fonts.draw(canvas, layer::RECORD_NEWS, &panel.name, &Placing { tint, ..Placing::at((at + moved).trunc()) });
                    }
                }
                SMALL_PICTURE | OPENED_SMALL_PICTURE => {
                    if let Some((at, moved, tint)) = placed(track, group) {
                        draw_small_picture(canvas, layer::RECORD_NEWS, &self.small, panel.icon, at + moved.trunc(), tint);
                    }
                }
                // The bar, cut to how far the count has come where it lies at
                // rest, wherever it is on its way (0x7297C).
                BAR => {
                    let Some(bar) = layout.first_of(group).map(|object| &layout.objects.objects[object]) else { continue };
                    let from = Vec2::new(bar.position[0] as f32, bar.position[1] as f32);
                    let across = (bar.size[0] as f32 * panel.share).trunc();
                    if across > 0.0 {
                        canvas.cut_to(Some(Rect::from_corners(from, from + Vec2::new(across, bar.size[1] as f32))));
                        layout.draw_track(canvas, layer::RECORD_NEWS, player, track, &Look::default());
                        canvas.cut_to(None);
                    }
                }
                // The hundredths in two figures (0x72A30), each from its cell of
                // the figures' sheet: the second where the group is, the first a
                // figure's width and one to its left.
                HUNDREDTHS => {
                    let Some(width) = layout.first_of(group).map(|object| layout.objects.objects[object].size[0] as f32) else { continue };
                    let hundredths = (panel.share * 100.0) as i32;
                    for (place, figure) in [hundredths / 10, hundredths % 10].into_iter().enumerate() {
                        let inner = Affine2::from_translation(Vec2::new((place as f32 - 1.0) * (width + 1.0), 0.0));
                        let shift = Vec2::new(FIGURE_CELL * figure as f32 / self.figures, 0.0);
                        layout.draw_track(canvas, layer::RECORD_NEWS, player, track, &Look { inner, shift, ..default() });
                    }
                }
                _ => layout.draw_track(canvas, layer::RECORD_NEWS, player, track, &Look::default()),
            }
        }
    }
}

/// Has the news of the records told and the panel that is up go on. The field
/// asks for news every frame (0xEDA84), and a battle every frame of one that the
/// records are told of (0x8D038).
#[allow(clippy::too_many_arguments)]
fn step(
    mode: Res<State<Mode>>,
    source: Option<Res<BattleSource>>,
    records: Res<Records>,
    progress: Option<ResMut<Progress>>,
    fonts: Res<Fonts>,
    colours: Res<TextColours>,
    mut news: ResMut<RecordNews>,
    mut board: ResMut<Board>,
) {
    let asked = match mode.get() {
        Mode::Field => true,
        Mode::Battle => source.is_some_and(|source| !source.unrecorded()),
        // The ride on the bike asks for none.
        Mode::Ride => false,
    };
    if let (true, Some(mut progress)) = (asked, progress) {
        if news.tell(&records, &mut progress, &fonts, &colours) {
            if let Some((kind, record)) = news.up() {
                info!("the panel of the records tells of record {record}: {kind:?}");
            }
        }
    }
    news.step(&mut board);
}

fn draw(news: Res<RecordNews>, fonts: Res<Fonts>, mut canvas: ResMut<Canvas>) {
    news.draw(&mut canvas, &fonts);
}

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, step.after(crate::battle::conclude)).add_systems(Update, draw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::tables::PlayerStats;
    use crate::records::{flag, Event, World};
    use brs_engine::canvas::Vertex;

    fn game() -> Option<(RecordNews, Records, Fonts, TextColours)> {
        let data = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")).ok()?;
        let mut images = Assets::<Image>::default();
        let news = RecordNews::load(&data, &mut images).unwrap();
        Some((news, Records::load(&data).unwrap(), Fonts::load(&data, &mut images).unwrap(), TextColours::load(&data).unwrap()))
    }

    /// Has so many battles fought in a game.
    fn fight(records: &Records, progress: &mut Progress, battles: usize) {
        let mut stats = PlayerStats { level: 1, experience: 0, max_hp: 300.0, hp: 300.0, attack: 20, special: 20, defence: 20, burst_bullets: 3, dodge_heat: 15, skill_bonus: [0; 4], skills: [0; 4], weapon: 129, suit: 133 };
        let mut world = World { progress, stats: &mut stats, stage: 0 };
        for _ in 0..battles {
            records.battle_finished(Event::battle(100, false, true), &mut world).unwrap();
        }
    }

    fn extent(vertices: &[Vertex]) -> Rect {
        vertices.iter().fold(Rect::from_corners(vertices[0].position, vertices[0].position), |area, vertex| area.union_point(vertex.position))
    }

    #[test]
    fn its_layout_has_a_coming_and_a_going_for_each_panel() {
        let Some((news, ..)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let layout = &news.layout;
        assert_eq!((layout.textures.len(), layout.motions.sets.len()), (7, 6));
        let groups = |set: usize| layout.motions.sets[set].iter().map(|track| track.group as usize).collect::<Vec<_>>();
        for kind in [Kind::Counted, Kind::Cleared, Kind::Opened] {
            let (coming, going) = kind.sets();
            assert_eq!(groups(coming), groups(going), "{kind:?}");
        }
        // The bar and the hundredths are of a count that is not at its goal only.
        let has = |kind: Kind, group: usize| groups(kind.sets().0).contains(&group);
        assert!(has(Kind::Counted, BAR) && has(Kind::Counted, HUNDREDTHS) && has(Kind::Counted, NAME) && has(Kind::Counted, SMALL_PICTURE));
        assert!(!has(Kind::Cleared, BAR) && !has(Kind::Cleared, HUNDREDTHS) && has(Kind::Cleared, NAME) && has(Kind::Cleared, SMALL_PICTURE));
        assert!(has(Kind::Opened, OPENED_NAME) && has(Kind::Opened, OPENED_SMALL_PICTURE));
        assert_eq!(groups(Kind::Opened.sets().0).iter().filter(|&&group| group == OPENED_NAME).count(), 2);
        // No object is drawn with the texture there is none of.
        assert!(layout.objects.objects.iter().all(|object| object.texture != Some(5)));
        // The panels lie along the foot of the screen, on the left.
        let object = |group: usize| &layout.objects.objects[layout.first_of(group).unwrap()];
        assert_eq!((object(BAR).position, object(BAR).size), ([16, 262], [193, 2]));
        assert_eq!((object(NAME).position, object(OPENED_NAME).position), ([32, 246], [32, 247]));
    }

    #[test]
    fn a_record_opened_and_then_counted_has_a_panel_each_time() {
        let Some((mut news, records, fonts, colours)) = game() else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut progress = Progress::new_game();
        let mut board = Board::default();
        // With no news there is nothing to tell.
        assert!(!news.tell(&records, &mut progress, &fonts, &colours) && !news.busy());
        // The first battle opens the first record: that is told, and forgotten.
        fight(&records, &mut progress, 1);
        assert!(news.tell(&records, &mut progress, &fonts, &colours));
        assert_eq!(news.up(), Some((Kind::Opened, 0)));
        assert!(!progress.flag(flag::OPENED_NEWS) && records.news(&progress).is_none());
        // It comes in three frames, stays sixty and one more, and goes in three;
        // gone, it is waited for one frame more. For the last two nothing is
        // drawn.
        let mut frames = 0;
        let mut drawn = 0;
        while news.busy() {
            // Nothing else is told while it is up.
            assert!(!news.tell(&records, &mut progress, &fonts, &colours));
            news.step(&mut board);
            frames += 1;
            let mut canvas = Canvas::default();
            news.draw(&mut canvas, &fonts);
            drawn += usize::from(!canvas.vertices().is_empty());
            assert!(frames < 200);
        }
        assert_eq!((frames, drawn), (68, 66));
        // The second battle is a count that is news: half of its goal.
        fight(&records, &mut progress, 1);
        assert!(news.tell(&records, &mut progress, &fonts, &colours));
        assert_eq!(news.up(), Some((Kind::Counted, 0)));
        assert!(!progress.flag(flag::NEWS));
        for _ in 0..10 {
            news.step(&mut board);
        }
        let mut canvas = Canvas::default();
        news.draw(&mut canvas, &fonts);
        let all = extent(canvas.vertices());
        assert!(all.min.x >= 5.0 && all.min.y >= 233.0 && all.max.x <= 218.0 && all.max.y <= 271.0, "{all:?}");
        while news.busy() {
            news.step(&mut board);
        }
        // The fourth clears it, which has the other panel.
        fight(&records, &mut progress, 2);
        assert!(news.tell(&records, &mut progress, &fonts, &colours));
        assert_eq!(news.up(), Some((Kind::Cleared, 0)));
    }
}
