/**
 * True in dev servers and plain browser builds, where the sample dataset (`?mock=`) and the design
 * gallery (`#gallery`) are available. False in the production bundle that `tauri build` makes, so
 * neither is compiled into the shipped app. Set by `define` in vite.config.ts.
 */
declare const __SAMPLE_BUILD__: boolean;

/** The app version from package.json (kept equal to Cargo.toml and tauri.conf.json by a test). */
declare const __APP_VERSION__: string;
