'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { decodeGatewayFrames } = require('../decoder');
const { queryDurableRun } = require('../durable');
const { HTTP_500_ERROR_BODY } = require('../../mock-upstream');

const RUN_ID = '123e4567-e89b-42d3-a456-426614174000';

function frames(events) {
  return events.map((event) => [Buffer.from(JSON.stringify(event))]);
}

function target() {
  return {
    evidence_role: 'gateway-support-target',
    durable: {
      query_run: {
        method: 'GET',
        url_template: 'http://127.0.0.1:4100/api/agent/v1/runs/{run_id}',
        headers: { authorization: 'Bearer must-not-leak' },
      },
    },
  };
}

test('Root #1461 AC WebSocket decoder records one Gateway run trace', () => {
  const trace = decodeGatewayFrames(frames([
    { type: 'response.created', response: { id: `resp_${RUN_ID}` } },
    { type: 'response.output_text.delta', delta: 'mock-000041:chunk-1' },
    { type: 'response.output_text.delta', delta: 'mock-000041:chunk-2' },
    { type: 'response.completed', response: { id: `resp_${RUN_ID}` } },
  ]), { clientTraceId: 'ws-gateway-000001' });
  assert.equal(trace.response_id, `resp_${RUN_ID}`);
  assert.equal(trace.run_id, RUN_ID);
  assert.equal(trace.upstream_nonce, 'mock-000041');
  assert.equal(trace.terminal_count, 1);
});

test('Root #1461 AC durable target binds protocol run id and returns a secret-free digest', async () => {
  const calls = [];
  const evidence = await queryDurableRun(target(), {
    run_id: RUN_ID,
    client_trace_id: 'ws-gateway-000001',
  }, async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { data: {
          id: RUN_ID,
          status: 'succeeded',
          correlation: { external_trace_id: 'ws-gateway-000001' },
          application_id: 'application-1',
          provider_instance_id: 'provider-1',
          secret: 'must-not-leak',
        } };
      },
    };
  });
  assert.equal(calls[0].url.endsWith(RUN_ID), true);
  assert.equal(evidence.run.id, RUN_ID);
  assert.match(evidence.digest_sha256, /^[0-9a-f]{64}$/u);
  assert.equal(JSON.stringify(evidence).includes('must-not-leak'), false);
});

test('Root #1461 controlled negative: decoder rejects non-durable upstream mock response ids', () => {
  assert.throws(() => decodeGatewayFrames(frames([
    { type: 'response.created', response: { id: 'resp_mock-000001' } },
    { type: 'response.output_text.delta', delta: 'mock-000001:chunk-1' },
    { type: 'response.completed', response: { id: 'resp_mock-000001' } },
  ]), { clientTraceId: 'ws-gateway-000002' }), /durable run UUID/u);
});

test('Root #1461 Delivery #1474 decodes response.failed without inventing a success nonce', () => {
  const trace = decodeGatewayFrames(frames([
    { type: 'response.created', response: { id: `resp_${RUN_ID}` } },
    {
      type: 'response.failed',
      response: {
        id: `resp_${RUN_ID}`,
        error: { message: HTTP_500_ERROR_BODY, type: 'provider_error', code: 'provider_upstream_error' },
      },
    },
  ]), { clientTraceId: 'ws-gateway-error-000001' });
  assert.equal(trace.run_id, RUN_ID);
  assert.equal(trace.terminal_type, 'response.failed');
  assert.equal(trace.error_message, HTTP_500_ERROR_BODY);
  assert.equal(trace.upstream_nonce, null);
});

test('Root #1461 Delivery #1474 durable evidence preserves failed run error.message', async () => {
  const evidence = await queryDurableRun(target(), {
    run_id: RUN_ID,
    client_trace_id: 'ws-gateway-error-000001',
  }, async () => ({
    ok: true,
    async json() {
      return { data: {
        id: RUN_ID,
        status: 'failed',
        correlation: { external_trace_id: 'ws-gateway-error-000001' },
        error: { message: HTTP_500_ERROR_BODY },
      } };
    },
  }));
  assert.equal(evidence.run.status, 'failed');
  assert.equal(evidence.run.error_message, HTTP_500_ERROR_BODY);
});
