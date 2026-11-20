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

## 2026-11-19: M4 — switching patches two attribute values, nothing else

Why: `gameSettings.xml` is the game's file and holds the user's passwords; the app may change only what it means to change. The core replaces the byte ranges of the `active` and `directory` values (roxmltree positions), escaping only `&`, `<`, `>` and the quote that delimits the value, so every other byte, including formatting, survives and a switch and its undo give a byte-identical file (verified on the real file). The game's default folder is `active="false"` with `directory` left alone. Rejected: re-serialising the document; regex edits; pointing `directory` at `mods`.

## 2026-11-19: M4 — every write is guarded, backed up, atomic, verified and journaled

Why: the game rewrites this file itself. A write is refused while the game or the dedicated server runs (checked again just before the rename), and when the file changed since it was read. The current file is first copied to `%APPDATA%\com.simvizion.modmanager\backups` (newest 20 kept), the new content goes to a temp file beside it and replaces it with a rename, and the result is re-read and checked, with the original put back on a mismatch. Each write is a row in `library.sqlite`'s `ops` journal; Undo is by operation id and only while the file still holds what that operation wrote. Pruning old backups and removing temp files deletes the app's own files directly; the Recycle Bin rule is for the user's files. Rejected: writing in place; backups beside the game file; unlimited backups; a global undo.

## 2026-11-19: Debug builds write game files only when asked

Why: a development session must not change the developer's real settings by accident. Debug builds refuse with `writesDisabled` unless `FSMM_ALLOW_GAME_WRITES=1` is set; release builds always write. Tests only ever write to temp folders. Rejected: no guard; a guard that also blocks release builds.

## 2026-11-19: Roadmap renumbered — file operations first

Why: the user chose to split the old M5. M5 is file operations (copy, move, remove to the Recycle Bin, profile rename and remove-from-list); M6 is Compare mode, savegame comparison, tags and notes; M7 is mod updates; M8 is the release. Rejected: everything in one milestone; the read-only views first.

## 2026-11-19: Recycling refuses rather than deletes

Why: the project rule is that nothing is deleted outright, and Microsoft does not document what `IFileOperation` does with an item it cannot recycle (reports say it deletes it). The recycler sets `FOFX_RECYCLEONDELETE` (plus `FOF_WANTNUKEWARNING`), vetoes in `PreDeleteItem` any delete Windows does not flag `TSF_DELETE_RECYCLE_IF_POSSIBLE` (the approach Electron uses), and counts an item as recycled only when its delete succeeded and a Recycle Bin item was created. Verified 2026-11-19: a file on a local disk went to the Recycle Bin; a file on `\\localhost\C$` (no Recycle Bin) was refused and stayed. Rejected: `SHFileOperation` with `FOF_ALLOWUNDO` (no veto possible); the `trash` crate (no veto, no recycled-item check).

## 2026-11-19: M5 — a replaced mod goes to the Recycle Bin only after its replacement verified

Why: a mod can be 3.9 GB, and a copy can be cancelled or fail. The new copy is staged and verified first; the old mod is recycled just before the staged copy takes its name, so a cancelled or failed Replace leaves the old mod where it was. The exception is a move on one volume, an instant rename, where the old mod goes first. Copied files are synced to disk (`sync_all`) before that rename, so a power cut cannot leave a mod under its real name with missing data. Rejected: recycling the old mod before copying (the original M5 plan; it could leave the folder with neither version); renaming the source to a staging name for moves (a crash between the renames could hide the only copy).

## 2026-11-19: M5 — file operations have their own journal and no Undo

Why: copy, move and recycle rows go into a `file_ops` table in `library.sqlite`, with one row per item and per replaced mod, and ids prefixed `f`. They do not go into the `ops` table, whose rows Undo walks for `gameSettings.xml` writes, so a file operation can never become the "latest write" that Undo looks at. There is no Undo for file operations: recycled mods are restored from the Recycle Bin, and the result banners say so. Rejected: sharing the `ops` table; journal-driven undo of copies and moves (possible later).

