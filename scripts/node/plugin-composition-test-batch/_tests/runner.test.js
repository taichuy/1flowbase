const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { manifest, parseList, selectTests, passedExact, nodeTapResult, validateSources } = require('../selection.js');
const target = { id: 'fixture:lib', regressionFilters: ['old::'], minimum: 1 };
const required = [{ target: target.id, name: 'new::root_2007_case', expected: 1 }];
test('finite manifest covers actual Root source inventory and all AC/AUTH rows', () => {
  assert.equal(manifest.required.length, 47);
  assert.equal(manifest.targets.length, 12);
  assert.equal(manifest.node.length, 4);
  for (const command of manifest.node.filter(command => command.args.includes('--test'))) {
    assert.deepEqual(command.args.filter(arg => arg.startsWith('--test-reporter')), ['--test-reporter=tap'],
      'Node24 spec defaults must not select the evidence format');
    assert.ok(command.args.indexOf('--test-reporter=tap') < command.args.findIndex(arg => arg.endsWith('.js')));
  }
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

  const tap = 'TAP version 13\n# Subtest: fixture\nok 1 - fixture\n1..1\n# tests 1\n# suites 0\n# pass 1\n# fail 0\n# cancelled 0\n# skipped 0\n# todo 0\n# duration_ms 1\n';
  assert.deepEqual(nodeTapResult(tap, 0), {
    passed: true, reason: null, counts: { tests: 1, suites: 0, pass: 1, fail: 0, cancelled: 0, skipped: 0, todo: 0 },
  });
  // Node24's default spec output is real success evidence for humans, but not our selected
  // machine protocol. The manifest above forces TAP so default/terminal presentation cannot win.
  const spec = '✔ fixture (1ms)\nℹ tests 1\nℹ suites 0\nℹ pass 1\nℹ fail 0\nℹ cancelled 0\nℹ skipped 0\nℹ todo 0\n';
  assert.equal(nodeTapResult(spec, 0).passed, false);
  for (const output of [
    tap.replace('# tests 1', '# tests 0'),
    tap.replace('# pass 1', '# pass 0'),
    ...['fail', 'cancelled', 'skipped', 'todo'].map(field => tap.replace(`# ${field} 0`, `# ${field} 1`)),
    tap.replace('ok 1 - fixture', 'not ok 1 - fixture'),
    tap.replace('ok 1 - fixture', 'ok 1 - fixture # SKIP unavailable'),
    tap.replace('ok 1 - fixture', 'ok 1 - fixture # TODO pending'),
    tap.replace('1..1', '1..2'),
    tap.replace('ok 1 - fixture', 'ok 2 - fixture'),
    tap.replace('# skipped 0\n', ''),
    `${tap}# tests 1\n`,
    `${tap}Bail out! aborted\n`,
    '# tests 1\n',
  ]) assert.equal(nodeTapResult(output, 0).passed, false, output);
  for (const exitCode of [1, null]) assert.equal(nodeTapResult(tap, exitCode).passed, false);
});
