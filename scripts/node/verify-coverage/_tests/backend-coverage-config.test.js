const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildBackendCommands,
  main,
} = require('../../verify-coverage.js');
const {
  backendThresholds,
  frontendThresholds,
} = require('../../testing/coverage-thresholds.js');

test('coverage thresholds include critical runtime areas', () => {
  assert.equal(
    backendThresholds.find((threshold) => threshold.packageName === 'control-plane')?.line,
    69
  );
  assert.equal(
    backendThresholds.some((threshold) => threshold.packageName === 'runtime-extension-host'),
    true
  );
  assert.equal(
    backendThresholds.some((threshold) => threshold.packageName === 'orchestration-runtime'),
    true
  );
  assert.equal(
    frontendThresholds.some((threshold) => threshold.prefix === 'packages/page-runtime/'),
    true
  );
  assert.deepEqual(
    frontendThresholds.find((threshold) => threshold.key === 'settings')?.thresholds,
    { lines: 56, functions: 54, statements: 55, branches: 46 }
  );
});

test('backend coverage includes the migrated PostgreSQL integration harness', () => {
  const storageCommands = buildBackendCommands({
    repoRoot: '/repo-root',
    cargoParallelism: 4,
    cargoTestThreads: 2,
    backendKeys: ['storage-postgres'],
  });

  assert.equal(storageCommands.length, 2);
  const [storageCommand, integrationCommand] = storageCommands;
  assert.equal(storageCommand.label, 'backend-coverage-storage-postgres');
  assert.equal(storageCommand.args[2], 'storage-durable-postgres');
  assert.equal(
    integrationCommand.label,
    'backend-coverage-storage-postgres-control-plane-integration'
  );
  assert.deepEqual(integrationCommand.args.slice(1, 7), [
    '--package',
    'storage-durable-postgres',
    '--package',
    'control-plane-postgres-tests',
    '--exclude-from-report',
    'control-plane-postgres-tests',
  ]);
  assert.equal(integrationCommand.args.includes('dependency_boundaries'), false);
  assert.equal(integrationCommand.args.includes('orchestration_runtime_integration'), true);
  const outputPathIndex = storageCommand.args.indexOf('--output-path');
  assert.notEqual(outputPathIndex, -1);
  assert.match(
    storageCommand.args[outputPathIndex + 1],
    /tmp\/test-governance\/coverage\/backend\/storage-postgres\.json$/u
  );
});

test('control-plane coverage includes its storage-backed management service tests', () => {
  const commands = buildBackendCommands({
    repoRoot: '/repo-root',
    cargoParallelism: 4,
    cargoTestThreads: 2,
    backendKeys: ['control-plane'],
  });

  assert.deepEqual(
    commands.map((command) => command.label),
    [
      'backend-coverage-control-plane-tests',
      'backend-coverage-control-plane-mcp-management-integration',
      'backend-coverage-control-plane-ui-management-integration',
      'backend-coverage-control-plane-mcp-routes-integration',
      'backend-coverage-control-plane-ui-routes-integration',
      'backend-coverage-control-plane-doc-tests',
    ]
  );
  assert.deepEqual(
    commands[1].args.slice(-2),
    ['--', '--test-threads=2']
  );
  assert.deepEqual(
    commands[2].args.slice(-2),
    ['ui_management_repository_tests', '--test-threads=2']
  );
  assert.equal(commands[1].args.includes('--exclude-from-report'), true);
  assert.equal(commands[1].args.includes('control-plane-postgres-tests'), true);
  assert.equal(commands[1].args.includes('mcp_management_repository_tests'), false);
  const testTargetIndex = commands[1].args.indexOf('--test');
  assert.equal(commands[1].args[testTargetIndex + 1], 'mcp_management_integration');
  const root = path.resolve(__dirname, '../../../..');
  assert.ok(fs.existsSync(path.join(root, 'api/crates/control-plane-postgres-tests/tests/mcp_management_integration.rs')));
  assert.equal(commands[2].args.includes('--output-path'), true);
  assert.deepEqual(
    commands[3].args.slice(-2),
    ['mcp_management_routes', '--test-threads=2']
  );
  assert.equal(commands[3].args.includes('api-server'), true);
  assert.deepEqual(
    commands[4].args.slice(-2),
    ['ui_management_routes', '--test-threads=2']
  );
  assert.deepEqual(commands[5].args, ['test', '--package', 'control-plane', '--doc', '--', '--test-threads=2']);
});

