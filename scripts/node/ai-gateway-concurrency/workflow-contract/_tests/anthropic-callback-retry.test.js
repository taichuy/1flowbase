'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  ANTHROPIC_CALLBACK_RETRY_ORACLE,
  callbackRetryMockEvidence,
  continuationRequest,
  initialRequest,
  verifyAnthropicCallbackRetry,
} = require('../anthropic-callback-retry');
const { CALLBACK_RETRY_FINAL_SENTINEL } = require('../../mock-upstream/client-vector-contract');

const TOOL_INPUT = { file_path: '/tmp/1flowbase-callback-retry-fixture.txt' };

function streamedToolEvents() {
  return [
    { type: 'message_start', message: { role: 'assistant', content: [] } },
    { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } },
    { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: 'Reading.' } },
    { type: 'content_block_stop', index: 0 },
    { type: 'content_block_start', index: 1, content_block: {
      type: 'tool_use', id: 'toolu_fixture', name: 'Read', input: {},
    } },
    { type: 'content_block_delta', index: 1, delta: {
      type: 'input_json_delta', partial_json: '{"file_path":"/tmp/1flowbase-',
    } },
    { type: 'content_block_delta', index: 1, delta: {
      type: 'input_json_delta', partial_json: 'callback-retry-fixture.txt"}',
    } },
    { type: 'content_block_stop', index: 1 },
    { type: 'message_delta', delta: { stop_reason: 'tool_use' } },
    { type: 'message_stop' },
  ];
}

function sseResponse(events) {
  const bytes = new TextEncoder().encode(events.map((event) => (
    `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`
  )).join(''));
  return new Response(new ReadableStream({
    start(controller) {
      for (let offset = 0; offset < bytes.length; offset += 17) {
        controller.enqueue(bytes.slice(offset, offset + 17));
      }
      controller.close();
    },
  }));
}

async function verifyFixture(toolEvents, { retryError = 'rate_limit_error', finalText = CALLBACK_RETRY_FINAL_SENTINEL } = {}) {
  const requests = [];
  const entries = [];
  const runs = [{ id: 'failed-run', status: 'failed' }, { id: 'recovered-run', status: 'succeeded' }];
  const provider = {
    application_id: 'fixture-application', model: 'fixture', api_key: 'fixture-key',
    durable: {
      list_runs: { url: 'https://fixture.test/runs' },
      query_run: { url_template: 'https://fixture.test/runs/{run_id}' },
    },
  };
  const result = await verifyAnthropicCallbackRetry({
    ready: {
      schema_version: '1flowbase.ai-gateway-fixture/v1',
      gateway_base_url: 'https://fixture.test',
      targets: { openai: provider, anthropic: provider, openai_compatible: provider },
    },
    mockSnapshot: async () => ({ entries: [...entries] }),
  }, {
    fetchImpl: async (url, options) => {
      if (url.endsWith('/runs')) {
        return Response.json({ items: requests.length === 0 ? [] : runs });
      }
      const run = runs.find((candidate) => url.endsWith(`/runs/${candidate.id}`));
      if (run) return Response.json(run);
      assert.equal(url, 'https://fixture.test/v1/messages');
      requests.push(options.body);
      const turn = requests.length;
      const nonce = `mock-${turn}`;
      const record = (event, extra = {}) => entries.push({ sequence: entries.length + 1, event, nonce, ...extra });
      record('arrival');
      record(['tool_call', 'retryable_tool_result_rejection', 'retryable_tool_result_recovered'][turn - 1]);
      record('settled', { outcome: turn === 2 ? 'http-429' : 'completed' });
      if (turn === 1) return sseResponse(toolEvents);
      const body = JSON.parse(options.body);
      assert.deepEqual(body.messages.at(-2).content, [{
        type: 'tool_use', id: 'toolu_fixture', name: 'Read', input: TOOL_INPUT,
      }]);
      assert.deepEqual(body.messages.at(-1).content, [{
        type: 'tool_result', tool_use_id: 'toolu_fixture', content: '1flowbase-client-tool-result',
      }]);
      if (turn === 2) return sseResponse([{ type: 'error', error: { type: retryError } }]);
      assert.equal(options.body, requests[1]);
      return sseResponse([
        { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: finalText } },
        { type: 'message_stop' },
      ]);
    },
  });
  assert.equal(requests.length, 3);
  assert.equal(result.verdict, 'PASS');
  assert.equal(result.durable_runs, 2);
  assert.deepEqual(result.provider_outcomes, ['completed', 'http-429', 'completed']);
  return result;
}

test('full callback retry gate assembles indexed streamed JSON before replaying identical requests', async () => {
  await verifyFixture(streamedToolEvents());
});

test('full callback retry gate also consumes tool input completed in the start block', async () => {
  const events = streamedToolEvents().filter((event) => event.delta?.type !== 'input_json_delta');
  events.find((event) => event.content_block?.type === 'tool_use').content_block.input = TOOL_INPUT;
  await verifyFixture(events);
});

test('full callback retry gate rejects incomplete, misindexed, malformed, or duplicate tool blocks', async (t) => {
  for (const variant of ['missing_stop', 'wrong_stop', 'wrong_delta_index', 'delta_after_stop', 'malformed_json', 'duplicate_tool', 'duplicate_message_stop']) {
    await t.test(variant, async () => {
      const events = streamedToolEvents();
      if (variant === 'missing_stop') events.splice(7, 1);
      if (variant === 'wrong_stop') events[7].index = 2;
      if (variant === 'wrong_delta_index') events[5].index = 2;
      if (variant === 'delta_after_stop') [events[6], events[7]] = [events[7], events[6]];
      if (variant === 'malformed_json') events[6].delta.partial_json = 'invalid JSON';
      if (variant === 'duplicate_tool') events.splice(8, 0, {
        type: 'content_block_start', index: 2,
        content_block: { type: 'tool_use', id: 'toolu_second', name: 'Read', input: {} },
      });
      if (variant === 'duplicate_message_stop') events.push({ type: 'message_stop' });
      await assert.rejects(verifyFixture(events), /Anthropic callback retry.*(tool_use|message_stop)/u);
    });
  }
});

test('full callback retry gate retains the retryable error and final sentinel requirements', async () => {
  await assert.rejects(
    verifyFixture(streamedToolEvents(), { retryError: 'authentication_error' }),
    /did not project one rate_limit_error/u,
  );
  await assert.rejects(
    verifyFixture(streamedToolEvents(), { finalText: 'unrelated output' }),
    /did not recover its final Provider response/u,
  );
});

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
