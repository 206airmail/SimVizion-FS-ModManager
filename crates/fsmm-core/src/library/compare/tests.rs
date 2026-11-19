//! Compare mode on generated folders.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use super::super::fixtures::{add, ctx, folder, library, write_folder_mod, write_mod};
use super::super::{index_by_name, Library};
use crate::model::{CompareRow, CompareState, ErrorCode, ModFormat};
use crate::scan::{self, FolderEntry};

struct Fixture {
    lib: Library,
    left: String,
    right: String,
    left_dir: std::path::PathBuf,
    right_dir: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library();
    let left_dir = folder(tmp.path(), "left");
    let right_dir = folder(tmp.path(), "right");
    Fixture {
        left: add(&lib, &left_dir).id,
        right: add(&lib, &right_dir).id,
        lib,
        left_dir,
        right_dir,
        _tmp: tmp,
    }
}

impl Fixture {
    fn compare(&self) -> Vec<CompareRow> {
        self.lib
            .compare_profiles(&self.left, &self.right, &ctx())
            .unwrap()
            .rows
    }
}

fn row<'a>(rows: &'a [CompareRow], name: &str) -> &'a CompareRow {
    rows.iter().find(|r| r.mod_name == name).unwrap()
}

#[test]
fn every_state_is_reported() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_OnlyLeft.zip", "1.0.0.0", b"a");
    write_mod(&f.right_dir, "FS25_OnlyRight.zip", "1.0.0.0", b"a");
    write_mod(&f.left_dir, "FS25_Same.zip", "1.0.0.0", b"same");
    write_mod(&f.right_dir, "FS25_Same.zip", "1.0.0.0", b"same");
    write_mod(&f.left_dir, "FS25_Version.zip", "1.0.0.0", b"x");
    write_mod(&f.right_dir, "FS25_Version.zip", "2.0.0.0", b"x");
    // Same version and same size, different bytes: only the CRC tells them apart.
    write_mod(&f.left_dir, "FS25_Content.zip", "1.0.0.0", b"aaaa");
    write_mod(&f.right_dir, "FS25_Content.zip", "1.0.0.0", b"aaab");

    let rows = f.compare();
    assert_eq!(rows.len(), 5);
    assert_eq!(row(&rows, "FS25_OnlyLeft").state, CompareState::OnlyLeft);
    assert_eq!(row(&rows, "FS25_OnlyRight").state, CompareState::OnlyRight);
    assert_eq!(row(&rows, "FS25_Same").state, CompareState::Identical);
    assert_eq!(
        row(&rows, "FS25_Version").state,
        CompareState::VersionDiffers
    );
    assert_eq!(
        row(&rows, "FS25_Content").state,
        CompareState::ContentDiffers
    );
}

#[test]
fn a_version_difference_wins_over_a_content_difference() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_A.zip", "1.0.0.0", b"one");
    write_mod(&f.right_dir, "FS25_A.zip", "1.1.0.0", b"two");
    assert_eq!(f.compare()[0].state, CompareState::VersionDiffers);
}

#[test]
fn versions_are_compared_as_trimmed_strings() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_A.zip", "  1.0.0.0 ", b"same");
    write_mod(&f.right_dir, "FS25_A.zip", "1.0.0.0", b"same");
    // The modDesc differs in whitespace only, so the zips are not byte-identical, but the
    // fingerprint (names, CRCs, sizes) differs only through modDesc.xml itself.
    let state = f.compare()[0].state;
    assert_eq!(state, CompareState::ContentDiffers);
    // "1.0" and "1.00" are different strings.
    write_mod(&f.left_dir, "FS25_B.zip", "1.0", b"same");
    write_mod(&f.right_dir, "FS25_B.zip", "1.00", b"same");
    assert_eq!(
        row(&f.compare(), "FS25_B").state,
        CompareState::VersionDiffers
    );
}

#[test]
fn rows_are_sorted_by_lower_cased_mod_name() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_b.zip", "1", b"");
    write_mod(&f.left_dir, "FS25_C.zip", "1", b"");
    write_mod(&f.right_dir, "FS25_A.zip", "1", b"");
    write_mod(&f.right_dir, "fs25_D.zip", "1", b"");
    let names: Vec<String> = f.compare().into_iter().map(|r| r.mod_name).collect();
    assert_eq!(names, ["FS25_A", "FS25_b", "FS25_C", "fs25_D"]);
}

#[test]
fn names_pair_case_insensitively_and_the_left_spelling_and_title_win() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_Mixed.zip", "1.0", b"same");
    write_mod(&f.right_dir, "fs25_mixed.zip", "1.0", b"same");
    let rows = f.compare();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].mod_name, "FS25_Mixed");
    assert_eq!(rows[0].state, CompareState::Identical);
    assert_eq!(
        rows[0].left.as_ref().unwrap().summary.file_name,
        "FS25_Mixed.zip"
    );
    assert_eq!(
        rows[0].right.as_ref().unwrap().summary.file_name,
        "fs25_mixed.zip"
    );
}

