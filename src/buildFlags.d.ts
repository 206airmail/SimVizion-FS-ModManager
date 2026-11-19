/**
 * True in dev servers and plain browser builds, where the sample dataset (`?mock=`) and the design
 * gallery (`#gallery`) are available. False in the production bundle that `tauri build` makes, so
 * neither is compiled into the shipped app. Set by `define` in vite.config.ts.
 */
declare const __SAMPLE_BUILD__: boolean;
