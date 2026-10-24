const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { main, buildBackendCommands } = require('../../verify-coverage.js');

// Exercise the real coverage orchestration. A passing neighboring suite and a
// perfect coverage summary must not mask one empty or failed test selection.
test('each control-plane coverage command requires its own executed passing tests', async () => {
  const labels = buildBackendCommands({
    repoRoot: '/fixture', cargoParallelism: 2, cargoTestThreads: 2,
    backendKeys: ['control-plane'],
  }).filter(command => command.args[0] === 'llvm-cov').map(command => command.label);
  for (const label of labels) {
    for (const [output, expected] of [
      ['', 1],
      ['test result: ok. 0 passed; 0 failed; 15 ignored;', 1],
      ['Finished test profile [unoptimized] target(s)', 1],
      ['test result: FAILED. 1 passed; 1 failed;', 1],
      ['test result: ok. 1 passed; 0 failed;', 0],
      ['test result: \u001b[32mok\u001b[0m. 2 passed; 0 failed;', 0],
    ]) {
      const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-execution-'));
      try {
        const commands = buildBackendCommands({
          repoRoot, cargoParallelism: 2, cargoTestThreads: 2,
          backendKeys: ['control-plane'],
        });
        let diagnostics = '';
        let reportsRead = 0;
        const startedLabels = [];
        let cleanupCalls = 0;
        const status = await main(['backend', 'control-plane'], {
          repoRoot, env: {},
          runtimeConfig: { backend: { cargoJobs: 2, cargoTestThreads: 2 } },
          preflightSpawnSyncImpl: () => ({ status: 0 }),
          writeStdout() {}, writeStderr(text) { diagnostics += text; },
          spawnSyncImpl(_command, args) {
            const selected = commands.find(command =>
              JSON.stringify(command.args) === JSON.stringify(args));
            if (args[1] === 'clean') cleanupCalls += 1;
            else startedLabels.push(selected?.label);
            return {
              status: 0,
              stdout: args[1] === 'clean'
                ? ''
                : selected?.label === label ? output : 'test result: ok. 3 passed; 0 failed;',
              stderr: '',
            };
          },
          managedRunnerImpl(options) {
            return options.runCommandSequenceImpl(options);
          },
          readFileSyncImpl() {
            reportsRead += 1;
            return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
          },
        });
        assert.equal(status, expected, `${label}: ${output}`);
        assert.equal(reportsRead > 0, expected === 0);
        assert.equal(cleanupCalls, 2, 'cleanup must run after an evidence failure');
        const failedIndex = commands.findIndex(command => command.label === label);
        assert.deepEqual(startedLabels, (expected === 0 ? commands : commands.slice(0, failedIndex + 1))
          .map(command => command.label));
        if (expected) assert.ok(diagnostics.includes(`${label}: no executed passing tests`));
      } finally {
        fs.rmSync(repoRoot, { recursive: true, force: true });
      }
    }
  }
});

for (const backendKey of ['control-plane', 'orchestration-runtime', 'runtime-extension-host']) {
  test(`the explicit ${backendKey} doctest step permits zero examples but rejects missing or failed results`, async () => {
    for (const [output, exitCode, expected] of [
      ['test result: ok. 0 passed; 0 failed; 0 ignored;', 0, 0],
      ['test result: ok. 2 passed; 0 failed; 0 ignored;', 0, 0],
      ['', 0, 1],
      ['test result: FAILED. 0 passed; 1 failed; 0 ignored;', 0, 1],
      ['test result: FAILED. 0 passed; 1 failed; 0 ignored;', 101, 101],
    ]) {
      const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-doctests-'));
      try {
        let doctestCalls = 0;
        const status = await main(['backend', backendKey], {
          repoRoot, env: {},
          runtimeConfig: { backend: { cargoJobs: 2, cargoTestThreads: 2 } },
          preflightSpawnSyncImpl: () => ({ status: 0 }),
          writeStdout() {}, writeStderr() {},
          spawnSyncImpl(_command, args) {
            if (args[0] === 'test' && args.includes('--doc')) {
              doctestCalls += 1;
              assert.deepEqual(args, ['test', '--package', backendKey, '--doc', '--', '--test-threads=2']);
              return { status: exitCode, stdout: output, stderr: '' };
            }
            return { status: 0, stdout: args[1] === 'clean' ? '' : 'test result: ok. 3 passed; 0 failed;', stderr: '' };
          },
          managedRunnerImpl(options) { return options.runCommandSequenceImpl(options); },
          readFileSyncImpl() {
            return JSON.stringify({ data: [{ totals: { lines: { percent: 100 } } }] });
          },
        });
        assert.equal(doctestCalls, 1);
        assert.equal(status, expected, `${exitCode}: ${output}`);
      } finally {
        fs.rmSync(repoRoot, { recursive: true, force: true });
      }
    }
  });
}
