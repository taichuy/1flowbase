const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { requireExecutedCargoTests } = require('../cargo-test-results.js');

test('Cargo evidence accepts executed tests and a neighboring empty doc-test suite', () => {
  assert.deepEqual(requireExecutedCargoTests(
    '\u001b[32mtest result: ok. 4 passed; 0 failed; 0 ignored;\u001b[0m\n'
      + 'test result: ok. 0 passed; 0 failed; 0 ignored;',
  ), { passedCount: 4, failedCount: 0 });
});

test('Cargo evidence rejects zero selection, ignored-only, compilation and failing summaries', () => {
  for (const output of [
    'test result: ok. 0 passed; 0 failed; 0 ignored; 10 filtered out;',
    'test result: ok. 0 passed; 0 failed; 8 ignored;',
    'Finished dev profile [unoptimized] target(s)',
    'test result: FAILED. 3 passed; 1 failed; 0 ignored;',
    '',
  ]) assert.throws(() => requireExecutedCargoTests(output), /refusing invalid Cargo test evidence/u);
});

test('log CLI requires every command to execute passing tests independently', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cargo-evidence-'));
  const cli = path.resolve(__dirname, '../cargo-test-results.js');
  try {
    const passing = path.join(dir, 'passing.log');
    const empty = path.join(dir, 'empty.log');
    fs.writeFileSync(passing, 'test result: ok. 2 passed; 0 failed; 0 ignored;');
    fs.writeFileSync(empty, 'test result: ok. 0 passed; 0 failed; 0 ignored;');
    assert.equal(spawnSync(process.execPath, [cli, passing]).status, 0);
    assert.equal(spawnSync(process.execPath, [cli, passing, empty]).status, 1);
    assert.equal(spawnSync(process.execPath, [cli, path.join(dir, 'missing.log')]).status, 1);
    assert.equal(spawnSync(process.execPath, [cli]).status, 1);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
