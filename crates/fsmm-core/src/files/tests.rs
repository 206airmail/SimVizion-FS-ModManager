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
