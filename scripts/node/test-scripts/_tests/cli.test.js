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

  assert.deepEqual(buildCommand({ repoRoot, files }), {
    label: 'scripts-node-tests',
    command: process.execPath,
    args: ['--test', ...files],
    cwd: repoRoot,
  });
});

test('main runs filtered script tests and captures warning output', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-test-scripts-'));
  const calls = [];

  const status = main(['page-debug'], {
    repoRoot,
    env: {},
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
    path.join(repoRoot, 'scripts', 'node', 'page-debug', '_tests', 'core.test.js'),
  ]);

  const warningLogPath = path.join(repoRoot, 'tmp', 'test-governance', 'test-scripts.warnings.log');
  assert.equal(fs.existsSync(warningLogPath), true);
  assert.match(fs.readFileSync(warningLogPath, 'utf8'), /warning: node:test advisory/u);
});
