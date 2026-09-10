const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');

test('AC-001 API image assembly does not download or bundle provider packages', () => {
  const dockerfile = fs.readFileSync(path.join(repoRoot, 'docker/api-server.Dockerfile'), 'utf8');
  assert.doesNotMatch(dockerfile, /default-extension|package-default-extension|plugins\/bootstrap/u);
});

test('AC-002 API startup has no default provider installation entry point', () => {
  const startup = fs.readFileSync(path.join(repoRoot, 'api/apps/api-server/src/lib.rs'), 'utf8');
  assert.doesNotMatch(startup, /extension_bootstrap|bootstrap_locked_extensions/u);
});
