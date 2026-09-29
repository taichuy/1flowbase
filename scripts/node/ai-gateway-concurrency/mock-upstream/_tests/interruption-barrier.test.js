'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { createInterruptionBarrier } = require('../interruption-barrier');

test('interruption release requires the exact waiting nonce and is consumed once', async () => {
  const barriers = createInterruptionBarrier();
  const events = [];
  const held = barriers.arm({ nonce: 'mock-000001', record: (event) => events.push(event) });
  assert.equal(barriers.release('mock-000001'), false);
  const pending = held.wait();
  assert.equal(barriers.release(undefined), false);
  assert.equal(barriers.release('mock-999999'), false);
  assert.deepEqual(events, ['interruption_barrier_waiting']);
  assert.equal(barriers.release('mock-000001'), true);
  assert.equal(barriers.release('mock-000001'), false);
  assert.equal(await pending, true);
  held.abandon();
  assert.equal(barriers.release('mock-000001'), false);
  assert.deepEqual(events, ['interruption_barrier_waiting', 'interruption_delta_acknowledged']);
});

test('abandon and owner cleanup release waits without inventing a receiver acknowledgement', async () => {
  for (const cleanup of ['abandon', 'close']) {
    const barriers = createInterruptionBarrier();
    const events = [];
    const held = barriers.arm({ nonce: 'mock-000001', record: (event) => events.push(event) });
    const pending = held.wait();
    if (cleanup === 'close') barriers.close(); else held.abandon();
    assert.equal(await pending, false);
    assert.deepEqual(events, ['interruption_barrier_waiting']);
    assert.equal(barriers.release('mock-000001'), false);
  }
});
