//! File primitives against temp folders and a fake Recycle Bin; never a real game folder.

use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tempfile::TempDir;

use super::fakes::FakeRecycler;
use super::*;
use crate::scan;
use crate::testutil::{write_folder, write_zip, Method};

const MIB: usize = 1024 * 1024;

struct Dirs {
    _tmp: TempDir,
    src: PathBuf,
    dst: PathBuf,
    recycler: FakeRecycler,
}

fn dirs() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    let recycler = FakeRecycler::new(&tmp.path().join("bin"));
    Dirs {
        _tmp: tmp,
        src,
        dst,
        recycler,
    }
}

fn mtime(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

fn set_mtime(path: &Path, time: SystemTime) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A zip with a mod description and a payload of `payload` bytes.
fn zip_mod(path: &Path, payload: usize) {
    let data: Vec<u8> = (0..payload).map(|i| (i % 251) as u8).collect();
    write_zip(
        path,
        &[("modDesc.xml", b"<modDesc/>"), ("data.bin", &data)],
        Method::Stored,
    );
}

fn folder_mod(root: &Path) {
    write_folder(
        root,
        &[
            ("modDesc.xml", b"<modDesc/>"),
            ("scripts/a.lua", b"print(1)"),
            ("scripts/deep/b.lua", b"print(22)"),
            ("textures/t.dds", &[7u8; 5000]),
        ],
    );
    fs::create_dir_all(root.join("empty")).unwrap();
}

#[test]
fn staging_name_is_hidden_and_suffixed() {
    assert_eq!(staging_name("FS25_Mod.zip"), ".FS25_Mod.zip.fsmm-partial");
}

#[test]
fn size_of_a_file_and_a_folder() {
    let d = dirs();
    let zip = d.src.join("A.zip");
    zip_mod(&zip, 1000);
    assert_eq!(size_of(&zip).unwrap(), fs::metadata(&zip).unwrap().len());
    let folder = d.src.join("F");
    folder_mod(&folder);
    assert_eq!(size_of(&folder).unwrap(), 10 + 8 + 9 + 5000);
    assert!(size_of(&d.src.join("missing")).is_err());
}

#[test]
fn copies_a_zip_keeping_its_mtime_and_reporting_progress() {
    let d = dirs();
    let src = d.src.join("FS25_Big.zip");
    zip_mod(&src, 9 * MIB);
    set_mtime(&src, mtime(1_500_000_000));
    let size = fs::metadata(&src).unwrap().len();

    let mut reports = Vec::new();
    let out = copy_staged(&src, &d.dst, &mut |n| reports.push(n), &never()).unwrap();

    assert_eq!(out, d.dst.join("FS25_Big.zip"));
    assert_eq!(fs::read(&out).unwrap(), fs::read(&src).unwrap());
    assert_eq!(modified(&out), mtime(1_500_000_000));
    assert_eq!(reports.iter().sum::<u64>(), size);
    assert!(reports.len() >= 3, "9 MiB is at least three 4 MiB chunks");
    assert!(reports.iter().all(|&n| n as usize <= 4 * MIB));
    assert_eq!(names(&d.dst), ["FS25_Big.zip"]);
    assert!(src.exists());
}

#[test]
fn copies_a_folder_with_subfolders_and_keeps_file_mtimes() {
    let d = dirs();
    let src = d.src.join("FS25_Folder");
    folder_mod(&src);
    set_mtime(&src.join("scripts/a.lua"), mtime(1_400_000_000));
    set_mtime(&src.join("textures/t.dds"), mtime(1_300_000_000));

    let mut total = 0;
    let out = copy_staged(&src, &d.dst, &mut |n| total += n, &never()).unwrap();

    assert_eq!(out, d.dst.join("FS25_Folder"));
    assert_eq!(total, 10 + 8 + 9 + 5000);
    assert_eq!(
        file_list(&src).unwrap(),
        file_list(&out).unwrap(),
        "same files and sizes"
    );
    assert!(out.join("empty").is_dir());
    assert_eq!(modified(&out.join("scripts/a.lua")), mtime(1_400_000_000));
    assert_eq!(modified(&out.join("textures/t.dds")), mtime(1_300_000_000));
    assert_eq!(names(&d.dst), ["FS25_Folder"]);
}

#[test]
fn a_source_that_is_not_a_readable_zip_is_copied_as_is() {
    let d = dirs();
    let src = d.src.join("Broken.zip");
    fs::write(&src, b"not really a zip").unwrap();
    let out = copy_staged(&src, &d.dst, &mut |_| {}, &never()).unwrap();
    assert_eq!(fs::read(out).unwrap(), b"not really a zip");
}

#[test]
fn refuses_an_existing_target_before_writing_anything() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    fs::write(d.dst.join("A.zip"), b"mine").unwrap();

    let mut called = false;
    let err = copy_staged(&src, &d.dst, &mut |_| called = true, &never()).unwrap_err();

    assert!(matches!(err, FileOpError::Exists));
    assert!(!called);
    assert_eq!(names(&d.dst), ["A.zip"]);
    assert_eq!(fs::read(d.dst.join("A.zip")).unwrap(), b"mine");
}

