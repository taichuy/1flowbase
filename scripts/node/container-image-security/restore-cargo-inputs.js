#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');

// Cargo uses input mtimes for freshness. A new checkout gives identical sources
// newer mtimes than cached dep-info. Restore times only after content and mode
// verification; never use commit dates (a revert can then hide changed inputs).
function restoreCargoInputs(repoRoot, manifestPath) {
  const previous = fs.existsSync(manifestPath)
    ? JSON.parse(fs.readFileSync(manifestPath, 'utf8')) : {};
  const tracked = execFileSync('git', ['ls-files', '-z', '--', 'api'], {
    cwd: repoRoot, maxBuffer: 16 * 1024 * 1024,
  }).toString().split('\0').filter(Boolean);
  const current = {};
  const directories = new Set();
  let restored = 0;
  const hash = value => crypto.createHash('sha256').update(value).digest('hex');
  function record(relative, signature, stat) {
    const old = previous[relative];
    if (signature && old?.signature === signature && Number.isFinite(old.mtimeMs)) {
      fs.utimesSync(path.join(repoRoot, relative), stat.atimeMs / 1000, old.mtimeMs / 1000);
      restored++;
    }
    current[relative] = { signature, mtimeMs: fs.statSync(path.join(repoRoot, relative)).mtimeMs };
  }
  for (const relative of tracked) {
    const absolute = path.join(repoRoot, relative);
    if (!fs.existsSync(absolute)) continue;
    const stat = fs.lstatSync(absolute);
    // Symlinks and their parents retain checkout timestamps. Do not follow them.
    if (stat.isFile()) record(relative, hash(fs.readFileSync(absolute)) + ':' + stat.mode, stat);
    let parent = path.posix.dirname(relative);
    while (parent !== '.') {
      directories.add(parent);
      parent = path.posix.dirname(parent);
    }
  }
  // Build scripts may watch entire directories (e.g. SQL migrations). Restore a
  // directory only if every actual child is accounted for and unchanged.
  for (const relative of [...directories].sort((a, b) => b.split('/').length - a.split('/').length)) {
    const absolute = path.join(repoRoot, relative);
    const stat = fs.lstatSync(absolute);
    if (!stat.isDirectory()) continue;
    const children = fs.readdirSync(absolute).sort();
    const entries = children.map(name => [name, current[`${relative}/${name}`]?.signature]);
    const signature = entries.every(([, value]) => value)
      ? hash(JSON.stringify(entries)) + ':' + stat.mode : null;
    record(relative, signature, stat);
  }
  fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
  // Capture before compilation; saved with target only after a successful build.
  fs.writeFileSync(manifestPath, JSON.stringify(current));
  return { restored, inputs: Object.keys(current).length };
}

if (require.main === module) {
  if (!process.argv[2]) throw new Error('Usage: restore-cargo-inputs.js <target-cache>/cargo-inputs.json');
  console.log(JSON.stringify(restoreCargoInputs(process.cwd(), path.resolve(process.argv[2]))));
}
module.exports = { restoreCargoInputs };
