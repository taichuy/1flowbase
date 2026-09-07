'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const http = require('node:http');
const { collectGatewayFrames } = require('../../workflow-contract/gateway-websocket');
const { CLOSE_PROBES, interruptedTrace } = require('../lifecycle');
const ID = '018f7af7-3694-7ba0-90bf-83b5ec689705';
function frames(events) { return events.map((event) => [Buffer.from(JSON.stringify(event))]); }

test('Root #1998 P3: first-delta disconnect requires unique run and never accepts success', () => {
  const events = [{ type: 'response.created', response: { id: `resp_${ID}` } }, { type: 'response.output_text.delta', delta: 'mock-000001' }];
  assert.equal(interruptedTrace(frames(events), 'trace').run_id, ID);
  assert.throws(() => interruptedTrace(frames(events.slice(0, 1)), 'trace'), /first delta/u);
  assert.throws(() => interruptedTrace(frames([...events, { type: 'response.completed' }]), 'trace'), /emitted success/u);
});

test('Root #1998 P3: real socket collector checks server close code for each unsupported operation', async () => {
  for (const probe of CLOSE_PROBES.filter((item) => item.payload !== undefined)) {
    let observedOpcode;
    const server = http.createServer();
    server.on('upgrade', (_request, socket) => {
      socket.write('HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n');
      socket.once('data', (chunk) => {
        observedOpcode = chunk[0] & 15;
        const close = Buffer.alloc(4);
        close[0] = 0x88; close[1] = 2; close.writeUInt16BE(probe.closeCode, 2);
        socket.end(close);
      });
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    try {
      const collected = await collectGatewayFrames({ url: `ws://127.0.0.1:${server.address().port}/v1/responses`, connect_headers: {} }, 'trace', { probe, timeoutMs: 1000 });
      assert.equal(collected.close_code, probe.closeCode);
      assert.equal(observedOpcode, probe.opcode ?? 1);
    } finally { await new Promise((resolve) => server.close(resolve)); }
  }
});

const { assertBarrierOrdering } = require('../lifecycle');

function heldEvidence(status = 'succeeded') {
  return {
    trace: { run_id: ID, upstream_nonce: 'mock-000001' }, cancel_response: { id: ID },
    barrier_key: 'barrier', barrier: { nonce: 'mock-000001' },
    pre_action_native: { status: 'running' },
    runtime_after_close: { active: { websocket_connections: 0 } },
    upstream_before_release: [{ event: 'terminal_barrier_waiting' }],
    wire: {
      action_ns: '20', closed_ns: '30',
      events: status === 'cancelled' ? [{ type: 'response.cancelled' }] : [{ type: 'response.output_text.delta' }],
    },
    native: { status }, durable: { status },
    upstream: [
      { event: 'arrival', nonce: 'mock-000001', sequence: 1 },
      { event: 'terminal_barrier_waiting', nonce: 'mock-000001', barrier_key: 'barrier', monotonic_ns: '10', sequence: 2 },
      { event: 'terminal_barrier_released', nonce: 'mock-000001', barrier_key: 'barrier', monotonic_ns: '40', sequence: 3 },
      { event: 'settled', sequence: 4, successTerminalCount: status === 'succeeded' ? 1 : 0 },
    ],
  };
}

test('Root #1998 F1: delivery failure preserves business success only with complete pre-terminal barrier evidence', () => {
  assert.doesNotThrow(() => assertBarrierOrdering(heldEvidence(), 'succeeded'));
  for (const mutate of [
    (row) => { row.wire.action_ns = '5'; },
    (row) => { row.wire.closed_ns = '50'; },
    (row) => { row.pre_action_native.status = 'succeeded'; },
    (row) => { row.upstream_before_release.push({ event: 'settled' }); },
    (row) => { row.runtime_after_close.active.websocket_connections = 1; },
    (row) => { row.wire.events.push({ type: 'response.completed' }); },
    (row) => { row.upstream.at(-1).successTerminalCount = 0; },
  ]) {
    const row = heldEvidence(); mutate(row);
    assert.throws(() => assertBarrierOrdering(row, 'succeeded'));
  }
});

test('Root #1998 F1: explicit Native cancel requires both cancelled business state and terminal wire projection', () => {
  assert.doesNotThrow(() => assertBarrierOrdering(heldEvidence('cancelled'), 'cancelled'));
  assert.throws(() => assertBarrierOrdering(heldEvidence(), 'cancelled'), /expected business cancelled/u);
  const missing = heldEvidence('cancelled'); missing.wire.events = [];
  assert.throws(() => assertBarrierOrdering(missing, 'cancelled'), /unique failed\/cancelled/u);
});

