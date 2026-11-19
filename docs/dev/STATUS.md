# Status

Updated: 2026-11-19, branch `m7-mod-updates`

## Active work

Milestone 7 (mod updates) is built and waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-19-m7-mod-updates.md`. M1–M6 are merged to `main` (PRs #1–#6).

- **`fsmm-core::net`** (new; the only network code in the app):
  - **Interfaces:** the `Catalog` (SiloAPI: lookup, by-hash, detail) and `Web` (ModHub page, download) traits, with fakes for tests.
  - **`Http`:** one `ureq` agent with timeouts, no proxy, `https` only, a host allow-list checked on every redirect hop, capped bodies, no retries.
  - **Also:** the strict ModHub download-link parser and the version comparison.
  - **Debug builds** print `[net] METHOD host/path -> status`, never bodies or queries.
- **`fsmm-core::library::updates`:**
  - `check_updates`: names, then fingerprints for zips no name explains. It refuses while updates are off, uses a 3-hour cache, debounces Check now to 60 s, and stores 429s as `retry_after`.
  - The overlay on `list_mods` and `list_profiles`: `source`, `links`, `update` and `updateCount`.
  - `plan_update`, and `install_update`, which goes through `files::install_download` and the M5 path.
  - `link_url` and `set_source_link`.
- **Store:**
  - `library.sqlite` migration 5: `mod_meta.source_url` and `app_settings`.
  - `cache.sqlite` migration 4: `catalog_matches` and the `file_sha256` cache.
- **`src-tauri`:** seven commands in `updates.rs`. `open_mod_link` resolves the URL in the core.
- **Frontend:**
  - The Settings screen with an Online section, the first-run prompt and the status-bar item.
  - Inspector: the update banner and a Source section with page buttons and "Link source…".
  - The Update dialog, which ticks only the folder on screen, and "Update N" for a selection.
  - `Checkbox` and `Switch` primitives, and mock switches `?mock=updatesOn|offline|rateLimited|hashMismatch|noModHubLink`.

## Next step

The user reviews the M7 pull request. After the merge, plan Milestone 8 (the release).

## Last verification

2026-11-19, M7 integration:

- **Automated checks:**
  - `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  - `cargo test --workspace`: 622 + 1 passed, 3 ignored.
  - `npm run check`: 331 tests passed.
  - `npm run gen:types`: no diff.
- **Real app: the update check.** The user's library was checked against the real SiloAPI.
  - **Requests:** the first check made 6, one at a time, all 200: 2 lookups, 3 by-hash and 1 lookup for the hash-matched names. It took 21 s, mostly the first SHA-256 pass over 240 unmatched zips.
  - **Matches:** of 599 distinct names, 356 matched by name and 1 by fingerprint (a Syncthing `sync-conflict` copy of RealisticLivestock); 242 are unmatched. 20 matches have a GitHub release; the rest install through ModHub pages.
  - **Updates shown:** 52 in `mods`, 2 in `$offlineTestingMods` and 2 in `$testingMods`.
  - **Spot checks:** EnhancedVehicle 1.1.7.1 and ContractBoost 1.1.2.8 match their latest GitHub releases. Easy Development Controls 1.2.0.0 matches its ModHub page.
  - **Restarts:** after two app restarts, the startup check answered from the cache (10–12 ms) and sent nothing.
- **Real app: installs into two scratch folders**, with `FSMM_ALLOW_GAME_WRITES=1` for this approved test only.
  - **GitHub:** `FS25_Courseplay` went from a generated 7.0.0.0 placeholder to v8.1.0.3 in both folders at once. That took 3 requests: the catalog detail, then github.com's 302 to `release-assets.githubusercontent.com`. Both files' SHA-256 equals GitHub's published `.sha256`.
  - **ModHub:** `FS25_EasyDevControls` went from a 1.0.0.0 placeholder to 1.2.0.0 through its mod page and `cdn25.giants-software.com`. The file is 459,860 bytes, as the CDN reported.
  - **Recycle Bin and journal:** each placeholder went to the Recycle Bin, and the journal shows `recycle` then `update` rows, all `ok`. The scratch profiles were removed from the list and the scratch folders recycled.
- **Snapshot:** the real mod folders (2,748 files: name, size, mtime), the `gameSettings.xml` hash and the `savegame1/careerSavegame.xml` hash are identical before and after.

## Known issues

- **Unmatched mods:** 242 of 599 names are not in SiloAPI. The user can link those by hand.
- **Not exercised in the real window:**
  - adding, editing or removing a manual link (each link is stored per mod name, so a test would touch the user's real mods' metadata);
  - the "Open on …" buttons, which would open the user's browser;
  - Cancel during a real download;
  - a real 429 or offline state (all covered by fakes and the mock).
- **Courseplay's latest GitHub release** is tagged `v8.1.0.3-cc.7`, while SiloAPI reports `v8.1.0.3`. The app follows the catalog.
- **From earlier milestones:**
  - Savegame report sorting is by file name while titles are shown.
  - At 960×640 the Savegames layout is tight.
  - Hash-cache rows for deleted mods are never pruned.
  - `listSavegames` cannot tell "no user folder" from "no saves".
  - GIANTS and Epic detection is fixture-only.
  - The descVersion rule is unconfirmed.
  - The emblem is a placeholder.
