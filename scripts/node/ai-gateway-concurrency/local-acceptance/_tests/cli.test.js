'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { parseArgs } = require('../cli');

test('Root #1477 local acceptance can consume one frozen manifest without discovery or network', () => {
  assert.deepEqual(parseArgs(['run']), {});
  assert.deepEqual(parseArgs(['run', '--manifest', '/tmp/frozen.json']), {
    manifest: '/tmp/frozen.json',
  });
  assert.throws(() => parseArgs(['run', '--latest']), /usage/u);
});
