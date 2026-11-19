# Decision log

Append-only. Newest at the bottom.

## 2026-11-18: Tauri 2 + Rust core + React/TypeScript frontend

Why: small binary and native filesystem speed for large mod collections; the SimVizion tokens already exist as CSS; a web frontend can be reviewed in a browser with mock data. Rejected: Electron (size, memory, the startup failures reported against the existing Electron mod manager); a native Rust UI toolkit (tokens would need re-implementing, no browser-based visual review).

## 2026-11-18: Folders are profiles

Why: a profile is a real mod folder and switching only edits `modsDirectoryOverride`. Nothing moves, existing folders work as they are, and the layout stays usable without the app. Rejected: an app-managed library with hard-linked profile folders (needs an import, one drive only, breaks when a sync tool rewrites files); a single folder with enable/disable by moving archives (hundreds of file moves per switch, mixed state if interrupted). Hard links remain a possible later space saver.

## 2026-11-18: Profile Rail layout

Why: profiles down the left, a dense mod table in the centre with a list/grid toggle, a collapsible inspector on the right. Strongest for switching and for scanning several hundred rows. Rejected as the shell: an icon-card catalogue (slow for bulk work and long collections); a two-pane workbench (poor for browsing). The workbench returns later as a Compare mode.

## 2026-11-18: Build for one machine, design for public release

Why: the app will be released later, so nothing may assume one machine: resolve every path at runtime, detect Steam, GIANTS-launcher and Epic installs, keep all UI text in `src/strings.ts`. Code signing, auto-update and translations wait for the release milestone. Rejected: a personal-only tool; a public product from the first version.

## 2026-11-18: Dedicated server — stay out of its way

Why: no real server setup was available to inspect. The app never edits server files and refuses writes while the dedicated server runs; a server's mod folder can be added as an ordinary profile. Rejected: server sync as a core feature; no handling at all.

## 2026-11-18: Removal goes to the Windows Recycle Bin

Why: the user's choice — familiar, restored through Explorer. Constraint: the operation must fail with a message when a file cannot be recycled, never fall back to permanent deletion (API behaviour to verify when implemented). Rejected: an app-managed holding folder; no removal in the app.

## 2026-11-18: Name — SimVizion Mod Manager

Why: plain and self-explanatory, with the subtitle "Farming Simulator 25" carrying the game. Rejected: SimVizion FS Mod Manager; SimVizion MM.

## 2026-11-18: Private GitHub repository, commit and push as work lands

Why: the user asked for it. One commit per verified stage on a milestone branch, merged to `main` by pull request after review. Rejected: a public repository now; committing only on request.

## 2026-11-18: Mod updates are a planned feature

Why: the user wants update checks and one-click updates for mods from the official ModHub and from GitHub. It is built after profile operations, but the data contract carries `source` and `update` from the start and the prototype shows them, because retrofitting them into the layout later costs more. Feasibility of reading ModHub versions is unresearched. Rejected: an offline-only product.

## 2026-11-18: TypeScript pinned to 6.0.x

Why: `typescript-eslint` 8.73 declares `typescript <6.1.0`, and TypeScript 7 is the current `latest`. Revisit when the linter supports it.

## 2026-11-18: M2 read-only core — libraries

Why: smallest set that covers what the real mod collection uses (surveyed read-only: Stored/Deflate zips; DXT1/3/5, BC7 and uncompressed DDS icons; UTF-8 modDesc files). `zip` (Deflate only, minor version pinned because releases are frequent), `roxmltree`, `bcdec_rs` with our own DDS header parsing, `png`, `rayon`, `keyvalues-parser`, `winreg`, `dirs` (honours a redirected Documents folder), `windows-sys` ToolHelp for the process check, `humantime`. Rejected: `image`/`image_dds`/`dds` (heavier, unclear 24-bit BGR support), `sysinfo` (more than a name check needs), `steamlocate` (hides the root paths tests need), `quick-xml` (only needed if M4's byte-range write needs it).

## 2026-11-18: descVersion is checked only against a current game log

Why: the game logs the highest `ModDesc Version` it supports, but `log.txt` is only rewritten when the game runs. On the dev machine it was written by 1.10.0.0 (descVersion 100) while 1.25.1.0 is installed and mods go up to 116, so a naive check flags a third of the library. The log value is used only when its game version equals the install's `VERSION` file; mods above it get a warning, lower values are not flagged. Rejected: always trusting the log; flagging lower values (the game accepts older descVersions; exact rule unconfirmed).

## 2026-11-18: Issue details carry data, not sentences

Why: user-facing text lives in `src/strings.ts`. The core puts the data a message needs in `ModIssue.detail` (the original mod name, the twin's file name, the missing dependency names, the logged descVersion, a reason code for `notAMod`), and the UI words it per issue kind. The M1 mock's English details change to match in M3. Rejected: English sentences built in Rust.

## 2026-11-18: Paid DLC dependencies are not flagged

Why: mods may depend on `pdlc_*` names, which are DLCs, never in a mod folder, and DLC ownership is not known yet. They count as satisfied until a later milestone reads ownership. Rejected: flagging them as missing (false positives).

## 2026-11-18: M3 — every listing is fresh, the cache only saves reads

Why: re-enumerating a folder costs about 45 ms for 590 zips, so each `listProfiles`/`listMods` (and Rescan) looks at the disk and re-reads only files whose path, size or mtime changed. No stale views and no file watcher yet. Rejected: showing the cache until a manual rescan; a file watcher now.

## 2026-11-18: M3 — progress events, not partial lists

Why: health rules and sorting need the whole folder, and rows appearing under the user's selection are worse than a short wait with "Reading 120 of 590 mods". Rejected: streaming batches of mods into the table.

## 2026-11-18: M3 — user data and cache in separate SQLite files

Why: `library.sqlite` (app data) holds what the user chose, `cache.sqlite` (app cache) holds scan records and thumbnails that can always be rebuilt, so the cache can be deleted, or recreated when corrupt, without losing anything. Thumbnails are BLOBs keyed by path, size and mtime: no file naming or orphan cleanup. `rusqlite` with bundled SQLite. Rejected: one database; thumbnail files; JSON files.

## 2026-11-18: M3 — thumbnails through a custom protocol, commands take ids

Why: `ModEntry.icon` is `http://thumb.localhost/<profile>/<mod>?v=<size>-<mtime>`, decoded on first request and cached, so only icons that scroll into view cost anything. The handler and every command resolve ids through the registry and the last scan, so the frontend can never make the app read or open an arbitrary path. Rejected: the asset protocol with a filesystem scope; decoding every icon during the scan; path arguments from the frontend.

## 2026-11-18: Dependencies and the core are optimised in dev builds

Why: unoptimised zip inflate, DDS decoding and resizing made first-time thumbnails average about 610 ms each in `tauri dev` (34 ms once optimised; cache hits 0.6 ms). `[profile.dev.package."*"]` and `fsmm-core` use `opt-level = 2`; the cost is a longer first dev build. Rejected: accepting a slow dev app; testing only release builds.
