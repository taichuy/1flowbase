'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  evaluateMockAttempt, networkObserverEvidence, reconcileAttempt, snapshotRuns, verifyIdle,
  waitForBarrierWaiting,
} = require('../durable');
const { PROVIDER_ERROR_BODY } = require('../vector-manifest');

function response(data) {
  return { ok: true, async json() { return { data }; } };
}

const target = {
  durable: {
    list_runs: { url: 'http://fixture/runs', headers: { cookie: 'secret' } },
    query_run: { url_template: 'http://fixture/runs/{run_id}', headers: { authorization: 'secret' } },
  },
  runtimeActivity: { url: 'http://fixture/activity' },
  activeStreams: { url: 'http://fixture/streams' },
};

test('WP-14A reconciles one terminal durable run for text and callback-resumed tool turns', async () => {
  const before = { ids: ['old-run'] };
  const payloads = {
    'http://fixture/runs': { items: [
      { id: 'old-run', status: 'succeeded' },
      { id: 'new-1', status: 'succeeded' },
      { id: 'new-2', status: 'succeeded' },
    ] },
    'http://fixture/runs/new-1': { id: 'new-1', status: 'succeeded' },
    'http://fixture/runs/new-2': { id: 'new-2', status: 'succeeded' },
  };
  const fetchImpl = async (url) => response(payloads[url]);
  const tool = await reconcileAttempt({
    target, before: { ids: ['old-run', 'new-2'] }, expectedRuns: 1, fetchImpl, graceMs: 0,
  });
  assert.deepEqual(tool.runs.map((run) => run.id), ['new-1']);
  const text = await reconcileAttempt({
    target, before: { ids: ['old-run', 'new-1'] }, expectedRuns: 1, fetchImpl, graceMs: 0,
  });
  assert.deepEqual(text.runs.map((run) => run.id), ['new-2']);
  assert.deepEqual(await snapshotRuns(target, fetchImpl), {
    ids: ['old-run', 'new-1', 'new-2'],
    runs: payloads['http://fixture/runs'].items,
  });
  await assert.rejects(
    reconcileAttempt({ target, before, expectedRuns: 1, fetchImpl, graceMs: 0 }),
    /expected exactly 1 new durable run, observed 2/u,
  );
});

test('WP-14A mock evidence proves one text arrival and ordered two-turn tool arrivals', () => {
  const before = { entries: [{ sequence: 4 }] };
  const text = { entries: [
    { sequence: 4, event: 'settled', nonce: 'old' },
    { sequence: 5, event: 'arrival', nonce: 'text' },
    { sequence: 6, event: 'settled', nonce: 'text', outcome: 'completed' },
  ] };
  const after = { entries: [
    { sequence: 4, event: 'settled', nonce: 'old' },
    { sequence: 5, event: 'arrival', nonce: 'one' },
    { sequence: 6, event: 'tool_call', nonce: 'one' },
    { sequence: 7, event: 'settled', nonce: 'one', outcome: 'completed' },
    { sequence: 8, event: 'arrival', nonce: 'two' },
    { sequence: 9, event: 'second_upstream_request', nonce: 'two' },
    { sequence: 10, event: 'settled', nonce: 'two', outcome: 'completed' },
  ] };
  assert.equal(evaluateMockAttempt(before, text, 1).arrivals, 1);
  assert.equal(evaluateMockAttempt(before, after, 2).arrivals, 2);
  assert.throws(() => evaluateMockAttempt(before, after, 1), /expected 1 mock arrival/u);
});

test('WP-D4B mock evidence fixes terminal counts, Claude request keys, and executor=0', () => {
  const before = {
    entries: [{ sequence: 10 }],
    counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
  };
  const after = {
    entries: [
      ...before.entries,
      {
        sequence: 11,
        event: 'arrival',
        nonce: 'claude-profile',
        request: { body: { keys: ['context_management', 'messages', 'output_config', 'thinking'] } },
      },
      {
        sequence: 12,
        event: 'settled',
        nonce: 'claude-profile',
        outcome: 'completed',
        successTerminalCount: 1,
      },
    ],
    counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
  };
  const evidence = evaluateMockAttempt(before, after, {
    provider_requests: 1,
    provider_outcomes: ['completed'],
    success_terminal_counts: [1],
    request_body_keys: ['context_management', 'output_config', 'thinking'],
    gateway_executor_invocations: 0,
    network_observer_outbound: 0,
  });
  assert.equal(evidence.gateway_executor_invocations, 0);
  assert.equal(evidence.network_observer_outbound, 0);
  assert.deepEqual(evidence.success_terminal_counts, [1]);
  assert.throws(() => evaluateMockAttempt(before, {
    ...after,
    counters: { gatewayExecutorInvocations: 1 },
  }, {
    provider_requests: 1,
    provider_outcomes: ['completed'],
    success_terminal_counts: [1],
    gateway_executor_invocations: 0,
  }), /expected gateway executor=0/u);
  assert.throws(() => networkObserverEvidence({
    counters: { networkObserverOutbound: 1 },
  }, 0), /expected network observer outbound=0/u);
});

