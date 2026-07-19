'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { MOCK_ROUTE, SCENARIO, TRANSPORT } = require('../../contracts');
const { createMockUpstream } = require('../../mock-upstream');
const { executeCharacterizePlan, validateRequestResult } = require('../engine');
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
