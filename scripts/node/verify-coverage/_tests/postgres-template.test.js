const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { main } = require('../../verify-coverage.js');

async function runCoverageFixture(repoRoot, key, calls, overrides = {}) {
  return main(['backend', key], {
    repoRoot, env: { CARGO_TARGET_DIR: '../tmp/coverage-base', ONEFLOWBASE_VERIFY_LOCK_TOKEN: 'coverage-lock' },
    runtimeConfig: { backend: { cargoJobs: 3, cargoTestThreads: 2 } },
    preflightSpawnSyncImpl: () => ({ status: 0 }),
    writeStdout() {}, writeStderr() {},
    managedRunnerImpl(options) {
      assert.equal(options.lockMode, 'heavy');
      return options.runCommandSequenceImpl(options);
    },
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return overrides.result?.(command, args)
        || { status: 0, stdout: args.includes('--doc') ? 'test result: ok. 0 passed; 0 failed;' : 'test result: ok. 3 passed; 0 failed;', stderr: '' };
    },
    readFileSyncImpl() {
      return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
    },
  });
}

test('coverage wraps API selections only, retaining the coverage target, metrics and doctest owner', async (t) => {
  for (const [key, count] of [['api-server', 1], ['control-plane', 2], ['storage-postgres', 0]]) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-template-'));
    t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
    const calls = [];
    assert.equal(await runCoverageFixture(repoRoot, key, calls), 0);
    const wrappers = calls.filter((call) => call.command.endsWith('/with-postgres-template'));
    assert.equal(wrappers.length, count);
    for (const call of calls) {
      assert.equal(call.options.env.CARGO_TARGET_DIR, '../tmp/coverage-base');
      assert.equal(call.options.env.ONEFLOWBASE_VERIFY_LOCK_TOKEN, 'coverage-lock');
      assert.equal(call.options.env.POSTGRES_TEST_METRICS_DIR,
        path.join(repoRoot, 'tmp/test-governance/postgres-fixtures', `verify-coverage-backend-${key}`));
    }
    for (const call of wrappers) {
      assert.equal(call.command, path.join(repoRoot, 'tmp/coverage-base/debug/with-postgres-template'));
      assert.deepEqual(call.args.slice(0, 3), ['--', 'cargo', 'llvm-cov']);
      assert.equal(call.options.env.CARGO_BUILD_JOBS, '3');
      assert.equal(call.options.env.CARGO_PROFILE_TEST_DEBUG, '0');
      assert.equal(calls[calls.indexOf(call) - 1].args[0], 'build');
      assert.ok(call.args.includes('--output-path'));
    }
    if (key === 'control-plane') {
      const docs = calls.find((call) => call.args.includes('--doc'));
      assert.equal(docs.command, 'cargo');
      assert.equal(docs.args[0], 'test');
    }
    assert.deepEqual(calls[0].args, ['llvm-cov', 'clean', '--workspace']);
    assert.deepEqual(calls.at(-1).args, ['llvm-cov', 'clean', '--workspace']);
  }
});

test('coverage preserves failed helper/wrapper status and always runs its existing cleanup', async (t) => {
  for (const failure of ['build', 'wrapper', 'cancel', 'empty']) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-template-failure-'));
    t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
    const calls = [];
    const status = await runCoverageFixture(repoRoot, 'control-plane', calls, {
      result(command, args) {
        if (failure === 'build' && args[0] === 'build') return { status: 7, stdout: '', stderr: '' };
        if (command.endsWith('/with-postgres-template')) {
          if (failure === 'wrapper') return { status: 8, stdout: '', stderr: '' };
          if (failure === 'cancel') return { status: null, signal: 'SIGTERM', stdout: '', stderr: '' };
          if (failure === 'empty') return { status: 0, stdout: 'test result: ok. 0 passed; 0 failed;', stderr: '' };
        }
        return undefined;
      },
    });
    assert.equal(status, failure === 'build' ? 7 : failure === 'wrapper' ? 8 : 1);
    assert.deepEqual(calls.at(-1).args, ['llvm-cov', 'clean', '--workspace']);
    if (failure === 'build') assert.equal(calls.some((call) => call.command.endsWith('/with-postgres-template')), false);
  }
});
