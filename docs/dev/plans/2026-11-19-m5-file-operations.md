# Milestone 5 — file operations: copy, move, remove, profile management

Date: 2026-11-19. Approving this plan authorises **Milestone 5 only**. Earlier plans are in `docs/dev/plans/`. This plan is copied to `docs/dev/plans/2026-11-19-m5-file-operations.md` in Stage 0.

## Context

The app lists real profiles (M3) and switches the active one safely (M4). The remaining inert buttons are the everyday chores: "Copy to…", "Remove", and managing the profile list. M5 makes those work. These are the first operations that change the user's mod files, so they follow the M4 pattern: guarded, staged, verified and journaled. Nothing is ever deleted outright.

User choices (2026-11-19):
- **Scope:** file operations first. The roadmap is renumbered:
  - M5: file operations;
  - M6: Compare mode, savegame comparison, tags and notes;
  - M7: mod updates;
  - M8: release.
- **Clashes:** when the target already has a mod with the same name, ask with both versions shown. For each mod the choice is Replace (the old one goes to the Recycle Bin) or Skip, with "apply to all".

Facts:
- **Mod sizes:** up to 3.9 GB per zip.
- **Folders:** all three live on drive `D:` under the user folder. A user-added folder may be on another drive.
- **Syncthing:** the folders are synced, so changes propagate to the user's other devices. That is expected behaviour, not something to stop, and the Remove confirmation mentions it when a `.stfolder` is present.

## Outcome (acceptance criteria)

1. **Copy to / Move to.** For one or many selected mods (list or grid selection, from the inspector), a dialog picks the target profile. Then:
   - Same-volume moves are instant renames.
   - Copies and cross-volume moves stream with progress (bytes and current mod) and can be cancelled.
   - A cross-volume move sends each source to the Recycle Bin only after its copy is verified.
2. **Clash dialog.** Before anything is written, each clash lists both versions (version, size, modified date). Per mod: Replace or Skip, plus "apply to all". Replaced mods go to the Recycle Bin, never deleted.
3. **Remove.** For one or many mods, after a confirmation stating count and total size (and the Syncthing note when relevant), the mods go to the Recycle Bin.
   - **Never a permanent deletion.** If Windows would delete instead of recycling (the file is too big for the bin, or the drive has no bin), that item fails with `recycleUnavailable` and stays where it is. Explorer is the user's way to delete it by hand.
4. **No partial mods visible to the game.** A copy is written under a hidden staging name (`.<name>.fsmm-partial`), verified (size; for a zip, that it opens with the same entry count; for a folder, the same file list and sizes), and only then renamed into place. Cancel or failure removes the staged copy. The original timestamps are kept.
5. **Guards.** Every operation is refused while the game or the dedicated server runs (`gameRunning`/`serverRunning`), checked before starting and before each item. Copies are refused up front when the target drive lacks free space (`notEnoughSpace`). Only one file operation runs at a time (`busy`).
6. **Profile management.** Rename a profile (display name only; the folder is untouched). Remove a profile from the list (the folder is untouched). Removing the active folder or the game's default folder is not offered, because the app would re-add them on the next start.
7. **Results.** A summary banner says done / skipped / failed, with reasons per failed mod. Lists, counts and "also in" refresh. Each item is journaled in `ops` (kind copy, move, recycle; result per item).
8. **Mock and design.** The mock supports all of it, including clashes, progress, cancel and a recycle refusal, for browser review. New `Dialog` and `Menu` primitives follow the simvizion-ui dialog and popup guidance and appear in `#gallery`.

## Decisions

