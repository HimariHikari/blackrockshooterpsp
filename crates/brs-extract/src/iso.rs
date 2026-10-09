//! Reads a disc image (ISO 9660) as far as a PSP's game disc needs: the first
//! volume descriptor, and the folders under its root with their files, each of
//! which lies on the disc in one piece.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{ensure, Context, Result};

const SECTOR: usize = 2048;
/// The sector the volume's descriptors begin at.
const DESCRIPTORS: u64 = 16;
/// Where the first descriptor has the record of the disc's root folder.
const ROOT_RECORD: usize = 156;
/// How deep the folders of a disc may go.
const DEEPEST: usize = 8;

/// A file of the disc: its path with `/` between its parts, and where it lies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    sector: u32,
    pub size: u32,
}

/// A disc image that is open, and the files it has, in the order of their paths.
pub struct Iso {
    file: File,
    files: Vec<Entry>,
}

impl Iso {
    pub fn open(path: &Path) -> Result<Iso> {
        let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut descriptor = [0u8; SECTOR];
        file.seek(SeekFrom::Start(DESCRIPTORS * SECTOR as u64))?;
        file.read_exact(&mut descriptor).context("the file is too short to be a disc image")?;
        ensure!(
            descriptor[0] == 1 && &descriptor[1..6] == b"CD001",
            "this is not a disc image (.iso); one that is compressed (.cso) has to be made an .iso first"
        );
        let (sector, size) = extent(&descriptor[ROOT_RECORD..]);
        let mut iso = Iso { file, files: Vec::new() };
        iso.walk(String::new(), sector, size, 0)?;
        iso.files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(iso)
    }

    pub fn files(&self) -> &[Entry] {
        &self.files
    }

    /// The whole of a file.
    pub fn read(&mut self, entry: &Entry) -> Result<Vec<u8>> {
        let mut data = vec![0; entry.size as usize];
        self.file.seek(SeekFrom::Start(entry.sector as u64 * SECTOR as u64))?;
        self.file.read_exact(&mut data).with_context(|| format!("reading {} from the disc image", entry.path))?;
        Ok(data)
    }

    /// Goes through a folder's records, and through the folders it has.
    fn walk(&mut self, folder: String, sector: u32, size: u32, depth: usize) -> Result<()> {
        ensure!(depth <= DEEPEST, "the disc's folders go too deep");
        let records = self.read(&Entry { path: folder.clone(), sector, size })?;
        let mut at = 0;
        while at < records.len() {
            let length = records[at] as usize;
            if length == 0 {
                // No record goes over the end of a sector: the next is in the next.
                at = (at / SECTOR + 1) * SECTOR;
                continue;
            }
            let record = records.get(at..at + length).filter(|record| record.len() > 33).context("a folder of the disc is cut short")?;
            at += length;
            let name = record.get(33..33 + record[32] as usize).context("a name on the disc is cut short")?;
            // The folder itself and the one above it.
            if name == [0] || name == [1] {
                continue;
            }
            // A file's name ends with its version, and with a dot if it has no extension.
            let name = String::from_utf8_lossy(name);
            let name = name.split(';').next().unwrap_or_default().trim_end_matches('.');
            let path = if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") };
            let (sector, size) = extent(record);
            if record[25] & 2 != 0 {
                self.walk(path, sector, size, depth + 1)?;
            } else {
                self.files.push(Entry { path, sector, size });
            }
        }
        Ok(())
    }
}

/// Where what a record names lies and how long it is; a record keeps each in both
/// byte orders, the low end first.
fn extent(record: &[u8]) -> (u32, u32) {
    let word = |at: usize| u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]);
    (word(2), word(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder's record of something, as a disc has it.
    fn record(name: &[u8], sector: u32, size: u32, folder: bool) -> Vec<u8> {
        let mut record = vec![0u8; 33 + name.len() + (name.len() + 1) % 2];
        record[0] = record.len() as u8;
        record[2..6].copy_from_slice(&sector.to_le_bytes());
        record[10..14].copy_from_slice(&size.to_le_bytes());
        record[25] = if folder { 2 } else { 0 };
        record[32] = name.len() as u8;
        record[33..33 + name.len()].copy_from_slice(name);
        record
    }

    #[test]
    fn the_files_of_a_disc_image_are_found_with_their_paths_and_read() {
        // A root at sector 18 with a folder at 19, which has a file at 20.
        let mut image = vec![0u8; 21 * SECTOR];
        image[16 * SECTOR] = 1;
        image[16 * SECTOR + 1..16 * SECTOR + 6].copy_from_slice(b"CD001");
        let root = record(&[0], 18, SECTOR as u32, true);
        image[16 * SECTOR + ROOT_RECORD..16 * SECTOR + ROOT_RECORD + root.len()].copy_from_slice(&root);
        let mut put = |sector: usize, records: &[Vec<u8>]| {
            let all = records.concat();
            image[sector * SECTOR..sector * SECTOR + all.len()].copy_from_slice(&all);
        };
        put(18, &[record(&[0], 18, SECTOR as u32, true), record(&[1], 18, SECTOR as u32, true), record(b"PSP_GAME", 19, SECTOR as u32, true)]);
        put(19, &[record(&[0], 19, SECTOR as u32, true), record(&[1], 18, SECTOR as u32, true), record(b"ICON0.PNG;1", 20, 5, false)]);
        image[20 * SECTOR..20 * SECTOR + 5].copy_from_slice(b"hello");
        let path = std::env::temp_dir().join(format!("brs-iso-test-{}.iso", std::process::id()));
        std::fs::write(&path, &image).unwrap();
        let opened = Iso::open(&path);
        let found = opened.and_then(|mut iso| {
            let files = iso.files().to_vec();
            Ok((files.iter().map(|file| (file.path.clone(), file.size)).collect::<Vec<_>>(), iso.read(&files[0])?))
        });
        std::fs::remove_file(&path).unwrap();
        assert_eq!(found.unwrap(), (vec![("PSP_GAME/ICON0.PNG".to_owned(), 5)], b"hello".to_vec()));
        // What is no disc image is said to be none.
        let path = std::env::temp_dir().join(format!("brs-iso-test-{}.bin", std::process::id()));
        std::fs::write(&path, vec![0u8; 17 * SECTOR]).unwrap();
        let opened = Iso::open(&path).map(|_| ());
        std::fs::remove_file(&path).unwrap();
        assert!(opened.unwrap_err().to_string().contains("not a disc image"));
    }
}
