const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { parseCliArgs, listTestFiles, selectTestFiles, buildCommand, main } = require('../../test-scripts.js');

test('listTestFiles skips executable fixtures below _tests', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-discovery-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const testsDir = path.join(repoRoot, 'scripts', 'node', 'example', '_tests');
  fs.mkdirSync(path.join(testsDir, 'fixtures'), { recursive: true });
  const testFile = path.join(testsDir, 'core.test.js');
  fs.writeFileSync(testFile, '');
  fs.writeFileSync(path.join(testsDir, 'fixtures', 'service.js'), '');
  assert.deepEqual(listTestFiles(repoRoot), [testFile]);
});

test('parseCliArgs defaults to all script tests', () => {
  assert.deepEqual(parseCliArgs([]), {
    help: false,
    filters: [],
  });
});

test('selectTestFiles keeps sorted test files and supports substring filters', () => {
  const files = [
    '/repo/scripts/node/page-debug/_tests/core.test.js',
    '/repo/scripts/node/dev-up/_tests/core.test.js',
    '/repo/scripts/node/verify-backend/_tests/cli.test.js',
  ];

  assert.deepEqual(selectTestFiles(files, []), [
    '/repo/scripts/node/dev-up/_tests/core.test.js',
    '/repo/scripts/node/page-debug/_tests/core.test.js',
    '/repo/scripts/node/verify-backend/_tests/cli.test.js',
  ]);
  assert.deepEqual(selectTestFiles(files, ['page-debug']), [
    '/repo/scripts/node/page-debug/_tests/core.test.js',
  ]);
});

test('buildCommand runs node --test over discovered script tests', () => {
  const repoRoot = '/repo-root';
  const files = [
    path.join(repoRoot, 'scripts', 'node', 'dev-up', '_tests', 'core.test.js'),
    path.join(repoRoot, 'scripts', 'node', 'page-debug', '_tests', 'core.test.js'),
  ];

  assert.deepEqual(buildCommand({ repoRoot, files, runtimeConfig: { scripts: { testConcurrency: 2 } } }), {
    label: 'scripts-node-tests',
    command: process.execPath,
    args: ['--test', '--test-concurrency=2', ...files],
    cwd: repoRoot,
  });
});

test('main runs filtered script tests and captures warning output', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-scripts-'));
  const calls = [];

  const status = main(['page-debug'], {
    repoRoot,
    env: {},
    runtimeConfig: { scripts: { testConcurrency: 1 } },
    listTestFilesImpl() {
      return [
        path.join(repoRoot, 'scripts', 'node', 'dev-up', '_tests', 'core.test.js'),
        path.join(repoRoot, 'scripts', 'node', 'page-debug', '_tests', 'core.test.js'),
      ];
    },
    writeStdout() {},
    writeStderr() {},
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });

      return {
        status: 0,
        stdout: '',
        stderr: 'warning: node:test advisory\n',
      };
    },
  });

  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, process.execPath);
  assert.deepEqual(calls[0].args, [
    '--test',
    '--test-concurrency=1',
    path.join(repoRoot, 'scripts', 'node', 'page-debug', '_tests', 'core.test.js'),
  ]);

  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'test-scripts.warnings.log');
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /warning: node:test advisory/u);
});


test('script runner executes every selected file under its budget and propagates test failure', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'node-budget-execution-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const testDir = path.join(repoRoot, 'scripts/node/budget/_tests');
  fs.mkdirSync(testDir, { recursive: true });
  for (const name of ['first', 'second', 'third']) {
    fs.writeFileSync(path.join(testDir, `${name}.test.js`), `
      const test = require('node:test');
      const fs = require('node:fs');
      test('${name}', async () => {
        fs.mkdirSync('active-worker');
        try {
          await new Promise(resolve => setTimeout(resolve, 25));
          fs.appendFileSync('executed.txt', '${name}\\n');
        } finally { fs.rmdirSync('active-worker'); }
      });
    `);
  }
  const fixtureEnv = { ...process.env };
  // This fixture launches a real runner, not a recursive invocation of the parent test.
  delete fixtureEnv.NODE_TEST_CONTEXT;
  const deps = { repoRoot, env: fixtureEnv, runtimeConfig: { scripts: { testConcurrency: 1 } }, writeStdout() {}, writeStderr() {} };
  assert.equal(main([], deps), 0);
  assert.deepEqual(fs.readFileSync(path.join(repoRoot, 'executed.txt'), 'utf8').trim().split('\n').sort(), ['first', 'second', 'third']);
  fs.writeFileSync(path.join(testDir, 'failure.test.js'), "require('node:test')('reject wrong result', () => require('node:assert/strict').equal(2 + 2, 5));");
  assert.notEqual(main(['failure.test.js'], deps), 0);
});
