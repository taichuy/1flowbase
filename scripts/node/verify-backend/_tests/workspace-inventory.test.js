const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { BACKEND_SHARDS, BACKEND_CI_TEST_SHARDS } = require('../../verify/backend-targets.js');

const repoRoot = path.resolve(__dirname, '../../../..');
// Current workspace uses explicit member paths and literal package names. Fail closed
// if that shape changes; Cargo compilation alone cannot prove own-target coverage.
function workspacePackages() {
  const manifest = fs.readFileSync(path.join(repoRoot, 'api/Cargo.toml'), 'utf8');
  const members = manifest.match(/members\s*=\s*\[([\s\S]*?)\]/u);
  assert.ok(members, 'workspace must declare explicit members');
  return [...members[1].matchAll(/"([^"]+)"/gu)].map(([, member]) => {
    const cargo = fs.readFileSync(path.join(repoRoot, 'api', member, 'Cargo.toml'), 'utf8');
    const name = cargo.match(/\[package\]\s*\nname\s*=\s*"([^"]+)"/u);
    assert.ok(name, `missing package identity: ${member}`);
    return name[1];
  });
}

function assertCovered(members, shards) {
  const covered = new Set(shards.flatMap((shard) => shard.packages));
  assert.deepEqual([...covered].sort(), [...members].sort(), 'workspace targets must all be covered');
}

test('check/clippy and CI tests cover every current Rust workspace member', () => {
  const members = workspacePackages();
  assertCovered(members, BACKEND_SHARDS);
  assertCovered(members, BACKEND_CI_TEST_SHARDS);
});

test('workspace inventory rejects a new unassigned member and stale package target', () => {
  const members = workspacePackages();
  assert.throws(() => assertCovered([...members, 'new-unassigned-crate'], BACKEND_SHARDS));
  assert.throws(() => assertCovered(members.filter((name) => name !== 'interface-runtime'), BACKEND_SHARDS));
});
