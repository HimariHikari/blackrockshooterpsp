//! Makes the port ready to play from a disc image of the game.
//!
//! Usage: `brs-setup <the game's .iso> [the folder to make the port in] [--no-sound] [--jobs <n>]`
//!
//! On Windows the disc image can be dropped on the program, which then asks for
//! the folder. Into that folder go the game's data, unpacked (`game/unpacked`),
//! the films and the few other files the game reads as they are on the disc
//! (`game/extracted`), the recordings made playable if ffmpeg is on the PATH,
//! and the game's program if it lies beside this one. Nothing of the disc image
//! is changed. Run again for the same folder, it goes on where it left off.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, ensure, Context, Result};
use brs_extract::iso::{Entry, Iso};
use brs_extract::sound;
use brs_extract::unpack::{process, Options, Stats};

/// The game's data on the disc, which is unpacked; its films, which the game
/// reads as they are; and the disc's own folder, whose few files (the icon among
/// them) are kept too.
const GAME_DATA: &str = "PSP_GAME/USRDIR/GAMEDATA/";
const FILMS: &str = "PSP_GAME/USRDIR/MOVIE/";
const DISC: &str = "PSP_GAME/";
const UNPACKED: &str = "game/unpacked";
const EXTRACTED: &str = "game/extracted";
/// The file that says the data was unpacked, all of it.
const DONE: &str = "unpacked.txt";
/// How many recordings are converted at once unless `--jobs` says otherwise.
const JOBS: usize = 3;

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
    // Started by dropping a file on it, its window goes as it ends: it waits then.
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
    let mut asked = Asked { image: None, folder: None, sound: true, jobs: JOBS };
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
    let Some(image) = asked.image else {
        bail!("give it the game's disc image: drop the .iso on this program, or run\n  brs-setup <game.iso> [folder]")
    };
    let mut iso = Iso::open(&image)?;
    let files = iso.files().to_vec();
    let data: Vec<&Entry> = files.iter().filter(|file| file.path.starts_with(GAME_DATA)).collect();
    ensure!(!data.is_empty(), "this disc image has no {GAME_DATA}: is it Black Rock Shooter: The Game?");
    let folder = match asked.folder {
        Some(folder) => folder,
        None => ask_for_the_folder()?,
    };
    fs::create_dir_all(&folder).with_context(|| format!("making {}", folder.display()))?;
    println!("Making the port in {}", folder.display());

    // What the game reads as it is on the disc.
    let extracted = folder.join(EXTRACTED);
    let kept = |path: &str| path.starts_with(FILMS) || path.strip_prefix(DISC).is_some_and(|rest| !rest.contains('/'));
    for file in files.iter().filter(|file| kept(&file.path)) {
        let to = extracted.join(&file.path);
        if fs::metadata(&to).is_ok_and(|there| there.len() == file.size as u64) {
            continue;
        }
        fs::create_dir_all(to.parent().context("a file of the disc has no folder")?)?;
        fs::write(&to, iso.read(file)?).with_context(|| format!("writing {}", to.display()))?;
    }
    println!("  the films and the disc's own files are in {}", extracted.display());

    // The data, unpacked.
    let unpacked = folder.join(UNPACKED);
    if unpacked.join(DONE).is_file() {
        println!("  the data is unpacked already");
    } else {
        let (mut stats, options) = (Stats::default(), Options { png: false, split_pdk: false });
        for (number, file) in data.iter().enumerate() {
            let name = &file.path[GAME_DATA.len()..];
            stats.source_files += 1;
            if let Err(error) = process(&iso.read(file)?, &unpacked.join(name), &options, &mut stats) {
                stats.errors.push(format!("{name}: {error:#}"));
            }
            if number % 20 == 0 {
                print!("\r  unpacking the data: {} of {} files", number + 1, data.len());
                let _ = io::stdout().flush();
            }
        }
        println!("\r  unpacked the data: {} files of the disc, {} archives in them", stats.source_files, stats.archives);
        if !stats.errors.is_empty() {
            for error in &stats.errors {
                eprintln!("    {error}");
            }
            bail!("{} of the disc's files could not be unpacked", stats.errors.len());
        }
        fs::write(unpacked.join(DONE), format!("Unpacked whole by brs-setup from {}.\n", image.display()))?;
    }

    // The recordings, which ffmpeg decodes.
    let has_ffmpeg = Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
    if !asked.sound {
        println!("  the sound is left for later (--no-sound)");
    } else if has_ffmpeg {
        println!("  making the recordings playable: this takes a while, and can be stopped and run again");
        sound::convert(&unpacked, asked.jobs)?;
    } else {
        println!("  ffmpeg was not found, so the game will play without sound. Install it (https://ffmpeg.org;");
        println!("  on Windows:  winget install ffmpeg), open a new window and run this again for the same folder.");
    }

    // The game itself.
    let program = format!("brs-game{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe().ok().and_then(|own| Some(own.parent()?.join(&program))).filter(|beside| beside.is_file());
    match beside {
        Some(beside) => {
            let to = folder.join(&program);
            if fs::canonicalize(&beside).ok() != fs::canonicalize(&to).ok() {
                fs::copy(&beside, &to).with_context(|| format!("copying the game to {}", to.display()))?;
            }
            println!("\nDone. Start the game with {}", to.display());
        }
        None => {
            println!("\nThe data is ready. The game's program ({program}) is not beside this one: build it with");
            println!("  cargo build --release -p brs-game");
            println!("and put it in {} (or run it with  --data \"{}\").", folder.display(), unpacked.display());
        }
    }
    Ok(())
}

/// Asks which folder the port is to be made in: with the system's own window for
/// choosing one where there is such, else in writing.
fn ask_for_the_folder() -> Result<PathBuf> {
    if cfg!(windows) {
        let script = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; \
                      $window = New-Object System.Windows.Forms.FolderBrowserDialog; \
                      $window.Description = 'Choose the folder to make the port of Black Rock Shooter: The Game in'; \
                      $window.ShowNewFolderButton = $true; \
                      if ($window.ShowDialog() -eq 'OK') { [Console]::Out.Write($window.SelectedPath) }";
        println!("Choose the folder to make the port in (a window has opened for it).");
        if let Ok(answer) = Command::new("powershell").args(["-NoProfile", "-STA", "-Command", script]).stderr(Stdio::null()).output() {
            let chosen = String::from_utf8_lossy(&answer.stdout).trim().to_owned();
            if !chosen.is_empty() {
                return Ok(chosen.into());
            }
        }
    }
    println!("Type the folder to make the port in (or drop the folder on this window), then press Enter:");
    let line = io::stdin().lock().lines().next().context("no folder was given")??;
    let folder = line.trim().trim_matches('"');
    ensure!(!folder.is_empty(), "no folder was given");
    Ok(Path::new(folder).to_owned())
}