| # | Decision | Rejected |
|---|---|---|
| 1 | **The Recycle Bin through `IFileOperation`** with `FOFX_RECYCLEONDELETE` and a progress sink whose `PreDeleteItem` aborts when `TSF_DELETE_RECYCLE_IF_POSSIBLE` is absent, i.e. when Windows would delete permanently. A Stage 0 spike confirms the API with the `windows` crate (COM `implement`); if it fails, stop and re-plan. | `SHFileOperation` with `FOF_ALLOWUNDO` (silently deletes permanently when an item is too big for the bin); the `trash` crate (does not expose the abort); permanent deletion |
| 2 | **Stage, verify, then rename into place** in the target folder, under a dot-prefixed name the scanner already ignores. | Copying straight to the final name (the game or scanner could see half a mod) |
| 3 | **Chunked copy in Rust** (4 MiB buffers) with a progress callback and a cancel flag; mtime set to the source's afterwards. | `std::fs::copy` (no progress or cancel) |
| 4 | **Moves try `rename` first.** On `CrossesDevices`, fall back to copy, verify, then recycle the source. | Always copying |
| 5 | **Plan, then execute.** `plan_transfer` lists items and clashes without writing anything. `execute_transfer` takes the user's per-clash choices. Everything is re-checked at execution, and anything changed since the plan fails that item. | One call that decides clashes itself |
| 6 | **The recycler sits behind a trait.** Tests use a fake that moves files into a temp "bin" and can be told to refuse, so recycling logic is tested without touching the real bin. The Windows implementation is tested once with a throwaway temp file. | Testing against the real Recycle Bin |
| 7 | **No Undo for file operations in M5.** Recycled items are restored from the Recycle Bin, and the result banner says so. Copies can be removed like any mod. | Journal-driven undo of copies and moves (later, if wanted) |

## Contract (Stage 0; `model.rs` → `gen:types`)

- New types:
  - `TransferMode { Copy, Move }`;
  - `TransferItem { mod_id, mod_name, size_bytes }`;
  - `TransferClash { mod_name, incoming: ModSummary, existing: ModSummary }` (`ModSummary = { file_name, version, size_bytes, modified }`);
  - `TransferPlan { plan_id, from_profile_id, to_profile_id, mode, items, clashes, total_bytes, free_bytes }`;
  - `ClashChoice { mod_name, replace: bool }`;
  - `OpProgress { done_bytes, total_bytes, current: Option<String>, done_items, total_items }`;
  - `OpResult { done: Vec<String>, skipped: Vec<String>, failed: Vec<ItemFailure { mod_name, code: ErrorCode }> }`.
- `ErrorCode` gains `recycleUnavailable`, `notEnoughSpace`, `busy`, `cancelled`, `targetChanged` and `cannotRemoveProfile`.
- `Api` additions:
  - `planTransfer(fromProfileId, modIds, toProfileId, mode)`;
  - `executeTransfer(planId, choices, onProgress)`;
  - `removeMods(profileId, modIds, onProgress)`;
  - `cancelOperation()`;
  - `renameProfile(profileId, name)`;
  - `removeProfile(profileId)`.

## Architecture

- **`fsmm-core::files` (new, temp-dir tested):**
  - `copy_staged(src, dst_dir, progress, cancel)` for files and folders: decisions 2 and 3, including verification and mtime.
  - `move_item` (decision 4).
  - The `Recycler` trait.
  - `free_space(dir)` behind a small trait so tests can fake it. The Windows version uses `GetDiskFreeSpaceExW`; this needs the `windows-sys` `Win32_Storage_FileSystem` feature.
- **`fsmm-core::library::transfer` and `remove`:**
  - Plans and plan ids, kept in memory and expiring after 10 min.
  - Clash detection by mod name (case-insensitive) against the target's fresh scan.
  - Execution with per-item guards (`running` closure, as in M4), journal rows and the result.
  - `rename_profile` and `remove_profile` in `store`. `cannotRemoveProfile` applies to the active and default folders.
  - One operation at a time via an atomic busy flag; a cancel flag is shared with the running operation.
- **`src-tauri`:**
  - Commands, including a `Channel<OpProgress>`.
  - `WindowsRecycler`, behind the `windows` crate (new dependency, `Win32_UI_Shell` and `Win32_System_Com` features). It lives in `src-tauri`, or in a `fsmm-core` Windows module, depending on where the spike lands.
  - File operations are not gated by the dev guard: that guard is for `gameSettings.xml`. Tests never touch real folders, and the coordinator's real-app check uses a test folder (see Verification).
- **Frontend:**
  - New primitives:
    - `src/ui/Dialog.tsx`: modal, focus trap, Escape, labelled; title, body, footer actions.
    - `src/ui/Menu.tsx`: button-anchored menu with keyboard navigation.
    - Both appear in the gallery.
  - Features:
    - Transfer dialog: profile picker, Copy/Move, a summary with free space, the clash list with both versions, and per-item and "apply to all" choices.
    - Progress banner with Cancel.
    - Result banner.
    - Remove confirmation.
  - Profile actions:
    - A rail item menu (Rename, Remove from list).
    - Rename as an inline field or a small dialog.
  - Multi-select actions: the inspector's multi-selection view gets Copy to, Move to and Remove.
  - All text in `strings.ts`; mock support for every state (`?mock=clashes`, `?mock=recycleFails`, slow progress).

