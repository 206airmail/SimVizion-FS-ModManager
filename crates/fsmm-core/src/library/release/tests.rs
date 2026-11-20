//! Health report, the chosen game folder and the app-update settings (plan M8): temp folders and
//! an in-memory library; never a real game folder.

use std::fs;
use std::time::UNIX_EPOCH;

use super::super::fixtures::{add, ctx, folder, library, write_mod};
use super::*;
use crate::model::IssueKind;
use crate::testutil::{write_zip, Method};

fn day(n: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(n * 24 * 60 * 60)
}

fn settings(enabled: bool, last_check: Option<SystemTime>) -> AppUpdateSettings {
    AppUpdateSettings {
        enabled,
        last_check: last_check.map(iso),
    }
}

// ---------------------------------------------------------------- app_update_due

#[test]
fn nothing_is_due_while_app_updates_are_off_even_when_forced() {
    let now = day(100);
    assert!(!app_update_due(&settings(false, None), now, false));
    assert!(!app_update_due(&settings(false, None), now, true));
    assert!(!app_update_due(&settings(false, Some(day(1))), now, true));
}

#[test]
fn a_forced_check_is_due_even_right_after_one() {
    let now = day(100);
    assert!(app_update_due(&settings(true, Some(now)), now, true));
}

#[test]
fn a_first_check_is_due() {
    assert!(app_update_due(&settings(true, None), day(100), false));
}

#[test]
fn a_check_is_due_from_24_hours_after_the_last_one() {
    let last = day(100);
    let s = settings(true, Some(last));
    assert!(!app_update_due(&s, last, false));
    assert!(!app_update_due(
        &s,
        last + APP_UPDATE_INTERVAL - Duration::from_secs(1),
        false
    ));
    assert!(app_update_due(&s, last + APP_UPDATE_INTERVAL, false));
    assert!(app_update_due(
        &s,
        last + APP_UPDATE_INTERVAL + Duration::from_secs(1),
        false
    ));
}

#[test]
fn a_last_check_in_the_future_is_due() {
    let s = settings(true, Some(day(101)));
    assert!(app_update_due(&s, day(100), false));
}

#[test]
fn an_unreadable_last_check_is_treated_as_never() {
    let s = AppUpdateSettings {
        enabled: true,
        last_check: Some("yesterday-ish".to_string()),
    };
    assert!(app_update_due(&s, day(100), false));
}

// ---------------------------------------------------------------- app-update settings

#[test]
fn app_updates_are_off_by_default_with_no_last_check() {
    let lib = library();
    assert_eq!(lib.app_update_settings().unwrap(), settings(false, None));
}

#[test]
fn switching_app_updates_on_and_off_is_stored() {
    let lib = library();
    let on = lib.set_app_updates_enabled(true).unwrap();
    assert!(on.enabled);
    assert_eq!(lib.app_update_settings().unwrap(), on);
    let off = lib.set_app_updates_enabled(false).unwrap();
    assert!(!off.enabled);
    assert_eq!(lib.app_update_settings().unwrap(), off);
}

#[test]
fn a_noted_check_is_stored_as_iso_time_and_survives_switching() {
    let lib = library();
    lib.note_app_update_check(day(20_000)).unwrap();
    lib.set_app_updates_enabled(true).unwrap();
    let stored = lib.app_update_settings().unwrap();
    assert_eq!(stored.last_check.as_deref(), Some("2024-10-04T00:00:00Z"));
    assert!(!app_update_due(&stored, day(20_000), false));
    assert!(app_update_due(&stored, day(20_001), false));
}

#[test]
fn app_update_settings_do_not_touch_the_mod_update_settings() {
    let lib = library();
    lib.set_app_updates_enabled(true).unwrap();
    lib.note_app_update_check(day(5)).unwrap();
    let mods = lib.update_settings().unwrap();
    assert!(!mods.enabled);
    assert_eq!(mods.last_check, None);
}

// ---------------------------------------------------------------- chosen game folder

#[test]
fn no_game_folder_is_chosen_by_default() {
    assert_eq!(library().chosen_game_folder().unwrap(), None);
}

#[test]
fn a_folder_with_a_mods_folder_or_game_settings_can_be_chosen() {
    let root = tempfile::tempdir().unwrap();
    let with_mods = folder(root.path(), "a");
    fs::create_dir_all(with_mods.join("mods")).unwrap();
    let with_settings = folder(root.path(), "b");
    fs::write(with_settings.join("gameSettings.xml"), b"<gameSettings/>").unwrap();

    let lib = library();
    lib.set_chosen_game_folder(Some(&with_mods)).unwrap();
    assert_eq!(lib.chosen_game_folder().unwrap(), Some(with_mods));
    lib.set_chosen_game_folder(Some(&with_settings)).unwrap();
    assert_eq!(lib.chosen_game_folder().unwrap(), Some(with_settings));
}