#[test]
fn an_existing_target_is_found_whatever_its_case() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    fs::write(d.dst.join("a.ZIP"), b"mine").unwrap();
    let err = copy_staged(&src, &d.dst, &mut |_| {}, &never()).unwrap_err();
    assert!(matches!(err, FileOpError::Exists));
}

#[test]
fn cancelling_mid_copy_leaves_neither_a_staged_nor_a_final_copy() {
    let d = dirs();
    let src = d.src.join("Big.zip");
    zip_mod(&src, 9 * MIB);
    let cancel = never();

    let mut calls = 0;
    let err = copy_staged(
        &src,
        &d.dst,
        &mut |_| {
            calls += 1;
            cancel.store(true, Ordering::SeqCst);
        },
        &cancel,
    )
    .unwrap_err();

    assert!(matches!(err, FileOpError::Cancelled));
    assert_eq!(calls, 1, "no chunk is written after the cancel");
    assert!(names(&d.dst).is_empty());
    assert!(src.exists());
}

#[test]
fn cancelling_a_folder_copy_removes_the_staged_folder() {
    let d = dirs();
    let src = d.src.join("Folder");
    write_folder(
        &src,
        &[
            ("a.bin", &vec![1u8; 5 * MIB]),
            ("b/c.bin", &vec![2u8; 5 * MIB]),
        ],
    );
    let cancel = never();
    let err = copy_staged(
        &src,
        &d.dst,
        &mut |_| cancel.store(true, Ordering::SeqCst),
        &cancel,
    )
    .unwrap_err();
    assert!(matches!(err, FileOpError::Cancelled));
    assert!(names(&d.dst).is_empty());
}

#[test]
fn a_cancel_set_before_the_copy_writes_nothing() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    let cancel = AtomicBool::new(true);
    let err = copy_staged(&src, &d.dst, &mut |_| {}, &cancel).unwrap_err();
    assert!(matches!(err, FileOpError::Cancelled));
    assert!(names(&d.dst).is_empty());
}

#[test]
fn a_failed_verification_removes_the_staged_zip() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 5000);
    let seen = Cell::new(false);

    let err = copy_staged_with(&src, &d.dst, &mut |_| {}, &never(), &|staged| {
        seen.set(staged.exists());
        OpenOptions::new()
            .write(true)
            .open(staged)
            .unwrap()
            .set_len(10)
            .unwrap();
    })
    .unwrap_err();

    assert!(seen.get(), "the hook ran on the staged copy");
    assert!(
        matches!(err, FileOpError::Verify(ref d) if d == "size"),
        "{err}"
    );
    assert!(names(&d.dst).is_empty());
}

