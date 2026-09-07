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
