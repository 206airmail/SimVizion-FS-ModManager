# SimVizion Mod Manager (Farming Simulator 25) — project plan

Date: 2026-11-18. Approving this plan authorises **Milestone 1 only** (foundation + Library screen prototype on sample data). Later milestones are outlined for direction and get their own plan before work starts.

## Context

The repo started as one empty commit. The goal is a polished, fast Windows desktop app for managing Farming Simulator 25 mods, mod folders and profiles, styled with the SimVizion v1.4 design system (the `simvizion-ui` skill). No other SimVizion or Tauri project exists on this machine, so this repo sets the conventions. The user's workflow requires: design alternatives before commitments, a frontend prototype on sample data before any backend feature, a design review gate, and no destructive tests against real mods.

## Decisions made (2026-11-18)

| # | Decision | Rejected |
|---|---|---|
| 1 | **Folders are profiles.** A profile is a real mod folder; switching only edits `modsDirectoryOverride`. | App-managed library with hard-linked profiles; one folder with enable/disable by moving zips |
| 2 | **Layout A, Profile Rail.** Profile rail left, mod table centre with list/grid toggle, collapsible inspector right. Workbench returns later as a Compare mode. | Catalog or Workbench as the shell |
| 3 | **Public release later.** Assume nothing about this machine; detect Steam, Giants-launcher and Epic installs; ship an installer. Signing, auto-update, translations deferred (UI strings kept in one module). | Personal-only; public from v1 |
| 4 | **Dedicated server: stay out of its way.** Never edit server files; refuse writes while `dedicatedServer.exe` runs; a server mod folder can be added as an ordinary profile. | Server sync as core; no handling |
| 5 | **Removal goes to the Windows Recycle Bin.** App journals removals. Must fail rather than permanently delete when a file cannot be recycled (API behaviour to verify when implementing). | App holding folder; no removal |
| 6 | **Name: SimVizion Mod Manager**, subtitle "Farming Simulator 25". | SimVizion FS Mod Manager; SimVizion MM |
| 7 | **Private GitHub repo, commit and push as work lands.** See Git section. | Public repo now; commits only on request |
| 8 | **Mod updates are a planned feature:** check for and install updates for mods from the official ModHub and from GitHub. Built after profile operations; designed for from M1. See Updates section. | Offline-only product |

Defaults chosen without asking (say so if wrong):
- Syncthing conflict copies and `.stfolder`/`_staged` entries are flagged or ignored, never removed automatically.
- No network calls before the Updates milestone, and never any telemetry. Online features are opt-in when they arrive.
- npm as package manager (only one installed); Tauri CLI as a dev dependency, no global install.

## Findings that shape the design

Reported by read-only research agents; not re-checked by the coordinator.

