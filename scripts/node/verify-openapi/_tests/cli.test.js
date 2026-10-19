const test = require('node:test');
const assert = require('node:assert/strict');

const { buildCommands, main } = require('../../cli/quality/verify-openapi.js');

test('buildCommands targets api-server OpenAPI alignment tests', () => {
  assert.deepEqual(buildCommands({ cargoJobs: 3, cargoTestThreads: 1 }), [
    {
      label: 'openapi-alignment',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'api-server',
        '--jobs',
        '3',
        'openapi',
        '--',
        '--test-threads=1',
      ],
      cwd: 'api',
      env: {
        CARGO_BUILD_JOBS: '3',
        CARGO_INCREMENTAL: '0',
      },
    },
  ]);
});

test('main routes OpenAPI verification through the heavy managed gate', async () => {
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
  assert.equal(capturedOptions.scope, 'verify-openapi');
  assert.equal(capturedOptions.lockMode, 'heavy');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/cli/quality/verify-openapi.js');
  assert.deepEqual(
    capturedOptions.commands,
    buildCommands({ cargoJobs: 2, cargoTestThreads: 1 })
  );
});
