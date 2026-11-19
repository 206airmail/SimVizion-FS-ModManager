# Status

Updated: 2026-11-19, branch `m4-safe-switching`

## Active work

Milestone 4 (safe switching of the active mod folder) is built and waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-18-m4-safe-switching.md`. M1–M3 are merged to `main` (PRs #1–#3).

- `fsmm-core::ops`: patches `modsDirectoryOverride` in place. Every write is guarded (game/server running, concurrent change), backed up (newest 20), atomically replaced, then verified.
- `fsmm-core::library` switching: `set_active`, `undo_switch`, `latest_backup`, `restore_backup`, plus the `ops` journal in `library.sqlite`.
- `src-tauri`: four commands, and the debug-build write guard (`FSMM_ALLOW_GAME_WRITES=1`).
- Frontend:
  - "Set as active" with a busy state; it is disabled, with an explanation, while the game or server runs.
  - A success banner with Undo.
  - Worded errors.
  - An emergency restore offer when `gameSettings.xml` is unreadable.
  - Mock switches `?mock=running|server|brokenSettings|writeFails`.

## Next step

User reviews the M4 pull request. After merge, plan Milestone 5 (profile operations): copy and move between folders, removal to the Recycle Bin, Compare mode, savegame comparison, tags, profile rename and remove-from-list.

## Last verification

2026-11-19, M4 integration branch:

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (272 passed, 1 ignored).
- `npm run check` (131 tests). `npm run gen:types` produces no diff.
- **Dev guard:** in `npm run tauri dev` without the variable, "Set as active" showed "This development build does not write game files, so nothing was changed." The file hash was unchanged and no backup was taken.
- **Real-file round trip** (user-approved, game and server closed, `FSMM_ALLOW_GAME_WRITES=1`):
  1. Set `mods` active. Only line 3 changed, to `active="false"`, with the directory kept and the BOM preserved. The rail, header and status bar followed, and the success banner with Undo appeared.
  2. Undo. The SHA-256 equalled the original.
  3. Set `$offlineTestingMods` active, then `$testingMods`. The SHA-256 equalled the original again.
  4. Four backups and four journal rows (switch, undo, switch, switch), all `ok`.
  5. No temp file left beside `gameSettings.xml`. The mod folders (name, size, mtime) were unchanged.

## Known issues

- The undo-failure banners (`nothingToUndo`, `settingsChanged`) are covered by tests but were not triggered in a real window.
- The 960×600 layout of the new banners was not checked.
- An undo whose previous state had no `directory` restores only `active` (exact for the real file's shape).
- From earlier milestones:
  - GIANTS and Epic detection is fixture-only.
  - The descVersion rule is unconfirmed.
  - Cold-boot timings are unmeasured.
  - The emblem is a placeholder.
  - Later-milestone buttons are inert.
