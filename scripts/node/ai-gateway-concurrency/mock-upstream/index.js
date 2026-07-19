'use strict';

const http = require('node:http');
const {
  MOCK_ROUTE,
  MOCK_SCENARIO_HEADER,
  SCENARIO,
  TRANSPORT,
  assertScenario,
} = require('../contracts');
const { anthropicEvents, responsesEvents } = require('./protocol-events');
const { acceptWebSocket, createFrameReader, sendClose, sendJson } = require('./websocket');

const SAFE_HEADER_NAMES = Object.freeze([
  'accept',
  'content-type',
  'user-agent',
  'x-request-id',
  MOCK_SCENARIO_HEADER,
]);
const SUCCESS_TERMINALS = new Set(['response.completed', 'message_stop']);

function requestBodySummary(body) {
  if (!body || typeof body !== 'object' || Array.isArray(body)) return { kind: typeof body };
  const summary = { keys: Object.keys(body).sort() };
  if (typeof body.model === 'string') summary.model = body.model;
  if (typeof body.stream === 'boolean') summary.stream = body.stream;
  if (Array.isArray(body.input)) summary.inputItems = body.input.length;
  if (typeof body.input === 'string') summary.inputCharacters = body.input.length;
  if (Array.isArray(body.messages)) summary.messageCount = body.messages.length;
  return summary;
}

function safeRequestSummary(request, body) {
  const headers = {};
  for (const name of SAFE_HEADER_NAMES) {
    if (typeof request.headers[name] === 'string') headers[name] = request.headers[name];
  }
  return {
    method: request.method,
    path: new URL(request.url, 'http://mock.invalid').pathname,
    headers,
    body: requestBodySummary(body),
  };
}

function scenarioFrom(request, body) {
  const url = new URL(request.url, 'http://mock.invalid');
  const scenario = request.headers[MOCK_SCENARIO_HEADER]
    ?? url.searchParams.get('scenario')
    ?? body?.metadata?.mock_scenario
    ?? SCENARIO.NORMAL;
  return assertScenario(scenario);
}

function readJson(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let length = 0;
    request.on('data', (chunk) => {
      length += chunk.length;
      if (length > 1024 * 1024) {
        reject(new Error('mock request body exceeds 1 MiB'));
        request.destroy();
        return;
      }
      chunks.push(chunk);
    });
    request.on('end', () => {
      if (chunks.length === 0) return resolve({});
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')));
      } catch (error) {
        reject(error);
      }
    });
    request.on('error', reject);
  });
}

function writeSse(response, event, data) {
  response.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
}

