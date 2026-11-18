const test = require('node:test');
const assert = require('node:assert/strict');
const { requireCargoTestTargets } = require('../cargo-test-results.js');

const target = { label: 'protected-state', packageName: 'control-plane', filter: 'state::transition' };
const passing = 'running 1 test\ntest state::transition ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n';
const launch = '     Running unittests src/lib.rs (target/debug/deps/control_plane-abc123)\n';

test('protected target reuse pairs native Cargo launches with passing libtest suites', () => {
  requireCargoTestTargets({
    stdout: passing + '\nrunning 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored;\n',
    stderr: launch + '   Doc-tests control_plane\n',
  }, [target]);
  requireCargoTestTargets({
    stdout: passing.replace('ok', '\u001b[32mok\u001b[0m'),
    stderr: launch.replace('Running', '\u001b[1mRunning\u001b[0m'),
  }, [target]);
  requireCargoTestTargets({
    stdout: passing,
    stderr: '     Running tests/runtime_record_integration.rs (target/debug/deps/runtime_record_integration-ab12)\n',
  }, [{ label: 'integration', packageName: 'control-plane-postgres-tests', testTarget: 'runtime_record_integration', filter: '' }]);
});

test('neighboring packages, ignored tests, missing launches and missing suites cannot supply evidence', () => {
  for (const result of [
    { stdout: passing, stderr: launch.replace('control_plane', 'other_package') },
    { stdout: passing + passing.replaceAll('state::transition', 'another_test'), stderr: launch.replace('control_plane', 'other_package') + launch },
    { stdout: passing.replace('... ok', '... ignored'), stderr: launch },
    { stdout: passing.replace('state::transition', 'state::unrelated'), stderr: launch },
    { stdout: passing, stderr: launch + launch },
    { stdout: passing, stderr: '' },
    { stdout: '', stderr: launch },
    { stdout: passing, stderr: '   Doc-tests control_plane\n' },
  ]) assert.throws(() => requireCargoTestTargets(result, [target]), /refusing protected target reuse/u);
});


test('API consistency evidence follows native binaries and retains the file HTTP/documentation split', () => {
  const { BACKEND_CONSISTENCY_TARGETS } = require('../backend-targets.js');
  const protectedApi = BACKEND_CONSISTENCY_TARGETS.filter(({ group }) => group === 'api');
  const fixtures = [
    ['api_application', ['model_definition_routes::create', 'runtime_model_routes::read']],
    ['api_identity', ['workspace_routes::authorize']],
    ['api_system_settings', ['file_management_routes::download']],
    ['api_documentation', ['file_management_routes::download_openapi']],
  ];
  const transcript = (entries) => ({
    stderr: entries.map(([binary]) => `     Running tests/${binary}.rs (target/debug/deps/${binary}-abc123)\n`).join(''),
    stdout: entries.map(([, names]) => `running ${names.length} tests\n${names.map((name) => `test ${name} ... ok\n`).join('')}test result: ok. ${names.length} passed; 0 failed; 0 ignored;\n`).join(''),
  });
  requireCargoTestTargets(transcript(fixtures), protectedApi);
  // A neighboring suite cannot stand in for either half of the original file contract.
  for (const absent of ['api_system_settings', 'api_documentation']) {
    assert.throws(() => requireCargoTestTargets(
      transcript(fixtures.filter(([binary]) => binary !== absent)), protectedApi,
    ), /consistency-api-file-management.*refusing protected target reuse/u);
  }
  // Matching test names in the former lib binary are no longer valid native evidence.
  assert.throws(() => requireCargoTestTargets(
    transcript(fixtures.map(([, names]) => ['api_server', names])), protectedApi,
  ), /refusing protected target reuse/u);
});
