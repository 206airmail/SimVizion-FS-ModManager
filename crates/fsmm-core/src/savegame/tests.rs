//! Savegame reading on generated fixtures in temp folders.

use std::fs;

use super::*;
use crate::testutil::{save_mod_xml, save_xml, write_save_file};

fn user() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn a_valid_save_is_read_completely() {
    let dir = user();
    let hash = "0123456789abcdef0123456789abcdef";
    write_save_file(
        dir.path(),
        "savegame1",
        &save_xml(&[
            save_mod_xml("FS25_TestMap", "1.0.0.0", Some(true), hash),
            save_mod_xml("FS25_Tool", "2.1.0.0", Some(false), ""),
        ]),
    );
    let file = read(dir.path(), "savegame1").unwrap();
    assert_eq!(file.info.id, "savegame1");
    assert_eq!(file.info.slot, 1);
    assert_eq!(file.info.name, "My Farm");
    assert_eq!(file.info.map_title, "Test Map");
    assert_eq!(file.info.map_mod.as_deref(), Some("FS25_TestMap"));
    assert_eq!(file.info.save_date.as_deref(), Some("2026-11-19"));
    assert_eq!(file.info.money, Some(12345));
    assert_eq!(file.info.play_minutes, Some(90));
    assert_eq!(file.info.mod_count, 2);
    assert_eq!(
        file.mods,
        vec![
            SavedMod {
                mod_name: "FS25_TestMap".into(),
                title: "Title of FS25_TestMap".into(),
                version: "1.0.0.0".into(),
                required: true,
                file_hash: hash.into(),
            },
            SavedMod {
                mod_name: "FS25_Tool".into(),
                title: "Title of FS25_Tool".into(),
                version: "2.1.0.0".into(),
                required: false,
                file_hash: String::new(),
            },
        ]
    );
}

#[test]
fn a_missing_required_attribute_means_required() {
    let dir = user();
    write_save_file(
        dir.path(),
        "savegame1",
        &save_xml(&[save_mod_xml("FS25_A", "1", None, "")]),
    );
    assert!(read(dir.path(), "savegame1").unwrap().mods[0].required);
}

#[test]
fn file_hashes_are_lower_cased_and_trimmed() {
    let dir = user();
    write_save_file(
        dir.path(),
        "savegame1",
        &save_xml(&[save_mod_xml("FS25_A", "1", None, " ABCDEF0123 ")]),
    );
    assert_eq!(
        read(dir.path(), "savegame1").unwrap().mods[0].file_hash,
        "abcdef0123"
    );
}

#[test]
fn a_dlc_entry_is_an_ordinary_mod_entry() {
    let dir = user();
    write_save_file(
        dir.path(),
        "savegame1",
        &save_xml(&[save_mod_xml("pdlc_testPack", "1.0.0.0", Some(false), "")]),
    );
    let file = read(dir.path(), "savegame1").unwrap();
    assert_eq!(file.mods[0].mod_name, "pdlc_testPack");
}

#[test]
fn a_base_game_map_has_no_map_mod() {
    let dir = user();
    let xml = save_xml(&[]).replace("FS25_TestMap.MapUS", "MapUS");
    write_save_file(dir.path(), "savegame1", &xml);
    let info = read(dir.path(), "savegame1").unwrap().info;
    assert_eq!(info.map_mod, None);
    assert_eq!(info.map_title, "Test Map");
}

#[test]
fn valid_false_is_not_a_save() {
    let dir = user();
    let xml = save_xml(&[]).replace("valid=\"true\"", "valid=\"false\"");
    write_save_file(dir.path(), "savegame1", &xml);
    assert_eq!(
        read(dir.path(), "savegame1").unwrap_err(),
        SavegameError::Missing
    );
    assert!(list(dir.path()).is_empty());
}

#[test]
fn a_missing_valid_attribute_is_not_a_save() {
    let dir = user();
    let xml = save_xml(&[]).replace(" valid=\"true\"", "");
    write_save_file(dir.path(), "savegame1", &xml);
    assert_eq!(
        read(dir.path(), "savegame1").unwrap_err(),
        SavegameError::Missing
    );
}

#[test]
fn another_revision_is_not_a_save() {
    let dir = user();
    let xml = save_xml(&[]).replace("revision=\"2\"", "revision=\"3\"");
    write_save_file(dir.path(), "savegame1", &xml);
    assert_eq!(
        read(dir.path(), "savegame1").unwrap_err(),
        SavegameError::Missing
    );
}

#[test]
fn another_root_element_is_not_a_save() {
    let dir = user();
    write_save_file(
        dir.path(),
        "savegame1",
        "<other revision=\"2\" valid=\"true\"/>",
    );
    assert_eq!(
        read(dir.path(), "savegame1").unwrap_err(),
        SavegameError::Missing
    );
}

#[test]
fn garbled_xml_is_unreadable_without_leaking_content() {
    let dir = user();
    write_save_file(
        dir.path(),
        "savegame1",
        "<careerSavegame revision=\"2\" secret-password",
    );
    match read(dir.path(), "savegame1") {
        Err(SavegameError::Unreadable(detail)) => assert!(!detail.contains("secret")),
        other => panic!("expected Unreadable, got {other:?}"),
    }
    // An unreadable save is left out of the list; it is reported when opened.
    assert!(list(dir.path()).is_empty());
}

