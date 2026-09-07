'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const { inspectClient, runGatewayErrorMatrix } = require('../error-matrix');
const { UPSTREAM_ERROR_FIXTURES, ERROR_SURFACES } = require('../../protocol-oracle/error-fidelity');

function records(surface, message, success) {
  if (success) return surface === 'openai-chat-sse' ? [{ done: true }]
    : [{ data: { type: surface === 'anthropic-sse' ? 'message_stop' : 'response.completed' } }];
  return [{ data: surface.startsWith('responses-')
    ? { type: 'response.failed', response: { error: { message } } }
    : { type: 'error', error: { message } } }];
}

function fixtureDependencies(corrupt = false) {
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
        if (!success) entries.push({ sequence: entries.length + 1, event: 'settled', errorFixture: fixture.id, status: fixture.status });
        runs.set(traceId, { run_id: traceId, native: { status: success ? 'succeeded' : 'failed', error: success ? null : { message } }, durable: { error_payload: success ? null : { message: corrupt ? message.trim() : message } } });
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