#[test]
fn an_invalid_folder_is_refused_and_the_previous_choice_stays() {
    let root = tempfile::tempdir().unwrap();
    let good = folder(root.path(), "good");
    fs::create_dir_all(good.join("mods")).unwrap();
    let empty = folder(root.path(), "empty");
    let file = root.path().join("gameSettings.xml.txt");
    fs::write(&file, b"x").unwrap();
    // A `mods` that is a file does not count.
    let odd = folder(root.path(), "odd");
    fs::write(odd.join("mods"), b"x").unwrap();

    let lib = library();
    lib.set_chosen_game_folder(Some(&good)).unwrap();
    for bad in [&empty, &file, &odd, &root.path().join("missing")] {
        let err = lib.set_chosen_game_folder(Some(bad)).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidGameFolder, "{bad:?}");
    }
    assert_eq!(lib.chosen_game_folder().unwrap(), Some(good));
}

#[test]
fn clearing_forgets_the_chosen_folder() {
    let root = tempfile::tempdir().unwrap();
    let dir = folder(root.path(), "a");
    fs::create_dir_all(dir.join("mods")).unwrap();
    let lib = library();
    lib.set_chosen_game_folder(Some(&dir)).unwrap();
    lib.set_chosen_game_folder(None).unwrap();
    assert_eq!(lib.chosen_game_folder().unwrap(), None);
    // Clearing when nothing is chosen is fine.
    lib.set_chosen_game_folder(None).unwrap();
}

/// The choice is validated when it is made, not when it is read: a folder that is unavailable
/// right now (a disconnected drive) is still the user's choice, so no write goes to the detected
/// folder instead. (This test used to expect `None`, which made that happen silently.)
#[test]
fn a_chosen_folder_that_became_unavailable_is_still_the_choice() {
    let root = tempfile::tempdir().unwrap();
    let dir = folder(root.path(), "a");
    fs::create_dir_all(dir.join("mods")).unwrap();
    let lib = library();
    lib.set_chosen_game_folder(Some(&dir)).unwrap();

    fs::remove_dir(dir.join("mods")).unwrap();
    assert_eq!(lib.chosen_game_folder().unwrap(), Some(dir.clone()));
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(lib.chosen_game_folder().unwrap(), Some(dir.clone()));
    // Setting one still validates, and forgetting it is the only way back to detection.
    assert_eq!(
        lib.set_chosen_game_folder(Some(&dir)).unwrap_err().code,
        ErrorCode::InvalidGameFolder
    );
    assert_eq!(lib.chosen_game_folder().unwrap(), Some(dir));
    lib.set_chosen_game_folder(None).unwrap();
    assert_eq!(lib.chosen_game_folder().unwrap(), None);
}

// ---------------------------------------------------------------- health report

/// A mod whose file name the game would not accept, so it has an `InvalidFileName` issue.
fn bad_name(dir: &Path, file: &str, title: &str) {
    write_zip(
        &dir.join(file),
        &[("modDesc.xml", titled(title).as_bytes())],
        Method::Stored,
    );
}

fn titled(title: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <modDesc descVersion=\"105\"><title><en>{title}</en></title>\
         <author>Someone</author><version>1.0.0.0</version></modDesc>"
    )
}

#[test]
fn health_report_of_no_profiles_is_empty() {
    let report = library().health_report(&ctx()).unwrap();
    assert!(report.rows.is_empty() && report.counts.is_empty());
}

