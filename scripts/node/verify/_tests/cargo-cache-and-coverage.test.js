const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildBackendConsistencyCommands } = require('../backend-consistency.js');
const { buildStateProtocolCommands, parseStateProtocolsCliArgs } = require('../state-protocols.js');
const { buildCoverageBackendCommands, parseCoverageCliArgs, runCoverage } = require('../coverage.js');

const repoRoot = path.resolve(__dirname, '../../../..');

test('backend-consistency and state-protocols preserve [profile.test] incremental settings instead of setting CARGO_INCREMENTAL=1', () => {
  const consistencyIncremental = buildBackendConsistencyCommands({
    cargoJobs: 4,
    cargoTestThreads: 2,
    incremental: true,
  });
  for (const command of consistencyIncremental) {
    assert.equal(command.env.CARGO_BUILD_JOBS, '4');
    assert.equal(command.env.CARGO_INCREMENTAL, undefined);
    assert.equal(command.env.CARGO_PROFILE_TEST_INCREMENTAL, 'true');
  }

  const stateProtocolsIncremental = buildStateProtocolCommands({
    repoRoot,
    env: {},
    cargoJobs: 4,
    cargoTestThreads: 2,
    incremental: true,
    options: parseStateProtocolsCliArgs(['--skip-live-acp']),
  });
  const anthropicCommand = stateProtocolsIncremental.find(
    (entry) => entry.label === 'state-protocol-anthropic-api-tests'
  );
  assert.equal(anthropicCommand.env.CARGO_BUILD_JOBS, '4');
  assert.equal(anthropicCommand.env.CARGO_INCREMENTAL, undefined);
  assert.equal(anthropicCommand.env.CARGO_PROFILE_TEST_INCREMENTAL, 'true');
});

test('real-workspace control-plane coverage selects narrow Cargo test targets instead of compiling all api-server targets', () => {
  const commands = buildCoverageBackendCommands({
    repoRoot,
    cargoParallelism: 4,
    cargoTestThreads: 2,
    backendKeys: ['control-plane'],
  });
  const byLabel = new Map(commands.map((command) => [command.label, command]));

  const uiRepo = byLabel.get('backend-coverage-control-plane-ui-management-integration');
  assert.equal(uiRepo.args.includes('--lib'), true);

  const mcpRoutes = byLabel.get('backend-coverage-control-plane-mcp-routes-integration');
  const mcpTargets = mcpRoutes.args.filter((_arg, index) => mcpRoutes.args[index - 1] === '--test');
  assert.deepEqual(mcpTargets, ['api_mcp_management']);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'api/apps/api-server/tests/http/api_mcp_management.rs')),
    true
  );

  const uiRoutes = byLabel.get('backend-coverage-control-plane-ui-routes-integration');
  const uiTargets = uiRoutes.args.filter((_arg, index) => uiRoutes.args[index - 1] === '--test');
  assert.deepEqual(uiTargets, ['api_private_contracts', 'frontstage_data_capability_routes']);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'api/apps/api-server/src/private_contracts_test.rs')),
    true
  );
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'api/apps/api-server/tests/frontstage_data_capability_routes.rs')),
    true
  );
});

test('runCoverage skips trailing llvm-cov clean when --keep-cache or ONEFLOWBASE_KEEP_LLVM_COV_CACHE=1 is set', async (t) => {
  assert.deepEqual(parseCoverageCliArgs(['backend', 'control-plane', '--keep-cache']), {
    help: false,
    target: 'backend',
    backendKeys: ['control-plane'],
    keepCache: true,
  });

  for (const [argv, env] of [
    [['backend', 'control-plane', '--keep-cache'], {}],
    [['backend', 'control-plane'], { ONEFLOWBASE_KEEP_LLVM_COV_CACHE: '1' }],
  ]) {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-keep-cache-'));
    t.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }));
    let cleanCount = 0;
    const status = await runCoverage(argv, {
      repoRoot: tempRoot,
      env,
      runtimeConfig: {
        backend: { cargoJobs: 2, cargoTestThreads: 2 },
        locks: { waitTimeoutMinutes: 30, waitTimeoutMs: 1_800_000, pollIntervalMs: 5000 },
      },
      preflightSpawnSyncImpl: () => ({ status: 0, stdout: '', stderr: '' }),
      writeStdout() {},
      writeStderr() {},
      managedRunnerImpl: (options) => options.runCommandSequenceImpl({ ...options, env }),
      spawnSyncImpl(_command, args) {
        if (args[1] === 'clean') cleanCount += 1;
        return {
          status: 0,
          stdout: args[1] === 'clean' ? '' : 'test result: ok. 1 passed; 0 failed;',
          stderr: '',
        };
      },
      readFileSyncImpl: () => JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] }),
    });
    assert.equal(status, 0);
    assert.equal(cleanCount, 1, 'pre-clean runs once while post-run clean is skipped');
  }
});