function createTimeline() {
  const startedAt = process.hrtime.bigint();
  const entries = [];
  let active = 0;
  let peak = 0;
  let requestSequence = 0;

  const record = (event, fields = {}) => {
    entries.push({
      sequence: entries.length + 1,
      offsetMs: Number(process.hrtime.bigint() - startedAt) / 1e6,
      event,
      ...fields,
    });
  };
  const arrive = (transport, scenario, summary) => {
    requestSequence += 1;
    active += 1;
    peak = Math.max(peak, active);
    const nonce = `mock-${String(requestSequence).padStart(6, '0')}`;
    record('arrival', { nonce, transport, scenario, active, request: summary });
    let finished = false;
    return {
      nonce,
      record: (event, fields) => record(event, { nonce, transport, scenario, ...fields }),
      finish(outcome, fields = {}) {
        if (finished) return;
        finished = true;
        active -= 1;
        record('settled', { nonce, transport, scenario, outcome, active, ...fields });
      },
    };
  };
  return {
    arrive,
    snapshot: () => ({ active, peak, arrivals: requestSequence, entries: structuredClone(entries) }),
  };
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function emitHttpStream({ response, request, scenario, stream, timeline, slowChunkDelayMs, cancelObservationMs }) {
  let disconnected = false;
  request.on('aborted', () => { disconnected = true; });
  response.on('close', () => { if (!response.writableEnded) disconnected = true; });
  const write = (event, data) => {
    if (disconnected || response.destroyed) return false;
    writeSse(response, event, data);
    timeline.record('chunk', { protocolEvent: event });
    return true;
  };

  for (const chunk of stream.chunks) {
    if (!write(chunk.event ?? chunk.type, chunk.data ?? chunk)) break;
    if (scenario === SCENARIO.SLOW) await delay(slowChunkDelayMs);
  }
  if (scenario === SCENARIO.STREAM_INTERRUPTION) {
    timeline.record('stream_interrupted');
    response.destroy();
    timeline.finish('interrupted');
    return;
  }
  if (scenario === SCENARIO.CANCEL_OBSERVATION) {
    const deadline = Date.now() + cancelObservationMs;
    while (!disconnected && Date.now() < deadline) await delay(5);
    if (disconnected) {
      timeline.record('cancel_observed');
      timeline.finish('cancelled', { successTerminalCount: 0 });
      return;
    }
  }
  const terminals = Array.isArray(stream.terminal) ? stream.terminal : [stream.terminal];
  let successTerminalCount = 0;
  for (const terminal of terminals) {
    const event = terminal.event ?? terminal.type;
    if (write(event, terminal.data ?? terminal) && SUCCESS_TERMINALS.has(event)) successTerminalCount += 1;
  }
  response.end();
  timeline.finish('completed', { successTerminalCount });
}

function beginSse(response) {
  response.writeHead(200, {
    'content-type': 'text/event-stream; charset=utf-8',
    'cache-control': 'no-cache',
    connection: 'keep-alive',
  });
  response.flushHeaders();
}

function createMockUpstream(options = {}) {
  const host = options.host ?? '127.0.0.1';
  const port = options.port ?? 0;
  const slowChunkDelayMs = options.slowChunkDelayMs ?? 25;
  const cancelObservationMs = options.cancelObservationMs ?? 250;
  const timeline = createTimeline();
  const sockets = new Set();

  const server = http.createServer(async (request, response) => {
    try {
      const path = new URL(request.url, 'http://mock.invalid').pathname;
      if (request.method !== 'POST' || ![MOCK_ROUTE.RESPONSES, MOCK_ROUTE.ANTHROPIC_MESSAGES].includes(path)) {
        response.writeHead(404, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ error: { type: 'not_found', message: 'mock route not found' } }));
        return;
      }
      const body = await readJson(request);
      const scenario = scenarioFrom(request, body);
      const transport = path === MOCK_ROUTE.RESPONSES ? TRANSPORT.RESPONSES_SSE : TRANSPORT.ANTHROPIC_SSE;
      const requestTimeline = timeline.arrive(transport, scenario, safeRequestSummary(request, body));
      if (scenario === SCENARIO.HTTP_500) {
        response.writeHead(500, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ error: { type: 'mock_upstream_error', nonce: requestTimeline.nonce } }));
        requestTimeline.finish('http-500', { status: 500, successTerminalCount: 0 });
        return;
      }
      beginSse(response);
      const stream = path === MOCK_ROUTE.RESPONSES
        ? responsesEvents(requestTimeline.nonce)
        : anthropicEvents(requestTimeline.nonce);
      await emitHttpStream({
        response,
        request,
        scenario,
        stream,
        timeline: requestTimeline,
        slowChunkDelayMs,
        cancelObservationMs,
      });
    } catch (error) {
      if (!response.headersSent) response.writeHead(400, { 'content-type': 'application/json' });
      if (!response.destroyed) response.end(JSON.stringify({ error: { type: 'invalid_mock_request', message: error.message } }));
    }
  });

  server.on('connection', (socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
  });

  server.on('upgrade', (request, socket, head) => {
    const path = new URL(request.url, 'http://mock.invalid').pathname;
    if (path !== MOCK_ROUTE.RESPONSES || !acceptWebSocket(request, socket)) {
      if (!socket.destroyed && path !== MOCK_ROUTE.RESPONSES) socket.end('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n');
      return;
    }
    if (head.length > 0) socket.unshift(head);
    let requestTimeline;
    let scenario = SCENARIO.NORMAL;
    let started = false;
    let cancelRequested = false;
    createFrameReader(socket, async (message) => {
      let body;
      try {
        body = JSON.parse(message);
      } catch {
        sendClose(socket, 1007, 'invalid json');
        return;
      }
      if (body.type === 'response.cancel') {
        cancelRequested = true;
        if (requestTimeline) {
          requestTimeline.record('cancel_observed');
          sendJson(socket, responsesEvents(requestTimeline.nonce).cancelled);
          requestTimeline.finish('cancelled', { successTerminalCount: 0 });
        }
        sendClose(socket);
        return;
      }
      if (body.type !== 'response.create' || started) return;
      started = true;
      scenario = scenarioFrom(request, body.response ?? body);
      requestTimeline = timeline.arrive(
        TRANSPORT.RESPONSES_WEBSOCKET,
        scenario,
        safeRequestSummary(request, body),
      );
      if (scenario === SCENARIO.HTTP_500) {
        sendJson(socket, {
          type: 'error',
          error: { type: 'mock_upstream_error', message: 'mock HTTP 500 equivalent', nonce: requestTimeline.nonce },
        });
        requestTimeline.finish('upstream-error', { status: 500, successTerminalCount: 0 });
        sendClose(socket, 1011, 'mock upstream error');
        return;
      }
      const stream = responsesEvents(requestTimeline.nonce);
      for (const chunk of stream.chunks) {
        if (cancelRequested || socket.destroyed) return;
        sendJson(socket, chunk);
        requestTimeline.record('chunk', { protocolEvent: chunk.type });
        if (scenario === SCENARIO.SLOW) await delay(slowChunkDelayMs);
      }
      if (scenario === SCENARIO.STREAM_INTERRUPTION) {
        requestTimeline.record('stream_interrupted');
        requestTimeline.finish('interrupted', { successTerminalCount: 0 });
        socket.destroy();
        return;
      }
      if (scenario === SCENARIO.CANCEL_OBSERVATION) return;
      sendJson(socket, stream.terminal);
      requestTimeline.record('chunk', { protocolEvent: stream.terminal.type });
      requestTimeline.finish('completed', { successTerminalCount: 1 });
      sendClose(socket);
    }, (kind) => {
      if (requestTimeline) requestTimeline.finish('disconnected', { kind, successTerminalCount: 0 });
    });
  });

  return {
    async start() {
      if (server.listening) throw new Error('mock upstream is already listening');
      await new Promise((resolve, reject) => {
        server.once('error', reject);
        server.listen(port, host, () => {
          server.off('error', reject);
          resolve();
        });
      });
      const address = server.address();
      return {
        httpBaseUrl: `http://${host}:${address.port}`,
        websocketBaseUrl: `ws://${host}:${address.port}`,
      };
    },
    async stop() {
      for (const socket of sockets) socket.destroy();
      if (!server.listening) return;
      await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    },
    snapshot: timeline.snapshot,
  };
}

module.exports = { createMockUpstream };