## 2026-11-19: Icons one size step above the design system's

Why: on the user's 100% scaled (96 DPI) monitors, 16px outline icons have too few pixels for smooth diagonals and curves and look jagged. The user compared the options on their monitor and chose 18px. Every tier moves up one step: 18px standard, 16px compact, 14px small marks, 22px status, 32px empty states. Strokes are absolute (Lucide's `absoluteStrokeWidth`) at the design system's weights: 1.75px standard, 1.5px compact and small, 2px status and empty. Line heights keep the 1.3 ratio rounded to whole pixels, and row dividers are inset lines, so icons sit on whole pixels instead of being smeared across two. This deliberately departs from the SimVizion 16px standard for this app. Rejected: 16px with thinner or sharper strokes (still jagged); Segoe Fluent Icons (hinted and cleanest, but a different icon style from the design system's Lucide family).

## 2026-11-19: M6 — savegames are read by mod name; DLCs are the only special case

Why: the game matches a save's mods by `modName` only (the zip name without `.zip`), so the report does too. Version and the game's own `fileHash` (`md5(zip ++ modName)`, verified on real files) are extra information the game never enforces for a single-player load. DLCs (`pdlc_<name>`) are checked against `<install>\pdlc\<name>.dlc`. Nothing else is built in: in 1.25.1.0 the game's `internalMods` list is empty, and `FS25_precisionFarming` (which gets game-supplied scripts) is an ordinary zip in the mods folder. Only `careerSavegame.xml` is read, never written; `farms.xml` and the other save files are not opened. Rejected: a "game content" state (nothing qualifies); reading backups (later).

## 2026-11-19: M7 — update checks go through SiloAPI only, opt-in and rate-aware

Why: SiloAPI is a free, public, rate-limited service (300 requests a minute per IP), and its terms forbid overwhelming it. Nothing goes online until the user turns updates on. A check batches every mod name into `POST /mods/lookup` (500 a request), and the zips no name explains into `POST /mods/by-hash` (whole-zip SHA-256, 100 a request). Requests go one at a time. The answers are cached for 3 hours (in `cache.sqlite` `catalog_matches`), "Check now" is debounced to 60 s, and a 429 stores `retry_after` with no retry loop. The real library (599 names) took 6 requests. All network code is in `fsmm-core::net` behind the `Catalog` and `Web` traits:

- one `ureq` agent, with timeouts and no proxy;
- `https` only;
- a host allow-list checked on every redirect hop;
- capped bodies.

Rejected: checking on every start; per-mod requests; a manual catalog search (fingerprints cover renamed zips).

## 2026-11-19: M7 — updates install only under the installed file name, through the M5 write path

Why: the game identifies a mod by its zip name, so an asset under another name would be a different mod. In that case the app offers the mod's page instead.

- **The download:** streamed to a hidden staging file in the target folder.
- **The checks:** it opens as a zip, has a `modDesc.xml`, and its version is at least the catalog's. Also its SHA-256 against the catalog's `archiveSha256` when that is for the same version, and its size against the ModHub page's "Size" row (1024-based, within 5%).
- **The replace:** the old zip goes to the Recycle Bin just before the rename, and other folders are filled from the verified file.
- **Every step:** journaled in `file_ops` (`recycle`, `update`) and refused while the game or the dedicated server runs.

Rejected: renaming the download; Silo's `.old` side-by-side swap.

## 2026-11-19: M7 — ModHub through its public page, not the game's downloader

Why: the game's own ModHub downloader is native code in the game's `.exe`, and GIANTS' Terms of Use (§3.4) forbid reverse engineering it. Each mod page on farming-simulator.com has a public DOWNLOAD link to GIANTS' CDN. The app fetches the page only when the user clicks Update. It takes the one link whose host, path, 8-digit mod id and file name all fit, and never guesses. The CDN serves a file only to a request that comes from its mod page (403 otherwise), so the download sends that page as `Referer`, as a browser does when the user clicks DOWNLOAD. `Web::download` refuses any referer that is not an allowed mod page.

