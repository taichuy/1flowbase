const test = require('node:test');
const assert = require('node:assert/strict');
const { main } = require('../official-packages.js');
const { buildGateCommand } = require('../../github-quality-gate/commands.js');

test('official package evidence rejects empty, ignored and failed execution and preserves Cargo errors', () => {
  for (const [status, stdout, expected] of [
    [0, '', 1], [0, 'test result: ok. 0 passed; 0 failed; 2 ignored;', 1],
    [0, 'test result: FAILED. 1 passed; 1 failed;', 1],
    [101, 'test result: FAILED. 0 passed; 1 failed;', 101],
    [0, 'test result: ok. 2 passed; 0 failed; 0 ignored;', 0],
  ]) {
    assert.equal(main([], { repoRoot: '/repo', env: {}, writeStdout() {}, writeStderr() {},
      spawnSyncImpl(command, args, options) {
        assert.equal(command, 'cargo');
        assert.deepEqual(args, ['test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p',
          'runtime-extension-host', '--test', 'official_plugin_compatibility', '--', '--ignored']);
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