#[test]
fn a_zip_with_the_wrong_entry_count_fails_verification() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 5000);
    let size = fs::metadata(&src).unwrap().len() as usize;

    let err = copy_staged_with(&src, &d.dst, &mut |_| {}, &never(), &|staged| {
        // Same length, but not an archive any more.
        fs::write(staged, vec![0u8; size]).unwrap();
    })
    .unwrap_err();

    assert!(
        matches!(err, FileOpError::Verify(ref d) if d == "zipEntries"),
        "{err}"
    );
    assert!(names(&d.dst).is_empty());
}

#[test]
fn a_failed_folder_verification_removes_the_staged_folder() {
    let d = dirs();
    let src = d.src.join("F");
    folder_mod(&src);

    let err = copy_staged_with(&src, &d.dst, &mut |_| {}, &never(), &|staged| {
        fs::remove_file(staged.join("scripts/a.lua")).unwrap();
    })
    .unwrap_err();

    assert!(
        matches!(err, FileOpError::Verify(ref d) if d == "files"),
        "{err}"
    );
    assert!(names(&d.dst).is_empty());
}

#[test]
fn the_staged_copy_is_invisible_to_the_scanner_while_it_exists() {
    let d = dirs();
    for (name, folder) in [("Z.zip", false), ("F", true)] {
        let src = d.src.join(name);
        if folder {
            folder_mod(&src);
        } else {
            zip_mod(&src, 100);
        }
        let listed = scan::enumerate(&d.dst).unwrap().0.len();
        let seen = Cell::new(usize::MAX);
        let staging_exists = Cell::new(false);
        copy_staged_with(&src, &d.dst, &mut |_| {}, &never(), &|staged| {
            staging_exists.set(staged.exists());
            seen.set(scan::enumerate(&d.dst).unwrap().0.len());
        })
        .unwrap();
        assert!(staging_exists.get());
        assert_eq!(seen.get(), listed, "{name}: only finished mods are listed");
    }
    assert_eq!(scan::enumerate(&d.dst).unwrap().0.len(), 2);
}

#[test]
fn a_leftover_staged_copy_is_replaced() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    fs::write(d.dst.join(staging_name("A.zip")), b"left by a crash").unwrap();
    copy_staged(&src, &d.dst, &mut |_| {}, &never()).unwrap();
    assert_eq!(names(&d.dst), ["A.zip"]);
}

#[test]
fn a_missing_target_folder_is_an_io_error_and_leaves_nothing() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    let err = copy_staged(&src, &d.dst.join("nowhere"), &mut |_| {}, &never()).unwrap_err();
    assert!(
        matches!(err, FileOpError::Io(ref k) if k == "NotFound"),
        "{err}"
    );
}

#[test]
fn a_move_on_one_drive_is_a_rename() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    let bytes = fs::read(&src).unwrap();

    let mut called = false;
    let out = move_item(&src, &d.dst, &mut |_| called = true, &never(), &d.recycler).unwrap();

    assert_eq!(out, d.dst.join("A.zip"));
    assert_eq!(fs::read(&out).unwrap(), bytes);
    assert!(!src.exists());
    assert!(!called, "a rename moves no bytes");
    assert!(d.recycler.calls().is_empty());
}

#[test]
fn a_move_refuses_an_existing_target() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    fs::write(d.dst.join("A.zip"), b"mine").unwrap();
    let err = move_item(&src, &d.dst, &mut |_| {}, &never(), &d.recycler).unwrap_err();
    assert!(matches!(err, FileOpError::Exists));
    assert!(src.exists());
    assert_eq!(fs::read(d.dst.join("A.zip")).unwrap(), b"mine");
}

fn crosses_devices(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::ErrorKind::CrossesDevices.into())
}

