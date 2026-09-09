const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { manifest, parseList, selectTests, passedExact, nodeTapResult, validateSources } = require('../selection.js');
const target = { id: 'fixture:lib', regressionFilters: ['old::'], minimum: 1 };
const required = [{ target: target.id, name: 'new::root_2007_case', expected: 1 }];
test('finite manifest covers actual Root source inventory and all AC/AUTH rows', () => {
  assert.equal(manifest.required.length, 50);
  const reinstall = manifest.required.filter(entry => entry.name.includes('::managed_reinstall_tests::'));
  assert.deepEqual(reinstall.map(entry => entry.name.split('::').at(-1)).sort(), [
    'root_2007_ir_f01_changed_reinstall_preserves_history',
    'root_2007_ir_f01_identical_archive_restores_artifact',
    'root_2007_ir_f01_upgrade_and_concurrent_identity',
  ]);
  for (const entry of reinstall) {
    assert.equal(entry.target, 'api-server:lib');
    assert.equal(entry.expected, 1);
    assert.deepEqual(entry.env, ['MANAGED_EVENT_WORKER_FIXTURE']);
  }
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

test('Root 2014 retains legacy requirements and rejects missing new candidate evidence', () => {
  const fs = require('node:fs');
  const { loadManifest, rootTestNames } = require('../selection.js');
  const root = path.resolve(__dirname, '../../../..');
  const { data } = loadManifest(root, 'scripts/node/plugin-composition-test-batch/root-2014-manifest.json');
  assert.equal(data.scope, 'plugin-composition-2014');
  for (const legacy of manifest.required) {
    const inherited = data.required.find(row => row.target === legacy.target && row.name === legacy.name);
    assert.ok(inherited, `legacy required test retained: ${legacy.name}`);
    const { originRoot, ...actual } = inherited;
    assert.equal(originRoot, 2007);
    assert.deepEqual(actual, legacy);
  }
  validateSources(root, data);
  assert.equal(data.requiredAc.length, 18);
  assert.deepEqual(rootTestNames('#[test]\nfn root_2014_real() {}\nfn root_2014_fixture() {}', data.rootPrefixes), ['root_2014_real']);
  const newRequired = [{ target: target.id, name: 'new::root_2014_real', expected: 1 }];
  assert.throws(() => selectTests(target, ['old::regression'], newRequired, data), /missing/u);
  assert.throws(() => selectTests(target, ['old::regression', 'new::root_2014_real', 'new::root_2014_unmapped'], newRequired, data), /unmapped/u);
  const inventory = JSON.parse(fs.readFileSync(path.join(root, 'scripts/node/plugin-composition-test-batch/root-2014-inventory.json'), 'utf8'));
  assert.equal(inventory.definitions.length, 457);
  assert.equal(inventory.bindings.length, 481);
  assert.deepEqual(inventory.approvedAddedDefinitions, ['ui_management.plugin_settings_page.view']);
  assert.equal(new Set(inventory.definitions).size, 457);
  assert.equal(new Set(inventory.bindings).size, 481);
});


test('R3 probe is finite and full acceptance retains every probe and compatibility case', () => {
  const { loadManifest } = require('../selection.js');
  const root = path.resolve(__dirname, '../../../..');
  const full = loadManifest(root, 'scripts/node/plugin-composition-test-batch/root-2014-manifest.json').data;
  const probe = loadManifest(root, 'scripts/node/plugin-composition-test-batch/root-2014-r3-probe-manifest.json').data;
  assert.equal(full.required.length, 86);
  assert.equal(probe.required.length, 6);
  assert.equal(probe.targets.length, 5);
  assert.equal(probe.node.length, 0);
  assert.equal(probe.browserBinary, undefined);
  assert.deepEqual(probe.requiredAuth, []);
  assert.ok(probe.targets.every(target => target.regressionFilters.length === 0));
  for (const row of probe.required) {
    assert.deepEqual(full.required.find(candidate => candidate.name === row.name && candidate.target === row.target), row);
  }
  validateSources(root, probe);
  const sample = probe.targets[0];
  const requiredNames = probe.required.filter(row => row.target === sample.id).map(row => row.name);
  assert.deepEqual(selectTests(sample, [...requiredNames, 'old::root_2014_unrelated'], probe.required, probe), requiredNames);
  assert.throws(() => selectTests(sample, [...requiredNames, 'new::root_2014_r3_probe_unmapped'], probe.required, probe), /unmapped/u);
});
