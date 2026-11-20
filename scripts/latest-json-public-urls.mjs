// Rewrites the download addresses in a release's latest.json to the public download links.
//
// tauri-action uploads latest.json while the release is still a draft, when the assets' public
// links do not exist yet, so it points the updater at GitHub's REST API instead
// (https://api.github.com/repos/<repo>/releases/assets/<id>). Those requests count against the
// API's unauthenticated limit of 60 an hour per address. Once the release is published, the
// public link https://github.com/<repo>/releases/download/<tag>/<file> serves the same file
// without that limit. The signature covers the installer, not latest.json, so nothing else changes.
//
// Run: node scripts/latest-json-public-urls.mjs <latest.json> <assets.json> <owner/repo> <tag>
//   assets.json is `gh release view <tag> --json assets` output.

import { readFileSync, writeFileSync } from 'node:fs';

const [latestPath, assetsPath, repo, tag] = process.argv.slice(2);
if (!latestPath || !assetsPath || !repo || !tag) {
  console.error(
    'usage: latest-json-public-urls.mjs <latest.json> <assets.json> <owner/repo> <tag>',
  );
  process.exit(2);
}
const latest = JSON.parse(readFileSync(latestPath, 'utf8'));
const { assets } = JSON.parse(readFileSync(assetsPath, 'utf8'));
const nameOf = new Map(assets.map((a) => [a.apiUrl, a.name]));

for (const [platform, entry] of Object.entries(latest.platforms)) {
  if (entry.url.startsWith(`https://github.com/${repo}/releases/download/`)) continue;
  const name = nameOf.get(entry.url);
  if (!name) {
    console.error(`${platform}: no release asset has the address ${entry.url}`);
    process.exit(1);
  }
  entry.url = `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(name)}`;
  console.log(`${platform}: ${entry.url}`);
}
writeFileSync(latestPath, `${JSON.stringify(latest, null, 2)}\n`);
