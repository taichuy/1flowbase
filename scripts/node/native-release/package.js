#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const { nativeEnvironment, maintenanceScripts } = require('./config.js');

function requiredFile(file) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) throw new Error(`Missing package input: ${file}`);
}
function packageName(platform, arch) {
  if (!['linux', 'macos', 'windows'].includes(platform) || !['x64', 'arm64'].includes(arch)) {
    throw new Error('Expected platform linux/macos/windows and arch x64/arm64');
  }
  return `1flowbase-${platform}-${arch}`;
}
// Node's native cpSync path can ignore dereference (nodejs/node#59168).
// A filter uses its JS traversal, which copies linked inputs as regular files.
function copyInputs(source, destination) {
  fs.cpSync(source, destination, { recursive: true, dereference: true, filter: () => true });
}
function stagePackage({ repoRoot, apiBinary, recoveryBinary, webDir, postgresDir, outDir, platform, arch, resourcesDir, version }) {
  const name = packageName(platform, arch);
  const suffix = platform === 'windows' ? '.exe' : '';
  for (const input of [apiBinary, recoveryBinary, path.join(webDir, 'index.html'), ...['postgres', 'pg_ctl', 'initdb', 'psql', 'createdb', 'pg_dump', 'pg_restore'].map(tool => path.join(postgresDir, 'bin', tool + suffix))]) requiredFile(input);
  const stage = path.join(path.resolve(outDir), name);
  // Refuse to erase an existing package or a user's accidentally selected data directory.
  fs.mkdirSync(path.dirname(stage), { recursive: true });
  fs.mkdirSync(stage);
  fs.copyFileSync(apiBinary, path.join(stage, '1flowbase' + suffix));
  fs.copyFileSync(recoveryBinary, path.join(stage, 'system_recovery' + suffix));
  copyInputs(webDir, path.join(stage, 'web'));
  copyInputs(postgresDir, path.join(stage, 'postgres'));
  for (const directory of ['host-extensions', 'sets', 'capability-plugins']) {
    copyInputs(path.join(repoRoot, 'api/plugins', directory), path.join(stage, 'api/plugins', directory));
  }
  if (resourcesDir) copyInputs(resourcesDir, path.join(stage, 'api/resources'));
  for (const file of ['README.md', 'README_CN.md']) fs.copyFileSync(path.join(repoRoot, 'docker/native', file), path.join(stage, file));
  fs.copyFileSync(path.join(repoRoot, 'LICENSE'), path.join(stage, 'LICENSE'));
  const environment = nativeEnvironment(repoRoot, platform);
  fs.writeFileSync(path.join(stage, '.env'), environment);
  fs.writeFileSync(path.join(stage, '.env.example'), environment);
  for (const [name, content] of Object.entries(maintenanceScripts(platform))) fs.writeFileSync(path.join(stage, name), content, { mode: 0o755 });
  const manifest = { format_version: 1, config_schema: 1, version: version || JSON.parse(fs.readFileSync(path.join(repoRoot, 'web/app/package.json'), 'utf8')).version, platform, arch, http_port: 7800, postgres_port: 35432, postgres_major: 18 };
  if (platform !== 'windows') {
    fs.chmodSync(path.join(stage, '1flowbase'), 0o755);
    fs.chmodSync(path.join(stage, 'system_recovery'), 0o755);
    fs.writeFileSync(path.join(stage, 'start.sh'), '#!/bin/sh\nset -eu\npackage_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$package_dir/1flowbase" "$@"\n', { mode: 0o755 });
  }
  manifest.files = {};
  function inventory(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) inventory(file);
      else {
        if (!entry.isFile()) throw new Error(`Package input must be a regular file: ${file}`);
        const relative = path.relative(stage, file).split(path.sep).join('/');
        if (relative !== '.env') manifest.files[relative] = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
      }
    }
  }
  inventory(stage);
  manifest.build_id = crypto.createHash('sha256').update(JSON.stringify({ version: manifest.version, platform, arch, files: manifest.files })).digest('hex');
  fs.writeFileSync(path.join(stage, 'native-distribution.json'), JSON.stringify(manifest, null, 2) + '\n');
  return stage;
}
function archivePackage(stage, platform) {
  const archive = stage + (platform === 'windows' ? '.zip' : '.tar.gz');
  if (fs.existsSync(archive)) throw new Error(`Archive already exists: ${archive}`);
  let result;
  if (platform === 'windows' && process.platform === 'win32') {
    result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', 'Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::CreateFromDirectory($env:FLOWBASE_PACKAGE_STAGE, $env:FLOWBASE_PACKAGE_ARCHIVE, [System.IO.Compression.CompressionLevel]::Optimal, $true)'], { env: { ...process.env, FLOWBASE_PACKAGE_STAGE: stage, FLOWBASE_PACKAGE_ARCHIVE: archive }, stdio: 'inherit' });
  } else {
    result = platform === 'windows'
      ? spawnSync('zip', ['-qr', archive, path.basename(stage)], { cwd: path.dirname(stage), stdio: 'inherit' })
      : spawnSync('tar', ['-czf', archive, '-C', path.dirname(stage), path.basename(stage)], { stdio: 'inherit' });
  }
  if (result.error || result.status !== 0) throw result.error || new Error('Archive creation failed');
  let listed;
  if (platform === 'windows' && process.platform === 'win32') {
    listed = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', '[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); Add-Type -AssemblyName System.IO.Compression.FileSystem; $zip = [System.IO.Compression.ZipFile]::OpenRead($env:FLOWBASE_PACKAGE_ARCHIVE); try { $zip.Entries | ForEach-Object { $_.FullName } } finally { $zip.Dispose() }'], { env: { ...process.env, FLOWBASE_PACKAGE_ARCHIVE: archive }, encoding: 'utf8' });
  } else {
    listed = platform === 'windows'
      ? spawnSync('unzip', ['-Z1', archive], { encoding: 'utf8' })
      : spawnSync('tar', ['-tzf', archive], { encoding: 'utf8' });
  }
  if (listed.status !== 0) throw new Error('Could not verify distribution archive entries');
  const entries = new Set(listed.stdout.replaceAll('\\', '/').split(/\r?\n/));
  const manifest = JSON.parse(fs.readFileSync(path.join(stage, 'native-distribution.json'), 'utf8'));
  for (const name of [...Object.keys(manifest.files), '.env', 'native-distribution.json']) {
    if (!entries.has(`${path.basename(stage)}/${name}`)) throw new Error(`Archive omitted ${name}`);
  }
  return archive;
}
if (require.main === module) {
  try {
    const args = process.argv.slice(2);
    const values = {};
    for (let i = 0; i < args.length; i += 2) {
      if (!args[i].startsWith('--') || !args[i + 1]) throw new Error('Expected --key value pairs');
      values[args[i].slice(2)] = args[i + 1];
    }
    const stage = stagePackage({ repoRoot: path.resolve(__dirname, '../../..'), apiBinary: values['api-binary'], recoveryBinary: values['recovery-binary'], webDir: values['web-dir'], postgresDir: values['postgres-dir'], outDir: values.out, platform: values.platform, arch: values.arch, resourcesDir: values['resources-dir'], version: values.version });
    console.log(archivePackage(stage, values.platform));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { packageName, stagePackage, archivePackage };
