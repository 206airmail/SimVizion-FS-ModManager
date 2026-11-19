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

/// Where a mod comes from, for the Source column: the update catalog's match first (plan M7),
/// else the user's manual link. `ModEntry::links` lists every page the mod can be opened on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ModSource {
    Unknown,
    ModHub {
        id: String,
    },
    GitHub {
        owner: String,
        repo: String,
    },
    /// A page the user linked by hand (KingMods, itch.io, a forum post): its host.
    Manual {
        host: String,
    },
}

/// Which page a mod link opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkKind {
    ModHub,
    GitHub,
    /// The user's own link.
    Manual,
}

/// A page a mod can be opened on. The URL stays in the core: the frontend asks to open a link
/// by mod name and kind (`openModLink`), so no URL ever crosses the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceLink {
    pub kind: LinkKind,
    /// What the button names: "ModHub", "GitHub" or the manual link's host.
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateState {
    Unknown,
    UpToDate,
    /// The catalog has a newer version. `installable` when the app can download it itself
    /// (GitHub release or ModHub download link); otherwise only its page can be opened.
    Available {
        version: String,
        installable: bool,
    },
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
    /// The user's tags for this mod name (shared by every profile), sorted.
    pub tags: Vec<String>,
    /// The user's note for this mod name (shared by every profile).
    pub note: Option<String>,
    /// Pages the mod can be opened on: the catalog's ModHub and GitHub pages, and the user's
    /// manual link.
    pub links: Vec<SourceLink>,
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
    /// A write to game files was refused because the game is running.
    GameRunning,
    /// A write to game files was refused because the dedicated server is running.
    ServerRunning,
    /// `gameSettings.xml` does not exist (the game has not been started yet).
    SettingsMissing,
    /// `gameSettings.xml` exists but is not readable XML in UTF-8.
    SettingsUnreadable,
    /// `gameSettings.xml` changed while the app was writing it, or no longer holds what the
    /// operation being undone wrote. Nothing was written.
    SettingsChanged,
    /// Writing `gameSettings.xml` failed; the original is unchanged.
    WriteFailed,
    /// This build refuses to write game files (debug builds without `FSMM_ALLOW_GAME_WRITES=1`).
    WritesDisabled,
    /// The operation to undo is unknown or is not the most recent one.
    NothingToUndo,
    /// No backup of `gameSettings.xml` exists to restore.
    NoBackup,
    /// Windows would delete the item permanently instead of recycling it (too large for the
    /// Recycle Bin, or a drive without one), so it was left in place.
    RecycleUnavailable,
    /// The target drive does not have room for the copy; nothing was written.
    NotEnoughSpace,
    /// Another file operation is still running.
    Busy,
    /// The user cancelled; items not finished were left as they were.
    Cancelled,
    /// A mod or the target folder changed between planning and doing the operation.
    TargetChanged,
    /// The active folder and the game's default folder cannot be removed from the list.
    CannotRemoveProfile,
    /// The savegame slot has no save (or the game's user folder was not found).
    SavegameMissing,
    /// The savegame's `careerSavegame.xml` exists but could not be read.
    SavegameUnreadable,
    /// The update catalog or download site could not be reached.
    Offline,
    /// The update catalog asked the app to wait; `detail` holds when it may ask again (ISO time).
    RateLimited,
    /// Online update checks are switched off in Settings.
    UpdatesDisabled,
    /// A downloaded update did not match what the catalog or page promised (hash, version, or
    /// not a mod zip); nothing was replaced.
    VerifyFailed,
    /// This update cannot be installed by the app (no direct download, or a file with another
    /// name); its page can still be opened.
    NotInstallable,
    /// A manual source link is not an http(s) address the app accepts.
    InvalidLink,
    /// Anything unexpected; `detail` holds a technical message for a bug report.
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransferMode {
    Copy,
    Move,
}

/// The facts shown side by side when two mods with the same name meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModSummary {
    pub file_name: String,
    pub version: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// ISO 8601 timestamp.
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferItem {
    pub mod_id: String,
    pub mod_name: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

/// A mod being transferred whose name already exists in the target folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferClash {
    pub mod_name: String,
    pub incoming: ModSummary,
    pub existing: ModSummary,
}

/// What a copy or move would do, worked out before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferPlan {
    pub plan_id: String,
    pub from_profile_id: String,
    pub to_profile_id: String,
    pub mode: TransferMode,
    pub items: Vec<TransferItem>,
    pub clashes: Vec<TransferClash>,
    /// Bytes that would be written to the target drive (0 for a same-drive move).
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// Free space on the target drive.
    #[ts(type = "number")]
    pub free_bytes: u64,
}

/// The user's answer for one clash: replace the existing mod (it goes to the Recycle Bin) or
/// skip the incoming one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClashChoice {
    pub mod_name: String,
    pub replace: bool,
}

