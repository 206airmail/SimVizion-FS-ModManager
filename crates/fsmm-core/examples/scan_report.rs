//! Read-only dev tool: scans a mod folder and prints what `fsmm_core::scan` makes of it.
//!
//! Usage: `scan_report <folder> [--lang xx] [--logged-desc-version N] [--list] [--thumbs]
//! [--thumbs-out <dir>]`
//!
//! The scanned folder is only read. `--list` adds one line per mod that has issues.
//! `--thumbs` also decodes every icon into a thumbnail and reports counts and time;
//! `--thumbs-out <dir>` writes them as `<mod name>.png` into `<dir>`, which must not be
//! inside the scanned folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use fsmm_core::container::{IconKind, ModContainer};
use fsmm_core::icons::{thumbnail, IconError, THUMBNAIL_EDGE};
use fsmm_core::scan::{assess, enumerate, read_mod};
use rayon::prelude::*;

const USAGE: &str = "usage: scan_report <folder> [--lang xx] [--logged-desc-version N] [--list] \
                     [--thumbs] [--thumbs-out <dir>]";

struct Args {
    folder: PathBuf,
    languages: Vec<String>,
    logged_desc_version: Option<u32>,
    list: bool,
    thumbs: bool,
    thumbs_out: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut folder = None;
    let mut languages = Vec::new();
    let mut logged_desc_version = None;
    let mut list = false;
    let mut thumbs = false;
    let mut thumbs_out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lang" => languages.push(args.next().ok_or("--lang needs a value")?),
            "--logged-desc-version" => {
                let value = args.next().ok_or("--logged-desc-version needs a value")?;
                logged_desc_version = Some(
                    value
                        .parse()
                        .map_err(|_| format!("not a number: {value}"))?,
                );
            }
            "--list" => list = true,
            "--thumbs" => thumbs = true,
            "--thumbs-out" => {
                thumbs = true;
                thumbs_out = Some(PathBuf::from(
                    args.next().ok_or("--thumbs-out needs a value")?,
                ));
            }
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other if folder.is_none() => folder = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    Ok(Args {
        folder: folder.ok_or("missing <folder>")?,
        languages,
        logged_desc_version,
        list,
        thumbs,
        thumbs_out,
    })
}

fn print_counts(title: &str, counts: &BTreeMap<String, usize>) {
    println!("{title}");
    if counts.is_empty() {
        println!("  (none)");
    }
    for (key, count) in counts {
        println!("  {key:<24} {count}");
    }
}

fn bump(counts: &mut BTreeMap<String, usize>, key: impl std::fmt::Debug) {
    bump_str(counts, &format!("{key:?}"));
}

