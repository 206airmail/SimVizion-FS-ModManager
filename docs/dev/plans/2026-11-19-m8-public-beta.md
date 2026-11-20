# Milestone 8: public beta release

Date: 2026-11-19. Approving this plan authorises **Milestone 8 only**. M1–M7 are merged (PRs #1–#7). In Stage 0 this plan is copied to `docs/dev/plans/2026-11-19-m8-public-beta.md`.

## Context

The app is feature-complete for its first audience, but it is not shippable yet:
- **No installer setup:** there is no bundle metadata, no update feed and no CI.
- **The emblem is a placeholder.**
- **Unfinished pieces are still visible:** "Launch game" does nothing, "Health" says "not built yet", and with no FS25 user folder the main area is blank.
- **Shipping and detection gaps:** the browser mock and the design gallery ship in the release bundle. GIANTS and Epic detection is fixture-only, and the Microsoft Store version is not detected at all.

M8 closes these gaps and publishes the source and a **beta** build.

### User choices (2026-11-19)

| Topic | Choice |
|---|---|
| Distribution | Make the **source repo public**. Releases and the app's update feed live on its GitHub Releases. |
| License | **MIT**. |
| Old email in history | **Rewrite** the 7 GitHub-made merge commits that carry the account email, before going public. |
| Signing | **Unsigned for now.** The README explains the SmartScreen warning. The build stays ready for a certificate later. |
| Scope | **Launch game**, a **no-game / first-run screen**, a **Health screen**, **Microsoft Store detection**. |
| Emblem | **New design. Codex designs a few options**, and the user picks one. |
| Release type | **Beta.** |
| Before release | **Independent reviews by Fable and by Codex (codex-astra)**, looking for bugs, security flaws and anything missed. |

### Facts (researched 2026-11-19)

- **Updater:** `tauri-plugin-updater` 2.x needs a minisign keypair (`tauri signer generate`). The public key goes in the config; the private key goes in `TAURI_SIGNING_PRIVATE_KEY` at build time.
  - `bundle.createUpdaterArtifacts: true` makes the signed update files. The feed is a `latest.json` (`version`, `notes`, `pub_date`, `platforms.windows-x86_64 {url, signature}`).
  - The update signature is mandatory and is separate from Windows code signing.
  - On Windows the installer runs in `passive` mode (a small progress window).
  - It needs a public URL: private release assets cannot be fetched.
- **GitHub "latest":** `/releases/latest` skips releases flagged "pre-release". Beta releases are therefore published as normal releases with a beta version (`0.9.0-beta.1`), and the README says beta.
- **NSIS:** the default `currentUser` install needs no admin and goes to `%LOCALAPPDATA%`.
  - Its uninstaller has an optional "delete app data" choice, which removes `%APPDATA%\<id>` and `%LOCALAPPDATA%\<id>` and is skipped during updates.
  - MSI (WiX) cannot take a pre-release version such as `-beta.1`, so the build is **NSIS only**.
  - `embedBootstrapper` adds about 1.8 MB and installs WebView2 if it is missing.
- **CI:** `tauri-apps/tauri-action@v1` builds, uploads the installer, its `.sig` and `latest.json` to a release (`uploadUpdaterJson`), and can leave it as a draft. Actions are free for public repos.
- **Signing:**
  - Azure Artifact Signing (about $9.99 a month) is open to individuals in the US and Canada only.
  - OV certificates cost about $150–300 a year.
  - Since 2024 neither gives instant SmartScreen trust.
  - An unsigned app shows "Windows protected your PC" (More info → Run anyway). Smart App Control can block it.
- **Microsoft Store version** (`GIANTSSoftware.FarmingSimulator25PC_<publisher hash>`): its user data is reported under `%LOCALAPPDATA%\Packages\<package>\LocalCache\Local\` (for example `...\Local\mods`), not under `My Games`. Community sources only, and there is no real install here, so detection will be **fixture-tested and labelled unverified**.
- **What the app stores:**
  - `%APPDATA%\com.simvizion.modmanager`: `library.sqlite`, and `backups\` holding copies of `gameSettings.xml`, which contain plain-text multiplayer passwords.
  - `%LOCALAPPDATA%\com.simvizion.modmanager`: `cache.sqlite` and the WebView2 profile.
- **Repo history:**
  - No machine paths, passwords or personal files are in any commit.
  - 56 commits use the GitHub no-reply address. The 7 merges made through GitHub carry the account email.
  - The docs describe the user's library in general terms: mod counts, Syncthing use, and mod names in STATUS.

## Outcome (acceptance criteria)

1. **Launch game.**
   - The header button starts the detected install:
     - **Steam:** `steam://rungameid/2300320`.
     - **Epic:** the launcher URL built from the manifest's `AppName`.
     - **GIANTS:** the game exe, started in its install folder.
     - **Microsoft Store:** `shell:AppsFolder\<PackageFamilyName>!<AppId>`.
   - It is refused while the game is running ("already running"), and disabled with a reason when nothing is installed.
   - Only the core builds the command or URL.
2. **No-game / first-run screen.** It replaces the blank panel:
   - when no FS25 user folder is found, it says what was looked for and offers **Choose game folder…** and **Add mod folder**;
   - when the user folder exists but no profile is registered, it offers **Add mod folder**.

   A chosen game folder must contain `gameSettings.xml` or a `mods` folder. It is stored in `app_settings` and overrides detection. `listSavegames` can now tell "no user folder" from "no saves".
3. **Health screen.**
   - The "Health" tool lists every mod with issues across all folders, grouped by issue kind, with counts that match the rail.
   - It filters by folder and kind, and clicking a row opens that mod in its folder.
   - It is read-only, with no new fix actions.
4. **Microsoft Store detection.**
   - A `Store::MicrosoftStore` install is found from the package registration.
   - When `My Games\FarmingSimulator2025` does not exist, its `LocalCache\Local` folder becomes the user folder.
   - Fixture tests cover both, and the status bar labels this source.
5. **Release-clean bundle.** The mock dataset and the gallery are loaded only in dev or browser mode, never in the Tauri release bundle (checked by a test that searches the built `dist`). No debug-only path reaches release.
6. **App self-update, opt-in like everything online.**
   - **Settings:** a second switch, "Check for app updates", off by default. The first-run prompt offers both switches.
   - **When on:** the app checks the GitHub `latest.json` at startup, at most once every 24 hours, plus "Check now". A banner offers **Install and restart**, showing the release notes.
   - **Feed:** `https://github.com/206airmail/SimVizion-FS-ModManager/releases/latest/download/latest.json`.
   - **Updates:** signed with the project's minisign key.
   - **Installer:** passive mode, keeping app data.
7. **Installer.**
   - NSIS, per-user, `embedBootstrapper`, product "SimVizion Mod Manager", publisher "SimVizion", MIT license page, Start Menu entry.
   - The uninstaller offers "Also delete settings, cache and gameSettings backups" (unticked).
   - The version is `0.9.0-beta.1`, set once in each of `Cargo.toml`, `tauri.conf.json` and `package.json`, with a test that fails if they differ.
8. **New emblem.** Codex designs about 3 options in the SimVizion style, and the user picks one. All icon sizes are generated (`npm run tauri icon`), and `Emblem.tsx` uses the same art.
9. **Public repo.**
   - **Files:** `LICENSE` (MIT); `README.md` (what it does, beta status, install and SmartScreen note, what goes online and when, build from source); `CHANGELOG.md`; `SECURITY.md` (how to report a flaw); third-party license notices for Rust and npm dependencies, shipped with the installer.
   - **History:** rewritten so no commit carries the account email.
   - **Visibility:** switched to public only after the reviews pass and with the user's explicit go-ahead at that moment.
10. **CI.**
    - `ci.yml` runs every check on pull requests and pushes.
    - `release.yml` runs on a `v*` tag: checks, NSIS build, updater signing, then a **draft** release with the installer, `.sig` and `latest.json`. The user publishes it.
11. **Independent reviews passed.** Fable and Codex each review the whole codebase independently. Each confirmed finding is fixed, or written down as accepted with a reason, before the release.

## Decisions

| # | Decision | Rejected |
|---|---|---|
| 1 | **NSIS only, per-user**, `embedBootstrapper`. | MSI (cannot take `-beta` versions, needs admin for per-machine); `offlineInstaller` (+127 MB) |
| 2 | **Beta versions as normal GitHub releases** (`0.9.0-beta.N`), so `/releases/latest` and the updater find them. | Pre-release flag (invisible to `latest`); a separate feed host |
| 3 | **The app's update check is opt-in** with its own switch, off by default (project rule: online features are opt-in). It uses the official updater plugin, so the update host is not added to the mod-update allow-list. | Always-on update checks; a hand-written updater |
| 4 | **The private minisign key never touches the repo or this session.** The user runs `tauri signer generate`, keeps the key file and its password, and stores them as GitHub secrets (`gh secret set`). Local updater tests use a throwaway test key. | Me generating or holding the real key |
| 5 | **Release builds come from CI on a tag, as a draft** that the user reviews and publishes. | Building releases on this machine; auto-publishing |
| 6 | **The history rewrite uses `git filter-branch --env-filter`**, limited to the account email, rewriting only author and committer emails. Then `main` is force-pushed and the merged remote branches are deleted. Each step needs the user's approval at that moment. The user turns on GitHub's "Keep my email address private" first, so the next web merge does not reintroduce it. The old merge-commit SHAs can still be reached through closed PR pages until GitHub garbage-collects them, which GitHub Support can speed up. | Leaving them; a fresh repo without PR history |
| 7 | **Launch targets are built in the core** from the detected install, the same "no URLs over IPC" rule as M7. | The frontend sending a URL or path to the opener |
| 8 | **Microsoft Store support is fixture-only and labelled unverified** in the README and STATUS until someone with that version confirms it. | Skipping it; claiming support |
| 9 | **The reviews come after everything is built, before going public.** The reviewers get the code and a scope, not my conclusions. | Reviewing parts as they land; giving reviewers the self-review |

## Contract (Stage 0: `model.rs` → `gen:types`)

- **`Store`** gains `MicrosoftStore`. `GameStatus` gains `launchable: bool` and `userDirSource: "detected" | "chosen" | "none"`.
- **`HealthRow`:** `{ profileId, profileName, modId, modName, title, issues: ModIssue[] }`. **`HealthReport`:** `{ rows, countsByKind }`.
- **`AppUpdateSettings`:** `{ enabled, lastCheck? }`. **`AppUpdate`:** `{ version, notes?, date? }`.
- **`ErrorCode`** gains `notLaunchable`, `invalidGameFolder` and `appUpdateFailed`.
- **`Api`** gains:
  - `launchGame()`
  - `healthReport()`
  - `chooseGameFolder()` (native picker → validated → stored) and `clearGameFolder()`
  - `getAppUpdateSettings()` and `setAppUpdatesEnabled(on)`
  - `checkAppUpdate(force)`, which returns `AppUpdate | null`
  - `installAppUpdate(onProgress)`
- The updater runs in `src-tauri` (it is a Tauri plugin). The core only stores the settings in `app_settings`.

## Work plan (new branch `m8-public-beta`)

**Stage 0: coordinator.**
1. Branch, plan copy, a pre-M8 snapshot of the real folders.
2. Contract, `gen:types`, Api and mock stubs, core skeletons, Tauri commands.
3. Dependency: `tauri-plugin-updater` (a 2.x version at least two weeks old), with its reason.
4. A version-sync test.
5. Checks, commit, push.

In parallel, I start a **Codex worker (Orca) to design the emblem options** as a comparison page. Its brief covers the SimVizion v1.4 palette and identity rules, a dark tile, legibility at 16 and 32 px, and no third-party or GIANTS marks. It returns SVGs. **The user picks one** while Stage 1 runs.

**Stage 1: two parallel implementers (worktrees).**

| Workstream | Owns | Acceptance |
|---|---|---|
| **WS-A core** | `crates/fsmm-core/src/**` | <ul><li>Launch target per store, with tests for each store and the running refusal.</li><li>The `health_report` aggregate over all profiles.</li><li>The Microsoft Store probe and user-folder fallback, with fixtures.</li><li>The game-folder override: validation, storage and its precedence over detection.</li><li>`list_savegames` "no user folder" vs "no saves".</li><li>App-update settings in `app_settings`.</li></ul> |
| **WS-C frontend** | `src/**` | <ul><li>The no-game / first-run screen.</li><li>The Health screen.</li><li>The working Launch button with its states.</li><li>The Settings switch for app updates, the update banner with notes and Install and restart, and the two-switch first-run prompt.</li><li>Mock and gallery split out of the release bundle, with a dist test.</li><li>Mock switches `?mock=noGame|msStore|appUpdate`.</li><li>Unit tests, and an Orca review at 1280×720 and 960×640.</li></ul> |

**Stage 2: coordinator.** Integrate, wire the updater and the release infrastructure.
1. Merge both workstreams; wire `tauri-plugin-updater` and the Launch and game-folder commands in `src-tauri`.
2. In `tauri.conf.json`: the bundle (NSIS, publisher, copyright, license, `embedBootstrapper`, `createUpdaterArtifacts`, updater `pubkey` and endpoint) and the version `0.9.0-beta.1`.
3. The chosen emblem: `tauri icon` and `Emblem.tsx`.
4. `LICENSE`, `README.md`, `CHANGELOG.md`, `SECURITY.md`, the third-party notices, `ci.yml` and `release.yml`.
5. A local release build and the real-app checks (see Verification).

**Stage 3: independent reviews (in parallel, read-only).**
- **Fable:** a fresh `claude` subagent on `model: fable`.
- **Codex (codex-astra):** an Orca worker.
- **Brief for both:** the same, with the repo path and scope. The scope covers:
  - correctness;
  - security: IPC surface, the network module and allow-list, zip and XML parsing, path handling and traversal, the updater config, CSP and capabilities;
  - data safety: game-file writes, the Recycle Bin, the journal;
  - privacy: logging and what goes online;
  - release hygiene.
- **They get no summary of my own findings.**
- **What I do with the results:** de-duplicate and verify each finding, fix the confirmed ones (failing test first), re-run every check, and report all findings to the user with what was done.

**Stage 4: going public (each outward step asks the user at that moment).**
1. The user turns on GitHub's private-email setting.
2. The history rewrite: author and committer emails only. I check that the commit tree is unchanged.
3. Force-push `main` and delete the merged remote branches.
4. The user creates the minisign key and adds the GitHub secrets.
5. Switch the repo to public.
6. Push the tag `v0.9.0-beta.1`. CI builds a draft release, and the user reviews and publishes it.
7. Open PR #8 before the tag, so the release is built from `main`.

## Verification

- **Checks:** `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `npm run check`, no diff from `gen:types`, and the version-sync and dist tests.
- **Local release build:** `npm run tauri build` with a throwaway updater key. The NSIS installer installs per-user without admin and creates a Start Menu entry. The installed app:
  - finds the Steam install, shows the real folders and counts, and sends nothing over the network until switched on;
  - shows the Health screen with counts that match the rail;
  - has no `?mock`, gallery or console window, and its `dist` holds no mock data.
- **Updater, end to end, with a test build:**
  - Install `0.9.0-beta.1`. A test-only config overlay points the updater at a local feed serving a `0.9.0-beta.2` build signed with the test key, over local HTTP via the plugin's insecure-transport option. That overlay is never committed.
  - Check, install and restart. The new version runs and the app data is kept.
  - A feed signed with a different key is refused.
- **Launch game:** checked by its command (the Steam URL) in tests. I start the real game only with the user's go-ahead at the time, or the user clicks it themselves.
- **No-game screen:** a test user-profile run with `FSMM_` test overrides, or the mock, shows the screen. "Choose game folder" accepts a valid scratch folder and refuses an empty one.
- **Uninstall:** with the box unticked, the app data stays. With it ticked, both app-data folders go. Tested last, after the data has been backed up to the scratchpad and restored afterwards.
- **CI:** `ci.yml` passes on the PR. `release.yml` is first run on a throwaway tag `v0.0.0-test` in draft mode, then the draft and tag are deleted with the user's approval.
- **Snapshot:** the real mod folders, `gameSettings.xml` and the savegame are unchanged.

## Out of scope

- Code signing (later, if the beta goes well).
- Translations.
- A winget or Microsoft Store listing.
- Fix actions on the Health screen.
- Verifying GIANTS, Epic or Microsoft Store detection on real installs (that needs testers).
- A 1.0 release.

## Status

2026-11-19: Stages 0–3 are done; Stage 4 (going public) waits for the user's approvals.

- **Stage 0:** done. Contract, skeletons, commands, `tauri-plugin-updater`.
- **Stage 1:** done. WS-A (core) and WS-C (frontend) were relaunched after a session restart cancelled them, and finished in their original worktrees.
- **Emblem:** after three rounds of Codex designs, the user chose their own SimVizion icon for 64 px and up, with Codex's simplified "eye and SV" for 16–48 px and the header.
- **Stage 2:** done. Updater wiring, NSIS bundle, version, icons, license and docs, CI. Three fixes came from the real-app checks:
  - the status-bar version;
  - the re-offer of a waiting update;
  - ESLint for `scripts/`.
- **Stage 3:** done.
  - Fable found 21 issues, Codex 9.
  - Two fixers (core, frontend) fixed every confirmed finding. The contract gained `replaceIncomplete` and `runningUnknown`.
  - Release CSP, metadata, workflow hardening and docs were fixed by the coordinator.
  - The installed-app checks were re-run after the fixes.
- **Stage 4:** pending. The private-email setting and the release key are done by the user. Still to do, each with approval: the history rewrite, the force-push, going public, the tag.
