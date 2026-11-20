import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// The Node global, declared here because the project carries no Node typings.
declare const process: { env: Record<string, string | undefined> };

// Port and watch settings follow what `tauri dev` expects.
export default defineConfig(({ command, mode }) => {
  // `tauri build` runs the frontend build with TAURI_ENV_* set. That bundle only ever runs inside
  // the desktop app, so the sample dataset and the design gallery are left out of it. Dev servers
  // and plain browser builds keep them (see src/buildFlags.d.ts).
  const desktopRelease = command === 'build' && process.env.TAURI_ENV_PLATFORM !== undefined;
  return {
    plugins: [react()],
    clearScreen: false,
    define: {
      __SAMPLE_BUILD__: JSON.stringify(!desktopRelease),
      __APP_VERSION__: JSON.stringify(process.env.npm_package_version ?? 'dev'),
    },
    server: {
      port: 1420,
      strictPort: true,
      watch: { ignored: ['**/src-tauri/**', '**/crates/**', '**/target/**'] },
    },
    test:
      // `vitest run --mode bundle` (npm run check:bundle) runs only the slow bundle check.
      mode === 'bundle'
        ? {
            environment: 'node',
            include: ['src/**/*.bundle.test.ts'],
            testTimeout: 180_000,
          }
        : {
            environment: 'node',
            include: ['src/**/*.test.ts'],
            exclude: ['**/node_modules/**', '**/*.bundle.test.ts'],
          },
  };
});
