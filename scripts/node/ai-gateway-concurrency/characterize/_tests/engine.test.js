'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { MOCK_ROUTE, SCENARIO, TRANSPORT } = require('../../contracts');
const { createMockUpstream } = require('../../mock-upstream');
const {
  authorizationHeadersByTransport,
  executeCharacterizePlan,
  normalizeHeadersByTransport,
  normalizeModelByTransport,
  requirePublishedModelsByTransport,
  validateRequestResult,
} = require('../engine');
const { CHARACTERIZE_CONCURRENCY, CHARACTERIZE_PLAN } = require('../plan');

async function withMock(run) {
  const mock = createMockUpstream({ slowChunkDelayMs: 10, cancelObservationMs: 150 });
  const endpoints = await mock.start();
  try {
    await run({
      mock,
      endpointSet: {
        [TRANSPORT.RESPONSES_SSE]: `${endpoints.httpBaseUrl}${MOCK_ROUTE.RESPONSES}`,
        [TRANSPORT.RESPONSES_WEBSOCKET]: `${endpoints.websocketBaseUrl}${MOCK_ROUTE.RESPONSES}`,
        [TRANSPORT.ANTHROPIC_SSE]: `${endpoints.httpBaseUrl}${MOCK_ROUTE.ANTHROPIC_MESSAGES}`,
      },
    });
  } finally {
    await mock.stop();
  }
}

test('AC-003/004/005: finite characterize fixture classifies transports, failures, cancellation, and peak', async () => {
  await withMock(async ({ mock, endpointSet }) => {
    const directMockModels = [];
    const plan = [
      { transport: TRANSPORT.RESPONSES_SSE, scenario: SCENARIO.NORMAL, concurrency: 2 },
      { transport: TRANSPORT.ANTHROPIC_SSE, scenario: SCENARIO.NORMAL, concurrency: 1 },
      { transport: TRANSPORT.RESPONSES_WEBSOCKET, scenario: SCENARIO.NORMAL, concurrency: 1 },
      { transport: TRANSPORT.RESPONSES_SSE, scenario: SCENARIO.SLOW, concurrency: 2 },
      { transport: TRANSPORT.RESPONSES_SSE, scenario: SCENARIO.CANCEL_OBSERVATION, concurrency: 1 },
      { transport: TRANSPORT.ANTHROPIC_SSE, scenario: SCENARIO.HTTP_500, concurrency: 1 },
      { transport: TRANSPORT.RESPONSES_WEBSOCKET, scenario: SCENARIO.STREAM_INTERRUPTION, concurrency: 1 },
    ];
    const result = await executeCharacterizePlan({
      endpointSet,
      plan,
      fetchImpl(url, options) {
        directMockModels.push(JSON.parse(options.body).model);
        return fetch(url, options);
      },
      mockSnapshot: mock.snapshot,
      timeoutMs: 1_000,
    });
    assert.equal(result.summary.verdict, 'PASS');
    assert.equal(result.summary.performanceBudgetApplied, false);
    assert.equal(result.summary.totals.requests, 9);
    assert.equal(result.summary.totals.contractFailures, 0);
    assert.equal(result.summary.metrics.mockArrivalPeak >= 2, true);
    assert.equal(result.summary.batches.every((batch) => batch.pass), true);
    assert.equal(result.summary.batches.every((batch) => typeof batch.metrics.throughputRps === 'number'), true);
    assert.equal(result.summary.batches.some((batch) => typeof batch.metrics.derivedQueueMaxMs === 'number'), true);
    assert.equal(directMockModels.every((model) => model === 'mock-model'), true);
    const cancelled = result.events.find((event) => event.kind === 'request' && event.scenario === SCENARIO.CANCEL_OBSERVATION);
    assert.equal(cancelled.outcome, 'cancelled');
    assert.equal(cancelled.terminalCount, 0);
    const interrupted = result.events.find((event) => event.kind === 'request' && event.scenario === SCENARIO.STREAM_INTERRUPTION);
    assert.equal(interrupted.outcome, 'interrupted');
    assert.equal(interrupted.terminalCount, 0);
  });
});

test('AC-003 controlled negative: mixed upstream nonces fail chunk authenticity', () => {
  const failures = validateRequestResult({
    scenario: SCENARIO.NORMAL,
    outcome: 'completed',
    terminalCount: 1,
    chunkTexts: ['mock-000001:chunk-1', 'mock-000002:chunk-2'],
    upstreamNonce: null,
    upstreamNonceCount: 2,
  });
  assert.equal(failures.some((failure) => failure.includes('expected one upstream nonce')), true);
});

