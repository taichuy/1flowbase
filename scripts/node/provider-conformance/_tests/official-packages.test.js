const test = require('node:test');
const assert = require('node:assert/strict');
const { main, REQUIRED_TESTS } = require('../official-packages.js');
const { buildGateCommand } = require('../../github-quality-gate/commands.js');

test('official package evidence rejects empty, ignored and failed execution and preserves Cargo errors', () => {
  for (const [status, stdout, expected] of [
    [0, '', 1], [0, 'test result: ok. 0 passed; 0 failed; 2 ignored;', 1],
    [0, 'test result: FAILED. 1 passed; 1 failed;', 1],
    [101, 'test result: FAILED. 0 passed; 1 failed;', 101],
    [0, 'test result: ok. 1 passed; 0 failed; 0 ignored;', 0],
  ]) {
    assert.equal(main([], { repoRoot: '/repo', env: {}, writeStdout() {}, writeStderr() {},
      spawnSyncImpl(command, args, options) {
        assert.equal(command, 'cargo');
        assert.deepEqual(args.slice(0, 9), ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p',
          'runtime-extension-host', '--test', 'official_plugin_compatibility', args[8]]);
        assert.ok(REQUIRED_TESTS.includes(args[8]));
        assert.deepEqual(args.slice(9), ['--', '--ignored', '--exact']);
        assert.equal(options.cwd, '/repo');
        return { status, stdout, stderr: '' };
      },
    }), expected);
  }
});

test('official provider scope uses the execution-aware runner', () => {
  assert.deepEqual(buildGateCommand({ repoRoot: '/repo', scope: 'provider-conformance' }), {
    command: process.execPath, args: ['/repo/scripts/node/provider-conformance/official-packages.js'], cwd: '/repo',
  });
});

test('one passing official contract cannot cover a second empty selection', () => {
  const selected = [];
  const status = main([], { repoRoot: '/repo', env: {}, writeStdout() {}, writeStderr() {},
    spawnSyncImpl(_command, args) {
      selected.push(args[8]);
      return { status: 0, stdout: selected.length === 1 ? 'test result: ok. 1 passed; 0 failed;'
        : 'test result: ok. 0 passed; 0 failed; 1 ignored;', stderr: '' };
    },
  });
  assert.equal(status, 1);
  assert.deepEqual(selected, REQUIRED_TESTS);
});
