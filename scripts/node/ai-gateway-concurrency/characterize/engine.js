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
const { CHARACTERIZE_PLAN, TOPOLOGY } = require('./plan');
const { collectDurableConvergence } = require('./durable-evidence');
const { createSseParser, eventText, nonceFromText, protocolEventType, protocolRunId } = require('./stream-parsers');
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

function requestBody(transport, scenario, clientNonce, model = 'mock-model') {
  const metadata = { mock_scenario: scenario, request_nonce: clientNonce, trace_id: clientNonce };
  if (transport === TRANSPORT.ANTHROPIC_SSE) {
    return {
      model,
      max_tokens: 32,
      stream: true,
      metadata,
      messages: [{ role: 'user', content: `concurrency probe ${clientNonce}` }],
    };
  }
  return {
    model,
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

function normalizeModelByTransport(modelByTransport = {}) {
  if (!modelByTransport || typeof modelByTransport !== 'object' || Array.isArray(modelByTransport)) {
    throw new Error('modelByTransport must be an object');
  }
  const normalized = {};
  for (const [transport, model] of Object.entries(modelByTransport)) {
    if (!AUTHORIZED_HTTP_TRANSPORTS.includes(transport)) {
      throw new Error(`model is not allowed for transport: ${transport}`);
    }
    if (typeof model !== 'string' || model.trim() === '') {
      throw new Error(`published model is required for transport: ${transport}`);
    }
    normalized[transport] = model.trim();
  }
  return normalized;
}

function requirePublishedModelsByTransport(modelByTransport) {
  const normalized = normalizeModelByTransport(modelByTransport);
  for (const transport of AUTHORIZED_HTTP_TRANSPORTS) {
    if (!normalized[transport]) throw new Error(`published model is required for transport: ${transport}`);
  }
  return normalized;
}

function requireAnthropicTargetPool(pool) {
  if (!Array.isArray(pool) || pool.length !== 2) {
    throw new Error('Anthropic multi-pool requires exactly two published targets');
  }
  const required = ['application_id', 'provider_instance_id', 'api_key', 'model', 'publication_id'];
  for (const [index, target] of pool.entries()) {
    for (const field of required) {
      if (typeof target?.[field] !== 'string' || !target[field].trim()) {
        throw new Error(`Anthropic pool target ${index} omitted ${field}`);
      }
    }
    endpointUrl({ [TRANSPORT.ANTHROPIC_SSE]: target.gateway?.anthropic_messages_url }, TRANSPORT.ANTHROPIC_SSE);
    if (
      typeof target.durable?.query_run?.url_template !== 'string'
      || typeof target.durable?.list_runs?.url !== 'string'
      || typeof target.runtime_activity?.url !== 'string'
      || typeof target.plugin_runner_active_streams?.url !== 'string'
    ) {
      throw new Error(`Anthropic pool target ${index} omitted durable or runtime evidence endpoint`);
    }
  }
  for (const field of ['application_id', 'provider_instance_id', 'api_key']) {
    if (new Set(pool.map((target) => target[field])).size !== pool.length) {
      throw new Error(`Anthropic pool targets reused ${field}`);
    }
  }
  if (new Set(pool.map((target) => target.plugin_runner_active_streams.url)).size !== 1) {
    throw new Error('Anthropic pool targets must share one active-stream endpoint');
  }
  return pool;
}

function gatewayRequestTarget(transport, target) {
  return {
    endpoint: endpointUrl({ [transport]: target.gateway.anthropic_messages_url }, transport),
    headers: { authorization: `Bearer ${target.api_key}` },
    model: target.model,
    applicationId: target.application_id,
    providerInstanceId: target.provider_instance_id,
    durableTarget: target,
    activeStreamsEndpoint: target.plugin_runner_active_streams,
  };
}

function hasExpectedActiveStreamOverlap(expectedInstanceIds, snapshots) {
  return snapshots.some((snapshot) => expectedInstanceIds.every(
    (instanceId) => snapshot.some((stream) => stream.providerInstanceId === instanceId)
  ));
}

async function observeActiveStreamOverlap({ endpoint, expectedInstanceIds, fetchImpl, isSettled }) {
  const snapshots = [];
  const errors = [];
  while (!isSettled()) {
    try {
      const response = await fetchImpl(endpoint.url, { method: endpoint.method ?? 'GET' });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const payload = await response.json();
      const streams = Array.isArray(payload?.streams) ? payload.streams : [];
      snapshots.push(streams.map((stream) => ({
        providerInstanceId: stream?.provider_instance_id ?? null,
        status: stream?.status ?? null,
        transport: stream?.transport ?? null,
      })));
    } catch (error) {
      errors.push(error.message);
    }
    if (!isSettled()) await new Promise((resolve) => setTimeout(resolve, 5));
  }
  return {
    expectedInstanceIds,
    observed: hasExpectedActiveStreamOverlap(expectedInstanceIds, snapshots),
    snapshots,
    errors,
  };
}

async function runSseRequest({
  endpoint, transport, scenario, clientNonce, model, headers, timeoutMs, batchStartedAt, fetchImpl,
  topology, batchBarrierId, targetIndex, applicationId, providerInstanceId,
}) {
  const startedAt = performance.now();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error('request timeout')), timeoutMs);
  const protocolEvents = [];
  const protocolIds = [];
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
      body: JSON.stringify(requestBody(transport, scenario, clientNonce, model)),
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
        const protocolId = protocolRunId(transport, event);
        if (protocolId) protocolIds.push(protocolId);
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
  const uniqueProtocolIds = [...new Set(protocolIds)];
  return {
    topology,
    batchBarrierId,
    targetIndex,
    applicationId,
    providerInstanceId,
    clientNonce,
    transport,
    scenario,
    outcome,
    httpStatus,
    protocolEvents,
    protocolId: uniqueProtocolIds.length === 1 ? uniqueProtocolIds[0] : null,
    protocolIdCount: uniqueProtocolIds.length,
    chunkTexts: texts,
    ...evidence,
    dispatchedOffsetMs: round(startedAt - batchStartedAt),
    ttftMs,
    totalLatencyMs: round(performance.now() - startedAt),
  };
}

