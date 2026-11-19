//! Read-only access to the files inside one mod, whether it is a zip or an unpacked folder.
//!
//! Entry names are normalised to `/` separators without a leading `/` or `./`, and looked up
//! case-insensitively, because the game runs on Windows and modders are inconsistent about case.
//! Only names listed by the container can be read, so a declared path such as `../x` cannot
//! reach outside the mod.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::model::ModFormat;

/// Where a mod's icon lives inside its container. Resolved by `scan`, decoded by `icons`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconLocation {
    /// Entry name as stored in the container (see [`ModContainer::find`]).
    pub rel: String,
    pub kind: IconKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconKind {
    Dds,
    Png,
}

#[derive(Debug)]
pub enum ReadError {
    NotFound,
    /// The entry decompresses to more than the caller's limit.
    TooLarge {
        limit: u64,
    },
    Io(io::Error),
    Zip(zip::result::ZipError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::NotFound => write!(f, "entry not found"),
            ReadError::TooLarge { limit } => write!(f, "entry is larger than {limit} bytes"),
            ReadError::Io(e) => write!(f, "read failed: {e}"),
            ReadError::Zip(e) => write!(f, "archive error: {e}"),
        }
    }
}

impl std::error::Error for ReadError {}

impl From<io::Error> for ReadError {
    fn from(e: io::Error) -> Self {
        ReadError::Io(e)
    }
}

impl From<zip::result::ZipError> for ReadError {
    fn from(e: zip::result::ZipError) -> Self {
        ReadError::Zip(e)
    }
}

enum Backing {
    Zip {
        archive: ZipArchive<BufReader<File>>,
        /// Archive index of each entry in `names`.
        zip_index: Vec<usize>,
    },
    Folder {
        root: PathBuf,
    },
}

pub struct ModContainer {
    backing: Backing,
    /// Normalised entry names (files only, no directories).
    names: Vec<String>,
    /// Lower-cased name → position in `names`.
    index: HashMap<String, usize>,
}

impl ModContainer {
    /// Opens a mod read-only. A directory is a folder mod; anything else is read as a zip.
    pub fn open(path: &Path) -> io::Result<Self> {
        if fs::metadata(path)?.is_dir() {
            Self::open_folder(path)
        } else {
            Self::open_zip(path)
        }
    }

