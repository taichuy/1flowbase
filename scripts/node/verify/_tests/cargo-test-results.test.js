const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { parseCargoTestCounts, requireExecutedCargoTests } = require('../cargo-test-results.js');

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

test('Cargo counts accumulate ANSI summaries across multiple test binaries', () => {
  const output = [
    '\u001b[1mRunning\u001b[0m unit tests',
    'test result: \u001b[32mok\u001b[0m. 12 passed; 0 failed; 3 ignored;',
    'test result: \u001b[31mFAILED\u001b[0m. 7 passed; 2 failed; 0 ignored;',
    '\u001b[32mtest result: ok. 0 passed; 0 failed; 0 ignored;\u001b[0m',
    'test result: ok. 4 passed; 0 failed; 1 ignored;',
  ].join('\n');
  assert.deepEqual(parseCargoTestCounts(output), { passedCount: 23, failedCount: 2 });
  assert.throws(() => requireExecutedCargoTests(output), /refusing invalid Cargo test evidence/u);
  assert.deepEqual(parseCargoTestCounts('Finished compilation'), {
    passedCount: null, failedCount: null,
  });
});

test('Cargo parser strips terminal controls once per command log', () => {
  const result = spawnSync(process.execPath, ['-e', `
    const util = require('node:util');
    const strip = util.stripVTControlCharacters;
    let normalizationCalls = 0;
    util.stripVTControlCharacters = (output) => {
      normalizationCalls += 1;
      return strip(output);
    };
    const { parseCargoTestCounts } = require(process.argv[1]);
    const output = '\\u001b[32mtest result: ok. 2 passed; 0 failed;\\u001b[0m\\n'.repeat(200);
    const counts = parseCargoTestCounts(output);
    process.stdout.write(JSON.stringify({ counts, normalizationCalls }));
  `, path.resolve(__dirname, '../cargo-test-results.js')], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    counts: { passedCount: 400, failedCount: 0 }, normalizationCalls: 1,
  });
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
