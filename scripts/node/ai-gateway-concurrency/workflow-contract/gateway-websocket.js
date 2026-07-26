'use strict';

const crypto = require('node:crypto');
const http = require('node:http');

const { decodeGatewayFrames } = require('../responses-websocket-acceptance/decoder');
const { queryDurableRun } = require('../responses-websocket-acceptance/durable');
const { createGatewayTarget } = require('../responses-websocket-acceptance/target');
const { createWireAudit } = require('../responses-websocket-acceptance/wire-audit');

function clientFrame(payload) {
  const body = Buffer.from(payload);
  if (body.length > 0xffff) throw new Error('Gateway WebSocket request exceeds 65535 bytes');
  const mask = crypto.randomBytes(4);
  const header = Buffer.alloc(body.length < 126 ? 2 : 4);
  header[0] = 0x81;
  if (body.length < 126) header[1] = 0x80 | body.length;
  else {
    header[1] = 0x80 | 126;
    header.writeUInt16BE(body.length, 2);
  }
  const masked = Buffer.from(body);
  for (let index = 0; index < masked.length; index += 1) masked[index] ^= mask[index % 4];
  return Buffer.concat([header, mask, masked]);
}

function consumeServerFrames(buffer, onFrame) {
  let offset = 0;
  while (buffer.length - offset >= 2) {
    const first = buffer[offset];
    const second = buffer[offset + 1];
    let length = second & 0x7f;
    let headerLength = 2;
    if (length === 126) {
      if (buffer.length - offset < 4) break;
      length = buffer.readUInt16BE(offset + 2);
      headerLength = 4;
    } else if (length === 127) {
      throw new Error('Gateway WebSocket frame exceeds supported evidence bound');
    }
    if ((second & 0x80) !== 0) throw new Error('Gateway sent a masked WebSocket frame');
    if (buffer.length - offset < headerLength + length) break;
    onFrame(first & 0x0f, buffer.subarray(offset + headerLength, offset + headerLength + length));
    offset += headerLength + length;
  }
  return buffer.subarray(offset);
}

function collectGatewayFrames(target, clientTraceId, { timeoutMs = 10_000 } = {}) {
  const url = new URL(target.url);
  if (url.protocol !== 'ws:') throw new Error('quality gate Gateway WebSocket must use loopback ws:');
  return new Promise((resolve, reject) => {
    const key = crypto.randomBytes(16).toString('base64');
    const frames = [];
    let settled = false;
    let activeSocket = null;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      activeSocket?.destroy();
      request.destroy();
      if (error) reject(error);
      else resolve(frames);
    };
    const request = http.request({
      hostname: url.hostname,
      port: url.port,
      path: `${url.pathname}${url.search}`,
      headers: {
        ...target.connect_headers,
        connection: 'Upgrade',
        upgrade: 'websocket',
        'sec-websocket-key': key,
        'sec-websocket-version': '13',
      },
    });
    const timer = setTimeout(() => finish(new Error('Gateway WebSocket evidence timed out')), timeoutMs);
    request.once('response', (response) => finish(new Error(`Gateway WebSocket upgrade returned HTTP ${response.statusCode}`)));
    request.once('error', finish);
    request.once('upgrade', (_response, socket, head) => {
      activeSocket = socket;
      let buffered = head;
      socket.on('data', (chunk) => {
        try {
          buffered = consumeServerFrames(Buffer.concat([buffered, chunk]), (opcode, payload) => {
            if (opcode === 0x8) return;
            if (opcode !== 0x1) throw new Error(`unsupported Gateway WebSocket opcode ${opcode}`);
            const text = payload.toString('utf8');
            frames.push(text);
            const event = JSON.parse(text);
            if (event.type === 'response.completed') finish();
            else if (event.type === 'error') finish(new Error(`Gateway WebSocket returned ${event.error?.message ?? 'an error'}`));
          });
        } catch (error) {
          finish(error);
        }
      });
      socket.once('error', finish);
      socket.once('close', () => {
        if (!settled) finish(new Error('Gateway WebSocket closed before response.completed'));
      });
      socket.write(clientFrame(JSON.stringify({
        type: 'response.create',
        response: {
          model: target.model,
          stream: true,
          metadata: { trace_id: clientTraceId },
          input: [{ role: 'user', content: [{ type: 'input_text', text: `gateway websocket ${clientTraceId}` }] }],
        },
      })));
    });
    request.end();
  });
}

async function runGatewayWebSocketAcceptance({ ready, mockSnapshot }, dependencies = {}) {
  const target = createGatewayTarget(ready);
  const clientTraceId = `ws-gateway-${crypto.randomUUID()}`;
  const before = mockSnapshot();
  const frames = await (dependencies.collectGatewayFrames || collectGatewayFrames)(target, clientTraceId);
  const trace = decodeGatewayFrames(frames, { clientTraceId });
  const durable = await (dependencies.queryDurableRun || queryDurableRun)(target, trace);
  const after = mockSnapshot();
  return {
    trace,
    durable,
    wire_audit: createWireAudit({ target, trace, durable, upstreamBefore: before, upstreamAfter: after }),
  };
}

module.exports = { clientFrame, collectGatewayFrames, consumeServerFrames, runGatewayWebSocketAcceptance };
