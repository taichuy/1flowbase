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

test('backend coverage uses the current storage durable postgres crate name', () => {
  const storageCommand = buildBackendCommands({
    repoRoot: '/repo-root',
    cargoParallelism: 4,
    cargoTestThreads: 2,
  }).find((command) => command.label === 'backend-coverage-storage-postgres');

  assert.ok(storageCommand);
  assert.equal(storageCommand.args[2], 'storage-durable-postgres');
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
    ]
  );
  assert.deepEqual(
    commands[1].args.slice(-2),
    ['mcp_management_repository_tests', '--test-threads=2']
  );
  assert.deepEqual(
    commands[2].args.slice(-2),
    ['ui_management_repository_tests', '--test-threads=2']
  );
  assert.equal(commands[1].args.includes('--exclude-from-report'), true);
  assert.equal(commands[1].args.includes('storage-durable-postgres'), true);
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
    spawnSyncImpl() {
      return { status: 0, stdout: '', stderr: '' };
    },
    readFileSyncImpl() {
      return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
    },
  });

  assert.equal(status, 0);
  assert.equal(fs.existsSync(stalePath), false);
});
