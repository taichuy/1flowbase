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
  assert.deepEqual(packages.slice(0, 6), [
    'api-server', 'api-host-kernel', 'api-public-gateway', 'api-applications', 'api-console-settings', 'api-mcp',
  ], 'all six current API owners must be selected in registry order');
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


test('managed MCP CI filters select their actual library owner and retain Root lifecycle tests', () => {
  const workflow = fs.readFileSync(path.join(repoRoot, '.github/workflows/browser-managed-plugin.yml'), 'utf8');
  for (const [label, filter, source, mount] of [
    ['mcp-http', 'managed_service_mcp', '_tests/mcp_routes/managed_service_mcp.rs', 'managed_service_mcp.rs'],
    ['mcp-media', 'result_delivery::media::tests', 'routes/mcp_protocol/result_delivery/_tests/media.rs', 'routes/mcp_protocol/result_delivery/media.rs'],
    ['mcp-errors', 'target_error_diagnostics', 'routes/mcp_protocol/_tests/target_error_diagnostics.rs', 'routes/mcp_protocol/virtual_ui.rs'],
  ]) {
    const lines = workflow.split('\n').map(line => line.trim()).filter(line => line.startsWith(`run_test ${label} `));
    assert.deepEqual(lines, [`run_test ${label} -p api-mcp --lib ${filter}`]);
    const sourceRoot = path.join(repoRoot, 'api/crates/api/mcp/src');
    assert.match(fs.readFileSync(path.join(sourceRoot, source), 'utf8'), /#\[(?:tokio::)?test\]/u);
    assert.ok(fs.readFileSync(path.join(sourceRoot, mount), 'utf8').includes(`#[path = "${source.startsWith('routes/') ? '_tests/' + path.basename(source) : source}"]`));
  }
  assert.ok(workflow.includes('run_test lifecycle -p api-server --lib managed_services::'));
});

test('pricing CI watches the current Settings implementation and Root billing integration tests', () => {
  const workflow = fs.readFileSync(path.join(repoRoot, '.github/workflows/model-pricing-pagination.yml'), 'utf8');
  assert.ok(workflow.includes("'api/crates/api/console-settings/src/routes/settings/billing*'"));
  assert.ok(workflow.includes("'api/crates/api/console-settings/src/routes/settings/billing*/**'"));
  assert.equal(fs.existsSync(path.join(repoRoot, 'api/crates/api/console-settings/src/routes/settings/billing.rs')), true);
  assert.ok(workflow.includes("'api/apps/api-server/src/_tests/system_settings/billing_routes/**'"));
  assert.ok(workflow.includes('-p api-server --test api_private_contracts _tests::billing_routes'));
});
