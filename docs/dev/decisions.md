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
