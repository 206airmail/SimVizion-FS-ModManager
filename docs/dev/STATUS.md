# Status

Updated: 2026-11-19, branch `m6-compare-savegames-tags`

## Active work

Milestone 6 (Compare mode, savegame comparison, tags and notes) is built and waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-19-m6-compare-savegames-tags.md`. M1–M5 are merged to `main` (PRs #1–#5).

- **`fsmm-core::savegame`:** reads `savegameN/careerSavegame.xml` only (revision 2, `valid="true"`, slot ids validated). Nothing in a save is written, and no other save file is opened.
- **`fsmm-core::files::hash`:** the game's `fileHash`, `md5(zip ++ modName)`. Hashes run in parallel and are cached in `cache.sqlite` (`file_hashes`, keyed by path, size, mtime and salt).
- **Content fingerprint:** a hash of each zip's central directory (name, CRC-32, size), or of a folder's file list, stored with the scan record (`SCAN_VERSION` 2). Compare uses it to spot the same version with different contents.
- **`fsmm-core::library`:**
  - `compare_profiles`.
  - `list_savegames`, `savegame_report`, `check_savegame_files` and `cancel_hashing`, with their own busy and cancel flags.
  - `set_tags`, `set_note` and `list_tags`, keyed by lower-cased mod name in `library.sqlite` `mod_meta` (migration 4). `ModEntry` carries `tags` and `note`.
- **`src-tauri`:** eight commands in `insight.rs`. Debug builds print timings for compare, the report and the file check.
- **Frontend:**
  - A Compare screen: two folder pickers, swap, filter segments with counts, a merged table, multi-select, and Copy or Move in either direction.
  - A Savegames screen: the save list, the report with state filters, a missing-required-map banner, "Copy N from <folder>", the "not in any of your folders" list, extras, and the automatic file check with progress and Cancel.
  - Tags and notes in the Inspector (one mod or several), and a tag filter.
  - `TransferDialog` presets.
  - Entry points: Tools, the profile menu, and the library title row.
  - Mock switches `?mock=slowHash|brokenSave|noSaves`.

## Next step

The user reviews the M6 pull request. After the merge, plan Milestone 7 (mod updates; source: SiloAPI, rate-limited, see the user memory note).

## Last verification

2026-11-19, M6 integration:

- **Automated checks:**
  - `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  - `cargo test --workspace`: 467 + 1 passed, 3 ignored.
  - `npm run check`: 262 tests passed.
  - `npm run gen:types`: no diff.
- **Real app, read-only against game files.** Expected numbers came from an independent PowerShell script: it read each zip's `modDesc.xml` and computed `md5(zip + modName)` for every same-version zip.
  - `savegame1` (477 mods, 1 required) against `mods`. The app matched the script exactly: 227 missing, 61 different version, 3 different file (`FS25_TankerUniversalFill`, `RMC25_TrailKing_TK70`, `FS25_ClaasXerion2500`), 182 present plus 4 installed DLCs.
  - Against `$testingMods`: 463 missing, 1 different version, 9 present plus 4 DLCs.
  - The required map mod `Feliz` is in none of the user's folders, and the app says so.
  - **Hash timing:** the cold file check against `mods` took 1.06 s for about 3.7 GB, with 185 zips hashed in parallel and the app's hash cache empty. Windows' file cache was probably warm from the script. With a warm hash cache the whole report takes 58 ms and nothing is hashed.
  - **Compare** `$testingMods` vs `mods`: 7 only left and 586 only right, matching the script's 585 plus one mod whose `modDesc` the script could not read. 4 different (2 by version, 2 same version with a different size) and 2 identical. It took 82 ms.
  - **Tags and notes:** a tag and a note on a mod survived an app restart and a rescan, and the tag showed on both copies of that mod name. Both were removed afterwards.
  - **"Copy 4 from $testingMods":** opened the preset dialog with the plan ("Copy 4 mods to mods", 191 MB). It was cancelled, so nothing was written.
- **Snapshot:** the real mod folders (2,748 files: name, size, mtime), the `gameSettings.xml` hash and the `savegame1/careerSavegame.xml` hash are identical before and after.

## Known issues

- **Savegame report sorting:** the report and Compare sort by mod (file) name, as the contract specifies, but show titles, so the title column looks unsorted.
- **960×640 Savegames layout:** the save list, banners and copy block leave little room for the table; the list is capped at 3.5 rows. The tag filter makes the library toolbar wrap to a third row at 960px.
- **Hash cache:** rows for deleted mods are never pruned. They are tiny.
- **`listSavegames`** cannot tell "no user folder" from "no saves"; both show the same empty state.
- **Not exercised in the real window:** executing a copy from Compare or Savegames (the M5 copy path is unchanged; only the preset dialog is new), cancelling a real hash run, multi-select tagging.
- **From earlier milestones:**
  - The `fsmm-test-a` scratch profile from M5 may still be listed until the user removes it.
  - GIANTS and Epic detection is fixture-only.
  - The descVersion rule is unconfirmed.
  - The emblem is a placeholder.
