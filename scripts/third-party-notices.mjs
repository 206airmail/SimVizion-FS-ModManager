// Writes THIRD-PARTY-NOTICES.txt: the license of every third-party Rust crate compiled into the
// app (normal dependencies of src-tauri for 64-bit Windows) and every npm package bundled into the
// frontend (the `dependencies` of package.json and theirs), with the license texts the packages
// ship. The installer includes the file (bundle.resources), and CI checks it is up to date.
//
// Run: node scripts/third-party-notices.mjs

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const LICENSE_FILE = /^(licen[cs]e|copying|notice)([-._].*)?$/i;

/** The license-like files at the top of a package folder, read in name order. */
function licenseTexts(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isFile() && LICENSE_FILE.test(e.name))
    .map((e) => e.name)
    .sort()
    .map((name) => ({
      name,
      text: readFileSync(join(dir, name), 'utf8').replace(/\r\n/g, '\n').trim(),
    }));
}

function rustPackages() {
  const metadata = JSON.parse(
    execFileSync(
      'cargo',
      [
        'metadata',
        '--format-version',
        '1',
        '--locked',
        '--filter-platform',
        'x86_64-pc-windows-msvc',
      ],
      { cwd: root, maxBuffer: 256 * 1024 * 1024 },
    ).toString(),
  );
  const byId = new Map(metadata.packages.map((p) => [p.id, p]));
  const nodes = new Map(metadata.resolve.nodes.map((n) => [n.id, n]));
  const workspace = new Set(metadata.workspace_members);
  const app = metadata.packages.find((p) => p.name === 'simvizion-mod-manager');
  // Walk normal (not dev, not build) dependencies from the app.
  const seen = new Set();
  const queue = [app.id];
  while (queue.length > 0) {
    const id = queue.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const dep of nodes.get(id)?.deps ?? []) {
      if (dep.dep_kinds.some((k) => k.kind === null)) queue.push(dep.pkg);
    }
  }
  return [...seen]
    .filter((id) => !workspace.has(id))
    .map((id) => byId.get(id))
    .map((p) => ({
      name: p.name,
      version: p.version,
      license: p.license ?? (p.license_file ? 'see license file' : 'not stated'),
      repository: p.repository ?? '',
      texts: licenseTexts(dirname(p.manifest_path)),
    }));
}

function npmPackages() {
  const seen = new Map();
  const visit = (name, from) => {
    // Node resolution: the nearest node_modules up from the dependent package.
    let dir = from;
    let found = null;
    for (;;) {
      const candidate = join(dir, 'node_modules', name);
      if (existsSync(join(candidate, 'package.json'))) {
        found = candidate;
        break;
      }
      const parent = dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
    if (found === null || seen.has(found)) return;
    const pkg = JSON.parse(readFileSync(join(found, 'package.json'), 'utf8'));
    seen.set(found, {
      name: pkg.name,
      version: pkg.version,
      license: typeof pkg.license === 'string' ? pkg.license : (pkg.license?.type ?? 'not stated'),
      repository: typeof pkg.repository === 'string' ? pkg.repository : (pkg.repository?.url ?? ''),
      texts: licenseTexts(found),
    });
    for (const dep of Object.keys(pkg.dependencies ?? {})) visit(dep, found);
  };
  const app = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  for (const dep of Object.keys(app.dependencies ?? {})) visit(dep, root);
  return [...seen.values()];
}

function section(title, packages) {
  const sorted = [...packages].sort(
    (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version),
  );
  const out = [`${'='.repeat(78)}\n${title} (${sorted.length})\n${'='.repeat(78)}`];
  for (const p of sorted) {
    out.push(`\n${'-'.repeat(78)}\n${p.name} ${p.version}\nLicense: ${p.license}`);
    if (p.repository) out.push(`Source: ${p.repository}`);
    if (p.texts.length === 0) {
      out.push(`\n(The package ships no license file; its license is ${p.license}.)`);
    }
    for (const t of p.texts) out.push(`\n--- ${t.name} ---\n${t.text}`);
  }
  return out.join('\n');
}

const text = [
  'SimVizion Mod Manager includes the following third-party software.',
  'Each is used under the license shown, whose text follows where the package provides it.',
  '',
  section('Rust crates', rustPackages()),
  '',
  section('npm packages (frontend)', npmPackages()),
  '',
].join('\n');
writeFileSync(join(root, 'THIRD-PARTY-NOTICES.txt'), text);
console.log(`THIRD-PARTY-NOTICES.txt: ${(text.length / 1024).toFixed(0)} KiB`);
