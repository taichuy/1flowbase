const test = require('node:test');
const assert = require('node:assert/strict');
const { main } = require('../cli.js');

test('CLI opts into warmup-only explicitly and keeps reset as the default', async () => {
  for (const [argv, expected] of [[[], false], [['--warmup-only'], true]]) {
    let invoked = false;
    assert.equal(await main({ argv, scopeImpl: ({ run }) => run(), runImpl: async ({ warmupOnly }) => {
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
  assert.match(help, /jobs=1/);
  assert.match(help, /半数主机逻辑 CPU/);
  await assert.rejects(main({ argv: ['--invalid'], runImpl }), /未知参数/);
});

 test('CLI resource gate runs before cleanup and propagates containment failures', async () => {
  let started = false;
  await assert.rejects(main({ argv: [], runImpl: () => { started = true; }, scopeImpl: async () => { throw new Error('containment unavailable'); } }), /containment unavailable/);
  assert.equal(started, false);
});
