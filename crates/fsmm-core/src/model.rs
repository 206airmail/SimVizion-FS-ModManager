//! The data contract shared by the core library, the Tauri layer and the frontend.
//!
//! The TypeScript types in `src/api/types/` are generated from this file by
//! `npm run gen:types`. Change the contract here, never in the generated files.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A mod folder the game can be pointed at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub path: String,
    /// The folder the game currently loads mods from.
    pub is_active: bool,
    /// The game's built-in `mods` folder, used when no override is set.
    pub is_game_default: bool,
    pub mod_count: u32,
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub issue_count: u32,
    pub update_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ModKind {
    Map,
    Vehicle,
    Placeable,
    Script,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ModFormat {
    Zip,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Dependency {
    pub mod_name: String,
    /// Whether a mod with this name exists in the same profile.
    pub satisfied: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IssueKind {
    /// No `modDesc.xml` at the root, or the archive cannot be read.
    NotAMod,
    /// The game only accepts `A-Z a-z 0-9 _` and no leading digit.
    InvalidFileName,
    /// An archive that contains other mod archives.
    NestedArchive,
    /// Both `Name.zip` and a `Name` folder exist; the game loads the folder.
    ZipAndFolderTwin,
    /// A sync-conflict or `(2)` style copy of another mod.
    ConflictCopy,
    /// `descVersion` differs from what the game last reported.
    DescVersionMismatch,
    MissingDependency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModIssue {
    pub kind: IssueKind,
    pub severity: Severity,
    pub detail: Option<String>,
}

/// Where a mod came from. Stored as user metadata keyed by mod name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ModSource {
    Unknown,
    ModHub { id: String },
    GitHub { owner: String, repo: String },
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateState {
    Unknown,
    UpToDate,
    Available { version: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModEntry {
    /// Unique within a profile: the file or folder name.
    pub id: String,
    pub profile_id: String,
    /// File name without extension. This is the mod's identity to the game.
    pub mod_name: String,
    pub file_name: String,
    pub format: ModFormat,
    pub title: String,
    pub author: String,
    pub version: String,
    pub desc_version: Option<u32>,
    pub kind: ModKind,
    pub description: String,
    /// URL of a decoded thumbnail, if the mod has a readable icon.
    pub icon: Option<String>,
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// ISO 8601 timestamp of the file's last modification.
    pub modified: String,
    pub multiplayer: bool,
    pub dependencies: Vec<Dependency>,
    pub issues: Vec<ModIssue>,
    pub source: ModSource,
    pub update: UpdateState,
    /// Ids of other profiles that contain a mod with the same name.
    pub also_in: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameStatus {
    pub game_version: Option<String>,
    pub game_running: bool,
    pub server_running: bool,
    /// Whether `gameSettings.xml` was found and parsed.
    pub settings_readable: bool,
    pub override_active: bool,
    pub effective_mods_dir: Option<String>,
    /// The `ModDesc Version` the game printed in its last log.
    pub logged_desc_version: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanSummary {
    pub profile_id: String,
    pub mod_count: u32,
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub issue_count: u32,
    pub duration_ms: u32,
    /// ISO 8601 timestamp.
    pub scanned_at: String,
}

/// Progress of reading one profile's mods, streamed while `listMods` runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanProgress {
    pub profile_id: String,
    pub done: u32,
    pub total: u32,
}

/// Why a command failed. The frontend words each code; `detail` carries data, not sentences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorCode {
    /// The profile id is not registered (it may have been removed in another window).
    ProfileNotFound,
    /// The profile's folder, or the mod, no longer exists on disk.
    FolderMissing,
    /// The folder exists but could not be listed or read.
    FolderUnreadable,
    /// Anything unexpected; `detail` holds a technical message for a bug report.
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiError {
    pub code: ErrorCode,
    pub detail: Option<String>,
}

impl ApiError {
    pub fn new(code: ErrorCode, detail: impl Into<Option<String>>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.detail {
            Some(detail) => write!(f, "{:?}: {detail}", self.code),
            None => write!(f, "{:?}", self.code),
        }
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_source_uses_a_type_tag() {
        let json = serde_json::to_string(&ModSource::GitHub {
            owner: "owner".into(),
            repo: "repo".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"gitHub","owner":"owner","repo":"repo"}"#);
        assert_eq!(
            serde_json::to_string(&ModSource::Unknown).unwrap(),
            r#"{"type":"unknown"}"#
        );
    }

    #[test]
    fn update_state_uses_a_type_tag() {
        let json = serde_json::to_string(&UpdateState::Available {
            version: "1.2.0.0".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"available","version":"1.2.0.0"}"#);
    }

    #[test]
    fn api_error_serialises_its_code_in_camel_case() {
        let error = ApiError::new(ErrorCode::FolderMissing, Some("p1".to_string()));
        assert_eq!(
            serde_json::to_string(&error).unwrap(),
            r#"{"code":"folderMissing","detail":"p1"}"#
        );
    }

    #[test]
    fn fields_are_camel_case() {
        let issue = ModIssue {
            kind: IssueKind::ZipAndFolderTwin,
            severity: Severity::Warning,
            detail: None,
        };
        assert_eq!(
            serde_json::to_string(&issue).unwrap(),
            r#"{"kind":"zipAndFolderTwin","severity":"warning","detail":null}"#
        );
    }
}
