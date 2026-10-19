const test = require('node:test');
const assert = require('node:assert/strict');
const { main } = require('../cli.js');

test('CLI opts into warmup-only explicitly and keeps reset as the default', async () => {
  for (const [argv, expected] of [[[], false], [['--warmup-only'], true]]) {
    let invoked = false;
    assert.equal(await main({ argv, runImpl: async ({ warmupOnly }) => {
      invoked = true;
      assert.equal(warmupOnly, expected);
      return 7;
    } }), 7);
    assert.equal(invoked, true);
  }
});

test('help and invalid options never start cache cleanup or compilation', async () => {
  const runImpl = () => assert.fail('must not start');
  let help = '';
  assert.equal(await main({ argv: ['--help'], runImpl, writeStdout: (text) => { help += text; } }), 0);
  assert.match(help, /--warmup-only/);
  await assert.rejects(main({ argv: ['--invalid'], runImpl }), /未知参数/);
});
