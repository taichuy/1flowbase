'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { createWireAudit } = require('../wire-audit');

const RUN_ID = '123e4567-e89b-42d3-a456-426614174000';

function target() {
  return {
    evidence_role: 'gateway-support-target',
    transport: 'responses-websocket',
    expected_upstream_transport: 'responses-websocket',
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

function evidence(transport = 'responses-websocket') {
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

test('declared upstream transport is exact for native WebSocket and explicit SSE targets', () => {
  for (const transport of ['responses-websocket', 'responses-sse']) {
    const input = auditInput(transport);
    input.target.expected_upstream_transport = transport;
    assert.equal(createWireAudit(input).gateway.expected_upstream_transport, transport);
    input.target.expected_upstream_transport = transport === 'responses-sse' ? 'responses-websocket' : 'responses-sse';
    assert.throws(() => createWireAudit(input), /expected one Gateway-to-upstream/u);
  }
  const missing = auditInput();
  delete missing.target.expected_upstream_transport;
  assert.throws(() => createWireAudit(missing), /expected upstream transport/u);
});

test('authenticity negatives reject probe role, model, nonce, run and duplicate arrivals', () => {
  const probe = auditInput();
  probe.target.evidence_role = 'upstream-probe-only';
  assert.throws(() => createWireAudit(probe), /requires a Gateway target/u);
  const wrongModel = auditInput();
  wrongModel.upstreamAfter.entries[1].request.body.model = 'direct-probe-model';
  assert.throws(() => createWireAudit(wrongModel), /expected one Gateway-to-upstream/u);
  const wrongNonce = auditInput();
  wrongNonce.trace.upstream_nonce = 'direct-probe-nonce';
  assert.throws(() => createWireAudit(wrongNonce), /nonce mismatch/u);
  const wrongRun = auditInput();
  wrongRun.durable.run.id = 'different-run';
  assert.throws(() => createWireAudit(wrongRun), /run id mismatch/u);
  const duplicate = auditInput();
  duplicate.upstreamAfter.entries.push({ ...duplicate.upstreamAfter.entries[1], sequence: 12 });
  assert.throws(() => createWireAudit(duplicate), /received 2/u);
});

test('Root #1461 authenticity negative: executor or tool outbound evidence fails closed', () => {
  const executor = auditInput();
  executor.upstreamAfter.counters.gatewayExecutorInvocations += 1;
  assert.throws(() => createWireAudit(executor), /executor observer/u);

  const outbound = auditInput();
  outbound.upstreamAfter.counters.networkObserverOutbound += 1;
  assert.throws(() => createWireAudit(outbound), /tool outbound/u);
});
