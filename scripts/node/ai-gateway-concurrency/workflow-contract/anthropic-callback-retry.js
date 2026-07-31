'use strict';

const crypto = require('node:crypto');
const { createSseParser, eventText, protocolEventType } = require('../characterize/stream-parsers');
const {
  CALLBACK_RETRY_FINAL_SENTINEL,
  CALLBACK_RETRY_VECTOR_MARKER,
} = require('../mock-upstream/client-vector-contract');
const { targetsFromReady } = require('../local-client-acceptance/contract');
const { reconcileAttempt, snapshotRuns } = require('../local-client-acceptance/durable');

const ANTHROPIC_CALLBACK_RETRY_ORACLE = Object.freeze({
  vector_id: 'tools-callback-retry-after-429',
  provider_outcomes: Object.freeze(['completed', 'http-429', 'completed']),
  durable_statuses: Object.freeze(['failed', 'succeeded']),
  client_tool_results: 1,
});

function requestHeaders(target) {
  return {
    accept: 'text/event-stream',
    authorization: `Bearer ${target.apiKey}`,
    'anthropic-version': '2023-06-01',
    'content-type': 'application/json',
  };
}

async function sendAnthropicTurn(target, body, fetchImpl) {
  const response = await fetchImpl(`${target.gatewayBaseUrl}/v1/messages`, {
    method: 'POST',
    headers: requestHeaders(target),
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(60_000),
  });
  if (!response.ok) {
    const raw = await response.text();
    throw new Error(`Anthropic callback retry returned HTTP ${response.status}: ${raw.slice(0, 500)}`);
  }
  const events = [];
  const parser = createSseParser((event) => events.push(event));
  const reader = response.body.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    parser.push(value);
  }
  parser.finish();
  return events;
}

function initialRequest(model, correlationKey) {
  return {
    model,
    max_tokens: 256,
    stream: true,
    metadata: { user_id: correlationKey },
    messages: [{
      role: 'user',
      content: [{
        type: 'text',
        text: [
          '1flowbase-client-tool-vector',
          CALLBACK_RETRY_VECTOR_MARKER,
          'TOOL_VECTOR_PATH=/tmp/1flowbase-callback-retry-fixture.txt',
          'Use the Read tool exactly once.',
        ].join(' '),
      }],
    }],
    tools: [{
      name: 'Read',
      description: 'Read one controlled local fixture.',
      input_schema: {
        type: 'object',
        properties: { file_path: { type: 'string' } },
        required: ['file_path'],
        additionalProperties: false,
      },
    }],
  };
}

function toolUseFrom(events) {
  const toolUse = events
    .map((event) => event.data?.content_block)
    .find((block) => block?.type === 'tool_use');
  if (!toolUse?.id || !toolUse.name || !toolUse.input) {
    throw new Error('Anthropic callback retry did not receive one complete tool_use block');
  }
  const eventTypes = events.map(protocolEventType);
  if (eventTypes.filter((type) => type === 'message_stop').length !== 1) {
    throw new Error('Anthropic callback retry tool turn omitted its unique message_stop');
  }
  return toolUse;
}

function continuationRequest(initial, toolUse) {
  return {
    ...initial,
    messages: [
      ...initial.messages,
      { role: 'assistant', content: [toolUse] },
      {
        role: 'user',
        content: [{
          type: 'tool_result',
          tool_use_id: toolUse.id,
          content: '1flowbase-client-tool-result',
        }],
      },
    ],
  };
}

function assertRetryableError(events) {
  const errors = events.filter((event) => protocolEventType(event) === 'error');
  if (errors.length !== 1 || errors[0].data?.error?.type !== 'rate_limit_error') {
    throw new Error('Anthropic callback retry did not project one rate_limit_error');
  }
}

function assertRecovered(events) {
  const text = events.map(eventText).filter((value) => typeof value === 'string').join('');
  if (!text.includes(CALLBACK_RETRY_FINAL_SENTINEL)) {
    throw new Error('Anthropic callback retry did not recover its final Provider response');
  }
  if (events.map(protocolEventType).filter((type) => type === 'message_stop').length !== 1) {
    throw new Error('Anthropic callback retry recovery omitted its unique message_stop');
  }
}

function callbackRetryMockEvidence(before, after) {
  const cursor = before?.entries?.at(-1)?.sequence ?? 0;
  const events = (after?.entries ?? []).filter((event) => event.sequence > cursor);
  const arrivals = events.filter((event) => event.event === 'arrival');
  const settled = events.filter((event) => event.event === 'settled');
  const outcomes = arrivals.map((arrival) => (
    settled.find((event) => event.nonce === arrival.nonce)?.outcome
  ));
  const toolCalls = events.filter((event) => event.event === 'tool_call');
  const rejected = events.filter((event) => event.event === 'retryable_tool_result_rejection');
  const recovered = events.filter((event) => event.event === 'retryable_tool_result_recovered');
  if (JSON.stringify(outcomes) !== JSON.stringify(ANTHROPIC_CALLBACK_RETRY_ORACLE.provider_outcomes)) {
    throw new Error(`Anthropic callback retry Provider outcomes were ${JSON.stringify(outcomes)}`);
  }
  if (toolCalls.length !== 1 || rejected.length !== 1 || recovered.length !== 1
    || !(toolCalls[0].sequence < rejected[0].sequence
      && rejected[0].sequence < recovered[0].sequence)) {
    throw new Error('Anthropic callback retry chronology or tool-result cardinality was invalid');
  }
  return { outcomes, client_tool_results: 1 };
}

async function verifyAnthropicCallbackRetry({ ready, mockSnapshot }, dependencies = {}) {
  const fetchImpl = dependencies.fetchImpl ?? globalThis.fetch;
  const target = targetsFromReady(ready).claude;
  const durableBefore = await snapshotRuns(target, fetchImpl);
  const mockBefore = await mockSnapshot();
  const initial = initialRequest(target.model, `callback-retry-${crypto.randomUUID()}`);
  const toolUse = toolUseFrom(await sendAnthropicTurn(target, initial, fetchImpl));
  const continuation = continuationRequest(initial, toolUse);
  assertRetryableError(await sendAnthropicTurn(target, continuation, fetchImpl));
  assertRecovered(await sendAnthropicTurn(target, continuation, fetchImpl));
  const mock = callbackRetryMockEvidence(mockBefore, await mockSnapshot());
  const durable = await reconcileAttempt({
    target,
    before: durableBefore,
    expectedRuns: 2,
    expectedStatuses: ANTHROPIC_CALLBACK_RETRY_ORACLE.durable_statuses,
    fetchImpl,
  });
  return {
    verdict: 'PASS',
    vector_id: ANTHROPIC_CALLBACK_RETRY_ORACLE.vector_id,
    provider_outcomes: mock.outcomes,
    durable_statuses: ANTHROPIC_CALLBACK_RETRY_ORACLE.durable_statuses,
    client_tool_results: mock.client_tool_results,
    durable_runs: durable.expected_runs,
  };
}

module.exports = {
  ANTHROPIC_CALLBACK_RETRY_ORACLE,
  callbackRetryMockEvidence,
  continuationRequest,
  initialRequest,
  verifyAnthropicCallbackRetry,
};