test('F4-CLIENT-GATE accepts legal callback request counts but requires paired lifecycle and resume chronology', () => {
  const before = {
    entries: [{ sequence: 20 }],
    counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
  };
  const entries = [
    ...before.entries,
    { sequence: 21, event: 'arrival', nonce: 'initial' },
    { sequence: 22, event: 'tool_call', nonce: 'initial' },
    { sequence: 23, event: 'settled', nonce: 'initial', outcome: 'completed', successTerminalCount: 1 },
    { sequence: 24, event: 'arrival', nonce: 'callback' },
    { sequence: 25, event: 'second_upstream_request', nonce: 'callback' },
    { sequence: 26, event: 'barrier_waiting', nonce: 'callback' },
    { sequence: 27, event: 'barrier_released', nonce: 'callback' },
    { sequence: 28, event: 'settled', nonce: 'callback', outcome: 'completed', successTerminalCount: 1 },
  ];
  const expectation = {
    minimum_provider_requests: 2,
    provider_outcomes: ['completed'],
    success_terminal_counts: [1],
    minimum_callback_resumes: 1,
    gateway_executor_invocations: 0,
    network_observer_outbound: 0,
  };
  const evidence = evaluateMockAttempt(before, { entries, counters: before.counters }, expectation);
  assert.equal(evidence.arrivals, 2);
  assert.equal(evidence.callback_resume.observed_resumes, 1);

  const retryEntries = [
    ...entries.slice(0, -1),
    { sequence: 28, event: 'settled', nonce: 'callback', outcome: 'completed', successTerminalCount: 1 },
    { sequence: 29, event: 'arrival', nonce: 'retry' },
    { sequence: 30, event: 'settled', nonce: 'retry', outcome: 'completed', successTerminalCount: 1 },
  ];
  assert.equal(evaluateMockAttempt(before, {
    entries: retryEntries, counters: before.counters,
  }, expectation).arrivals, 3);
  assert.throws(() => evaluateMockAttempt(before, {
    entries: entries.filter((event) => event.event !== 'barrier_released'),
    counters: before.counters,
  }, expectation), /callback resume chronology was incomplete/u);
  assert.throws(() => evaluateMockAttempt(before, {
    entries: entries.filter((event) => !(event.event === 'settled' && event.nonce === 'callback')),
    counters: before.counters,
  }, expectation), /expected 2 settled mock request, observed 1/u);

  const duplicateRound = [
    ...entries,
    { sequence: 29, event: 'arrival', nonce: 'callback-two' },
    { sequence: 30, event: 'second_upstream_request', nonce: 'callback-two' },
    { sequence: 31, event: 'barrier_waiting', nonce: 'callback-two' },
    { sequence: 32, event: 'barrier_released', nonce: 'callback-two' },
    {
      sequence: 33, event: 'settled', nonce: 'callback-two',
      outcome: 'completed', successTerminalCount: 1,
    },
  ];
  assert.throws(() => evaluateMockAttempt(before, {
    entries: duplicateRound, counters: before.counters,
  }, { ...expectation, minimum_provider_requests: 3, minimum_callback_resumes: 2 }),
  /distinct Provider tool-call rounds/u);
});

test('BLO-07 proves two settled sequential callbacks while only the final text callback uses a barrier', () => {
  const before = {
    entries: [{ sequence: 40 }],
    counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
  };
  const entries = [
    ...before.entries,
    { sequence: 41, event: 'arrival', nonce: 'initial' },
    { sequence: 42, event: 'tool_call', nonce: 'initial' },
    { sequence: 43, event: 'settled', nonce: 'initial', outcome: 'completed', successTerminalCount: 1 },
    { sequence: 44, event: 'arrival', nonce: 'callback-a' },
    { sequence: 45, event: 'second_upstream_request', nonce: 'callback-a' },
    { sequence: 46, event: 'tool_call', nonce: 'callback-a' },
    { sequence: 47, event: 'settled', nonce: 'callback-a', outcome: 'completed', successTerminalCount: 1 },
    { sequence: 48, event: 'arrival', nonce: 'callback-b' },
    { sequence: 49, event: 'second_upstream_request', nonce: 'callback-b' },
    { sequence: 50, event: 'barrier_waiting', nonce: 'callback-b' },
    { sequence: 51, event: 'barrier_released', nonce: 'callback-b' },
    { sequence: 52, event: 'settled', nonce: 'callback-b', outcome: 'completed', successTerminalCount: 1 },
  ];
  const expectation = {
    minimum_provider_requests: 3,
    provider_outcomes: ['completed'],
    success_terminal_counts: [1],
    minimum_callback_resumes: 2,
    tool_mode: 'sequential_callback_tasks_one_turn',
    gateway_executor_invocations: 0,
    network_observer_outbound: 0,
  };
  const evidence = evaluateMockAttempt(before, { entries, counters: before.counters }, expectation);
  assert.deepEqual(evidence.callback_resume.rounds.map((round) => round.barrier_waiting_sequence), [null, 50]);

  const fabricated = entries.toSpliced(7, 0,
    { sequence: 47.1, event: 'barrier_waiting', nonce: 'callback-a' },
    { sequence: 47.2, event: 'barrier_released', nonce: 'callback-a' },
  );
  assert.throws(
    () => evaluateMockAttempt(before, { entries: fabricated, counters: before.counters }, expectation),
    /fabricated a text barrier/u,
  );
});

