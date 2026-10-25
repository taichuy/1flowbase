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

test('Cargo filter evidence rejects a missing or ignored selector beside passing tests', () => {
  for (const otherEvidence of [
    '',
    'test _tests::beta_filter ... ignored\n',
    'beta_filter: test\n',
  ]) {
    const output = 'test _tests::alpha_filter ... ok\n'
      + otherEvidence
      + 'test result: ok. 1 passed; 0 failed; 1 ignored;';
    assert.throws(
      () => requireExecutedCargoTests(output, ['alpha_filter', 'beta_filter']),
      /beta_filter: no executed passing tests/u,
    );
  }
});

test('Cargo filter evidence requires successful execution summaries', () => {
  for (const output of [
    'alpha_filter: test\nbeta_filter: test\n2 tests, 0 benchmarks\n',
    'Finished test profile [unoptimized] target(s)',
    'test _tests::alpha_filter ... ok\ntest _tests::beta_filter ... ok\n'
      + 'test result: FAILED. 2 passed; 1 failed; 0 ignored;',
  ]) {
    assert.throws(
      () => requireExecutedCargoTests(output, ['alpha_filter', 'beta_filter']),
      /refusing invalid Cargo test evidence/u,
    );
  }
});

test('Cargo filters accept ANSI and CRLF execution evidence using literal substrings', () => {
  const output = '\u001b[32mtest _tests::alpha.beta_filter ... ok\u001b[0m\r\n'
    + 'test src/lib.rs - gamma_filter (line 7) ... ok\r\n'
    + '\u001b[32mtest result: ok. 2 passed; 0 failed; 0 ignored;\u001b[0m\r\n';
  assert.deepEqual(
    requireExecutedCargoTests(output, ['alpha.beta', 'beta_filter', 'gamma_filter']),
    { passedCount: 2, failedCount: 0 },
  );
  assert.throws(
    () => requireExecutedCargoTests(output.replace('alpha.beta', 'alphaxbeta'), ['alpha.beta']),
    /alpha\.beta: no executed passing tests/u,
  );
});

test('Cargo filter evidence rejects empty or malformed filter requirements', () => {
  const output = 'test _tests::alpha_filter ... ok\n'
    + 'test result: ok. 1 passed; 0 failed; 0 ignored;';
  for (const filters of [[''], [' '], [null], [1], 'alpha_filter', null]) {
    assert.throws(
      () => requireExecutedCargoTests(output, filters),
      /Cargo test filters must be non-empty strings/u,
    );
  }
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
    const { parseCargoTestCounts, requireExecutedCargoTests } = require(process.argv[1]);
    const output = '\\u001b[32mtest result: ok. 2 passed; 0 failed;\\u001b[0m\\n'.repeat(200);
    const counts = parseCargoTestCounts(output);
    const countNormalizationCalls = normalizationCalls;
    normalizationCalls = 0;
    const filterCounts = requireExecutedCargoTests(
      '\\u001b[32mtest _tests::alpha_filter ... ok\\u001b[0m\\n'
        + 'test result: ok. 1 passed; 0 failed; 0 ignored;',
      ['alpha_filter'],
    );
    process.stdout.write(JSON.stringify({
      counts, countNormalizationCalls, filterCounts, filterNormalizationCalls: normalizationCalls,
    }));
  `, path.resolve(__dirname, '../cargo-test-results.js')], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    counts: { passedCount: 400, failedCount: 0 }, countNormalizationCalls: 1,
    filterCounts: { passedCount: 1, failedCount: 0 }, filterNormalizationCalls: 1,
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

test('log CLI requires every filter in each log without borrowing neighboring evidence', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cargo-filter-evidence-'));
  const cli = path.resolve(__dirname, '../cargo-test-results.js');
  try {
    const alpha = path.join(dir, 'alpha.log');
    const beta = path.join(dir, 'beta.log');
    const both = path.join(dir, 'both.log');
    fs.writeFileSync(alpha, 'test _tests::alpha_filter ... ok\n'
      + 'test result: ok. 1 passed; 0 failed; 0 ignored;');
    fs.writeFileSync(beta, 'test _tests::beta_filter ... ok\n'
      + 'test result: ok. 1 passed; 0 failed; 0 ignored;');
    fs.writeFileSync(both, 'test _tests::alpha_filter ... ok\ntest _tests::beta_filter ... ok\n'
      + 'test result: ok. 2 passed; 0 failed; 0 ignored;');
    assert.equal(spawnSync(process.execPath, [cli, both, '--', 'alpha_filter', 'beta_filter']).status, 0);
    assert.equal(spawnSync(process.execPath, [cli, both, both, '--', 'alpha_filter', 'beta_filter']).status, 0);
    const split = spawnSync(process.execPath, [cli, alpha, beta, '--', 'alpha_filter', 'beta_filter'], {
      encoding: 'utf8',
    });
    assert.equal(split.status, 1);
    assert.match(split.stderr, /beta_filter: no executed passing tests/u);
    assert.equal(spawnSync(process.execPath, [cli, both, '--']).status, 1);
    assert.equal(spawnSync(process.execPath, [cli, '--', 'alpha_filter']).status, 1);
    assert.equal(spawnSync(process.execPath, [cli, both, '--', '']).status, 1);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
