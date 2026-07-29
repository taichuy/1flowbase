'use strict';

const { TRANSPORT } = require('../contracts');
const { publicTarget } = require('./target');

function counterDelta(before, after, name) {
  const start = Number(before?.counters?.[name] ?? 0);
  const end = Number(after?.counters?.[name] ?? 0);
  return end - start;
}

function assertNoSecrets(value, secrets) {
  const encoded = JSON.stringify(value);
  for (const secret of secrets) {
    if (typeof secret === 'string' && secret.length > 0 && encoded.includes(secret)) {
      throw new Error('Responses WebSocket WireAudit artifact contained a secret');
    }
  }
}

function createWireAudit({ target, trace, durable, upstreamBefore, upstreamAfter, secrets = [] }) {
  const gateway = publicTarget(target);
  const cursor = upstreamBefore?.entries?.at(-1)?.sequence ?? 0;
  const entries = (upstreamAfter?.entries ?? []).filter((entry) => entry.sequence > cursor);
  // The Gateway fixture configures the OpenAI provider as http_sse. A direct mock
  // WebSocket arrival is only an upstream probe and cannot satisfy this assertion.
  const gatewayArrivals = entries.filter((entry) =>
    entry.event === 'arrival'
    && entry.transport === TRANSPORT.RESPONSES_SSE
    && entry.request?.body?.model === target.upstream_model
  );
  if (gatewayArrivals.length !== 1) {
    throw new Error(`expected one Gateway-to-upstream Responses SSE arrival, received ${gatewayArrivals.length}`);
  }
  const arrival = gatewayArrivals[0];
  if (arrival.nonce !== trace?.upstream_nonce) throw new Error('Gateway trace/upstream nonce mismatch');
  if (durable?.run?.id !== trace?.run_id) throw new Error('Gateway trace/durable run id mismatch');

  const counters = {
    gateway_executor_invocations: counterDelta(upstreamBefore, upstreamAfter, 'gatewayExecutorInvocations'),
    tool_outbound: counterDelta(upstreamBefore, upstreamAfter, 'networkObserverOutbound'),
    provider_tool_executions: counterDelta(upstreamBefore, upstreamAfter, 'providerExecutions'),
  };
  if (counters.gateway_executor_invocations !== 0) throw new Error('Gateway executor observer recorded an invocation');
  if (counters.tool_outbound !== 0) throw new Error('Gateway emitted controlled tool outbound traffic');
  if (counters.provider_tool_executions !== 0) throw new Error('provider executed a tool during the WebSocket target');

  const audit = {
    schema_version: '1flowbase.responses-websocket-wire-audit/v1',
    verdict: 'PASS',
    gateway,
    run_trace: {
      client_trace_id: trace.client_trace_id,
      response_id: trace.response_id,
      run_id: trace.run_id,
      upstream_nonce: trace.upstream_nonce,
      upstream_arrival_sequence: arrival.sequence,
    },
    durable_digest_sha256: durable.digest_sha256,
    counters,
    direct_mock_websocket_support_evidence: false,
  };
  assertNoSecrets(audit, [target.api_key, target.connect_headers?.authorization, ...secrets]);
  return audit;
}

module.exports = { assertNoSecrets, counterDelta, createWireAudit };
