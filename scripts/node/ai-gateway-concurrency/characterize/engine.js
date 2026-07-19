'use strict';

const { performance } = require('node:perf_hooks');
const {
  MOCK_ROUTE,
  MOCK_SCENARIO_HEADER,
  SCENARIO,
  SUCCESS_TERMINAL,
  TRANSPORT,
  assertDistinctRequestNonces,
  assertScenario,
  assertTransport,
} = require('../contracts');
const { createMockUpstream } = require('../mock-upstream');
const { CHARACTERIZE_PLAN } = require('./plan');
const { createSseParser, eventText, nonceFromText, protocolEventType } = require('./stream-parsers');
const { writeCharacterizeArtifacts } = require('./report');

const EXPECTED_OUTCOME = Object.freeze({
  [SCENARIO.NORMAL]: 'completed',
  [SCENARIO.SLOW]: 'completed',
  [SCENARIO.CANCEL_OBSERVATION]: 'cancelled',
  [SCENARIO.HTTP_500]: 'failed',
  [SCENARIO.STREAM_INTERRUPTION]: 'interrupted',
});
const AUTHORIZED_HTTP_TRANSPORTS = Object.freeze([
  TRANSPORT.RESPONSES_SSE,
  TRANSPORT.ANTHROPIC_SSE,
]);

function round(value) {
  return Math.round(value * 1000) / 1000;
}

function percentile(values, fraction) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return round(sorted[Math.floor((sorted.length - 1) * fraction)]);
}

function createClientNonce(sequence) {
  return `load-${String(sequence).padStart(6, '0')}`;
}

function endpointUrl(endpointSet, transport) {
  const value = endpointSet[transport];
  if (typeof value !== 'string') throw new Error(`missing endpoint for transport: ${transport}`);
  const url = new URL(value);
  const allowed = transport === TRANSPORT.RESPONSES_WEBSOCKET ? ['ws:', 'wss:'] : ['http:', 'https:'];
  if (!allowed.includes(url.protocol)) throw new Error(`invalid ${transport} endpoint protocol: ${url.protocol}`);
  return url;
}

function requestBody(transport, scenario, clientNonce) {
  const metadata = { mock_scenario: scenario, request_nonce: clientNonce };
  if (transport === TRANSPORT.ANTHROPIC_SSE) {
    return {
      model: 'mock-model',
      max_tokens: 32,
      stream: true,
      metadata,
      messages: [{ role: 'user', content: `concurrency probe ${clientNonce}` }],
    };
  }
  return {
    model: 'mock-model',
    stream: true,
    metadata,
    input: [{ role: 'user', content: [{ type: 'input_text', text: `concurrency probe ${clientNonce}` }] }],
  };
}

function inspectProtocolEvents(transport, protocolEvents, texts) {
  const terminal = SUCCESS_TERMINAL[transport];
  const terminalCount = protocolEvents.filter((event) => event === terminal).length;
  const upstreamNonces = [...new Set(texts.map(nonceFromText).filter(Boolean))];
  return {
    terminalCount,
    upstreamNonce: upstreamNonces.length === 1 ? upstreamNonces[0] : null,
    upstreamNonceCount: upstreamNonces.length,
  };
}

function requestNonceEvidence(transport, protocolEvents, texts, errorNonce) {
  const evidence = inspectProtocolEvents(transport, protocolEvents, texts);
  if (!evidence.upstreamNonce && typeof errorNonce === 'string') {
    return { ...evidence, upstreamNonce: errorNonce, upstreamNonceCount: 1 };
  }
  return evidence;
}

function normalizeHeadersByTransport(headersByTransport = {}) {
  if (!headersByTransport || typeof headersByTransport !== 'object' || Array.isArray(headersByTransport)) {
    throw new Error('headersByTransport must be an object');
  }
  const normalized = {};
  for (const [transport, headers] of Object.entries(headersByTransport)) {
    if (!AUTHORIZED_HTTP_TRANSPORTS.includes(transport)) {
      throw new Error(`headers are not allowed for transport: ${transport}`);
    }
    if (!headers || typeof headers !== 'object' || Array.isArray(headers)) {
      throw new Error(`headers for ${transport} must be an object`);
    }
    normalized[transport] = {};
    for (const [name, value] of Object.entries(headers)) {
      if (typeof value !== 'string' || value.trim() === '') throw new Error(`header ${name} for ${transport} must be a non-empty string`);
      normalized[transport][name] = value;
    }
  }
  return normalized;
}

