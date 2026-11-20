# Changelog

## 0.9.0-beta.1

The first public beta.

- **Profiles are mod folders.** Switch the folder Farming Simulator 25 loads mods from with one click. Only `modsDirectoryOverride` in `gameSettings.xml` changes, with a backup first and Undo.
- **Library.** Search, sort and filter every mod in a folder, in a list or a grid with icons, and see each mod's details, dependencies and problems.
- **Health.** One screen lists every mod with problems across all folders: files that are not mods, invalid names, zip-and-folder twins, sync-conflict copies, outdated `descVersion`, missing dependencies.
- **Copy, move and remove** mods between folders. Removed and replaced mods go to the Windows Recycle Bin, never deleted outright.
- **Compare** two folders, and **check a savegame** against a folder: missing mods, other versions, other files, DLCs.
- **Tags and notes** per mod, shared by every folder.
- **Mod updates (opt-in).** Checks your mods against the SiloAPI catalog, and installs updates from ModHub and GitHub with one click, verified before anything is replaced. Link mods from other sites by hand.
- **App updates (opt-in).** The app can check for and install its own new versions.
- **Launch game** from the app (Steam, Epic, GIANTS and Microsoft Store installs).
- Nothing is written while the game or the dedicated server is running, and nothing goes online until you turn it on.

Known limits of this beta: only the Steam version has been tested on a real install; detection of the GIANTS, Epic and Microsoft Store versions is built from their documented layouts and needs testers. The installer is not code-signed yet, so Windows SmartScreen warns before the first run.
