# Status

Updated: 2026-11-19, branch `m5-file-operations`

## Active work

Milestone 5 (file operations) is built and waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-19-m5-file-operations.md`. M1–M4 are merged to `main` (PRs #1–#4).

- **`fsmm-core::files`:**
  - A staged copy (`.<name>.fsmm-partial`, 4 MiB chunks, progress, cancel, mtime kept, synced, verified, then renamed).
  - A move: a rename, with copy-then-recycle across drives.
  - `WindowsRecycler`, which refuses rather than deletes permanently.
  - `WindowsDiskSpace`.
- **`fsmm-core::library` transfer:**
  - Plan, then execute. Clashes are found case-insensitively, and the user chooses Replace or Skip for each.
  - Every item is guarded. A Replace recycles the old mod only after the new copy verified.
  - Remove to the Recycle Bin.
  - Busy and cancel flags.
  - A `file_ops` journal.
  - Profile rename and remove-from-list, which is refused for the active and default folders.
- **`src-tauri`:** six commands, with progress over a `Channel`.
- **Frontend:**
  - `Dialog` and `Menu` primitives, shown in the gallery.
  - A two-step transfer dialog with a free-space summary and a clash table (Replace/Skip, apply to all).
  - An app-level progress banner with Cancel, and result banners.
  - A remove confirmation.
  - A rail menu per profile (Rename, Remove from list).
  - Copy, move and remove for one mod or a multi-selection.
  - Mock switches `?mock=clashes|recycleFails|lowSpace`.

## Next step

The user reviews the M5 pull request. After the merge, plan Milestone 6: Compare mode, savegame comparison, tags and notes.

## Last verification

2026-11-19, M5 integration:

- **Automated checks:** all passed.
  - `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
  - `cargo test --workspace`: 358 passed, 3 ignored.
  - `npm run check`: 200 tests passed.
  - `npm run gen:types` produces no diff.
- **Real app** (`npm run tauri dev`, two generated scratch folders beside the real ones, never real mods):
  - **Copy with a clash, Replace:** both versions were shown. The new 2 MB v2 is in place with its mtime kept. The old v1 went to the Recycle Bin, listed with its original folder.
  - **Skip:** it is the default, and with nothing to do the primary action is disabled.
  - **Move on one drive:** an instant rename with no space needed; nothing was recycled.
  - **Remove:** the confirmation states count, size and the sync note. The mod went to the Recycle Bin, and the list and counts refreshed.
  - **Cancelling a 1.5 GB copy:** "Nothing was copied", and no staged file was left.
  - **The same copy run to completion:** the progress banner was seen at 97%, the byte count was exact, the mtime was kept, and "Also in" refreshed.
  - **Rename:** only the list name changed; the folder kept its name.
  - **Remove from list:** disabled for the active profile. On a scratch profile it removed the entry and left the folder and files untouched.
  - **Real mod folders:** the name, size and mtime snapshot of all three real folders (2,747 entries) and the `gameSettings.xml` hash were identical before and after.
  - **Cleanup:** the scratch folders were recycled afterwards.
- **Recycler:** a local file was recycled. A file on `\\localhost\C$` (no Recycle Bin) was refused and kept.

## Known issues

- **The `fsmm-test-a` scratch profile is still listed, with its folder now in the Recycle Bin.** The rail menu closes on window blur, and background UI automation blurs the window, so the scripted "Remove from list" could not be completed for it. The user can remove it from the list by hand.
- **Not run in the real window:**
  - multi-select actions (they need Ctrl+click);
  - a cross-drive move;
  - the recycle-refused banner.

  Each is covered by tests, and the first and last also by the browser review.

- **Syncthing:** the remove confirmation shows a general sync note instead of detecting `.stfolder`.
- **Profile names:** an invalid name is reported as `internal` with the detail `invalidName`; there is no dedicated error code.
- **Collapsed rail:** the profile menu opens only by right-click or Shift+F10.
- **Dialog backdrop:** no `--sv-backdrop-dialog` token exists; the backdrop uses `--sv-window` at 60%.
- **From earlier milestones:**
  - The undo-failure banners have not been triggered in a real window.
  - GIANTS and Epic detection is fixture-only.
  - The descVersion rule is unconfirmed.
  - The emblem is a placeholder.
