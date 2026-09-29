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


test('native WebSocket holds terminal until the matching first-delta barrier is released', { timeout: 5000 }, async () => {
  const mock = createMockUpstream();
  const marker = mock.terminalBarriers.arm('ws-held-request');
  const endpoint = await mock.start();
  let socket;
  try {
    const events = [];
    let firstDelta;
    const receivedDelta = new Promise((resolve) => { firstDelta = resolve; });
    const closed = new Promise((resolve, reject) => {
      socket = new WebSocket(`${endpoint.websocketBaseUrl}/v1/responses`);
      socket.addEventListener('open', () => socket.send(JSON.stringify({
        type: 'response.create', response: { model: 'mock', input: marker },
      })));
      socket.addEventListener('message', ({ data }) => {
        const event = JSON.parse(data);
        events.push(event);
        if (event.type === 'response.output_text.delta') firstDelta();
      });
      socket.addEventListener('close', resolve);
      socket.addEventListener('error', reject);
    });
    const waiting = await mock.terminalBarriers.wait('ws-held-request');
    await receivedDelta;
    assert.equal(events.some((event) => event.type === 'response.completed'), false);
    const held = mock.snapshot().entries;
    assert.equal(held.filter((entry) => entry.event === 'arrival').length, 1);
    assert.equal(held.find((entry) => entry.event === 'arrival').request.body.model, 'mock');
    assert.equal(held.some((entry) => entry.event === 'settled'), false);
    assert.equal(held.find((entry) => entry.event === 'terminal_barrier_waiting').nonce, waiting.nonce);
    assert.equal(mock.terminalBarriers.release('wrong-request'), false);
    assert.equal(mock.terminalBarriers.release('ws-held-request'), true);
    assert.equal(mock.terminalBarriers.release('ws-held-request'), false);
    await closed;
    await mock.terminalBarriers.waitSettled('ws-held-request');
    assert.equal(events.filter((event) => event.type === 'response.completed').length, 1);
    const entries = mock.snapshot().entries;
    assert.equal(new Set(entries.map((entry) => entry.nonce)).size, 1);
    assert.equal(entries.filter((entry) => entry.event === 'settled').length, 1);
    assert.ok(entries.find((entry) => entry.event === 'terminal_barrier_released').sequence
      < entries.find((entry) => entry.event === 'settled').sequence);
  } finally { socket?.close(); await mock.stop(); }
});


test('disconnecting a held WebSocket settles once and cannot emit success after release', { timeout: 5000 }, async () => {
  const mock = createMockUpstream();
  const marker = mock.terminalBarriers.arm('ws-disconnected-request');
  const endpoint = await mock.start();
  let socket;
  try {
    const closed = new Promise((resolve, reject) => {
      socket = new WebSocket(`${endpoint.websocketBaseUrl}/v1/responses`);
      socket.addEventListener('open', () => socket.send(JSON.stringify({
        type: 'response.create', response: { model: 'mock', input: marker },
      })));
      socket.addEventListener('close', resolve);
      socket.addEventListener('error', reject);
    });
    await mock.terminalBarriers.wait('ws-disconnected-request');
    socket.close();
    await closed;
    await mock.terminalBarriers.waitSettled('ws-disconnected-request');
    assert.equal(mock.terminalBarriers.release('ws-disconnected-request'), true);
    const entries = mock.snapshot().entries;
    assert.equal(entries.filter((entry) => entry.event === 'settled').length, 1);
    assert.equal(entries.some((entry) => entry.protocolEvent === 'response.completed'), false);
  } finally { socket?.close(); await mock.stop(); }
});
