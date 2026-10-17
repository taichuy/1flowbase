const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { buildBackendCommands, runBackend } = require('../index.js');
const { buildGateCommand, DEFAULT_AGGREGATE_SCOPES } = require('../../github-quality-gate/commands.js');

const options = { cargoJobs: 3, cargoTestThreads: 2, repoRoot: '/repo', env: {} };
test('complete backend acceptance includes seed and feature-gated maintenance evidence', () => {
  const all = buildBackendCommands(options);
  for (const target of ['official-i18n-seed', 'history-maintenance']) {
    const focused = buildBackendCommands({ ...options, target });
    for (const command of focused) assert.deepEqual(all.find((entry) => entry.label === command.label), command);
  }
  const commands = buildBackendCommands({ ...options, target: 'history-maintenance' });
  const tests = commands.filter((entry) => entry.args[0] === 'test');
  assert.equal(tests.length, 4);
  for (const pkg of ['control-plane-postgres-tests', 'control-plane', 'storage-durable-postgres', 'api-server']) {
    const entry = tests.find((command) => command.args.includes(pkg));
    assert.ok(entry, pkg);
    assert.ok(entry.args.includes('responses-history-migration'));
    assert.equal(entry.args.at(-1), '--test-threads=2');
  }
  assert.ok(tests.find((entry) => entry.args.includes('--test'))?.args.includes('responses_history_migration'));
  assert.ok(tests.find((entry) => entry.args.includes('--bin'))?.args.includes('responses-history-migrate'));
  const clippy = commands.find((entry) => entry.args[0] === 'clippy');
  assert.deepEqual(clippy.args.slice(-3), ['--', '-D', 'warnings']);
  assert.ok(clippy.args.includes('--all-targets'));
  assert.ok(commands.some((entry) => entry.args[0] === 'build' && entry.args.includes('responses-history-migrate')));
  assert.ok(DEFAULT_AGGREGATE_SCOPES.includes('repo-backend-history-maintenance'));
  assert.deepEqual(buildGateCommand({ repoRoot: '/repo', scope: 'repo-backend-history-maintenance' }).args,
    ['/repo/scripts/node/verify-backend.js', 'history-maintenance']);
});

test('each maintenance selector needs its own executed tests; build output is not test evidence', async () => {
  for (const badIndex of [-1, 0, 1, 2, 3]) {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'history-evidence-'));
    try {
      let selected = 0;
      const status = await runBackend(['history-maintenance'], {
        repoRoot, env: {}, runtimeConfig: { backend: { cargoJobs: 3, cargoTestThreads: 2 } },
        writeStdout() {}, writeStderr() {},
        managedRunnerImpl(options) {
          return options.runCommandSequenceImpl({ ...options, spawnSyncImpl(_command, args) {
            const empty = args[0] === 'test' && selected++ === badIndex;
            return { status: 0, stderr: '', stdout: args[0] !== 'test' ? '' :
              `test result: ok. ${empty ? 0 : 2} passed; 0 failed; 0 ignored;\n` };
          } });
        },
      });
      assert.equal(status, badIndex < 0 ? 0 : 1, `empty selector ${badIndex}`);
    } finally { fs.rmSync(repoRoot, { recursive: true, force: true }); }
  }
});