#[test]
fn a_move_across_drives_copies_then_recycles_the_source() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 3000);
    let bytes = fs::read(&src).unwrap();

    let mut total = 0;
    let out = move_item_with(
        &src,
        &d.dst,
        &mut |n| total += n,
        &never(),
        &d.recycler,
        &crosses_devices,
    )
    .unwrap();

    assert_eq!(fs::read(&out).unwrap(), bytes);
    assert_eq!(total, bytes.len() as u64);
    assert!(!src.exists());
    assert_eq!(d.recycler.calls(), [src]);
    assert_eq!(d.recycler.binned(), ["A.zip"]);
}

#[test]
fn a_folder_move_across_drives_recycles_the_whole_source_folder() {
    let d = dirs();
    let src = d.src.join("F");
    folder_mod(&src);
    let out = move_item_with(
        &src,
        &d.dst,
        &mut |_| {},
        &never(),
        &d.recycler,
        &crosses_devices,
    )
    .unwrap();
    assert!(out.join("scripts/deep/b.lua").is_file());
    assert!(!src.exists());
    assert_eq!(d.recycler.binned(), ["F"]);
}

#[test]
fn a_recycle_refusal_after_a_good_copy_keeps_both_copies() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 3000);
    d.recycler.refuse(&src);

    let err = move_item_with(
        &src,
        &d.dst,
        &mut |_| {},
        &never(),
        &d.recycler,
        &crosses_devices,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        FileOpError::Recycle(RecycleError::Unavailable)
    ));
    assert!(src.exists());
    assert_eq!(
        fs::read(d.dst.join("A.zip")).unwrap(),
        fs::read(&src).unwrap()
    );
    assert!(d.recycler.binned().is_empty());
}

#[test]
fn a_cancelled_cross_drive_move_leaves_the_source_alone() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 9 * MIB);
    let cancel = never();
    let err = move_item_with(
        &src,
        &d.dst,
        &mut |_| cancel.store(true, Ordering::SeqCst),
        &cancel,
        &d.recycler,
        &crosses_devices,
    )
    .unwrap_err();
    assert!(matches!(err, FileOpError::Cancelled));
    assert!(src.exists());
    assert!(names(&d.dst).is_empty());
    assert!(d.recycler.calls().is_empty());
}

#[test]
fn another_rename_error_is_an_io_error_and_nothing_is_copied() {
    let d = dirs();
    let src = d.src.join("A.zip");
    zip_mod(&src, 100);
    let err = move_item_with(&src, &d.dst, &mut |_| {}, &never(), &d.recycler, &|_, _| {
        Err(io::ErrorKind::PermissionDenied.into())
    })
    .unwrap_err();
    assert!(
        matches!(err, FileOpError::Io(ref k) if k == "PermissionDenied"),
        "{err}"
    );
    assert!(src.exists());
    assert!(names(&d.dst).is_empty());
}

#[test]
fn errors_carry_kinds_and_never_file_contents() {
    let d = dirs();
    let src = d.src.join("Secret.zip");
    fs::write(&src, b"hunter2-secret-content").unwrap();
    fs::write(d.dst.join("Secret.zip"), b"other-secret").unwrap();
    let err = copy_staged(&src, &d.dst, &mut |_| {}, &never()).unwrap_err();
    let text = format!("{err} {err:?}");
    assert!(!text.contains("hunter2") && !text.contains("other-secret"));
}

// ---------------------------------------------------------------- install_download (plan M7)

/// The bytes of a mod zip whose `modDesc.xml` has this version.
fn mod_zip_bytes(version: &str, extra: usize) -> Vec<u8> {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("m.zip");
    let desc = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><modDesc descVersion=\"105\">\
         <title><en>T</en></title><version>{version}</version></modDesc>"
    );
    let padding: Vec<u8> = (0..extra).map(|i| (i % 251) as u8).collect();
    write_zip(
        &path,
        &[("modDesc.xml", desc.as_bytes()), ("data.bin", &padding)],
        Method::Stored,
    );
    fs::read(path).unwrap()
}

