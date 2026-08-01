'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { parseArgs } = require('../cli');

test('Root #1477 local acceptance can consume one frozen manifest without discovery or network', () => {
  assert.deepEqual(parseArgs(['run']), { command: 'run', options: {} });
  assert.deepEqual(parseArgs(['run', '--manifest', '/tmp/frozen.json']), {
    command: 'run', options: { manifest: '/tmp/frozen.json' },
  });
  assert.throws(() => parseArgs(['run', '--latest']), /usage/u);
});

test('Root #1556 F08 exposes one stable executable CountTokens upgrade command', () => {
  assert.deepEqual(
    parseArgs(['count-tokens-upgrade', '--manifest', '/tmp/upgrade.json']),
    { command: 'count-tokens-upgrade', options: { manifest: '/tmp/upgrade.json' } },
  );
  assert.throws(() => parseArgs(['count-tokens-upgrade']), /usage/u);
});
