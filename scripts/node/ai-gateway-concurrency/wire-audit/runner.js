'use strict';

const {
  EPHEMERAL_NO_LEAK_ORACLE,
  REQUEST_FIDELITY_VECTORS,
  REQUEST_NEGATIVE_VECTORS,
  REQUEST_TRANSLATION_VECTORS,
  assertNoEphemeralRawLeak,
} = require('../protocol-oracle/request-fidelity');
const {
  ERROR_SURFACES,
  UPSTREAM_ERROR_FIXTURES,
  assertUpstreamErrorFidelity,
} = require('../protocol-oracle/error-fidelity');

function requestFidelityInventory() {
  const ingresses = REQUEST_FIDELITY_VECTORS.map((row) => row.ingress);
  if (new Set(ingresses).size !== 3) throw new Error('request fidelity oracle must cover three distinct ingresses');
  const negativeKinds = REQUEST_NEGATIVE_VECTORS.map((row) => row.kind);
  for (const expected of [
    'typed-opaque-conflict', 'reserved-field', 'unconsumed-residual',
  ]) {
    if (!negativeKinds.includes(expected)) throw new Error(`request negative oracle omitted ${expected}`);
  }
  if (!REQUEST_TRANSLATION_VECTORS.some((row) => row.kind === 'foreign-protocol')) {
    throw new Error('request translation oracle omitted foreign-protocol');
  }
  return {
    schema_version: '1flowbase.ai-gateway-request-fidelity/v1',
    positive_rows: REQUEST_FIDELITY_VECTORS.map((row) => row.id),
    negative_rows: REQUEST_NEGATIVE_VECTORS.map((row) => row.id),
    translation_rows: REQUEST_TRANSLATION_VECTORS.map((row) => row.id),
    ephemeral_lifetime: EPHEMERAL_NO_LEAK_ORACLE.raw_lifetime,
    raw_sinks_forbidden: EPHEMERAL_NO_LEAK_ORACLE.forbidden_sinks,
  };
}

function errorFidelityInventory() {
  if (UPSTREAM_ERROR_FIXTURES.length !== 5) throw new Error('upstream error oracle must contain five fixtures');
  if (ERROR_SURFACES.length !== 4) throw new Error('upstream error oracle must cover four public surfaces');
  return {
    schema_version: '1flowbase.ai-gateway-error-fidelity/v1',
    fixtures: UPSTREAM_ERROR_FIXTURES.map((row) => row.id),
    surfaces: ERROR_SURFACES,
    rows: UPSTREAM_ERROR_FIXTURES.length * ERROR_SURFACES.length,
  };
}

function assertRequestFidelityAudit(evidence, { rawCanaries = [] } = {}) {
  const expectedPositive = requestFidelityInventory().positive_rows;
  const positive = new Map((evidence.positive ?? []).map((row) => [row.id, row]));
  for (const id of expectedPositive) {
    const row = positive.get(id);
    if (!row) throw new Error(`request fidelity evidence omitted ${id}`);
    if (!/^[a-f0-9]{64}$/u.test(row.direct_sha256 ?? '')) {
      throw new Error(`request fidelity direct digest is invalid for ${id}`);
    }
    if (row.direct_sha256 !== row.gateway_sha256) {
      throw new Error(`request fidelity mismatch for ${id}`);
    }
  }
  const expectedNegative = requestFidelityInventory().negative_rows;
  const negative = new Map((evidence.negative ?? []).map((row) => [row.id, row]));
  for (const id of expectedNegative) {
    const row = negative.get(id);
    if (!row?.failed || row.upstream_arrivals !== 0) {
      throw new Error(`request negative did not fail before upstream for ${id}`);
    }
  }
  const expectedTranslation = requestFidelityInventory().translation_rows;
  const translation = new Map((evidence.translation ?? []).map((row) => [row.id, row]));
  for (const id of expectedTranslation) {
    const row = translation.get(id);
    if (!row?.succeeded || row.upstream_arrivals !== 1 || row.foreign_raw_in_upstream !== false) {
      throw new Error(`request translation did not omit foreign wire context for ${id}`);
    }
    if (!row.decisions?.includes('omitted_foreign_protocol_envelope')) {
      throw new Error(`request translation receipt omitted foreign-context decision for ${id}`);
    }
  }
  for (const phase of EPHEMERAL_NO_LEAK_ORACLE.raw_lifetime) {
    if (!evidence.ephemeral?.preserved_phases?.includes(phase)) {
      throw new Error(`ephemeral raw context was not preserved through ${phase}`);
    }
  }
  for (const phase of EPHEMERAL_NO_LEAK_ORACLE.cleanup) {
    if (!evidence.ephemeral?.cleanup_phases?.includes(phase)) {
      throw new Error(`ephemeral raw context was not cleaned after ${phase}`);
    }
  }
  if (evidence.ephemeral?.missing_before_terminal_failed !== true) {
    throw new Error('missing ephemeral raw context did not fail before terminal');
  }
  assertNoEphemeralRawLeak(evidence, rawCanaries);
  return {
    schema_version: '1flowbase.ai-gateway-request-fidelity-result/v1',
    verdict: 'PASS',
    positive_rows: expectedPositive.length,
    negative_rows: expectedNegative.length,
    translation_rows: expectedTranslation.length,
  };
}

