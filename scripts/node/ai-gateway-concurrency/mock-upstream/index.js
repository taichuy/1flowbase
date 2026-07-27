'use strict';

const http = require('node:http');
const {
  MOCK_ROUTE,
  SCENARIO,
  TRANSPORT,
  assertScenario,
  mockScenarioSentinel,
} = require('../contracts');
const {
  anthropicEvents,
  anthropicToolEvents,
  chatTextEvents,
  chatToolEvents,
  DEFAULT_BARRIER_MARKERS,
  responsesEvents,
  responsesToolEvents,
  responsesWireEvents,
} = require('./protocol-events');
const { acceptWebSocket, createFrameReader, sendClose, sendJson } = require('./websocket');
const { normalizedRequestFingerprint } = require('../protocol-oracle/request-fidelity');
const { errorFixtureFromBody } = require('../protocol-oracle/error-fidelity');

const SAFE_HEADER_NAMES = Object.freeze([
  'accept',
  'content-type',
  'user-agent',
  'x-request-id',
]);
const HTTP_500_ERROR_BODY =
  ' \n{"future_error":{"shape":"unknown"},"message":"keep complete body"}\n ';
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
    semantic_sha256: normalizedRequestFingerprint({
      method: request.method,
      url: request.url,
      headers: request.headers,
      body,
    }),
  };
}

function scenarioFrom(body) {
  const encoded = JSON.stringify(body);
  const scenarios = Object.values(SCENARIO).filter((scenario) => encoded.includes(mockScenarioSentinel(scenario)));
  if (scenarios.length > 1) throw new Error('mock request contains multiple scenario sentinels');
  return assertScenario(scenarios[0] ?? SCENARIO.NORMAL);
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
  response.write(`${event ? `event: ${event}\n` : ''}data: ${JSON.stringify(data)}\n\n`);
}

function createTimeline() {
  const startedAt = process.hrtime.bigint();
  const entries = [];
  let active = 0;
  let peak = 0;
  let requestSequence = 0;
  const waiters = new Map();

  const record = (event, fields = {}) => {
    entries.push({
      sequence: entries.length + 1,
      monotonic_ns: process.hrtime.bigint().toString(),
      offsetMs: Number(process.hrtime.bigint() - startedAt) / 1e6,
      event,
      ...fields,
    });
    for (const resolve of waiters.get(event) ?? []) resolve(structuredClone(entries.at(-1)));
    waiters.delete(event);
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
    waitFor(event) {
      const existing = entries.find((entry) => entry.event === event);
      if (existing) return Promise.resolve(structuredClone(existing));
      return new Promise((resolve) => {
        const pending = waiters.get(event) ?? [];
        pending.push(resolve);
        waiters.set(event, pending);
      });
    },
  };
}

function containsValue(value, marker) {
  if (typeof value === 'string') return value.includes(marker);
  if (Array.isArray(value)) return value.some((item) => containsValue(item, marker));
  if (value && typeof value === 'object') return Object.values(value).some((item) => containsValue(item, marker));
  return false;
}