fn bump_str(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

/// A short label for a thumbnail failure, so failures can be counted by cause.
fn failure_label(e: &IconError) -> String {
    match e {
        IconError::Read(e) => format!("read: {e}"),
        IconError::Unsupported(d) => format!("unsupported: {d}"),
        IconError::Malformed(d) => format!("malformed: {d}"),
        IconError::Encode(d) => format!("encode: {d}"),
    }
}

/// Creates the thumbnail folder, refusing one inside the scanned folder so the tool can
/// never write into a mod folder.
fn prepare_output(out: &Path, scanned: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let out = out
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", out.display()))?;
    let scanned = scanned
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", scanned.display()))?;
    if out.starts_with(&scanned) {
        return Err("--thumbs-out must not be inside the scanned folder".to_string());
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let languages: Vec<&str> = args.languages.iter().map(String::as_str).collect();

    let t = Instant::now();
    let (entries, skipped) = match enumerate(&args.folder) {
        Ok(found) => found,
        Err(e) => {
            eprintln!("cannot read folder: {e}");
            return ExitCode::FAILURE;
        }
    };
    let enumerate_time = t.elapsed();

    let t = Instant::now();
    let mut mods: Vec<_> = entries
        .par_iter()
        .map(|e| read_mod(e, "report", &languages))
        .collect();
    let read_time = t.elapsed();

    let t = Instant::now();
    assess(&mut mods, args.logged_desc_version);
    let assess_time = t.elapsed();

    let total_bytes: u64 = mods.iter().map(|r| r.entry.size_bytes).sum();
    let with_issues = mods.iter().filter(|r| !r.entry.issues.is_empty()).count();
    println!("folder:  {}", args.folder.display());
    println!("mods:    {}", mods.len());
    println!("size:    {:.1} MiB", total_bytes as f64 / (1024.0 * 1024.0));
    println!("with issues: {with_issues}");
    println!();

    let mut kinds = BTreeMap::new();
    let mut formats = BTreeMap::new();
    let mut icons = BTreeMap::new();
    let mut issue_counts = BTreeMap::new();
    for record in &mods {
        bump(&mut kinds, record.entry.kind);
        bump(&mut formats, record.entry.format);
        match &record.icon {
            Some(icon) => bump_str(
                &mut icons,
                match icon.kind {
                    IconKind::Dds => "icon (dds)",
                    IconKind::Png => "icon (png)",
                },
            ),
            None => bump_str(&mut icons, "no icon"),
        }
        for issue in &record.entry.issues {
            bump(&mut issue_counts, issue.kind);
        }
    }
    let mut skipped_counts = BTreeMap::new();
    for s in &skipped {
        bump(&mut skipped_counts, s.reason);
    }

    print_counts("by kind", &kinds);
    print_counts("by format", &formats);
    print_counts("icons", &icons);
    print_counts("issues by kind", &issue_counts);
    print_counts("skipped entries", &skipped_counts);
    println!();
    println!("enumerate: {enumerate_time:?}");
    println!("read:      {read_time:?}");
    println!("assess:    {assess_time:?}");

    if args.thumbs {
        if let Some(out) = &args.thumbs_out {
            if let Err(message) = prepare_output(out, &args.folder) {
                eprintln!("{message}");
                return ExitCode::FAILURE;
            }
        }
        let t = Instant::now();
        let results: Vec<Result<usize, String>> = mods
            .par_iter()
            .filter_map(|r| r.icon.as_ref().map(|icon| (r, icon)))
            .map(|(record, icon)| {
                let mut container =
                    ModContainer::open(&record.path).map_err(|e| format!("open: {}", e.kind()))?;
                let png = thumbnail(&mut container, icon, THUMBNAIL_EDGE)
                    .map_err(|e| failure_label(&e))?;
                if let Some(out) = &args.thumbs_out {
                    let file = out.join(format!("{}.png", record.entry.mod_name));
                    std::fs::write(file, &png).map_err(|e| format!("write: {}", e.kind()))?;
                }
                Ok(png.len())
            })
            .collect();
        let thumbs_time = t.elapsed();
        let ok: Vec<usize> = results
            .iter()
            .filter_map(|r| r.as_ref().ok().copied())
            .collect();
        let mut failures = BTreeMap::new();
        for e in results.iter().filter_map(|r| r.as_ref().err()) {
            bump_str(&mut failures, e);
        }
        println!();
        println!(
            "thumbnails: {} ok, {} failed",
            ok.len(),
            results.len() - ok.len()
        );
        if !ok.is_empty() {
            let bytes: usize = ok.iter().sum();
            println!(
                "  average size: {:.1} KiB",
                bytes as f64 / ok.len() as f64 / 1024.0
            );
        }
        print_counts("thumbnail failures", &failures);
        println!("thumbnails: {thumbs_time:?}");
    }

    if args.list {
        println!();
        for record in mods.iter().filter(|r| !r.entry.issues.is_empty()) {
            let issues: Vec<String> = record
                .entry
                .issues
                .iter()
                .map(|i| match &i.detail {
                    Some(detail) => format!("{:?}({detail})", i.kind),
                    None => format!("{:?}", i.kind),
                })
                .collect();
            println!("{}: {}", record.entry.file_name, issues.join("; "));
        }
    }
    ExitCode::SUCCESS
}