#[test]
fn the_title_is_the_left_copys_else_the_rights() {
    let f = fixture();
    write_mod(&f.right_dir, "FS25_R.zip", "1", b"");
    write_mod(&f.left_dir, "FS25_L.zip", "1", b"");
    // Titles come from modDesc.xml; both fixtures say "Title", so give one a different one.
    let path = f.right_dir.join("FS25_R.zip");
    crate::testutil::write_zip(
        &path,
        &[(
            "modDesc.xml",
            b"<modDesc descVersion=\"105\"><title><en>Right Title</en></title><version>1</version></modDesc>",
        )],
        crate::testutil::Method::Stored,
    );
    let rows = f.compare();
    assert_eq!(row(&rows, "FS25_R").title, "Right Title");
    assert_eq!(row(&rows, "FS25_L").title, "Title");
}

#[test]
fn sides_carry_the_mod_id_and_a_summary() {
    let f = fixture();
    let path = write_mod(&f.left_dir, "FS25_A.zip", "3.2.1.0", b"abc");
    let rows = f.compare();
    let side = rows[0].left.as_ref().unwrap();
    assert_eq!(side.mod_id, "FS25_A.zip");
    assert_eq!(side.summary.file_name, "FS25_A.zip");
    assert_eq!(side.summary.version, "3.2.1.0");
    assert_eq!(side.summary.size_bytes, fs::metadata(path).unwrap().len());
    assert!(side.summary.modified.ends_with('Z'));
    assert!(rows[0].right.is_none());
}

#[test]
fn folder_mods_compare_by_file_list_and_size() {
    let f = fixture();
    write_folder_mod(&f.left_dir, "FS25_Dir", "1.0", b"same");
    write_folder_mod(&f.right_dir, "FS25_Dir", "1.0", b"same");
    write_folder_mod(&f.left_dir, "FS25_Changed", "1.0", b"short");
    write_folder_mod(&f.right_dir, "FS25_Changed", "1.0", b"much longer");
    write_folder_mod(&f.left_dir, "FS25_Extra", "1.0", b"x");
    write_folder_mod(&f.right_dir, "FS25_Extra", "1.0", b"x");
    fs::write(f.right_dir.join("FS25_Extra").join("more.txt"), "x").unwrap();

    let rows = f.compare();
    assert_eq!(row(&rows, "FS25_Dir").state, CompareState::Identical);
    assert_eq!(
        row(&rows, "FS25_Changed").state,
        CompareState::ContentDiffers
    );
    assert_eq!(row(&rows, "FS25_Extra").state, CompareState::ContentDiffers);
    assert_eq!(
        row(&rows, "FS25_Dir")
            .left
            .as_ref()
            .unwrap()
            .summary
            .file_name,
        "FS25_Dir"
    );
}

#[test]
fn a_zip_and_its_unpacked_folder_are_not_called_identical() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_Same.zip", "1.0", b"same");
    write_folder_mod(&f.right_dir, "FS25_Same", "1.0", b"same");
    assert_eq!(f.compare()[0].state, CompareState::ContentDiffers);
}

#[test]
fn the_same_profile_on_both_sides_is_all_identical() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_A.zip", "1", b"a");
    write_folder_mod(&f.left_dir, "FS25_B", "1", b"b");
    let rows = f
        .lib
        .compare_profiles(&f.left, &f.left, &ctx())
        .unwrap()
        .rows;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.state == CompareState::Identical));
}

#[test]
fn empty_folders_compare_to_no_rows_and_the_ids_are_echoed() {
    let f = fixture();
    let comparison = f.lib.compare_profiles(&f.left, &f.right, &ctx()).unwrap();
    assert!(comparison.rows.is_empty());
    assert_eq!(comparison.left_profile_id, f.left);
    assert_eq!(comparison.right_profile_id, f.right);
}

#[test]
fn an_unknown_profile_is_profile_not_found_on_either_side() {
    let f = fixture();
    for (l, r) in [("p99", f.right.as_str()), (f.left.as_str(), "p99")] {
        let err = f.lib.compare_profiles(l, r, &ctx()).unwrap_err();
        assert_eq!(err.code, ErrorCode::ProfileNotFound);
    }
}

#[test]
fn a_missing_folder_is_folder_missing() {
    let f = fixture();
    fs::remove_dir_all(&f.right_dir).unwrap();
    let err = f
        .lib
        .compare_profiles(&f.left, &f.right, &ctx())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::FolderMissing);
}

