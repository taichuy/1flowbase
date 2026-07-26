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
    'mcp_approval_request', 'mcp_approval_response',
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
