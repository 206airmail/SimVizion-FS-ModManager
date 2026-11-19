// Builds src-tauri/icons from the two source images in src-tauri/icons/source (plan M8):
//   app-icon.png        the full SimVizion app icon, used for 64 px and up
//   app-icon-small.svg  its simplified version, used for 16, 24, 32 and 48 px
// `tauri icon` renders each source; this script then writes icon.ico with the small artwork for
// the small sizes and the full artwork for the large ones, and uses the small 32 px PNG.
//
// Run: node scripts/make-icons.mjs

import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const icons = join(root, 'src-tauri', 'icons');
const source = join(icons, 'source');
const work = mkdtempSync(join(tmpdir(), 'fsmm-icons-'));

const cli = join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');

function tauriIcon(input, output, sizes = []) {
  const args = [cli, 'icon', input, '-o', output, ...sizes.flatMap((s) => ['-p', String(s)])];
  execFileSync(process.execPath, args, { cwd: root, stdio: 'inherit' });
}

try {
  // Every platform icon from the full artwork, straight into src-tauri/icons. The app is
  // Windows-only, so the Android and iOS sets it also writes are removed again.
  tauriIcon(join(source, 'app-icon.png'), icons);
  for (const mobile of ['android', 'ios'])
    rmSync(join(icons, mobile), { recursive: true, force: true });
  // The small sizes from the simplified artwork.
  const small = join(work, 'small');
  tauriIcon(join(source, 'app-icon-small.svg'), small, [16, 24, 32, 48]);
  // The full artwork at the large ICO sizes.
  const large = join(work, 'large');
  tauriIcon(join(source, 'app-icon.png'), large, [64, 128, 256]);

  const entries = [
    ...[16, 24, 32, 48].map((size) => ({ size, file: join(small, `${size}x${size}.png`) })),
    ...[64, 128, 256].map((size) => ({ size, file: join(large, `${size}x${size}.png`) })),
  ];
  writeFileSync(join(icons, 'icon.ico'), ico(entries));
  copyFileSync(join(small, '32x32.png'), join(icons, '32x32.png'));
  console.log(`icon.ico: ${entries.map((e) => e.size).join(', ')} px`);
} finally {
  rmSync(work, { recursive: true, force: true });
}

/** An ICO file holding PNG images (supported by Windows Vista and later). */
function ico(entries) {
  const images = entries.map((e) => ({ size: e.size, data: readFileSync(e.file) }));
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(images.length, 4);
  let offset = 6 + 16 * images.length;
  const directory = images.map(({ size, data }) => {
    const entry = Buffer.alloc(16);
    entry.writeUInt8(size >= 256 ? 0 : size, 0); // width (0 means 256)
    entry.writeUInt8(size >= 256 ? 0 : size, 1); // height
    entry.writeUInt8(0, 2); // palette colours
    entry.writeUInt8(0, 3); // reserved
    entry.writeUInt16LE(1, 4); // colour planes
    entry.writeUInt16LE(32, 6); // bits per pixel
    entry.writeUInt32LE(data.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += data.length;
    return entry;
  });
  return Buffer.concat([header, ...directory, ...images.map((i) => i.data)]);
}
