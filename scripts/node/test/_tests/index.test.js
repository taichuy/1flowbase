const test = require('node:test');
const assert = require('node:assert/strict');

const { main } = require('../index.js');

test('test index dispatches backend subcommand with the complete selection', async () => {
  let capturedArgv = null;
  const selection = ['--package', 'domain', '--lib', '--filter', 'selected'];

  const status = await main(['backend', ...selection], {
    runBackendImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, selection);
});

test('test index dispatches frontend subcommand with remaining args', async () => {
  let capturedArgv = null;
  const selection = ['fast', '--', 'src/example.test.ts', '-t', 'selected'];

  const status = await main(['frontend', ...selection], {
    runFrontendImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, selection);
});

test('test index rejects unknown subcommands', async () => {
  await assert.rejects(
    () => main(['unknown']),
    /Unknown test command: unknown/u
  );
});


test('test-backend skips the Rust static scan only when explicitly requested', async () => {
  const { runBackend } = require('../index.js');
  for (const skip of [false, true]) {
    const status = await runBackend(['--package', 'domain', '--lib', ...(skip ? ['--skip-static-gate'] : [])], {
      repoRoot: '/repo',
      runtimeConfig: { backend: { cargoJobs: 2, cargoTestThreads: 1 } },
      managedRunnerImpl({ commands }) {
        assert.equal(commands.some((command) => command.args.includes('check-rust-backend')), !skip);
        assert.equal(commands.at(-1).command, 'cargo');
        assert.ok(commands.at(-1).args.includes('--lib'));
        return 0;
      },
    });
    assert.equal(status, 0);
  }
});
