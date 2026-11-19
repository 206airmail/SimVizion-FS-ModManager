# Milestone 4 — safe switching of the active mod folder

Date: 2026-11-18. Approving this plan authorises **Milestone 4 only**. Earlier plans are in `docs/dev/plans/`. This plan is copied to `docs/dev/plans/2026-11-18-m4-safe-switching.md` in Stage 0.

## Context

The app shows the user's real profiles (M3), but "Set as active" is inert. M4 makes it work. This is the project's first write to a game file: the `modsDirectoryOverride` element of `gameSettings.xml`, a file that also holds plain-text multiplayer passwords and that the game itself rewrites. The milestone is about doing that one write safely:

- never while the game or the dedicated server runs;
- never losing or reformatting anything else in the file;
- always with a backup;
- reversible with one click.

User choices (2026-11-18):
- **Restore scope:** Undo after a switch, plus an emergency "restore the newest backup" offer when `gameSettings.xml` becomes unreadable. Backups are otherwise kept silently (newest 20).
- **Real-file test:** the coordinator may test once on the real file, with the game closed and a backup taken. The test is a round trip `$testingMods` → `mods` → `$testingMods`, checked byte-for-byte against the original.

Facts from the real file:
- 5,370 bytes, UTF-8 with a BOM, LF line endings, Archive attribute only.
- The override is one self-closing line: `    <modsDirectoryOverride active="true" directory="D:\…\$testingMods"/>`.
- Path with backslashes and no trailing separator.
- Last written by the game on 2025-08-31.

## Outcome (acceptance criteria)

1. **Set as active.** On a profile that isn't active, "Set as active" makes it the folder the game loads:
   - A normal folder: `active="true"`, `directory="<folder>"`.
   - The game's default `mods` folder: `active="false"`, with `directory` left as it was.
   - Afterwards the rail, header and status bar update. A success banner names the new active folder and offers **Undo**.
2. **Only the attribute values change.** Every byte outside the changed values stays identical: BOM, line endings, indentation, attribute order, quote style, other settings, passwords. The round trip `A → B → A` gives a byte-identical file.
3. **Never while the game or server runs.** The button is disabled with an explanation when either is running (from the 5 s status poll). The core checks again immediately before writing and refuses with `gameRunning` or `serverRunning`.
4. **Never over someone else's write.** If `gameSettings.xml` changes between the read and the write, nothing is written and the user is asked to try again (`settingsChanged`).
5. **Every write is safe:**
   - It is backed up first, to `%APPDATA%\com.simvizion.modmanager\backups\gameSettings-<UTC timestamp>.xml`, keeping the newest 20.
   - It writes to a temp file in the same folder, flushes, then atomically replaces the original.
   - It re-reads and verifies the result.
   - On any failure the original stays in place (or is restored from the backup if verification fails after the replace), and the temp file is removed.
6. **Undo** switches back to the exact previous override through the same safe path. It refuses (`settingsChanged`) if the file no longer holds what the switch wrote.
7. **Emergency restore.** When `gameSettings.xml` exists but cannot be read and a backup exists, the status area offers "Restore the backup from <date>". Restoring follows the same guards, and the unreadable file is itself backed up first.
8. **Journal.** Every write (switch, undo, restore) is journaled in `library.sqlite` with time, kind, from/to override, backup file name and result.
9. **Dev guard.** Debug builds refuse to write the real file unless `FSMM_ALLOW_GAME_WRITES=1` is set (`writesDisabled`). Tests only ever write to temp dirs.
10. **No leaks.** No log line, error or test output contains `gameSettings.xml` text beyond the override element. Backups stay in the app's own folder and are never committed.

## Decisions