test('AC-003: one execute call keeps global nonce order and transport-specific authorization', async () => {
  await withMock(async ({ mock, endpointSet }) => {
    const fetchCalls = [];
    const websocketCalls = [];
    const websocketRequests = [];
    const fetchImpl = (url, options) => {
      fetchCalls.push({
        url: String(url),
        authorization: options.headers.authorization,
        clientNonce: JSON.parse(options.body).metadata.request_nonce,
        model: JSON.parse(options.body).model,
      });
      return fetch(url, options);
    };
    const WebSocketImpl = new Proxy(WebSocket, {
      construct(Target, args) {
        websocketCalls.push(args);
        const socket = Reflect.construct(Target, args);
        const send = socket.send.bind(socket);
        socket.send = (value) => {
          const request = JSON.parse(value);
          if (request.type === 'response.create') websocketRequests.push(request);
          return send(value);
        };
        return socket;
      },
    });
    const result = await executeCharacterizePlan({
      endpointSet,
      plan: [
        { transport: TRANSPORT.RESPONSES_SSE, scenario: SCENARIO.NORMAL, concurrency: 1 },
        { transport: TRANSPORT.RESPONSES_WEBSOCKET, scenario: SCENARIO.NORMAL, concurrency: 1 },
        { transport: TRANSPORT.ANTHROPIC_SSE, scenario: SCENARIO.NORMAL, concurrency: 1 },
      ],
      headersByTransport: {
        [TRANSPORT.RESPONSES_SSE]: { authorization: 'Bearer responses-key' },
        [TRANSPORT.ANTHROPIC_SSE]: { authorization: 'Bearer anthropic-key' },
      },
      modelByTransport: {
        [TRANSPORT.RESPONSES_SSE]: 'published-openai-model',
        [TRANSPORT.ANTHROPIC_SSE]: 'published-anthropic-model',
      },
      fetchImpl,
      WebSocketImpl,
      mockSnapshot: mock.snapshot,
      timeoutMs: 1_000,
    });
    assert.equal(result.summary.verdict, 'PASS');
    assert.deepEqual(fetchCalls.map((call) => call.authorization), [
      'Bearer responses-key',
      'Bearer anthropic-key',
    ]);
    assert.deepEqual(fetchCalls.map((call) => call.clientNonce), ['load-000001', 'load-000003']);
    assert.deepEqual(fetchCalls.map((call) => call.model), ['published-openai-model', 'published-anthropic-model']);
    assert.deepEqual(
      result.events.filter((event) => event.kind === 'request').map((event) => event.clientNonce),
      ['load-000001', 'load-000002', 'load-000003'],
    );
    assert.equal(websocketCalls.length, 1);
    assert.equal(websocketCalls[0].length, 1);
    assert.equal(String(websocketCalls[0][0]).includes('key'), false);
    assert.equal(websocketRequests.length, 1);
    assert.equal(websocketRequests[0].response.model, 'mock-model');
    assert.equal(websocketRequests[0].response.metadata.request_nonce, 'load-000002');
  });
});

test('AC-003 controlled negatives: WebSocket, unknown, missing, and shared authorization fail closed', () => {
  assert.throws(
    () => normalizeHeadersByTransport({ [TRANSPORT.RESPONSES_WEBSOCKET]: { authorization: 'Bearer forbidden' } }),
    /headers are not allowed for transport: responses-websocket/u,
  );
  assert.throws(
    () => authorizationHeadersByTransport({
      [TRANSPORT.RESPONSES_SSE]: 'responses-key',
      [TRANSPORT.ANTHROPIC_SSE]: 'anthropic-key',
      unknown: 'unknown-key',
    }),
    /authorization token is not allowed for transport: unknown/u,
  );
  assert.throws(
    () => authorizationHeadersByTransport({ [TRANSPORT.RESPONSES_SSE]: 'responses-key' }),
    /authorization token is required for transport: anthropic-sse/u,
  );
  assert.throws(
    () => authorizationHeadersByTransport({
      [TRANSPORT.RESPONSES_SSE]: 'shared-key',
      [TRANSPORT.ANTHROPIC_SSE]: 'shared-key',
    }),
    /must use distinct Application API keys/u,
  );
});

test('AC-003 controlled negatives: WebSocket, unknown, and missing published models fail closed', () => {
  assert.throws(
    () => normalizeModelByTransport({ [TRANSPORT.RESPONSES_WEBSOCKET]: 'forbidden-model' }),
    /model is not allowed for transport: responses-websocket/u,
  );
  assert.throws(
    () => normalizeModelByTransport({ unknown: 'unknown-model' }),
    /model is not allowed for transport: unknown/u,
  );
  assert.throws(
    () => requirePublishedModelsByTransport({ [TRANSPORT.RESPONSES_SSE]: 'published-openai-model' }),
    /published model is required for transport: anthropic-sse/u,
  );
});

test('AC-003/004: characterize matrix fixes 1/4/16/32 normal load and finite fault rows', () => {
  assert.deepEqual(CHARACTERIZE_CONCURRENCY, [1, 4, 16, 32]);
  for (const concurrency of CHARACTERIZE_CONCURRENCY) {
    const normalRows = CHARACTERIZE_PLAN.filter((row) => row.scenario === SCENARIO.NORMAL && row.concurrency === concurrency);
    assert.deepEqual(new Set(normalRows.map((row) => row.transport)), new Set(Object.values(TRANSPORT)));
  }
  for (const scenario of [SCENARIO.SLOW, SCENARIO.CANCEL_OBSERVATION, SCENARIO.HTTP_500, SCENARIO.STREAM_INTERRUPTION]) {
    assert.deepEqual(
      new Set(CHARACTERIZE_PLAN.filter((row) => row.scenario === scenario).map((row) => row.transport)),
      new Set(Object.values(TRANSPORT)),
    );
  }
  assert.equal(CHARACTERIZE_PLAN.filter((row) => row.scenario === SCENARIO.SLOW).every((row) => row.concurrency === 4), true);
  assert.equal(CHARACTERIZE_PLAN.filter((row) => [SCENARIO.CANCEL_OBSERVATION, SCENARIO.HTTP_500, SCENARIO.STREAM_INTERRUPTION].includes(row.scenario)).every((row) => row.concurrency === 1), true);
  assert.equal(CHARACTERIZE_PLAN.reduce((total, row) => total + row.concurrency, 0), 180);
});

test('AC-003: invalid plan rows fail closed before load generation', async () => {
  await assert.rejects(
    executeCharacterizePlan({
      endpointSet: {},
      plan: [{ transport: 'unknown', scenario: SCENARIO.NORMAL, concurrency: 33 }],
    }),
    /unsupported mock transport/u,
  );
});
