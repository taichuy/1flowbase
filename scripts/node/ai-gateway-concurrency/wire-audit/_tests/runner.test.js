'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { vectorBodies } = require('../runner');

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
    'programmatic_tool_calling', 'shell', 'mcp_list_tools', 'mcp_call',
    'mcp_approval_request', 'mcp_approval_response', canary,
  ]) assert.match(wire, new RegExp(value, 'u'));
});
