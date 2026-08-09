const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { parseCliArgs, buildCommands, main } = require('../../test-frontend.js');

test('parseCliArgs defaults to full frontend gate', () => {
  assert.deepEqual(parseCliArgs([]), {
    help: false,
    layer: 'full',
  });
});

test('parseCliArgs accepts page-regression frontend gate', () => {
  assert.deepEqual(parseCliArgs(['page-regression']), {
    help: false,
    layer: 'page-regression',
  });
});

test('parseCliArgs accepts pr frontend gate', () => {
  assert.deepEqual(parseCliArgs(['pr']), {
    help: false,
    layer: 'pr',
  });
});

test('frontend coverage reuses the stable full-test exclusion inventory', () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(__dirname, '../../../../web/app/package.json'), 'utf8')
  );
  const stableArgs = packageJson.scripts.test.split(' run ')[1];
  const coverageArgs = packageJson.scripts['test:coverage'].split(' coverage ')[1];

  assert.ok(stableArgs);
  assert.equal(coverageArgs, stableArgs);
});

test('buildCommands maps full layer to lint, test, build and style-boundary', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildCommands({ layer: 'full', repoRoot }), [
    {
      label: 'frontend-lint',
      command: 'pnpm',
      args: ['--dir', 'web', 'lint'],
      cwd: '.',
    },
    {
      label: 'frontend-test',
      command: 'pnpm',
      args: ['--dir', 'web', 'test'],
      cwd: '.',
    },
    {
      label: 'frontend-build',
      command: 'pnpm',
      args: ['--dir', 'web/app', 'build'],
      cwd: '.',
    },
    {
      label: 'frontend-style-boundary',
      command: process.execPath,
      args: [path.join(repoRoot, 'scripts', 'node', 'tooling.js'), 'check-style-boundary', 'all-pages'],
      cwd: repoRoot,
    },
  ]);
});

test('pr layer runs lint, PR smoke tests, and build without style-boundary', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildCommands({ layer: 'pr', repoRoot }), [
    {
      label: 'frontend-lint',
      command: 'pnpm',
      args: ['--dir', 'web', 'lint'],
      cwd: '.',
    },
    {
      label: 'frontend-pr-smoke-test',
      command: 'pnpm',
      args: ['--dir', 'web/app', 'test:pr'],
      cwd: '.',
    },
    {
      label: 'frontend-build',
      command: 'pnpm',
      args: ['--dir', 'web/app', 'build'],
      cwd: '.',
    },
  ]);
});

test('page-regression layer runs the long-term page regression suite', () => {
  const repoRoot = '/repo-root';

  assert.deepEqual(buildCommands({ layer: 'page-regression', repoRoot }), [
    {
      label: 'frontend-page-regression',
      command: 'pnpm',
      args: ['--dir', 'web/app', 'test:page-regression'],
      cwd: '.',
    },
  ]);
});

test('fast layer only runs app vitest and writes advisory warnings', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-frontend-'));
  const calls = [];

  const status = await main(['fast'], {
    repoRoot,
    env: {},
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });

      return {
        status: 0,
        stdout: '',
        stderr: 'warning: vitest advisory\n',
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, 'pnpm');
  assert.deepEqual(calls[0].args, ['--dir', 'web/app', 'test']);

  const fastLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'frontend-fast.log');
  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'frontend-fast.warnings.log');
  assert.equal(fs.existsSync(fastLogPath), true);
  assert.match(fs.readFileSync(fastLogPath, 'utf8'), /warning: vitest advisory/u);
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /warning: vitest advisory/u);
});

test('main marks full frontend gate as heavy lock mode', async () => {
  let capturedLockMode = null;

  const status = await main(['full'], {
    repoRoot: '/repo-root',
    env: {},
    managedRunnerImpl(options) {
      capturedLockMode = options.lockMode;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedLockMode, 'heavy');
});

test('main keeps fast frontend gate outside heavy lock mode', async () => {
  let capturedLockMode = null;
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-frontend-lock-'));

  const status = await main(['fast'], {
    repoRoot,
    env: {},
    managedRunnerImpl(options) {
      capturedLockMode = options.lockMode;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedLockMode, 'none');
});

test('main names pr frontend gate as a lightweight merge scope', async () => {
  let capturedOptions = null;

  const status = await main(['pr'], {
    repoRoot: '/repo-root',
    env: {},
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, 'frontend-pr');
  assert.equal(capturedOptions.lockMode, 'none');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/test-frontend.js pr');
});

test('main names page-regression as its own frontend scope', async () => {
  let capturedOptions = null;

  const status = await main(['page-regression'], {
    repoRoot: '/repo-root',
    env: {},
    managedRunnerImpl(options) {
      capturedOptions = options;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.equal(capturedOptions.scope, 'frontend-page-regression');
  assert.equal(capturedOptions.lockMode, 'none');
  assert.equal(capturedOptions.commandDisplay, 'node scripts/node/test-frontend.js page-regression');
});
