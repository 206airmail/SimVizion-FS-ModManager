# Security policy

SimVizion Mod Manager reads and writes your Farming Simulator 25 files and, when you turn it on, goes online. If you find a way it could damage files, expose data, run code or be tricked into downloading something it should not, please report it privately.

## Reporting a vulnerability

Use GitHub's private reporting: the **Security** tab of this repository → **Report a vulnerability**. Please do not open a public issue for security problems.

Include what you found, how to reproduce it, and which version you used (Settings shows it). You will get an answer within a week, and a fix or a plan within 30 days for confirmed problems.

## Supported versions

Only the latest release gets security fixes.

## What the app does, for reviewers

- **Writes:** only `modsDirectoryOverride` in `gameSettings.xml` (with a backup), and mod files you copy, move, remove or update. Removed or replaced files go to the Windows Recycle Bin; the app refuses rather than delete permanently. Nothing is written while the game or its dedicated server is running.
- **Network:** nothing until you switch it on in Settings. Mod updates contact `silo-api.hllmr.com` (mod file names, and SHA-256 hashes of zips it does not know by name), ModHub mod pages and downloads on `www.farming-simulator.com` and `cdn*.giants-software.com`, and GitHub release downloads, all over HTTPS with a host allow-list. App updates read this repository's release feed and only install packages signed with the project's key. No telemetry.
- **Local data:** `%APPDATA%\com.simvizion.modmanager` (settings, tags, notes, the operation journal, and backups of `gameSettings.xml`, which can contain multiplayer passwords) and `%LOCALAPPDATA%\com.simvizion.modmanager` (a disposable cache).
