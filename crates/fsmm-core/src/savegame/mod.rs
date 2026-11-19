//! Reading savegames (plan M6). Only `savegameN/careerSavegame.xml` is read, and never written;
//! no other file of a save is opened (some hold multiplayer passwords).
//!
//! The format, from the game's `FSCareerMissionInfo.lua` (1.25.1.0): the root is
//! `<careerSavegame revision="2" valid="true">`; a file with another revision or `valid="false"`
//! is not a save. `settings/savegameName`, `settings/mapId` (`<modName>.<id>` for a mod map),
//! `settings/mapTitle`, `settings/saveDate` (`YYYY-MM-DD`), `statistics/money` (an integer as
//! text) and `statistics/playTime` (minutes, a float). Each mod is a `<mod modName title version
//! required fileHash/>` directly under the root; `required` defaults to true when absent, and
//! `fileHash` is empty for folder mods.

use std::path::Path;

use crate::model::Savegame;

/// One `<mod>` entry of a save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedMod {
    pub mod_name: String,
    pub title: String,
    pub version: String,
    pub required: bool,
    /// Lower-case MD5 hex, or empty.
    pub file_hash: String,
}

/// A save's summary and its mods, in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct SavegameFile {
    pub info: Savegame,
    pub mods: Vec<SavedMod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavegameError {
    /// Not a slot id (`savegame<number>`), no such folder, no `careerSavegame.xml`, or a file
    /// the game itself would not treat as a save (`valid="false"`, another revision).
    Missing,
    /// The file exists but is not readable XML. The detail is an error kind, never content.
    Unreadable(String),
}

/// Every slot under `user_dir` that holds a save, by slot number. Slots whose file is missing
/// or not a save are left out; an unreadable one too (it is reported when opened).
pub fn list(_user_dir: &Path) -> Vec<Savegame> {
    todo!("WS-A")
}

/// Reads one save. `id` must be a slot folder name (`savegame<number>`); anything else is
/// `Missing`, so an id can never name another path.
pub fn read(_user_dir: &Path, _id: &str) -> Result<SavegameFile, SavegameError> {
    todo!("WS-A")
}
