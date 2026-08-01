const test = require('node:test');
const assert = require('node:assert/strict');

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  BACKEND_CONSISTENCY_GROUPS,
  BACKEND_CONSISTENCY_TARGETS,
} = require('../../verify/index.js');
const {
  buildCommands,
  main,
  runBackendConsistencyCommandSequence,
} = require('../../cli/verify-backend-consistency.js');

test('buildCommands targets backend consistency suites without workspace-wide reruns', () => {
  const commands = buildCommands({ cargoJobs: 4, cargoTestThreads: 1 });

  assert.deepEqual(
    commands.map((command) => [command.label, command.args[2], command.args[5]]),
    BACKEND_CONSISTENCY_TARGETS.map((target) => [target.label, target.packageName, target.filter])
  );

  for (const command of commands) {
    assert.equal(command.command, 'cargo');
    assert.equal(command.cwd, 'api');
    assert.deepEqual(command.env, {
      CARGO_BUILD_JOBS: '4',
      CARGO_INCREMENTAL: '0',
    });
    assert.deepEqual(command.args.slice(0, 2), ['test', '-p']);
    assert.deepEqual(command.args.slice(3, 5), ['--jobs', '4']);
    assert.deepEqual(command.args.slice(6), ['--', '--test-threads=1']);
  }
});

test('buildCommands partitions the consistency inventory without overlap or omission', () => {
  const groupedLabels = BACKEND_CONSISTENCY_GROUPS.flatMap((group) => (
    buildCommands({ cargoJobs: 4, cargoTestThreads: 1, group })
      .map((command) => command.label)
  ));

  assert.deepEqual(
    groupedLabels.slice().sort(),
    BACKEND_CONSISTENCY_TARGETS.map((target) => target.label).sort()
  );
  assert.equal(new Set(groupedLabels).size, BACKEND_CONSISTENCY_TARGETS.length);
});

test('main routes backend consistency through the heavy managed gate', async () => {
  let capturedOptions = null;

  const status = await main([], {
    repoRoot: '/repo-root',
    env: {},
    runtimeConfig: {
      backend: {
        cargoJobs: 2,
        cargoTestThreads: 1,
      },
      locks: {
        waitTimeoutMinutes: 30,
        waitTimeoutMs: 30 * 60 * 1000,
        pollIntervalMs: 5000,
      },
    },
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, 'verify-backend-consistency');
  assert.equal(capturedOptions.lockMode, 'heavy');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/cli/verify-backend-consistency.js');
  assert.deepEqual(
    capturedOptions.commands,
    buildCommands({ cargoJobs: 2, cargoTestThreads: 1 })
  );
});

test('main routes one consistency group through an independently reportable scope', async () => {
  let capturedOptions = null;

  const status = await main(['storage'], {
    repoRoot: '/repo-root',
    env: {},
    runtimeConfig: {
      backend: { cargoJobs: 2, cargoTestThreads: 1 },
      locks: { waitTimeoutMinutes: 30, waitTimeoutMs: 1_800_000, pollIntervalMs: 5000 },
    },
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, 'verify-backend-consistency-storage');
  assert.equal(
    capturedOptions.commandDisplay,
    'node scripts/node/cli/verify-backend-consistency.js storage'
  );
  assert.deepEqual(
    capturedOptions.commands.map((command) => command.label),
    BACKEND_CONSISTENCY_TARGETS
      .filter((target) => target.group === 'storage')
      .map((target) => target.label)
  );
});

test('runBackendConsistencyCommandSequence writes per-target result evidence', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-backend-consistency-results-'));
  let now = 1000;
  const commands = buildCommands({ cargoJobs: 2, cargoTestThreads: 1 }).slice(0, 2);

  const status = runBackendConsistencyCommandSequence({
    repoRoot,
    env: {},
    scope: 'verify-backend-consistency',
    commands,
    nowImpl: () => {
      now += 1250;
      return now;
    },
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: 'test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s\n',
        stderr: '',
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);

  const reportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'backend-consistency-targets.json'
  );
  const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));

  assert.deepEqual(report.targets.map((target) => ({
    label: target.label,
    status: target.status,
    durationMs: target.durationMs,
    passedCount: target.passedCount,
    failedCount: target.failedCount,
  })), [
    {
      label: commands[0].label,
      status: 'passed',
      durationMs: 1250,
      passedCount: 3,
      failedCount: 0,
    },
    {
      label: commands[1].label,
      status: 'passed',
      durationMs: 1250,
      passedCount: 3,
      failedCount: 0,
    },
  ]);
});

test('runBackendConsistencyCommandSequence records failed and skipped target states', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-backend-consistency-failure-'));
  const commands = buildCommands({ cargoJobs: 2, cargoTestThreads: 1 }).slice(0, 3);
  let callCount = 0;

  const status = runBackendConsistencyCommandSequence({
    repoRoot,
    env: {},
    scope: 'verify-backend-consistency',
    commands,
    nowImpl: () => 0,
    spawnSyncImpl() {
      callCount += 1;

      if (callCount === 1) {
        return {
          status: 0,
          stdout: 'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n',
          stderr: '',
        };
      }

      return {
        status: 101,
        stdout: 'test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\n',
        stderr: 'error: test failed\n',
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 101);
  assert.equal(callCount, 2);

  const report = JSON.parse(fs.readFileSync(path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'backend-consistency-targets.json'
  ), 'utf8'));

  assert.deepEqual(report.targets.map((target) => ({
    status: target.status,
    passedCount: target.passedCount,
    failedCount: target.failedCount,
  })), [
    { status: 'passed', passedCount: 2, failedCount: 0 },
    { status: 'failed', passedCount: 1, failedCount: 1 },
    { status: 'skipped', passedCount: null, failedCount: null },
  ]);
});