    fn open_zip(path: &Path) -> io::Result<Self> {
        let archive = ZipArchive::new(BufReader::new(File::open(path)?))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut names = Vec::with_capacity(archive.len());
        let mut zip_index = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let entry = archive
                .by_index_data(i)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if entry.is_dir() {
                continue;
            }
            let Ok(name) = entry.name() else { continue };
            let name = normalise(&name);
            if name.is_empty() {
                continue;
            }
            names.push(name);
            zip_index.push(i);
        }
        Ok(Self::build(Backing::Zip { archive, zip_index }, names))
    }

    fn open_folder(root: &Path) -> io::Result<Self> {
        let mut names = Vec::new();
        let mut pending = vec![PathBuf::new()];
        while let Some(rel_dir) = pending.pop() {
            for item in fs::read_dir(root.join(&rel_dir))? {
                let item = item?;
                let rel = rel_dir.join(item.file_name());
                let file_type = item.file_type()?;
                if file_type.is_dir() {
                    pending.push(rel);
                } else if file_type.is_file() {
                    names.push(normalise(&rel.to_string_lossy()));
                }
            }
        }
        names.sort();
        Ok(Self::build(
            Backing::Folder {
                root: root.to_path_buf(),
            },
            names,
        ))
    }

    fn build(backing: Backing, names: Vec<String>) -> Self {
        let mut index = HashMap::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            // First entry wins if two names differ only by case.
            index.entry(name.to_lowercase()).or_insert(i);
        }
        Self {
            backing,
            names,
            index,
        }
    }

    pub fn format(&self) -> ModFormat {
        match self.backing {
            Backing::Zip { .. } => ModFormat::Zip,
            Backing::Folder { .. } => ModFormat::Folder,
        }
    }

    /// All file entries, normalised, in container order (zip) or sorted (folder).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// A stable hash of what the mod contains, for telling copies of a mod apart without
    /// reading any file content (plan M6 decision 1). Internal: never shown, only compared.
    ///
    /// For a zip it is taken from the central directory, which is already read: the sorted list
    /// of (entry name, CRC-32, uncompressed size). For a folder it is the sorted list of
    /// (relative path, size), with CRC 0. The bytes hashed with MD5 are a kind byte (`z` or `f`),
    /// then per entry the name length (u32 LE), the name (UTF-8), the CRC (u32 LE) and the size
    /// (u64 LE). Names are the normalised ones of [`ModContainer::names`], so a zip and its
    /// unpacked folder differ by the kind byte (the folder has no CRCs to compare).
    pub fn fingerprint(&self) -> String {
        let (kind, mut entries): (u8, Vec<(&str, u32, u64)>) = match &self.backing {
            Backing::Zip { archive, zip_index } => (
                b'z',
                zip_index
                    .iter()
                    .zip(&self.names)
                    .filter_map(|(&i, name)| {
                        let entry = archive.by_index_data(i).ok()?;
                        Some((name.as_str(), entry.crc32(), entry.size()))
                    })
                    .collect(),
            ),
            Backing::Folder { root } => (
                b'f',
                self.names
                    .iter()
                    .map(|name| {
                        let size = fs::metadata(root.join(name)).map_or(0, |m| m.len());
                        (name.as_str(), 0, size)
                    })
                    .collect(),
            ),
        };
        entries.sort_unstable();
        let mut hasher = Md5::new();
        hasher.update([kind]);
        for (name, crc, size) in entries {
            hasher.update(u32::try_from(name.len()).unwrap_or(u32::MAX).to_le_bytes());
            hasher.update(name.as_bytes());
            hasher.update(crc.to_le_bytes());
            hasher.update(size.to_le_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// The stored name matching `rel`, ignoring case and `\` vs `/`.
    pub fn find(&self, rel: &str) -> Option<String> {
        self.position(rel).map(|i| self.names[i].clone())
    }

    /// Uncompressed size of an entry, as recorded by the container.
    pub fn size(&mut self, rel: &str) -> Result<u64, ReadError> {
        let i = self.position(rel).ok_or(ReadError::NotFound)?;
        match &mut self.backing {
            Backing::Zip { archive, zip_index } => Ok(archive.by_index_data(zip_index[i])?.size()),
            Backing::Folder { root } => Ok(fs::metadata(root.join(&self.names[i]))?.len()),
        }
    }

    /// Reads a whole entry, failing with [`ReadError::TooLarge`] beyond `limit` bytes.
    /// The limit is enforced on the bytes actually produced, not on the recorded size.
    pub fn read(&mut self, rel: &str, limit: u64) -> Result<Vec<u8>, ReadError> {
        let mut out = Vec::new();
        self.with_reader(rel, |r| {
            r.take(limit.saturating_add(1)).read_to_end(&mut out)?;
            Ok(())
        })?;
        if out.len() as u64 > limit {
            return Err(ReadError::TooLarge { limit });
        }
        Ok(out)
    }

    /// Reads at most the first `n` bytes of an entry.
    pub fn read_head(&mut self, rel: &str, n: usize) -> Result<Vec<u8>, ReadError> {
        let mut out = Vec::with_capacity(n);
        self.with_reader(rel, |r| {
            r.take(n as u64).read_to_end(&mut out)?;
            Ok(())
        })?;
        Ok(out)
    }

    fn position(&self, rel: &str) -> Option<usize> {
        self.index.get(&normalise(rel).to_lowercase()).copied()
    }

    fn with_reader(
        &mut self,
        rel: &str,
        f: impl FnOnce(&mut dyn Read) -> io::Result<()>,
    ) -> Result<(), ReadError> {
        let i = self.position(rel).ok_or(ReadError::NotFound)?;
        match &mut self.backing {
            Backing::Zip { archive, zip_index } => {
                let mut file = archive.by_index(zip_index[i])?;
                f(&mut file)?;
            }
            Backing::Folder { root } => {
                let mut file = File::open(root.join(&self.names[i]))?;
                f(&mut file)?;
            }
        }
        Ok(())
    }
}

fn normalise(name: &str) -> String {
    let mut s = name.replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./").or_else(|| s.strip_prefix('/')) {
        s = rest.to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{write_folder, write_zip, Method};

    #[test]
    fn zip_lookup_ignores_case_and_separators() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FS25_Test.zip");
        write_zip(
            &path,
            &[
                ("modDesc.xml", b"<modDesc/>"),
                ("Icons/Icon_A.dds", b"DDS "),
            ],
            Method::Deflated,
        );
        let mut c = ModContainer::open(&path).unwrap();
        assert_eq!(c.format(), ModFormat::Zip);
        assert_eq!(c.find("moddesc.XML").as_deref(), Some("modDesc.xml"));
        assert_eq!(
            c.find("icons\\icon_a.dds").as_deref(),
            Some("Icons/Icon_A.dds")
        );
        assert_eq!(
            c.find("./Icons/Icon_A.dds").as_deref(),
            Some("Icons/Icon_A.dds")
        );
        assert_eq!(c.read("MODDESC.xml", 100).unwrap(), b"<modDesc/>");
        assert_eq!(c.size("modDesc.xml").unwrap(), 10);
        assert!(matches!(
            c.read("missing.xml", 100),
            Err(ReadError::NotFound)
        ));
    }

    #[test]
    fn zip_lists_files_but_not_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FS25_Test.zip");
        write_zip(
            &path,
            &[("a/", b""), ("a/b.xml", b"x"), ("modDesc.xml", b"y")],
            Method::Stored,
        );
        let c = ModContainer::open(&path).unwrap();
        assert_eq!(c.names().collect::<Vec<_>>(), ["a/b.xml", "modDesc.xml"]);
    }

    #[test]
    fn read_enforces_the_limit_on_decompressed_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FS25_Big.zip");
        let big = vec![b'a'; 64 * 1024];
        write_zip(&path, &[("big.xml", &big)], Method::Deflated);
        let mut c = ModContainer::open(&path).unwrap();
        assert!(matches!(
            c.read("big.xml", 1024),
            Err(ReadError::TooLarge { limit: 1024 })
        ));
        assert_eq!(c.read("big.xml", 64 * 1024).unwrap().len(), 64 * 1024);
        assert_eq!(c.read_head("big.xml", 16).unwrap(), vec![b'a'; 16]);
    }

    #[test]
    fn folder_mods_are_walked_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("FS25_Folder");
        write_folder(
            &root,
            &[("modDesc.xml", b"<modDesc/>"), ("icons/icon.dds", b"DDS ")],
        );
        let mut c = ModContainer::open(&root).unwrap();
        assert_eq!(c.format(), ModFormat::Folder);
        assert_eq!(
            c.names().collect::<Vec<_>>(),
            ["icons/icon.dds", "modDesc.xml"]
        );
        assert_eq!(c.read("ICONS/icon.dds", 10).unwrap(), b"DDS ");
        assert!(matches!(
            c.read("../outside.txt", 10),
            Err(ReadError::NotFound)
        ));
    }

    fn zip_fingerprint(
        dir: &Path,
        name: &str,
        entries: &[(&str, &[u8])],
        method: Method,
    ) -> String {
        let path = dir.join(name);
        write_zip(&path, entries, method);
        ModContainer::open(&path).unwrap().fingerprint()
    }

    #[test]
    fn the_fingerprint_ignores_file_name_entry_order_and_compression() {
        let dir = tempfile::tempdir().unwrap();
        let entries: [(&str, &[u8]); 3] = [
            ("modDesc.xml", b"<modDesc/>"),
            ("a/b.lua", b"print(1)"),
            ("c.txt", b"hello"),
        ];
        let base = zip_fingerprint(dir.path(), "one.zip", &entries, Method::Deflated);
        assert_eq!(base.len(), 32);
        assert_eq!(
            base,
            zip_fingerprint(dir.path(), "other name.zip", &entries, Method::Deflated)
        );
        assert_eq!(
            base,
            zip_fingerprint(dir.path(), "stored.zip", &entries, Method::Stored)
        );
        let reversed = [entries[2], entries[1], entries[0]];
        assert_eq!(
            base,
            zip_fingerprint(dir.path(), "reversed.zip", &reversed, Method::Deflated)
        );
        // Directory entries are not content.
        let with_dir: [(&str, &[u8]); 4] = [("a/", b""), entries[0], entries[1], entries[2]];
        assert_eq!(
            base,
            zip_fingerprint(dir.path(), "dirs.zip", &with_dir, Method::Deflated)
        );
    }

    #[test]
    fn the_fingerprint_changes_with_content_name_or_size() {
        let dir = tempfile::tempdir().unwrap();
        let base_entries: [(&str, &[u8]); 2] = [("modDesc.xml", b"<modDesc/>"), ("x.txt", b"aaaa")];
        let base = zip_fingerprint(dir.path(), "base.zip", &base_entries, Method::Deflated);
        let variants: [[(&str, &[u8]); 2]; 4] = [
            // Same size, one byte different: only the CRC differs.
            [("modDesc.xml", b"<modDesc/>"), ("x.txt", b"aaab")],
            [("modDesc.xml", b"<modDesc/>"), ("x.txt", b"aaaaa")],
            [("modDesc.xml", b"<modDesc/>"), ("y.txt", b"aaaa")],
            [("modDesc.xml", b"<modDesc/>"), ("X.txt", b"aaaa")],
        ];
        for (i, entries) in variants.iter().enumerate() {
            let other =
                zip_fingerprint(dir.path(), &format!("v{i}.zip"), entries, Method::Deflated);
            assert_ne!(base, other, "variant {i}");
        }
    }

    #[test]
    fn a_folder_fingerprint_follows_names_and_sizes_and_differs_from_a_zips() {
        let dir = tempfile::tempdir().unwrap();
        let make = |name: &str, entries: &[(&str, &[u8])]| {
            let root = dir.path().join(name);
            write_folder(&root, entries);
            ModContainer::open(&root).unwrap().fingerprint()
        };
        let entries: [(&str, &[u8]); 2] = [("modDesc.xml", b"<modDesc/>"), ("sub/x.txt", b"abc")];
        let a = make("a", &entries);
        assert_eq!(a, make("b", &entries));
        assert_eq!(
            a,
            ModContainer::open(&dir.path().join("a"))
                .unwrap()
                .fingerprint()
        );
        assert_ne!(
            a,
            make(
                "c",
                &[("modDesc.xml", b"<modDesc/>"), ("sub/x.txt", b"abcd")]
            )
        );
        assert_ne!(
            a,
            make(
                "d",
                &[("modDesc.xml", b"<modDesc/>"), ("sub/y.txt", b"abc")]
            )
        );
        assert_ne!(
            a,
            zip_fingerprint(dir.path(), "z.zip", &entries, Method::Stored)
        );
    }

    #[test]
    fn a_file_that_is_not_a_zip_fails_to_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FS25_Broken.zip");
        fs::write(&path, b"not a zip").unwrap();
        assert!(ModContainer::open(&path).is_err());
    }
}
