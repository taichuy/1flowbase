const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { BACKEND_SHARDS } = require('../../verify/backend-targets.js');
const { buildBackendCommands } = require('../../verify/backend.js');
const { buildCoverageBackendCommands } = require('../../verify/coverage.js');
const { DEFAULT_AGGREGATE_SCOPES } = require('../../github-quality-gate/commands.js');

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
  const options = { repoRoot, cargoJobs: 2, cargoParallelism: 2, cargoTestThreads: 2, env: { QUALITY_GATE_SCOPE: 'ci' } };
  const commands = DEFAULT_AGGREGATE_SCOPES.filter((scope) => scope.startsWith('repo-backend-test-'))
    .flatMap((scope) => buildBackendCommands({ ...options, target: 'test', shard: scope.replace('repo-backend-test-', '') }));
  commands.push(...buildCoverageBackendCommands(options));
  assertCovered(members, commands.map(({ args }) => ({
    packages: args.filter((value, index) => args[index - 1] === '--package'),
  })));
});

test('workspace inventory rejects a new unassigned member and stale package target', () => {
  const members = workspacePackages();
  assert.throws(() => assertCovered([...members, 'new-unassigned-crate'], BACKEND_SHARDS));
  assert.throws(() => assertCovered(members.filter((name) => name !== 'interface-runtime'), BACKEND_SHARDS));
});

test('API app checks and every hash shard select all actual API owners', () => {
  const packages = require('../../testing/contracts/api-source-owners.js').apiSourceOwners(repoRoot).map((owner) => owner.packageName);
  for (const shard of ['apps', ...Array.from({ length: 4 }, (_, index) => `api-server-${index + 1}-of-4`)]) {
    const [command] = buildBackendCommands({ repoRoot, cargoJobs: 2, cargoTestThreads: 2, target: shard === 'apps' ? 'clippy' : 'test', shard });
    const selected = command.args.filter((_arg, index) => command.args[index - 1] === '--package');
    for (const name of packages) assert.ok(selected.includes(name), `${shard} must select ${name}`);
    if (shard !== 'apps') assert.deepEqual(selected, packages);
  }
  // A fixture containing only Root cannot accidentally issue absent API packages.
  const [command] = buildBackendCommands({ repoRoot: '/repo-root', cargoJobs: 2, cargoTestThreads: 2, target: 'test', shard: 'api-server-1-of-4' });
  assert.deepEqual(command.args.filter((_arg, index) => command.args[index - 1] === '--package'), ['api-server']);
});
