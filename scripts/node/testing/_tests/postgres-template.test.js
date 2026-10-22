const test = require('node:test');
const assert = require('node:assert/strict');
const {
  buildPostgresTemplateCommands,
  isApiTestCommand,
  withPostgresMetricsEnv,
} = require('../postgres-template.js');

test('template selection covers actual API tests and leaves lists, checks, docs and other packages untouched', () => {
  for (const args of [
    ['test', '--workspace'],
    ['test', '-p', 'api-server', '--', '--test-threads=2'],
    ['nextest', 'run', '--package', 'api-server', '--partition', 'hash:1/4'],
    ['llvm-cov', '--package', 'control-plane', '--package', 'api-server', '--', 'mcp_management_routes'],
    ['llvm-cov', 'nextest', '--package', 'api-server', '--no-tests=fail'],
  ]) assert.equal(isApiTestCommand({ command: 'cargo', args }), true, args.join(' '));
  for (const args of [
    ['test', '--package', 'control-plane'],
    ['test', '--workspace', '--exclude', 'api-server'],
    ['test', '-p', 'api-server', '--doc'],
    ['test', '-p', 'api-server', '--no-run'],
    ['check', '--workspace'],
    ['clippy', '-p', 'api-server', '--all-targets'],
    ['nextest', 'list', '--package', 'api-server'],
    ['llvm-cov', 'show-env', '--sh'],
    ['llvm-cov', 'clean', '--workspace'],
    ['llvm-cov', 'report', '--package', 'api-server'],
    ['llvm-cov', 'nextest', 'list', '--package', 'api-server'],
    ['test', '-p', 'domain', '--', 'api-server'],
  ]) {
    const command = { command: 'cargo', args };
    assert.equal(isApiTestCommand(command), false, args.join(' '));
    assert.deepEqual(buildPostgresTemplateCommands({ repoRoot: '/repo', command, env: {} }), [command]);
  }
});

test('helper builds separately in the existing target and preserves the test command, label and environment', () => {
  const original = {
    label: 'api-shard', command: 'cargo', cwd: 'api',
    args: ['nextest', 'run', '--package', 'api-server', '--partition', 'hash:3/4', '--no-tests=fail'],
    env: { CARGO_BUILD_JOBS: '3', CARGO_INCREMENTAL: '0', CARGO_PROFILE_TEST_DEBUG: '0', RUST_MIN_STACK: '8388608' },
    profilePattern: '/repo/tmp/test-governance/shard-3-%p-%m.profraw',
  };
  for (const [target, expected] of [
    [undefined, '/repo/api/target/debug/with-postgres-template'],
    ['../tmp/cached target', '/repo/tmp/cached target/debug/with-postgres-template'],
    ['/cache/api', '/cache/api/debug/with-postgres-template'],
  ]) {
    const [build, wrapped] = buildPostgresTemplateCommands({
      repoRoot: '/repo', command: original, env: { CARGO_TARGET_DIR: target },
    });
    assert.deepEqual(build, {
      label: 'postgres-template-build-api-shard', command: 'cargo', cwd: 'api', env: original.env,
      args: ['build', '--locked', '--package', 'postgres-test-support', '--bin', 'with-postgres-template', '--profile', 'test'],
    });
    assert.deepEqual(wrapped, { ...original, command: expected, args: ['--', 'cargo', ...original.args] });
    assert.deepEqual(buildPostgresTemplateCommands({ repoRoot: '/repo', command: wrapped, env: {} }), [wrapped], 'do not wrap twice');
  }
  const [build, wrapped] = buildPostgresTemplateCommands({
    repoRoot: '/repo', command: { ...original, env: { ...original.env, CARGO_TARGET_DIR: '/command-owned' } },
    env: { CARGO_TARGET_DIR: '/global' },
  });
  assert.equal(wrapped.command, '/command-owned/debug/with-postgres-template');
  assert.equal(build.env.CARGO_BUILD_JOBS, '3');
});

test('fixture metrics use the runner scope without dropping resource, lock or instrumentation environment', () => {
  const env = {
    ONEFLOWBASE_VERIFY_LOCK_TOKEN: 'owned-lock', CARGO_BUILD_JOBS: '4', CARGO_TARGET_DIR: '../tmp/api-target',
    RUSTC_WRAPPER: '/cargo-llvm-cov', CARGO_LLVM_COV: '1', CARGO_LLVM_COV_TARGET_DIR: '/repo/tmp/api-target',
    LLVM_PROFILE_FILE: '/repo/tmp/shard-%p-%m.profraw', DATABASE_URL: 'fixture-only',
  };
  assert.deepEqual(withPostgresMetricsEnv({ repoRoot: '/repo', scope: 'verify-backend-test-api-server-1-of-4', env }), {
    ...env, POSTGRES_TEST_METRICS_DIR: '/repo/tmp/test-governance/postgres-fixtures/verify-backend-test-api-server-1-of-4',
  });
  assert.equal(env.POSTGRES_TEST_METRICS_DIR, undefined);
});
