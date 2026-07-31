const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  parseCliArgs,
  buildFrontendCommand,
  buildFrontendPageRuntimeCommand,
  collectFrontendCoverageFailures,
  buildBackendCleanupCommands,
  buildBackendCommands,
  collectBackendCoverageFailures,
  ensureCargoLlvmCovInstalled,
  main,
} = require('../../verify-coverage.js');
const {
  backendThresholds,
} = require('../../testing/coverage-thresholds.js');

function expectedBackendCoverageCommand({ repoRoot, entry, cargoParallelism, cargoTestThreads }) {
  return {
    label: `backend-coverage-${entry.key}`,
    command: 'cargo',
    args: [
      'llvm-cov',
      '--package',
      entry.packageName,
      '--json',
      '--summary-only',
      '--output-path',
      `${repoRoot}/tmp/test-governance/coverage/backend/${entry.key}.json`,
      '--',
      `--test-threads=${cargoTestThreads}`,
    ],
    cwd: 'api',
    env: {
      CARGO_BUILD_JOBS: String(cargoParallelism),
      CARGO_INCREMENTAL: '0',
      ...(entry.key === 'api-server'
        ? { CARGO_PROFILE_TEST_DEBUG: '0' }
        : {}),
    },
  };
}

test('parseCliArgs defaults to all coverage gates', () => {
  assert.deepEqual(parseCliArgs([]), { help: false, target: 'all' });
});

test('parseCliArgs accepts a single backend coverage package', () => {
  assert.deepEqual(parseCliArgs(['backend', 'storage-postgres']), {
    help: false,
    target: 'backend',
    backendKeys: ['storage-postgres'],
  });
});

test('buildFrontendCommand runs Vitest coverage through the app package', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildFrontendCommand({ repoRoot }), {
    label: 'frontend-coverage',
    command: 'pnpm',
    args: ['--dir', 'web/app', 'test:coverage'],
    cwd: repoRoot,
  });
});

test('buildFrontendPageRuntimeCommand runs page runtime package coverage', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildFrontendPageRuntimeCommand({ repoRoot }), {
    label: 'frontend-page-runtime-coverage',
    command: 'pnpm',
    args: [
      '--dir',
      'web/packages/page-runtime',
      'exec',
      'vitest',
      'run',
      '--coverage',
      '--coverage.reporter=json-summary',
      '--coverage.reportsDirectory=../../../tmp/test-governance/coverage/frontend/page-runtime',
    ],
    cwd: repoRoot,
  });
});

test('collectFrontendCoverageFailures only checks configured high-risk prefixes', () => {
  const summary = {
    total: {
      lines: { pct: 91 },
      functions: { pct: 90 },
      statements: { pct: 92 },
      branches: { pct: 80 },
    },
    '/repo/web/app/src/features/agent-flow/pages/AgentFlowEditorPage.tsx': {
      lines: { pct: 74 },
      functions: { pct: 73 },
      statements: { pct: 71 },
      branches: { pct: 59 },
    },
    '/repo/web/app/src/features/settings/components/RolePermissionPanel.tsx': {
      lines: { pct: 68 },
      functions: { pct: 67 },
      statements: { pct: 68 },
      branches: { pct: 52 },
    },
    '/repo/web/app/src/features/dashboard/pages/DashboardPage.tsx': {
      lines: { pct: 10 },
      functions: { pct: 10 },
      statements: { pct: 10 },
      branches: { pct: 10 },
    },
    '/repo/web/app/packages/page-runtime/src/index.ts': {
      lines: { pct: 90 },
      functions: { pct: 90 },
      statements: { pct: 90 },
      branches: { pct: 90 },
    },
  };

  assert.deepEqual(collectFrontendCoverageFailures(summary), []);
});

test('buildBackendCommands emits one cargo llvm-cov command per protected package', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(
    buildBackendCommands({ repoRoot, cargoParallelism: 4, cargoTestThreads: 2 }),
    backendThresholds.map((entry) => expectedBackendCoverageCommand({
      repoRoot,
      entry,
      cargoParallelism: 4,
      cargoTestThreads: 2,
    }))
  );
});

test('buildBackendCommands can restrict backend coverage to one package', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(
    buildBackendCommands({
      repoRoot,
      cargoParallelism: 4,
      cargoTestThreads: 2,
      backendKeys: ['api-server'],
    }).map((command) => command.label),
    ['backend-coverage-api-server']
  );
});

test('buildBackendCleanupCommands emits cargo llvm-cov clean for workspace artifacts', () => {
  assert.deepEqual(buildBackendCleanupCommands(), [
    {
      label: 'backend-coverage-clean',
      command: 'cargo',
      args: ['llvm-cov', 'clean', '--workspace'],
      cwd: 'api',
    },
  ]);
});

test('collectBackendCoverageFailures compares line coverage per package only', () => {
  const summaries = {
    ...Object.fromEntries(backendThresholds.map((entry) => [
      entry.key,
      { data: [{ totals: { lines: { percent: 100 } } }] },
    ])),
  };

  assert.deepEqual(collectBackendCoverageFailures(summaries), []);
});

test('ensureCargoLlvmCovInstalled throws an actionable error when the cargo subcommand is absent', () => {
  assert.throws(
    () => ensureCargoLlvmCovInstalled(() => ({ status: 101, stdout: '', stderr: 'no such command: llvm-cov' })),
    /cargo llvm-cov is required/u
  );
});

