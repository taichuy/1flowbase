const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { runStateProtocols } = require('../../verify/state-protocols.js');
const { runManagedCommandSequence } = require('../../testing/warning-capture.js');

function execute(t, cargoResult) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'state-protocol-evidence-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  return runStateProtocols(['--skip-live-acp'], {
    repoRoot,
    env: { PATH: process.env.PATH },
    runtimeConfig: { backend: { cargoJobs: 2, cargoTestThreads: 2, incremental: false } },
    managedRunnerImpl: (options) => runManagedCommandSequence({
      ...options,
      withHeavyVerifyLockImpl: (lock, run) => run(lock.env),
    }),
    spawnSyncImpl(command) {
      // Neighboring Node tests cannot supply Cargo execution evidence.
      return command === 'cargo' ? cargoResult : {
        status: 0, stdout: '# tests 10\n# pass 10\n# fail 0\n', stderr: '',
      };
    },
    writeStdout() {},
    writeStderr() {},
  });
}

for (const [name, stdout] of [
  ['empty selection', 'test result: ok. 0 passed; 0 failed; 0 ignored; 100 filtered out;\n'],
  ['all ignored', 'test result: ok. 0 passed; 0 failed; 5 ignored; 0 filtered out;\n'],
  ['compile only', 'Finished test profile\n'],
  ['failed tests despite zero exit', 'test result: FAILED. 1 passed; 1 failed; 0 ignored;\n'],
]) {
  test(`state protocol gate rejects ${name} after neighboring Node success`, async (t) => {
    await assert.rejects(execute(t, { status: 0, stdout, stderr: '' }), /no executed passing tests or failed tests/u);
  });
}

test('state protocol gate accepts real passing Cargo evidence with empty sibling targets', async (t) => {
  assert.equal(await execute(t, {
    status: 0,
    stdout: 'test result: ok. 0 passed; 0 failed; 0 ignored;\n',
    stderr: 'test result: ok. 3 passed; 0 failed; 0 ignored;\n',
  }), 0);
});

test('state protocol gate preserves a nonzero Cargo process failure', async (t) => {
  assert.equal(await execute(t, { status: 17, stdout: '', stderr: 'compiler failure\n' }), 17);
});
