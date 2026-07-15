const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildCommands, main } = require('../../verify-repo.js');

test('buildCommands composes gate router, hygiene, schema and frontstage governance, reports, script tests, frontend gates and backend verify gate', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildCommands({ repoRoot }), [
    {
      label: 'repo-gate-router',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'gate-router'],
      cwd: repoRoot,
    },
    {
      label: 'repo-hygiene',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'repo-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-i18n-hygiene',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'i18n-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-schema-hygiene',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'schema-hygiene'],
      cwd: repoRoot,
    },
    {
      label: 'repo-frontstage-governance-hygiene',
      command: process.execPath,
      args: [
        path.join(repoRoot, 'scripts', 'node', 'tooling.js'),
        'frontstage-governance-hygiene',
      ],
      cwd: repoRoot,
    },
    {
      label: 'repo-growth-table-report',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'growth-table-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-raw-jsonb-report',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'raw-jsonb-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-log-query-contract-report',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'log-query-contract-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-capacity-report',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'capacity-report'],
      cwd: repoRoot,
    },
    {
      label: 'repo-security-risk',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'security-risk'],
      cwd: repoRoot,
    },
    {
      label: 'repo-script-tests',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'scripts'],
      cwd: repoRoot,
    },
    {
      label: 'repo-contract-tests',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'contracts'],
      cwd: repoRoot,
    },
    {
      label: 'repo-vite-lazy-deps-gate',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'vite-lazy-deps-gate'],
      cwd: repoRoot,
    },
    {
      label: 'repo-frontend-full',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'full'],
      cwd: repoRoot,
    },
    {
      label: 'repo-frontend-page-regression',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'page-regression'],
      cwd: repoRoot,
    },
    {
      label: 'repo-backend-full',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'verify.js'), 'backend'],
      cwd: repoRoot,
    },
  ]);
});

test('buildCommands can select repository gate slices for parallel CI jobs', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(
    buildCommands({ repoRoot, target: 'tooling' }).map((command) => command.label),
    [
      'repo-gate-router',
      'repo-hygiene',
      'repo-i18n-hygiene',
      'repo-schema-hygiene',
      'repo-frontstage-governance-hygiene',
      'repo-growth-table-report',
      'repo-raw-jsonb-report',
      'repo-log-query-contract-report',
      'repo-capacity-report',
      'repo-security-risk',
      'repo-script-tests',
      'repo-contract-tests',
    ]
  );
  assert.deepEqual(
    buildCommands({ repoRoot, target: 'frontend' }).map((command) => command.label),
    ['repo-vite-lazy-deps-gate', 'repo-frontend-full', 'repo-frontend-page-regression']
  );
  assert.deepEqual(
    buildCommands({ repoRoot, target: 'frontend-pr' }).map((command) => command.label),
    ['repo-vite-lazy-deps-gate', 'repo-frontend-pr']
  );
  assert.deepEqual(
    buildCommands({ repoRoot, target: 'backend' }).map((command) => command.label),
    ['repo-backend-full']
  );
});

test('main runs repository full gate in order and captures advisory output', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-repo-'));
  const calls = [];

  const status = await main([], {
    repoRoot,
    env: {},
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });

      return {
        status: 0,
        stdout: '',
        stderr: `warning: ${args.slice(0, 2).join('/')} advisory\n`,
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 16);
  assert.deepEqual(
    calls.map((call) => call.args),
    [
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'gate-router'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'repo-hygiene'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'i18n-hygiene'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'schema-hygiene'],
      [
        path.join(repoRoot, 'scripts', 'node', 'tooling.js'),
        'frontstage-governance-hygiene',
      ],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'growth-table-report'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'raw-jsonb-report'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'log-query-contract-report'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'capacity-report'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'security-risk'],
      [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'scripts'],
      [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'contracts'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'vite-lazy-deps-gate'],
      [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'full'],
      [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'page-regression'],
      [path.join(repoRoot, 'scripts', 'node', 'verify.js'), 'backend'],
    ]
  );

  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'verify-repo.warnings.log');
  assert.equal(fs.existsSync(warningLogPath), true);
  const warningLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.match(warningLog, /warning: .*tooling\.js\/gate-router advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/repo-hygiene advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/i18n-hygiene advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/schema-hygiene advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/growth-table-report advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/raw-jsonb-report advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/log-query-contract-report advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/capacity-report advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/security-risk advisory/u);
  assert.match(warningLog, /warning: .*test\.js\/scripts advisory/u);
  assert.match(warningLog, /warning: .*test\.js\/contracts advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/vite-lazy-deps-gate advisory/u);
  assert.match(warningLog, /warning: .*test\.js\/frontend advisory/u);
  assert.match(warningLog, /warning: .*verify\.js\/backend advisory/u);
});

test('main runs only the requested repository gate slice', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-repo-slice-'));
  const calls = [];

  const status = await main(['frontend'], {
    repoRoot,
    env: {},
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: '', stderr: '' };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 3);
  assert.deepEqual(calls[0].args, [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'vite-lazy-deps-gate']);
  assert.deepEqual(calls[1].args, [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'full']);
  assert.deepEqual(calls[2].args, [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'page-regression']);
});

test('main runs the requested frontend PR repository gate slice', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-repo-pr-slice-'));
  const calls = [];

  const status = await main(['frontend-pr'], {
    repoRoot,
    env: {},
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: '', stderr: '' };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0].args, [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'vite-lazy-deps-gate']);
  assert.deepEqual(calls[1].args, [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'pr']);
});

test('main passes the inherited lock token through every repository gate command', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-repo-'));
  const calls = [];

  const status = await main([], {
    repoRoot,
    env: { ONEFLOWBASE_VERIFY_LOCK_TOKEN: 'chain-token' },
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: '', stderr: '' };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 16);
  assert.equal(
    calls.every((call) => call.options.env.ONEFLOWBASE_VERIFY_LOCK_TOKEN === 'chain-token'),
    true
  );
});

test('main routes the repository gate through the heavy managed runner', async () => {
  let capturedOptions = null;

  const status = await main([], {
    repoRoot: '/repo-root',
    env: {},
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, 'verify-repo');
  assert.equal(capturedOptions.lockMode, 'heavy');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/verify-repo.js');
});
