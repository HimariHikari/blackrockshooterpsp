//! Makes the port ready to play from a disc image of the game.
//!
//! It is what the setup program does in its black window, and what the game does
//! in the window it opens when it finds none of its data. Into the folder go the
//! game's data, unpacked (`game/unpacked`), the films and the few other files
//! the game reads as they are on the disc (`game/extracted`), and the recordings
//! made playable. The recordings are decoded by ffmpeg: where there is none on
//! the computer, one is fetched into the folder (`tools`), on Windows. Nothing
//! of the disc image is changed. Done again for the same folder, it goes on
//! where it left off.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};

use crate::iso::{Entry, Iso};
use crate::sound;
use crate::unpack::{process, Options, Stats};

/// The game's data on the disc, which is unpacked; its films, which the game
/// reads as they are; and the disc's own folder, whose few files (the icon among
/// them) are kept too.
const GAME_DATA: &str = "PSP_GAME/USRDIR/GAMEDATA/";
const FILMS: &str = "PSP_GAME/USRDIR/MOVIE/";
const DISC: &str = "PSP_GAME/";
/// Where the two go in the folder.
pub const UNPACKED: &str = "game/unpacked";
pub const EXTRACTED: &str = "game/extracted";
/// The file that says the data was unpacked, all of it.
const DONE: &str = "unpacked.txt";
/// The file that says a setup was begun in the folder and is not through: it
/// names the disc image, so that the setup can be gone on with unasked.
pub const BEGUN: &str = "game/setup-begun.txt";
/// How many recordings are converted at once unless it is said otherwise.
pub const JOBS: usize = 3;
/// Where ffmpeg is kept in the folder when it had to be fetched, where a build
/// of it for Windows is fetched from, and about how many bytes that is.
const TOOLS: &str = "tools";
const FFMPEG_ARCHIVE: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
pub const FFMPEG_BYTES: u64 = 115 << 20;

/// What is to be made of what.
pub struct Asked {
    /// The disc image of the game.
    pub image: PathBuf,
    /// The folder the game is made in.
    pub folder: PathBuf,
    /// Whether the recordings are made playable.
    pub sound: bool,
    /// How many of them at once.
    pub jobs: usize,
}

/// What the setup is doing, for whoever shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    /// Copying what the game reads as it is on the disc.
    Films,
    /// Unpacking the data: so many of the disc's files are through.
    Data { done: usize, of: usize },
    /// Fetching ffmpeg: so many bytes have come.
    Ffmpeg { fetched: u64 },
    /// Making the recordings playable.
    Sound { done: usize, of: usize },
}

/// What became of the game's sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// The recordings are playable.
    Made,
    /// It was asked that they be left for later.
    LeftOut,
    /// There is no ffmpeg and none could be fetched: the game plays without sound.
    NoFfmpeg,
}

/// Makes the game in the folder, telling `report` what it is at as it goes.
pub fn make(asked: &Asked, report: &(dyn Fn(Doing) + Sync)) -> Result<Sound> {
    let mut iso = Iso::open(&asked.image)?;
    let files = iso.files().to_vec();
    let data: Vec<&Entry> = files.iter().filter(|file| file.path.starts_with(GAME_DATA)).collect();
    ensure!(!data.is_empty(), "this disc image has no {GAME_DATA}: is it Black Rock Shooter: The Game?");
    let folder = &asked.folder;
    let begun = folder.join(BEGUN);
    fs::create_dir_all(begun.parent().context("the folder has no place for the game")?).with_context(|| format!("making {}", folder.display()))?;
    fs::write(&begun, asked.image.to_string_lossy().as_bytes()).with_context(|| format!("writing in {}", folder.display()))?;

    // What the game reads as it is on the disc.
    report(Doing::Films);
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

    // The data, unpacked.
    let unpacked = folder.join(UNPACKED);
    if !unpacked.join(DONE).is_file() {
        let (mut stats, options) = (Stats::default(), Options { png: false, split_pdk: false });
        for (number, file) in data.iter().enumerate() {
            report(Doing::Data { done: number, of: data.len() });
            let name = &file.path[GAME_DATA.len()..];
            stats.source_files += 1;
            if let Err(error) = process(&iso.read(file)?, &unpacked.join(name), &options, &mut stats) {
                stats.errors.push(format!("{name}: {error:#}"));
            }
        }
        if !stats.errors.is_empty() {
            bail!("{} of the disc's files could not be unpacked, the first: {}", stats.errors.len(), stats.errors[0]);
        }
        fs::write(unpacked.join(DONE), format!("Unpacked whole from {}.\n", asked.image.display()))?;
    }
    report(Doing::Data { done: data.len(), of: data.len() });

    // The recordings, which ffmpeg decodes.
    let sound = if !asked.sound {
        Sound::LeftOut
    } else if ffmpeg_is_there(folder, report) {
        sound::convert_reporting(&unpacked, asked.jobs, &|done, of| report(Doing::Sound { done, of }))?;
        Sound::Made
    } else {
        Sound::NoFfmpeg
    };
    let _ = fs::remove_file(&begun);
    Ok(sound)
}

