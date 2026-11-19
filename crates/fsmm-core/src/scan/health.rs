//! Health rules that depend on mod names or on the whole folder.

use std::collections::{HashMap, HashSet};

use super::ModRecord;
use crate::model::{IssueKind, ModFormat, ModIssue, Severity};

/// The issue kinds `assess` owns: it removes and re-adds them, so it can run repeatedly.
/// `NotAMod` and `NestedArchive` come from `read_mod` and are left alone.
const OWNED: [IssueKind; 5] = [
    IssueKind::ConflictCopy,
    IssueKind::InvalidFileName,
    IssueKind::ZipAndFolderTwin,
    IssueKind::MissingDependency,
    IssueKind::DescVersionMismatch,
];

/// Applies the rules that need the whole folder (twins, conflict copies, dependencies) and
/// the descVersion check against `logged_desc_version` (plan decision 2).
pub fn assess(records: &mut [ModRecord], logged_desc_version: Option<u32>) {
    let present: HashSet<String> = records
        .iter()
        .map(|r| r.entry.mod_name.to_lowercase())
        .collect();
    let twins = find_twins(records);

    for (i, record) in records.iter_mut().enumerate() {
        let entry = &mut record.entry;
        entry.issues.retain(|issue| !OWNED.contains(&issue.kind));

        // A conflict copy has an unusable name by construction; it is not also reported as invalid.
        if let Some(original) = conflict_original(&entry.mod_name) {
            entry.issues.push(ModIssue {
                kind: IssueKind::ConflictCopy,
                severity: Severity::Warning,
                detail: Some(original),
            });
        } else if !is_valid_mod_name(&entry.mod_name) {
            entry.issues.push(ModIssue {
                kind: IssueKind::InvalidFileName,
                severity: Severity::Error,
                detail: None,
            });
        }

        if let Some(other) = twins.get(&i) {
            entry.issues.push(ModIssue {
                kind: IssueKind::ZipAndFolderTwin,
                severity: Severity::Warning,
                detail: Some(other.clone()),
            });
        }

        // Paid DLC (`pdlc_*`) is not in mod folders; DLC ownership is a later milestone.
        let mut missing = Vec::new();
        for dep in &mut entry.dependencies {
            dep.satisfied = dep.mod_name.to_lowercase().starts_with("pdlc_")
                || present.contains(&dep.mod_name.to_lowercase());
            if !dep.satisfied {
                missing.push(dep.mod_name.as_str());
            }
        }
        if !missing.is_empty() {
            let detail = missing.join(", ");
            entry.issues.push(ModIssue {
                kind: IssueKind::MissingDependency,
                severity: Severity::Warning,
                detail: Some(detail),
            });
        }

        if let (Some(logged), Some(declared)) = (logged_desc_version, entry.desc_version) {
            if declared > logged {
                entry.issues.push(ModIssue {
                    kind: IssueKind::DescVersionMismatch,
                    severity: Severity::Warning,
                    detail: Some(logged.to_string()),
                });
            }
        }
    }
}

/// For each record in a zip and folder pair with the same mod name, the other one's file name.
fn find_twins(records: &[ModRecord]) -> HashMap<usize, String> {
    let mut by_name: HashMap<String, (Option<usize>, Option<usize>)> = HashMap::new();
    for (i, record) in records.iter().enumerate() {
        let slot = by_name
            .entry(record.entry.mod_name.to_lowercase())
            .or_default();
        match record.entry.format {
            ModFormat::Zip => slot.0.get_or_insert(i),
            ModFormat::Folder => slot.1.get_or_insert(i),
        };
    }
    let mut twins = HashMap::new();
    for (zip, folder) in by_name.into_values() {
        if let (Some(z), Some(f)) = (zip, folder) {
            twins.insert(z, records[f].entry.file_name.clone());
            twins.insert(f, records[z].entry.file_name.clone());
        }
    }
    twins
}

/// The game only accepts `[A-Za-z_][A-Za-z0-9_]*`.
fn is_valid_mod_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The original mod's name when `name` is a copy of it: Syncthing's
/// `<orig>.sync-conflict-<date>-<time>-<id>`, Windows' `<orig> (N)`, or `<orig> - Copy` with an
/// optional ` (N)`.
fn conflict_original(name: &str) -> Option<String> {
    if let Some((original, rest)) = name.split_once(".sync-conflict-") {
        let mut parts = rest.split('-');
        let (date, time, id) = (parts.next()?, parts.next()?, parts.next()?);
        let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        let is_id = !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric());
        return (all_digits(date) && all_digits(time) && is_id && parts.next().is_none())
            .then_some(original)
            .filter(|o| !o.is_empty())
            .map(str::to_string);
    }

    let (base, numbered) = strip_counter(name);
    let original = match base.strip_suffix(" - Copy") {
        Some(original) => original,
        None if numbered => base,
        None => return None,
    };
    (!original.is_empty()).then(|| original.to_string())
}