#[test]
fn the_comparison_follows_the_disk_between_calls() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_A.zip", "1.0", b"a");
    assert_eq!(f.compare()[0].state, CompareState::OnlyLeft);
    // As after a copy through the transfer dialog.
    write_mod(&f.right_dir, "FS25_A.zip", "1.0", b"a");
    assert_eq!(f.compare()[0].state, CompareState::Identical);
}

#[test]
fn fingerprints_are_stable_across_a_rescan_and_a_cache_round_trip() {
    let f = fixture();
    write_mod(&f.left_dir, "FS25_A.zip", "1.0", b"a");
    write_mod(&f.right_dir, "FS25_A.zip", "1.0", b"a");
    write_mod(&f.left_dir, "FS25_B.zip", "1.0", b"b1");
    write_mod(&f.right_dir, "FS25_B.zip", "1.0", b"b2");
    let first = f.compare();
    // The second call reads every record back from cache.sqlite.
    let second = f.compare();
    assert_eq!(first, second);

    let cached = f
        .store_fingerprint(&f.left_dir.join("FS25_A.zip"))
        .expect("the scan stored a fingerprint");
    assert!(!cached.is_empty());
    let record = scan::read_mod(
        &FolderEntry {
            path: f.left_dir.join("FS25_A.zip"),
            name: "FS25_A.zip".to_string(),
            format: ModFormat::Zip,
            size: 0,
            modified: UNIX_EPOCH,
        },
        "x",
        &[],
    );
    assert_eq!(record.fingerprint, cached);
}

impl Fixture {
    /// The fingerprint `cache.sqlite` holds for a zip of this fixture, as of its last scan.
    fn store_fingerprint(&self, path: &Path) -> Option<String> {
        let meta = fs::metadata(path).unwrap();
        let stamp = crate::store::FileStamp {
            size: meta.len(),
            modified: meta.modified().unwrap(),
        };
        self.lib
            .store
            .cached_mod(path, stamp)
            .unwrap()
            .map(|c| c.fingerprint)
    }
}

#[test]
fn a_zip_wins_over_a_folder_of_the_same_name_in_one_profile() {
    let f = fixture();
    write_folder_mod(&f.left_dir, "FS25_Twin", "9.9", b"folder");
    write_mod(&f.left_dir, "FS25_Twin.zip", "1.0", b"zip");
    write_mod(&f.right_dir, "FS25_Twin.zip", "1.0", b"zip");
    let rows = f.compare();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, CompareState::Identical);
    assert_eq!(
        rows[0].left.as_ref().unwrap().summary.file_name,
        "FS25_Twin.zip"
    );
}

fn unreadable(name: &str, format: ModFormat) -> crate::scan::ModRecord {
    scan::read_mod(
        &FolderEntry {
            path: Path::new("does-not-exist").join(name),
            name: name.to_string(),
            format,
            size: 0,
            modified: UNIX_EPOCH,
        },
        "p",
        &[],
    )
}

#[test]
fn duplicate_names_pick_the_zip_then_the_smaller_file_name_whatever_the_order() {
    // Two spellings differing only by case cannot share a Windows folder, so the picking rule
    // is tested on records directly.
    let records = [
        unreadable("fs25_twin.zip", ModFormat::Zip),
        unreadable("FS25_Twin.zip", ModFormat::Zip),
        unreadable("FS25_Twin", ModFormat::Folder),
        unreadable("FS25_Solo", ModFormat::Folder),
    ];
    let index = index_by_name(&records);
    assert_eq!(index.len(), 2);
    assert_eq!(
        index["fs25_twin"], 1,
        "zip first, then the smaller file name"
    );
    assert_eq!(index["fs25_solo"], 3);

    let mut reversed = records.to_vec();
    reversed.reverse();
    let reversed_index = index_by_name(&reversed);
    assert_eq!(
        reversed[reversed_index["fs25_twin"]].entry.file_name,
        "FS25_Twin.zip"
    );
}

#[test]
fn unreadable_mods_with_no_fingerprint_compare_by_size() {
    let f = fixture();
    // Not zips at all: both have an empty fingerprint and no version.
    fs::write(f.left_dir.join("FS25_Bad.zip"), b"not a zip").unwrap();
    fs::write(f.right_dir.join("FS25_Bad.zip"), b"not a zip").unwrap();
    fs::write(f.left_dir.join("FS25_Bad2.zip"), b"short").unwrap();
    fs::write(f.right_dir.join("FS25_Bad2.zip"), b"a longer text").unwrap();
    let rows = f.compare();
    assert_eq!(row(&rows, "FS25_Bad").state, CompareState::Identical);
    assert_eq!(row(&rows, "FS25_Bad2").state, CompareState::ContentDiffers);
}