function runWebSocketRequest({
  endpoint, scenario, clientNonce, timeoutMs, batchStartedAt, WebSocketImpl,
  topology, batchBarrierId, targetIndex, applicationId, providerInstanceId,
}) {
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
        topology,
        batchBarrierId,
        targetIndex,
        applicationId,
        providerInstanceId,
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
      response: requestBody(TRANSPORT.RESPONSES_WEBSOCKET, scenario, clientNonce, 'mock-model'),
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

function requestErrorResult({
  transport, scenario, clientNonce, batchStartedAt,
  topology, batchBarrierId, targetIndex, applicationId, providerInstanceId,
}, error) {
  return {
    topology,
    batchBarrierId,
    targetIndex,
    applicationId,
    providerInstanceId,
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

function identifiedSamePoolMockPeakFailure(row, results, mockEvidence) {
  if (
    (row.topology ?? TOPOLOGY.SAME_POOL) !== TOPOLOGY.SAME_POOL
    || !AUTHORIZED_HTTP_TRANSPORTS.includes(row.transport)
    || results.length === 0
  ) return null;
  const applicationIds = results.map((result) => result.applicationId);
  const providerInstanceIds = results.map((result) => result.providerInstanceId);
  const identified = applicationIds.every((id) => typeof id === 'string' && id)
    && providerInstanceIds.every((id) => typeof id === 'string' && id)
    && new Set(applicationIds).size === 1
    && new Set(providerInstanceIds).size === 1;
  if (!identified || mockEvidence.mockArrivalPeak === 1) return null;
  return `identified same-pool HTTP SSE row expected mock arrival peak 1, received ${mockEvidence.mockArrivalPeak ?? 'unavailable'}`;
}

function batchSummary({ row, results, durationMs, mockEvidence, overlapEvidence, durableFailures = [] }) {
  const failures = mockEvidence.contractFailures.map((message) => ({ clientNonce: null, message }));
  const peakFailure = identifiedSamePoolMockPeakFailure(row, results, mockEvidence);
  if (peakFailure) failures.push({ clientNonce: null, message: peakFailure });
  failures.push(...durableFailures.map((message) => ({ clientNonce: null, message })));
  if (overlapEvidence && !overlapEvidence.observed) {
    failures.push({ clientNonce: null, message: 'multi-pool slow row did not observe both provider instances active' });
  }
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
    targetDistribution: Object.fromEntries(results.reduce((counts, result) => {
      const key = result.applicationId && result.providerInstanceId
        ? `${result.applicationId}/${result.providerInstanceId}`
        : 'unidentified';
      counts.set(key, (counts.get(key) ?? 0) + 1);
      return counts;
    }, new Map())),
    overlapEvidence,
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
  modelByTransport = {},
  mockSnapshot,
  timeoutMs = 5_000,
  fetchImpl = globalThis.fetch,
  WebSocketImpl = globalThis.WebSocket,
  targetPoolsByTransport = {},
  durableGraceMs,
  durablePollIntervalMs,
}) {
  if (!Array.isArray(plan) || plan.length === 0) throw new Error('characterize plan must not be empty');
  if (typeof fetchImpl !== 'function') throw new Error('fetch implementation is unavailable');
  const requestHeaders = normalizeHeadersByTransport(headersByTransport);
  const requestModels = normalizeModelByTransport(modelByTransport);
  const batches = [];
  const events = [];
  const durableRows = [];
  let nonceSequence = 0;
  let batchSequence = 0;
  for (const row of plan) {
    assertTransport(row.transport);
    assertScenario(row.scenario);
    if (!Number.isInteger(row.concurrency) || row.concurrency < 1 || row.concurrency > 32) {
      throw new Error(`invalid characterize concurrency: ${row.concurrency}`);
    }
    const topology = row.topology ?? TOPOLOGY.SAME_POOL;
    if (![TOPOLOGY.SAME_POOL, TOPOLOGY.MULTI_POOL].includes(topology)) {
      throw new Error(`invalid characterize topology: ${topology}`);
    }
    const configuredPool = targetPoolsByTransport[row.transport];
    if (topology === TOPOLOGY.MULTI_POOL && (!Array.isArray(configuredPool) || configuredPool.length !== 2)) {
      throw new Error(`multi-pool row requires two targets for ${row.transport}`);
    }
    const fallbackTarget = {
      endpoint: endpointUrl(endpointSet, row.transport),
      headers: requestHeaders[row.transport] ?? {},
      model: row.transport === TRANSPORT.RESPONSES_WEBSOCKET
        ? 'mock-model'
        : (requestModels[row.transport] ?? 'mock-model'),
      applicationId: null,
      providerInstanceId: null,
      durableTarget: null,
      activeStreamsEndpoint: null,
    };
    const pool = Array.isArray(configuredPool) && configuredPool.length ? configuredPool : [fallbackTarget];
    const before = mockSnapshot?.();
    const batchStartedAt = performance.now();
    batchSequence += 1;
    const batchBarrierId = `batch-${String(batchSequence).padStart(3, '0')}`;
    let releaseBarrier;
    const barrier = new Promise((resolve) => { releaseBarrier = resolve; });
    const requests = Array.from({ length: row.concurrency }, (_, requestIndex) => {
      nonceSequence += 1;
      const targetIndex = topology === TOPOLOGY.MULTI_POOL ? requestIndex % pool.length : 0;
      const target = pool[targetIndex];
      const request = {
        endpoint: target.endpoint,
        transport: row.transport,
        scenario: row.scenario,
        clientNonce: createClientNonce(nonceSequence),
        model: target.model,
        headers: target.headers,
        timeoutMs,
        batchStartedAt,
        topology,
        batchBarrierId,
        targetIndex,
        applicationId: target.applicationId,
        providerInstanceId: target.providerInstanceId,
      };
      const pending = (async () => {
        await barrier;
        if (row.transport === TRANSPORT.RESPONSES_WEBSOCKET) {
          if (typeof WebSocketImpl !== 'function') throw new Error('WebSocket implementation is unavailable');
          return runWebSocketRequest({ ...request, WebSocketImpl });
        }
        return runSseRequest({ ...request, fetchImpl });
      })();
      return pending.catch((error) => requestErrorResult(request, error));
    });
    let batchSettled = false;
    const observeOverlap = topology === TOPOLOGY.MULTI_POOL && row.scenario === SCENARIO.SLOW;
    const overlapPending = observeOverlap ? observeActiveStreamOverlap({
      endpoint: pool[0].activeStreamsEndpoint,
      expectedInstanceIds: pool.map((target) => target.providerInstanceId),
      fetchImpl,
      isSettled: () => batchSettled,
    }) : null;
    releaseBarrier();
    const results = await Promise.all(requests);
    batchSettled = true;
    const overlapEvidence = overlapPending ? await overlapPending : null;
    const durationMs = performance.now() - batchStartedAt;
    const after = await waitForMockBatch(before, mockSnapshot, row.concurrency, timeoutMs);
    const evidence = mockBatchEvidence(before, after, results);
    const rowDurableLedgers = [];
    for (const targetIndex of [...new Set(results.map((result) => result.targetIndex))]) {
      const target = pool[targetIndex];
      if (!target?.durableTarget) continue;
      const ledger = await collectDurableConvergence({
        requestEvents: results.filter((result) => result.targetIndex === targetIndex),
        targetsByTransport: { [row.transport]: target.durableTarget },
        fetchImpl,
        graceMs: durableGraceMs,
        pollIntervalMs: durablePollIntervalMs,
      });
      rowDurableLedgers.push({ targetIndex, applicationId: target.applicationId, ledger });
      durableRows.push({ batchBarrierId, topology, transport: row.transport, scenario: row.scenario, targetIndex, ledger });
    }
    const durableFailures = rowDurableLedgers.flatMap(({ targetIndex, ledger }) =>
      ledger.failures.map((message) => `target ${targetIndex}: ${message}`));
    const summary = batchSummary({ row: { ...row, topology, batchBarrierId }, results, durationMs, mockEvidence: evidence, overlapEvidence, durableFailures });
    batches.push(summary);
    events.push(...results.map((result) => ({
      kind: 'request',
      ...result,
      topologyOverlapObserved: overlapEvidence?.observed ?? null,
    })));
    if (overlapEvidence) {
      events.push({
        kind: 'topology-overlap',
        batchBarrierId,
        topology,
        applicationIds: pool.map((target) => target.applicationId),
        providerInstanceIds: pool.map((target) => target.providerInstanceId),
        ...overlapEvidence,
      });
    }
    events.push(...evidence.arrivalEntries.map((entry) => ({ kind: 'mock-timeline', ...entry })));
  }
  const failures = batches.flatMap((batch) => batch.failures.map((failure) => ({
    batch: `${batch.topology}/${batch.transport}/${batch.scenario}/c${batch.concurrency}`,
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
    durableLedger: {
      schemaVersion: 1,
      verdict: durableRows.every((row) => row.ledger.verdict === 'PASS') ? 'PASS' : 'FAIL',
      rows: durableRows,
      failures: durableRows.flatMap((row) => row.ledger.failures),
    },
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
      plan: CHARACTERIZE_PLAN.filter((row) => row.topology === TOPOLOGY.SAME_POOL),
      mockSnapshot: mock.snapshot,
      timeoutMs,
    });
    return { ...result, artifacts: writeCharacterizeArtifacts({ repoRoot, ...result }) };
  } finally {
    await mock.stop();
  }
}

async function runGatewayCharacterize({
  repoRoot,
  endpointSet,
  authorizationTokenByTransport,
  modelByTransport,
  mockSnapshot,
  timeoutMs,
  durableTargetsByTransport,
  durableGraceMs,
  durablePollIntervalMs,
  anthropicTargetPool,
  fetchImpl = globalThis.fetch,
}) {
  const headersByTransport = authorizationHeadersByTransport(authorizationTokenByTransport);
  const publishedModels = requirePublishedModelsByTransport(modelByTransport);
  const anthropicPool = requireAnthropicTargetPool(anthropicTargetPool);
  const targetPoolsByTransport = {
    [TRANSPORT.ANTHROPIC_SSE]: anthropicPool.map(
      (target) => gatewayRequestTarget(TRANSPORT.ANTHROPIC_SSE, target)
    ),
    [TRANSPORT.RESPONSES_SSE]: [{
      endpoint: endpointUrl(endpointSet, TRANSPORT.RESPONSES_SSE),
      headers: headersByTransport[TRANSPORT.RESPONSES_SSE],
      model: publishedModels[TRANSPORT.RESPONSES_SSE],
      applicationId: durableTargetsByTransport[TRANSPORT.RESPONSES_SSE].application_id,
      providerInstanceId: durableTargetsByTransport[TRANSPORT.RESPONSES_SSE].provider_instance_id,
      durableTarget: durableTargetsByTransport[TRANSPORT.RESPONSES_SSE],
      activeStreamsEndpoint: durableTargetsByTransport[TRANSPORT.RESPONSES_SSE].plugin_runner_active_streams,
    }],
  };
  const result = await executeCharacterizePlan({
    endpointSet,
    plan: CHARACTERIZE_PLAN,
    headersByTransport,
    modelByTransport: publishedModels,
    mockSnapshot,
    timeoutMs,
    fetchImpl,
    targetPoolsByTransport,
    durableGraceMs,
    durablePollIntervalMs,
  });
  const durableLedger = result.durableLedger;
  result.summary.durableConvergence = {
    verdict: durableLedger.verdict,
    requests: durableLedger.rows.reduce((total, row) => total + row.ledger.requests.length, 0),
    polls: durableLedger.rows.reduce((total, row) => total + row.ledger.polls.length, 0),
    rows: durableLedger.rows.length,
  };
  return {
    ...result,
    durableLedger,
    artifacts: writeCharacterizeArtifacts({ repoRoot, ...result, durableLedger }),
  };
}

module.exports = {
  authorizationHeadersByTransport,
  executeCharacterizePlan,
  hasExpectedActiveStreamOverlap,
  identifiedSamePoolMockPeakFailure,
  normalizeHeadersByTransport,
  normalizeModelByTransport,
  requirePublishedModelsByTransport,
  requireAnthropicTargetPool,
  runDirectMockCharacterize,
  runGatewayCharacterize,
  validateRequestResult,
};
