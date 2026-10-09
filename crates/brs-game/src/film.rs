//! Films: the story's films, which a map's script has played over the screen
//! (`s83` to `s86`).
//!
//! The original makes a task for a film, "Movie" (0xC3170), and hands the film to
//! the PSP's own player (`scePsmfPlayer`, 0x11863C): the task's frame has the
//! player go on (0x1189FC, 0x118B2C) and its drawing takes the player's picture
//! and draws it as it is over the whole screen (0x118B58: fifteen strips of
//! thirty-two pixels, 480 by 272 from a texture of 512). The task is number 0x37E
//! of what is drawn: over the field's own display and the windows, under the
//! menus, the screen's fade and a scene's captions. When the player says the film
//! has played through the task marks it over (0x11854C), and draws no more.
//!
//! Here the script says what it wants in the scene (`scene::FilmAsked`) and this
//! does it: the film is read and taken apart (`brs_formats::psmf`), its picture
//! decoded on a thread (`brs_engine::film`), and its sound, which `brs-extract
//! --sound` has made an Ogg Vorbis file of, played by the mixer from the moment the
//! picture starts. A film is ready when its first frame is decoded and its sound
//! can be played; it goes by the game's clock, and stands still under the pause.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use brs_engine::canvas::{Canvas, Paint, Seen, Vertex, HEIGHT, WIDTH};
use brs_engine::film::{Frame, Next, Pictures, FRAMES};

use crate::data::GameData;
use crate::field::FieldState;
use crate::interface::layer;
use crate::pause::Paused;
use crate::sound::{Board, Mixer, Request};
use crate::Mode;

/// The film that is up, if one is, and the picture it is shown by.
#[derive(Resource, Default)]
pub struct Films {
    playing: Option<Playing>,
    picture: Option<Handle<Image>>,
}

struct Playing {
    /// Its file, as the script named it.
    file: String,
    pictures: Pictures,
    /// How long it is.
    seconds: f64,
    /// Its first frame, decoded and waiting for the film to be started.
    first: Option<Frame>,
    started: bool,
    /// How long it has played, and how many of its frames have been taken.
    time: f64,
    taken: u32,
    /// The decoder has no more frames.
    ended: bool,
    /// One of its frames is up.
    shown: bool,
}

impl Playing {
    /// The frame that is to be seen now, if it is another than the one that is:
    /// the latest of those whose time has come.
    fn due(&mut self) -> Option<Frame> {
        let due = (self.time * f64::from(FRAMES.0) / f64::from(FRAMES.1)) as u32;
        let mut latest = None;
        while self.taken <= due && !self.ended {
            let frame = match self.first.take() {
                Some(first) => first,
                None => match self.pictures.next() {
                    Next::Frame(frame) => frame,
                    Next::NotYet => break,
                    Next::Over => {
                        self.ended = true;
                        break;
                    }
                },
            };
            self.taken += 1;
            latest = Some(frame);
        }
        latest
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Films>().add_systems(Update, play.run_if(in_state(Mode::Field)));
}

/// Does what the script asks about a film, and shows the one that is playing.
#[allow(clippy::too_many_arguments)]
fn play(
    data: Res<GameData>,
    time: Res<Time>,
    paused: Res<Paused>,
    mixer: Res<Mixer>,
    mut board: ResMut<Board>,
    mut state: ResMut<FieldState>,
    mut films: ResMut<Films>,
    mut images: ResMut<Assets<Image>>,
    mut canvas: ResMut<Canvas>,
    seen: Res<Seen>,
) {
    if state.scene.film.is_none() && films.playing.is_none() {
        return;
    }
    let films = &mut *films;
    let asked = &mut state.scene.film;
    // A film nobody waits for any more is stopped (0xC32A4, 0x118958); one that
    // is asked for is made ready, in place of any that was there.
    let another = match (&*asked, &films.playing) {
        (Some(asked), Some(playing)) => playing.file != asked.file,
        (wanted, playing) => wanted.is_some() != playing.is_some(),
    };
    if another {
        if films.playing.take().is_some() {
            board.ask(Request::FilmStop);
        }
        if let Some(asked) = asked.as_mut() {
            match data.film(&asked.file) {
                Ok(film) => {
                    info!("the film {} is made ready: {:.1} s", asked.file, film.seconds());
                    let sound = GameData::film_sound(&asked.file);
                    if film.sound.is_empty() {
                        debug!("the film has no sound");
                    } else if data.exists(&sound) {
                        board.ask(Request::Film(sound));
                    } else {
                        warn!("the film's sound has not been made ({sound}): run `brs-extract --sound`");
                    }
                    films.playing = Some(Playing {
                        file: asked.file.clone(),
                        seconds: film.seconds(),
                        pictures: Pictures::decode(film.picture),
                        first: None,
                        started: false,
                        time: 0.0,
                        taken: 0,
                        ended: false,
                        shown: false,
                    });
                }
                // A film that cannot be played is over at once: the script goes on.
                Err(error) => {
                    warn!("the film {} cannot be played: {error:#}", asked.file);
                    (asked.ready, asked.over) = (true, true);
                }
            }
        }
    }
    let (Some(asked), Some(playing)) = (asked.as_mut(), films.playing.as_mut()) else { return };
    if asked.over {
        return;
    }

    // Ready when its first frame is there and its sound can start with it.
    if !asked.ready {
        if playing.first.is_none() {
            match playing.pictures.next() {
                Next::Frame(frame) => playing.first = Some(frame),
                Next::NotYet => {}
                Next::Over => {
                    (asked.ready, asked.over) = (true, true);
                    return;
                }
            }
        }
        asked.ready = playing.first.is_some() && mixer.film_ready();
    }
    if asked.started && !playing.started {
        playing.started = true;
        board.ask(Request::FilmStart);
    }
    if !playing.started {
        return;
    }

    if !paused.up() {
        playing.time += time.delta_secs_f64();
    }
    if let Some(frame) = playing.due() {
        let size = Extent3d { width: frame.width, height: frame.height, depth_or_array_layers: 1 };
        let image = Image::new(size, TextureDimension::D2, frame.rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
        match &films.picture {
            Some(picture) => {
                if let Err(error) = images.insert(picture, image) {
                    warn!("a frame of the film could not be put up: {error}");
                }
            }
            None => films.picture = Some(images.add(image)),
        }
        playing.shown = true;
    }
    // Played through: nothing more of it is drawn, and the script that waits for
    // it goes on.
    if playing.time >= playing.seconds || (playing.ended && playing.first.is_none()) {
        info!("the film {} is over", playing.file);
        asked.over = true;
        return;
    }
    if let (Some(picture), true) = (&films.picture, playing.shown) {
        // A window of another shape than the screen shows something beside it,
        // which the PSP has nothing of: it is black while a film is up.
        canvas.fill(layer::FILM, seen.0, [0, 0, 0, 255]);
        let white = [255; 4];
        let (from, to) = (Vertex::new(Vec2::ZERO, Vec2::ZERO, white), Vertex::new(Vec2::new(WIDTH, HEIGHT), Vec2::ONE, white));
        canvas.sprite(layer::FILM, Paint::textured(picture), from, to);
    }
}
