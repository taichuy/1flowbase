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
    backendThresholds.some((threshold) => threshold.packageName === 'plugin-runner'),
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
});

test('backend coverage uses the current storage-postgres crate name', () => {
  const storageCommand = buildBackendCommands({
    repoRoot: '/repo-root',
    cargoParallelism: 4,
    cargoTestThreads: 2,
  }).find((command) => command.label === 'backend-coverage-storage-postgres');

  assert.ok(storageCommand);
  assert.equal(storageCommand.args[2], 'storage-postgres');
  assert.match(
    storageCommand.args[6].replace(/\\/gu, '/'),
    /tmp\/test-governance\/coverage\/backend\/storage-postgres\.json$/u
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
