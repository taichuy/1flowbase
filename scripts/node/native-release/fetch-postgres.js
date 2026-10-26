#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const sources = require('./postgres-sources.json');

function fetchPostgres(target, destination) {
  const source = sources.targets[target];
  if (!source) throw new Error(`No pinned PostgreSQL bundle for ${target}`);
  const directory = path.resolve(destination);
  if (fs.existsSync(directory)) throw new Error(`Destination already exists: ${directory}`);
  fs.mkdirSync(directory, { recursive: true });
  const archive = path.join(directory, 'postgres.tar.gz');
  const download = spawnSync(process.platform === 'win32' ? 'curl.exe' : 'curl', ['--fail', '--location', '--retry', '2', '--connect-timeout', '20', '--max-time', '300', '--output', archive, source.url], { stdio: 'inherit' });
  if (download.error || download.status !== 0) throw download.error || new Error('PostgreSQL download failed');
  const digest = crypto.createHash('sha256').update(fs.readFileSync(archive)).digest('hex');
  if (digest !== source.sha256) throw new Error('PostgreSQL archive checksum mismatch');
  // Git Bash GNU tar interprets a drive-letter archive path as host:file.
  // A relative archive name also works with the system tar on macOS/Linux.
  const unpack = spawnSync('tar', ['-xzf', path.basename(archive)], { cwd: directory, stdio: 'inherit' });
  if (unpack.error || unpack.status !== 0) throw unpack.error || new Error('PostgreSQL extraction failed');
  fs.unlinkSync(archive);
  // Upstream archives may wrap their relocatable install tree in one directory.
  const suffix = target.startsWith('windows-') ? '.exe' : '';
  const roots = [directory, ...fs.readdirSync(directory, { withFileTypes: true }).filter(entry => entry.isDirectory()).map(entry => path.join(directory, entry.name))];
  const root = roots.find(candidate => fs.existsSync(path.join(candidate, 'bin', 'pg_ctl' + suffix)));
  if (!root) throw new Error('Downloaded bundle has no PostgreSQL bin directory');
  fs.writeFileSync(path.join(root, '1flowbase-postgres-source.json'), JSON.stringify({ version: sources.version, ...source }, null, 2) + '\n');
  return root;
}
if (require.main === module) {
  try { console.log(fetchPostgres(process.argv[2], process.argv[3])); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { fetchPostgres };