test('main cleans llvm-cov artifacts before and after backend coverage runs', async () => {
  const calls = [];
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-coverage-'));

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
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: '', stderr: '' };
    },
    readFileSyncImpl(filePath) {
      return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(
    calls.map((call) => call.args),
    [
      ['llvm-cov', 'clean', '--workspace'],
      ...backendThresholds.map((entry) => expectedBackendCoverageCommand({
        repoRoot,
        entry,
        cargoParallelism: 2,
        cargoTestThreads: 4,
      }).args),
      ['llvm-cov', 'clean', '--workspace'],
    ]
  );
});

test('main routes backend coverage through the heavy lock and uses configured backend jobs', async () => {
  let capturedOptions = null;
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-coverage-managed-'));

  const status = await main(['backend'], {
    repoRoot,
    env: {},
    runtimeConfig: {
      backend: {
        cargoJobs: 3,
        cargoTestThreads: 1,
      },
      locks: {
        waitTimeoutMinutes: 30,
        waitTimeoutMs: 30 * 60 * 1000,
        pollIntervalMs: 5000,
      },
    },
    preflightSpawnSyncImpl() {
      return { status: 0, stdout: '', stderr: '' };
    },
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
    readFileSyncImpl() {
      return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.lockMode, 'heavy');
  assert.equal(capturedOptions.runtimeConfig.backend.cargoJobs, 3);
  assert.deepEqual(capturedOptions.commands[0].args, ['llvm-cov', 'clean', '--workspace']);
  assert.match(capturedOptions.commands[1].args.join(' '), /--package control-plane/u);
  assert.equal(capturedOptions.commands[1].env.CARGO_BUILD_JOBS, '3');
  assert.equal(capturedOptions.commands[1].args.at(-1), '--test-threads=1');
});

test('main writes coverage summary output under tmp/test-governance', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-coverage-log-'));

  const status = await main(['frontend'], {
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
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: 'frontend coverage run complete\n',
        stderr: 'coverage advisory\n',
      };
    },
    readFileSyncImpl() {
      return JSON.stringify({
        total: {
          lines: { pct: 100 },
          functions: { pct: 100 },
          statements: { pct: 100 },
          branches: { pct: 100 },
        },
        [`${repoRoot}/web/app/src/features/agent-flow/pages/AgentFlowEditorPage.tsx`]: {
          lines: { pct: 100 },
          functions: { pct: 100 },
          statements: { pct: 100 },
          branches: { pct: 100 },
        },
        [`${repoRoot}/web/app/src/features/settings/components/RolePermissionPanel.tsx`]: {
          lines: { pct: 100 },
          functions: { pct: 100 },
          statements: { pct: 100 },
          branches: { pct: 100 },
        },
        [`${repoRoot}/web/app/packages/page-runtime/src/index.ts`]: {
          lines: { pct: 100 },
          functions: { pct: 100 },
          statements: { pct: 100 },
          branches: { pct: 100 },
        },
      });
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);
  const summaryLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'coverage-summary.log');
  const warningLogPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'verify-coverage-frontend.warnings.log'
  );
  assert.equal(fs.existsSync(summaryLogPath), true);
  assert.match(fs.readFileSync(summaryLogPath, 'utf8'), /frontend coverage run complete/u);
  assert.match(fs.readFileSync(summaryLogPath, 'utf8'), /Coverage thresholds passed/u);
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /coverage advisory/u);
});

test('main merges page runtime package coverage into frontend thresholds', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-coverage-page-runtime-'));
  const calls = [];
  let stdout = '';

  function writeSummary(relativePath, summary) {
    const filePath = path.join(repoRoot, relativePath);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
  }

  const status = await main(['frontend'], {
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
    spawnSyncImpl(command, args) {
      calls.push({ command, args });

      if (args.includes('web/app')) {
        writeSummary('tmp/test-governance/coverage/frontend/coverage-summary.json', {
          total: {
            lines: { pct: 100 },
            functions: { pct: 100 },
            statements: { pct: 100 },
            branches: { pct: 100 },
          },
          [`${repoRoot}/web/app/src/features/agent-flow/pages/AgentFlowEditorPage.tsx`]: {
            lines: { pct: 100 },
            functions: { pct: 100 },
            statements: { pct: 100 },
            branches: { pct: 100 },
          },
          [`${repoRoot}/web/app/src/features/settings/components/RolePermissionPanel.tsx`]: {
            lines: { pct: 100 },
            functions: { pct: 100 },
            statements: { pct: 100 },
            branches: { pct: 100 },
          },
        });
      }

      if (args.includes('web/packages/page-runtime')) {
        writeSummary('tmp/test-governance/coverage/frontend/page-runtime/coverage-summary.json', {
          total: {
            lines: { pct: 100 },
            functions: { pct: 100 },
            statements: { pct: 100 },
            branches: { pct: 100 },
          },
          [`${repoRoot}/web/packages/page-runtime/src/index.ts`]: {
            lines: { pct: 100 },
            functions: { pct: 100 },
            statements: { pct: 100 },
            branches: { pct: 100 },
          },
        });
      }

      return { status: 0, stdout: '', stderr: '' };
    },
    writeStdout(text) {
      stdout += text;
    },
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.deepEqual(
    calls.map((call) => call.args.slice(0, 2)),
    [
      ['--dir', 'web/app'],
      ['--dir', 'web/packages/page-runtime'],
    ]
  );
  assert.match(stdout, /Coverage thresholds passed for frontend/u);
});
