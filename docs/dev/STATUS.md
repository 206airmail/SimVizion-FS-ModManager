# Status

Updated: 2026-11-18, branch `m1-foundation-prototype`

## Active work

Milestone 1: foundation + Library screen prototype on sample data (plan: `docs/dev/plans/2026-11-18-project-plan.md`).

- Stage 0 foundation: done (workspace, Tauri shell, Vite/React, tokens, data contract, project docs).
- Stage 1 UI primitives: next.
- Stage 2 screen (sample data, shell, library): pending.
- Stage 3 integration and polish, then the design review gate: pending.

## Next step

Build the `src/ui/` primitives and a dev gallery route, then the Profile Rail shell and Library screen against `src/api/mock`.

## Last verification

2026-11-18: `npm run check` passes; `cargo test -p fsmm-core` 15 passed; `cargo fmt --all --check` and `cargo clippy -p fsmm-core --all-targets -- -D warnings` clean; `vite build` succeeds.

## Known issues

- **Tauri shell does not compile on the development machine.** The Windows SDK 10.0.26100.0 headers are corrupted (many files, including `Windows.h` and `winnt.h`, are zero-filled), so the `vswhom-sys` C++ build step fails. Needs a repair of the Windows SDK through the Visual Studio Installer. Until then `npm run tauri dev`, `cargo check --workspace` and `cargo clippy --workspace` fail; the frontend (`npm run dev`) and `fsmm-core` are unaffected.
- The app emblem (`src-tauri/app-icon.svg`) is a placeholder.
