# Status

Updated: 2026-11-19, branch `m8-public-beta`

## Active work

Milestone 8 (public beta) is built, reviewed and verified, and is waiting for the user's review as a pull request. Plan: `docs/dev/plans/2026-11-19-m8-public-beta.md`. M1–M7 are merged to `main` (PRs #1–#7).

- **New in the app:**
  - **Launch game:** Steam, Epic, GIANTS and Microsoft Store targets are built in the core.
  - **Health screen:** every mod with issues, across all folders.
  - **No-game / first-run screen,** plus **Choose game folder…** and a Game folder section in Settings.
  - **Microsoft Store detection:** fixtures only, unverified.
  - **Opt-in app updates:** `tauri-plugin-updater`, signed with the project key.
  - **Single instance.**
  - **Close guard** while an operation runs.
  - **Start-up error message.**
- **Release setup:**
  - Installer: NSIS per-user, `0.9.0-beta.1`.
  - Icons: the user's SimVizion icon at 64 px and up, Codex's "eye and SV" at 16–48 px and in the header (`scripts/make-icons.mjs`).
  - Repo files: `LICENSE` (MIT), `README`, `CHANGELOG`, `SECURITY`, `THIRD-PARTY-NOTICES.txt` (`scripts/third-party-notices.mjs`).
  - Workflows: `ci.yml`, and `release.yml` (draft release with `latest.json`).
  - The release key's public half is in `tauri.conf.json`. The private key and its password are GitHub secrets.
- **Independent reviews:**
  - Fable found 21 issues: 5 medium, 16 low.
  - Codex (`gpt-6.2-astra`) found 9: 5 high, 4 medium.
  - All confirmed issues are fixed, each with a test that failed first where testable.
  - Accepted, not fixed:
    - R5, no Authenticode signing: the user's choice.
    - R4: fixed by the real key.
    - F-21: see Known issues.
  - Reports: the session scratchpad. The findings and fixes are summarised in the PR.

## Next step

1. The user reviews and merges the M8 pull request.
2. With approval at each step:
   - rewrite the 7 merge commits that carry the account email;
   - force-push `main` and delete the merged branches;
   - make the repo public and turn on private vulnerability reporting;
   - tag `v0.9.0-beta.1`.
3. Review the CI draft release, then publish.

## Last verification

2026-11-19, M8 integration after the review fixes:

- **Automated checks:**
  - `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
  - `cargo test --workspace --locked`: 712 + 3 passed, 3 ignored.
  - `npm run check`: 397 tests. `npm run check:bundle`: 2 tests.
  - `gen:types` and the third-party notices: no diff.
- **Installed release build** (built locally with a throwaway test key and a test-only config overlay pointing the updater at a local feed):
  - **Install:** per-user, no admin, Start Menu entry, version listed in Windows Apps. The installer is 6.6 MB and contains no mock data or gallery.
  - **App:** finds the Steam install and real folders, shows `VERSION 0.9.0-BETA.1`, and sends nothing until switched on.
  - **Single instance:** a second launch exits.
  - **Updater:**
    - beta.1 found beta.2, downloaded it, installed it passively and restarted as beta.2.
    - An update found but not installed was offered again after a restart.
    - A replayed older build announced as beta.3 was refused (`requireSignedVersion`), and so was a file signed with another key.
  - **Uninstall:** with the box unticked, the data stays. With "Delete the application data" ticked, both data folders go. The real data was backed up and restored byte-identical.
- **Dev app on real data:**
  - Health shows 13 mods with issues across 3 folders, matching the rail, and a row opens the mod in its folder.
  - Choose game folder refuses an empty folder, accepts a valid one, and "Use detected folder" returns to detection with no setting left behind.
- **Snapshot:** the real mod folders (2,748 files), the `gameSettings.xml` hash and the `savegame1` hash are identical before and after M8.

## Known issues

- **Not exercised on a real install:**
  - Launch game (it would start the user's game).
  - The close-guard dialog and the start-up error dialog.
  - GIANTS, Epic and Microsoft Store detection (fixtures only; testers needed).
- **Leftover staging files:** a staged copy left by a crash (`.<name>.<id>.fsmm-partial`) is never cleaned up automatically. The game and the app ignore it.
- **Large folders (F-21):** a very large folder added as a profile (for example a drive root) is walked in full on every listing.
- **Restore after an upgrade:** older loose `gameSettings.xml` backups and Undo rows from before M8 are no longer offered, because they cannot be tied to one settings file.
- **From earlier milestones:**
  - 242 of 599 mod names are not in SiloAPI.
  - Savegame report sorting is by file name.
  - At 960×640 the Savegames layout is tight.
  - Hash-cache rows for deleted mods are never pruned.
  - The descVersion rule is unconfirmed.
