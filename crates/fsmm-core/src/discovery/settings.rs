//! `gameSettings.xml`: reads only the `modsDirectoryOverride` element.
//!
//! The file holds plain-text multiplayer passwords, so nothing from it is ever kept in an
//! error, a log line or a return value other than the one element's two attributes.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use super::{ModsOverride, SettingsError};

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Io(kind) => write!(f, "gameSettings.xml could not be read ({kind})"),
            SettingsError::Malformed { line, column } => write!(
                f,
                "gameSettings.xml is not well-formed at line {line}, column {column}"
            ),
        }
    }
}

impl std::error::Error for SettingsError {}

pub(super) fn read(game_settings: &Path) -> Result<ModsOverride, SettingsError> {
    let bytes = fs::read(game_settings).map_err(|e| SettingsError::Io(e.kind()))?;
    let bytes = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF][..])
        .unwrap_or(&bytes);
    parse(&String::from_utf8_lossy(bytes))
}

fn parse(text: &str) -> Result<ModsOverride, SettingsError> {
    // roxmltree's own error text can quote element names, so only its position is kept.
    let doc = roxmltree::Document::parse(text).map_err(|e| {
        let pos = e.pos();
        SettingsError::Malformed {
            line: pos.row,
            column: pos.col,
        }
    })?;
    let Some(node) = doc
        .descendants()
        .find(|n| n.is_element() && n.has_tag_name("modsDirectoryOverride"))
    else {
        return Ok(ModsOverride::default());
    };
    let active = node
        .attribute("active")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1");
    let directory = node
        .attribute("directory")
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from);
    Ok(ModsOverride { active, directory })
}
