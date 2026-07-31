'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  ANTHROPIC_CALLBACK_RETRY_ORACLE,
  callbackRetryMockEvidence,
  continuationRequest,
  initialRequest,
} = require('../anthropic-callback-retry');

test('Anthropic callback retry gate reuses one byte-equivalent tool result after 429', () => {
  const initial = initialRequest('published-model', 'stable-correlation');
  const toolUse = {
    type: 'tool_use',
    id: 'toolu_fixture',
    name: 'Read',
    input: { file_path: '/tmp/1flowbase-callback-retry-fixture.txt' },
  };
  const firstRetry = continuationRequest(initial, toolUse);
  const secondRetry = continuationRequest(initial, toolUse);
  assert.deepEqual(firstRetry, secondRetry);
  assert.equal(JSON.stringify(firstRetry).match(/1flowbase-client-tool-result/gu)?.length, 1);
  assert.deepEqual(ANTHROPIC_CALLBACK_RETRY_ORACLE.provider_outcomes, [
    'completed', 'http-429', 'completed',
  ]);
});

test('Anthropic callback retry gate requires one tool call, one rejection, and one recovery', () => {
  const before = { entries: [] };
  const entries = [
    { sequence: 1, event: 'arrival', nonce: 'mock-1' },
    { sequence: 2, event: 'tool_call', nonce: 'mock-1' },
    { sequence: 3, event: 'settled', nonce: 'mock-1', outcome: 'completed' },
    { sequence: 4, event: 'arrival', nonce: 'mock-2' },
    { sequence: 5, event: 'retryable_tool_result_rejection', nonce: 'mock-2' },
    { sequence: 6, event: 'settled', nonce: 'mock-2', outcome: 'http-429' },
    { sequence: 7, event: 'arrival', nonce: 'mock-3' },
    { sequence: 8, event: 'retryable_tool_result_recovered', nonce: 'mock-3' },
    { sequence: 9, event: 'settled', nonce: 'mock-3', outcome: 'completed' },
  ];
  assert.deepEqual(callbackRetryMockEvidence(before, { entries }), {
    outcomes: ['completed', 'http-429', 'completed'],
    client_tool_results: 1,
  });
  assert.throws(
    () => callbackRetryMockEvidence(before, { entries: entries.filter((entry) => entry.sequence !== 8) }),
    /chronology or tool-result cardinality/u,
  );
});
