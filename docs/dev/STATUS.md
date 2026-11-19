# Status

Updated: 2026-11-18, branch `m3-real-data`

## Active work

Milestone 3 (real data in the app) is built and waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-18-m3-real-data.md`. M1 and M2 are merged to `main` (PRs #1 and #2).

- `fsmm-core::store`: `library.sqlite` (profile registry) and `cache.sqlite` (scan records, thumbnails), with migrations.
- `fsmm-core::library`:
  - registry seeding;
  - fresh, cache-assisted scans with progress;
  - `also_in`, profile summaries, thumbnails.
- `fsmm-core::discovery::game_context`: folders, language and the current descVersion for the library.
- `src-tauri`: six commands (ids only), the `thumb` protocol, the folder picker, and Explorer open/reveal.
- Frontend:
  - Tauri `Api` client, chosen automatically in the desktop app;
  - read progress;
  - worded error codes and issue details;
  - Add mod folder, Open folder, Show in folder;
  - game status polled every 5 s and on focus;
  - sample-data pill only for the mock.

## Next step

User reviews the M3 pull request. After merge, plan Milestone 4: switching the active folder (writing `modsDirectoryOverride` safely), backups and restore, and the running-game guard.

## Last verification

2026-11-18, M3 integration branch:

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (214 passed, 1 ignored).
- `npm run check` (108 tests). `npm run gen:types` produces no diff.

Real app (`npm run tauri dev`, read-only, screenshots through `orca computer`):

- **Profiles and library:**
  - The three real folders became profiles, with counts 592 / 6 / 15.
  - `$testingMods` is marked ACTIVE, and `mods` is marked as the game default folder.
  - Totals: 613 mods, 26.8 GB, 13 with issues.
  - Real icons appear in list and grid.
  - The inspector on a conflict copy shows "Copy of FS25_AgroForests".
  - Descriptions keep their line breaks (fixed during integration).
- **Explorer and the folder picker:**
  - Open folder and Show in folder open Explorer on the right folder.
  - The Add mod folder picker opens, and cancelling changes nothing.
- **Timings (dev build):**
  - `list_profiles`: 428 ms on the first launch with dependencies unoptimised, 207 ms with an empty cache once dependencies are optimised, 125 ms on a warm cache.
  - `list_mods` for `mods` (592): 108 ms.
  - Thumbnails: 612 ms average on first decode while dependencies were unoptimised, 34 ms after the dev opt-level change, 0.6 ms from the cache.
- **Safety:** the three mod folders (2,747 files; name, size, mtime) and the `gameSettings.xml` mtime were identical before and after the session.

## Known issues

- Not checked in the real window:
  - the 960×600 layout (window resizing needs foreground control);
  - the game-running state (the game was not started);
  - grid smoothness, which was only judged by eye on screenshots while scrolling.
- `list_mods` scans other profiles that have no last scan one after another, so the first listing can scan every profile.
- Cold-cache (first boot) disk timings are unmeasured.
- From earlier milestones:
  - GIANTS and Epic detection is fixture-only.
  - The descVersion acceptance rule is unconfirmed.
  - The emblem is a placeholder.
  - Later-milestone buttons are inert.
  - Checkbox is not built.
