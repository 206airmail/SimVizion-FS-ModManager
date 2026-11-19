// Prints the CHANGELOG.md section of one version (its body, without the heading), for the release
// notes and the app's update banner. Fails when the version has no section.
// Run: node scripts/changelog-section.mjs 0.9.0-beta.1

import { readFileSync } from 'node:fs';

const version = process.argv[2];
const text = readFileSync(new URL('../CHANGELOG.md', import.meta.url), 'utf8');
const lines = text.split(/\r?\n/);
const start = lines.findIndex((l) => l.startsWith(`## ${version}`));
if (!version || start < 0) {
  console.error(`CHANGELOG.md has no "## ${version}" section`);
  process.exit(1);
}
const end = lines.findIndex((l, i) => i > start && l.startsWith('## '));
console.log(
  lines
    .slice(start + 1, end < 0 ? undefined : end)
    .join('\n')
    .trim(),
);