/// Progress of a running file operation, streamed to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpProgress {
    #[ts(type = "number")]
    pub done_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// File name of the mod being worked on.
    pub current: Option<String>,
    pub done_items: u32,
    pub total_items: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemFailure {
    pub mod_name: String,
    pub code: ErrorCode,
}

/// The outcome of a copy, move or remove, item by item (by mod name).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpResult {
    pub done: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<ItemFailure>,
}

/// The user's online-update settings (plan M7). Off by default: nothing goes online until enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateSettings {
    pub enabled: bool,
    /// ISO time of the last successful check.
    pub last_check: Option<String>,
    /// ISO time before which the catalog asked not to be contacted again.
    pub retry_after: Option<String>,
}

/// The outcome of an update check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateCheck {
    /// ISO time the results were fetched.
    pub checked_at: String,
    /// Distinct mod names looked up.
    pub checked: u32,
    pub matched_by_name: u32,
    pub matched_by_hash: u32,
    /// Distinct mod names with a newer version.
    pub updates: u32,
    /// The results came from the cache; nothing was sent.
    pub from_cache: bool,
}

/// One folder that holds the mod an update is planned for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateFolder {
    pub profile_id: String,
    pub mod_id: String,
    pub version: String,
}

/// What installing an update would do, before anything is downloaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdatePlan {
    pub mod_name: String,
    pub to_version: String,
    /// Where the file will come from: `gitHub` or `modHub`.
    pub source: LinkKind,
    /// Every folder holding a mod with this name, in profile order.
    pub folders: Vec<UpdateFolder>,
}

/// How the mods with one name compare between two profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CompareState {
    OnlyLeft,
    OnlyRight,
    /// Both have it, with different `modDesc` versions.
    VersionDiffers,
    /// Same version, but the contents differ (zip central directory or folder file list).
    ContentDiffers,
    Identical,
}

/// One profile's copy of a mod in a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompareSide {
    pub mod_id: String,
    pub summary: ModSummary,
}

/// One mod name in a comparison of two profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompareRow {
    pub mod_name: String,
    /// The left copy's title, else the right's.
    pub title: String,
    pub left: Option<CompareSide>,
    pub right: Option<CompareSide>,
    pub state: CompareState,
}

/// Every mod name of two profiles, sorted by mod name (case-insensitive).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Comparison {
    pub left_profile_id: String,
    pub right_profile_id: String,
    pub rows: Vec<CompareRow>,
}

/// A savegame slot that holds a save. Read from `savegameN/careerSavegame.xml`, never written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Savegame {
    /// The slot folder name, e.g. `savegame1`.
    pub id: String,
    pub slot: u32,
    pub name: String,
    pub map_title: String,
    /// The map's mod name when the map is a mod (`mapId` is `<modName>.<id>`).
    pub map_mod: Option<String>,
    /// `YYYY-MM-DD` as the game writes it.
    pub save_date: Option<String>,
    /// The game's `playTime` statistic, in minutes.
    pub play_minutes: Option<u32>,
    #[ts(type = "number | null")]
    pub money: Option<i64>,
    pub mod_count: u32,
}

/// How one of a savegame's mods stands in a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SaveModState {
    /// Same version, and the file hash matches the save's (or there is none to compare).
    Present,
    VersionDiffers,
    /// Same version, but the zip's hash differs from the save's `fileHash`.
    FileDiffers,
    /// Same version; the file hash has not been computed yet.
    HashPending,
    Missing,
    DlcInstalled,
    DlcMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaveModRow {
    pub mod_name: String,
    /// The title the save recorded.
    pub title: String,
    /// The version the save recorded.
    pub saved_version: String,
    /// Only the map mod is required; the game will not load the save without it.
    pub required: bool,
    pub state: SaveModState,
    /// The profile's copy, if it has one.
    pub installed: Option<CompareSide>,
    /// Other profiles that have a mod with this name.
    pub found_in: Vec<String>,
}

/// A savegame's mods checked against one profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SavegameReport {
    pub savegame_id: String,
    pub profile_id: String,
    /// Required first, then by mod name (case-insensitive).
    pub rows: Vec<SaveModRow>,
    /// Ids of the profile's mods the save does not use.
    pub extras: Vec<String>,
}

/// Progress of hashing mod files, streamed while `checkSavegameFiles` runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HashProgress {
    pub done_files: u32,
    pub total_files: u32,
    #[ts(type = "number")]
    pub done_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

/// The outcome of switching the active mod folder (or undoing a switch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SwitchResult {
    /// Journal id of this write; pass it to `undoSwitch`.
    pub op_id: String,
    /// The profile that was active before, if it is registered.
    pub from_profile_id: Option<String>,
    /// The profile that is active now, if it is registered.
    pub to_profile_id: Option<String>,
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
            installable: true,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"type":"available","version":"1.2.0.0","installable":true}"#
        );
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