function authorizationHeadersByTransport(authorizationTokenByTransport) {
  if (!authorizationTokenByTransport || typeof authorizationTokenByTransport !== 'object') {
    throw new Error('authorizationTokenByTransport is required');
  }
  const keys = Object.keys(authorizationTokenByTransport);
  for (const transport of keys) {
    if (!AUTHORIZED_HTTP_TRANSPORTS.includes(transport)) throw new Error(`authorization token is not allowed for transport: ${transport}`);
  }
  for (const transport of AUTHORIZED_HTTP_TRANSPORTS) {
    if (
      typeof authorizationTokenByTransport[transport] !== 'string'
      || authorizationTokenByTransport[transport].trim() === ''
    ) {
      throw new Error(`authorization token is required for transport: ${transport}`);
    }
  }
  if (authorizationTokenByTransport[TRANSPORT.RESPONSES_SSE] === authorizationTokenByTransport[TRANSPORT.ANTHROPIC_SSE]) {
    throw new Error('Responses SSE and Anthropic SSE must use distinct Application API keys');
  }
  return Object.fromEntries(AUTHORIZED_HTTP_TRANSPORTS.map((transport) => [
    transport,
    { authorization: `Bearer ${authorizationTokenByTransport[transport]}` },
  ]));
}

async function runSseRequest({ endpoint, transport, scenario, clientNonce, headers, timeoutMs, batchStartedAt, fetchImpl }) {
  const startedAt = performance.now();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error('request timeout')), timeoutMs);
  const protocolEvents = [];
  const texts = [];
  let ttftMs = null;
  let httpStatus = null;
  let errorNonce = null;
  let outcome = 'interrupted';
  try {
    const response = await fetchImpl(endpoint, {
      method: 'POST',
      headers: {
        accept: 'text/event-stream',
        'content-type': 'application/json',
        [MOCK_SCENARIO_HEADER]: scenario,
        ...headers,
      },
      body: JSON.stringify(requestBody(transport, scenario, clientNonce)),
      signal: controller.signal,
    });
    httpStatus = response.status;
    if (!response.ok) {
      const errorBody = await response.json().catch(() => null);
      if (typeof errorBody?.error?.nonce === 'string') errorNonce = errorBody.error.nonce;
      outcome = 'failed';
    } else {
      const reader = response.body.getReader();
      let cancelSent = false;
      const parser = createSseParser((event) => {
        const type = protocolEventType(event);
        if (type) protocolEvents.push(type);
        const text = eventText(event);
        if (text) texts.push(text);
        if (ttftMs === null && text) ttftMs = round(performance.now() - startedAt);
        if (scenario === SCENARIO.CANCEL_OBSERVATION && text && !cancelSent) {
          cancelSent = true;
          controller.abort();
        }
      });
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          parser.push(value);
        }
        parser.finish();
      } catch (error) {
        if (!(scenario === SCENARIO.CANCEL_OBSERVATION && controller.signal.aborted)) throw error;
      }
      const evidence = inspectProtocolEvents(transport, protocolEvents, texts);
      if (scenario === SCENARIO.CANCEL_OBSERVATION && controller.signal.aborted) outcome = 'cancelled';
      else if (evidence.terminalCount === 1) outcome = 'completed';
      else outcome = 'interrupted';
    }
  } catch (error) {
    if (scenario === SCENARIO.CANCEL_OBSERVATION && controller.signal.aborted) outcome = 'cancelled';
    else if (scenario === SCENARIO.STREAM_INTERRUPTION) outcome = 'interrupted';
    else throw error;
  } finally {
    clearTimeout(timer);
  }
  const evidence = requestNonceEvidence(transport, protocolEvents, texts, errorNonce);
  return {
    clientNonce,
    transport,
    scenario,
    outcome,
    httpStatus,
    protocolEvents,
    chunkTexts: texts,
    ...evidence,
    dispatchedOffsetMs: round(startedAt - batchStartedAt),
    ttftMs,
    totalLatencyMs: round(performance.now() - startedAt),
  };
}

