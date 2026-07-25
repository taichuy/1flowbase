'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { MOCK_ROUTE, SCENARIO, TRANSPORT, mockScenarioSentinel } = require('../../contracts');
const { createMockUpstream } = require('../../mock-upstream');
const {
  authorizationHeadersByTransport,
  countMockScenarioObservations,
  executeCharacterizePlan,
  hasExpectedActiveStreamOverlap,
  identifiedSamePoolMockPeakFailure,
  normalizeHeadersByTransport,
  normalizeModelByTransport,
  requirePublishedModelsByTransport,
  requireAnthropicTargetPool,
  validateRequestResult,
} = require('../engine');
const {
  CHARACTERIZE_PLAN,
  CORRECTNESS_CONCURRENCY,
  GATE_ROLE,
  PERFORMANCE_CONCURRENCY,
  TOPOLOGY,
} = require('../plan');

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

test('AC-004: Gateway converts accepted HTTP upstream interruption to explicit failure', () => {
  const base = {
    transport: TRANSPORT.RESPONSES_SSE,
    scenario: SCENARIO.STREAM_INTERRUPTION,
    terminalCount: 0,
    chunkTexts: [],
    upstreamNonce: null,
    upstreamNonceCount: 0,
  };
  assert.deepEqual(validateRequestResult({ ...base, outcome: 'interrupted' }), []);
  assert.deepEqual(validateRequestResult({
    ...base,
    outcome: 'failed',
    applicationId: 'application-1',
    providerInstanceId: 'provider-1',
  }), []);
  assert.match(validateRequestResult({
    ...base,
    outcome: 'interrupted',
    applicationId: 'application-1',
    providerInstanceId: 'provider-1',
  }).join('\n'), /expected outcome failed/u);
});

