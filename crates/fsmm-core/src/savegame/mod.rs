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

use std::fs;
use std::io;
use std::path::Path;

use crate::model::Savegame;

#[cfg(test)]
mod tests;

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
pub fn list(user_dir: &Path) -> Vec<Savegame> {
    let Ok(entries) = fs::read_dir(user_dir) else {
        return Vec::new();
    };
    let mut saves: Vec<Savegame> = entries
        .flatten()
        .filter_map(|item| {
            let name = item.file_name().to_string_lossy().into_owned();
            slot_of(&name)?;
            read(user_dir, &name).ok().map(|file| file.info)
        })
        .collect();
    saves.sort_by_key(|s| s.slot);
    saves
}

/// Reads one save. `id` must be a slot folder name (`savegame<number>`); anything else is
/// `Missing`, so an id can never name another path.
pub fn read(user_dir: &Path, id: &str) -> Result<SavegameFile, SavegameError> {
    let slot = slot_of(id).ok_or(SavegameError::Missing)?;
    let path = user_dir.join(id).join("careerSavegame.xml");
    let unreadable = |detail: &str| SavegameError::Unreadable(detail.to_string());
    let meta = fs::metadata(&path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => SavegameError::Missing,
        kind => SavegameError::Unreadable(format!("{kind:?}")),
    })?;
    if !meta.is_file() {
        return Err(SavegameError::Missing);
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(unreadable("tooLarge"));
    }
    let bytes =
        fs::read(&path).map_err(|e| SavegameError::Unreadable(format!("{:?}", e.kind())))?;
    let text = String::from_utf8(bytes).map_err(|_| unreadable("invalidUtf8"))?;
    let doc = roxmltree::Document::parse(&text).map_err(|_| unreadable("invalidXml"))?;
    parse(&doc, id, slot)
}

/// A save file is a few hundred KB even with hundreds of mods; refuse absurd sizes.
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// The slot number of `savegame<digits>`; nothing else names a slot.
fn slot_of(id: &str) -> Option<u32> {
    let digits = id.strip_prefix("savegame")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn parse(
    doc: &roxmltree::Document<'_>,
    id: &str,
    slot: u32,
) -> Result<SavegameFile, SavegameError> {
    let root = doc.root_element();
    let is_save = root.has_tag_name("careerSavegame")
        && root.attribute("revision").map(str::trim) == Some("2")
        && root.attribute("valid").is_some_and(is_true);
    if !is_save {
        return Err(SavegameError::Missing);
    }

    let mods: Vec<SavedMod> = root
        .children()
        .filter(|n| n.has_tag_name("mod"))
        .filter_map(|n| {
            let mod_name = n.attribute("modName")?.trim();
            if mod_name.is_empty() {
                return None;
            }
            Some(SavedMod {
                mod_name: mod_name.to_string(),
                title: n.attribute("title").unwrap_or_default().to_string(),
                version: n.attribute("version").unwrap_or_default().to_string(),
                required: n.attribute("required").is_none_or(is_true),
                file_hash: n
                    .attribute("fileHash")
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase(),
            })
        })
        .collect();

    let setting = |name: &str| child_text(root, "settings", name);
    let statistic = |name: &str| child_text(root, "statistics", name);
    let map_id = setting("mapId").unwrap_or_default();
    // A mod map is `<modName>.<id>`; a base-game map has no dot.
    let map_mod = map_id
        .split_once('.')
        .map(|(name, _)| name)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let info = Savegame {
        id: id.to_string(),
        slot,
        name: setting("savegameName").unwrap_or_default(),
        map_title: setting("mapTitle").unwrap_or_default(),
        map_mod,
        save_date: setting("saveDate").filter(|d| !d.is_empty()),
        play_minutes: statistic("playTime").and_then(|t| minutes(&t)),
        money: statistic("money").and_then(|m| whole_number(&m)),
        mod_count: u32::try_from(mods.len()).unwrap_or(u32::MAX),
    };
    Ok(SavegameFile { info, mods })
}

fn is_true(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1")
}

/// The trimmed text of `<root><group><name>text</name></group></root>`.
fn child_text(root: roxmltree::Node<'_, '_>, group: &str, name: &str) -> Option<String> {
    let group = root.children().find(|n| n.has_tag_name(group))?;
    let node = group.children().find(|n| n.has_tag_name(name))?;
    Some(node.text().unwrap_or_default().trim().to_string())
}

/// `statistics/money` is an integer as text and can be negative; a float is rounded.
fn whole_number(text: &str) -> Option<i64> {
    if let Ok(n) = text.parse::<i64>() {
        return Some(n);
    }
    let f = text.parse::<f64>().ok().filter(|f| f.is_finite())?;
    // The cast saturates, which is the right answer for an absurdly large value.
    #[allow(clippy::cast_possible_truncation)]
    Some(f.round() as i64)
}

/// `statistics/playTime` is a float in minutes, rounded to whole minutes.
fn minutes(text: &str) -> Option<u32> {
    let f = text.parse::<f64>().ok().filter(|f| f.is_finite())?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(f.round().max(0.0) as u32)
}
