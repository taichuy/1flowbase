const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { main } = require('../../verify-backend.js');

test('actual backend runner wraps API/workspace tests and records metrics for all Rust tests', async (t) => {
  for (const [argv, wrappedCount] of [
    [['test', 'api-server-1-of-4'], 1], [['test', 'apps'], 1], [['test'], 1],
    [['test', 'control-plane'], 0], [['test', 'storage-postgres-3-of-4'], 0],
    [['official-i18n-seed'], 0], [['history-maintenance'], 0], [['clippy', 'apps'], 0], [['check', 'apps'], 0],
  ]) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'backend-template-'));
    t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
    const calls = [];
    const status = await main(argv, {
      repoRoot,
      env: { CARGO_TARGET_DIR: '../tmp/shared-target', ONEFLOWBASE_VERIFY_LOCK_TOKEN: 'existing-lock' },
      runtimeConfig: { backend: { cargoJobs: 3, cargoTestThreads: 2 } },
      writeStdout() {}, writeStderr() {},
      managedRunnerImpl(options) {
        assert.equal(options.lockMode, 'heavy');
        return options.runCommandSequenceImpl(options);
      },
      spawnSyncImpl(command, args, options) {
        calls.push({ command, args, options });
        return { status: 0, stdout: 'test result: ok. 3 passed; 0 failed;', stderr: '' };
      },
    });
    assert.equal(status, 0);
    const wrapped = calls.filter((call) => call.command.endsWith('/with-postgres-template'));
    assert.equal(wrapped.length, wrappedCount, argv.join(' '));
    for (const call of calls) {
      assert.equal(call.options.env.CARGO_BUILD_JOBS, '3');
      assert.equal(call.options.env.ONEFLOWBASE_VERIFY_LOCK_TOKEN, 'existing-lock');
      assert.equal(call.options.env.POSTGRES_TEST_METRICS_DIR,
        path.join(repoRoot, 'tmp/test-governance/postgres-fixtures', `verify-backend-${argv.join('-')}`));
    }
    for (const call of wrapped) {
      assert.equal(call.command, path.join(repoRoot, 'tmp/shared-target/debug/with-postgres-template'));
      assert.deepEqual(call.args.slice(0, 2), ['--', 'cargo']);
      assert.equal(calls[calls.indexOf(call) - 1].args[0], 'build');
      assert.ok(call.args.includes('--test-threads=2') || call.args.includes('--test-threads'));
    }
  }
});

test('helper build failure prevents API execution and wrapped failure/cancellation stays failed', async (t) => {
  for (const [buildStatus, childStatus, expected] of [[7, 0, 7], [0, 9, 9], [0, null, 1]]) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'backend-template-failure-'));
    t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
    const calls = [];
    const status = await main(['test', 'api-server-2-of-4'], {
      repoRoot, env: {}, runtimeConfig: { backend: { cargoJobs: 2, cargoTestThreads: 1 } },
      writeStdout() {}, writeStderr() {},
      managedRunnerImpl(options) { return options.runCommandSequenceImpl(options); },
      spawnSyncImpl(command, args) {
        calls.push({ command, args });
        return {
          status: args[0] === 'build' ? buildStatus : childStatus,
          signal: args[0] !== 'build' && childStatus === null ? 'SIGTERM' : null,
          stdout: '', stderr: '',
        };
      },
    });
    assert.equal(status, expected);
    assert.equal(calls.length, buildStatus === 0 ? 2 : 1);
    assert.equal(calls[0].command, 'cargo');
    assert.equal(calls[0].args[0], 'build');
    if (buildStatus === 0) assert.deepEqual(calls[1].args.slice(0, 4), ['--', 'cargo', 'nextest', 'run']);
  }
});
