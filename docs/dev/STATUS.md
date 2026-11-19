# Status

Updated: 2026-11-18, branch `m1-foundation-prototype`

## Active work

Milestone 1 (foundation + Library screen prototype on sample data) is built and waiting at the design review gate. Plan: `docs/dev/plans/2026-11-18-project-plan.md`.

- Done: workspace and tooling, tokens, data contract, UI primitives, sample data, Profile Rail shell, Library screen (list, grid, inspector, states).
- Review switches: `#gallery` (all primitives), `?mock=stress` (5,000-mod profile), `?mock=slow`, `?mock=error`, `?mock=empty`.

## Next step

User reviews the prototype (`npm run dev`, open `http://localhost:1420`). After sign-off, merge the pull request and plan Milestone 2 (read-only core: discovery, settings read, scan, modDesc parse, icons).

## Last verification

2026-11-18: `npm run check` passes (78 tests); `cargo test -p fsmm-core` 15 passed; `cargo fmt --all --check` and `cargo clippy -p fsmm-core --all-targets -- -D warnings` clean; `vite build` succeeds. Rendered review in Orca's browser at 1920×1080, 1440×810, 1280×720 and 960×640.

2026-11-19: after the Windows SDK reinstall, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass, and `npm run tauri dev` opens the "SimVizion Mod Manager" window (process responding, WebView2 running). Window contents not screenshotted: Orca's screen capture is blocked by antivirus on this machine.

Production build, 5,000 mods: list scroll step median 2.9 ms (p95 4.0 ms); sorting causes no task over 50 ms; grid scroll step median 8.1 ms (p95 29 ms).

## Known issues

- Grid view drops frames when a whole row of cards mounts at once with 5,000 mods (p95 29 ms). Re-measure with real cached thumbnails in Milestone 3.
- Keyboard focus ring on the list was checked by code, not seen: scripted key presses do not trigger `:focus-visible`.
- The app emblem (`src-tauri/app-icon.svg`, `Emblem.tsx`) is a placeholder.
- Buttons for later milestones are present but inert: Launch game, Set as active, Open folder, Add mod folder, Update, Link source, Show in folder, Copy to, Remove.
- Not built from the Stage 1 list: Checkbox (nothing uses one yet).