function assertErrorFidelityAudit(evidence) {
  const rows = new Map((evidence.rows ?? []).map((row) => [`${row.fixture}:${row.surface}`, row]));
  for (const fixture of UPSTREAM_ERROR_FIXTURES) {
    for (const surface of ERROR_SURFACES) {
      const row = rows.get(`${fixture.id}:${surface}`);
      if (!row) throw new Error(`error fidelity evidence omitted ${fixture.id}:${surface}`);
      assertUpstreamErrorFidelity(fixture, {
        nativeMessage: row.native_message,
        durableMessage: row.durable_message,
        clientMessages: [row.client_message],
      });
      if (fixture.id === 'retry' && row.attempts !== fixture.attempts) {
        throw new Error(`error retry evidence used ${row.attempts} attempts`);
      }
    }
  }
  return {
    schema_version: '1flowbase.ai-gateway-error-fidelity-result/v1',
    verdict: 'PASS', rows: rows.size,
  };
}

function vectorBodies(observers, secretCanary) {
  return [
    {
      name: 'tool-search-additional-tools',
      body: {
        model: 'fixture-model', stream: true,
        tools: [{ type: 'tool_search', execution: 'client', x_gateway_executor_observer: observers.gatewayExecutorObserverUrl }],
        input: [
          { type: 'tool_search_call', id: 'ts_1', arguments: '{}' }
        ],
      },
    },
    {
      name: 'gateway-executor-probe',
      body: {
        model: 'fixture-model', stream: true,
        input: [{
          type: 'message', role: 'user', content: [{ type: 'input_text', text: [
            '1flowbase-client-tool-vector',
            `GATEWAY_EXECUTOR_PROBE_URL=${observers.gatewayExecutorObserverUrl}`,
          ].join(' ') }],
        }],
      },
    },
    {
      name: 'tool-search-output-additional-tools',
      body: {
        model: 'fixture-model', stream: true,
        input: [
          { type: 'tool_search_output', id: 'ts_1', tools: [{ type: 'function', name: 'fixture_read' }] },
          { type: 'additional_tools', id: 'at_1', tools: [{ type: 'function', name: 'fixture_read' }] },
        ],
      },
    },
    {
      name: 'hosted-tools',
      body: {
        model: 'fixture-model', stream: true,
        tools: [
          { type: 'file_search', vector_store_ids: ['vs_fixture'] },
          { type: 'programmatic_tool_calling', name: 'fixture_program' },
          { type: 'shell', environment: { type: 'container_auto' } },
        ],
        input: 'controlled hosted tool transport',
      },
    },
    {
      name: 'mcp-list-call-approval',
      body: {
        model: 'fixture-model', stream: true,
        tools: [{
          type: 'mcp', server_label: 'fixture_mcp', server_url: observers.networkObserverUrl,
          authorization: secretCanary, headers: { 'x-mcp-canary': secretCanary },
        }],
        input: 'ordinary user request for an MCP lookup',
      },
    },
  ];
}

function mcpApprovalContinuation(capture) {
  const payloads = capture.split(/\r?\n/u)
    .filter((line) => line.startsWith('data: ') && line !== 'data: [DONE]')
    .map((line) => JSON.parse(line.slice(6)));
  const responseId = payloads
    .filter((payload) => payload.type === 'response.created')
    .map((payload) => payload.response?.id)
    .find((id) => typeof id === 'string' && id.trim().length > 0);
  const approvalRequestId = payloads.flatMap((payload) => [
    payload.item,
    ...(Array.isArray(payload.response?.output) ? payload.response.output : []),
  ]).find((item) => item?.type === 'mcp_approval_request')?.id;
  if (!responseId) throw new Error('WireAudit MCP start omitted provider response id');
  if (!approvalRequestId) throw new Error('WireAudit MCP start omitted approval request id');
  return {
    previous_response_id: responseId,
    input: [{
      type: 'mcp_approval_response', approval_request_id: approvalRequestId, approve: true,
    }],
  };
}