fn sha_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn check_of(min_version: &str) -> DownloadCheck {
    DownloadCheck {
        sha256: None,
        min_version: min_version.to_string(),
        approx_size: None,
    }
}

fn filler(bytes: Vec<u8>) -> impl FnMut(&mut File) -> Result<(), FileOpError> {
    move |file| file.write_all(&bytes).map_err(|e| io_err(&e))
}

fn nothing() -> impl FnMut() -> Result<(), FileOpError> {
    || Ok(())
}

/// Installs `bytes` as `FS25_X.zip` in the dst folder with a recycle of the old file.
fn install(d: &Dirs, bytes: Vec<u8>, check: &DownloadCheck) -> Result<PathBuf, FileOpError> {
    let old = d.dst.join("FS25_X.zip");
    install_download(&d.dst, "FS25_X.zip", &mut filler(bytes), check, &mut || {
        d.recycler.recycle(&old).map_err(FileOpError::Recycle)
    })
}

fn write_old(d: &Dirs) -> Vec<u8> {
    let old = mod_zip_bytes("1.0.0.0", 10);
    fs::write(d.dst.join("FS25_X.zip"), &old).unwrap();
    old
}

#[test]
fn a_good_download_replaces_the_installed_mod_and_recycles_the_old_one() {
    let d = dirs();
    let old = write_old(&d);
    let new = mod_zip_bytes("1.1.0.0", 5000);
    let check = DownloadCheck {
        sha256: Some(sha_hex(&new).to_uppercase()),
        min_version: "v1.1.0.0".to_string(),
        approx_size: Some(new.len() as u64),
    };
    let path = install(&d, new.clone(), &check).unwrap();
    assert_eq!(path, d.dst.join("FS25_X.zip"));
    assert_eq!(fs::read(&path).unwrap(), new);
    assert_eq!(names(&d.dst), ["FS25_X.zip"], "no staged file is left");
    assert_eq!(d.recycler.binned(), ["FS25_X.zip"]);
    assert_eq!(fs::read(&d.recycler.binned_files()[0]).unwrap(), old);
}

#[test]
fn a_download_with_nothing_to_replace_just_installs() {
    let d = dirs();
    let new = mod_zip_bytes("2.0", 10);
    let path = install_download(
        &d.dst,
        "FS25_X.zip",
        &mut filler(new.clone()),
        &check_of("2.0.0.0"),
        &mut nothing(),
    )
    .unwrap();
    assert_eq!(fs::read(path).unwrap(), new);
    assert!(d.recycler.calls().is_empty());
}

#[test]
fn the_download_is_staged_under_a_hidden_name_and_verified_before_the_old_mod_goes() {
    let d = dirs();
    write_old(&d);
    let order = std::cell::RefCell::new(Vec::new());
    let new = mod_zip_bytes("1.1", 10);
    install_download(
        &d.dst,
        "FS25_X.zip",
        &mut |file: &mut File| {
            order.borrow_mut().push(format!("fill {:?}", names(&d.dst)));
            file.write_all(&new).map_err(|e| io_err(&e))
        },
        &check_of("1.1"),
        &mut || {
            // The staged file is complete and still hidden, the old mod is still in place.
            order
                .borrow_mut()
                .push(format!("rename {:?}", names(&d.dst)));
            d.recycler
                .recycle(&d.dst.join("FS25_X.zip"))
                .map_err(FileOpError::Recycle)
        },
    )
    .unwrap();
    let staged = r#"[".FS25_X.zip.fsmm-partial", "FS25_X.zip"]"#;
    assert_eq!(
        order.into_inner(),
        [format!("fill {staged}"), format!("rename {staged}")]
    );
}

fn assert_untouched(d: &Dirs, old: &[u8]) {
    assert_eq!(
        names(&d.dst),
        ["FS25_X.zip"],
        "only the old mod, no staged file"
    );
    assert_eq!(fs::read(d.dst.join("FS25_X.zip")).unwrap(), old);
    assert!(
        d.recycler.calls().is_empty(),
        "the old mod was never touched"
    );
}

