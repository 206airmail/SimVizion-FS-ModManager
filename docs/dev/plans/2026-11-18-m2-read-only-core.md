# Milestone 2 — read-only core (`fsmm-core`)

Date: 2026-11-18. Approving this plan authorises **Milestone 2 only**. The M1 plan is in `docs/dev/plans/2026-11-18-project-plan.md`; this plan is copied to `docs/dev/plans/2026-11-18-m2-read-only-core.md` in Stage 0.

## Context

M1 shipped the Library screen on mock data (merged as PR #1, `a5cb3a7`). M2 builds the part of the Rust core that **reads** real Farming Simulator 25 data:
- find the game and the user folder;
- read the mods-folder override and the game log;
- scan a mod folder into `ModEntry` records with health issues;
- decode mod icons into PNG thumbnails.

Nothing in M2 writes to game files, touches the network, or changes the UI. M3 wires this into Tauri commands, a SQLite cache and streaming. So M2's API is shaped around M3's needs: cheap enumeration, a per-mod read that is the unit a cache miss pays for, and a pure health pass over a whole folder.

## What the real data says (read-only survey of 604 zips and 8 folders, 2026-11-18)

- **modDesc files.** 603 of 604 `modDesc.xml` files parse. The exception, `FS25_ClaasTrionPack`, has an unescaped `&` in its `<l10n>` text and the game still loads it, so the parser needs a lenient retry. All files are valid UTF-8, 363 of them with a BOM. 6 have no XML declaration and 8 have comments before the root. The root is always `<modDesc descVersion="…">`.
- **Titles and descriptions.** Almost all use language children, always with `<en>`. A few are plain text, 555 use CDATA, and none use `$l10n_` references. Language codes are inconsistent (`cz`/`cs`, `uk`/`ua`).
- **Icons.**
  - Format: 601 DDS and 2 real PNG. The DDS icons are DXT1 534, DXT5 35, BC7 (DX10 header, DXGI 98) 27, DXT3 2, uncompressed 24-bit BGR 2 and 32-bit BGRA 1.
  - Path: 115 declare a `.png` that is missing while the `.dds` exists, and 42 live in a subfolder.
  - Dimensions: 256–1024, including 500, 552 and 512×256.
  - `mipMapCount` 0 occurs and is treated as 1.
- **Zip structure.**
  - Compression: entries are Deflate or Stored only.
  - Not seen: zip64, backslash names, wrapper folders or mod packs.
  - Directory entries: 352 zips have explicit ones.
  - Largest zip: 3,444 entries.
  - Conflict copies: 6 zips are Syncthing `*.sync-conflict-*` copies.
- **Mod kind evidence.** 7 mods have `<maps>`. 446 have storeItems, whose XML roots are `<vehicle>` or `<placeable>`, with 6 mixed. 146 are script-only (extraSourceFiles or specializations), and 8 have none of these.
- **Install and log.**
  - Steam app 2300320.
  - The installed version is in `<install>\VERSION` (`1.25.1.0`). The exe file version is `10.0.0.0` on every exe, so it is useless.
  - Process names: `FarmingSimulator2025.exe`, `x64\FarmingSimulator2025Game.exe`, `dedicatedServer.exe`.
  - **`log.txt` is stale.** It was last written 2025-08-31 by game `1.10.0.0 b34141`, with `ModDesc Version: 100`, while mods go up to descVersion 116. A naive descVersion check would flag a third of the library.
- **Timing (warm OS cache).** Opening each zip, reading `modDesc.xml` and the first 148 icon bytes takes about 0.3 s for all 604. Cold-cache timing is unmeasured.

## Decisions for M2

| # | Decision | Rejected |
|---|---|---|
| 1 | **No contract change.** `model.rs` stays as is. The core produces `ModEntry` with `icon: None`, `source`/`update` `Unknown` and empty `also_in`. M3 fills those and decides which discovery types to export. | Exporting discovery types now (premature before M3's commands exist) |
| 2 | **descVersion check only against a current log.** `logged_desc_version` is used only when the log's game version matches the installed `VERSION`; otherwise it is `None` and no mismatch issues are raised. A mod above the logged value gets a Warning; lower values are not flagged (the game accepts older descVersions; the exact acceptance rule is unconfirmed). | Always comparing against the log (false positives today); flagging lower values |
| 3 | **Lenient modDesc parse.** On a parse error, retry once with bare `&` (not followed by an entity or char reference) escaped. Still failing → `NotAMod` with a detail. | Failing the mod; regex extraction |
| 4 | **Conflict copies.** `*.sync-conflict-*`, ` (N)` and ` - Copy` names get `ConflictCopy` (naming the original). `InvalidFileName` is not also raised for them. | Double flagging |
| 5 | **Not mods.** `.stfolder`, `.stversions`, `.stignore` and hidden or system entries are ignored. Other non-zip files go in a `skipped` list with a reason. A folder with no root `modDesc.xml` gets `NotAMod`. | Flagging every stray file |
| 6 | **Platform calls behind a `SystemProbe` trait.** That covers the registry, the process list, the known Documents folder and the Epic manifest directory. Discovery logic is tested with a fake probe and temp dirs. | Calling Win32 directly inside discovery logic (untestable) |
| 7 | **Thumbnails** are RGBA PNG with a 256 px longest edge, decoded from mip 0 and area-averaged down. They are returned as bytes; M3 caches them on disk. | Full-size PNGs; an imaging framework |
| 8 | **Caching identity** is path + size + mtime (M3). Not used in M2. FS25's `fileHash` algorithm is undocumented, so no hashing is used. | MD5 or blake3 hashing now |

## Dependencies (`crates/fsmm-core`; each commit states the reason; versions re-checked when added)

| Crate | Use | Notes |
|---|---|---|
| `zip` ~9.7, `default-features = false` + read-side Deflate only | Central-directory access to mod zips | Releases are frequent, so pin the minor version. Confirm the minimal feature set builds and reads Deflate. |
| `roxmltree` 0.21 | Parse `modDesc.xml` and `gameSettings.xml` (read) | UTF-8 only, which matches all real files. The BOM is stripped before parsing. |
| `bcdec_rs` 0.2 | Decode BC1, BC2, BC3 and BC7 blocks | Pure Rust. The DDS header is parsed by our code (about 100 lines), so `ddsfile`, `image` and `image_dds` are not needed. |
| `png` 0.18 | Decode the PNG icons and encode thumbnails | |
| `rayon` 1.12 | Parallel per-mod reads and icon decoding | |
| `keyvalues-parser` 0.2 | Steam `libraryfolders.vdf` and `appmanifest_*.acf` | Chosen over `steamlocate`, which hides the root paths that tests need. |
| `winreg` 0.56 | Steam path and Uninstall keys | |
| `dirs` | Known Folder Documents | Calls `SHGetKnownFolderPath`, so it honours redirection. |
| `windows-sys` (ToolHelp feature only) | Process snapshot | Lighter than `sysinfo` for a name check. |
| `humantime` | RFC 3339 timestamps for `modified` and `scanned_at` | Or a small tested helper if it is unsuitable. |
| `serde_json` (move to normal deps) | Epic `.item` manifests | |
| dev: `tempfile` | Temp dirs for tests | |

Rejected: `quick-xml` (only needed for byte-range writes in M4, if at all), `image`, `image_dds` and `dds` (heavier, and 24-bit BGR support is unclear), `sysinfo`, `fast_image_resize` and `md-5`.

## Module interfaces (Stage 0 commits these signatures; workers fill them in)

Each workstream owns a directory module, so `lib.rs` and the manifests stay coordinator-owned.

```rust
// container.rs — shared, coordinator-implemented in Stage 0
pub enum ModContainer { Zip(..), Folder(PathBuf) }
impl ModContainer {
    pub fn open(path: &Path) -> io::Result<Self>;                 // read-only open
    pub fn find(&self, rel: &str) -> Option<String>;              // case-insensitive, `\`→`/`
    pub fn read(&mut self, rel: &str, limit: u64) -> Result<Vec<u8>, ReadError>; // size-capped
    pub fn read_head(&mut self, rel: &str, n: usize) -> Result<Vec<u8>, ReadError>;
    pub fn names(&self) -> impl Iterator<Item = &str>;
}

// discovery/ — WS-A
pub trait SystemProbe { fn registry_string(..); fn registry_subkeys(..); fn process_names(..);
                        fn documents_dir(..); fn program_data_dir(..); }
pub struct WindowsProbe;                                           // real impl
pub struct GameInstall { pub store: Store /* Steam|Giants|Epic */, pub dir: PathBuf, pub version: Option<String> }
pub fn find_installs(p: &dyn SystemProbe) -> Vec<GameInstall>;     // each validated by FarmingSimulator2025.exe
pub fn user_dir(p: &dyn SystemProbe) -> Option<PathBuf>;           // Documents\My Games\FarmingSimulator2025
pub fn running(p: &dyn SystemProbe) -> Running { game, server };
pub fn read_mods_override(game_settings: &Path) -> Result<ModsOverride, SettingsError>; // errors carry no file text
pub fn read_log_summary(log: &Path) -> Result<LogSummary, io::Error>;  // game_version, mod_desc_version
pub fn suggest_profiles(user_dir: &Path) -> Vec<PathBuf>;          // `mods` + subfolders holding a mod
pub fn game_status(p: &dyn SystemProbe) -> GameStatus;             // assembles model::GameStatus, decision 2

// scan/ — WS-B  (moddesc.rs, health.rs inside)
pub struct FolderEntry { pub path: PathBuf, pub format: ModFormat, pub size: u64, pub modified: SystemTime }
pub struct ModRecord { pub entry: ModEntry, pub icon: Option<IconLocation> }
pub struct IconLocation { pub rel: String, pub kind: IconKind /* Dds|Png */ }
pub fn enumerate(dir: &Path) -> io::Result<(Vec<FolderEntry>, Vec<Skipped>)>;  // stat only
pub fn read_mod(e: &FolderEntry, profile_id: &str, langs: &[&str]) -> ModRecord; // never fails: errors → NotAMod
pub fn assess(records: &mut [ModRecord], logged_desc_version: Option<u32>);     // folder-level rules
pub fn scan_folder(dir, profile_id, langs, logged_desc_version) -> io::Result<FolderScan>; // enumerate → par read → assess

// icons/ — WS-C
pub fn thumbnail(c: &mut ModContainer, icon: &IconLocation, max_edge: u32) -> Result<Vec<u8>, IconError>;
pub fn decode_dds(bytes: &[u8]) -> Result<Rgba, IconError>;   // DXT1/3/5, BC7 (DX10), BGR24, BGRA32
```

Stage 0 also adds `#[cfg(test)] testutil` with builders that generate fixtures in temp dirs: a zip with Stored or Deflate entries, a folder mod, and minimal DDS and PNG byte builders. No real mod files, real icons or machine paths go into the repo.

## Work plan

**Stage 0 — coordinator, serial** (on branch `m2-read-only-core`)
1. Stop the leftover `npm run tauri dev` background shell. Create the branch and copy this plan to `docs/dev/plans/`.
2. Add the dependencies to the manifests (one commit with their reasons). Write `container.rs` and its tests, `testutil`, and the compiling module skeletons with the signatures above.
3. Run the checks, commit and push.

**Stage 1 — three parallel implementers** (git worktrees, `claude/m2-*` branches, never pushed; briefs per `/parallel-work`)

| Workstream | Owns | Acceptance (tests in temp dirs only) |
|---|---|---|
| **WS-A discovery** | `src/discovery/**`, `examples/discover.rs` | <ul><li>**Steam:** registry path → `libraryfolders.vdf` → `appmanifest_2300320.acf`, multi-library.</li><li>**GIANTS:** via Uninstall keys.</li><li>**Epic:** via `.item` manifests.</li><li>Each install is validated by its exe, and its version comes from `VERSION`.</li><li>**Override:** active, inactive, missing, BOM and malformed cases.</li><li>**Secrets:** the error Display never contains file text. A fixture with a fake password proves it.</li><li>**Log:** version and ModDesc lines are parsed. A stale log yields `logged_desc_version: None`.</li><li>**Processes:** both processes are detected via the fake probe.</li></ul> |
| **WS-B scan + moddesc + health** | `src/scan/**`, `examples/scan_report.rs` | <ul><li>**modDesc fields:** title and description language choice (preferred → `en` → first → plain text), CDATA, BOM, no declaration, leading comments, the bare-`&` retry, author, version, descVersion, multiplayer, dependencies.</li><li>**Kind:** maps → Map; storeItem roots by majority → Vehicle or Placeable; extraSourceFiles or specializations only → Script; else Other. `$data/` paths are skipped.</li><li>**Icon resolution:** exact → `.png`↔`.dds` swap → case-insensitive.</li><li>**Size caps:** decompressed reads are capped, so a zip bomb cannot exhaust memory.</li><li>**Rules:** every `IssueKind` rule, including twins, conflict copies, nested archive or wrapper folder, invalid names, missing dependencies and decision 2.</li><li>**Folder mods:** size and mtime are correct.</li></ul> |
| **WS-C icons** | `src/icons/**` | <ul><li>**Formats:** DXT1, DXT3, DXT5, BC7 (DX10 header), BGR24 and BGRA32 decode to the expected pixels from generated blocks. BC7 uses a known mode-6 block.</li><li>**Shapes:** non-power-of-two, non-square and mip-count 0 inputs work.</li><li>**Bad input:** truncated or oversized headers are rejected without panicking, and dimensions are capped at 8192.</li><li>**PNG:** RGB and RGBA inputs work.</li><li>**Output:** longest edge ≤ 256, aspect ratio kept.</li></ul> |

Each worker runs `cargo fmt`, clippy with `-D warnings` and `cargo test -p fsmm-core` before reporting. Workers do not edit `lib.rs`, manifests, `model.rs`, `container.rs` or `testutil`. They ask for any change there.

**Stage 2 — coordinator integration**
1. Merge the three branches. Add `--thumbs <dir>` to `scan_report`, writing PNGs to the scratchpad only. Review the whole diff.
2. Run the real-data smoke checks below, then update `decisions.md` and `STATUS.md`.
3. Commit, push and open a PR to `main` for the user's review.

## Verification

- `cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`; `npm run check` (unchanged frontend, contract unchanged → `npm run gen:types` produces no diff).
- **Real-data smoke, read-only, output to the terminal only, nothing committed:**
  - `cargo run -p fsmm-core --release --example discover` must find the Steam install at 1.25.1.0, find the user folder through the redirected Documents, report whether the override is active and whether the effective folder exists without printing the file, report the log as stale, and show the running states.
  - `cargo run -p fsmm-core --release --example scan_report -- <mods>` on each of the three folders must show:
    - **Mods:** 604 of 604 zips and 8 of 8 folders become mods, with ClaasTrionPack parsed by the retry.
    - **Kinds:** 7 maps.
    - **Conflict copies:** 6 flagged.
    - **Twins:** zip and folder twins flagged.
    - **Thumbnails:** every icon produces one (601 DDS and 2 PNG), and a sample is inspected visually from the scratchpad.
  - **Timing:** report the time for metadata only and for metadata plus thumbnails. Target: metadata for `mods` (590 zips) in ≤ 2 s on a warm cache. Report the first run of the session as the closest available cold figure.
- Report what was run, separately from what was only inspected. Unverified by design:
  - GIANTS and Epic detection is tested only against fixtures; neither install exists on this machine.
  - The game's exact descVersion acceptance rule is unconfirmed.
  - Whether the game itself does the `.png`→`.dds` swap is unconfirmed.

## Out of scope for M2

Tauri commands, SQLite, streaming, file watching, and real data in the UI (M3). Any writes, backups or the running-game guard on writes (M4). Savegame reading (M5). Network (M6).
