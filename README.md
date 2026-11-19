# SimVizion Mod Manager

A Windows desktop app for managing **Farming Simulator 25** mods: mod folders as profiles, safe switching, health checks, copy/move between folders, savegame checks, and one-click mod updates.

> **Beta.** This is a public beta (0.9.x). Back up anything you cannot replace, and please report problems in the issue tracker.

![SimVizion Mod Manager icon](src-tauri/icons/128x128.png)

## What it does

- **Profiles are mod folders.** Every folder of mods is a profile. Switching changes only the `modsDirectoryOverride` line in the game's `gameSettings.xml`, after a backup, with Undo.
- **Library.** Search, sort and filter the mods in a folder, as a list or a grid with icons. The details panel shows each mod's version, author, dependencies and problems.
- **Health.** One screen lists every mod with problems in every folder: files that are not mods, invalid file names, zip-and-folder twins, sync-conflict copies, an outdated `descVersion`, missing dependencies.
- **Copy, move and remove** mods between folders, with progress and Cancel. Removed and replaced mods go to the Windows Recycle Bin.
- **Compare** two folders side by side, and **check a savegame** against a folder: which mods it needs that are missing, in another version, or a different file.
- **Tags and notes** on mods.
- **Mod updates** (off until you turn them on): finds newer versions through the [SiloAPI](https://silo-api.hllmr.com) catalog and installs ModHub and GitHub updates in one click, checked before anything is replaced. Mods from other sites can be linked by hand.
- **Launch game** from the app.

## Install

1. Download `SimVizion.Mod.Manager_<version>_x64-setup.exe` from the [latest release](../../releases/latest).
2. Run it. It installs for your Windows user only and needs no administrator rights.
3. **Windows SmartScreen** may say "Windows protected your PC", because the beta is not code-signed yet. Choose **More info → Run anyway** if you downloaded it from this repository's Releases page. (Windows 11's Smart App Control, if turned on, can block unsigned apps entirely.)

Requirements: Windows 10 or 11 (64-bit) and Microsoft Edge WebView2, which Windows 11 includes and the installer adds if it is missing.

**Updates:** turn on "Check for app updates" in Settings and the app tells you when a new version is out and installs it for you. Every update package is signed, and the app refuses one that is not.

**Uninstall:** from Windows Settings → Apps. The uninstaller asks whether to delete the app's data too (settings, tags, notes, cache and the `gameSettings.xml` backups); your mods and game files are never touched.

## Which game versions

| Store                      | Status                                                                   |
| -------------------------- | ------------------------------------------------------------------------ |
| Steam                      | Tested                                                                   |
| GIANTS Software (direct)   | Detected from its uninstall entry; not yet tested on a real install      |
| Epic Games Store           | Detected from the launcher's manifest; not yet tested on a real install  |
| Microsoft Store / Xbox app | Detected from its package registration; not yet tested on a real install |

If the app does not find your game, use **Choose game folder…** and pick the folder that holds `gameSettings.xml` (normally `Documents\My Games\FarmingSimulator2025`). Reports from players with the non-Steam versions are very welcome.

## Privacy and what goes online

Nothing goes online until you switch it on in Settings, and there is no telemetry.

- **Mod updates** send your mod file names, and SHA-256 fingerprints of zips the catalog does not know by name, to `silo-api.hllmr.com`. Installing an update fetches the mod's ModHub page and download (`www.farming-simulator.com`, `cdn*.giants-software.com`) or its GitHub release download. Checks run at most every 3 hours.
- **App updates** read this repository's release feed on GitHub, at most once a day.

## Safety

- Nothing is written while Farming Simulator 25 or its dedicated server is running.
- `gameSettings.xml` is backed up before every change (the last 20 backups are kept in `%APPDATA%\com.simvizion.modmanager\backups`). These backups are copies of your settings and can contain multiplayer passwords.
- Mods that are removed or replaced go to the Recycle Bin. If Windows cannot recycle a file, the app leaves it in place rather than delete it.

## Build from source

You need Windows, [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 24 or later, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (Visual Studio Build Tools with the C++ workload).

```sh
npm install
npm run tauri dev      # the desktop app, reading your real game folders
npm run dev            # the interface only, in a browser, with sample data (http://localhost:1420)
npm run check          # type check, lint, format check and frontend tests
cargo test --workspace # Rust tests
```

Debug builds refuse to write game files unless the environment variable `FSMM_ALLOW_GAME_WRITES=1` is set. See [`CLAUDE.md`](CLAUDE.md) for the project layout and conventions, and [`docs/dev`](docs/dev) for plans and decisions.

Release builds come from the `Release` workflow on a version tag; the app icon set is generated by `node scripts/make-icons.mjs`, and `THIRD-PARTY-NOTICES.txt` by `node scripts/third-party-notices.mjs`.

## License

[MIT](LICENSE). Third-party licenses are listed in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt).

SimVizion Mod Manager is a fan-made tool. It is not affiliated with or endorsed by GIANTS Software. Farming Simulator is a trademark of GIANTS Software GmbH. Mod update information comes from the community [SiloAPI](https://silo-api.hllmr.com) catalog.
