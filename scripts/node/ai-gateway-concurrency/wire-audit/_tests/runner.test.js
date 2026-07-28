'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  assertErrorFidelityAudit,
  assertRequestFidelityAudit,
  errorFidelityInventory,
  requestFidelityInventory,
  runWireAudit,
  vectorBodies,
} = require('../runner');
const { ERROR_SURFACES, UPSTREAM_ERROR_FIXTURES } = require('../../protocol-oracle/error-fidelity');

// Root AC-019/020/023/024/027: finite vectors preserve owner and observer boundaries.
test('controlled WireAudit vectors cover tool search, hosted tools, MCP, approval, and canary paths', () => {
  const canary = 'wire-secret-canary';
  const vectors = vectorBodies({
    networkObserverUrl: 'http://127.0.0.1:41009/__observer/mcp-network',
    gatewayExecutorObserverUrl: 'http://127.0.0.1:41009/__observer/gateway-executor',
  }, canary);
  assert.deepEqual(vectors.map((vector) => vector.name), [
    'tool-search-additional-tools', 'gateway-executor-probe',
    'tool-search-output-additional-tools', 'hosted-tools', 'mcp-list-call-approval',
  ]);
  const wire = JSON.stringify(vectors);
  for (const value of [
    'tool_search_call', 'tool_search_output', 'additional_tools', 'file_search',
    'programmatic_tool_calling', 'shell', 'ordinary user request for an MCP lookup', canary,
  ]) assert.match(wire, new RegExp(value, 'u'));
  assert.doesNotMatch(wire, /mcp_list_tools|mcp_call|mcp_approval_request/u);
  assert.doesNotMatch(wire, /mcp_approval_response/u);
  assert.doesNotMatch(wire, /wire_audit_vector/u);
});

// Root AC-021/024: provider approval is a second request correlated to the first response.
test('controlled WireAudit submits MCP approval as a provider continuation', async () => {
  const providerResponseId = 'resp_provider_owned';
  const approvalRequestId = 'approval_provider_owned';
  const requests = [];
  let snapshotCount = 0;
  const observed = [
    'client_tool_search', 'server_tool_search', 'additional_tools', 'provider_tool_execution',
    'mcp_server_definition', 'mcp_list', 'mcp_call', 'mcp_approval',
  ];
  const fetchImpl = async (url, options) => {
    if (url.endsWith('/snapshot')) {
      snapshotCount += 1;
      return Response.json(snapshotCount === 1 ? {
        entries: [], counters: {
          gatewayExecutorInvocations: 0, networkObserverOutbound: 0, providerExecutions: 0,
        },
      } : {
        entries: observed.map((event, index) => ({ sequence: index + 1, event })),
        counters: {
          gatewayExecutorInvocations: 0, networkObserverOutbound: 0, providerExecutions: 3,
        },
      });
    }
    const body = JSON.parse(options.body);
    requests.push(body);
    const inputTypes = Array.isArray(body.input) ? body.input.map((item) => item.type) : [];
    const toolTypes = Array.isArray(body.tools) ? body.tools.map((tool) => tool.type) : [];
    let data = { type: 'response.completed', response: { id: `resp_fixture_${requests.length}` } };
    if (toolTypes.includes('tool_search')) data = { ...data, fixture: 'tool_search_call' };
    if (inputTypes.includes('tool_search_output')) {
      data = { ...data, fixture: ['tool_search_output', 'additional_tools'] };
    }
    if (toolTypes.includes('file_search')) {
      data = { ...data, fixture: ['file_search_call', 'program', 'shell_call'] };
    }
    if (toolTypes.includes('mcp')) {
      data = {
        type: 'response.created',
        response: {
          id: providerResponseId,
          output: [
            {
              type: 'mcp_list_tools', id: 'mcp_list_provider_owned',
              server_label: 'fixture_mcp', status: 'completed', tools: [{ name: 'lookup' }],
            },
            {
              type: 'mcp_call', id: 'mcp_call_provider_owned', server_label: 'fixture_mcp',
              status: 'completed', name: 'lookup', arguments: '{"query":"fixture"}',
            },
            {
              type: 'mcp_approval_request', id: approvalRequestId, server_label: 'fixture_mcp',
              status: 'in_progress', name: 'lookup', arguments: '{"query":"approval fixture"}',
            },
          ],
        },
      };
    }
    if (inputTypes.includes('mcp_approval_response')) data = { ...data, fixture: 'approval-accepted' };
    return new Response(`data: ${JSON.stringify(data)}\n\n`, {
      status: 200, headers: { 'content-type': 'text/event-stream' },
    });
  };

  await runWireAudit({ manifest: {
    gatewayBaseUrl: 'http://gateway.invalid',
    openai: { api_key: 'fixture-key', model: 'fixture-model' },
    controlledUpstream: {
      snapshotUrl: 'http://upstream.invalid/snapshot',
      networkObserverUrl: 'http://observer.invalid/mcp',
      gatewayExecutorObserverUrl: 'http://observer.invalid/executor',
    },
  } }, { fetchImpl, secretCanary: 'wire-secret-canary' });

  const startIndex = requests.findIndex((body) => body.tools?.some((tool) => tool.type === 'mcp'));
  assert.notEqual(startIndex, -1);
  assert.equal(requests[startIndex].input, 'ordinary user request for an MCP lookup');
  assert.equal(JSON.stringify(requests[startIndex].input).includes('mcp_'), false);
  const continuation = requests[startIndex + 1];
  assert.equal(continuation.previous_response_id, providerResponseId);
  assert.deepEqual(continuation.input, [{
    type: 'mcp_approval_response', approval_request_id: approvalRequestId, approve: true,
  }]);
});

test('Root #1477 AC-001/004/005/006: request audit inventory is finite and fail closed', () => {
  const inventory = requestFidelityInventory();
  assert.equal(inventory.positive_rows.length, 3);
  assert.equal(inventory.negative_rows.length, 4);
  assert.equal(inventory.raw_sinks_forbidden.includes('durable'), true);
  const digest = 'a'.repeat(64);
  const evidence = {
    positive: inventory.positive_rows.map((id) => ({ id, direct_sha256: digest, gateway_sha256: digest })),
    negative: inventory.negative_rows.map((id) => ({ id, failed: true, upstream_arrivals: 0 })),
    ephemeral: {
      preserved_phases: ['initial-invocation', 'tool-callback', 'retry'],
      cleanup_phases: ['terminal-success', 'terminal-failure'],
      missing_before_terminal_failed: true,
    },
  };
  assert.equal(assertRequestFidelityAudit(evidence).verdict, 'PASS');
  evidence.negative[0].upstream_arrivals = 1;
  assert.throws(() => assertRequestFidelityAudit(evidence), /did not fail before upstream/u);
});

test('Root #1477 AC-008: error audit requires 5 fixtures across all 4 public surfaces', () => {
  const inventory = errorFidelityInventory();
  assert.equal(inventory.rows, 20);
  const rows = UPSTREAM_ERROR_FIXTURES.flatMap((fixture) => ERROR_SURFACES.map((surface) => {
    const message = fixture.body || `upstream returned HTTP ${fixture.status}`;
    return {
      fixture: fixture.id, surface, attempts: fixture.attempts,
      native_message: message, durable_message: message, client_message: message,
    };
  }));
  assert.deepEqual(assertErrorFidelityAudit({ rows }), {
    schema_version: '1flowbase.ai-gateway-error-fidelity-result/v1', verdict: 'PASS', rows: 20,
  });
  rows.pop();
  assert.throws(() => assertErrorFidelityAudit({ rows }), /evidence omitted/u);
});
