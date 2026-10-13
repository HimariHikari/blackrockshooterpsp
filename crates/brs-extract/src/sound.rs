//! Turns the game's recordings into something the port can play.
//!
//! The music and everything spoken is ATRAC3plus, Sony's codec, which the port has no
//! decoder for. ffmpeg has: this has it make an Ogg Vorbis file beside each
//! recording, cut to the samples the recording says are its own (a decoder gives
//! some before the first of them, and a part of a frame after the last). The port
//! then reads the recording for its length and its loop, and the `.ogg` for its sound.
//!
//! ffmpeg is run at idle priority and a few at a time, so that this can go on beside
//! whatever else the machine is doing.
//!
//! The films (`MOVIE/*.PMF`, beside the tree that is unpacked) have their sound in
//! the same codec, inside the film. It is taken out of each, and its `.ogg` is put
//! in `MOVIE` under what was unpacked; the picture the port reads from the film.
//!
//! A pack that was unpacked before its kind was known lies in the tree as one file
//! (the `PDK` packs of the ride on the bike, in a tree made before the extractor
//! took them apart). The recordings inside such a one are converted too, into a
//! folder beside the pack, `<pack>.sound`, each under its member's number; the
//! pack itself is left as it is.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use brs_formats::at3::{self, Recording};
use brs_formats::container;
use brs_formats::psmf::{self, Film};

/// The quality Vorbis is made at: about 110 kbit/s for two channels, which is more
/// than the recordings themselves have (64 kbit/s).
const QUALITY: &str = "3";

#[derive(Default)]
struct Tally {
    made: usize,
    there: usize,
    failed: Vec<String>,
}

/// Converts every recording under `root` that has no `.ogg` beside it yet, `jobs` at
/// a time, and says how far it is now and then.
pub fn convert(root: &Path, jobs: usize) -> Result<()> {
    convert_reporting(root, jobs, &|done, of| {
        if done % 250 == 0 {
            println!("  {done} of {of}");
        }
    })
}

/// The same, telling `report` after each recording how many are through, and of
/// how many.
pub fn convert_reporting(root: &Path, jobs: usize, report: &(dyn Fn(usize, usize) + Sync)) -> Result<()> {
    let mut ffmpeg = Command::new("ffmpeg");
    ffmpeg.arg("-version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    quietly(&mut ffmpeg);
    if ffmpeg.status().is_err() {
        bail!("ffmpeg is not on the PATH: it is what decodes the game's recordings");
    }
    let mut recordings = Vec::new();
    collect(root, &mut recordings).with_context(|| format!("reading {}", root.display()))?;
    recordings.sort();
    println!("recordings   : {}", recordings.len());

    let (next, tally) = (AtomicUsize::new(0), Mutex::new(Tally::default()));
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| loop {
                let Some(path) = recordings.get(next.fetch_add(1, Ordering::Relaxed)) else { return };
                let result = convert_one(path);
                let mut tally = tally.lock().unwrap();
                match result {
                    Ok(true) => tally.made += 1,
                    Ok(false) => tally.there += 1,
                    Err(error) => tally.failed.push(format!("{}: {error:#}", path.display())),
                }
                report(tally.made + tally.there + tally.failed.len(), recordings.len());
            });
        }
    });
    let tally = tally.into_inner().unwrap();
    println!("  converted  : {}", tally.made);
    println!("  were there : {}", tally.there);
    println!("  failed     : {}", tally.failed.len());
    for failure in &tally.failed {
        println!("    {failure}");
    }
    convert_packed(root)?;
    convert_films(root)
}

/// The folder the recordings of a pack that lies as one file are converted into.
pub fn converted_beside(pack: &Path) -> PathBuf {
    let mut beside = pack.as_os_str().to_owned();
    beside.push(".sound");
    PathBuf::from(beside)
}

