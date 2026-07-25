'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { TRANSPORT } = require('../../contracts');
const {
  collectDurableConvergence,
  evaluateListObservations,
  evaluateSnapshot,
  runUuidFromProtocolId,
} = require('../durable-evidence');

const RUN_ID = '018f7af7-3694-7ba0-90bf-83b5ec689705';

function request(overrides = {}) {
  return {
    clientNonce: 'load-000001',
    transport: TRANSPORT.RESPONSES_SSE,
    protocolId: `resp_${RUN_ID}`,
    protocolIdCount: 1,
    runId: RUN_ID,
    ...overrides,
  };
}

function snapshot(overrides = {}) {
  return {
    lists: {
      [TRANSPORT.RESPONSES_SSE]: [{ id: RUN_ID, status: 'succeeded', externalTraceId: 'load-000001' }],
    },
    queries: { 'load-000001': { id: RUN_ID, status: 'succeeded' } },
    runtimeActiveTotals: { [TRANSPORT.RESPONSES_SSE]: 0 },
    pluginStreams: { 'http://127.0.0.1:4200/providers/active-streams': [] },
    ...overrides,
  };
}

test('AC durable controlled negatives fail direct query, duplicate UUID, running, activity, and streams', () => {
  assert.match(evaluateSnapshot([request(), request({ clientNonce: 'load-000002' })], snapshot()).join('\n'), /duplicate run UUID/u);
  assert.match(evaluateSnapshot([request()], snapshot({ queries: {} })).join('\n'), /query_run result missing/u);
  assert.match(evaluateSnapshot([request()], snapshot({ queries: { 'load-000001': { id: '118f7af7-3694-7ba0-90bf-83b5ec689705', status: 'succeeded' } } })).join('\n'), /protocol\/query run id mismatch/u);
  assert.match(evaluateSnapshot([request()], snapshot({ queries: { 'load-000001': { id: RUN_ID, status: 'running' } } })).join('\n'), /remained running/u);
  assert.match(evaluateSnapshot([request()], snapshot({ runtimeActiveTotals: { [TRANSPORT.RESPONSES_SSE]: 1 } })).join('\n'), /active\.total was 1/u);
  assert.match(evaluateSnapshot([request()], snapshot({ pluginStreams: { streams: [{ invocation_id: 'secret-free-id' }] } })).join('\n'), /contained 1 active stream/u);
  assert.equal(runUuidFromProtocolId(TRANSPORT.ANTHROPIC_SSE, `msg_${RUN_ID}`), RUN_ID);
  assert.equal(runUuidFromProtocolId(TRANSPORT.RESPONSES_SSE, `msg_${RUN_ID}`), null);
});

test('AC durable list visibility is advisory while direct query remains authoritative', () => {
  const missing = snapshot({ lists: { [TRANSPORT.RESPONSES_SSE]: [] } });
  assert.deepEqual(evaluateSnapshot([request()], missing), []);
  assert.match(evaluateListObservations([request()], missing).join('\n'), /one list correlation/u);

  const stale = snapshot({ lists: { [TRANSPORT.RESPONSES_SSE]: [
    { id: RUN_ID, status: 'running', externalTraceId: 'load-000001' },
  ] } });
  assert.deepEqual(evaluateSnapshot([request()], stale), []);
  assert.match(evaluateListObservations([request()], stale).join('\n'), /list status remained running/u);
  assert.match(evaluateListObservations([request()], stale).join('\n'), /list\/query status mismatch/u);
});

test('AC durable list endpoint failure remains advisory when direct truth converges', async () => {
  const target = {
    durable: {
      list_runs: { url: 'http://fixture/list' },
      query_run: { url_template: 'http://fixture/query/{run_id}' },
    },
    runtime_activity: { url: 'http://fixture/activity' },
    plugin_runner_active_streams: { url: 'http://fixture/streams' },
  };
  const response = (data) => ({ ok: true, status: 200, async json() { return data; } });
  const ledger = await collectDurableConvergence({
    requestEvents: [request()],
    targetsByTransport: { [TRANSPORT.RESPONSES_SSE]: target },
    async fetchImpl(url) {
      if (url.endsWith('/list')) return { ok: false, status: 503 };
      if (url.includes('/query/')) return response({ data: { id: RUN_ID, status: 'succeeded' } });
      if (url.endsWith('/activity')) return response({ data: { active: { total: 0 } } });
      return response({ streams: [] });
    },
  });
  assert.equal(ledger.verdict, 'PASS');
  assert.match(ledger.advisories.join('\n'), /list_runs observation failed/u);
});

test('AC durable controlled negative: deadline preserves a still-running evidence ledger', async () => {
  const target = {
    durable: {
      list_runs: { url: 'http://fixture/list' },
      query_run: { url_template: 'http://fixture/query/{run_id}' },
    },
    runtime_activity: { url: 'http://fixture/activity' },
    plugin_runner_active_streams: { url: 'http://fixture/streams' },
  };
  const response = (data) => ({ ok: true, status: 200, async json() { return data; } });
  const ledger = await collectDurableConvergence({
    requestEvents: [request()],
    targetsByTransport: { [TRANSPORT.RESPONSES_SSE]: target },
    graceMs: 20,
    pollIntervalMs: 25,
    async fetchImpl(url) {
      if (url.endsWith('/list')) return response({ data: { items: [{ id: RUN_ID, status: 'running', correlation: { external_trace_id: 'load-000001' } }] } });
      if (url.includes('/query/')) return response({ data: { id: RUN_ID, status: 'running' } });
      if (url.endsWith('/activity')) return response({ data: { active: { total: 1 } } });
      return response({ streams: [{ invocation_id: 'active' }] });
    },
  });
  assert.equal(ledger.verdict, 'FAIL');
  assert.equal(ledger.polls.length, 1);
  assert.equal(ledger.failures.some((failure) => failure.includes('remained running')), true);
});

test('AC durable positive: fixed poll converges from running/active to one terminal and zero', async () => {
  let round = 0;
  const target = {
    durable: {
      list_runs: { url: 'http://fixture/list' },
      query_run: { url_template: 'http://fixture/query/{run_id}' },
    },
    runtime_activity: { url: 'http://fixture/activity' },
    plugin_runner_active_streams: { url: 'http://fixture/streams' },
  };
  const jsonResponse = (data) => ({ ok: true, status: 200, async json() { return data; } });
  const ledger = await collectDurableConvergence({
    requestEvents: [request()],
    targetsByTransport: { [TRANSPORT.RESPONSES_SSE]: target },
    pollIntervalMs: 1,
    graceMs: 100,
    async fetchImpl(url) {
      if (url.endsWith('/list')) round += 1;
      const terminal = round >= 2;
      if (url.endsWith('/list')) return jsonResponse({ data: { items: [{ id: RUN_ID, status: terminal ? 'succeeded' : 'running', correlation: { external_trace_id: 'load-000001' } }] } });
      if (url.includes('/query/')) return jsonResponse({ data: { id: RUN_ID, status: terminal ? 'succeeded' : 'running' } });
      if (url.endsWith('/activity')) return jsonResponse({ data: { active: { total: terminal ? 0 : 1 } } });
      return jsonResponse({ streams: terminal ? [] : [{ invocation_id: 'active' }] });
    },
  });
  assert.equal(ledger.verdict, 'PASS');
  assert.equal(ledger.polls.length, 2);
  assert.equal(ledger.polls[0].failures.some((failure) => failure.includes('running')), true);
  assert.deepEqual(ledger.polls[1].pluginStreams['http://fixture/streams'], []);
});