test('AC-011: downstream disconnect preserves an accepted Gateway run but cancels a direct mock request', () => {
  const cancelledEntries = [{ event: 'cancel_observed' }];
  const completedEntries = [{ event: 'settled', outcome: 'completed' }];
  assert.equal(countMockScenarioObservations(
    SCENARIO.CANCEL_OBSERVATION,
    cancelledEntries,
    [{ applicationId: null, providerInstanceId: null }],
  ), 1);
  assert.equal(countMockScenarioObservations(
    SCENARIO.CANCEL_OBSERVATION,
    completedEntries,
    [{ applicationId: 'application-1', providerInstanceId: 'provider-1' }],
  ), 1);
  assert.equal(countMockScenarioObservations(
    SCENARIO.CANCEL_OBSERVATION,
    cancelledEntries,
    [{ applicationId: 'application-1', providerInstanceId: 'provider-1' }],
  ), 0);
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
        prompt: JSON.stringify(JSON.parse(options.body)),
        metadata: JSON.parse(options.body).metadata,
        traceId: JSON.parse(options.body).metadata.trace_id,
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
    assert.deepEqual(fetchCalls.map((call) => call.prompt.includes(mockScenarioSentinel(SCENARIO.NORMAL))), [true, true]);
    assert.deepEqual(fetchCalls.map((call) => call.metadata), [
      { trace_id: 'load-000001' },
      { trace_id: 'load-000003' },
    ]);
    assert.deepEqual(fetchCalls.map((call) => call.traceId), ['load-000001', 'load-000003']);
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
    assert.deepEqual(websocketRequests[0].response.metadata, { trace_id: 'load-000002' });
    assert.equal(JSON.stringify(websocketRequests[0].response).includes(mockScenarioSentinel(SCENARIO.NORMAL)), true);
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

test('AC topology: each row has one barrier; same-pool pins first and multi-pool round-robins', async () => {
  await withMock(async ({ mock, endpointSet }) => {
    const calls = [];
    const endpoint = endpointSet[TRANSPORT.ANTHROPIC_SSE];
    const result = await executeCharacterizePlan({
      endpointSet,
      plan: [TOPOLOGY.SAME_POOL, TOPOLOGY.MULTI_POOL].map((topology) => ({
        transport: TRANSPORT.ANTHROPIC_SSE,
        scenario: SCENARIO.NORMAL,
        concurrency: 4,
        topology,
      })),
      targetPoolsByTransport: {
        [TRANSPORT.ANTHROPIC_SSE]: [0, 1].map((index) => ({
          endpoint: new URL(endpoint),
          headers: { authorization: `Bearer pool-${index}` },
          model: 'published-anthropic-model',
          applicationId: null,
          providerInstanceId: null,
          durableTarget: null,
          activeStreamsEndpoint: null,
        })),
      },
      fetchImpl(url, options) {
        calls.push({ authorization: options.headers.authorization, traceId: JSON.parse(options.body).metadata.trace_id });
        return fetch(url, options);
      },
      mockSnapshot: mock.snapshot,
      timeoutMs: 1_000,
    });
    assert.equal(result.summary.verdict, 'PASS');
    assert.deepEqual(calls.map((call) => call.authorization), [
      'Bearer pool-0', 'Bearer pool-0', 'Bearer pool-0', 'Bearer pool-0',
      'Bearer pool-0', 'Bearer pool-1', 'Bearer pool-0', 'Bearer pool-1',
    ]);
    const requests = result.events.filter((event) => event.kind === 'request');
    assert.deepEqual(requests.map((event) => event.targetIndex), [0, 0, 0, 0, 0, 1, 0, 1]);
    assert.equal(new Set(requests.map((event) => event.batchBarrierId)).size, 2);
    assert.equal(requests.every((event) => event.applicationId === null), true);
  });
});

test('QA F2: identified OpenAI and Anthropic same-pool HTTP SSE peak2 fails and peak1 passes', () => {
  const results = [1, 2].map(() => ({
    applicationId: 'application-1',
    providerInstanceId: 'instance-1',
  }));
  for (const transport of [TRANSPORT.RESPONSES_SSE, TRANSPORT.ANTHROPIC_SSE]) {
    const row = { topology: TOPOLOGY.SAME_POOL, transport };
    assert.equal(identifiedSamePoolMockPeakFailure(row, results, { mockArrivalPeak: 1 }), null);
    assert.match(
      identifiedSamePoolMockPeakFailure(row, results, { mockArrivalPeak: 2 }),
      /expected mock arrival peak 1, received 2/u,
    );
  }
});

test('QA F2 exclusions: unidentified direct mock, Responses WebSocket, and multi-pool ignore peak2', () => {
  const identified = [{ applicationId: 'application-1', providerInstanceId: 'instance-1' }];
  assert.equal(identifiedSamePoolMockPeakFailure({
    topology: TOPOLOGY.SAME_POOL,
    transport: TRANSPORT.ANTHROPIC_SSE,
  }, [{ applicationId: null, providerInstanceId: null }], { mockArrivalPeak: 2 }), null);
  assert.equal(identifiedSamePoolMockPeakFailure({
    topology: TOPOLOGY.SAME_POOL,
    transport: TRANSPORT.RESPONSES_WEBSOCKET,
  }, identified, { mockArrivalPeak: 2 }), null);
  assert.equal(identifiedSamePoolMockPeakFailure({
    topology: TOPOLOGY.MULTI_POOL,
    transport: TRANSPORT.ANTHROPIC_SSE,
  }, identified, { mockArrivalPeak: 2 }), null);
});

test('AC multi-pool controlled negatives: missing tuple and duplicate identity/key fail closed', () => {
  const target = (ordinal) => ({
    application_id: `application-${ordinal}`,
    provider_instance_id: `instance-${ordinal}`,
    api_key: `key-${ordinal}`,
    model: 'shared-model',
    publication_id: `publication-${ordinal}`,
    gateway: { anthropic_messages_url: 'http://127.0.0.1:4100/v1/messages' },
    durable: {
      query_run: { url_template: 'http://127.0.0.1:4100/api/agent/v1/runs/{run_id}' },
      list_runs: { url: `http://127.0.0.1:4100/applications/${ordinal}/runs` },
    },
    runtime_activity: { url: `http://127.0.0.1:4100/applications/${ordinal}/activity` },
    plugin_runner_active_streams: { url: 'http://127.0.0.1:4200/providers/active-streams' },
  });
  assert.throws(() => requireAnthropicTargetPool([target(1)]), /exactly two/u);
  assert.throws(() => requireAnthropicTargetPool([target(1), { ...target(2), provider_instance_id: 'instance-1' }]), /reused provider_instance_id/u);
  assert.throws(() => requireAnthropicTargetPool([target(1), { ...target(2), api_key: 'key-1' }]), /reused api_key/u);
  assert.equal(hasExpectedActiveStreamOverlap(['instance-1', 'instance-2'], [[
    { providerInstanceId: 'instance-1' },
  ]]), false);
  assert.equal(hasExpectedActiveStreamOverlap(['instance-1', 'instance-2'], [[
    { providerInstanceId: 'instance-1' }, { providerInstanceId: 'instance-2' },
  ]]), true);
});

test('AC-003/004: characterize matrix separates blocking correctness from performance observations', () => {
  assert.deepEqual(CORRECTNESS_CONCURRENCY, [1, 4]);
  assert.deepEqual(PERFORMANCE_CONCURRENCY, [16, 32]);
  for (const concurrency of [...CORRECTNESS_CONCURRENCY, ...PERFORMANCE_CONCURRENCY]) {
    const normalRows = CHARACTERIZE_PLAN.filter((row) => (
      row.topology === TOPOLOGY.SAME_POOL
      && row.scenario === SCENARIO.NORMAL
      && row.concurrency === concurrency
    ));
    assert.deepEqual(new Set(normalRows.map((row) => row.transport)), new Set(Object.values(TRANSPORT)));
    assert.equal(
      normalRows.every((row) => row.gateRole === (
        CORRECTNESS_CONCURRENCY.includes(concurrency) ? GATE_ROLE.BLOCKING : GATE_ROLE.ADVISORY
      )),
      true,
    );
  }
  for (const scenario of [SCENARIO.SLOW, SCENARIO.CANCEL_OBSERVATION, SCENARIO.HTTP_500, SCENARIO.STREAM_INTERRUPTION]) {
    assert.deepEqual(
      new Set(CHARACTERIZE_PLAN.filter((row) => (
        row.gateRole === GATE_ROLE.BLOCKING && row.scenario === scenario
      )).map((row) => row.transport)),
      new Set(Object.values(TRANSPORT)),
    );
  }
  assert.equal(CHARACTERIZE_PLAN.filter((row) => (
    row.gateRole === GATE_ROLE.BLOCKING && row.scenario !== SCENARIO.NORMAL
  )).every((row) => row.concurrency === 1), true);
  const multiRows = CHARACTERIZE_PLAN.filter((row) => row.topology === TOPOLOGY.MULTI_POOL);
  assert.deepEqual(multiRows.map((row) => [row.transport, row.scenario, row.concurrency, row.gateRole]), [
    [TRANSPORT.ANTHROPIC_SSE, SCENARIO.NORMAL, 2, GATE_ROLE.BLOCKING],
    [TRANSPORT.ANTHROPIC_SSE, SCENARIO.NORMAL, 16, GATE_ROLE.ADVISORY],
    [TRANSPORT.ANTHROPIC_SSE, SCENARIO.NORMAL, 32, GATE_ROLE.ADVISORY],
    [TRANSPORT.ANTHROPIC_SSE, SCENARIO.SLOW, 4, GATE_ROLE.ADVISORY],
  ]);
  assert.equal(CHARACTERIZE_PLAN.filter((row) => row.gateRole === GATE_ROLE.BLOCKING).reduce((total, row) => total + row.concurrency, 0), 29);
  assert.equal(CHARACTERIZE_PLAN.filter((row) => row.gateRole === GATE_ROLE.ADVISORY).reduce((total, row) => total + row.concurrency, 0), 196);
  assert.equal(CHARACTERIZE_PLAN.reduce((total, row) => total + row.concurrency, 0), 225);
});

test('AC-029: non-blocking performance failures remain visible without changing the protocol verdict', async () => {
  const result = await executeCharacterizePlan({
    endpointSet: { [TRANSPORT.RESPONSES_SSE]: 'http://127.0.0.1:1/v1/responses' },
    plan: [{
      transport: TRANSPORT.RESPONSES_SSE,
      scenario: SCENARIO.NORMAL,
      concurrency: 1,
      gateRole: GATE_ROLE.ADVISORY,
    }],
    async fetchImpl() { throw new Error('controlled performance timeout'); },
    timeoutMs: 10,
  });
  assert.equal(result.summary.verdict, 'PASS');
  assert.equal(result.summary.totals.contractFailures, 0);
  assert.equal(result.summary.totals.advisoryFailures > 0, true);
  assert.match(result.summary.advisories.map((item) => item.message).join('\n'), /expected outcome completed/u);
});

test('AC-003: invalid plan rows fail closed before load generation', async () => {
  await assert.rejects(
    executeCharacterizePlan({
      endpointSet: {},
      plan: [{ transport: 'unknown', scenario: SCENARIO.NORMAL, concurrency: 33 }],
    }),
    /unsupported mock transport/u,
  );
  await assert.rejects(
    executeCharacterizePlan({
      endpointSet: {},
      plan: [{
        transport: TRANSPORT.RESPONSES_SSE,
        scenario: SCENARIO.NORMAL,
        concurrency: 1,
        gateRole: 'ignored-failure',
      }],
    }),
    /invalid characterize gate role/u,
  );
});