/// Every `PDK` pack under `dir` that lies there as one file.
fn collect_packs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_packs(&path, out)?;
        } else {
            use std::io::Read;
            let mut head = [0u8; 4];
            let read = fs::File::open(&path).and_then(|mut file| file.read(&mut head))?;
            if head[..read].starts_with(container::PDK_MAGIC) {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Converts the recordings inside the packs that lie as one file, each into
/// `<pack>.sound/<member's number>.ogg`.
fn convert_packed(root: &Path) -> Result<()> {
    let mut packs = Vec::new();
    collect_packs(root, &mut packs)?;
    packs.sort();
    let (mut made, mut there, mut failed) = (0, 0, Vec::new());
    for pack in &packs {
        let bytes = fs::read(pack)?;
        let Ok(members) = container::parse_pdk(&bytes) else { continue };
        let folder = converted_beside(pack);
        for (number, member) in members.iter().enumerate().filter(|(_, member)| at3::is_recording(member)) {
            let output = folder.join(format!("{number:03}.ogg"));
            if fs::metadata(&output).is_ok_and(|file| file.len() > 0) {
                there += 1;
                continue;
            }
            let converted = (|| -> Result<()> {
                fs::create_dir_all(&folder)?;
                let recording = Recording::parse(member)?;
                let taken_out = output.with_extension("at3.part");
                fs::write(&taken_out, member)?;
                let encoded = encode(&taken_out, &output, u64::from(recording.lead_in), u64::from(recording.samples));
                let _ = fs::remove_file(&taken_out);
                encoded
            })();
            match converted {
                Ok(()) => made += 1,
                Err(error) => failed.push(format!("{} member {number}: {error:#}", pack.display())),
            }
        }
    }
    println!("in packs     : {}", made + there + failed.len());
    println!("  converted  : {made}");
    println!("  were there : {there}");
    println!("  failed     : {}", failed.len());
    for failure in &failed {
        println!("    {failure}");
    }
    Ok(())
}

/// The file a recording's sound is kept in once converted.
pub fn converted(recording: &Path) -> PathBuf {
    recording.with_extension("ogg")
}

/// Every recording under `dir`: the files that start as one does.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_none_or(|extension| extension != "ogg" && extension != "part") {
            use std::io::Read;
            let mut head = [0u8; 12];
            let read = fs::File::open(&path).and_then(|mut file| file.read(&mut head))?;
            if at3::is_recording(&head[..read]) {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Converts one recording; false if it had been converted before.
fn convert_one(path: &Path) -> Result<bool> {
    let output = converted(path);
    if fs::metadata(&output).is_ok_and(|file| file.len() > 0) {
        return Ok(false);
    }
    let recording = Recording::parse(&fs::read(path)?)?;
    encode(path, &output, u64::from(recording.lead_in), u64::from(recording.samples))?;
    Ok(true)
}

/// Has ffmpeg make `output` of the recording `input`: its `samples` from the one
/// numbered `first` of what a decoder gives.
fn encode(input: &Path, output: &Path, first: u64, samples: u64) -> Result<()> {
    // Written under another name first, so that a run cut short leaves no half file
    // to be taken for a whole one.
    let partial = output.with_extension("ogg.part");
    // The cut keeps the recording's own samples and numbers them anew: with the
    // times the decoder gives them the file comes out saying it is half as long.
    let cut = format!("atrim=start_sample={first}:end_sample={},asetpts=N/SR/TB", first + samples);
    let mut command = Command::new("ffmpeg");
    command
        .args(["-v", "error", "-nostdin", "-y", "-i"])
        .arg(input)
        .args(["-af", &cut, "-c:a", "libvorbis", "-q:a", QUALITY, "-f", "ogg"])
        .arg(&partial)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    quietly(&mut command);
    let run = command.output().context("running ffmpeg")?;
    if !run.status.success() {
        let _ = fs::remove_file(&partial);
        bail!("ffmpeg: {}", String::from_utf8_lossy(&run.stderr).trim());
    }
    fs::rename(&partial, output)?;
    Ok(())
}

/// Where the films are, beside the tree that was unpacked into `root`, and the
/// folder under `root` their sound is kept in once converted.
const FILMS: &str = "../extracted/PSP_GAME/USRDIR/MOVIE";
const FILMS_SOUND: &str = "MOVIE";

/// The file a film's sound is kept in, by the film's name: `MOVIE/<NAME>.ogg` under
/// what was unpacked.
pub fn film_sound(root: &Path, film: &Path) -> PathBuf {
    let name = film.file_stem().unwrap_or_default().to_string_lossy().to_ascii_uppercase();
    root.join(FILMS_SOUND).join(format!("{name}.ogg"))
}

/// Converts the sound of every film that has no `.ogg` yet. A film's picture the
/// port decodes itself, from the film; its sound is the same codec as the
/// recordings', inside the film (`brs_formats::psmf`): it is written out as a
/// recording for ffmpeg, cut to what is heard from the first picture on.
fn convert_films(root: &Path) -> Result<()> {
    let folder = root.join(FILMS);
    let Ok(entries) = fs::read_dir(&folder) else {
        println!("films        : none ({} is not there)", folder.display());
        return Ok(());
    };
    let mut films: Vec<PathBuf> = entries.filter_map(|entry| Some(entry.ok()?.path())).collect();
    films.retain(|path| path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("pmf")));
    films.sort();
    println!("films        : {}", films.len());
    fs::create_dir_all(root.join(FILMS_SOUND))?;
    let (mut made, mut there, mut silent, mut failed) = (0, 0, 0, Vec::new());
    for path in &films {
        let output = film_sound(root, path);
        if fs::metadata(&output).is_ok_and(|file| file.len() > 0) {
            there += 1;
            continue;
        }
        let converted = (|| -> Result<bool> {
            let film = Film::parse(&fs::read(path)?)?;
            let Some(recording) = film.recording() else { return Ok(false) };
            let taken_apart = output.with_extension("at3.part");
            fs::write(&taken_apart, recording)?;
            let samples = film.ticks * u64::from(psmf::RATE) / psmf::TICKS;
            let encoded = encode(&taken_apart, &output, u64::from(film.lead_samples()), samples);
            let _ = fs::remove_file(&taken_apart);
            encoded.map(|()| true)
        })();
        match converted {
            Ok(true) => made += 1,
            Ok(false) => silent += 1,
            Err(error) => failed.push(format!("{}: {error:#}", path.display())),
        }
    }
    println!("  converted  : {made}");
    println!("  were there : {there}");
    println!("  no sound   : {silent}");
    println!("  failed     : {}", failed.len());
    for failure in &failed {
        println!("    {failure}");
    }
    Ok(())
}

/// Has a command run at idle priority and without a window of its own.
#[cfg(windows)]
fn quietly(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const IDLE_PRIORITY_CLASS: u32 = 0x0000_0040;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(IDLE_PRIORITY_CLASS | CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn quietly(_: &mut Command) {}