function runWebSocketRequest({ endpoint, scenario, clientNonce, timeoutMs, batchStartedAt, WebSocketImpl }) {
  const startedAt = performance.now();
  const url = new URL(endpoint);
  url.searchParams.set('scenario', scenario);
  return new Promise((resolve, reject) => {
    const protocolEvents = [];
    const texts = [];
    let ttftMs = null;
    let cancelSent = false;
    let errorNonce = null;
    let settled = false;
    const socket = new WebSocketImpl(url);
    const finish = (outcome) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      const evidence = requestNonceEvidence(TRANSPORT.RESPONSES_WEBSOCKET, protocolEvents, texts, errorNonce);
      resolve({
        clientNonce,
        transport: TRANSPORT.RESPONSES_WEBSOCKET,
        scenario,
        outcome,
        httpStatus: null,
        protocolEvents,
        chunkTexts: texts,
        ...evidence,
        dispatchedOffsetMs: round(startedAt - batchStartedAt),
        ttftMs,
        totalLatencyMs: round(performance.now() - startedAt),
      });
    };
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error(`WebSocket request timed out: ${clientNonce}`));
    }, timeoutMs);
    socket.addEventListener('open', () => socket.send(JSON.stringify({
      type: 'response.create',
      response: requestBody(TRANSPORT.RESPONSES_WEBSOCKET, scenario, clientNonce),
    })));
    socket.addEventListener('message', (message) => {
      let event;
      try {
        event = JSON.parse(message.data);
      } catch (error) {
        clearTimeout(timer);
        socket.close();
        reject(new Error(`invalid WebSocket JSON for ${clientNonce}: ${error.message}`));
        return;
      }
      if (event.type) protocolEvents.push(event.type);
      if (typeof event.error?.nonce === 'string') errorNonce = event.error.nonce;
      const text = event.type === 'response.output_text.delta' ? event.delta : null;
      if (text) {
        texts.push(text);
        if (ttftMs === null) ttftMs = round(performance.now() - startedAt);
      }
      if (scenario === SCENARIO.CANCEL_OBSERVATION && text && !cancelSent) {
        cancelSent = true;
        socket.send(JSON.stringify({ type: 'response.cancel' }));
      }
      if (event.type === 'response.cancelled') finish('cancelled');
      else if (event.type === 'response.completed') finish('completed');
      else if (event.type === 'error') finish('failed');
    });
    socket.addEventListener('close', () => {
      if (settled) return;
      if (scenario === SCENARIO.STREAM_INTERRUPTION) finish('interrupted');
      else if (scenario === SCENARIO.HTTP_500) finish('failed');
      else finish('interrupted');
    });
    socket.addEventListener('error', () => {
      if (scenario === SCENARIO.HTTP_500) finish('failed');
      else if (scenario === SCENARIO.STREAM_INTERRUPTION) finish('interrupted');
    });
  });
}

function validateRequestResult(result) {
  const failures = [];
  const expected = EXPECTED_OUTCOME[result.scenario];
  if (result.outcome !== expected) failures.push(`expected outcome ${expected}, received ${result.outcome}`);
  if ([SCENARIO.NORMAL, SCENARIO.SLOW].includes(result.scenario)) {
    if (result.terminalCount !== 1) failures.push(`expected one success terminal, received ${result.terminalCount}`);
    if (result.chunkTexts.length !== 2) failures.push(`expected two text chunks, received ${result.chunkTexts.length}`);
    if (result.upstreamNonceCount !== 1) failures.push(`expected one upstream nonce, received ${result.upstreamNonceCount}`);
    if (result.upstreamNonce && !result.chunkTexts.every((text) => text.startsWith(`${result.upstreamNonce}:chunk-`))) {
      failures.push('text chunks crossed upstream nonce boundaries');
    }
  } else if (result.terminalCount !== 0) {
    failures.push(`non-success scenario emitted ${result.terminalCount} success terminal(s)`);
  }
  return failures;
}

function requestErrorResult({ transport, scenario, clientNonce, batchStartedAt }, error) {
  return {
    clientNonce,
    transport,
    scenario,
    outcome: 'request-error',
    httpStatus: null,
    protocolEvents: [],
    chunkTexts: [],
    terminalCount: 0,
    upstreamNonce: null,
    upstreamNonceCount: 0,
    dispatchedOffsetMs: 0,
    ttftMs: null,
    totalLatencyMs: round(performance.now() - batchStartedAt),
    errorType: error?.name ?? 'Error',
  };
}

async function waitForMockBatch(before, mockSnapshot, expectedArrivals, timeoutMs) {
  if (!before || !mockSnapshot) return null;
  const deadline = performance.now() + Math.min(timeoutMs, 1_000);
  let after = mockSnapshot();
  while ((after.arrivals - before.arrivals < expectedArrivals || after.active !== before.active) && performance.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 5));
    after = mockSnapshot();
  }
  return after;
}