## Work plan (branch `m5-file-operations`)

**Stage 0 — coordinator (serial).**
1. Create the branch and copy the plan. Update the roadmap numbering in the decision log.
2. **Spike: the Recycle Bin.** A researcher checks the IFileOperation and sink approach and the `windows` crate version. The coordinator prototypes `WindowsRecycler` and runs it on a throwaway temp file (1 KB, named `fsmm-recycle-test-*.txt`); it ends up in the user's Recycle Bin, as the user will be told. Proving that it refuses permanent deletion needs a volume whose bin is too small or absent. Use a small VHD or a `subst` drive if it can be set up without admin rights; otherwise mark it unverified and rely on the documented flag, which is reported.
3. Contract, `gen:types`, the `Api` interface, and the `files`/`library` skeletons; manifests (the `windows` crate, the `windows-sys` feature); command names in the `tauri.ts` header.
4. Checks, commit, push.

**Stage 1 — two parallel implementers (worktrees):**

| Workstream | Owns | Acceptance (tests in temp dirs, fake recycler and free space) |
|---|---|---|
| **WS-A core** | `crates/fsmm-core/src/files/**`, `library/**`, `store/**` | <ul><li>**Staged copy:** zips and folders keep their mtime; cancel mid-file leaves no staged copy or target; verification failure is cleaned up; the staged name is invisible to `scan::enumerate`.</li><li>**Move:** same volume (rename) and the cross-volume fallback, simulated by a hook.</li><li>**Plans and clashes:** clash detection is case-insensitive; execution honours the choices; a changed source or target gives `targetChanged`; replace recycles the old mod first, and refusal leaves both mods untouched.</li><li>**Remove:** recycle success, and `recycleUnavailable` with the file kept.</li><li>**Guards and results:** guards per item; `notEnoughSpace`; `busy`; progress is monotonic; journal rows; rename and remove-profile rules; `also_in` and counts refresh after an operation.</li></ul> |
| **WS-C frontend** | `src/ui/Dialog.tsx`, `src/ui/Menu.tsx` (+ gallery), `src/api/tauri.ts`, `client.ts`, `mock/**`, `src/features/**`, `src/lib/**`, `src/strings.ts` | <ul><li>**Primitives:** accessible, keyboard complete, styled to the design system's dialog and popup references.</li><li>**Flows:** every flow and state wired; mock scenarios.</li><li>**Tests:** unit tests for strings, plan and clash choices, and mock semantics.</li><li>**Review:** Orca browser review at 1280×720 and about 960 px of the transfer dialog with clashes, progress, results, remove confirmation and the profile menu.</li></ul> |

**Stage 2 — coordinator.**
1. Merge both. Write the commands and `WindowsRecycler` wiring. Review the whole diff.
2. Run the real-app check below.
3. Update docs. Commit, push, PR.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; `npm run check`; `gen:types` leaves no diff.
- **Real app, without touching the user's mods.** Create two scratch profile folders, `…\FarmingSimulator2025\fsmm-test-a` and `fsmm-test-b`. Fill `-a` with a few generated test zips (tiny generated mods, never real ones), add both through "Add mod folder", then:
  - copy, including a clash with Replace and with Skip;
  - move within `D:`;
  - remove to the Recycle Bin, confirming the files are in the bin via Shell.Application;
  - cancel a large copy (a generated 1–2 GB test zip);
  - rename and remove-from-list for the scratch profiles.

  Afterwards remove the scratch profiles from the list. The scratch folders are deleted by recycling them, and the user is told.
- Screenshots of each dialog and state of the real window via `orca computer`.
- **The user's mod folders:** the name, size and mtime snapshot of the three real folders is identical before and after; the operations never touch them.
- Report what was run separately from what was only inspected, including the "permanent delete refused" proof or its absence.

## Out of scope

Compare mode, savegame comparison, tags and notes (M6). Mod updates (M7). Undo for file operations. Hard-link space saving, file watching, launching the game.