/// The disc image a setup that was begun in the folder and is not through was
/// begun from, if that is still where it was.
pub fn begun_from(folder: &Path) -> Option<PathBuf> {
    let image = PathBuf::from(fs::read_to_string(folder.join(BEGUN)).ok()?.trim());
    image.is_file().then_some(image)
}

/// Whether ffmpeg can be run: one that is on the computer, one this fetched
/// into the folder before, or, on Windows, one it fetches now.
fn ffmpeg_is_there(folder: &Path, report: &(dyn Fn(Doing) + Sync)) -> bool {
    let runs = || {
        let mut ffmpeg = Command::new("ffmpeg");
        ffmpeg.arg("-version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        unseen(&mut ffmpeg);
        ffmpeg.status().is_ok()
    };
    if runs() {
        return true;
    }
    let tools = folder.join(TOOLS);
    let program = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);
    if find(&tools, &program, 4).is_none() && cfg!(windows) {
        report(Doing::Ffmpeg { fetched: 0 });
        if let Err(error) = fetch_ffmpeg(&tools, report) {
            eprintln!("ffmpeg could not be fetched: {error:#}");
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
fn fetch_ffmpeg(tools: &Path, report: &(dyn Fn(Doing) + Sync)) -> Result<()> {
    fs::create_dir_all(tools)?;
    let archive = tools.join("ffmpeg.zip");
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into())).join("System32");
    let mut curl = Command::new(system.join("curl.exe"));
    curl.args(["-L", "--fail", "-sS", "-o"]).arg(&archive).arg(FFMPEG_ARCHIVE).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    unseen(&mut curl);
    let mut fetching = curl.spawn().context("running curl")?;
    let fetched = loop {
        if let Some(ended) = fetching.try_wait()? {
            break ended;
        }
        report(Doing::Ffmpeg { fetched: fs::metadata(&archive).map_or(0, |file| file.len()) });
        std::thread::sleep(Duration::from_millis(250));
    };
    if !fetched.success() {
        let mut said = String::new();
        if let Some(mut errors) = fetching.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut errors, &mut said);
        }
        let _ = fs::remove_file(&archive);
        bail!("the download did not come through ({})", said.trim());
    }
    let mut tar = Command::new(system.join("tar.exe"));
    tar.arg("-xf").arg(&archive).arg("-C").arg(tools).stdin(Stdio::null());
    unseen(&mut tar);
    if !tar.status().is_ok_and(|status| status.success()) {
        // Windows before it had tar: PowerShell can do it too.
        let script = format!("Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force", archive.display(), tools.display());
        let mut powershell = Command::new("powershell");
        powershell.args(["-NoProfile", "-Command", &script]).stdin(Stdio::null());
        unseen(&mut powershell);
        ensure!(powershell.status().context("unpacking the download")?.success(), "the download could not be unpacked");
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

/// Has a command run without a window of its own.
#[cfg(windows)]
fn unseen(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn unseen(_: &mut Command) {}

/// What a window of Windows' own for choosing something answers, if there is
/// such a window here and something was chosen in it. `window` is the
/// PowerShell that makes it, as `$window`. It is given an owner that stays over
/// everything, so that it does not come up behind the window it was asked from.
pub fn chosen_in_a_window(window: &str) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let script = format!(
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; {window}; \
         $over = New-Object System.Windows.Forms.Form; $over.TopMost = $true; \
         if ($window.ShowDialog($over) -eq 'OK') {{ [Console]::Out.Write($(if ($window.FileName) {{ $window.FileName }} else {{ $window.SelectedPath }})) }}"
    );
    let mut powershell = Command::new("powershell");
    powershell.args(["-NoProfile", "-STA", "-Command", &script]).stdin(Stdio::null()).stderr(Stdio::null());
    unseen(&mut powershell);
    let answer = powershell.output().ok()?;
    let chosen = String::from_utf8_lossy(&answer.stdout).trim().to_owned();
    (!chosen.is_empty()).then_some(chosen)
}

/// The disc image chosen in a window of Windows' own for opening a file, if one
/// was.
pub fn choose_the_image() -> Option<PathBuf> {
    let window = "$window = New-Object System.Windows.Forms.OpenFileDialog; \
                  $window.Title = 'Choose the disc image (.iso) of Black Rock Shooter: The Game'; \
                  $window.Filter = 'Disc images (*.iso)|*.iso|All files (*.*)|*.*'";
    chosen_in_a_window(window).map(PathBuf::from)
}