fn verify_detail(result: Result<PathBuf, FileOpError>) -> String {
    match result {
        Err(FileOpError::Verify(detail)) => detail,
        other => panic!("expected a verify failure, got {other:?}"),
    }
}

#[test]
fn a_wrong_sha256_fails_and_leaves_the_old_mod_alone() {
    let d = dirs();
    let old = write_old(&d);
    let new = mod_zip_bytes("1.1", 10);
    let check = DownloadCheck {
        sha256: Some(sha_hex(b"something else")),
        ..check_of("1.1")
    };
    assert_eq!(verify_detail(install(&d, new, &check)), "sha256");
    assert_untouched(&d, &old);
}

#[test]
fn a_file_that_is_not_a_zip_fails() {
    let d = dirs();
    let old = write_old(&d);
    let detail = verify_detail(install(
        &d,
        b"<html>not a zip</html>".to_vec(),
        &check_of("1.1"),
    ));
    assert_eq!(detail, "notAZip");
    assert_untouched(&d, &old);
    // An empty download is not a zip either.
    assert_eq!(
        verify_detail(install(&d, Vec::new(), &check_of("1.1"))),
        "notAZip"
    );
    assert_untouched(&d, &old);
}

#[test]
fn a_zip_without_a_readable_mod_desc_fails() {
    let d = dirs();
    let old = write_old(&d);
    let tmp = tempfile::tempdir().unwrap();
    let none = tmp.path().join("none.zip");
    write_zip(&none, &[("readme.txt", b"hi".as_slice())], Method::Stored);
    let bad = tmp.path().join("bad.zip");
    write_zip(
        &bad,
        &[("modDesc.xml", b"<modDesc><unclosed>".as_slice())],
        Method::Stored,
    );
    for file in [none, bad] {
        let detail = verify_detail(install(&d, fs::read(file).unwrap(), &check_of("1.1")));
        assert_eq!(detail, "noModDesc");
        assert_untouched(&d, &old);
    }
}

#[test]
fn a_mod_desc_older_than_promised_fails_and_an_equal_or_newer_one_passes() {
    let d = dirs();
    let old = write_old(&d);
    for version in ["1.0.9.9", "0.9", "", "garbage"] {
        let detail = verify_detail(install(
            &d,
            mod_zip_bytes(version, 10),
            &check_of("1.1.0.0"),
        ));
        assert_eq!(detail, "version", "{version:?}");
        assert_untouched(&d, &old);
    }
    for (version, wanted) in [("1.1", "v1.1.0.0"), ("1.1.0.1", "1.1"), ("2.0", "1.1.0.0")] {
        let d = dirs();
        write_old(&d);
        install(&d, mod_zip_bytes(version, 10), &check_of(wanted)).unwrap();
    }
}

#[test]
fn an_unparseable_promised_version_only_matches_exactly() {
    let d = dirs();
    let old = write_old(&d);
    assert_eq!(
        verify_detail(install(&d, mod_zip_bytes("1.0", 10), &check_of("nightly"))),
        "version"
    );
    assert_untouched(&d, &old);
    install(&d, mod_zip_bytes("nightly", 10), &check_of("nightly")).unwrap();
}

#[test]
fn the_size_must_be_within_five_percent_of_the_page() {
    let new = mod_zip_bytes("1.1", 20_000);
    let size = new.len() as u64;
    let with = |approx: u64| DownloadCheck {
        approx_size: Some(approx),
        ..check_of("1.1")
    };
    for approx in [size, size * 104 / 100, size * 96 / 100, size * 1000 / 1049] {
        let d = dirs();
        write_old(&d);
        install(&d, new.clone(), &with(approx)).unwrap_or_else(|e| panic!("{approx}: {e}"));
    }
    for approx in [size * 2, size / 2, size * 106 / 100, size * 94 / 100, 1] {
        let d = dirs();
        let old = write_old(&d);
        assert_eq!(
            verify_detail(install(&d, new.clone(), &with(approx))),
            "size",
            "{approx}"
        );
        assert_untouched(&d, &old);
    }
    // A page that gave no size (or 0) is not checked.
    let d = dirs();
    write_old(&d);
    install(&d, new.clone(), &with(0)).unwrap();
}

