'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { probeVersions } = require('../canary');

test('D7-AC-008: canary reports newer versions without changing the blocking lock', () => {
  const result = probeVersions({ npmView: () => JSON.stringify('999.0.0') });
  assert.equal(result.blocking_lock_changed, false);
  assert.equal(Object.keys(result.packages).length, 5);
  assert.equal(Object.values(result.packages).every((item) => item.update_available), true);
});
