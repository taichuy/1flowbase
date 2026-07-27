'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  evaluateMockAttempt, reconcileAttempt, snapshotRuns, verifyIdle, waitForBarrierWaiting,
} = require('../durable');

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
    counters: { gatewayExecutorInvocations: 0 },
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
    counters: { gatewayExecutorInvocations: 0 },
  };
  const evidence = evaluateMockAttempt(before, after, {
    provider_requests: 1,
    provider_outcomes: ['completed'],
    success_terminal_counts: [1],
    request_body_keys: ['context_management', 'output_config', 'thinking'],
    gateway_executor_invocations: 0,
  });
  assert.equal(evidence.gateway_executor_invocations, 0);
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