async function snapshot(url, fetchImpl) {
  const response = await fetchImpl(url);
  if (!response.ok) throw new Error(`controlled upstream snapshot returned HTTP ${response.status}`);
  return response.json();
}

async function runWireAudit(inputs, { fetchImpl = globalThis.fetch, secretCanary }) {
  const controlled = inputs.manifest.controlledUpstream;
  if (!controlled) throw new Error('WireAudit requires controlled upstream observers');
  const before = await snapshot(controlled.snapshotUrl, fetchImpl);
  const vectors = vectorBodies(controlled, secretCanary);
  for (const vector of vectors) {
    const response = await fetchImpl(`${inputs.manifest.gatewayBaseUrl}/v1/responses`, {
      method: 'POST',
      headers: {
        accept: 'text/event-stream',
        authorization: `Bearer ${inputs.manifest.openai.api_key}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        ...vector.body,
        model: inputs.manifest.openai.model,
      }),
    });
    if (!response.ok) throw new Error(`WireAudit ${vector.name} returned HTTP ${response.status}`);
    vector.capture = await response.text();
    if (vector.name === 'mcp-list-call-approval') {
      const continuation = mcpApprovalContinuation(vector.capture);
      const approvalResponse = await fetchImpl(`${inputs.manifest.gatewayBaseUrl}/v1/responses`, {
        method: 'POST',
        headers: {
          accept: 'text/event-stream',
          authorization: `Bearer ${inputs.manifest.openai.api_key}`,
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          model: inputs.manifest.openai.model,
          stream: true,
          ...continuation,
        }),
      });
      if (!approvalResponse.ok) {
        throw new Error(
          `WireAudit mcp-approval-continuation ${continuation.previous_response_id} returned HTTP ${approvalResponse.status}: ${await approvalResponse.text()}`,
        );
      }
      vector.capture += `\n${await approvalResponse.text()}`;
    }
  }
  const after = await snapshot(controlled.snapshotUrl, fetchImpl);
  const cursor = before.entries.at(-1)?.sequence ?? 0;
  const events = after.entries.filter((event) => event.sequence > cursor).map((event) => event.event);
  for (const expected of [
    'client_tool_search', 'server_tool_search', 'additional_tools', 'provider_tool_execution',
    'mcp_server_definition', 'mcp_list', 'mcp_call', 'mcp_approval',
  ]) {
    if (!events.includes(expected)) throw new Error(`WireAudit did not observe ${expected}`);
  }
  const capturedWire = vectors.map((vector) => vector.capture).join('\n');
  for (const expected of [
    'tool_search_call', 'tool_search_output', 'additional_tools', 'file_search_call',
    'program', 'shell_call', 'mcp_list_tools', 'mcp_call',
    'mcp_approval_request',
  ]) {
    if (!capturedWire.includes(expected)) throw new Error(`WireAudit output omitted ${expected}`);
  }
  const gatewayExecutorInvocations = after.counters.gatewayExecutorInvocations
    - before.counters.gatewayExecutorInvocations;
  const networkObserverOutbound = after.counters.networkObserverOutbound
    - before.counters.networkObserverOutbound;
  if (gatewayExecutorInvocations !== 0) {
    throw new Error('gateway executor observer recorded an invocation');
  }
  if (networkObserverOutbound !== 0) {
    throw new Error('gateway connected to controlled MCP server_url');
  }
  return {
    schema_version: '1flowbase.ai-gateway-wire-audit/v1',
    vectors: vectors.map((vector) => vector.name),
    observed_events: [...new Set(events)],
    counters: {
      gateway_executor_invocations: gatewayExecutorInvocations,
      network_observer_outbound: networkObserverOutbound,
      provider_executions: after.counters.providerExecutions - before.counters.providerExecutions,
    },
  };
}

module.exports = {
  assertErrorFidelityAudit,
  assertRequestFidelityAudit,
  errorFidelityInventory,
  requestFidelityInventory,
  runWireAudit,
  vectorBodies,
};