function mockBatchEvidence(before, after, results) {
  if (!before || !after) {
    return { available: false, mockArrivalPeak: null, derivedQueueMaxMs: null, arrivalEntries: [], contractFailures: [] };
  }
  const entries = after.entries.slice(before.entries.length);
  const arrivals = entries.filter((entry) => entry.event === 'arrival');
  const arrivalByNonce = new Map(arrivals.map((entry) => [entry.nonce, entry]));
  const correlated = results
    .map((result) => ({ result, arrival: arrivalByNonce.get(result.upstreamNonce) }))
    .filter((item) => item.arrival);
  let derivedQueueMaxMs = null;
  if (correlated.length > 0) {
    const firstArrival = Math.min(...correlated.map((item) => item.arrival.offsetMs));
    const firstDispatch = Math.min(...correlated.map((item) => item.result.dispatchedOffsetMs));
    derivedQueueMaxMs = round(Math.max(0, ...correlated.map((item) =>
      (item.arrival.offsetMs - firstArrival) - (item.result.dispatchedOffsetMs - firstDispatch))));
  }
  const contractFailures = [];
  if (arrivals.length !== results.length) contractFailures.push(`expected ${results.length} mock arrivals, received ${arrivals.length}`);
  if (after.active !== before.active) contractFailures.push(`mock active count did not return to ${before.active}: ${after.active}`);
  const scenario = results[0]?.scenario;
  const observedScenarioCount = scenario === SCENARIO.CANCEL_OBSERVATION
    ? entries.filter((entry) => entry.event === 'cancel_observed').length
    : scenario === SCENARIO.STREAM_INTERRUPTION
      ? entries.filter((entry) => entry.event === 'stream_interrupted').length
      : scenario === SCENARIO.HTTP_500
        ? entries.filter((entry) => ['http-500', 'upstream-error'].includes(entry.outcome)).length
        : entries.filter((entry) => entry.outcome === 'completed').length;
  if (observedScenarioCount !== results.length) {
    contractFailures.push(`expected ${results.length} mock ${scenario} observations, received ${observedScenarioCount}`);
  }
  return {
    available: true,
    mockArrivalPeak: arrivals.length ? Math.max(...arrivals.map((entry) => entry.active)) : null,
    derivedQueueMaxMs,
    arrivalEntries: entries,
    contractFailures,
  };
}

function batchSummary({ row, results, durationMs, mockEvidence }) {
  const failures = mockEvidence.contractFailures.map((message) => ({ clientNonce: null, message }));
  for (const result of results) {
    for (const message of validateRequestResult(result)) failures.push({ clientNonce: result.clientNonce, message });
  }
  const streamNonces = results.map((result) => result.upstreamNonce).filter(Boolean);
  try {
    assertDistinctRequestNonces(streamNonces);
  } catch (error) {
    failures.push({ clientNonce: null, message: error.message });
  }
  const outcomes = {};
  for (const result of results) outcomes[result.outcome] = (outcomes[result.outcome] ?? 0) + 1;
  return {
    ...row,
    pass: failures.length === 0,
    outcomes,
    failures,
    metrics: {
      ttftP50Ms: percentile(results.map((result) => result.ttftMs).filter((value) => value !== null), 0.5),
      totalLatencyP50Ms: percentile(results.map((result) => result.totalLatencyMs), 0.5),
      throughputRps: round(results.length / (durationMs / 1000)),
      mockArrivalPeak: mockEvidence.mockArrivalPeak,
      derivedQueueMaxMs: mockEvidence.derivedQueueMaxMs,
    },
  };
}

