//! Read-only dev tool: scans a mod folder and prints what `fsmm_core::scan` makes of it.
//!
//! Usage: `scan_report <folder> [--lang xx] [--logged-desc-version N] [--list]`
//!
//! Nothing is written anywhere. `--list` adds one line per mod that has issues.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use fsmm_core::container::IconKind;
use fsmm_core::scan::{assess, enumerate, read_mod};
use rayon::prelude::*;

const USAGE: &str = "usage: scan_report <folder> [--lang xx] [--logged-desc-version N] [--list]";

struct Args {
    folder: PathBuf,
    languages: Vec<String>,
    logged_desc_version: Option<u32>,
    list: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut folder = None;
    let mut languages = Vec::new();
    let mut logged_desc_version = None;
    let mut list = false;
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
    *counts.entry(format!("{key:?}")).or_default() += 1;
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
            Some(icon) => bump(
                &mut icons,
                match icon.kind {
                    IconKind::Dds => "icon (dds)",
                    IconKind::Png => "icon (png)",
                },
            ),
            None => bump(&mut icons, "no icon"),
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
