//! `game_md5` against references computed in the test with the md-5 crate directly.

use std::fs;
use std::sync::atomic::AtomicBool;

use super::*;

fn reference(bytes: &[u8], name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    hasher.update(name.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn hash_of(bytes: &[u8], name: &str) -> (String, u64) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("FS25_Test.zip");
    fs::write(&path, bytes).unwrap();
    let mut total = 0;
    let hash = game_md5(&path, name, &mut |n| total += n, &AtomicBool::new(false)).unwrap();
    (hash, total)
}

#[test]
fn matches_a_known_md5_vector() {
    // md5("abc") = 900150983cd24fb0d6963f7d28e17f72, split between the file and the salt.
    assert_eq!(hash_of(b"ab", "c").0, "900150983cd24fb0d6963f7d28e17f72");
    // md5("") is the empty file with an empty salt.
    assert_eq!(hash_of(b"", "").0, "d41d8cd98f00b204e9800998ecf8427e");
}

#[test]
fn appends_the_mod_name_as_utf8() {
    let (hash, _) = hash_of(b"zip bytes", "FS25_Ünïcode");
    assert_eq!(hash, reference(b"zip bytes", "FS25_Ünïcode"));
    assert_ne!(hash, reference(b"zip bytes", "FS25_Other"));
}

#[test]
fn a_file_of_several_chunks_matches_the_reference_and_reports_every_byte() {
    // Not a multiple of the chunk size, and more than two chunks.
    let bytes: Vec<u8> = (0..(CHUNK * 2 + 12_345)).map(|i| (i % 251) as u8).collect();
    let (hash, total) = hash_of(&bytes, "FS25_Big");
    assert_eq!(hash, reference(&bytes, "FS25_Big"));
    assert_eq!(total, bytes.len() as u64);
}

#[test]
fn progress_arrives_per_chunk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("FS25_Test.zip");
    fs::write(&path, vec![7u8; CHUNK * 2 + 1]).unwrap();
    let mut steps = Vec::new();
    game_md5(&path, "x", &mut |n| steps.push(n), &AtomicBool::new(false)).unwrap();
    assert!(steps.len() >= 3);
    assert!(steps.iter().all(|&n| n <= CHUNK as u64));
    assert_eq!(steps.iter().sum::<u64>(), CHUNK as u64 * 2 + 1);
}

#[test]
fn cancel_between_chunks_stops_with_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("FS25_Test.zip");
    fs::write(&path, vec![1u8; CHUNK * 3]).unwrap();
    let cancel = AtomicBool::new(false);
    let mut chunks = 0;
    let result = game_md5(
        &path,
        "x",
        &mut |_| {
            chunks += 1;
            cancel.store(true, Ordering::SeqCst);
        },
        &cancel,
    );
    assert!(matches!(result, Err(FileOpError::Cancelled)));
    assert_eq!(chunks, 1);
}

#[test]
fn cancel_before_the_start_reads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("FS25_Test.zip");
    fs::write(&path, b"data").unwrap();
    let mut chunks = 0;
    let result = game_md5(&path, "x", &mut |_| chunks += 1, &AtomicBool::new(true));
    assert!(matches!(result, Err(FileOpError::Cancelled)));
    assert_eq!(chunks, 0);
}

#[test]
fn a_missing_file_is_an_io_error_with_a_kind_not_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret-name.zip");
    match game_md5(&path, "x", &mut |_| {}, &AtomicBool::new(false)) {
        Err(FileOpError::Io(kind)) => {
            assert_eq!(kind, "NotFound");
            assert!(!kind.contains("secret"));
        }
        other => panic!("expected Io, got {other:?}"),
    }
}
