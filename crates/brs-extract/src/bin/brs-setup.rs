//! Makes the port ready to play from a disc image of the game.
//!
//! Usage: `brs-setup [the game's .iso] [the folder to make the port in] [--no-sound] [--jobs <n>]`
//!
//! It needs no command line: started by a double click it asks for the disc
//! image and for the folder in windows of the system's own, and a disc image
//! can be dropped on it instead. Into the folder go the game's data, unpacked
//! (`game/unpacked`), the films and the few other files the game reads as they
//! are on the disc (`game/extracted`), the recordings made playable, and the
//! game's program if it lies beside this one. The recordings are decoded by
//! ffmpeg: where there is none on the computer, one is fetched into the folder
//! (`tools`), on Windows. Nothing of the disc image is changed. Run again for
//! the same folder, it goes on where it left off.

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
/// Where ffmpeg is kept in the folder when it had to be fetched, and where a
/// build of it for Windows is fetched from (about 115 MB).
const TOOLS: &str = "tools";
const FFMPEG_ARCHIVE: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";

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
    println!("Black Rock Shooter: The Game, the PC port: setting up from your disc image.\n");
    let image = match asked.image {
        Some(image) => image,
        None => ask_for_the_image()?,
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
    println!("Making the game in {}", folder.display());

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
    println!("  1 of 4: the films are copied");

    // The data, unpacked.
    let unpacked = folder.join(UNPACKED);
    if unpacked.join(DONE).is_file() {
        println!("  2 of 4: the data is unpacked already");
    } else {
        let (mut stats, options) = (Stats::default(), Options { png: false, split_pdk: false });
        for (number, file) in data.iter().enumerate() {
            let name = &file.path[GAME_DATA.len()..];
            stats.source_files += 1;
            if let Err(error) = process(&iso.read(file)?, &unpacked.join(name), &options, &mut stats) {
                stats.errors.push(format!("{name}: {error:#}"));
            }
            if number % 20 == 0 {
                print!("\r  2 of 4: unpacking the data, {} of {} files", number + 1, data.len());
                let _ = io::stdout().flush();
            }
        }
        println!("\r  2 of 4: the data is unpacked ({} files of the disc)        ", stats.source_files);
        if !stats.errors.is_empty() {
            for error in &stats.errors {
                eprintln!("    {error}");
            }
            bail!("{} of the disc's files could not be unpacked", stats.errors.len());
        }
        fs::write(unpacked.join(DONE), format!("Unpacked whole by brs-setup from {}.\n", image.display()))?;
    }

    // The recordings, which ffmpeg decodes.
    if !asked.sound {
        println!("  3 of 4: the sound is left for later (--no-sound)");
    } else if ffmpeg_is_there(&folder) {
        println!("  3 of 4: making the music and the voices playable. This is the longest part, a few minutes;");
        println!("          it can be closed and started again for the same folder, and goes on where it was.");
        sound::convert(&unpacked, asked.jobs)?;
    } else {
        println!("  3 of 4: ffmpeg, which decodes the game's sound, could not be had, so the game will play without sound.");
        println!("          Install it (https://ffmpeg.org) and run this again for the same folder.");
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
            println!("  4 of 4: the game is in place\n\nDone. To play, open {}", to.display());
        }
        None => {
            println!("  4 of 4: the data is ready, but the game's program ({program}) is not beside this one.");
            println!("          Put it in {} and open it there.", folder.display());
        }
    }
    Ok(())
}

