const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildCommands, main } = require('../../verify-ci.js');

test('buildCommands composes repo full gate and coverage without repeating protected workspace tests', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildCommands({ repoRoot }), [
    {
      label: 'ci-verify-repo',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'verify.js'), 'repo'],
      cwd: repoRoot,
    },
    {
      label: 'ci-coverage-all',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'verify.js'), 'coverage', 'all'],
      cwd: repoRoot,
    },
  ]);
});

test('main runs repo and coverage gates in order and captures advisory output', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-ci-'));
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
        stderr: `warning: ${args[1]} advisory\n`,
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 2);
  assert.deepEqual(
    calls.map((call) => call.args),
    [
      [path.join(repoRoot, 'scripts', 'node', 'verify.js'), 'repo'],
      [path.join(repoRoot, 'scripts', 'node', 'verify.js'), 'coverage', 'all'],
    ]
  );

  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'verify-ci.warnings.log');
  assert.equal(fs.existsSync(warningLogPath), true);
  const warningLog = fs.readFileSync(warningLogPath, 'utf8');
  assert.match(warningLog, /warning: repo advisory/u);
  assert.match(warningLog, /warning: coverage advisory/u);
});

test('main passes the inherited lock token to repo and coverage gates', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-verify-ci-'));
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
  assert.equal(calls.length, 2);
  assert.equal(calls[0].options.env.ONEFLOWBASE_VERIFY_LOCK_TOKEN, 'chain-token');
  assert.equal(calls[1].options.env.ONEFLOWBASE_VERIFY_LOCK_TOKEN, 'chain-token');
});

test('main routes the CI gate through the heavy managed runner', async () => {
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
  assert.equal(capturedOptions.scope, 'verify-ci');
  assert.equal(capturedOptions.lockMode, 'heavy');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/verify-ci.js');
});

test('repo evidence failure prevents coverage from hiding the missing protected target', async () => {
  const calls = [];
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'verify-ci-stop-'));
  try {
    const status = await main([], {
      repoRoot, env: {}, writeStdout() {}, writeStderr() {},
      spawnSyncImpl(_command, args) {
        calls.push(args[1]);
        return { status: 23, stdout: '', stderr: 'missing protected test evidence' };
      },
    });
    assert.equal(status, 23);
    assert.deepEqual(calls, ['repo']);
  } finally { fs.rmSync(repoRoot, { recursive: true, force: true }); }
});
