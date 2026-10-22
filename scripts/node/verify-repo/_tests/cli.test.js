const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildCommands, main } = require('../../verify-repo.js');

test('buildCommands can select repository gate slices for parallel CI jobs', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(
    buildCommands({ repoRoot, target: 'tooling' }).map((command) => command.label),
    [
      'repo-gate-router',
      'repo-hygiene',
      'repo-i18n-hygiene',
      'repo-data-contracts',
      'repo-frontstage-governance-hygiene',
      'repo-security-risk',
      'repo-script-tests',
      'repo-contract-tests',
    ]
  );
  assert.deepEqual(
    buildCommands({ repoRoot, target: 'frontend' }).map((command) => command.label),
    ['repo-dev-experience', 'repo-vite-lazy-deps-gate', 'repo-frontend-full', 'repo-frontend-page-regression']
  );
  assert.deepEqual(
    buildCommands({ repoRoot, target: 'frontend-pr' }).map((command) => command.label),
    ['repo-dev-experience', 'repo-vite-lazy-deps-gate', 'repo-frontend-pr']
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
  assert.equal(calls.length, 13);
  assert.deepEqual(
    calls.map((call) => call.args),
    [
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'gate-router'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'repo-hygiene'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'i18n-hygiene'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'data-contracts'],
      [
        path.join(repoRoot, 'scripts', 'node', 'tooling.js'),
        'frontstage-governance-hygiene',
      ],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'security-risk'],
      [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'scripts'],
      [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'contracts'],
      [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'dev-experience'],
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
  assert.match(warningLog, /warning: .*tooling\.js\/data-contracts advisory/u);
  assert.doesNotMatch(warningLog, /tooling\.js\/growth-table-report/u);
  assert.match(warningLog, /warning: .*tooling\.js\/security-risk advisory/u);
  assert.match(warningLog, /warning: .*test\.js\/scripts advisory/u);
  assert.match(warningLog, /warning: .*test\.js\/contracts advisory/u);
  assert.match(warningLog, /warning: .*tooling\.js\/dev-experience advisory/u);
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
  assert.equal(calls.length, 4);
  assert.deepEqual(calls[0].args, [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'dev-experience']);
  assert.deepEqual(calls[1].args, [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'vite-lazy-deps-gate']);
  assert.deepEqual(calls[2].args, [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'full']);
  assert.deepEqual(calls[3].args, [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'page-regression']);
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
  assert.equal(calls.length, 3);
  assert.deepEqual(calls[0].args, [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'dev-experience']);
  assert.deepEqual(calls[1].args, [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'vite-lazy-deps-gate']);
  assert.deepEqual(calls[2].args, [path.join(repoRoot, 'scripts', 'node', 'test.js'), 'frontend', 'pr']);
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
  assert.equal(calls.length, 13);
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
