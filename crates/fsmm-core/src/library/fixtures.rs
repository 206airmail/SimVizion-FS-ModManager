//! Builders shared by the M6 library tests (compare, savegames, tags): generated mods in temp
//! folders and an in-memory `Library`. Never a real game folder.

use std::fs;
use std::path::{Path, PathBuf};

use super::Library;
use crate::discovery::GameContext;
use crate::model::{ModEntry, Profile};
use crate::store::Store;
use crate::testutil::{write_folder, write_zip, Method};

pub fn library() -> Library {
    Library::new(Store::open_in_memory().unwrap(), "http://thumb.localhost")
}

pub fn ctx() -> GameContext {
    GameContext {
        languages: vec!["en".to_string()],
        ..GameContext::default()
    }
}

pub fn desc(version: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <modDesc descVersion=\"105\"><title><en>Title</en></title>\
         <author>Someone</author><version>{version}</version></modDesc>"
    )
}

/// A zip mod with a `modDesc.xml` of this version and an `extra.txt` of this content.
pub fn write_mod(folder: &Path, file: &str, version: &str, extra: &[u8]) -> PathBuf {
    let path = folder.join(file);
    write_zip(
        &path,
        &[
            ("modDesc.xml", desc(version).as_bytes()),
            ("extra.txt", extra),
        ],
        Method::Deflated,
    );
    path
}

/// An unpacked folder mod with the same layout as [`write_mod`].
pub fn write_folder_mod(folder: &Path, name: &str, version: &str, extra: &[u8]) -> PathBuf {
    let path = folder.join(name);
    write_folder(
        &path,
        &[
            ("modDesc.xml", desc(version).as_bytes()),
            ("extra.txt", extra),
        ],
    );
    path
}

pub fn folder(parent: &Path, name: &str) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn add(lib: &Library, folder: &Path) -> Profile {
    lib.add_profile(folder, &ctx()).unwrap()
}

pub fn list(lib: &Library, profile_id: &str) -> Vec<ModEntry> {
    lib.list_mods(profile_id, &ctx(), &|_| {}).unwrap()
}