- Documents is redirected to another drive on the development machine; the FS25 user folder is `<Documents>\My Games\FarmingSimulator2025\`. Use the Known Folder API.
- `gameSettings.xml`: UTF-8 with BOM, currently `<modsDirectoryOverride active="true" directory="...\$testingMods"/>`. Contains plain-text multiplayer passwords — never log its contents.
- Steam install only on the development machine, in a secondary Steam library (game 1.25.1.0; last run 1.10.0.0).
- `mods`: 590 zips + 3 folders, 23.5 GB, largest 3.9 GB. `$testingMods`: 11 + 5. `$offlineTestingMods`: 3 + 3.
- Zip + folder with the same mod name occurs; the game loads the folder.
- All 590 zips have root `modDesc.xml` (349 with BOM); `descVersion` 90–116; 111 of 113 icons declared `.png` are `.dds` (131,200 bytes each, consistent with 512×512 BC1 — inference).
- `log.txt` prints `ModDesc Version: N` and per-mod hashes; `careerSavegame.xml` lists mods with `modName`, `version`, `required`, `fileHash`.
- Concurrent writes to `gameSettings.xml` by game and tool are a documented corruption source.
- Unconfirmed: accepted `descVersion` range per patch, `fileHash` algorithm, `_update` zips, dedicated-server config paths.

## Scope

- **P0 (MVP):** discovery of install and effective mod folder; scan zips and unpacked mods; search, sort, filter, list/grid, inspector, thumbnails; health flags (not a mod, invalid file name, nested/modpack zip, zip+folder twins, conflict and `(2)` copies, `descVersion` vs the game's last-logged value, missing declared dependencies); safe folder switching with backup and restore.
- **P1:** create profiles; copy/move between folders; removal to Recycle Bin; cross-folder version comparison (Compare mode); savegame comparison; tags and notes; launch game.
- **P2:** mod update checking and one-click updating for ModHub and GitHub mods; file watching; hard-link space saving; profile export/import; log viewer; dedicated-server features once a real server is available.
- **Out:** browsing or installing new mods from inside the app (a mod store), editing mod contents, FS22.

## Architecture

- **Stack:** Tauri 2 + Rust + React/TypeScript + Vite. Rejected Electron (size, memory, Mod Assistant's reported startup failures) and native Rust UI (design tokens already exist as CSS; loses browser-based visual review).
- **Layout of the repo**
  - `Cargo.toml` — workspace.
  - `crates/fsmm-core/` — pure Rust, no Tauri. Modules over time: `model` (DTO contract), `discovery`, `settings`, `scan`, `moddesc`, `icons`, `health`, `ops`, `store`.
  - `src-tauri/` — thin command/event layer.
  - `src/styles/tokens.css` — vendored from the skill's `examples/tokens.css`, reconciled against `references/colors.md`; the only place colours are defined.
  - `src/ui/` — primitives using `.sv-*` classes. `src/features/` — screens. `src/api/` — one interface, `mock` and `tauri` implementations. `src/strings.ts` — all user-facing text.
  - `docs/dev/STATUS.md`, `docs/dev/decisions.md`, `docs/dev/plans/` — per the user's `project-setup`/`save-progress` conventions. `CLAUDE.md` under 100 lines.
- **Contract:** DTOs defined once in `fsmm-core::model`; TypeScript types generated from them and committed; a check fails on drift.
- **Storage (from M3):** SQLite via `rusqlite` (bundled) for scan cache keyed by path + size + mtime, user metadata, profile registry, operation journal. Thumbnails as PNG files.
- **Scan (from M2):** enumerate → stat → cache lookup → parse misses in parallel from the zip central directory only → stream batches → decode icons at lower priority.
- **Safety (from M4):** all mutations through `ops`: preflight (game or dedicated server running → refuse) → journal → act → verify. `gameSettings.xml` gets a byte-range patch of the one element, temp file in the same folder, atomic replace, re-read, timestamped backup. `fsmm-core` receives every root path as a parameter; tests use temp dirs only; debug builds refuse writes into the real FS25 folder unless explicitly enabled.
- **Dependencies:** each added with a stated reason and its version checked at the time (the versions the research agent listed are not relied on).

## Roadmap

| Milestone | Work | Parallel worktrees |
|---|---|---|
| **M1 Foundation + Library prototype** | See below | Screens after primitives land |
| Gate | Design review of M1 at three window sizes; user sign-off | — |
| M2 Read-only core | `discovery` + `settings` read · `scan` + `moddesc` + `health` · `icons`; fixture suite | Three |
| M3 Integration | Tauri commands, SQLite cache, streaming scan, real data in the UI | No |
| M4 Safe switching | `settings` write, backups, running-game guard, restore | No; coordinator-reviewed |
| M5 Profile operations | Copy/move, Recycle Bin removal, Compare mode, savegame comparison, tags | By module |
| M6 Mod updates | Research spike, then source linking, update check, download-verify-replace for ModHub and GitHub mods | Sources in parallel after the shared update flow |
| M7 Release | Other-store install detection hardening, installer, signing, app updater | — |

Coordinator-owned shared files (workers do not edit): workspace manifests, `package.json`, `tauri.conf.json`, `tokens.css`, `fsmm-core::model`, `src/api` interface, `src/strings.ts` structure. Each worker brief names objective, allowed paths and acceptance criteria; `/parallel-work` is followed before spawning.

## Mod updates (M6) — direction and what must be designed in now

Requested by the user 2026-11-18. Nothing here is researched yet; the milestone opens with a spike.

- **Goal:** show which installed mods have a newer version, and update them in one click, for mods from the official ModHub and mods released on GitHub.
- **Designed in from M1 (cheap now, costly to retrofit):**
  - `ModEntry` carries a `source` (unknown / ModHub with id / GitHub with owner and repo / manual) and an `update` state (none / available with version / unknown). Source links are user metadata keyed by mod name, so they survive rescans.
  - The M1 prototype shows these with sample data: a Source column, an "Update available" pill, an Updates filter, and update actions in the inspector, so the layout is reviewed with them present.
  - All network code lives in one `fsmm-core` module behind an explicit setting, with Tauri's network permissions limited to the hosts it needs.
- **Update flow (uses the M4/M5 safety path):** download to a temp file on the target drive → verify it opens and its `modDesc.xml` name and version match → refuse while the game runs → old file to the Recycle Bin → move new file in → rescan. A mod present in several profile folders is updated per folder, with the user choosing which.
- **Open questions for the spike:**
  - ModHub: whether a public, permitted way exists to read a mod's current version and download it. No documented API was found during initial research; scraping and terms of use need checking. Fallback is "check version, open the ModHub page".
  - GitHub: how a local mod is linked to a repo (pasted URL, remembered; possibly a small curated list), release-asset naming, pre-releases, and the unauthenticated rate limit.
  - Savegame impact of version changes, and whether to keep the previous version for rollback.

## Milestone 1 — foundation + Library screen prototype

No filesystem access and no backend features. The Tauri shell only hosts the frontend.

**Stage 0 — foundation (coordinator, serial)**
0. Create the private GitHub repo, set `origin`, push `main`, create branch `m1-foundation-prototype`.
1. Scaffold workspace, Tauri 2 app (`productName` "SimVizion Mod Manager"), Vite + React + TypeScript, npm scripts: `dev`, `build`, `typecheck`, `lint`, `test`, `tauri`.
2. Vendor `tokens.css`; base app styles (window, scrollbars, focus, reduced motion).
3. `fsmm-core::model`: `Profile`, `ModEntry`, `ModIssue`, `GameStatus`, `ScanSummary`; generated TS types.
4. `CLAUDE.md`, `docs/dev/STATUS.md`, `docs/dev/decisions.md` (seeded with the six decisions), this plan copied to `docs/dev/plans/`, `.gitignore`, `.claude/settings.json` allow rules for the read-only and check commands (written as both `Bash(...)` and `PowerShell(...)`).

**Stage 1 — primitives (one worker or coordinator)**
`src/ui/`: Button (primary, secondary, destructive, icon), SearchField, Select/Menu, SegmentedControl, Checkbox, Pill, Panel, Tooltip, Banner, EmptyState, Skeleton, StatusBar, virtualised Table. Every state from the skill's `controls.md`, `tables.md`, `layouts.md`. A dev-only gallery route shows them all.

**Stage 2 — screen (parallel after Stage 1 merges)**
- WS-data: sample dataset (~600 mods, plus a 5,000-row stress set) shaped like the real collection: maps, scripts, vehicles, size range to 3.9 GB, `descVersion` spread, invalid entries, twins, conflict copies, missing dependencies; generated placeholder icons (no real mod artwork in the repo); `api/mock`; pure search/sort/filter functions with unit tests. Paths: `src/api/mock/`, `src/features/library/query*`.
- WS-shell: header with wordmark, active-profile context and Launch (inert), profile rail with ACTIVE marker and health counts, status bar, responsive collapse (rail to icons, inspector to overlay). Paths: `src/features/shell/`.
- WS-library: toolbar (search, type, issue and update filters, list/grid toggle), table with Source column and update pill, grid cards, multi-select, inspector with (inert) update actions, empty/loading/error states. Paths: `src/features/library/` except `query*`.

**Stage 3 — integration and polish (coordinator)**
Merge, whole-screen polish pass per the skill's quality gate, self-review of the full diff.

**Acceptance criteria**
- Profile Rail layout renders 600 sample mods across three sample profiles; search, sort, filter, selection, inspector and list/grid toggle work with keyboard and mouse.
- Scrolling 5,000 rows stays smooth; no layout shift on hover or selection.
- Lime only on the primary action and on/checked states; teal only for selection, focus, navigation; text near-white; no hard-coded colours outside `tokens.css`.
- No clipped or unreachable controls at ~960 px wide.

## Verification (run by the coordinator in the main session)

- `npm run typecheck`, `npm run lint`, `npm test`
- `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`
- `npm run tauri dev` launches the window
- `npm run dev` in Orca's browser: screenshots and written critique at 1920×1080, 1280×720 and ~960 px wide, plus focus, selected, empty, loading and error states
- Report separates what was run from what was only inspected, and lists unverified items

## Git and GitHub (authorised by the user 2026-11-18)

- **Remote:** create a **private** GitHub repo `SimVizion-FS-ModManager` under the owner's account with `gh repo create`, add it as `origin`, push `main`. Done 2026-11-18.
- **Standing permission:** commit and push without asking at sensible points. Recorded in the project `CLAUDE.md` so later sessions inherit it.
- **Cadence:** one commit per completed, verified stage (Stage 0, Stage 1, each Stage 2 workstream after merge, Stage 3), pushed right after. No commit of a state that fails the check commands.
- **Branches:** milestone work happens on a branch (`m1-foundation-prototype`), pushed to `origin`. It merges to `main` after the user's design review, via a pull request. Worker worktree branches (`claude/*`) commit freely and are never pushed.
- **Never without explicit approval:** force-push, hard reset, deleting branches, changing repo visibility.
- **Not committed:** secrets, `.claude/settings.local.json`, real mod files or icons, anything copied from `gameSettings.xml`, machine-specific paths.
