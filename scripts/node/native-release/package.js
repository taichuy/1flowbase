#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function requiredFile(file) {
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) throw new Error(`Missing package input: ${file}`);
}
function packageName(platform, arch) {
  if (!['linux', 'macos', 'windows'].includes(platform) || !['x64', 'arm64'].includes(arch)) {
    throw new Error('Expected platform linux/macos/windows and arch x64/arm64');
  }
  return `1flowbase-${platform}-${arch}`;
}
function stagePackage({ repoRoot, apiBinary, recoveryBinary, webDir, postgresDir, outDir, platform, arch, resourcesDir }) {
  const name = packageName(platform, arch);
  const suffix = platform === 'windows' ? '.exe' : '';
  for (const input of [apiBinary, recoveryBinary, path.join(webDir, 'index.html'), ...['postgres', 'pg_ctl', 'initdb', 'psql', 'createdb', 'pg_dump', 'pg_restore'].map(tool => path.join(postgresDir, 'bin', tool + suffix))]) requiredFile(input);
  const stage = path.join(path.resolve(outDir), name);
  // Refuse to erase an existing package or a user's accidentally selected data directory.
  fs.mkdirSync(path.dirname(stage), { recursive: true });
  fs.mkdirSync(stage);
  fs.copyFileSync(apiBinary, path.join(stage, '1flowbase' + suffix));
  fs.copyFileSync(recoveryBinary, path.join(stage, 'system_recovery' + suffix));
  fs.cpSync(webDir, path.join(stage, 'web'), { recursive: true, dereference: true });
  fs.cpSync(postgresDir, path.join(stage, 'postgres'), { recursive: true, dereference: true });
  for (const directory of ['host-extensions', 'sets', 'capability-plugins']) {
    fs.cpSync(path.join(repoRoot, 'api/plugins', directory), path.join(stage, 'api/plugins', directory), { recursive: true, dereference: true });
  }
  if (resourcesDir) fs.cpSync(resourcesDir, path.join(stage, 'api/resources'), { recursive: true, dereference: true });
  for (const file of ['README.md', 'README_CN.md']) fs.copyFileSync(path.join(repoRoot, 'docker/native', file), path.join(stage, file));
  fs.copyFileSync(path.join(repoRoot, 'LICENSE'), path.join(stage, 'LICENSE'));
  const manifest = { version: JSON.parse(fs.readFileSync(path.join(repoRoot, 'web/app/package.json'), 'utf8')).version, platform, arch, http_port: 7800, postgres_port: 35432, postgres_major: 18 };
  fs.writeFileSync(path.join(stage, 'native-distribution.json'), JSON.stringify(manifest, null, 2) + '\n');
  if (platform !== 'windows') {
    fs.chmodSync(path.join(stage, '1flowbase'), 0o755);
    fs.chmodSync(path.join(stage, 'system_recovery'), 0o755);
    fs.writeFileSync(path.join(stage, 'start.sh'), '#!/bin/sh\nset -eu\npackage_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$package_dir/1flowbase" "$@"\n', { mode: 0o755 });
  }
  return stage;
}
function archivePackage(stage, platform) {
  const archive = stage + (platform === 'windows' ? '.zip' : '.tar.gz');
  if (fs.existsSync(archive)) throw new Error(`Archive already exists: ${archive}`);
  let result;
  if (platform === 'windows' && process.platform === 'win32') {
    result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', 'Compress-Archive -LiteralPath $env:FLOWBASE_PACKAGE_STAGE -DestinationPath $env:FLOWBASE_PACKAGE_ARCHIVE'], { env: { ...process.env, FLOWBASE_PACKAGE_STAGE: stage, FLOWBASE_PACKAGE_ARCHIVE: archive }, stdio: 'inherit' });
  } else {
    result = platform === 'windows'
      ? spawnSync('zip', ['-qr', archive, path.basename(stage)], { cwd: path.dirname(stage), stdio: 'inherit' })
      : spawnSync('tar', ['-czf', archive, '-C', path.dirname(stage), path.basename(stage)], { stdio: 'inherit' });
  }
  if (result.error || result.status !== 0) throw result.error || new Error('Archive creation failed');
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
    const stage = stagePackage({ repoRoot: path.resolve(__dirname, '../../..'), apiBinary: values['api-binary'], recoveryBinary: values['recovery-binary'], webDir: values['web-dir'], postgresDir: values['postgres-dir'], outDir: values.out, platform: values.platform, arch: values.arch, resourcesDir: values['resources-dir'] });
    console.log(archivePackage(stage, values.platform));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { packageName, stagePackage, archivePackage };
