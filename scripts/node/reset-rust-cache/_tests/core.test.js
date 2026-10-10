const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildRustWarmupPlan,
  runRustCacheReset,
} = require('../core.js');

test('buildRustWarmupPlan builds the dev-up binary before warming all tests with configured resources', () => {
  assert.deepEqual(buildRustWarmupPlan({ cargoJobs: 3, incremental: true }), [
    {
      label: 'api-server dev-up target',
      args: [
        'build', '--manifest-path', 'api/Cargo.toml', '-p', 'api-server', '--bin', 'api-server',
        '--features', 'tikv-jemallocator/stats', '--locked', '--timings',
      ],
      env: { CARGO_BUILD_JOBS: '3', CARGO_INCREMENTAL: '1' },
    },
    {
      label: 'workspace test targets',
      args: [
        'test', '--manifest-path', 'api/Cargo.toml', '--workspace', '--all-targets', '--no-run', '--locked', '--timings',
      ],
      env: {
        CARGO_BUILD_JOBS: '3',
        CARGO_INCREMENTAL: '1',
        CARGO_PROFILE_TEST_DEBUG: '0',
      },
    },
  ]);
});

test('runRustCacheReset cleans first and warms every stage in order', async () => {
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
    'workspace test targets:2:0',
  ]);
});

test('runRustCacheReset stops after the first failed warmup stage', async () => {
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
      return label === 'api-server dev-up target' ? 7 : 0;
    },
    writeStdout: () => {},
  });

  assert.equal(status, 7);
  assert.deepEqual(events, ['clean', 'api-server dev-up target']);
});

test('test warmup waits for the runtime build to finish', async () => {
  const events = [];
  let finishRuntime;
  const runtimeFinished = new Promise((resolve) => { finishRuntime = resolve; });
  let runtimeStarted;
  const started = new Promise((resolve) => { runtimeStarted = resolve; });
  const pending = runRustCacheReset({
    repoRoot: '/repo',
    loadRuntimeConfigImpl: () => ({ backend: { cargoJobs: 2, incremental: true } }),
    cleanupBackendCacheImpl: async () => 0,
    runCargoImpl: async ({ label }) => {
      events.push(label);
      if (label === 'api-server dev-up target') {
        runtimeStarted();
        await runtimeFinished;
      }
      return 0;
    },
    writeStdout: () => {},
  });
  await started;
  try {
    assert.deepEqual(events, ['api-server dev-up target']);
  } finally {
    finishRuntime();
    await pending;
  }
  assert.deepEqual(events, ['api-server dev-up target', 'workspace test targets']);
});
