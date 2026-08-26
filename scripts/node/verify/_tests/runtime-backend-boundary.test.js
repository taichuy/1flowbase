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
