# Milestone 7: mod updates (SiloAPI + GitHub + ModHub)

Date: 2026-11-19. Approving this plan authorises **Milestone 7 only**. M1–M6 are merged (PRs #1–#6). This plan is copied to `docs/dev/plans/2026-11-19-m7-mod-updates.md` in Stage 0.

## Context

Since M1 the contract has carried `ModEntry.source` and `ModEntry.update`, with a Source column, an Updates filter, an update pill and an inert "Update" button. The core still always reports `Unknown`.

M7 makes these real. Updates are detected through SiloAPI, and updates are installed from GitHub releases and from ModHub's public download links. It is the app's first network feature. It must be a good citizen of free services, and it changes mod files only through the safe M5 write path.

### User choices (2026-11-19)

- **Install:** GitHub and ModHub mods are both installed with one click.
- **Page buttons:** buttons to open a mod's page on ModHub and/or GitHub are always shown when the catalog knows them, not only as a fallback.
- **Checking:** automatic on startup, cached between restarts, plus "Check now".
- **Linking:** automatic matching by name, and also by content hash (`by-hash`). In addition, the user can **link a source by hand** for mods from elsewhere (KingMods, itch.io, forums): a pasted page URL, remembered per mod name, opened with a button. A manual catalog search is not needed, because hashes cover renamed zips.
- **ModHub:** not by copying the game's downloader, which is not possible, but through the website's public Download link.

### Facts (researched 2026-11-19)

**SiloAPI.** Researched from the API, the MIT-licensed Silo client (`HLLMR-VENTURES/silo` `siloapi.rs`, `update.rs`) and the Silo terms.
- **Terms and limits:** a public read-only API with no accounts. Rate limiting is per IP: `x-ratelimit-limit: 300` per 60 s, `cache-control: max-age=300`. Abuse and overwhelming volume are prohibited.
- **`POST /mods/lookup`:**
  - The body is `{"techNames":[...]}`, at most 500 names. Drop blanks and duplicates; a blank name fails the batch.
  - The answer is `results[{techName, id, title, latestVersion, download:{source,url}|null, pageUrl, tags}]`.
  - `techName` is the game's mod name.
- **`POST /mods/by-hash`:**
  - The body is `{"hashes":[sha256]}`, at most 100 hashes.
  - The answer is `results[{sha256, matches[{id, techName, version, source, fileName, downloadUrl}]}]`. Every hash asked gets an entry; a 404 means an older server.
  - It identifies exact builds, so renamed zips can still be linked.
- **`GET /mods/:id`:** `sources[]` (`source`, `sourceUrl`, `version`, `installable`, `downloadUrl`), `archiveSha256` for the latest build, `verified` and `pageUrl`.
- **Versions** look like `v8.1.0.3` or `V.1.1.5.0`. Compare them as Silo does: drop a leading `v`, split on `.`, `-`, `_` or `+`, compare numerically, and treat missing trailing segments as 0.
- **Install rule:** never install an asset whose file name differs from the installed zip's. Renaming changes the mod's identity to the game.

**The game's ModHub downloader.**
- It is native engine code: `initModDownloadManager`, `installMod(modId)`, `updateMod(modId)` and `getModMetaAttribute*`, called from `ModHubController.lua`. The protocol lives in the `.exe`.
- GIANTS Terms of Use §3.4: "editing, decompiling, disassembling, and reverse engineering are prohibited." So it is **not** copied.

**The ModHub website.**
- Each mod page (`https://www.farming-simulator.com/mod.php?mod_id=<id>&title=fs2025`) has a public DOWNLOAD link to GIANTS' CDN. For example, `https://cdn27.giants-software.com/modHub/storage/00331515/FS25_Courseplay.zip`; the path is the 8-digit mod id plus the game's file name. The page shows the version and size.
- GIANTS' terms do not forbid automated access, only impairing their services (§15.2). The app fetches a page only on a user's click, one request at a time.

## Outcome (acceptance criteria)

1. **Opt-in.** Nothing goes online until the user enables "Check for mod updates online", in Settings or through a one-time prompt banner.
   - The prompt says what is sent: mod file names, and SHA-256 hashes of zips not matched by name.
   - Off means no network use at all.
   - The Settings screen replaces its placeholder with an Online section: the switch, the time of the last check, any retry-after, and "Check now".
2. **Automatic check on startup**, when enabled and the last successful check is more than 3 hours old. Otherwise cached results are used. "Check now" is always available and is debounced to 60 s.
3. **Matching.**
   - All installed mod names, across every profile and deduplicated, go in batched `lookup` calls; about 2 requests for this library.
   - Zips with no name match are hashed with SHA-256 (cached like the M6 MD5s) and sent in batched `by-hash` calls.
   - Results are cached per lower-cased mod name.
4. **States.**
   - `UpdateState` is `UpToDate`; or `Available { version, installable }` when the catalog version is newer; or `Unknown` when the mod is unmatched or never checked.
   - `ModSource` is filled from the catalog: ModHub with its id, or GitHub with owner and repo.
   - Real `updateCount`, with the Updates filter, pill, Inspector banner and rail counts working.
5. **One-click install.** "Update" (in the Inspector, or "Update N" for a selection) opens a dialog listing the folders that hold the mod, each with its current version; the current folder is ticked. On confirm:
   - **Resolve the download.**
     - **GitHub:** the `downloadUrl` from `GET /mods/:id`.
     - **ModHub:** fetch the mod's page, then take the DOWNLOAD href. It must match `https://cdn<n>.giants-software.com/modHub/storage/<8-digit id>/<File>.zip` with the same mod id.
     - **Either:** the asset's file name must equal the installed file name (case-insensitive), otherwise the result is "Open page".
   - **Download** with progress and Cancel, streamed to a hidden staging file in the first target folder.
   - **Verify:**
     - the SHA-256 against the catalog's `archiveSha256`, when it has one for that version;
     - always: the zip opens, it has a `modDesc.xml`, and its version is at least the catalog version (`versions_equal` or newer);
     - the size against the page's size (ModHub), when given.
   - **Replace** through the M5 path: refused while the game or server runs, the old version to the Recycle Bin, never a permanent delete, a staged rename, journaled as `update`.
   - **Other ticked folders** are filled from the verified file with `copy_staged_replacing`.
   - **Any failure** leaves the old mod in place and removes the staged file.
6. **Page buttons.** The Inspector shows "Open on ModHub" and/or "Open on GitHub" whenever the catalog knows those pages, and "Open <host>" for a manual link. They open in the default browser through the opener plugin, `http`/`https` only.
   - The frontend never sends a URL. It calls `openModLink(modName, kind)`, and the core looks the URL up from the catalog cache or the user's link.
   - When an update cannot be installed (nothing installable, the ModHub link not found, or the asset name differs), the dialog says why and offers the page button instead.
7. **Manual source links.** The inert "Link source" control becomes "Link source…" and "Edit link…": a small dialog to paste a page URL.
   - The URL must be `http` or `https`, at most 2000 characters, with no credentials in it.
   - The dialog shows the host and has a Remove action.
   - Links are stored per lower-cased mod name in `library.sqlite` `mod_meta`, like tags, and survive rescans, copies and updates.
   - A pasted `github.com/<owner>/<repo>` link shows as a GitHub source; any other host as "Other (host)".
   - Manual links are for reference and opening; they get no update checks, because those sites have no catalog. A catalog match, if any, still drives update checks, and both links are shown.
8. **Good citizen.**
   - All network code is in one `fsmm-core::net` module behind traits.
   - One agent, with connect and read timeouts and a neutral `User-Agent` (`SimVizion-Mod-Manager/<version>`). It uses no proxy from the environment.
   - Strictly one request at a time.
   - **Allowed hosts:**
     - API: `silo-api.hllmr.com`, with no redirects.
     - ModHub pages: `www.farming-simulator.com`.
     - Downloads: `github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com` and `cdn*.giants-software.com`. Redirects are followed only within this list.
   - A 429 or 5xx stops the check (no retry loop) and records `retry_after` from `Retry-After` or `x-ratelimit-reset`.
   - ModHub pages are fetched only on an install click.
   - No telemetry and no image fetching.
9. **Mock and UI.** The mock covers:
   - disabled;
   - the first-run prompt;
   - checking;
   - results;
   - a 429 cooldown;
   - offline;
   - GitHub and ModHub installs that succeed;
   - a hash mismatch;
   - the ModHub link not found (falls back to the page);
   - manual links (add, edit, remove, an invalid URL);
   - the game running.

   The screens follow simvizion-ui.

## Decisions

| # | Decision | Rejected |
|---|---|---|
| 1 | **The HTTP client is `ureq` 3** (blocking, rustls; new, because the core needs HTTPS). It fits the blocking-command pattern and Silo uses it. `sha2` is also new (SHA-256 for `by-hash` and the catalog's `archiveSha256`). | reqwest (async runtime); a Tauri http plugin (network in the frontend, CSP changes) |
| 2 | **Traits `Catalog { lookup, by_hash, detail }` and `Web { fetch_page, download }`** in `fsmm-core::net`. Tests use fakes that count calls and return canned JSON, HTML and bytes, so batching, caching, 429 handling, link parsing and verification are tested without the network. | Real-network tests |
| 3 | **The ModHub link is parsed strictly**: the one DOWNLOAD href whose host, path shape, mod id and file name all match. Anything else means "Open page", never a guess. The parser is tested against a saved copy of a real page, with only the needed snippet trimmed in. | Guessing CDN URLs from the mod id (host varies); a general HTML scraper |
| 4 | **Check results are cached in `cache.sqlite`** `catalog_matches(name_key, catalog_id, latest_version, installable, modhub_url, github_url, download_kind, matched_by, checked_at)`. `library.sqlite` migration 5 adds `mod_meta.source_url` (the manual link; `drop_empty_meta` must keep rows that have one) and `app_settings(key, value)` with `updates.enabled`, `updates.lastCheck` and `updates.retryAfter`. | Results kept in memory only; a Tauri store plugin; a separate links table |
| 8 | **URLs never come from the frontend.** `openModLink(modName, kind)` resolves the URL in the core, from the catalog cache or the stored manual link. Only `http`/`https` URLs are opened. | Passing URLs over IPC to an opener (any page could be opened) |
| 5 | **Install reuses the M5 file-operation lock, `OpProgress`, the journal and the Recycle Bin.** One new primitive, `files::install_download`, streams to the staged name, verifies, then `before_rename` recycles the old version. | A separate write path; Silo's `.old` swap with backups |
| 6 | **Never rename on install.** A different asset file name means "Open page". | Renaming the download to fit |
| 7 | **The automatic check runs at most every 3 h, and "Check now" is debounced to 60 s.** | Checking on every start |

## Contract (Stage 0: `model.rs` → `gen:types`)

- **`UpdateState`:** `Unknown | UpToDate | Available { version, installable: bool }`.
- **`ModSource`:** `Unknown | ModHub { id } | GitHub { owner, repo } | Manual { host }`. This is the primary source, for the column and icon: catalog first, else the manual link.
- **`ModEntry.links`:** `Vec<SourceLink { kind: modHub | gitHub | manual, label }>`, where `label` is the host or a short name. The links shown as buttons; there are no URLs in the contract.
- **`UpdateSettings`:** `{ enabled, lastCheck?, retryAfter? }`.
- **`UpdateCheck`:** `{ checkedAt, checked, matchedByName, matchedByHash, updates, fromCache }`.
- **`UpdatePlan`:** `{ modName, toVersion, source: "gitHub" | "modHub", folders: [{ profileId, modId, version }] }`.
- **`ErrorCode`** gains `offline`, `rateLimited`, `updatesDisabled`, `verifyFailed`, `notInstallable` and `invalidLink`.
- **`Api`** gains:
  - `getUpdateSettings()`
  - `setUpdatesEnabled(enabled)`
  - `checkForUpdates(force)`
  - `planUpdate(modName)`
  - `installUpdate(modName, profileIds, onProgress)`, which returns `OpResult`
  - `openModLink(modName, kind)`
  - `setSourceLink(modName, url | null)`, which returns the stored host or rejects with `invalidLink`

  `listMods` and `listProfiles` fill `source`, `links`, `update` and `updateCount` from the cache and the user's links.

## Architecture

- **`fsmm-core::net`** (new):
  - `Catalog` and `Web` traits, with `SiloCatalog` and `HttpWeb` built on ureq.
  - Batching (500 for lookup, 100 for by-hash).
  - Response-size caps, the host allow-list and redirect policy, version parsing and comparison, the ModHub link parser, and `CatalogError { Offline, RateLimited { retry_after }, Server, BadAnswer }`.
- **`fsmm-core::files`:** `sha256` with a cache (the M6 pattern) and `install_download`.
- **`fsmm-core::library::updates`** (new):
  - `check_updates`: names, then lookup, then the unmatched zips' SHA-256, then by-hash, then storing the results.
  - `plan_update` and `install_update`, which resolve the source, download, verify, replace and fill the other folders.
  - `set_source_link` and `link_url(name, kind)`.
  - The overlay in `list_mods` and `summarize`.
- **`fsmm-core::store`:** `library.sqlite` migration 5 (`mod_meta.source_url` and `app_settings`); `cache.sqlite` migrations for `catalog_matches` and the SHA-256 cache.
- **`src-tauri`:**
  - The commands. `openModLink` uses `tauri-plugin-opener` from Rust (already present).
  - The frontend triggers `checkForUpdates(false)` after the first load.
  - Capabilities and CSP are unchanged; the frontend never fetches.
- **Frontend:**
  - The Settings screen (Online section) and the first-run prompt banner.
  - An update-status item in the status bar: checking, last check, offline, rate-limited.
  - The Update dialog (folders and versions), with progress and result through `FileOpsNotices`.
  - The Inspector: the update banner (Update, or why not plus the page button), and a Source section with the page buttons and the link dialog (Link source…, Edit link…, Remove).
  - "Update N" for a selection.
  - Mock switches `?mock=updatesOff|offline|rateLimited|hashMismatch|noModHubLink`.

## Work plan (new branch `m7-mod-updates`)

**Stage 0: coordinator.**
1. Create the branch, copy the plan, and snapshot the real folders.
2. Contract, `gen:types`, the Api and mock stubs.
3. Skeletons: `net`, `updates` and the store migrations as `todo!()`.
4. The Tauri commands, and the dependencies `ureq` and `sha2`. Pick versions at least two weeks old, and state the reasons.
5. Save a trimmed real ModHub page snippet as a test fixture: only the download-link markup and the version and size, with no other page content.
6. Checks, commit, push.

**Stage 1: two parallel implementers (worktrees).**

| Workstream | Owns | Acceptance |
|---|---|---|
| **WS-A core** | `crates/fsmm-core/src/**` | <ul><li>With fake `Catalog` and `Web`: batching at 500 and 100, one request at a time, and call counts.</li><li>Cache reuse within 3 h, the force debounce, and 429/`retry_after` handling with no retry loop.</li><li>Version ordering.</li><li>The overlay on `list_mods` and the counts.</li><li>The ModHub link parser: valid, wrong host, wrong id, renamed file, no link.</li><li>The host allow-list and redirect policy.</li><li>`install_download`, for GitHub and for ModHub:<ul><li>a good install;</li><li>SHA-256 mismatch;</li><li>modDesc version too old;</li><li>not a zip;</li><li>renamed asset (Open page);</li><li>cancel mid-download;</li><li>the game running;</li><li>the recycle refused (old mod kept);</li><li>filling several folders;</li><li>journal rows.</li></ul></li><li>Manual links: validation, GitHub URL detection, storage per name, surviving a rescan, `drop_empty_meta` keeping link-only rows, `link_url` resolution, and http/https only.</li><li>The migrations.</li></ul> |
| **WS-C frontend** | `src/**` | <ul><li>Settings, the prompt, the status item, the Update dialog, the Inspector and selection actions, and the mock states.</li><li>Unit tests.</li><li>Orca review at 1280×720 and 960×640.</li></ul> |

**Stage 2: coordinator.** Integrate, wire the real clients, run the real-app check, update the docs, and open the PR.

## Verification

- **Checks:** `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `npm run check`. `gen:types` produces no diff.
- **Real check, after enabling updates in the real app:**
  - Log the requests in debug builds: method, host, path and status, never bodies. Expect about 2 lookups and at most 1–2 by-hash calls.
  - Report the counts matched by name, matched by hash and unmatched, so the user can decide on manual linking.
  - Spot-check 3 updates against the catalog.
  - After a restart, no request within 3 hours.
- **Real installs, in scratch folders only, using generated old-version placeholder zips named like real mods:**
  - A GitHub mod (for example `FS25_Courseplay`), downloaded from its release and SHA-256 verified.
  - A small ModHub-only mod, through its page link.
  - Each: the placeholder goes to the Recycle Bin, the real zip is in place, and a journal row is written.
  - Recycle the scratch folders afterwards and tell the user.
- **Snapshot:** the real mod folders, `gameSettings.xml` and savegame hashes are unchanged.

## Out of scope

- Manual catalog search (hash matching covers renamed zips; reconsider after the unmatched count).
- Update checks for manually linked sites (KingMods, itch.io and others have no catalog).
- Browsing or installing new mods.
- Using the game's own downloader.
- Keeping older versions beyond the Recycle Bin.
- Installing dependencies automatically.
- The app's own self-update (M8).
- "Verified" provenance badges.

## Status

2026-11-19: all stages are done, and the milestone is waiting for review as a pull request.

- **Stage 0:** done. Contract, skeletons, commands, `ureq` and `sha2`, and the ModHub page fixture.
- **Stage 1:** done. WS-A (core) and WS-C (frontend) were merged.
- **Stage 2:** done. Three changes came out of integration and the real-app check:
  - **"Update N":** a refusal about one mod (for example `notInstallable`) now fails only that mod. Before, it failed every mod still waiting.
  - **ModHub downloads** send the mod page as `Referer`. Without it, GIANTS' CDN answers 403.
  - **The Update dialog** ticks only the folder on screen, as this plan says. It had ticked every folder with an older copy.
- **Real-app check:** passed. The results are in `docs/dev/STATUS.md`.
