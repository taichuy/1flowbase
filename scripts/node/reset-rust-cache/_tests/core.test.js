const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildRustWarmupPlan,
  runRustCacheReset,
} = require('../core.js');

test('buildRustWarmupPlan prepares the test-profile library before all targets with configured jobs', () => {
  assert.deepEqual(buildRustWarmupPlan({ cargoJobs: 2, incremental: true }), [
    {
      label: 'api-server dev-up target',
      args: [
        'build', '--manifest-path', 'api/Cargo.toml', '-p', 'api-server', '--bin', 'api-server',
        '--features', 'tikv-jemallocator/stats', '--locked', '--timings',
      ],
      env: { CARGO_BUILD_JOBS: '2', CARGO_INCREMENTAL: '1' },
    },
    {
      label: 'workspace test-profile libraries and binary tests',
      args: [
        'test', '--manifest-path', 'api/Cargo.toml', '--workspace', '--bins', '--no-run', '--locked', '--timings',
      ],
      env: {
        CARGO_BUILD_JOBS: '2',
        CARGO_INCREMENTAL: '1',
        CARGO_PROFILE_TEST_DEBUG: '0',
      },
    },
    {
      label: 'workspace test targets',
      args: [
        'test', '--manifest-path', 'api/Cargo.toml', '--workspace', '--all-targets', '--no-run', '--locked', '--timings',
      ],
      env: {
        CARGO_BUILD_JOBS: '2',
        CARGO_INCREMENTAL: '1',
        CARGO_PROFILE_TEST_DEBUG: '0',
      },
    },
  ]);
});

test('runRustCacheReset runs each stage in order with the configured compilation budget', async () => {
  const events = [];
  const status = await runRustCacheReset({
    repoRoot: '/repo',
    loadRuntimeConfigImpl: () => ({ backend: { cargoJobs: 2, incremental: false } }),
    cleanupBackendCacheImpl: async () => {
      events.push('clean');
      return 0;
    },
    runCargoImpl: async ({ label, env }) => {
      events.push(`${label}:${env.CARGO_BUILD_JOBS}:${env.CARGO_INCREMENTAL}`);
      return 0;
    },
    writeStdout: () => {},
  });

  assert.equal(status, 0);
  assert.deepEqual(events, [
    'clean',
    'api-server dev-up target:2:0',
    'workspace test-profile libraries and binary tests:2:0',
    'workspace test targets:2:0',
  ]);
});

test('runRustCacheReset does not start all targets when test-profile preparation fails', async () => {
  const events = [];
  const status = await runRustCacheReset({
    repoRoot: '/repo',
    loadRuntimeConfigImpl: () => ({ backend: { cargoJobs: 2, incremental: true } }),
    cleanupBackendCacheImpl: async () => {
      events.push('clean');
      return 0;
    },
    runCargoImpl: async ({ label }) => {
      events.push(label);
      return label === 'workspace test-profile libraries and binary tests' ? 7 : 0;
    },
    writeStdout: () => {},
  });

  assert.equal(status, 7);
  assert.deepEqual(events, [
    'clean', 'api-server dev-up target', 'workspace test-profile libraries and binary tests',
  ]);
});

test('all-target warmup waits for both the runtime build and test-profile preparation', async () => {
  const events = [];
  let finishRuntime;
  const runtimeFinished = new Promise((resolve) => { finishRuntime = resolve; });
  let runtimeStarted;
  const started = new Promise((resolve) => { runtimeStarted = resolve; });
  let finishPreparation;
  const preparationFinished = new Promise((resolve) => { finishPreparation = resolve; });
  let preparationStarted;
  const preparing = new Promise((resolve) => { preparationStarted = resolve; });
  const pending = runRustCacheReset({
    repoRoot: '/repo',
    loadRuntimeConfigImpl: () => ({ backend: { cargoJobs: 2, incremental: true } }),
    cleanupBackendCacheImpl: async () => 0,
    runCargoImpl: async ({ label }) => {
      events.push(label);
      if (label === 'api-server dev-up target') {
        runtimeStarted();
        await runtimeFinished;
      } else if (label === 'workspace test-profile libraries and binary tests') {
        preparationStarted();
        await preparationFinished;
      }
      return 0;
    },
    writeStdout: () => {},
  });
  await started;
  try {
    assert.deepEqual(events, ['api-server dev-up target']);
    finishRuntime();
    await Promise.race([
      preparing,
      pending.then(() => assert.fail('warmup completed without test-profile preparation')),
    ]);
    assert.deepEqual(events, [
      'api-server dev-up target', 'workspace test-profile libraries and binary tests',
    ]);
  } finally {
    finishRuntime();
    finishPreparation();
    await pending;
  }
  assert.deepEqual(events, [
    'api-server dev-up target', 'workspace test-profile libraries and binary tests', 'workspace test targets',
  ]);
});
