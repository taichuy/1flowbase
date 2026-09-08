const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { manifest, parseList, selectTests, passedExact, validateSources } = require('../selection.js');
const target = { id: 'fixture:lib', regressionFilters: ['old::'], minimum: 1 };
const required = [{ target: target.id, name: 'new::root_2007_case', expected: 1 }];
test('finite manifest covers actual Root source inventory and all AC/AUTH rows', () => {
  assert.equal(manifest.required.length, 47);
  assert.equal(manifest.targets.length, 12);
  validateSources(path.resolve(__dirname, '../../../..'));
});
test('list selection unions overlapping requirements without duplicate execution', () => {
  const listed = parseList('old::regression: test\nnew::root_2007_case: test\n2 tests, 0 benchmarks\n');
  assert.deepEqual(selectTests(target, listed, required), ['new::root_2007_case', 'old::regression']);
  assert.throws(() => selectTests(target, ['old::regression'], required), /missing/u);
  assert.throws(() => selectTests(target, ['new::root_2007_case'], required), /zero regression/u);
  assert.throws(() => selectTests(target, [...listed, 'new::root_2007_unmapped'], required), /unmapped/u);
  assert.throws(() => selectTests(target, [...listed, listed[0]], required), /duplicate/u);
});
test('zero, ignored, failed and wrong-count executions cannot satisfy a required test', () => {
  const name = required[0].name;
  const success = `test ${name} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n`;
  assert.equal(passedExact(success, name, 0), true);
  assert.equal(passedExact(success, name, 1), false);
  assert.equal(passedExact(success.replace('1 passed', '0 passed'), name, 0), false);
  assert.equal(passedExact(success.replace('0 ignored', '1 ignored'), name, 0), false);
  assert.equal(passedExact(success.replace('... ok', '... FAILED'), name, 0), false);
});
