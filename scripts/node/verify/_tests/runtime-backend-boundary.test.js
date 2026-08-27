'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const repoRoot = path.resolve(__dirname, '../../../..');
const forbidden = /PLUGIN_RUNNER|plugin-runner|plugin_runner|PluginRunner|\b7801\b/u;
const ignoredDirectories = new Set(['_tests', 'node_modules', 'target', 'volumes']);

function productionFiles(root) {
  const files = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (ignoredDirectories.has(entry.name)) continue;
      const candidate = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(candidate);
      else if (entry.isFile()) files.push(candidate);
    }
  };
  visit(root);
  return files;
}

test('Delivery 1898 removes the standalone runtime service from production tooling', () => {
  const roots = ['scripts', 'docker', '.github'].map((directory) => path.join(repoRoot, directory));
  const violations = roots.flatMap(productionFiles).flatMap((file) => {
    const source = fs.readFileSync(file, 'utf8');
    return forbidden.test(source) ? [path.relative(repoRoot, file)] : [];
  });
  assert.deepEqual(violations, []);
});

test('Delivery 1898 keeps one Backend executable in the Cargo workspace', () => {
  const workspace = fs.readFileSync(path.join(repoRoot, 'api/Cargo.toml'), 'utf8');
  assert.match(workspace, /"apps\/api-server"/u);
  assert.doesNotMatch(workspace, forbidden);
  assert.equal(fs.existsSync(path.join(repoRoot, 'api/apps/plugin-runner')), false);
});

test('Delivery 1898 binds the production Backend Slot and hides concrete registries', () => {
  const boot = fs.readFileSync(path.join(repoRoot, 'api/apps/api-server/src/lib.rs'), 'utf8');
  const runtime = fs.readFileSync(
    path.join(repoRoot, 'api/apps/api-server/src/provider_runtime/mod.rs'),
    'utf8',
  );
  const host = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-extension-host/src/runtime_host.rs'),
    'utf8',
  );
  const contract = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-core/src/runtime_backend.rs'),
    'utf8',
  );

  assert.match(boot, /RuntimeBackendSlot::default\(\)/u);
  assert.match(boot, /runtime_backend_slot\.bind\(runtime_extension_host\.clone\(\)\)/u);
  assert.match(runtime, /orchestration_backend/u);
  assert.doesNotMatch(runtime, /\.(?:provider|data_source|capability|network_egress)_registry\(\)/u);
  assert.doesNotMatch(host, /pub fn (?:provider|data_source|capability|network_egress)_registry/u);
  assert.match(contract, /pub struct RuntimeArtifactReference/u);
  assert.doesNotMatch(contract, /pub package_root:/u);
});
