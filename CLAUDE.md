# SimVizion Mod Manager

A Windows desktop app for managing Farming Simulator 25 mods, mod folders and profiles. Tauri 2 shell, Rust core, React + TypeScript frontend, styled with the SimVizion v1.4 design system. A profile is a real mod folder; switching profiles only edits `modsDirectoryOverride` in the game's `gameSettings.xml`.

Current work state: `docs/dev/STATUS.md` (read before substantial work). Decisions and their reasons: `docs/dev/decisions.md`. Plans: `docs/dev/plans/`.

## Layout

- `crates/fsmm-core/` — pure Rust library, no Tauri or UI. Everything that reads or writes Farming Simulator files goes here.
- `crates/fsmm-core/src/model.rs` — the data contract. `src/api/types/` is generated from it.
- `src-tauri/` — thin Tauri command and event layer over `fsmm-core`.
- `src/api/` — the frontend's only door to data: one `Api` interface, with a mock implementation (and later a Tauri one).
- `src/ui/` — design-system primitives. `src/features/` — screens. `src/strings.ts` — all user-facing text.
- `src/styles/tokens.css` — the only file that defines colours.

## Commands

| Task                              | Command                                                       |
| --------------------------------- | ------------------------------------------------------------- |
| Install                           | `npm install`                                                 |
| Frontend in a browser (mock data) | `npm run dev` then open `http://localhost:1420`               |
| Desktop app (real folders)        | `npm run tauri dev`                                           |
| All frontend checks               | `npm run check`                                               |
| Type-check / lint / format        | `npm run typecheck` / `npm run lint` / `npm run format`       |
| Frontend tests (all / one file)   | `npm test` / `npm test -- src/path/file.test.ts`              |
| Regenerate TS contract types      | `npm run gen:types`                                           |
| Rust format / lint                | `cargo fmt --all` / `cargo clippy --workspace -- -D warnings` |
| Rust tests (all / one)            | `cargo test --workspace` / `cargo test -p fsmm-core <name>`   |

## Conventions

- **Design system:** load the `simvizion-ui` skill before any UI work. Colours come only from `--sv-*` tokens; shared styling uses `.sv-*` classes, app-specific layout uses `.mm-*`. Lime is for the primary action and on/checked states; teal for selection, focus and navigation; ordinary text is near-white.
- **Contract first:** change `model.rs`, run `npm run gen:types`, commit the generated files. Never hand-edit `src/api/types/`.
- **Text:** no user-facing string literals in components; add them to `src/strings.ts`.
- **Dependencies:** each new one needs a stated reason in the commit message. TypeScript stays on 6.0.x until `typescript-eslint` supports newer.
- **TypeScript:** `noUncheckedIndexedAccess` is on.

## Safety rules for game files

- Tests never touch a real Farming Simulator folder. `fsmm-core` functions take every root path as a parameter; tests use temp directories and generated fixtures.
- Never log or commit the contents of `gameSettings.xml` (it holds plain-text multiplayer passwords), real mod files, real mod icons, or machine-specific paths.
- Nothing is deleted outright. Removal goes to the Windows Recycle Bin and must fail rather than fall back to permanent deletion.
- No writes to game files while the game or the dedicated server is running.
- The desktop app reads the real game folders. Its own files are `library.sqlite` (user data, `%APPDATA%\com.simvizion.modmanager`) and `cache.sqlite` (disposable, `%LOCALAPPDATA%\com.simvizion.modmanager`).
- No network calls before the mod-updates milestone, and never telemetry.

## Git

- The user has authorised committing and pushing without asking at sensible points: one commit per completed stage that passes the checks, pushed right after.
- Milestone work happens on a milestone branch pushed to `origin`. It merges to `main` through a pull request after the user's review.
- Scratch worktree branches (`claude/*`) may commit freely and are never pushed.
- Never without explicit approval: force-push, hard reset, deleting branches, changing repository visibility.
