//! Tags and notes: storage, validation, sharing between profiles and the listing.

use std::fs;

use super::super::fixtures::{add, folder, library, list, write_mod};
use super::*;
use crate::model::ModEntry;

fn tags(list: &[&str]) -> Vec<String> {
    list.iter().map(|t| (*t).to_string()).collect()
}

fn entry<'a>(mods: &'a [ModEntry], file: &str) -> &'a ModEntry {
    mods.iter().find(|m| m.file_name == file).unwrap()
}

fn detail(err: &ApiError) -> (ErrorCode, Option<&str>) {
    (err.code, err.detail.as_deref())
}

#[test]
fn tags_are_trimmed_deduplicated_case_insensitively_and_sorted() {
    let lib = library();
    let stored = lib
        .set_tags(
            "FS25_A",
            &tags(&[
                "  zebra ", "", "   ", "Apple", "apple", "APPLE", "mango", "Zebra",
            ]),
        )
        .unwrap();
    // The first spelling of each stays.
    assert_eq!(stored, tags(&["Apple", "mango", "zebra"]));
    assert_eq!(lib.list_tags().unwrap(), stored);
}

#[test]
fn setting_tags_replaces_and_an_empty_list_clears() {
    let lib = library();
    lib.set_tags("FS25_A", &tags(&["one", "two"])).unwrap();
    assert_eq!(
        lib.set_tags("FS25_A", &tags(&["three"])).unwrap(),
        tags(&["three"])
    );
    assert_eq!(lib.list_tags().unwrap(), tags(&["three"]));
    assert!(lib.set_tags("FS25_A", &[]).unwrap().is_empty());
    assert!(lib.list_tags().unwrap().is_empty());
    assert!(lib.store.all_mod_meta().unwrap().is_empty());
    // Only blanks is the same as none.
    assert!(lib
        .set_tags("FS25_A", &tags(&[" ", ""]))
        .unwrap()
        .is_empty());
}

#[test]
fn a_note_is_trimmed_and_an_empty_one_clears() {
    let lib = library();
    lib.set_note("FS25_A", "  needs a patch\nbefore use  ")
        .unwrap();
    assert_eq!(
        lib.store.mod_meta("fs25_a").unwrap().note.as_deref(),
        Some("needs a patch\nbefore use")
    );
    lib.set_note("FS25_A", " \n\t ").unwrap();
    assert_eq!(lib.store.mod_meta("fs25_a").unwrap().note, None);
    assert!(lib.store.all_mod_meta().unwrap().is_empty());
}

#[test]
fn tags_and_the_note_are_independent() {
    let lib = library();
    lib.set_tags("FS25_A", &tags(&["x"])).unwrap();
    lib.set_note("FS25_A", "n").unwrap();
    lib.set_tags("FS25_A", &[]).unwrap();
    assert_eq!(
        lib.store.mod_meta("fs25_a").unwrap().note.as_deref(),
        Some("n")
    );
    lib.set_tags("FS25_A", &tags(&["y"])).unwrap();
    lib.set_note("FS25_A", "").unwrap();
    assert_eq!(lib.store.mod_meta("fs25_a").unwrap().tags, tags(&["y"]));
}

#[test]
fn limits_are_errors_and_store_nothing() {
    let lib = library();
    lib.set_tags("FS25_A", &tags(&["kept"])).unwrap();

    let long = "x".repeat(MAX_TAG_CHARS + 1);
    let err = lib.set_tags("FS25_A", &[long]).unwrap_err();
    assert_eq!(detail(&err), (ErrorCode::Internal, Some(INVALID_TAG)));

    let many: Vec<String> = (0..=MAX_TAGS).map(|i| format!("tag{i}")).collect();
    let err = lib.set_tags("FS25_A", &many).unwrap_err();
    assert_eq!(detail(&err), (ErrorCode::Internal, Some(INVALID_TAG)));

    let err = lib
        .set_note("FS25_A", &"n".repeat(MAX_NOTE_CHARS + 1))
        .unwrap_err();
    assert_eq!(detail(&err), (ErrorCode::Internal, Some(INVALID_NOTE)));

    assert_eq!(lib.store.mod_meta("fs25_a").unwrap().tags, tags(&["kept"]));
    assert_eq!(lib.store.mod_meta("fs25_a").unwrap().note, None);
}

#[test]
fn the_limits_themselves_are_allowed_and_count_characters_not_bytes() {
    let lib = library();
    let exact_tag = "é".repeat(MAX_TAG_CHARS);
    assert_eq!(
        lib.set_tags("FS25_A", std::slice::from_ref(&exact_tag))
            .unwrap(),
        [exact_tag]
    );
    let twenty: Vec<String> = (0..MAX_TAGS).map(|i| format!("tag{i:02}")).collect();
    assert_eq!(lib.set_tags("FS25_A", &twenty).unwrap().len(), MAX_TAGS);
    // Duplicates and blanks do not count towards the limit.
    let mut padded = twenty.clone();
    padded.extend(twenty.iter().map(|t| t.to_uppercase()));
    padded.push(String::new());
    assert_eq!(lib.set_tags("FS25_A", &padded).unwrap().len(), MAX_TAGS);
    lib.set_note("FS25_A", &"é".repeat(MAX_NOTE_CHARS)).unwrap();
    // Padding with spaces does not make a note too long.
    let padded_note = format!("  {}  ", "n".repeat(MAX_NOTE_CHARS));
    lib.set_note("FS25_A", &padded_note).unwrap();
}

