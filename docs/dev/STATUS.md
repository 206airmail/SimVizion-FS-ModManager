# Status

Updated: 2026-11-18, branch `m2-read-only-core`

## Active work

Milestone 2 (read-only core in `crates/fsmm-core`) is built and waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-18-m2-read-only-core.md`. Milestone 1 merged to `main` as PR #1.

- `container`: read-only zip/folder access, case-insensitive names, size-capped reads.
- `discovery`: Steam (registry + library folders), GIANTS (Uninstall keys) and Epic (manifests) installs; user folder via the Known Folder API; `modsDirectoryOverride` read without exposing any other settings text; game log summary; running game/server; suggested profile folders; `game_status`.
- `scan`: enumerate, `read_mod` (modDesc with lenient `&` retry, language choice, kind, icon resolution), `assess` (all `IssueKind` rules), `scan_folder` in parallel.
- `icons`: DDS (DXT1/3/5, BC7, uncompressed BGR/BGRA) and PNG to 256 px PNG thumbnails.
- Dev tools (read-only): `cargo run -p fsmm-core --release --example discover`, `... --example scan_report -- <folder> [--list] [--thumbs-out <dir>]`.

## Next step

User reviews the M2 pull request. After merge, plan Milestone 3: Tauri commands, SQLite cache, streaming scan, real data in the UI. M3 also has to update the mock and Inspector so issue details are worded in `src/strings.ts` (the core now returns data, not sentences).

## Last verification

2026-11-18, M2 integration branch:

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (156 passed, 1 ignored timing test).
- `npm run check` (78 tests).
- `npm run gen:types` produces no diff.

Real-data smoke on the dev machine (read-only, release build, warm OS cache):

- `discover` finds the Steam install (1.25.1.0) and the redirected user folder, and reports the override as active with its folder present. It reports the log as stale (written by 1.10.0.0), so the descVersion check is off. It sees no game or server running and suggests the three mod folders.
- `scan_report`:
  - **Coverage:** all 590 zips in `mods` parse, including the one needing the `&` retry. Every one of the 612 icons across the three folders becomes a thumbnail.
  - **Issues:** 6 sync-conflict copies and 3 zip/folder twins are flagged. The empty Syncthing `_staged` folder is reported as not a mod.
  - **Timing in `mods`:** enumerate 43 ms, metadata 70–77 ms, thumbnails 1.1 s (591 icons, about 95 KiB of PNG each).
- A visual check of BC7, BGR24, BGRA32, DXT3 and non-square thumbnails looked correct.

M1, 2026-11-18/19 (unchanged since): `npm run tauri dev` opens the window and matches the browser rendering. Production build with 5,000 mods: list scroll step median 2.9 ms, p95 4.0 ms; grid p95 29 ms.

## Known issues

- Cold-cache scan time is unmeasured (all runs were on a warm OS cache).
- GIANTS and Epic detection is tested only against fixtures; neither install exists on the dev machine.
- The game's exact descVersion acceptance rule and whether it swaps a missing `.png` icon for `.dds` are unconfirmed.
- Thumbnails average about 95 KiB as PNG. Revisit the size when M3 adds the on-disk cache.
- From M1:
  - The grid view's p95 is 29 ms with 5,000 mods.
  - The focus ring was checked by code, not seen.
  - The emblem is a placeholder.
  - Buttons for later milestones are inert.
  - Checkbox is not built.