Rejected: copying the game's protocol; guessing CDN URLs from the mod id.

## 2026-11-19: M7 — URLs never cross IPC; manual links are reference only

Why: an IPC call that opens any URL it is given would let any script in the webview open arbitrary pages. The frontend asks `openModLink(modName, kind)`, and the core reads the URL from the catalog cache or the user's stored link, opening only `http` and `https`. Manual source links (KingMods, itch.io, forums) are stored per lower-cased mod name in `library.sqlite` `mod_meta.source_url` and survive rescans, copies and updates. They get no update checks, because those sites have no catalog.

Rejected: passing URLs to the opener plugin from the frontend; scraping third-party mod sites.

## 2026-11-19: M8 — public beta: NSIS per-user, unsigned, updates signed with the project key

Why: the source repo goes public under MIT, and the first release is a beta (`0.9.0-beta.N`, published as normal GitHub releases so `releases/latest` and the updater find them).

- **Installer:** NSIS only (MSI cannot take a pre-release version), per-user (no admin), `embedBootstrapper` for WebView2. The uninstaller's "Delete the application data" box removes `%APPDATA%` and `%LOCALAPPDATA%\com.simvizion.modmanager`; unticked keeps them.
- **No Authenticode signing yet:** the user chose to ship the beta unsigned (Azure Artifact Signing is US/Canada-only for individuals; OV certificates cost money and give no instant SmartScreen trust either). The README explains the SmartScreen warning. Codex review R5 is accepted on this basis.
- **App updates:** `tauri-plugin-updater`, opt-in like every online feature, checking `releases/latest/download/latest.json` at most once a day or at each start while an update waits. Packages are signed with the project's minisign key (`75DC04A29A8CA5C2`); the private key and its password exist only with the owner and as GitHub secrets. `requireSignedVersion` binds each signature to its version, so an older signed build cannot be replayed as newer.
- **Releases come from CI on a tag, as a draft** the owner publishes; the job builds with `--locked` and restores no caches.

Rejected: MSI; offline WebView2 (+127 MB); always-on update checks; building releases on this machine.

## 2026-11-19: M8 — launch targets, the chosen game folder and Microsoft Store detection live in the core

Why: as with M7's links, nothing that is started or opened comes from the frontend. The core builds the launch target per store (Steam URL, Epic launcher URL from the manifest's `AppName`, the GIANTS exe, a Microsoft Store `shell:AppsFolder` id), each id restricted to a safe alphabet. A game folder the user chooses must hold `gameSettings.xml` or `mods`, wins over detection, and stays the choice even while unavailable, so a write never silently goes to another folder. Microsoft Store detection (package registration, `LocalCache\Local` user folder) is fixture-tested only and labelled unverified.

Rejected: the frontend passing paths or URLs; skipping the Microsoft Store version; claiming support without a test install.

## 2026-11-19: M8 — settings backups and Undo belong to one settings file

Why: with a chosen game folder there can be two `gameSettings.xml` files, and a global backup folder let Restore write one folder's settings (and passwords) into the other (reviews R2/F-13). Backups now live in `backups/<hash of the settings path>/`, and each journal row records its settings file; Undo and Restore only act on the matching file. Older loose backups are never restored automatically.

Rejected: one shared backup folder; asking which backup to use every time.

## 2026-11-19: M8 — a staged copy is ours alone, and is never thrown away after the old mod went

Why: the reviews found that the fixed staging name could delete a same-named entry the app did not create (R1), and that a failed rename after the old mod was recycled deleted the verified new copy (F-03). Staging names are now unique and created exclusively, and only the entry an operation created is ever removed. Once the old mod is in the Recycle Bin, a failing rename is retried for a few seconds and then the new file is kept under its hidden name (`replaceIncomplete`). Every replace looks again at the game, Cancel and the target just before recycling (R3), and an unreadable process list refuses writes (`runningUnknown`, R7/F-07).

Rejected: cleaning up leftovers by name; recycling first and copying after.