/// Whether ffmpeg can be run: one that is on the computer, one this fetched
/// into the folder before, or, on Windows, one it fetches now.
fn ffmpeg_is_there(folder: &Path) -> bool {
    let runs = || Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
    if runs() {
        return true;
    }
    let tools = folder.join(TOOLS);
    let program = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);
    if find(&tools, &program, 4).is_none() && cfg!(windows) {
        println!("  3 of 4: fetching ffmpeg, which decodes the game's sound (about 115 MB, from gyan.dev, into {})", tools.display());
        if let Err(error) = fetch_ffmpeg(&tools) {
            println!("          it could not be fetched: {error:#}");
        }
    }
    let Some(found) = find(&tools, &program, 4) else { return false };
    // What makes the recordings playable runs it by its name.
    let mut path = found.parent().map(|bin| bin.as_os_str().to_owned()).unwrap_or_default();
    if let Some(old) = std::env::var_os("PATH") {
        path.push(if cfg!(windows) { ";" } else { ":" });
        path.push(old);
    }
    std::env::set_var("PATH", path);
    runs()
}

/// Fetches a build of ffmpeg for Windows and unpacks it, with the tools Windows
/// has for both.
fn fetch_ffmpeg(tools: &Path) -> Result<()> {
    fs::create_dir_all(tools)?;
    let archive = tools.join("ffmpeg.zip");
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into())).join("System32");
    let fetched = Command::new(system.join("curl.exe")).args(["-L", "--fail", "--progress-bar", "-o"]).arg(&archive).arg(FFMPEG_ARCHIVE).status().context("running curl")?;
    ensure!(fetched.success(), "the download did not come through");
    let unpacked = Command::new(system.join("tar.exe")).arg("-xf").arg(&archive).arg("-C").arg(tools).status().is_ok_and(|status| status.success());
    if !unpacked {
        // Windows before it had tar: PowerShell can do it too.
        let script = format!("Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force", archive.display(), tools.display());
        let status = Command::new("powershell").args(["-NoProfile", "-Command", &script]).status().context("unpacking the download")?;
        ensure!(status.success(), "the download could not be unpacked");
    }
    let _ = fs::remove_file(&archive);
    Ok(())
}

/// A file of a name somewhere under a folder, so many folders down at most.
fn find(folder: &Path, name: &str, deep: usize) -> Option<PathBuf> {
    let mut folders = Vec::new();
    for entry in fs::read_dir(folder).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            folders.push(path);
        } else if path.file_name().is_some_and(|file| file.eq_ignore_ascii_case(name)) {
            return Some(path);
        }
    }
    folders.into_iter().filter(|_| deep > 0).find_map(|folder| find(&folder, name, deep - 1))
}

/// What a window of Windows' own for choosing something answers, if there is
/// such a window here and something was chosen in it.
fn chosen_in_a_window(window: &str) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let script = format!(
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; {window}; \
         if ($window.ShowDialog() -eq 'OK') {{ [Console]::Out.Write($(if ($window.FileName) {{ $window.FileName }} else {{ $window.SelectedPath }})) }}"
    );
    let answer = Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]).stderr(Stdio::null()).output().ok()?;
    let chosen = String::from_utf8_lossy(&answer.stdout).trim().to_owned();
    (!chosen.is_empty()).then_some(chosen)
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
    let window = "$window = New-Object System.Windows.Forms.OpenFileDialog; \
                  $window.Title = 'Choose the disc image (.iso) of Black Rock Shooter: The Game'; \
                  $window.Filter = 'Disc images (*.iso)|*.iso|All files (*.*)|*.*'";
    match chosen_in_a_window(window) {
        Some(chosen) => Ok(chosen.into()),
        None => typed("Type where the disc image (.iso) is (or drop it on this window), then press Enter:"),
    }
}

/// Asks which folder the game is to be made in.
fn ask_for_the_folder() -> Result<PathBuf> {
    println!("Choose the folder to make the game in (a window has opened for it).");
    let window = "$window = New-Object System.Windows.Forms.FolderBrowserDialog; \
                  $window.Description = 'Choose the folder to make the game in (about 1.4 GB)'; \
                  $window.ShowNewFolderButton = $true";
    match chosen_in_a_window(window) {
        Some(chosen) => Ok(chosen.into()),
        None => typed("Type the folder to make the game in (or drop the folder on this window), then press Enter:"),
    }
}
