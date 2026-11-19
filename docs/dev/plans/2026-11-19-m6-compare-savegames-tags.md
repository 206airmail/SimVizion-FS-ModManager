# Milestone 6: Compare mode, savegame comparison, tags and notes

Date: 2026-11-19. Approving this plan authorises **Milestone 6 only**. M1–M5 are merged (PRs #1–#5); earlier plans are in `docs/dev/plans/`. This plan is copied to `docs/dev/plans/2026-11-19-m6-compare-savegames-tags.md` in Stage 0.

## Context

The app manages folders and files safely (M3–M5). The remaining P1 features from the project plan are understanding features:
- Which folders differ (Compare mode, the "workbench" deferred in M1).
- Will this savegame load with this folder (savegame comparison).
- The user's own labels (tags and notes).

The Savegames nav item is a "not built yet" placeholder today. No savegame, compare, tag or note code exists.

### User choices (2026-11-19)

- **Savegames:** a report, plus copying the missing mods in from the user's other folders through the M5 copy dialog.
- **Compare:** a diff of two folders, plus copying or moving the selected rows across with the M5 dialog.
- **Tags and notes:** per mod name, shared across all folders.
- **Hashing:** the user rejected "a few minutes". Measured on this PC, MD5 runs at 566 MB/s on one core, and the disk is an NVMe 970 EVO Plus. So:
  - Compare uses an instant fingerprint from each zip's central directory.
  - The savegame check hashes automatically, in parallel and cached, and only the zips that need it.

### Facts

Researched in the decompiled 1.25.1.0 scripts and checked against the real files.

- **The savegame mod list.** `savegameN/careerSavegame.xml` (root `revision="2" valid="true"`) has one `<mod modName title version required fileHash/>` per mod, directly under the root.
  - The list is every mod that was active in the session, not only the ones in use. The user's savegame1 has 477.
  - `required` is true only for the map mod. When reading, a missing attribute defaults to true.
  - Other fields: `settings/savegameName`, `mapId` (`<modName>.<id>` for a mod map), `mapTitle`, `saveDate` (YYYY-MM-DD), `statistics/money`, `statistics/playTime`.
  - There are 20 slots. A slot folder can exist with no save (only `steam_autocloud.vdf`). Backups live in `savegameBackup/` and are out of scope.
- **How the game matches mods.** It matches by modName only, which is the zip file name without `.zip`, and never checks version or hash for a single-player load.
  - DLCs are `pdlc_<name>`, from `<install>/pdlc/<name>.dlc`.
  - Built-in content such as `FS25_precisionFarming` is listed but has no zip.
- **`fileHash`** = `md5(zip bytes ++ modName as UTF-8)`; it was verified on 17 real zips. Folder mods have `fileHash=""`.

## Outcome (acceptance criteria)

1. **Compare mode.**
   - A Compare screen with two folder pickers, reachable from Tools, from the profile menu ("Compare with…") and from the library title row.
   - Each row is one mod name, with the left and right version, size and date, and a state: **only left, only right, different version, same version but different content, identical**.
   - Filters: Differences (the default), Only left, Only right, Different, All. Plus search.
   - Select rows, then **Copy → / ← or Move → / ←** through the existing transfer dialog, so clashes ask Replace or Skip. The comparison refreshes afterwards.
2. **Savegames screen.**
   - A list of the saves that exist, with name, map, save date, play time, money and mod count.
   - Selecting a save and a folder (the active one by default) shows each of the save's mods as:
     - **Present**;
     - **Different version** (the save's version and the folder's);
     - **Different file** (same version, the hash differs from the save's `fileHash`);
     - **Missing** (not in this folder, with the folders that do have it);
     - **DLC** (installed or not);
     - **Game content** (built-in, never "missing").
   - The map mod is marked required, and a missing required mod is called out first.
   - The folder's **extra mods** (loaded but not used by this save) appear in a collapsible list.
3. **Copy missing.** For missing mods that exist in other folders, one button per source folder ("Copy 12 from Main Farm") opens the M5 transfer dialog with them preselected. Mods found nowhere are listed as "Not in any of your folders".
4. **Hashing is fast and never repeated needlessly.**
   - Hashes run in parallel across cores and are streamed. They are cached in `cache.sqlite` by path, size and mtime, so an unchanged zip is never hashed twice.
   - Progress and Cancel use the M5 banner pattern. The version check shows at once, and hash results fill in.
   - Only zips whose version matches the save, and whose save entry has a non-empty `fileHash`, are hashed.
   - Target: the whole 23.5 GB `mods` folder cold in about 15 s on this PC. Measured and reported.
5. **Tags and notes.**
   - Per mod name, case-insensitive, stored in `library.sqlite`, shown wherever the mod is.
   - In the Inspector: tag chips with add and remove (suggesting existing tags) and a note field that saves on blur.
   - Multi-selection: add a tag to all, or remove one from all.
   - Library: a Tag filter. Tags and notes survive rescans, copies, moves and updates.
6. **Read-only towards the game.** Savegame files are only read. No savegame write path exists.
7. **Mock and design.** The mock supports everything: several saves, missing, DLC, game content, different-file, slow hashing and cancel. The new screens follow simvizion-ui: dense tables, semantic tones for states, and lime only for primary actions.

## Decisions

| # | Decision | Rejected |
|---|---|---|
| 1 | **The Compare content check is a zip fingerprint:** a hash of the central directory's (name, CRC-32, size) list, computed during the scan, where the archive is already open. It is cached with the scan record, and a `SCAN_VERSION` bump re-reads records. Folder mods use their file list and sizes. | A full-file hash for compare (slow); size and mtime only (misses same-size changes, flags plain copies) |
| 2 | **The savegame check uses the game's own `md5(zip ++ modName)`**, via the `md-5` crate (new dependency; the reason is to match the game's `fileHash`). It runs in parallel with rayon, streams in 4 MiB chunks, can be cancelled, and is cached in a new `cache.sqlite` table keyed by path_key, size, mtime and salt. | A faster non-MD5 hash (cannot match the save); hashing every mod |
| 3 | **Savegames are discovered under `ctx.user_dir`** (`savegame1..N`, where a valid `careerSavegame.xml` exists). The file is parsed with roxmltree; nothing else in the save is read. | Reading `farms.xml` or other files (passwords; not needed); backups (later) |
| 4 | **DLC and built-in classification.** `pdlc_*` means DLC, and it counts as installed when `<install>/pdlc/<name>.dlc` exists. Built-in content is recognised by a Stage 0 spike: the game install's internal mod location, found in the decompiled scripts. Until confirmed, a non-pdlc name with no zip anywhere whose save `fileHash` matches nothing counts as "Not in your folders". | Hard-coding a name list without a source |
| 5 | **Tags and notes are user metadata keyed by `lower(mod_name)`** in a `mod_meta` table (migration 4). `ModEntry` gains `tags` and `note`. The source links planned in M3 will use the same table later. | Per-file metadata; storing in `cache.sqlite` (disposable) |
| 6 | **Copy actions reuse the M5 plan → execute flow and dialog**, preset with from, to and mod ids. No new file-operation code. | A separate copy path for Compare or Savegames |
| 7 | **Compare is one screen with two pickers and a single merged table** (one row per mod name), not two panes. It is denser, sorts and filters as one list, and works at 960px. | Two side-by-side lists (poor at 960px; rows misalign) |

## Contract (Stage 0: `model.rs` → `gen:types`)

- **`ModEntry`** gains `tags: Vec<String>` and `note: Option<String>`.
- **Compare:**
  - `CompareState { OnlyLeft, OnlyRight, VersionDiffers, ContentDiffers, Identical }`
  - `CompareSide { mod_id, summary: ModSummary }`
  - `CompareRow { mod_name, title, left: Option<CompareSide>, right: Option<CompareSide>, state }`
  - `Comparison { left_profile_id, right_profile_id, rows }`
- **Savegames:**
  - `Savegame { id ("savegame1"), slot, name, map_title, map_mod: Option<String>, save_date, play_minutes, money, mod_count }`
  - `SaveModState { Present, VersionDiffers, FileDiffers, HashPending, Missing, DlcInstalled, DlcMissing, GameContent }`
  - `SaveModRow { mod_name, title, saved_version, required, state, installed: Option<CompareSide>, found_in: Vec<String> }`
  - `SavegameReport { savegame_id, profile_id, rows, extras: Vec<String> /* mod ids */ }`
  - `HashProgress { done_files, total_files, done_bytes, total_bytes }`
- **ErrorCode** gains `savegameMissing` and `savegameUnreadable`.
- **Api** gains:
  - `compareProfiles(leftId, rightId)`
  - `listSavegames()`
  - `savegameReport(savegameId, profileId)`: returns at once. Rows whose hash is cached are final; the others are `HashPending`.
  - `checkSavegameFiles(savegameId, profileId, onProgress)`: hashes the pending zips and returns the final report. The UI calls it automatically after the first report.
  - `cancelHashing()`, `setTags(modName, tags)`, `setNote(modName, note)`, `listTags()`

## Architecture

- **`fsmm-core::savegame`** (new):
  - `list(user_dir)` and `read(user_dir, id)`, which parse the settings, statistics and mod entries.
  - Tests use generated `careerSavegame.xml` fixtures: valid, `valid="false"`, a wrong revision, a missing `required`, a DLC, a mod map, a garbled file.
- **`fsmm-core::files::hash`** (or `scan::hash`):
  - `game_md5(path, mod_name, progress, cancel)`.
  - `hash_many(items)`, run in parallel with rayon, with aggregated progress.
  - Cache access through `store`.
- **`fsmm-core::scan`:** the zip fingerprint (decision 1), added to the internal scan record (not to `ModEntry`). Bump `SCAN_VERSION`.
- **`fsmm-core::library`:**
  - `compare_profiles`, built on the per-profile `Scan` cache. `Scan.names` becomes a name → index map.
  - `savegame_report` and `check_savegame_files`.
  - `set_tags`, `set_note` and `list_tags`; `list_mods` fills `tags` and `note`.
  - Hashing shares the busy and cancel infrastructure pattern of M5's `TransferState`, with its own flags so a hash can run alongside reads.
- **`fsmm-core::store`:**
  - `library.sqlite` migration 4: `mod_meta(name_key PK, tags_json, note, updated_at)`.
  - `cache.sqlite` migration: `file_hashes(path_key, size, mtime, salt, md5, PK(path_key, salt))`.
- **`src-tauri`:** the commands. `checkSavegameFiles` uses a `Channel<HashProgress>`, and `cancel_hashing` is synchronous, as `cancel_operation` is.
- **Frontend:**
  - **Section types:** `Section` gains `compare`; `savegames` replaces its placeholder.
  - **`src/features/compare/`:** the pickers, a swap button, filter segments, search, the merged table with state pills, multi-select, and Copy/Move in both directions (opens `TransferDialog` with presets).
  - **`src/features/savegames/`:**
    - the save list as cards or rows;
    - the report table, grouped with the required mod first, then by state, with counts as filter segments;
    - an extras disclosure;
    - "Copy N from <folder>" actions;
    - the hash progress banner with Cancel.
  - **Tags and notes:**
    - In the Inspector: a tag editor (chips, an input with suggestions, Enter to add, × to remove) and a note textarea that saves on blur.
    - Multi-select: add or remove a tag.
    - `query.ts`: a tag filter; the library toolbar gets a Tag select.
  - **Entry points:** Tools → Compare; the profile menu "Compare with…"; Savegames.
  - **`TransferDialog`:** accepts a preset (from, to, mode, mod ids) and skips the target step when the target is given.
  - **Mock:** saves built from the mock profiles' mods, with some missing, some versions changed, two DLCs and a built-in. Simulated hashing with progress. `?mock=slowHash`.

## Work plan (new branch `m6-compare-savegames-tags`)

**Stage 0: coordinator (serial)**

1. Create the branch, copy this plan, and push.
2. Built-in content spike, about 30 minutes in the decompiled scripts and the install folder, read-only. Find where internal mods such as `FS25_precisionFarming` come from and how to recognise them. Record it in decisions.
3. Write the contract and run `gen:types`. Add the Api, the mock stubs, the core skeletons (`savegame`, `hash`, the library methods, the store migrations as `todo!()`) and the Tauri commands. Name the `md-5` dependency and its reason.
4. Run the checks, commit and push.

**Stage 1: two parallel implementers (worktrees)**

| Workstream | Owns | Acceptance |
|---|---|---|
| **WS-A core** | `crates/fsmm-core/src/**` | <ul><li>Savegame parsing on fixtures: all the cases in Architecture.</li><li>`game_md5` equals a reference computed in the test.</li><li>The hash cache hits on an unchanged file and misses after a change.</li><li>Parallel progress is monotonic; cancel stops it.</li><li>Fingerprint: equal for two copies, different after one entry changes, and stable across a rescan.</li><li>`compare_profiles`: every state, case-insensitive names, folder mods.</li><li>Savegame report: every state, `found_in`, extras, the required map first, DLC installed or missing via a fake install dir, built-in content.</li><li>Tags and notes: set, clear, case-insensitive, survive a rescan, appear on `list_mods`.</li><li>The two migrations, from any older version.</li></ul> |
| **WS-C frontend** | `src/**` | <ul><li>The Compare and Savegames screens; tags and notes UI; tag filter; the `TransferDialog` preset; entry points.</li><li>Every state in the mock.</li><li>Unit tests: compare filters and counts, report grouping, the tag editor logic, the tag filter, preset transfers.</li><li>Orca browser review at 1280×720 and 960×640.</li></ul> |

**Stage 2: coordinator.** Merge both, review the whole diff, run the real-app check, update docs, and open the PR.

## Verification

- **Checks:** `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `npm run check`. `gen:types` produces no diff.
- **Real app, read-only against game files:**
  - The Savegames screen lists savegame1 with its real counts (477 mods, 1 required).
  - Its report against `mods` and against `$testingMods` is spot-checked: a few rows by hand against `careerSavegame.xml`, and the hash states against an independent PowerShell MD5 of zip + modName for 3 mods.
  - Hashing time is measured cold and warm on the real `mods` folder.
  - Compare `mods` with `$testingMods`: the counts per state are checked against a script.
- **Tags and notes:** written only to `library.sqlite`. Add a tag and a note, restart the app, and confirm they persist. Rescan and they survive.
- **Copy missing:** exercised with generated scratch folders only, never real mods, as in M5.
- **Snapshots:** the real mod folders, `gameSettings.xml` and `savegame1/careerSavegame.xml` hash are identical before and after.
- **Privacy:** no savegame contents in logs or commits; fixtures are generated.

## Out of scope

- Savegame backups.
- Editing or repairing savegames.
- Downloading missing mods (M7).
- Version comparison of DLC content.
- Launching the game with a save.
- Bulk tag management screen (rename or delete a tag everywhere).
