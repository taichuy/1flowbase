'use strict';

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
        input: [
          { type: 'mcp_list_tools', id: 'mcp_list_1', server_label: 'fixture_mcp', tools: [] },
          { type: 'mcp_call', id: 'mcp_call_1', server_label: 'fixture_mcp', name: 'lookup', arguments: '{}' },
          { type: 'mcp_approval_request', id: 'mcp_approval_1', server_label: 'fixture_mcp', name: 'lookup', arguments: '{}' },
          { type: 'mcp_approval_response', approval_request_id: 'mcp_approval_1', approve: true },
        ],
      },
    },
  ];
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
        metadata: { wire_audit_vector: vector.name },
      }),
    });
    if (!response.ok) throw new Error(`WireAudit ${vector.name} returned HTTP ${response.status}`);
    vector.capture = await response.text();
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
    'mcp_approval_request', 'mcp_approval_response', 'response.future_gateway_drift',
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

module.exports = { runWireAudit, vectorBodies };