function toolVectorPath(body) {
  const encoded = JSON.stringify(body);
  const match = /TOOL_VECTOR_PATH=([^\\"\s]+)/u.exec(encoded);
  return match?.[1] ?? '/tmp/1flowbase-missing-tool-vector';
}

function gatewayExecutorProbeUrl(body) {
  const encoded = JSON.stringify(body);
  return /GATEWAY_EXECUTOR_PROBE_URL=([^\\"\s]+)/u.exec(encoded)?.[1] ?? null;
}

function requestedResponsesTool(body) {
  const names = (Array.isArray(body.tools) ? body.tools : [])
    .filter((tool) => tool?.type === 'function')
    .map((tool) => tool.name ?? tool.function?.name);
  if (names.includes('shell_command')) return 'shell_command';
  if (names.includes('read')) return 'read';
  return 'shell_command';
}

function wireAuditVectorFromBody(body) {
  const values = [
    ...(Array.isArray(body.tools) ? body.tools : []),
    ...(Array.isArray(body.input) ? body.input : []),
  ];
  const kinds = new Set(values.map((item) => item?.type).filter(Boolean));
  if (kinds.has('mcp_approval_response')) return 'mcp-approval-continuation';
  if (kinds.has('mcp')) return 'mcp-list-call-approval';
  if (['file_search', 'programmatic_tool_calling', 'shell'].some((kind) => kinds.has(kind))) {
    return 'hosted-tools';
  }
  if (kinds.has('tool_search_output') || kinds.has('additional_tools')) {
    return 'tool-search-output-additional-tools';
  }
  if (kinds.has('tool_search') || kinds.has('tool_search_call')) {
    return 'tool-search-additional-tools';
  }
  return null;
}

function observeWireVectors(body, requestTimeline, counters) {
  const values = [...(Array.isArray(body.tools) ? body.tools : []), ...(Array.isArray(body.input) ? body.input : [])];
  const kinds = values.map((item) => item?.type).filter(Boolean);
  for (const kind of kinds) {
    if (['file_search', 'code_interpreter', 'image_generation', 'programmatic_tool_calling', 'shell'].includes(kind)) {
      counters.providerExecutions += 1;
      requestTimeline.record('provider_tool_execution', { toolKind: kind });
    }
    if (kind === 'mcp') requestTimeline.record('mcp_server_definition');
    if (kind === 'tool_search' || kind === 'tool_search_call') requestTimeline.record('client_tool_search');
    if (kind === 'tool_search_output') requestTimeline.record('server_tool_search');
    if (kind === 'additional_tools') requestTimeline.record('additional_tools');
  }
}

function observeProviderWireOutput(stream, requestTimeline, counters) {
  for (const type of stream.providerOutputTypes ?? []) {
    if (type === 'mcp_list_tools') requestTimeline.record('mcp_list');
    if (type === 'mcp_call') {
      counters.providerExecutions += 1;
      requestTimeline.record('mcp_call');
    }
    if (type === 'mcp_approval_request') requestTimeline.record('mcp_approval');
  }
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function emitHttpStream({
  response,
  request,
  scenario,
  stream,
  timeline,
  slowChunkDelayMs,
  cancelObservationMs,
  barrier,
}) {
  let disconnected = false;
  request.on('aborted', () => { disconnected = true; });
  response.on('close', () => { if (!response.writableEnded) disconnected = true; });
  const write = (event, data) => {
    if (disconnected || response.destroyed) return false;
    writeSse(response, event, data);
    timeline.record('chunk', { protocolEvent: event });
    return true;
  };

  let visibleDeltaReleased = false;
  const barrierEvents = stream.barrierEvent
    ? [stream.barrierEvent]
    : ['content_block_delta', 'response.output_text.delta', 'chat.completion.chunk'];
  for (const chunk of stream.chunks) {
    const event = chunk.event ?? chunk.type ?? (chunk.choices ? 'chat.completion.chunk' : undefined);
    if (!write(event, chunk.data ?? chunk)) break;
    const visibleMarker = JSON.stringify(chunk).includes(barrier.marker);
    if (!visibleDeltaReleased && barrier.enabled && visibleMarker && barrierEvents.includes(event)) {
      visibleDeltaReleased = true;
      timeline.record('barrier_waiting', { protocolEvent: event });
      await barrier.wait();
      timeline.record('barrier_released', { protocolEvent: event });
    }
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
  if (stream.doneSentinel && !disconnected && !response.destroyed) {
    response.write('data: [DONE]\n\n');
    successTerminalCount += 1;
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
  const counters = { gatewayExecutorInvocations: 0, networkObserverOutbound: 0, providerExecutions: 0 };
  const errorFixtureAttempts = new Map();
  const sockets = new Set();
  const barrierWaiters = new Set();
  const barrier = {
    enabled: options.barrierEnabled === true,
    marker: options.barrierMarker ?? DEFAULT_BARRIER_MARKERS.first,
    wait() {
      return new Promise((resolve) => barrierWaiters.add(resolve));
    },
    release() {
      const count = barrierWaiters.size;
      for (const resolve of barrierWaiters) resolve();
      barrierWaiters.clear();
      return count;
    },
  };

  const server = http.createServer(async (request, response) => {
    try {
      const path = new URL(request.url, 'http://mock.invalid').pathname;
      if (path === '/__observer/gateway-executor') {
        counters.gatewayExecutorInvocations += 1;
        response.writeHead(204).end();
        return;
      }
      if (path === '/__observer/mcp-network') {
        counters.networkObserverOutbound += 1;
        response.writeHead(204).end();
        return;
      }
      if (request.method === 'GET' && path === '/__control/snapshot') {
        response.writeHead(200, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ ...timeline.snapshot(), counters }));
        return;
      }
      if (request.method === 'POST' && path === '/__control/barrier/release') {
        const released = barrier.release();
        response.writeHead(200, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ released }));
        return;
      }
      if (request.method !== 'POST' || ![
        MOCK_ROUTE.RESPONSES, MOCK_ROUTE.CHAT_COMPLETIONS, MOCK_ROUTE.ANTHROPIC_MESSAGES,
      ].includes(path)) {
        response.writeHead(404, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ error: { type: 'not_found', message: 'mock route not found' } }));
        return;
      }
      const body = await readJson(request);
      const scenario = scenarioFrom(body);
      const transport = path === MOCK_ROUTE.RESPONSES
        ? TRANSPORT.RESPONSES_SSE
        : path === MOCK_ROUTE.CHAT_COMPLETIONS ? 'chat-completions-sse' : TRANSPORT.ANTHROPIC_SSE;
      const requestTimeline = timeline.arrive(transport, scenario, safeRequestSummary(request, body));
      observeWireVectors(body, requestTimeline, counters);
      const errorFixture = errorFixtureFromBody(body);
      const errorFixtureKey = `${errorFixture?.id ?? ''}:${body.fixture_retry_key ?? ''}`;
      const errorFixtureAttempt = (errorFixtureAttempts.get(errorFixtureKey) ?? 0) + 1;
      if (errorFixture) errorFixtureAttempts.set(errorFixtureKey, errorFixtureAttempt);
      const shouldEmitFixtureError = errorFixture
        && (errorFixture.id !== 'retry' || errorFixtureAttempt === 1);
      if (shouldEmitFixtureError) {
        response.writeHead(errorFixture.status, { 'content-type': errorFixture.contentType });
        response.end(errorFixture.body);
        requestTimeline.finish(`http-${errorFixture.status}`, {
          status: errorFixture.status,
          errorFixture: errorFixture.id,
          errorFixtureAttempt,
          successTerminalCount: 0,
        });
        return;
      }
      if (scenario === SCENARIO.HTTP_500) {
        response.writeHead(500, { 'content-type': 'application/json' });
        response.end(HTTP_500_ERROR_BODY);
        requestTimeline.finish('http-500', { status: 500, successTerminalCount: 0 });
        return;
      }
      beginSse(response);
      const isToolTurn = containsValue(body, '1flowbase-client-tool-vector');
      const isToolResult = containsValue(body, '1flowbase-client-tool-result');
      const isClientTextTurn = containsValue(body, '1flowbase gateway sentinel ok');
      const wireAuditVector = wireAuditVectorFromBody(body);
      if (isToolTurn && !isToolResult) requestTimeline.record('tool_call');
      if (isToolResult) requestTimeline.record('second_upstream_request');
      const stream = path === MOCK_ROUTE.RESPONSES
        ? (wireAuditVector && wireAuditVector !== 'gateway-executor-probe'
          ? responsesWireEvents(requestTimeline.nonce, wireAuditVector)
          : isToolTurn || isToolResult
          ? responsesToolEvents(
            requestTimeline.nonce, toolVectorPath(body), isToolResult,
            gatewayExecutorProbeUrl(body), requestedResponsesTool(body)
          )
          : isClientTextTurn
            ? responsesEvents(requestTimeline.nonce, '1flowbase gateway sentinel ', 'ok')
            : responsesEvents(requestTimeline.nonce))
        : path === MOCK_ROUTE.CHAT_COMPLETIONS
          ? (isToolTurn || isToolResult
            ? chatToolEvents(requestTimeline.nonce, toolVectorPath(body), isToolResult)
            : isClientTextTurn
              ? chatTextEvents(requestTimeline.nonce, '1flowbase gateway sentinel ', 'ok')
              : chatTextEvents(requestTimeline.nonce))
          : (isToolTurn || isToolResult
            ? anthropicToolEvents(requestTimeline.nonce, toolVectorPath(body), isToolResult)
            : isClientTextTurn
              ? anthropicEvents(requestTimeline.nonce, '1flowbase gateway sentinel ', 'ok')
              : anthropicEvents(requestTimeline.nonce));
      observeProviderWireOutput(stream, requestTimeline, counters);
      await emitHttpStream({
        response,
        request,
        scenario,
        stream,
        timeline: requestTimeline,
        slowChunkDelayMs,
        cancelObservationMs,
        barrier,
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
      scenario = scenarioFrom(body.response ?? body);
      requestTimeline = timeline.arrive(
        TRANSPORT.RESPONSES_WEBSOCKET,
        scenario,
        safeRequestSummary(request, body),
      );
      const errorFixture = errorFixtureFromBody(body.response ?? body);
      if (errorFixture) {
        sendJson(socket, {
          type: 'error',
          error: {
            type: 'mock_upstream_error',
            message: errorFixture.body,
            status: errorFixture.status,
            nonce: requestTimeline.nonce,
          },
        });
        requestTimeline.finish('upstream-error', {
          status: errorFixture.status,
          errorFixture: errorFixture.id,
          successTerminalCount: 0,
        });
        sendClose(socket, 1011, 'mock upstream error');
        return;
      }
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
        barrierReleaseUrl: `http://${host}:${address.port}/__control/barrier/release`,
        snapshotUrl: `http://${host}:${address.port}/__control/snapshot`,
        networkObserverUrl: `http://${host}:${address.port}/__observer/mcp-network`,
        gatewayExecutorObserverUrl: `http://${host}:${address.port}/__observer/gateway-executor`,
      };
    },
    async stop() {
      for (const socket of sockets) socket.destroy();
      if (!server.listening) return;
      await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    },
    snapshot: () => ({ ...timeline.snapshot(), counters: structuredClone(counters) }),
    waitForEvent: (event) => timeline.waitFor(event),
    releaseBarrier: () => barrier.release(),
  };
}

module.exports = { HTTP_500_ERROR_BODY, createMockUpstream, wireAuditVectorFromBody };