async function executeCharacterizePlan({
  endpointSet,
  plan,
  headersByTransport = {},
  mockSnapshot,
  timeoutMs = 5_000,
  fetchImpl = globalThis.fetch,
  WebSocketImpl = globalThis.WebSocket,
}) {
  if (!Array.isArray(plan) || plan.length === 0) throw new Error('characterize plan must not be empty');
  if (typeof fetchImpl !== 'function') throw new Error('fetch implementation is unavailable');
  const requestHeaders = normalizeHeadersByTransport(headersByTransport);
  const batches = [];
  const events = [];
  let nonceSequence = 0;
  for (const row of plan) {
    assertTransport(row.transport);
    assertScenario(row.scenario);
    if (!Number.isInteger(row.concurrency) || row.concurrency < 1 || row.concurrency > 32) {
      throw new Error(`invalid characterize concurrency: ${row.concurrency}`);
    }
    const endpoint = endpointUrl(endpointSet, row.transport);
    const before = mockSnapshot?.();
    const batchStartedAt = performance.now();
    const requests = Array.from({ length: row.concurrency }, () => {
      nonceSequence += 1;
      const request = {
        endpoint,
        transport: row.transport,
        scenario: row.scenario,
        clientNonce: createClientNonce(nonceSequence),
        timeoutMs,
        batchStartedAt,
      };
      let pending;
      if (row.transport === TRANSPORT.RESPONSES_WEBSOCKET) {
        if (typeof WebSocketImpl !== 'function') throw new Error('WebSocket implementation is unavailable');
        pending = runWebSocketRequest({ ...request, WebSocketImpl });
      } else {
        pending = runSseRequest({ ...request, headers: requestHeaders[row.transport] ?? {}, fetchImpl });
      }
      return pending.catch((error) => requestErrorResult(request, error));
    });
    const results = await Promise.all(requests);
    const durationMs = performance.now() - batchStartedAt;
    const after = await waitForMockBatch(before, mockSnapshot, row.concurrency, timeoutMs);
    const evidence = mockBatchEvidence(before, after, results);
    const summary = batchSummary({ row, results, durationMs, mockEvidence: evidence });
    batches.push(summary);
    events.push(...results.map((result) => ({ kind: 'request', ...result })));
    events.push(...evidence.arrivalEntries.map((entry) => ({ kind: 'mock-timeline', ...entry })));
  }
  const failures = batches.flatMap((batch) => batch.failures.map((failure) => ({
    batch: `${batch.transport}/${batch.scenario}/c${batch.concurrency}`,
    ...failure,
  })));
  try {
    assertDistinctRequestNonces(events
      .filter((event) => event.kind === 'request')
      .map((event) => event.clientNonce));
    assertDistinctRequestNonces(events
      .filter((event) => event.kind === 'request' && event.upstreamNonce)
      .map((event) => event.upstreamNonce));
  } catch (error) {
    failures.push({ batch: 'all-streams', clientNonce: null, message: error.message });
  }
  const observedPeaks = batches.map((batch) => batch.metrics.mockArrivalPeak).filter((value) => value !== null);
  return {
    summary: {
      schemaVersion: 1,
      profile: 'characterize',
      verdict: failures.length === 0 ? 'PASS' : 'FAIL',
      performanceBudgetApplied: false,
      totals: {
        requests: batches.reduce((total, batch) => total + batch.concurrency, 0),
        contractFailures: failures.length,
      },
      metrics: {
        mockArrivalPeak: observedPeaks.length ? Math.max(...observedPeaks) : null,
      },
      batches,
      failures,
    },
    events,
  };
}

async function runDirectMockCharacterize({ repoRoot, timeoutMs }) {
  const mock = createMockUpstream();
  const endpoints = await mock.start();
  try {
    const result = await executeCharacterizePlan({
      endpointSet: {
        [TRANSPORT.RESPONSES_SSE]: `${endpoints.httpBaseUrl}${MOCK_ROUTE.RESPONSES}`,
        [TRANSPORT.RESPONSES_WEBSOCKET]: `${endpoints.websocketBaseUrl}${MOCK_ROUTE.RESPONSES}`,
        [TRANSPORT.ANTHROPIC_SSE]: `${endpoints.httpBaseUrl}${MOCK_ROUTE.ANTHROPIC_MESSAGES}`,
      },
      plan: CHARACTERIZE_PLAN,
      mockSnapshot: mock.snapshot,
      timeoutMs,
    });
    return { ...result, artifacts: writeCharacterizeArtifacts({ repoRoot, ...result }) };
  } finally {
    await mock.stop();
  }
}

async function runGatewayCharacterize({ repoRoot, endpointSet, authorizationTokenByTransport, mockSnapshot, timeoutMs }) {
  const headersByTransport = authorizationHeadersByTransport(authorizationTokenByTransport);
  const result = await executeCharacterizePlan({
    endpointSet,
    plan: CHARACTERIZE_PLAN,
    headersByTransport,
    mockSnapshot,
    timeoutMs,
  });
  return { ...result, artifacts: writeCharacterizeArtifacts({ repoRoot, ...result }) };
}

module.exports = {
  authorizationHeadersByTransport,
  executeCharacterizePlan,
  normalizeHeadersByTransport,
  runDirectMockCharacterize,
  runGatewayCharacterize,
  validateRequestResult,
};