#[test]
fn health_report_lists_mods_with_issues_by_profile_then_title_with_counts_per_kind() {
    let root = tempfile::tempdir().unwrap();
    let one = folder(root.path(), "one");
    let two = folder(root.path(), "two");
    bad_name(&one, "bad name.zip", "beta");
    bad_name(&one, "also bad.zip", "Alpha");
    bad_name(&one, "worse name.zip", "alpha");
    write_mod(&one, "FS25_Fine.zip", "1.0.0.0", b"");
    fs::write(one.join("FS25_Broken.zip"), b"not a zip").unwrap();
    bad_name(&two, "x y.zip", "Zed");
    write_mod(&two, "FS25_AlsoFine.zip", "1.0.0.0", b"");

    // Registered in this order: "two" first, so it is listed first.
    let lib = library();
    let two_id = add(&lib, &two).id;
    let one_id = add(&lib, &one).id;
    let report = lib.health_report(&ctx()).unwrap();

    let rows: Vec<(&str, &str, &str)> = report
        .rows
        .iter()
        .map(|r| {
            (
                r.profile_name.as_str(),
                r.title.as_str(),
                r.mod_name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("two", "Zed", "x y"),
            // Case-insensitive title order; equal titles by mod id.
            ("one", "Alpha", "also bad"),
            ("one", "alpha", "worse name"),
            ("one", "beta", "bad name"),
            ("one", "FS25_Broken", "FS25_Broken"),
        ]
    );
    assert_eq!(report.rows[0].profile_id, two_id);
    assert_eq!(report.rows[1].profile_id, one_id);

    // Row fields agree with the mod listing the Mods screen shows.
    let listed = lib.list_mods(&one_id, &ctx(), &|_| {}).unwrap();
    for row in report.rows.iter().filter(|r| r.profile_id == one_id) {
        let entry = listed.iter().find(|m| m.id == row.mod_id).unwrap();
        assert_eq!(row.issues, entry.issues);
        assert_eq!(row.title, entry.title);
        assert_eq!(row.mod_name, entry.mod_name);
    }
    let broken = report
        .rows
        .iter()
        .find(|r| r.mod_name == "FS25_Broken")
        .unwrap();
    assert_eq!(
        broken.issues.iter().map(|i| i.kind).collect::<Vec<_>>(),
        [IssueKind::NotAMod]
    );

    // Counts: mods with at least one issue of the kind, in enum order, only kinds present.
    let counts: Vec<(IssueKind, u32)> = report.counts.iter().map(|c| (c.kind, c.count)).collect();
    assert_eq!(
        counts,
        [(IssueKind::NotAMod, 1), (IssueKind::InvalidFileName, 4)]
    );
}

#[test]
fn health_report_counts_a_mod_once_per_kind_even_with_several_issues_of_it() {
    let root = tempfile::tempdir().unwrap();
    let dir = folder(root.path(), "mods");
    // Not a mod and an invalid name: one row, one count for each kind.
    fs::write(dir.join("not a mod.zip"), b"garbage").unwrap();
    let lib = library();
    add(&lib, &dir);
    let report = lib.health_report(&ctx()).unwrap();
    assert_eq!(report.rows.len(), 1);
    let kinds: Vec<IssueKind> = report.rows[0].issues.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&IssueKind::NotAMod) && kinds.contains(&IssueKind::InvalidFileName));
    let counts: Vec<(IssueKind, u32)> = report.counts.iter().map(|c| (c.kind, c.count)).collect();
    assert_eq!(
        counts,
        [(IssueKind::NotAMod, 1), (IssueKind::InvalidFileName, 1)]
    );
}

#[test]
fn health_rows_equal_the_sum_of_the_profiles_issue_counts() {
    let root = tempfile::tempdir().unwrap();
    let one = folder(root.path(), "one");
    let two = folder(root.path(), "two");
    let three = folder(root.path(), "three");
    bad_name(&one, "bad name.zip", "A");
    fs::write(one.join("FS25_Broken.zip"), b"not a zip").unwrap();
    write_mod(&one, "FS25_Fine.zip", "1.0.0.0", b"");
    fs::write(two.join("not a mod.zip"), b"garbage").unwrap();
    write_mod(&three, "FS25_Fine.zip", "1.0.0.0", b"");

    let lib = library();
    for dir in [&one, &two, &three] {
        add(&lib, dir);
    }
    let summed: u32 = lib
        .list_profiles(&ctx())
        .unwrap()
        .iter()
        .map(|p| p.issue_count)
        .sum();
    let report = lib.health_report(&ctx()).unwrap();
    assert_eq!(summed, 3);
    assert_eq!(u32::try_from(report.rows.len()).unwrap(), summed);
}

#[test]
fn a_profile_whose_folder_is_missing_contributes_nothing_and_does_not_fail_the_report() {
    let root = tempfile::tempdir().unwrap();
    let gone = folder(root.path(), "gone");
    let here = folder(root.path(), "here");
    bad_name(&gone, "bad name.zip", "Gone");
    bad_name(&here, "bad name.zip", "Here");
    let lib = library();
    add(&lib, &gone);
    let here_id = add(&lib, &here).id;
    fs::remove_dir_all(&gone).unwrap();

    let report = lib.health_report(&ctx()).unwrap();
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].profile_id, here_id);
    assert_eq!(report.rows[0].title, "Here");
    let counts: Vec<(IssueKind, u32)> = report.counts.iter().map(|c| (c.kind, c.count)).collect();
    assert_eq!(counts, [(IssueKind::InvalidFileName, 1)]);
}