#[test]
fn invalid_utf8_is_unreadable() {
    let dir = user();
    fs::create_dir_all(dir.path().join("savegame1")).unwrap();
    fs::write(
        dir.path().join("savegame1").join("careerSavegame.xml"),
        [0xff, 0xfe, 0x00],
    )
    .unwrap();
    assert!(matches!(
        read(dir.path(), "savegame1"),
        Err(SavegameError::Unreadable(_))
    ));
}

#[test]
fn a_slot_folder_without_the_file_is_missing() {
    let dir = user();
    let slot = dir.path().join("savegame2");
    fs::create_dir_all(&slot).unwrap();
    fs::write(slot.join("steam_autocloud.vdf"), "x").unwrap();
    assert_eq!(
        read(dir.path(), "savegame2").unwrap_err(),
        SavegameError::Missing
    );
    assert_eq!(
        read(dir.path(), "savegame9").unwrap_err(),
        SavegameError::Missing
    );
    assert!(list(dir.path()).is_empty());
}

#[test]
fn only_slot_ids_are_accepted() {
    let dir = user();
    write_save_file(dir.path(), "savegame1", &save_xml(&[]));
    // A file the id could reach if it were joined unchecked.
    let outside = dir.path().join("x");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("careerSavegame.xml"), save_xml(&[])).unwrap();
    for id in [
        "..\\x",
        "../x",
        "savegameX",
        "savegame",
        "savegame1\\..",
        "savegame1/",
        "Savegame1",
        "savegame-1",
        "savegame 1",
        "",
        "x",
        "savegameBackup",
        "savegame99999999999999",
    ] {
        assert_eq!(
            read(dir.path(), id).unwrap_err(),
            SavegameError::Missing,
            "{id}"
        );
    }
}

#[test]
fn negative_money_and_fractional_play_time() {
    let dir = user();
    let xml = save_xml(&[])
        .replace("<money>12345</money>", "<money>-2500</money>")
        .replace(
            "<playTime>90.4</playTime>",
            "<playTime>1234.5678</playTime>",
        );
    write_save_file(dir.path(), "savegame1", &xml);
    let info = read(dir.path(), "savegame1").unwrap().info;
    assert_eq!(info.money, Some(-2500));
    assert_eq!(info.play_minutes, Some(1235));
}

#[test]
fn play_time_rounds_to_the_nearest_minute() {
    for (text, expected) in [
        ("0", Some(0)),
        ("0.49", Some(0)),
        ("0.5", Some(1)),
        ("59.6", Some(60)),
        ("-3", Some(0)),
        ("junk", None),
        ("", None),
    ] {
        assert_eq!(minutes(text), expected, "{text:?}");
    }
}

#[test]
fn absent_statistics_are_none() {
    let dir = user();
    write_save_file(
        dir.path(),
        "savegame1",
        "<careerSavegame revision=\"2\" valid=\"true\"/>",
    );
    let info = read(dir.path(), "savegame1").unwrap().info;
    assert_eq!(info.money, None);
    assert_eq!(info.play_minutes, None);
    assert_eq!(info.save_date, None);
    assert_eq!(info.name, "");
    assert_eq!(info.mod_count, 0);
}

#[test]
fn list_returns_valid_slots_by_slot_number_and_ignores_the_rest() {
    let dir = user();
    write_save_file(dir.path(), "savegame10", &save_xml(&[]));
    write_save_file(dir.path(), "savegame2", &save_xml(&[]));
    write_save_file(dir.path(), "savegame3", "<broken");
    write_save_file(
        dir.path(),
        "savegame4",
        &save_xml(&[]).replace("valid=\"true\"", "valid=\"false\""),
    );
    fs::create_dir_all(dir.path().join("savegame5")).unwrap();
    // Backups and other folders are not slots, even with a save inside.
    write_save_file(dir.path(), "savegameBackup", &save_xml(&[]));
    fs::create_dir_all(dir.path().join("mods")).unwrap();
    fs::write(dir.path().join("savegame6"), "a file, not a folder").unwrap();

    let slots: Vec<u32> = list(dir.path()).iter().map(|s| s.slot).collect();
    assert_eq!(slots, [2, 10]);
}

#[test]
fn list_of_a_missing_folder_is_empty() {
    let dir = user();
    assert!(list(&dir.path().join("nope")).is_empty());
}

#[test]
fn nothing_but_the_career_file_is_opened() {
    // farms.xml can hold passwords: reading must not depend on it, even when it is garbage
    // or unreadable as a file.
    let dir = user();
    write_save_file(dir.path(), "savegame1", &save_xml(&[]));
    let slot = dir.path().join("savegame1");
    fs::write(slot.join("farms.xml"), [0xff, 0xff]).unwrap();
    fs::create_dir_all(slot.join("farms2.xml")).unwrap();
    assert!(read(dir.path(), "savegame1").is_ok());
    assert_eq!(list(dir.path()).len(), 1);
}
