'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { inspectClient, observeRun, runGatewayErrorMatrix } = require('../error-matrix');
const { UPSTREAM_ERROR_FIXTURES, ERROR_SURFACES } = require('../../protocol-oracle/error-fidelity');

function records(surface, message, success) {
  if (success) return surface === 'openai-chat-sse' ? [{ done: true }]
    : [{ data: { type: surface === 'anthropic-sse' ? 'message_stop' : 'response.completed' } }];
  return [{ data: surface.startsWith('responses-')
    ? { type: 'response.failed', response: { error: { message } } }
    : { type: 'error', error: { message } } }];
}

function fixtureDependencies(corrupt = false, websocketRecovery = false) {
  const entries = [];
  const runs = new Map();
  const keys = new Map();
  return {
    mockSnapshot: () => ({ entries: [...entries] }),
    dependencies: {
      async observeClient(surface, _target, fixture, traceId, retryKey) {
        const attempt = (keys.get(retryKey) ?? 0) + 1;
        keys.set(retryKey, attempt);
        const success = fixture.id === 'retry' && attempt === 2;
        const message = fixture.body || 'upstream returned HTTP 503';
        entries.push({ sequence: entries.length + 1, event: 'arrival', nonce: `mock-${String(entries.length + 1).padStart(6, '0')}` });
        if (websocketRecovery && surface === 'responses-websocket' && !success) {
          entries.push({ sequence: entries.length + 1, event: 'arrival', nonce: `mock-${String(entries.length + 1).padStart(6, '0')}` });
        }
        if (!success) entries.push({ sequence: entries.length + 1, event: 'settled', errorFixture: fixture.id, status: fixture.status });
        runs.set(traceId, { run_id: traceId, native: { status: success ? 'succeeded' : 'failed', error: success ? null : { message } }, durable: { error_payload: success ? null : { message: corrupt ? message.trim() : message, ...(websocketRecovery && surface === 'responses-websocket' ? { ai_native_recovery: { provider_attempts_consumed: 2 } } : {}) } } });
        return { http_status: 200, records: records(surface, message, success) };
      },
      async observeRun(_target, traceId) { return runs.get(traceId); },
    },
  };
}

test('Root #1998 P7: executes all 20 online rows and four separate failed/recovered retry pairs', async () => {
  const fixture = fixtureDependencies();
  const result = await runGatewayErrorMatrix({ ready: { targets: {} }, mockSnapshot: fixture.mockSnapshot }, fixture.dependencies);
  assert.equal(result.verdict, 'PASS');
  assert.deepEqual(result.rows.map((row) => row.id), UPSTREAM_ERROR_FIXTURES.flatMap((item) => ERROR_SURFACES.map((surface) => `${item.id}/${surface}`)));
  assert.equal(result.rows.reduce((sum, row) => sum + row.attempts.length, 0), 24);
  assert.equal(new Set(result.rows.flatMap((row) => row.attempts.map((attempt) => attempt.run_id))).size, 24);
  assert.equal(new Set(result.rows.flatMap((row) => row.attempts.map((attempt) => attempt.upstream_nonce))).size, 24);
});

test('WebSocket recovery counts controlled upstream attempts from the durable receipt', async () => {
  const fixture = fixtureDependencies(false, true);
  const result = await runGatewayErrorMatrix({ ready: { targets: {} }, mockSnapshot: fixture.mockSnapshot }, fixture.dependencies);
  assert.equal(result.verdict, 'PASS');
  assert.equal(result.rows.length, 20);
});

test('Root #1998 P7 authenticity: durable whitespace loss fails rows while remaining online matrix still executes', async () => {
  const fixture = fixtureDependencies(true);
  const result = await runGatewayErrorMatrix({ ready: { targets: {} }, mockSnapshot: fixture.mockSnapshot }, fixture.dependencies);
  assert.equal(result.verdict, 'FAIL');
  assert.equal(result.rows.length, 20);
  assert.ok(result.rows.filter((row) => row.fixture === 'json').every((row) => row.verdict === 'FAIL' && /exact upstream body/u.test(row.error)));
  assert.ok(result.rows.filter((row) => row.fixture === 'empty').every((row) => row.verdict === 'PASS'));
});

test('Root #1998 P7 authenticity: success following error, missing terminal and duplicate failures cannot pass', () => {
  for (const surface of ERROR_SURFACES) {
    assert.throws(() => inspectClient(surface, [], false), /cardinality/u);
    const failed = records(surface, 'body', false);
    assert.throws(() => inspectClient(surface, [...failed, ...failed], false), /cardinality/u);
    assert.throws(() => inspectClient(surface, [...failed, ...records(surface, null, true)], false), /cardinality/u);
  }
});


test('error matrix reads persisted error payload from trace export, not metadata-only overview', async () => {
  const traceId = 'trace-1';
  const runId = 'run-1';
  const message = ' exact upstream body\n';
  const calls = [];
  const target = {
    durable: {
      list_runs: {
        url: 'https://fixture/api/console/applications/app-1/logs/runs?page=1',
        headers: { cookie: 'owner-session' },
      },
      query_run: {
        url_template: 'https://fixture/api/agent/v1/runs/{run_id}',
        headers: { authorization: 'Bearer app-key' },
      },
    },
  };
  const fetchImpl = async (url, options) => {
    calls.push({ url, options });
    if (url === target.durable.list_runs.url) {
      return { ok: true, json: async () => ({ items: [{ id: runId, correlation: { external_trace_id: traceId } }] }) };
    }
    if (url === target.durable.query_run.url_template.replace('{run_id}', runId)) {
      return { ok: true, json: async () => ({ id: runId, status: 'failed', metadata: { external_trace_id: traceId }, error: { message } }) };
    }
    if (url === `https://fixture/api/console/applications/app-1/logs/runs/${runId}/export`) {
      return { ok: true, json: async () => ({ flow_run: { id: runId, status: 'failed', error_payload: { message } } }) };
    }
    if (url.endsWith('/overview')) {
      return { ok: true, json: async () => ({ flow_run: { id: runId, status: 'failed' } }) };
    }
    throw new Error(`unexpected evidence URL: ${url}`);
  };

  const originalFetch = globalThis.fetch;
  globalThis.fetch = fetchImpl;
  let observed;
  try {
    observed = await observeRun(target, traceId);
  } finally {
    globalThis.fetch = originalFetch;
  }

  assert.deepEqual(observed.durable, {
    id: runId, status: 'failed', error_payload: { message },
  });
  assert.ok(calls.some(({ url }) => url.endsWith(`/${runId}/export`)));
  assert.ok(calls.every(({ url }) => !url.endsWith('/overview')));
  assert.deepEqual(calls.find(({ url }) => url.endsWith(`/${runId}/export`)).options.headers, target.durable.list_runs.headers);
});
