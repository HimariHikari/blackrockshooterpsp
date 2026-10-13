//! Makes the port ready to play from a disc image of the game, in a folder of
//! one's choosing.
//!
//! Usage: `brs-setup [the game's .iso] [the folder to make the port in] [--no-sound] [--jobs <n>]`
//!
//! The game does the same by itself, in a window, when it is started where its
//! data is not: this is for making it somewhere else, or without a window. It
//! needs no command line either: started by a double click it asks for the disc
//! image and for the folder in windows of the system's own, and a disc image
//! can be dropped on it instead. What goes into the folder, and how, is
//! `brs_extract::setup`; the game's program is put there too if it lies beside
//! this one.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use brs_extract::setup::{self, Doing, Sound};

struct Asked {
    image: Option<PathBuf>,
    folder: Option<PathBuf>,
    sound: bool,
    jobs: usize,
}

fn main() {
    let asked = match parse() {
        Ok(asked) => asked,
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(2);
        }
    };
    // Started by a double click or by dropping a file on it, its window goes as
    // it ends: it waits then.
    let waits = asked.folder.is_none();
    let outcome = run(asked);
    if let Err(error) = &outcome {
        eprintln!("\nIt did not work: {error:#}");
    }
    if waits {
        println!("\nPress Enter to close.");
        let _ = io::stdin().lock().lines().next();
    }
    if outcome.is_err() {
        std::process::exit(1);
    }
}

fn parse() -> Result<Asked> {
    let mut asked = Asked { image: None, folder: None, sound: true, jobs: setup::JOBS };
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-sound" => asked.sound = false,
            "--jobs" => asked.jobs = arguments.next().context("--jobs needs a number")?.parse().context("--jobs takes a number")?,
            _ if argument.starts_with("--") => bail!("unknown option {argument}"),
            _ if asked.image.is_none() => asked.image = Some(argument.into()),
            _ if asked.folder.is_none() => asked.folder = Some(argument.into()),
            _ => bail!("too much was given: the disc image and the folder are all it takes"),
        }
    }
    Ok(asked)
}

fn run(asked: Asked) -> Result<()> {
    println!("Black Rock Shooter: The Game, the PC port: setting up from your disc image.\n");
    let image = match asked.image {
        Some(image) => image,
        None => ask_for_the_image()?,
    };
    let folder = match asked.folder {
        Some(folder) => folder,
        None => ask_for_the_folder()?,
    };
    println!("Making the game in {}", folder.display());

    let asked = setup::Asked { image, folder, sound: asked.sound, jobs: asked.jobs };
    let sound = setup::make(&asked, &said)?;
    println!();
    match sound {
        Sound::Made => println!("  3 of 4: the music and the voices are playable"),
        Sound::LeftOut => println!("  3 of 4: the sound is left for later (--no-sound)"),
        Sound::NoFfmpeg => {
            println!("  3 of 4: ffmpeg, which decodes the game's sound, could not be had, so the game will play without sound.");
            println!("          Install it (https://ffmpeg.org) and run this again for the same folder.");
        }
    }

    // The game itself.
    let folder = &asked.folder;
    let program = format!("brs-game{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe().ok().and_then(|own| Some(own.parent()?.join(&program))).filter(|beside| beside.is_file());
    match beside {
        Some(beside) => {
            let to = folder.join(&program);
            if fs::canonicalize(&beside).ok() != fs::canonicalize(&to).ok() {
                fs::copy(&beside, &to).with_context(|| format!("copying the game to {}", to.display()))?;
            }
            println!("  4 of 4: the game is in place\n\nDone. To play, open {}", to.display());
        }
        None => {
            println!("  4 of 4: the data is ready, but the game's program ({program}) is not beside this one.");
            println!("          Put it in {} and open it there.", folder.display());
        }
    }
    Ok(())
}

/// Says what the setup is at, on one line for each of its parts.
fn said(doing: Doing) {
    match doing {
        Doing::Films => println!("  1 of 4: copying the films"),
        Doing::Data { done, of } if done == of => println!("\r  2 of 4: the data is unpacked ({of} files of the disc)        "),
        Doing::Data { done, of } if done % 20 == 0 => print!("\r  2 of 4: unpacking the data, {} of {of} files", done + 1),
        Doing::Ffmpeg { fetched: 0 } => println!("  3 of 4: fetching ffmpeg, which decodes the game's sound (about {} MB, from gyan.dev)", setup::FFMPEG_BYTES >> 20),
        Doing::Ffmpeg { fetched } => print!("\r          {} MB", fetched >> 20),
        Doing::Sound { done: 1, .. } => {
            println!("\r  3 of 4: making the music and the voices playable. This is the longest part, a few minutes;");
            println!("          it can be closed and started again for the same folder, and goes on where it was.");
        }
        Doing::Sound { done, of } if done == of => println!("\r          {done} of {of}"),
        Doing::Sound { done, of } if done % 250 == 0 => print!("\r          {done} of {of}"),
        _ => {}
    }
    let _ = io::stdout().flush();
}

/// What is typed in answer to a question, without the quotes a dropped file
/// comes in.
fn typed(question: &str) -> Result<PathBuf> {
    println!("{question}");
    let line = io::stdin().lock().lines().next().context("nothing was given")??;
    let answer = line.trim().trim_matches('"');
    ensure!(!answer.is_empty(), "nothing was given");
    Ok(Path::new(answer).to_owned())
}

/// Asks for the disc image, when none was dropped on the program.
fn ask_for_the_image() -> Result<PathBuf> {
    println!("Choose the disc image (.iso) of your copy of the game (a window has opened for it).");
    match setup::choose_the_image() {
        Some(chosen) => Ok(chosen),
        None => typed("Type where the disc image (.iso) is (or drop it on this window), then press Enter:"),
    }
}

/// Asks which folder the game is to be made in.
fn ask_for_the_folder() -> Result<PathBuf> {
    println!("Choose the folder to make the game in (a window has opened for it).");
    let window = "$window = New-Object System.Windows.Forms.FolderBrowserDialog; \
                  $window.Description = 'Choose the folder to make the game in (about 1.4 GB)'; \
                  $window.ShowNewFolderButton = $true";
    match setup::chosen_in_a_window(window) {
        Some(chosen) => Ok(chosen.into()),
        None => typed("Type the folder to make the game in (or drop the folder on this window), then press Enter:"),
    }
}