#[test]
fn an_empty_mod_name_is_refused() {
    let lib = library();
    for name in ["", "   "] {
        assert_eq!(
            detail(&lib.set_tags(name, &tags(&["x"])).unwrap_err()),
            (ErrorCode::Internal, Some(INVALID_NAME))
        );
        assert_eq!(
            detail(&lib.set_note(name, "x").unwrap_err()),
            (ErrorCode::Internal, Some(INVALID_NAME))
        );
    }
}

#[test]
fn list_tags_is_distinct_and_sorted_across_mods() {
    let lib = library();
    lib.set_tags("FS25_A", &tags(&["beta", "Alpha"])).unwrap();
    lib.set_tags("FS25_B", &tags(&["ALPHA", "gamma"])).unwrap();
    lib.set_tags("FS25_C", &tags(&["delta", "beta"])).unwrap();
    assert_eq!(
        lib.list_tags().unwrap(),
        tags(&["Alpha", "beta", "delta", "gamma"])
    );
}

#[test]
fn tags_and_notes_appear_on_the_listing_in_every_profile_whatever_the_case() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library();
    let one = folder(tmp.path(), "one");
    let two = folder(tmp.path(), "two");
    let p1 = add(&lib, &one).id;
    let p2 = add(&lib, &two).id;
    write_mod(&one, "FS25_Shared.zip", "1.0", b"a");
    write_mod(&one, "FS25_Plain.zip", "1.0", b"b");
    write_mod(&two, "fs25_shared.zip", "2.0", b"c");

    // Set through a spelling that matches neither file exactly.
    lib.set_tags("FS25_SHARED", &tags(&["Tractor", "old"]))
        .unwrap();
    lib.set_note("fs25_shared", "my note").unwrap();

    let first = list(&lib, &p1);
    let shared = entry(&first, "FS25_Shared.zip");
    assert_eq!(shared.tags, tags(&["old", "Tractor"]));
    assert_eq!(shared.note.as_deref(), Some("my note"));
    let plain = entry(&first, "FS25_Plain.zip");
    assert!(plain.tags.is_empty());
    assert_eq!(plain.note, None);

    let second = list(&lib, &p2);
    let shared = entry(&second, "fs25_shared.zip");
    assert_eq!(shared.tags, tags(&["old", "Tractor"]));
    assert_eq!(shared.note.as_deref(), Some("my note"));

    // Clearing is seen at once.
    lib.set_tags("FS25_Shared", &[]).unwrap();
    lib.set_note("FS25_Shared", "").unwrap();
    let shared = list(&lib, &p1);
    assert!(entry(&shared, "FS25_Shared.zip").tags.is_empty());
    assert_eq!(entry(&shared, "FS25_Shared.zip").note, None);
}

#[test]
fn tags_survive_a_rescan_a_replaced_file_and_a_removed_mod() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library();
    let dir = folder(tmp.path(), "mods");
    let id = add(&lib, &dir).id;
    write_mod(&dir, "FS25_A.zip", "1.0", b"a");
    lib.set_tags("FS25_A", &tags(&["keep"])).unwrap();
    lib.set_note("FS25_A", "note").unwrap();

    assert_eq!(list(&lib, &id)[0].tags, tags(&["keep"]));
    // The same listing again, now served from the scan cache.
    assert_eq!(list(&lib, &id)[0].tags, tags(&["keep"]));
    // An update replaces the file with a new version.
    write_mod(&dir, "FS25_A.zip", "2.0", b"updated");
    let updated = list(&lib, &id);
    assert_eq!(updated[0].version, "2.0");
    assert_eq!(updated[0].tags, tags(&["keep"]));
    assert_eq!(updated[0].note.as_deref(), Some("note"));
    // Removing the mod leaves the user's data for when it comes back.
    fs::remove_file(dir.join("FS25_A.zip")).unwrap();
    assert!(list(&lib, &id).is_empty());
    write_mod(&dir, "FS25_A.zip", "3.0", b"back");
    assert_eq!(list(&lib, &id)[0].tags, tags(&["keep"]));
}

#[test]
fn the_scan_cache_never_holds_tags() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library();
    let dir = folder(tmp.path(), "mods");
    let id = add(&lib, &dir).id;
    let path = write_mod(&dir, "FS25_A.zip", "1.0", b"a");
    lib.set_tags("FS25_A", &tags(&["x"])).unwrap();
    list(&lib, &id);
    let meta = fs::metadata(&path).unwrap();
    let cached = lib
        .store
        .cached_mod(
            &path,
            crate::store::FileStamp {
                size: meta.len(),
                modified: meta.modified().unwrap(),
            },
        )
        .unwrap()
        .unwrap();
    assert!(cached.entry.tags.is_empty());
}