test('backend coverage removes stale json summaries before threshold reporting', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-coverage-stale-'));
  const stalePath = path.join(repoRoot, 'tmp', 'test-governance', 'coverage', 'backend', 'storage-pg.json');

  fs.mkdirSync(path.dirname(stalePath), { recursive: true });
  fs.writeFileSync(stalePath, '{"stale":true}', 'utf8');

  const status = await main(['backend'], {
    repoRoot,
    env: {},
    runtimeConfig: {
      backend: {
        cargoJobs: 2,
        cargoTestThreads: 4,
      },
      locks: {
        waitTimeoutMinutes: 30,
        waitTimeoutMs: 30 * 60 * 1000,
        pollIntervalMs: 5000,
      },
    },
    writeStdout() {},
    writeStderr() {},
    preflightSpawnSyncImpl() {
      return { status: 0, stdout: '', stderr: '' };
    },
    spawnSyncImpl(_command, args) {
      return { status: 0, stdout: args[1] === 'clean' ? '' : 'test result: ok. 1 passed; 0 failed;', stderr: '' };
    },
    readFileSyncImpl() {
      return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
    },
  });

  assert.equal(status, 0);
  assert.equal(fs.existsSync(stalePath), false);
});

test('ordinary API coverage selects existing owners and weights the original 60 percent aggregate', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ordinary-api-coverage-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const owner of ['kernel', 'public-gateway']) {
    const manifest = path.join(root, 'api/crates/api', owner, 'Cargo.toml');
    fs.mkdirSync(path.dirname(manifest), { recursive: true });
    fs.writeFileSync(manifest, '[package]\n');
  }
  const [command] = buildBackendCommands({ repoRoot: root, cargoParallelism: 2, cargoTestThreads: 2, backendKeys: ['api-server'] });
  assert.deepEqual(command.args.flatMap((arg, i, args) => arg === '--package' ? [args[i + 1]] : []),
    ['api-server', 'api-host-kernel', 'api-public-gateway']);
  const { collectBackendCoverageFailures } = require('../../verify/coverage-report.js');
  const files = [
    ['apps/api-server', 1, 1], ['crates/api/kernel', 49, 49], ['crates/api/public-gateway', 50, 0],
  ].map(([owner, count, covered]) => ({ filename: `/repo/api/${owner}/src/lib.rs`, summary: { lines: { count, covered } } }));
  const summaries = { 'api-server': { data: [{ totals: { lines: { percent: 100 } }, files }] } };
  assert.deepEqual(collectBackendCoverageFailures(summaries, ['api-server']), [
    { key: 'api-server', metric: 'lines', expectedPct: 60, actualPct: 50 },
  ]);
  files[2].summary.lines.covered = 10;
  assert.deepEqual(collectBackendCoverageFailures(summaries, ['api-server']), []);
  const manifest = path.join(root, 'api/crates/api/applications/Cargo.toml');
  fs.mkdirSync(path.dirname(manifest), { recursive: true });
  fs.writeFileSync(manifest, '[package]\n');
  const [withApplications] = buildBackendCommands({ repoRoot: root, cargoParallelism: 2, cargoTestThreads: 2, backendKeys: ['api-server'] });
  assert.deepEqual(withApplications.args.flatMap((arg, i, args) => arg === '--package' ? [args[i + 1]] : []),
    ['api-server', 'api-host-kernel', 'api-public-gateway', 'api-applications']);
  files[2].filename = '/repo/api/crates/api/applications/src/lib.rs';
  files[2].summary.lines.covered = 0;
  assert.deepEqual(collectBackendCoverageFailures(summaries, ['api-server']), [
    { key: 'api-server', metric: 'lines', expectedPct: 60, actualPct: 50 },
  ]);
  files[2].summary.lines.covered = 10;
  assert.deepEqual(collectBackendCoverageFailures(summaries, ['api-server']), []);
});
