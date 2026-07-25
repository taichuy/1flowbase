#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const { loadLock } = require('./lock');

function probeVersions(options = {}) {
  const lock = loadLock(options.lockPath);
  const npmView = options.npmView ?? ((name) => execFileSync(
    'npm', ['view', name, 'version', '--json'],
    { encoding: 'utf8', maxBuffer: 1024 * 1024 },
  ).trim());
  const packages = Object.fromEntries(Object.entries(lock.packages).filter(([, spec]) => spec.canary !== false).map(([key, spec]) => {
    const parsed = JSON.parse(npmView(spec.name));
    const latest = Array.isArray(parsed) ? parsed.at(-1) : parsed;
    return [key, { name: spec.name, pinned: spec.version, latest, update_available: latest !== spec.version }];
  }));
  return {
    schema_version: '1flowbase.ai-gateway-client-canary/v1',
    blocking_lock_changed: false,
    checked_at: new Date().toISOString(),
    packages,
  };
}

function main() {
  const root = path.resolve('tmp/test-governance/ai-gateway-client-canary');
  fs.mkdirSync(root, { recursive: true, mode: 0o700 });
  fs.writeFileSync(path.join(root, 'versions.json'), `${JSON.stringify(probeVersions(), null, 2)}\n`, { mode: 0o600 });
}

if (require.main === module) main();

module.exports = { probeVersions };