| # | Decision | Rejected |
|---|---|---|
| 1 | **Patch attribute values in place.** Parse with roxmltree (positions are on by default) and replace `range_value()` of `active`/`directory`, with values XML-escaped. If an attribute or the element is missing, write one canonical element: replacing the element, or inserting it as the root's first child with the next sibling's indentation. Refuse if the file is not UTF-8. | Re-serialising the document (reformats the file); regex editing (breaks on escaping and comments) |
| 2 | **The default folder means `active="false"`.** The `directory` value stays, so the round trip is byte-identical. | Pointing `directory` at `mods` with `active="true"` |
| 3 | **Atomic replace with `std::fs::rename`** (MoveFileEx with REPLACE_EXISTING on the same volume) from a temp file next to the original. A read-only file fails cleanly with `writeFailed`; the app never clears attributes. | Writing in place; `ReplaceFileW` (new FFI for no gain here) |
| 4 | **Concurrency check by content.** Just before the rename, re-read the original and compare its bytes with what was read at the start, and re-check running processes. | Trusting mtime alone |
| 5 | **Backups in the app's data folder**, not next to the game file. They hold passwords, so they stay local, are never logged, and are pruned to 20. | Backups beside `gameSettings.xml`; unlimited backups |
| 6 | **Undo by operation id.** A switch returns `{ opId, … }`, and Undo applies that operation's `from` state if the file still holds its `to` state. | A global "undo last" that might undo a change the game made since |

## Contract (Stage 0; `model.rs` → `npm run gen:types`)

- `ErrorCode` gains:
  - `gameRunning`, `serverRunning`;
  - `settingsMissing`, `settingsUnreadable`, `settingsChanged`;
  - `writeFailed`, `writesDisabled`;
  - `nothingToUndo`, `noBackup`.
- `SwitchResult { op_id: String, from_profile_id: Option<String>, to_profile_id: Option<String> }`. A profile id is `None` when the override pointed at an unregistered folder or the default.
- `Api` additions:
  - `setActiveProfile(profileId): Promise<SwitchResult>`;
  - `undoSwitch(opId): Promise<SwitchResult>`;
  - `latestSettingsBackup(): Promise<string | null>` (ISO time);
  - `restoreSettingsBackup(): Promise<void>`.

## Architecture

**`fsmm-core::ops` (new, pure, temp-dir tested):**
- `patch_override(bytes, target: &ModsOverride) -> Result<Vec<u8>, OpError>`: decision 1, in memory.
- `write_settings(path, new_bytes, expected_original, guard) -> Result<(), OpError>`: decisions 3 and 4. `guard` re-checks the running state and has test hooks for injecting a concurrent change or a failure before and after the replace.
- `backup(path, bytes, dir, keep: 20)` and `latest_backup(dir)`.
- `OpError` maps to `ApiError` codes, and error texts never include file contents.

**`fsmm-core::store`:** a `library.sqlite` migration adds `ops(id, at, kind, from_active, from_dir, to_active, to_dir, backup_file, result)`, with insert and lookup methods.

**`fsmm-core::library`:**
- `set_active(profile_id, ctx, running, backups_dir) -> SwitchResult`;
- `undo_switch(op_id, …)`;
- `latest_backup(backups_dir)`;
- `restore_backup(ctx, running, backups_dir)`.

`running: &dyn Fn() -> Running` is injected: tests pass fakes, Tauri passes `discovery::running(&WindowsProbe)`. The settings path is `ctx.user_dir/gameSettings.xml` (missing → `settingsMissing`). The flow is preflight → read → patch → no-op if unchanged → backup → guarded write → verify → journal.

**`src-tauri`:**
- Four commands that run on the blocking pool, like M3's.
- The backups dir is `app_data_dir/backups`.
- `writes_allowed()`: always in release, and in debug only when `FSMM_ALLOW_GAME_WRITES=1`.

**Frontend:**
- "Set as active" is wired, with a busy state, and disabled with a tooltip while the game or server runs.
- The success banner has Undo; undo success and failure are worded.
- After a write, profiles and game status reload.
- The emergency restore offer appears when `!game.settingsReadable` and `latestSettingsBackup()` returns a time.
- New error codes are worded in `strings.errors`.
- The mock implements the new methods (switching moves `isActive` in memory) so the browser can review every state.