#[test]
fn a_failing_fill_leaves_nothing_behind() {
    let d = dirs();
    let old = write_old(&d);
    let errors: [fn() -> FileOpError; 2] = [
        || FileOpError::Cancelled,
        || FileOpError::Io("BrokenPipe".to_string()),
    ];
    for make in errors {
        let result = install_download(
            &d.dst,
            "FS25_X.zip",
            &mut |file: &mut File| {
                file.write_all(b"partial").map_err(|e| io_err(&e))?;
                Err(make())
            },
            &check_of("1.1"),
            &mut nothing(),
        );
        assert!(result.is_err());
        assert_untouched(&d, &old);
    }
}

#[test]
fn a_refused_recycle_keeps_the_old_mod_and_removes_the_download() {
    let d = dirs();
    let old = write_old(&d);
    d.recycler.refuse(&d.dst.join("FS25_X.zip"));
    let err = install(&d, mod_zip_bytes("1.1", 10), &check_of("1.1")).unwrap_err();
    assert!(
        matches!(err, FileOpError::Recycle(RecycleError::Unavailable)),
        "{err}"
    );
    assert_eq!(names(&d.dst), ["FS25_X.zip"]);
    assert_eq!(fs::read(d.dst.join("FS25_X.zip")).unwrap(), old);
}

#[test]
fn an_old_mod_that_is_still_there_after_before_rename_is_never_overwritten() {
    let d = dirs();
    let old = write_old(&d);
    let err = install_download(
        &d.dst,
        "FS25_X.zip",
        &mut filler(mod_zip_bytes("1.1", 10)),
        &check_of("1.1"),
        &mut nothing(),
    )
    .unwrap_err();
    assert!(matches!(err, FileOpError::Exists));
    assert_untouched(&d, &old);
}

#[test]
fn a_leftover_staged_file_is_replaced_not_reused() {
    let d = dirs();
    fs::write(
        d.dst.join(staging_name("FS25_X.zip")),
        b"leftover from a crash",
    )
    .unwrap();
    let new = mod_zip_bytes("1.1", 10);
    let path = install_download(
        &d.dst,
        "FS25_X.zip",
        &mut filler(new.clone()),
        &check_of("1.1"),
        &mut nothing(),
    )
    .unwrap();
    assert_eq!(fs::read(path).unwrap(), new);
    assert_eq!(names(&d.dst), ["FS25_X.zip"]);
}

#[test]
fn only_a_bare_file_name_is_installed() {
    let d = dirs();
    for name in ["", "a/b.zip", "..\\x.zip", "..", ".", "C:\\x.zip", "sub/"] {
        let result = install_download(
            &d.dst,
            name,
            &mut filler(mod_zip_bytes("1.1", 10)),
            &check_of("1.1"),
            &mut nothing(),
        );
        assert!(matches!(result, Err(FileOpError::Io(_))), "{name:?}");
    }
    assert!(names(&d.dst).is_empty());
    assert!(names(d.dst.parent().unwrap())
        .iter()
        .all(|n| n != "x.zip" && n != "b.zip"));
}

#[test]
fn verify_failures_carry_the_check_name_and_never_the_content() {
    let d = dirs();
    write_old(&d);
    let err = install(&d, b"hunter2-secret-content".to_vec(), &check_of("1.1")).unwrap_err();
    let text = format!("{err} {err:?}");
    assert!(text.contains("notAZip") && !text.contains("hunter2"));
}
