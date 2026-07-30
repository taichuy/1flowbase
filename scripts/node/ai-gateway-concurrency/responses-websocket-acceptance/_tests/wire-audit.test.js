'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { createWireAudit } = require('../wire-audit');

const RUN_ID = '123e4567-e89b-42d3-a456-426614174000';

function target() {
  return {
    evidence_role: 'gateway-support-target',
    transport: 'responses-websocket',
    url: 'ws://127.0.0.1:4100/v1/responses',
    application_id: 'application-1',
    provider_instance_id: 'provider-1',
    model: 'published-model',
    upstream_model: 'upstream-model',
    api_key: 'application-secret',
    connect_headers: {
      authorization: 'Bearer application-secret',
      'openai-beta': 'responses_websockets=2026-02-06',
    },
  };
}

function evidence(transport = 'responses-sse') {
  return {
    upstreamBefore: {
      counters: { gatewayExecutorInvocations: 7, networkObserverOutbound: 3, providerExecutions: 2 },
      entries: [{ sequence: 10, event: 'settled' }],
    },
    upstreamAfter: {
      counters: { gatewayExecutorInvocations: 7, networkObserverOutbound: 3, providerExecutions: 2 },
      entries: [
        { sequence: 10, event: 'settled' },
        {
          sequence: 11,
          event: 'arrival',
          nonce: 'mock-000041',
          transport,
          request: { body: { model: 'upstream-model' } },
        },
      ],
    },
  };
}

function auditInput(transport) {
  return {
    target: target(),
    trace: {
      client_trace_id: 'ws-gateway-000001',
      response_id: `resp_${RUN_ID}`,
      run_id: RUN_ID,
      upstream_nonce: 'mock-000041',
    },
    durable: { run: { id: RUN_ID, status: 'succeeded' }, digest_sha256: 'a'.repeat(64) },
    ...evidence(transport),
  };
}

test('Root #1461 AC WireAudit proves Gateway traversal, durable trace, redaction, and zero execution', () => {
  const audit = createWireAudit(auditInput());
  assert.equal(audit.verdict, 'PASS');
  assert.equal(audit.run_trace.upstream_arrival_sequence, 11);
  assert.equal(audit.durable_digest_sha256, 'a'.repeat(64));
  assert.deepEqual(audit.counters, {
    gateway_executor_invocations: 0,
    tool_outbound: 0,
    provider_tool_executions: 0,
  });
  assert.equal(audit.direct_mock_websocket_support_evidence, false);
  assert.equal(JSON.stringify(audit).includes('application-secret'), false);
});

test('Root #1461 authenticity negative: direct mock WebSocket arrival is not Gateway support evidence', () => {
  assert.throws(
    () => createWireAudit(auditInput('responses-websocket')),
    /Gateway-to-upstream Responses SSE arrival/u,
  );
});

test('Root #1461 authenticity negative: executor or tool outbound evidence fails closed', () => {
  const executor = auditInput();
  executor.upstreamAfter.counters.gatewayExecutorInvocations += 1;
  assert.throws(() => createWireAudit(executor), /executor observer/u);

  const outbound = auditInput();
  outbound.upstreamAfter.counters.networkObserverOutbound += 1;
  assert.throws(() => createWireAudit(outbound), /tool outbound/u);
});