## Work plan (branch `m4-safe-switching`)

**Stage 0 — coordinator.**
1. Create the branch and copy the plan.
2. Contract, then `gen:types`.
3. `Api` interface.
4. `ops` skeleton with signatures.
5. The `ops` table migration slot and the skeletons for the new `library` methods.
6. Command names in the `src/api/tauri.ts` header.
7. Checks, commit, push.

**Stage 1 — two parallel implementers (worktrees, `/parallel-work`):**

| Workstream | Owns | Acceptance (tests in temp dirs only) |
|---|---|---|
| **WS-A core** | `crates/fsmm-core/src/ops/**`, `store/**`, `library/**` | <ul><li>**Patch fidelity:** a fixture shaped like the real file (BOM, LF, indentation, a fake password elsewhere) changes only the attribute bytes; CRLF variant; `'` quotes; special characters escaped (`&`, `"`, `<`, `'`); an existing `&amp;` round-trips; missing attribute; missing element; not UTF-8 → refused.</li><li>**Round trip:** `A → B → A` is byte-identical.</li><li>**Guards:** running game or server → refused, with no backup, temp file or write; concurrent change → `settingsChanged` with the file untouched.</li><li>**Failures:** a read-only file gives `writeFailed`, the original is intact and no temp file remains; a simulated verification failure after the replace restores the original.</li><li>**Bookkeeping:** backup retention 20; journal rows.</li><li>**Undo:** works, and is refused after an external change.</li><li>**Restore:** restore of the latest backup (the broken file is backed up first); `noBackup`; `settingsMissing`.</li><li>**Secrets:** no error text contains the fake password.</li></ul> |
| **WS-C frontend** | `src/api/tauri.ts`, `client.ts`, `mock/**`, `src/features/**`, `src/lib/**`, `src/strings.ts` | <ul><li>Set as active is wired with its busy, disabled and tooltip states.</li><li>Success banner with Undo.</li><li>Error wording for every new code.</li><li>Emergency restore offer.</li><li>The mock supports switching and undo, and can simulate the game running and an unreadable settings file (`?mock=running`, `?mock=brokenSettings`).</li><li>Unit tests for the new strings and the mock behaviour.</li><li>Orca browser review of each state at 1280×720.</li></ul> |

**Stage 2 — coordinator.**
1. Merge both. Write the Tauri commands and the dev guard. Review the whole diff.
2. Run the real-file test below.
3. Update docs. Commit, push, open the PR.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; `npm run check`; `gen:types` leaves no diff.
- **Dev guard:** in `npm run tauri dev` without the env var, "Set as active" shows the `writesDisabled` message and the real file's hash and mtime are unchanged.
- **Real-file round trip** (user-approved, once; game and server confirmed closed):
  1. Record the SHA-256 and mtime of `gameSettings.xml` and copy it into the scratchpad as an extra backup. Its contents are never printed.
  2. Run the app with `FSMM_ALLOW_GAME_WRITES=1`. Set `mods` active.
     - The only changed line is the override line, now `active="false"`, checked by a line diff that prints only that line.
     - The app shows `mods` as ACTIVE.
     - A backup exists in the app's folder.
  3. Click Undo. The file's SHA-256 equals the original, and `$testingMods` is ACTIVE again.
  4. Set `$offlineTestingMods` active, then set `$testingMods` active. The hash equals the original again, and the journal has 4 rows.
  5. If anything differs, restore the scratchpad copy and report.
- **Screenshots** of the real window (via `orca computer`): the success banner with Undo, the disabled button tooltip text (from the accessibility tree), and the journal and backup listing.
- **Mod folders:** the before/after name, size and mtime snapshot of the mod folders is unchanged, as in M3.
- Report what was run separately from what was only inspected.

## Out of scope

Launching the game, profile rename and remove, copy, move, recycle and Compare (M5). A backups browser (declined in favour of emergency restore). Editing any other setting. Server configuration files (decision: stay out of the dedicated server's way).