/// Splits a trailing ` (N)` off `name`; the flag says whether there was one.
fn strip_counter(name: &str) -> (&str, bool) {
    if let Some(open) = name.strip_suffix(')').and_then(|s| s.rfind(" (")) {
        let digits = &name[open + 2..name.len() - 1];
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            return (&name[..open], true);
        }
    }
    (name, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, ModEntry, ModKind, ModSource, UpdateState};
    use std::path::PathBuf;

    fn rec(file_name: &str, format: ModFormat) -> ModRecord {
        let mod_name = file_name.strip_suffix(".zip").unwrap_or(file_name);
        ModRecord {
            path: PathBuf::from(file_name),
            icon: None,
            fingerprint: String::new(),
            entry: ModEntry {
                id: file_name.into(),
                profile_id: "p".into(),
                mod_name: mod_name.into(),
                file_name: file_name.into(),
                format,
                title: mod_name.into(),
                author: String::new(),
                version: String::new(),
                desc_version: None,
                kind: ModKind::Other,
                description: String::new(),
                icon: None,
                size_bytes: 0,
                modified: String::new(),
                multiplayer: false,
                dependencies: Vec::new(),
                issues: Vec::new(),
                source: ModSource::Unknown,
                update: UpdateState::Unknown,
                also_in: Vec::new(),
                tags: Vec::new(),
                note: None,
                links: Vec::new(),
            },
        }
    }

    fn zip(name: &str) -> ModRecord {
        rec(&format!("{name}.zip"), ModFormat::Zip)
    }

    fn with_deps(mut r: ModRecord, deps: &[&str]) -> ModRecord {
        r.entry.dependencies = deps
            .iter()
            .map(|d| Dependency {
                mod_name: (*d).into(),
                satisfied: false,
            })
            .collect();
        r
    }

    fn issues(r: &ModRecord) -> Vec<(IssueKind, Severity, Option<&str>)> {
        r.entry
            .issues
            .iter()
            .map(|i| (i.kind, i.severity, i.detail.as_deref()))
            .collect()
    }

    #[test]
    fn valid_and_invalid_names() {
        for ok in ["FS25_Mod", "_x", "a1_B2", "FS25"] {
            assert!(is_valid_mod_name(ok), "{ok}");
        }
        for bad in [
            "", "1Mod", "My Mod", "FS25-Mod", "FS25.Mod", "Mod!", "Mödel",
        ] {
            assert!(!is_valid_mod_name(bad), "{bad}");
        }
    }

    #[test]
    fn invalid_file_name_is_an_error_without_detail() {
        let mut records = vec![zip("FS25_Good"), zip("1BadStart"), zip("Bad Name")];
        assess(&mut records, None);
        assert!(records[0].entry.issues.is_empty());
        for r in &records[1..] {
            assert_eq!(
                issues(r),
                [(IssueKind::InvalidFileName, Severity::Error, None)]
            );
        }
    }

    #[test]
    fn conflict_copy_patterns_name_the_original() {
        let cases = [
            (
                "FS25_AgroForests.sync-conflict-20261117-155914-UPKGIQY",
                "FS25_AgroForests",
            ),
            ("FS25_Mod (2)", "FS25_Mod"),
            ("FS25_Mod (12)", "FS25_Mod"),
            ("FS25_Mod - Copy", "FS25_Mod"),
            ("FS25_Mod - Copy (3)", "FS25_Mod"),
        ];
        for (name, original) in cases {
            assert_eq!(conflict_original(name).as_deref(), Some(original), "{name}");
        }
        for name in [
            "FS25_Mod",
            "FS25_Mod ()",
            "FS25_Mod (a)",
            "FS25_Mod.sync-conflict-x-y-Z",
            "FS25_Mod.sync-conflict-1-2-",
            "FS25_Mod.sync-conflict-1-2-AB-CD",
            " (2)",
            " - Copy",
        ] {
            assert_eq!(conflict_original(name), None, "{name}");
        }
    }

    #[test]
    fn conflict_copies_are_warnings_and_not_also_invalid() {
        let mut records = vec![
            zip("FS25_AgroForests"),
            zip("FS25_AgroForests.sync-conflict-20261117-155914-UPKGIQY"),
            zip("FS25_Mod (2)"),
            zip("FS25_Mod - Copy (2)"),
        ];
        assess(&mut records, None);
        assert!(records[0].entry.issues.is_empty());
        assert_eq!(
            issues(&records[1]),
            [(
                IssueKind::ConflictCopy,
                Severity::Warning,
                Some("FS25_AgroForests")
            )]
        );
        assert_eq!(
            issues(&records[2]),
            [(IssueKind::ConflictCopy, Severity::Warning, Some("FS25_Mod"))]
        );
        assert_eq!(
            issues(&records[3]),
            [(IssueKind::ConflictCopy, Severity::Warning, Some("FS25_Mod"))]
        );
    }

    #[test]
    fn zip_and_folder_twins_flag_both_with_the_other_file_name() {
        let mut records = vec![
            rec("FS25_Twin", ModFormat::Folder),
            zip("fs25_twin"),
            zip("FS25_Alone"),
            rec("FS25_AloneFolder", ModFormat::Folder),
        ];
        assess(&mut records, None);
        assert_eq!(
            issues(&records[0]),
            [(
                IssueKind::ZipAndFolderTwin,
                Severity::Warning,
                Some("fs25_twin.zip")
            )]
        );
        assert_eq!(
            issues(&records[1]),
            [(
                IssueKind::ZipAndFolderTwin,
                Severity::Warning,
                Some("FS25_Twin")
            )]
        );
        assert!(records[2].entry.issues.is_empty());
        assert!(records[3].entry.issues.is_empty());
    }

    #[test]
    fn missing_dependencies_are_one_issue_listing_all() {
        let mut records = vec![
            with_deps(
                zip("FS25_A"),
                &["FS25_B", "FS25_Missing1", "fs25_c", "FS25_Missing2"],
            ),
            zip("FS25_B"),
            rec("FS25_C", ModFormat::Folder),
        ];
        assess(&mut records, None);
        assert_eq!(
            issues(&records[0]),
            [(
                IssueKind::MissingDependency,
                Severity::Warning,
                Some("FS25_Missing1, FS25_Missing2")
            )]
        );
        let satisfied: Vec<bool> = records[0]
            .entry
            .dependencies
            .iter()
            .map(|d| d.satisfied)
            .collect();
        assert_eq!(satisfied, [true, false, true, false]);
    }

    #[test]
    fn satisfied_dependencies_raise_no_issue() {
        let mut records = vec![with_deps(zip("FS25_A"), &["FS25_B"]), zip("FS25_B")];
        assess(&mut records, None);
        assert!(records[0].entry.issues.is_empty());
        assert!(records[0].entry.dependencies[0].satisfied);
    }

    #[test]
    fn paid_dlc_dependencies_are_satisfied_and_never_flagged() {
        let mut records = vec![with_deps(
            zip("FS25_A"),
            &["pdlc_highlandsFishingPack", "PDLC_Other"],
        )];
        assess(&mut records, None);
        assert!(records[0].entry.issues.is_empty());
        assert!(records[0].entry.dependencies.iter().all(|d| d.satisfied));
    }

    #[test]
    fn desc_version_is_flagged_only_above_a_known_logged_value() {
        let mut records: Vec<ModRecord> = [Some(115), Some(110), Some(100), None]
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                let mut r = zip(&format!("FS25_V{i}"));
                r.entry.desc_version = v;
                r
            })
            .collect();

        assess(&mut records, None);
        assert!(records.iter().all(|r| r.entry.issues.is_empty()));

        assess(&mut records, Some(110));
        assert_eq!(
            issues(&records[0]),
            [(
                IssueKind::DescVersionMismatch,
                Severity::Warning,
                Some("110")
            )]
        );
        assert!(records[1].entry.issues.is_empty(), "equal is fine");
        assert!(records[2].entry.issues.is_empty(), "lower is fine");
        assert!(records[3].entry.issues.is_empty(), "unknown is fine");
    }

    #[test]
    fn assess_is_idempotent_and_keeps_read_issues() {
        let mut records = vec![
            with_deps(zip("FS25_A"), &["FS25_Gone"]),
            zip("FS25_A (2)"),
            zip("9bad"),
            rec("FS25_A", ModFormat::Folder),
        ];
        records[0].entry.desc_version = Some(120);
        records[2].entry.issues.push(ModIssue {
            kind: IssueKind::NestedArchive,
            severity: Severity::Warning,
            detail: Some("inner.zip".into()),
        });

        assess(&mut records, Some(110));
        let first = records.clone();
        assess(&mut records, Some(110));
        assert_eq!(records, first);
        assert_eq!(records[0].entry.issues.len(), 3);
        assert!(records[2]
            .entry
            .issues
            .iter()
            .any(|i| i.kind == IssueKind::NestedArchive));

        // Changing inputs replaces owned issues instead of accumulating them.
        assess(&mut records, None);
        assert_eq!(records[0].entry.issues.len(), 2);
    }
}