test('F4-CLIENT-GATE retries remain strict and every durable error body is byte exact', async () => {
  const mockBefore = {
    entries: [{ sequence: 30 }],
    counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
  };
  const retryEntries = [mockBefore.entries[0]];
  for (let index = 0; index < 3; index += 1) {
    const sequence = 31 + (index * 2);
    const nonce = `error-${index}`;
    retryEntries.push(
      { sequence, event: 'arrival', nonce },
      {
        sequence: sequence + 1,
        event: 'settled',
        nonce,
        outcome: 'http-500',
        successTerminalCount: 0,
      },
    );
  }
  const mockExpectation = {
    minimum_provider_requests: 1,
    provider_outcomes: ['http-500'],
    success_terminal_counts: [0],
    gateway_executor_invocations: 0,
    network_observer_outbound: 0,
  };
  assert.equal(evaluateMockAttempt(mockBefore, {
    entries: retryEntries, counters: mockBefore.counters,
  }, mockExpectation).arrivals, 3);
  retryEntries.at(-1).outcome = 'completed';
  assert.throws(() => evaluateMockAttempt(mockBefore, {
    entries: retryEntries, counters: mockBefore.counters,
  }, mockExpectation), /outcome completed did not match http-500/u);

  const before = { ids: ['old-run'], entries: [{ sequence: 30 }] };
  const payloads = {
    'http://fixture/runs': { items: [
      { id: 'old-run', status: 'succeeded' },
      { id: 'failed-1', status: 'failed' },
      { id: 'failed-2', status: 'failed' },
    ] },
    'http://fixture/runs/failed-1': {
      id: 'failed-1', status: 'failed', error: { message: PROVIDER_ERROR_BODY },
    },
    'http://fixture/runs/failed-2': {
      id: 'failed-2', status: 'failed', error: { message: PROVIDER_ERROR_BODY },
    },
  };
  const fetchImpl = async (url) => response(payloads[url]);
  const evidence = await reconcileAttempt({
    target,
    before,
    expectedRuns: 2,
    expectedStatuses: ['failed'],
    expectedErrorBody: PROVIDER_ERROR_BODY,
    fetchImpl,
    graceMs: 0,
  });
  assert.deepEqual(evidence.runs.map((run) => run.error_message), [
    PROVIDER_ERROR_BODY, PROVIDER_ERROR_BODY,
  ]);
  payloads['http://fixture/runs/failed-2'].error.message = `client wrapper ${PROVIDER_ERROR_BODY}`;
  await assert.rejects(reconcileAttempt({
    target,
    before,
    expectedRuns: 2,
    expectedStatuses: ['failed'],
    expectedErrorBody: PROVIDER_ERROR_BODY,
    fetchImpl,
    graceMs: 0,
  }), /did not preserve the exact upstream body/u);
});

test('F1 releases tool barrier only for barrier_waiting after the attempt snapshot cursor', async () => {
  const before = { entries: [
    { sequence: 7, event: 'arrival', nonce: 'old' },
    { sequence: 8, event: 'barrier_waiting', nonce: 'old' },
  ] };
  const snapshots = [
    before,
    { entries: [
      ...before.entries,
      { sequence: 9, event: 'arrival', nonce: 'current' },
      { sequence: 10, event: 'barrier_waiting', nonce: 'current' },
    ] },
  ];
  let reads = 0;
  const waiting = await waitForBarrierWaiting({
    before,
    mockSnapshot: async () => snapshots[Math.min(reads++, snapshots.length - 1)],
    graceMs: 100,
    pollIntervalMs: 0,
  });
  assert.equal(waiting.sequence, 10);
  assert.equal(waiting.nonce, 'current');
  assert.equal(reads, 2);
});

test('WP-14A final reconciliation requires zero runtime activity and active streams', async () => {
  const fetchImpl = async (url) => response(url.endsWith('/activity')
    ? { active: { total: 0 } }
    : { streams: [] });
  const result = await verifyIdle([target, target], fetchImpl);
  assert.equal(result.runtime_targets, 1);
  assert.equal(result.stream_targets, 1);
  await assert.rejects(verifyIdle([target], async (url) => response(url.endsWith('/activity')
    ? { active: { total: 1 } }
    : { streams: [] }), { graceMs: 0 }), /runtime activity remained 1/u);
});
