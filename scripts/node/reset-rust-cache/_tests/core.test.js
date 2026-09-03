const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildRustWarmupPlan,
  runRustCacheReset,
} = require('../core.js');

test('buildRustWarmupPlan covers workspace dev, dev-up runtime, and canonical test profile', () => {
  assert.deepEqual(buildRustWarmupPlan({ cargoJobs: 3, incremental: true }), [
    {
      label: 'workspace dev targets',
      args: ['build', '--manifest-path', 'api/Cargo.toml', '--workspace', '--all-targets', '--locked'],
      env: { CARGO_BUILD_JOBS: '3', CARGO_INCREMENTAL: '1' },
    },
    {
      label: 'api-server dev-up target',
      args: [
        'build', '--manifest-path', 'api/Cargo.toml', '-p', 'api-server', '--bin', 'api-server', '--locked',
      ],
      env: { CARGO_BUILD_JOBS: '3', CARGO_INCREMENTAL: '1' },
    },
    {
      label: 'workspace test targets',
      args: [
        'test', '--manifest-path', 'api/Cargo.toml', '--workspace', '--all-targets', '--no-run', '--locked',
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
    'workspace dev targets:2:0',
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
      return label === 'workspace dev targets' ? 7 : 0;
    },
    writeStdout: () => {},
  });

  assert.equal(status, 7);
  assert.deepEqual(events, ['clean', 'workspace dev targets']);
});
