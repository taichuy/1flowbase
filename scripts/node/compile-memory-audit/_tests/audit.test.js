const assert = require('node:assert/strict');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { test } = require('node:test');

test('exportable compile audit keeps workload, source, recovery and report guards', () => {
  // Unit fixtures launch no Cargo, compiler, or nested heavy-job admission.
  const result = spawnSync('python3', ['-B', '-m', 'unittest', 'discover', '-s', __dirname, '-p', '*_test.py', '-v'], {
    cwd: path.resolve(__dirname, '../../../..'), encoding: 'utf8', maxBuffer: 1024 * 1024,
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stderr, /Ran 13 tests/);
});
