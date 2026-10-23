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
