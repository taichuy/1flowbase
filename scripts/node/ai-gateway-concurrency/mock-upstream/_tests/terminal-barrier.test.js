'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { createMockUpstream } = require('..');

test('Root #1998 F1: actual controlled upstream holds after first delta until correlated barrier release', async () => {
  const mock = createMockUpstream();
  const marker = mock.terminalBarriers.arm('held-request');
  const endpoint = await mock.start();
  try {
    const response = await fetch(`${endpoint.httpBaseUrl}/v1/responses`, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model: 'mock', stream: true, input: marker }), signal: AbortSignal.timeout(5000),
    });
    const waiting = await mock.terminalBarriers.wait('held-request');
    const before = mock.snapshot().entries;
    assert.equal(before.filter((entry) => entry.event === 'arrival').length, 1);
    assert.equal(before.some((entry) => entry.event === 'settled'), false);
    assert.equal(before.some((entry) => entry.protocolEvent === 'response.completed'), false);
    assert.equal(before.find((entry) => entry.event === 'terminal_barrier_waiting').nonce, waiting.nonce);
    assert.equal(mock.terminalBarriers.release('different-request'), false);
    assert.equal(mock.terminalBarriers.release('held-request'), true);
    const body = await response.text();
    await mock.terminalBarriers.waitSettled('held-request');
    assert.match(body, /response.completed/u);
    const timeline = mock.snapshot().entries;
    assert.ok(timeline.find((entry) => entry.event === 'terminal_barrier_released').sequence < timeline.find((entry) => entry.event === 'settled').sequence);
  } finally { await mock.stop(); }
});
